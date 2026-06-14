#![cfg(feature = "vrw")]

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Manages TLS certificate generation and loading.
///
/// On first use (or when certificates don't exist), vrw generates a
/// self-signed certificate using `rcgen`. The certificate and key are saved
/// as PEM files in the configured directory. Authorized clients must be
/// given the certificate to establish trust.
pub struct TlsManager;

impl TlsManager {
    /// Returns the default certificate and key file paths.
    pub fn default_paths() -> (PathBuf, PathBuf) {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("vrw");
        (dir.join("cert.pem"), dir.join("key.pem"))
    }

    /// Load or generate a TLS rustls ServerConfig.
    pub fn load_or_generate_config(
        cert_file: Option<&str>,
        key_file: Option<&str>,
    ) -> Result<Arc<rustls::ServerConfig>> {
        // Install rustls crypto provider (required by rustls 0.23).
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("failed to install rustls crypto provider");

        let (cert_pem, key_pem) = Self::load_or_generate(cert_file, key_file)?;

        let cert_slice = &mut cert_pem.as_slice();
        let certs: Vec<rustls::pki_types::CertificateDer> = rustls_pemfile::certs(cert_slice)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse certificate")?;

        let key = rustls_pemfile::private_key(&mut key_pem.as_slice())
            .context("Failed to parse private key")?
            .ok_or_else(|| anyhow::anyhow!("No private key found in PEM file"))?;

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .context("Failed to create TLS server config")?;

        Ok(Arc::new(config))
    }

    /// Load or generate raw PEM certificate and key bytes.
    pub fn load_or_generate(
        cert_file: Option<&str>,
        key_file: Option<&str>,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let (cert_path, key_path) = match (cert_file, key_file) {
            (Some(c), Some(k)) => (PathBuf::from(c), PathBuf::from(k)),
            _ => Self::default_paths(),
        };

        if cert_path.exists() && key_path.exists() {
            tracing::info!("Loading TLS certificate from {}", cert_path.display());
            let cert_pem = std::fs::read(&cert_path)
                .with_context(|| format!("Failed to read certificate: {}", cert_path.display()))?;
            let key_pem = std::fs::read(&key_path)
                .with_context(|| format!("Failed to read private key: {}", key_path.display()))?;
            return Ok((cert_pem, key_pem));
        }

        tracing::info!("Generating self-signed TLS certificate...");
        let (cert_pem, key_pem) = super::cert_helpers::generate_self_signed_cert(
            super::cert_helpers::CertGenerationConfig::localhost("vrw", vec![]),
        )?;

        super::cert_helpers::write_cert_pair(
            &cert_path, &key_path,
            cert_pem.as_bytes(), key_pem.as_bytes(),
        )?;

        tracing::info!(
            "TLS certificate saved to: {}\n\
             TLS private key saved to: {}\n\
             \n\
             Distribute the certificate file to authorized clients.\n\
             Clients can use it with: curl --cacert {} https://localhost:{}/",
            cert_path.display(),
            key_path.display(),
            cert_path.display(),
            "{PORT}"
        );

        Ok((cert_pem.into_bytes(), key_pem.into_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_manager_default_paths() {
        let (cert, key) = TlsManager::default_paths();
        assert!(cert.to_string_lossy().contains("vrw"));
        assert!(key.to_string_lossy().contains("vrw"));
        assert!(cert.to_string_lossy().ends_with("cert.pem"));
        assert!(key.to_string_lossy().ends_with("key.pem"));
    }

    #[test]
    fn test_tls_manager_generate_certificate() {
        let (cert_pem, key_pem) = super::super::cert_helpers::generate_self_signed_cert(
            super::super::cert_helpers::CertGenerationConfig::localhost("vrw", vec![]),
        ).unwrap();
        assert!(cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(key_pem.contains("BEGIN PRIVATE KEY"));
    }

    #[test]
    fn test_tls_manager_load_or_generate() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        let (cert_pem, key_pem) = TlsManager::load_or_generate(
            Some(cert_path.to_str().unwrap()),
            Some(key_path.to_str().unwrap()),
        ).unwrap();
        assert!(!cert_pem.is_empty());
        assert!(!key_pem.is_empty());
        // Files should have been created
        assert!(cert_path.exists());
        assert!(key_path.exists());
    }
}
