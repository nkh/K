/// Terminal capability flags.
#[derive(Debug, Clone, Copy)]
pub struct Capabilities {
    /// Support for 24-bit truecolor (RGB)
    pub truecolor: bool,
    /// Support for 256 colors
    pub color256: bool,
    /// Support for 16 ANSI colors
    pub color16: bool,
    /// Mouse reporting support
    pub mouse: bool,
    /// Alternate screen buffer support
    pub alternate_screen: bool,
    /// Bracketed paste mode
    pub bracketed_paste: bool,
    /// Focus event reporting
    pub focus_events: bool,
    /// Unicode wide character support
    pub unicode: bool,
    /// Cursor style change support
    pub cursor_style: bool,
    /// Window title setting support
    pub window_title: bool,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            truecolor: true,
            color256: true,
            color16: true,
            mouse: false,
            alternate_screen: true,
            bracketed_paste: true,
            focus_events: false,
            unicode: true,
            cursor_style: true,
            window_title: true,
        }
    }
}
