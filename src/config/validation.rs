use std::path::Path;

use super::schema::Config;

/// Severity level for a configuration validation issue.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationLevel {
    /// A fatal issue that prevents startup.
    Error,
    /// A non-fatal issue that is logged but does not prevent startup.
    Warning,
}

/// A single configuration validation finding.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Dot-separated path to the offending field (e.g. "vtty.rows").
    pub field: String,
    /// Severity of the issue.
    pub level: ValidationLevel,
    /// Human-readable description of the problem.
    pub message: String,
}

/// Validate a [`Config`] and return all issues found.
///
/// Errors are fatal (e.g. dimensions out of range) and should cause the
/// program to abort.  Warnings are non-fatal and are logged but startup
/// continues.
pub fn validate_config(config: &Config) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    // ── server settings (vrunner only) ──────────────────────────
    #[cfg(feature = "vrunner")]
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

    // ── vtty dimensions ──────────────────────────────────────────
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

    // ── display.refresh_ms ───────────────────────────────────────
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

    // ── daemon stdout / stderr parent dir ────────────────────────
    if !config.daemon.stdout_file.is_empty() {
        if let Some(parent) = Path::new(&config.daemon.stdout_file).parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                issues.push(ValidationIssue {
                    field: "daemon.stdout_file".into(),
                    level: ValidationLevel::Warning,
                    message: format!(
                        "Parent directory for daemon stdout does not exist: {}",
                        parent.display()
                    ),
                });
            }
        }
    }
    if !config.daemon.stderr_file.is_empty() {
        if let Some(parent) = Path::new(&config.daemon.stderr_file).parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                issues.push(ValidationIssue {
                    field: "daemon.stderr_file".into(),
                    level: ValidationLevel::Warning,
                    message: format!(
                        "Parent directory for daemon stderr does not exist: {}",
                        parent.display()
                    ),
                });
            }
        }
    }

    // ── command_log.file parent dir ──────────────────────────────
    if let Some(ref log_file) = config.command_log.file {
        if !log_file.is_empty() {
            if let Some(parent) = Path::new(log_file).parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    issues.push(ValidationIssue {
                        field: "command_log.file".into(),
                        level: ValidationLevel::Warning,
                        message: format!(
                            "Parent directory for command log does not exist: {}",
                            parent.display()
                        ),
                    });
                }
            }
        }
    }

    // ── command_log.pty_raw_log parent dir ───────────────────────
    if let Some(ref pty_log) = config.command_log.pty_raw_log {
        if !pty_log.is_empty() {
            if let Some(parent) = Path::new(pty_log).parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    issues.push(ValidationIssue {
                        field: "command_log.pty_raw_log".into(),
                        level: ValidationLevel::Warning,
                        message: format!(
                            "Parent directory for PTY raw log does not exist: {}",
                            parent.display()
                        ),
                    });
                }
            }
        }
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
    fn test_valid_config_produces_no_issues() {
        let config = default_config();
        let issues = validate_config(&config);
        let errors: Vec<_> = issues
            .iter()
            .filter(|i| i.level == ValidationLevel::Error)
            .collect();
        assert!(
            errors.is_empty(),
            "Default config should produce no errors, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_invalid_vtty_dims() {
        let mut config = default_config();
        config.vtty.rows = 0;
        config.vtty.cols = 0;
        let issues = validate_config(&config);
        let vtty_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.field.starts_with("vtty"))
            .collect();
        assert_eq!(vtty_issues.len(), 2);
        assert!(vtty_issues
            .iter()
            .all(|i| i.level == ValidationLevel::Error));
    }

    #[test]
    fn test_refresh_ms_too_low() {
        let mut config = default_config();
        config.display.refresh_ms = 1;
        let issues = validate_config(&config);
        let refresh_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.field == "display.refresh_ms")
            .collect();
        assert_eq!(refresh_issues.len(), 1);
        assert_eq!(refresh_issues[0].level, ValidationLevel::Error);
    }

    #[test]
    fn test_refresh_ms_boundary() {
        let mut config = default_config();
        config.display.refresh_ms = 10;
        let issues = validate_config(&config);
        assert!(
            issues.iter().all(|i| i.field != "display.refresh_ms"),
            "refresh_ms = 10 should be valid (boundary)"
        );
    }

    #[test]
    fn test_daemon_stdout_missing_parent_warns() {
        let mut config = default_config();
        config.daemon.stdout_file = "/nonexistent/dir/stdout.log".into();
        let issues = validate_config(&config);
        let daemon_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.field == "daemon.stdout_file")
            .collect();
        assert_eq!(daemon_issues.len(), 1);
        assert_eq!(daemon_issues[0].level, ValidationLevel::Warning);
    }
}
