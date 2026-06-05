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
    pub packet_state: TableState,
    pub subscription_state: TableState,
    pub poll_state: TableState,
    pub nodelist_state: TableState,
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
        let mut packet_state = TableState::default();
        packet_state.select(Some(0));
        let mut subscription_state = TableState::default();
        subscription_state.select(Some(0));
        let mut poll_state = TableState::default();
        poll_state.select(Some(0));
        let mut nodelist_state = TableState::default();
        nodelist_state.select(Some(0));
        Self {
            theme,
            view: OxideNetView::Dashboard,
            dashboard: None,
            application_state,
            node_state,
            packet_state,
            subscription_state,
            poll_state,
            nodelist_state,
            pending_action: None,
            status: None,
            error: None,
        }
    }

    pub fn refresh(&mut self, db: &oxidebbs_db::OxideDb) {
        match OxideNetAdminService::load(db) {
            Ok(dashboard) => {
                select_or_clear(&mut self.application_state, dashboard.applications.len());
                select_or_clear(&mut self.node_state, dashboard.nodes.len());
                select_or_clear(&mut self.packet_state, dashboard.packets.len());
                select_or_clear(
                    &mut self.subscription_state,
                    dashboard.subscription_rows.len(),
                );
                select_or_clear(&mut self.poll_state, dashboard.poll_log_rows.len());
                select_or_clear(&mut self.nodelist_state, dashboard.nodelist.len());
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
                KeyCode::Up => self.move_selection(false),
                KeyCode::Down => self.move_selection(true),
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
            OxideNetView::Queues => self.render_queues(frame, layout[1]),
            OxideNetView::Subscriptions => self.render_subscriptions(frame, layout[1]),
            OxideNetView::PollLogs => self.render_poll_logs(frame, layout[1]),
            OxideNetView::Nodelist => self.render_nodelist(frame, layout[1]),
            OxideNetView::ConfigPackage => self.render_config_packages(frame, layout[1]),
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
            OxideNetView::Queues
            | OxideNetView::Subscriptions
            | OxideNetView::PollLogs
            | OxideNetView::ConfigPackage => "↑↓ Move | 1-8 Tabs | F5 Refresh | Esc Back",
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

    fn render_queues(&self, frame: &mut Frame, area: Rect) {
        let packets = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.packets.as_slice())
            .unwrap_or(&[]);
        let rows = packets
            .iter()
            .map(|packet| {
                Row::new(vec![
                    packet.status.clone(),
                    packet.direction.clone(),
                    packet.filename.clone(),
                    packet.size_bytes.to_string(),
                    packet.created_at.clone(),
                    packet.error_message.clone().unwrap_or_default(),
                ])
                .style(if packet.status == "quarantined" {
                    self.theme.warning_style()
                } else {
                    self.theme.normal_style()
                })
            })
            .collect::<Vec<_>>();
        let mut state = self.packet_state;
        render_table(
            frame,
            area,
            &mut state,
            TableSpec {
                rows,
                header: vec!["Status", "Dir", "File", "Bytes", "Created", "Error"],
                widths: [
                    Constraint::Length(14),
                    Constraint::Length(9),
                    Constraint::Length(24),
                    Constraint::Length(10),
                    Constraint::Length(20),
                    Constraint::Min(18),
                ],
                title: " OxideNet Packet Queues ",
            },
            &self.theme,
        );
    }

    fn render_subscriptions(&self, frame: &mut Frame, area: Rect) {
        let rows = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.subscription_rows.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|subscription| {
                Row::new(vec![
                    if subscription.subscribed { "yes" } else { "no" }.to_string(),
                    subscription.area_id.clone(),
                    subscription.link_id.clone(),
                    subscription.source.clone(),
                    subscription.subscribed_at.clone(),
                    subscription.unsubscribed_at.clone().unwrap_or_default(),
                ])
                .style(if subscription.subscribed {
                    self.theme.normal_style()
                } else {
                    self.theme.muted_style()
                })
            })
            .collect::<Vec<_>>();
        let mut state = self.subscription_state;
        render_table(
            frame,
            area,
            &mut state,
            TableSpec {
                rows,
                header: vec!["On", "Area", "Link", "Source", "Subscribed", "Unsubscribed"],
                widths: [
                    Constraint::Length(4),
                    Constraint::Length(20),
                    Constraint::Length(20),
                    Constraint::Length(12),
                    Constraint::Length(20),
                    Constraint::Min(20),
                ],
                title: " OxideNet Area Subscriptions ",
            },
            &self.theme,
        );
    }

    fn render_poll_logs(&self, frame: &mut Frame, area: Rect) {
        let rows = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.poll_log_rows.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|poll| {
                Row::new(vec![
                    poll.started_at.clone(),
                    poll.status.clone(),
                    poll.direction.clone(),
                    poll.packets_in.to_string(),
                    poll.packets_out.to_string(),
                    poll.bytes_in.to_string(),
                    poll.bytes_out.to_string(),
                    poll.error_message.clone().unwrap_or_default(),
                ])
                .style(if poll.status == "success" {
                    self.theme.normal_style()
                } else {
                    self.theme.warning_style()
                })
            })
            .collect::<Vec<_>>();
        let mut state = self.poll_state;
        render_table(
            frame,
            area,
            &mut state,
            TableSpec {
                rows,
                header: vec![
                    "Started",
                    "Status",
                    "Dir",
                    "In",
                    "Out",
                    "Bytes In",
                    "Bytes Out",
                    "Error",
                ],
                widths: [
                    Constraint::Length(20),
                    Constraint::Length(12),
                    Constraint::Length(9),
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Min(18),
                ],
                title: " OxideNet Poll Logs ",
            },
            &self.theme,
        );
    }

    fn render_nodelist(&self, frame: &mut Frame, area: Rect) {
        let rows = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.nodelist.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|entry| {
                Row::new(vec![
                    format!(
                        "{}:{}/{}{}",
                        entry.zone,
                        entry.net,
                        entry.node,
                        if entry.point > 0 {
                            format!(".{}", entry.point)
                        } else {
                            String::new()
                        }
                    ),
                    entry.parsed_name.clone().unwrap_or_default(),
                    entry.sysop_name.clone().unwrap_or_default(),
                    entry.phone.clone().unwrap_or_default(),
                    entry.speed.clone().unwrap_or_default(),
                    entry.flags.clone(),
                ])
                .style(self.theme.normal_style())
            })
            .collect::<Vec<_>>();
        let mut state = self.nodelist_state;
        render_table(
            frame,
            area,
            &mut state,
            TableSpec {
                rows,
                header: vec!["Address", "Board", "Sysop", "Host", "Port", "Flags"],
                widths: [
                    Constraint::Length(12),
                    Constraint::Length(24),
                    Constraint::Length(18),
                    Constraint::Length(24),
                    Constraint::Length(7),
                    Constraint::Min(16),
                ],
                title: " OxideNet Nodelist ",
            },
            &self.theme,
        );
    }

    fn render_config_packages(&self, frame: &mut Frame, area: Rect) {
        let nodes = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.nodes.as_slice())
            .unwrap_or(&[]);
        let rows = nodes
            .iter()
            .map(|node| {
                Row::new(vec![
                    node.address.clone(),
                    node.status.clone(),
                    node.board_name.clone(),
                    node.host.clone(),
                    node.binkp_port.to_string(),
                    node.flags.clone(),
                ])
                .style(if node.status == "suspended" {
                    self.theme.warning_style()
                } else {
                    self.theme.normal_style()
                })
            })
            .collect::<Vec<_>>();
        let mut state = self.node_state;
        render_table(
            frame,
            area,
            &mut state,
            TableSpec {
                rows,
                header: vec!["Address", "Status", "Board", "Host", "Port", "Flags"],
                widths: [
                    Constraint::Length(12),
                    Constraint::Length(18),
                    Constraint::Length(24),
                    Constraint::Length(24),
                    Constraint::Length(7),
                    Constraint::Min(16),
                ],
                title: " OxideNet Config Package Nodes ",
            },
            &self.theme,
        );
    }

    fn move_selection(&mut self, down: bool) {
        let count = match self.view {
            OxideNetView::Applications => self
                .dashboard
                .as_ref()
                .map(|dashboard| dashboard.applications.len())
                .unwrap_or(0),
            OxideNetView::Nodes | OxideNetView::ConfigPackage => self
                .dashboard
                .as_ref()
                .map(|dashboard| dashboard.nodes.len())
                .unwrap_or(0),
            OxideNetView::Queues => self
                .dashboard
                .as_ref()
                .map(|dashboard| dashboard.packets.len())
                .unwrap_or(0),
            OxideNetView::Subscriptions => self
                .dashboard
                .as_ref()
                .map(|dashboard| dashboard.subscription_rows.len())
                .unwrap_or(0),
            OxideNetView::PollLogs => self
                .dashboard
                .as_ref()
                .map(|dashboard| dashboard.poll_log_rows.len())
                .unwrap_or(0),
            OxideNetView::Nodelist => self
                .dashboard
                .as_ref()
                .map(|dashboard| dashboard.nodelist.len())
                .unwrap_or(0),
            OxideNetView::Dashboard => 0,
        };
        let state = match self.view {
            OxideNetView::Applications => &mut self.application_state,
            OxideNetView::Nodes | OxideNetView::ConfigPackage => &mut self.node_state,
            OxideNetView::Queues => &mut self.packet_state,
            OxideNetView::Subscriptions => &mut self.subscription_state,
            OxideNetView::PollLogs => &mut self.poll_state,
            OxideNetView::Nodelist => &mut self.nodelist_state,
            OxideNetView::Dashboard => return,
        };
        move_table_state(state, count, down);
    }
}

