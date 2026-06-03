use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use ratatui::widgets::ListState;

use oxidebbs_db::OxideDb;

use crate::command_palette::{CommandPalette, PaletteAction, PaletteCommand};
use crate::events::{AppEvent, EventHandler};
use crate::input::{ScreenId, UiEvent, translate_key};
use crate::screens::ansi::AnsiScreen;
use crate::screens::audit::AuditScreen;
use crate::screens::common::UiAction;
use crate::screens::config::ConfigScreen;
use crate::screens::dashboard::DashboardScreen;
use crate::screens::database::DatabaseScreen;
use crate::screens::doors::DoorsScreen;
use crate::screens::help::HelpScreen;
use crate::screens::logs::LogsScreen;
use crate::screens::messages::MessagesScreen;
use crate::screens::nodes::NodesScreen;
use crate::screens::users::UsersScreen;
use crate::services::node_service::NodeAdminService;
use crate::theme::Theme;
use crate::widgets::header::HeaderWidget;
use crate::widgets::modal::{ModalKind, render_modal};
use crate::widgets::nav_rail::NavRail;
use crate::widgets::status_bar::StatusBar;

pub struct AppConfig {
    pub config_path: PathBuf,
    pub readonly: bool,
    pub tick_rate: Duration,
    pub db_path: Option<PathBuf>,
    pub control_socket_path: Option<PathBuf>,
    pub node_count: u16,
    pub board_name: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_path: PathBuf::from("config/oxidebbs.toml"),
            readonly: false,
            tick_rate: Duration::from_millis(250),
            db_path: None,
            control_socket_path: None,
            node_count: 8,
            board_name: "OxideBBS".to_string(),
        }
    }
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
    pub node_service: NodeAdminService,
    pub dashboard: DashboardScreen,
    pub nodes_screen: NodesScreen,
    pub users_screen: UsersScreen,
    pub messages_screen: MessagesScreen,
    pub doors_screen: DoorsScreen,
    pub ansi_screen: AnsiScreen,
    pub config_screen: ConfigScreen,
    pub database_screen: DatabaseScreen,
    pub logs_screen: LogsScreen,
    pub audit_screen: AuditScreen,
    pub help_screen: HelpScreen,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let theme = Theme::oxide_classic();
        let mut nav_state = ListState::default();
        nav_state.select(Some(0));

        let commands = Self::build_palette_commands(config.readonly);
        let node_count = config.node_count;
        let board_name = config.board_name.clone();

        let node_service = NodeAdminService::new(config.control_socket_path.clone());

        Self {
            theme: theme.clone(),
            current_screen: ScreenId::Dashboard,
            nav_state,
            modal: None,
            command_palette: CommandPalette::new(commands),
            db: None,
            config,
            should_quit: false,
            board_name,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: 0,
            node_count,
            active_nodes: 0,
            user_count: 0,
            alert_count: 0,
            node_service,
            dashboard: DashboardScreen::new(theme.clone(), node_count),
            nodes_screen: NodesScreen::new(theme.clone(), node_count),
            users_screen: UsersScreen::new(theme.clone()),
            messages_screen: MessagesScreen::new(theme.clone()),
            doors_screen: DoorsScreen::new(theme.clone()),
            ansi_screen: AnsiScreen::new(theme.clone()),
            config_screen: ConfigScreen::new(theme.clone()),
            database_screen: DatabaseScreen::new(theme.clone()),
            logs_screen: LogsScreen::new(theme.clone()),
            audit_screen: AuditScreen::new(theme.clone()),
            help_screen: HelpScreen::new(theme.clone()),
        }
    }

    fn build_palette_commands(readonly: bool) -> Vec<PaletteCommand> {
        let mut commands = vec![
            PaletteCommand {
                id: "nav.dashboard".into(),
                label: "Go to Dashboard".into(),
                description: "Open the main dashboard".into(),
                shortcut: None,
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Dashboard),
            },
            PaletteCommand {
                id: "nav.nodes".into(),
                label: "Go to Nodes".into(),
                description: "View and manage nodes".into(),
                shortcut: Some("Ctrl+N".into()),
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Nodes),
            },
            PaletteCommand {
                id: "nav.users".into(),
                label: "Go to Users".into(),
                description: "View and manage users".into(),
                shortcut: Some("Ctrl+U".into()),
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Users),
            },
            PaletteCommand {
                id: "nav.doors".into(),
                label: "Go to Doors".into(),
                description: "Manage door definitions".into(),
                shortcut: Some("Ctrl+D".into()),
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Doors),
            },
            PaletteCommand {
                id: "nav.messages".into(),
                label: "Go to Messages".into(),
                description: "Manage message areas".into(),
                shortcut: Some("Ctrl+M".into()),
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Messages),
            },
            PaletteCommand {
                id: "nav.logs".into(),
                label: "Go to Logs".into(),
                description: "View server logs".into(),
                shortcut: Some("Ctrl+L".into()),
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Logs),
            },
            PaletteCommand {
                id: "nav.database".into(),
                label: "Go to Database".into(),
                description: "Database health and backup".into(),
                shortcut: Some("Ctrl+B".into()),
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Database),
            },
            PaletteCommand {
                id: "nav.audit".into(),
                label: "Go to Audit".into(),
                description: "View audit events".into(),
                shortcut: None,
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Audit),
            },
            PaletteCommand {
                id: "nav.help".into(),
                label: "Go to Help".into(),
                description: "Show help".into(),
                shortcut: Some("F1".into()),
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Help),
            },
        ];
        if !readonly {
            commands.push(PaletteCommand {
                id: "nodes.reset_stale".into(),
                label: "Reset Stale Nodes".into(),
                description: "Reset all stale node sessions".into(),
                shortcut: None,
                is_destructive: true,
                action: PaletteAction::RunCommand("nodes.reset_stale".into()),
            });
        }
        commands
    }

    pub fn navigate_to(&mut self, screen: ScreenId) {
        self.current_screen = screen;
        let idx = ScreenId::all()
            .iter()
            .position(|s| *s == screen)
            .unwrap_or(0);
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

    fn refresh_data(&mut self) {
        if let Some(ref db) = self.db {
            self.dashboard.refresh(db.db(), &self.node_service);
            self.nodes_screen.refresh(db.db(), &self.node_service);
            self.users_screen.refresh(db);
            self.doors_screen.refresh(db);
            self.messages_screen.refresh(db);
            self.database_screen.refresh(db);
            self.audit_screen.refresh(db);
            self.active_nodes = self
                .dashboard
                .nodes
                .iter()
                .filter(|n| n.state != "available" && n.state != "offline")
                .count();
            if let Ok(count) =
                crate::services::database_service::DatabaseAdminService::count_users(db.db())
            {
                self.user_count = count as usize;
            }
        }
    }

    fn delegate_event(&mut self, event: UiEvent) -> UiAction {
        match self.current_screen {
            ScreenId::Dashboard => {
                self.dashboard
                    .handle_event(event, &self.db, self.config.readonly)
            }
            ScreenId::Nodes => self.nodes_screen.handle_event(
                event,
                &self.db,
                &self.node_service,
                self.config.readonly,
            ),
            ScreenId::Users => {
                self.users_screen
                    .handle_event(event, &self.db, self.config.readonly)
            }
            ScreenId::Doors => {
                self.doors_screen
                    .handle_event(event, &self.db, self.config.readonly)
            }
            ScreenId::Messages => {
                self.messages_screen
                    .handle_event(event, &self.db, self.config.readonly)
            }
            ScreenId::Database => {
                self.database_screen
                    .handle_event(event, &self.db, self.config.readonly)
            }
            ScreenId::Audit => {
                self.audit_screen
                    .handle_event(event, &self.db, self.config.readonly)
            }
            ScreenId::Logs => self
                .logs_screen
                .handle_event(event, &self.db, self.config.readonly),
            ScreenId::Ansi => self
                .ansi_screen
                .handle_event(event, &self.db, self.config.readonly),
            ScreenId::Config => {
                self.config_screen
                    .handle_event(event, &self.db, self.config.readonly)
            }
            _ => UiAction::None,
        }
    }

    fn render_current_screen(&self, frame: &mut Frame, area: Rect) {
        match self.current_screen {
            ScreenId::Dashboard => self.dashboard.render(frame, area),
            ScreenId::Nodes => self.nodes_screen.render(frame, area),
            ScreenId::Users => self.users_screen.render(frame, area),
            ScreenId::Messages => self.messages_screen.render(frame, area),
            ScreenId::Doors => self.doors_screen.render(frame, area),
            ScreenId::Ansi => self.ansi_screen.render(frame, area),
            ScreenId::Config => self.config_screen.render(frame, area),
            ScreenId::Database => self.database_screen.render(frame, area),
            ScreenId::Logs => self.logs_screen.render(frame, area),
            ScreenId::Audit => self.audit_screen.render(frame, area),
            ScreenId::Help => self.help_screen.render(frame, area),
        }
    }
}

