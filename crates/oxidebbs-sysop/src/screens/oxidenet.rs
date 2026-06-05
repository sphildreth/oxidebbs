use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::SysopError;
use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::oxidenet_service::{OxideNetAdminService, OxideNetDashboard};
use crate::theme::Theme;
use crate::widgets::modal::{ConfirmModal, ModalKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OxideNetView {
    Dashboard,
    Applications,
    Nodes,
    Queues,
    Subscriptions,
    PollLogs,
    Nodelist,
    ConfigPackage,
}

impl OxideNetView {
    fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Applications => "Applications",
            Self::Nodes => "Nodes",
            Self::Queues => "Queues",
            Self::Subscriptions => "Subscriptions",
            Self::PollLogs => "Poll Logs",
            Self::Nodelist => "Nodelist",
            Self::ConfigPackage => "Config Package",
        }
    }
}

pub struct OxideNetScreen {
    pub theme: Theme,
    pub view: OxideNetView,
    pub dashboard: Option<OxideNetDashboard>,
    pub application_state: TableState,
    pub node_state: TableState,
    pub pending_action: Option<OxideNetPendingAction>,
    pub status: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OxideNetPendingAction {
    InstallHub,
    ApproveApplication { application_id: String },
    RejectApplication { application_id: String },
    HoldApplication { application_id: String },
    SuspendNode { node_id: String },
    ActivateNode { node_id: String },
    RotatePassword { node_id: String },
    IssueJoinToken { node_id: String },
    GenerateNodelist,
}

impl OxideNetScreen {
    pub fn new(theme: Theme) -> Self {
        let mut application_state = TableState::default();
        application_state.select(Some(0));
        let mut node_state = TableState::default();
        node_state.select(Some(0));
        Self {
            theme,
            view: OxideNetView::Dashboard,
            dashboard: None,
            application_state,
            node_state,
            pending_action: None,
            status: None,
            error: None,
        }
    }

