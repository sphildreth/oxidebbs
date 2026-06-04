use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::{error, info, warn};

use oxidebbs_binkp::{BinkpOutboundFile, BinkpServer, BinkpServerHandshake, LinkSessionRegistry};
use oxidebbs_db::{NetworkLinkRecord, OxideDb, find_network_profile_by_id};
use oxidebbs_ftn::{ScannerPaths, TosserPaths};

use crate::config::OxideConfig;
use crate::serve::{ServeError, ServeResult};

#[derive(Clone)]
struct BinkpListenerState {
    config: Arc<OxideConfig>,
    db: Arc<OxideDb>,
    session_registry: Arc<LinkSessionRegistry>,
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

    let listener = TcpListener::bind(bind).await?;
    let state = BinkpListenerState {
        config,
        db,
        session_registry: Arc::new(LinkSessionRegistry::new()),
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
                tokio::spawn(async move {
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
    mut stream: std::net::TcpStream,
    peer_addr: SocketAddr,
    state: BinkpListenerState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = BinkpServer::new();

    // Build allowed addresses and per-link passwords from all enabled links
    let links = oxidebbs_db::list_network_links(state.db.db())?;
    let allowed_addresses: Vec<String> = links
        .iter()
        .filter(|link| link.enabled)
        .map(|link| link.address.clone())
        .collect();

    let link_passwords: HashMap<String, String> = links
        .iter()
        .filter(|link| link.enabled)
        .map(|link| (link.address.clone(), link.password.clone()))
        .collect();

    // Perform server handshake with per-link password validation
    let handshake = BinkpServerHandshake::with_link_passwords(allowed_addresses, link_passwords);

    let session = server.accept_handshake(&mut stream, &handshake)?;

    if !session.authenticated {
        warn!(%peer_addr, "BinkP handshake failed: not authenticated");
        return Ok(());
    }

    // Find the link that authenticated
    let peer_address = session
        .peer_addresses
        .first()
        .ok_or("No peer address in session")?;

    let link = find_authenticated_link(peer_address, &links)?;

    // Acquire session permit
    let _permit = state.session_registry.try_acquire(&link.id)?;

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

    for file in received_files {
        let file_path = spool_path.inbound_drop.join(&file.name);
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
    let mut outbound_files = Vec::new();
    if outbound_ready.exists() {
        for entry in std::fs::read_dir(&outbound_ready)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let name = path
                    .file_name()
                    .ok_or_else(|| format!("Invalid file name: {:?}", path))?
                    .to_string_lossy()
                    .to_string();
                let metadata = std::fs::metadata(&path)?;
                let mtime = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let bytes = std::fs::read(&path)?;
                outbound_files.push(
                    BinkpOutboundFile::new(name, mtime, bytes)
                        .map_err(|e| format!("Invalid outbound file: {e}"))?,
                );
            }
        }
    }

    if !outbound_files.is_empty() {
        info!(
            %peer_addr,
            files = outbound_files.len(),
            "Sending outbound BinkP files"
        );
        server.send_batch_with_acknowledgements(&mut stream, &outbound_files)?;

        // Mark outbound files as processed after successful send
        for file in &outbound_files {
            let file_path = outbound_ready.join(&file.name);
            if file_path.exists() {
                std::fs::remove_file(&file_path)?;
            }
        }
    } else {
        // Send end of batch if no outbound files
        server.send_end_of_batch(&mut stream)?;
    }

    info!(%peer_addr, "BinkP session completed");
    Ok(())
}

fn find_authenticated_link(
    peer_address: &str,
    links: &[NetworkLinkRecord],
) -> Result<NetworkLinkRecord, Box<dyn std::error::Error + Send + Sync>> {
    links
        .iter()
        .find(|link| link.enabled && link.address == peer_address)
        .cloned()
        .ok_or_else(|| format!("No enabled link found for address: {}", peer_address).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Arc<OxideConfig> {
        let toml = r#"
[board]
name = "Example BBS"

[network]
enabled = true

[network.binkp_listener]
enabled = true
bind = "127.0.0.1:24554"
max_connections = 10
"#;
        Arc::new(toml::from_str(toml).expect("parse config"))
    }

    #[test]
    fn test_binkp_listener_state_clone() {
        let config = test_config();
        let db = Arc::new(OxideDb::open_memory().expect("open DB"));
        let state = BinkpListenerState {
            config,
            db,
            session_registry: Arc::new(LinkSessionRegistry::new()),
        };

        let cloned = state.clone();
        assert!(Arc::ptr_eq(&state.config, &cloned.config));
        assert!(Arc::ptr_eq(&state.db, &cloned.db));
    }
}
