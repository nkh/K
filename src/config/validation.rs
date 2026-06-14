use std::path::Path;

use super::schema::Config;

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub field: String,
    pub level: ValidationLevel,
    pub message: String,
}

/// Check that the parent directory of `path` exists; push a warning if not.
fn check_parent_dir(path: &str, field: &str, issues: &mut Vec<ValidationIssue>) {
    if path.is_empty() {
        return;
    }
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            issues.push(ValidationIssue {
                field: field.into(),
                level: ValidationLevel::Warning,
                message: format!("Parent directory does not exist: {}", parent.display()),
            });
        }
    }
}

/// Validate a [`Config`] and return all issues found.
pub fn validate_config(config: &Config) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    // ── server settings (vrw only) ──
    #[cfg(feature = "vrw")]
    {
        if config.server.port == 0 {
            issues.push(ValidationIssue {
                field: "server.port".into(),
                level: ValidationLevel::Error,
                message: "Server port must not be 0".into(),
            });
        }
        if config.server.bind.is_empty() {
            issues.push(ValidationIssue {
                field: "server.bind".into(),
                level: ValidationLevel::Error,
                message: "Server bind address must not be empty".into(),
            });
        }
    }

    // ── vtty dimensions ──
    if config.vtty.rows == 0 {
        issues.push(ValidationIssue {
            field: "vtty.rows".into(),
            level: ValidationLevel::Error,
            message: "VTTY rows must be >= 1".into(),
        });
    }
    if config.vtty.cols == 0 {
        issues.push(ValidationIssue {
            field: "vtty.cols".into(),
            level: ValidationLevel::Error,
            message: "VTTY cols must be >= 1".into(),
        });
    }

    // ── display.refresh_ms ──
    if config.display.refresh_ms < 10 {
        issues.push(ValidationIssue {
            field: "display.refresh_ms".into(),
            level: ValidationLevel::Error,
            message: format!(
                "Refresh interval must be >= 10 ms, got {} ms",
                config.display.refresh_ms
            ),
        });
    }

    // ── parent directory checks ──
    check_parent_dir(&config.daemon.stdout_file, "daemon.stdout_file", &mut issues);
    check_parent_dir(&config.daemon.stderr_file, "daemon.stderr_file", &mut issues);
    if let Some(ref f) = config.command_log.file {
        check_parent_dir(f, "command_log.file", &mut issues);
    }
    if let Some(ref f) = config.command_log.pty_raw_log {
        check_parent_dir(f, "command_log.pty_raw_log", &mut issues);
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> Config {
        Config::default()
    }

    #[test]
    fn test_valid_config_no_errors() {
        let issues = validate_config(&default_config());
        assert!(issues.iter().all(|i| i.level != ValidationLevel::Error));
    }

    #[test]
    fn test_invalid_vtty_dims() {
        let mut config = default_config();
        config.vtty.rows = 0;
        config.vtty.cols = 0;
        let issues = validate_config(&config);
        let vtty: Vec<_> = issues.iter().filter(|i| i.field.starts_with("vtty")).collect();
        assert_eq!(vtty.len(), 2);
        assert!(vtty.iter().all(|i| i.level == ValidationLevel::Error));
    }

    #[test]
    fn test_refresh_ms_too_low() {
        let mut config = default_config();
        config.display.refresh_ms = 1;
        let issues = validate_config(&config);
        assert!(issues.iter().any(|i| i.field == "display.refresh_ms"));
    }

    #[test]
    fn test_daemon_stdout_missing_parent_warns() {
        let mut config = default_config();
        config.daemon.stdout_file = "/nonexistent/dir/stdout.log".into();
        let issues = validate_config(&config);
        let out: Vec<_> = issues.iter().filter(|i| i.field == "daemon.stdout_file").collect();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].level, ValidationLevel::Warning);
    }
}
