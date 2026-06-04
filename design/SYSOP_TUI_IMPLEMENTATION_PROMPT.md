# OxideBBS Sysop TUI Implementation Prompt

## Overview

This document is a comprehensive implementation guide for building the **OxideBBS Sysop TUI** — a full-featured, keyboard-driven, Ratatui-based local administration console for OxideBBS.

The TUI should feel like a **classic BBS control center rebuilt as a modern Rust terminal application**. It must be dense, fast, charming, and operationally useful.

**Guiding principle:** The TUI must not duplicate business logic. It calls the same application services used by the CLI.

**Target:** An 8-node Blackboard BBS dashboard as the default design target, with clean scaling to 16, 32, and 64+ nodes.

---

## Codebase Context

### Workspace Layout

```
crates/
  oxidebbs-server/   # binary entrypoint, CLI, telnet server, control socket
  oxidebbs-core/     # domain models: User, Node, Session, Door, Message, Audit
  oxidebbs-term/     # ANSI/CP437 rendering, CP437 encode/decode
  oxidebbs-telnet/   # telnet transport and negotiation
  oxidebbs-db/       # DecentDB repository layer, OxideDb, schema v8
  oxidebbs-door/     # door definitions, drop files, runners
  oxidebbs-sysop/    # local sysop TUI
design/              # architecture docs, specs, ADRs
config/              # oxidebbs.example.toml
scripts/             # dev-check.sh
```

### Current State of `oxidebbs-sysop`

**File:** `crates/oxidebbs-sysop/src/lib.rs` (279 lines)

Contains the local Ratatui sysop TUI plus compatibility helpers:
- `SysopError` enum
- `AdminCommand` enum (ListUsers, ResetPassword, ListNodes, ShowRecentCalls, TestDoorConfig, PrototypeConsole)
- `DoorConfigCheck` struct (definitions count, enabled count)
- `SysopConsoleSnapshot` struct (board_name, active_nodes, recent_calls)
- Functions: `list_users()`, `reset_password()`, `list_nodes()`, `show_recent_calls()`, `test_door_config()`, `render_sysop_console()`, `render_sysop_console_text()`
- Tests using `OxideDb::open_memory()`

**Cargo.toml:**
```toml
[dependencies]
oxidebbs-db = { path = "../oxidebbs-db" }
oxidebbs-door = { path = "../oxidebbs-door" }
ratatui = { version = "0.30.0", default-features = false }
thiserror.workspace = true
```

### Current State of CLI (`oxidebbs-server`)

The CLI is **substantially implemented** with 14 command groups, full clap-based parsing, JSON output, audit logging, Argon2id password hashing, log rotation, and control socket integration.

**Key file:** `crates/oxidebbs-server/src/sysop_cli.rs` (1025 lines)
- `CliError` enum with variants: Message, Io, Config, Database, Door, Json, TomlDe, TomlSer, Serve
- `AppContext` struct: config_path, config, json
- Helper functions: `open_database()`, `hash_password()`, `audit()`, `require_user()`, `generated_uuid()`, `current_timestamp()`, `print_json()`, `emit_ok()`

**Key file:** `crates/oxidebbs-server/src/commands/sysop.rs`
- `run_sysop_tui()` — launches the local Ratatui sysop console

### Control Socket

**File:** `crates/oxidebbs-server/src/control.rs` (1461 lines)

Key types:
```rust
pub enum ControlRequest {
    Status,
    NodesList,
    NodeDisconnect { node_number: u16, reason: String },
    NodeMessage { node_number: u16, text: String },
    NodeBroadcast { text: String },
    NodesResetStale,
}

pub enum ControlResponse {
    Ok { ok: bool },
    Status { ok: bool, status: ControlStatus },
    Nodes { ok: bool, nodes: Vec<ControlNodeStatus> },
    Error { ok: bool, error: String },
}

pub struct ControlStatus {
    pub board_name: String,
    pub uptime_seconds: u64,
    pub node_count: u16,
    pub active_nodes: usize,
    pub audit_write_failures: u64,
}

pub struct ControlNodeStatus {
    pub node_number: u16,
    pub state: String,
    pub user_alias: Option<String>,
    pub remote_address: Option<String>,
    pub connected_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub heartbeat_age_seconds: Option<u64>,
}
```

### Core Domain Types

**`oxidebbs-core/src/node.rs`:**
```rust
pub enum NodeStatus {
    Idle, Connected, LoggingIn, InMenu, InDoor,
    Uploading, Downloading, Chatting, Voting, Disconnected,
}
```

**`oxidebbs-core/src/user.rs`:**
```rust
pub enum UserStatus { Active, Inactive, Locked }
pub struct User {
    pub id: String, pub alias: String, pub real_name: String,
    pub email: Option<String>, pub password_hash: String,
    pub security_level: i32, pub is_sysop: bool,
    pub created_at: String, pub last_login_at: Option<String>,
    pub total_calls: i64, pub time_bank_minutes: i64,
    pub status: UserStatus,
}
```

### DB Layer Types

**`oxidebbs-db/src/lib.rs`:**
```rust
pub struct OxideDb { db: Db }
impl OxideDb {
    pub fn open_or_create(path: impl AsRef<Path>) -> decentdb::Result<Self>;
    pub fn open_memory() -> decentdb::Result<Self>;
    pub fn db(&self) -> &Db;
}
```

Key record types: `UserRecord`, `SessionRecord`, `MessageAreaRecord`, `MessageRecord`, `AuditEventRecord`, `DoorDefinitionRecord`, `DoorRunRecord`, `AuthAttemptRecord`.

### Workspace Dependencies

```toml
anyhow = "1"
argon2 = "0.5"
rand_core = { version = "0.6", features = ["getrandom"] }
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
clap = { version = "4", features = ["derive", "env"] }
decentdb = { git = "https://github.com/sphildreth/decentdb", tag = "v2.8.0" }
toml = "0.8"
nix = { version = "0.31.3", features = ["term", "fs", "user"] }
```

### Hard Constraints

1. **Rust only**, edition 2024.
2. **DecentDB is the only database.** No SQLite, Postgres, MySQL, Redis, MongoDB, or ORM.
3. **Telnet-only** for v1.
4. **ANSI/CP437 is byte-oriented** for the caller UI.
5. **Ratatui for local sysop TUI only**, NOT for remote caller UI.
6. **Keep door execution isolated** from core session logic.
7. **Never hold a lock across `.await`.**
8. **Prefer `Result<T, E>` with typed errors.** No `unwrap()`/`expect()` in library code.
9. **Use `cargo add`** for new deps; workspace deps pattern.
10. **Agents must never create commits** without explicit approval.

### CI Gate

```bash
cargo fmt --all --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

---

## Architecture

### Module Structure

```
oxidebbs-sysop/src/
  lib.rs              # crate root, re-exports, SysopError
  app.rs              # main TUI application state machine, event loop
  theme.rs            # Oxide Classic theme system (colors, styles)
  input.rs            # keyboard input handling and routing
  command_palette.rs  # fuzzy command launcher
  events.rs           # event system for live data (tick, control socket)
  services/
    mod.rs
    node_service.rs
    user_service.rs
    door_service.rs
    message_service.rs
    database_service.rs
    log_service.rs
    audit_service.rs
  screens/
    mod.rs
    dashboard.rs
    nodes.rs
    users.rs
    messages.rs
    doors.rs
    ansi.rs
    config.rs
    database.rs
    logs.rs
    audit.rs
    help.rs
  widgets/
    mod.rs
    node_map.rs       # compact node grid widget
    node_table.rs     # detailed node table widget
    status_bar.rs     # global footer
    nav_rail.rs       # left navigation
    header.rs         # global header
    modal.rs          # confirmation, form, error modals
    event_log.rs      # recent events widget
    health_panel.rs   # health summary widget
```

### Dependency Direction

```
oxidebbs-sysop -> oxidebbs-core, oxidebbs-db, oxidebbs-door, oxidebbs-term
```

The sysop crate must NOT depend on `oxidebbs-server`. It accesses the control socket through its own client implementation.

### Screen Trait

```rust
pub trait Screen {
    fn title(&self) -> &str;
    fn handle_event(&mut self, event: UiEvent) -> UiAction;
    fn render(&self, frame: &mut Frame, area: Rect);
}
```

### Action System

```rust
pub enum UiAction {
    None,
    Navigate(ScreenId),
    OpenModal(ModalKind),
    RunCommand(CommandRequest),
    Refresh,
    Quit,
}
```

### Service Layer

The TUI calls application services, not direct DB queries. Each service wraps `OxideDb` and optionally the control socket client.

```
NodeAdminService      — node list, detail, disconnect, message, broadcast
UserAdminService      — user list, detail, add, edit, reset password, enable/disable
DoorAdminService      — door list, detail, check, test, dropfile, runs
MessageAdminService   — area list, message list, detail, delete
DatabaseAdminService  — stats, backup, doctor, verify
LogService            — tail, recent, search
AuditService          — recent, filter by user/node/door
```

### Theme: Oxide Classic

| Meaning | Color |
|---|---|
| Primary accent | Oxide orange (`Color::Rgb(255, 140, 0)`) |
| Active/online/success | Terminal green (`Color::Rgb(0, 200, 0)`) |
| Warning | Amber (`Color::Rgb(255, 180, 0)`) |
| Error/destructive | Red (`Color::Rgb(220, 50, 50)`) |
| Muted/inactive | Dark gray (`Color::Rgb(100, 100, 100)`) |
| Neutral text | Off-white (`Color::Rgb(220, 220, 220)`) |
| Data labels | Steel gray (`Color::Rgb(160, 160, 160)`) |
| Selection | Orange border or reversed row |
| Background | Black/charcoal (`Color::Rgb(20, 20, 20)`) |

### Node Activity Codes

| Code | Meaning | Color |
|---|---|---|
| `FREE` | Node available | Muted gray |
| `CONN` | Caller connecting | Green |
| `LOGN` | Login/new-user flow | Green |
| `MENU` | Main menu | Green |
| `MSGS` | Reading messages | Green |
| `POST` | Posting message | Green |
| `MAIL` | Private mail | Green |
| `DOOR` | In a generic door | Orange |
| `LORD` | In LORD specifically | Orange/cyan |
| `CHAT` | Sysop chat | Cyan |
| `IDLE` | Idle too long | Amber |
| `DISC` | Disconnecting | Amber |
| `STALE` | Stale/crashed session | Red |
| `DOWN` | Node disabled/offline | Dark gray |

### Node Scaling Requirements

| Configured Nodes | Dashboard Behavior |
|---:|---|
| 1–4 | Full detail rows fit on dashboard |
| 5–8 | Default target: 2 rows × 4 columns compact node map |
| 9–16 | Compact grid; avoid full-width detail rows |
| 17–32 | Summary + active/problem nodes only |
| 33+ | Aggregate counts + active/problem subset |

**Hard requirement:** No dashboard, table, grid, command, or node-detail workflow may assume exactly 4 nodes.

---

## Phase 1: Foundation (TUI-0)

### Goal

Build the app shell, theme system, navigation rail, header/footer, command palette shell, help modal shell, and keyboard handling. The TUI should launch, display a static layout, and respond to navigation keys.

### Files to Create

#### `crates/oxidebbs-sysop/Cargo.toml` (modify)

Add dependencies:

```toml
[dependencies]
oxidebbs-core = { path = "../oxidebbs-core" }
oxidebbs-db = { path = "../oxidebbs-db" }
oxidebbs-door = { path = "../oxidebbs-door" }
oxidebbs-term = { path = "../oxidebbs-term" }
ratatui = { version = "0.30.0", default-features = false, features = ["crossterm"] }
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
thiserror.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
fuzzy-matcher = "0.3"
```

Use `cargo add` to add these to the workspace and crate.

#### `crates/oxidebbs-sysop/src/lib.rs` (modify)

Expand the crate root to declare all new modules and re-export the public TUI entry point:

```rust
pub mod app;
pub mod theme;
pub mod input;
pub mod command_palette;
pub mod events;
pub mod services;
pub mod screens;
pub mod widgets;

