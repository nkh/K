//! Token-bucket rate limiter for VTTY output notifications.
//!
//! Provides a simple, synchronous rate limiter used to throttle how often
//! VTTY buffer-change notifications are sent to WebSocket clients.
//!
//! # Design
//!
//! Uses a **token bucket** algorithm:
//! - The bucket holds a fractional number of tokens (up to `max_tokens`).
//! - Each [`RateLimiter::allow`] call consumes one token.
//! - Tokens are continuously refilled at `max_rate` tokens/second based on
//!   wall-clock elapsed time since the last refill.
//!
//! When the rate limiter denies a request (no tokens available), the caller
//! is expected to buffer the latest state and retry on the next periodic
//! flush tick.
//!
//! # Thread safety
//!
//! `RateLimiter` is intentionally **not** `Sync` — it uses `&mut self`
//! methods and is designed to be owned exclusively by a single async task
//! (the PTY output consumer in the spawner). This avoids any locking
//! overhead.

use std::time::{Duration, Instant};

/// Maximum burst size: allow a small burst of notifications when the
/// system has been idle, but cap it to prevent a sudden flood.
const MAX_BURST_TOKENS: f64 = 3.0;

/// A token-bucket rate limiter.
///
/// ```text
///   PTY output arrives → rate_limiter.allow()?
///       YES → notify sinks immediately
///       NO  → buffer latest snapshot, flush on next tick
/// ```
///
/// # Examples
///
/// ```
/// use vrc_core::vtty::rate_limiter::RateLimiter;
///
/// let mut limiter = RateLimiter::new(30); // 30 updates/sec
/// assert!(limiter.allow()); // first call always succeeds
/// ```
pub struct RateLimiter {
    /// Maximum sustained rate (tokens per second).
    max_rate: u32,
    /// Current number of tokens in the bucket.
    tokens: f64,
    /// Last time tokens were refilled.
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// * `max_updates_per_sec` — target sustained rate. A value of `0`
    ///   disables rate limiting (every call to [`allow`](Self::allow)
    ///   returns `true`).
    ///
    /// # Panics
    ///
    /// Panics if `max_updates_per_sec` is `0` (use
    /// [`disabled`](Self::disabled) instead).
    #[track_caller]
    pub fn new(max_updates_per_sec: u32) -> Self {
        assert!(
            max_updates_per_sec > 0,
            "max_updates_per_sec must be > 0; use RateLimiter::disabled() for unlimited"
        );
        Self {
            max_rate: max_updates_per_sec,
            tokens: 1.0, // Allow the first notification immediately
            last_refill: Instant::now(),
        }
    }

    /// Create a rate limiter that always allows (no limiting).
    ///
    /// Useful as a default when rate limiting is disabled via config.
    pub fn disabled() -> Self {
        Self {
            max_rate: u32::MAX,
            tokens: f64::MAX,
            last_refill: Instant::now(),
        }
    }

    /// Create a rate limiter from config.
    ///
    /// * `max_updates_per_sec` — if `0` or `None`, rate limiting is disabled.
    /// * Otherwise, the value is used as the max sustained rate.
    pub fn from_config(max_updates_per_sec: u32) -> Self {
        if max_updates_per_sec == 0 {
            Self::disabled()
        } else {
            Self::new(max_updates_per_sec)
        }
    }

    /// Attempt to consume one token.
    ///
    /// Returns `true` if a notification is allowed (token was available),
    /// `false` if the rate limit has been exceeded. In the latter case,
    /// the caller should buffer the latest snapshot and retry later.
    pub fn allow(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Peek at whether a token is available without consuming it.
    pub fn peek(&mut self) -> bool {
        self.refill();
        self.tokens >= 1.0
    }

    /// The interval between allowed notifications at the configured rate.
    ///
    /// Returns `Duration::ZERO` if rate limiting is disabled.
    pub fn interval(&self) -> Duration {
        if self.max_rate == u32::MAX {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(1.0 / self.max_rate as f64)
        }
    }

    /// The configured maximum sustained rate (updates per second).
    pub fn max_rate(&self) -> u32 {
        self.max_rate
    }

    /// Whether rate limiting is effectively disabled.
    pub fn is_disabled(&self) -> bool {
        self.max_rate == u32::MAX
    }

    /// Refill tokens based on elapsed time.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;

        if self.max_rate == u32::MAX {
            return; // Unlimited — no refill needed
        }

        // Refill tokens: elapsed_seconds * max_rate
        self.tokens += elapsed * self.max_rate as f64;

        // Cap to burst size to prevent accumulation during idle periods
        if self.tokens > MAX_BURST_TOKENS {
            self.tokens = MAX_BURST_TOKENS;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_rate_limiter_throttles() {
        // 10 updates/sec = 100ms between allowed updates.
        let mut limiter = RateLimiter::new(10);

        // First call consumes the initial token.
        assert!(limiter.allow());

        // Immediate second call should be denied (no time has passed).
        assert!(!limiter.allow());
        assert!(!limiter.allow());
    }

    #[test]
    fn test_rate_limiter_refills_over_time() {
        let mut limiter = RateLimiter::new(100); // 10ms per token

        // Consume the initial token.
        assert!(limiter.allow());

        // Wait long enough for one token to refill.
        thread::sleep(Duration::from_millis(20));

        // Should have refilled enough for one token.
        assert!(limiter.allow());
    }

    #[test]
    fn test_disabled_allows_all() {
        let mut limiter = RateLimiter::disabled();
        for _ in 0..1000 {
            assert!(limiter.allow());
        }
    }

    #[test]
    fn test_peek_does_not_consume() {
        let mut limiter = RateLimiter::new(10);
        assert!(limiter.peek());
        assert!(limiter.peek());
        // Both peeks succeed without consuming — allow still works
        assert!(limiter.allow());
    }

    #[test]
    fn test_burst_cap() {
        // Create limiter, let lots of time pass, then check tokens are capped.
        let mut limiter = RateLimiter::new(10);
        // Simulate a lot of time passing by manipulating last_refill.
        limiter.last_refill = Instant::now() - Duration::from_secs(10);
        limiter.refill();
        // After 10 seconds at 10 tokens/sec, raw refill would be 100 tokens,
        // but it should be capped at MAX_BURST_TOKENS (3.0).
        assert!(limiter.tokens <= MAX_BURST_TOKENS + 0.01);
    }
}
