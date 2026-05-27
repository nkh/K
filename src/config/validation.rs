use std::path::Path;

use super::schema::Config;

/// Severity level for a configuration validation issue.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationLevel {
    /// A fatal issue that prevents server startup.
    Error,
    /// A non-fatal issue that is logged but does not prevent startup.
    Warning,
}

/// A single configuration validation finding.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Dot-separated path to the offending field (e.g. "server.port").
    pub field: String,
    /// Severity of the issue.
    pub level: ValidationLevel,
    /// Human-readable description of the problem.
    pub message: String,
}

/// Validate a [`Config`] and return all issues found.
///
/// Errors are fatal (e.g. port out of range) and should cause the server
/// to abort.  Warnings are non-fatal (e.g. TLS cert file missing) and are
/// logged but startup continues.
pub fn validate_config(config: &Config) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    // ── server.port ──────────────────────────────────────────────
    if config.server.port == 0 {
        issues.push(ValidationIssue {
            field: "server.port".into(),
            level: ValidationLevel::Error,
            message: "Port must be between 1 and 65535, got 0".into(),
        });
    }

    // ── server.bind ──────────────────────────────────────────────
    if config.server.bind.is_empty() {
        issues.push(ValidationIssue {
            field: "server.bind".into(),
            level: ValidationLevel::Error,
            message: "Bind address must not be empty".into(),
        });
    } else if config.server.bind.parse::<std::net::IpAddr>().is_err() {
        issues.push(ValidationIssue {
            field: "server.bind".into(),
            level: ValidationLevel::Warning,
            message: format!(
                "Bind address '{}' does not look like a valid IP address",
                config.server.bind
            ),
        });
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

    // ── web.rate_limit.max_updates_per_sec ───────────────────────
    match config.web.rate_limit.max_updates_per_sec {
        0 => {} // disabled is fine
        n if n > 1000 => {
            issues.push(ValidationIssue {
                field: "web.rate_limit.max_updates_per_sec".into(),
                level: ValidationLevel::Warning,
                message: format!(
                    "Rate limit {} updates/sec is very high; values > 1000 may cause excessive CPU usage",
                    n
                ),
            });
        }
        _ => {}
    }

    // ── web.dirty_check_ms ───────────────────────────────────────
    if config.web.dirty_check_ms < 10 {
        issues.push(ValidationIssue {
            field: "web.dirty_check_ms".into(),
            level: ValidationLevel::Error,
            message: format!(
                "Dirty check interval must be >= 10 ms, got {} ms",
                config.web.dirty_check_ms
            ),
        });
    }

    // ── tls cert / key files ─────────────────────────────────────
    if config.tls.enabled {
        if let Some(ref cert) = config.tls.cert_file {
            if !Path::new(cert).exists() {
                issues.push(ValidationIssue {
                    field: "tls.cert_file".into(),
                    level: ValidationLevel::Warning,
                    message: format!("TLS certificate file does not exist: {}", cert),
                });
            }
        }
        if let Some(ref key) = config.tls.key_file {
            if !Path::new(key).exists() {
                issues.push(ValidationIssue {
                    field: "tls.key_file".into(),
                    level: ValidationLevel::Warning,
                    message: format!("TLS private key file does not exist: {}", key),
                });
            }
        }
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

    // ── security.token_file parent dir ───────────────────────────
    if !config.security.token_file.is_empty() {
        if let Some(parent) = Path::new(&config.security.token_file).parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                issues.push(ValidationIssue {
                    field: "security.token_file".into(),
                    level: ValidationLevel::Warning,
                    message: format!(
                        "Parent directory for token file does not exist: {}",
                        parent.display()
                    ),
                });
            }
        }
    }

    // ── security.cors.policy ─────────────────────────────────────
    match config.security.cors.policy.as_str() {
        "any" | "none" => {}
        custom => {
            // Validate each comma-separated origin parses as a valid HTTP header value
            let origins: Vec<_> = custom.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            if origins.is_empty() {
                issues.push(ValidationIssue {
                    field: "security.cors.policy".into(),
                    level: ValidationLevel::Error,
                    message: "CORS policy must be \"any\", \"none\", or a non-empty comma-separated list of origins".into(),
                });
            } else {
                let mut valid_count = 0;
                for origin in &origins {
                    if origin.parse::<axum::http::HeaderValue>().is_ok() {
                        valid_count += 1;
                    } else {
                        issues.push(ValidationIssue {
                            field: "security.cors.policy".into(),
                            level: ValidationLevel::Warning,
                            message: format!(
                                "CORS origin '{}' is not a valid HTTP header value and will be ignored",
                                origin
                            ),
                        });
                    }
                }
                if valid_count == 0 {
                    issues.push(ValidationIssue {
                        field: "security.cors.policy".into(),
                        level: ValidationLevel::Warning,
                        message: "No valid CORS origins found; falling back to permissive CORS".into(),
                    });
                }
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
        let errors: Vec<_> = issues.iter().filter(|i| i.level == ValidationLevel::Error).collect();
        assert!(
            errors.is_empty(),
            "Default config should produce no errors, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_invalid_port_zero() {
        let mut config = default_config();
        config.server.port = 0;
        let issues = validate_config(&config);
        let port_issues: Vec<_> = issues.iter().filter(|i| i.field == "server.port").collect();
        assert_eq!(port_issues.len(), 1);
        assert_eq!(port_issues[0].level, ValidationLevel::Error);
    }

    #[test]
    fn test_invalid_port_is_error_not_warning() {
        let mut config = default_config();
        config.server.port = 0;
        let issues = validate_config(&config);
        assert_eq!(issues[0].level, ValidationLevel::Error);
    }

    #[test]
    fn test_valid_port_high() {
        let mut config = default_config();
        config.server.port = 65535;
        let issues = validate_config(&config);
        assert!(
            issues.iter().all(|i| i.field != "server.port"),
            "Port 65535 should be valid"
        );
    }

    #[test]
    fn test_invalid_vtty_dims() {
        let mut config = default_config();
        config.vtty.rows = 0;
        config.vtty.cols = 0;
        let issues = validate_config(&config);
        let vtty_issues: Vec<_> = issues.iter().filter(|i| i.field.starts_with("vtty")).collect();
        assert_eq!(vtty_issues.len(), 2);
        assert!(vtty_issues.iter().all(|i| i.level == ValidationLevel::Error));
    }

    #[test]
    fn test_refresh_ms_too_low() {
        let mut config = default_config();
        config.display.refresh_ms = 1;
        let issues = validate_config(&config);
        let refresh_issues: Vec<_> = issues.iter().filter(|i| i.field == "display.refresh_ms").collect();
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
    fn test_rate_limit_zero_is_valid() {
        let mut config = default_config();
        config.web.rate_limit.max_updates_per_sec = 0;
        let issues = validate_config(&config);
        assert!(
            issues.iter().all(|i| i.field != "web.rate_limit.max_updates_per_sec"),
            "Rate limit 0 (disabled) should be valid"
        );
    }

    #[test]
    fn test_rate_limit_excessive_warns() {
        let mut config = default_config();
        config.web.rate_limit.max_updates_per_sec = 1001;
        let issues = validate_config(&config);
        let rl_issues: Vec<_> = issues.iter().filter(|i| i.field == "web.rate_limit.max_updates_per_sec").collect();
        assert_eq!(rl_issues.len(), 1);
        assert_eq!(rl_issues[0].level, ValidationLevel::Warning);
    }

    #[test]
    fn test_dirty_check_ms_too_low() {
        let mut config = default_config();
        config.web.dirty_check_ms = 0;
        let issues = validate_config(&config);
        let dc_issues: Vec<_> = issues.iter().filter(|i| i.field == "web.dirty_check_ms").collect();
        assert_eq!(dc_issues.len(), 1);
        assert_eq!(dc_issues[0].level, ValidationLevel::Error);
    }

    #[test]
    fn test_tls_enabled_missing_cert_warns() {
        let mut config = default_config();
        config.tls.enabled = true;
        config.tls.cert_file = Some("/nonexistent/path/cert.pem".into());
        config.tls.key_file = Some("/nonexistent/path/key.pem".into());
        let issues = validate_config(&config);
        let tls_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.field.starts_with("tls."))
            .collect();
        assert_eq!(tls_issues.len(), 2);
        assert!(tls_issues.iter().all(|i| i.level == ValidationLevel::Warning));
    }

    #[test]
    fn test_tls_disabled_no_cert_warning() {
        let mut config = default_config();
        config.tls.enabled = false;
        config.tls.cert_file = Some("/nonexistent/cert.pem".into());
        config.tls.key_file = Some("/nonexistent/key.pem".into());
        let issues = validate_config(&config);
        let tls_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.field.starts_with("tls."))
            .collect();
        assert!(
            tls_issues.is_empty(),
            "TLS disabled should not warn about missing cert/key"
        );
    }

    #[test]
    fn test_bind_empty_is_error() {
        let mut config = default_config();
        config.server.bind = String::new();
        let issues = validate_config(&config);
        let bind_issues: Vec<_> = issues.iter().filter(|i| i.field == "server.bind").collect();
        assert_eq!(bind_issues.len(), 1);
        assert_eq!(bind_issues[0].level, ValidationLevel::Error);
    }

    #[test]
    fn test_bind_invalid_ip_warns() {
        let mut config = default_config();
        config.server.bind = "not-an-ip".into();
        let issues = validate_config(&config);
        let bind_issues: Vec<_> = issues.iter().filter(|i| i.field == "server.bind").collect();
        assert_eq!(bind_issues.len(), 1);
        assert_eq!(bind_issues[0].level, ValidationLevel::Warning);
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

    #[test]
    fn test_cors_policy_any_is_valid() {
        let config = default_config();
        let issues = validate_config(&config);
        let cors_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.field == "security.cors.policy")
            .collect();
        assert!(cors_issues.is_empty(), "Policy 'any' should be valid");
    }

    #[test]
    fn test_cors_policy_none_is_valid() {
        let mut config = default_config();
        config.security.cors.policy = "none".into();
        let issues = validate_config(&config);
        let cors_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.field == "security.cors.policy")
            .collect();
        assert!(cors_issues.is_empty(), "Policy 'none' should be valid");
    }

    #[test]
    fn test_cors_policy_custom_origins_valid() {
        let mut config = default_config();
        config.security.cors.policy = "https://myapp.example.com,https://admin.example.com".into();
        let issues = validate_config(&config);
        let cors_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.field == "security.cors.policy")
            .collect();
        assert!(cors_issues.is_empty(), "Valid custom origins should produce no issues");
    }

    #[test]
    fn test_cors_policy_empty_custom_is_error() {
        let mut config = default_config();
        config.security.cors.policy = ",".into();
        let issues = validate_config(&config);
        let cors_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.field == "security.cors.policy" && i.level == ValidationLevel::Error)
            .collect();
        assert_eq!(cors_issues.len(), 1, "Empty custom policy should be an error");
    }

    #[test]
    fn test_cors_policy_invalid_origin_warns() {
        let mut config = default_config();
        // Control characters make an invalid header value
        config.security.cors.policy = "https://valid.example.com,\x00invalid".into();
        let issues = validate_config(&config);
        let cors_warnings: Vec<_> = issues
            .iter()
            .filter(|i| i.field == "security.cors.policy" && i.level == ValidationLevel::Warning)
            .collect();
        assert!(
            !cors_warnings.is_empty(),
            "Invalid origin in custom policy should produce a warning"
        );
    }
}
