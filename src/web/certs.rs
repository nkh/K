#![cfg(feature = "vrw")]
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
            .join("vrw")
            .join("certs");
        Self {
            entries: HashMap::new(),
            token_to_name: HashMap::new(),
            certs_dir,
        }
    }

    /// Load certificates from config entries; auto-generate if missing.
    pub fn load_or_generate(entries: Vec<CertificateEntry>) -> Result<Self> {
        let certs_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("vrw")
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
        use super::cert_helpers::{CertGenerationConfig, generate_self_signed_cert};

        let extra_dns = vec![rcgen::SanType::DnsName(
            rcgen::Ia5String::try_from(name)
                .unwrap_or(rcgen::Ia5String::try_from("localhost").unwrap()),
        )];
        let mut config = CertGenerationConfig::localhost(name, extra_dns);
        config.extended_key_usages.push(rcgen::ExtendedKeyUsagePurpose::ClientAuth);

        let (cert_pem, key_pem) = generate_self_signed_cert(config)?;

        super::cert_helpers::write_cert_pair(
            cert_path, key_path,
            cert_pem.as_bytes(), key_pem.as_bytes(),
        )?;

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
        let base = PathBuf::from("/tmp/vrw/certs");
        let result = CertificateStore::resolve_path("/etc/ssl/cert.pem", &base);
        assert_eq!(result, PathBuf::from("/etc/ssl/cert.pem"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let base = PathBuf::from("/tmp/vrw/certs");
        let result = CertificateStore::resolve_path("my-app/cert.pem", &base);
        assert_eq!(result, PathBuf::from("/tmp/vrw/certs/my-app/cert.pem"));
    }

    #[test]
    fn test_certificate_store_new() {
        let store = CertificateStore::new();
        assert!(store.entries.is_empty());
        assert!(store.token_to_name.is_empty());
        assert!(store.list().is_empty());
        assert!(store.certs_dir().to_string_lossy().contains("certs"));
    }

    #[test]
    fn test_certificate_store_default() {
        let store = CertificateStore::default();
        assert!(store.list().is_empty());
    }

    #[test]
    fn test_certificate_store_generate() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        CertificateStore::generate_named_certificate("test-cert", &cert_path, &key_path).unwrap();
        assert!(cert_path.exists());
        assert!(key_path.exists());
    }

    #[test]
    fn test_certificate_entry_derive_token() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        CertificateStore::generate_named_certificate("tok-test", &cert_path, &key_path).unwrap();
        let entry = CertificateEntry {
            name: "tok-test".to_string(),
            cert_file: cert_path.to_string_lossy().to_string(),
            key_file: key_path.to_string_lossy().to_string(),
        };
        let token = entry.derive_token().unwrap();
        // SHA-256 hex is always 64 chars
        assert_eq!(token.len(), 64);
        // All hex chars
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_certificate_entry_derive_token_missing_file() {
        let entry = CertificateEntry {
            name: "missing".to_string(),
            cert_file: "/nonexistent/cert.pem".to_string(),
            key_file: "/nonexistent/key.pem".to_string(),
 };
        assert!(entry.derive_token().is_err());
    }

    #[test]
    fn test_certificate_store_generate_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = CertificateStore {
            entries: HashMap::new(),
            token_to_name: HashMap::new(),
            certs_dir: dir.path().to_path_buf(),
        };
        let entry = store.generate("my-cert").unwrap();
        assert_eq!(entry.name, "my-cert");
        assert!(entry.cert_file.contains("my-cert"));
        assert!(entry.key_file.contains("my-cert"));
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.get("my-cert").unwrap().name, "my-cert");
        assert!(store.exists("my-cert"));
    }

    #[test]
    fn test_certificate_store_generate_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = CertificateStore {
            entries: HashMap::new(),
            token_to_name: HashMap::new(),
            certs_dir: dir.path().to_path_buf(),
        };
        store.generate("dup").unwrap();
        assert!(store.generate("dup").is_err());
    }

    #[test]
    fn test_certificate_store_get_missing() {
        let store = CertificateStore::new();
        assert!(store.get("nonexistent").is_none());
    }

    #[test]
    fn test_certificate_store_validate_token() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = CertificateStore {
            entries: HashMap::new(),
            token_to_name: HashMap::new(),
            certs_dir: dir.path().to_path_buf(),
        };
        store.generate("auth-test").unwrap();
        let token = store.get("auth-test").unwrap().derive_token().unwrap();
        assert_eq!(store.validate_token(&token), Some("auth-test"));
        assert!(store.validate_token("invalid").is_none());
    }

    #[test]
    fn test_certificate_store_validate_empty_token() {
        let store = CertificateStore::new();
        assert!(store.validate_token("").is_none());
    }

    #[test]
    fn test_certificate_store_exists() {
        let store = CertificateStore::new();
        assert!(!store.exists("nope"));
    }

    #[test]
    fn test_certificate_store_remove() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = CertificateStore {
            entries: HashMap::new(),
            token_to_name: HashMap::new(),
            certs_dir: dir.path().to_path_buf(),
        };
        store.generate("removable").unwrap();
        assert!(store.exists("removable"));
        let removed = store.remove("removable").unwrap();
        assert_eq!(removed.name, "removable");
        assert!(!store.exists("removable"));
 }

    #[test]
 fn test_certificate_store_remove_missing() {
        let mut store = CertificateStore::new();
        assert!(store.remove("ghost").is_none());
 }

    #[test]
 fn test_certificate_entry_serde_roundtrip() {
        let entry = CertificateEntry {
            name: "test".to_string(),
            cert_file: "/certs/test.pem".to_string(),
            key_file: "/certs/test.key".to_string(),
 };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: CertificateEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, entry.name);
        assert_eq!(parsed.cert_file, entry.cert_file);
        assert_eq!(parsed.key_file, entry.key_file);
    }

    #[test]
 fn test_load_or_generate_missing_auto_creates() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![CertificateEntry {
            name: "auto-gen".to_string(),
            cert_file: dir.path().join("certs").join("cert.pem").to_string_lossy().to_string(),
            key_file: dir.path().join("certs").join("key.pem").to_string_lossy().to_string(),
 }];
        let store = CertificateStore::load_or_generate(entries).unwrap();
        assert!(store.exists("auto-gen"));
        let entry = store.get("auto-gen").unwrap();
        assert!(std::path::PathBuf::from(&entry.cert_file).exists());
        assert!(std::path::PathBuf::from(&entry.key_file).exists());
 }

    #[test]
 fn test_load_or_generate_with_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        // Pre-generate the cert/key
        let cert_path = dir.path().join("pre.pem");
        let key_path = dir.path().join("pre.key");
        CertificateStore::generate_named_certificate("pre", &cert_path, &key_path).unwrap();
        let entries = vec![CertificateEntry {
            name: "pre".to_string(),
            cert_file: cert_path.to_string_lossy().to_string(),
            key_file: key_path.to_string_lossy().to_string(),
 }];
        let store = CertificateStore::load_or_generate(entries).unwrap();
        assert!(store.exists("pre"));
 }

    #[test]
 fn test_list_returns_all() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = CertificateStore {
            entries: HashMap::new(),
            token_to_name: HashMap::new(),
            certs_dir: dir.path().to_path_buf(),
        };
        store.generate("a").unwrap();
        store.generate("b").unwrap();
        store.generate("c").unwrap();
        let list = store.list();
        assert_eq!(list.len(), 3);
        let names: Vec<&str> = list.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
        assert!(names.contains(&"c"));
 }
}
