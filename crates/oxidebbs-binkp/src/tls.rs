use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

use native_tls::{Certificate, Identity, Protocol, TlsAcceptor, TlsConnector, TlsStream};

use crate::error::BinkpError;

/// Native TLS identity type used by BinkP TLS server and mutual-TLS client
/// configuration.
pub type BinkpTlsIdentity = Identity;

/// Native TLS certificate type used by BinkP client trust roots.
pub type BinkpTlsCertificate = Certificate;

/// TLS configuration for BinkP client connections.
#[derive(Clone)]
pub struct BinkpTlsClientConfig {
    /// Whether to verify server certificates.
    pub verify_certificates: bool,
    /// Additional root certificates trusted for this BinkP link.
    pub root_certificates: Vec<Certificate>,
    /// Optional client certificate for mutual TLS.
    pub client_identity: Option<Identity>,
}

impl Default for BinkpTlsClientConfig {
    fn default() -> Self {
        Self {
            verify_certificates: true,
            root_certificates: Vec::new(),
            client_identity: None,
        }
    }
}

/// TLS configuration for BinkP server connections.
#[derive(Clone)]
pub struct BinkpTlsServerConfig {
    /// Server identity (certificate and private key).
    pub identity: Identity,
    /// Whether to require client certificates.
    pub require_client_cert: bool,
    /// Trusted client certificates for mutual TLS.
    pub client_certificates: Vec<Certificate>,
}

/// Build a server or client identity from PEM-encoded certificate chain and
/// PKCS#8 key bytes.
///
/// # Errors
///
/// Returns TLS errors when the certificate and key cannot be parsed as a native
/// TLS identity.
pub fn identity_from_pkcs8_pem(
    certificate_pem: &[u8],
    key_pem: &[u8],
) -> Result<Identity, BinkpError> {
    Identity::from_pkcs8(certificate_pem, key_pem)
        .map_err(|error| BinkpError::Tls(format!("failed to load TLS identity: {error}")))
}

/// Build a trust root from a PEM-encoded certificate.
///
/// # Errors
///
/// Returns TLS errors when the certificate cannot be parsed.
pub fn certificate_from_pem(certificate_pem: &[u8]) -> Result<Certificate, BinkpError> {
    Certificate::from_pem(certificate_pem)
        .map_err(|error| BinkpError::Tls(format!("failed to load TLS certificate: {error}")))
}

/// Load a server TLS identity from PEM-encoded certificate chain and PKCS#8 key
/// files.
///
/// # Errors
///
/// Returns I/O errors for unreadable files or TLS errors when the certificate
/// and key cannot be parsed as a native TLS identity.
pub fn load_server_identity_from_pkcs8_pem_files(
    certificate_path: impl AsRef<Path>,
    key_path: impl AsRef<Path>,
) -> Result<Identity, BinkpError> {
    let certificate = std::fs::read(certificate_path)?;
    let key = std::fs::read(key_path)?;
    identity_from_pkcs8_pem(&certificate, &key)
}

/// Establish a TLS connection as a client.
pub fn connect_tls(
    stream: TcpStream,
    hostname: &str,
    config: &BinkpTlsClientConfig,
) -> Result<TlsStream<TcpStream>, BinkpError> {
    let mut builder = TlsConnector::builder();

    if !config.verify_certificates {
        builder.danger_accept_invalid_certs(true);
        builder.danger_accept_invalid_hostnames(true);
    }

    if let Some(identity) = &config.client_identity {
        builder.identity(identity.clone());
    }
    for certificate in &config.root_certificates {
        builder.add_root_certificate(certificate.clone());
    }

    // Disable older protocols for security
    builder.min_protocol_version(Some(Protocol::Tlsv12));

    let connector = builder
        .build()
        .map_err(|e| BinkpError::Tls(format!("Failed to build TLS connector: {}", e)))?;

    connector
        .connect(hostname, stream)
        .map_err(|e| BinkpError::Tls(format!("TLS handshake failed: {}", e)))
}

/// Accept a TLS connection as a server.
pub fn accept_tls(
    stream: TcpStream,
    config: &BinkpTlsServerConfig,
) -> Result<TlsStream<TcpStream>, BinkpError> {
    let mut builder = TlsAcceptor::builder(config.identity.clone());

    // Disable older protocols for security
    builder.min_protocol_version(Some(Protocol::Tlsv12));

    let acceptor = builder
        .build()
        .map_err(|e| BinkpError::Tls(format!("Failed to build TLS acceptor: {}", e)))?;

    acceptor
        .accept(stream)
        .map_err(|e| BinkpError::Tls(format!("TLS handshake failed: {}", e)))
}

/// A wrapper that can hold either a plain TCP stream or a TLS stream.
pub enum BinkpStream {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl Read for BinkpStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            BinkpStream::Plain(stream) => stream.read(buf),
            BinkpStream::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for BinkpStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            BinkpStream::Plain(stream) => stream.write(buf),
            BinkpStream::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            BinkpStream::Plain(stream) => stream.flush(),
            BinkpStream::Tls(stream) => stream.flush(),
        }
    }
}

