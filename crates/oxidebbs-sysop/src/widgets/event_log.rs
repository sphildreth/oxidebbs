use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub struct EventLogEntry {
    pub timestamp: String,
    pub event_type: String,
    pub details: String,
}

pub struct EventLogWidget<'a> {
    pub entries: &'a [EventLogEntry],
    pub theme: &'a Theme,
}

impl<'a> Widget for EventLogWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self
            .entries
            .iter()
            .map(|entry| {
                Line::from(vec![
                    Span::styled(entry.timestamp.as_str(), self.theme.label_style()),
                    Span::raw("  "),
                    Span::styled(
                        entry.event_type.as_str(),
                        Style::default().fg(self.theme.accent),
                    ),
                    Span::raw("  "),
                    Span::styled(entry.details.as_str(), self.theme.normal_style()),
                ])
            })
            .collect();

        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" Recent Events ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, buf);
    }
}
