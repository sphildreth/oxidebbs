use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

use oxidebbs_binkp::{
    BinkpOutboundFile, BinkpServer, BinkpServerHandshake, BinkpStream, BinkpTlsServerConfig,
    LinkSessionRegistry, accept_tls, load_server_identity_from_pkcs8_pem_files,
};
use oxidebbs_db::{
    NetworkLinkRecord, NetworkPacketRecord, OxideDb, find_network_profile_by_id,
    finish_network_packet, list_network_links, list_network_packets,
};
use oxidebbs_ftn::{ScannerPaths, TosserPaths};

use crate::config::OxideConfig;
use crate::serve::{ServeError, ServeResult};

#[derive(Clone)]
struct BinkpListenerState {
    config: Arc<OxideConfig>,
    db: Arc<OxideDb>,
    session_registry: Arc<LinkSessionRegistry>,
    tls_config: Option<Arc<BinkpTlsServerConfig>>,
    connection_limit: Arc<Semaphore>,
}

pub(crate) async fn start_binkp_listener(
    config: Arc<OxideConfig>,
    db: Arc<OxideDb>,
) -> ServeResult<tokio::task::JoinHandle<ServeResult<()>>> {
    let listener_config = config
        .network
        .binkp_listener
        .as_ref()
        .ok_or_else(|| ServeError::Config("binkp_listener not configured".into()))?;

    if !listener_config.enabled {
        return Err(ServeError::Config("binkp_listener is disabled".into()));
    }

    let bind: SocketAddr = listener_config
        .bind
        .parse()
        .map_err(|error| ServeError::Config(format!("invalid binkp_listener.bind: {error}")))?;
    let max_connections = listener_config.max_connections;
    let tls_config = match (
        listener_config.tls_cert_path.as_ref(),
        listener_config.tls_key_path.as_ref(),
    ) {
        (Some(certificate_path), Some(key_path)) => {
            let identity = load_server_identity_from_pkcs8_pem_files(certificate_path, key_path)
                .map_err(|error| {
                    ServeError::Config(format!("invalid binkp_listener TLS identity: {error}"))
                })?;
            Some(Arc::new(BinkpTlsServerConfig {
                identity,
                require_client_cert: false,
                client_certificates: Vec::new(),
            }))
        }
        (None, None) => None,
        _ => {
            return Err(ServeError::Config(
                "binkp_listener TLS requires both tls_cert_path and tls_key_path".into(),
            ));
        }
    };

    if tls_config.is_none()
        && list_network_links(db.db())?
            .into_iter()
            .any(|link| link.enabled && link.transport_security == "tls_required")
    {
        return Err(ServeError::Config(
            "binkp_listener needs a TLS identity when enabled links require TLS".into(),
        ));
    }

    let listener = TcpListener::bind(bind).await?;
    let state = BinkpListenerState {
        config,
        db,
        session_registry: Arc::new(LinkSessionRegistry::new()),
        tls_config,
        connection_limit: Arc::new(Semaphore::new(max_connections as usize)),
    };

    info!(bind = %listener.local_addr()?, "BinkP listener started");

    Ok(tokio::spawn(async move {
        binkp_accept_loop(listener, state).await
    }))
}

async fn binkp_accept_loop(listener: TcpListener, state: BinkpListenerState) -> ServeResult<()> {
    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                let state = state.clone();
                let permit = match state.connection_limit.clone().acquire_owned().await {
                    Ok(permit) => permit,
                    Err(error) => {
                        return Err(ServeError::Runtime(format!(
                            "BinkP connection limiter closed: {error}"
                        )));
                    }
                };
                tokio::spawn(async move {
                    let _permit = permit;
                    if let Err(error) = handle_binkp_connection(stream, peer_addr, state).await {
                        warn!(%peer_addr, %error, "BinkP connection failed");
                    }
                });
            }
            Err(error) => {
                error!(%error, "BinkP accept failed");
            }
        }
    }
}

async fn handle_binkp_connection(
    stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    state: BinkpListenerState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!(%peer_addr, "BinkP connection accepted");

    // Convert tokio stream to std stream for synchronous BinkP implementation
    let std_stream = stream.into_std()?;
    std_stream.set_nonblocking(false)?;

    // Use spawn_blocking to run synchronous BinkP protocol
    tokio::task::spawn_blocking(move || handle_binkp_session_sync(std_stream, peer_addr, state))
        .await?
}

