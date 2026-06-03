use crate::events::NodeStatusSnapshot;
use crate::theme::Theme;
use crate::widgets::node_map::NodeMapWidget;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Row, Table, TableState};

pub struct NodeTableWidget<'a> {
    pub nodes: &'a [NodeStatusSnapshot],
    pub total_configured: u16,
    pub theme: &'a Theme,
}

impl<'a> NodeTableWidget<'a> {
    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut TableState) {
        let header = Row::new(vec![
            "#", "User", "Activity", "Time On", "Idle", "Remote", "Status",
        ])
        .style(self.theme.label_style())
        .height(1);

        let rows: Vec<Row> = if self.nodes.is_empty() {
            vec![
                Row::new(vec![
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "--".to_string(),
                    "--".to_string(),
                    "--".to_string(),
                    "No nodes match".to_string(),
                ])
                .style(self.theme.muted_style()),
            ]
        } else {
            self.nodes
                .iter()
                .map(|node| {
                    let code = NodeMapWidget::activity_code(&node.state);
                    let alias = node.user_alias.as_deref().unwrap_or("-");
                    let remote = node.remote_address.as_deref().unwrap_or("--");
                    let connected = node.connected_at.as_deref().unwrap_or("--");
                    let style = NodeMapWidget::activity_style(&node.state, self.theme);
                    Row::new(vec![
                        node.node_number.to_string(),
                        alias.to_string(),
                        code.to_string(),
                        connected.to_string(),
                        node.heartbeat_age_seconds
                            .map(|age| format!("{age}s"))
                            .unwrap_or_else(|| "--".to_string()),
                        remote.to_string(),
                        node.state.clone(),
                    ])
                    .style(style)
                })
                .collect()
        };

        let widths = [
            Constraint::Length(4),
            Constraint::Length(14),
            Constraint::Length(18),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(17),
            Constraint::Length(14),
        ];

        ratatui::prelude::StatefulWidget::render(
            Table::new(rows, widths)
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.block_style(true))
                        .title(" Nodes ")
                        .title_style(self.theme.title_style()),
                )
                .row_highlight_style(self.theme.selected_style()),
            area,
            buf,
            state,
        );
    }
}
