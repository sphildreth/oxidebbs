use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::theme::Theme;

pub struct AnsiScreen {
    pub theme: Theme,
}

impl AnsiScreen {
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<oxidebbs_db::OxideDb>,
        _readonly: bool,
    ) -> UiAction {
        match event {
            UiEvent::Key(key) if key.code == KeyCode::Esc => {
                return UiAction::Navigate(ScreenId::Dashboard);
            }
            _ => {}
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let lines = vec![
            Line::from("ANSI Preview"),
            Line::from(""),
            Line::from("Preview ANSI/CP437 screens and test terminal rendering."),
            Line::from(""),
            Line::from("Not yet implemented."),
        ];

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" ANSI Preview ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }
}
