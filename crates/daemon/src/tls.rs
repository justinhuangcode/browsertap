use std::io::BufReader;
use std::sync::Arc;

use anyhow::{Context, Result};
use rcgen::generate_simple_self_signed;
use tokio_rustls::TlsAcceptor;
use tracing::info;

use crate::state::AppState;

/// Create a TLS acceptor, generating self-signed certs if needed.
pub fn create_tls_acceptor(_state: &AppState) -> Result<TlsAcceptor> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let cert_dir = std::path::PathBuf::from(&home)
        .join(".browsertap")
        .join("certs");
    let cert_path = cert_dir.join("localhost.pem");
    let key_path = cert_dir.join("localhost-key.pem");

    // Generate self-signed cert if not exists
    if !cert_path.exists() || !key_path.exists() {
        std::fs::create_dir_all(&cert_dir).context("failed to create cert directory")?;

        let subject_alt_names = vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string(),
        ];
        let cert = generate_simple_self_signed(subject_alt_names)
            .context("failed to generate self-signed certificate")?;

        std::fs::write(&cert_path, cert.cert.pem()).context("failed to write certificate")?;
        std::fs::write(&key_path, cert.key_pair.serialize_pem())
            .context("failed to write private key")?;

        info!(
            "generated self-signed TLS certificate at {}",
            cert_dir.display()
        );
    }

    // Load certificate chain
    let cert_file = std::fs::File::open(&cert_path).context("failed to open certificate file")?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<_> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .context("failed to parse certificates")?;

    // Load private key
    let key_file = std::fs::File::open(&key_path).context("failed to open key file")?;
    let mut key_reader = BufReader::new(key_file);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .context("failed to parse private key")?
        .context("no private key found")?;

    // Build rustls config
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("failed to build TLS config")?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}
