use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub enum ModalKind {
    Confirm(ConfirmModal),
    Form(FormModal),
    Error(ErrorModal),
    Info(InfoModal),
}

pub struct ConfirmModal {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    pub confirm_label: String,
    pub cancel_label: String,
}

pub struct FormModal {
    pub title: String,
    pub fields: Vec<FormField>,
    pub active_field: usize,
}

pub struct FormField {
    pub label: String,
    pub value: String,
    pub is_password: bool,
}

pub struct ErrorModal {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    pub suggestion: Option<String>,
}

pub struct InfoModal {
    pub title: String,
    pub message: String,
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn render_modal(modal: &ModalKind, frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(60, 40, frame.area());
    frame.render_widget(Clear, area);

    match modal {
        ModalKind::Confirm(m) => {
            let text = if let Some(detail) = &m.detail {
                format!("{}\n\n{}", m.message, detail)
            } else {
                m.message.clone()
            };
            let footer = format!(" Y {} | N {} ", m.confirm_label, m.cancel_label);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.block_style(true))
                .title(format!(" {} ", m.title))
                .title_style(theme.warning_style());
            let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
            frame.render_widget(paragraph, area);
            // Also render footer at bottom of modal area
            if area.height > 3 {
                let footer_area = Rect {
                    x: area.x,
                    y: area.y + area.height.saturating_sub(3),
                    width: area.width,
                    height: 3,
                };
                frame.render_widget(
                    Paragraph::new(footer).style(theme.muted_style()),
                    footer_area,
                );
            }
        }
        ModalKind::Form(m) => {
            // Render form fields with active field highlighted
            let mut lines = Vec::new();
            for (i, field) in m.fields.iter().enumerate() {
                let marker = if i == m.active_field { "> " } else { "  " };
                let value = if field.is_password {
                    "*".repeat(field.value.len())
                } else {
                    field.value.clone()
                };
                lines.push(format!("{}{}: {}", marker, field.label, value));
            }
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.block_style(true))
                .title(format!(" {} ", m.title))
                .title_style(theme.title_style());
            let paragraph = Paragraph::new(lines.join("\n"))
                .block(block)
                .wrap(Wrap { trim: true });
            frame.render_widget(paragraph, area);
        }
        ModalKind::Error(m) => {
            let mut text = m.message.clone();
            if let Some(detail) = &m.detail {
                text.push_str(&format!("\n\n{}", detail));
            }
            if let Some(suggestion) = &m.suggestion {
                text.push_str(&format!("\n\nSuggested: {}", suggestion));
            }
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.danger))
                .title(format!(" {} ", m.title))
                .title_style(theme.danger_style());
            let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
            frame.render_widget(paragraph, area);
        }
        ModalKind::Info(m) => {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.block_style(true))
                .title(format!(" {} ", m.title))
                .title_style(theme.title_style());
            let paragraph = Paragraph::new(m.message.as_str())
                .block(block)
                .wrap(Wrap { trim: true });
            frame.render_widget(paragraph, area);
        }
    }
}
