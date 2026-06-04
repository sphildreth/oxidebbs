use std::collections::{HashMap, VecDeque};
use std::fmt::Write as FmtWrite;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

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
    AuditEventRecord, MessageAreaRecord, MessageRecord, OxideDb, SessionRecord, UserInsertError,
    UserRecord, clear_auth_attempt, end_session, find_user_by_alias_ci, insert_audit_event,
    insert_message, insert_message_area, insert_session, insert_user_if_alias_available,
    is_auth_scope_locked, list_audit_events, list_auth_attempts, list_door_definitions,
    list_door_runs, list_message_areas, list_messages, list_recent_sessions,
    list_user_aliases_by_ids, list_users, list_visible_messages_in_area, normalize_alias,
    record_auth_failure, update_session_user, update_user_login,
};
use oxidebbs_telnet::telnet::{
    DO, IAC, SB, SE, TELOPT_ECHO, TELOPT_SUPPRESS_GO_AHEAD, TELOPT_TTYPE_SEND, WILL,
};
use oxidebbs_telnet::{
    TELOPT_NAWS, TELOPT_TERMINAL_TYPE, TcpTransport, TelnetCommand, TelnetEvent, TelnetParser,
    Transport, TransportError,
};
use oxidebbs_term::{
    LoadedScreen, ScreenAsset as TermScreenAsset, TerminalCapabilities, encode_cp437,
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
    if !config.telnet.enabled {
        info!(bind = %config.telnet.bind, "telnet disabled; service not started");
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

    let listener = TcpListener::bind(&config.telnet.bind).await?;
    insert_required_startup_audit_event(
        db.as_ref(),
        "server_start",
        None,
        None,
        format!(
            "serving {} on {} with {} node(s)",
            config.board.name, config.telnet.bind, config.nodes.count
        ),
    )?;

    info!(bind = %config.telnet.bind, "listening for telnet callers");

    let mut shutdown = Box::pin(shutdown_signal);
    let mut accept_error = None;

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
                    ip: peer_addr.ip().to_string(),
                    port: i64::from(peer_addr.port()),
                };

                if let Some(allocation) = runtime.try_allocate_node() {
                    info!(
                        node = %allocation.node_number,
                        remote = %peer.address,
                        remote_ip = %peer.ip,
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
                    let resources = CallerResources {
                        db: Arc::clone(&db),
                        config: Arc::clone(&shared_config),
                        login_menu: Arc::clone(&login_menu),
                        main_menu: Arc::clone(&main_menu),
                        menus: Arc::clone(&menus),
                        runtime: Arc::clone(&runtime),
                    };

                    tokio::spawn(async move {
                        if let Err(error) = handle_caller(allocation, stream, peer, resources).await {
                            warn!("caller session ended with error: {error}");
                        }
                    });
                } else {
                    warn!(
                        remote = %peer.address,
                        remote_ip = %peer.ip,
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
    let bytes = encode_text(REJECTION_MESSAGE);
    stream.write_all(&bytes).await?;
    stream.shutdown().await?;
    Ok(())
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

struct CallerResources {
    db: Arc<OxideDb>,
    config: Arc<OxideConfig>,
    login_menu: Arc<Menu>,
    main_menu: Arc<Menu>,
    menus: Arc<HashMap<String, Arc<Menu>>>,
    runtime: Arc<ServerRuntime>,
}

async fn handle_caller(
    allocation: NodeAllocation,
    stream: TcpStream,
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
    let mut transport = TcpTransport::new(stream);
    let mut input = InputSession::default();
    let idle_timeout = Duration::from_secs(config.telnet.idle_timeout_seconds);
    let mut authenticated_user: Option<User> = None;

    insert_session(
        db.db(),
        &SessionRecord {
            id: session_id.clone(),
            node_number,
            user_id: None,
            transport: "telnet".to_string(),
            remote_address: peer.address.clone(),
            remote_ip: Some(peer.ip.clone()),
            remote_port: Some(peer.port),
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
        remote_ip = %peer.ip,
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

    let mut capabilities = negotiate_terminal_capabilities(
        &mut transport,
        &mut input,
        TERMINAL_CAPABILITY_NEGOTIATION_TIMEOUT,
    )
    .await?;
    debug!(
        node = %node_number,
        session_id = %session_id,
        remote = %peer.address,
        remote_ip = %peer.ip,
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
    send_terminal_asset(
        &mut transport,
        &config.terminal.welcome_screen,
        &config,
        capabilities,
    )
    .await?;
    send_login_flow(&mut transport, &config, &login_menu, &mut capabilities).await?;

    let mut in_main_menu = false;
    let mut disconnect_reason = "caller_disconnected".to_string();

    loop {
        if process_runtime_commands(
            &mut transport,
            runtime.take_node_commands(node_number_u16),
            &mut disconnect_reason,
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
                if process_runtime_commands(&mut transport, commands, &mut disconnect_reason)
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
                send_text(&mut transport, "Idle timeout. Goodbye.\r\n").await?;
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

                if !in_main_menu {
                    let route = current_menu.route(&key);
                    if route.is_some()
                        && let Some(entry) = current_menu.route_entry(&key)
                        && entry.min_security_level > 0
                    {
                        send_text(&mut transport, ACCESS_DENIED_MESSAGE).await?;
                        send_menu_prompt(&mut transport, &current_menu).await?;
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
                                remote_ip: &peer.ip,
                                session_id: &session_id,
                                authenticated_user: &mut authenticated_user,
                                idle_timeout,
                                disconnect_reason: &mut disconnect_reason,
                            };
                            match run_login_flow(&mut transport, &mut input, &mut auth_state)
                                .await?
                            {
                                AuthFlowResult::Success => {
                                    if let Some(user) = authenticated_user.as_ref() {
                                        runtime.set_node_user(
                                            node_number_u16,
                                            Some(user.id.clone()),
                                            Some(user.alias.clone()),
                                        );
                                    }
                                    current_menu = Arc::clone(&main_menu);
                                    show_post_login_screens(
                                        &mut transport,
                                        &config,
                                        &mut capabilities,
                                    )
                                    .await?;
                                    send_main_menu(
                                        &mut transport,
                                        &config,
                                        &main_menu,
                                        &mut capabilities,
                                    )
                                    .await?;
                                    runtime.mark_node_main_menu(node_number_u16);
                                    in_main_menu = true;
                                }
                                AuthFlowResult::Retry => {
                                    send_menu_prompt(&mut transport, &current_menu).await?;
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
                                remote_ip: &peer.ip,
                                session_id: &session_id,
                                authenticated_user: &mut authenticated_user,
                                idle_timeout,
                                disconnect_reason: &mut disconnect_reason,
                            };
                            match run_new_user_flow(&mut transport, &mut input, &mut auth_state)
                                .await?
                            {
                                AuthFlowResult::Success => {
                                    if let Some(user) = authenticated_user.as_ref() {
                                        runtime.set_node_user(
                                            node_number_u16,
                                            Some(user.id.clone()),
                                            Some(user.alias.clone()),
                                        );
                                    }
                                    current_menu = Arc::clone(&main_menu);
                                    show_post_login_screens(
                                        &mut transport,
                                        &config,
                                        &mut capabilities,
                                    )
                                    .await?;
                                    send_main_menu(
                                        &mut transport,
                                        &config,
                                        &main_menu,
                                        &mut capabilities,
                                    )
                                    .await?;
                                    runtime.mark_node_main_menu(node_number_u16);
                                    in_main_menu = true;
                                }
                                AuthFlowResult::Retry => {
                                    send_menu_prompt(&mut transport, &current_menu).await?;
                                }
                                AuthFlowResult::Exit => break,
                            }
                        }
                        Some(MenuAction::Logoff) => {
                            debug!(node = %node_number, "caller selected login-menu logoff");
                            disconnect_reason = "caller_logoff".to_string();
                            send_logoff_screen(&mut transport, &config, capabilities).await;
                            break;
                        }
                        Some(MenuAction::Submenu { menu_id }) => {
                            debug!(node = %node_number, submenu = %menu_id, "caller selected submenu");
                            if let Some(submenu) = resolve_submenu(&menus, &menu_id) {
                                current_menu = Arc::clone(&submenu);
                                send_menu_prompt(&mut transport, &current_menu).await?;
                            } else {
                                send_text(
                                    &mut transport,
                                    "Configured submenu menu is missing.\r\n",
                                )
                                .await?;
                                send_menu_prompt(&mut transport, &current_menu).await?;
                            }
                        }
                        _ => {
                            send_text(&mut transport, "Select Login, New User, or Goodbye.\r\n")
                                .await?;
                            send_menu_prompt(&mut transport, &current_menu).await?;
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
                        send_text(&mut transport, ACCESS_DENIED_MESSAGE).await?;
                        send_menu_prompt(&mut transport, &current_menu).await?;
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
                            )
                            .await?
                            {
                                MenuFlowResult::Continue => {
                                    runtime.mark_node_main_menu(node_number_u16);
                                    send_menu_prompt(&mut transport, &current_menu).await?;
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
                            )
                            .await?
                            {
                                MenuFlowResult::Continue => {
                                    runtime.mark_node_main_menu(node_number_u16);
                                    send_menu_prompt(&mut transport, &current_menu).await?;
                                }
                                MenuFlowResult::Exit => {
                                    break;
                                }
                            }
                        }
                        Some(MenuAction::NewUser) => {
                            debug!(node = %node_number, "authenticated caller selected new-user action");
                            send_text(&mut transport, "Already signed in. Return to menu.\r\n")
                                .await?;
                            send_menu_prompt(&mut transport, &current_menu).await?;
                        }
                        Some(MenuAction::Logoff) => {
                            debug!(node = %node_number, "caller selected main-menu logoff");
                            disconnect_reason = "caller_logoff".to_string();
                            send_logoff_screen(&mut transport, &config, capabilities).await;
                            break;
                        }
                        Some(MenuAction::ShowScreen { screen }) => {
                            debug!(node = %node_number, screen = %screen.asset, "caller selected show-screen action");
                            send_screen(&mut transport, &config, &screen.asset, &mut capabilities)
                                .await?;
                            send_menu_prompt(&mut transport, &current_menu).await?;
                        }
                        Some(MenuAction::Submenu { menu_id }) => {
                            debug!(node = %node_number, submenu = %menu_id, "caller selected submenu");
                            if let Some(submenu) = resolve_submenu(&menus, &menu_id) {
                                current_menu = Arc::clone(&submenu);
                                send_menu_prompt(&mut transport, &current_menu).await?;
                            } else {
                                send_text(
                                    &mut transport,
                                    "Configured submenu menu is missing.\r\n",
                                )
                                .await?;
                                send_menu_prompt(&mut transport, &current_menu).await?;
                            }
                        }
                        Some(MenuAction::Login) => {
                            debug!(node = %node_number, "authenticated caller selected login action");
                            send_text(&mut transport, "Already signed in. Return to menu.\r\n")
                                .await?;
                            send_menu_prompt(&mut transport, &current_menu).await?;
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
                            send_text(&mut transport, "Unknown option.\r\n").await?;
                            send_menu_prompt(&mut transport, &current_menu).await?;
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

    runtime.mark_node_disconnecting(node_number_u16);
    if let Err(error) = flush_pending_replies(&mut transport, &mut input).await {
        warn!("failed to flush pending negotiation replies: {error}");
    }
    if let Err(error) = transport.hangup().await {
        warn!("failed to hang up telnet transport: {error}");
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
) -> ServeResult<TerminalCapabilities> {
    let mut capabilities = TerminalCapabilities::plain_text();
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
            capabilities.supports_ansi = terminal_type_supports_ansi(&terminal_type);
            *terminal_type_evaluated = true;
        }
        TelnetEvent::WindowSize { columns, .. } if columns > 0 => {
            capabilities.width = columns;
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

fn terminal_type_supports_ansi(terminal_type: &[u8]) -> bool {
    let terminal_type = String::from_utf8_lossy(terminal_type);
    let normalized = terminal_type.trim().to_ascii_lowercase();

    normalized.contains("syncterm")
        || normalized == "ansi"
        || normalized.contains("ansi.sys")
        || normalized.contains("ansi-bbs")
        || normalized.contains("bbs-ansi")
        || normalized == "pc-ansi"
        || normalized.contains("pcansi")
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

async fn run_login_flow(
    transport: &mut TcpTransport,
    input: &mut InputSession,
    state: &mut AuthFlowState<'_>,
) -> ServeResult<AuthFlowResult> {
    let db = state.db;
    let node_number = state.node_number;
    let session_id = state.session_id;
    let idle_timeout = state.idle_timeout;
    let disconnect_reason = &mut *state.disconnect_reason;
    let authenticated_user = &mut *state.authenticated_user;

    send_text(transport, "\r\n-- Login --\r\n").await?;

    let alias =
        match prompt_for_line(transport, input, idle_timeout, false, false, "Alias: ").await? {
            PromptLineResult::Value(value) => value,
            PromptLineResult::Disconnected => {
                *disconnect_reason = "caller_dropped_during_login".to_string();
                return Ok(AuthFlowResult::Exit);
            }
            PromptLineResult::IdleTimeout => {
                *disconnect_reason = "idle_timeout".to_string();
                send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
                return Ok(AuthFlowResult::Exit);
            }
            PromptLineResult::Rejected => {
                unreachable!("prompt_for_line handles rejected input internally");
            }
        };

    let password =
        match prompt_for_line(transport, input, idle_timeout, false, true, "Password: ").await? {
            PromptLineResult::Value(value) => value,
            PromptLineResult::Disconnected => {
                *disconnect_reason = "caller_dropped_during_login".to_string();
                return Ok(AuthFlowResult::Exit);
            }
            PromptLineResult::IdleTimeout => {
                *disconnect_reason = "idle_timeout".to_string();
                send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
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
        send_text(transport, LOGIN_LOCKOUT_MESSAGE).await?;
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
        send_text(transport, INVALID_LOGIN_MESSAGE).await?;
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
        send_text(transport, INVALID_LOGIN_MESSAGE).await?;
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
    send_text(transport, "Login successful. Welcome back.\r\n").await?;
    Ok(AuthFlowResult::Success)
}

async fn run_new_user_flow(
    transport: &mut TcpTransport,
    input: &mut InputSession,
    state: &mut AuthFlowState<'_>,
) -> ServeResult<AuthFlowResult> {
    let db = state.db;
    let node_number = state.node_number;
    let session_id = state.session_id;
    let idle_timeout = state.idle_timeout;
    let disconnect_reason = &mut *state.disconnect_reason;
    let authenticated_user = &mut *state.authenticated_user;

    send_text(transport, "\r\n-- Registration --\r\n").await?;

    let alias = match prompt_for_line(
        transport,
        input,
        idle_timeout,
        false,
        false,
        "Choose an alias: ",
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
            send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::Rejected => {
            unreachable!("prompt_for_line handles rejected input internally");
        }
    };

    let real_name =
        match prompt_for_line(transport, input, idle_timeout, false, false, "Real name: ").await? {
            PromptLineResult::Value(value) => value,
            PromptLineResult::Disconnected => {
                *disconnect_reason = "caller_dropped_during_login".to_string();
                return Ok(AuthFlowResult::Exit);
            }
            PromptLineResult::IdleTimeout => {
                *disconnect_reason = "idle_timeout".to_string();
                send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
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
            send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
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
            send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
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
            send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
            return Ok(AuthFlowResult::Exit);
        }
        PromptLineResult::Rejected => {
            unreachable!("prompt_for_line handles rejected input internally");
        }
    };

    if password != password_confirmation {
        send_text(transport, "Passwords did not match.\r\n").await?;
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
            send_text(transport, &format!("Unable to create account: {error}\r\n")).await?;
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
                send_text(transport, "That alias is already in use.\r\n").await?;
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

    send_text(transport, "Account created. Welcome.\r\n").await?;
    Ok(AuthFlowResult::Success)
}

async fn run_doors_flow(
    authenticated_user: Option<&User>,
    transport: &mut TcpTransport,
    input: &mut InputSession,
    state: &mut DoorFlowState<'_>,
) -> ServeResult<MenuFlowResult> {
    let Some(user) = authenticated_user else {
        send_text(transport, "You must be signed in to use doors.\r\n").await?;
        return Ok(MenuFlowResult::Continue);
    };

    let service = DoorService::new(state.db, state.config);
    let doors = service.list_enabled_doors()?;
    if doors.is_empty() {
        send_text(transport, "No doors are available.\r\n").await?;
        return Ok(MenuFlowResult::Continue);
    }

    loop {
        send_text(transport, &render_door_menu(&doors)).await?;
        let selected = match prompt_for_line(
            transport,
            input,
            state.idle_timeout,
            true,
            false,
            "Door key or number (blank to return): ",
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
                send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
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
                send_text(transport, "Unknown door.\r\n").await?;
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

        if user.security_level < door.min_security_level as i32 {
            debug!(
                node = %state.node_number,
                user_level = user.security_level,
                door_level = door.min_security_level,
                door_key = %door.key,
                "caller denied by door min_security_level"
            );
            send_text(transport, ACCESS_DENIED_MESSAGE).await?;
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
            )
            .await?;
            continue;
        }

        send_text(transport, &format!("\r\nLaunching {}...\r\n", door.name)).await?;
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
        send_text(transport, &door_summary_text(&summary)).await?;
        return Ok(MenuFlowResult::Continue);
    }
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
    transport: &mut TcpTransport,
    input: &mut InputSession,
    state: &mut MessageFlowState<'_>,
) -> ServeResult<MenuFlowResult> {
    let db = state.db;
    let idle_timeout = state.idle_timeout;
    let runtime = state.runtime;
    let node_number = state.node_number;
    let disconnect_reason = &mut *state.disconnect_reason;

    let Some(user) = authenticated_user else {
        send_text(transport, "You must be signed in to use messages.\r\n").await?;
        return Ok(MenuFlowResult::Continue);
    };

    ensure_default_message_area(db, transport).await?;
    let area_records = list_message_areas(db.db())?
        .into_iter()
        .filter(|area| area.enabled)
        .collect::<Vec<_>>();
    if area_records.is_empty() {
        send_text(transport, "No message areas are configured.\r\n").await?;
        return Ok(MenuFlowResult::Continue);
    }

    loop {
        send_text(transport, "\r\nMessage areas:\r\n").await?;
        for area in &area_records {
            send_text(
                transport,
                &format!("{} - {}\r\n", area.key, area.description),
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
                send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
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
                send_text(transport, "Unknown area.\r\n").await?;
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
            display_message_list(transport, db, &area, &visible).await?;

            let action = match prompt_for_line(
                transport,
                input,
                idle_timeout,
                true,
                false,
                "Read (R), Post (P), Reply (Y), Back (blank): ",
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
                    send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
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
                    display_message(transport, db, &visible[index]).await?;
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
                            send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::Rejected => {
                            unreachable!("prompt_for_line handles rejected input internally");
                        }
                    };
                    if validate_caller_cp437_text(&subject).is_err() {
                        send_text(transport, CP437_INPUT_REJECT_LINE).await?;
                        continue;
                    }

                    let body = match prompt_for_message_body(transport, input, idle_timeout).await?
                    {
                        PromptLineResult::Value(value) => value,
                        PromptLineResult::Disconnected => {
                            *disconnect_reason = "caller_dropped_during_messages".to_string();
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::IdleTimeout => {
                            *disconnect_reason = "idle_timeout".to_string();
                            send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::Rejected => {
                            unreachable!(
                                "prompt_for_message_body only returns rejection on CP437 validation"
                            );
                        }
                    };
                    if validate_caller_cp437_text(&body).is_err() {
                        send_text(transport, CP437_INPUT_REJECT_LINE).await?;
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
                            send_text(transport, &format!("Cannot post message: {error}\r\n"))
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
                    send_text(transport, "Message posted.\r\n").await?;
                    runtime.mark_node_reading_messages(node_number);
                }
                Some('Y') => {
                    if visible.is_empty() {
                        send_text(transport, "No messages to reply to.\r\n").await?;
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
                    )
                    .await?
                    {
                        MessageIndexPromptResult::Index(index) => index,
                        MessageIndexPromptResult::Retry => continue,
                        MessageIndexPromptResult::Exit => return Ok(MenuFlowResult::Exit),
                    };

                    let body = match prompt_for_message_body(transport, input, idle_timeout).await?
                    {
                        PromptLineResult::Value(value) => value,
                        PromptLineResult::Disconnected => {
                            *disconnect_reason = "caller_dropped_during_messages".to_string();
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::IdleTimeout => {
                            *disconnect_reason = "idle_timeout".to_string();
                            send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
                            return Ok(MenuFlowResult::Exit);
                        }
                        PromptLineResult::Rejected => {
                            unreachable!(
                                "prompt_for_message_body only returns rejection on CP437 validation"
                            );
                        }
                    };
                    if validate_caller_cp437_text(&body).is_err() {
                        send_text(transport, CP437_INPUT_REJECT_LINE).await?;
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
                            send_text(transport, &format!("Cannot reply: {error}\r\n")).await?;
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
                    send_text(transport, "Reply posted.\r\n").await?;
                    runtime.mark_node_reading_messages(node_number);
                }
                Some(_) => {
                    send_text(transport, "Unknown command.\r\n").await?;
                }
            }
        }
    }
}

async fn ensure_default_message_area(
    db: &OxideDb,
    transport: &mut TcpTransport,
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
        send_text(transport, "Messages are not available right now.\r\n").await?;
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
) -> ServeResult<()> {
    let author_aliases = message_author_aliases(db, messages);
    send_text(transport, &format!("\r\n{} messages:\r\n", area.name)).await?;
    if messages.is_empty() {
        send_text(transport, "No messages in this area.\r\n").await?;
        return Ok(());
    }

    for (index, message) in messages.iter().enumerate() {
        let author = author_alias_from_map(&author_aliases, &message.author_user_id);
        send_text(
            transport,
            &format!("  {}) {} (from {})\r\n", index + 1, message.subject, author),
        )
        .await?;
    }
    Ok(())
}

async fn display_message<T: Transport>(
    transport: &mut T,
    db: &OxideDb,
    message: &Message,
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
    )
    .await
}

async fn prompt_for_message_index(
    transport: &mut TcpTransport,
    input: &mut InputSession,
    idle_timeout: Duration,
    disconnect_reason: &mut String,
    message_count: usize,
    prompt: &str,
) -> ServeResult<MessageIndexPromptResult> {
    if message_count == 0 {
        send_text(transport, "No messages are available.\r\n").await?;
        return Ok(MessageIndexPromptResult::Retry);
    }

    let selected =
        match prompt_for_line(transport, input, idle_timeout, false, false, prompt).await? {
            PromptLineResult::Value(value) => value,
            PromptLineResult::Disconnected => {
                *disconnect_reason = "caller_dropped_during_messages".to_string();
                return Ok(MessageIndexPromptResult::Exit);
            }
            PromptLineResult::IdleTimeout => {
                *disconnect_reason = "idle_timeout".to_string();
                send_text(transport, "Idle timeout. Goodbye.\r\n").await?;
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
            send_text(transport, "Invalid message number.\r\n").await?;
            Ok(MessageIndexPromptResult::Retry)
        }
    }
}

async fn prompt_for_message_body<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    idle_timeout: Duration,
) -> ServeResult<PromptLineResult> {
    let mut output = Vec::new();
    write_text_buffered(
        transport,
        "Enter message body. End with a single . on its own line.\r\n",
        &mut output,
    )
    .await?;
    let mut lines = Vec::new();

    loop {
        match prompt_for_line(transport, input, idle_timeout, true, false, "> ").await? {
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
) -> ServeResult<PromptLineResult> {
    let mut output = Vec::new();
    loop {
        write_text_buffered(transport, prompt, &mut output).await?;
        match read_line_input(transport, input, idle_timeout, allow_empty, hide_input).await? {
            PromptLineResult::Rejected => {
                send_text(transport, CP437_INPUT_REJECT_LINE).await?;
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
                        write_text_buffered(transport, "\r\n", &mut output).await?;
                        break;
                    }
                    b'\x08' | b'\x7f' => {
                        if line.pop().is_some() {
                            write_text_buffered(transport, "\x08 \x08", &mut output).await?;
                        }
                    }
                    b'\t' => {}
                    raw => {
                        line.push(raw);
                        match raw {
                            raw if hide_input && (raw.is_ascii_graphic() || raw == b' ') => {
                                write_text_buffered(transport, "*", &mut output).await?
                            }
                            raw if !hide_input && (raw.is_ascii_graphic() || raw == b' ') => {
                                write_text_buffered(
                                    transport,
                                    &String::from_utf8_lossy(&[raw]),
                                    &mut output,
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

async fn send_terminal_asset(
    transport: &mut TcpTransport,
    asset_name: &str,
    config: &OxideConfig,
    capabilities: TerminalCapabilities,
) -> ServeResult<()> {
    let payload =
        load_terminal_asset_payload(config, asset_name, capabilities).unwrap_or_else(|error| {
            report_configured_asset_load_failure(
                "terminal asset",
                asset_name,
                capabilities,
                &error,
            );
            fallback_screen_payload(asset_name, &error)
        });
    transport.write_all(&payload).await?;
    Ok(())
}

async fn send_logoff_screen(
    transport: &mut TcpTransport,
    config: &OxideConfig,
    capabilities: TerminalCapabilities,
) {
    let asset_name = &config.terminal.logoff_screen;
    let payload =
        load_terminal_asset_payload(config, asset_name, capabilities).unwrap_or_else(|error| {
            warn!(
                asset = asset_name,
                supports_ansi = capabilities.supports_ansi,
                "failed to load configured logoff screen; falling back to plain goodbye: {error}"
            );
            normalize_caller_line_endings(&encode_text("Goodbye.\r\n"))
        });
    let _ = transport.write_all(&payload).await;
}

fn load_terminal_asset_payload(
    config: &OxideConfig,
    asset_name: &str,
    capabilities: TerminalCapabilities,
) -> Result<Vec<u8>, String> {
    if !capabilities.supports_ansi
        && let Some(payload) = load_plain_terminal_asset_payload(config, asset_name)?
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
        )))
    }
}

fn load_plain_terminal_asset_payload(
    config: &OxideConfig,
    asset_name: &str,
) -> Result<Option<Vec<u8>>, String> {
    for candidate in plain_terminal_asset_candidates(asset_name) {
        let asset_path = config.paths.ansi.join(&candidate);
        match std::fs::read(&asset_path) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                return Ok(Some(encode_text(&text)));
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

fn plain_terminal_asset_candidates(asset_name: &str) -> [String; 2] {
    let asset_path = Path::new(asset_name);
    [
        asset_path
            .with_extension("asc")
            .to_string_lossy()
            .into_owned(),
        asset_path
            .with_extension("txt")
            .to_string_lossy()
            .into_owned(),
    ]
}

async fn send_screen(
    transport: &mut TcpTransport,
    config: &OxideConfig,
    screen_key: &str,
    capabilities: &mut TerminalCapabilities,
) -> ServeResult<()> {
    let payload = load_screen_payload(config, screen_key, *capabilities).unwrap_or_else(|error| {
        report_configured_asset_load_failure("screen", screen_key, *capabilities, &error);
        fallback_screen_payload(screen_key, &error)
    });
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
        ascii: screen_config.ascii.clone(),
        text: screen_config.text.clone(),
        pause: screen_config.pause,
    };

    match term_screen.load(&config.paths.screens, capabilities) {
        Ok(LoadedScreen::Ansi(bytes)) => Ok(normalize_caller_line_endings(&bytes)),
        Ok(LoadedScreen::PlainText(text)) => Ok(normalize_caller_line_endings(&encode_text(&text))),
        Err(error) => Err(error.to_string()),
    }
}

fn fallback_screen_payload(screen_key: &str, details: &str) -> Vec<u8> {
    let mut message = String::new();
    let _ = writeln!(&mut message, "[{}]", screen_key);
    let _ = write!(&mut message, "{details}");
    message.push_str(PROMPT_TERMINATOR);
    normalize_caller_line_endings(&encode_text(&message))
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
) -> ServeResult<()> {
    encode_text_into(message, output);
    transport.write_all(output).await?;
    output.clear();
    Ok(())
}

async fn write_text_buffered<T: Transport>(
    transport: &mut T,
    message: &str,
    output: &mut Vec<u8>,
) -> ServeResult<()> {
    send_text_buffered(transport, message, output).await
}

async fn send_text<T: Transport>(transport: &mut T, message: &str) -> ServeResult<()> {
    let mut output = Vec::new();
    send_text_buffered(transport, message, &mut output).await?;
    Ok(())
}

async fn process_runtime_commands(
    transport: &mut TcpTransport,
    commands: RuntimeNodeCommands,
    disconnect_reason: &mut String,
) -> ServeResult<bool> {
    let mut output = Vec::new();
    for message in commands.messages {
        send_text_buffered(transport, &format!("\r\n{message}\r\n"), &mut output).await?;
    }

    if let Some(reason) = commands.disconnect_reason {
        *disconnect_reason = reason;
        send_text_buffered(transport, "\r\nDisconnected by sysop.\r\n", &mut output).await?;
        return Ok(true);
    }

    Ok(false)
}

fn encode_text(text: &str) -> Vec<u8> {
    let mut output = Vec::new();
    encode_text_into(text, &mut output);
    output
}

fn encode_text_into(text: &str, output: &mut Vec<u8>) {
    output.clear();
    if text.is_ascii() {
        output.reserve(text.len());
        output.extend_from_slice(text.as_bytes());
        return;
    }

    match encode_cp437(text) {
        Ok(bytes) => output.extend_from_slice(&bytes),
        Err(_) => encode_text_lossy_into(text, output),
    }
}

fn encode_text_lossy_into(text: &str, output: &mut Vec<u8>) {
    output.clear();
    output.reserve(text.len());
    for character in text.chars() {
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

fn parse_next_event(
    input: &mut InputSession,
    reply: &mut Vec<u8>,
    byte: u8,
) -> Option<TelnetEvent> {
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
}

struct CallerPeer {
    address: String,
    ip: String,
    port: i64,
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
    use std::net::SocketAddr;
    use std::path::{Path, PathBuf};
    use std::time::{Duration as TestDuration, Instant, SystemTime, UNIX_EPOCH};

    use oxidebbs_db::insert_user;
    use oxidebbs_telnet::{
        LoopbackTransport,
        telnet::{
            DO, IAC, SB, SE, TELOPT_ECHO, TELOPT_NAWS, TELOPT_SUPPRESS_GO_AHEAD,
            TELOPT_TERMINAL_TYPE, TELOPT_TTYPE_IS, WILL,
        },
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::oneshot;

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
        assert!(terminal_type_supports_ansi(b"SyncTERM"));
        assert!(terminal_type_supports_ansi(b"ANSI"));
        assert!(terminal_type_supports_ansi(b"ANSI-BBS"));
        assert!(terminal_type_supports_ansi(b"BBS-ANSI"));
        assert!(terminal_type_supports_ansi(b"ANSI.SYS"));
        assert!(terminal_type_supports_ansi(b"PC-ANSI"));
        assert!(terminal_type_supports_ansi(b"pcansi"));
        assert!(!terminal_type_supports_ansi(b"xterm-256color"));
        assert!(!terminal_type_supports_ansi(b"vt100"));
    }

    #[tokio::test]
    async fn capability_negotiation_defaults_to_plain_text_without_response() {
        let (mut transport, mut client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        let capabilities =
            negotiate_terminal_capabilities(&mut transport, &mut input, Duration::from_millis(5))
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

        let capabilities =
            negotiate_terminal_capabilities(&mut transport, &mut input, Duration::from_millis(20))
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

        let capabilities =
            negotiate_terminal_capabilities(&mut transport, &mut input, Duration::from_millis(20))
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

        let capabilities =
            negotiate_terminal_capabilities(&mut transport, &mut input, Duration::from_millis(20))
                .await
                .expect("negotiate capabilities");
        let payload = load_screen_payload(&config, &config.flow.login_screen, capabilities)
            .expect("load login screen");

        assert!(!capabilities.supports_ansi);
        assert_eq!(capabilities.width, 40);
        assert_eq!(payload, b"ASCII\r\n");

        let _ = std::fs::remove_dir_all(base_dir);
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
        let payload = fallback_screen_payload("login", "missing file");
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
                ascii: Some("line-endings/screen.asc".to_string()),
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

        encode_text_into("Main menu? ", &mut output);

        assert_eq!(output, b"Main menu? ");
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

        let hash = server_hash_password("secret 🚀", &config).expect("hash password");

        assert_eq!(
            verify_stored_password("secret 🚀", &hash, &config).expect("verify"),
            PasswordVerification::Accepted
        );
    }

    #[test]
    fn cp437_box_drawing_output_still_encodes() {
        let text = "┌─┐";

        assert_eq!(
            encode_text(text),
            encode_cp437(text).expect("box drawing is CP437-compatible")
        );
    }

    #[test]
    fn generated_output_replaces_unencodable_text_with_question_mark() {
        assert_eq!(encode_text("Diagnostic 🚀"), b"Diagnostic ?");
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
        client.write_all(b"secret\r").await.expect("password");
        read_until(&mut client, "Confirm password: ").await;
        client
            .write_all(b"secret\r")
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
        )
        .await
        .expect("read");

        assert!(matches!(value, PromptLineResult::Rejected));
    }

    #[tokio::test]
    async fn read_line_input_ignores_cp437_policy_for_hidden_input() {
        let (mut transport, client) = LoopbackTransport::new();
        let mut input = InputSession::default();

        client
            .write_bytes("secret 🚀\r".as_bytes())
            .expect("write value");

        let value = read_line_input(
            &mut transport,
            &mut input,
            Duration::from_secs(1),
            false,
            true,
        )
        .await
        .expect("read");

        match value {
            PromptLineResult::Value(value) => assert_eq!(value, "secret 🚀"),
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

        let value = prompt_for_message_body(&mut transport, &mut input, Duration::from_secs(1))
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
        seed_login_user(&db, &config, "Alice", "secret");

        let missing = run_login_subflow(&db, &config, "Nobody", "bad", "127.0.0.1")
            .await
            .1;
        let wrong = run_login_subflow(&db, &config, "Alice", "bad", "127.0.0.2")
            .await
            .1;

        assert!(missing.contains(INVALID_LOGIN_MESSAGE.trim()));
        assert!(wrong.contains(INVALID_LOGIN_MESSAGE.trim()));
        assert_eq!(failure_line(&missing), failure_line(&wrong));

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
        seed_login_user(&db, &config, "Alice", "secret");

        let (result, output) =
            run_new_user_subflow(&db, &config, "alice", "Alice Clone", "secret").await;

        assert!(matches!(result, AuthFlowResult::Retry));
        assert!(output.contains("That alias is already in use."));

        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn rate_limiter_bounds_login_failure_audit_writes() {
        let db = OxideDb::open_memory().expect("open db");
        let base_dir = temp_dir("auth-audit-bound");
        let config = smoke_config(free_loopback_addr(), &base_dir, &base_dir.join("auth.ddb"));

        let mut last_output = String::new();
        for _ in 0..6 {
            last_output = run_login_subflow(&db, &config, "Nobody", "bad", "127.0.0.1")
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
        seed_login_user(&db, &config, "Alice", "secret");
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
            run_login_subflow(&db, &config, "Alice", "secret", "127.0.0.1").await;

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
        let hash = server_hash_password("secret", &config).expect("hash password");

        assert!(hash.starts_with("$argon2id$"));
        assert_eq!(
            verify_stored_password("secret", &hash, &config).expect("verify"),
            PasswordVerification::Accepted
        );
        assert_eq!(
            verify_stored_password("wrong", &hash, &config).expect("verify"),
            PasswordVerification::Rejected
        );
    }

    #[test]
    fn invalid_password_hash_runs_dummy_verify_and_fails_closed() {
        let config = Argon2Config::default();

        let result = verify_stored_password("secret", "not-a-phc", &config).expect("verify");

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

        display_message_list(&mut transport, &db, &area, &messages)
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

        display_message_list(&mut transport, &db, &area, &messages)
            .await
            .expect("display");
        let output = String::from_utf8_lossy(&client.read_output_bytes()).to_string();

        assert!(output.contains("1) Missing (from Unknown)"));
    }

    fn seed_login_user(db: &OxideDb, config: &OxideConfig, alias: &str, password: &str) {
        let now = current_timestamp(db).expect("timestamp");
        let user = UserRecord {
            id: generated_uuid(db).expect("uuid"),
            alias: alias.to_string(),
            real_name: format!("{alias} User"),
            email: None,
            password_hash: server_hash_password(password, &config.auth.argon2).expect("hash"),
            security_level: 10,
            is_sysop: false,
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
        let result = run_login_flow(&mut transport, &mut input, &mut state)
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
        let result = run_new_user_flow(&mut transport, &mut input, &mut state)
            .await
            .expect("new user flow");
        let output = client_task.await.expect("client task");
        (result, output)
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
                ascii: Some("login/login.asc".to_string()),
                text: Some("login/login.txt".to_string()),
                pause: false,
            },
        );

        let login_dir = config.paths.screens.join("login");
        std::fs::create_dir_all(&login_dir).expect("create login screen dir");
        std::fs::write(login_dir.join("login.ans"), b"ANSI80\r\n").expect("write 80-col ANSI");
        std::fs::write(login_dir.join("login-40.ans"), b"ANSI40\r\n").expect("write 40-col ANSI");
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
