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

        let rows: Vec<Row> = (1..=self.total_configured)
            .map(|node_num| {
                let node = self.nodes.iter().find(|n| n.node_number == node_num);
                match node {
                    Some(n) => {
                        let code = NodeMapWidget::activity_code(&n.state);
                        let alias = n.user_alias.as_deref().unwrap_or("-");
                        let remote = n.remote_address.as_deref().unwrap_or("--");
                        let connected = n.connected_at.as_deref().unwrap_or("--");
                        let style = NodeMapWidget::activity_style(&n.state, self.theme);
                        Row::new(vec![
                            node_num.to_string(),
                            alias.to_string(),
                            code.to_string(),
                            connected.to_string(),
                            "--".to_string(),
                            remote.to_string(),
                            n.state.clone(),
                        ])
                        .style(style)
                    }
                    None => Row::new(vec![
                        node_num.to_string(),
                        "-".to_string(),
                        "FREE".to_string(),
                        "--".to_string(),
                        "--".to_string(),
                        "--".to_string(),
                        "Available".to_string(),
                    ])
                    .style(self.theme.muted_style()),
                }
            })
            .collect();

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
