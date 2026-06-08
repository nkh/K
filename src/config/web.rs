#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use serde::{Deserialize, Serialize};

/// Web admin panel and VTTY streaming configuration.
///
/// Controls how the web UI discovers that a terminal buffer has changed.
/// Two update modes are supported:
///
/// - **push** (default): The server detects buffer changes via a periodic
///   dirty-check loop and sends lightweight "dirty" signals over the
///   existing WebSocket connection.  The client then fetches fresh HTML
///   at its own pace (debounced).  This is the most efficient mode
///   because no polling is required — the server only sends when
///   something actually changed.
///
/// - **poll**: The web client periodically calls the
///   `GET /api/commands/:id/vtty/changed` endpoint to ask "has the
///   buffer changed since last time?".  If yes, the client fetches
///   the full HTML.  This mode is useful when WebSocket connections
///   are unreliable (e.g. reverse proxies that buffer frames) or
///   when the client wants full control over refresh timing.
///
/// The dirty-check interval (`dirty_check_ms`) only affects server-side
/// behaviour in push mode — it controls how often the server compares
/// the current buffer against the last-sent snapshot.
///
/// Example YAML:
/// ```yaml
/// web:
///   update_mode: push       # or "poll"
///   dirty_check_ms: 200     # server check interval (push mode)
///   default_poll_ms: 500    # client poll interval (poll mode)
///   rate_limit:
///     max_updates_per_sec: 30
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebConfig {
    /// How the web UI discovers buffer changes: "push" or "poll".
    /// Default: "push".
    pub update_mode: String,
    /// Server-side dirty-check interval in milliseconds.
    /// Only relevant in push mode. The server compares the VTTY buffer
    /// against the last-sent snapshot at this interval and sends a
    /// "vtty_dirty" WebSocket message when changes are detected.
    /// Default: 200 ms.
    pub dirty_check_ms: u64,
    /// Default client-side polling interval in milliseconds.
    /// Only relevant in poll mode. The web UI will poll
    /// `GET /api/commands/:id/vtty/changed` at this interval.
    /// The user can override this via the web UI controls.
    /// Default: 500 ms.
    pub default_poll_ms: u64,
    /// Rate limiting for VTTY update notifications sent to WebSocket clients.
    ///
    /// When a command produces very high output (e.g., `find /`, build logs),
    /// the server can flood clients with buffer-change notifications.  This
    /// configures a token-bucket rate limiter that throttles how often
    /// notifications are sent per command.
    ///
    /// When the rate is exceeded, intermediate buffer snapshots are buffered
    /// and the **latest** state is sent on the next allowed tick, ensuring
    /// the client always receives the most recent terminal content.
    ///
    /// Set `max_updates_per_sec` to `0` to disable rate limiting entirely
    /// (every buffer change triggers an immediate notification).
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            update_mode: "push".to_string(),
            dirty_check_ms: 200,
            default_poll_ms: 500,
            rate_limit: RateLimitConfig::default(),
        }
    }
}

/// Rate limiting configuration for VTTY output notifications.
///
/// Uses a token-bucket algorithm to throttle how often buffer-change
/// notifications are sent to WebSocket clients per command.
///
/// # YAML Example
///
/// ```yaml
/// web:
///   rate_limit:
///     max_updates_per_sec: 30
/// ```
///
/// # How it works
///
/// 1. Each command has its own rate limiter instance.
/// 2. When the PTY produces output, the emulator processes it and a
///    notification would normally be sent immediately.
/// 3. The rate limiter checks if a token is available:
///    - **Yes**: notification is sent immediately.
///    - **No**: the latest buffer snapshot is held in a pending buffer.
/// 4. A periodic flush timer (at the configured interval) sends any
///    pending buffered update, ensuring the client always receives the
///    most recent terminal state — just not every intermediate one.
///
/// The default of 30 updates/sec is sufficient for smooth terminal
/// rendering while preventing flood from high-output commands.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateLimitConfig {
    /// Maximum number of VTTY update notifications per second per command.
    ///
    /// Controls the sustained rate at which buffer-change notifications are
    /// sent to WebSocket clients.  A small burst (up to 3 notifications)
    /// is allowed when the system has been idle.
    ///
    /// - `30` (default): good balance of smoothness and bandwidth savings.
    ///   Terminal output at 30fps is indistinguishable from 60fps for
    ///   most text content.
    /// - `0`: disable rate limiting entirely (every change is sent
    ///   immediately).  Useful for latency-sensitive commands.
    /// - `10`: aggressive throttling for bandwidth-constrained environments.
    #[serde(default = "default_max_updates_per_sec")]
    pub max_updates_per_sec: u32,
}

fn default_max_updates_per_sec() -> u32 {
    30
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_updates_per_sec: default_max_updates_per_sec(),
        }
    }
}
