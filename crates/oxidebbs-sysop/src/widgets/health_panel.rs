use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub struct HealthItem {
    pub label: String,
    pub value: String,
    pub is_ok: bool,
}

pub struct HealthPanelWidget<'a> {
    pub items: &'a [HealthItem],
    pub theme: &'a Theme,
}

impl<'a> Widget for HealthPanelWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self
            .items
            .iter()
            .map(|item| {
                let marker = if item.is_ok { "[OK]" } else { "[!!]" };
                let marker_style = if item.is_ok {
                    self.theme.success_style()
                } else {
                    self.theme.warning_style()
                };
                Line::from(vec![
                    Span::styled(marker, marker_style),
                    Span::raw(" "),
                    Span::styled(format!("{}: ", item.label), self.theme.label_style()),
                    Span::styled(item.value.as_str(), self.theme.normal_style()),
                ])
            })
            .collect();

        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" Health ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, buf);
    }
}
