use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

const CONFIRM_MIN_WIDTH: u16 = 40;
const CONFIRM_MAX_WIDTH: u16 = 88;
const MODAL_HORIZONTAL_MARGIN: u16 = 4;
const MODAL_VERTICAL_MARGIN: u16 = 2;

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
    match modal {
        ModalKind::Confirm(m) => render_confirm_modal(m, frame, theme),
        ModalKind::Form(m) => {
            let area = centered_rect(60, 40, frame.area());
            frame.render_widget(Clear, area);

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
            let area = centered_rect(60, 40, frame.area());
            frame.render_widget(Clear, area);

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
            let area = centered_rect(60, 40, frame.area());
            frame.render_widget(Clear, area);

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

fn render_confirm_modal(modal: &ConfirmModal, frame: &mut Frame, theme: &Theme) {
    let area = confirm_modal_area(modal, frame.area());
    frame.render_widget(Clear, area);

    let text = confirm_text(modal);
    let footer = confirm_footer(modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.block_style(true))
        .title(format!(" {} ", modal.title))
        .title_style(theme.warning_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let footer_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    let body_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };

    frame.render_widget(
        Paragraph::new(text)
            .style(theme.normal_style())
            .wrap(Wrap { trim: true }),
        body_area,
    );
    frame.render_widget(
        Paragraph::new(footer).style(theme.label_style()),
        footer_area,
    );
}

fn confirm_modal_area(modal: &ConfirmModal, frame_area: Rect) -> Rect {
    let text = confirm_text(modal);
    let footer = confirm_footer(modal);
    let content_width = text
        .lines()
        .map(display_width)
        .chain([display_width(&modal.title), display_width(&footer)])
        .max()
        .unwrap_or(0);

    let available_width = frame_area
        .width
        .saturating_sub(MODAL_HORIZONTAL_MARGIN)
        .max(1);
    let max_width = available_width.clamp(1, CONFIRM_MAX_WIDTH);
    let min_width = CONFIRM_MIN_WIDTH.min(max_width).max(1);
    let width = u16::try_from(content_width.saturating_add(4))
        .unwrap_or(CONFIRM_MAX_WIDTH)
        .clamp(min_width, max_width);

    let inner_width = width.saturating_sub(2).max(1);
    let body_lines = wrapped_line_count(&text, inner_width);
    let preferred_height = body_lines.saturating_add(3).max(5);
    let max_height = frame_area
        .height
        .saturating_sub(MODAL_VERTICAL_MARGIN)
        .max(1);
    let height = preferred_height.min(max_height);

    centered_fixed_rect(width, height, frame_area)
}

fn centered_fixed_rect(width: u16, height: u16, area: Rect) -> Rect {
    if area.width == 0 || area.height == 0 {
        return area;
    }

    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn confirm_text(modal: &ConfirmModal) -> String {
    if let Some(detail) = &modal.detail {
        format!("{}\n\n{}", modal.message, detail)
    } else {
        modal.message.clone()
    }
}

fn confirm_footer(modal: &ConfirmModal) -> String {
    format!(" Y {} | N {} ", modal.confirm_label, modal.cancel_label)
}

fn display_width(value: &str) -> usize {
    value.chars().count()
}

fn wrapped_line_count(text: &str, width: u16) -> u16 {
    if text.is_empty() {
        return 1;
    }

    let width = usize::from(width.max(1));
    let count = text
        .lines()
        .map(|line| {
            let len = display_width(line);
            if len == 0 { 1 } else { len.div_ceil(width) }
        })
        .sum::<usize>();
    u16::try_from(count).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quit_modal() -> ConfirmModal {
        ConfirmModal {
            title: "Quit Sysop TUI".to_string(),
            message: "Quit the sysop TUI?".to_string(),
            detail: Some(
                "If this sysop session started an embedded server, quitting will stop it."
                    .to_string(),
            ),
            confirm_label: "Quit".to_string(),
            cancel_label: "Cancel".to_string(),
        }
    }

    #[test]
    fn confirm_modal_sizes_to_quit_prompt_content() {
        let area = confirm_modal_area(&quit_modal(), Rect::new(0, 0, 200, 60));

        assert!(area.width <= CONFIRM_MAX_WIDTH);
        assert!(area.width < 100);
        assert!((5..=8).contains(&area.height));
    }

    #[test]
    fn confirm_modal_wraps_long_detail_within_width_cap() {
        let mut modal = quit_modal();
        modal.detail = Some("This is a deliberately long confirmation detail that should wrap inside the dialog instead of expanding the modal across the whole sysop console.".to_string());

        let area = confirm_modal_area(&modal, Rect::new(0, 0, 120, 40));

        assert_eq!(area.width, CONFIRM_MAX_WIDTH);
        assert!(area.height > 5);
        assert!(area.height < 12);
    }
}
