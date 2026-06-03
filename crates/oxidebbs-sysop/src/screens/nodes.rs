use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, TableState};

use crate::events::NodeStatusSnapshot;
use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::node_service::NodeAdminService;
use crate::theme::Theme;
use crate::widgets::modal::{ConfirmModal, FormField, FormModal, ModalKind};
use crate::widgets::node_map::NodeMapWidget;
use crate::widgets::node_table::NodeTableWidget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeView {
    Table,
    Grid,
    ActiveOnly,
    DoorOnly,
    ProblemOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeSort {
    NodeNumber,
    Activity,
    User,
    TimeOn,
}

pub struct NodesScreen {
    pub theme: Theme,
    pub view: NodeView,
    pub nodes: Vec<NodeStatusSnapshot>,
    pub total_configured: u16,
    pub table_state: TableState,
    pub grid_selected: u16,
    pub filter: String,
    pub sort: NodeSort,
    pub detail_node: Option<u16>,
    pub auto_refresh_seconds: u16,
}

impl NodesScreen {
    pub fn new(theme: Theme, total_configured: u16) -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            theme,
            view: NodeView::Table,
            nodes: Vec::new(),
            total_configured,
            table_state,
            grid_selected: 1,
            filter: String::new(),
            sort: NodeSort::NodeNumber,
            detail_node: None,
            auto_refresh_seconds: 2,
        }
    }

    pub fn title(&self) -> &str {
        "Nodes"
    }

    pub fn refresh(&mut self, db: &oxidebbs_db::Db, node_service: &NodeAdminService) {
        if let Ok(nodes) = node_service.list_nodes(db, self.total_configured) {
            self.nodes = nodes;
        }
    }

    #[allow(dead_code)]
    fn filtered_nodes(&self) -> Vec<&NodeStatusSnapshot> {
        let mut nodes: Vec<&NodeStatusSnapshot> = self.nodes.iter().collect();
        match self.view {
            NodeView::ActiveOnly => {
                nodes.retain(|n| n.state != "available" && n.state != "offline");
            }
            NodeView::DoorOnly => {
                nodes.retain(|n| n.state == "in_door");
            }
            NodeView::ProblemOnly => {
                nodes.retain(|n| n.state == "stale" || n.state == "disconnecting");
            }
            _ => {}
        }
        if !self.filter.is_empty() {
            let f = self.filter.to_ascii_lowercase();
            nodes.retain(|n| {
                n.node_number.to_string().contains(&f)
                    || n.state.to_ascii_lowercase().contains(&f)
                    || n.user_alias
                        .as_deref()
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .contains(&f)
            });
        }
        nodes
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<oxidebbs_db::OxideDb>,
        _node_service: &NodeAdminService,
        readonly: bool,
    ) -> UiAction {
        if self.detail_node.is_some() {
            return self.handle_detail_event(event, readonly);
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
                    let max = self.total_configured.saturating_sub(1) as usize;
                    self.table_state.select(Some((current + 1).min(max)));
                }
                KeyCode::Left => {
                    if self.grid_selected > 1 {
                        self.grid_selected -= 1;
                    }
                }
                KeyCode::Right => {
                    if self.grid_selected < self.total_configured {
                        self.grid_selected += 1;
                    }
                }
                KeyCode::Enter => {
                    let idx = self.table_state.selected().unwrap_or(0) as u16 + 1;
                    self.detail_node = Some(idx.min(self.total_configured));
                }
                KeyCode::Char('v') => {
                    self.view = match self.view {
                        NodeView::Table => NodeView::Grid,
                        NodeView::Grid => NodeView::ActiveOnly,
                        NodeView::ActiveOnly => NodeView::DoorOnly,
                        NodeView::DoorOnly => NodeView::ProblemOnly,
                        NodeView::ProblemOnly => NodeView::Table,
                    };
                }
                KeyCode::Char('t') => self.view = NodeView::Table,
                KeyCode::Char('g') => self.view = NodeView::Grid,
                KeyCode::Char('a') => self.view = NodeView::ActiveOnly,
                KeyCode::Char('p') => self.view = NodeView::ProblemOnly,
                KeyCode::Char('d') => self.view = NodeView::DoorOnly,
                KeyCode::Char('f') | KeyCode::Char('/') => {
                    return UiAction::OpenModal(ModalKind::Form(FormModal {
                        title: "Filter Nodes".to_string(),
                        fields: vec![FormField {
                            label: "Filter".to_string(),
                            value: self.filter.clone(),
                            is_password: false,
                        }],
                        active_field: 0,
                    }));
                }
                KeyCode::Char('m') if !readonly => {
                    let node = self.table_state.selected().unwrap_or(0) as u16 + 1;
                    return UiAction::OpenModal(ModalKind::Form(FormModal {
                        title: "Send Message".to_string(),
                        fields: vec![
                            FormField {
                                label: "Node".to_string(),
                                value: node.to_string(),
                                is_password: false,
                            },
                            FormField {
                                label: "Message".to_string(),
                                value: String::new(),
                                is_password: false,
                            },
                        ],
                        active_field: 1,
                    }));
                }
                KeyCode::Char('b') if !readonly => {
                    return UiAction::OpenModal(ModalKind::Form(FormModal {
                        title: "Broadcast Message".to_string(),
                        fields: vec![FormField {
                            label: "Message".to_string(),
                            value: String::new(),
                            is_password: false,
                        }],
                        active_field: 0,
                    }));
                }
                KeyCode::Char('k') if !readonly => {
                    let node = self.table_state.selected().unwrap_or(0) as u16 + 1;
                    return UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
                        title: "Kill Door".to_string(),
                        message: format!("Kill door on node {}?", node),
                        detail: Some("This will terminate the running door process.".to_string()),
                        confirm_label: "Kill".to_string(),
                        cancel_label: "Cancel".to_string(),
                    }));
                }
                KeyCode::Char('r') if !readonly => {
                    return UiAction::Refresh;
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
                    let max = self.total_configured.saturating_sub(1) as usize;
                    self.table_state.select(Some((current + 5).min(max)));
                }
                KeyCode::Home => {
                    self.table_state.select(Some(0));
                }
                KeyCode::End => {
                    let max = self.total_configured.saturating_sub(1) as usize;
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

    fn handle_detail_event(&mut self, event: UiEvent, readonly: bool) -> UiAction {
        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Esc => {
                    self.detail_node = None;
                }
                KeyCode::Char('m') if !readonly => {
                    if let Some(node) = self.detail_node {
                        return UiAction::OpenModal(ModalKind::Form(FormModal {
                            title: "Send Message".to_string(),
                            fields: vec![
                                FormField {
                                    label: "Node".to_string(),
                                    value: node.to_string(),
                                    is_password: false,
                                },
                                FormField {
                                    label: "Message".to_string(),
                                    value: String::new(),
                                    is_password: false,
                                },
                            ],
                            active_field: 1,
                        }));
                    }
                }
                KeyCode::Char('d') if !readonly => {
                    if let Some(node) = self.detail_node {
                        return UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
                            title: "Disconnect Node".to_string(),
                            message: format!("Disconnect node {}?", node),
                            detail: Some("This will terminate the active session.".to_string()),
                            confirm_label: "Disconnect".to_string(),
                            cancel_label: "Cancel".to_string(),
                        }));
                    }
                }
                _ => {}
            },
            UiEvent::Cancel => {
                self.detail_node = None;
            }
            _ => {}
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if let Some(node_num) = self.detail_node {
            self.render_detail(frame, area, node_num);
            return;
        }

        // Toolbar
        let toolbar_text = format!(
            "Nodes: {}/{} active | View: {:?} | Filter: {} | Sort: {:?}",
            self.nodes.iter().filter(|n| n.state != "available").count(),
            self.total_configured,
            self.view,
            if self.filter.is_empty() {
                "all".to_string()
            } else {
                self.filter.clone()
            },
            self.sort,
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

        match self.view {
            NodeView::Grid => {
                NodeMapWidget {
                    nodes: &self.nodes,
                    total_configured: self.total_configured,
                    selected: Some(self.grid_selected),
                    theme: &self.theme,
                }
                .render(main_layout[1], frame.buffer_mut());
            }
            _ => {
                NodeTableWidget {
                    nodes: &self.nodes,
                    total_configured: self.total_configured,
                    theme: &self.theme,
                }
                .render(
                    main_layout[1],
                    frame.buffer_mut(),
                    &mut TableState::default(),
                );
            }
        }

        // Footer hints
        let hints = "↑↓ Move | Enter Detail | M Msg | D Disconnect | K Kill | B Broadcast | F Filter | Esc Back";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[2], frame.buffer_mut());
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect, node_num: u16) {
        let node = self.nodes.iter().find(|n| n.node_number == node_num);
        let mut lines = vec![Line::from(format!("Node: {}", node_num))];
        if let Some(n) = node {
            lines.push(Line::from(vec![
                Span::styled("State: ", self.theme.label_style()),
                Span::styled(
                    &n.state,
                    NodeMapWidget::activity_style(&n.state, &self.theme),
                ),
            ]));
            if let Some(ref alias) = n.user_alias {
                lines.push(Line::from(vec![
                    Span::styled("User: ", self.theme.label_style()),
                    Span::styled(alias.as_str(), self.theme.normal_style()),
                ]));
            }
            if let Some(ref remote) = n.remote_address {
                lines.push(Line::from(vec![
                    Span::styled("Remote: ", self.theme.label_style()),
                    Span::styled(remote.as_str(), self.theme.normal_style()),
                ]));
            }
            if let Some(ref connected) = n.connected_at {
                lines.push(Line::from(vec![
                    Span::styled("Connected: ", self.theme.label_style()),
                    Span::styled(connected.as_str(), self.theme.normal_style()),
                ]));
            }
        } else {
            lines.push(Line::from("No active session on this node."));
        }

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(format!(" Node {} Detail ", node_num))
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }
}
