use std::collections::{HashMap, VecDeque};
use std::fmt::Write as FmtWrite;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use argon2::password_hash::{PasswordHash, PasswordHasher, SaltString};
use argon2::{Algorithm, Argon2, Params, PasswordVerifier as Argon2PasswordVerifier, Version};
use rand_core::OsRng;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::oneshot;
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

use oxidebbs_core::auth::{NewUserInput, create_new_user};
use oxidebbs_core::menu::{Menu, MenuAction};
use oxidebbs_core::message::{
    AreaKind, Message, MessageArea, MessageVisibility, PostMessageCommand, ReplyMessageCommand,
    post_message, reply_message,
};
use oxidebbs_core::user::{User, UserStatus};
use oxidebbs_db::{
    AuditEventRecord, FileAreaRecord, FileEntryRecord, FileTransferRecord, MessageAreaRecord,
    MessageRecord, OxideDb, SessionRecord, UserInsertError, UserRecord, clear_auth_attempt,
    end_session, find_file_entry_by_storage_name, find_user_by_alias_ci,
    increment_file_entry_download_count, insert_audit_event, insert_file_entry,
    insert_file_transfer, insert_message, insert_message_area, insert_session,
    insert_user_if_alias_available, is_auth_scope_locked, list_audit_events, list_auth_attempts,
    list_door_definitions, list_door_runs, list_message_areas, list_messages, list_recent_sessions,
    list_user_aliases_by_ids, list_users, list_visible_messages_in_area, normalize_alias,
    record_auth_failure, update_session_user, update_user_login,
};
use oxidebbs_telnet::telnet::{
    DO, IAC, SB, SE, TELOPT_ECHO, TELOPT_NAWS, TELOPT_SUPPRESS_GO_AHEAD, TELOPT_TERMINAL_TYPE,
    TELOPT_TTYPE_SEND, TelnetCommand, TelnetEvent, TelnetParser, WILL,
};
use oxidebbs_telnet::{
    SerialFlowControl, SerialParity, SerialPortConfig, SerialTransport, TcpTransport, Transport,
    TransportError,
};
use oxidebbs_term::{
    LoadedScreen, ScreenAsset as TermScreenAsset, TerminalCapabilities, TerminalCharset,
    TerminalProfile, char_to_petscii_byte, encode_cp437, render_petscii_lossy,
};
use oxidebbs_transfer::adapter::TransportAdapter;
use oxidebbs_transfer::{
    TransferError, TransferProtocol, sanitize_filename, validate_path_within_base,
};

use crate::config::{Argon2Config, AuthConfig, OxideConfig};
use crate::control::{
    ControlError, NodeAllocation, RuntimeNodeCommands, ServerRuntime, start_control_listener,
};
use crate::door_session::{
    DoorExecutionSummary, DoorSelection, DoorService, render_door_menu, select_door,
};

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

pub(crate) type ServeResult<T> = Result<T, ServeError>;

const REJECTION_MESSAGE: &str = "System is busy. Please try again later.\r\n";
const INVALID_LOGIN_MESSAGE: &str = "Invalid alias or password. Please try again.\r\n";
const LOGIN_LOCKOUT_MESSAGE: &str = "Too many login attempts. Try again later.\r\n";
const CP437_INPUT_REJECT_MESSAGE: &str = "This BBS only accepts CP437-compatible text here.";
const CP437_INPUT_REJECT_LINE: &str = "This BBS only accepts CP437-compatible text here.\r\n";
const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$b3hpZGViYnMtZHVtbXktYXV0aC1zYWx0$CNvsc4yCQyC6gccREXpHZ6l9604svk9VP98AyAVSMtY";
const PROMPT_TERMINATOR: &str = "\r\n";
const MAIN_MENU_POST_LOGIN: &str = "Please choose from the menu.\r\n";
const ACCESS_DENIED_MESSAGE: &str = "Access denied. Return to menu.\r\n";
const TERMINAL_CAPABILITY_NEGOTIATION_TIMEOUT: Duration = Duration::from_millis(300);
#[cfg(unix)]
const STALE_NODE_SWEEP_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct ScreenRenderContext {
    node_number: u16,
    node_count: u16,
    board_name: String,
    sysop_name: String,
    caller_alias: Option<String>,
    security_level: Option<i32>,
}

fn server_start_audit_details(config: &OxideConfig) -> String {
    let binkp_listener = config
        .network
        .binkp_listener
        .as_ref()
        .filter(|listener| config.network.enabled && listener.enabled);
    match (
        config.telnet.enabled,
        config.admin_web.enabled,
        binkp_listener,
    ) {
        (true, true, _) => format!(
            "serving {} on {} with {} node(s); admin web status on {}",
            config.board.name, config.telnet.bind, config.nodes.count, config.admin_web.bind
        ),
        (true, false, _) => format!(
            "serving {} on {} with {} node(s)",
            config.board.name, config.telnet.bind, config.nodes.count
        ),
        (false, true, _) => format!(
            "serving {} admin web status on {} with telnet disabled",
            config.board.name, config.admin_web.bind
        ),
        (false, false, Some(listener)) => format!(
            "serving {} BinkP listener on {} with telnet disabled",
            config.board.name, listener.bind
        ),
        (false, false, None) => format!("{} service disabled", config.board.name),
    }
}

pub async fn run(config: &OxideConfig, config_path: &Path) -> ServeResult<()> {
    run_until_shutdown(config, config_path, wait_for_shutdown_signal()).await
}

pub(crate) fn validate_startup_database(config: &OxideConfig) -> ServeResult<()> {
    if !config.telnet.enabled {
        return Ok(());
    }

    let db = OxideDb::open_or_create(&config.database.path).map_err(ServeError::Database)?;
    validate_startup_database_health(&db)
}

pub(crate) async fn run_until_shutdown<S>(
    config: &OxideConfig,
    config_path: &Path,
    shutdown_signal: S,
) -> ServeResult<()>
where
    S: Future<Output = ServeResult<()>> + Send,
{
    let binkp_listener_enabled = config
        .network
        .binkp_listener
        .as_ref()
        .is_some_and(|listener| config.network.enabled && listener.enabled);
    let serial_enabled = config.serial.enabled && !config.serial.devices.is_empty();

    if !config.telnet.enabled
        && !config.admin_web.enabled
        && !binkp_listener_enabled
        && !serial_enabled
    {
        info!(bind = %config.telnet.bind, "caller, admin web, and BinkP listeners disabled; service not started");
        return Ok(());
    }

    let db =
        Arc::new(OxideDb::open_or_create(&config.database.path).map_err(ServeError::Database)?);
    validate_startup_database_health(db.as_ref())?;
    insert_required_startup_audit_event(
        db.as_ref(),
        "config_loaded",
        None,
        None,
        format!(
            "config loaded from {} for board {}",
            config_path.display(),
            config.board.name
        ),
    )?;

    let runtime = Arc::new(ServerRuntime::new(
        config.board.name.clone(),
        config.nodes.count,
        config.telnet.max_connections,
        config.telnet.idle_timeout_seconds.saturating_add(30),
    ));
    let shared_config = Arc::new(config.clone());

    let mut resolved_menus: HashMap<String, Arc<Menu>> = HashMap::new();
    for menu_id in config.menus.keys() {
        let menu = config
            .core_menu(menu_id)
            .map_err(|error| ServeError::Config(error.to_string()))?;
        resolved_menus.insert(menu_id.clone(), Arc::new(menu));
    }

    let login_menu_id = config.flow.login_menu.clone();
    let main_menu_id = config.flow.main_menu.clone();
    let login_menu = resolved_menus
        .get(&login_menu_id)
        .cloned()
        .ok_or_else(|| ServeError::Config(format!("missing login menu {login_menu_id:?}")))?;
    let main_menu = resolved_menus
        .get(&main_menu_id)
        .cloned()
        .ok_or_else(|| ServeError::Config(format!("missing main menu {main_menu_id:?}")))?;
    let menus = Arc::new(resolved_menus);
    let caller_resources = caller_resources(
        Arc::clone(&db),
        Arc::clone(&shared_config),
        Arc::clone(&login_menu),
        Arc::clone(&main_menu),
        Arc::clone(&menus),
        Arc::clone(&runtime),
    );

    let control_listener =
        match start_control_listener(&config.paths.runtime, Arc::clone(&runtime)).await {
            Ok(handle) => Some(handle),
            Err(ControlError::Unsupported(message)) => {
                warn!("control listener unavailable: {message}");
                None
            }
            Err(error) => {
                return Err(ServeError::Runtime(format!(
                    "failed to start control listener: {error}"
                )));
            }
        };

    #[cfg(unix)]
    let (mut stale_node_shutdown, stale_node_sweeper) = {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        (
            Some(shutdown_tx),
            Some(start_stale_node_sweeper(
                Arc::clone(&runtime),
                STALE_NODE_SWEEP_INTERVAL,
                shutdown_rx,
            )),
        )
    };
    #[cfg(not(unix))]
    let (mut stale_node_shutdown, stale_node_sweeper): (
        Option<oneshot::Sender<()>>,
        Option<tokio::task::JoinHandle<()>>,
    ) = (None, None);

    let telnet_listener = if config.telnet.enabled {
        Some(TcpListener::bind(&config.telnet.bind).await?)
    } else {
        None
    };

    let admin_web_handle = if config.admin_web.enabled {
        Some(
            crate::admin_web::start_admin_web(
                Arc::clone(&shared_config),
                Arc::clone(&db),
                Arc::clone(&runtime),
                if config.web_terminal.enabled {
                    Some(caller_resources.clone())
                } else {
                    None
                },
            )
            .await?,
        )
    } else {
        None
    };

    let binkp_listener_handle = if config.network.enabled && config.network.binkp_listener.is_some()
    {
        match crate::binkp_listener::start_binkp_listener(
            Arc::clone(&shared_config),
            Arc::clone(&db),
        )
        .await
        {
            Ok(handle) => Some(handle),
            Err(error) => {
                warn!(%error, "BinkP listener failed to start");
                None
            }
        }
    } else {
        None
    };

    let serial_handles = if serial_enabled {
        start_serial_callers(
            Arc::clone(&shared_config),
            Arc::clone(&db),
            Arc::clone(&runtime),
            caller_resources.clone(),
        )?
    } else {
        Vec::new()
    };

    insert_required_startup_audit_event(
        db.as_ref(),
        "server_start",
        None,
        None,
        server_start_audit_details(config),
    )?;

    let mut shutdown = Box::pin(shutdown_signal);
    let mut accept_error = None;

    if let Some(listener) = telnet_listener {
        info!(bind = %config.telnet.bind, "listening for telnet callers");

        loop {
            tokio::select! {
                accept = listener.accept() => {
                    let (stream, peer_addr) = match accept {
                        Ok(accepted) => accepted,
                        Err(error) => {
                            accept_error = Some(ServeError::Network(error));
                            break;
                        }
                    };
                    let peer = CallerPeer {
                        address: peer_addr.to_string(),
                        ip: Some(peer_addr.ip().to_string()),
                        port: i64::from(peer_addr.port()),
                    };

                    if let Some(allocation) = runtime.try_allocate_node() {
                        info!(
                            node = %allocation.node_number,
                            remote = %peer.address,
                            remote_ip = %peer.ip.as_deref().unwrap_or("-"),
                            remote_port = peer.port,
                            "caller connection accepted"
                        );
                        emit_audit_event_with_runtime(
                            db.as_ref(),
                            "node_assigned",
                            None,
                            Some(i64::from(allocation.node_number)),
                            format!("node {} assigned to {}", allocation.node_number, peer.address),
                            Some(runtime.as_ref()),
                        );
                        let resources = caller_resources.clone();
                        tokio::spawn(async move {
                            if let Err(error) = handle_caller(allocation, stream, peer, resources).await
                            {
                                warn!("caller session ended with error: {error}");
                            }
                        });
                    } else {
                        warn!(
                            remote = %peer.address,
                            remote_ip = %peer.ip.as_deref().unwrap_or("-"),
                            remote_port = peer.port,
                            "caller rejected because no node is available"
                        );
                        tokio::spawn(async move {
                            if let Err(error) = reject_connection(stream).await {
                                warn!("failed to reject caller: {error}");
                            }
                        });
                    }
                }
                shutdown_result = &mut shutdown => {
                    if let Err(error) = shutdown_result {
                        accept_error = Some(error);
                    }
                    break;
                }
            }
        }
    } else {
        info!("telnet disabled; waiting for shutdown with enabled background listeners");
        if let Err(error) = (&mut shutdown).await {
            accept_error = Some(error);
        }
    }

    let active_nodes = runtime
        .nodes_snapshot()
        .into_iter()
        .filter_map(|status| {
            if status.connected_at.is_some() {
                Some(status.node_number)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    for node_number in &active_nodes {
        runtime.request_node_disconnect(*node_number, "server stopping".to_string());
    }
    if !active_nodes.is_empty() {
        for _ in 0..20 {
            if runtime
                .nodes_snapshot()
                .iter()
                .all(|status| status.connected_at.is_none())
            {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }

    if let Some(shutdown_tx) = stale_node_shutdown.take() {
        let _ = shutdown_tx.send(());
    }
    if let Some(handle) = stale_node_sweeper
        && let Err(error) = handle.await
    {
        warn!("stale node sweeper shutdown failed: {error}");
    }

    if let Some(handle) = control_listener {
        handle.abort();
    }
    if let Some(handle) = admin_web_handle {
        handle.abort();
    }
    if let Some(handle) = binkp_listener_handle {
        handle.abort();
    }
    for handle in serial_handles {
        handle.abort();
    }

    emit_audit_event_with_runtime(
        db.as_ref(),
        "server_stop",
        None,
        None,
        format!(
            "shutdown complete with {} active node(s)",
            active_nodes.len()
        ),
        Some(runtime.as_ref()),
    );

    if let Some(error) = accept_error {
        return Err(error);
    }

    Ok(())
}

async fn reject_connection(mut stream: TcpStream) -> ServeResult<()> {
    let bytes = encode_text(REJECTION_MESSAGE, TerminalCharset::Cp437);
    stream.write_all(&bytes).await?;
    stream.shutdown().await?;
    Ok(())
}

fn start_serial_callers(
    config: Arc<OxideConfig>,
    _db: Arc<OxideDb>,
    runtime: Arc<ServerRuntime>,
    resources: CallerResources,
) -> ServeResult<Vec<tokio::task::JoinHandle<()>>> {
    let mut handles = Vec::new();
    for device in &config.serial.devices {
        let serial_config = serial_port_config_from_device(device)?;
        let transport = SerialTransport::open(serial_config)
            .map_err(|error| ServeError::Runtime(error.to_string()))?;
        let Some(allocation) = runtime.try_allocate_node() else {
            warn!(
                device = %device.name,
                path = %device.path,
                "serial caller device could not start because no node is available"
            );
            continue;
        };
        let peer = CallerPeer {
            address: format!("serial:{}", device.name),
            ip: None,
            port: 0,
        };
        info!(
            node = %allocation.node_number,
            device = %device.name,
            path = %device.path,
            "serial caller device opened"
        );
        let resources = resources.clone();
        handles.push(tokio::spawn(async move {
            if let Err(error) =
                handle_raw_caller_transport(allocation, transport, "serial", peer, resources).await
            {
                warn!("serial caller session ended with error: {error}");
            }
        }));
    }
    Ok(handles)
}

fn serial_port_config_from_device(
    device: &crate::config::SerialDeviceConfig,
) -> ServeResult<SerialPortConfig> {
    Ok(SerialPortConfig {
        path: device.path.clone(),
        baud_rate: device.baud_rate,
        data_bits: device.data_bits,
        parity: serial_parity_from_config(&device.parity)?,
        stop_bits: device.stop_bits,
        flow_control: serial_flow_control_from_config(&device.flow_control)?,
        init_strings: device.init_strings.clone(),
        answer_string: device.answer_string.clone(),
        require_carrier_detect: device.require_carrier_detect,
        drop_dtr_on_hangup: device.drop_dtr_on_hangup,
        read_timeout_ms: device.read_timeout_ms,
    })
}

fn serial_flow_control_from_config(value: &str) -> ServeResult<SerialFlowControl> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(SerialFlowControl::None),
        "rtscts" | "rts_cts" | "hardware" => Ok(SerialFlowControl::RtsCts),
        "xonxoff" | "xon_xoff" | "software" => Ok(SerialFlowControl::XonXoff),
        other => Err(ServeError::Config(format!(
            "serial flow_control must be one of none, rtscts, or xonxoff, got {other:?}"
        ))),
    }
}

fn serial_parity_from_config(value: &str) -> ServeResult<SerialParity> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(SerialParity::None),
        "odd" => Ok(SerialParity::Odd),
        "even" => Ok(SerialParity::Even),
        other => Err(ServeError::Config(format!(
            "serial parity must be one of none, odd, or even, got {other:?}"
        ))),
    }
}

#[cfg(unix)]
fn start_stale_node_sweeper(
    runtime: Arc<ServerRuntime>,
    interval: Duration,
    mut shutdown: oneshot::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = sleep(interval) => {
                    let _ = runtime.request_stale_disconnects("stale_node_timeout");
                }
                _ = &mut shutdown => {
                    break;
                }
            }
        }
    })
}

#[derive(Clone)]
pub(crate) struct CallerResources {
    db: Arc<OxideDb>,
    config: Arc<OxideConfig>,
    login_menu: Arc<Menu>,
    main_menu: Arc<Menu>,
    menus: Arc<HashMap<String, Arc<Menu>>>,
    runtime: Arc<ServerRuntime>,
}

#[derive(Clone)]
pub(crate) struct CallerPeer {
    pub(crate) address: String,
    pub(crate) ip: Option<String>,
    pub(crate) port: i64,
}

pub(crate) fn caller_resources(
    db: Arc<OxideDb>,
    config: Arc<OxideConfig>,
    login_menu: Arc<Menu>,
    main_menu: Arc<Menu>,
    menus: Arc<HashMap<String, Arc<Menu>>>,
    runtime: Arc<ServerRuntime>,
) -> CallerResources {
    CallerResources {
        db,
        config,
        login_menu,
        main_menu,
        menus,
        runtime,
    }
}

async fn handle_caller(
    allocation: NodeAllocation,
    stream: TcpStream,
    peer: CallerPeer,
    resources: CallerResources,
) -> ServeResult<()> {
    let transport = TcpTransport::new(stream);
    handle_caller_transport(allocation, transport, "telnet", true, None, peer, resources).await
}

pub(crate) async fn handle_raw_caller_transport<T: Transport>(
    allocation: NodeAllocation,
    transport: T,
    transport_name: &'static str,
    peer: CallerPeer,
    resources: CallerResources,
) -> ServeResult<()> {
    handle_raw_caller_transport_with_capabilities(
        allocation,
        transport,
        transport_name,
        None,
        peer,
        resources,
    )
    .await
}

pub(crate) async fn handle_raw_caller_transport_with_capabilities<T: Transport>(
    allocation: NodeAllocation,
    transport: T,
    transport_name: &'static str,
    raw_capabilities: Option<TerminalCapabilities>,
    peer: CallerPeer,
    resources: CallerResources,
) -> ServeResult<()> {
    handle_caller_transport(
        allocation,
        transport,
        transport_name,
        false,
        raw_capabilities,
        peer,
        resources,
    )
    .await
}