// Re-export the main entry point
pub use app::run_tui;
```

Keep existing types (`SysopError`, `AdminCommand`, etc.) for backward compatibility with earlier sysop crate APIs.

#### `crates/oxidebbs-sysop/src/theme.rs` (create)

```rust
use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub muted: Color,
    pub label: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub border: Color,
    pub border_focused: Color,
}

impl Theme {
    pub fn oxide_classic() -> Self {
        Self {
            background: Color::Rgb(20, 20, 20),
            foreground: Color::Rgb(220, 220, 220),
            accent: Color::Rgb(255, 140, 0),
            success: Color::Rgb(0, 200, 0),
            warning: Color::Rgb(255, 180, 0),
            danger: Color::Rgb(220, 50, 50),
            muted: Color::Rgb(100, 100, 100),
            label: Color::Rgb(160, 160, 160),
            selection_bg: Color::Rgb(255, 140, 0),
            selection_fg: Color::Rgb(0, 0, 0),
            border: Color::Rgb(80, 80, 80),
            border_focused: Color::Rgb(255, 140, 0),
        }
    }
}

impl Theme {
    pub fn title_style(&self) -> Style {
        Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub fn header_style(&self) -> Style {
        Style::default().fg(self.foreground).bg(self.background)
    }

    pub fn selected_style(&self) -> Style {
        Style::default().fg(self.selection_fg).bg(self.selection_bg)
    }

    pub fn normal_style(&self) -> Style {
        Style::default().fg(self.foreground)
    }

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn success_style(&self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    pub fn danger_style(&self) -> Style {
        Style::default().fg(self.danger)
    }

    pub fn label_style(&self) -> Style {
        Style::default().fg(self.label)
    }

    pub fn block_style(&self, focused: bool) -> Style {
        if focused {
            Style::default().fg(self.border_focused)
        } else {
            Style::default().fg(self.border)
        }
    }
}
```

#### `crates/oxidebbs-sysop/src/events.rs` (create)

```rust
use crossterm::event::{Event as CrosstermEvent, KeyEvent};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
    ControlSocketData(ControlSocketUpdate),
    Quit,
}

#[derive(Debug)]
pub enum ControlSocketUpdate {
    NodeStatuses(Vec<NodeStatusSnapshot>),
    StatusUpdate(StatusSnapshot),
    ConnectionLost,
    ConnectionRestored,
}

#[derive(Debug, Clone)]
pub struct NodeStatusSnapshot {
    pub node_number: u16,
    pub state: String,
    pub user_alias: Option<String>,
    pub remote_address: Option<String>,
    pub connected_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub heartbeat_age_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct StatusSnapshot {
    pub board_name: String,
    pub uptime_seconds: u64,
    pub node_count: u16,
    pub active_nodes: usize,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    _tx: mpsc::UnboundedSender<AppEvent>,
}

impl EventHandler {
    pub fn new(tick_rate: std::time::Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let _tx = tx.clone();

        // Spawn crossterm event reader
        tokio::spawn(async move {
            loop {
                if crossterm::event::poll(tick_rate).unwrap_or(false) {
                    match crossterm::event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            if tx.send(AppEvent::Key(key)).is_err() { break; }
                        }
                        Ok(CrosstermEvent::Resize(w, h)) => {
                            if tx.send(AppEvent::Resize(w, h)).is_err() { break; }
                        }
                        _ => {}
                    }
                } else {
                    if tx.send(AppEvent::Tick).is_err() { break; }
                }
            }
        });

        Self { rx, _tx }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }

    pub fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self._tx.clone()
    }
}
```

#### `crates/oxidebbs-sysop/src/input.rs` (create)

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
    FocusNext,
    FocusPrev,
    Confirm,
    Cancel,
    Quit,
    Help,
    CommandPalette,
    Search,
    Refresh,
    NavigateTo(ScreenId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenId {
    Dashboard,
    Nodes,
    Users,
    Messages,
    Doors,
    Ansi,
    Config,
    Database,
    Logs,
    Audit,
    Help,
}

impl ScreenId {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Nodes => "Nodes",
            Self::Users => "Users",
            Self::Messages => "Messages",
            Self::Doors => "Doors",
            Self::Ansi => "ANSI",
            Self::Config => "Config",
            Self::Database => "Database",
            Self::Logs => "Logs",
            Self::Audit => "Audit",
            Self::Help => "Help",
        }
    }

    pub fn all() -> &'static [ScreenId] {
        &[
            Self::Dashboard, Self::Nodes, Self::Users, Self::Messages,
            Self::Doors, Self::Ansi, Self::Config, Self::Database,
            Self::Logs, Self::Audit, Self::Help,
        ]
    }
}

pub fn translate_key(key: KeyEvent) -> UiEvent {
    match (key.modifiers, key.code) {
        (_, KeyCode::F(1)) => UiEvent::Help,
        (_, KeyCode::F(2)) => UiEvent::CommandPalette,
        (_, KeyCode::F(3)) => UiEvent::Search,
        (_, KeyCode::F(5)) => UiEvent::Refresh,
        (_, KeyCode::Tab) => UiEvent::FocusNext,
        (KeyModifiers::SHIFT, KeyCode::BackTab) => UiEvent::FocusPrev,
        (_, KeyCode::BackTab) => UiEvent::FocusPrev,
        (_, KeyCode::Enter) => UiEvent::Confirm,
        (_, KeyCode::Esc) => UiEvent::Cancel,
        (KeyModifiers::NONE, KeyCode::Char('q')) => UiEvent::Quit,
        (KeyModifiers::NONE, KeyCode::Char('?')) => UiEvent::Help,
        (KeyModifiers::NONE, KeyCode::Char('/')) => UiEvent::Search,
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => UiEvent::NavigateTo(ScreenId::Nodes),
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => UiEvent::NavigateTo(ScreenId::Users),
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => UiEvent::NavigateTo(ScreenId::Doors),
        (KeyModifiers::CONTROL, KeyCode::Char('m')) => UiEvent::NavigateTo(ScreenId::Messages),
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => UiEvent::NavigateTo(ScreenId::Logs),
        (KeyModifiers::CONTROL, KeyCode::Char('b')) => UiEvent::NavigateTo(ScreenId::Database),
        _ => UiEvent::Key(key),
    }
}
```

#### `crates/oxidebbs-sysop/src/widgets/mod.rs` (create)

```rust
pub mod node_map;
pub mod node_table;
pub mod status_bar;
pub mod nav_rail;
pub mod header;
pub mod modal;
pub mod event_log;
pub mod health_panel;
```

#### `crates/oxidebbs-sysop/src/widgets/header.rs` (create)

Renders the global header bar at the top of the screen.

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use crate::theme::Theme;

pub struct HeaderWidget<'a> {
    pub board_name: &'a str,
    pub version: &'a str,
    pub uptime: &'a str,
    pub node_summary: &'a str,
    pub user_count: usize,
    pub alert_count: usize,
    pub clock: &'a str,
    pub theme: &'a Theme,
}

impl<'a> Widget for HeaderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let header_text = format!(
            " {} │ {} │ Up {} │ {} │ Users {} │ Alerts {} │ {} ",
            self.board_name, self.version, self.uptime,
            self.node_summary, self.user_count, self.alert_count, self.clock
        );
        Paragraph::new(header_text)
            .style(self.theme.header_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" OxideBBS Sysop ")
                    .title_style(self.theme.title_style())
            )
            .render(area, buf);
    }
}
```

#### `crates/oxidebbs-sysop/src/widgets/nav_rail.rs` (create)

Renders the left navigation rail.

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use crate::input::ScreenId;
use crate::theme::Theme;

pub struct NavRail<'a> {
    pub items: &'a [ScreenId],
    pub selected: usize,
    pub theme: &'a Theme,
}

impl<'a> NavRail<'a> {
    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let items: Vec<ListItem> = self.items.iter().map(|screen| {
            ListItem::new(screen.label())
        }).collect();

        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" NAV ")
                    .title_style(self.theme.title_style())
            )
            .highlight_style(self.theme.selected_style())
            .render(area, buf, state);
    }
}
```

#### `crates/oxidebbs-sysop/src/widgets/status_bar.rs` (create)

Renders the global footer with keyboard shortcuts.

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use crate::theme::Theme;

pub struct StatusBar<'a> {
    pub shortcuts: Vec<(&'a str, &'a str)>,
    pub theme: &'a Theme,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text: String = self.shortcuts.iter()
            .map(|(key, label)| format!("{} {}", key, label))
            .collect::<Vec<_>>()
            .join(" │ ");

        Paragraph::new(text)
            .style(self.theme.muted_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
            )
            .render(area, buf);
    }
}
```

#### `crates/oxidebbs-sysop/src/widgets/modal.rs` (create)

Renders centered modal overlays for confirmation, forms, and errors.

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use crate::theme::Theme;

pub enum ModalKind {
    Confirm(ConfirmModal),
    Form(FormModal),
    Error(ErrorModal),
    Info(InfoModal),
}

pub struct ConfirmModal {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    pub confirm_label: String,
    pub cancel_label: String,
}

pub struct FormModal {
    pub title: String,
    pub fields: Vec<FormField>,
    pub active_field: usize,
}

pub struct FormField {
    pub label: String,
    pub value: String,
    pub is_password: bool,
}

pub struct ErrorModal {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    pub suggestion: Option<String>,
}

pub struct InfoModal {
    pub title: String,
    pub message: String,
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn render_modal(modal: &ModalKind, frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(60, 40, frame.area());
    frame.render_widget(Clear, area);

    match modal {
        ModalKind::Confirm(m) => {
            let text = if let Some(detail) = &m.detail {
                format!("{}\n\n{}", m.message, detail)
            } else {
                m.message.clone()
            };
            let footer = format!(" Y {} │ N {} ", m.confirm_label, m.cancel_label);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.block_style(true))
                .title(format!(" {} ", m.title))
                .title_style(theme.warning_style());
            let paragraph = Paragraph::new(text)
                .block(block)
                .wrap(Wrap { trim: true });
            frame.render_widget(paragraph, area);
        }
        ModalKind::Form(m) => {
            // Render form fields with active field highlighted
            let mut lines = Vec::new();
            for (i, field) in m.fields.iter().enumerate() {
                let marker = if i == m.active_field { "▸ " } else { "  " };
                let value = if field.is_password {
                    "*".repeat(field.value.len())
                } else {
                    field.value.clone()
                };
                lines.push(format!("{}{}: {}", marker, field.label, value));
            }
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.block_style(true))
                .title(format!(" {} ", m.title))
                .title_style(theme.title_style());
            let paragraph = Paragraph::new(lines.join("\n"))
                .block(block)
                .wrap(Wrap { trim: true });
            frame.render_widget(paragraph, area);
        }
        ModalKind::Error(m) => {
            let mut text = m.message.clone();
            if let Some(detail) = &m.detail {
                text.push_str(&format!("\n\n{}", detail));
            }
            if let Some(suggestion) = &m.suggestion {
                text.push_str(&format!("\n\nSuggested: {}", suggestion));
            }
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.danger))
                .title(format!(" {} ", m.title))
                .title_style(theme.danger_style());
            let paragraph = Paragraph::new(text)
                .block(block)
                .wrap(Wrap { trim: true });
            frame.render_widget(paragraph, area);
        }
        ModalKind::Info(m) => {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.block_style(true))
                .title(format!(" {} ", m.title))
                .title_style(theme.title_style());
            let paragraph = Paragraph::new(m.message.as_str())
                .block(block)
                .wrap(Wrap { trim: true });
            frame.render_widget(paragraph, area);
        }
    }
}
```

