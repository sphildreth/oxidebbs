use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::log_service::LogService;
use crate::theme::Theme;
use std::path::PathBuf;

pub struct LogsScreen {
    pub theme: Theme,
    pub entries: Vec<crate::services::log_service::LogEntry>,
    pub table_state: TableState,
    pub log_path: Option<PathBuf>,
}

impl LogsScreen {
    pub fn new(theme: Theme) -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            theme,
            entries: Vec::new(),
            table_state,
            log_path: None,
        }
    }

    pub fn refresh(&mut self) {
        if let Some(ref path) = self.log_path
            && let Ok(entries) = LogService::tail(path, 200)
        {
            self.entries = entries;
        }
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<oxidebbs_db::OxideDb>,
        _readonly: bool,
    ) -> UiAction {
        if let UiEvent::Key(key) = event {
            match key.code {
                KeyCode::Up => {
                    let current = self.table_state.selected().unwrap_or(0);
                    if current > 0 {
                        self.table_state.select(Some(current - 1));
                    }
                }
                KeyCode::Down => {
                    let current = self.table_state.selected().unwrap_or(0);
                    let max = self.entries.len().saturating_sub(1);
                    self.table_state.select(Some((current + 1).min(max)));
                }
                KeyCode::Esc => {
                    return UiAction::Navigate(ScreenId::Dashboard);
                }
                KeyCode::PageUp => {
                    let current = self.table_state.selected().unwrap_or(0);
                    self.table_state.select(Some(current.saturating_sub(5)));
                }
                KeyCode::PageDown => {
                    let current = self.table_state.selected().unwrap_or(0);
                    let max = self.entries.len().saturating_sub(1);
                    self.table_state.select(Some((current + 5).min(max)));
                }
                KeyCode::Home => {
                    self.table_state.select(Some(0));
                }
                KeyCode::End => {
                    let max = self.entries.len().saturating_sub(1);
                    self.table_state.select(Some(max));
                }
                _ => {}
            }
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let toolbar_text = format!(
            "Log Entries: {} | Path: {}",
            self.entries.len(),
            self.log_path
                .as_ref()
                .and_then(|p| p.to_str())
                .unwrap_or("none")
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

        let header = Row::new(vec!["Time", "Level", "Target", "Message"])
            .style(self.theme.label_style())
            .height(1);

        let rows: Vec<Row> = self
            .entries
            .iter()
            .map(|e| {
                let style = match e.level.as_str() {
                    "ERROR" => self.theme.danger_style(),
                    "WARN" => self.theme.warning_style(),
                    _ => self.theme.normal_style(),
                };
                Row::new(vec![
                    e.timestamp.clone(),
                    e.level.clone(),
                    e.target.clone(),
                    e.message.clone(),
                ])
                .style(style)
            })
            .collect();

        let widths = [
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(20),
            Constraint::Min(20),
        ];

        let mut table_state = self.table_state;
        ratatui::prelude::StatefulWidget::render(
            Table::new(rows, widths)
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.block_style(true))
                        .title(" Logs ")
                        .title_style(self.theme.title_style()),
                )
                .row_highlight_style(self.theme.selected_style()),
            main_layout[1],
            frame.buffer_mut(),
            &mut table_state,
        );

        let hints = "↑↓ Move | Esc Back";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[2], frame.buffer_mut());
    }
}
