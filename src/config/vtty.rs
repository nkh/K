use serde::{Deserialize, Serialize};

/// Virtual terminal configuration.
/// Controls the dimensions, TERM value, and capabilities of the pseudo-terminal
/// allocated for each spawned command.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VttyConfig {
    /// Number of rows in the virtual terminal.
    pub rows: u16,
    /// Number of columns in the virtual terminal.
    pub cols: u16,
    /// The TERM value reported to child processes.
    pub term: String,
    /// Maximum number of scrollback lines retained.
    pub scrollback: usize,
    /// Enable 24-bit truecolor support.
    pub truecolor: bool,
    /// Enable mouse event forwarding.
    pub mouse: bool,
    /// Default font size (in points) for PNG screenshot rendering.
    /// Used by the `vrunner screenshot` CLI command and the web UI
    /// screenshot button when no explicit size is specified.
    /// Default: 12.
    #[serde(default = "default_screenshot_font_size")]
    pub screenshot_font_size: f32,
    /// Default font name/path for PNG screenshot rendering.
    /// When set to "monospace" (the default), the renderer searches common
    /// system paths for a monospace TTF font. Set to an absolute path to
    /// use a specific font file.
    /// Default: "monospace".
    #[serde(default = "default_screenshot_font_name")]
    pub screenshot_font_name: String,
}

fn default_screenshot_font_size() -> f32 {
    12.0
}

fn default_screenshot_font_name() -> String {
    "monospace".to_string()
}

impl Default for VttyConfig {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            term: "xterm-256color".to_string(),
            scrollback: 5000,
            truecolor: true,
            mouse: false,
            screenshot_font_size: default_screenshot_font_size(),
            screenshot_font_name: default_screenshot_font_name(),
        }
    }
}