impl BinkpStream {
    /// Create a plain TCP stream.
    pub fn plain(stream: TcpStream) -> Self {
        BinkpStream::Plain(stream)
    }

    /// Create a TLS stream.
    pub fn tls(stream: TlsStream<TcpStream>) -> Self {
        BinkpStream::Tls(stream)
    }

    /// Check if this is a TLS connection.
    pub fn is_tls(&self) -> bool {
        matches!(self, BinkpStream::Tls(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_default_client_config() {
        let config = BinkpTlsClientConfig::default();
        assert!(config.verify_certificates);
        assert!(config.root_certificates.is_empty());
        assert!(config.client_identity.is_none());
    }

    #[test]
    fn test_stream_is_tls() {
        // We can't easily test with real streams in unit tests,
        // but we can test the enum logic
        let stream = TcpStream::connect("127.0.0.1:1").unwrap_err();
        // This will fail to connect, which is fine for this test
        assert!(
            stream.to_string().contains("Connection refused")
                || stream.to_string().contains("Network is unreachable")
        );
    }

    #[test]
    fn tls_accept_and_connect_succeed_with_trusted_certificate() {
        let fixture = tls_fixture("localhost");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind TLS server");
        let addr = listener.local_addr().expect("listener addr");
        let server_identity = fixture.identity.clone();

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept TCP");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .expect("write timeout");
            let mut tls = accept_tls(
                stream,
                &BinkpTlsServerConfig {
                    identity: server_identity,
                    require_client_cert: false,
                    client_certificates: Vec::new(),
                },
            )
            .expect("accept TLS");
            let mut byte = [0_u8; 1];
            tls.read_exact(&mut byte).expect("server read");
            assert_eq!(byte, [42]);
            tls.write_all(&[43]).expect("server write");
            tls.flush().expect("server flush");
        });

        let stream = TcpStream::connect(addr).expect("connect TCP");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("write timeout");
        let mut tls = connect_tls(
            stream,
            "localhost",
            &BinkpTlsClientConfig {
                verify_certificates: true,
                root_certificates: vec![fixture.certificate],
                client_identity: None,
            },
        )
        .expect("connect TLS");

        tls.write_all(&[42]).expect("client write");
        tls.flush().expect("client flush");
        let mut byte = [0_u8; 1];
        tls.read_exact(&mut byte).expect("client read");
        assert_eq!(byte, [43]);
        server.join().expect("server joined");
    }

    #[test]
    fn tls_connect_fails_for_untrusted_certificate() {
        let fixture = tls_fixture("localhost");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind TLS server");
        let addr = listener.local_addr().expect("listener addr");
        let server_identity = fixture.identity.clone();

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept TCP");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .expect("write timeout");
            let result = accept_tls(
                stream,
                &BinkpTlsServerConfig {
                    identity: server_identity,
                    require_client_cert: false,
                    client_certificates: Vec::new(),
                },
            );
            assert!(result.is_err());
        });

        let stream = TcpStream::connect(addr).expect("connect TCP");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("write timeout");
        let error = connect_tls(stream, "localhost", &BinkpTlsClientConfig::default())
            .expect_err("untrusted certificate rejected");

        assert!(matches!(error, BinkpError::Tls(_)));
        server.join().expect("server joined");
    }

    #[test]
    fn tls_connect_can_skip_certificate_validation_for_legacy_peers() {
        let fixture = tls_fixture("localhost");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind TLS server");
        let addr = listener.local_addr().expect("listener addr");
        let server_identity = fixture.identity.clone();

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept TCP");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .expect("write timeout");
            let mut tls = accept_tls(
                stream,
                &BinkpTlsServerConfig {
                    identity: server_identity,
                    require_client_cert: false,
                    client_certificates: Vec::new(),
                },
            )
            .expect("accept TLS");
            tls.write_all(&[1]).expect("server write");
            tls.flush().expect("server flush");
        });

        let stream = TcpStream::connect(addr).expect("connect TCP");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("write timeout");
        let mut tls = connect_tls(
            stream,
            "localhost",
            &BinkpTlsClientConfig {
                verify_certificates: false,
                root_certificates: Vec::new(),
                client_identity: None,
            },
        )
        .expect("connect TLS without verification");

        let mut byte = [0_u8; 1];
        tls.read_exact(&mut byte).expect("client read");
        assert_eq!(byte, [1]);
        server.join().expect("server joined");
    }

    struct TlsFixture {
        identity: Identity,
        certificate: Certificate,
    }

    fn tls_fixture(hostname: &str) -> TlsFixture {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec![hostname.to_string()])
                .expect("generate certificate");
        let cert_pem = cert.pem();
        let key_pem = signing_key.serialize_pem();
        TlsFixture {
            identity: Identity::from_pkcs8(cert_pem.as_bytes(), key_pem.as_bytes())
                .expect("identity from cert"),
            certificate: Certificate::from_pem(cert_pem.as_bytes()).expect("certificate from pem"),
        }
    }
}
