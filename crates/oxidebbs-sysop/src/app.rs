use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, ListState, Paragraph, Wrap};

use oxidebbs_db::OxideDb;

use crate::SysopError;
use crate::command_palette::{CommandPalette, PaletteAction, PaletteCommand};
use crate::events::{AppEvent, EventHandler};
use crate::input::{ScreenId, UiEvent, translate_key};
use crate::screens::ansi::AnsiScreen;
use crate::screens::audit::AuditScreen;
use crate::screens::common::UiAction;
use crate::screens::config::ConfigScreen;
use crate::screens::dashboard::DashboardScreen;
use crate::screens::database::DatabaseScreen;
use crate::screens::doctor::DoctorScreen;
use crate::screens::doors::DoorsScreen;
use crate::screens::files::FilesScreen;
use crate::screens::help::HelpScreen;
use crate::screens::logs::LogsScreen;
use crate::screens::messages::MessagesScreen;
use crate::screens::network::NetworkScreen;
use crate::screens::nodes::NodesScreen;
use crate::screens::oxidenet::OxideNetScreen;
use crate::screens::users::UsersScreen;
use crate::services::node_service::NodeAdminService;
use crate::theme::Theme;
use crate::widgets::header::HeaderWidget;
use crate::widgets::modal::{
    ConfirmModal, ErrorModal, FormField, FormModal, InfoModal, ModalKind, centered_rect,
    render_modal,
};
use crate::widgets::nav_rail::NavRail;
use crate::widgets::status_bar::StatusBar;

pub struct AppConfig {
    pub config_path: PathBuf,
    pub readonly: bool,
    pub confirm_quit: bool,
    pub tick_rate: Duration,
    pub db_path: Option<PathBuf>,
    pub logs_path: Option<PathBuf>,
    pub screens_path: Option<PathBuf>,
    pub control_socket_path: Option<PathBuf>,
    pub node_count: u16,
    pub theme_name: String,
    pub board_name: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_path: PathBuf::from("config/oxidebbs.toml"),
            readonly: false,
            confirm_quit: true,
            tick_rate: Duration::from_millis(250),
            db_path: None,
            logs_path: None,
            screens_path: None,
            control_socket_path: None,
            node_count: 8,
            theme_name: "oxide-classic".to_string(),
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
    pub files_screen: FilesScreen,
    pub network_screen: NetworkScreen,
    pub oxidenet_screen: OxideNetScreen,
    pub doors_screen: DoorsScreen,
    pub ansi_screen: AnsiScreen,
    pub config_screen: ConfigScreen,
    pub database_screen: DatabaseScreen,
    pub doctor_screen: DoctorScreen,
    pub logs_screen: LogsScreen,
    pub audit_screen: AuditScreen,
    pub help_screen: HelpScreen,
    pub status_message: Option<String>,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let theme = Theme::from_name(&config.theme_name).unwrap_or_else(Theme::oxide_classic);
        let mut nav_state = ListState::default();
        nav_state.select(Some(0));

