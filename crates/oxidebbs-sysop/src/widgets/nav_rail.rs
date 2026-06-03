use crate::input::ScreenId;
use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub struct NavRail<'a> {
    pub items: &'a [ScreenId],
    pub selected: usize,
    pub theme: &'a Theme,
}

impl<'a> NavRail<'a> {
    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|screen| ListItem::new(screen.label()))
            .collect();

        ratatui::prelude::StatefulWidget::render(
            List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.block_style(true))
                        .title(" NAV ")
                        .title_style(self.theme.title_style()),
                )
                .highlight_style(self.theme.selected_style()),
            area,
            buf,
            state,
        );
    }
}