fn handle_binkp_session_sync(
    stream: std::net::TcpStream,
    peer_addr: SocketAddr,
    state: BinkpListenerState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = prepare_binkp_stream(stream, &state)?;
    let secure_transport = stream.is_tls();
    let server = BinkpServer::new();

    // Build allowed addresses and per-link passwords from all enabled links
    let links = list_network_links(state.db.db())?;
    let allowed_addresses: Vec<String> = links
        .iter()
        .filter(|link| link.enabled)
        .map(|link| link.address.clone())
        .collect();

    let link_passwords: HashMap<String, String> = links
        .iter()
        .filter(|link| link.enabled && !link.password.is_empty())
        .map(|link| (link.address.clone(), link.password.clone()))
        .collect();
    let tls_required_addresses = links
        .iter()
        .filter(|link| link.enabled && link.transport_security == "tls_required")
        .map(|link| link.address.clone())
        .collect::<Vec<_>>();

    // Perform server handshake with per-link password validation
    let handshake = BinkpServerHandshake::with_link_passwords_and_tls_requirements(
        allowed_addresses,
        link_passwords,
        tls_required_addresses,
    );

    let session = server.accept_handshake_with_transport_security(
        &mut stream,
        &handshake,
        secure_transport,
    )?;

    if !session.authenticated {
        warn!(%peer_addr, "BinkP handshake failed: not authenticated");
        return Ok(());
    }

    // Find the link that authenticated
    let link = find_authenticated_link(&session.peer_addresses, &links)?;

    // Acquire session permit
    let _permit = state.session_registry.try_acquire(&link.key)?;

    info!(
        %peer_addr,
        link_id = %link.id,
        link_key = %link.key,
        "BinkP session established"
    );

    // Receive files from remote
    let received_files = server.receive_batch(&mut stream)?;

    // Write files to inbound spool
    let profile = find_network_profile_by_id(state.db.db(), &link.network_id)?
        .ok_or_else(|| format!("Profile not found for network_id: {}", link.network_id))?;
    let spool_path = TosserPaths::under_runtime(&state.config.paths.runtime, &profile.key);
    std::fs::create_dir_all(&spool_path.inbound_drop)?;

    for file in received_files {
        let file_path = available_spool_destination(&spool_path.inbound_drop.join(&file.name));
        std::fs::write(&file_path, &file.bytes)?;
        info!(
            %peer_addr,
            file = %file.name,
            bytes = file.bytes.len(),
            "BinkP file received"
        );
    }

    // Send outbound files to remote
    let scanner_paths = ScannerPaths::under_runtime(&state.config.paths.runtime, &profile.key);
    let outbound_ready = scanner_paths.outbound_root.join(&link.key).join("ready");
    let outbound_files =
        outbound_files_for_listener(&state.db, &profile.id, &link, &outbound_ready)?;
    let binkp_files = outbound_files
        .iter()
        .map(|file| file.file.clone())
        .collect::<Vec<_>>();

    if !binkp_files.is_empty() {
        info!(
            %peer_addr,
            files = binkp_files.len(),
            "Sending outbound BinkP files"
        );
        server.send_batch_with_acknowledgements(&mut stream, &binkp_files)?;

        // Mark outbound files as processed after successful send
        for file in &outbound_files {
            if let Some(packet_id) = &file.packet_id {
                finish_network_packet(state.db.db(), packet_id, "processed", None)?;
            }
            if file.path.exists() {
                std::fs::remove_file(&file.path)?;
            }
        }
    } else {
        // Send end of batch if no outbound files
        server.send_end_of_batch(&mut stream)?;
    }

    info!(%peer_addr, "BinkP session completed");
    Ok(())
}

fn prepare_binkp_stream(
    stream: std::net::TcpStream,
    state: &BinkpListenerState,
) -> Result<BinkpStream, Box<dyn std::error::Error + Send + Sync>> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    let mut first_byte = [0_u8; 1];
    let is_tls = stream
        .peek(&mut first_byte)
        .is_ok_and(|read| read == 1 && first_byte[0] == 0x16);
    if !is_tls {
        return Ok(BinkpStream::plain(stream));
    }

    let tls_config = state
        .tls_config
        .as_ref()
        .ok_or("received TLS BinkP connection but listener TLS is not configured")?;
    let tls_stream = accept_tls(stream, tls_config)?;
    Ok(BinkpStream::tls(tls_stream))
}