#### `crates/oxidebbs-sysop/src/widgets/node_map.rs` (create)

Compact node grid widget for the dashboard.

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use crate::theme::Theme;
use crate::events::NodeStatusSnapshot;

pub struct NodeMapWidget<'a> {
    pub nodes: &'a [NodeStatusSnapshot],
    pub total_configured: u16,
    pub selected: Option<u16>,
    pub theme: &'a Theme,
}

impl<'a> NodeMapWidget<'a> {
    pub fn activity_code(state: &str) -> &'static str {
        match state {
            "available" => "FREE",
            "connecting" => "CONN",
            "login" => "LOGN",
            "main_menu" => "MENU",
            "reading_messages" => "MSGS",
            "posting_message" => "POST",
            "in_door" => "DOOR",
            "disconnecting" => "DISC",
            "offline" => "DOWN",
            "stale" => "STALE",
            _ => "????",
        }
    }

    pub fn activity_style(state: &str, theme: &Theme) -> Style {
        match state {
            "available" => theme.muted_style(),
            "connecting" | "login" | "main_menu" | "reading_messages" | "posting_message" => {
                theme.success_style()
            }
            "in_door" => Style::default().fg(theme.accent),
            "disconnecting" => theme.warning_style(),
            "stale" => theme.danger_style(),
            "offline" => Style::default().fg(Color::Rgb(60, 60, 60)),
            _ => theme.normal_style(),
        }
    }

    pub fn render(self, area: Rect, buf: &mut Buffer) {
        let columns = if self.total_configured <= 8 { 4 } else { 4 };
        let rows = (self.total_configured as usize + columns - 1) / columns;

        let mut lines: Vec<Line> = Vec::new();
        for row in 0..rows {
            let mut spans: Vec<Span> = Vec::new();
            for col in 0..columns {
                let idx = row * columns + col;
                if idx >= self.total_configured as usize {
                    break;
                }
                let node_num = (idx + 1) as u16;

                if let Some(node) = self.nodes.iter().find(|n| n.node_number == node_num) {
                    let code = Self::activity_code(&node.state);
                    let alias = node.user_alias.as_deref().unwrap_or("-");
                    let style = Self::activity_style(&node.state, self.theme);
                    let is_selected = self.selected == Some(node_num);

                    let text = format!("{:02} {:<10} {:<6}", node_num, alias, code);
                    if is_selected {
                        spans.push(Span::styled(text, self.theme.selected_style()));
                    } else {
                        spans.push(Span::styled(text, style));
                    }
                } else {
                    let text = format!("{:02} {:<10} {:<6}", node_num, "-", "FREE");
                    spans.push(Span::styled(text, self.theme.muted_style()));
                }

                if col < columns - 1 {
                    spans.push(Span::raw(" │ "));
                }
            }
            lines.push(Line::from(spans));
        }

        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" Node Map ")
                    .title_style(self.theme.title_style())
            )
            .render(area, buf);
    }
}
```

#### `crates/oxidebbs-sysop/src/widgets/event_log.rs` (create)

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use crate::theme::Theme;

pub struct EventLogEntry {
    pub timestamp: String,
    pub event_type: String,
    pub details: String,
}

pub struct EventLogWidget<'a> {
    pub entries: &'a [EventLogEntry],
    pub theme: &'a Theme,
}

impl<'a> Widget for EventLogWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self.entries.iter().map(|entry| {
            Line::from(vec![
                Span::styled(entry.timestamp.as_str(), self.theme.label_style()),
                Span::raw("  "),
                Span::styled(entry.event_type.as_str(), self.theme.accent.into()),
                Span::raw("  "),
                Span::styled(entry.details.as_str(), self.theme.normal_style()),
            ])
        }).collect();

        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" Recent Events ")
                    .title_style(self.theme.title_style())
            )
            .render(area, buf);
    }
}
```

#### `crates/oxidebbs-sysop/src/widgets/health_panel.rs` (create)

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use crate::theme::Theme;

pub struct HealthItem {
    pub label: String,
    pub value: String,
    pub is_ok: bool,
}

pub struct HealthPanelWidget<'a> {
    pub items: &'a [HealthItem],
    pub theme: &'a Theme,
}

impl<'a> Widget for HealthPanelWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self.items.iter().map(|item| {
            let marker = if item.is_ok { "[OK]" } else { "[!!]" };
            let marker_style = if item.is_ok {
                self.theme.success_style()
            } else {
                self.theme.warning_style()
            };
            Line::from(vec![
                Span::styled(marker, marker_style),
                Span::raw(" "),
                Span::styled(format!("{}: ", item.label), self.theme.label_style()),
                Span::styled(item.value.as_str(), self.theme.normal_style()),
            ])
        }).collect();

        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" Health ")
                    .title_style(self.theme.title_style())
            )
            .render(area, buf);
    }
}
```

#### `crates/oxidebbs-sysop/src/widgets/node_table.rs` (create)

Detailed node table widget for the Nodes screen.

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Row, Table, TableState};
use crate::events::NodeStatusSnapshot;
use crate::theme::Theme;
use crate::widgets::node_map::NodeMapWidget;

pub struct NodeTableWidget<'a> {
    pub nodes: &'a [NodeStatusSnapshot],
    pub total_configured: u16,
    pub theme: &'a Theme,
}

impl<'a> NodeTableWidget<'a> {
    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut TableState) {
        let header = Row::new(vec!["#", "User", "Activity", "Time On", "Idle", "Remote", "Status"])
            .style(self.theme.label_style())
            .height(1);

        let rows: Vec<Row> = (1..=self.total_configured).map(|node_num| {
            let node = self.nodes.iter().find(|n| n.node_number == node_num);
            match node {
                Some(n) => {
                    let code = NodeMapWidget::activity_code(&n.state);
                    let alias = n.user_alias.as_deref().unwrap_or("-");
                    let remote = n.remote_address.as_deref().unwrap_or("--");
                    let connected = n.connected_at.as_deref().unwrap_or("--");
                    let style = NodeMapWidget::activity_style(&n.state, self.theme);
                    Row::new(vec![
                        node_num.to_string(),
                        alias.to_string(),
                        code.to_string(),
                        connected.to_string(),
                        "--".to_string(),
                        remote.to_string(),
                        n.state.clone(),
                    ]).style(style)
                }
                None => {
                    Row::new(vec![
                        node_num.to_string(),
                        "-".to_string(),
                        "FREE".to_string(),
                        "--".to_string(),
                        "--".to_string(),
                        "--".to_string(),
                        "Available".to_string(),
                    ]).style(self.theme.muted_style())
                }
            }
        }).collect();

        let widths = [
            Constraint::Length(4),
            Constraint::Length(14),
            Constraint::Length(18),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(17),
            Constraint::Length(14),
        ];

        Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" Nodes ")
                    .title_style(self.theme.title_style())
            )
            .highlight_style(self.theme.selected_style())
            .render(area, buf, state);
    }
}
```

#### `crates/oxidebbs-sysop/src/command_palette.rs` (create)

```rust
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use crate::input::ScreenId;

#[derive(Debug, Clone)]
pub struct PaletteCommand {
    pub id: String,
    pub label: String,
    pub description: String,
    pub shortcut: Option<String>,
    pub is_destructive: bool,
    pub action: PaletteAction,
}

#[derive(Debug, Clone)]
pub enum PaletteAction {
    Navigate(ScreenId),
    RunCommand(String),
}

pub struct CommandPalette {
    pub commands: Vec<PaletteCommand>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub visible: bool,
    matcher: SkimMatcherV2,
}

impl CommandPalette {
    pub fn new(commands: Vec<PaletteCommand>) -> Self {
        let filtered: Vec<usize> = (0..commands.len()).collect();
        Self {
            commands,
            query: String::new(),
            filtered,
            selected: 0,
            visible: false,
            matcher: SkimMatcherV2::default(),
        }
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.query.clear();
        self.refilter();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.refilter();
    }

    pub fn update_query(&mut self, query: String) {
        self.query = query;
        self.refilter();
    }

    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub fn selected_command(&self) -> Option<&PaletteCommand> {
        self.filtered.get(self.selected).map(|&idx| &self.commands[idx])
    }

    fn refilter(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..self.commands.len()).collect();
        } else {
            self.filtered = self.commands.iter().enumerate()
                .filter_map(|(idx, cmd)| {
                    self.matcher.fuzzy_match(&cmd.label, &self.query)
                        .map(|score| (idx, score))
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|(idx, _)| idx)
                .collect();
        }
        self.selected = 0;
    }
}
```

#### `crates/oxidebbs-sysop/src/services/mod.rs` (create)

```rust
pub mod node_service;
pub mod user_service;
pub mod door_service;
pub mod message_service;
pub mod database_service;
pub mod log_service;
pub mod audit_service;
```

#### `crates/oxidebbs-sysop/src/services/node_service.rs` (create)

```rust
use oxidebbs_db::{Db, SessionRecord, list_active_sessions, find_active_session_by_node};
use crate::SysopError;

pub struct NodeAdminService;

impl NodeAdminService {
    pub fn list_active(db: &Db) -> Result<Vec<SessionRecord>, SysopError> {
        Ok(list_active_sessions(db)?)
    }

    pub fn find_session(db: &Db, node_number: i64) -> Result<Option<SessionRecord>, SysopError> {
        Ok(find_active_session_by_node(db, node_number)?)
    }
}
```

#### `crates/oxidebbs-sysop/src/services/user_service.rs` (create)

```rust
use oxidebbs_db::{
    Db, UserRecord, list_users, find_user_by_id, find_user_by_alias_ci,
    insert_user, update_user_password_hash, update_user_security_level,
    update_user_status, update_user_is_sysop,
};
use crate::SysopError;

pub struct UserAdminService;

impl UserAdminService {
    pub fn list(db: &Db) -> Result<Vec<UserRecord>, SysopError> {
        Ok(list_users(db)?)
    }

    pub fn find_by_id(db: &Db, id: &str) -> Result<Option<UserRecord>, SysopError> {
        Ok(find_user_by_id(db, id)?)
    }

    pub fn find_by_alias(db: &Db, alias: &str) -> Result<Option<UserRecord>, SysopError> {
        Ok(find_user_by_alias_ci(db, alias)?)
    }

    pub fn reset_password(db: &Db, user_id: &str, hash: &str) -> Result<(), SysopError> {
        update_user_password_hash(db, user_id, hash)?;
        Ok(())
    }

    pub fn set_security_level(db: &Db, user_id: &str, level: i32) -> Result<(), SysopError> {
        update_user_security_level(db, user_id, level)?;
        Ok(())
    }

    pub fn set_status(db: &Db, user_id: &str, status: &str) -> Result<(), SysopError> {
        update_user_status(db, user_id, status)?;
        Ok(())
    }

    pub fn set_sysop(db: &Db, user_id: &str, is_sysop: bool) -> Result<(), SysopError> {
        update_user_is_sysop(db, user_id, is_sysop)?;
        Ok(())
    }
}
```