    pub fn refresh(&mut self, db: &oxidebbs_db::OxideDb) {
        match OxideNetAdminService::load(db) {
            Ok(dashboard) => {
                if dashboard.applications.is_empty() {
                    self.application_state.select(None);
                } else if self.application_state.selected().is_none() {
                    self.application_state.select(Some(0));
                }
                if dashboard.nodes.is_empty() {
                    self.node_state.select(None);
                } else if self.node_state.selected().is_none() {
                    self.node_state.select(Some(0));
                }
                self.dashboard = Some(dashboard);
                self.error = None;
            }
            Err(error) => {
                self.dashboard = None;
                self.error = Some(error.to_string());
            }
        }
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<oxidebbs_db::OxideDb>,
        readonly: bool,
    ) -> UiAction {
        if let UiEvent::Key(key) = event {
            match key.code {
                KeyCode::Char('1') => self.view = OxideNetView::Dashboard,
                KeyCode::Char('2') => self.view = OxideNetView::Applications,
                KeyCode::Char('3') => self.view = OxideNetView::Nodes,
                KeyCode::Char('4') => self.view = OxideNetView::Queues,
                KeyCode::Char('5') => self.view = OxideNetView::Subscriptions,
                KeyCode::Char('6') => self.view = OxideNetView::PollLogs,
                KeyCode::Char('7') => self.view = OxideNetView::Nodelist,
                KeyCode::Char('8') => self.view = OxideNetView::ConfigPackage,
                KeyCode::Esc => return UiAction::Navigate(ScreenId::Dashboard),
                KeyCode::Up if self.view == OxideNetView::Applications => {
                    let current = self.application_state.selected().unwrap_or(0);
                    self.application_state
                        .select(Some(current.saturating_sub(1)));
                }
                KeyCode::Down if self.view == OxideNetView::Applications => {
                    let current = self.application_state.selected().unwrap_or(0);
                    let max = self
                        .dashboard
                        .as_ref()
                        .map(|dashboard| dashboard.applications.len().saturating_sub(1))
                        .unwrap_or(0);
                    self.application_state.select(Some((current + 1).min(max)));
                }
                KeyCode::Up if self.view == OxideNetView::Nodes => {
                    let current = self.node_state.selected().unwrap_or(0);
                    self.node_state.select(Some(current.saturating_sub(1)));
                }
                KeyCode::Down if self.view == OxideNetView::Nodes => {
                    let current = self.node_state.selected().unwrap_or(0);
                    let max = self
                        .dashboard
                        .as_ref()
                        .map(|dashboard| dashboard.nodes.len().saturating_sub(1))
                        .unwrap_or(0);
                    self.node_state.select(Some((current + 1).min(max)));
                }
                KeyCode::Char('i') if self.view == OxideNetView::Dashboard && !readonly => {
                    self.pending_action = Some(OxideNetPendingAction::InstallHub);
                    return confirm("Install OxideNet Hub", "Install OxideNet hub defaults?");
                }
                KeyCode::Char('a') if self.view == OxideNetView::Applications && !readonly => {
                    if let Some(application_id) = self.selected_application_id() {
                        self.pending_action =
                            Some(OxideNetPendingAction::ApproveApplication { application_id });
                        return confirm(
                            "Approve OxideNet Application",
                            "Approve this application, assign the next member address, and generate credentials?",
                        );
                    }
                }
                KeyCode::Char('r') if self.view == OxideNetView::Applications && !readonly => {
                    if let Some(application_id) = self.selected_application_id() {
                        self.pending_action =
                            Some(OxideNetPendingAction::RejectApplication { application_id });
                        return confirm(
                            "Reject OxideNet Application",
                            "Reject this OxideNet application?",
                        );
                    }
                }
                KeyCode::Char('h') if self.view == OxideNetView::Applications && !readonly => {
                    if let Some(application_id) = self.selected_application_id() {
                        self.pending_action =
                            Some(OxideNetPendingAction::HoldApplication { application_id });
                        return confirm(
                            "Hold OxideNet Application",
                            "Place this application on hold?",
                        );
                    }
                }
                KeyCode::Char('s') if self.view == OxideNetView::Nodes && !readonly => {
                    if let Some((node_id, suspended)) = self.selected_node_status() {
                        self.pending_action = if suspended {
                            Some(OxideNetPendingAction::ActivateNode { node_id })
                        } else {
                            Some(OxideNetPendingAction::SuspendNode { node_id })
                        };
                        return confirm(
                            if suspended {
                                "Activate OxideNet Node"
                            } else {
                                "Suspend OxideNet Node"
                            },
                            if suspended {
                                "Reactivate this OxideNet node?"
                            } else {
                                "Suspend this OxideNet node and block mail exchange?"
                            },
                        );
                    }
                }
                KeyCode::Char('p') if self.view == OxideNetView::Nodes && !readonly => {
                    if let Some((node_id, _)) = self.selected_node_status() {
                        self.pending_action =
                            Some(OxideNetPendingAction::RotatePassword { node_id });
                        return confirm(
                            "Rotate OxideNet Password",
                            "Rotate this node's BinkP session password?",
                        );
                    }
                }
                KeyCode::Char('t') if self.view == OxideNetView::Nodes && !readonly => {
                    if let Some((node_id, _)) = self.selected_node_status() {
                        self.pending_action =
                            Some(OxideNetPendingAction::IssueJoinToken { node_id });
                        return confirm("Issue OxideNet Token", "Issue a one-time join token?");
                    }
                }
                KeyCode::Char('g') if self.view == OxideNetView::Nodelist && !readonly => {
                    self.pending_action = Some(OxideNetPendingAction::GenerateNodelist);
                    return confirm(
                        "Generate OxideNet Nodelist",
                        "Generate and publish the OxideNet nodelist rows?",
                    );
                }
                _ => {}
            }
        }
        UiAction::None
    }

