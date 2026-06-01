use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Menu {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub screen: ScreenAsset,
    pub entries: Vec<MenuEntry>,
    #[serde(default)]
    pub pre_menu_screens: Vec<ScreenAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuEntry {
    pub key: String,
    pub label: String,
    pub action: MenuAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScreenAsset {
    pub asset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MenuAction {
    Doors,
    Messages,
    Logoff,
    NewUser,
    Login,
    ShowScreen { screen: ScreenAsset },
    Submenu { menu_id: String },
    Noop,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MenuError {
    #[error("duplicate hotkey '{key}' in menu '{menu_id}'")]
    DuplicateHotkey { menu_id: String, key: char },

    #[error("invalid hotkey in menu '{menu_id}': '{key}'")]
    InvalidHotkey { menu_id: String, key: String },
}

impl Menu {
    /// Returns an action for a pressed key, matching ASCII keys case-insensitively.
    pub fn route(&self, pressed_key: &str) -> Option<MenuAction> {
        let normalized = normalize_key(pressed_key)?;
        self.entries
            .iter()
            .find(|entry| normalize_key(&entry.key) == Some(normalized))
            .map(|entry| entry.action.clone())
    }

    /// Validates that menu entries use unique hotkeys after ASCII case normalization.
    pub fn validate(&self) -> Result<(), MenuError> {
        let mut seen = HashSet::new();
        for entry in &self.entries {
            let key = normalize_key(&entry.key).ok_or_else(|| MenuError::InvalidHotkey {
                menu_id: self.id.clone(),
                key: entry.key.clone(),
            })?;
            if !seen.insert(key) {
                return Err(MenuError::DuplicateHotkey {
                    menu_id: self.id.clone(),
                    key,
                });
            }
        }
        Ok(())
    }
}

fn normalize_key(raw_key: &str) -> Option<char> {
    let mut chars = raw_key.trim().chars();
    let key = chars.next()?;
    if chars.next().is_some() || !key.is_ascii() {
        return None;
    }
    Some(key.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_menu() -> Menu {
        Menu {
            id: "main".to_string(),
            title: "Main menu".to_string(),
            description: Some("Callers menu".to_string()),
            screen: ScreenAsset {
                asset: "screens/menus/main/main.ans".to_string(),
            },
            pre_menu_screens: vec![
                ScreenAsset {
                    asset: "ansi/welcome.ans".to_string(),
                },
                ScreenAsset {
                    asset: "ansi/main-menu.ans".to_string(),
                },
            ],
            entries: vec![
                MenuEntry {
                    key: "D".to_string(),
                    label: "Doors".to_string(),
                    action: MenuAction::Doors,
                },
                MenuEntry {
                    key: "L".to_string(),
                    label: "Logoff".to_string(),
                    action: MenuAction::Logoff,
                },
                MenuEntry {
                    key: "S".to_string(),
                    label: "Show".to_string(),
                    action: MenuAction::ShowScreen {
                        screen: ScreenAsset {
                            asset: "ansi/show.ans".to_string(),
                        },
                    },
                },
            ],
        }
    }

    #[test]
    fn route_d_maps_to_doors() {
        let menu = demo_menu();
        assert_eq!(menu.route("D"), Some(MenuAction::Doors));
    }

    #[test]
    fn route_matches_lowercase_input_case_insensitive() {
        let menu = demo_menu();
        assert_eq!(menu.route("d"), Some(MenuAction::Doors));
    }

    #[test]
    fn route_unknown_key_returns_none() {
        let menu = demo_menu();
        assert_eq!(menu.route("Z"), None);
    }

    #[test]
    fn duplicate_keys_rejected_case_insensitive() {
        let menu = Menu {
            id: "main".to_string(),
            title: "Main menu".to_string(),
            description: None,
            screen: ScreenAsset {
                asset: "screens/menus/main/main.ans".to_string(),
            },
            entries: vec![
                MenuEntry {
                    key: "D".to_string(),
                    label: "Doors".to_string(),
                    action: MenuAction::Doors,
                },
                MenuEntry {
                    key: "d".to_string(),
                    label: "Different doors".to_string(),
                    action: MenuAction::Messages,
                },
            ],
            pre_menu_screens: vec![],
        };
        assert_eq!(
            menu.validate(),
            Err(MenuError::DuplicateHotkey {
                menu_id: "main".to_string(),
                key: 'D'
            })
        );
    }

    #[test]
    fn menu_models_pre_menu_screen_sequence() {
        let menu = demo_menu();
        assert_eq!(menu.screen.asset, "screens/menus/main/main.ans");
        assert_eq!(
            menu.pre_menu_screens
                .iter()
                .map(|screen| screen.asset.as_str())
                .collect::<Vec<_>>(),
            vec!["ansi/welcome.ans", "ansi/main-menu.ans"]
        );
        assert_eq!(
            menu.route("S"),
            Some(MenuAction::ShowScreen {
                screen: ScreenAsset {
                    asset: "ansi/show.ans".to_string()
                }
            })
        );
    }
}
