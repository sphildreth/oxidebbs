use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
    FocusNext,
    FocusPrev,
    Confirm,
    Cancel,
    Quit,
    Help,
    CommandPalette,
    Search,
    Refresh,
    NavigateTo(ScreenId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenId {
    Dashboard,
    Nodes,
    Users,
    Messages,
    Doors,
    Ansi,
    Config,
    Database,
    Logs,
    Audit,
    Help,
}

impl ScreenId {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Nodes => "Nodes",
            Self::Users => "Users",
            Self::Messages => "Messages",
            Self::Doors => "Doors",
            Self::Ansi => "ANSI",
            Self::Config => "Config",
            Self::Database => "Database",
            Self::Logs => "Logs",
            Self::Audit => "Audit",
            Self::Help => "Help",
        }
    }

    pub fn all() -> &'static [ScreenId] {
        &[
            Self::Dashboard,
            Self::Nodes,
            Self::Users,
            Self::Messages,
            Self::Doors,
            Self::Ansi,
            Self::Config,
            Self::Database,
            Self::Logs,
            Self::Audit,
            Self::Help,
        ]
    }
}

pub fn translate_key(key: KeyEvent) -> UiEvent {
    match (key.modifiers, key.code) {
        (_, KeyCode::F(1)) => UiEvent::Help,
        (_, KeyCode::F(2)) => UiEvent::CommandPalette,
        (_, KeyCode::F(3)) => UiEvent::Search,
        (_, KeyCode::F(5)) => UiEvent::Refresh,
        (_, KeyCode::Tab) => UiEvent::FocusNext,
        (KeyModifiers::SHIFT, KeyCode::BackTab) => UiEvent::FocusPrev,
        (_, KeyCode::BackTab) => UiEvent::FocusPrev,
        (_, KeyCode::Enter) => UiEvent::Confirm,
        (_, KeyCode::Esc) => UiEvent::Cancel,
        (KeyModifiers::NONE, KeyCode::Char('q')) => UiEvent::Quit,
        (KeyModifiers::NONE, KeyCode::Char('?')) => UiEvent::Help,
        (KeyModifiers::NONE, KeyCode::Char('/')) => UiEvent::Search,
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => UiEvent::NavigateTo(ScreenId::Nodes),
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => UiEvent::NavigateTo(ScreenId::Users),
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => UiEvent::NavigateTo(ScreenId::Doors),
        (KeyModifiers::CONTROL, KeyCode::Char('m')) => UiEvent::NavigateTo(ScreenId::Messages),
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => UiEvent::NavigateTo(ScreenId::Logs),
        (KeyModifiers::CONTROL, KeyCode::Char('b')) => UiEvent::NavigateTo(ScreenId::Database),
        _ => UiEvent::Key(key),
    }
}
