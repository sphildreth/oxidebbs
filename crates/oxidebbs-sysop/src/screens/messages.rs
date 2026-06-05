use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::message_service::MessageAdminService;
use crate::theme::Theme;
use crate::widgets::modal::{ConfirmModal, ModalKind};
use oxidebbs_db::{MessageAreaRecord, MessageRecord, OxideDb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageView {
    Areas,
    Messages,
    Detail,
    Search,
}

pub struct MessagesScreen {
    pub theme: Theme,
    pub view: MessageView,
    pub areas: Vec<MessageAreaRecord>,
    pub messages: Vec<MessageRecord>,
    pub area_table_state: TableState,
    pub message_table_state: TableState,
    pub selected_area: Option<String>,
    pub detail_message: Option<String>,
    pub search_query: String,
    pub search_results: Vec<MessageRecord>,
    pub search_state: TableState,
    pub pending_action: Option<MessagePendingAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagePendingAction {
    SetAreaEnabled { area_id: String, enabled: bool },
}

impl MessagesScreen {
    pub fn new(theme: Theme) -> Self {
        let mut area_table_state = TableState::default();
        area_table_state.select(Some(0));
        let mut message_table_state = TableState::default();
        message_table_state.select(Some(0));
        let mut search_state = TableState::default();
        search_state.select(Some(0));
        Self {
            theme,
            view: MessageView::Areas,
            areas: Vec::new(),
            messages: Vec::new(),
            area_table_state,
            message_table_state,
            selected_area: None,
            detail_message: None,
            search_query: String::new(),
            search_results: Vec::new(),
            search_state,
            pending_action: None,
        }
    }

    pub fn refresh(&mut self, db: &OxideDb) {
        if let Ok(areas) = MessageAdminService::list_areas(db.db()) {
            self.areas = areas;
        }
        if let Some(ref area_id) = self.selected_area
            && let Ok(msgs) = MessageAdminService::list_messages(db.db(), area_id)
        {
            self.messages = msgs;
        }
    }

    fn selected_area_id(&self) -> Option<String> {
        self.area_table_state
            .selected()
            .and_then(|idx| self.areas.get(idx))
            .map(|a| a.id.clone())
    }

    pub fn selected_message_id(&self) -> Option<String> {
        self.message_table_state
            .selected()
            .and_then(|idx| self.messages.get(idx))
            .map(|m| m.id.clone())
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        db: &Option<OxideDb>,
        readonly: bool,
    ) -> UiAction {
        match self.view {
            MessageView::Detail => return self.handle_detail_event(event),
            MessageView::Search => return self.handle_search_event(event, db),
            MessageView::Messages => return self.handle_message_event(event, db, readonly),
            MessageView::Areas => {}
        }

        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Up => {
                    let current = self.area_table_state.selected().unwrap_or(0);
                    if current > 0 {
                        self.area_table_state.select(Some(current - 1));
                    }
                }
                KeyCode::Down => {
                    let current = self.area_table_state.selected().unwrap_or(0);
                    let max = self.areas.len().saturating_sub(1);
                    self.area_table_state.select(Some((current + 1).min(max)));
                }
                KeyCode::Enter => {
                    if let Some(id) = self.selected_area_id() {
                        self.selected_area = Some(id);
                        self.message_table_state.select(Some(0));
                        self.view = MessageView::Messages;
                        return UiAction::Refresh;
                    }
                }
                KeyCode::Char('d') if !readonly => {
                    if let Some(id) = self.selected_area_id()
                        && let Some(area) = self.areas.iter().find(|area| area.id == id)
                    {
                        let enabled = !area.enabled;
                        self.pending_action = Some(MessagePendingAction::SetAreaEnabled {
                            area_id: area.id.clone(),
                            enabled,
                        });
                        return UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
                            title: if enabled {
                                "Enable Message Area".to_string()
                            } else {
                                "Disable Message Area".to_string()
                            },
                            message: format!(
                                "{} message area {}?",
                                if enabled { "Enable" } else { "Disable" },
                                area.key
                            ),
                            detail: Some(area.name.clone()),
                            confirm_label: "Confirm".to_string(),
                            cancel_label: "Cancel".to_string(),
                        }));
                    }
                }
                KeyCode::Esc => {
                    return UiAction::Navigate(ScreenId::Dashboard);
                }
                KeyCode::PageUp => {
                    let current = self.area_table_state.selected().unwrap_or(0);
                    self.area_table_state
                        .select(Some(current.saturating_sub(5)));
                }
                KeyCode::PageDown => {
                    let current = self.area_table_state.selected().unwrap_or(0);
                    let max = self.areas.len().saturating_sub(1);
                    self.area_table_state.select(Some((current + 5).min(max)));
                }
                KeyCode::Home => {
                    self.area_table_state.select(Some(0));
                }
                KeyCode::End => {
                    let max = self.areas.len().saturating_sub(1);
                    self.area_table_state.select(Some(max));
                }
                _ => {}
            },
            UiEvent::Search => {
                self.search_query.clear();
                self.search_results.clear();
                self.search_state.select(Some(0));
                self.view = MessageView::Search;
            }
            UiEvent::Refresh => {
                return UiAction::Refresh;
            }
            _ => {}
        }
        UiAction::None
    }

    pub fn confirm_pending_action(
        &mut self,
        db: &Option<OxideDb>,
    ) -> Result<(), crate::SysopError> {
        let Some(action) = self.pending_action.take() else {
            return Ok(());
        };
        let Some(db) = db else {
            return Err(crate::SysopError::Message(
                "database is unavailable for message action".to_string(),
            ));
        };
        match action {
            MessagePendingAction::SetAreaEnabled { area_id, enabled } => {
                MessageAdminService::set_area_enabled(db.db(), &area_id, enabled)
            }
        }
    }

    pub fn cancel_pending_action(&mut self) {
        self.pending_action = None;
    }

    fn handle_search_event(&mut self, event: UiEvent, db: &Option<OxideDb>) -> UiAction {
        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Esc => {
                    self.view = MessageView::Areas;
                    self.search_query.clear();
                    self.search_results.clear();
                }
                KeyCode::Enter => {
                    if let Some(db) = db
                        && !self.search_query.is_empty()
                    {
                        self.search_results =
                            search_all_areas(db.db(), &self.search_query).unwrap_or_default();
                        self.search_state.select(Some(0));
                    }
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                }
                KeyCode::Up => {
                    let current = self.search_state.selected().unwrap_or(0);
                    if current > 0 {
                        self.search_state.select(Some(current - 1));
                    }
                }
                KeyCode::Down => {
                    let current = self.search_state.selected().unwrap_or(0);
                    let max = self.search_results.len().saturating_sub(1);
                    self.search_state.select(Some((current + 1).min(max)));
                }
                _ => {}
            },
            UiEvent::Cancel => {
                self.view = MessageView::Areas;
                self.search_query.clear();
                self.search_results.clear();
            }
            _ => {}
        }
        UiAction::None
    }

    fn handle_message_event(
        &mut self,
        event: UiEvent,
        _db: &Option<OxideDb>,
        readonly: bool,
    ) -> UiAction {
        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Up => {
                    let current = self.message_table_state.selected().unwrap_or(0);
                    if current > 0 {
                        self.message_table_state.select(Some(current - 1));
                    }
                }
                KeyCode::Down => {
                    let current = self.message_table_state.selected().unwrap_or(0);
                    let max = self.messages.len().saturating_sub(1);
                    self.message_table_state
                        .select(Some((current + 1).min(max)));
                }
                KeyCode::Enter => {
                    if let Some(id) = self.selected_message_id() {
                        self.detail_message = Some(id);
                        self.view = MessageView::Detail;
                    }
                }
                KeyCode::Char('d') if !readonly => {
                    if let Some(id) = self.selected_message_id() {
                        return UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
                            title: "Delete Message".to_string(),
                            message: format!("Soft-delete message {}?", id),
                            detail: Some("Sets visibility to deleted.".to_string()),
                            confirm_label: "Delete".to_string(),
                            cancel_label: "Cancel".to_string(),
                        }));
                    }
                }
                KeyCode::Esc => {
                    self.view = MessageView::Areas;
                    self.detail_message = None;
                }
                KeyCode::PageUp => {
                    let current = self.message_table_state.selected().unwrap_or(0);
                    self.message_table_state
                        .select(Some(current.saturating_sub(5)));
                }
                KeyCode::PageDown => {
                    let current = self.message_table_state.selected().unwrap_or(0);
                    let max = self.messages.len().saturating_sub(1);
                    self.message_table_state
                        .select(Some((current + 5).min(max)));
                }
                KeyCode::Home => {
                    self.message_table_state.select(Some(0));
                }
                KeyCode::End => {
                    let max = self.messages.len().saturating_sub(1);
                    self.message_table_state.select(Some(max));
                }
                _ => {}
            },
            UiEvent::Search => {
                self.search_query.clear();
                self.search_results.clear();
                self.search_state.select(Some(0));
                self.view = MessageView::Search;
            }
            UiEvent::Cancel => {
                self.view = MessageView::Areas;
                self.detail_message = None;
            }
            UiEvent::Refresh => {
                return UiAction::Refresh;
            }
            _ => {}
        }
        UiAction::None
    }

    fn handle_detail_event(&mut self, event: UiEvent) -> UiAction {
        match event {
            UiEvent::Key(key) if key.code == KeyCode::Esc => {
                self.view = MessageView::Messages;
                self.detail_message = None;
            }
            UiEvent::Cancel => {
                self.view = MessageView::Messages;
                self.detail_message = None;
            }
            _ => {}
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        match self.view {
            MessageView::Areas => self.render_areas(frame, area),
            MessageView::Messages => self.render_messages(frame, area),
            MessageView::Search => self.render_search(frame, area),
            MessageView::Detail => {
                if let Some(ref id) = self.detail_message {
                    self.render_detail(frame, area, id);
                } else {
                    self.render_messages(frame, area);
                }
            }
        }
    }

    fn render_areas(&self, frame: &mut Frame, area: Rect) {
        let toolbar_text = format!("Message Areas: {} total", self.areas.len());
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

        let header = Row::new(vec![
            "Key",
            "Name",
            "Description",
            "Kind",
            "Read",
            "Post",
            "Mod",
            "En",
        ])
        .style(self.theme.label_style())
        .height(1);

        let rows: Vec<Row> = self
            .areas
            .iter()
            .map(|a| {
                let style = if a.enabled {
                    self.theme.normal_style()
                } else {
                    self.theme.muted_style()
                };
                Row::new(vec![
                    a.key.clone(),
                    a.name.clone(),
                    a.description.clone(),
                    a.kind.clone(),
                    a.read_security_level.to_string(),
                    a.post_security_level.to_string(),
                    if a.moderated { "Yes" } else { "No" }.to_string(),
                    if a.enabled { "Yes" } else { "No" }.to_string(),
                ])
                .style(style)
            })
            .collect();

        let widths = [
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Length(20),
            Constraint::Length(8),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(4),
            Constraint::Length(3),
        ];

        let mut area_table_state = self.area_table_state;
        ratatui::prelude::StatefulWidget::render(
            Table::new(rows, widths)
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.block_style(true))
                        .title(" Message Areas ")
                        .title_style(self.theme.title_style()),
                )
                .row_highlight_style(self.theme.selected_style()),
            main_layout[1],
            frame.buffer_mut(),
            &mut area_table_state,
        );

        let hints = "↑↓ Move | Enter Open Area | D Enable/Disable | F3 Search | Esc Back";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[2], frame.buffer_mut());
    }

    fn render_search(&self, frame: &mut Frame, area: Rect) {
        let toolbar_text = format!(
            "Search messages: {}",
            if self.search_query.is_empty() {
                "<type query and press Enter>"
            } else {
                &self.search_query
            }
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

        if self.search_results.is_empty() && !self.search_query.is_empty() {
            let msg = Paragraph::new("No results. Press Enter to search.")
                .style(self.theme.muted_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.block_style(true))
                        .title(" Search Results ")
                        .title_style(self.theme.title_style()),
                );
            msg.render(main_layout[1], frame.buffer_mut());
        } else {
            let header = Row::new(vec!["Area", "Author", "Subject", "Created", "Vis"])
                .style(self.theme.label_style())
                .height(1);

            let rows: Vec<Row> = self
                .search_results
                .iter()
                .map(|m| {
                    let area_name = self
                        .areas
                        .iter()
                        .find(|a| a.id == m.area_id)
                        .map(|a| a.key.as_str())
                        .unwrap_or("?");
                    let style = match m.visibility.as_str() {
                        "deleted" => self.theme.danger_style(),
                        "hidden" => self.theme.warning_style(),
                        _ => self.theme.normal_style(),
                    };
                    Row::new(vec![
                        area_name.to_string(),
                        m.author_user_id.clone(),
                        m.subject.clone(),
                        m.created_at.clone(),
                        m.visibility.clone(),
                    ])
                    .style(style)
                })
                .collect();

            let widths = [
                Constraint::Length(10),
                Constraint::Length(18),
                Constraint::Length(28),
                Constraint::Length(16),
                Constraint::Length(8),
            ];

            let mut search_state = self.search_state;
            ratatui::prelude::StatefulWidget::render(
                Table::new(rows, widths)
                    .header(header)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(self.theme.block_style(true))
                            .title(" Search Results ")
                            .title_style(self.theme.title_style()),
                    )
                    .row_highlight_style(self.theme.selected_style()),
                main_layout[1],
                frame.buffer_mut(),
                &mut search_state,
            );
        }

        let hints = "Enter Search | Esc Back";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[2], frame.buffer_mut());
    }

    fn render_messages(&self, frame: &mut Frame, area: Rect) {
        let area_name = self
            .selected_area
            .as_ref()
            .and_then(|id| self.areas.iter().find(|a| a.id == *id))
            .map(|a| a.name.as_str())
            .unwrap_or("Unknown");
        let toolbar_text = format!("Area: {} | Messages: {}", area_name, self.messages.len());
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

        let header = Row::new(vec!["Author", "Subject", "Created", "Vis"])
            .style(self.theme.label_style())
            .height(1);

        let rows: Vec<Row> = self
            .messages
            .iter()
            .map(|m| {
                let style = match m.visibility.as_str() {
                    "deleted" => self.theme.danger_style(),
                    "hidden" => self.theme.warning_style(),
                    _ => self.theme.normal_style(),
                };
                Row::new(vec![
                    m.author_user_id.clone(),
                    m.subject.clone(),
                    m.created_at.clone(),
                    m.visibility.clone(),
                ])
                .style(style)
            })
            .collect();

        let widths = [
            Constraint::Length(20),
            Constraint::Length(30),
            Constraint::Length(16),
            Constraint::Length(8),
        ];

        let mut message_table_state = self.message_table_state;
        ratatui::prelude::StatefulWidget::render(
            Table::new(rows, widths)
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.block_style(true))
                        .title(" Messages ")
                        .title_style(self.theme.title_style()),
                )
                .row_highlight_style(self.theme.selected_style()),
            main_layout[1],
            frame.buffer_mut(),
            &mut message_table_state,
        );

        let hints = "↑↓ Move | Enter Detail | D Delete | F3 Search | Esc Back";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[2], frame.buffer_mut());
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect, message_id: &str) {
        let msg = self.messages.iter().find(|m| m.id == message_id);
        let mut lines = vec![Line::from("Message Detail")];
        if let Some(m) = msg {
            lines.push(Line::from(vec![
                Span::styled("Subject: ", self.theme.label_style()),
                Span::styled(&m.subject, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Author: ", self.theme.label_style()),
                Span::styled(&m.author_user_id, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Created: ", self.theme.label_style()),
                Span::styled(&m.created_at, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Visibility: ", self.theme.label_style()),
                Span::styled(&m.visibility, self.theme.normal_style()),
            ]));
            if let Some(ref to) = m.to_user_id {
                lines.push(Line::from(vec![
                    Span::styled("To: ", self.theme.label_style()),
                    Span::styled(to.as_str(), self.theme.normal_style()),
                ]));
            }
            if let Some(ref reply) = m.reply_to_id {
                lines.push(Line::from(vec![
                    Span::styled("Reply To: ", self.theme.label_style()),
                    Span::styled(reply.as_str(), self.theme.normal_style()),
                ]));
            }
            if let Some(ref network_message_id) = m.network_message_id {
                lines.push(Line::from(vec![
                    Span::styled("Network Message: ", self.theme.label_style()),
                    Span::styled(network_message_id.as_str(), self.theme.normal_style()),
                ]));
            }
            if let Some(ref address) = m.author_network_address {
                lines.push(Line::from(vec![
                    Span::styled("Origin Address: ", self.theme.label_style()),
                    Span::styled(address.as_str(), self.theme.normal_style()),
                ]));
            }
            lines.push(Line::from(""));
            for line in m.body.lines() {
                lines.push(Line::from(line.to_string()));
            }
        } else {
            lines.push(Line::from("Message not found."));
        }

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" Message Detail ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }
}

fn search_all_areas(
    db: &oxidebbs_db::Db,
    query: &str,
) -> Result<Vec<MessageRecord>, oxidebbs_db::DbError> {
    let all_messages = oxidebbs_db::list_messages(db)?;
    let lower = query.to_ascii_lowercase();
    let results: Vec<MessageRecord> = all_messages
        .into_iter()
        .filter(|m| {
            m.subject.to_ascii_lowercase().contains(&lower)
                || m.body.to_ascii_lowercase().contains(&lower)
        })
        .collect();
    Ok(results)
}
