use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub struct HeaderWidget<'a> {
    pub board_name: &'a str,
    pub version: &'a str,
    pub uptime: &'a str,
    pub node_summary: &'a str,
    pub user_count: usize,
    pub alert_count: usize,
    pub clock: &'a str,
    pub theme: &'a Theme,
}

impl<'a> Widget for HeaderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let header_text = format!(
            " {} | {} | Up {} | {} | Users {} | Alerts {} | {} ",
            self.board_name,
            self.version,
            self.uptime,
            self.node_summary,
            self.user_count,
            self.alert_count,
            self.clock
        );
        Paragraph::new(header_text)
            .style(self.theme.header_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" OxideBBS Sysop ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, buf);
    }
}
