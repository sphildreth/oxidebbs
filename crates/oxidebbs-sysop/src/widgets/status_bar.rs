use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub struct StatusBar<'a> {
    pub shortcuts: Vec<(&'a str, &'a str)>,
    pub message: Option<&'a str>,
    pub theme: &'a Theme,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let shortcuts: String = self
            .shortcuts
            .iter()
            .map(|(key, label)| format!("{} {}", key, label))
            .collect::<Vec<_>>()
            .join(" | ");
        let text = match self.message {
            Some(message) if !message.trim().is_empty() => format!("{message} | {shortcuts}"),
            _ => shortcuts,
        };

        Paragraph::new(text)
            .style(self.theme.muted_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false)),
            )
            .render(area, buf);
    }
}
