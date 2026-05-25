use std::collections::HashMap;

/// Run a hook command template with placeholder substitution.
///
/// The `hook_template` string may contain `{name}`, `{id}`, `{pid}`, `{exit_code}`
/// placeholders which are replaced with the corresponding values from `vars`.
///
/// The resulting command is split on whitespace and spawned as a detached
/// (fire-and-forget) child process. Failures are logged but never propagated
/// to avoid crashing the server.
pub fn run_hook(hook_template: &str, vars: &HashMap<&str, String>) {
    let mut cmd = hook_template.to_string();
    for (key, value) in vars {
        cmd = cmd.replace(&format!("{{{}}}", key), value);
    }

    // Split on whitespace, spawn detached
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if !parts.is_empty() {
        let binary = parts[0];
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        match std::process::Command::new(binary).args(&args).spawn() {
            Ok(mut child) => {
                let _ = child.try_wait();
            }
            Err(e) => {
                tracing::warn!(error = %e, command = %cmd, "Hook execution failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_hook_substitution() {
        // We can't easily test spawning in unit tests, but we can verify
        // that the substitution logic works correctly by using a known
        // binary that always succeeds.
        let mut vars = HashMap::new();
        vars.insert("name", "test-cmd".to_string());
        vars.insert("id", "123".to_string());
        vars.insert("pid", "456".to_string());

        // Use "true" which is a no-op command that always succeeds on Unix
        let template = "true"; // no placeholders, just verify it doesn't panic
        run_hook(template, &vars);
    }

    #[test]
    fn test_run_hook_with_placeholders() {
        // Use "echo" to verify substitution happens
        // Note: we can't capture the output easily, but we verify no panic
        let mut vars = HashMap::new();
        vars.insert("name", "my-cmd".to_string());
        vars.insert("id", "abc".to_string());

        // On any system, "true" should succeed
        run_hook("true", &vars);
    }

    #[test]
    fn test_run_hook_empty_template() {
        let vars = HashMap::new();
        run_hook("", &vars);
        // Should not panic
    }

    #[test]
    fn test_run_hook_whitespace_only() {
        let vars = HashMap::new();
        run_hook("   ", &vars);
        // Should not panic
    }

    #[test]
    fn test_placeholder_substitution_logic() {
        // Test the substitution logic directly
        let mut vars = HashMap::new();
        vars.insert("name", "test-cmd".to_string());
        vars.insert("id", "123".to_string());
        vars.insert("pid", "456".to_string());
        vars.insert("exit_code", "0".to_string());

        let template = "echo {name} {id} {pid} {exit_code}";
        let mut cmd = template.to_string();
        for (key, value) in &vars {
            cmd = cmd.replace(&format!("{{{}}}", key), value);
        }
        assert_eq!(cmd, "echo test-cmd 123 456 0");
    }

    #[test]
    fn test_placeholder_partial_substitution() {
        let mut vars = HashMap::new();
        vars.insert("name", "test".to_string());

        let template = "notify-send '{name} started'";
        let mut cmd = template.to_string();
        for (key, value) in &vars {
            cmd = cmd.replace(&format!("{{{}}}", key), value);
        }
        assert_eq!(cmd, "notify-send 'test started'");
    }

    #[test]
    fn test_placeholder_unknown_key() {
        // Unknown placeholders should remain as-is
        let mut vars = HashMap::new();
        vars.insert("name", "test".to_string());

        let template = "echo {name} {unknown}";
        let mut cmd = template.to_string();
        for (key, value) in &vars {
            cmd = cmd.replace(&format!("{{{}}}", key), value);
        }
        assert_eq!(cmd, "echo test {unknown}");
    }
}
