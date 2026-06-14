#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use serde::{Deserialize, Serialize};

/// Web admin panel and VTTY streaming. update_mode: "push" or "poll".
/// Breaking change: `web.rate_limit.max_updates_per_sec` → `web.max_updates_per_sec`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebConfig {
    /// How the web UI discovers buffer changes: "push" or "poll".
    /// Default: "push".
    pub update_mode: String,
    /// Server-side dirty-check interval in ms (push mode). Default: 200.
    pub dirty_check_ms: u64,
    /// Client-side polling interval in ms (poll mode). Default: 500.
    pub default_poll_ms: u64,
    /// Per-server panel header colors. Empty = built-in dark palette.
    #[serde(default)]
    pub panel_colors: Vec<PanelColorEntry>,
    /// Max VTTY updates/sec/command. 0 = disabled.
    #[serde(default = "default_max_updates_per_sec")]
    pub max_updates_per_sec: u32,
}

/// A (background, text) color pair for panel headers.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PanelColorEntry {
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub text: String,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            update_mode: "push".to_string(),
            dirty_check_ms: 200,
            default_poll_ms: 500,
            panel_colors: Vec::new(), // empty = use built-in palette
            max_updates_per_sec: default_max_updates_per_sec(),
        }
    }
}

fn default_max_updates_per_sec() -> u32 {
    30
}


