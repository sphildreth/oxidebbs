use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::audit_service::AuditService;
use crate::theme::Theme;
use crate::widgets::modal::{FormField, FormModal, ModalKind};
use oxidebbs_db::OxideDb;

pub struct AuditScreen {
    pub theme: Theme,
    pub events: Vec<oxidebbs_db::AuditEventRecord>,
    pub table_state: TableState,
    pub filter_user: Option<String>,
}

impl AuditScreen {
    pub fn new(theme: Theme) -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            theme,
            events: Vec::new(),
            table_state,
            filter_user: None,
        }
    }

    pub fn refresh(&mut self, db: &OxideDb) {
        if let Ok(events) = AuditService::recent(db.db(), 100) {
            self.events = events;
        }
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<OxideDb>,
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
                    let max = self.filtered_events().len().saturating_sub(1);
                    self.table_state.select(Some((current + 1).min(max)));
                }
                KeyCode::Esc => {
                    return UiAction::Navigate(ScreenId::Dashboard);
                }
                KeyCode::Char('f') | KeyCode::Char('/') => {
                    return UiAction::OpenModal(ModalKind::Form(FormModal {
                        title: "Filter Audit User".to_string(),
                        fields: vec![FormField {
                            label: "User".to_string(),
                            value: self.filter_user.clone().unwrap_or_default(),
                            is_password: false,
                        }],
                        active_field: 0,
                    }));
                }
                KeyCode::PageUp => {
                    let current = self.table_state.selected().unwrap_or(0);
                    self.table_state.select(Some(current.saturating_sub(5)));
                }
                KeyCode::PageDown => {
                    let current = self.table_state.selected().unwrap_or(0);
                    let max = self.filtered_events().len().saturating_sub(1);
                    self.table_state.select(Some((current + 5).min(max)));
                }
                KeyCode::Home => {
                    self.table_state.select(Some(0));
                }
                KeyCode::End => {
                    let max = self.filtered_events().len().saturating_sub(1);
                    self.table_state.select(Some(max));
                }
                _ => {}
            }
        }
        UiAction::None
    }

    fn filtered_events(&self) -> Vec<&oxidebbs_db::AuditEventRecord> {
        if let Some(ref user) = self.filter_user {
            self.events
                .iter()
                .filter(|e| {
                    e.user_id.as_deref().is_some_and(|u| {
                        u.to_ascii_lowercase().contains(&user.to_ascii_lowercase())
                    })
                })
                .collect()
        } else {
            self.events.iter().collect()
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let events = self.filtered_events();
        let toolbar_text = format!(
            "Audit Events: {} total{}",
            self.events.len(),
            self.filter_user
                .as_ref()
                .map(|u| format!(" | Filter: {u}"))
                .unwrap_or_default()
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

        let header = Row::new(vec!["Time", "Type", "User", "Node", "Details"])
            .style(self.theme.label_style())
            .height(1);

        let rows: Vec<Row> = events
            .iter()
            .map(|e| {
                Row::new(vec![
                    e.created_at.clone(),
                    e.event_type.clone(),
                    e.user_id.clone().unwrap_or("--".to_string()),
                    e.node_number
                        .map(|n| n.to_string())
                        .unwrap_or("--".to_string()),
                    e.details.clone(),
                ])
                .style(self.theme.normal_style())
            })
            .collect();

        let widths = [
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Length(6),
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
                        .title(" Audit Events ")
                        .title_style(self.theme.title_style()),
                )
                .row_highlight_style(self.theme.selected_style()),
            main_layout[1],
            frame.buffer_mut(),
            &mut table_state,
        );

        let hints = "↑↓ Move | F Filter User | Esc Back";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[2], frame.buffer_mut());
    }
}
