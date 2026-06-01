use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use argon2::password_hash::{PasswordHash, PasswordHasher, SaltString};
use argon2::{Argon2, PasswordVerifier as Argon2PasswordVerifier};
use rand_core::OsRng;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;
use tracing::{error, info, warn};

use oxidebbs_core::auth::{
    LoginAttempt, NewUserInput, PasswordVerifier as CorePasswordVerifier, create_new_user,
    login_user,
};
use oxidebbs_core::menu::{Menu, MenuAction};
use oxidebbs_core::message::{
    AreaKind, Message, MessageArea, MessageVisibility, PostMessageCommand, ReplyMessageCommand,
    post_message, readable_messages, reply_message,
};
use oxidebbs_core::user::{User, UserStatus};
use oxidebbs_db::{
    AuditEventRecord, MessageAreaRecord, MessageRecord, OxideDb, SessionRecord, UserRecord,
    end_session, find_message_area_by_key, find_user_by_alias_ci, find_user_by_id,
    insert_audit_event, insert_message, insert_message_area, insert_session, insert_user,
    list_message_areas, list_messages_in_area, update_session_user, update_user_login,
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
const DOORS_PLACEHOLDER: &str = "Doors feature placeholder: not implemented yet.\r\n";

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
    let mut input = InputSession::default();
    let mut capabilities = TerminalCapabilities::ansi_80();
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
        let event = next_event(&mut transport, &mut input, idle_timeout).await;
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
                drain_line_ending_after_menu_key(&mut transport, &mut input).await?;

                if !in_main_menu {
                    match login_menu.route(&key) {
                        Some(MenuAction::Login) => {
                            let mut auth_state = AuthFlowState {
                                db: db.as_ref(),
                                node_number,
                                session_id: &session_id,
                                authenticated_user: &mut authenticated_user,
                                idle_timeout,
                                disconnect_reason: &mut disconnect_reason,
                            };
                            match run_login_flow(&mut transport, &mut input, &mut auth_state)
                                .await?
                            {
                                AuthFlowResult::Success => {
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
                                    in_main_menu = true;
                                }
                                AuthFlowResult::Retry => {
                                    send_menu_prompt(&mut transport, &login_menu).await?;
                                }
                                AuthFlowResult::Exit => break,
                            }
                        }
                        Some(MenuAction::NewUser) => {
                            let mut auth_state = AuthFlowState {
                                db: db.as_ref(),
                                node_number,
                                session_id: &session_id,
                                authenticated_user: &mut authenticated_user,
                                idle_timeout,
                                disconnect_reason: &mut disconnect_reason,
                            };
                            match run_new_user_flow(&mut transport, &mut input, &mut auth_state)
                                .await?
                            {
                                AuthFlowResult::Success => {
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
                                    in_main_menu = true;
                                }
                                AuthFlowResult::Retry => {
                                    send_menu_prompt(&mut transport, &login_menu).await?;
                                }
                                AuthFlowResult::Exit => break,
                            }
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
                            match run_messages_flow(
                                authenticated_user.as_ref(),
                                &mut transport,
                                &mut input,
                                &db,
                                idle_timeout,
                                &mut disconnect_reason,
                            )
                            .await?
                            {
                                MenuFlowResult::Continue => {
                                    send_menu_prompt(&mut transport, &main_menu).await?;
                                }
                                MenuFlowResult::Exit => {
                                    break;
                                }
                            }
                        }
                        Some(MenuAction::NewUser) => {
                            send_text(&mut transport, "Already signed in. Return to menu.\r\n")
                                .await?;
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
                            send_text(&mut transport, "Already signed in. Return to menu.\r\n")
                                .await?;
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
            user_id: authenticated_user.as_ref().map(|user| user.id.clone()),
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
    Disconnected,
    IdleTimeout,
}

struct AuthFlowState<'a> {
    db: &'a OxideDb,
    node_number: i64,
    session_id: &'a str,
    authenticated_user: &'a mut Option<User>,
    idle_timeout: Duration,
    disconnect_reason: &'a mut String,
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
        };

    let login_at = current_timestamp(db)?;
    let user_record = match find_user_by_alias_ci(db.db(), &alias)? {
        Some(record) => record,
        None => {
            let event_error = format!("login failed for alias {alias}");
            if let Err(error) = insert_audit_event(
                db.db(),
                &AuditEventRecord {
                    id: generated_uuid(db)?,
                    created_at: login_at,
                    event_type: "login_failure".to_string(),
                    user_id: None,
                    node_number: Some(node_number),
                    details: event_error.clone(),
                },
            ) {
                warn!("failed to insert login_failure event: {error}");
            }

            send_text(
                transport,
                "Invalid alias or password. Please try again.\r\n",
            )
            .await?;
            return Ok(AuthFlowResult::Retry);
        }
    };

    let user = user_from_record(&user_record)?;
    let attempt = LoginAttempt {
        alias,
        password,
        login_at: login_at.clone(),
    };
    let user = match login_user(&user, &attempt, &ServerPasswordVerifier) {
        Ok(success) => success.user,
        Err(error) => {
            let event_error = format!("login failed for user {}: {error}", user.alias);
            if let Err(error) = insert_audit_event(
                db.db(),
                &AuditEventRecord {
                    id: generated_uuid(db)?,
                    created_at: login_at,
                    event_type: "login_failure".to_string(),
                    user_id: Some(user.id.clone()),
                    node_number: Some(node_number),
                    details: event_error,
                },
            ) {
                warn!("failed to insert login_failure event: {error}");
            }

            send_text(
                transport,
                "Invalid alias or password. Please try again.\r\n",
            )
            .await?;
            return Ok(AuthFlowResult::Retry);
        }
    };

    if let Err(error) = update_user_login(db.db(), &user.id, &login_at) {
        warn!(
            "failed to update user login counters for {}: {error}",
            user.alias
        );
    }

    if let Err(error) = update_session_user(db.db(), session_id, &user.id) {
        warn!(
            "failed to associate user {} with session {}: {error}",
            user.alias, session_id
        );
    }

    if let Err(error) = insert_audit_event(
        db.db(),
        &AuditEventRecord {
            id: generated_uuid(db)?,
            created_at: login_at,
            event_type: "login_success".to_string(),
            user_id: Some(user.id.clone()),
            node_number: Some(node_number),
            details: format!("login successful for {}", user.alias),
        },
    ) {
        warn!("failed to insert login_success event: {error}");
    }

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

    send_text(transport, "\r\n-- New User Registration --\r\n").await?;

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
    };

    if find_user_by_alias_ci(db.db(), &alias)?.is_some() {
        send_text(transport, "That alias is already in use.\r\n").await?;
        return Ok(AuthFlowResult::Retry);
    }

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
    };

    if password != password_confirmation {
        send_text(transport, "Passwords did not match.\r\n").await?;
        return Ok(AuthFlowResult::Retry);
    }

    let created_at = current_timestamp(db)?;
    let password_hash = server_hash_password(&password)?;
    let user = match create_new_user(NewUserInput {
        id: generated_uuid(db)?,
        alias,
        real_name,
        email,
        password_hash,
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
    insert_user(db.db(), &record)?;

    if let Err(error) = update_user_login(db.db(), &user.id, &created_at) {
        warn!(
            "failed to update new user login counters for {}: {error}",
            user.alias
        );
    } else {
        user.last_login_at = Some(created_at.clone());
        user.total_calls += 1;
    }

    if let Err(error) = update_session_user(db.db(), session_id, &user.id) {
        warn!(
            "failed to associate user {} with session {}: {error}",
            user.alias, session_id
        );
    }

    *authenticated_user = Some(user.clone());

    if let Err(error) = insert_audit_event(
        db.db(),
        &AuditEventRecord {
            id: generated_uuid(db)?,
            created_at: created_at.clone(),
            event_type: "new_user_created".to_string(),
            user_id: Some(user.id.clone()),
            node_number: Some(node_number),
            details: format!("new user created for {}", user.alias),
        },
    ) {
        warn!("failed to insert new_user_created event: {error}");
    }

    if let Err(error) = insert_audit_event(
        db.db(),
        &AuditEventRecord {
            id: generated_uuid(db)?,
            created_at,
            event_type: "login_success".to_string(),
            user_id: Some(user.id.clone()),
            node_number: Some(node_number),
            details: format!("new user logged in as {}", user.alias),
        },
    ) {
        warn!("failed to insert new user login_success event: {error}");
    }

    send_text(transport, "Account created. Welcome.\r\n").await?;
    Ok(AuthFlowResult::Success)
}

async fn run_messages_flow(
    authenticated_user: Option<&User>,
    transport: &mut TcpTransport,
    input: &mut InputSession,
    db: &OxideDb,
    idle_timeout: Duration,
    disconnect_reason: &mut String,
) -> ServeResult<MenuFlowResult> {
    let Some(user) = authenticated_user else {
        send_text(transport, "You must be signed in to use messages.\r\n").await?;
        return Ok(MenuFlowResult::Continue);
    };

    ensure_default_message_area(db, transport).await?;

    loop {
        let area_records = list_message_areas(db.db())?;
        if area_records.is_empty() {
            send_text(transport, "No message areas are configured.\r\n").await?;
            return Ok(MenuFlowResult::Continue);
        }

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
        };

        if selected_area_key.is_empty() {
            return Ok(MenuFlowResult::Continue);
        }

        let area_record = match find_message_area_by_key(db.db(), &selected_area_key)? {
            Some(area) => area,
            None => {
                send_text(transport, "Unknown area.\r\n").await?;
                continue;
            }
        };
        let area = message_area_from_record(&area_record)?;

        loop {
            let visible =
                visible_messages_for_user(db, &area, user.security_level, transport).await?;
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
                    display_message(transport, db, &visible[index]).await?;
                }
                Some('P') => {
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
                    };

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
                    insert_message(db.db(), &message_record_from_message(&message))?;
                    send_text(transport, "Message posted.\r\n").await?;
                }
                Some('Y') => {
                    if visible.is_empty() {
                        send_text(transport, "No messages to reply to.\r\n").await?;
                        continue;
                    }

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
                    };

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
                    insert_message(db.db(), &message_record_from_message(&message))?;
                    send_text(transport, "Reply posted.\r\n").await?;
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
        warn!("failed to seed default message area: {error}");
        send_text(transport, "Messages are not available right now.\r\n").await?;
    }
    Ok(())
}