    pub fn confirm_pending_action(
        &mut self,
        db: &Option<oxidebbs_db::OxideDb>,
    ) -> Result<Option<String>, SysopError> {
        let Some(action) = self.pending_action.take() else {
            return Ok(None);
        };
        let Some(db) = db else {
            return Err(SysopError::Message(
                "database is unavailable for OxideNet action".to_string(),
            ));
        };
        let message = match action {
            OxideNetPendingAction::InstallHub => {
                OxideNetAdminService::install_hub_defaults(db)?;
                "OxideNet hub defaults installed".to_string()
            }
            OxideNetPendingAction::ApproveApplication { application_id } => {
                let secret = OxideNetAdminService::approve_application(db, &application_id)?;
                format!(
                    "Application approved. Credential {} plaintext {}",
                    secret.credential_id, secret.plaintext
                )
            }
            OxideNetPendingAction::RejectApplication { application_id } => {
                OxideNetAdminService::review_application(
                    db,
                    &application_id,
                    oxidebbs_oxidenet::ReviewDecision::Reject,
                )?;
                "Application rejected".to_string()
            }
            OxideNetPendingAction::HoldApplication { application_id } => {
                OxideNetAdminService::review_application(
                    db,
                    &application_id,
                    oxidebbs_oxidenet::ReviewDecision::Hold,
                )?;
                "Application placed on hold".to_string()
            }
            OxideNetPendingAction::SuspendNode { node_id } => {
                OxideNetAdminService::set_node_suspended(db, &node_id, true)?;
                "Node suspended".to_string()
            }
            OxideNetPendingAction::ActivateNode { node_id } => {
                OxideNetAdminService::set_node_suspended(db, &node_id, false)?;
                "Node activated".to_string()
            }
            OxideNetPendingAction::RotatePassword { node_id } => {
                let secret = OxideNetAdminService::rotate_node_password(db, &node_id)?;
                format!(
                    "Password rotated. Credential {} plaintext {}",
                    secret.credential_id, secret.plaintext
                )
            }
            OxideNetPendingAction::IssueJoinToken { node_id } => {
                let secret = OxideNetAdminService::issue_join_token(db, &node_id)?;
                format!(
                    "Join token issued. Credential {} plaintext {}",
                    secret.credential_id, secret.plaintext
                )
            }
            OxideNetPendingAction::GenerateNodelist => {
                let count = OxideNetAdminService::generate_nodelist(db)?;
                format!("Nodelist generated with {count} entries")
            }
        };
        self.status = Some(message.clone());
        self.refresh(db);
        Ok(Some(message))
    }

    pub fn cancel_pending_action(&mut self) {
        self.pending_action = None;
    }

    fn selected_application_id(&self) -> Option<String> {
        self.dashboard.as_ref().and_then(|dashboard| {
            self.application_state
                .selected()
                .and_then(|index| dashboard.applications.get(index))
                .map(|application| application.id.clone())
        })
    }

    fn selected_node_status(&self) -> Option<(String, bool)> {
        self.dashboard.as_ref().and_then(|dashboard| {
            self.node_state
                .selected()
                .and_then(|index| dashboard.nodes.get(index))
                .map(|node| (node.id.clone(), node.status == "suspended"))
        })
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_tabs(frame, layout[0]);
        match self.view {
            OxideNetView::Dashboard => self.render_dashboard(frame, layout[1]),
            OxideNetView::Applications => self.render_applications(frame, layout[1]),
            OxideNetView::Nodes => self.render_nodes(frame, layout[1]),
            OxideNetView::Queues => self.render_placeholder(frame, layout[1], "Packet Queues"),
            OxideNetView::Subscriptions => {
                self.render_placeholder(frame, layout[1], "Area Subscriptions");
            }
            OxideNetView::PollLogs => self.render_placeholder(frame, layout[1], "Poll Logs"),
            OxideNetView::Nodelist => self.render_placeholder(frame, layout[1], "Nodelist"),
            OxideNetView::ConfigPackage => {
                self.render_placeholder(frame, layout[1], "Config Packages");
            }
        }
        let hints = match self.view {
            OxideNetView::Dashboard => "1-8 Tabs | I Install Hub | F5 Refresh | Esc Back",
            OxideNetView::Applications => {
                "↑↓ Move | A Approve | R Reject | H Hold | 1-8 Tabs | Esc Back"
            }
            OxideNetView::Nodes => {
                "↑↓ Move | S Suspend/Activate | P Rotate Password | T Token | 1-8 Tabs | Esc Back"
            }
            OxideNetView::Nodelist => "G Generate Nodelist | 1-8 Tabs | Esc Back",
            _ => "1-8 Tabs | F5 Refresh | Esc Back",
        };
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(layout[2], frame.buffer_mut());
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let labels = [
            OxideNetView::Dashboard,
            OxideNetView::Applications,
            OxideNetView::Nodes,
            OxideNetView::Queues,
            OxideNetView::Subscriptions,
            OxideNetView::PollLogs,
            OxideNetView::Nodelist,
            OxideNetView::ConfigPackage,
        ];
        let line = labels
            .iter()
            .enumerate()
            .map(|(index, view)| {
                let text = format!(" {} {} ", index + 1, view.label());
                if *view == self.view {
                    Span::styled(text, self.theme.selected_style())
                } else {
                    Span::styled(text, self.theme.muted_style())
                }
            })
            .collect::<Vec<_>>();
        Paragraph::new(Line::from(line))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" OxideNet ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }

    fn render_dashboard(&self, frame: &mut Frame, area: Rect) {
        let text = if let Some(error) = &self.error {
            format!("OxideNet unavailable: {error}")
        } else if let Some(dashboard) = &self.dashboard {
            format!(
                "applications={} pending={} nodes={} suspended={}\nactive_tokens={} packet_queue={} quarantine={} subscriptions={} poll_logs={}\n{}",
                dashboard.applications.len(),
                dashboard.pending_applications,
                dashboard.nodes.len(),
                dashboard.suspended_nodes,
                dashboard.active_tokens,
                dashboard.packet_queue_count,
                dashboard.quarantine_count,
                dashboard.subscriptions,
                dashboard.poll_logs,
                self.status.as_deref().unwrap_or("Ready")
            )
        } else {
            "OxideNet dashboard unavailable: database is not open".to_string()
        };
        Paragraph::new(text)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" OxideNet Dashboard ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }

    fn render_applications(&self, frame: &mut Frame, area: Rect) {
        let applications = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.applications.as_slice())
            .unwrap_or(&[]);
        let rows = applications
            .iter()
            .map(|application| {
                Row::new(vec![
                    application.status.clone(),
                    application.board_name.clone(),
                    application.sysop_alias.clone(),
                    application.host.clone(),
                    application.assigned_address.clone().unwrap_or_default(),
                ])
                .style(self.theme.normal_style())
            })
            .collect::<Vec<_>>();
        let mut state = self.application_state;
        ratatui::prelude::StatefulWidget::render(
            Table::new(
                rows,
                [
                    Constraint::Length(16),
                    Constraint::Length(22),
                    Constraint::Length(16),
                    Constraint::Length(24),
                    Constraint::Length(12),
                ],
            )
            .header(
                Row::new(vec!["Status", "Board", "Sysop", "Host", "Address"])
                    .style(self.theme.label_style()),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" OxideNet Applications ")
                    .title_style(self.theme.title_style()),
            )
            .row_highlight_style(self.theme.selected_style()),
            area,
            frame.buffer_mut(),
            &mut state,
        );
    }

