use crossterm::event::KeyCode;

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::theme::Theme;
use oxidebbs_db::OxideDb;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub struct HelpScreen {
    pub theme: Theme,
}

impl HelpScreen {
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }
}

impl HelpScreen {
    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<OxideDb>,
        _readonly: bool,
    ) -> UiAction {
        if let UiEvent::Key(key) = event
            && key.code == KeyCode::Esc
        {
            return UiAction::Navigate(ScreenId::Dashboard);
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        Paragraph::new(
            "OxideBBS Sysop TUI Help\n\n\
            F1      - Help\n\
            F2      - Command Palette\n\
            F3      - Search\n\
            F5      - Refresh\n\
            Tab     - Next panel\n\
            Enter   - Open/Confirm\n\
            Esc     - Cancel/Back\n\
            Q       - Quit/Shutdown\n\n\
            Ctrl+N  - Nodes\n\
            Ctrl+U  - Users\n\
            Ctrl+D  - Doors\n\
            Ctrl+M  - Messages\n\
            Ctrl+L  - Logs\n\
            Ctrl+B  - Database\n\
            Ctrl+O  - Doctor\n",
        )
        .style(self.theme.normal_style())
        .render(area, frame.buffer_mut());
    }
}
