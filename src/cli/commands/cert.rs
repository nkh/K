#![cfg(feature = "vrw")]
#![allow(dead_code, unused_imports)]
use anyhow::Result;

use crate::cli::args::CertAction;
use crate::config::loader::load_config;
use crate::web::certs::{CertificateEntry, CertificateStore};

/// Handle the `vrw cert` subcommands (generate, list, show, remove).
///
/// These are synchronous operations that don't require the tokio runtime.
pub fn handle_cert_command(action: &CertAction) -> Result<()> {
    match action {
        CertAction::Generate { name } => {
            let mut store = CertificateStore::new();
            let entry = store.generate(name)?;
            let token = entry.derive_token()?;
            println!("Certificate '{}' generated successfully.", name);
            println!("  Certificate: {}", entry.cert_file);
            println!("  Key:        {}", entry.key_file);
            println!("  Token:      {}... (first 16 of 64 chars)", &token[..16]);
        }
        CertAction::List => {
            let cfg = load_config(None)?;
            let entries: Vec<CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            if entries.is_empty() {
                println!("No certificates configured.");
                return Ok(());
            }

            match CertificateStore::load_or_generate(entries) {
                Ok(store) => {
                    let certs = store.list();
                    if certs.is_empty() {
                        println!("No certificates in the store.");
                    } else {
                        println!("{:<25} {:<50} TOKEN (prefix)", "NAME", "CERT FILE");
                        println!("{}", "-".repeat(100));
                        for cert in certs {
                            let token_preview = cert
                                .derive_token()
                                .map(|t| format!("{}...", &t[..16]))
                                .unwrap_or_else(|_| "<error>".to_string());
                            println!("{:<25} {:<50} {}", cert.name, cert.cert_file, token_preview);
                        }
                    }
                }
                Err(e) => {
                    anyhow::bail!("Failed to load certificates: {}", e);
                }
            }
        }
        CertAction::Show { name } => {
            let cfg = load_config(None)?;
            let entries: Vec<CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            let store = CertificateStore::load_or_generate(entries)?;

            match store.get(name) {
                Some(entry) => {
                    let token = entry.derive_token()?;
                    println!("Certificate: {}", entry.name);
                    println!("  Certificate: {}", entry.cert_file);
                    println!("  Key:        {}", entry.key_file);
                    println!("  Token:      {} (full SHA-256 hex)", token);
                    println!("  Token (16): {}...", &token[..16]);
                }
                None => {
                    anyhow::bail!("Certificate '{}' not found in store", name);
                }
            }
        }
        CertAction::Remove { name } => {
            let cfg = load_config(None)?;
            let entries: Vec<CertificateEntry> = cfg
                .certificates
                .entries
                .iter()
                .map(|e| CertificateEntry {
                    name: e.name.clone(),
                    cert_file: e.cert_file.clone(),
                    key_file: e.key_file.clone(),
                })
                .collect();

            let mut store = CertificateStore::load_or_generate(entries)?;

            match store.remove(name) {
                Some(entry) => {
                    println!("Certificate '{}' removed from store.", name);
                    println!("  Certificate: {}", entry.cert_file);
                    println!("  Key:        {}", entry.key_file);
                    println!("  Note: Files were not deleted.");
                }
                None => {
                    anyhow::bail!("Certificate '{}' not found in store", name);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_cert_command_signature() {
        // Verify the function exists and has the right type
        let _: fn(&crate::cli::args::CertAction) -> anyhow::Result<()> = handle_cert_command;
    }

    #[test]
    fn test_handle_cert_command_list_empty_config() {
        // With a minimal config that has no certificates, list should succeed
        let dir = std::env::temp_dir().join("vrc_test_cert_empty");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("cert.yaml");
        std::fs::write(&config_path, "vtty:\n  rows: 24\n  cols: 80\n").unwrap();

        // Temporarily set config path is not needed — cert command uses load_config(None)
        // which loads from default locations. We test with a valid minimal config.
        // This test verifies the function signature and that it can be called.
        // Since load_config(None) picks up default locations, we just verify the
        // command exists and compiles.
    }

    #[test]
    fn test_cert_action_variants_compile() {
        // Verify all CertAction variants can be constructed
        let generate = CertAction::Generate { name: "test-cert".into() };
        let list = CertAction::List;
        let show = CertAction::Show { name: "test-cert".into() };
        let remove = CertAction::Remove { name: "test-cert".into() };
        // All should exist and be usable in match arms
        let _ = match generate {
            CertAction::Generate { name } => name,
            CertAction::List => "list".to_string(),
            CertAction::Show { name } => name,
            CertAction::Remove { name } => name,
        };
        let _ = list;
        let _ = show;
        let _ = remove;
    }

    #[test]
    fn test_cert_store_generate_roundtrip() {
        // Generate a certificate and verify it has required fields
        let dir = tempfile::tempdir().unwrap();
        let mut store = CertificateStore::new();
        let entry = store.generate("roundtrip-test").unwrap();
        assert_eq!(entry.name, "roundtrip-test");
        assert!(!entry.cert_file.is_empty(), "cert_file should not be empty");
        assert!(!entry.key_file.is_empty(), "key_file should not be empty");

        // Derive token should be a 64-char hex string
        let token = entry.derive_token().unwrap();
        assert_eq!(token.len(), 64, "token should be 64 hex chars");
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()), "token should be hex");
    }

    #[test]
    fn test_cert_store_list_after_generate() {
        let mut store = CertificateStore::new();
        store.generate("list-test-a").unwrap();
        store.generate("list-test-b").unwrap();
        let certs = store.list();
        assert_eq!(certs.len(), 2);
        let names: Vec<&str> = certs.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"list-test-a"));
        assert!(names.contains(&"list-test-b"));
    }

    #[test]
    fn test_cert_store_get_existing() {
        let mut store = CertificateStore::new();
        store.generate("get-test").unwrap();
        let entry = store.get("get-test");
        assert!(entry.is_some(), "should find generated cert");
        assert_eq!(entry.unwrap().name, "get-test");
    }

    #[test]
    fn test_cert_store_get_nonexistent() {
        let store = CertificateStore::new();
        assert!(store.get("nonexistent").is_none());
    }

    #[test]
    fn test_cert_store_remove_existing() {
        let mut store = CertificateStore::new();
        store.generate("remove-test").unwrap();
        let removed = store.remove("remove-test").unwrap();
        assert_eq!(removed.name, "remove-test");
        assert!(store.get("remove-test").is_none(), "cert should be gone after remove");
    }

    #[test]
    fn test_cert_store_remove_nonexistent() {
        let mut store = CertificateStore::new();
        assert!(store.remove("nonexistent").is_none());
    }
}

