use std::io::IsTerminal;

use crossterm::style::{Color, Stylize};

/// Colorize text using crossterm when stdout is a TTY, plain text otherwise.
pub fn c(text: &str, color: Color, bold: bool) -> String {
    if !std::io::stdout().is_terminal() {
        return text.to_string();
    }
    let styled = text.with(color);
    if bold {
        styled.bold().to_string()
    } else {
        styled.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_plain_when_not_tty() {
        // Just verify it returns a string without panic
        let result = c("test", Color::Red, true);
        assert!(result.contains("test"));
    }
}
