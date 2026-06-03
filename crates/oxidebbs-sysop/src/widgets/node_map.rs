use crate::events::NodeStatusSnapshot;
use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub struct NodeMapWidget<'a> {
    pub nodes: &'a [NodeStatusSnapshot],
    pub total_configured: u16,
    pub selected: Option<u16>,
    pub theme: &'a Theme,
}

impl<'a> NodeMapWidget<'a> {
    pub fn activity_code(state: &str) -> &'static str {
        match state {
            "available" => "FREE",
            "connecting" => "CONN",
            "login" => "LOGN",
            "main_menu" => "MENU",
            "reading_messages" => "MSGS",
            "posting_message" => "POST",
            "in_door" => "DOOR",
            "disconnecting" => "DISC",
            "offline" => "DOWN",
            "stale" => "STALE",
            _ => "????",
        }
    }

    pub fn activity_style(state: &str, theme: &Theme) -> Style {
        match state {
            "available" => theme.muted_style(),
            "connecting" | "login" | "main_menu" | "reading_messages" | "posting_message" => {
                theme.success_style()
            }
            "in_door" => Style::default().fg(theme.accent),
            "disconnecting" => theme.warning_style(),
            "stale" => theme.danger_style(),
            "offline" => Style::default().fg(Color::Rgb(60, 60, 60)),
            _ => theme.normal_style(),
        }
    }

    pub fn render(self, area: Rect, buf: &mut Buffer) {
        let columns = 4;
        let rows = (self.total_configured as usize).div_ceil(columns);

        let mut lines: Vec<Line> = Vec::new();
        for row in 0..rows {
            let mut spans: Vec<Span> = Vec::new();
            for col in 0..columns {
                let idx = row * columns + col;
                if idx >= self.total_configured as usize {
                    break;
                }
                let node_num = (idx + 1) as u16;

                if let Some(node) = self.nodes.iter().find(|n| n.node_number == node_num) {
                    let code = Self::activity_code(&node.state);
                    let alias = node.user_alias.as_deref().unwrap_or("-");
                    let style = Self::activity_style(&node.state, self.theme);
                    let is_selected = self.selected == Some(node_num);

                    let text = format!("{:02} {:<10} {:<6}", node_num, alias, code);
                    if is_selected {
                        spans.push(Span::styled(text, self.theme.selected_style()));
                    } else {
                        spans.push(Span::styled(text, style));
                    }
                } else {
                    let text = format!("{:02} {:<10} {:<6}", node_num, "-", "FREE");
                    spans.push(Span::styled(text, self.theme.muted_style()));
                }

                if col < columns - 1 {
                    spans.push(Span::raw(" | "));
                }
            }
            lines.push(Line::from(spans));
        }

        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" Node Map ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, buf);
    }
}
