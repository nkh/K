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