async fn handle_caller_transport<T: Transport>(
    allocation: NodeAllocation,
    mut transport: T,
    transport_name: &'static str,
    telnet_protocol: bool,
    raw_capabilities: Option<TerminalCapabilities>,
    peer: CallerPeer,
    resources: CallerResources,
) -> ServeResult<()> {
    let CallerResources {
        db,
        config,
        login_menu,
        main_menu,
        menus,
        runtime,
    } = resources;
    let node_number_u16 = allocation.node_number;
    let node_number = i64::from(node_number_u16);
    let session_id = generated_uuid(&db)?;
    let connected_at = current_timestamp(&db)?;
    let remote_ip_for_log = peer.ip.as_deref().unwrap_or("-");
    let auth_remote_scope = peer.ip.as_deref().unwrap_or(&peer.address);
    let mut input = if telnet_protocol {
        InputSession::default()
    } else {
        InputSession::raw()
    };
    let idle_timeout = Duration::from_secs(config.telnet.idle_timeout_seconds);
    let mut authenticated_user: Option<User> = None;

    insert_session(
        db.db(),
        &SessionRecord {
            id: session_id.clone(),
            node_number,
            user_id: None,
            transport: transport_name.to_string(),
            remote_address: peer.address.clone(),
            remote_ip: peer.ip.clone(),
            remote_port: if telnet_protocol {
                Some(peer.port)
            } else {
                None
            },
            started_at: connected_at.clone(),
            ended_at: None,
            disconnect_reason: None,
        },
    )
    .map_err(|error| {
        emit_db_write_failed_event(
            &db,
            Some(node_number),
            None,
            "insert_session",
            &error,
            "failed to open caller session",
            Some(&runtime),
        );
        error!("failed to insert session record: {error}");
        ServeError::Database(error)
    })?;
    runtime.mark_node_connected(
        node_number_u16,
        session_id.clone(),
        peer.address.clone(),
        connected_at.clone(),
    );
    debug!(
        node = %node_number,
        session_id = %session_id,
        remote = %peer.address,
        remote_ip = %remote_ip_for_log,
        remote_port = peer.port,
        "caller session opened"
    );
    let mut current_menu = Arc::clone(&login_menu);

    emit_audit_event_with_runtime(
        db.as_ref(),
        "caller_connected",
        None,
        Some(node_number),
        format!("caller connected from {}", peer.address),
        Some(runtime.as_ref()),
    );

    let fallback_capabilities = config
        .terminal
        .default_capabilities()
        .map_err(|error| ServeError::Config(error.to_string()))?;
    let mut capabilities = if telnet_protocol {
        negotiate_terminal_capabilities(
            &mut transport,
            &mut input,
            TERMINAL_CAPABILITY_NEGOTIATION_TIMEOUT,
            fallback_capabilities,
        )
        .await?
    } else {
        raw_capabilities.unwrap_or(fallback_capabilities)
    };
    debug!(
        node = %node_number,
        session_id = %session_id,
        remote = %peer.address,
        remote_ip = %remote_ip_for_log,
        remote_port = peer.port,
        supports_ansi = capabilities.supports_ansi,
        terminal_width = capabilities.width,
        "terminal capabilities negotiated"
    );

    if config.terminal.clear_screen_on_connect && capabilities.supports_ansi {
        transport
            .write_all(oxidebbs_term::CLEAR_SCREEN_AND_HOME)
            .await
            .map_err(ServeError::Transport)?;
    }

    runtime.mark_node_login(node_number_u16);
    let mut screen_context = ScreenRenderContext {
        node_number: node_number_u16,
        node_count: config.nodes.count,
        board_name: config.board.name.clone(),
        sysop_name: config.board.sysop_name.clone(),
        caller_alias: None,
        security_level: None,
    };
    send_terminal_asset(
        &mut transport,
        &config.terminal.welcome_screen,
        &config,
        capabilities,
        &screen_context,
    )
    .await?;
    send_login_flow(
        &mut transport,
        &config,
        &login_menu,
        &mut capabilities,
        &screen_context,
    )
    .await?;

    let mut in_main_menu = false;
    let mut disconnect_reason = "caller_disconnected".to_string();

    loop {
        if process_runtime_commands(
            &mut transport,
            runtime.take_node_commands(node_number_u16),
            &mut disconnect_reason,
            capabilities.charset,
        )
        .await?
        {
            break;
        }

        let wait = tokio::select! {
            commands = runtime.wait_for_node_commands(node_number_u16) => {
                CallerWait::Runtime(commands)
            }
            event = next_event(&mut transport, &mut input, idle_timeout) => {
                CallerWait::Input(event)
            }
        };

        let event = match wait {
            CallerWait::Runtime(commands) => {
                if process_runtime_commands(
                    &mut transport,
                    commands,
                    &mut disconnect_reason,
                    capabilities.charset,
                )
                .await?
                {
                    break;
                }
                continue;
            }
            CallerWait::Input(event) => event,
        };
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
                send_text(
                    &mut transport,
                    "Idle timeout. Goodbye.\r\n",
                    capabilities.charset,
                )
                .await?;
                break;
            }
            Err(error) => {
                warn!("caller transport failed: {error}");
                return Err(error);
            }
        };
        runtime.heartbeat_node(node_number_u16);

        match event {
            TelnetEvent::Data(raw_key) => {
                let key = match normalize_key(raw_key) {
                    Some(key) => key,
                    None => continue,
                };
                drain_line_ending_after_menu_key(&mut transport, &mut input).await?;
                debug!(
                    node = %node_number,
                    menu = %current_menu.id,
                    key = %key,
                    authenticated = in_main_menu,
                    "caller selected menu key"
                );

                let user_security = if in_main_menu {
                    authenticated_user.as_ref().map(|user| user.security_level)
                } else {
                    None
                };
                if key == "?" {
                    send_menu_help(
                        &mut transport,
                        &config,
                        &current_menu,
                        &mut capabilities,
                        user_security,
                        &screen_context,
                    )
                    .await?;
                    send_menu_prompt(
                        &mut transport,
                        &current_menu,
                        &screen_context,
                        capabilities.charset,
                    )
                    .await?;
                    continue;
                }
                if key == "R" && current_menu.route_entry("R").is_none() {
                    send_screen(
                        &mut transport,
                        &config,
                        &current_menu.screen.asset,
                        &mut capabilities,
                        &screen_context,
                    )
                    .await?;
                    send_menu_prompt(
                        &mut transport,
                        &current_menu,
                        &screen_context,
                        capabilities.charset,
                    )
                    .await?;
                    continue;
                }

                if !in_main_menu {
                    let route = current_menu.route(&key);
                    if route.is_some()
                        && let Some(entry) = current_menu.route_entry(&key)
                        && entry.min_security_level > 0
                    {
                        send_text(&mut transport, ACCESS_DENIED_MESSAGE, capabilities.charset)
                            .await?;
                        send_menu_prompt(
                            &mut transport,
                            &current_menu,
                            &screen_context,
                            capabilities.charset,
                        )
                        .await?;
                        continue;
                    }
                    match route {
                        Some(MenuAction::Login) => {
                            debug!(node = %node_number, "caller selected login flow");
                            let mut auth_state = AuthFlowState {
                                db: db.as_ref(),
                                config: &config,
                                runtime: runtime.as_ref(),
                                node_number,
                                remote_ip: auth_remote_scope,
                                session_id: &session_id,
                                authenticated_user: &mut authenticated_user,
                                idle_timeout,
                                disconnect_reason: &mut disconnect_reason,
                            };
                            match run_login_flow(
                                &mut transport,
                                &mut input,
                                &mut auth_state,
                                capabilities.charset,
                            )
                            .await?
                            {
                                AuthFlowResult::Success => {
                                    if let Some(user) = authenticated_user.as_ref() {
                                        runtime.set_node_user(
                                            node_number_u16,
                                            Some(user.id.clone()),
                                            Some(user.alias.clone()),
                                        );
                                        screen_context.caller_alias = Some(user.alias.clone());
                                        screen_context.security_level = Some(user.security_level);
                                    }
                                    current_menu = Arc::clone(&main_menu);
                                    show_post_login_screens(
                                        &mut transport,
                                        &config,
                                        &mut capabilities,
                                        &screen_context,
                                    )
                                    .await?;
                                    send_main_menu(
                                        &mut transport,
                                        &config,
                                        &main_menu,
                                        &mut capabilities,
                                        &screen_context,
                                    )
                                    .await?;
                                    runtime.mark_node_main_menu(node_number_u16);
                                    in_main_menu = true;
                                }
                                AuthFlowResult::Retry => {
                                    send_menu_prompt(
                                        &mut transport,
                                        &current_menu,
                                        &screen_context,
                                        capabilities.charset,
                                    )
                                    .await?;
                                }
                                AuthFlowResult::Exit => break,
                            }
                        }
                        Some(MenuAction::NewUser) => {
                            debug!(node = %node_number, "caller selected new-user flow");
                            let mut auth_state = AuthFlowState {
                                db: db.as_ref(),
                                config: &config,
                                runtime: runtime.as_ref(),
                                node_number,
                                remote_ip: auth_remote_scope,
                                session_id: &session_id,
                                authenticated_user: &mut authenticated_user,
                                idle_timeout,
                                disconnect_reason: &mut disconnect_reason,
                            };
                            match run_new_user_flow(
                                &mut transport,
                                &mut input,
                                &mut auth_state,
                                capabilities.charset,
                            )
                            .await?
                            {
                                AuthFlowResult::Success => {
                                    if let Some(user) = authenticated_user.as_ref() {
                                        runtime.set_node_user(
                                            node_number_u16,
                                            Some(user.id.clone()),
                                            Some(user.alias.clone()),
                                        );
                                        screen_context.caller_alias = Some(user.alias.clone());
                                        screen_context.security_level = Some(user.security_level);
                                    }
                                    current_menu = Arc::clone(&main_menu);
                                    show_post_login_screens(
                                        &mut transport,
                                        &config,
                                        &mut capabilities,
                                        &screen_context,
                                    )
                                    .await?;
                                    send_main_menu(
                                        &mut transport,
                                        &config,
                                        &main_menu,
                                        &mut capabilities,
                                        &screen_context,
                                    )
                                    .await?;
                                    runtime.mark_node_main_menu(node_number_u16);
                                    in_main_menu = true;
                                }
                                AuthFlowResult::Retry => {
                                    send_menu_prompt(
                                        &mut transport,
                                        &current_menu,
                                        &screen_context,
                                        capabilities.charset,
                                    )
                                    .await?;
                                }
                                AuthFlowResult::Exit => break,
                            }
                        }
                        Some(MenuAction::Logoff) => {
                            debug!(node = %node_number, "caller selected login-menu logoff");
                            disconnect_reason = "caller_logoff".to_string();
                            send_logoff_screen(
                                &mut transport,
                                &config,
                                capabilities,
                                &screen_context,
                            )
                            .await;
                            break;
                        }
                        Some(MenuAction::Submenu { menu_id }) => {
                            debug!(node = %node_number, submenu = %menu_id, "caller selected submenu");
                            if let Some(submenu) = resolve_submenu(&menus, &menu_id) {
                                current_menu = Arc::clone(&submenu);
                                send_menu_prompt(
                                    &mut transport,
                                    &current_menu,
                                    &screen_context,
                                    capabilities.charset,
                                )
                                .await?;
                            } else {
                                send_text(
                                    &mut transport,
                                    "Configured submenu menu is missing.\r\n",
                                    capabilities.charset,
                                )
                                .await?;
                                send_menu_prompt(
                                    &mut transport,
                                    &current_menu,
                                    &screen_context,
                                    capabilities.charset,
                                )
                                .await?;
                            }
                        }
                        _ => {
                            send_text(
                                &mut transport,
                                "Select Login, New User, or Goodbye.\r\n",
                                capabilities.charset,
                            )
                            .await?;
                            send_menu_prompt(
                                &mut transport,
                                &current_menu,
                                &screen_context,
                                capabilities.charset,
                            )
                            .await?;
                        }
                    }
                } else {
                    let entry = current_menu.route_entry(&key);
                    let user_security = authenticated_user.as_ref().map(|user| user.security_level);
                    let denied = match (entry, user_security) {
                        (Some(entry), Some(level)) => level < entry.min_security_level,
                        (Some(entry), None) => entry.min_security_level > 0,
                        (None, _) => false,
                    };
                    if denied {
                        debug!(
                            node = %node_number,
                            "caller denied by menu item min_security_level"
                        );
                        send_text(&mut transport, ACCESS_DENIED_MESSAGE, capabilities.charset)
                            .await?;
                        send_menu_prompt(
                            &mut transport,
                            &current_menu,
                            &screen_context,
                            capabilities.charset,
                        )
                        .await?;
                        continue;
                    }
                    match current_menu.route(&key) {
                        Some(MenuAction::Doors) => {
                            debug!(node = %node_number, "caller selected doors");
                            let mut door_state = DoorFlowState {
                                db: db.as_ref(),
                                config: config.as_ref(),
                                idle_timeout,
                                disconnect_reason: &mut disconnect_reason,
                                runtime: runtime.as_ref(),
                                node_number: node_number_u16,
                            };
                            match run_doors_flow(
                                authenticated_user.as_ref(),
                                &mut transport,
                                &mut input,
                                &mut door_state,
                                capabilities.charset,
                            )
                            .await?
                            {
                                MenuFlowResult::Continue => {
                                    runtime.mark_node_main_menu(node_number_u16);
                                    send_menu_prompt(
                                        &mut transport,
                                        &current_menu,
                                        &screen_context,
                                        capabilities.charset,
                                    )
                                    .await?;
                                }
                                MenuFlowResult::Exit => break,
                            }
                        }
                        Some(MenuAction::Messages) => {
                            debug!(node = %node_number, "caller selected messages");
                            runtime.mark_node_reading_messages(node_number_u16);
                            let mut message_state = MessageFlowState {
                                db: db.as_ref(),
                                idle_timeout,
                                disconnect_reason: &mut disconnect_reason,
                                runtime: runtime.as_ref(),
                                node_number: node_number_u16,
                            };
                            match run_messages_flow(
                                authenticated_user.as_ref(),
                                &mut transport,
                                &mut input,
                                &mut message_state,
                                capabilities.charset,
                            )
                            .await?
                            {
                                MenuFlowResult::Continue => {
                                    runtime.mark_node_main_menu(node_number_u16);
                                    send_menu_prompt(
                                        &mut transport,
                                        &current_menu,
                                        &screen_context,
                                        capabilities.charset,
                                    )
                                    .await?;
                                }
                                MenuFlowResult::Exit => {
                                    break;
                                }
                            }
                        }
                        Some(MenuAction::Files) => {
                            debug!(node = %node_number, "caller selected files");
                            match run_files_flow(
                                authenticated_user.as_ref(),
                                &mut transport,
                                &mut input,
                                db.as_ref(),
                                config.as_ref(),
                                telnet_protocol,
                                idle_timeout,
                                &mut disconnect_reason,
                                node_number_u16,
                                capabilities.charset,
                            )
                            .await?
                            {
                                MenuFlowResult::Continue => {
                                    runtime.mark_node_main_menu(node_number_u16);
                                    send_menu_prompt(
                                        &mut transport,
                                        &current_menu,
                                        &screen_context,
                                        capabilities.charset,
                                    )
                                    .await?;
                                }
                                MenuFlowResult::Exit => break,
                            }
                        }
                        Some(MenuAction::NewUser) => {
                            debug!(node = %node_number, "authenticated caller selected new-user action");
                            send_text(
                                &mut transport,
                                "Already signed in. Return to menu.\r\n",
                                capabilities.charset,
                            )
                            .await?;
                            send_menu_prompt(
                                &mut transport,
                                &current_menu,
                                &screen_context,
                                capabilities.charset,
                            )
                            .await?;
                        }
                        Some(MenuAction::Logoff) => {
                            debug!(node = %node_number, "caller selected main-menu logoff");
                            disconnect_reason = "caller_logoff".to_string();
                            send_logoff_screen(
                                &mut transport,
                                &config,
                                capabilities,
                                &screen_context,
                            )
                            .await;
                            break;
                        }
                        Some(MenuAction::ShowScreen { screen }) => {
                            debug!(node = %node_number, screen = %screen.asset, "caller selected show-screen action");
                            send_screen(
                                &mut transport,
                                &config,
                                &screen.asset,
                                &mut capabilities,
                                &screen_context,
                            )
                            .await?;
                            send_menu_prompt(
                                &mut transport,
                                &current_menu,
                                &screen_context,
                                capabilities.charset,
                            )
                            .await?;
                        }
                        Some(MenuAction::Submenu { menu_id }) => {
                            debug!(node = %node_number, submenu = %menu_id, "caller selected submenu");
                            if let Some(submenu) = resolve_submenu(&menus, &menu_id) {
                                current_menu = Arc::clone(&submenu);
                                send_menu_prompt(
                                    &mut transport,
                                    &current_menu,
                                    &screen_context,
                                    capabilities.charset,
                                )
                                .await?;
                            } else {
                                send_text(
                                    &mut transport,
                                    "Configured submenu menu is missing.\r\n",
                                    capabilities.charset,
                                )
                                .await?;
                                send_menu_prompt(
                                    &mut transport,
                                    &current_menu,
                                    &screen_context,
                                    capabilities.charset,
                                )
                                .await?;
                            }
                        }
                        Some(MenuAction::Login) => {
                            debug!(node = %node_number, "authenticated caller selected login action");
                            send_text(
                                &mut transport,
                                "Already signed in. Return to menu.\r\n",
                                capabilities.charset,
                            )
                            .await?;
                            send_menu_prompt(
                                &mut transport,
                                &current_menu,
                                &screen_context,
                                capabilities.charset,
                            )
                            .await?;
                        }
                        Some(MenuAction::Noop) => {
                            debug!(node = %node_number, "caller selected noop action");
                        }
                        None => {
                            debug!(
                                node = %node_number,
                                menu = %current_menu.id,
                                key = %key,
                                "caller selected unknown menu key"
                            );
                            send_text(&mut transport, "Unknown option.\r\n", capabilities.charset)
                                .await?;
                            send_menu_prompt(
                                &mut transport,
                                &current_menu,
                                &screen_context,
                                capabilities.charset,
                            )
                            .await?;
                        }
                    }
                }
            }
            TelnetEvent::WindowSize { columns, rows } => {
                if columns > 0 {
                    capabilities.width = columns;
                    if capabilities.supports_ansi && columns <= 40 {
                        capabilities.profile = TerminalProfile::Ansi40;
                    } else if capabilities.supports_ansi {
                        capabilities.profile = TerminalProfile::Ansi80;
                    }
                }
                if rows > 0 {
                    capabilities.height = rows;
                }
            }
            TelnetEvent::Negotiation { .. }
            | TelnetEvent::TerminalType(_)
            | TelnetEvent::TerminalTypeRequest
            | TelnetEvent::Subnegotiation { .. } => {}
        }
    }

    runtime.mark_node_disconnecting(node_number_u16);
    if let Err(error) = flush_pending_replies(&mut transport, &mut input).await {
        warn!("failed to flush pending negotiation replies: {error}");
    }
    if let Err(error) = transport.hangup().await {
        warn!("failed to hang up caller transport: {error}");
    }

    let ended_at = current_timestamp(&db)?;
    if let Err(error) = end_session(db.db(), &session_id, &ended_at, &disconnect_reason) {
        warn!("failed to close session record: {error}");
        emit_db_write_failed_event(
            db.as_ref(),
            Some(node_number),
            authenticated_user.as_ref().map(|user| user.id.clone()),
            "end_session",
            &error,
            "failed to close session",
            Some(&runtime),
        );
    }
    runtime.mark_node_disconnected(node_number_u16);

    emit_audit_event_with_runtime(
        db.as_ref(),
        "caller_disconnected",
        authenticated_user.as_ref().map(|user| user.id.clone()),
        Some(node_number),
        format!("disconnect reason: {disconnect_reason}"),
        Some(runtime.as_ref()),
    );

    info!(
        node = %node_number,
        remote = %peer.address,
        reason = %disconnect_reason,
        "session ended"
    );

    Ok(())
}

#[derive(Debug)]
enum AuthFlowResult {
    Success,
    Retry,
    Exit,
}

#[derive(Debug)]
enum MenuFlowResult {
    Continue,
    Exit,
}

#[derive(Debug)]
enum MessageIndexPromptResult {
    Index(usize),
    Retry,
    Exit,
}

#[derive(Debug)]
enum PromptLineResult {
    Value(String),
    Rejected,
    Disconnected,
    IdleTimeout,
}

async fn negotiate_terminal_capabilities<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    negotiation_timeout: Duration,
    fallback_capabilities: TerminalCapabilities,
) -> ServeResult<TerminalCapabilities> {
    let mut capabilities = fallback_capabilities;
    let mut terminal_type_evaluated = false;
    let mut naws_seen = false;
    transport.write_all(&terminal_capability_requests()).await?;

    let Ok(result) = timeout(negotiation_timeout, async {
        loop {
            match next_event(transport, input, negotiation_timeout).await? {
                CallerInput::Event(TelnetEvent::Data(byte)) => {
                    input
                        .pending_inputs
                        .push_front(CallerInput::Event(TelnetEvent::Data(byte)));
                    return Ok(()) as ServeResult<()>;
                }
                CallerInput::Event(event) => {
                    if apply_capability_event(
                        transport,
                        &mut capabilities,
                        &mut terminal_type_evaluated,
                        &mut naws_seen,
                        event,
                    )
                    .await?
                    {
                        return Ok(()) as ServeResult<()>;
                    }
                }
                CallerInput::IdleTimeout => return Ok(()),
                other @ CallerInput::Disconnected => {
                    input.pending_inputs.push_front(other);
                    return Ok(());
                }
            }
        }
    })
    .await
    else {
        return Ok(capabilities);
    };

    result?;
    Ok(capabilities)
}

fn terminal_capability_requests() -> [u8; 15] {
    [
        IAC,
        WILL,
        TELOPT_ECHO,
        IAC,
        WILL,
        TELOPT_SUPPRESS_GO_AHEAD,
        IAC,
        DO,
        TELOPT_SUPPRESS_GO_AHEAD,
        IAC,
        DO,
        TELOPT_TERMINAL_TYPE,
        IAC,
        DO,
        TELOPT_NAWS,
    ]
}

async fn apply_capability_event<T: Transport>(
    transport: &mut T,
    capabilities: &mut TerminalCapabilities,
    terminal_type_evaluated: &mut bool,
    naws_seen: &mut bool,
    event: TelnetEvent,
) -> ServeResult<bool> {
    match event {
        TelnetEvent::Negotiation {
            command: TelnetCommand::Will,
            option: TELOPT_TERMINAL_TYPE,
            accepted: true,
        } => {
            transport
                .write_all(&[IAC, SB, TELOPT_TERMINAL_TYPE, TELOPT_TTYPE_SEND, IAC, SE])
                .await?;
        }
        TelnetEvent::TerminalType(terminal_type) => {
            apply_terminal_type(capabilities, &terminal_type, *naws_seen);
            *terminal_type_evaluated = true;
        }
        TelnetEvent::WindowSize { columns, rows } if columns > 0 => {
            capabilities.width = columns;
            if rows > 0 {
                capabilities.height = rows;
            }
            if capabilities.supports_ansi && columns <= 40 {
                capabilities.profile = TerminalProfile::Ansi40;
            } else if capabilities.supports_ansi {
                capabilities.profile = TerminalProfile::Ansi80;
            }
            *naws_seen = true;
        }
        TelnetEvent::Data(_) => return Ok(true),
        TelnetEvent::Negotiation { .. }
        | TelnetEvent::TerminalTypeRequest
        | TelnetEvent::WindowSize { .. }
        | TelnetEvent::Subnegotiation { .. } => {}
    }

    Ok(*terminal_type_evaluated && *naws_seen)
}

fn terminal_type_capabilities(terminal_type: &[u8]) -> TerminalCapabilities {
    let terminal_type = String::from_utf8_lossy(terminal_type);
    let normalized = terminal_type.trim().to_ascii_lowercase();

    if normalized.contains("c64")
        || normalized.contains("commodore 64")
        || normalized.contains("c64 ultimate")
        || normalized.contains("ultimate 64")
        || normalized.contains("petscii")
        || normalized.contains("cgterm")
    {
        return TerminalCapabilities::c64();
    }

    if normalized.contains("syncterm")
        || normalized == "ansi"
        || normalized.contains("ansi.sys")
        || normalized.contains("ansi-bbs")
        || normalized.contains("bbs-ansi")
        || normalized == "pc-ansi"
        || normalized.contains("pcansi")
    {
        TerminalCapabilities::ansi_80()
    } else {
        TerminalCapabilities::plain_text()
    }
}

fn apply_terminal_type(
    capabilities: &mut TerminalCapabilities,
    terminal_type: &[u8],
    naws_seen: bool,
) {
    let detected = terminal_type_capabilities(terminal_type);
    let reported_width = capabilities.width;
    let reported_height = capabilities.height;
    *capabilities = detected;
    if naws_seen {
        capabilities.width = reported_width;
        capabilities.height = reported_height;
        if capabilities.profile == TerminalProfile::Ansi80 && reported_width <= 40 {
            capabilities.profile = TerminalProfile::Ansi40;
        }
    }
}

struct AuthFlowState<'a> {
    db: &'a OxideDb,
    config: &'a OxideConfig,
    runtime: &'a ServerRuntime,
    node_number: i64,
    remote_ip: &'a str,
    session_id: &'a str,
    authenticated_user: &'a mut Option<User>,
    idle_timeout: Duration,
    disconnect_reason: &'a mut String,
}

struct MessageFlowState<'a> {
    db: &'a OxideDb,
    idle_timeout: Duration,
    disconnect_reason: &'a mut String,
    runtime: &'a ServerRuntime,
    node_number: u16,
}

struct DoorFlowState<'a> {
    db: &'a OxideDb,
    config: &'a OxideConfig,
    idle_timeout: Duration,
    disconnect_reason: &'a mut String,
    runtime: &'a ServerRuntime,
    node_number: u16,
}

