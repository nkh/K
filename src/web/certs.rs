#![cfg(feature = "vrunner")]
#![allow(dead_code, unused_imports)]
use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A named certificate entry in the certificate pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateEntry {
    /// Logical name used to reference this certificate (e.g., "webapp-frontend").
    pub name: String,
    /// Path to the PEM-encoded certificate file.
    pub cert_file: String,
    /// Path to the PEM-encoded private key file.
    pub key_file: String,
}

impl CertificateEntry {
    /// Derive a bearer token from the certificate's public key.
    /// The token is the SHA-256 hash of the certificate PEM, hex-encoded.
    pub fn derive_token(&self) -> Result<String> {
        let cert_pem = std::fs::read(&self.cert_file)
            .with_context(|| format!("Failed to read certificate: {}", self.cert_file))?;
        let mut hasher = Sha256::new();
        hasher.update(&cert_pem);
        let hash = hasher.finalize();
        Ok(hex::encode(hash))
    }
}

/// Manages a pool of named certificates for per-command access control.
///
/// Each certificate in the pool can be bound to a running command. When a command
/// is bound to a certificate, only clients presenting that certificate (via mTLS)
/// or its derived bearer token can interact with the command's endpoints.
pub struct CertificateStore {
    /// Map from certificate name to its entry (paths + metadata).
    entries: HashMap<String, CertificateEntry>,
    /// Map from derived token hash to certificate name (for fast auth lookups).
    token_to_name: HashMap<String, String>,
    /// Base directory for auto-generated certificates.
    certs_dir: PathBuf,
}

impl CertificateStore {
    /// Create a new certificate store.
    pub fn new() -> Self {
        let certs_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("vrunner")
            .join("certs");
        Self {
            entries: HashMap::new(),
            token_to_name: HashMap::new(),
            certs_dir,
        }
    }

    /// Load certificates from config entries and ensure the store is ready.
    ///
    /// For entries where both files exist, they are loaded as-is.
    /// For entries where files are missing, new certificates are auto-generated.
    pub fn load_or_generate(entries: Vec<CertificateEntry>) -> Result<Self> {
        let certs_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("vrunner")
            .join("certs");

        let mut store = Self {
            entries: HashMap::new(),
            token_to_name: HashMap::new(),
            certs_dir: certs_dir.clone(),
        };

        for mut entry in entries {
            // Resolve relative paths against certs_dir
            let cert_path = Self::resolve_path(&entry.cert_file, &certs_dir);
            let key_path = Self::resolve_path(&entry.key_file, &certs_dir);

            if !cert_path.exists() || !key_path.exists() {
                // Auto-generate if missing
                Self::generate_named_certificate(&entry.name, &cert_path, &key_path)?;
            }

            entry.cert_file = cert_path.to_string_lossy().to_string();
            entry.key_file = key_path.to_string_lossy().to_string();

            let token = entry.derive_token()?;
            store.token_to_name.insert(token, entry.name.clone());
            store.entries.insert(entry.name.clone(), entry);
        }

        Ok(store)
    }

    /// Generate a new certificate and add it to the store.
    pub fn generate(&mut self, name: &str) -> Result<CertificateEntry> {
        if self.entries.contains_key(name) {
            anyhow::bail!("Certificate '{}' already exists", name);
        }

        let cert_dir = self.certs_dir.join(name);
        let cert_path = cert_dir.join("cert.pem");
        let key_path = cert_dir.join("key.pem");

        Self::generate_named_certificate(name, &cert_path, &key_path)?;

        let entry = CertificateEntry {
            name: name.to_string(),
            cert_file: cert_path.to_string_lossy().to_string(),
            key_file: key_path.to_string_lossy().to_string(),
        };

        let token = entry.derive_token()?;
        self.token_to_name.insert(token.clone(), entry.name.clone());
        self.entries.insert(entry.name.clone(), entry.clone());

        tracing::info!(
            "Generated certificate '{}': cert={}, key={}, token={}",
            name,
            cert_path.display(),
            key_path.display(),
            &token[..16]
        );

        Ok(entry)
    }

