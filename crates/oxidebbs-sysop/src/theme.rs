use ratatui::style::{Color, Modifier, Style};

#[derive(Clone)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub muted: Color,
    pub label: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub border: Color,
    pub border_focused: Color,
}

impl Theme {
    pub fn oxide_classic() -> Self {
        Self {
            background: Color::Rgb(20, 20, 20),
            foreground: Color::Rgb(220, 220, 220),
            accent: Color::Rgb(255, 140, 0),
            success: Color::Rgb(0, 200, 0),
            warning: Color::Rgb(255, 180, 0),
            danger: Color::Rgb(220, 50, 50),
            muted: Color::Rgb(100, 100, 100),
            label: Color::Rgb(160, 160, 160),
            selection_bg: Color::Rgb(255, 140, 0),
            selection_fg: Color::Rgb(0, 0, 0),
            border: Color::Rgb(80, 80, 80),
            border_focused: Color::Rgb(255, 140, 0),
        }
    }
}

impl Theme {
    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn header_style(&self) -> Style {
        Style::default().fg(self.foreground).bg(self.background)
    }

    pub fn selected_style(&self) -> Style {
        Style::default().fg(self.selection_fg).bg(self.selection_bg)
    }

    pub fn normal_style(&self) -> Style {
        Style::default().fg(self.foreground)
    }

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn success_style(&self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    pub fn danger_style(&self) -> Style {
        Style::default().fg(self.danger)
    }

    pub fn label_style(&self) -> Style {
        Style::default().fg(self.label)
    }

    pub fn block_style(&self, focused: bool) -> Style {
        if focused {
            Style::default().fg(self.border_focused)
        } else {
            Style::default().fg(self.border)
        }
    }
}