async fn visible_messages_for_user(
    db: &OxideDb,
    area: &MessageArea,
    security_level: i32,
    transport: &mut TcpTransport,
) -> ServeResult<Vec<Message>> {
    let records = list_messages_in_area(db.db(), &area.id)?;
    let messages = messages_from_records(&records);
    match readable_messages(area, &messages, security_level) {
        Ok(messages) => Ok(messages.into_iter().cloned().collect()),
        Err(error) => {
            send_text(transport, &format!("Unable to read messages: {error}\r\n")).await?;
            Ok(Vec::new())
        }
    }
}

async fn display_message_list(
    transport: &mut TcpTransport,
    db: &OxideDb,
    area: &MessageArea,
    messages: &[Message],
) -> ServeResult<()> {
    send_text(transport, &format!("\r\n{} messages:\r\n", area.name)).await?;
    if messages.is_empty() {
        send_text(transport, "No messages in this area.\r\n").await?;
        return Ok(());
    }

    for (index, message) in messages.iter().enumerate() {
        let author = message_author_alias(db, &message.author_user_id);
        send_text(
            transport,
            &format!("  {}) {} (from {})\r\n", index + 1, message.subject, author),
        )
        .await?;
    }
    Ok(())
}

async fn display_message(
    transport: &mut TcpTransport,
    db: &OxideDb,
    message: &Message,
) -> ServeResult<()> {
    let author = message_author_alias(db, &message.author_user_id);
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
    write_text(
        transport,
        "Enter message body. End with a single . on its own line.\r\n",
    )
    .await?;
    let mut lines = Vec::new();

    loop {
        match prompt_for_line(transport, input, idle_timeout, true, false, "> ").await? {
            PromptLineResult::Value(value) if value.trim() == "." => break,
            PromptLineResult::Value(value) => lines.push(value),
            PromptLineResult::Disconnected => return Ok(PromptLineResult::Disconnected),
            PromptLineResult::IdleTimeout => return Ok(PromptLineResult::IdleTimeout),
        }
    }

    Ok(PromptLineResult::Value(lines.join("\r\n")))
}