struct TableSpec<'a, const N: usize> {
    rows: Vec<Row<'a>>,
    header: Vec<&'a str>,
    widths: [Constraint; N],
    title: &'a str,
}

fn render_table<const N: usize>(
    frame: &mut Frame,
    area: Rect,
    state: &mut TableState,
    spec: TableSpec<'_, N>,
    theme: &Theme,
) {
    ratatui::prelude::StatefulWidget::render(
        Table::new(spec.rows, spec.widths)
            .header(Row::new(spec.header).style(theme.label_style()))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.block_style(true))
                    .title(spec.title)
                    .title_style(theme.title_style()),
            )
            .row_highlight_style(theme.selected_style()),
        area,
        frame.buffer_mut(),
        state,
    );
}

fn select_or_clear(state: &mut TableState, count: usize) {
    if count == 0 {
        state.select(None);
    } else if state.selected().is_none_or(|index| index >= count) {
        state.select(Some(0));
    }
}

fn move_table_state(state: &mut TableState, count: usize, down: bool) {
    if count == 0 {
        state.select(None);
        return;
    }
    let current = state.selected().unwrap_or(0);
    let next = if down {
        (current + 1).min(count.saturating_sub(1))
    } else {
        current.saturating_sub(1)
    };
    state.select(Some(next));
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
