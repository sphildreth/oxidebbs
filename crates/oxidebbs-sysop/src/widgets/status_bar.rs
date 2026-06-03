use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub struct StatusBar<'a> {
    pub shortcuts: Vec<(&'a str, &'a str)>,
    pub theme: &'a Theme,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text: String = self
            .shortcuts
            .iter()
            .map(|(key, label)| format!("{} {}", key, label))
            .collect::<Vec<_>>()
            .join(" | ");

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