async fn run_login_flow<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    state: &mut AuthFlowState<'_>,
    charset: TerminalCharset,
) -> ServeResult<AuthFlowResult> {
    let db = state.db;
    let node_number = state.node_number;
    let session_id = state.session_id;
    let idle_timeout = state.idle_timeout;
    let disconnect_reason = &mut *state.disconnect_reason;
    let authenticated_user = &mut *state.authenticated_user;

    send_text(transport, "\r\n-- Login --\r\n", charset).await?;

    let alias = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        false,
        false,
        "Alias: ",
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => value,
        PromptLineResult::Disconnected => {
            *disconnect_reason = "caller_dropped_during_login".to_string();
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::IdleTimeout => {
            *disconnect_reason = "idle_timeout".to_string();
            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::Rejected => {
            unreachable!("prompt_for_line handles rejected input internally");
        }
    };

    let password = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        false,
        true,
        "Password: ",
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => value,
        PromptLineResult::Disconnected => {
            *disconnect_reason = "caller_dropped_during_login".to_string();
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::IdleTimeout => {
            *disconnect_reason = "idle_timeout".to_string();
            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::Rejected => {
            unreachable!("prompt_for_line handles rejected input internally");
        }
    };

    let login_at = current_timestamp(db)?;
    let alias_scope_key = normalize_alias(&alias);
    if is_auth_scope_locked(db.db(), "ip", state.remote_ip, &login_at)?
        || is_auth_scope_locked(db.db(), "alias", &alias_scope_key, &login_at)?
    {
        debug!(
            node = %node_number,
            remote_ip = %state.remote_ip,
            alias_scope = %alias_scope_key,
            "login rejected by rate limiter"
        );
        send_text(transport, LOGIN_LOCKOUT_MESSAGE, charset).await?;
        return Ok(AuthFlowResult::Retry);
    }

    let user_record = find_user_by_alias_ci(db.db(), &alias)?;
    let Some(user_record) = user_record else {
        run_dummy_password_verify(&password, &state.config.auth.argon2)?;
        record_login_failure_scopes(
            db,
            state.remote_ip,
            &alias_scope_key,
            &login_at,
            &state.config.auth,
        )?;
        emit_audit_event_with_runtime(
            db,
            "login_failure",
            None,
            Some(node_number),
            format!("login failed for alias {alias_scope_key}"),
            Some(state.runtime),
        );
        debug!(
            node = %node_number,
            remote_ip = %state.remote_ip,
            alias_scope = %alias_scope_key,
            "login rejected for unknown alias"
        );
        send_text(transport, INVALID_LOGIN_MESSAGE, charset).await?;
        return Ok(AuthFlowResult::Retry);
    };

    let mut user = user_from_record(&user_record)?;
    let verification =
        verify_stored_password(&password, &user.password_hash, &state.config.auth.argon2)?;
    if verification == PasswordVerification::HashParseFailure {
        emit_audit_event_with_runtime(
            db,
            "password_hash_parse_failure",
            Some(user.id.clone()),
            Some(node_number),
            format!(
                "stored password hash could not be parsed for {}",
                user.alias
            ),
            Some(state.runtime),
        );
    }
    let rejected =
        user.status != UserStatus::Active || verification != PasswordVerification::Accepted;
    if rejected {
        record_login_failure_scopes(
            db,
            state.remote_ip,
            &alias_scope_key,
            &login_at,
            &state.config.auth,
        )?;
        emit_audit_event_with_runtime(
            db,
            "login_failure",
            Some(user.id.clone()),
            Some(node_number),
            format!("login failed for user {}", user.alias),
            Some(state.runtime),
        );
        debug!(
            node = %node_number,
            remote_ip = %state.remote_ip,
            user_id = %user.id,
            alias = %user.alias,
            status = ?user.status,
            verification = ?verification,
            "login rejected for user"
        );
        send_text(transport, INVALID_LOGIN_MESSAGE, charset).await?;
        return Ok(AuthFlowResult::Retry);
    }

    user.last_login_at = Some(login_at.clone());
    user.total_calls += 1;

    if let Err(error) = update_user_login(db.db(), &user.id, &login_at) {
        emit_db_write_failed_event(
            db,
            Some(node_number),
            Some(user.id.clone()),
            "update_user_login",
            &error,
            "failed to update user login counters",
            Some(state.runtime),
        );
        warn!(
            "failed to update user login counters for {}: {error}",
            user.alias
        );
    }

    clear_auth_attempt(db.db(), "ip", state.remote_ip)?;
    clear_auth_attempt(db.db(), "alias", &alias_scope_key)?;

    if let Err(error) = update_session_user(db.db(), session_id, &user.id) {
        emit_db_write_failed_event(
            db,
            Some(node_number),
            Some(user.id.clone()),
            "update_session_user",
            &error,
            "failed to associate user with session",
            Some(state.runtime),
        );
        warn!(
            "failed to associate user {} with session {}: {error}",
            user.alias, session_id
        );
    }

    emit_audit_event_with_runtime(
        db,
        "login_success",
        Some(user.id.clone()),
        Some(node_number),
        format!("login successful for {}", user.alias),
        Some(state.runtime),
    );
    debug!(
        node = %node_number,
        remote_ip = %state.remote_ip,
        user_id = %user.id,
        alias = %user.alias,
        security_level = user.security_level,
        "caller login accepted"
    );

    *authenticated_user = Some(user);
    send_text(transport, "Login successful. Welcome back.\r\n", charset).await?;
    Ok(AuthFlowResult::Success)
}

async fn run_new_user_flow<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    state: &mut AuthFlowState<'_>,
    charset: TerminalCharset,
) -> ServeResult<AuthFlowResult> {
    let db = state.db;
    let node_number = state.node_number;
    let session_id = state.session_id;
    let idle_timeout = state.idle_timeout;
    let disconnect_reason = &mut *state.disconnect_reason;
    let authenticated_user = &mut *state.authenticated_user;

    send_text(transport, "\r\n-- Registration --\r\n", charset).await?;

    let alias = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        false,
        false,
        "Choose an alias: ",
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => value,
        PromptLineResult::Disconnected => {
            *disconnect_reason = "caller_dropped_during_login".to_string();
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::IdleTimeout => {
            *disconnect_reason = "idle_timeout".to_string();
            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::Rejected => {
            unreachable!("prompt_for_line handles rejected input internally");
        }
    };

    let real_name = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        false,
        false,
        "Real name: ",
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => value,
        PromptLineResult::Disconnected => {
            *disconnect_reason = "caller_dropped_during_login".to_string();
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::IdleTimeout => {
            *disconnect_reason = "idle_timeout".to_string();
            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::Rejected => {
            unreachable!("prompt_for_line handles rejected input internally");
        }
    };

    let email = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        true,
        false,
        "Email (optional): ",
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => {
            let email = value.trim();
            if email.is_empty() {
                None
            } else {
                Some(email.to_string())
            }
        }
        PromptLineResult::Disconnected => {
            *disconnect_reason = "caller_dropped_during_login".to_string();
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::IdleTimeout => {
            *disconnect_reason = "idle_timeout".to_string();
            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::Rejected => {
            unreachable!("prompt_for_line handles rejected input internally");
        }
    };

    let password = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        false,
        true,
        "Choose password: ",
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => value,
        PromptLineResult::Disconnected => {
            *disconnect_reason = "caller_dropped_during_login".to_string();
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::IdleTimeout => {
            *disconnect_reason = "idle_timeout".to_string();
            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::Rejected => {
            unreachable!("prompt_for_line handles rejected input internally");
        }
    };

    let password_confirmation = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        false,
        true,
        "Confirm password: ",
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => value,
        PromptLineResult::Disconnected => {
            *disconnect_reason = "caller_dropped_during_login".to_string();
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::IdleTimeout => {
            *disconnect_reason = "idle_timeout".to_string();
            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::Rejected => {
            unreachable!("prompt_for_line handles rejected input internally");
        }
    };

    if password != password_confirmation {
        send_text(transport, "Passwords did not match.\r\n", charset).await?;
        return Ok(AuthFlowResult::Retry);
    }

    let created_at = current_timestamp(db)?;
    let password_hash = server_hash_password(&password, &state.config.auth.argon2)?;
    let user = match create_new_user(NewUserInput {
        id: generated_uuid(db)?,
        alias,
        real_name,
        email,
        password_hash,
        security_level: state.config.auth.new_user_security_level,
        created_at: created_at.clone(),
    }) {
        Ok(user) => user,
        Err(error) => {
            send_text(
                transport,
                &format!("Unable to create account: {error}\r\n"),
                charset,
            )
            .await?;
            return Ok(AuthFlowResult::Retry);
        }
    };
    let mut user = user;
    let record = UserRecord {
        id: user.id.clone(),
        alias: user.alias.clone(),
        real_name: user.real_name.clone(),
        email: user.email.clone(),
        password_hash: user.password_hash.clone(),
        security_level: i64::from(user.security_level),
        is_sysop: user.is_sysop,
        created_at: user.created_at.clone(),
        last_login_at: user.last_login_at.clone(),
        total_calls: user.total_calls,
        time_bank_minutes: user.time_bank_minutes,
        status: user_status_to_db(&user.status),
    };
    if let Err(error) = insert_user_if_alias_available(db.db(), &record) {
        match error {
            UserInsertError::DuplicateAlias { .. } => {
                debug!(
                    node = %node_number,
                    remote_ip = %state.remote_ip,
                    alias = %user.alias,
                    "new-user alias rejected as duplicate"
                );
                send_text(transport, "That alias is already in use.\r\n", charset).await?;
                return Ok(AuthFlowResult::Retry);
            }
            UserInsertError::Db(error) => {
                emit_db_write_failed_event(
                    db,
                    Some(node_number),
                    Some(user.id.clone()),
                    "insert_user",
                    &error,
                    "failed to create new user record",
                    Some(state.runtime),
                );
                return Ok(AuthFlowResult::Retry);
            }
        }
    }

    if let Err(error) = update_user_login(db.db(), &user.id, &created_at) {
        emit_db_write_failed_event(
            db,
            Some(node_number),
            Some(user.id.clone()),
            "update_user_login",
            &error,
            "failed to update new user login counters",
            Some(state.runtime),
        );
        warn!(
            "failed to update new user login counters for {}: {error}",
            user.alias
        );
    } else {
        user.last_login_at = Some(created_at.clone());
        user.total_calls += 1;
    }

    if let Err(error) = update_session_user(db.db(), session_id, &user.id) {
        emit_db_write_failed_event(
            db,
            Some(node_number),
            Some(user.id.clone()),
            "update_session_user",
            &error,
            "failed to associate new user with session",
            Some(state.runtime),
        );
        warn!(
            "failed to associate user {} with session {}: {error}",
            user.alias, session_id
        );
    }

    *authenticated_user = Some(user.clone());

    emit_audit_event_with_runtime(
        db,
        "new_user_created",
        Some(user.id.clone()),
        Some(node_number),
        format!("new user created for {}", user.alias),
        Some(state.runtime),
    );
    emit_audit_event_with_runtime(
        db,
        "login_success",
        Some(user.id.clone()),
        Some(node_number),
        format!("new user logged in as {}", user.alias),
        Some(state.runtime),
    );
    debug!(
        node = %node_number,
        remote_ip = %state.remote_ip,
        user_id = %user.id,
        alias = %user.alias,
        security_level = user.security_level,
        "new user created and signed in"
    );

    send_text(transport, "Account created. Welcome.\r\n", charset).await?;
    Ok(AuthFlowResult::Success)
}

async fn run_doors_flow(
    authenticated_user: Option<&User>,
    transport: &mut impl Transport,
    input: &mut InputSession,
    state: &mut DoorFlowState<'_>,
    charset: TerminalCharset,
) -> ServeResult<MenuFlowResult> {
    let Some(user) = authenticated_user else {
        send_text(
            transport,
            "You must be signed in to use doors.\r\n",
            charset,
        )
        .await?;
        return Ok(MenuFlowResult::Continue);
    };

    let service = DoorService::new(state.db, state.config);
    let doors = service.list_enabled_doors()?;
    if doors.is_empty() {
        send_text(transport, "No doors are available.\r\n", charset).await?;
        return Ok(MenuFlowResult::Continue);
    }

    loop {
        send_text(transport, &render_door_menu(&doors), charset).await?;
        let selected = match prompt_for_line(
            transport,
            input,
            state.idle_timeout,
            true,
            false,
            "Door key or number (blank to return): ",
            charset,
        )
        .await?
        {
            PromptLineResult::Value(value) => value,
            PromptLineResult::Disconnected => {
                *state.disconnect_reason = "caller_dropped_during_door_menu".to_string();
                return Ok(MenuFlowResult::Exit);
            }
            PromptLineResult::IdleTimeout => {
                *state.disconnect_reason = "idle_timeout".to_string();
                send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
                return Ok(MenuFlowResult::Exit);
            }
            PromptLineResult::Rejected => {
                unreachable!("prompt_for_line handles rejected input internally");
            }
        };

        let door = match select_door(&doors, &selected) {
            DoorSelection::Return => return Ok(MenuFlowResult::Continue),
            DoorSelection::Door(door) => door,
            DoorSelection::Invalid => {
                send_text(transport, "Unknown door.\r\n", charset).await?;
                continue;
            }
        };
        debug!(
            node = %state.node_number,
            user_id = %user.id,
            alias = %user.alias,
            door_key = %door.key,
            door_name = %door.name,
            "caller selected door"
        );

        if door_access_denied(user.security_level, door.min_security_level) {
            debug!(
                node = %state.node_number,
                user_level = user.security_level,
                door_level = door.min_security_level,
                door_key = %door.key,
                "caller denied by door min_security_level"
            );
            send_text(transport, ACCESS_DENIED_MESSAGE, charset).await?;
            continue;
        }

        if let Err(message) = service.validate_door(door, state.node_number) {
            warn!(
                door = %door.key,
                node = %state.node_number,
                "door unavailable before launch: {message}"
            );
            emit_audit_event_with_runtime(
                state.db,
                "door_unavailable",
                Some(user.id.clone()),
                Some(i64::from(state.node_number)),
                format!("door {} unavailable before launch: {message}", door.key),
                Some(state.runtime),
            );
            send_text(
                transport,
                "This door is not available right now. Contact the sysop.\r\n",
                charset,
            )
            .await?;
            continue;
        }

        send_text(
            transport,
            &format!("\r\nLaunching {}...\r\n", door.name),
            charset,
        )
        .await?;
        let summary = service
            .execute_interactive(transport, state.runtime, user, state.node_number, door)
            .await?;
        debug!(
            node = %state.node_number,
            user_id = %user.id,
            alias = %user.alias,
            door_key = %door.key,
            door_name = %door.name,
            run_id = ?summary.run_id,
            exit_code = ?summary.exit_code,
            timed_out = summary.timed_out,
            caller_disconnected = summary.caller_disconnected,
            disconnect_forced = summary.disconnect_forced,
            bytes_in = summary.bytes_in,
            bytes_out = summary.bytes_out,
            "door run completed"
        );

        if summary.caller_disconnected {
            *state.disconnect_reason = "caller_dropped_during_door".to_string();
            return Ok(MenuFlowResult::Exit);
        }
        if let Some(reason) = summary.disconnect_reason.as_ref() {
            *state.disconnect_reason = reason.clone();
            return Ok(MenuFlowResult::Exit);
        }

        state.runtime.mark_node_main_menu(state.node_number);
        send_text(transport, &door_summary_text(&summary), charset).await?;
        return Ok(MenuFlowResult::Continue);
    }
}

fn door_access_denied(user_security_level: i32, door_min_security_level: i64) -> bool {
    user_security_level < door_min_security_level as i32
}

fn door_summary_text(summary: &DoorExecutionSummary) -> String {
    let run_id = summary
        .run_id
        .as_deref()
        .map(|id| format!(" Run id: {id}."))
        .unwrap_or_default();
    let diagnostics = if summary.stdout_log.is_some() || summary.stderr_log.is_some() {
        " Door diagnostics were captured."
    } else {
        ""
    };
    if let Some(error) = summary.launch_error.as_ref() {
        warn!(
            door = %summary.door_name,
            "door launch failed after run record was created: {error}"
        );
        return format!(
            "Unable to launch {}. Contact the sysop.{run_id}{diagnostics}\r\n",
            summary.door_name
        );
    }
    if summary.timed_out {
        return format!(
            "{} timed out and was closed.{run_id}{diagnostics}\r\n",
            summary.door_name
        );
    }
    if summary.early_exit_before_com1 {
        return format!(
            "{} finished before opening the serial bridge. Exit code: {:?}.{run_id}{diagnostics}\r\n",
            summary.door_name, summary.exit_code
        );
    }

    format!(
        "{} finished. Exit code: {:?}.{run_id}{diagnostics}\r\n",
        summary.door_name, summary.exit_code
    )
}

async fn run_messages_flow(
    authenticated_user: Option<&User>,
    transport: &mut impl Transport,
    input: &mut InputSession,
    state: &mut MessageFlowState<'_>,
    charset: TerminalCharset,
) -> ServeResult<MenuFlowResult> {
    let db = state.db;
    let idle_timeout = state.idle_timeout;
    let runtime = state.runtime;
    let node_number = state.node_number;
    let disconnect_reason = &mut *state.disconnect_reason;

    let Some(user) = authenticated_user else {
        send_text(
            transport,
            "You must be signed in to use messages.\r\n",
            charset,
        )
        .await?;
        return Ok(MenuFlowResult::Continue);
    };

    ensure_default_message_area(db, transport, charset).await?;
    let area_records = list_message_areas(db.db())?
        .into_iter()
        .filter(|area| area.enabled)
        .collect::<Vec<_>>();
    if area_records.is_empty() {
        send_text(transport, "No message areas are configured.\r\n", charset).await?;
        return Ok(MenuFlowResult::Continue);
    }

    loop {
        send_text(transport, "\r\nMessage areas:\r\n", charset).await?;
        for area in &area_records {
            send_text(
                transport,
                &format!("{} - {}\r\n", area.key, area.description),
                charset,
            )
            .await?;
        }

        let selected_area_key = match prompt_for_line(
            transport,
            input,
            idle_timeout,
            true,
            false,
            "Area key (blank to return): ",
            charset,
        )
        .await?
        {
            PromptLineResult::Value(value) => value.trim().to_ascii_lowercase(),
            PromptLineResult::Disconnected => {
                *disconnect_reason = "caller_dropped_during_messages".to_string();
                return Ok(MenuFlowResult::Exit);
            }
            PromptLineResult::IdleTimeout => {
                *disconnect_reason = "idle_timeout".to_string();
                send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
                return Ok(MenuFlowResult::Exit);
            }
            PromptLineResult::Rejected => {
                unreachable!("prompt_for_line handles rejected input internally");
            }
        };

        if selected_area_key.is_empty() {
            return Ok(MenuFlowResult::Continue);
        }

        let area_record = match area_records
            .iter()
            .find(|area| area.key.eq_ignore_ascii_case(&selected_area_key))
        {
            Some(area) => area,
            None => {
                send_text(transport, "Unknown area.\r\n", charset).await?;
                continue;
            }
        };
        let area = message_area_from_record(area_record)?;
        debug!(
            node = %node_number,
            user_id = %user.id,
            alias = %user.alias,
            area_key = %area.key,
            area_id = %area.id,
            "caller selected message area"
        );

        loop {
            runtime.mark_node_reading_messages(node_number);
            let visible = visible_messages_for_user(db, &area, user.security_level)?;
            display_message_list(transport, db, &area, &visible, charset).await?;

            let action = match prompt_for_line(
                transport,
                input,
                idle_timeout,
                true,
                false,
                "Read (R), Post (P), Reply (Y), Back (blank): ",
                charset,
            )
            .await?
            {
                PromptLineResult::Value(value) => value,
                PromptLineResult::Disconnected => {
                    *disconnect_reason = "caller_dropped_during_messages".to_string();
                    return Ok(MenuFlowResult::Exit);
                }
                PromptLineResult::IdleTimeout => {
                    *disconnect_reason = "idle_timeout".to_string();
                    send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
                    return Ok(MenuFlowResult::Exit);
                }
                PromptLineResult::Rejected => {
                    unreachable!("prompt_for_line handles rejected input internally");
                }
            };
            let action = action
                .trim()
                .chars()
                .next()
                .map(|key| key.to_ascii_uppercase());

            match action {
                None | Some('B') => break,
                Some('R') => {
                    let index = match prompt_for_message_index(
                        transport,
                        input,
                        idle_timeout,
                        disconnect_reason,
                        visible.len(),
                        "Message number to read: ",
                        charset,
                    )
                    .await?
                    {
                        MessageIndexPromptResult::Index(index) => index,
                        MessageIndexPromptResult::Retry => continue,
                        MessageIndexPromptResult::Exit => return Ok(MenuFlowResult::Exit),
                    };
                    debug!(
                        node = %node_number,
                        user_id = %user.id,
                        alias = %user.alias,
                        area_key = %area.key,
                        message_id = %visible[index].id,
                        action = "read",
                        "caller selected message"
                    );
                    display_message(transport, db, &visible[index], charset).await?;
                }
                Some('P') => {
                    runtime.mark_node_posting_message(node_number);
                    let subject = match prompt_for_line(
                        transport,
                        input,
                        idle_timeout,
                        false,
                        false,
                        "Message subject: ",
                        charset,
                    )
                    .await?
                    {
                        PromptLineResult::Value(value) => value,
                        PromptLineResult::Disconnected => {
                            *disconnect_reason = "caller_dropped_during_messages".to_string();
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::IdleTimeout => {
                            *disconnect_reason = "idle_timeout".to_string();
                            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::Rejected => {
                            unreachable!("prompt_for_line handles rejected input internally");
                        }
                    };
                    if validate_caller_cp437_text(&subject).is_err() {
                        send_text(transport, CP437_INPUT_REJECT_LINE, charset).await?;
                        continue;
                    }

                    let body = match prompt_for_message_body(
                        transport,
                        input,
                        idle_timeout,
                        charset,
                    )
                    .await?
                    {
                        PromptLineResult::Value(value) => value,
                        PromptLineResult::Disconnected => {
                            *disconnect_reason = "caller_dropped_during_messages".to_string();
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::IdleTimeout => {
                            *disconnect_reason = "idle_timeout".to_string();
                            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::Rejected => {
                            unreachable!(
                                "prompt_for_message_body only returns rejection on CP437 validation"
                            );
                        }
                    };
                    if validate_caller_cp437_text(&body).is_err() {
                        send_text(transport, CP437_INPUT_REJECT_LINE, charset).await?;
                        continue;
                    }

                    let draft = PostMessageCommand {
                        id: generated_uuid(db)?,
                        author_user_id: user.id.clone(),
                        author_security_level: user.security_level,
                        to_user_id: None,
                        subject,
                        body,
                        created_at: current_timestamp(db)?,
                    };
                    let message = match post_message(&area, draft) {
                        Ok(message) => message,
                        Err(error) => {
                            send_text(
                                transport,
                                &format!("Cannot post message: {error}\r\n"),
                                charset,
                            )
                            .await?;
                            continue;
                        }
                    };
                    if let Err(error) =
                        insert_message(db.db(), &message_record_from_message(&message))
                    {
                        emit_db_write_failed_event(
                            db,
                            Some(i64::from(node_number)),
                            Some(user.id.clone()),
                            "insert_message",
                            &error,
                            "failed to save posted message",
                            Some(state.runtime),
                        );
                        return Err(ServeError::Database(error));
                    }
                    debug!(
                        node = %node_number,
                        user_id = %user.id,
                        alias = %user.alias,
                        area_key = %area.key,
                        message_id = %message.id,
                        action = "post",
                        "caller posted message"
                    );
                    send_text(transport, "Message posted.\r\n", charset).await?;
                    runtime.mark_node_reading_messages(node_number);
                }
                Some('Y') => {
                    if visible.is_empty() {
                        send_text(transport, "No messages to reply to.\r\n", charset).await?;
                        continue;
                    }

                    runtime.mark_node_posting_message(node_number);
                    let index = match prompt_for_message_index(
                        transport,
                        input,
                        idle_timeout,
                        disconnect_reason,
                        visible.len(),
                        "Message number to reply to: ",
                        charset,
                    )
                    .await?
                    {
                        MessageIndexPromptResult::Index(index) => index,
                        MessageIndexPromptResult::Retry => continue,
                        MessageIndexPromptResult::Exit => return Ok(MenuFlowResult::Exit),
                    };

                    let body = match prompt_for_message_body(
                        transport,
                        input,
                        idle_timeout,
                        charset,
                    )
                    .await?
                    {
                        PromptLineResult::Value(value) => value,
                        PromptLineResult::Disconnected => {
                            *disconnect_reason = "caller_dropped_during_messages".to_string();
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::IdleTimeout => {
                            *disconnect_reason = "idle_timeout".to_string();
                            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::Rejected => {
                            unreachable!(
                                "prompt_for_message_body only returns rejection on CP437 validation"
                            );
                        }
                    };
                    if validate_caller_cp437_text(&body).is_err() {
                        send_text(transport, CP437_INPUT_REJECT_LINE, charset).await?;
                        continue;
                    }

                    let draft = ReplyMessageCommand {
                        id: generated_uuid(db)?,
                        author_user_id: user.id.clone(),
                        author_security_level: user.security_level,
                        body,
                        created_at: current_timestamp(db)?,
                    };
                    let message = match reply_message(&area, &visible[index], draft) {
                        Ok(message) => message,
                        Err(error) => {
                            send_text(transport, &format!("Cannot reply: {error}\r\n"), charset)
                                .await?;
                            continue;
                        }
                    };
                    if let Err(error) =
                        insert_message(db.db(), &message_record_from_message(&message))
                    {
                        emit_db_write_failed_event(
                            db,
                            Some(i64::from(node_number)),
                            Some(user.id.clone()),
                            "insert_message",
                            &error,
                            "failed to save reply",
                            Some(state.runtime),
                        );
                        return Err(ServeError::Database(error));
                    }
                    debug!(
                        node = %node_number,
                        user_id = %user.id,
                        alias = %user.alias,
                        area_key = %area.key,
                        message_id = %message.id,
                        reply_to_id = ?message.reply_to_id,
                        action = "reply",
                        "caller posted reply"
                    );
                    send_text(transport, "Reply posted.\r\n", charset).await?;
                    runtime.mark_node_reading_messages(node_number);
                }
                Some(_) => {
                    send_text(transport, "Unknown command.\r\n", charset).await?;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_files_flow<T: Transport>(
    authenticated_user: Option<&User>,
    transport: &mut T,
    input: &mut InputSession,
    db: &OxideDb,
    config: &OxideConfig,
    telnet_protocol: bool,
    idle_timeout: Duration,
    disconnect_reason: &mut String,
    node_number: u16,
    charset: TerminalCharset,
) -> ServeResult<MenuFlowResult> {
    let Some(user) = authenticated_user else {
        send_text(
            transport,
            "You must be signed in to use file areas.\r\n",
            charset,
        )
        .await?;
        return Ok(MenuFlowResult::Continue);
    };

    if !config.file_transfers.enabled {
        send_text(transport, "File transfers are disabled.\r\n", charset).await?;
        return Ok(MenuFlowResult::Continue);
    }

    let file_areas = oxidebbs_db::list_file_areas(db.db())?
        .into_iter()
        .filter(|area| area.enabled)
        .collect::<Vec<_>>();

    if file_areas.is_empty() {
        send_text(transport, "No file areas are configured.\r\n", charset).await?;
        return Ok(MenuFlowResult::Continue);
    }

    loop {
        send_text(transport, "\r\nFile areas:\r\n", charset).await?;
        for (index, area) in file_areas.iter().enumerate() {
            let accessible = user.security_level >= area.read_security_level as i32;
            let marker = if accessible { " " } else { "*" };
            send_text(
                transport,
                &format!(
                    "{}[{}] {} - {}\r\n",
                    marker,
                    index + 1,
                    area.key,
                    area.description
                ),
                charset,
            )
            .await?;
        }

        let selection = match prompt_for_line(
            transport,
            input,
            idle_timeout,
            true,
            false,
            "Area number (blank to return): ",
            charset,
        )
        .await?
        {
            PromptLineResult::Value(value) => value,
            PromptLineResult::Disconnected => {
                *disconnect_reason = "caller_dropped_during_files".to_string();
                return Ok(MenuFlowResult::Exit);
            }
            PromptLineResult::IdleTimeout => {
                *disconnect_reason = "idle_timeout".to_string();
                send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
                return Ok(MenuFlowResult::Exit);
            }
            PromptLineResult::Rejected => {
                send_text(transport, CP437_INPUT_REJECT_LINE, charset).await?;
                continue;
            }
        };

        let trimmed = selection.trim();
        if trimmed.is_empty() {
            return Ok(MenuFlowResult::Continue);
        }

        let area_index = match trimmed.parse::<usize>() {
            Ok(n) if n >= 1 && n <= file_areas.len() => n - 1,
            _ => {
                send_text(transport, "Invalid selection.\r\n", charset).await?;
                continue;
            }
        };

        let area = &file_areas[area_index];
        if user.security_level < area.read_security_level as i32 {
            send_text(
                transport,
                "Access denied. Security level too low.\r\n",
                charset,
            )
            .await?;
            continue;
        }

        loop {
            let files = approved_files_for_area(db, area)?;
            send_text(
                transport,
                &format!("\r\nFiles in {}:\r\n", area.name),
                charset,
            )
            .await?;
            if files.is_empty() {
                send_text(transport, "No approved files in this area.\r\n", charset).await?;
            } else {
                for (index, file) in files.iter().enumerate() {
                    send_text(
                        transport,
                        &format!(
                            "[{}] {} ({} bytes)\r\n",
                            index + 1,
                            file.display_name,
                            file.size_bytes
                        ),
                        charset,
                    )
                    .await?;
                }
            }

            let action = match prompt_for_line(
                transport,
                input,
                idle_timeout,
                true,
                false,
                "Files: D)ownload U)pload R)eturn: ",
                charset,
            )
            .await?
            {
                PromptLineResult::Value(value) => value.trim().to_ascii_uppercase(),
                PromptLineResult::Disconnected => {
                    *disconnect_reason = "caller_dropped_during_files".to_string();
                    return Ok(MenuFlowResult::Exit);
                }
                PromptLineResult::IdleTimeout => {
                    *disconnect_reason = "idle_timeout".to_string();
                    send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
                    return Ok(MenuFlowResult::Exit);
                }
                PromptLineResult::Rejected => {
                    send_text(transport, CP437_INPUT_REJECT_LINE, charset).await?;
                    continue;
                }
            };

            if action.is_empty() || action == "R" {
                break;
            }

            match action.as_str() {
                "D" => {
                    if files.is_empty() {
                        send_text(
                            transport,
                            "No files are available for download.\r\n",
                            charset,
                        )
                        .await?;
                        continue;
                    }
                    run_file_download(
                        user,
                        transport,
                        input,
                        db,
                        area,
                        &files,
                        telnet_protocol,
                        idle_timeout,
                        disconnect_reason,
                        node_number,
                        charset,
                    )
                    .await?;
                }
                "U" => {
                    run_file_upload(
                        user,
                        transport,
                        input,
                        db,
                        config,
                        area,
                        telnet_protocol,
                        idle_timeout,
                        disconnect_reason,
                        node_number,
                        charset,
                    )
                    .await?;
                }
                _ => {
                    send_text(transport, "Unknown file command.\r\n", charset).await?;
                }
            }
        }
    }
}

fn approved_files_for_area(
    db: &OxideDb,
    area: &FileAreaRecord,
) -> ServeResult<Vec<FileEntryRecord>> {
    Ok(oxidebbs_db::list_file_entries(db.db())?
        .into_iter()
        .filter(|entry| entry.approved && entry.area_id == area.id)
        .collect::<Vec<_>>())
}

#[allow(clippy::too_many_arguments)]
async fn run_file_download<T: Transport>(
    user: &User,
    transport: &mut T,
    input: &mut InputSession,
    db: &OxideDb,
    area: &FileAreaRecord,
    files: &[FileEntryRecord],
    telnet_protocol: bool,
    idle_timeout: Duration,
    disconnect_reason: &mut String,
    node_number: u16,
    charset: TerminalCharset,
) -> ServeResult<()> {
    if user.security_level < area.download_security_level as i32 {
        send_text(
            transport,
            "Access denied. Security level too low for download.\r\n",
            charset,
        )
        .await?;
        return Ok(());
    }

    let file_selection = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        true,
        false,
        "File number (blank to return): ",
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => value,
        PromptLineResult::Disconnected => {
            *disconnect_reason = "caller_dropped_during_files".to_string();
            return Ok(());
        }
        PromptLineResult::IdleTimeout => {
            *disconnect_reason = "idle_timeout".to_string();
            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
            return Ok(());
        }
        PromptLineResult::Rejected => {
            send_text(transport, CP437_INPUT_REJECT_LINE, charset).await?;
            return Ok(());
        }
    };
    let file_trimmed = file_selection.trim();
    if file_trimmed.is_empty() {
        return Ok(());
    }
    let file_index = match file_trimmed.parse::<usize>() {
        Ok(n) if n >= 1 && n <= files.len() => n - 1,
        _ => {
            send_text(transport, "Invalid selection.\r\n", charset).await?;
            return Ok(());
        }
    };
    let protocol = match prompt_transfer_protocol(transport, input, idle_timeout, charset).await? {
        Some(protocol) => protocol,
        None => return Ok(()),
    };

    let file = &files[file_index];
    let file_path = file_entry_path(area, file);
    if !file_path.exists() {
        send_text(transport, "File not found on disk.\r\n", charset).await?;
        return Ok(());
    }
    let file_bytes = match std::fs::read(&file_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            send_text(transport, "Cannot read file.\r\n", charset).await?;
            return Ok(());
        }
    };

    send_text(
        transport,
        &format!(
            "\r\nSending {} ({} bytes) via {}...\r\n",
            file.display_name,
            file.size_bytes,
            transfer_protocol_label(protocol)
        ),
        charset,
    )
    .await?;
    if protocol == TransferProtocol::XmodemCrc {
        send_text(
            transport,
            "Start XMODEM receive in your terminal now. CRC is used when the terminal requests it.\r\n", charset)
        .await?;
    }

    let started_at = current_timestamp(db)?;
    let started = Instant::now();
    let transfer_result = {
        let mut adapter = if telnet_protocol {
            TransportAdapter::new_telnet(&mut *transport)
        } else {
            TransportAdapter::new_raw(&mut *transport)
        };
        match protocol {
            TransferProtocol::Zmodem => oxidebbs_transfer::zmodem::send_zmodem_file(
                &mut adapter,
                &file.display_name,
                &file_bytes,
            )
            .await
            .map(|stats| stats.retries),
            TransferProtocol::XmodemCrc => {
                oxidebbs_transfer::xmodem::send_xmodem_crc(&mut adapter, &file_bytes)
                    .await
                    .map(|()| 0)
            }
        }
    };
    if protocol == TransferProtocol::Zmodem && transfer_result.is_ok() {
        drain_zmodem_finish_sequence(transport, input).await?;
    }
    let ended_at = current_timestamp(db)?;
    let duration_ms = elapsed_millis(started);

    match transfer_result {
        Ok(retry_count) => {
            increment_file_entry_download_count(db.db(), &file.id)?;
            record_file_transfer(
                db,
                FileTransferInput {
                    node_number,
                    user_id: &user.id,
                    area_id: Some(&area.id),
                    file_entry_id: Some(&file.id),
                    direction: "download",
                    protocol,
                    requested_name: Some(&file.display_name),
                    storage_name: Some(&file.storage_name),
                    declared_size_bytes: Some(file.size_bytes),
                    transferred_payload_bytes: file_bytes.len() as i64,
                    committed_size_bytes: Some(file_bytes.len() as i64),
                    started_at,
                    ended_at: Some(ended_at),
                    duration_ms: Some(duration_ms),
                    outcome: "success",
                    error: None,
                    retry_count: i64::from(retry_count),
                },
            )?;
            send_text(transport, "\r\nTransfer complete.\r\n", charset).await?;
            debug!(node = %node_number, user_id = %user.id, file_id = %file.id, "caller downloaded file");
        }
        Err(error) => {
            record_file_transfer(
                db,
                FileTransferInput {
                    node_number,
                    user_id: &user.id,
                    area_id: Some(&area.id),
                    file_entry_id: Some(&file.id),
                    direction: "download",
                    protocol,
                    requested_name: Some(&file.display_name),
                    storage_name: Some(&file.storage_name),
                    declared_size_bytes: Some(file.size_bytes),
                    transferred_payload_bytes: 0,
                    committed_size_bytes: None,
                    started_at,
                    ended_at: Some(ended_at),
                    duration_ms: Some(duration_ms),
                    outcome: transfer_error_outcome(&error),
                    error: Some(&error),
                    retry_count: 0,
                },
            )?;
            send_text(
                transport,
                &format!("\r\nTransfer failed: {error}\r\n"),
                charset,
            )
            .await?;
            debug!(node = %node_number, user_id = %user.id, file_id = %file.id, %error, "file transfer failed");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_file_upload<T: Transport>(
    user: &User,
    transport: &mut T,
    input: &mut InputSession,
    db: &OxideDb,
    config: &OxideConfig,
    area: &FileAreaRecord,
    telnet_protocol: bool,
    idle_timeout: Duration,
    disconnect_reason: &mut String,
    node_number: u16,
    charset: TerminalCharset,
) -> ServeResult<()> {
    if user.security_level < area.upload_security_level as i32 {
        send_text(
            transport,
            "Access denied. Security level too low for upload.\r\n",
            charset,
        )
        .await?;
        return Ok(());
    }
    let protocol = match prompt_transfer_protocol(transport, input, idle_timeout, charset).await? {
        Some(protocol) => protocol,
        None => return Ok(()),
    };
    let upload_limit = upload_limit_bytes(area, config);

    let mut xmodem_name = None;
    let mut declared_size = None;
    if protocol == TransferProtocol::XmodemCrc {
        let filename = prompt_required_value(
            transport,
            input,
            idle_timeout,
            "Upload filename: ",
            disconnect_reason,
            charset,
        )
        .await?;
        let Some(filename) = filename else {
            return Ok(());
        };
        let safe_name = match sanitize_filename(filename.trim()) {
            Ok(name) => name,
            Err(_) => {
                send_text(transport, "Invalid upload filename.\r\n", charset).await?;
                return Ok(());
            }
        };
        let declared = prompt_for_line(
            transport,
            input,
            idle_timeout,
            true,
            false,
            "Declared size bytes (blank if unknown): ",
            charset,
        )
        .await?;
        if let PromptLineResult::Value(value) = declared {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                match trimmed.parse::<u64>() {
                    Ok(size) => declared_size = Some(size),
                    Err(_) => {
                        send_text(transport, "Invalid declared size.\r\n", charset).await?;
                        return Ok(());
                    }
                }
            }
        }
        xmodem_name = Some(safe_name);
    }

    send_text(
        transport,
        &format!(
            "\r\nReady to receive via {}...\r\n",
            transfer_protocol_label(protocol)
        ),
        charset,
    )
    .await?;

    let started_at = current_timestamp(db)?;
    let started = Instant::now();
    let transfer_result = {
        let mut adapter = if telnet_protocol {
            TransportAdapter::new_telnet(&mut *transport)
        } else {
            TransportAdapter::new_raw(&mut *transport)
        };
        match protocol {
            TransferProtocol::Zmodem => {
                oxidebbs_transfer::zmodem::receive_zmodem_file(&mut adapter, upload_limit)
                    .await
                    .map(|file| (file.filename, file.payload, None, 0))
            }
            TransferProtocol::XmodemCrc => {
                let result = if let Some(size) = declared_size {
                    oxidebbs_transfer::xmodem::receive_xmodem_crc_with_size(
                        &mut adapter,
                        size as usize,
                    )
                    .await
                } else {
                    oxidebbs_transfer::xmodem::receive_xmodem_crc(&mut adapter).await
                };
                result.map(|payload| {
                    (
                        xmodem_name
                            .clone()
                            .unwrap_or_else(|| "upload.bin".to_string()),
                        payload,
                        declared_size,
                        0,
                    )
                })
            }
        }
    };
    if protocol == TransferProtocol::Zmodem && transfer_result.is_ok() {
        drain_zmodem_finish_sequence(transport, input).await?;
    }
    let ended_at = current_timestamp(db)?;
    let duration_ms = elapsed_millis(started);

    match transfer_result {
        Ok((requested_name, payload, declared_size, retry_count)) => {
            if let Some(limit) = upload_limit
                && payload.len() as u64 > limit
            {
                send_text(
                    transport,
                    "Upload exceeds configured size limit.\r\n",
                    charset,
                )
                .await?;
                record_file_transfer(
                    db,
                    FileTransferInput {
                        node_number,
                        user_id: &user.id,
                        area_id: Some(&area.id),
                        file_entry_id: None,
                        direction: "upload",
                        protocol,
                        requested_name: Some(&requested_name),
                        storage_name: None,
                        declared_size_bytes: declared_size.map(|size| size as i64),
                        transferred_payload_bytes: payload.len() as i64,
                        committed_size_bytes: None,
                        started_at,
                        ended_at: Some(ended_at),
                        duration_ms: Some(duration_ms),
                        outcome: "failed",
                        error: Some(&TransferError::QuotaDenied),
                        retry_count,
                    },
                )?;
                return Ok(());
            }
            let safe_name = match sanitize_filename(&requested_name) {
                Ok(name) => name,
                Err(_) => {
                    send_text(transport, "Invalid upload filename.\r\n", charset).await?;
                    return Ok(());
                }
            };
            let entry_seed = generated_uuid(db)?;
            let storage_name = storage_name_for_upload(&safe_name, &entry_seed);
            let root = PathBuf::from(&area.root_path);
            std::fs::create_dir_all(&root)?;
            let destination = root.join(&storage_name);
            validate_path_within_base(&root, &destination)
                .map_err(|error| ServeError::Runtime(error.to_string()))?;
            std::fs::write(&destination, &payload)?;

            let size_bytes = i64::try_from(payload.len())
                .map_err(|_| ServeError::Runtime("uploaded file is too large".to_string()))?;
            let crc = oxidebbs_transfer::zmodem::crc32_iso_hdlc(&payload);
            let entry = FileEntryRecord {
                id: entry_seed,
                area_id: area.id.clone(),
                storage_name: storage_name.clone(),
                display_name: safe_name.clone(),
                original_name: Some(requested_name.clone()),
                size_bytes,
                content_crc32: Some(format!("{crc:08X}")),
                description: "Caller upload pending sysop review".to_string(),
                uploader_user_id: Some(user.id.clone()),
                download_count: 0,
                approved: false,
                created_at: current_timestamp(db)?,
                updated_at: current_timestamp(db)?,
            };
            insert_file_entry(db.db(), &entry)?;
            let stored_entry =
                find_file_entry_by_storage_name(db.db(), &area.id, &storage_name)?.unwrap_or(entry);
            record_file_transfer(
                db,
                FileTransferInput {
                    node_number,
                    user_id: &user.id,
                    area_id: Some(&area.id),
                    file_entry_id: Some(&stored_entry.id),
                    direction: "upload",
                    protocol,
                    requested_name: Some(&requested_name),
                    storage_name: Some(&storage_name),
                    declared_size_bytes: declared_size.map(|size| size as i64),
                    transferred_payload_bytes: size_bytes,
                    committed_size_bytes: Some(size_bytes),
                    started_at,
                    ended_at: Some(ended_at),
                    duration_ms: Some(duration_ms),
                    outcome: "success",
                    error: None,
                    retry_count,
                },
            )?;
            send_text(
                transport,
                "\r\nUpload complete. File is pending sysop review.\r\n",
                charset,
            )
            .await?;
        }
        Err(error) => {
            record_file_transfer(
                db,
                FileTransferInput {
                    node_number,
                    user_id: &user.id,
                    area_id: Some(&area.id),
                    file_entry_id: None,
                    direction: "upload",
                    protocol,
                    requested_name: xmodem_name.as_deref(),
                    storage_name: None,
                    declared_size_bytes: declared_size.map(|size| size as i64),
                    transferred_payload_bytes: 0,
                    committed_size_bytes: None,
                    started_at,
                    ended_at: Some(ended_at),
                    duration_ms: Some(duration_ms),
                    outcome: transfer_error_outcome(&error),
                    error: Some(&error),
                    retry_count: 0,
                },
            )?;
            send_text(
                transport,
                &format!("\r\nUpload failed: {error}\r\n"),
                charset,
            )
            .await?;
        }
    }
    Ok(())
}

async fn prompt_transfer_protocol<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    idle_timeout: Duration,
    charset: TerminalCharset,
) -> ServeResult<Option<TransferProtocol>> {
    let protocol = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        true,
        false,
        "Protocol: Z) ZMODEM  X) XMODEM  blank to return: ",
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => value.trim().to_ascii_uppercase(),
        PromptLineResult::Disconnected | PromptLineResult::IdleTimeout => return Ok(None),
        PromptLineResult::Rejected => {
            send_text(transport, CP437_INPUT_REJECT_LINE, charset).await?;
            return Ok(None);
        }
    };

    match protocol.as_str() {
        "" => Ok(None),
        "Z" => Ok(Some(TransferProtocol::Zmodem)),
        "X" => Ok(Some(TransferProtocol::XmodemCrc)),
        _ => {
            send_text(transport, "Unsupported transfer protocol.\r\n", charset).await?;
            Ok(None)
        }
    }
}

async fn prompt_required_value<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    idle_timeout: Duration,
    prompt: &str,
    disconnect_reason: &mut String,
    charset: TerminalCharset,
) -> ServeResult<Option<String>> {
    match prompt_for_line(
        transport,
        input,
        idle_timeout,
        false,
        false,
        prompt,
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => Ok(Some(value)),
        PromptLineResult::Disconnected => {
            *disconnect_reason = "caller_dropped_during_files".to_string();
            Ok(None)
        }
        PromptLineResult::IdleTimeout => {
            *disconnect_reason = "idle_timeout".to_string();
            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
            Ok(None)
        }
        PromptLineResult::Rejected => {
            send_text(transport, CP437_INPUT_REJECT_LINE, charset).await?;
            Ok(None)
        }
    }
}

fn file_entry_path(area: &FileAreaRecord, file: &FileEntryRecord) -> PathBuf {
    let root = Path::new(&area.root_path);
    let current = root.join(&file.storage_name);
    if current.exists() {
        current
    } else {
        root.join("files").join(&file.id).join(&file.storage_name)
    }
}

fn upload_limit_bytes(area: &FileAreaRecord, config: &OxideConfig) -> Option<u64> {
    let global = u64::try_from(config.file_transfers.max_upload_bytes)
        .ok()
        .filter(|value| *value > 0);
    let area_limit = area
        .max_upload_bytes
        .and_then(|value| u64::try_from(value).ok());
    match (area_limit, global) {
        (Some(area), Some(global)) => Some(area.min(global)),
        (Some(area), None) => Some(area),
        (None, Some(global)) => Some(global),
        (None, None) => None,
    }
}

fn storage_name_for_upload(filename: &str, id: &str) -> String {
    Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| {
            !extension.is_empty()
                && extension
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        .map_or_else(|| id.to_string(), |extension| format!("{id}.{extension}"))
}

fn elapsed_millis(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

fn transfer_protocol_label(protocol: TransferProtocol) -> &'static str {
    match protocol {
        TransferProtocol::Zmodem => "ZMODEM",
        TransferProtocol::XmodemCrc => "XMODEM",
    }
}

fn transfer_protocol_db_value(protocol: TransferProtocol) -> &'static str {
    match protocol {
        TransferProtocol::Zmodem => "zmodem",
        TransferProtocol::XmodemCrc => "xmodem_crc",
    }
}

fn transfer_error_outcome(error: &TransferError) -> &'static str {
    match error {
        TransferError::Canceled => "cancelled",
        _ => "failed",
    }
}

fn transfer_error_code(error: &TransferError) -> &'static str {
    match error {
        TransferError::ProtocolError => "protocol_error",
        TransferError::Timeout => "timeout",
        TransferError::Transport => "transport",
        TransferError::IoError(_) => "io_error",
        TransferError::SecurityDenied => "security_denied",
        TransferError::QuotaDenied => "quota_denied",
        TransferError::Canceled => "cancelled",
        TransferError::Unsupported => "unsupported",
        TransferError::PathInvalid => "path_invalid",
    }
}

struct FileTransferInput<'a> {
    node_number: u16,
    user_id: &'a str,
    area_id: Option<&'a str>,
    file_entry_id: Option<&'a str>,
    direction: &'a str,
    protocol: TransferProtocol,
    requested_name: Option<&'a str>,
    storage_name: Option<&'a str>,
    declared_size_bytes: Option<i64>,
    transferred_payload_bytes: i64,
    committed_size_bytes: Option<i64>,
    started_at: String,
    ended_at: Option<String>,
    duration_ms: Option<i64>,
    outcome: &'a str,
    error: Option<&'a TransferError>,
    retry_count: i64,
}

fn record_file_transfer(db: &OxideDb, input: FileTransferInput<'_>) -> ServeResult<()> {
    insert_file_transfer(
        db.db(),
        &FileTransferRecord {
            id: String::new(),
            node_number: i64::from(input.node_number),
            user_id: input.user_id.to_string(),
            area_id: input.area_id.map(str::to_string),
            file_entry_id: input.file_entry_id.map(str::to_string),
            direction: input.direction.to_string(),
            protocol: transfer_protocol_db_value(input.protocol).to_string(),
            requested_name: input.requested_name.map(str::to_string),
            storage_name: input.storage_name.map(str::to_string),
            declared_size_bytes: input.declared_size_bytes,
            transferred_payload_bytes: input.transferred_payload_bytes,
            committed_size_bytes: input.committed_size_bytes,
            started_at: input.started_at,
            ended_at: input.ended_at,
            duration_ms: input.duration_ms,
            outcome: input.outcome.to_string(),
            error_code: input.error.map(transfer_error_code).map(str::to_string),
            error_message: input.error.map(ToString::to_string),
            retry_count: input.retry_count,
        },
    )
    .map_err(ServeError::Database)
}

async fn ensure_default_message_area<T: Transport>(
    db: &OxideDb,
    transport: &mut T,
    charset: TerminalCharset,
) -> ServeResult<()> {
    if !list_message_areas(db.db())?.is_empty() {
        return Ok(());
    }

    if let Err(error) = seed_default_message_area(db) {
        emit_db_write_failed_event(
            db,
            None,
            None,
            "seed_default_message_area",
            &error,
            "failed to seed default message area",
            None,
        );
        warn!("failed to seed default message area: {error}");
        send_text(
            transport,
            "Messages are not available right now.\r\n",
            charset,
        )
        .await?;
    }
    Ok(())
}

fn visible_messages_for_user(
    db: &OxideDb,
    area: &MessageArea,
    security_level: i32,
) -> ServeResult<Vec<Message>> {
    let records = list_visible_messages_in_area(db.db(), &area.id, i64::from(security_level))?;
    Ok(messages_from_records(&records))
}

async fn display_message_list<T: Transport>(
    transport: &mut T,
    db: &OxideDb,
    area: &MessageArea,
    messages: &[Message],
    charset: TerminalCharset,
) -> ServeResult<()> {
    let author_aliases = message_author_aliases(db, messages);
    send_text(
        transport,
        &format!("\r\n{} messages:\r\n", area.name),
        charset,
    )
    .await?;
    if messages.is_empty() {
        send_text(transport, "No messages in this area.\r\n", charset).await?;
        return Ok(());
    }

    for (index, message) in messages.iter().enumerate() {
        let author = author_alias_from_map(&author_aliases, &message.author_user_id);
        send_text(
            transport,
            &format!("  {}) {} (from {})\r\n", index + 1, message.subject, author),
            charset,
        )
        .await?;
    }
    Ok(())
}

async fn display_message<T: Transport>(
    transport: &mut T,
    db: &OxideDb,
    message: &Message,
    charset: TerminalCharset,
) -> ServeResult<()> {
    let author_aliases = message_author_aliases(db, std::slice::from_ref(message));
    let author = author_alias_from_map(&author_aliases, &message.author_user_id);
    send_text(
        transport,
        &format!(
            "\r\nSubject: {}\r\nFrom: {}\r\nDate: {}\r\n{}\r\n{}\r\n",
            message.subject,
            author,
            message.created_at,
            "-".repeat(40),
            message.body
        ),
        charset,
    )
    .await
}

async fn prompt_for_message_index<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    idle_timeout: Duration,
    disconnect_reason: &mut String,
    message_count: usize,
    prompt: &str,
    charset: TerminalCharset,
) -> ServeResult<MessageIndexPromptResult> {
    if message_count == 0 {
        send_text(transport, "No messages are available.\r\n", charset).await?;
        return Ok(MessageIndexPromptResult::Retry);
    }

    let selected = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        false,
        false,
        prompt,
        charset,
    )
    .await?
    {
        PromptLineResult::Value(value) => value,
        PromptLineResult::Disconnected => {
            *disconnect_reason = "caller_dropped_during_messages".to_string();
            return Ok(MessageIndexPromptResult::Exit);
        }
        PromptLineResult::IdleTimeout => {
            *disconnect_reason = "idle_timeout".to_string();
            send_text(transport, "Idle timeout. Goodbye.\r\n", charset).await?;
            return Ok(MessageIndexPromptResult::Exit);
        }
        PromptLineResult::Rejected => {
            unreachable!("prompt_for_line handles rejected input internally");
        }
    };

    match selected.trim().parse::<usize>() {
        Ok(index) if (1..=message_count).contains(&index) => {
            Ok(MessageIndexPromptResult::Index(index - 1))
        }
        Ok(_) | Err(_) => {
            send_text(transport, "Invalid message number.\r\n", charset).await?;
            Ok(MessageIndexPromptResult::Retry)
        }
    }
}

async fn prompt_for_message_body<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    idle_timeout: Duration,
    charset: TerminalCharset,
) -> ServeResult<PromptLineResult> {
    let mut output = Vec::new();
    write_text_buffered(
        transport,
        "Enter message body. End with a single . on its own line.\r\n",
        &mut output,
        charset,
    )
    .await?;
    let mut lines = Vec::new();

    loop {
        match prompt_for_line(transport, input, idle_timeout, true, false, "> ", charset).await? {
            PromptLineResult::Value(value) if value.trim() == "." => break,
            PromptLineResult::Value(value) => lines.push(value),
            PromptLineResult::Disconnected => return Ok(PromptLineResult::Disconnected),
            PromptLineResult::IdleTimeout => return Ok(PromptLineResult::IdleTimeout),
            PromptLineResult::Rejected => {
                unreachable!("prompt_for_message_body only returns rejection for CP437 validation");
            }
        }
    }

    Ok(PromptLineResult::Value(lines.join("\r\n")))
}

fn message_author_aliases(db: &OxideDb, messages: &[Message]) -> HashMap<String, String> {
    let mut user_ids = Vec::new();
    for message in messages {
        if !user_ids
            .iter()
            .any(|user_id| user_id == &message.author_user_id)
        {
            user_ids.push(message.author_user_id.clone());
        }
    }

    match list_user_aliases_by_ids(db.db(), &user_ids) {
        Ok(aliases) => aliases.into_iter().collect(),
        Err(error) => {
            warn!("failed to load message author aliases: {error}");
            HashMap::new()
        }
    }
}

fn author_alias_from_map(author_aliases: &HashMap<String, String>, user_id: &str) -> String {
    match author_aliases.get(user_id) {
        Some(alias) if !alias.is_empty() => alias.clone(),
        _ => "Unknown".to_string(),
    }
}

fn message_record_from_message(message: &Message) -> MessageRecord {
    MessageRecord {
        id: message.id.clone(),
        area_id: message.area_id.clone(),
        author_user_id: message.author_user_id.clone(),
        author_kind: "local".to_string(),
        author_display_name: String::new(),
        author_network_address: None,
        to_user_id: message.to_user_id.clone(),
        subject: message.subject.clone(),
        body: message.body.clone(),
        created_at: message.created_at.clone(),
        reply_to_id: message.reply_to_id.clone(),
        network_message_id: message.network_message_id.clone(),
        visibility: db_visibility_from_core(&message.visibility),
    }
}

async fn prompt_for_line<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    idle_timeout: Duration,
    allow_empty: bool,
    hide_input: bool,
    prompt: &str,
    charset: TerminalCharset,
) -> ServeResult<PromptLineResult> {
    let mut output = Vec::new();
    loop {
        write_text_buffered(transport, prompt, &mut output, charset).await?;
        match read_line_input(
            transport,
            input,
            idle_timeout,
            allow_empty,
            hide_input,
            charset,
        )
        .await?
        {
            PromptLineResult::Rejected => {
                send_text(transport, CP437_INPUT_REJECT_LINE, charset).await?;
            }
            result => return Ok(result),
        }
    }
}

async fn read_line_input<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    idle_timeout: Duration,
    allow_empty: bool,
    hide_input: bool,
    charset: TerminalCharset,
) -> ServeResult<PromptLineResult> {
    let mut line = Vec::new();
    let mut output = Vec::new();

    loop {
        let event = next_event(transport, input, idle_timeout).await?;
        match event {
            CallerInput::Disconnected => return Ok(PromptLineResult::Disconnected),
            CallerInput::IdleTimeout => return Ok(PromptLineResult::IdleTimeout),
            CallerInput::Event(event) => match event {
                TelnetEvent::Data(raw) => match raw {
                    b'\0' | b'\n' if line.is_empty() => {}
                    b'\r' if line.is_empty() && !allow_empty => {}
                    b'\r' | b'\n' => {
                        write_text_buffered(transport, "\r\n", &mut output, charset).await?;
                        break;
                    }
                    b'\x08' | b'\x7f' => {
                        if line.pop().is_some() {
                            write_text_buffered(transport, "\x08 \x08", &mut output, charset)
                                .await?;
                        }
                    }
                    b'\t' => {}
                    raw => {
                        line.push(raw);
                        match raw {
                            raw if hide_input && (raw.is_ascii_graphic() || raw == b' ') => {
                                write_text_buffered(transport, "*", &mut output, charset).await?
                            }
                            raw if !hide_input && (raw.is_ascii_graphic() || raw == b' ') => {
                                write_text_buffered(
                                    transport,
                                    &String::from_utf8_lossy(&[raw]),
                                    &mut output,
                                    charset,
                                )
                                .await?
                            }
                            _ => {}
                        }
                    }
                },
                TelnetEvent::Negotiation { .. }
                | TelnetEvent::WindowSize { .. }
                | TelnetEvent::TerminalType(_)
                | TelnetEvent::TerminalTypeRequest
                | TelnetEvent::Subnegotiation { .. } => {}
            },
        }
    }

    let value = String::from_utf8_lossy(&line).to_string();
    if !hide_input && !is_cp437_compatible(&value) {
        return Ok(PromptLineResult::Rejected);
    }

    Ok(PromptLineResult::Value(value))
}

fn seed_default_message_area(db: &OxideDb) -> ServeResult<()> {
    let id = generated_uuid(db)?;
    let area = MessageAreaRecord {
        id,
        key: "general".to_string(),
        name: "General".to_string(),
        description: "Default local message area".to_string(),
        kind: "local".to_string(),
        network_id: None,
        read_security_level: 0,
        post_security_level: 10,
        moderated: false,
        enabled: true,
    };
    insert_message_area(db.db(), &area)?;
    Ok(())
}

fn user_status_to_db(status: &UserStatus) -> String {
    match status {
        UserStatus::Active => "active".to_string(),
        UserStatus::Locked => "locked".to_string(),
        UserStatus::Inactive => "disabled".to_string(),
    }
}

fn user_status_from_db(value: &str) -> ServeResult<UserStatus> {
    match value {
        "active" => Ok(UserStatus::Active),
        "locked" => Ok(UserStatus::Locked),
        "disabled" => Ok(UserStatus::Inactive),
        other => Err(ServeError::Runtime(format!(
            "unsupported user status {other}"
        ))),
    }
}

fn user_from_record(record: &oxidebbs_db::UserRecord) -> ServeResult<User> {
    let security_level = i32::try_from(record.security_level)
        .map_err(|error| ServeError::Runtime(format!("security level out of range: {error}")))?;
    Ok(User {
        id: record.id.clone(),
        alias: record.alias.clone(),
        real_name: record.real_name.clone(),
        email: record.email.clone(),
        password_hash: record.password_hash.clone(),
        security_level,
        is_sysop: record.is_sysop,
        created_at: record.created_at.clone(),
        last_login_at: record.last_login_at.clone(),
        total_calls: record.total_calls,
        time_bank_minutes: record.time_bank_minutes,
        status: user_status_from_db(&record.status)?,
    })
}

fn message_area_from_record(area: &MessageAreaRecord) -> ServeResult<MessageArea> {
    let area_kind = match area.kind.as_str() {
        "local" => AreaKind::Local,
        "echomail" => AreaKind::EchoMail,
        "netmail" => AreaKind::NetMail,
        other => {
            return Err(ServeError::Runtime(format!(
                "unsupported message area kind {other}"
            )));
        }
    };
    Ok(MessageArea {
        id: area.id.clone(),
        key: area.key.clone(),
        name: area.name.clone(),
        description: area.description.clone(),
        kind: area_kind,
        network_id: area.network_id.clone(),
        read_security_level: i32::try_from(area.read_security_level).map_err(|error| {
            ServeError::Runtime(format!("read security level out of range: {error}"))
        })?,
        post_security_level: i32::try_from(area.post_security_level).map_err(|error| {
            ServeError::Runtime(format!("post security level out of range: {error}"))
        })?,
        moderated: area.moderated,
    })
}

fn messages_from_records(records: &[MessageRecord]) -> Vec<Message> {
    records
        .iter()
        .map(|record| Message {
            id: record.id.clone(),
            area_id: record.area_id.clone(),
            author_user_id: record.author_user_id.clone(),
            to_user_id: record.to_user_id.clone(),
            subject: record.subject.clone(),
            body: record.body.clone(),
            created_at: record.created_at.clone(),
            reply_to_id: record.reply_to_id.clone(),
            network_message_id: record.network_message_id.clone(),
            visibility: message_visibility_from_db(&record.visibility),
        })
        .collect()
}

fn message_visibility_from_db(value: &str) -> MessageVisibility {
    match value {
        "deleted" => MessageVisibility::Deleted,
        "hidden" => MessageVisibility::PendingModeration,
        _ => MessageVisibility::Normal,
    }
}

fn db_visibility_from_core(visibility: &MessageVisibility) -> String {
    match visibility {
        MessageVisibility::Normal => "normal".to_string(),
        MessageVisibility::Deleted => "deleted".to_string(),
        MessageVisibility::PendingModeration => "hidden".to_string(),
    }
}

fn server_hash_password(password: &str, config: &Argon2Config) -> ServeResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = argon2_from_config(config)?
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| ServeError::Runtime(format!("password hashing failed: {error}")))?;
    Ok(password_hash.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasswordVerification {
    Accepted,
    Rejected,
    HashParseFailure,
}

fn verify_stored_password(
    password: &str,
    password_hash: &str,
    config: &Argon2Config,
) -> ServeResult<PasswordVerification> {
    let Ok(parsed_hash) = PasswordHash::new(password_hash) else {
        run_dummy_password_verify(password, config)?;
        return Ok(PasswordVerification::HashParseFailure);
    };
    if argon2_from_config(config)?
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
    {
        Ok(PasswordVerification::Accepted)
    } else {
        Ok(PasswordVerification::Rejected)
    }
}

fn run_dummy_password_verify(password: &str, config: &Argon2Config) -> ServeResult<()> {
    let parsed_hash = PasswordHash::new(DUMMY_PASSWORD_HASH)
        .map_err(|error| ServeError::Runtime(format!("dummy password hash is invalid: {error}")))?;
    let _ = argon2_from_config(config)?.verify_password(password.as_bytes(), &parsed_hash);
    Ok(())
}

fn argon2_from_config(config: &Argon2Config) -> ServeResult<Argon2<'static>> {
    let params = Params::new(
        config.memory_cost_kib,
        config.iterations,
        config.parallelism,
        None,
    )
    .map_err(|error| ServeError::Runtime(format!("invalid Argon2 parameters: {error}")))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn record_login_failure_scopes(
    db: &OxideDb,
    remote_ip: &str,
    alias_scope_key: &str,
    now: &str,
    config: &AuthConfig,
) -> ServeResult<()> {
    record_auth_failure(
        db.db(),
        "ip",
        remote_ip,
        now,
        config.failed_login_window_minutes,
        config.failed_login_lockout_minutes,
        config.failed_login_threshold,
    )?;
    record_auth_failure(
        db.db(),
        "alias",
        alias_scope_key,
        now,
        config.failed_login_window_minutes,
        config.failed_login_lockout_minutes,
        config.failed_login_threshold,
    )?;
    Ok(())
}

async fn send_login_flow<T: Transport>(
    transport: &mut T,
    config: &OxideConfig,
    login_menu: &Menu,
    capabilities: &mut TerminalCapabilities,
    context: &ScreenRenderContext,
) -> ServeResult<()> {
    send_screen(
        transport,
        config,
        &config.flow.login_screen,
        capabilities,
        context,
    )
    .await?;
    send_menu_prompt(transport, login_menu, context, capabilities.charset).await
}

async fn send_main_menu<T: Transport>(
    transport: &mut T,
    config: &OxideConfig,
    menu: &Menu,
    capabilities: &mut TerminalCapabilities,
    context: &ScreenRenderContext,
) -> ServeResult<()> {
    send_screen(transport, config, &menu.screen.asset, capabilities, context).await?;
    send_menu_prompt(transport, menu, context, capabilities.charset).await
}

async fn send_menu_prompt<T: Transport>(
    transport: &mut T,
    menu: &Menu,
    context: &ScreenRenderContext,
    charset: TerminalCharset,
) -> ServeResult<()> {
    let prompt = menu
        .description
        .clone()
        .unwrap_or_else(|| "Command? ".to_string());
    let payload = expand_screen_runtime_tokens(encode_text(&prompt, charset), context, charset);
    transport.write_all(&payload).await?;
    Ok(())
}

async fn show_post_login_screens<T: Transport>(
    transport: &mut T,
    config: &OxideConfig,
    capabilities: &mut TerminalCapabilities,
    context: &ScreenRenderContext,
) -> ServeResult<()> {
    for screen in &config.flow.post_login_screens {
        send_screen(transport, config, screen, capabilities, context).await?;
    }
    send_text(transport, MAIN_MENU_POST_LOGIN, capabilities.charset).await
}

async fn send_terminal_asset<T: Transport>(
    transport: &mut T,
    asset_name: &str,
    config: &OxideConfig,
    capabilities: TerminalCapabilities,
    context: &ScreenRenderContext,
) -> ServeResult<()> {
    let payload =
        load_terminal_asset_payload(config, asset_name, capabilities).unwrap_or_else(|error| {
            report_configured_asset_load_failure(
                "terminal asset",
                asset_name,
                capabilities,
                &error,
            );
            fallback_screen_payload(asset_name, &error, capabilities.charset)
        });
    let payload = expand_screen_runtime_tokens(payload, context, capabilities.charset);
    transport.write_all(&payload).await?;
    Ok(())
}

async fn send_logoff_screen<T: Transport>(
    transport: &mut T,
    config: &OxideConfig,
    capabilities: TerminalCapabilities,
    context: &ScreenRenderContext,
) {
    let asset_name = &config.terminal.logoff_screen;
    let payload =
        load_terminal_asset_payload(config, asset_name, capabilities).unwrap_or_else(|error| {
            warn!(
                asset = asset_name,
                supports_ansi = capabilities.supports_ansi,
                "failed to load configured logoff screen; falling back to plain goodbye: {error}"
            );
            normalize_caller_line_endings(&encode_text("Goodbye.\r\n", capabilities.charset))
        });
    let payload = expand_screen_runtime_tokens(payload, context, capabilities.charset);
    let _ = transport.write_all(&payload).await;
}

fn load_terminal_asset_payload(
    config: &OxideConfig,
    asset_name: &str,
    capabilities: TerminalCapabilities,
) -> Result<Vec<u8>, String> {
    if !capabilities.supports_ansi
        && let Some(payload) = load_plain_terminal_asset_payload(config, asset_name, capabilities)?
    {
        return Ok(normalize_caller_line_endings(&payload));
    }

    let asset_path = config.paths.ansi.join(asset_name);
    let bytes = std::fs::read(&asset_path).map_err(|error| {
        format!(
            "failed to read terminal asset {}: {error}",
            asset_path.display()
        )
    })?;

    if capabilities.supports_ansi {
        Ok(normalize_caller_line_endings(&bytes))
    } else {
        Ok(normalize_caller_line_endings(&encode_text(
            &oxidebbs_term::render_plain_text(&bytes),
            capabilities.charset,
        )))
    }
}

fn load_plain_terminal_asset_payload(
    config: &OxideConfig,
    asset_name: &str,
    capabilities: TerminalCapabilities,
) -> Result<Option<Vec<u8>>, String> {
    for candidate in plain_terminal_asset_candidates(asset_name, capabilities) {
        let asset_path = config.paths.ansi.join(&candidate);
        match std::fs::read(&asset_path) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                return Ok(Some(encode_text(&text, capabilities.charset)));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to read terminal asset {}: {error}",
                    asset_path.display()
                ));
            }
        }
    }

    Ok(None)
}

fn plain_terminal_asset_candidates(
    asset_name: &str,
    capabilities: TerminalCapabilities,
) -> Vec<String> {
    let asset_path = Path::new(asset_name);
    let mut candidates = Vec::new();
    if capabilities.width <= 40 {
        candidates.push(
            asset_path
                .with_extension("")
                .to_string_lossy()
                .trim_end_matches('.')
                .to_string()
                + "-40.asc",
        );
        candidates.push(
            asset_path
                .with_extension("")
                .to_string_lossy()
                .trim_end_matches('.')
                .to_string()
                + "-40.txt",
        );
    }
    candidates.extend([
        asset_path
            .with_extension("asc")
            .to_string_lossy()
            .into_owned(),
        asset_path
            .with_extension("txt")
            .to_string_lossy()
            .into_owned(),
    ]);
    candidates
}

async fn send_screen<T: Transport>(
    transport: &mut T,
    config: &OxideConfig,
    screen_key: &str,
    capabilities: &mut TerminalCapabilities,
    context: &ScreenRenderContext,
) -> ServeResult<()> {
    let payload = load_screen_payload(config, screen_key, *capabilities).unwrap_or_else(|error| {
        report_configured_asset_load_failure("screen", screen_key, *capabilities, &error);
        fallback_screen_payload(screen_key, &error, capabilities.charset)
    });
    let payload = expand_screen_runtime_tokens(payload, context, capabilities.charset);
    transport.write_all(&payload).await?;
    Ok(())
}

fn report_configured_asset_load_failure(
    asset_kind: &str,
    asset_name: &str,
    capabilities: TerminalCapabilities,
    error: &str,
) {
    warn!(
        asset_kind,
        asset = asset_name,
        supports_ansi = capabilities.supports_ansi,
        width = capabilities.width,
        "failed to load configured caller asset; sending fallback text: {error}"
    );
    eprintln!(
        "warning: failed to load configured {asset_kind} {asset_name:?} for terminal ansi={} width={}: {error}; sending fallback text",
        capabilities.supports_ansi, capabilities.width
    );
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
        ascii_40: screen_config.ascii_40.clone(),
        ascii: screen_config.ascii.clone(),
        text_40: screen_config.text_40.clone(),
        text: screen_config.text.clone(),
        pause: screen_config.pause,
    };

    match term_screen.load(&config.paths.screens, capabilities) {
        Ok(LoadedScreen::Ansi(bytes)) => Ok(normalize_caller_line_endings(&bytes)),
        Ok(LoadedScreen::PlainText(text)) => Ok(normalize_caller_line_endings(&encode_text(
            &text,
            capabilities.charset,
        ))),
        Err(error) => Err(error.to_string()),
    }
}

fn fallback_screen_payload(screen_key: &str, details: &str, charset: TerminalCharset) -> Vec<u8> {
    let mut message = String::new();
    let _ = writeln!(&mut message, "[{}]", screen_key);
    let _ = write!(&mut message, "{details}");
    message.push_str(PROMPT_TERMINATOR);
    normalize_caller_line_endings(&encode_text(&message, charset))
}

fn expand_screen_runtime_tokens(
    payload: Vec<u8>,
    context: &ScreenRenderContext,
    charset: TerminalCharset,
) -> Vec<u8> {
    let payload = expand_oxide_display_codes(&payload, context, charset);
    expand_legacy_screen_tokens(&payload, context)
}

fn expand_oxide_display_codes(
    payload: &[u8],
    context: &ScreenRenderContext,
    charset: TerminalCharset,
) -> Vec<u8> {
    const MAX_DISPLAY_CODE_LENGTH: usize = 48;

    let mut output = Vec::with_capacity(payload.len());
    let mut cursor = 0;
    while cursor < payload.len() {
        if payload[cursor] != b'@' {
            output.push(payload[cursor]);
            cursor += 1;
            continue;
        }

        if payload.get(cursor + 1) == Some(&b'@') {
            output.push(b'@');
            cursor += 2;
            continue;
        }

        let Some(relative_end) = payload[cursor + 1..]
            .iter()
            .take(MAX_DISPLAY_CODE_LENGTH + 1)
            .position(|byte| *byte == b'@')
        else {
            output.push(payload[cursor]);
            cursor += 1;
            continue;
        };
        let end = cursor + 1 + relative_end;
        let display_code = &payload[cursor + 1..end];

        if display_code.len() <= MAX_DISPLAY_CODE_LENGTH
            && let Some(expanded) = expand_display_code(display_code, context, charset)
        {
            output.extend_from_slice(&expanded);
            cursor = end + 1;
            continue;
        }

        output.push(payload[cursor]);
        cursor += 1;
    }

    output
}

fn expand_display_code(
    display_code: &[u8],
    context: &ScreenRenderContext,
    charset: TerminalCharset,
) -> Option<Vec<u8>> {
    if display_code.is_empty()
        || !display_code
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b':' | b'-'))
    {
        return None;
    }

    let display_code = std::str::from_utf8(display_code).ok()?;
    let (name, format) = display_code.split_once(':').unwrap_or((display_code, ""));
    let value = display_code_value(&name.to_ascii_uppercase(), context)?;
    format_display_code_value(encode_text(&value, charset), format)
}