#### `crates/oxidebbs-sysop/src/services/door_service.rs` (create)

```rust
use oxidebbs_db::{
    Db, DoorDefinitionRecord, DoorRunRecord,
    list_door_definitions, find_door_by_key, list_door_runs,
};
use oxidebbs_door::parse_doors_toml;
use crate::SysopError;

pub struct DoorAdminService;

impl DoorAdminService {
    pub fn list(db: &Db) -> Result<Vec<DoorDefinitionRecord>, SysopError> {
        Ok(list_door_definitions(db)?)
    }

    pub fn find(db: &Db, key: &str) -> Result<Option<DoorDefinitionRecord>, SysopError> {
        Ok(find_door_by_key(db, key)?)
    }

    pub fn list_runs(db: &Db, limit: i64) -> Result<Vec<DoorRunRecord>, SysopError> {
        Ok(list_door_runs(db, limit)?)
    }

    pub fn check_config(contents: &str) -> Result<(usize, usize), SysopError> {
        let definitions = parse_doors_toml(contents)?;
        let enabled = definitions.iter().filter(|d| d.enabled).count();
        Ok((definitions.len(), enabled))
    }
}
```

#### `crates/oxidebbs-sysop/src/services/message_service.rs` (create)

```rust
use oxidebbs_db::{
    Db, MessageAreaRecord, MessageRecord,
    list_message_areas, find_message_area_by_key,
    list_messages_in_area, find_message_by_id,
    update_message_visibility,
};
use crate::SysopError;

pub struct MessageAdminService;

impl MessageAdminService {
    pub fn list_areas(db: &Db) -> Result<Vec<MessageAreaRecord>, SysopError> {
        Ok(list_message_areas(db)?)
    }

    pub fn find_area(db: &Db, key: &str) -> Result<Option<MessageAreaRecord>, SysopError> {
        Ok(find_message_area_by_key(db, key)?)
    }

    pub fn list_messages(db: &Db, area_id: &str, limit: i64) -> Result<Vec<MessageRecord>, SysopError> {
        Ok(list_messages_in_area(db, area_id, limit)?)
    }

    pub fn find_message(db: &Db, id: &str) -> Result<Option<MessageRecord>, SysopError> {
        Ok(find_message_by_id(db, id)?)
    }

    pub fn delete_message(db: &Db, message_id: &str) -> Result<(), SysopError> {
        update_message_visibility(db, message_id, "deleted")?;
        Ok(())
    }
}
```

#### `crates/oxidebbs-sysop/src/services/database_service.rs` (create)

```rust
use oxidebbs_db::{Db, OxideDb, read_schema_version};
use crate::SysopError;

pub struct DatabaseAdminService;

impl DatabaseAdminService {
    pub fn schema_version(db: &Db) -> Result<i64, SysopError> {
        Ok(read_schema_version(db)?)
    }

    pub fn count_users(db: &Db) -> Result<i64, SysopError> {
        let result = db.execute("SELECT COUNT(*) FROM users")?;
        Ok(result.rows().first()
            .and_then(|row| row.values().first())
            .and_then(|v| match v { oxidebbs_db::Value::Integer(n) => Some(*n), _ => None })
            .unwrap_or(0))
    }

    pub fn count_messages(db: &Db) -> Result<i64, SysopError> {
        let result = db.execute("SELECT COUNT(*) FROM messages")?;
        Ok(result.rows().first()
            .and_then(|row| row.values().first())
            .and_then(|v| match v { oxidebbs_db::Value::Integer(n) => Some(*n), _ => None })
            .unwrap_or(0))
    }

    pub fn count_audit_events(db: &Db) -> Result<i64, SysopError> {
        let result = db.execute("SELECT COUNT(*) FROM audit_events")?;
        Ok(result.rows().first()
            .and_then(|row| row.values().first())
            .and_then(|v| match v { oxidebbs_db::Value::Integer(n) => Some(*n), _ => None })
            .unwrap_or(0))
    }
}
```

#### `crates/oxidebbs-sysop/src/services/log_service.rs` (create)

```rust
use std::path::Path;
use std::fs;
use crate::SysopError;

pub struct LogService;

pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

impl LogService {
    pub fn tail(log_path: &Path, lines: usize) -> Result<Vec<LogEntry>, SysopError> {
        if !log_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(log_path)
            .map_err(|e| SysopError::Io(e))?;
        let entries: Vec<LogEntry> = content.lines()
            .rev()
            .take(lines)
            .rev()
            .map(|line| {
                // Parse log line format: "TIMESTAMP LEVEL TARGET MESSAGE"
                let parts: Vec<&str> = line.splitn(4, ' ').collect();
                LogEntry {
                    timestamp: parts.first().copied().unwrap_or("").to_string(),
                    level: parts.get(1).copied().unwrap_or("").to_string(),
                    target: parts.get(2).copied().unwrap_or("").to_string(),
                    message: parts.get(3).copied().unwrap_or("").to_string(),
                }
            })
            .collect();
        Ok(entries)
    }
}
```

#### `crates/oxidebbs-sysop/src/services/audit_service.rs` (create)

```rust
use oxidebbs_db::{Db, AuditEventRecord, list_audit_events, list_audit_events_for_user};
use crate::SysopError;

pub struct AuditService;

impl AuditService {
    pub fn recent(db: &Db, limit: i64) -> Result<Vec<AuditEventRecord>, SysopError> {
        Ok(list_audit_events(db, limit)?)
    }

    pub fn for_user(db: &Db, user_id: &str, limit: i64) -> Result<Vec<AuditEventRecord>, SysopError> {
        Ok(list_audit_events_for_user(db, user_id, limit)?)
    }
}
```

#### `crates/oxidebbs-sysop/src/screens/mod.rs` (create)

```rust
pub mod dashboard;
pub mod nodes;
pub mod users;
pub mod messages;
pub mod doors;
pub mod ansi;
pub mod config;
pub mod database;
pub mod logs;
pub mod audit;
pub mod help;
```

#### `crates/oxidebbs-sysop/src/screens/dashboard.rs` (create — stub)

```rust
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use crate::input::UiEvent;
use crate::theme::Theme;

pub enum UiAction {
    None,
    Navigate(crate::input::ScreenId),
    OpenModal(crate::widgets::modal::ModalKind),
    Refresh,
    Quit,
}

pub struct DashboardScreen {
    pub theme: Theme,
}

impl DashboardScreen {
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }

    pub fn title(&self) -> &str { "Dashboard" }

    pub fn handle_event(&mut self, _event: UiEvent) -> UiAction {
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        Paragraph::new("Dashboard — loading...")
            .style(self.theme.normal_style())
            .render(area, frame.buffer_mut());
    }
}
```

Create similar stubs for all other screens: `nodes.rs`, `users.rs`, `messages.rs`, `doors.rs`, `ansi.rs`, `config.rs`, `database.rs`, `logs.rs`, `audit.rs`, `help.rs`.

#### `crates/oxidebbs-sysop/src/app.rs` (create)

The main application state machine and event loop.

