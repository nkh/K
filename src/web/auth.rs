#![cfg(feature = "vrunner")]
#![allow(dead_code, unused_imports)]
use anyhow::{Context, Result};
use rand::Rng;
use std::fs;
use std::path::Path;

/// Manages bearer token authentication.
///
/// When auth is enabled, the token is loaded from a file (or generated if it
/// doesn't exist). The token must be provided in the `Authorization: Bearer <token>`
/// header for all API requests.
pub struct AuthManager;

impl AuthManager {
    /// Load or generate a bearer token.
    ///
    /// If `token_file` exists, the token is read from it (first line, trimmed).
    /// Otherwise, a new 256-bit random token is generated and saved as 64 hex chars.
    pub fn load_or_generate(token_file: &str) -> Result<String> {
        let path = Path::new(token_file);

        if path.exists() {
            let token = fs::read_to_string(path)
                .with_context(|| format!("Failed to read token file: {}", token_file))?
                .trim()
                .to_string();
            if !token.is_empty() {
                tracing::info!("Loaded auth token from {}", token_file);
                return Ok(token);
            }
        }

        // Generate a new random 256-bit token (64 hex characters)
        let token: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        // Write with restrictive permissions (owner read/write only)
        fs::write(path, &token)
            .with_context(|| format!("Failed to write token file: {}", token_file))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(path, perms)
                .with_context(|| format!("Failed to set permissions on: {}", token_file))?;
        }

        tracing::info!(
            "Generated new auth token, saved to: {}\n\
             \n\
             Use this token in API requests:\n\
             curl -H 'Authorization: Bearer {}' https://localhost:{}/api/commands",
            token_file,
            token,
            "{PORT}"
        );

        Ok(token)
    }
}