    /// Generate a certificate without adding to store (static helper).
    pub fn generate_named_certificate(
        name: &str,
        cert_path: &std::path::Path,
        key_path: &std::path::Path,
    ) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = cert_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let mut params = rcgen::CertificateParams::default();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, name);
        params
            .distinguished_name
            .push(rcgen::DnType::OrganizationName, "vrunner");

        params.key_usages = vec![
            rcgen::KeyUsagePurpose::DigitalSignature,
            rcgen::KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![
            rcgen::ExtendedKeyUsagePurpose::ServerAuth,
            rcgen::ExtendedKeyUsagePurpose::ClientAuth,
        ];
        params.is_ca = rcgen::IsCa::NoCa;

        params.not_before = rcgen::date_time_ymd(2025, 1, 1);
        params.not_after = rcgen::date_time_ymd(2030, 1, 1);

        // SANs: localhost + the certificate name as DNS
        let san_entries = vec![
            rcgen::SanType::DnsName(rcgen::Ia5String::try_from("localhost").unwrap()),
            rcgen::SanType::DnsName(
                rcgen::Ia5String::try_from(name)
                    .unwrap_or(rcgen::Ia5String::try_from("localhost").unwrap()),
            ),
            rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
            rcgen::SanType::IpAddress(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)),
        ];
        params.subject_alt_names = san_entries;

        let key_pair = rcgen::KeyPair::generate().context("Failed to generate key pair")?;
        let cert = params
            .self_signed(&key_pair)
            .context("Failed to create certificate")?;

        let cert_pem = cert.pem();
        let key_pem = key_pair.serialize_pem();

        std::fs::write(cert_path, &cert_pem)
            .with_context(|| format!("Failed to write certificate: {}", cert_path.display()))?;
        std::fs::write(key_path, &key_pem)
            .with_context(|| format!("Failed to write key: {}", key_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(key_path, perms)
                .with_context(|| format!("Failed to set permissions on: {}", key_path.display()))?;
        }

        tracing::info!(
            "Certificate '{}' generated: cert={}, key={}",
            name,
            cert_path.display(),
            key_path.display()
        );

        Ok(())
    }

    /// List all certificates in the store.
    pub fn list(&self) -> Vec<&CertificateEntry> {
        self.entries.values().collect()
    }

    /// Get a certificate entry by name.
    pub fn get(&self, name: &str) -> Option<&CertificateEntry> {
        self.entries.get(name)
    }

    /// Validate a bearer token and return the associated certificate name.
    ///
    /// The token is the full SHA-256 hex of the cert PEM (64 chars).
    /// Returns `None` if no certificate matches this token.
    pub fn validate_token(&self, token: &str) -> Option<&str> {
        self.token_to_name.get(token).map(|s| s.as_str())
    }

    /// Check if a certificate name exists in the store.
    pub fn exists(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// Return the certificates directory path.
    pub fn certs_dir(&self) -> &PathBuf {
        &self.certs_dir
    }

    /// Remove a certificate from the store (does not delete files).
    pub fn remove(&mut self, name: &str) -> Option<CertificateEntry> {
        if let Some(entry) = self.entries.remove(name) {
            if let Ok(token) = entry.derive_token() {
                self.token_to_name.remove(&token);
            }
            Some(entry)
        } else {
            None
        }
    }

    /// Resolve a path: if absolute, use as-is; if relative, join with base.
    fn resolve_path(path: &str, base: &std::path::Path) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            base.join(p)
        }
    }
}

impl Default for CertificateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_path_absolute() {
        let base = PathBuf::from("/tmp/vrunner/certs");
        let result = CertificateStore::resolve_path("/etc/ssl/cert.pem", &base);
        assert_eq!(result, PathBuf::from("/etc/ssl/cert.pem"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let base = PathBuf::from("/tmp/vrunner/certs");
        let result = CertificateStore::resolve_path("my-app/cert.pem", &base);
        assert_eq!(result, PathBuf::from("/tmp/vrunner/certs/my-app/cert.pem"));
    }
}
