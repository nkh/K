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
    /// Default font size for PNG screenshots (vrw only).
    #[cfg(feature = "vrw")]
    pub screenshot_font_size: f32,
    /// Default font file path for PNG screenshots (vrw only).
    #[cfg(feature = "vrw")]
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
            #[cfg(feature = "vrw")]
            screenshot_font_size: 14.0,
            #[cfg(feature = "vrw")]
            screenshot_font_name: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_vtty_config() {
        let cfg = VttyConfig::default();
        assert_eq!(cfg.rows, 24);
        assert_eq!(cfg.cols, 80);
        assert_eq!(cfg.term, "xterm-256color");
        assert_eq!(cfg.scrollback, 5000);
        assert!(cfg.truecolor);
        assert!(!cfg.mouse);
    }

    #[test]
    fn test_vtty_config_custom() {
        let cfg = VttyConfig {
            rows: 50,
            cols: 160,
            term: "xterm-kitty".to_string(),
            scrollback: 10000,
            truecolor: false,
            mouse: true,
            #[cfg(feature = "vrw")]
            screenshot_font_size: 18.0,
            #[cfg(feature = "vrw")]
            screenshot_font_name: Some("FiraCode.ttf".to_string()),
        };
        assert_eq!(cfg.rows, 50);
        assert_eq!(cfg.cols, 160);
        assert_eq!(cfg.term, "xterm-kitty");
        assert_eq!(cfg.scrollback, 10000);
        assert!(!cfg.truecolor);
        assert!(cfg.mouse);
    }

    #[test]
    fn test_vtty_config_serialization_roundtrip() {
        let cfg = VttyConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: VttyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.rows, cfg.rows);
        assert_eq!(deserialized.cols, cfg.cols);
        assert_eq!(deserialized.term, cfg.term);
        assert_eq!(deserialized.scrollback, cfg.scrollback);
        assert_eq!(deserialized.truecolor, cfg.truecolor);
        assert_eq!(deserialized.mouse, cfg.mouse);
    }

    #[test]
    fn test_vtty_config_debug_clone() {
        let cfg = VttyConfig::default();
        let cloned = cfg.clone();
        assert_eq!(cfg.rows, cloned.rows);
        assert_eq!(cfg.cols, cloned.cols);
        let debug_str = format!("{:?}", cfg);
        assert!(debug_str.contains("VttyConfig"));
    }
}