pub async fn run_tui(config: AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);

    let db_path = app
        .config
        .db_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("data/oxidebbs.ddb"));
    if let Ok(db) = OxideDb::open_or_create(&db_path) {
        app.db = Some(db);
    }
    app.refresh_data();

    let mut events = EventHandler::new(app.config.tick_rate);

    loop {
        terminal.draw(|frame| {
            render_app(&mut app, frame);
        })?;

        if let Some(event) = events.next().await {
            match event {
                AppEvent::Key(key) => {
                    let ui_event = translate_key(key);
                    handle_ui_event(&mut app, ui_event);
                }
                AppEvent::Tick => {
                    app.refresh_data();
                }
                AppEvent::Resize(_, _) => {}
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

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(10),   // body
            Constraint::Length(3), // footer
        ])
        .split(area);

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

    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16), // nav rail
            Constraint::Min(40),    // content
        ])
        .split(main_layout[1]);

    let screens = ScreenId::all();
    NavRail {
        items: screens,
        selected: app.nav_state.selected().unwrap_or(0),
        theme: &app.theme,
    }
    .render(body_layout[0], frame.buffer_mut(), &mut app.nav_state);

    app.render_current_screen(frame, body_layout[1]);

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

    if let Some(modal) = &app.modal {
        render_modal(modal, frame, &app.theme);
    }
}