```rust
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use ratatui::widgets::ListState;

use oxidebbs_db::OxideDb;

use crate::command_palette::{CommandPalette, PaletteAction, PaletteCommand};
use crate::events::{AppEvent, EventHandler};
use crate::input::{ScreenId, UiEvent, translate_key};
use crate::theme::Theme;
use crate::widgets::header::HeaderWidget;
use crate::widgets::modal::{ModalKind, render_modal};
use crate::widgets::nav_rail::NavRail;
use crate::widgets::status_bar::StatusBar;

pub struct AppConfig {
    pub config_path: PathBuf,
    pub readonly: bool,
    pub tick_rate: Duration,
}

pub struct App {
    pub theme: Theme,
    pub current_screen: ScreenId,
    pub nav_state: ListState,
    pub modal: Option<ModalKind>,
    pub command_palette: CommandPalette,
    pub db: Option<OxideDb>,
    pub config: AppConfig,
    pub should_quit: bool,
    pub board_name: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub node_count: u16,
    pub active_nodes: usize,
    pub user_count: usize,
    pub alert_count: usize,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let theme = Theme::oxide_classic();
        let mut nav_state = ListState::default();
        nav_state.select(Some(0));

        let commands = Self::build_palette_commands();

        Self {
            theme,
            current_screen: ScreenId::Dashboard,
            nav_state,
            modal: None,
            command_palette: CommandPalette::new(commands),
            db: None,
            config,
            should_quit: false,
            board_name: "OxideBBS".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: 0,
            node_count: 0,
            active_nodes: 0,
            user_count: 0,
            alert_count: 0,
        }
    }

    fn build_palette_commands() -> Vec<PaletteCommand> {
        vec![
            PaletteCommand {
                id: "nav.dashboard".into(), label: "Go to Dashboard".into(),
                description: "Open the main dashboard".into(), shortcut: None,
                is_destructive: false, action: PaletteAction::Navigate(ScreenId::Dashboard),
            },
            PaletteCommand {
                id: "nav.nodes".into(), label: "Go to Nodes".into(),
                description: "View and manage nodes".into(), shortcut: Some("Ctrl+N".into()),
                is_destructive: false, action: PaletteAction::Navigate(ScreenId::Nodes),
            },
            PaletteCommand {
                id: "nav.users".into(), label: "Go to Users".into(),
                description: "View and manage users".into(), shortcut: Some("Ctrl+U".into()),
                is_destructive: false, action: PaletteAction::Navigate(ScreenId::Users),
            },
            PaletteCommand {
                id: "nav.doors".into(), label: "Go to Doors".into(),
                description: "Manage door definitions".into(), shortcut: Some("Ctrl+D".into()),
                is_destructive: false, action: PaletteAction::Navigate(ScreenId::Doors),
            },
            PaletteCommand {
                id: "nav.messages".into(), label: "Go to Messages".into(),
                description: "Manage message areas".into(), shortcut: Some("Ctrl+M".into()),
                is_destructive: false, action: PaletteAction::Navigate(ScreenId::Messages),
            },
            PaletteCommand {
                id: "nav.logs".into(), label: "Go to Logs".into(),
                description: "View server logs".into(), shortcut: Some("Ctrl+L".into()),
                is_destructive: false, action: PaletteAction::Navigate(ScreenId::Logs),
            },
            PaletteCommand {
                id: "nav.database".into(), label: "Go to Database".into(),
                description: "Database health and backup".into(), shortcut: Some("Ctrl+B".into()),
                is_destructive: false, action: PaletteAction::Navigate(ScreenId::Database),
            },
            PaletteCommand {
                id: "nav.audit".into(), label: "Go to Audit".into(),
                description: "View audit events".into(), shortcut: None,
                is_destructive: false, action: PaletteAction::Navigate(ScreenId::Audit),
            },
        ]
    }

    pub fn navigate_to(&mut self, screen: ScreenId) {
        self.current_screen = screen;
        let idx = ScreenId::all().iter().position(|s| *s == screen).unwrap_or(0);
        self.nav_state.select(Some(idx));
    }

    pub fn nav_next(&mut self) {
        let screens = ScreenId::all();
        let current = self.nav_state.selected().unwrap_or(0);
        let next = (current + 1).min(screens.len() - 1);
        self.nav_state.select(Some(next));
        self.current_screen = screens[next];
    }

    pub fn nav_prev(&mut self) {
        let screens = ScreenId::all();
        let current = self.nav_state.selected().unwrap_or(0);
        let prev = current.saturating_sub(1);
        self.nav_state.select(Some(prev));
        self.current_screen = screens[prev];
    }
}

pub fn run_tui(config: AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);
    let mut events = EventHandler::new(Duration::from_millis(250));

    // Open database
    // (In a real implementation, pass the config path from the CLI)
    // app.db = Some(OxideDb::open_or_create("...")?);

    loop {
        terminal.draw(|frame| {
            render_app(&mut app, frame);
        })?;

        if let Some(event) = tokio::runtime::Handle::current().block_on(events.next()) {
            match event {
                AppEvent::Key(key) => {
                    let ui_event = translate_key(key);
                    handle_ui_event(&mut app, ui_event);
                }
                AppEvent::Tick => {
                    // Refresh data on tick
                }
                AppEvent::Resize(_, _) => {
                    // Terminal handles resize automatically
                }
                AppEvent::Quit => {
                    app.should_quit = true;
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn render_app(app: &mut App, frame: &mut Frame) {
    let area = frame.area();

    // Layout: header (3 rows) | nav (left) + content (right) | footer (3 rows)
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Min(10),   // body
            Constraint::Length(3), // footer
        ])
        .split(area);

    // Header
    let uptime = format_duration(app.uptime_seconds);
    let node_summary = format!("{}/{}", app.active_nodes, app.node_count);
    frame.render_widget(
        HeaderWidget {
            board_name: &app.board_name,
            version: &app.version,
            uptime: &uptime,
            node_summary: &node_summary,
            user_count: app.user_count,
            alert_count: app.alert_count,
            clock: &current_time_string(),
            theme: &app.theme,
        },
        main_layout[0],
    );

    // Body: nav rail + content
    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16), // nav rail
            Constraint::Min(40),   // content
        ])
        .split(main_layout[1]);

    // Nav rail
    let screens = ScreenId::all();
    NavRail {
        items: screens,
        selected: app.nav_state.selected().unwrap_or(0),
        theme: &app.theme,
    }.render(body_layout[0], frame.buffer_mut(), &mut app.nav_state);

    // Content area — render current screen
    match app.current_screen {
        ScreenId::Dashboard => {
            ratatui::widgets::Paragraph::new("Dashboard")
                .style(app.theme.normal_style())
                .render(body_layout[1], frame.buffer_mut());
        }
        _ => {
            ratatui::widgets::Paragraph::new(app.current_screen.label())
                .style(app.theme.normal_style())
                .render(body_layout[1], frame.buffer_mut());
        }
    }

    // Footer
    frame.render_widget(
        StatusBar {
            shortcuts: vec![
                ("F1", "Help"),
                ("F2", "Command"),
                ("F3", "Search"),
                ("F5", "Refresh"),
                ("Tab", "Panel"),
                ("Enter", "Open"),
                ("Esc", "Back"),
                ("Q", "Quit"),
            ],
            theme: &app.theme,
        },
        main_layout[2],
    );

    // Modal overlay
    if let Some(modal) = &app.modal {
        render_modal(modal, frame, &app.theme);
    }
}

fn handle_ui_event(app: &mut App, event: UiEvent) {
    // Handle modal first
    if app.modal.is_some() {
        match event {
            UiEvent::Cancel => { app.modal = None; }
            UiEvent::Confirm => { /* handle confirm action */ app.modal = None; }
            _ => {}
        }
        return;
    }

    // Handle command palette
    if app.command_palette.visible {
        match event {
            UiEvent::Cancel => { app.command_palette.close(); }
            UiEvent::Confirm => {
                if let Some(cmd) = app.command_palette.selected_command().cloned() {
                    app.command_palette.close();
                    match cmd.action {
                        PaletteAction::Navigate(screen) => app.navigate_to(screen),
                        PaletteAction::RunCommand(_) => {}
                    }
                }
            }
            UiEvent::Key(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Up => app.command_palette.select_prev(),
                    KeyCode::Down => app.command_palette.select_next(),
                    KeyCode::Char(c) => {
                        let mut q = app.command_palette.query.clone();
                        q.push(c);
                        app.command_palette.update_query(q);
                    }
                    KeyCode::Backspace => {
                        let mut q = app.command_palette.query.clone();
                        q.pop();
                        app.command_palette.update_query(q);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        return;
    }

    // Global events
    match event {
        UiEvent::Quit => { app.should_quit = true; }
        UiEvent::Help => { app.navigate_to(ScreenId::Help); }
        UiEvent::CommandPalette => { app.command_palette.open(); }
        UiEvent::FocusNext => { app.nav_next(); }
        UiEvent::FocusPrev => { app.nav_prev(); }
        UiEvent::NavigateTo(screen) => { app.navigate_to(screen); }
        UiEvent::Refresh => { /* trigger data refresh */ }
        _ => {
            // Delegate to current screen
        }
    }
}

fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
}

fn current_time_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hours = (secs / 3600) % 24;
    let minutes = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, s)
}
```

### Acceptance Criteria (Phase 1)

1. `cargo check --workspace --locked` passes.
2. `cargo clippy --workspace --all-targets --locked -- -D warnings` passes.
3. `cargo fmt --all --check` passes.
4. The TUI launches with `oxidebbs-server sysop`.
5. Navigation rail shows all 11 screens.
6. Tab/Shift+Tab cycles through screens.
7. Ctrl+N/U/D/M/L/B navigate to the correct screens.
8. F1 opens Help screen.
9. F2 opens command palette overlay.
10. F5 triggers refresh (even if no-op).
11. Q quits the application.
12. Esc closes modals and command palette.
13. Theme renders correctly with Oxide Classic colors.
14. Header shows board name, version, uptime, node count, clock.
15. Footer shows keyboard shortcuts.
16. Terminal resize is handled gracefully.
17. 80x25 minimum terminal size works with reduced layout.

---

## Phase 2: Dashboard and Nodes (TUI-1)

### Goal

Build the live dashboard with node map, recent events, health panel, and alerts. Build the full Nodes screen with table view, grid view, node detail, disconnect, message, and broadcast.

### Dashboard Mockup (8 nodes)

```text
┌──────────────────────────────────────── Dashboard ──────────────────────────────────────────────┐
│ Blackboard BBS │ OxideBBS 0.1.0 │ Up 03:12:44 │ Nodes 3/8 │ Doors 1 │ Alerts 0 │ 23:59:14      │
├─────────────────────────────────────── Node Map ────────────────────────────────────────────────┤
│ 01 steven    MSGS   │ 02 nightowl  LORD   │ 03 guest     MENU   │ 04 -         FREE             │
│ 05 -         FREE   │ 06 cactus    POST   │ 07 -         FREE   │ 08 -         FREE             │
├────────────────────────────────────── Recent Events ────────────────────────────────────────────┤
│ 22:18:03 node=2 door_started lord user=nightowl                                                 │
│ 22:20:44 node=6 message_posted area=general user=cactus                                         │
│ 22:22:01 system db_backup_started                                                               │
├───────────────────────────────┬────────────────────────────────────────────────────────────────┤
│ Health                        │ Alerts                                                         │
│ DB:        OK                 │ No active alerts                                                │
│ Telnet:    0.0.0.0:2323       │                                                                │
│ Doors:     3 enabled          │                                                                │
│ OxideNet:  disabled           │                                                                │
└───────────────────────────────┴────────────────────────────────────────────────────────────────┘
```

### Nodes Screen Mockup

```text
┌────────────────────────────────────────── Nodes ────────────────────────────────────────────────┐
│ Nodes: 3/8 active      Filter: all      Sort: node      View: table      Auto-refresh: 2s       │
├────┬──────────────┬────────────────────┬──────────┬────────┬─────────────────┬────────────────┤
│ #  │ User         │ Activity           │ Time On  │ Idle   │ Remote          │ Status         │
├────┼──────────────┼────────────────────┼──────────┼────────┼─────────────────┼────────────────┤
│ 1  │ steven       │ Reading Messages   │ 00:24:12 │ 00:01  │ 192.168.1.50    │ Online         │
│ 2  │ nightowl     │ Door: LORD         │ 00:12:44 │ 00:00  │ 10.0.0.44       │ In Door        │
│ 3  │ guest        │ Main Menu          │ 00:02:10 │ 00:30  │ 192.168.1.80    │ Online         │
│ 4  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 5  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 6  │ cactus       │ Posting Message    │ 00:08:19 │ 00:00  │ 192.168.1.99    │ Online         │
│ 7  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 8  │ -            │ Available          │ --       │ --     │ --              │ Available      │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Move │ Enter Detail │ M Msg │ C Chat │ D Disconnect │ K Kill Door │ B Broadcast │ F Filter    │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Node Detail Mockup

```text
┌──────────────────────────────────────── Node 2 Detail ──────────────────────────────────────────┐
│ User: nightowl                  Alias: Night Owl                  Security: 50                 │
│ Activity: Door: LORD            Connected: 00:12:44               Idle: 00:00:03               │
│ Remote: 10.0.0.44               Terminal: SyncTERM                Encoding: CP437              │
├──────────────────────────────────────── Door Session ───────────────────────────────────────────┤
│ Door: Legend of the Red Dragon                                                                 │
│ Runner: DOSBox                                                                                 │
│ Runtime Dir: ./runtime/nodes/002/lord                                                          │
│ Drop File: DORINFO1.DEF                                                                        │
│ Started: 22:18:03                                                                              │
│ Time Limit: 30 min                                                                             │
├──────────────────────────────────────── Recent Node Events ─────────────────────────────────────┤
│ 22:17:55 menu_command D                                                                         │
│ 22:18:03 door_started lord                                                                      │
│ 22:18:04 dropfile_written DORINFO1.DEF                                                         │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ M Message │ C Chat │ D Disconnect │ K Kill Door │ T Tail I/O │ A Audit │ Esc Back              │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Node Grid View Mockup

```text
┌──────────────────────────────────────── Node Grid ──────────────────────────────────────────────┐
│ 01 steven    MSGS   │ 02 nightowl  LORD   │ 03 guest     MENU   │ 04 -         FREE             │
│ 05 -         FREE   │ 06 cactus    POST   │ 07 -         FREE   │ 08 -         FREE             │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Detail │ T Table │ A Active │ P Problems │ D Doors │ M Message │ B Broadcast │ Esc Back    │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Files to Modify/Create

#### `crates/oxidebbs-sysop/src/screens/dashboard.rs` (replace stub)

Implement the full dashboard screen:

- Load data from `NodeAdminService`, `AuditService`, `DatabaseAdminService`
- Render `NodeMapWidget` with live node data
- Render `EventLogWidget` with recent audit events
- Render `HealthPanelWidget` with DB status, telnet status, door count
- Render alerts panel (empty for now, or show stale nodes)
- Auto-refresh on tick events
- Enter key on a node navigates to Nodes screen with that node selected

**Keyboard shortcuts on dashboard:**
- `Enter` on selected node → navigate to Node Detail
- `Ctrl+N` → Nodes screen
- `Ctrl+L` → Logs screen
- `F5` → manual refresh

#### `crates/oxidebbs-sysop/src/screens/nodes.rs` (replace stub)

Implement the full Nodes screen with multiple views:

```rust
pub enum NodeView {
    Table,
    Grid,
    ActiveOnly,
    DoorOnly,
    ProblemOnly,
}

