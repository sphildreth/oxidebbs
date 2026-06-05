use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use oxidebbs_telnet::Transport;
use oxidebbs_telnet::TransportError;
use tracing::warn;

use crate::config::OxideConfig;
use crate::serve::{CallerPeer, CallerResources, handle_raw_caller_transport};

const BUSY_MESSAGE: &str = "System busy. Try again later.";

#[derive(Clone)]
pub(crate) struct WebTerminalState {
    pub(crate) config: Arc<OxideConfig>,
    pub(crate) _db: Arc<oxidebbs_db::OxideDb>,
    pub(crate) runtime: Arc<crate::control::ServerRuntime>,
    pub(crate) caller_resources: CallerResources,
}

pub(crate) fn web_terminal_router(state: WebTerminalState) -> Router {
    Router::new()
        .route("/terminal", get(terminal_page))
        .route("/terminal/", get(terminal_page))
        .route("/terminal/styles.css", get(terminal_styles))
        .route("/terminal/main.js", get(terminal_script))
        .route("/terminal/zmodem.js", get(terminal_zmodem_script))
        .route("/terminal/ws", get(ws_handler))
        .with_state(state)
}

async fn terminal_page() -> Html<&'static str> {
    Html(include_str!("../../../web-terminal/index.html"))
}

async fn terminal_styles() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../../web-terminal/styles.css"),
    )
        .into_response()
}

async fn terminal_script() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../../../web-terminal/src/main.js"),
    )
        .into_response()
}

async fn terminal_zmodem_script() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../../../web-terminal/zmodem.js"),
    )
        .into_response()
}

fn same_request_origin(config: &OxideConfig, headers: &HeaderMap, origin: &str) -> bool {
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let scheme = if config.admin_web.behind_reverse_proxy {
        headers
            .get("x-forwarded-proto")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("http")
    } else {
        "http"
    };

    origin.eq_ignore_ascii_case(&format!("{scheme}://{host}"))
}

fn ensure_terminal_origin_allowed(
    config: &OxideConfig,
    headers: &HeaderMap,
) -> Result<(), StatusCode> {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        return Ok(());
    };

    if config
        .admin_web
        .allowed_origins
        .iter()
        .any(|allowed| allowed == origin)
        || same_request_origin(config, headers, origin)
    {
        return Ok(());
    }

    Err(StatusCode::FORBIDDEN)
}

fn forwarder_peer(
    config: &OxideConfig,
    headers: &HeaderMap,
    socket_addr: &SocketAddr,
) -> CallerPeer {
    if !config.admin_web.behind_reverse_proxy {
        return socket_peer(socket_addr);
    }

    let Some(forwarded_for) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
    else {
        return socket_peer(socket_addr);
    };

    let forwarded_for = forwarded_for
        .split(',')
        .map(str::trim)
        .find(|value| !value.is_empty());
    let Some(forwarded_for) = forwarded_for else {
        return socket_peer(socket_addr);
    };

    if let Ok(socket_addr) = forwarded_for.parse::<SocketAddr>() {
        return CallerPeer {
            address: socket_addr.to_string(),
            ip: Some(socket_addr.ip().to_string()),
            port: i64::from(socket_addr.port()),
        };
    }

    if let Ok(ip_addr) = forwarded_for.parse::<std::net::IpAddr>() {
        return CallerPeer {
            address: ip_addr.to_string(),
            ip: Some(ip_addr.to_string()),
            port: 0,
        };
    }

    socket_peer(socket_addr)
}

fn socket_peer(socket_addr: &SocketAddr) -> CallerPeer {
    CallerPeer {
        address: socket_addr.to_string(),
        ip: Some(socket_addr.ip().to_string()),
        port: i64::from(socket_addr.port()),
    }
}

#[derive(Debug)]
struct WsTransport {
    socket: WebSocket,
    input_buffer: VecDeque<u8>,
}

impl WsTransport {
    fn new(socket: WebSocket) -> Self {
        Self {
            socket,
            input_buffer: VecDeque::new(),
        }
    }
}