fn display_code_value(name: &str, context: &ScreenRenderContext) -> Option<String> {
    match name {
        "NODE" | "ND" => Some(context.node_number.to_string()),
        "NODES" | "NT" => Some(context.node_count.to_string()),
        "BBS" | "BN" => Some(context.board_name.clone()),
        "SYSOP" | "SN" => Some(context.sysop_name.clone()),
        "USER" | "ALIAS" | "UH" => Some(
            context
                .caller_alias
                .clone()
                .unwrap_or_else(|| "Guest".to_string()),
        ),
        "SECURITY" | "SEC" | "SL" => Some(context.security_level.unwrap_or_default().to_string()),
        _ => None,
    }
}

fn format_display_code_value(mut value: Vec<u8>, format: &str) -> Option<Vec<u8>> {
    if format.is_empty() {
        return Some(value);
    }

    let mut format = format;
    let left_align = if let Some(stripped) = format.strip_prefix('-') {
        format = stripped;
        true
    } else {
        false
    };
    let zero_pad = if !left_align && format.len() > 1 {
        if let Some(stripped) = format.strip_prefix('0') {
            format = stripped;
            true
        } else {
            false
        }
    } else {
        false
    };
    if format.is_empty() || !format.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let width = format.parse::<usize>().ok()?;
    if width > 200 {
        return None;
    }
    if value.len() > width {
        value.truncate(width);
    }
    if value.len() < width {
        let pad_byte = if zero_pad { b'0' } else { b' ' };
        let mut padding = vec![pad_byte; width - value.len()];
        if left_align {
            value.append(&mut padding);
        } else {
            padding.extend_from_slice(&value);
            value = padding;
        }
    }
    Some(value)
}