pub struct NodesScreen {
    pub theme: Theme,
    pub view: NodeView,
    pub nodes: Vec<NodeStatusSnapshot>,
    pub total_configured: u16,
    pub table_state: TableState,
    pub grid_selected: u16,
    pub filter: String,
    pub sort: NodeSort,
    pub detail_node: Option<u16>,
    pub auto_refresh_seconds: u16,
}

pub enum NodeSort {
    NodeNumber,
    Activity,
    User,
    TimeOn,
}
```

**Keyboard shortcuts on Nodes screen:**
- `↑/↓` — move selection in table
- `←/→` — move selection in grid
- `Enter` — open node detail
- `v` — cycle view (table → grid → active → door → problem)
- `t` — table view
- `g` — grid view
- `a` — active-only view
- `p` — problem nodes
- `d` — door nodes
- `/` — search/filter nodes
- `f` — filter
- `m` — send message to selected node (opens form modal)
- `b` — broadcast message (opens form modal)
- `c` — request sysop chat with node
- `k` — kill door on selected node (opens confirm modal)
- `r` — reset stale nodes
- `PageUp/PageDown` — page through nodes
- `Home/End` — jump to first/last node
- `Esc` — back to dashboard

**Node detail view:**
- Show user info, activity, connection details
- Show door session info if in door
- Show recent node events from audit log
- Actions: Message, Chat, Disconnect, Kill Door, Audit, Tail I/O

**Disconnect confirmation modal:**
```text
┌──────────────────────────────────── Confirm Disconnect ─────────────────────────────────────────┐
│ Disconnect node 2?                                                                              │
│                                                                                                │
│ User: nightowl                                                                                  │
│ Activity: Door: LORD                                                                            │
│                                                                                                │
│ This will terminate the active session and may kill the running door process.                    │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Y Confirm │ N Cancel                                                                            │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### `crates/oxidebbs-sysop/src/services/node_service.rs` (expand)

Add control socket client for live node operations:

```rust
pub struct NodeAdminService {
    control_socket_path: Option<PathBuf>,
}

impl NodeAdminService {
    pub fn new(control_socket_path: Option<PathBuf>) -> Self {
        Self { control_socket_path }
    }

    pub fn list_from_db(db: &Db) -> Result<Vec<SessionRecord>, SysopError> { ... }

    pub fn list_from_control(&self) -> Result<Vec<NodeStatusSnapshot>, SysopError> {
        // Connect to control socket, send NodesList, parse response
        // Fall back to DB if socket unavailable
    }

    pub fn disconnect_node(&self, node_number: u16, reason: &str) -> Result<(), SysopError> {
        // Send NodeDisconnect via control socket
        // Audit log the action
    }

    pub fn send_message(&self, node_number: u16, text: &str) -> Result<(), SysopError> {
        // Send NodeMessage via control socket
    }

    pub fn broadcast(&self, text: &str) -> Result<(), SysopError> {
        // Send NodeBroadcast via control socket
    }

    pub fn reset_stale(&self) -> Result<(), SysopError> {
        // Send NodesResetStale via control socket
    }
}
```

### Acceptance Criteria (Phase 2)

1. Dashboard shows live node map with correct activity codes and colors.
2. Dashboard shows recent events from audit log.
3. Dashboard shows health panel with DB status.
4. Dashboard auto-refreshes every 2 seconds.
5. Nodes screen shows table view with all configured nodes.
6. Nodes screen supports grid view, active-only, door-only, problem views.
7. Node detail shows full session information.
8. Disconnect opens confirmation modal and executes on confirm.
9. Message opens form modal and sends via control socket.
10. Broadcast opens form modal and sends to all nodes.
11. Kill door opens confirmation modal.
12. All destructive actions are audit logged.
13. Node scaling works for 1, 4, 8, 16, 32, 64 nodes.
14. Narrow terminal (80x25) degrades gracefully.
15. Wide terminal (120x40) uses space efficiently.

---

## Phase 3: Users (TUI-2)

### Goal

Build user list, search/filter, user detail, add user, edit user, reset password, set security level, enable/disable, and user audit view.

### User List Mockup

```text
┌────────────────────────────────────────── Users ────────────────────────────────────────────────┐
│ Search: ste                         Filter: active                         Users: 128           │
├──────────┬──────────────┬──────────────┬───────┬────────────┬─────────────┬────────────────────┤
│ ID       │ Alias        │ Real Name    │ Sec   │ Calls      │ Last Login  │ Status             │
├──────────┼──────────────┼──────────────┼───────┼────────────┼─────────────┼────────────────────┤
│ 000001   │ steven       │ Steven H.    │ 100   │ 42         │ Today       │ Sysop              │
│ 000014   │ nightowl     │ -            │ 50    │ 12         │ Yesterday   │ Active             │
│ 000031   │ guest        │ -            │ 10    │ 2          │ 2026-05-29  │ Limited            │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Open │ A Add │ E Edit │ P Password │ L Level │ D Disable │ / Search │ F Filter           │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### User Detail Mockup

```text
┌──────────────────────────────────────── User: steven ───────────────────────────────────────────┐
│ Alias:        steven                         Status:       Active                              │
│ Real Name:    Steven H.                      Role:         Sysop                               │
│ Security:     100                            Time Bank:    120 min                             │
│ Calls:        42                             Last Login:   2026-06-02 22:11                    │
│ Created:      2026-05-31                     Last Remote:  192.168.1.50                        │
├──────────────────────────────────────── Permissions ────────────────────────────────────────────┤
│ [x] Sysop          [x] Manage Users     [x] Manage Doors     [x] Manage OxideNet               │
│ [x] Post Messages  [x] Access Doors     [ ] Suspended        [ ] New User Hold                 │
├──────────────────────────────────────── Recent Activity ────────────────────────────────────────┤
│ 22:11 login_success node=1                                                                      │
│ 22:13 message_posted area=OXIDE.GENERAL                                                         │
│ 22:18 door_started lord                                                                         │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ E Edit │ P Reset Password │ L Set Level │ D Disable │ A Audit │ S Sessions │ Esc Back           │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Add User Form Modal

```text
┌──────────────────────────────────── Add User ───────────────────────────────────────────────────┐
│ Alias:        ____________________                                                              │
│ Real Name:    ____________________                                                              │
│ Security:     10                                                                                │
│ Password:     ****************                                                                  │
│ Confirm:      ****************                                                                  │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Save │ Esc Cancel │ Tab Next Field                                                        │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Files to Modify

#### `crates/oxidebbs-sysop/src/screens/users.rs` (replace stub)

```rust
pub struct UsersScreen {
    pub theme: Theme,
    pub users: Vec<UserRecord>,
    pub filtered_users: Vec<usize>,
    pub table_state: TableState,
    pub search_query: String,
    pub filter: UserFilter,
    pub detail_user: Option<String>,  // user_id
    pub show_add_form: bool,
    pub add_form: AddUserForm,
}

pub enum UserFilter {
    All,
    Active,
    Disabled,
    Sysops,
    NewUsers,
}

pub struct AddUserForm {
    pub alias: String,
    pub real_name: String,
    pub security_level: i32,
    pub password: String,
    pub password_confirm: String,
    pub active_field: usize,
}
```

**Keyboard shortcuts on Users screen:**
- `↑/↓` — move selection
- `Enter` — open user detail
- `a` — open add user form
- `e` — edit selected user (future)
- `p` — reset password (opens form modal)
- `l` — set security level (opens form modal)
- `d` — disable/enable user (opens confirm modal)
- `/` — search users
- `f` — cycle filter (all → active → disabled → sysops → new)
- `s` — view user sessions
- `Esc` — back to dashboard

**All user write actions must be audit logged:**
- `user_added`
- `user_password_reset`
- `user_security_level_changed`
- `user_disabled`
- `user_enabled`
- `user_promoted_sysop`
- `user_demoted_sysop`

### Acceptance Criteria (Phase 3)

1. User list shows all users with alias, real name, security level, calls, last login, status.
2. Search filters users by alias, real name, or ID.
3. Filter cycles through all/active/disabled/sysops/new.
4. User detail shows full profile information.
5. Add user form validates alias uniqueness and password match.
6. Reset password generates new Argon2id hash and audit logs.
7. Set security level validates range and audit logs.
8. Disable/enable toggles status with confirmation and audit log.
9. Promote/demote sysop with confirmation and audit log.
10. User audit view shows recent audit events for the selected user.
11. Read-only mode hides all write actions.

---

## Phase 4: Doors (TUI-3)

### Goal

Build door list, door detail, config check, drop-file viewer, dry-run, test launch, run history, door logs, and runtime cleanup.

### Door List Mockup

```text
┌────────────────────────────────────────── Doors ────────────────────────────────────────────────┐
│ Runner: all                              Enabled: 3                         Failed Today: 1      │
├──────────────┬──────────────────────────────┬──────────┬──────────┬────────────┬───────────────┤
│ Key          │ Name                         │ Runner   │ Dropfile │ Runs Today │ Status        │
├──────────────┼──────────────────────────────┼──────────┼──────────┼────────────┼───────────────┤
│ lord         │ Legend of the Red Dragon     │ DOSBox   │ DORINFO  │ 4          │ Enabled       │
│ trivia       │ Death by Trivia              │ DOSBox   │ DOOR.SYS │ 2          │ Enabled       │
│ usurper      │ Usurper                      │ DOSBox   │ DORINFO  │ 0          │ Disabled      │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Open │ T Test │ C Check │ D Disable │ R Runs │ F Dropfile │ L Logs │ A Add               │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Door Detail Mockup

```text
┌────────────────────────────────────── Door: lord ───────────────────────────────────────────────┐
│ Name:        Legend of the Red Dragon                    Status:      Enabled                   │
│ Runner:      DOSBox                                      Exclusive:   No                        │
│ Command:     LORD.EXE                                    Drop File:   DORINFO1.DEF              │
│ Work Dir:    ./doors/lord                                Time Limit:  30 min                    │
│ Runtime:     ./runtime/nodes/{node}/lord                                                         │
├─────────────────────────────────────── Health Check ────────────────────────────────────────────┤
│ [OK] Working directory exists                                                                    │
│ [OK] Command exists                                                                              │
│ [OK] Runtime directory writable                                                                  │
│ [OK] Drop-file format supported                                                                  │
│ [!!] Last run exited with code 1                                                                 │
├─────────────────────────────────────── Recent Runs ─────────────────────────────────────────────┤
│ 22:18 node=2 user=nightowl duration=00:12:10 exit=0                                              │
│ 21:04 node=1 user=steven   duration=00:00:12 exit=1                                              │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ T Test │ C Check │ F View Dropfile │ E Edit │ R Runs │ L Logs │ D Disable │ Esc Back            │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Door Test Modal

```text
┌────────────────────────────────────── Test Door: LORD ──────────────────────────────────────────┐
│ User:        sysop                                                                               │
│ Node:        1                                                                                   │
│ Mode:        Dry Run                                                                             │
│ Dropfile:    DORINFO1.DEF                                                                        │
│ Runtime Dir: ./runtime/test/node-001/lord                                                        │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ This will generate the runtime directory and drop file without launching DOSBox.                 │
│                                                                                                │
│ [ ] Launch actual door after dry-run                                                            │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Run Test │ F View Dropfile │ Esc Cancel                                                    │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Drop-File Viewer Mockup

