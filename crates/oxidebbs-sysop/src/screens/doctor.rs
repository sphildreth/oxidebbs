use std::path::Path;

use crossterm::event::KeyCode;
use oxidebbs_db::OxideDb;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::database_service::{DatabaseAdminService, DoctorReport, DoctorStatus};
use crate::theme::Theme;

pub struct DoctorScreen {
    pub theme: Theme,
    pub report: Option<DoctorReport>,
    pub scroll: u16,
}

impl DoctorScreen {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            report: None,
            scroll: 0,
        }
    }

    pub fn refresh(
        &mut self,
        db: Option<&OxideDb>,
        db_path: Option<&Path>,
        configured_node_count: u16,
    ) {
        self.report = Some(DatabaseAdminService::run_doctor(
            db,
            db_path,
            configured_node_count,
        ));
        self.scroll = 0;
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<OxideDb>,
        _readonly: bool,
    ) -> UiAction {
        if let UiEvent::Key(key) = event {
            match key.code {
                KeyCode::Esc => return UiAction::Navigate(ScreenId::Dashboard),
                KeyCode::Char('r' | 'R') => return UiAction::Refresh,
                KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
                KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
                KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(8),
                KeyCode::PageDown => self.scroll = self.scroll.saturating_add(8),
                KeyCode::Home => self.scroll = 0,
                _ => {}
            }
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(8),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_summary(frame, layout[0]);
        self.render_checks(frame, layout[1]);
        self.render_hints(frame, layout[2]);
    }

    fn render_summary(&self, frame: &mut Frame, area: Rect) {
        let lines = match &self.report {
            Some(report) => vec![
                Line::from(vec![
                    Span::styled("Checks: ", self.theme.label_style()),
                    Span::styled(report.checks.len().to_string(), self.theme.normal_style()),
                    Span::raw("  "),
                    Span::styled("Passed: ", self.theme.label_style()),
                    Span::styled(
                        report.passed_count().to_string(),
                        self.theme.success_style(),
                    ),
                    Span::raw("  "),
                    Span::styled("Warnings: ", self.theme.label_style()),
                    Span::styled(
                        report.warning_count().to_string(),
                        self.theme.warning_style(),
                    ),
                    Span::raw("  "),
                    Span::styled("Failed: ", self.theme.label_style()),
                    Span::styled(report.failed_count().to_string(), self.theme.danger_style()),
                ]),
                Line::from(vec![
                    Span::styled("Ran: ", self.theme.label_style()),
                    Span::styled(report.checked_at.as_str(), self.theme.normal_style()),
                    Span::raw("  "),
                    Span::styled("Database: ", self.theme.label_style()),
                    Span::styled(
                        report
                            .database_path
                            .as_deref()
                            .unwrap_or("<not configured>"),
                        self.theme.normal_style(),
                    ),
                ]),
            ],
            None => vec![Line::from(vec![Span::styled(
                "Doctor has not run yet. Press R or F5 to run checks.",
                self.theme.warning_style(),
            )])],
        };

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" Doctor ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }

    fn render_checks(&self, frame: &mut Frame, area: Rect) {
        let mut lines = Vec::new();
        if let Some(report) = &self.report {
            for check in &report.checks {
                let status_style = match check.status {
                    DoctorStatus::Pass => self.theme.success_style(),
                    DoctorStatus::Warn => self.theme.warning_style(),
                    DoctorStatus::Fail => self.theme.danger_style(),
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("[{}] ", check.status.label()), status_style),
                    Span::styled(check.name.as_str(), self.theme.title_style()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("      Detail: ", self.theme.label_style()),
                    Span::styled(check.detail.as_str(), self.theme.normal_style()),
                ]));
                if let Some(remediation) = &check.remediation {
                    lines.push(Line::from(vec![
                        Span::styled("      Fix: ", self.theme.label_style()),
                        Span::styled(remediation.as_str(), self.theme.muted_style()),
                    ]));
                }
                lines.push(Line::from(""));
            }
        } else {
            lines.push(Line::from(vec![Span::styled(
                "No report is available.",
                self.theme.muted_style(),
            )]));
        }

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" Checks ")
                    .title_style(self.theme.title_style()),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0))
            .render(area, frame.buffer_mut());
    }

    fn render_hints(&self, frame: &mut Frame, area: Rect) {
        Paragraph::new(
            "R Run Doctor | F5 Refresh | Up/Down Scroll | PageUp/PageDown | Home Top | Esc Back",
        )
        .style(self.theme.muted_style())
        .block(Block::default().borders(Borders::ALL))
        .render(area, frame.buffer_mut());
    }
}
