use std::fmt::Write as FmtWrite;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;
use tracing::{error, info, warn};

use oxidebbs_core::menu::{Menu, MenuAction};
use oxidebbs_db::{
    AuditEventRecord, OxideDb, SessionRecord, end_session, insert_audit_event, insert_session,
};
use oxidebbs_telnet::{TcpTransport, TelnetEvent, TelnetParser, Transport, TransportError};
use oxidebbs_term::{
    LoadedScreen, ScreenAsset as TermScreenAsset, TerminalCapabilities, encode_cp437,
};

use crate::config::OxideConfig;

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("network error: {0}")]
    Network(#[from] std::io::Error),

    #[error("transport error: {0}")]
    Transport(#[from] TransportError),

    #[error("database error: {0}")]
    Database(#[from] oxidebbs_db::DbError),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("runtime error: {0}")]
    Runtime(String),
}

type ServeResult<T> = Result<T, ServeError>;

const REJECTION_MESSAGE: &str = "System is busy. Please try again later.\r\n";
const PROMPT_TERMINATOR: &str = "\r\n";
const MAIN_MENU_POST_LOGIN: &str = "Please choose from the menu.\r\n";
const LOGIN_PLACEHOLDER: &str = "Login flow placeholder: not implemented yet.\r\n";
const NEW_USER_PLACEHOLDER: &str = "New user flow placeholder: not implemented yet.\r\n";
const DOORS_PLACEHOLDER: &str = "Doors feature placeholder: not implemented yet.\r\n";
const MESSAGES_PLACEHOLDER: &str = "Messages feature placeholder: not implemented yet.\r\n";

pub async fn run(config: &OxideConfig) -> ServeResult<()> {
    if !config.telnet.enabled {
        info!(bind = %config.telnet.bind, "telnet disabled; service not started");
        return Ok(());
    }

    let db =
        Arc::new(OxideDb::open_or_create(&config.database.path).map_err(ServeError::Database)?);

    let login_menu = Arc::new(
        config
            .core_menu(&config.flow.login_menu)
            .map_err(|error| ServeError::Config(error.to_string()))?,
    );

    let main_menu = Arc::new(
        config
            .core_menu(&config.flow.main_menu)
            .map_err(|error| ServeError::Config(error.to_string()))?,
    );

    let node_slots = Arc::new(NodeCoordinator::new(
        config.nodes.count,
        config.telnet.max_connections,
    ));
    let listener = TcpListener::bind(&config.telnet.bind).await?;

    info!(bind = %config.telnet.bind, "listening for telnet callers");

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let peer = CallerPeer {
            address: peer_addr.to_string(),
            ip: peer_addr.ip().to_string(),
            port: i64::from(peer_addr.port()),
        };

        if let Some(allocation) = node_slots.try_allocate() {
            let db = Arc::clone(&db);
            let config = Arc::new(config.clone());
            let login_menu = Arc::clone(&login_menu);
            let main_menu = Arc::clone(&main_menu);

            tokio::spawn(async move {
                if let Err(error) =
                    handle_caller(allocation, stream, peer, db, config, login_menu, main_menu).await
                {
                    warn!("caller session ended with error: {error}");
                }
            });
        } else {
            tokio::spawn(async move {
                if let Err(error) = reject_connection(stream).await {
                    warn!("failed to reject caller: {error}");
                }
            });
        }
    }
}

async fn reject_connection(mut stream: TcpStream) -> ServeResult<()> {
    let bytes = encode_text(REJECTION_MESSAGE);
    stream.write_all(&bytes).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn handle_caller(
    allocation: NodeAllocation,
    stream: TcpStream,
    peer: CallerPeer,
    db: Arc<OxideDb>,
    config: Arc<OxideConfig>,
    login_menu: Arc<Menu>,
    main_menu: Arc<Menu>,
) -> ServeResult<()> {
    let node_number = i64::from(allocation.node_number);
    let session_id = generated_uuid(&db)?;
    let connected_at = current_timestamp(&db)?;
    let mut transport = TcpTransport::new(stream);
    let mut parser = TelnetParser::default();
    let mut capabilities = TerminalCapabilities::ansi_80();
    let idle_timeout = Duration::from_secs(config.telnet.idle_timeout_seconds);

    insert_session(
        db.db(),
        &SessionRecord {
            id: session_id.clone(),
            node_number,
            user_id: None,
            transport: "telnet".to_string(),
            remote_address: peer.address.clone(),
            remote_ip: Some(peer.ip),
            remote_port: Some(peer.port),
            started_at: connected_at.clone(),
            ended_at: None,
            disconnect_reason: None,
        },
    )
    .map_err(|error| {
        error!("failed to insert session record: {error}");
        ServeError::Database(error)
    })?;

    if let Err(error) = insert_audit_event(
        db.db(),
        &AuditEventRecord {
            id: generated_uuid(&db)?,
            created_at: connected_at,
            event_type: "caller_connected".to_string(),
            user_id: None,
            node_number: Some(node_number),
            details: format!("caller connected from {}", peer.address),
        },
    ) {
        warn!("failed to insert caller_connected event: {error}");
    }

    if config.terminal.clear_screen_on_connect {
        transport
            .write_all(oxidebbs_term::CLEAR_SCREEN_AND_HOME)
            .await
            .map_err(ServeError::Transport)?;
    }

    send_login_flow(&mut transport, &config, &login_menu, &mut capabilities).await?;

    let mut in_main_menu = false;
    let mut disconnect_reason = "caller_disconnected".to_string();

    loop {
        let event = next_event(&mut transport, &mut parser, idle_timeout).await;
        let event = match event {
            Ok(CallerInput::Event(event)) => event,
            Ok(CallerInput::Disconnected) => {
                if !in_main_menu {
                    disconnect_reason = "caller_dropped_during_login".to_string();
                }
                break;
            }
            Ok(CallerInput::IdleTimeout) => {
                disconnect_reason = "idle_timeout".to_string();
                send_text(&mut transport, "Idle timeout. Goodbye.\r\n").await?;
                break;
            }
            Err(error) => {
                warn!("caller transport failed: {error}");
                return Err(error);
            }
        };

        match event {
            TelnetEvent::Data(raw_key) => {
                let key = match normalize_key(raw_key) {
                    Some(key) => key,
                    None => continue,
                };

                if !in_main_menu {
                    match login_menu.route(&key) {
                        Some(MenuAction::Login) => {
                            send_text(&mut transport, LOGIN_PLACEHOLDER).await?;
                            show_post_login_screens(&mut transport, &config, &mut capabilities)
                                .await?;
                            send_main_menu(&mut transport, &config, &main_menu, &mut capabilities)
                                .await?;
                            in_main_menu = true;
                        }
                        Some(MenuAction::NewUser) => {
                            send_text(&mut transport, NEW_USER_PLACEHOLDER).await?;
                            show_post_login_screens(&mut transport, &config, &mut capabilities)
                                .await?;
                            send_main_menu(&mut transport, &config, &main_menu, &mut capabilities)
                                .await?;
                            in_main_menu = true;
                        }
                        Some(MenuAction::Logoff) => {
                            disconnect_reason = "caller_logoff".to_string();
                            send_text(&mut transport, "Goodbye.\r\n").await?;
                            break;
                        }
                        _ => {
                            send_text(&mut transport, "Select Login, New User, or Goodbye.\r\n")
                                .await?;
                            send_menu_prompt(&mut transport, &login_menu).await?;
                        }
                    }
                } else {
                    match main_menu.route(&key) {
                        Some(MenuAction::Doors) => {
                            send_text(&mut transport, DOORS_PLACEHOLDER).await?;
                            send_menu_prompt(&mut transport, &main_menu).await?;
                        }
                        Some(MenuAction::Messages) => {
                            send_text(&mut transport, MESSAGES_PLACEHOLDER).await?;
                            send_menu_prompt(&mut transport, &main_menu).await?;
                        }
                        Some(MenuAction::NewUser) => {
                            send_text(&mut transport, NEW_USER_PLACEHOLDER).await?;
                            send_menu_prompt(&mut transport, &main_menu).await?;
                        }
                        Some(MenuAction::Logoff) => {
                            disconnect_reason = "caller_logoff".to_string();
                            send_text(&mut transport, "Goodbye.\r\n").await?;
                            break;
                        }
                        Some(MenuAction::ShowScreen { screen }) => {
                            send_screen(&mut transport, &config, &screen.asset, &mut capabilities)
                                .await?;
                            send_menu_prompt(&mut transport, &main_menu).await?;
                        }
                        Some(MenuAction::Submenu { .. }) => {
                            send_text(&mut transport, "Submenus are not yet implemented.\r\n")
                                .await?;
                            send_menu_prompt(&mut transport, &main_menu).await?;
                        }
                        Some(MenuAction::Login) => {
                            send_text(&mut transport, LOGIN_PLACEHOLDER).await?;
                            send_menu_prompt(&mut transport, &main_menu).await?;
                        }
                        Some(MenuAction::Noop) => {}
                        None => {
                            send_text(&mut transport, "Unknown option.\r\n").await?;
                            send_menu_prompt(&mut transport, &main_menu).await?;
                        }
                    }
                }
            }
            TelnetEvent::WindowSize { columns, .. } => {
                if columns > 0 {
                    capabilities.width = columns;
                }
            }
            TelnetEvent::Negotiation { .. }
            | TelnetEvent::TerminalType(_)
            | TelnetEvent::TerminalTypeRequest
            | TelnetEvent::Subnegotiation { .. } => {}
        }
    }

    if let Err(error) = transport.hangup().await {
        warn!("failed to hang up telnet transport: {error}");
    }

    let ended_at = current_timestamp(&db)?;
    if let Err(error) = end_session(db.db(), &session_id, &ended_at, &disconnect_reason) {
        warn!("failed to close session record: {error}");
    }

    if let Err(error) = insert_audit_event(
        db.db(),
        &AuditEventRecord {
            id: generated_uuid(&db)?,
            created_at: ended_at,
            event_type: "caller_disconnected".to_string(),
            user_id: None,
            node_number: Some(node_number),
            details: format!("disconnect reason: {disconnect_reason}"),
        },
    ) {
        warn!("failed to insert caller_disconnected event: {error}");
    }

    info!(
        node = %node_number,
        remote = %peer.address,
        reason = %disconnect_reason,
        "session ended"
    );

    drop(allocation);
    Ok(())
}

async fn send_login_flow(
    transport: &mut TcpTransport,
    config: &OxideConfig,
    login_menu: &Menu,
    capabilities: &mut TerminalCapabilities,
) -> ServeResult<()> {
    send_screen(transport, config, &config.flow.login_screen, capabilities).await?;
    send_menu_prompt(transport, login_menu).await
}

async fn send_main_menu(
    transport: &mut TcpTransport,
    config: &OxideConfig,
    menu: &Menu,
    capabilities: &mut TerminalCapabilities,
) -> ServeResult<()> {
    send_screen(transport, config, &menu.screen.asset, capabilities).await?;
    send_menu_prompt(transport, menu).await
}

async fn send_menu_prompt(transport: &mut TcpTransport, menu: &Menu) -> ServeResult<()> {
    let prompt = menu
        .description
        .clone()
        .unwrap_or_else(|| "Command? ".to_string());
    send_text(transport, &prompt).await?;
    Ok(())
}

async fn show_post_login_screens(
    transport: &mut TcpTransport,
    config: &OxideConfig,
    capabilities: &mut TerminalCapabilities,
) -> ServeResult<()> {
    for screen in &config.flow.post_login_screens {
        send_screen(transport, config, screen, capabilities).await?;
    }
    send_text(transport, MAIN_MENU_POST_LOGIN).await
}

async fn send_screen(
    transport: &mut TcpTransport,
    config: &OxideConfig,
    screen_key: &str,
    capabilities: &mut TerminalCapabilities,
) -> ServeResult<()> {
    let payload = load_screen_payload(config, screen_key, *capabilities)
        .unwrap_or_else(|error| fallback_screen_payload(screen_key, &error));
    transport.write_all(&payload).await?;
    Ok(())
}

fn load_screen_payload(
    config: &OxideConfig,
    screen_key: &str,
    capabilities: TerminalCapabilities,
) -> Result<Vec<u8>, String> {
    let Some(screen_config) = config.screens.get(screen_key) else {
        return Err(format!("missing screen key {screen_key}"));
    };

    let term_screen = TermScreenAsset {
        ansi: screen_config.ansi.clone(),
        ansi_40: screen_config.ansi_40.clone(),
        ascii: screen_config.ascii.clone(),
        text: screen_config.text.clone(),
        pause: screen_config.pause,
    };

    match term_screen.load(&config.paths.screens, capabilities) {
        Ok(LoadedScreen::Ansi(bytes)) => Ok(bytes),
        Ok(LoadedScreen::PlainText(text)) => Ok(encode_text(&text)),
        Err(error) => Err(error.to_string()),
    }
}

fn fallback_screen_payload(screen_key: &str, details: &str) -> Vec<u8> {
    let mut message = String::new();
    let _ = writeln!(&mut message, "[{}]", screen_key);
    let _ = write!(&mut message, "{details}");
    message.push_str(PROMPT_TERMINATOR);
    encode_text(&message)
}

async fn send_text(transport: &mut TcpTransport, message: &str) -> ServeResult<()> {
    let bytes = encode_text(message);
    transport.write_all(&bytes).await?;
    Ok(())
}

fn encode_text(text: &str) -> Vec<u8> {
    match encode_cp437(text) {
        Ok(bytes) => bytes,
        Err(_) => text.as_bytes().to_vec(),
    }
}

fn normalize_key(byte: u8) -> Option<String> {
    let ch = byte as char;
    if !ch.is_ascii() || ch.is_ascii_control() {
        return None;
    }
    Some(ch.to_ascii_uppercase().to_string())
}

async fn next_event<T: Transport>(
    transport: &mut T,
    parser: &mut TelnetParser,
    idle_timeout: Duration,
) -> ServeResult<CallerInput> {
    loop {
        let read = timeout(idle_timeout, transport.read_byte()).await;
        let byte = match read {
            Ok(Ok(Some(byte))) => byte,
            Ok(Ok(None)) => return Ok(CallerInput::Disconnected),
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => return Ok(CallerInput::IdleTimeout),
        };

        let mut reply = Vec::new();
        let event = parser.feed(byte, &mut reply);
        if !reply.is_empty() {
            transport.write_all(&reply).await?;
        }

        if let Some(event) = event {
            return Ok(CallerInput::Event(event));
        }
    }
}

#[derive(Debug)]
enum CallerInput {
    Event(TelnetEvent),
    Disconnected,
    IdleTimeout,
}

struct CallerPeer {
    address: String,
    ip: String,
    port: i64,
}

fn generated_uuid(db: &OxideDb) -> ServeResult<String> {
    db_scalar_text(db, "SELECT UUID_TO_STRING(GEN_RANDOM_UUID())")
}

fn current_timestamp(db: &OxideDb) -> ServeResult<String> {
    db_scalar_text(db, "SELECT CAST(NOW() AS TEXT)")
}

fn db_scalar_text(db: &OxideDb, sql: &str) -> ServeResult<String> {
    let result = db.db().execute(sql).map_err(ServeError::Database)?;
    let value = result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .ok_or_else(|| ServeError::Runtime(format!("query returned no scalar value: {sql}")))?;

    match value {
        oxidebbs_db::Value::Text(value) => Ok(value.clone()),
        other => Err(ServeError::Runtime(format!(
            "query returned non-text scalar for {sql}: {other:?}"
        ))),
    }
}

struct NodeCoordinator {
    occupied: Mutex<Vec<bool>>,
    limit: Arc<Semaphore>,
}

impl NodeCoordinator {
    fn new(node_count: u16, max_connections: u32) -> Self {
        let node_count = usize::from(node_count);
        let max_connections = usize::try_from(max_connections).unwrap_or(usize::MAX);
        let max_slots = node_count.min(max_connections);

        Self {
            occupied: Mutex::new(vec![false; node_count]),
            limit: Arc::new(Semaphore::new(max_slots)),
        }
    }

    fn try_allocate(self: &Arc<Self>) -> Option<NodeAllocation> {
        let permit = self.limit.clone().try_acquire_owned().ok()?;

        let mut occupied = self.occupied.lock().ok()?;
        let Some(index) = occupied.iter().position(|used| !*used) else {
            drop(permit);
            return None;
        };

        occupied[index] = true;

        Some(NodeAllocation {
            node_number: (index + 1) as u16,
            coordinator: Arc::clone(self),
            _permit: permit,
        })
    }
}

struct NodeAllocation {
    node_number: u16,
    coordinator: Arc<NodeCoordinator>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for NodeAllocation {
    fn drop(&mut self) {
        if let Ok(mut occupied) = self.coordinator.occupied.lock()
            && self.node_number > 0
        {
            let index = usize::from(self.node_number - 1);
            if let Some(slot) = occupied.get_mut(index) {
                *slot = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidebbs_telnet::{
        LoopbackTransport,
        telnet::{DO, IAC, TELOPT_SUPPRESS_GO_AHEAD},
    };

    #[test]
    fn normalize_key_uppercases_ascii() {
        assert_eq!(normalize_key(b'l').as_deref(), Some("L"));
        assert_eq!(normalize_key(b'R').as_deref(), Some("R"));
    }

    #[test]
    fn normalize_key_rejects_control_chars() {
        assert_eq!(normalize_key(b'\n'), None);
        assert_eq!(normalize_key(0x00), None);
        assert_eq!(normalize_key(0x1b), None);
    }

    #[test]
    fn fallback_payload_includes_context() {
        let payload = fallback_screen_payload("login", "missing file");
        let decoded = String::from_utf8_lossy(&payload);

        assert!(decoded.contains("[login]"));
        assert!(decoded.contains("missing file"));
    }

    #[test]
    fn node_slots_are_reused_after_drop() {
        let coordinator = Arc::new(NodeCoordinator::new(2, 4));
        let first = coordinator.try_allocate().expect("first slot");
        let second = coordinator.try_allocate().expect("second slot");

        assert!(coordinator.try_allocate().is_none());

        drop(first);
        let third = coordinator.try_allocate().expect("slot should be released");

        assert!(third.node_number > 0);
        drop(second);
        drop(third);
    }

    #[tokio::test]
    async fn next_event_reads_through_telnet_negotiation_frames() {
        let (mut transport, client) = LoopbackTransport::new();
        client
            .write_bytes(&[IAC, DO, TELOPT_SUPPRESS_GO_AHEAD])
            .expect("write negotiation");
        let mut parser = TelnetParser::default();

        let event = next_event(&mut transport, &mut parser, Duration::from_secs(1))
            .await
            .expect("read event");

        match event {
            CallerInput::Event(TelnetEvent::Negotiation { accepted, .. }) => {
                assert!(accepted);
            }
            other => panic!("expected negotiation event, got {other:?}"),
        }
    }
}