```text
┌──────────────────────────────────── Generated DORINFO1.DEF ─────────────────────────────────────┐
│ Blackboard BBS                                                                                  │
│ Steven Hildreth                                                                                 │
│ COM1                                                                                            │
│ 38400                                                                                           │
│ 0                                                                                               │
│ steven                                                                                          │
│ Steven                                                                                          │
│ Hildreth                                                                                        │
│ 100                                                                                             │
│ 30                                                                                              │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Format: DORINFO1.DEF │ Encoding: CP437 │ Line endings: CRLF │ F Save │ C Copy │ Esc Back         │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Files to Modify

#### `crates/oxidebbs-sysop/src/screens/doors.rs` (replace stub)

```rust
pub struct DoorsScreen {
    pub theme: Theme,
    pub doors: Vec<DoorDefinitionRecord>,
    pub table_state: TableState,
    pub detail_door: Option<String>,
    pub show_test_modal: bool,
    pub test_modal: DoorTestModal,
    pub show_dropfile_viewer: bool,
    pub dropfile_content: String,
    pub show_runs: bool,
    pub runs: Vec<DoorRunRecord>,
}

pub struct DoorTestModal {
    pub door_key: String,
    pub user_alias: String,
    pub node_number: u16,
    pub dry_run: bool,
    pub launch_after: bool,
}
```

**Keyboard shortcuts on Doors screen:**
- `↑/↓` — move selection
- `Enter` — open door detail
- `t` — open test modal
- `c` — run config check
- `d` — disable/enable door (confirm modal)
- `r` — view run history
- `f` — view generated drop file
- `l` — view door logs
- `a` — add door (future)
- `Esc` — back to dashboard

**Door detail health checks:**
- Working directory exists
- Command exists
- Runtime directory writable
- Drop-file format supported
- Last run exit code

### Acceptance Criteria (Phase 4)

1. Door list shows all configured doors with key, name, runner, dropfile format, runs today, status.
2. Door detail shows full configuration and health check results.
3. Config check validates all door properties and shows OK/warning/error markers.
4. Drop-file viewer generates and displays drop file content.
5. Dry-run test generates runtime directory and drop file without launching.
6. Run history shows recent door runs with duration and exit code.
7. Disable/enable door with confirmation and audit log.
8. Failed runs are highlighted in red.

---

## Phase 5: Messages (TUI-4)

### Goal

Build message area list, area detail, message list, message detail, and delete message.

### Message Areas Mockup

```text
┌──────────────────────────────────────── Message Areas ──────────────────────────────────────────┐
│ Filter: all                                  Local: 5     Network: 4                            │
├───────────────┬──────────────────────────────┬──────────┬────────┬────────┬───────────────────┤
│ Key           │ Name                         │ Type     │ Msgs   │ Sec    │ Status            │
├───────────────┼──────────────────────────────┼──────────┼────────┼────────┼───────────────────┤
│ general       │ General Discussion           │ Local    │ 124    │ 10/10  │ Active            │
│ sysop         │ Sysop Discussion             │ Local    │ 18     │ 90/90  │ Active            │
│ ox-general    │ OXIDE.GENERAL                │ Network  │ 42     │ 10/10  │ Active            │
│ ox-test       │ OXIDE.TEST                   │ Network  │ 7      │ 10/10  │ Active            │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Open │ A Add │ E Edit │ D Disable │ M Messages │ S Security │ R Recount │ / Search        │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Message List Mockup

```text
┌──────────────────────────────────── Messages: OXIDE.GENERAL ───────────────────────────────────┐
│ Search:                            Sort: newest                         Messages: 42            │
├────────┬──────────────────────────────┬──────────────┬────────────────────┬────────────────────┤
│ ID     │ Subject                      │ From         │ Date               │ Flags              │
├────────┼──────────────────────────────┼──────────────┼────────────────────┼────────────────────┤
│ 1042   │ Welcome to OxideNet          │ 42:1/1       │ 2026-06-02 22:14   │ Net, Pinned        │
│ 1041   │ Door runner notes            │ steven       │ 2026-06-02 21:02   │ Local              │
│ 1040   │ ANSI color experiments       │ nightowl     │ 2026-06-01 19:55   │ Net                │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Read │ D Delete │ P Pin │ M Move │ L Lock │ A Audit │ / Search │ Esc Back               │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Message Detail Mockup

```text
┌──────────────────────────────────────── Message 1042 ───────────────────────────────────────────┐
│ Area: OXIDE.GENERAL              From: 42:1/1 Blackboard BBS              Date: 2026-06-02       │
│ Subject: Welcome to OxideNet                                                                    │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Welcome to OxideNet. If you are reading this message, your first poll worked.                   │
│                                                                                                │
│ Reply in OXIDE.TEST to verify outbound scanning from your BBS.                                  │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ R Reply │ D Delete │ P Pin │ M Move │ X Export Metadata │ Esc Back                              │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Files to Modify

#### `crates/oxidebbs-sysop/src/screens/messages.rs` (replace stub)

```rust
pub struct MessagesScreen {
    pub theme: Theme,
    pub areas: Vec<MessageAreaRecord>,
    pub area_table_state: TableState,
    pub selected_area: Option<String>,
    pub messages: Vec<MessageRecord>,
    pub message_table_state: TableState,
    pub selected_message: Option<String>,
    pub view: MessageView,
}

pub enum MessageView {
    AreaList,
    MessageList { area_key: String },
    MessageDetail { message_id: String },
}
```

**Keyboard shortcuts on Messages screen:**
- `↑/↓` — move selection
- `Enter` — open area → open message list → read message
- `d` — delete message (confirm modal, audit log)
- `/` — search messages
- `Esc` — back to area list / back to dashboard

### Acceptance Criteria (Phase 5)

1. Area list shows all message areas with key, name, type, message count, security levels, status.
2. Message list shows messages in selected area with ID, subject, from, date, flags.
3. Message detail shows full message body.
4. Delete message opens confirmation modal and audit logs the deletion.
5. Read-only mode hides delete action.

---

## Phase 6: Database, Logs, Audit (TUI-5)

### Goal

Build database status/backup/doctor, live logs viewer, and audit search/filter.

### Database Screen Mockup

```text
┌────────────────────────────────────── Database ─────────────────────────────────────────────────┐
│ Path: ./data/oxidebbs.ddb                               Status: OK                              │
├────────────────────┬───────────────────────────────────────────────────────────────────────────┤
│ Users              │ 128                                                                       │
│ Messages           │ 1,482                                                                     │
│ Door Runs          │ 344                                                                       │
│ Audit Events       │ 2,104                                                                     │
│ Last Backup        │ 2026-06-02 21:00                                                          │
│ Schema Version     │ 4                                                                         │
├────────────────────────────────────── Health Checks ────────────────────────────────────────────┤
│ [OK] Database opens                                                                             │
│ [OK] Schema current                                                                             │
│ [OK] Runtime writable                                                                           │
│ [!!] Last backup older than 24h                                                                 │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ B Backup │ D Doctor │ S Stats │ V Verify │ E Export │ Esc Back                                 │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Logs Screen Mockup

```text
┌────────────────────────────────────────── Logs ─────────────────────────────────────────────────┐
│ Level: info+        Filter: door                         Follow: on                            │
├──────────┬─────────┬─────────────┬─────────────────────────────────────────────────────────────┤
│ Time     │ Level   │ Target      │ Message                                                     │
├──────────┼─────────┼─────────────┼─────────────────────────────────────────────────────────────┤
│ 22:18:03 │ INFO    │ door        │ door_started key=lord node=2 user=nightowl                  │
│ 22:18:04 │ DEBUG   │ door        │ dropfile_written path=runtime/node-002/DORINFO1.DEF         │
│ 22:30:13 │ INFO    │ door        │ door_finished key=lord exit=0 duration=00:12:10             │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ / Filter │ L Level │ F Follow │ C Clear View │ E Export │ Esc Back                              │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Audit Screen Mockup

```text
┌───────────────────────────────────────── Audit ─────────────────────────────────────────────────┐
│ Filter: admin actions                         Range: today                                      │
├────────────────────┬──────────────┬──────────────┬─────────────────────────────────────────────┤
│ Time               │ Actor        │ Event        │ Details                                     │
├────────────────────┼──────────────┼──────────────┼─────────────────────────────────────────────┤
│ 2026-06-02 22:14   │ sysop        │ user_disable │ target=guest2 reason=spam                   │
│ 2026-06-02 21:55   │ sysop        │ door_test    │ door=lord result=ok                         │
│ 2026-06-02 21:00   │ system       │ db_backup    │ output=backups/oxidebbs-20260602.ddb        │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ / Search │ U User │ N Node │ D Door │ E Export │ Esc Back                                     │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Files to Modify

#### `crates/oxidebbs-sysop/src/screens/database.rs` (replace stub)

```rust
pub struct DatabaseScreen {
    pub theme: Theme,
    pub db_path: String,
    pub schema_version: i64,
    pub stats: DatabaseStats,
    pub health_checks: Vec<HealthCheckResult>,
    pub backup_in_progress: bool,
}

pub struct DatabaseStats {
    pub user_count: i64,
    pub message_count: i64,
    pub door_run_count: i64,
    pub audit_event_count: i64,
    pub session_count: i64,
    pub message_area_count: i64,
}

pub struct HealthCheckResult {
    pub label: String,
    pub is_ok: bool,
    pub detail: String,
}
```

**Keyboard shortcuts:**
- `b` — start backup (with path prompt)
- `d` — run doctor
- `s` — refresh stats
- `v` — verify database
- `Esc` — back to dashboard

#### `crates/oxidebbs-sysop/src/screens/logs.rs` (replace stub)

```rust
pub struct LogsScreen {
    pub theme: Theme,
    pub entries: Vec<LogEntry>,
    pub scroll_offset: usize,
    pub follow: bool,
    pub level_filter: LogLevelFilter,
    pub target_filter: String,
    pub search_query: String,
}

pub enum LogLevelFilter {
    All,
    Info,
    Warn,
    Error,
    Debug,
    Trace,
}
```

**Keyboard shortcuts:**
- `↑/↓` — scroll
- `/` — search/filter
- `l` — cycle level filter
- `f` — toggle follow mode
- `c` — clear view
- `Esc` — back to dashboard

#### `crates/oxidebbs-sysop/src/screens/audit.rs` (replace stub)

```rust
pub struct AuditScreen {
    pub theme: Theme,
    pub events: Vec<AuditEventRecord>,
    pub table_state: TableState,
    pub filter: AuditFilter,
    pub search_query: String,
}

pub enum AuditFilter {
    All,
    User(String),
    Node(i64),
    Door(String),
    AdminActions,
}
```

**Keyboard shortcuts:**
- `↑/↓` — move selection
- `/` — search
- `u` — filter by user (prompt)
- `n` — filter by node (prompt)
- `d` — filter by door (prompt)
- `Esc` — back to dashboard

### Acceptance Criteria (Phase 6)

1. Database screen shows path, schema version, counts, health checks.
2. Backup command starts backup and shows progress.
3. Doctor runs validation and shows results.
4. Logs screen tails log file with follow mode.
5. Logs support level filtering and text search.
6. Audit screen shows audit events with filtering by user, node, door.
7. All screens handle empty data gracefully.

---

## Phase 7: Polish

### Goal

Complete the ANSI screens, Config screen, Help system, and command palette with all commands.

### ANSI Screens Mockup

```text
┌────────────────────────────────────── ANSI Screens ─────────────────────────────────────────────┐
│ Path: ./assets/ansi                          Encoding: CP437                                    │
├─────────────────────┬──────────┬──────────┬──────────────┬─────────────────────────────────────┤
│ Screen              │ Size     │ Modified │ Valid        │ Notes                               │
├─────────────────────┼──────────┼──────────┼──────────────┼─────────────────────────────────────┤
│ welcome.ans         │ 4.2 KB   │ Today    │ OK           │ Main welcome                        │
│ logon.ans           │ 2.1 KB   │ Today    │ OK           │ Login screen                        │
│ main-menu.ans       │ 3.8 KB   │ Today    │ Warning      │ Uses unsupported escape             │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Preview │ V Validate │ I Inspect │ R Reload │ D Duplicate │ Esc Back                         │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Config Screen Mockup