fn handle_ui_event(app: &mut App, event: UiEvent) {
    // Handle modal first
    if app.modal.is_some() {
        match event {
            UiEvent::Cancel => {
                app.modal = None;
            }
            UiEvent::Confirm => {
                if let Some(ModalKind::Form(ref m)) = app.modal.take() {
                    if m.title == "Send Message" {
                        if let Some(node_field) = m.fields.first()
                            && let Some(msg_field) = m.fields.get(1)
                            && let Ok(node_num) = node_field.value.parse::<u16>()
                        {
                            let _ = app.node_service.send_message(node_num, &msg_field.value);
                        }
                    } else if m.title == "Broadcast Message" {
                        if let Some(msg_field) = m.fields.first() {
                            let _ = app.node_service.broadcast(&msg_field.value);
                        }
                    } else if m.title == "Filter Nodes"
                        && let Some(field) = m.fields.first()
                    {
                        app.nodes_screen.filter = field.value.clone();
                    } else if m.title == "Filter Users"
                        && let Some(field) = m.fields.first()
                    {
                        app.users_screen.filter = field.value.clone();
                    } else if m.title == "Reset Password" {
                        if let Some(user_field) = m.fields.first()
                            && let Some(pw_field) = m.fields.get(1)
                        {
                            let alias = user_field.value.clone();
                            let password = pw_field.value.clone();
                            if let Some(db) = &app.db
                                && let Ok(Some(user)) =
                                    crate::services::user_service::UserAdminService::find_by_alias(
                                        db.db(),
                                        &alias,
                                    )
                            {
                                let _ =
                                    crate::services::user_service::UserAdminService::reset_password(
                                        db.db(),
                                        &user.id,
                                        &password,
                                    );
                            }
                        }
                    } else if m.title == "Set Security Level"
                        && let Some(user_field) = m.fields.first()
                        && let Some(level_field) = m.fields.get(1)
                        && let Ok(level) = level_field.value.parse::<i64>()
                    {
                        let alias = user_field.value.clone();
                        if let Some(db) = &app.db
                            && let Ok(Some(user)) =
                                crate::services::user_service::UserAdminService::find_by_alias(
                                    db.db(),
                                    &alias,
                                )
                        {
                            let _ =
                                crate::services::user_service::UserAdminService::set_security_level(
                                    db.db(),
                                    &user.id,
                                    level,
                                );
                        }
                    }
                } else if let Some(ModalKind::Confirm(ref m)) = app.modal.take()
                    && m.title == "Delete Message"
                    && let Some(db) = &app.db
                    && let Some(msg_id) = app.messages_screen.selected_message_id()
                {
                    let _ = crate::services::message_service::MessageAdminService::delete_message(
                        db.db(),
                        &msg_id,
                    );
                }
            }
            UiEvent::Key(key) => {
                use crossterm::event::KeyCode;
                if let Some(ModalKind::Form(ref mut m)) = app.modal {
                    match key.code {
                        KeyCode::Up => {
                            if m.active_field > 0 {
                                m.active_field -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if m.active_field + 1 < m.fields.len() {
                                m.active_field += 1;
                            }
                        }
                        KeyCode::Char(c) => {
                            if let Some(field) = m.fields.get_mut(m.active_field) {
                                field.value.push(c);
                            }
                        }
                        KeyCode::Backspace => {
                            if let Some(field) = m.fields.get_mut(m.active_field) {
                                field.value.pop();
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        return;
    }

    // Handle command palette
    if app.command_palette.visible {
        match event {
            UiEvent::Cancel => {
                app.command_palette.close();
            }
            UiEvent::Confirm => {
                if let Some(cmd) = app.command_palette.selected_command().cloned() {
                    app.command_palette.close();
                    match cmd.action {
                        PaletteAction::Navigate(screen) => app.navigate_to(screen),
                        PaletteAction::RunCommand(ref id) => {
                            if id == "nodes.reset_stale" {
                                let _ = app.node_service.reset_stale();
                            }
                        }
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

    // Global navigation shortcuts
    match event {
        UiEvent::Quit => {
            app.should_quit = true;
        }
        UiEvent::Help => {
            app.navigate_to(ScreenId::Help);
        }
        UiEvent::CommandPalette => {
            app.command_palette.open();
        }
        UiEvent::FocusNext => {
            app.nav_next();
        }
        UiEvent::FocusPrev => {
            app.nav_prev();
        }
        UiEvent::NavigateTo(screen) => {
            app.navigate_to(screen);
        }
        UiEvent::Refresh => {
            app.refresh_data();
        }
        _ => {
            let action = app.delegate_event(event);
            match action {
                UiAction::None => {}
                UiAction::Navigate(screen) => app.navigate_to(screen),
                UiAction::OpenModal(modal) => app.modal = Some(modal),
                UiAction::Refresh => app.refresh_data(),
                UiAction::Quit => app.should_quit = true,
            }
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