        let commands = Self::build_palette_commands(config.readonly);
        let node_count = config.node_count;
        let board_name = config.board_name.clone();
        let config_path = config.config_path.clone();
        let db_path = config.db_path.clone();
        let logs_path = config
            .logs_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("logs"));
        let screens_path = config
            .screens_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("assets/screens"));

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
            files_screen: FilesScreen::new(theme.clone()),
            network_screen: NetworkScreen::new(theme.clone()),
            oxidenet_screen: OxideNetScreen::new(theme.clone()),
            doors_screen: DoorsScreen::new(theme.clone()),
            ansi_screen: AnsiScreen::new(theme.clone(), screens_path),
            config_screen: ConfigScreen::new(theme.clone(), config_path),
            database_screen: DatabaseScreen::new(theme.clone(), db_path),
            doctor_screen: DoctorScreen::new(theme.clone()),
            logs_screen: LogsScreen::new(theme.clone(), logs_path),
            audit_screen: AuditScreen::new(theme.clone()),
            help_screen: HelpScreen::new(theme.clone()),
            status_message: None,
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
                id: "nav.files".into(),
                label: "Go to Files".into(),
                description: "Manage file areas, uploads, and transfer history".into(),
                shortcut: Some("Ctrl+F".into()),
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Files),
            },
            PaletteCommand {
                id: "nav.network".into(),
                label: "Go to Network".into(),
                description: "View FTN and OxideNet status".into(),
                shortcut: Some("Ctrl+X".into()),
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Network),
            },
            PaletteCommand {
                id: "nav.oxidenet".into(),
                label: "Go to OxideNet".into(),
                description: "Manage OxideNet applications, nodes, queues, and nodelists".into(),
                shortcut: Some("Ctrl+O".into()),
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::OxideNet),
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
                id: "nav.doctor".into(),
                label: "Go to Doctor".into(),
                description: "Run verbose sysop health checks".into(),
                shortcut: None,
                is_destructive: false,
                action: PaletteAction::Navigate(ScreenId::Doctor),
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
            commands.extend([
                PaletteCommand {
                    id: "oxidenet.install_hub".into(),
                    label: "Install OxideNet Hub".into(),
                    description: "Install default OxideNet hub profile, node, and areas".into(),
                    shortcut: None,
                    is_destructive: true,
                    action: PaletteAction::RunCommand("oxidenet.install_hub".into()),
                },
                PaletteCommand {
                    id: "oxidenet.generate_nodelist".into(),
                    label: "Generate OxideNet Nodelist".into(),
                    description: "Publish the OxideNet nodelist from the registry".into(),
                    shortcut: None,
                    is_destructive: true,
                    action: PaletteAction::RunCommand("oxidenet.generate_nodelist".into()),
                },
            ]);
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
        if screen == ScreenId::Doctor {
            self.run_doctor();
        }
    }

    pub fn nav_next(&mut self) {
        let screens = ScreenId::all();
        let current = self.nav_state.selected().unwrap_or(0);
        let next = (current + 1).min(screens.len() - 1);
        self.navigate_to(screens[next]);
    }

    pub fn nav_prev(&mut self) {
        let screens = ScreenId::all();
        let current = self.nav_state.selected().unwrap_or(0);
        let prev = current.saturating_sub(1);
        self.navigate_to(screens[prev]);
    }

    fn refresh_data(&mut self) {
        self.config_screen.refresh();
        self.ansi_screen.refresh();
        self.logs_screen.refresh();
        if let Some(ref db) = self.db {
            self.dashboard.refresh(db.db(), &self.node_service);
            self.nodes_screen.refresh(db.db(), &self.node_service);
            self.users_screen.refresh(db);
            self.doors_screen.refresh(db);
            self.messages_screen.refresh(db);
            self.files_screen.refresh(db);
            self.network_screen.refresh(db);
            self.oxidenet_screen.refresh(db);
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

    fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
    }

    fn resolved_db_path(&self) -> PathBuf {
        self.config
            .db_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("data/oxidebbs.ddb"))
    }

    fn run_doctor(&mut self) {
        let db_path = self.resolved_db_path();
        self.doctor_screen
            .refresh(self.db.as_ref(), Some(db_path.as_path()), self.node_count);
        if let Some(report) = &self.doctor_screen.report {
            self.set_status(format!(
                "Doctor complete: {} passed, {} warnings, {} failed",
                report.passed_count(),
                report.warning_count(),
                report.failed_count()
            ));
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
            ScreenId::Files => {
                self.files_screen
                    .handle_event(event, &self.db, self.config.readonly)
            }
            ScreenId::Network => {
                self.network_screen
                    .handle_event(event, &self.db, self.config.readonly)
            }
            ScreenId::OxideNet => {
                self.oxidenet_screen
                    .handle_event(event, &self.db, self.config.readonly)
            }
            ScreenId::Database => {
                self.database_screen
                    .handle_event(event, &self.db, self.config.readonly)
            }
            ScreenId::Doctor => {
                self.doctor_screen
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
            ScreenId::Files => self.files_screen.render(frame, area),
            ScreenId::Network => self.network_screen.render(frame, area),
            ScreenId::OxideNet => self.oxidenet_screen.render(frame, area),
            ScreenId::Doors => self.doors_screen.render(frame, area),
            ScreenId::Ansi => self.ansi_screen.render(frame, area),
            ScreenId::Config => self.config_screen.render(frame, area),
            ScreenId::Database => self.database_screen.render(frame, area),
            ScreenId::Doctor => self.doctor_screen.render(frame, area),
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
                    request_quit(&mut app);
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
            message: app.status_message.as_deref(),
            theme: &app.theme,
        },
        main_layout[2],
    );

    if let Some(modal) = &app.modal {
        render_modal(modal, frame, &app.theme);
    }

    if app.command_palette.visible {
        render_command_palette(app, frame);
    }
}