```text
┌──────────────────────────────────────── Config ─────────────────────────────────────────────────┐
│ File: config/oxidebbs.toml                                      Status: Valid                   │
├──────────────────────┬─────────────────────────────────────────────────────────────────────────┤
│ Section              │ Value                                                                   │
├──────────────────────┼─────────────────────────────────────────────────────────────────────────┤
│ board.name           │ Blackboard BBS                                                          │
│ telnet.bind          │ 0.0.0.0:2323                                                            │
│ nodes.count          │ 4                                                                       │
│ database.path        │ ./data/oxidebbs.ddb                                                     │
│ paths.ansi           │ ./assets/ansi                                                           │
│ paths.doors          │ ./doors                                                                 │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ C Check │ R Reload │ E External Edit │ P Paths │ Esc Back                                      │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Help Screen Mockup

```text
┌──────────────────────────────────────── Help: Doors ────────────────────────────────────────────┐
│ This screen manages DOS door game definitions and runtime testing.                              │
├─────────────────────────────────────── Common Actions ──────────────────────────────────────────┤
│ T    Test selected door                                                                          │
│ C    Check selected door configuration                                                           │
│ F    View generated drop file                                                                    │
│ L    View logs for selected door                                                                 │
│ D    Disable selected door                                                                       │
├─────────────────────────────────────── Tips ────────────────────────────────────────────────────┤
│ Use dry-run before launching a new DOS door.                                                     │
│ Check that the working directory and runtime directory are writable.                             │
│ Use drop-file viewer to confirm user name, node number, and time limit.                          │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Files to Modify

#### `crates/oxidebbs-sysop/src/screens/ansi.rs` (replace stub)

List ANSI screen assets from the configured path. Show file name, size, modified date, validation status. Support preview and validation.

#### `crates/oxidebbs-sysop/src/screens/config.rs` (replace stub)

Show loaded config as key-value table. Support check and reload.

#### `crates/oxidebbs-sysop/src/screens/help.rs` (replace stub)

Context-sensitive help. Show current screen name, description, keyboard shortcuts, and tips.

#### `crates/oxidebbs-sysop/src/command_palette.rs` (expand)

Add all commands to the palette:
- `users reset-password <alias>`
- `users list disabled`
- `doors test <door>`
- `doors check <door>`
- `db backup`
- `db doctor`
- `nodes disconnect <node>`
- `nodes broadcast`
- `messages delete <id>`
- All navigation commands

### Acceptance Criteria (Phase 7)

1. ANSI screen lists assets with validation status.
2. Config screen shows all config values.
3. Help screen shows context-sensitive help for each screen.
4. Command palette has all commands with fuzzy search.
5. Read-only mode hides destructive commands from palette.

---

## Phase 8: Integration

### Goal

Wire up `oxidebbs-server sysop` to launch the full TUI.

### Files to Modify

#### `crates/oxidebbs-server/src/commands/sysop.rs` (modify)

Use `run_sysop_tui()` for full TUI launch:

```rust
use oxidebbs_sysop::app::{AppConfig, run_tui};
use crate::sysop_cli::{AppContext, CliResult};

pub fn run_sysop_tui(ctx: &AppContext) -> CliResult<()> {
    let config = AppConfig {
        config_path: ctx.config_path.clone(),
        readonly: false,
        tick_rate: std::time::Duration::from_millis(250),
    };
    run_tui(config).map_err(|e| crate::sysop_cli::CliError::Message(
        format!("TUI error: {}", e)
    ))
}
```

#### `crates/oxidebbs-server/src/sysop_cli.rs` (modify)

Update the `Command::Sysop` match arm to call `run_sysop_tui`.

Add `--readonly` flag to the `Sysop` command variant.

### Acceptance Criteria (Phase 8)

1. `oxidebbs-server sysop` launches the full TUI.
2. `oxidebbs-server sysop --readonly` launches in read-only mode.
3. `oxidebbs-server sysop --config path/to/config.toml` uses the specified config.
4. The TUI connects to the database and shows live data.
5. The TUI connects to the control socket when the server is running.
6. All existing CLI tests still pass.

---

## Testing Requirements

### Unit Tests

Every service module should have unit tests using `OxideDb::open_memory()`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oxidebbs_db::OxideDb;

    #[test]
    fn list_users_returns_all_users() {
        let db = OxideDb::open_memory().expect("open db");
        // Insert test data...
        let users = UserAdminService::list(db.db()).expect("list");
        assert!(!users.is_empty());
    }
}
```

### Widget Tests

Test widgets render without panicking at various terminal sizes:

```rust
#[test]
fn node_map_renders_8_nodes() {
    let area = Rect::new(0, 0, 80, 10);
    let mut buffer = Buffer::empty(area);
    let nodes = vec![/* 8 test nodes */];
    NodeMapWidget {
        nodes: &nodes,
        total_configured: 8,
        selected: None,
        theme: &Theme::oxide_classic(),
    }.render(area, &mut buffer);
    // Assert rendered content
}
```

### Node Scaling Tests

Test the following configurations:
- 1 node
- 4 nodes
- 8 nodes
- 16 nodes
- 32 nodes
- 64 nodes
- No active nodes
- All active nodes
- One stale node
- Multiple door nodes
- Narrow terminal (80x25)
- Wide terminal (160x50)

### Integration Tests

Test the full TUI event loop with mock data:

```rust
#[test]
fn app_navigates_between_screens() {
    let mut app = App::new(AppConfig { ... });
    assert_eq!(app.current_screen, ScreenId::Dashboard);
    app.navigate_to(ScreenId::Nodes);
    assert_eq!(app.current_screen, ScreenId::Nodes);
}

#[test]
fn command_palette_filters_commands() {
    let mut palette = CommandPalette::new(/* commands */);
    palette.update_query("reset".into());
    assert!(palette.filtered.len() > 0);
}
```

### Acceptance Criteria (Testing)

1. All service modules have unit tests.
2. All widgets render correctly at 80x25 and 160x50.
3. Node scaling tests pass for 1, 4, 8, 16, 32, 64 nodes.
4. Navigation tests pass for all screen transitions.
5. Modal tests pass for confirm, form, and error modals.
6. Command palette fuzzy search works correctly.
7. `cargo test --workspace --locked` passes.

---

## Code Quality Standards

### Error Handling

Follow the pattern in `crates/oxidebbs-server/src/sysop_cli.rs`:

```rust
#[derive(Debug, Error)]
pub enum SysopError {
    #[error("database error: {0}")]
    Database(#[from] oxidebbs_db::DbError),

    #[error("door config error: {0}")]
    DoorConfig(#[from] oxidebbs_door::DoorError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("control socket error: {0}")]
    Control(String),

    #[error("{0}")]
    Message(String),
}
```

**Rules:**
- No `unwrap()` or `expect()` in library code.
- All public functions return `Result<T, SysopError>`.
- Use `?` operator for error propagation.
- Provide meaningful error messages.

### Audit Logging

Every admin write action must be audit logged:

```rust
pub fn audit_action(
    db: &Db,
    event_type: &str,
    user_id: Option<&str>,
    node_number: Option<i64>,
    details: &str,
) -> Result<(), SysopError> {
    insert_audit_event(db, &AuditEventRecord {
        id: generate_uuid(),
        created_at: current_timestamp(),
        event_type: event_type.to_string(),
        user_id: user_id.map(ToOwned::to_owned),
        node_number,
        details: details.to_string(),
    })?;
    Ok(())
}
```

**Audit event types:**
- `user_added`, `user_disabled`, `user_enabled`
- `user_password_reset`, `user_security_level_changed`
- `user_promoted_sysop`, `user_demoted_sysop`
- `node_disconnected`, `node_message_sent`, `broadcast_sent`
- `door_test_started`, `door_disabled`, `door_enabled`
- `message_deleted`
- `db_backup_started`, `db_doctor_run`

### Read-Only Mode

When `readonly` is true:
- Hide all destructive actions from menus and keyboard shortcuts.
- Hide write commands from command palette.
- Show "(read-only)" in header.
- Allow all read operations.

### No Locks Across Await

Never hold a `Mutex` or `RwLock` guard across an `.await` point. Use `tokio::sync::Mutex` if async locking is needed.

### Formatting

Run `cargo fmt --all` after every change.

### Clippy

Run `cargo clippy --workspace --all-targets --locked -- -D warnings` and fix all warnings.

---

## Validation

### CI Gate

After completing each phase, run the full CI gate:

```bash
./scripts/dev-check.sh
```

This runs:
1. `cargo fmt --all --check`
2. `cargo check --workspace --locked`
3. `cargo test --workspace --locked`
4. `cargo clippy --workspace --all-targets --locked -- -D warnings`

All four must pass before considering a phase complete.

### Manual Testing

1. Launch the TUI: `cargo run --bin oxidebbs-server -- sysop`
2. Navigate through all screens.
3. Test keyboard shortcuts.
4. Test modals (confirm, form, error).
5. Test command palette.
6. Test with a running server (control socket).
7. Test with no running server (DB-only mode).
8. Test read-only mode.
9. Test terminal resize.
10. Test at 80x25 and 160x50.

### Checklist

- [x] Phase 1: Foundation — app shell, theme, navigation, keyboard handling
- [x] Phase 2: Dashboard and Nodes — live dashboard, node views, node actions
- [x] Phase 3: Users — user management screens
- [x] Phase 4: Doors — door management and troubleshooting
- [x] Phase 5: Messages — message area and message management
- [x] Phase 6: Database, Logs, Audit — DB health, logs, audit
- [x] Phase 7: Polish — ANSI, Config, Help, command palette completion
- [x] Phase 8: Integration — wire up `oxidebbs-server sysop`
- [x] All tests pass locally
- [ ] CI gate passes
- [ ] Manual testing complete

---

## Summary

This prompt describes the complete implementation of the OxideBBS Sysop TUI. The TUI is a full-featured, keyboard-driven, Ratatui-based local administration console that provides:

- **Dashboard** with live node map, recent events, health panel, and alerts
- **Nodes** with table/grid/active/problem views, disconnect, message, broadcast
- **Users** with list, search, filter, detail, add, edit, reset password, enable/disable
- **Doors** with list, detail, config check, drop-file viewer, dry-run, test, run history
- **Messages** with area list, message list, detail, delete
- **Database** with status, backup, doctor, verify
- **Logs** with tail, filter, search, follow
- **Audit** with search and filter by user/node/door
- **ANSI** with asset list, preview, validation
- **Config** with key-value display, check, reload
- **Help** with context-sensitive shortcuts and tips
- **Command Palette** with fuzzy search for all commands

The implementation follows the architecture in the master spec, uses the same service layer as the CLI, and adheres to all hard constraints from AGENTS.md.

**Build in this order:**
1. TUI shell with mock data (Phase 1)
2. Dashboard and Nodes with real data (Phase 2)
3. Users (Phase 3)
4. Doors (Phase 4)
5. Messages (Phase 5)
6. Database, Logs, Audit (Phase 6)
7. Polish (Phase 7)
8. Integration (Phase 8)

Each phase should pass the CI gate before proceeding to the next.
