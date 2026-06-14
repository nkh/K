#![cfg(feature = "vrw")]
use anyhow::{Context, Result};

/// Configuration for self-signed certificate generation.
pub struct CertGenerationConfig {
    /// Common Name (CN) for the certificate subject.
    pub common_name: String,
    /// Organization (O) for the certificate subject.
    pub organization: String,
    /// Key usage purposes (e.g. DigitalSignature, KeyEncipherment).
    pub key_usages: Vec<rcgen::KeyUsagePurpose>,
    /// Extended key usage purposes (e.g. ServerAuth, ClientAuth).
    pub extended_key_usages: Vec<rcgen::ExtendedKeyUsagePurpose>,
    /// Whether this certificate is a CA.
    pub is_ca: rcgen::IsCa,
    /// Certificate validity start (UTC).
    pub not_before: rcgen::Time,
    /// Certificate validity end (UTC).
    pub not_after: rcgen::Time,
    /// Subject Alternative Names (DNS names, IP addresses, etc.).
    pub subject_alt_names: Vec<rcgen::SanType>,
}

impl CertGenerationConfig {
    /// Convenience: build a `CertGenerationConfig` for a local dev certificate.
    /// Includes localhost DNS, loopback IPs, and the given `extra_dns` names.
    pub fn localhost(common_name: &str, extra_dns: Vec<rcgen::SanType>) -> Self {
        let mut sans = vec![
            rcgen::SanType::DnsName(rcgen::Ia5String::try_from("localhost").unwrap()),
            rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
            rcgen::SanType::IpAddress(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)),
        ];
        sans.extend(extra_dns);
        Self {
            common_name: common_name.to_string(),
            organization: "vrw".to_string(),
            key_usages: vec![
                rcgen::KeyUsagePurpose::DigitalSignature,
                rcgen::KeyUsagePurpose::KeyEncipherment,
            ],
            extended_key_usages: vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth],
            is_ca: rcgen::IsCa::NoCa,
            not_before: rcgen::date_time_ymd(2025, 1, 1),
            not_after: rcgen::date_time_ymd(2030, 1, 1),
            subject_alt_names: sans,
        }
    }
}

/// Generate a self-signed X.509 certificate and private key using `rcgen`.
///
/// Returns `(cert_pem, key_pem)` as PEM-encoded strings.
pub fn generate_self_signed_cert(config: CertGenerationConfig) -> Result<(String, String)> {
    let mut params = rcgen::CertificateParams::default();

    // Distinguished name
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, &config.common_name);
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, &config.organization);

    // Key usage
    params.key_usages = config.key_usages;
    params.extended_key_usages = config.extended_key_usages;

    // Basic constraints
    params.is_ca = config.is_ca;

    // Validity period
    params.not_before = config.not_before;
    params.not_after = config.not_after;

    // Subject Alternative Names
    params.subject_alt_names = config.subject_alt_names;

    // Generate key pair and self-signed certificate
    let key_pair =
        rcgen::KeyPair::generate().context("Failed to generate TLS key pair")?;
    let cert = params
        .self_signed(&key_pair)
        .context("Failed to create self-signed certificate")?;

    Ok((cert.pem(), key_pair.serialize_pem()))
}

/// Write a certificate/key PEM pair to disk.
///
/// Creates parent directories if needed, writes both files, and sets
/// restrictive permissions (0600) on the key file on Unix.
pub fn write_cert_pair(
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
    cert_pem: &[u8],
    key_pem: &[u8],
) -> Result<()> {
    if let Some(parent) = cert_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    std::fs::write(cert_path, cert_pem)
        .with_context(|| format!("Failed to write certificate: {}", cert_path.display()))?;
    std::fs::write(key_path, key_pem)
        .with_context(|| format!("Failed to write key: {}", key_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(key_path, perms)
            .with_context(|| format!("Failed to set permissions on: {}", key_path.display()))?;
    }

    Ok(())
}