impl Transport for WsTransport {
    async fn read_byte(&mut self) -> Result<Option<u8>, TransportError> {
        if let Some(byte) = self.input_buffer.pop_front() {
            return Ok(Some(byte));
        }

        loop {
            match self.socket.recv().await {
                Some(Ok(Message::Binary(bytes))) => {
                    self.input_buffer.extend(bytes.iter().copied());
                    if let Some(byte) = self.input_buffer.pop_front() {
                        return Ok(Some(byte));
                    }
                }
                Some(Ok(Message::Text(_))) => continue,
                Some(Ok(Message::Close(_))) | None => return Ok(None),
                Some(Ok(_)) => continue,
                Some(Err(_)) => return Err(TransportError::Closed),
            }
        }
    }

    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.socket
            .send(Message::Binary(bytes.to_vec().into()))
            .await
            .map_err(|_| TransportError::Closed)?;
        Ok(())
    }

    async fn hangup(&mut self) -> Result<(), TransportError> {
        self.socket
            .send(Message::Close(None))
            .await
            .map_err(|_| TransportError::Closed)?;
        Ok(())
    }
}

async fn ws_handler(
    State(state): State<WebTerminalState>,
    headers: HeaderMap,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> Response {
    if let Err(status) = ensure_terminal_origin_allowed(&state.config, &headers) {
        return status.into_response();
    }

    let peer = forwarder_peer(&state.config, &headers, &remote_addr);

    let Some(allocation) = state.runtime.try_allocate_node() else {
        let message = BUSY_MESSAGE.as_bytes().to_vec();
        return ws
            .on_upgrade(|socket| async move {
                let mut transport = WsTransport::new(socket);
                if let Err(error) = transport.write_all(&message).await {
                    warn!(%error, "failed to write websocket busy message");
                }
                if let Err(error) = transport.hangup().await {
                    warn!(%error, "failed to close busy websocket");
                }
            })
            .into_response();
    };

    let resources = state.caller_resources;
    ws.on_upgrade(move |socket| async move {
        if let Err(error) = handle_raw_caller_transport(
            allocation,
            WsTransport::new(socket),
            "websocket",
            peer,
            resources,
        )
        .await
        {
            warn!(%error, "web websocket caller session ended with error");
        }
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::routing::get;
    use futures_util::SinkExt;
    use futures_util::StreamExt;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};

    use crate::control::ServerRuntime;
    use crate::serve::{CallerResources, caller_resources};
    use oxidebbs_db::list_recent_sessions;
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::time::{sleep, timeout};

    fn test_state() -> WebTerminalState {
        let mut config: crate::config::OxideConfig =
            toml::from_str(include_str!("../../../config/oxidebbs.example.toml"))
                .expect("parse example config");
        config.admin_web.enabled = true;
        config.admin_web.behind_reverse_proxy = false;
        config.web_terminal.enabled = true;
        config.database.path = ":memory:".into();

        let db = Arc::new(oxidebbs_db::OxideDb::open_memory().expect("open memory db"));
        let runtime = Arc::new(ServerRuntime::new(
            config.board.name.clone(),
            1,
            config.telnet.max_connections,
            config.telnet.idle_timeout_seconds,
        ));
        let caller_resources = caller_resources_from_config(&config, &db, &runtime);

        WebTerminalState {
            config: Arc::new(config),
            _db: db,
            runtime,
            caller_resources,
        }
    }

    fn caller_resources_from_config(
        config: &crate::config::OxideConfig,
        db: &Arc<oxidebbs_db::OxideDb>,
        runtime: &Arc<ServerRuntime>,
    ) -> CallerResources {
        let mut menus = HashMap::new();
        for menu_id in config.menus.keys() {
            menus.insert(
                menu_id.clone(),
                Arc::new(config.core_menu(menu_id).expect("default config menu")),
            );
        }
        let login_menu = menus
            .get(&config.flow.login_menu)
            .expect("default login menu")
            .clone();
        let main_menu = menus
            .get(&config.flow.main_menu)
            .expect("default main menu")
            .clone();
        caller_resources(
            Arc::clone(db),
            Arc::new(config.clone()),
            login_menu,
            main_menu,
            Arc::new(menus),
            Arc::clone(runtime),
        )
    }

    #[tokio::test]
    async fn terminal_page_is_full_tab_shell() {
        let response = terminal_page().await.into_response();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read html");
        let body = String::from_utf8(body.to_vec()).expect("html text");
        assert!(body.contains("id=\"terminal\""));
        assert!(body.contains("/terminal/styles.css"));
        assert!(body.contains("/terminal/main.js"));
        assert!(body.contains("/terminal/zmodem.js"));
    }

    #[tokio::test]
    async fn ws_transport_reads_binary_frames_bytewise() {
        let app = Router::new().route(
            "/ws",
            get(|ws: WebSocketUpgrade| async move {
                ws.on_upgrade(|socket| async move {
                    let mut transport = WsTransport::new(socket);
                    let mut bytes = Vec::new();
                    while bytes.len() < 4 {
                        match transport.read_byte().await.expect("read byte") {
                            Some(byte) => bytes.push(byte),
                            None => break,
                        }
                    }
                    transport.write_all(&bytes).await.expect("write response");
                })
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let local_addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });

        let (mut socket, _) = connect_async(format!("ws://{}/ws", local_addr))
            .await
            .expect("connect websocket");

        socket
            .send(WsMessage::Binary(vec![1u8, 2].into()))
            .await
            .expect("send first binary frame");
        socket
            .send(WsMessage::Text("x".into()))
            .await
            .expect("ignore text frame");
        socket
            .send(WsMessage::Binary(vec![3u8, 4].into()))
            .await
            .expect("send second binary frame");

        let message = socket
            .next()
            .await
            .expect("server reply")
            .expect("reply message");
        match message {
            WsMessage::Binary(bytes) => assert_eq!(&bytes[..], &[1, 2, 3, 4]),
            _ => panic!("expected binary output"),
        }

        let _ = socket.close(None).await;
        server.abort();
    }

    #[tokio::test]
    async fn ws_transport_writes_binary_frames() {
        let app = Router::new().route(
            "/ws",
            get(|ws: WebSocketUpgrade| async move {
                ws.on_upgrade(|socket| async move {
                    let mut transport = WsTransport::new(socket);
                    if let Ok(Some(4)) = transport.read_byte().await {
                        transport
                            .write_all(b"x")
                            .await
                            .expect("write transport bytes");
                    }
                })
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let local_addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });

        let (mut socket, _) = connect_async(format!("ws://{}/ws", local_addr))
            .await
            .expect("connect websocket");
        socket
            .send(WsMessage::Binary(vec![4].into()))
            .await
            .expect("send byte");

        let message = socket.next().await.expect("server reply").expect("reply");
        match message {
            WsMessage::Binary(_) => {}
            WsMessage::Text(text) => panic!("expected binary reply got text: {text}"),
            _ => panic!("expected binary output"),
        }

        let _ = socket.close(None).await;
        server.abort();
    }

    #[tokio::test]
    async fn ws_origin_check_rejects_bad_origin() {
        let mut config = test_state().config.as_ref().clone();
        config.admin_web.allowed_origins = vec!["https://example.com".to_string()];

        let mut headers = HeaderMap::new();
        headers.insert(
            header::ORIGIN,
            "https://evil.example".parse().expect("origin header"),
        );
        let result = ensure_terminal_origin_allowed(&config, &headers);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn ws_handler_rejects_when_no_node_available() {
        let state = test_state();
        let busy_allocation = state
            .runtime
            .try_allocate_node()
            .expect("allocate first slot");
        let app = web_terminal_router(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let local_addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });

        let (mut socket, _) = connect_async(format!("ws://{}/terminal/ws", local_addr))
            .await
            .expect("connect busy websocket");
        let first = socket
            .next()
            .await
            .expect("busy response")
            .expect("message from server");
        assert!(matches!(first, WsMessage::Binary(_)));
        let payload = match first {
            WsMessage::Binary(bytes) => bytes.to_vec(),
            _ => Vec::new(),
        };
        assert_eq!(String::from_utf8_lossy(&payload), BUSY_MESSAGE);

        drop(busy_allocation);
        let _ = socket.close(None).await;
        server.abort();
    }

    #[tokio::test]
    async fn ws_connection_creates_websocket_session_record() {
        let state = test_state();
        let (transport, client_handle) = oxidebbs_telnet::LoopbackTransport::new();
        let allocation = state
            .runtime
            .try_allocate_node()
            .expect("allocate test node");
        let server = tokio::spawn({
            let resources = state.caller_resources.clone();
            async move {
                handle_raw_caller_transport(
                    allocation,
                    transport,
                    "websocket",
                    CallerPeer {
                        address: "127.0.0.1:9999".to_string(),
                        ip: Some("127.0.0.1".to_string()),
                        port: 9999,
                    },
                    resources,
                )
                .await
                .expect("websocket session ended")
            }
        });

        let latest = timeout(Duration::from_secs(1), async {
            loop {
                let sessions = list_recent_sessions(state._db.db(), 1).expect("sessions query");
                if let Some(session) = sessions
                    .into_iter()
                    .next()
                    .filter(|session| session.transport == "websocket")
                {
                    break session;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("websocket session record appeared");

        assert_eq!(latest.transport, "websocket");
        drop(client_handle);
        server.abort();
        let _ = latest;
    }
}