fn message_author_alias(db: &OxideDb, user_id: &str) -> String {
    match find_user_by_id(db.db(), user_id) {
        Ok(Some(author)) if !author.alias.is_empty() => author.alias,
        _ => "Unknown".to_string(),
    }
}

fn message_record_from_message(message: &Message) -> MessageRecord {
    MessageRecord {
        id: message.id.clone(),
        area_id: message.area_id.clone(),
        author_user_id: message.author_user_id.clone(),
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
    write_text(transport, prompt).await?;
    read_line_input(transport, input, idle_timeout, allow_empty, hide_input).await
}

async fn read_line_input<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    idle_timeout: Duration,
    allow_empty: bool,
    hide_input: bool,
) -> ServeResult<PromptLineResult> {
    let mut line = Vec::new();

    loop {
        let event = next_event(transport, input, idle_timeout).await?;
        match event {
            CallerInput::Disconnected => return Ok(PromptLineResult::Disconnected),
            CallerInput::IdleTimeout => return Ok(PromptLineResult::IdleTimeout),
            CallerInput::Event(event) => match event {
                TelnetEvent::Data(raw) => match raw {
                    b'\n' if line.is_empty() => {}
                    b'\r' if line.is_empty() && !allow_empty => {}
                    b'\r' | b'\n' => {
                        write_text(transport, "\r\n").await?;
                        break;
                    }
                    b'\x08' | b'\x7f' => {
                        if line.pop().is_some() {
                            write_text(transport, "\x08 \x08").await?;
                        }
                    }
                    b'\t' => {}
                    raw if raw.is_ascii_graphic() || raw == b' ' => {
                        line.push(raw);
                        if hide_input {
                            write_text(transport, "*").await?;
                        } else {
                            write_text(transport, &String::from_utf8_lossy(&[raw])).await?;
                        }
                    }
                    _ => {}
                },
                TelnetEvent::Negotiation { .. }
                | TelnetEvent::WindowSize { .. }
                | TelnetEvent::TerminalType(_)
                | TelnetEvent::TerminalTypeRequest
                | TelnetEvent::Subnegotiation { .. } => {}
            },
        }
    }

    Ok(PromptLineResult::Value(
        String::from_utf8_lossy(&line).to_string(),
    ))
}