fn expand_legacy_screen_tokens(payload: &[u8], context: &ScreenRenderContext) -> Vec<u8> {
    let node_number = context.node_number.min(999);
    let node_count = context.node_count.min(999);
    let node_status = format!("{node_number:03} / {node_count:03}");
    let node_badge = format!("NODE {node_number:03}");
    let node_of_total = format!("Node: {} of {}", context.node_number, context.node_count);

    let payload = replace_bytes_all(payload, b"NNN / TTT", node_status.as_bytes());
    let payload = replace_bytes_all(&payload, b"001 / 004", node_status.as_bytes());
    let payload = replace_bytes_all(&payload, b"NODE 001", node_badge.as_bytes());
    let payload = replace_bytes_all(&payload, b"Node: 1 of 4", node_of_total.as_bytes());
    replace_bytes_all(&payload, b"Node: N of T", node_of_total.as_bytes())
}

fn replace_bytes_all(payload: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    if needle.is_empty() {
        return payload.to_vec();
    }

    let mut next = Vec::with_capacity(payload.len());
    let mut cursor = 0;
    while cursor < payload.len() {
        if payload[cursor..].starts_with(needle) {
            next.extend_from_slice(replacement);
            cursor += needle.len();
        } else {
            next.push(payload[cursor]);
            cursor += 1;
        }
    }
    next
}

#[cfg(test)]
mod display_code_tests {
    use super::*;

    fn display_context() -> ScreenRenderContext {
        ScreenRenderContext {
            node_number: 2,
            node_count: 8,
            board_name: "Blackboard".to_string(),
            sysop_name: "CmdrTallen".to_string(),
            caller_alias: Some("Cmdr".to_string()),
            security_level: Some(10),
        }
    }

    #[test]
    fn expands_display_codes_with_width_formatting() {
        let output = expand_screen_runtime_tokens(
            b"Node @NODE:03@/@NT:03@ User @USER:-8@ Sec @SEC:03@".to_vec(),
            &display_context(),
            TerminalCharset::Cp437,
        );

        assert_eq!(output, b"Node 002/008 User Cmdr     Sec 010");
    }

    #[test]
    fn preserves_literal_at_and_unknown_tokens() {
        let output = expand_screen_runtime_tokens(
            b"Email sysop@example.com @@ @NOPE@ @BBS@".to_vec(),
            &display_context(),
            TerminalCharset::Cp437,
        );

        assert_eq!(output, b"Email sysop@example.com @ @NOPE@ Blackboard");
    }
}

fn normalize_caller_line_endings(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len());
    let mut previous_was_cr = false;

    for byte in bytes {
        if *byte == b'\n' && !previous_was_cr {
            output.push(b'\r');
        }
        output.push(*byte);
        previous_was_cr = *byte == b'\r';
    }

    output
}

async fn send_text_buffered<T: Transport>(
    transport: &mut T,
    message: &str,
    output: &mut Vec<u8>,
    charset: TerminalCharset,
) -> ServeResult<()> {
    encode_text_into(message, output, charset);
    *output = normalize_caller_line_endings(output);
    transport.write_all(output).await?;
    output.clear();
    Ok(())
}

async fn write_text_buffered<T: Transport>(
    transport: &mut T,
    message: &str,
    output: &mut Vec<u8>,
    charset: TerminalCharset,
) -> ServeResult<()> {
    send_text_buffered(transport, message, output, charset).await
}

async fn send_text<T: Transport>(
    transport: &mut T,
    message: &str,
    charset: TerminalCharset,
) -> ServeResult<()> {
    let mut output = Vec::new();
    send_text_buffered(transport, message, &mut output, charset).await?;
    Ok(())
}

async fn process_runtime_commands<T: Transport>(
    transport: &mut T,
    commands: RuntimeNodeCommands,
    disconnect_reason: &mut String,
    charset: TerminalCharset,
) -> ServeResult<bool> {
    let mut output = Vec::new();
    for message in commands.messages {
        send_text_buffered(
            transport,
            &format!("\r\n{message}\r\n"),
            &mut output,
            charset,
        )
        .await?;
    }

    if let Some(reason) = commands.disconnect_reason {
        *disconnect_reason = reason;
        send_text_buffered(
            transport,
            "\r\nDisconnected by sysop.\r\n",
            &mut output,
            charset,
        )
        .await?;
        return Ok(true);
    }

    Ok(false)
}

fn encode_text(text: &str, charset: TerminalCharset) -> Vec<u8> {
    let mut output = Vec::new();
    encode_text_into(text, &mut output, charset);
    output
}

fn encode_text_into(text: &str, output: &mut Vec<u8>, charset: TerminalCharset) {
    output.clear();
    if let TerminalCharset::Petscii = charset {
        output.extend_from_slice(&render_petscii_lossy(text));
        return;
    }

    if text.is_ascii() {
        output.reserve(text.len());
        output.extend_from_slice(text.as_bytes());
        return;
    }

    match encode_cp437(text) {
        Ok(bytes) => output.extend_from_slice(&bytes),
        Err(_) => encode_text_lossy_into(text, output, charset),
    }
}

fn encode_text_lossy_into(text: &str, output: &mut Vec<u8>, charset: TerminalCharset) {
    output.clear();
    output.reserve(text.len());
    for character in text.chars() {
        if let TerminalCharset::Petscii = charset {
            output.push(char_to_petscii_byte(character).unwrap_or(b'?'));
            continue;
        }
        let mut buffer = [0_u8; 4];
        let encoded = character.encode_utf8(&mut buffer);
        match encode_cp437(encoded) {
            Ok(encoded_bytes) => output.extend_from_slice(&encoded_bytes),
            Err(_) => output.push(b'?'),
        }
    }
}

fn is_cp437_compatible(text: &str) -> bool {
    if text.is_ascii() {
        return true;
    }
    encode_cp437(text).is_ok()
}

fn validate_caller_cp437_text(text: &str) -> Result<(), &'static str> {
    if is_cp437_compatible(text) {
        Ok(())
    } else {
        Err(CP437_INPUT_REJECT_MESSAGE)
    }
}

fn normalize_key(byte: u8) -> Option<String> {
    let ch = byte as char;
    if !ch.is_ascii() || ch.is_ascii_control() {
        return None;
    }
    Some(ch.to_ascii_uppercase().to_string())
}

async fn drain_line_ending_after_menu_key<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
) -> ServeResult<()> {
    let mut reply = Vec::new();

    for _ in 0..2 {
        let immediate = timeout(Duration::ZERO, transport.read_byte()).await;
        let byte = match immediate {
            Ok(Ok(Some(byte))) => byte,
            Ok(Ok(None)) => {
                input.pending_inputs.push_back(CallerInput::Disconnected);
                break;
            }
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => break,
        };

        match parse_next_event(input, &mut reply, byte) {
            Some(TelnetEvent::Data(b'\0' | b'\r' | b'\n')) => {}
            Some(event) => {
                input.pending_inputs.push_front(CallerInput::Event(event));
                break;
            }
            None => {}
        }
    }

    Ok(())
}

async fn next_event<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    idle_timeout: Duration,
) -> ServeResult<CallerInput> {
    if !input.pending_replies.is_empty() {
        flush_pending_replies(transport, input).await?;
    }

    if let Some(pending) = input.pending_inputs.pop_front() {
        return Ok(pending);
    }

    let mut reply = Vec::new();
    loop {
        let read = timeout(idle_timeout, transport.read_byte()).await;
        let byte = match read {
            Ok(Ok(Some(byte))) => byte,
            Ok(Ok(None)) => {
                return Ok(CallerInput::Disconnected);
            }
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => return Ok(CallerInput::IdleTimeout),
        };

        if let Some(event) = parse_next_event(input, &mut reply, byte) {
            input.pending_inputs.push_back(CallerInput::Event(event));
        }

        loop {
            let immediate = timeout(Duration::ZERO, transport.read_byte()).await;
            let immediate_byte = match immediate {
                Ok(Ok(Some(byte))) => byte,
                Ok(Ok(None)) => {
                    input.pending_inputs.push_back(CallerInput::Disconnected);
                    break;
                }
                Ok(Err(error)) => return Err(error.into()),
                Err(_) => break,
            };

            if let Some(event) = parse_next_event(input, &mut reply, immediate_byte) {
                input.pending_inputs.push_back(CallerInput::Event(event));
            }
        }

        if let Some(front) = input.pending_inputs.pop_front() {
            flush_pending_replies(transport, input).await?;
            return Ok(front);
        }
    }
}

async fn drain_zmodem_finish_sequence<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
) -> ServeResult<()> {
    let mut bytes = Vec::with_capacity(2);
    for _ in 0..2 {
        match timeout(Duration::from_millis(100), transport.read_byte()).await {
            Ok(Ok(Some(byte))) => bytes.push(byte),
            Ok(Ok(None)) => {
                input.pending_inputs.push_back(CallerInput::Disconnected);
                break;
            }
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => break,
        }
    }

    if bytes == b"OO" {
        return Ok(());
    }

    for byte in bytes {
        queue_input_byte(input, byte);
    }
    Ok(())
}

fn queue_input_byte(input: &mut InputSession, byte: u8) {
    let mut reply = Vec::new();
    if let Some(event) = parse_next_event(input, &mut reply, byte) {
        input.pending_inputs.push_back(CallerInput::Event(event));
    }
}

fn parse_next_event(
    input: &mut InputSession,
    reply: &mut Vec<u8>,
    byte: u8,
) -> Option<TelnetEvent> {
    if input.raw {
        return Some(TelnetEvent::Data(byte));
    }

    if !reply.is_empty() {
        reply.clear();
    }

    let event = input.parser.feed(byte, reply);
    if !reply.is_empty() {
        input.pending_replies.extend_from_slice(reply);
    }

    event
}

async fn flush_pending_replies<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
) -> ServeResult<()> {
    if !input.pending_replies.is_empty() {
        transport.write_all(&input.pending_replies).await?;
        input.pending_replies.clear();
    }
    Ok(())
}

#[derive(Debug)]
enum CallerInput {
    Event(TelnetEvent),
    Disconnected,
    IdleTimeout,
}

enum CallerWait {
    Input(ServeResult<CallerInput>),
    Runtime(RuntimeNodeCommands),
}

#[derive(Default)]
struct InputSession {
    parser: TelnetParser,
    pending_inputs: VecDeque<CallerInput>,
    pending_replies: Vec<u8>,
    raw: bool,
}

impl InputSession {
    fn raw() -> Self {
        Self {
            raw: true,
            ..Self::default()
        }
    }
}

async fn send_menu_help<T: Transport>(
    transport: &mut T,
    config: &crate::config::OxideConfig,
    menu: &Menu,
    capabilities: &mut oxidebbs_term::TerminalCapabilities,
    user_security_level: Option<i32>,
    context: &ScreenRenderContext,
) -> ServeResult<()> {
    if let Some(help_screen) = &menu.help_screen {
        send_screen(transport, config, &help_screen.asset, capabilities, context).await?;
        return Ok(());
    }

    let mut help = String::new();
    let title = if menu.title.trim().is_empty() {
        menu.id.as_str()
    } else {
        menu.title.as_str()
    };
    help.push_str("\r\n");
    help.push_str(&title.to_ascii_uppercase());
    help.push_str(" HELP\r\n\r\n");

    for entry in menu
        .entries
        .iter()
        .filter(|entry| entry.key.trim() != "?")
        .filter(|entry| menu_entry_visible_to_security_level(entry, user_security_level))
    {
        help.push_str(&format!("[{}] {}\r\n", entry.key, entry.label));
    }

    help.push_str("[?] Help\r\n");
    if menu.route_entry("R").is_none() {
        help.push_str("[R] Redisplay screen\r\n");
    }
    help.push_str("\r\n");

    send_text(transport, &help, capabilities.charset).await
}

fn menu_entry_visible_to_security_level(
    entry: &oxidebbs_core::menu::MenuEntry,
    user_security_level: Option<i32>,
) -> bool {
    match user_security_level {
        Some(level) => level >= entry.min_security_level,
        None => entry.min_security_level <= 0,
    }
}

fn resolve_submenu(menus: &HashMap<String, Arc<Menu>>, menu_id: &str) -> Option<Arc<Menu>> {
    menus.get(menu_id).cloned()
}

fn generated_uuid(db: &OxideDb) -> ServeResult<String> {
    db_scalar_text(db, "SELECT UUID_TO_STRING(GEN_RANDOM_UUID())")
}

fn current_timestamp(db: &OxideDb) -> ServeResult<String> {
    db_scalar_text(db, "SELECT CAST(NOW() AS TEXT)")
}