    fn render_nodes(&self, frame: &mut Frame, area: Rect) {
        let nodes = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.nodes.as_slice())
            .unwrap_or(&[]);
        let rows = nodes
            .iter()
            .map(|node| {
                let style = if node.status == "suspended" {
                    self.theme.warning_style()
                } else {
                    self.theme.normal_style()
                };
                Row::new(vec![
                    node.address.clone(),
                    node.status.clone(),
                    node.board_name.clone(),
                    node.host.clone(),
                    node.last_successful_poll_at.clone().unwrap_or_default(),
                ])
                .style(style)
            })
            .collect::<Vec<_>>();
        let mut state = self.node_state;
        ratatui::prelude::StatefulWidget::render(
            Table::new(
                rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(18),
                    Constraint::Length(24),
                    Constraint::Length(24),
                    Constraint::Min(16),
                ],
            )
            .header(
                Row::new(vec!["Address", "Status", "Board", "Host", "Last Poll"])
                    .style(self.theme.label_style()),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" OxideNet Nodes ")
                    .title_style(self.theme.title_style()),
            )
            .row_highlight_style(self.theme.selected_style()),
            area,
            frame.buffer_mut(),
            &mut state,
        );
    }

    fn render_placeholder(&self, frame: &mut Frame, area: Rect, title: &str) {
        let dashboard = self.dashboard.as_ref();
        let text = match title {
            "Packet Queues" => format!(
                "pending packets={}\nquarantined packets={}",
                dashboard
                    .map(|dashboard| dashboard.packet_queue_count)
                    .unwrap_or(0),
                dashboard
                    .map(|dashboard| dashboard.quarantine_count)
                    .unwrap_or(0)
            ),
            "Area Subscriptions" => format!(
                "subscriptions={}",
                dashboard
                    .map(|dashboard| dashboard.subscriptions)
                    .unwrap_or(0)
            ),
            "Poll Logs" => format!(
                "poll logs={}",
                dashboard.map(|dashboard| dashboard.poll_logs).unwrap_or(0)
            ),
            "Nodelist" => "Generate the published OxideNet nodelist with G.".to_string(),
            "Config Packages" => {
                "Config packages are generated from the Applications and Nodes tabs.".to_string()
            }
            _ => String::new(),
        };
        Paragraph::new(text)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(format!(" {title} "))
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }
}

fn confirm(title: &str, message: &str) -> UiAction {
    UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
        title: title.to_string(),
        message: message.to_string(),
        detail: None,
        confirm_label: "Confirm".to_string(),
        cancel_label: "Cancel".to_string(),
    }))
}
