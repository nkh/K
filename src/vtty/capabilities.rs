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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_capabilities() {
        let cap = Capabilities::default();
        assert!(cap.truecolor);
        assert!(cap.color256);
        assert!(cap.color16);
        assert!(!cap.mouse);
        assert!(cap.alternate_screen);
        assert!(cap.bracketed_paste);
        assert!(!cap.focus_events);
        assert!(cap.unicode);
        assert!(cap.cursor_style);
        assert!(cap.window_title);
    }

    #[test]
    fn test_capabilities_custom() {
        let cap = Capabilities {
            truecolor: false,
            color256: true,
            color16: true,
            mouse: true,
            alternate_screen: false,
            bracketed_paste: false,
            focus_events: true,
            unicode: true,
            cursor_style: false,
            window_title: false,
        };
        assert!(!cap.truecolor);
        assert!(cap.mouse);
        assert!(!cap.alternate_screen);
        assert!(cap.focus_events);
        assert!(!cap.cursor_style);
        assert!(!cap.window_title);
    }

    #[test]
    fn test_capabilities_clone() {
        let cap = Capabilities::default();
        let cloned = cap;
        assert_eq!(cap.truecolor, cloned.truecolor);
        assert_eq!(cap.mouse, cloned.mouse);
    }

    #[test]
    fn test_capabilities_copy() {
        let cap = Capabilities::default();
        let copied = cap;
        // Copy trait allows assigning without move
        let _another = copied;
    }

    #[test]
    fn test_capabilities_debug() {
        let cap = Capabilities::default();
        let debug_str = format!("{:?}", cap);
        assert!(debug_str.contains("Capabilities"));
        assert!(debug_str.contains("truecolor"));
        assert!(debug_str.contains("mouse"));
    }

    #[test]
    fn test_capabilities_minimal() {
        let cap = Capabilities {
            truecolor: false,
            color256: false,
            color16: false,
            mouse: false,
            alternate_screen: false,
            bracketed_paste: false,
            focus_events: false,
            unicode: false,
            cursor_style: false,
            window_title: false,
        };
        assert!(!cap.truecolor);
        assert!(!cap.color256);
        assert!(!cap.color16);
        assert!(!cap.mouse);
        assert!(!cap.alternate_screen);
        assert!(!cap.bracketed_paste);
        assert!(!cap.focus_events);
        assert!(!cap.unicode);
        assert!(!cap.cursor_style);
        assert!(!cap.window_title);
    }
}
