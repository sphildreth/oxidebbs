use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::database_service::DatabaseAdminService;
use crate::theme::Theme;
use oxidebbs_db::OxideDb;
use std::path::PathBuf;

pub struct DatabaseScreen {
    pub theme: Theme,
    pub path: Option<PathBuf>,
    pub schema_version: i64,
    pub user_count: i64,
    pub message_count: i64,
    pub session_count: i64,
    pub door_count: i64,
    pub door_run_count: i64,
    pub audit_count: i64,
    pub healthy: bool,
}

impl DatabaseScreen {
    pub fn new(theme: Theme, path: Option<PathBuf>) -> Self {
        Self {
            theme,
            path,
            schema_version: 0,
            user_count: 0,
            message_count: 0,
            session_count: 0,
            door_count: 0,
            door_run_count: 0,
            audit_count: 0,
            healthy: false,
        }
    }

    pub fn refresh(&mut self, db: &OxideDb) {
        self.healthy = true;
        self.schema_version = DatabaseAdminService::schema_version(db.db()).unwrap_or(0);
        self.user_count = DatabaseAdminService::count_users(db.db()).unwrap_or(0);
        self.message_count = DatabaseAdminService::count_messages(db.db()).unwrap_or(0);
        self.session_count = DatabaseAdminService::count_sessions(db.db()).unwrap_or(0);
        self.door_count = DatabaseAdminService::count_doors(db.db()).unwrap_or(0);
        self.door_run_count = DatabaseAdminService::count_door_runs(db.db()).unwrap_or(0);
        self.audit_count = DatabaseAdminService::count_audit_events(db.db()).unwrap_or(0);
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<OxideDb>,
        _readonly: bool,
    ) -> UiAction {
        match event {
            UiEvent::Key(key) if key.code == KeyCode::Esc => {
                return UiAction::Navigate(ScreenId::Dashboard);
            }
            _ => {}
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Path: ", self.theme.label_style()),
                Span::styled(
                    self.path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "data/oxidebbs.ddb".to_string()),
                    self.theme.normal_style(),
                ),
            ]),
            Line::from(vec![
                Span::styled("Schema Version: ", self.theme.label_style()),
                Span::styled(self.schema_version.to_string(), self.theme.normal_style()),
            ]),
            Line::from(vec![
                Span::styled("Users: ", self.theme.label_style()),
                Span::styled(self.user_count.to_string(), self.theme.normal_style()),
            ]),
            Line::from(vec![
                Span::styled("Messages: ", self.theme.label_style()),
                Span::styled(self.message_count.to_string(), self.theme.normal_style()),
            ]),
            Line::from(vec![
                Span::styled("Sessions: ", self.theme.label_style()),
                Span::styled(self.session_count.to_string(), self.theme.normal_style()),
            ]),
            Line::from(vec![
                Span::styled("Doors: ", self.theme.label_style()),
                Span::styled(self.door_count.to_string(), self.theme.normal_style()),
            ]),
            Line::from(vec![
                Span::styled("Door Runs: ", self.theme.label_style()),
                Span::styled(self.door_run_count.to_string(), self.theme.normal_style()),
            ]),
            Line::from(vec![
                Span::styled("Audit Events: ", self.theme.label_style()),
                Span::styled(self.audit_count.to_string(), self.theme.normal_style()),
            ]),
        ];
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Health: ", self.theme.label_style()),
            Span::styled(
                if self.healthy { "OK" } else { "Error" },
                if self.healthy {
                    self.theme.success_style()
                } else {
                    self.theme.danger_style()
                },
            ),
        ]));

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" Database ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }
}
