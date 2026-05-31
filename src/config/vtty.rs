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
    /// Default font size for PNG screenshots (vrunner only).
    #[cfg(feature = "vrunner")]
    pub screenshot_font_size: f32,
    /// Default font file path for PNG screenshots (vrunner only).
    #[cfg(feature = "vrunner")]
    pub screenshot_font_name: Option<String>,
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
            #[cfg(feature = "vrunner")]
            screenshot_font_size: 14.0,
            #[cfg(feature = "vrunner")]
            screenshot_font_name: None,
        }
    }
}
