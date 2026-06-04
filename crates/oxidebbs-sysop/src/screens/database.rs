use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::database_service::DatabaseAdminService;
use crate::theme::Theme;
use oxidebbs_db::OxideDb;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseView {
    Summary,
    Verify,
}

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
    pub view: DatabaseView,
    pub verify_results: Vec<VerifyResult>,
}

#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub name: String,
    pub passed: bool,
    pub detail: String,
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
            view: DatabaseView::Summary,
            verify_results: Vec::new(),
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

    fn run_verify(&mut self, db: &OxideDb) {
        self.verify_results.clear();
        let db_inner = db.db();

        let tables = [
            "system_config",
            "users",
            "auth_attempts",
            "audit_events",
            "message_areas",
            "messages",
            "sessions",
            "doors",
            "door_runs",
        ];

        for table in tables {
            match db_inner.execute(&format!("SELECT COUNT(*) FROM {table}")) {
                Ok(result) => {
                    let count = result
                        .rows()
                        .first()
                        .and_then(|row| row.values().first())
                        .and_then(|v| match v {
                            oxidebbs_db::Value::Int64(n) => Some(*n),
                            _ => None,
                        })
                        .unwrap_or(0);
                    self.verify_results.push(VerifyResult {
                        name: format!("{table} table"),
                        passed: true,
                        detail: format!("Readable, {count} row(s)"),
                    });
                }
                Err(error) => {
                    self.verify_results.push(VerifyResult {
                        name: format!("{table} table"),
                        passed: false,
                        detail: error.to_string(),
                    });
                }
            }
        }

        match crate::services::database_service::DatabaseAdminService::schema_version(db_inner) {
            Ok(version) => {
                let expected = oxidebbs_db::SCHEMA_VERSION;
                let passed = version == expected;
                self.verify_results.push(VerifyResult {
                    name: "Schema version".to_string(),
                    passed,
                    detail: if passed {
                        format!("{version} matches expected {expected}")
                    } else {
                        format!("{version} does not match expected {expected}")
                    },
                });
            }
            Err(error) => {
                self.verify_results.push(VerifyResult {
                    name: "Schema version".to_string(),
                    passed: false,
                    detail: error.to_string(),
                });
            }
        }
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        db: &Option<OxideDb>,
        _readonly: bool,
    ) -> UiAction {
        if let UiEvent::Key(key) = event {
            match key.code {
                KeyCode::Esc => {
                    if self.view == DatabaseView::Verify {
                        self.view = DatabaseView::Summary;
                    } else {
                        return UiAction::Navigate(ScreenId::Dashboard);
                    }
                }
                KeyCode::Char('v') => {
                    self.view = DatabaseView::Verify;
                    if let Some(db) = db {
                        self.run_verify(db);
                    }
                }
                _ => {}
            }
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        match self.view {
            DatabaseView::Summary => self.render_summary(frame, area),
            DatabaseView::Verify => self.render_verify(frame, area),
        }
    }

    fn render_summary(&self, frame: &mut Frame, area: Rect) {
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

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(3)])
            .split(area);

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" Database ")
                    .title_style(self.theme.title_style()),
            )
            .render(main_layout[0], frame.buffer_mut());

        let hints = "Esc Back | V Verify Schema";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[1], frame.buffer_mut());
    }

    fn render_verify(&self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["Status", "Check", "Detail"])
            .style(self.theme.label_style())
            .height(1);

        let rows: Vec<Row> = self
            .verify_results
            .iter()
            .map(|r| {
                let status = if r.passed { "PASS" } else { "FAIL" };
                let style = if r.passed {
                    self.theme.success_style()
                } else {
                    self.theme.danger_style()
                };
                Row::new(vec![status.to_string(), r.name.clone(), r.detail.clone()]).style(style)
            })
            .collect();

        let passed = self.verify_results.iter().filter(|r| r.passed).count();
        let failed = self.verify_results.len().saturating_sub(passed);

        let toolbar_text = format!(
            "Verify Results: {} total | {} passed | {} failed",
            self.verify_results.len(),
            passed,
            failed,
        );
        let toolbar = Paragraph::new(toolbar_text).style(self.theme.label_style());

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        toolbar.render(main_layout[0], frame.buffer_mut());

        let widths = [
            Constraint::Length(6),
            Constraint::Length(22),
            Constraint::Min(20),
        ];

        ratatui::prelude::StatefulWidget::render(
            Table::new(rows, widths).header(header).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" Schema Verify ")
                    .title_style(self.theme.title_style()),
            ),
            main_layout[1],
            frame.buffer_mut(),
            &mut TableState::default(),
        );

        let hints = "Esc Back to Summary";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[2], frame.buffer_mut());
    }
}