fn validate_startup_database_health(db: &OxideDb) -> ServeResult<()> {
    db.schema_version()
        .map_err(|error| startup_database_check_error("system_config schema_version", error))?;
    list_users(db.db()).map_err(|error| startup_database_check_error("users", error))?;
    list_auth_attempts(db.db())
        .map_err(|error| startup_database_check_error("auth_attempts", error))?;
    list_message_areas(db.db())
        .map_err(|error| startup_database_check_error("message_areas", error))?;
    list_messages(db.db()).map_err(|error| startup_database_check_error("messages", error))?;
    list_recent_sessions(db.db(), 1)
        .map_err(|error| startup_database_check_error("sessions", error))?;
    list_door_definitions(db.db()).map_err(|error| startup_database_check_error("doors", error))?;
    list_door_runs(db.db(), 1).map_err(|error| startup_database_check_error("door_runs", error))?;
    list_audit_events(db.db(), 1)
        .map_err(|error| startup_database_check_error("audit_events", error))?;
    Ok(())
}

fn startup_database_check_error(table: &str, error: oxidebbs_db::DbError) -> ServeError {
    ServeError::Runtime(format!(
        "startup database health check failed while reading {table}: {error}. Refusing to start; run `oxidebbs-server db doctor` and repair or restore the DecentDB data files before serving callers"
    ))
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

fn insert_required_startup_audit_event(
    db: &OxideDb,
    event_type: &str,
    user_id: Option<String>,
    node_number: Option<i64>,
    details: String,
) -> ServeResult<()> {
    debug!(
        event_type,
        user_id = ?user_id,
        node_number = ?node_number,
        details = %details,
        "required startup audit event"
    );
    insert_audit_event(
        db.db(),
        &AuditEventRecord {
            id: String::new(),
            created_at: String::new(),
            event_type: event_type.to_string(),
            user_id,
            node_number,
            details,
        },
    )
    .map_err(|error| {
        ServeError::Runtime(format!(
            "required startup audit event {event_type:?} could not be written: {error}. Refusing to start because audit storage is not writable"
        ))
    })?;
    Ok(())
}

fn emit_audit_event_with_runtime(
    db: &OxideDb,
    event_type: &str,
    user_id: Option<String>,
    node_number: Option<i64>,
    details: String,
    runtime: Option<&ServerRuntime>,
) {
    debug!(
        event_type,
        user_id = ?user_id,
        node_number = ?node_number,
        details = %details,
        "audit event"
    );
    if let Err(error) = insert_audit_event(
        db.db(),
        &AuditEventRecord {
            id: String::new(),
            created_at: String::new(),
            event_type: event_type.to_string(),
            user_id,
            node_number,
            details,
        },
    ) {
        warn!("failed to insert {event_type} audit event: {error}");
        if let Some(runtime) = runtime {
            runtime.record_audit_write_failure();
        }
    }
}

fn emit_db_write_failed_event(
    db: &OxideDb,
    node_number: Option<i64>,
    user_id: Option<String>,
    operation: &str,
    error: &dyn std::fmt::Display,
    context: &str,
    runtime: Option<&ServerRuntime>,
) {
    emit_audit_event_with_runtime(
        db,
        "db_write_failed",
        user_id,
        node_number,
        format!("{context} during {operation}: {error}"),
        runtime,
    );
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() -> ServeResult<()> {
    let mut terminate = signal(SignalKind::terminate()).map_err(|error| {
        ServeError::Runtime(format!("failed to register SIGTERM handler: {error}"))
    })?;

    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result
                .map_err(|error| ServeError::Runtime(format!("failed to wait for ctrl-c signal: {error}")))?;
            Ok(())
        }
        _ = terminate.recv() => Ok(())
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() -> ServeResult<()> {
    tokio::signal::ctrl_c().await.map_err(|error| {
        ServeError::Runtime(format!("failed to wait for ctrl-c signal: {error}"))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::future::Future;
    use std::net::SocketAddr;
    use std::path::{Path, PathBuf};
    use std::pin::Pin;
    use std::time::{Duration as TestDuration, Instant, SystemTime, UNIX_EPOCH};

    use oxidebbs_db::insert_user;
    use oxidebbs_telnet::{
        SerialHandle, SerialLoopback,
        telnet::{
            DO, IAC, SB, SE, TELOPT_ECHO, TELOPT_NAWS, TELOPT_SUPPRESS_GO_AHEAD,
            TELOPT_TERMINAL_TYPE, TELOPT_TTYPE_IS, WILL,
        },
        transport::{LoopbackHandle, LoopbackTransport},
    };
    use oxidebbs_transfer::{ByteTransport, TransferError, TransferRead};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::oneshot;

    struct LoopbackClientBytes {
        handle: LoopbackHandle,
    }

    struct SerialClientBytes {
        handle: SerialHandle,
    }

    fn test_password() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
            .to_string()
    }

    fn test_password_with_emoji() -> String {
        let mut password = test_password();
        password.push(' ');
        password.push('🚀');
        password
    }

    fn mismatched_password(password: &str) -> String {
        let mut value = password.to_string();
        value.push('x');
        value
    }

    fn line_input(value: &str) -> Vec<u8> {
        format!("{value}\r").into_bytes()
    }

    impl ByteTransport for LoopbackClientBytes {
        fn read_byte(
            &mut self,
            timeout_secs: u64,
        ) -> Pin<Box<dyn Future<Output = Result<TransferRead, TransferError>> + Send + '_>>
        {
            Box::pin(async move {
                match timeout(Duration::from_secs(timeout_secs), self.handle.read_byte()).await {
                    Ok(Some(byte)) => Ok(TransferRead::Byte(byte)),
                    Ok(None) => Ok(TransferRead::Closed),
                    Err(_) => Ok(TransferRead::TimedOut),
                }
            })
        }

        fn write_all<'a>(
            &'a mut self,
            buf: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + 'a>> {
            Box::pin(async move {
                self.handle
                    .write_bytes(buf)
                    .map_err(|_| TransferError::Transport)
            })
        }

        fn flush(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }
    }

    impl ByteTransport for SerialClientBytes {
        fn read_byte(
            &mut self,
            timeout_secs: u64,
        ) -> Pin<Box<dyn Future<Output = Result<TransferRead, TransferError>> + Send + '_>>
        {
            Box::pin(async move {
                match timeout(Duration::from_secs(timeout_secs), self.handle.read_byte()).await {
                    Ok(Some(byte)) => Ok(TransferRead::Byte(byte)),
                    Ok(None) => Ok(TransferRead::Closed),
                    Err(_) => Ok(TransferRead::TimedOut),
                }
            })
        }

        fn write_all<'a>(
            &'a mut self,
            buf: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + 'a>> {
            Box::pin(async move {
                self.handle
                    .write_bytes(buf)
                    .map_err(|_| TransferError::Transport)
            })
        }

        fn flush(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = Result<(), TransferError>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }
    }

    #[tokio::test]
    async fn xmodem_crc_transfer_over_serial_loopback_round_trips() {
        let (serial, handle) = SerialLoopback::new();
        let server = tokio::spawn(async move {
            let mut adapter = TransportAdapter::new_raw(serial);
            oxidebbs_transfer::xmodem::send_xmodem_crc(&mut adapter, b"serial-xmodem")
                .await
                .expect("send xmodem over serial");
        });
        let client = tokio::spawn(async move {
            let mut adapter = SerialClientBytes { handle };
            oxidebbs_transfer::xmodem::receive_xmodem_crc_with_size(&mut adapter, 13)
                .await
                .expect("receive xmodem over serial")
        });

        server.await.expect("server task");
        let payload = client.await.expect("client task");
        assert_eq!(payload, b"serial-xmodem");
    }

    #[tokio::test]
    async fn zmodem_transfer_over_serial_loopback_round_trips() {
        let (serial, handle) = SerialLoopback::new();
        let server = tokio::spawn(async move {
            let mut adapter = TransportAdapter::new_raw(serial);
            oxidebbs_transfer::zmodem::send_zmodem_file(
                &mut adapter,
                "serial-zmodem.bin",
                b"serial-zmodem",
            )
            .await
            .expect("send zmodem over serial");
        });
        let client = tokio::spawn(async move {
            let mut adapter = SerialClientBytes { handle };
            oxidebbs_transfer::zmodem::receive_zmodem_file(&mut adapter, Some(1024))
                .await
                .expect("receive zmodem over serial")
        });

        server.await.expect("server task");
        let file = client.await.expect("client task");
        assert_eq!(file.filename, "serial-zmodem.bin");
        assert_eq!(file.payload, b"serial-zmodem");
    }

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
    fn terminal_type_supports_only_explicit_ansi_clients() {
        assert!(terminal_type_capabilities(b"SyncTERM").supports_ansi);
        assert!(terminal_type_capabilities(b"ANSI").supports_ansi);
        assert!(terminal_type_capabilities(b"ANSI-BBS").supports_ansi);
        assert!(terminal_type_capabilities(b"BBS-ANSI").supports_ansi);
        assert!(terminal_type_capabilities(b"ANSI.SYS").supports_ansi);
        assert!(terminal_type_capabilities(b"PC-ANSI").supports_ansi);
        assert!(terminal_type_capabilities(b"pcansi").supports_ansi);
        assert!(!terminal_type_capabilities(b"xterm-256color").supports_ansi);
        assert!(!terminal_type_capabilities(b"vt100").supports_ansi);
        assert!(!terminal_type_capabilities(b"C64 Ultimate").supports_ansi);
    }

    #[test]
    fn terminal_type_detects_c64_profile_without_ansi() {
        for terminal_type in [b"C64".as_slice(), b"C64 Ultimate", b"PETSCII", b"CGTerm"] {
            let capabilities = terminal_type_capabilities(terminal_type);

            assert_eq!(capabilities.profile, TerminalProfile::C64);
            assert_eq!(capabilities.width, 40);
            assert_eq!(capabilities.height, 25);
            assert!(!capabilities.supports_ansi);
            assert!(!capabilities.supports_color);
        }
    }

    #[tokio::test]
    async fn capability_negotiation_defaults_to_plain_text_without_response() {
        let (mut transport, mut client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        let capabilities = negotiate_terminal_capabilities(
            &mut transport,
            &mut input,
            Duration::from_millis(5),
            TerminalCapabilities::plain_text(),
        )
        .await
        .expect("negotiate capabilities");
        let request = client.read_output_bytes();

        assert_eq!(
            request,
            [
                IAC,
                WILL,
                TELOPT_ECHO,
                IAC,
                WILL,
                TELOPT_SUPPRESS_GO_AHEAD,
                IAC,
                DO,
                TELOPT_SUPPRESS_GO_AHEAD,
                IAC,
                DO,
                TELOPT_TERMINAL_TYPE,
                IAC,
                DO,
                TELOPT_NAWS
            ]
        );
        assert_eq!(capabilities, TerminalCapabilities::plain_text());
    }

    #[tokio::test]
    async fn capability_negotiation_detects_syncterm_and_naws_width() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();
        client
            .write_bytes(&[
                IAC,
                WILL,
                TELOPT_TERMINAL_TYPE,
                IAC,
                WILL,
                TELOPT_NAWS,
                IAC,
                SB,
                TELOPT_TERMINAL_TYPE,
                TELOPT_TTYPE_IS,
                b'S',
                b'y',
                b'n',
                b'c',
                b'T',
                b'E',
                b'R',
                b'M',
                IAC,
                SE,
                IAC,
                SB,
                TELOPT_NAWS,
                0,
                40,
                0,
                24,
                IAC,
                SE,
            ])
            .expect("write negotiation frames");

        let capabilities = negotiate_terminal_capabilities(
            &mut transport,
            &mut input,
            Duration::from_millis(20),
            TerminalCapabilities::plain_text(),
        )
        .await
        .expect("negotiate capabilities");

        assert!(capabilities.supports_ansi);
        assert_eq!(capabilities.width, 40);
    }

    #[tokio::test]
    async fn capability_negotiation_selects_40_column_ansi_screen() {
        let base_dir = temp_dir("ansi-40-screen");
        let db_path = base_dir.join("oxidebbs.ddb");
        let bind_addr = free_loopback_addr();
        let mut config = smoke_config(bind_addr, &base_dir, &db_path);
        write_login_screen_variants(&mut config);

        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();
        client
            .write_bytes(&[
                IAC,
                WILL,
                TELOPT_TERMINAL_TYPE,
                IAC,
                WILL,
                TELOPT_NAWS,
                IAC,
                SB,
                TELOPT_TERMINAL_TYPE,
                TELOPT_TTYPE_IS,
                b'S',
                b'y',
                b'n',
                b'c',
                b'T',
                b'E',
                b'R',
                b'M',
                IAC,
                SE,
                IAC,
                SB,
                TELOPT_NAWS,
                0,
                40,
                0,
                24,
                IAC,
                SE,
            ])
            .expect("write negotiation frames");

        let capabilities = negotiate_terminal_capabilities(
            &mut transport,
            &mut input,
            Duration::from_millis(20),
            TerminalCapabilities::plain_text(),
        )
        .await
        .expect("negotiate capabilities");
        let payload = load_screen_payload(&config, &config.flow.login_screen, capabilities)
            .expect("load login screen");

        assert_eq!(payload, b"ANSI40\r\n");

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn capability_negotiation_selects_plain_screen_for_plain_40_column_client() {
        let base_dir = temp_dir("plain-40-screen");
        let db_path = base_dir.join("oxidebbs.ddb");
        let bind_addr = free_loopback_addr();
        let mut config = smoke_config(bind_addr, &base_dir, &db_path);
        write_login_screen_variants(&mut config);

        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();
        client
            .write_bytes(&[
                IAC,
                WILL,
                TELOPT_NAWS,
                IAC,
                SB,
                TELOPT_NAWS,
                0,
                40,
                0,
                24,
                IAC,
                SE,
            ])
            .expect("write negotiation frames");

        let capabilities = negotiate_terminal_capabilities(
            &mut transport,
            &mut input,
            Duration::from_millis(20),
            TerminalCapabilities::plain_text(),
        )
        .await
        .expect("negotiate capabilities");
        let payload = load_screen_payload(&config, &config.flow.login_screen, capabilities)
            .expect("load login screen");

        assert!(!capabilities.supports_ansi);
        assert_eq!(capabilities.width, 40);
        assert_eq!(payload, b"ASCII40\r\n");

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn capability_negotiation_detects_c64_terminal_type() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();
        client
            .write_bytes(&[
                IAC,
                WILL,
                TELOPT_TERMINAL_TYPE,
                IAC,
                SB,
                TELOPT_TERMINAL_TYPE,
                TELOPT_TTYPE_IS,
                b'C',
                b'6',
                b'4',
                b' ',
                b'U',
                b'l',
                b't',
                b'i',
                b'm',
                b'a',
                b't',
                b'e',
                IAC,
                SE,
            ])
            .expect("write C64 terminal type");

        let capabilities = negotiate_terminal_capabilities(
            &mut transport,
            &mut input,
            Duration::from_millis(20),
            TerminalCapabilities::plain_text(),
        )
        .await
        .expect("negotiate capabilities");

        assert_eq!(capabilities.profile, TerminalProfile::C64);
        assert_eq!(capabilities.width, 40);
        assert!(!capabilities.supports_ansi);
    }

    #[tokio::test]
    async fn capability_negotiation_can_default_to_c64_profile() {
        let (mut transport, _client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        let capabilities = negotiate_terminal_capabilities(
            &mut transport,
            &mut input,
            Duration::from_millis(5),
            TerminalCapabilities::c64(),
        )
        .await
        .expect("negotiate capabilities");

        assert_eq!(capabilities.profile, TerminalProfile::C64);
        assert_eq!(capabilities.width, 40);
        assert!(!capabilities.supports_ansi);
    }

    #[tokio::test]
    async fn capability_negotiation_completes_before_timeout() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();
        client
            .write_bytes(&[
                IAC,
                WILL,
                TELOPT_TERMINAL_TYPE,
                IAC,
                WILL,
                TELOPT_NAWS,
                IAC,
                SB,
                TELOPT_TERMINAL_TYPE,
                TELOPT_TTYPE_IS,
                b'S',
                b'y',
                b'n',
                b'c',
                b'T',
                b'E',
                b'R',
                b'M',
                IAC,
                SE,
                IAC,
                SB,
                TELOPT_NAWS,
                0,
                80,
                0,
                24,
                IAC,
                SE,
            ])
            .expect("write negotiation frames");

        let start = Instant::now();
        let capabilities = negotiate_terminal_capabilities(
            &mut transport,
            &mut input,
            TestDuration::from_millis(120),
            TerminalCapabilities::plain_text(),
        )
        .await
        .expect("negotiate capabilities");
        let elapsed = start.elapsed();

        assert!(capabilities.supports_ansi);
        assert_eq!(capabilities.width, 80);
        assert!(elapsed < TestDuration::from_millis(90));
    }

    #[tokio::test]
    async fn capability_negotiation_ends_on_real_data_without_losing_data() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();
        client
            .write_bytes(&[IAC, WILL, TELOPT_TERMINAL_TYPE, b'X'])
            .expect("write partial negotiation and data");

        let capabilities = negotiate_terminal_capabilities(
            &mut transport,
            &mut input,
            TestDuration::from_millis(60),
            TerminalCapabilities::plain_text(),
        )
        .await
        .expect("negotiate capabilities");
        assert_eq!(capabilities, TerminalCapabilities::plain_text());

        let event = next_event(&mut transport, &mut input, TestDuration::from_millis(1))
            .await
            .expect("read preserved data");

        match event {
            CallerInput::Event(TelnetEvent::Data(b'X')) => {}
            other => panic!("expected preserved data event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn next_event_flushes_multiple_negotiation_replies_without_blocking() {
        let (mut transport, mut client) = LoopbackTransport::new();
        let mut input = InputSession::default();
        client
            .write_bytes(&[IAC, WILL, TELOPT_ECHO, IAC, WILL, TELOPT_SUPPRESS_GO_AHEAD])
            .expect("write two negotiation frames");

        let event = next_event(&mut transport, &mut input, TestDuration::from_millis(100))
            .await
            .expect("read event");

        match event {
            CallerInput::Event(TelnetEvent::Negotiation {
                command: _,
                option,
                accepted: true,
            }) => assert_eq!(option, TELOPT_ECHO),
            other => panic!("expected negotiation event, got {other:?}"),
        }

        let output = client.read_output_bytes();
        assert_eq!(
            output,
            vec![IAC, DO, TELOPT_ECHO, IAC, DO, TELOPT_SUPPRESS_GO_AHEAD]
        );
    }

    #[test]
    fn resolve_submenu_menu_prefers_configured_menu_entry() {
        let mut menus = HashMap::new();
        let submenu = Arc::new(Menu {
            id: "submenu".to_string(),
            title: "Submenu".to_string(),
            description: None,
            screen: oxidebbs_core::menu::ScreenAsset {
                asset: "submenu".to_string(),
            },
            help_screen: None,
            entries: Vec::new(),
            pre_menu_screens: Vec::new(),
        });
        menus.insert("submenu".to_string(), Arc::clone(&submenu));

        let selected =
            resolve_submenu(&menus, "submenu").expect("configured submenu should resolve");

        assert!(std::sync::Arc::ptr_eq(&selected, &submenu));
    }

    #[test]
    fn resolve_submenu_menu_missing_menu_is_none() {
        let menus: HashMap<String, Arc<Menu>> = HashMap::new();

        assert!(resolve_submenu(&menus, "missing").is_none());
    }

    #[test]
    fn fallback_payload_includes_context() {
        let payload = fallback_screen_payload("login", "missing file", TerminalCharset::Cp437);
        let decoded = String::from_utf8_lossy(&payload);

        assert!(decoded.contains("[login]"));
        assert!(decoded.contains("missing file"));
    }

    #[test]
    fn runtime_counter_increments_when_audit_insert_fails() {
        let db = OxideDb::open_memory().expect("open db");
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 60);

        emit_audit_event_with_runtime(
            &db,
            "forced_audit_failure",
            Some("not-a-uuid".to_string()),
            Some(1),
            "forced failure".to_string(),
            Some(&runtime),
        );

        assert_eq!(runtime.audit_write_failures(), 1);
    }

    #[test]
    fn startup_database_health_check_fails_when_audit_events_are_unreadable() {
        let db = OxideDb::open_memory().expect("open db");
        db.db()
            .execute_batch("DROP TABLE audit_events")
            .expect("drop audit_events");

        let error =
            validate_startup_database_health(&db).expect_err("missing audit_events should fail");
        let message = error.to_string();

        assert!(message.contains("startup database health check failed"));
        assert!(message.contains("audit_events"));
        assert!(message.contains("Refusing to start"));
    }

    #[test]
    fn required_startup_audit_event_fails_loudly_when_write_fails() {
        let db = OxideDb::open_memory().expect("open db");

        let error = insert_required_startup_audit_event(
            &db,
            "server_start",
            Some("not-a-uuid".to_string()),
            Some(1),
            "forced failure".to_string(),
        )
        .expect_err("invalid startup audit user id should fail");
        let message = error.to_string();

        assert!(message.contains("required startup audit event"));
        assert!(message.contains("server_start"));
        assert!(message.contains("Refusing to start"));
    }

    #[test]
    fn launch_error_summary_hides_verbose_host_details_from_caller() {
        let summary = DoorExecutionSummary {
            door_name: "Test Door".to_string(),
            run_id: Some("run-1".to_string()),
            exit_code: None,
            timed_out: false,
            disconnect_forced: false,
            caller_disconnected: false,
            disconnect_reason: None,
            early_exit_before_com1: false,
            bytes_in: 0,
            bytes_out: 0,
            launch_error: Some(
                "door runner validation failed before launch: /var/lib/oxidebbs/doors".to_string(),
            ),
            stdout_log: None,
            stderr_log: None,
        };

        let text = door_summary_text(&summary);

        assert!(text.contains("Unable to launch Test Door. Contact the sysop."));
        assert!(text.contains("Run id: run-1"));
        assert!(!text.contains("/var/lib/oxidebbs"));
        assert!(!text.contains("validation failed"));
    }

    #[test]
    fn door_access_gate_uses_door_min_security_level() {
        assert!(door_access_denied(10, 50));
        assert!(!door_access_denied(50, 50));
        assert!(!door_access_denied(255, 50));
    }

    #[test]
    fn terminal_asset_payload_loads_from_ansi_path() {
        let base_dir = temp_dir("terminal-asset");
        let db_path = base_dir.join("oxidebbs.ddb");
        let bind_addr = free_loopback_addr();
        let config = smoke_config(bind_addr, &base_dir, &db_path);
        std::fs::create_dir_all(&config.paths.ansi).expect("create ANSI dir");
        std::fs::write(config.paths.ansi.join("welcome.ans"), b"\x1b[1mWelcome\r\n")
            .expect("write welcome asset");
        std::fs::write(config.paths.ansi.join("welcome.asc"), b"ASCII welcome\r\n")
            .expect("write plain welcome asset");

        let ansi_payload =
            load_terminal_asset_payload(&config, "welcome.ans", TerminalCapabilities::ansi_80())
                .expect("load ANSI welcome");
        assert_eq!(ansi_payload, b"\x1b[1mWelcome\r\n");

        let plain_payload =
            load_terminal_asset_payload(&config, "welcome.ans", TerminalCapabilities::plain_text())
                .expect("load plain welcome");
        assert_eq!(plain_payload, b"ASCII welcome\r\n");

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[test]
    fn terminal_asset_payload_falls_back_to_stripped_ansi_when_plain_asset_is_missing() {
        let base_dir = temp_dir("terminal-asset-ansi-fallback");
        let db_path = base_dir.join("oxidebbs.ddb");
        let bind_addr = free_loopback_addr();
        let config = smoke_config(bind_addr, &base_dir, &db_path);
        std::fs::create_dir_all(&config.paths.ansi).expect("create ANSI dir");
        std::fs::write(config.paths.ansi.join("welcome.ans"), b"\x1b[1mWelcome\r\n")
            .expect("write welcome asset");

        let plain_payload =
            load_terminal_asset_payload(&config, "welcome.ans", TerminalCapabilities::plain_text())
                .expect("load plain welcome");
        assert_eq!(plain_payload, b"Welcome\r\n");

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[test]
    fn c64_terminal_asset_payload_prefers_40_column_plain_asset() {
        let base_dir = temp_dir("terminal-asset-c64");
        let db_path = base_dir.join("oxidebbs.ddb");
        let bind_addr = free_loopback_addr();
        let config = smoke_config(bind_addr, &base_dir, &db_path);
        std::fs::create_dir_all(&config.paths.ansi).expect("create ANSI dir");
        std::fs::write(
            config.paths.ansi.join("welcome.ans"),
            b"\x1b[1mANSI welcome\r\n",
        )
        .expect("write ANSI welcome");
        std::fs::write(
            config.paths.ansi.join("welcome.asc"),
            b"Wide plain welcome\r\n",
        )
        .expect("write wide plain welcome");
        std::fs::write(config.paths.ansi.join("welcome-40.asc"), b"C64 welcome\r\n")
            .expect("write C64 welcome");

        let payload =
            load_terminal_asset_payload(&config, "welcome.ans", TerminalCapabilities::c64())
                .expect("load C64 welcome");

        assert_eq!(payload, b"C64 welcome\r\n");
        assert!(!payload.windows(2).any(|window| window == b"\x1b["));

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[test]
    fn terminal_asset_payload_normalizes_bare_lf_for_telnet_callers() {
        let base_dir = temp_dir("terminal-asset-line-endings");
        let db_path = base_dir.join("oxidebbs.ddb");
        let bind_addr = free_loopback_addr();
        let config = smoke_config(bind_addr, &base_dir, &db_path);
        std::fs::create_dir_all(&config.paths.ansi).expect("create ANSI dir");
        std::fs::write(
            config.paths.ansi.join("welcome.ans"),
            b"\x1b[1mWelcome\nNext\n",
        )
        .expect("write welcome asset");
        std::fs::write(
            config.paths.ansi.join("welcome.asc"),
            b"ASCII welcome\nNext\n",
        )
        .expect("write plain welcome asset");

        let ansi_payload =
            load_terminal_asset_payload(&config, "welcome.ans", TerminalCapabilities::ansi_80())
                .expect("load ANSI welcome");
        assert_eq!(ansi_payload, b"\x1b[1mWelcome\r\nNext\r\n");

        let plain_payload =
            load_terminal_asset_payload(&config, "welcome.ans", TerminalCapabilities::plain_text())
                .expect("load plain welcome");
        assert_eq!(plain_payload, b"ASCII welcome\r\nNext\r\n");

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[test]
    fn logoff_terminal_asset_payload_selects_ansi_or_plain_sibling() {
        let base_dir = temp_dir("logoff-terminal-asset");
        let db_path = base_dir.join("oxidebbs.ddb");
        let bind_addr = free_loopback_addr();
        let config = smoke_config(bind_addr, &base_dir, &db_path);
        std::fs::create_dir_all(&config.paths.ansi).expect("create ANSI dir");
        std::fs::write(config.paths.ansi.join("logoff.ans"), b"\x1b[1mANSI bye\r\n")
            .expect("write ANSI logoff");
        std::fs::write(config.paths.ansi.join("logoff.asc"), b"Plain bye\r\n")
            .expect("write plain logoff");

        let ansi_payload =
            load_terminal_asset_payload(&config, "logoff.ans", TerminalCapabilities::ansi_80())
                .expect("load ANSI logoff");
        assert_eq!(ansi_payload, b"\x1b[1mANSI bye\r\n");

        let plain_payload =
            load_terminal_asset_payload(&config, "logoff.ans", TerminalCapabilities::plain_text())
                .expect("load plain logoff");
        assert_eq!(plain_payload, b"Plain bye\r\n");

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn logoff_screen_falls_back_to_goodbye_when_asset_is_missing() {
        let base_dir = temp_dir("logoff-terminal-missing");
        let db_path = base_dir.join("oxidebbs.ddb");
        let bind_addr = free_loopback_addr();
        let config = smoke_config(bind_addr, &base_dir, &db_path);

        let output = capture_logoff_output(config, TerminalCapabilities::plain_text()).await;

        assert_eq!(output, "Goodbye.\r\n");
        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn logoff_screen_is_safe_when_caller_disconnects_early() {
        let base_dir = temp_dir("logoff-terminal-early-disconnect");
        let db_path = base_dir.join("oxidebbs.ddb");
        let bind_addr = free_loopback_addr();
        let config = smoke_config(bind_addr, &base_dir, &db_path);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("listener addr");
        let client = TcpStream::connect(addr).await.expect("connect");
        let (stream, _) = listener.accept().await.expect("accept");
        drop(client);
        let mut transport = TcpTransport::new(stream);

        send_logoff_screen(
            &mut transport,
            &config,
            TerminalCapabilities::plain_text(),
            &ScreenRenderContext {
                node_number: 1,
                node_count: 1,
                board_name: "Test".to_string(),
                sysop_name: "Sysop".to_string(),
                caller_alias: None,
                security_level: None,
            },
        )
        .await;

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[test]
    fn screen_payload_normalizes_bare_lf_for_telnet_callers() {
        let base_dir = temp_dir("screen-payload-line-endings");
        let db_path = base_dir.join("oxidebbs.ddb");
        let bind_addr = free_loopback_addr();
        let mut config = smoke_config(bind_addr, &base_dir, &db_path);
        config.screens.insert(
            "line_endings".to_string(),
            crate::config::ScreenConfig {
                ansi: Some("line-endings/screen.ans".to_string()),
                ansi_40: None,
                ascii_40: None,
                ascii: Some("line-endings/screen.asc".to_string()),
                text_40: None,
                text: None,
                pause: false,
            },
        );

        let screen_dir = config.paths.screens.join("line-endings");
        std::fs::create_dir_all(&screen_dir).expect("create screen dir");
        std::fs::write(screen_dir.join("screen.ans"), b"\x1b[1mANSI\nNext\n")
            .expect("write ANSI screen");
        std::fs::write(screen_dir.join("screen.asc"), b"ASCII\nNext\n")
            .expect("write ASCII screen");

        let ansi_payload =
            load_screen_payload(&config, "line_endings", TerminalCapabilities::ansi_80())
                .expect("load ANSI screen");
        assert_eq!(ansi_payload, b"\x1b[1mANSI\r\nNext\r\n");

        let plain_payload =
            load_screen_payload(&config, "line_endings", TerminalCapabilities::plain_text())
                .expect("load plain screen");
        assert_eq!(plain_payload, b"ASCII\r\nNext\r\n");

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[test]
    fn ascii_text_encodes_without_cp437_lookup() {
        let mut output = Vec::new();

        encode_text_into("Main menu? ", &mut output, TerminalCharset::Cp437);

        assert_eq!(output, b"Main menu? ");
    }

    #[test]
    fn petscii_charset_encodes_text_to_petscii_bytes() {
        let c64 = TerminalCapabilities::c64();
        assert_eq!(c64.charset, TerminalCharset::Petscii);

        assert_eq!(
            encode_text("ABC", TerminalCharset::Petscii),
            [0x41, 0x42, 0x43]
        );

        let box_drawing = encode_text("\u{250c}\u{2500}\u{2510}", TerminalCharset::Petscii);
        assert_eq!(box_drawing, [0xb4, 0xb1, 0xb5]);
    }

    #[test]
    fn petscii_lossy_replaces_unsupported_glyphs_instead_of_failing() {
        let bytes = encode_text("C64 \u{1f680}", TerminalCharset::Petscii);
        assert_eq!(bytes, [b'C', b'6', b'4', b' ', b'?']);
    }

    #[test]
    fn non_petscii_charset_keeps_cp437_box_drawing_bytes() {
        let bytes = encode_text("\u{2554}\u{2550}", TerminalCharset::Cp437);
        assert_eq!(bytes, [0xc9, 0xcd]);
    }

    #[test]
    fn ascii_is_cp437_compatible() {
        assert!(is_cp437_compatible("Main menu? 123."));
    }

    #[test]
    fn message_subject_containing_emoji_is_rejected_before_storage() {
        assert_eq!(
            validate_caller_cp437_text("Local update 🚀"),
            Err(CP437_INPUT_REJECT_MESSAGE)
        );
    }

    #[test]
    fn message_body_containing_emoji_is_rejected_before_storage() {
        assert_eq!(
            validate_caller_cp437_text("Line one\r\nEmoji 🚀"),
            Err(CP437_INPUT_REJECT_MESSAGE)
        );
    }

    #[test]
    fn password_hashing_accepts_unencodable_password_text() {
        let config = Argon2Config::default();
        let password = test_password_with_emoji();

        let hash = server_hash_password(&password, &config).expect("hash password");

        assert_eq!(
            verify_stored_password(&password, &hash, &config).expect("verify"),
            PasswordVerification::Accepted
        );
    }

    #[test]
    fn cp437_box_drawing_output_still_encodes() {
        let text = "┌─┐";

        assert_eq!(
            encode_text(text, TerminalCharset::Cp437),
            encode_cp437(text).expect("box drawing is CP437-compatible")
        );
    }

    #[test]
    fn generated_output_replaces_unencodable_text_with_question_mark() {
        assert_eq!(
            encode_text("Diagnostic 🚀", TerminalCharset::Cp437),
            b"Diagnostic ?"
        );
    }

    #[tokio::test]
    async fn send_text_normalizes_bare_lf_to_crlf() {
        let (mut transport, mut client) = LoopbackTransport::new();

        send_text(&mut transport, "One\nTwo\n", TerminalCharset::Cp437)
            .await
            .expect("send text");

        assert_eq!(client.read_output_bytes(), b"One\r\nTwo\r\n");
    }

    #[test]
    fn node_slots_are_reused_after_drop() {
        let runtime = Arc::new(ServerRuntime::new("test".to_string(), 2, 4, 30));
        let first = runtime.try_allocate_node().expect("first slot");
        let second = runtime.try_allocate_node().expect("second slot");

        assert!(runtime.try_allocate_node().is_none());

        drop(first);
        let third = runtime
            .try_allocate_node()
            .expect("slot should be released");

        assert!(third.node_number > 0);
        drop(second);
        drop(third);
    }

    #[tokio::test]
    async fn telnet_runtime_smoke_creates_user_logs_off_and_records_lifecycle() {
        let base_dir = temp_dir("runtime-smoke");
        let db_path = base_dir.join("oxidebbs.ddb");
        let config_path = base_dir.join("oxidebbs.toml");
        let bind_addr = free_loopback_addr();
        let password = test_password();
        let mut config = smoke_config(bind_addr, &base_dir, &db_path);
        config.terminal.clear_screen_on_connect = false;
        std::fs::create_dir_all(&config.paths.ansi).expect("create ANSI dir");
        std::fs::write(
            config.paths.ansi.join(&config.terminal.welcome_screen),
            b"\x1b[1mANSI smoke welcome\r\n",
        )
        .expect("write welcome screen");
        std::fs::write(
            config.paths.ansi.join("welcome.asc"),
            b"Plain smoke welcome\r\n",
        )
        .expect("write plain welcome screen");

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server_config = config.clone();
        let server_config_path = config_path.clone();
        let server = tokio::spawn(async move {
            run_until_shutdown(&server_config, &server_config_path, async move {
                let _ = shutdown_rx.await;
                Ok(())
            })
            .await
        });

        let mut client = connect_with_retry(bind_addr).await;
        let login_output = read_until(&mut client, "Login? ").await;
        assert!(login_output.contains("Plain smoke welcome"));
        assert!(!login_output.contains("ANSI smoke welcome"));
        client.write_all(b"N\r").await.expect("select new user");
        read_until(&mut client, "Choose an alias: ").await;
        client.write_all(b"SmokeUser\r").await.expect("alias");
        read_until(&mut client, "Real name: ").await;
        client.write_all(b"Smoke User\r").await.expect("real name");
        read_until(&mut client, "Email (optional): ").await;
        client.write_all(b"\r").await.expect("blank email");
        read_until(&mut client, "Choose password: ").await;
        client
            .write_all(&line_input(&password))
            .await
            .expect("password");
        read_until(&mut client, "Confirm password: ").await;
        client
            .write_all(&line_input(&password))
            .await
            .expect("password confirmation");
        read_until(&mut client, "Command? ").await;
        client.write_all(b"L\r").await.expect("logoff");
        read_until(&mut client, "Goodbye.").await;
        drop(client);

        shutdown_tx.send(()).expect("send shutdown");
        timeout(Duration::from_secs(5), server)
            .await
            .expect("server shutdown timeout")
            .expect("server join")
            .expect("server result");

        let db = OxideDb::open_or_create(&db_path).expect("open smoke db");
        let users = oxidebbs_db::list_users(db.db()).expect("list users");
        assert!(users.iter().any(|user| user.alias == "SmokeUser"));

        let event_types = oxidebbs_db::list_audit_events(db.db(), 100)
            .expect("list audit")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<HashSet<_>>();
        for expected in [
            "config_loaded",
            "server_start",
            "node_assigned",
            "caller_connected",
            "new_user_created",
            "login_success",
            "caller_disconnected",
            "server_stop",
        ] {
            assert!(
                event_types.contains(expected),
                "missing audit event {expected}; got {event_types:?}"
            );
        }

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn file_flow_zmodem_download_persists_history() {
        let (base_dir, config, db, user, area) = file_flow_fixture("zmodem-download");
        let entry = insert_approved_file(&db, &area, b"download\xffpayload");
        let (mut transport, handle) = LoopbackTransport::new();
        let (flow_done_tx, flow_done_rx) = oneshot::channel();

        let client_task = tokio::spawn(async move {
            let mut handle = handle;
            loopback_read_until(&mut handle, "Area number").await;
            handle.write_bytes(b"1\r").expect("select file area");
            loopback_read_until(&mut handle, "Files: D)ownload").await;
            handle.write_bytes(b"D\r").expect("select download");
            loopback_read_until(&mut handle, "File number").await;
            handle.write_bytes(b"1\r").expect("select file");
            loopback_read_until(&mut handle, "Protocol:").await;
            handle.write_bytes(b"Z\r").expect("select zmodem");
            loopback_read_until(&mut handle, "via ZMODEM").await;

            let mut client = LoopbackClientBytes { handle };
            let received = oxidebbs_transfer::zmodem::receive_zmodem_file(&mut client, Some(4096))
                .await
                .expect("receive zmodem download");
            let mut handle = client.handle;
            loopback_read_until(&mut handle, "Files: D)ownload").await;
            handle.write_bytes(b"R\r").expect("leave file area");
            loopback_read_until(&mut handle, "Area number").await;
            handle.write_bytes(b"\r").expect("leave file menu");
            let _ = flow_done_rx.await;
            received
        });

        let mut input = InputSession::raw();
        let mut disconnect_reason = "test".to_string();
        let flow = async {
            let result = timeout(
                Duration::from_secs(5),
                run_files_flow(
                    Some(&user),
                    &mut transport,
                    &mut input,
                    &db,
                    &config,
                    false,
                    Duration::from_secs(1),
                    &mut disconnect_reason,
                    1,
                    TerminalCharset::Cp437,
                ),
            )
            .await;
            let _ = flow_done_tx.send(());
            result
        };
        let (flow_result, client_result) = tokio::join!(flow, client_task);
        let received = client_result.expect("client task");
        flow_result.expect("file flow timeout").expect("file flow");

        assert_eq!(received.filename, entry.display_name);
        assert_eq!(received.payload, b"download\xffpayload");
        let updated = oxidebbs_db::find_file_entry_by_id(db.db(), &entry.id)
            .expect("find updated file")
            .expect("updated file");
        assert_eq!(updated.download_count, 1);
        let transfers = oxidebbs_db::list_file_transfers(db.db()).expect("list transfers");
        assert!(transfers.iter().any(|transfer| {
            transfer.direction == "download"
                && transfer.protocol == "zmodem"
                && transfer.outcome == "success"
                && transfer.file_entry_id.as_deref() == Some(entry.id.as_str())
        }));

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn file_flow_zmodem_upload_persists_pending_entry_and_history() {
        let (base_dir, config, db, user, area) = file_flow_fixture("zmodem-upload");
        let (mut transport, handle) = LoopbackTransport::new();
        let (flow_done_tx, flow_done_rx) = oneshot::channel();

        let client_task = tokio::spawn(async move {
            let mut handle = handle;
            loopback_read_until(&mut handle, "Area number").await;
            handle.write_bytes(b"1\r").expect("select file area");
            loopback_read_until(&mut handle, "Files: D)ownload").await;
            handle.write_bytes(b"U\r").expect("select upload");
            loopback_read_until(&mut handle, "Protocol:").await;
            handle.write_bytes(b"Z\r").expect("select zmodem");
            loopback_read_until(&mut handle, "Ready to receive via ZMODEM").await;

            let mut client = LoopbackClientBytes { handle };
            oxidebbs_transfer::zmodem::send_zmodem_file(
                &mut client,
                "caller-upload.bin",
                b"uploaded payload",
            )
            .await
            .expect("send upload");
            let mut handle = client.handle;
            loopback_read_until(&mut handle, "Files: D)ownload").await;
            handle.write_bytes(b"R\r").expect("leave file area");
            loopback_read_until(&mut handle, "Area number").await;
            handle.write_bytes(b"\r").expect("leave file menu");
            let _ = flow_done_rx.await;
        });

        let mut input = InputSession::raw();
        let mut disconnect_reason = "test".to_string();
        let flow = async {
            let result = timeout(
                Duration::from_secs(5),
                run_files_flow(
                    Some(&user),
                    &mut transport,
                    &mut input,
                    &db,
                    &config,
                    false,
                    Duration::from_secs(1),
                    &mut disconnect_reason,
                    1,
                    TerminalCharset::Cp437,
                ),
            )
            .await;
            let _ = flow_done_tx.send(());
            result
        };
        let (flow_result, client_result) = tokio::join!(flow, client_task);
        client_result.expect("client task");
        flow_result.expect("file flow timeout").expect("file flow");

        let entries = oxidebbs_db::list_file_entries(db.db()).expect("list file entries");
        let upload = entries
            .iter()
            .find(|entry| entry.original_name.as_deref() == Some("caller-upload.bin"))
            .expect("uploaded entry");
        assert!(!upload.approved);
        assert_eq!(upload.size_bytes, 16);
        assert_eq!(upload.uploader_user_id.as_deref(), Some(user.id.as_str()));
        let stored = Path::new(&area.root_path).join(&upload.storage_name);
        assert_eq!(
            std::fs::read(stored).expect("read uploaded file"),
            b"uploaded payload"
        );
        let transfers = oxidebbs_db::list_file_transfers(db.db()).expect("list transfers");
        assert!(transfers.iter().any(|transfer| {
            transfer.direction == "upload"
                && transfer.protocol == "zmodem"
                && transfer.outcome == "success"
                && transfer.file_entry_id.as_deref() == Some(upload.id.as_str())
        }));

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn serial_loopback_login_menu_and_logoff_records_session() {
        let base_dir = temp_dir("serial-loopback-session");
        let db_path = base_dir.join("oxidebbs.ddb");
        let config_path = base_dir.join("oxidebbs.toml");
        let mut config = sysop_submenu_smoke_config(free_loopback_addr(), &base_dir, &db_path);
        config.terminal.clear_screen_on_connect = false;
        write_sysop_submenu_smoke_screens(&config);
        let db = Arc::new(OxideDb::open_or_create(&db_path).expect("open db"));
        let password = test_password();
        seed_login_user(&db, &config, "SerialUser", &password);
        let runtime = Arc::new(ServerRuntime::new("serial smoke".to_string(), 1, 1, 60));
        let allocation = runtime.try_allocate_node().expect("allocate serial node");
        let mut menus = HashMap::new();
        for menu_id in config.menus.keys() {
            let menu = config.core_menu(menu_id).expect("core menu");
            menus.insert(menu_id.clone(), Arc::new(menu));
        }
        let login_menu = menus
            .get(&config.flow.login_menu)
            .expect("login menu")
            .clone();
        let main_menu = menus
            .get(&config.flow.main_menu)
            .expect("main menu")
            .clone();
        let resources = CallerResources {
            db: Arc::clone(&db),
            config: Arc::new(config),
            login_menu,
            main_menu,
            menus: Arc::new(menus),
            runtime,
        };
        let (transport, mut client) = SerialLoopback::new();
        let server = tokio::spawn(async move {
            handle_caller_transport(
                allocation,
                transport,
                "serial",
                false,
                None,
                CallerPeer {
                    address: "serial:test".to_string(),
                    ip: None,
                    port: 0,
                },
                resources,
            )
            .await
        });

        client
            .write_bytes(format!("L\rSerialUser\r{password}\rL\r").as_bytes())
            .expect("write serial login flow");
        let output = serial_read_until(&mut client, "Goodbye.").await;
        assert!(output.contains("Login successful. Welcome back."));
        timeout(Duration::from_secs(5), server)
            .await
            .expect("serial server timeout")
            .expect("serial join")
            .expect("serial session");

        let sessions = oxidebbs_db::list_recent_sessions(db.db(), 10).expect("sessions");
        assert!(sessions.iter().any(|session| {
            session.transport == "serial"
                && session.disconnect_reason.as_deref() == Some("caller_logoff")
        }));

        let _ = config_path;
        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn normal_level_caller_cannot_open_sysop_submenu() {
        let output = run_sysop_submenu_access_smoke(10, false).await;

        assert!(output.contains(ACCESS_DENIED_MESSAGE.trim()));
    }

    #[tokio::test]
    async fn sysop_level_caller_can_open_sysop_submenu() {
        let output = run_sysop_submenu_access_smoke(255, true).await;

        assert!(output.contains("Sysop? "));
        assert!(!output.contains(ACCESS_DENIED_MESSAGE.trim()));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stale_node_sweeper_marks_nodes_for_disconnect() {
        let runtime = Arc::new(ServerRuntime::new("test".to_string(), 1, 4, 1));
        runtime.mark_node_connected(
            1,
            "session-1".to_string(),
            "127.0.0.1:5000".to_string(),
            "connected".to_string(),
        );
        runtime.force_node_heartbeat_age(1, Duration::from_secs(5));

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let sweeper =
            start_stale_node_sweeper(runtime.clone(), Duration::from_millis(20), shutdown_rx);

        timeout(Duration::from_secs(1), async {
            loop {
                if runtime.take_node_commands(1).disconnect_reason.as_deref()
                    == Some("stale_node_timeout")
                {
                    break;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("stale-node sweep should trigger");

        let _ = shutdown_tx.send(());
        assert!(sweeper.await.is_ok());
        assert_eq!(runtime.node_status(1).expect("node").state, "disconnecting");
    }

    #[tokio::test]
    async fn next_event_reads_through_telnet_negotiation_frames() {
        let (mut transport, client) = LoopbackTransport::new();
        client
            .write_bytes(&[IAC, DO, TELOPT_SUPPRESS_GO_AHEAD])
            .expect("write negotiation");
        let mut input = InputSession::default();

        let event = next_event(&mut transport, &mut input, Duration::from_secs(1))
            .await
            .expect("read event");

        match event {
            CallerInput::Event(TelnetEvent::Negotiation { accepted, .. }) => {
                assert!(accepted);
            }
            other => panic!("expected negotiation event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn menu_line_ending_drain_keeps_following_input() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client.write_bytes(b"\r\nX").expect("write input");

        drain_line_ending_after_menu_key(&mut transport, &mut input)
            .await
            .expect("drain");
        let event = next_event(&mut transport, &mut input, Duration::from_secs(1))
            .await
            .expect("read event");

        match event {
            CallerInput::Event(TelnetEvent::Data(b'X')) => {}
            other => panic!("expected preserved input, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_line_input_returns_blank_when_allowed() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client.write_bytes(b"\r").expect("write blank line");

        let value = read_line_input(
            &mut transport,
            &mut input,
            Duration::from_secs(1),
            true,
            false,
            TerminalCharset::Cp437,
        )
        .await
        .expect("read");

        match value {
            PromptLineResult::Value(value) => assert_eq!(value, ""),
            other => panic!("expected value, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_line_input_rejects_blank_when_required() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client.write_bytes(b"\rHello\r").expect("write value");

        let value = read_line_input(
            &mut transport,
            &mut input,
            Duration::from_secs(1),
            false,
            false,
            TerminalCharset::Cp437,
        )
        .await
        .expect("read");

        match value {
            PromptLineResult::Value(value) => assert_eq!(value, "Hello"),
            other => panic!("expected value, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_line_input_rejects_non_cp437_text() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client
            .write_bytes("Hello 🚀\r".as_bytes())
            .expect("write value");

        let value = read_line_input(
            &mut transport,
            &mut input,
            Duration::from_secs(1),
            false,
            false,
            TerminalCharset::Cp437,
        )
        .await
        .expect("read");

        assert!(matches!(value, PromptLineResult::Rejected));
    }

    #[tokio::test]
    async fn read_line_input_ignores_cp437_policy_for_hidden_input() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();
        let password = test_password_with_emoji();

        client
            .write_bytes(&line_input(&password))
            .expect("write value");

        let value = read_line_input(
            &mut transport,
            &mut input,
            Duration::from_secs(1),
            false,
            true,
            TerminalCharset::Cp437,
        )
        .await
        .expect("read");

        match value {
            PromptLineResult::Value(value) => assert_eq!(value, password),
            other => panic!("expected value, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_line_input_treats_backspace_byte_as_delete() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client.write_bytes(b"AB\x08C\r").expect("write value");

        let value = read_line_input(
            &mut transport,
            &mut input,
            Duration::from_secs(1),
            false,
            false,
            TerminalCharset::Cp437,
        )
        .await
        .expect("read");

        match value {
            PromptLineResult::Value(value) => assert_eq!(value, "AC"),
            other => panic!("expected value, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_line_input_treats_delete_byte_as_delete() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client.write_bytes(b"AB\x7fC\r").expect("write value");

        let value = read_line_input(
            &mut transport,
            &mut input,
            Duration::from_secs(1),
            false,
            false,
            TerminalCharset::Cp437,
        )
        .await
        .expect("read");

        match value {
            PromptLineResult::Value(value) => assert_eq!(value, "AC"),
            other => panic!("expected value, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn menu_line_ending_drain_single_key_does_not_wait() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client.write_bytes(b"N").expect("write menu key");

        let drained = timeout(
            TestDuration::from_millis(4),
            drain_line_ending_after_menu_key(&mut transport, &mut input),
        )
        .await
        .expect("menu key drain should not wait");
        drained.expect("drain should succeed");

        let event = next_event(&mut transport, &mut input, TestDuration::from_millis(1))
            .await
            .expect("read preserved key");

        match event {
            CallerInput::Event(TelnetEvent::Data(b'N')) => {}
            other => panic!("expected preserved N event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn menu_line_ending_drain_discards_telnet_cr_nul() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client.write_bytes(b"\r\0X").expect("write line ending");

        let drained = timeout(
            TestDuration::from_millis(4),
            drain_line_ending_after_menu_key(&mut transport, &mut input),
        )
        .await
        .expect("menu key drain should not wait");
        drained.expect("drain should succeed");

        let event = next_event(&mut transport, &mut input, TestDuration::from_millis(1))
            .await
            .expect("read preserved key");

        match event {
            CallerInput::Event(TelnetEvent::Data(b'X')) => {}
            other => panic!("expected preserved X event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_line_input_ignores_leading_lf_from_crlf() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client.write_bytes(b"\nHello\r").expect("write value");

        let value = read_line_input(
            &mut transport,
            &mut input,
            Duration::from_secs(1),
            true,
            false,
            TerminalCharset::Cp437,
        )
        .await
        .expect("read");

        match value {
            PromptLineResult::Value(value) => assert_eq!(value, "Hello"),
            other => panic!("expected value, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_line_input_ignores_leading_nul_from_telnet_cr_nul() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client.write_bytes(b"\0Hello\r").expect("write value");

        let value = read_line_input(
            &mut transport,
            &mut input,
            Duration::from_secs(1),
            true,
            false,
            TerminalCharset::Cp437,
        )
        .await
        .expect("read");

        match value {
            PromptLineResult::Value(value) => assert_eq!(value, "Hello"),
            other => panic!("expected value, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn prompt_for_message_body_collects_lines_until_dot() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client
            .write_bytes(b"First line\r\n\r\nLast line\r\n.\r\n")
            .expect("write body");

        let value = prompt_for_message_body(
            &mut transport,
            &mut input,
            Duration::from_secs(1),
            TerminalCharset::Cp437,
        )
        .await
        .expect("read body");

        match value {
            PromptLineResult::Value(value) => assert_eq!(value, "First line\r\n\r\nLast line"),
            other => panic!("expected value, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn alias_miss_and_wrong_password_show_identical_failure_message() {
        let db = OxideDb::open_memory().expect("open db");
        let base_dir = temp_dir("auth-visible-failure");
        let config = smoke_config(free_loopback_addr(), &base_dir, &base_dir.join("auth.ddb"));
        let password = test_password();
        let wrong_password = mismatched_password(&password);
        seed_login_user(&db, &config, "Alice", &password);

        let missing = run_login_subflow(&db, &config, "Nobody", &wrong_password, "127.0.0.1")
            .await
            .1;
        let wrong = run_login_subflow(&db, &config, "Alice", &wrong_password, "127.0.0.2")
            .await
            .1;

        assert!(missing.contains(INVALID_LOGIN_MESSAGE.trim()));
        assert!(wrong.contains(INVALID_LOGIN_MESSAGE.trim()));
        assert_eq!(failure_line(&missing), failure_line(&wrong));

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn login_flow_runs_over_loopback_transport() {
        let db = OxideDb::open_memory().expect("open db");
        let base_dir = temp_dir("auth-loopback-transport");
        let config = smoke_config(free_loopback_addr(), &base_dir, &base_dir.join("auth.ddb"));
        let password = test_password();
        seed_login_user(&db, &config, "Alice", &password);

        let (mut transport, mut client) = LoopbackTransport::new();
        client
            .write_bytes(format!("Alice\r{password}\r").as_bytes())
            .expect("write credentials");

        let mut input = InputSession::default();
        let mut authenticated_user = None;
        let mut disconnect_reason = "test".to_string();
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 60);
        let mut state = AuthFlowState {
            db: &db,
            config: &config,
            runtime: &runtime,
            node_number: 1,
            remote_ip: "127.0.0.1",
            session_id: "00000000-0000-4000-8000-000000000779",
            authenticated_user: &mut authenticated_user,
            idle_timeout: Duration::from_secs(1),
            disconnect_reason: &mut disconnect_reason,
        };

        let result = run_login_flow(
            &mut transport,
            &mut input,
            &mut state,
            TerminalCharset::Cp437,
        )
        .await
        .expect("login flow");
        let output = String::from_utf8_lossy(&client.read_output_bytes()).into_owned();

        assert!(matches!(result, AuthFlowResult::Success));
        assert!(output.contains("Login successful. Welcome back."));
        assert_eq!(
            authenticated_user.as_ref().map(|user| user.alias.as_str()),
            Some("Alice")
        );

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn new_user_flow_reports_duplicate_alias_friendly_message() {
        let db = OxideDb::open_memory().expect("open db");
        let base_dir = temp_dir("new-user-duplicate");
        let config = smoke_config(
            free_loopback_addr(),
            &base_dir,
            &base_dir.join("duplicate.ddb"),
        );
        let password = test_password();
        seed_login_user(&db, &config, "Alice", &password);

        let (result, output) =
            run_new_user_subflow(&db, &config, "alice", "Alice Clone", &password).await;

        assert!(matches!(result, AuthFlowResult::Retry));
        assert!(output.contains("That alias is already in use."));

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn rate_limiter_bounds_login_failure_audit_writes() {
        let db = OxideDb::open_memory().expect("open db");
        let base_dir = temp_dir("auth-audit-bound");
        let config = smoke_config(free_loopback_addr(), &base_dir, &base_dir.join("auth.ddb"));
        let password = test_password();

        let mut last_output = String::new();
        for _ in 0..6 {
            last_output = run_login_subflow(&db, &config, "Nobody", &password, "127.0.0.1")
                .await
                .1;
        }

        assert!(last_output.contains(LOGIN_LOCKOUT_MESSAGE.trim()));
        let failure_events = oxidebbs_db::list_audit_events(db.db(), 100)
            .expect("list audit")
            .into_iter()
            .filter(|event| event.event_type == "login_failure")
            .count();
        assert_eq!(failure_events, 5);

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn successful_login_clears_persistent_auth_attempt_scopes() {
        let db = OxideDb::open_memory().expect("open db");
        let base_dir = temp_dir("auth-clear");
        let config = smoke_config(free_loopback_addr(), &base_dir, &base_dir.join("auth.ddb"));
        let password = test_password();
        seed_login_user(&db, &config, "Alice", &password);
        let now = current_timestamp(&db).expect("timestamp");
        record_auth_failure(
            db.db(),
            "ip",
            "127.0.0.1",
            &now,
            config.auth.failed_login_window_minutes,
            config.auth.failed_login_lockout_minutes,
            config.auth.failed_login_threshold,
        )
        .expect("record ip failure");
        record_auth_failure(
            db.db(),
            "alias",
            "alice",
            &now,
            config.auth.failed_login_window_minutes,
            config.auth.failed_login_lockout_minutes,
            config.auth.failed_login_threshold,
        )
        .expect("record alias failure");

        let (result, output, authenticated_user) =
            run_login_subflow(&db, &config, "Alice", &password, "127.0.0.1").await;

        assert!(matches!(result, AuthFlowResult::Success));
        assert!(output.contains("Login successful. Welcome back."));
        assert_eq!(
            authenticated_user.as_ref().map(|user| user.alias.as_str()),
            Some("Alice")
        );
        assert!(
            oxidebbs_db::find_auth_attempt(db.db(), "ip", "127.0.0.1")
                .expect("find ip")
                .is_none()
        );
        assert!(
            oxidebbs_db::find_auth_attempt(db.db(), "alias", "alice")
                .expect("find alias")
                .is_none()
        );

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[test]
    fn server_password_hashes_verify_with_argon2() {
        let config = Argon2Config::default();
        let password = test_password();
        let wrong_password = mismatched_password(&password);
        let hash = server_hash_password(&password, &config).expect("hash password");

        assert!(hash.starts_with("$argon2id$"));
        assert_eq!(
            verify_stored_password(&password, &hash, &config).expect("verify"),
            PasswordVerification::Accepted
        );
        assert_eq!(
            verify_stored_password(&wrong_password, &hash, &config).expect("verify"),
            PasswordVerification::Rejected
        );
    }

    #[test]
    fn invalid_password_hash_runs_dummy_verify_and_fails_closed() {
        let config = Argon2Config::default();
        let password = test_password();

        let result = verify_stored_password(&password, "not-a-phc", &config).expect("verify");

        assert_eq!(result, PasswordVerification::HashParseFailure);
    }

    #[test]
    fn message_record_preserves_reply_metadata() {
        let message = Message {
            id: "00000000-0000-4000-8000-000000000301".to_string(),
            area_id: "00000000-0000-4000-8000-000000000101".to_string(),
            author_user_id: "00000000-0000-4000-8000-000000000011".to_string(),
            to_user_id: None,
            subject: "Re: Hello".to_string(),
            body: "Reply".to_string(),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            reply_to_id: Some("00000000-0000-4000-8000-000000000201".to_string()),
            network_message_id: Some("net-1".to_string()),
            visibility: MessageVisibility::Normal,
        };

        let record = message_record_from_message(&message);

        assert_eq!(record.reply_to_id, message.reply_to_id);
        assert_eq!(record.network_message_id, message.network_message_id);
        assert_eq!(record.visibility, "normal");
    }

    #[test]
    fn hidden_message_visibility_maps_to_pending() {
        assert!(matches!(
            message_visibility_from_db("hidden"),
            MessageVisibility::PendingModeration
        ));
    }

    #[tokio::test]
    async fn display_message_list_uses_author_aliases_for_multiple_authors() {
        let db = OxideDb::open_memory().expect("open db");
        insert_author_user(&db, "00000000-0000-4000-8000-000000000901", "alice");
        insert_author_user(&db, "00000000-0000-4000-8000-000000000902", "bob");
        let area = test_message_area();
        let messages = vec![
            test_message(
                "00000000-0000-4000-8000-000000000301",
                "00000000-0000-4000-8000-000000000901",
                "One",
            ),
            test_message(
                "00000000-0000-4000-8000-000000000302",
                "00000000-0000-4000-8000-000000000902",
                "Two",
            ),
        ];
        let (mut transport, mut client) = LoopbackTransport::new();

        display_message_list(
            &mut transport,
            &db,
            &area,
            &messages,
            TerminalCharset::Cp437,
        )
        .await
        .expect("display");
        let output = String::from_utf8_lossy(&client.read_output_bytes()).to_string();

        assert!(output.contains("1) One (from alice)"));
        assert!(output.contains("2) Two (from bob)"));
    }

    #[tokio::test]
    async fn display_message_list_uses_unknown_for_missing_author() {
        let db = OxideDb::open_memory().expect("open db");
        let area = test_message_area();
        let messages = vec![test_message(
            "00000000-0000-4000-8000-000000000303",
            "00000000-0000-4000-8000-000000000999",
            "Missing",
        )];
        let (mut transport, mut client) = LoopbackTransport::new();

        display_message_list(
            &mut transport,
            &db,
            &area,
            &messages,
            TerminalCharset::Cp437,
        )
        .await
        .expect("display");
        let output = String::from_utf8_lossy(&client.read_output_bytes()).to_string();

        assert!(output.contains("1) Missing (from Unknown)"));
    }

    fn seed_login_user(db: &OxideDb, config: &OxideConfig, alias: &str, password: &str) {
        seed_login_user_with_level(db, config, alias, password, 10, false);
    }

    fn file_flow_fixture(name: &str) -> (PathBuf, OxideConfig, OxideDb, User, FileAreaRecord) {
        let base_dir = temp_dir(name);
        let db_path = base_dir.join("oxidebbs.ddb");
        let mut config = smoke_config(free_loopback_addr(), &base_dir, &db_path);
        config.file_transfers.enabled = true;
        let db = OxideDb::open_or_create(&db_path).expect("open file flow db");
        let password = test_password();
        seed_login_user_with_level(&db, &config, "FileUser", &password, 50, false);
        let user_record = find_user_by_alias_ci(db.db(), "FileUser")
            .expect("find file user")
            .expect("file user exists");
        let user = user_from_record(&user_record).expect("user from record");
        let root = base_dir.join("file-area");
        std::fs::create_dir_all(&root).expect("create file area root");
        let area = FileAreaRecord {
            id: String::new(),
            key: "main".to_string(),
            name: "Main Files".to_string(),
            description: "Main file area".to_string(),
            root_path: root.to_string_lossy().into_owned(),
            read_security_level: 0,
            download_security_level: 10,
            upload_security_level: 10,
            max_upload_bytes: Some(4096),
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        };
        oxidebbs_db::insert_file_area(db.db(), &area).expect("insert file area");
        let stored_area = oxidebbs_db::list_file_areas(db.db())
            .expect("list file areas")
            .into_iter()
            .find(|area| area.key == "main")
            .expect("stored area");
        (base_dir, config, db, user, stored_area)
    }

    fn insert_approved_file(
        db: &OxideDb,
        area: &FileAreaRecord,
        payload: &[u8],
    ) -> FileEntryRecord {
        let storage_name = "demo.bin".to_string();
        std::fs::write(Path::new(&area.root_path).join(&storage_name), payload)
            .expect("write fixture file");
        let entry = FileEntryRecord {
            id: String::new(),
            area_id: area.id.clone(),
            storage_name: storage_name.clone(),
            display_name: "demo.bin".to_string(),
            original_name: Some("demo.bin".to_string()),
            size_bytes: payload.len() as i64,
            content_crc32: None,
            description: "Fixture file".to_string(),
            uploader_user_id: None,
            download_count: 0,
            approved: true,
            created_at: String::new(),
            updated_at: String::new(),
        };
        oxidebbs_db::insert_file_entry(db.db(), &entry).expect("insert file entry");
        find_file_entry_by_storage_name(db.db(), &area.id, &storage_name)
            .expect("find fixture file")
            .expect("fixture file exists")
    }

    async fn serial_read_until(client: &mut SerialHandle, needle: &str) -> String {
        let mut output = Vec::new();
        timeout(Duration::from_secs(5), async {
            loop {
                let Some(byte) = client.read_byte().await else {
                    panic!(
                        "serial closed before {needle:?}; output was {:?}",
                        String::from_utf8_lossy(&output)
                    );
                };
                output.push(byte);
                if String::from_utf8_lossy(&output).contains(needle) {
                    break;
                }
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "timed out waiting for serial {needle:?}; output was {:?}",
                String::from_utf8_lossy(&output)
            )
        });
        String::from_utf8_lossy(&output).to_string()
    }

    async fn loopback_read_until(client: &mut LoopbackHandle, needle: &str) -> String {
        let mut output = Vec::new();
        timeout(Duration::from_secs(5), async {
            loop {
                let Some(byte) = client.read_byte().await else {
                    panic!(
                        "loopback closed before {needle:?}; output was {:?}",
                        String::from_utf8_lossy(&output)
                    );
                };
                output.push(byte);
                if String::from_utf8_lossy(&output).contains(needle) {
                    break;
                }
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "timed out waiting for loopback {needle:?}; output was {:?}",
                String::from_utf8_lossy(&output)
            )
        });
        String::from_utf8_lossy(&output).to_string()
    }

    fn seed_login_user_with_level(
        db: &OxideDb,
        config: &OxideConfig,
        alias: &str,
        password: &str,
        security_level: i64,
        is_sysop: bool,
    ) {
        let now = current_timestamp(db).expect("timestamp");
        let user = UserRecord {
            id: generated_uuid(db).expect("uuid"),
            alias: alias.to_string(),
            real_name: format!("{alias} User"),
            email: None,
            password_hash: server_hash_password(password, &config.auth.argon2).expect("hash"),
            security_level,
            is_sysop,
            created_at: now,
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: "active".to_string(),
        };
        insert_user(db.db(), &user).expect("insert login user");
    }

    fn insert_author_user(db: &OxideDb, id: &str, alias: &str) {
        insert_user(
            db.db(),
            &UserRecord {
                id: id.to_string(),
                alias: alias.to_string(),
                real_name: format!("{alias} User"),
                email: None,
                password_hash: "hash".to_string(),
                security_level: 10,
                is_sysop: false,
                created_at: "2026-01-01T00:00:00.000000Z".to_string(),
                last_login_at: None,
                total_calls: 0,
                time_bank_minutes: 0,
                status: "active".to_string(),
            },
        )
        .expect("insert author");
    }

    fn test_message_area() -> MessageArea {
        MessageArea {
            id: "00000000-0000-4000-8000-000000000101".to_string(),
            key: "general".to_string(),
            name: "General".to_string(),
            description: "General discussion".to_string(),
            kind: AreaKind::Local,
            network_id: None,
            read_security_level: 0,
            post_security_level: 10,
            moderated: false,
        }
    }

    fn test_message(id: &str, author_user_id: &str, subject: &str) -> Message {
        Message {
            id: id.to_string(),
            area_id: "00000000-0000-4000-8000-000000000101".to_string(),
            author_user_id: author_user_id.to_string(),
            to_user_id: None,
            subject: subject.to_string(),
            body: "Body".to_string(),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            reply_to_id: None,
            network_message_id: None,
            visibility: MessageVisibility::Normal,
        }
    }

    async fn run_login_subflow(
        db: &OxideDb,
        config: &OxideConfig,
        alias: &str,
        password: &str,
        remote_ip: &str,
    ) -> (AuthFlowResult, String, Option<User>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("listener addr");
        let alias = alias.to_string();
        let password = password.to_string();
        let client_task = tokio::spawn(async move {
            let mut client = TcpStream::connect(addr).await.expect("connect");
            client
                .write_all(format!("{alias}\r{password}\r").as_bytes())
                .await
                .expect("write credentials");
            read_until_any(
                &mut client,
                &[
                    INVALID_LOGIN_MESSAGE.trim(),
                    LOGIN_LOCKOUT_MESSAGE.trim(),
                    "Login successful. Welcome back.",
                ],
            )
            .await
        });
        let (stream, _) = listener.accept().await.expect("accept");
        let mut transport = TcpTransport::new(stream);
        let mut input = InputSession::default();
        let mut authenticated_user = None;
        let mut disconnect_reason = "test".to_string();
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 60);
        let mut state = AuthFlowState {
            db,
            config,
            runtime: &runtime,
            node_number: 1,
            remote_ip,
            session_id: "00000000-0000-4000-8000-000000000777",
            authenticated_user: &mut authenticated_user,
            idle_timeout: Duration::from_secs(1),
            disconnect_reason: &mut disconnect_reason,
        };
        let result = run_login_flow(
            &mut transport,
            &mut input,
            &mut state,
            TerminalCharset::Cp437,
        )
        .await
        .expect("login flow");
        let output = client_task.await.expect("client task");
        (result, output, authenticated_user)
    }

    async fn run_new_user_subflow(
        db: &OxideDb,
        config: &OxideConfig,
        alias: &str,
        real_name: &str,
        password: &str,
    ) -> (AuthFlowResult, String) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("listener addr");
        let alias = alias.to_string();
        let real_name = real_name.to_string();
        let password = password.to_string();
        let client_task = tokio::spawn(async move {
            let mut client = TcpStream::connect(addr).await.expect("connect");
            client
                .write_all(format!("{alias}\r{real_name}\r\r{password}\r{password}\r").as_bytes())
                .await
                .expect("write registration");
            read_until_any(
                &mut client,
                &[
                    "That alias is already in use.",
                    "Account created. Welcome.",
                    "Unable to create account:",
                ],
            )
            .await
        });
        let (stream, _) = listener.accept().await.expect("accept");
        let mut transport = TcpTransport::new(stream);
        let mut input = InputSession::default();
        let mut authenticated_user = None;
        let mut disconnect_reason = "test".to_string();
        let runtime = ServerRuntime::new("test".to_string(), 1, 1, 60);
        let mut state = AuthFlowState {
            db,
            config,
            runtime: &runtime,
            node_number: 1,
            remote_ip: "127.0.0.1",
            session_id: "00000000-0000-4000-8000-000000000778",
            authenticated_user: &mut authenticated_user,
            idle_timeout: Duration::from_secs(1),
            disconnect_reason: &mut disconnect_reason,
        };
        let result = run_new_user_flow(
            &mut transport,
            &mut input,
            &mut state,
            TerminalCharset::Cp437,
        )
        .await
        .expect("new user flow");
        let output = client_task.await.expect("client task");
        (result, output)
    }

    async fn capture_logoff_output(
        config: OxideConfig,
        capabilities: TerminalCapabilities,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("listener addr");
        let client_task = tokio::spawn(async move {
            let mut client = TcpStream::connect(addr).await.expect("connect");
            let mut output = Vec::new();
            timeout(Duration::from_secs(2), client.read_to_end(&mut output))
                .await
                .expect("logoff read timeout")
                .expect("read logoff output");
            String::from_utf8_lossy(&output).to_string()
        });
        let (stream, _) = listener.accept().await.expect("accept");
        let mut transport = TcpTransport::new(stream);

        send_logoff_screen(
            &mut transport,
            &config,
            capabilities,
            &ScreenRenderContext {
                node_number: 1,
                node_count: 1,
                board_name: "Test".to_string(),
                sysop_name: "Sysop".to_string(),
                caller_alias: None,
                security_level: None,
            },
        )
        .await;
        drop(transport);

        client_task.await.expect("client task")
    }

    async fn run_sysop_submenu_access_smoke(security_level: i64, is_sysop: bool) -> String {
        let base_dir = temp_dir("sysop-submenu-access");
        let db_path = base_dir.join("oxidebbs.ddb");
        let config_path = base_dir.join("oxidebbs.toml");
        let bind_addr = free_loopback_addr();
        let config = sysop_submenu_smoke_config(bind_addr, &base_dir, &db_path);
        write_sysop_submenu_smoke_screens(&config);
        let password = test_password();
        {
            let db = OxideDb::open_or_create(&db_path).expect("open db");
            seed_login_user_with_level(
                &db,
                &config,
                "AccessUser",
                &password,
                security_level,
                is_sysop,
            );
        }

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server_config = config.clone();
        let server_config_path = config_path.clone();
        let server = tokio::spawn(async move {
            run_until_shutdown(&server_config, &server_config_path, async move {
                let _ = shutdown_rx.await;
                Ok(())
            })
            .await
        });

        let mut client = connect_with_retry(bind_addr).await;
        read_until(&mut client, "Login? ").await;
        client.write_all(b"L\r").await.expect("select login");
        read_until(&mut client, "Alias: ").await;
        client.write_all(b"AccessUser\r").await.expect("alias");
        read_until(&mut client, "Password: ").await;
        client
            .write_all(&line_input(&password))
            .await
            .expect("password");
        read_until(&mut client, "Command? ").await;
        client.write_all(b"S\r").await.expect("select sysop");
        let output = if security_level >= 255 {
            read_until(&mut client, "Sysop? ").await
        } else {
            read_until(&mut client, "Command? ").await
        };
        client.write_all(b"L\r").await.expect("logoff");
        read_until(&mut client, "Goodbye.").await;
        drop(client);

        shutdown_tx.send(()).expect("send shutdown");
        timeout(Duration::from_secs(5), server)
            .await
            .expect("server shutdown timeout")
            .expect("server join")
            .expect("server result");

        let _ = std::fs::remove_dir_all(base_dir);
        output
    }

    async fn read_until_any(client: &mut TcpStream, needles: &[&str]) -> String {
        let mut output = Vec::new();
        timeout(Duration::from_secs(5), async {
            loop {
                let mut byte = [0u8; 1];
                let read = client.read(&mut byte).await.expect("read login output");
                if read == 0 {
                    break;
                }
                output.push(byte[0]);
                let text = String::from_utf8_lossy(&output);
                if needles.iter().any(|needle| text.contains(needle)) {
                    break;
                }
            }
        })
        .await
        .expect("login output");
        String::from_utf8_lossy(&output).to_string()
    }

    fn failure_line(output: &str) -> Option<&str> {
        output
            .lines()
            .find(|line| line.contains(INVALID_LOGIN_MESSAGE.trim()))
    }

    fn free_loopback_addr() -> SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback probe");
        let addr = listener.local_addr().expect("probe local addr");
        drop(listener);
        addr
    }

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "oxidebbs-server-{name}-{}-{suffix}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_login_screen_variants(config: &mut OxideConfig) {
        config.screens.insert(
            "login".to_string(),
            crate::config::ScreenConfig {
                ansi: Some("login/login.ans".to_string()),
                ansi_40: Some("login/login-40.ans".to_string()),
                ascii_40: Some("login/login-40.asc".to_string()),
                ascii: Some("login/login.asc".to_string()),
                text_40: Some("login/login-40.txt".to_string()),
                text: Some("login/login.txt".to_string()),
                pause: false,
            },
        );

        let login_dir = config.paths.screens.join("login");
        std::fs::create_dir_all(&login_dir).expect("create login screen dir");
        std::fs::write(login_dir.join("login.ans"), b"ANSI80\r\n").expect("write 80-col ANSI");
        std::fs::write(login_dir.join("login-40.ans"), b"ANSI40\r\n").expect("write 40-col ANSI");
        std::fs::write(login_dir.join("login-40.asc"), b"ASCII40\r\n").expect("write 40-col ASCII");
        std::fs::write(login_dir.join("login-40.txt"), b"TEXT40\r\n").expect("write 40-col text");
        std::fs::write(login_dir.join("login.asc"), b"ASCII\r\n").expect("write ASCII");
        std::fs::write(login_dir.join("login.txt"), b"TEXT\r\n").expect("write text");
    }

    fn smoke_config(bind_addr: SocketAddr, base_dir: &Path, db_path: &Path) -> OxideConfig {
        let mut config: OxideConfig = toml::from_str(
            r#"
[board]
name = "Smoke BBS"

[telnet]
enabled = true
bind = "127.0.0.1:0"
max_connections = 1
idle_timeout_seconds = 5

[database]
path = "oxidebbs.ddb"

[paths]
ansi = "ansi"
screens = "screens"
doors = "doors"
runtime = "runtime"
logs = "logs"

[flow]
login_screen = "login"
login_menu = "login"
main_menu = "main"

[screens.login]
text = "login.txt"

[screens.main_menu]
text = "main.txt"

[menus.login]
screen = "login"
prompt = "Login? "

[[menus.login.items]]
key = "N"
label = "New User"
action = "new_user"

[[menus.login.items]]
key = "L"
label = "Logoff"
action = "logoff"

[menus.main]
screen = "main_menu"
prompt = "Command? "

[[menus.main.items]]
key = "L"
label = "Logoff"
action = "logoff"
"#,
        )
        .expect("parse smoke config");
        config.telnet.bind = bind_addr.to_string();
        config.database.path = db_path.to_path_buf();
        config.paths.ansi = base_dir.join("ansi");
        config.paths.screens = base_dir.join("screens");
        config.paths.doors = base_dir.join("doors");
        config.paths.runtime = base_dir.join("runtime");
        config.paths.logs = base_dir.join("logs");
        config
    }

    fn sysop_submenu_smoke_config(
        bind_addr: SocketAddr,
        base_dir: &Path,
        db_path: &Path,
    ) -> OxideConfig {
        let mut config: OxideConfig = toml::from_str(
            r#"
[board]
name = "Sysop Smoke BBS"

[telnet]
enabled = true
bind = "127.0.0.1:0"
max_connections = 1
idle_timeout_seconds = 5

[database]
path = "oxidebbs.ddb"

[paths]
ansi = "ansi"
screens = "screens"
doors = "doors"
runtime = "runtime"
logs = "logs"

[flow]
login_screen = "login"
login_menu = "login"
main_menu = "main"

[screens.login]
text = "login.txt"

[screens.main_menu]
text = "main.txt"

[screens.sysop_menu]
text = "sysop.txt"

[menus.login]
screen = "login"
prompt = "Login? "

[[menus.login.items]]
key = "L"
label = "Logon"
action = "login"

[[menus.login.items]]
key = "G"
label = "Goodbye"
action = "logoff"

[menus.main]
screen = "main_menu"
prompt = "Command? "

[[menus.main.items]]
key = "S"
label = "Sysop"
action = "submenu"
target = "sysop"
min_security_level = 255

[[menus.main.items]]
key = "L"
label = "Logoff"
action = "logoff"

[menus.sysop]
screen = "sysop_menu"
prompt = "Sysop? "

[[menus.sysop.items]]
key = "L"
label = "Goodbye"
action = "logoff"
"#,
        )
        .expect("parse sysop submenu smoke config");
        config.telnet.bind = bind_addr.to_string();
        config.database.path = db_path.to_path_buf();
        config.paths.ansi = base_dir.join("ansi");
        config.paths.screens = base_dir.join("screens");
        config.paths.doors = base_dir.join("doors");
        config.paths.runtime = base_dir.join("runtime");
        config.paths.logs = base_dir.join("logs");
        config
    }

    fn write_sysop_submenu_smoke_screens(config: &OxideConfig) {
        std::fs::create_dir_all(&config.paths.screens).expect("create screen dir");
        std::fs::write(config.paths.screens.join("login.txt"), b"Login\r\n")
            .expect("write login screen");
        std::fs::write(config.paths.screens.join("main.txt"), b"Main\r\n")
            .expect("write main screen");
        std::fs::write(config.paths.screens.join("sysop.txt"), b"Sysop Menu\r\n")
            .expect("write sysop screen");
    }

    async fn connect_with_retry(addr: SocketAddr) -> TcpStream {
        let mut last_error = None;
        for _ in 0..50 {
            match TcpStream::connect(addr).await {
                Ok(stream) => return stream,
                Err(error) => {
                    last_error = Some(error);
                    sleep(Duration::from_millis(20)).await;
                }
            }
        }
        panic!("failed to connect to smoke server at {addr}: {last_error:?}");
    }

    async fn read_until(client: &mut TcpStream, needle: &str) -> String {
        let mut output = Vec::new();
        timeout(Duration::from_secs(5), async {
            loop {
                let mut byte = [0u8; 1];
                let read = client.read(&mut byte).await.expect("read smoke output");
                assert!(
                    read > 0,
                    "server closed before {needle:?}; output was {:?}",
                    String::from_utf8_lossy(&output)
                );
                output.push(byte[0]);
                if String::from_utf8_lossy(&output).contains(needle) {
                    break;
                }
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "timed out waiting for {needle:?}; output was {:?}",
                String::from_utf8_lossy(&output)
            )
        });
        String::from_utf8_lossy(&output).to_string()
    }
}
