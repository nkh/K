use std::io::IsTerminal;

use crossterm::style::{Color, Stylize};

use crate::instance::info::InstanceInfo;

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

/// Format an instance list as a table string.
pub fn format_instance_list(instances: &[InstanceInfo]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<10} {:<20} {:<10} {:<10} COMMAND\n",
        "PID", "BIND", "DAEMON", "DISPLAY"
    ));
    for info in instances {
        out.push_str(&format!(
            "{:<10} {:<20} {:<10} {:<10} {}\n",
            info.pid,
            info.bind,
            if info.daemon { "yes" } else { "no" },
            if info.display { "yes" } else { "no" },
            info.command.as_deref().unwrap_or("(idle)")
        ));
    }
    out
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