fn find_authenticated_link(
    peer_addresses: &[String],
    links: &[NetworkLinkRecord],
) -> Result<NetworkLinkRecord, Box<dyn std::error::Error + Send + Sync>> {
    links
        .iter()
        .find(|link| {
            link.enabled
                && peer_addresses
                    .iter()
                    .any(|address| address == &link.address)
        })
        .cloned()
        .ok_or_else(|| format!("No enabled link found for addresses: {peer_addresses:?}").into())
}

#[derive(Debug)]
struct ListenerOutboundFile {
    file: BinkpOutboundFile,
    path: PathBuf,
    packet_id: Option<String>,
}

fn outbound_files_for_listener(
    db: &OxideDb,
    network_id: &str,
    link: &NetworkLinkRecord,
    outbound_ready: &Path,
) -> Result<Vec<ListenerOutboundFile>, Box<dyn std::error::Error + Send + Sync>> {
    let packets = list_network_packets(db.db())?
        .into_iter()
        .filter(|packet| {
            packet.network_id == network_id
                && packet.link_id.as_deref() == Some(link.id.as_str())
                && packet.direction == "outbound"
                && packet.status == "pending"
        })
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    for packet in packets {
        files.push(outbound_file_from_packet(&packet)?);
    }

    if outbound_ready.exists() {
        for entry in std::fs::read_dir(outbound_ready)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && !files
                    .iter()
                    .any(|file: &ListenerOutboundFile| file.path == path)
            {
                files.push(outbound_file_from_path(path, None)?);
            }
        }
    }

    Ok(files)
}

fn outbound_file_from_packet(
    packet: &NetworkPacketRecord,
) -> Result<ListenerOutboundFile, Box<dyn std::error::Error + Send + Sync>> {
    outbound_file_from_path(PathBuf::from(&packet.filename), Some(packet.id.clone()))
}