async fn write_text<T: Transport>(transport: &mut T, message: &str) -> ServeResult<()> {
    transport.write_all(&encode_text(message)).await?;
    Ok(())
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

fn server_hash_password(password: &str) -> ServeResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| ServeError::Runtime(format!("password hashing failed: {error}")))?;
    Ok(password_hash.to_string())
}

struct ServerPasswordVerifier;

impl CorePasswordVerifier for ServerPasswordVerifier {
    fn verify(&self, password: &str, password_hash: &str) -> bool {
        let Ok(parsed_hash) = PasswordHash::new(password_hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    }
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

async fn drain_line_ending_after_menu_key<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
) -> ServeResult<()> {
    for _ in 0..2 {
        let immediate = timeout(
            Duration::from_millis(5),
            next_event(transport, input, Duration::from_secs(1)),
        )
        .await;

        match immediate {
            Ok(Ok(CallerInput::Event(TelnetEvent::Data(b'\r' | b'\n')))) => {}
            Ok(Ok(other)) => {
                input.pending_inputs.push_front(other);
                break;
            }
            Ok(Err(error)) => return Err(error),
            Err(_) => break,
        }
    }

    Ok(())
}

async fn next_event<T: Transport>(
    transport: &mut T,
    input: &mut InputSession,
    idle_timeout: Duration,
) -> ServeResult<CallerInput> {
    if let Some(pending) = input.pending_inputs.pop_front() {
        return Ok(pending);
    }

    loop {
        let read = timeout(idle_timeout, transport.read_byte()).await;
        let byte = match read {
            Ok(Ok(Some(byte))) => byte,
            Ok(Ok(None)) => return Ok(CallerInput::Disconnected),
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => return Ok(CallerInput::IdleTimeout),
        };

        let mut reply = Vec::new();
        let event = input.parser.feed(byte, &mut reply);
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

#[derive(Default)]
struct InputSession {
    parser: TelnetParser,
    pending_inputs: VecDeque<CallerInput>,
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

    #[test]
    fn server_password_hashes_verify_with_argon2() {
        let hash = server_hash_password("secret").expect("hash password");
        let verifier = ServerPasswordVerifier;

        assert!(hash.starts_with("$argon2id$"));
        assert!(verifier.verify("secret", &hash));
        assert!(!verifier.verify("wrong", &hash));
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
}
