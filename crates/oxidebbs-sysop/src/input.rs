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
    Files,
    Network,
    OxideNet,
    Doors,
    Ansi,
    Config,
    Database,
    Doctor,
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
            Self::Files => "Files",
            Self::Network => "Network",
            Self::OxideNet => "OxideNet",
            Self::Doors => "Doors",
            Self::Ansi => "ANSI",
            Self::Config => "Config",
            Self::Database => "Database",
            Self::Doctor => "Doctor",
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
            Self::Files,
            Self::Network,
            Self::OxideNet,
            Self::Doors,
            Self::Ansi,
            Self::Config,
            Self::Database,
            Self::Doctor,
            Self::Logs,
            Self::Audit,
            Self::Help,
        ]
    }
}

pub fn translate_key(key: KeyEvent) -> UiEvent {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::F(1) => UiEvent::Help,
        KeyCode::F(2) => UiEvent::CommandPalette,
        KeyCode::F(3) => UiEvent::Search,
        KeyCode::F(5) => UiEvent::Refresh,
        KeyCode::Tab => UiEvent::FocusNext,
        KeyCode::BackTab => UiEvent::FocusPrev,
        KeyCode::Enter => UiEvent::Confirm,
        KeyCode::Esc => UiEvent::Cancel,
        KeyCode::Char('q') if key.modifiers == KeyModifiers::NONE => UiEvent::Quit,
        KeyCode::Char('?') if key.modifiers == KeyModifiers::NONE => UiEvent::Help,
        KeyCode::Char('/') if key.modifiers == KeyModifiers::NONE => UiEvent::Search,
        KeyCode::Char('n' | 'N') if control => UiEvent::NavigateTo(ScreenId::Nodes),
        KeyCode::Char('u' | 'U') if control => UiEvent::NavigateTo(ScreenId::Users),
        KeyCode::Char('d' | 'D') if control => UiEvent::NavigateTo(ScreenId::Doors),
        KeyCode::Char('m' | 'M') if control => UiEvent::NavigateTo(ScreenId::Messages),
        KeyCode::Char('f' | 'F') if control => UiEvent::NavigateTo(ScreenId::Files),
        KeyCode::Char('x' | 'X') if control => UiEvent::NavigateTo(ScreenId::Network),
        KeyCode::Char('o' | 'O') if control => UiEvent::NavigateTo(ScreenId::OxideNet),
        KeyCode::Char('l' | 'L') if control => UiEvent::NavigateTo(ScreenId::Logs),
        KeyCode::Char('b' | 'B') if control => UiEvent::NavigateTo(ScreenId::Database),
        KeyCode::Char('r' | 'R') if control => UiEvent::NavigateTo(ScreenId::Doctor),
        _ => UiEvent::Key(key),
    }
}

#[cfg(test)]
mod tests {
    use super::{ScreenId, UiEvent, translate_key};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn control_navigation_tolerates_shifted_control_letters() {
        assert_eq!(
            translate_key(KeyEvent::new(
                KeyCode::Char('N'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            )),
            UiEvent::NavigateTo(ScreenId::Nodes)
        );
        assert_eq!(
            translate_key(KeyEvent::new(
                KeyCode::Char('O'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            )),
            UiEvent::NavigateTo(ScreenId::OxideNet)
        );
    }

    #[test]
    fn plain_enter_and_escape_translate_to_modal_semantics() {
        assert_eq!(
            translate_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            UiEvent::Confirm
        );
        assert_eq!(
            translate_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            UiEvent::Cancel
        );
    }

    #[test]
    fn network_screen_is_in_navigation_order() {
        assert!(ScreenId::all().contains(&ScreenId::Network));
        assert!(ScreenId::all().contains(&ScreenId::Files));
        assert!(ScreenId::all().contains(&ScreenId::OxideNet));
        assert_eq!(
            translate_key(KeyEvent::new(
                KeyCode::Char('X'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            )),
            UiEvent::NavigateTo(ScreenId::Network)
        );
        assert_eq!(
            translate_key(KeyEvent::new(
                KeyCode::Char('F'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            )),
            UiEvent::NavigateTo(ScreenId::Files)
        );
        assert_eq!(
            translate_key(KeyEvent::new(
                KeyCode::Char('O'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            )),
            UiEvent::NavigateTo(ScreenId::OxideNet)
        );
    }
}