fn render_command_palette(app: &mut App, frame: &mut Frame) {
    let area = centered_rect(64, 58, frame.area());
    frame.render_widget(Clear, area);

    let max_commands = usize::from(area.height.saturating_sub(6).max(1));
    let selected = app
        .command_palette
        .selected
        .min(app.command_palette.filtered.len().saturating_sub(1));
    let start = selected.saturating_sub(max_commands.saturating_sub(1));
    let end = (start + max_commands).min(app.command_palette.filtered.len());
    let query = if app.command_palette.query.is_empty() {
        "<type to filter>".to_string()
    } else {
        app.command_palette.query.clone()
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Search: ", app.theme.label_style()),
            Span::styled(query, app.theme.normal_style()),
        ]),
        Line::from(""),
    ];

    if app.command_palette.filtered.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "No matching commands",
            app.theme.muted_style(),
        )]));
    } else {
        for (visible_row, command_index) in
            app.command_palette.filtered[start..end].iter().enumerate()
        {
            let row = start + visible_row;
            let command = &app.command_palette.commands[*command_index];
            let marker = if row == selected { "> " } else { "  " };
            let shortcut = command
                .shortcut
                .as_ref()
                .map(|value| format!(" [{}]", value))
                .unwrap_or_default();
            let style = if row == selected {
                app.theme.selected_style()
            } else if command.is_destructive {
                app.theme.warning_style()
            } else {
                app.theme.normal_style()
            };
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(command.label.as_str(), style),
                Span::styled(shortcut, app.theme.muted_style()),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(command.description.as_str(), app.theme.muted_style()),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Enter", app.theme.label_style()),
        Span::styled(" run  ", app.theme.muted_style()),
        Span::styled("Esc/F2", app.theme.label_style()),
        Span::styled(" close", app.theme.muted_style()),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.block_style(true))
        .title(" Command Palette ")
        .title_style(app.theme.title_style());
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn handle_ui_event(app: &mut App, event: UiEvent) {
    // Handle modal first
    if app.modal.is_some() {
        match event {
            UiEvent::Cancel => {
                app.users_screen.cancel_pending_action();
                app.doors_screen.cancel_pending_action();
                app.messages_screen.cancel_pending_action();
                app.files_screen.cancel_pending_action();
                app.oxidenet_screen.cancel_pending_action();
                app.modal = None;
            }
            UiEvent::Confirm => {
                if let Some(modal) = app.modal.take() {
                    match modal {
                        ModalKind::Form(form) => {
                            if app.config.readonly && form_submit_mutates(&form.title) {
                                block_readonly_action(app, &form.title);
                            } else {
                                handle_form_submit(app, form);
                            }
                        }
                        ModalKind::Confirm(confirm) => {
                            if app.config.readonly && confirm_submit_mutates(&confirm.title) {
                                block_readonly_action(app, &confirm.title);
                            } else {
                                handle_confirm_submit(app, &confirm.title);
                            }
                        }
                        ModalKind::Error(_) | ModalKind::Info(_) => {}
                    }
                }
            }
            UiEvent::Key(key) => {
                use crossterm::event::KeyCode;
                match app.modal {
                    Some(ModalKind::Form(ref mut m)) => match key.code {
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
                    },
                    Some(ModalKind::Confirm(_)) => match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            if let Some(ModalKind::Confirm(confirm)) = app.modal.take() {
                                if app.config.readonly && confirm_submit_mutates(&confirm.title) {
                                    block_readonly_action(app, &confirm.title);
                                } else {
                                    handle_confirm_submit(app, &confirm.title);
                                }
                            }
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            app.users_screen.cancel_pending_action();
                            app.doors_screen.cancel_pending_action();
                            app.messages_screen.cancel_pending_action();
                            app.files_screen.cancel_pending_action();
                            app.oxidenet_screen.cancel_pending_action();
                            app.modal = None;
                        }
                        _ => {}
                    },
                    Some(ModalKind::Error(_)) | Some(ModalKind::Info(_)) | None => {}
                }
            }
            _ => {}
        }
        return;
    }

    // Handle command palette
    if app.command_palette.visible {
        match event {
            UiEvent::CommandPalette => {
                app.command_palette.close();
            }
            UiEvent::Cancel => {
                app.command_palette.close();
            }
            UiEvent::Quit => {
                app.command_palette.close();
            }
            UiEvent::Help => {
                app.command_palette.close();
                app.navigate_to(ScreenId::Help);
            }
            UiEvent::Confirm => {
                if let Some(cmd) = app.command_palette.selected_command().cloned() {
                    app.command_palette.close();
                    match cmd.action {
                        PaletteAction::Navigate(screen) => app.navigate_to(screen),
                        PaletteAction::RunCommand(ref id) => {
                            if app.config.readonly {
                                block_readonly_action(app, &cmd.label);
                            } else if id == "nodes.reset_stale" {
                                match app.node_service.reset_stale() {
                                    Ok(()) => {
                                        if let Some(db) = &app.db {
                                            let _ = crate::services::audit_service::AuditService::record(
                                                db.db(),
                                                "nodes_reset_stale",
                                                None,
                                                None,
                                                "reset stale nodes from sysop TUI",
                                            );
                                        }
                                        app.refresh_data();
                                    }
                                    Err(error) => set_error(app, "Reset Stale Nodes", error),
                                }
                            } else if id == "oxidenet.install_hub" {
                                if let Some(db) = &app.db {
                                    match crate::services::oxidenet_service::OxideNetAdminService::install_hub_defaults(db) {
                                        Ok(()) => {
                                            app.refresh_data();
                                            app.set_status("OxideNet hub defaults installed");
                                        }
                                        Err(error) => set_error(app, "Install OxideNet Hub", error),
                                    }
                                }
                            } else if id == "oxidenet.generate_nodelist"
                                && let Some(db) = &app.db
                            {
                                match crate::services::oxidenet_service::OxideNetAdminService::generate_nodelist(db) {
                                    Ok(count) => {
                                        app.refresh_data();
                                        app.set_status(format!("OxideNet nodelist generated with {count} entries"));
                                    }
                                    Err(error) => set_error(app, "Generate OxideNet Nodelist", error),
                                }
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
            request_quit(app);
        }
        UiEvent::Help => {
            app.navigate_to(ScreenId::Help);
        }
        UiEvent::CommandPalette => {
            app.command_palette.open();
            app.set_status("Command palette opened");
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
            refresh_for_current_screen(app);
        }
        UiEvent::Search => {
            if !open_search_for_current_screen(app) {
                set_info_message(
                    app,
                    "Search",
                    "Search/filter is available on Nodes, Users, Doors, and Audit.",
                );
            }
        }
        UiEvent::Confirm | UiEvent::Cancel => {
            let action = app.delegate_event(screen_event_for_semantic(event));
            apply_ui_action(app, action);
        }
        _ => {
            let action = app.delegate_event(event);
            apply_ui_action(app, action);
        }
    }
}

fn screen_event_for_semantic(event: UiEvent) -> UiEvent {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    match event {
        UiEvent::Confirm => UiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        UiEvent::Cancel => UiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        UiEvent::Search => UiEvent::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)),
        other => other,
    }
}

fn apply_ui_action(app: &mut App, action: UiAction) {
    match action {
        UiAction::None => {}
        UiAction::Navigate(screen) => app.navigate_to(screen),
        UiAction::OpenModal(modal) => app.modal = Some(modal),
        UiAction::Refresh => {
            refresh_for_current_screen(app);
        }
        UiAction::Quit => app.should_quit = true,
    }
}

fn form_submit_mutates(title: &str) -> bool {
    matches!(
        title,
        "Send Message"
            | "Broadcast Message"
            | "Reset Password"
            | "Set Security Level"
            | "Set Config Value"
    )
}

fn confirm_submit_mutates(title: &str) -> bool {
    !matches!(title, "Quit Sysop TUI" | "Active Nodes")
}

fn block_readonly_action(app: &mut App, title: &str) {
    app.users_screen.cancel_pending_action();
    app.doors_screen.cancel_pending_action();
    app.messages_screen.cancel_pending_action();
    app.files_screen.cancel_pending_action();
    app.oxidenet_screen.cancel_pending_action();
    set_info_message(
        app,
        "Read Only",
        &format!("{title} is unavailable while the sysop TUI is running in read-only mode."),
    );
}

fn request_quit(app: &mut App) {
    app.refresh_data();
    if app.active_nodes > 0 {
        app.modal = Some(ModalKind::Confirm(ConfirmModal {
            title: "Active Nodes".to_string(),
            message: "Nodes are active. Continue to shutdown?".to_string(),
            detail: Some(format!(
                "{} active node(s) are currently in use. If this sysop session started an embedded server, continuing will stop it and disconnect callers.",
                app.active_nodes
            )),
            confirm_label: "Shutdown".to_string(),
            cancel_label: "Cancel".to_string(),
        }));
        app.set_status("Quit confirmation required: active nodes");
    } else if app.config.confirm_quit {
        app.modal = Some(ModalKind::Confirm(ConfirmModal {
            title: "Quit Sysop TUI".to_string(),
            message: "Quit the sysop TUI?".to_string(),
            detail: Some(
                "If this sysop session started an embedded server, quitting will stop it."
                    .to_string(),
            ),
            confirm_label: "Quit".to_string(),
            cancel_label: "Cancel".to_string(),
        }));
        app.set_status("Quit confirmation required");
    } else {
        app.should_quit = true;
    }
}

fn refresh_for_current_screen(app: &mut App) {
    app.refresh_data();
    if app.current_screen == ScreenId::Doctor {
        app.run_doctor();
    } else {
        app.set_status("Refreshed");
    }
}

fn open_search_for_current_screen(app: &mut App) -> bool {
    let modal = match app.current_screen {
        ScreenId::Nodes => Some(ModalKind::Form(FormModal {
            title: "Filter Nodes".to_string(),
            fields: vec![FormField {
                label: "Filter".to_string(),
                value: app.nodes_screen.filter.clone(),
                is_password: false,
            }],
            active_field: 0,
        })),
        ScreenId::Users => Some(ModalKind::Form(FormModal {
            title: "Filter Users".to_string(),
            fields: vec![FormField {
                label: "Filter".to_string(),
                value: app.users_screen.filter.clone(),
                is_password: false,
            }],
            active_field: 0,
        })),
        ScreenId::Doors => Some(ModalKind::Form(FormModal {
            title: "Filter Doors".to_string(),
            fields: vec![FormField {
                label: "Filter".to_string(),
                value: app.doors_screen.filter.clone(),
                is_password: false,
            }],
            active_field: 0,
        })),
        ScreenId::Audit => Some(ModalKind::Form(FormModal {
            title: "Filter Audit User".to_string(),
            fields: vec![FormField {
                label: "User ID".to_string(),
                value: app.audit_screen.filter_user.clone().unwrap_or_default(),
                is_password: false,
            }],
            active_field: 0,
        })),
        _ => None,
    };

    if let Some(modal) = modal {
        app.modal = Some(modal);
        app.set_status("Search/filter opened");
        true
    } else {
        false
    }
}

fn handle_form_submit(app: &mut App, form: crate::widgets::modal::FormModal) {
    match form.title.as_str() {
        "Send Message" => {
            if let Some(node_field) = form.fields.first()
                && let Some(msg_field) = form.fields.get(1)
            {
                match node_field.value.parse::<u16>() {
                    Ok(node_num) => match app.node_service.send_message(node_num, &msg_field.value)
                    {
                        Ok(()) => {
                            if let Some(db) = &app.db {
                                let _ = crate::services::audit_service::AuditService::record(
                                    db.db(),
                                    "node_message_sent",
                                    None,
                                    Some(i64::from(node_num)),
                                    &format!("message_length={}", msg_field.value.len()),
                                );
                            }
                        }
                        Err(error) => set_error(app, "Send Message", error),
                    },
                    Err(error) => set_error_message(
                        app,
                        "Send Message",
                        &format!("invalid node number: {error}"),
                    ),
                }
            }
        }
        "Broadcast Message" => {
            if let Some(msg_field) = form.fields.first() {
                match app.node_service.broadcast(&msg_field.value) {
                    Ok(()) => {
                        if let Some(db) = &app.db {
                            let _ = crate::services::audit_service::AuditService::record(
                                db.db(),
                                "broadcast_sent",
                                None,
                                None,
                                &format!("message_length={}", msg_field.value.len()),
                            );
                        }
                    }
                    Err(error) => set_error(app, "Broadcast Message", error),
                }
            }
        }
        "Filter Nodes" => {
            if let Some(field) = form.fields.first() {
                app.nodes_screen.filter = field.value.clone();
            }
        }
        "Filter Users" => {
            if let Some(field) = form.fields.first() {
                app.users_screen.filter = field.value.clone();
            }
        }
        "Filter Doors" => {
            if let Some(field) = form.fields.first() {
                app.doors_screen.filter = field.value.clone();
            }
        }
        "Filter Audit User" => {
            if let Some(field) = form.fields.first() {
                let value = field.value.trim();
                app.audit_screen.filter_user = (!value.is_empty()).then(|| value.to_string());
            }
        }
        "Reset Password" => {
            if let Some(user_field) = form.fields.first()
                && let Some(pw_field) = form.fields.get(1)
                && let Some(db) = &app.db
            {
                match crate::services::user_service::UserAdminService::find_by_alias(
                    db.db(),
                    &user_field.value,
                ) {
                    Ok(Some(user)) => {
                        match crate::services::user_service::UserAdminService::reset_password(
                            db.db(),
                            &user.id,
                            &pw_field.value,
                        ) {
                            Ok(()) => app.refresh_data(),
                            Err(error) => set_error(app, "Reset Password", error),
                        }
                    }
                    Ok(None) => set_error_message(app, "Reset Password", "user was not found"),
                    Err(error) => set_error(app, "Reset Password", error),
                }
            }
        }
        "Set Security Level" => {
            if let Some(user_field) = form.fields.first()
                && let Some(level_field) = form.fields.get(1)
                && let Some(db) = &app.db
            {
                match level_field.value.parse::<i64>() {
                    Ok(level) => {
                        match crate::services::user_service::UserAdminService::find_by_alias(
                            db.db(),
                            &user_field.value,
                        ) {
                            Ok(Some(user)) => {
                                match crate::services::user_service::UserAdminService::set_security_level(
                                    db.db(),
                                    &user.id,
                                    level,
                                ) {
                                    Ok(()) => app.refresh_data(),
                                    Err(error) => set_error(app, "Set Security Level", error),
                                }
                            }
                            Ok(None) => {
                                set_error_message(app, "Set Security Level", "user was not found");
                            }
                            Err(error) => set_error(app, "Set Security Level", error),
                        }
                    }
                    Err(error) => set_error_message(
                        app,
                        "Set Security Level",
                        &format!("invalid security level: {error}"),
                    ),
                }
            }
        }
        "Set Config Value" => {
            if let Some(key_field) = form.fields.first()
                && let Some(value_field) = form.fields.get(1)
            {
                match app
                    .config_screen
                    .set_value(&key_field.value, &value_field.value)
                {
                    Ok(()) => {
                        if let Some(db) = &app.db {
                            let _ = crate::services::audit_service::AuditService::record(
                                db.db(),
                                "config_value_set",
                                None,
                                None,
                                &format!("key={}", key_field.value),
                            );
                        }
                        app.refresh_data();
                        app.set_status(format!("Config value {} updated", key_field.value));
                    }
                    Err(error) => set_error(app, "Set Config Value", error),
                }
            }
        }
        _ => {}
    }
}

fn handle_confirm_submit(app: &mut App, title: &str) {
    match title {
        "Quit Sysop TUI" | "Active Nodes" => {
            app.should_quit = true;
        }
        "Delete Message" => {
            if let Some(db) = &app.db
                && let Some(msg_id) = app.messages_screen.selected_message_id()
            {
                match crate::services::message_service::MessageAdminService::delete_message(
                    db.db(),
                    &msg_id,
                ) {
                    Ok(()) => app.refresh_data(),
                    Err(error) => set_error(app, "Delete Message", error),
                }
            }
        }
        "Disconnect Node" | "Kill Door" => {
            if let Some(node) = app.nodes_screen.selected_node_number() {
                let reason = if title == "Kill Door" {
                    "sysop_kill_door"
                } else {
                    "sysop_disconnect"
                };
                match app.node_service.disconnect_node(node, reason) {
                    Ok(()) => {
                        if let Some(db) = &app.db {
                            let _ = crate::services::audit_service::AuditService::record(
                                db.db(),
                                if title == "Kill Door" {
                                    "node_door_killed"
                                } else {
                                    "node_disconnected"
                                },
                                None,
                                Some(i64::from(node)),
                                reason,
                            );
                        }
                        app.refresh_data();
                    }
                    Err(error) => set_error(app, title, error),
                }
            }
        }
        "Enable User"
        | "Disable User"
        | "Grant Sysop"
        | "Revoke Sysop"
        | "Set Status to active"
        | "Set Status to locked"
        | "Set Status to disabled"
        | "Update Security Level" => match app.users_screen.confirm_pending_action(&app.db) {
            Ok(()) => app.refresh_data(),
            Err(error) => set_error(app, title, error),
        },
        "Enable Door" | "Disable Door" | "Add Door" | "Update Door" => {
            match app.doors_screen.confirm_pending_action(&app.db) {
                Ok(()) => app.refresh_data(),
                Err(error) => set_error(app, title, error),
            }
        }
        "Enable Message Area" | "Disable Message Area" => {
            match app.messages_screen.confirm_pending_action(&app.db) {
                Ok(()) => app.refresh_data(),
                Err(error) => set_error(app, title, error),
            }
        }
        "Enable File Area"
        | "Disable File Area"
        | "Approve File Entry"
        | "Unapprove File Entry" => match app.files_screen.confirm_pending_action(&app.db) {
            Ok(Some(message)) => {
                app.refresh_data();
                app.set_status(message);
            }
            Ok(None) => app.refresh_data(),
            Err(error) => set_error(app, title, error),
        },
        "Install OxideNet Hub"
        | "Approve OxideNet Application"
        | "Reject OxideNet Application"
        | "Hold OxideNet Application"
        | "Suspend OxideNet Node"
        | "Activate OxideNet Node"
        | "Rotate OxideNet Password"
        | "Issue OxideNet Token"
        | "Generate OxideNet Nodelist" => {
            match app.oxidenet_screen.confirm_pending_action(&app.db) {
                Ok(Some(message)) => {
                    app.refresh_data();
                    app.set_status(message);
                }
                Ok(None) => app.refresh_data(),
                Err(error) => set_error(app, title, error),
            }
        }
        _ => {}
    }
}

fn set_error(app: &mut App, title: &str, error: SysopError) {
    set_error_message(app, title, &error.to_string());
}

fn set_error_message(app: &mut App, title: &str, message: &str) {
    app.modal = Some(ModalKind::Error(ErrorModal {
        title: title.to_string(),
        message: message.to_string(),
        detail: None,
        suggestion: None,
    }));
}

fn set_info_message(app: &mut App, title: &str, message: &str) {
    app.modal = Some(ModalKind::Info(InfoModal {
        title: title.to_string(),
        message: message.to_string(),
    }));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};

    fn rendered_text(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> String {
        let buffer = terminal.backend().buffer();
        let mut output = String::new();
        for row in 0..height {
            for column in 0..width {
                output.push_str(buffer[(column, row)].symbol());
            }
            if row + 1 < height {
                output.push('\n');
            }
        }
        output
    }

    #[test]
    fn f1_then_f2_renders_visible_command_palette() {
        let mut app = App::new(AppConfig::default());
        handle_ui_event(&mut app, UiEvent::Help);
        assert_eq!(app.current_screen, ScreenId::Help);

        handle_ui_event(&mut app, UiEvent::CommandPalette);
        assert!(app.command_palette.visible);

        let width = 100;
        let height = 30;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| render_app(&mut app, frame))
            .expect("render app");

        let rendered = rendered_text(&terminal, width, height);
        assert!(rendered.contains("Command Palette"));
        assert!(rendered.contains("Search:"));
        assert!(rendered.contains("Go to Dashboard"));

        handle_ui_event(&mut app, UiEvent::CommandPalette);
        assert!(!app.command_palette.visible);
    }

    #[test]
    fn f3_opens_filter_modal_on_searchable_screen() {
        let mut app = App::new(AppConfig::default());
        handle_ui_event(&mut app, UiEvent::NavigateTo(ScreenId::Nodes));

        handle_ui_event(&mut app, UiEvent::Search);

        match app.modal {
            Some(ModalKind::Form(ref modal)) => {
                assert_eq!(modal.title, "Filter Nodes");
            }
            _ => panic!("expected nodes filter modal"),
        }
        assert_eq!(app.status_message.as_deref(), Some("Search/filter opened"));
    }

    #[test]
    fn f3_reports_unavailable_search_on_help_screen() {
        let mut app = App::new(AppConfig::default());
        handle_ui_event(&mut app, UiEvent::Help);

        handle_ui_event(&mut app, UiEvent::Search);

        match app.modal {
            Some(ModalKind::Info(ref modal)) => {
                assert_eq!(modal.title, "Search");
                assert!(modal.message.contains("Nodes, Users, Doors, and Audit"));
            }
            _ => panic!("expected search information modal"),
        }
    }

    #[test]
    fn f5_refreshes_and_renders_status_message() {
        let mut app = App::new(AppConfig::default());

        handle_ui_event(&mut app, UiEvent::Refresh);

        assert_eq!(app.status_message.as_deref(), Some("Refreshed"));

        let width = 100;
        let height = 30;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| render_app(&mut app, frame))
            .expect("render app");

        let rendered = rendered_text(&terminal, width, height);
        assert!(rendered.contains("F5 Refresh"));
        assert!(rendered.contains("Refreshed"));
    }

    #[test]
    fn q_opens_quit_confirmation_by_default() {
        let mut app = App::new(AppConfig::default());

        handle_ui_event(&mut app, UiEvent::Quit);

        assert!(!app.should_quit);
        match app.modal {
            Some(ModalKind::Confirm(ref modal)) => {
                assert_eq!(modal.title, "Quit Sysop TUI");
                assert!(modal.message.contains("Quit the sysop TUI?"));
            }
            _ => panic!("expected quit confirmation modal"),
        }
        assert_eq!(
            app.status_message.as_deref(),
            Some("Quit confirmation required")
        );
    }

    #[test]
    fn q_quits_without_prompt_when_confirm_disabled_and_no_nodes_are_active() {
        let mut app = App::new(AppConfig {
            confirm_quit: false,
            ..AppConfig::default()
        });

        handle_ui_event(&mut app, UiEvent::Quit);

        assert!(app.should_quit);
        assert!(app.modal.is_none());
    }

    #[test]
    fn q_shows_active_node_warning_even_when_confirm_disabled() {
        let mut app = App::new(AppConfig {
            confirm_quit: false,
            ..AppConfig::default()
        });
        app.active_nodes = 2;

        handle_ui_event(&mut app, UiEvent::Quit);

        assert!(!app.should_quit);
        match app.modal {
            Some(ModalKind::Confirm(ref modal)) => {
                assert_eq!(modal.title, "Active Nodes");
                assert_eq!(modal.message, "Nodes are active. Continue to shutdown?");
                assert!(
                    modal
                        .detail
                        .as_deref()
                        .is_some_and(|detail| detail.contains("2 active node(s)"))
                );
            }
            _ => panic!("expected active-node confirmation modal"),
        }
        assert_eq!(
            app.status_message.as_deref(),
            Some("Quit confirmation required: active nodes")
        );
    }

    #[test]
    fn confirming_quit_modal_sets_should_quit() {
        let mut app = App::new(AppConfig::default());
        handle_ui_event(&mut app, UiEvent::Quit);

        handle_ui_event(&mut app, UiEvent::Confirm);

        assert!(app.should_quit);
        assert!(app.modal.is_none());
    }

    #[test]
    fn readonly_dashboard_does_not_open_node_message_forms() {
        let mut app = App::new(AppConfig {
            readonly: true,
            ..AppConfig::default()
        });

        handle_ui_event(
            &mut app,
            UiEvent::Key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE)),
        );
        assert!(app.modal.is_none());

        handle_ui_event(
            &mut app,
            UiEvent::Key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)),
        );
        assert!(app.modal.is_none());
    }

    #[test]
    fn readonly_blocks_mutating_form_submission() {
        let mut app = App::new(AppConfig {
            readonly: true,
            ..AppConfig::default()
        });
        app.modal = Some(ModalKind::Form(FormModal {
            title: "Broadcast Message".to_string(),
            fields: vec![FormField {
                label: "Message".to_string(),
                value: "hello".to_string(),
                is_password: false,
            }],
            active_field: 0,
        }));

        handle_ui_event(&mut app, UiEvent::Confirm);

        match app.modal {
            Some(ModalKind::Info(ref modal)) => {
                assert_eq!(modal.title, "Read Only");
                assert!(modal.message.contains("Broadcast Message"));
            }
            _ => panic!("expected read-only info modal"),
        }
    }

    #[test]
    fn readonly_blocks_mutating_confirm_submission() {
        let mut app = App::new(AppConfig {
            readonly: true,
            ..AppConfig::default()
        });
        app.modal = Some(ModalKind::Confirm(ConfirmModal {
            title: "Delete Message".to_string(),
            message: "Delete?".to_string(),
            detail: None,
            confirm_label: "Delete".to_string(),
            cancel_label: "Cancel".to_string(),
        }));

        handle_ui_event(&mut app, UiEvent::Confirm);

        match app.modal {
            Some(ModalKind::Info(ref modal)) => {
                assert_eq!(modal.title, "Read Only");
                assert!(modal.message.contains("Delete Message"));
            }
            _ => panic!("expected read-only info modal"),
        }
    }

    #[test]
    fn readonly_allows_quit_confirmation() {
        let mut app = App::new(AppConfig {
            readonly: true,
            ..AppConfig::default()
        });
        handle_ui_event(&mut app, UiEvent::Quit);

        handle_ui_event(&mut app, UiEvent::Confirm);

        assert!(app.should_quit);
        assert!(app.modal.is_none());
    }

    #[test]
    fn doctor_nav_item_runs_and_renders_verbose_report() {
        let mut app = App::new(AppConfig {
            db_path: Some(PathBuf::from(":memory:")),
            ..AppConfig::default()
        });
        app.db = Some(OxideDb::open_memory().expect("open memory database"));

        handle_ui_event(&mut app, UiEvent::NavigateTo(ScreenId::Doctor));

        assert_eq!(app.current_screen, ScreenId::Doctor);
        assert!(app.doctor_screen.report.is_some());
        assert!(
            app.status_message
                .as_deref()
                .is_some_and(|message| message.starts_with("Doctor complete:"))
        );

        let width = 120;
        let height = 40;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| render_app(&mut app, frame))
            .expect("render app");

        let rendered = rendered_text(&terminal, width, height);
        assert!(rendered.contains("Doctor"));
        assert!(rendered.contains("[PASS]"));
        assert!(rendered.contains("Schema version"));
        assert!(rendered.contains("Detail:"));
        assert!(rendered.contains("Fix:"));
    }

    #[test]
    fn doctor_r_key_reruns_report() {
        let mut app = App::new(AppConfig {
            db_path: Some(PathBuf::from(":memory:")),
            ..AppConfig::default()
        });
        app.db = Some(OxideDb::open_memory().expect("open memory database"));
        handle_ui_event(&mut app, UiEvent::NavigateTo(ScreenId::Doctor));
        app.doctor_screen.report = None;

        handle_ui_event(
            &mut app,
            UiEvent::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
        );

        assert!(app.doctor_screen.report.is_some());
        assert!(
            app.status_message
                .as_deref()
                .is_some_and(|message| message.starts_with("Doctor complete:"))
        );
    }
}
