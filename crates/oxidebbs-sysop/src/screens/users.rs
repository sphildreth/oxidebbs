use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::SysopError;
use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::user_service::UserAdminService;
use crate::theme::Theme;
use crate::widgets::modal::{ConfirmModal, FormField, FormModal, ModalKind};
use oxidebbs_db::{OxideDb, UserRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserSort {
    Alias,
    SecurityLevel,
    Calls,
    LastLogin,
}

pub struct UsersScreen {
    pub theme: Theme,
    pub users: Vec<UserRecord>,
    pub table_state: TableState,
    pub filter: String,
    pub sort: UserSort,
    pub detail_user: Option<String>, // user id
    pub pending_action: Option<UserPendingAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserPendingAction {
    ResetPassword { user_id: String },
    SetSecurityLevel { user_id: String },
    ToggleStatus { user_id: String, new_status: String },
    ToggleSysop { user_id: String, new_sysop: bool },
}

impl UsersScreen {
    pub fn new(theme: Theme) -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            theme,
            users: Vec::new(),
            table_state,
            filter: String::new(),
            sort: UserSort::Alias,
            detail_user: None,
            pending_action: None,
        }
    }

    pub fn refresh(&mut self, db: &OxideDb) {
        if let Ok(users) = UserAdminService::list(db.db()) {
            self.users = users;
            self.sort_users();
        }
    }

    fn sort_users(&mut self) {
        match self.sort {
            UserSort::Alias => self.users.sort_by(|a, b| a.alias.cmp(&b.alias)),
            UserSort::SecurityLevel => self
                .users
                .sort_by_key(|b| std::cmp::Reverse(b.security_level)),
            UserSort::Calls => self.users.sort_by_key(|b| std::cmp::Reverse(b.total_calls)),
            UserSort::LastLogin => self
                .users
                .sort_by(|a, b| b.last_login_at.cmp(&a.last_login_at)),
        }
    }

    fn filtered_users(&self) -> Vec<&UserRecord> {
        let mut users: Vec<&UserRecord> = self.users.iter().collect();
        if !self.filter.is_empty() {
            let f = self.filter.to_ascii_lowercase();
            users.retain(|u| {
                u.alias.to_ascii_lowercase().contains(&f)
                    || u.real_name.to_ascii_lowercase().contains(&f)
                    || u.status.to_ascii_lowercase().contains(&f)
            });
        }
        users
    }

    fn selected_user_id(&self) -> Option<String> {
        let users = self.filtered_users();
        self.table_state
            .selected()
            .and_then(|idx| users.get(idx))
            .map(|u| u.id.clone())
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        db: &Option<OxideDb>,
        readonly: bool,
    ) -> UiAction {
        if self.detail_user.is_some() {
            return self.handle_detail_event(event, db, readonly);
        }

        if let Some(action) = self.pending_action.take() {
            return self.handle_pending_action(event, action, db);
        }

        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Up => {
                    let current = self.table_state.selected().unwrap_or(0);
                    if current > 0 {
                        self.table_state.select(Some(current - 1));
                    }
                }
                KeyCode::Down => {
                    let current = self.table_state.selected().unwrap_or(0);
                    let max = self.filtered_users().len().saturating_sub(1);
                    self.table_state.select(Some((current + 1).min(max)));
                }
                KeyCode::Enter => {
                    if let Some(id) = self.selected_user_id() {
                        self.detail_user = Some(id);
                    }
                }
                KeyCode::Char('f') | KeyCode::Char('/') => {
                    return UiAction::OpenModal(ModalKind::Form(FormModal {
                        title: "Filter Users".to_string(),
                        fields: vec![FormField {
                            label: "Filter".to_string(),
                            value: self.filter.clone(),
                            is_password: false,
                        }],
                        active_field: 0,
                    }));
                }
                KeyCode::Char('s') => {
                    self.sort = match self.sort {
                        UserSort::Alias => UserSort::SecurityLevel,
                        UserSort::SecurityLevel => UserSort::Calls,
                        UserSort::Calls => UserSort::LastLogin,
                        UserSort::LastLogin => UserSort::Alias,
                    };
                    self.sort_users();
                }
                KeyCode::Char('r') if !readonly => {
                    if let Some(id) = self.selected_user_id() {
                        return UiAction::OpenModal(ModalKind::Form(FormModal {
                            title: "Reset Password".to_string(),
                            fields: vec![
                                FormField {
                                    label: "User".to_string(),
                                    value: self
                                        .users
                                        .iter()
                                        .find(|u| u.id == id)
                                        .map(|u| u.alias.clone())
                                        .unwrap_or_default(),
                                    is_password: false,
                                },
                                FormField {
                                    label: "New Password".to_string(),
                                    value: String::new(),
                                    is_password: true,
                                },
                            ],
                            active_field: 1,
                        }));
                    }
                }
                KeyCode::Char('l') if !readonly => {
                    if let Some(id) = self.selected_user_id() {
                        return UiAction::OpenModal(ModalKind::Form(FormModal {
                            title: "Set Security Level".to_string(),
                            fields: vec![
                                FormField {
                                    label: "User".to_string(),
                                    value: self
                                        .users
                                        .iter()
                                        .find(|u| u.id == id)
                                        .map(|u| u.alias.clone())
                                        .unwrap_or_default(),
                                    is_password: false,
                                },
                                FormField {
                                    label: "Level".to_string(),
                                    value: self
                                        .users
                                        .iter()
                                        .find(|u| u.id == id)
                                        .map(|u| u.security_level.to_string())
                                        .unwrap_or_default(),
                                    is_password: false,
                                },
                            ],
                            active_field: 1,
                        }));
                    }
                }
                KeyCode::Char('d') if !readonly => {
                    if let Some(id) = self.selected_user_id()
                        && let Some(user) = self.users.iter().find(|u| u.id == id)
                    {
                        let new_status = if user.status == "active" {
                            "disabled"
                        } else {
                            "active"
                        };
                        let (title, verb) = if new_status == "disabled" {
                            ("Disable User", "Disable")
                        } else {
                            ("Enable User", "Enable")
                        };
                        self.pending_action = Some(UserPendingAction::ToggleStatus {
                            user_id: id,
                            new_status: new_status.to_string(),
                        });
                        return UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
                            title: title.to_string(),
                            message: format!("{verb} user {}?", user.alias),
                            detail: None,
                            confirm_label: verb.to_string(),
                            cancel_label: "Cancel".to_string(),
                        }));
                    }
                }
                KeyCode::Char('o') if !readonly => {
                    if let Some(id) = self.selected_user_id()
                        && let Some(user) = self.users.iter().find(|u| u.id == id)
                    {
                        let new_sysop = !user.is_sysop;
                        self.pending_action = Some(UserPendingAction::ToggleSysop {
                            user_id: id,
                            new_sysop,
                        });
                        return UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
                            title: if new_sysop {
                                "Grant Sysop".to_string()
                            } else {
                                "Revoke Sysop".to_string()
                            },
                            message: format!(
                                "{} sysop privileges for {}?",
                                if new_sysop { "Grant" } else { "Revoke" },
                                user.alias
                            ),
                            detail: None,
                            confirm_label: "Confirm".to_string(),
                            cancel_label: "Cancel".to_string(),
                        }));
                    }
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
                    let max = self.filtered_users().len().saturating_sub(1);
                    self.table_state.select(Some((current + 5).min(max)));
                }
                KeyCode::Home => {
                    self.table_state.select(Some(0));
                }
                KeyCode::End => {
                    let max = self.filtered_users().len().saturating_sub(1);
                    self.table_state.select(Some(max));
                }
                _ => {}
            },
            UiEvent::Refresh => {
                return UiAction::Refresh;
            }
            _ => {}
        }
        UiAction::None
    }

    pub fn confirm_pending_action(&mut self, db: &Option<OxideDb>) -> Result<(), SysopError> {
        let Some(action) = self.pending_action.take() else {
            return Ok(());
        };
        let Some(db) = db else {
            return Err(SysopError::Message(
                "database is unavailable for user action".to_string(),
            ));
        };

        match action {
            UserPendingAction::ToggleStatus {
                user_id,
                new_status,
            } => UserAdminService::set_status(db.db(), &user_id, &new_status),
            UserPendingAction::ToggleSysop { user_id, new_sysop } => {
                UserAdminService::set_sysop(db.db(), &user_id, new_sysop)
            }
            UserPendingAction::ResetPassword { .. }
            | UserPendingAction::SetSecurityLevel { .. } => Ok(()),
        }
    }

    pub fn cancel_pending_action(&mut self) {
        self.pending_action = None;
    }

    fn handle_detail_event(
        &mut self,
        event: UiEvent,
        _db: &Option<OxideDb>,
        _readonly: bool,
    ) -> UiAction {
        match event {
            UiEvent::Key(key) if key.code == KeyCode::Esc => {
                self.detail_user = None;
            }
            UiEvent::Cancel => {
                self.detail_user = None;
            }
            _ => {}
        }
        UiAction::None
    }

    fn handle_pending_action(
        &mut self,
        event: UiEvent,
        action: UserPendingAction,
        db: &Option<OxideDb>,
    ) -> UiAction {
        match event {
            UiEvent::Confirm => {
                if let Some(db) = db {
                    match action {
                        UserPendingAction::ToggleStatus {
                            user_id,
                            new_status,
                        } => {
                            let _ = UserAdminService::set_status(db.db(), &user_id, &new_status);
                        }
                        UserPendingAction::ToggleSysop { user_id, new_sysop } => {
                            let _ = UserAdminService::set_sysop(db.db(), &user_id, new_sysop);
                        }
                        _ => {}
                    }
                }
                self.pending_action = None;
                return UiAction::Refresh;
            }
            UiEvent::Cancel => {
                self.pending_action = None;
            }
            _ => {
                self.pending_action = Some(action);
            }
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if let Some(ref user_id) = self.detail_user {
            self.render_detail(frame, area, user_id);
            return;
        }

        let users = self.filtered_users();
        let toolbar_text = format!(
            "Users: {} total | Sort: {:?} | Filter: {}",
            self.users.len(),
            self.sort,
            if self.filter.is_empty() {
                "all".to_string()
            } else {
                self.filter.clone()
            },
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

        let header = Row::new(vec![
            "Alias",
            "Name",
            "Sec",
            "Calls",
            "Status",
            "Last Login",
        ])
        .style(self.theme.label_style())
        .height(1);

        let rows: Vec<Row> = users
            .iter()
            .map(|u| {
                let status_style = match u.status.as_str() {
                    "active" => self.theme.normal_style(),
                    "disabled" => self.theme.danger_style(),
                    _ => self.theme.warning_style(),
                };
                Row::new(vec![
                    u.alias.clone(),
                    u.real_name.clone(),
                    u.security_level.to_string(),
                    u.total_calls.to_string(),
                    u.status.clone(),
                    u.last_login_at.clone().unwrap_or("--".to_string()),
                ])
                .style(status_style)
            })
            .collect();

        let widths = [
            Constraint::Length(14),
            Constraint::Length(20),
            Constraint::Length(4),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(16),
        ];

        let mut table_state = self.table_state;
        ratatui::prelude::StatefulWidget::render(
            Table::new(rows, widths)
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.block_style(true))
                        .title(" Users ")
                        .title_style(self.theme.title_style()),
                )
                .row_highlight_style(self.theme.selected_style()),
            main_layout[1],
            frame.buffer_mut(),
            &mut table_state,
        );

        let hints = "↑↓ Move | Enter Detail | F Filter | S Sort | R Reset PW | L Set Level | D Enable/Disable | O Sysop | Esc Back";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[2], frame.buffer_mut());
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect, user_id: &str) {
        let user = self.users.iter().find(|u| u.id == user_id);
        let mut lines = vec![Line::from("User Detail")];
        if let Some(u) = user {
            lines.push(Line::from(vec![
                Span::styled("Alias: ", self.theme.label_style()),
                Span::styled(&u.alias, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Name: ", self.theme.label_style()),
                Span::styled(&u.real_name, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Security: ", self.theme.label_style()),
                Span::styled(u.security_level.to_string(), self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Sysop: ", self.theme.label_style()),
                Span::styled(
                    if u.is_sysop { "Yes" } else { "No" },
                    self.theme.normal_style(),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Status: ", self.theme.label_style()),
                Span::styled(&u.status, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Calls: ", self.theme.label_style()),
                Span::styled(u.total_calls.to_string(), self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Time Bank: ", self.theme.label_style()),
                Span::styled(
                    format!("{} min", u.time_bank_minutes),
                    self.theme.normal_style(),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Created: ", self.theme.label_style()),
                Span::styled(&u.created_at, self.theme.normal_style()),
            ]));
            if let Some(ref last) = u.last_login_at {
                lines.push(Line::from(vec![
                    Span::styled("Last Login: ", self.theme.label_style()),
                    Span::styled(last.as_str(), self.theme.normal_style()),
                ]));
            }
            if let Some(ref email) = u.email {
                lines.push(Line::from(vec![
                    Span::styled("Email: ", self.theme.label_style()),
                    Span::styled(email.as_str(), self.theme.normal_style()),
                ]));
            }
        } else {
            lines.push(Line::from("User not found."));
        }

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" User Detail ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }
}
