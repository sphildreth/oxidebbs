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
    /// All available theme names in canonical form.
    pub fn available_names() -> &'static [&'static str] {
        &[
            "oxide-classic",
            "wildcat",
            "telegard",
            "vbbs",
            "mystic",
            "midnight",
            "high-contrast",
        ]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let normalized = name.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "oxide-classic" => Some(Self::oxide_classic()),
            "wildcat" => Some(Self::wildcat()),
            "telegard" => Some(Self::telegard()),
            "vbbs" => Some(Self::vbbs()),
            "mystic" => Some(Self::mystic()),
            "midnight" => Some(Self::midnight()),
            "high-contrast" => Some(Self::high_contrast()),
            _ => None,
        }
    }

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

    fn wildcat() -> Self {
        Self {
            background: Color::Rgb(12, 12, 12),
            foreground: Color::Rgb(210, 210, 210),
            accent: Color::Rgb(0, 180, 240),
            success: Color::Rgb(80, 255, 120),
            warning: Color::Rgb(255, 200, 80),
            danger: Color::Rgb(255, 64, 64),
            muted: Color::Rgb(96, 96, 110),
            label: Color::Rgb(150, 150, 150),
            selection_bg: Color::Rgb(0, 180, 240),
            selection_fg: Color::Rgb(12, 12, 12),
            border: Color::Rgb(72, 72, 72),
            border_focused: Color::Rgb(0, 180, 240),
        }
    }

    fn telegard() -> Self {
        Self {
            background: Color::Rgb(8, 18, 38),
            foreground: Color::Rgb(214, 219, 232),
            accent: Color::Rgb(92, 157, 255),
            success: Color::Rgb(82, 214, 107),
            warning: Color::Rgb(255, 179, 67),
            danger: Color::Rgb(255, 92, 90),
            muted: Color::Rgb(104, 117, 149),
            label: Color::Rgb(156, 168, 196),
            selection_bg: Color::Rgb(92, 157, 255),
            selection_fg: Color::Rgb(8, 18, 38),
            border: Color::Rgb(53, 68, 102),
            border_focused: Color::Rgb(92, 157, 255),
        }
    }

    fn vbbs() -> Self {
        Self {
            background: Color::Rgb(16, 18, 15),
            foreground: Color::Rgb(220, 235, 220),
            accent: Color::Rgb(0, 198, 178),
            success: Color::Rgb(88, 240, 140),
            warning: Color::Rgb(255, 206, 84),
            danger: Color::Rgb(255, 101, 101),
            muted: Color::Rgb(110, 128, 112),
            label: Color::Rgb(160, 180, 160),
            selection_bg: Color::Rgb(0, 198, 178),
            selection_fg: Color::Rgb(16, 18, 15),
            border: Color::Rgb(68, 76, 70),
            border_focused: Color::Rgb(0, 198, 178),
        }
    }

    fn mystic() -> Self {
        Self {
            background: Color::Rgb(18, 18, 30),
            foreground: Color::Rgb(215, 215, 240),
            accent: Color::Rgb(150, 120, 255),
            success: Color::Rgb(140, 244, 140),
            warning: Color::Rgb(255, 216, 97),
            danger: Color::Rgb(255, 108, 116),
            muted: Color::Rgb(114, 114, 146),
            label: Color::Rgb(170, 170, 204),
            selection_bg: Color::Rgb(150, 120, 255),
            selection_fg: Color::Rgb(18, 18, 30),
            border: Color::Rgb(76, 76, 102),
            border_focused: Color::Rgb(150, 120, 255),
        }
    }

    fn midnight() -> Self {
        Self {
            background: Color::Rgb(8, 9, 11),
            foreground: Color::Rgb(190, 194, 200),
            accent: Color::Rgb(138, 146, 157),
            success: Color::Rgb(184, 184, 184),
            warning: Color::Rgb(150, 150, 150),
            danger: Color::Rgb(210, 210, 210),
            muted: Color::Rgb(95, 102, 111),
            label: Color::Rgb(154, 161, 170),
            selection_bg: Color::Rgb(47, 51, 56),
            selection_fg: Color::Rgb(240, 242, 244),
            border: Color::Rgb(54, 58, 64),
            border_focused: Color::Rgb(138, 146, 157),
        }
    }

    fn high_contrast() -> Self {
        Self {
            background: Color::Black,
            foreground: Color::White,
            accent: Color::Yellow,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            muted: Color::Gray,
            label: Color::LightYellow,
            selection_bg: Color::White,
            selection_fg: Color::Black,
            border: Color::White,
            border_focused: Color::Yellow,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_supports_documented_theme_presets() {
        assert!(Theme::from_name("oxide-classic").is_some());
        assert!(Theme::from_name("wildcat").is_some());
        assert!(Theme::from_name("telegard").is_some());
        assert!(Theme::from_name("vbbs").is_some());
        assert!(Theme::from_name("mystic").is_some());
        assert!(Theme::from_name("midnight").is_some());
        assert!(Theme::from_name("high-contrast").is_some());
    }

    #[test]
    fn from_name_normalizes_cli_inputs() {
        assert!(Theme::from_name("Oxide_Classic").is_some());
        assert!(Theme::from_name("HIGH-CONTRAST").is_some());
    }

    #[test]
    fn from_name_returns_none_for_unknown_theme() {
        assert!(Theme::from_name("not-a-theme").is_none());
    }

    #[test]
    fn available_names_is_stable() {
        assert_eq!(
            Theme::available_names(),
            &[
                "oxide-classic",
                "wildcat",
                "telegard",
                "vbbs",
                "mystic",
                "midnight",
                "high-contrast",
            ]
        );
    }
}
