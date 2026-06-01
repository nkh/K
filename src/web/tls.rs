#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
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
    ///
    /// If `cert_file` and `key_file` are provided and exist, they are loaded.
    /// Otherwise, defaults are used, and if they don't exist, a new self-signed
    /// certificate is generated and saved.
    pub fn load_or_generate_config(
        cert_file: Option<&str>,
        key_file: Option<&str>,
    ) -> Result<Arc<rustls::ServerConfig>> {
        // rustls 0.23 requires an explicit process-level CryptoProvider.
        // The `ring` feature is enabled in Cargo.toml but the default
        // provider is not set automatically — calling `install_default()`
        // before any TLS operation prevents the runtime panic.
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

        // If both files exist, load them
        if cert_path.exists() && key_path.exists() {
            tracing::info!("Loading TLS certificate from {}", cert_path.display());
            let cert_pem = std::fs::read(&cert_path)
                .with_context(|| format!("Failed to read certificate: {}", cert_path.display()))?;
            let key_pem = std::fs::read(&key_path)
                .with_context(|| format!("Failed to read private key: {}", key_path.display()))?;
            return Ok((cert_pem, key_pem));
        }

        // Generate self-signed certificate
        tracing::info!("Generating self-signed TLS certificate...");
        let (cert_pem_str, key_pem_str) = Self::generate_certificate()?;

        // Ensure parent directory exists
        if let Some(parent) = cert_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        // Write certificate
        std::fs::write(&cert_path, &cert_pem_str)
            .with_context(|| format!("Failed to write certificate: {}", cert_path.display()))?;

        // Write private key
        std::fs::write(&key_path, &key_pem_str)
            .with_context(|| format!("Failed to write private key: {}", key_path.display()))?;

        // Set restrictive permissions on the key file (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&key_path, perms)
                .with_context(|| format!("Failed to set permissions on: {}", key_path.display()))?;
        }

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

        Ok((cert_pem_str.into_bytes(), key_pem_str.into_bytes()))
    }

    /// Generate a self-signed X.509 certificate using rcgen.
    /// Returns (cert_pem_string, key_pem_string).
    fn generate_certificate() -> Result<(String, String)> {
        let mut params = rcgen::CertificateParams::default();

        // Set distinguished name
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "vrw");
        params
            .distinguished_name
            .push(rcgen::DnType::OrganizationName, "vrw");

        // Set key usage
        params.key_usages = vec![
            rcgen::KeyUsagePurpose::DigitalSignature,
            rcgen::KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];

        params.is_ca = rcgen::IsCa::NoCa;

        // Valid for 5 years from 2025
        params.not_before = rcgen::date_time_ymd(2025, 1, 1);
        params.not_after = rcgen::date_time_ymd(2030, 1, 1);

        // Add Subject Alternative Names for localhost
        let san_entries = vec![
            rcgen::SanType::DnsName(rcgen::Ia5String::try_from("localhost").unwrap()),
            rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
            rcgen::SanType::IpAddress(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)),
        ];
        params.subject_alt_names = san_entries;

        let key_pair = rcgen::KeyPair::generate().context("Failed to generate TLS key pair")?;

        let cert = params
            .self_signed(&key_pair)
            .context("Failed to create self-signed certificate")?;

        let cert_pem = cert.pem();
        let key_pem = key_pair.serialize_pem();

        Ok((cert_pem, key_pem))
    }
}
