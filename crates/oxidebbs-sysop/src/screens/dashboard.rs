use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::events::NodeStatusSnapshot;
use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::audit_service::AuditService;
use crate::services::database_service::DatabaseAdminService;
use crate::services::node_service::NodeAdminService;
use crate::theme::Theme;
use crate::widgets::event_log::{EventLogEntry, EventLogWidget};
use crate::widgets::health_panel::{HealthItem, HealthPanelWidget};
use crate::widgets::modal::{FormField, FormModal, ModalKind};
use crate::widgets::node_map::NodeMapWidget;

pub struct DashboardScreen {
    pub theme: Theme,
    pub nodes: Vec<NodeStatusSnapshot>,
    pub total_configured: u16,
    pub selected_node: u16,
    pub events: Vec<EventLogEntry>,
    pub health_items: Vec<HealthItem>,
    pub alerts: Vec<String>,
}

impl DashboardScreen {
    pub fn new(theme: Theme, total_configured: u16) -> Self {
        Self {
            theme,
            nodes: Vec::new(),
            total_configured,
            selected_node: 1,
            events: Vec::new(),
            health_items: Vec::new(),
            alerts: Vec::new(),
        }
    }

    pub fn title(&self) -> &str {
        "Dashboard"
    }

    pub fn refresh(&mut self, db: &oxidebbs_db::Db, node_service: &NodeAdminService) {
        // called directly from App::refresh_data
        // Load nodes
        if let Ok(nodes) = node_service.list_nodes(db, self.total_configured) {
            self.nodes = nodes;
        }

        // Load recent audit events
        self.events = match AuditService::recent(db, 10) {
            Ok(events) => events
                .into_iter()
                .map(|e| EventLogEntry {
                    timestamp: e.created_at,
                    event_type: e.event_type,
                    details: e.details,
                })
                .collect(),
            Err(_) => Vec::new(),
        };

        // Build health panel
        let mut items = Vec::new();
        let db_ok = DatabaseAdminService::schema_version(db).is_ok();
        items.push(HealthItem {
            label: "DB".to_string(),
            value: if db_ok {
                "OK".to_string()
            } else {
                "Error".to_string()
            },
            is_ok: db_ok,
        });
        let door_count = DatabaseAdminService::count_users(db).unwrap_or(0);
        items.push(HealthItem {
            label: "Users".to_string(),
            value: door_count.to_string(),
            is_ok: true,
        });
        self.health_items = items;

        // Alerts: stale nodes
        self.alerts = self
            .nodes
            .iter()
            .filter(|n| n.state == "stale")
            .map(|n| format!("Node {} is stale", n.node_number))
            .collect();
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<oxidebbs_db::OxideDb>,
        _readonly: bool,
    ) -> UiAction {
        match event {
            UiEvent::Key(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Left | KeyCode::Up => {
                        if self.selected_node > 1 {
                            self.selected_node -= 1;
                        }
                    }
                    KeyCode::Right | KeyCode::Down => {
                        if self.selected_node < self.total_configured {
                            self.selected_node += 1;
                        }
                    }
                    KeyCode::Enter => {
                        return UiAction::Navigate(ScreenId::Nodes);
                    }
                    KeyCode::Char('m') => {
                        return UiAction::OpenModal(ModalKind::Form(FormModal {
                            title: "Send Message".to_string(),
                            fields: vec![
                                FormField {
                                    label: "Node".to_string(),
                                    value: self.selected_node.to_string(),
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
                    KeyCode::Char('b') => {
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
                    _ => {}
                }
            }
            UiEvent::Refresh => {
                return UiAction::Refresh;
            }
            _ => {}
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5), // node map
                Constraint::Min(5), // events
                Constraint::Min(5), // health + alerts
            ])
            .split(area);

        // Node map
        NodeMapWidget {
            nodes: &self.nodes,
            total_configured: self.total_configured,
            selected: Some(self.selected_node),
            theme: &self.theme,
        }
        .render(main_layout[0], frame.buffer_mut());

        // Recent events
        EventLogWidget {
            entries: &self.events,
            theme: &self.theme,
        }
        .render(main_layout[1], frame.buffer_mut());

        // Bottom: health + alerts side by side
        let bottom_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_layout[2]);

        HealthPanelWidget {
            items: &self.health_items,
            theme: &self.theme,
        }
        .render(bottom_layout[0], frame.buffer_mut());

        // Alerts panel
        let alert_text = if self.alerts.is_empty() {
            "No active alerts".to_string()
        } else {
            self.alerts.join("\n")
        };
        Paragraph::new(alert_text)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" Alerts ")
                    .title_style(self.theme.title_style()),
            )
            .render(bottom_layout[1], frame.buffer_mut());
    }
}