fn outbound_file_from_path(
    path: PathBuf,
    packet_id: Option<String>,
) -> Result<ListenerOutboundFile, Box<dyn std::error::Error + Send + Sync>> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("Invalid file name: {path:?}"))?
        .to_string_lossy()
        .to_string();
    let metadata = std::fs::metadata(&path)?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_else(current_unix_seconds);
    let bytes = std::fs::read(&path)?;
    let file = BinkpOutboundFile::new(name, mtime, bytes)
        .map_err(|error| format!("Invalid outbound file: {error}"))?;
    Ok(ListenerOutboundFile {
        file,
        path,
        packet_id,
    })
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn available_spool_destination(destination: &Path) -> PathBuf {
    if !destination.exists() {
        return destination.to_path_buf();
    }

    let parent = destination
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let stem = destination
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("inbound");
    let extension = destination
        .extension()
        .and_then(|extension| extension.to_str());
    for index in 1.. {
        let candidate_name = match extension {
            Some(extension) => format!("{stem}.{index}.{extension}"),
            None => format!("{stem}.{index}"),
        };
        let candidate = parent.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search returns a free path")
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidebbs_binkp::{
        BinkpClient, BinkpClientHandshake, BinkpTlsCertificate, BinkpTlsClientConfig,
        BinkpTlsIdentity, certificate_from_pem, connect_tls, identity_from_pkcs8_pem,
    };
    use oxidebbs_db::{
        NetworkPacketRecord, NetworkProfileRecord, insert_network_link, insert_network_packet,
        insert_network_profile, list_network_packets,
    };
    use std::net::TcpListener as StdTcpListener;
    use std::thread;

    fn test_config() -> Arc<OxideConfig> {
        test_config_under(std::env::temp_dir())
    }

    fn test_config_under(runtime: impl AsRef<Path>) -> Arc<OxideConfig> {
        let runtime = runtime.as_ref().display();
        let toml = r#"
[board]
name = "Example BBS"

[paths]
runtime = "__RUNTIME__"

[network]
enabled = true

[network.binkp_listener]
enabled = true
bind = "127.0.0.1:24554"
max_connections = 10
"#
        .replace("__RUNTIME__", &runtime.to_string());
        Arc::new(toml::from_str(&toml).expect("parse config"))
    }

    #[test]
    fn test_binkp_listener_state_clone() {
        let config = test_config();
        let db = Arc::new(OxideDb::open_memory().expect("open DB"));
        let state = BinkpListenerState {
            config,
            db,
            session_registry: Arc::new(LinkSessionRegistry::new()),
            tls_config: None,
            connection_limit: Arc::new(Semaphore::new(10)),
        };

        let cloned = state.clone();
        assert!(Arc::ptr_eq(&state.config, &cloned.config));
        assert!(Arc::ptr_eq(&state.db, &cloned.db));
    }

    #[test]
    fn listener_accepts_tls_for_tls_required_link() {
        let db = Arc::new(OxideDb::open_memory().expect("open DB"));
        let profile = test_profile();
        let mut link = test_link(&profile.id);
        link.transport_security = "tls_required".to_string();
        insert_network_profile(db.db(), &profile).expect("insert profile");
        insert_network_link(db.db(), &link).expect("insert link");
        let root = temp_root("listener-tls");
        let fixture = tls_fixture("localhost");
        let state = BinkpListenerState {
            config: test_config_under(&root),
            db,
            session_registry: Arc::new(LinkSessionRegistry::new()),
            tls_config: Some(Arc::new(BinkpTlsServerConfig {
                identity: fixture.identity,
                require_client_cert: false,
                client_certificates: Vec::new(),
            })),
            connection_limit: Arc::new(Semaphore::new(10)),
        };
        let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let server = thread::spawn(move || {
            let (stream, peer_addr) = listener.accept().expect("accept");
            handle_binkp_session_sync(stream, peer_addr, state).expect("handle TLS session");
        });

        let stream = std::net::TcpStream::connect(addr).expect("connect");
        let mut stream = connect_tls(
            stream,
            "localhost",
            &BinkpTlsClientConfig {
                verify_certificates: true,
                root_certificates: vec![fixture.certificate],
                client_identity: None,
            },
        )
        .expect("TLS connect");
        let client = BinkpClient::new();
        client
            .handshake(
                &mut stream,
                &BinkpClientHandshake::new(vec!["1:105/1".to_string()], Some("SECRET".to_string())),
            )
            .expect("client handshake");
        client
            .send_batch(&mut stream, &[])
            .expect("send empty batch");
        let received = client.receive_batch(&mut stream).expect("receive batch");
        assert!(received.is_empty());
        server.join().expect("server joined");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn listener_rejects_plaintext_for_tls_required_link() {
        let db = Arc::new(OxideDb::open_memory().expect("open DB"));
        let profile = test_profile();
        let mut link = test_link(&profile.id);
        link.transport_security = "tls_required".to_string();
        insert_network_profile(db.db(), &profile).expect("insert profile");
        insert_network_link(db.db(), &link).expect("insert link");
        let root = temp_root("listener-tls-required");
        let state = BinkpListenerState {
            config: test_config_under(&root),
            db,
            session_registry: Arc::new(LinkSessionRegistry::new()),
            tls_config: None,
            connection_limit: Arc::new(Semaphore::new(10)),
        };
        let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let server = thread::spawn(move || {
            let (stream, peer_addr) = listener.accept().expect("accept");
            handle_binkp_session_sync(stream, peer_addr, state)
                .expect_err("plaintext TLS-required session rejected");
        });

        let mut stream = std::net::TcpStream::connect(addr).expect("connect");
        let client = BinkpClient::new();
        let error = client
            .handshake(
                &mut stream,
                &BinkpClientHandshake::new(vec!["1:105/1".to_string()], Some("SECRET".to_string())),
            )
            .expect_err("client sees refusal");
        assert!(matches!(
            error,
            oxidebbs_binkp::BinkpError::ConnectionRefused
        ));
        server.join().expect("server joined");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn listener_sends_outbound_and_marks_packet_processed() {
        let db = Arc::new(OxideDb::open_memory().expect("open DB"));
        let profile = test_profile();
        let link = test_link(&profile.id);
        insert_network_profile(db.db(), &profile).expect("insert profile");
        insert_network_link(db.db(), &link).expect("insert link");
        let root = temp_root("listener-outbound");
        let outbound_dir = root.join("network/fidonet/outbound/hub/ready");
        std::fs::create_dir_all(&outbound_dir).expect("create outbound");
        let outbound_path = outbound_dir.join("00000001.pkt");
        std::fs::write(&outbound_path, b"outbound packet").expect("write outbound");
        insert_network_packet(
            db.db(),
            &NetworkPacketRecord {
                id: "00000000-0000-4000-8000-000000003103".to_string(),
                network_id: profile.id.clone(),
                direction: "outbound".to_string(),
                link_id: Some(link.id.clone()),
                filename: outbound_path.display().to_string(),
                sha256: "hash".to_string(),
                size_bytes: 15,
                status: "pending".to_string(),
                error_message: None,
                received_at: None,
                processed_at: None,
                created_at: "2026-06-04T00:00:00Z".to_string(),
            },
        )
        .expect("insert packet");
        let state = BinkpListenerState {
            config: test_config_under(&root),
            db: Arc::clone(&db),
            session_registry: Arc::new(LinkSessionRegistry::new()),
            tls_config: None,
            connection_limit: Arc::new(Semaphore::new(10)),
        };
        let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let server = thread::spawn(move || {
            let (stream, peer_addr) = listener.accept().expect("accept");
            handle_binkp_session_sync(stream, peer_addr, state).expect("handle plaintext session");
        });

        let mut stream = std::net::TcpStream::connect(addr).expect("connect");
        let client = BinkpClient::new();
        client
            .handshake(
                &mut stream,
                &BinkpClientHandshake::new(vec!["1:105/1".to_string()], Some("SECRET".to_string())),
            )
            .expect("client handshake");
        client
            .send_batch(&mut stream, &[])
            .expect("send empty batch");
        let received = client.receive_batch(&mut stream).expect("receive outbound");
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].name, "00000001.pkt");
        assert_eq!(received[0].bytes, b"outbound packet");
        server.join().expect("server joined");

        let packets = list_network_packets(db.db()).expect("list packets");
        assert_eq!(packets[0].status, "processed");
        assert!(!outbound_path.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    fn test_profile() -> NetworkProfileRecord {
        NetworkProfileRecord {
            id: "00000000-0000-4000-8000-000000003101".to_string(),
            key: "fidonet".to_string(),
            name: "FidoNet".to_string(),
            adapter: "legacy-ftn".to_string(),
            local_zone: 1,
            local_net: 105,
            local_node: 42,
            local_point: 0,
            enabled: true,
            created_at: "2026-06-04T00:00:00Z".to_string(),
            updated_at: "2026-06-04T00:00:00Z".to_string(),
        }
    }

    fn test_link(network_id: &str) -> NetworkLinkRecord {
        NetworkLinkRecord {
            id: "00000000-0000-4000-8000-000000003102".to_string(),
            key: "hub".to_string(),
            network_id: network_id.to_string(),
            address: "1:105/1".to_string(),
            host: "127.0.0.1".to_string(),
            binkp_port: 24554,
            password: "SECRET".to_string(),
            poll_schedule_minutes: 60,
            compression: "none".to_string(),
            transport_security: "plaintext_legacy".to_string(),
            enabled: true,
            created_at: "2026-06-04T00:00:00Z".to_string(),
            updated_at: "2026-06-04T00:00:00Z".to_string(),
        }
    }

    struct TlsFixture {
        identity: BinkpTlsIdentity,
        certificate: BinkpTlsCertificate,
    }

    fn tls_fixture(hostname: &str) -> TlsFixture {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec![hostname.to_string()])
                .expect("generate certificate");
        let cert_pem = cert.pem();
        let key_pem = signing_key.serialize_pem();
        TlsFixture {
            identity: identity_from_pkcs8_pem(cert_pem.as_bytes(), key_pem.as_bytes())
                .expect("identity"),
            certificate: certificate_from_pem(cert_pem.as_bytes()).expect("certificate"),
        }
    }

    fn temp_root(test_name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("oxidebbs-binkp-listener-{test_name}-{suffix}"))
    }
}
