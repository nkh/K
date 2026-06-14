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