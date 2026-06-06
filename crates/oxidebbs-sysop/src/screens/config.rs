use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::theme::Theme;
use crate::widgets::modal::{FormField, FormModal, ModalKind};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct ConfigScreen {
    pub theme: Theme,
    pub config_path: PathBuf,
    pub status: String,
    pub values: Vec<(String, String)>,
}

impl ConfigScreen {
    pub fn new(theme: Theme, config_path: PathBuf) -> Self {
        Self {
            theme,
            config_path,
            status: "Not loaded".to_string(),
            values: Vec::new(),
        }
    }

    pub fn refresh(&mut self) {
        match fs::read_to_string(&self.config_path) {
            Ok(contents) => match contents.parse::<toml::Value>() {
                Ok(value) => {
                    self.status = "OK".to_string();
                    self.values = flatten_config(&value);
                }
                Err(error) => {
                    self.status = format!("Parse error: {error}");
                    self.values.clear();
                }
            },
            Err(error) => {
                self.status = format!("Read error: {error}");
                self.values.clear();
            }
        }
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<oxidebbs_db::OxideDb>,
        _readonly: bool,
    ) -> UiAction {
        match event {
            UiEvent::Key(key) if key.code == KeyCode::Esc => {
                return UiAction::Navigate(ScreenId::Dashboard);
            }
            UiEvent::Key(key) if key.code == KeyCode::Char('r') => {
                self.refresh();
            }
            UiEvent::Key(key) if key.code == KeyCode::Char('e') && !_readonly => {
                self.status = launch_editor(&self.config_path);
            }
            UiEvent::Key(key) if key.code == KeyCode::Char('s') && !_readonly => {
                return UiAction::OpenModal(ModalKind::Form(FormModal {
                    title: "Set Config Value".to_string(),
                    fields: vec![
                        FormField {
                            label: "Key".to_string(),
                            value: String::new(),
                            is_password: false,
                        },
                        FormField {
                            label: "Value".to_string(),
                            value: String::new(),
                            is_password: false,
                        },
                    ],
                    active_field: 0,
                }));
            }
            _ => {}
        }
        UiAction::None
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<(), crate::SysopError> {
        let contents = fs::read_to_string(&self.config_path)?;
        let mut root = contents
            .parse::<toml::Value>()
            .map_err(|error| crate::SysopError::Message(format!("config parse failed: {error}")))?;
        set_dotted_value(&mut root, key, parse_config_value(value))?;
        fs::write(
            &self.config_path,
            toml::to_string_pretty(&root).map_err(|error| {
                crate::SysopError::Message(format!("config write failed: {error}"))
            })?,
        )?;
        self.refresh();
        Ok(())
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Path: ", self.theme.label_style()),
                Span::styled(
                    self.config_path.display().to_string(),
                    self.theme.normal_style(),
                ),
            ]),
            Line::from(vec![
                Span::styled("Status: ", self.theme.label_style()),
                Span::styled(
                    &self.status,
                    if self.status == "OK" {
                        self.theme.success_style()
                    } else {
                        self.theme.warning_style()
                    },
                ),
            ]),
            Line::from(""),
        ];
        lines.extend(self.values.iter().take(28).map(|(key, value)| {
            Line::from(vec![
                Span::styled(format!("{key}: "), self.theme.label_style()),
                Span::styled(value, self.theme.normal_style()),
            ])
        }));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Actions: ", self.theme.label_style()),
            Span::styled(
                "R Reload | S Set Value | E Editor | Esc Back",
                self.theme.muted_style(),
            ),
        ]));

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" Config ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }
}

fn parse_config_value(value: &str) -> toml::Value {
    if let Ok(parsed) = value.parse::<bool>() {
        toml::Value::Boolean(parsed)
    } else if let Ok(parsed) = value.parse::<i64>() {
        toml::Value::Integer(parsed)
    } else {
        toml::Value::String(value.to_string())
    }
}

fn set_dotted_value(
    root: &mut toml::Value,
    key: &str,
    value: toml::Value,
) -> Result<(), crate::SysopError> {
    let parts = key
        .split('.')
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(crate::SysopError::Message(
            "config key must not be blank".to_string(),
        ));
    }
    let mut current = root;
    for part in &parts[..parts.len() - 1] {
        let table = current.as_table_mut().ok_or_else(|| {
            crate::SysopError::Message(format!("config path {part:?} is not a table"))
        })?;
        current = table
            .entry((*part).to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    }
    let table = current.as_table_mut().ok_or_else(|| {
        crate::SysopError::Message("config target parent is not a table".to_string())
    })?;
    table.insert(parts[parts.len() - 1].to_string(), value);
    Ok(())
}

fn launch_editor(path: &std::path::Path) -> String {
    let editor = std::env::var("EDITOR").unwrap_or_default();
    if editor.trim().is_empty() {
        return format!(
            "Editor launch skipped for {}: EDITOR is not set",
            path.display()
        );
    }
    match Command::new(editor).arg(path).status() {
        Ok(status) => format!("Editor exited with {status} for {}", path.display()),
        Err(error) => format!("Editor launch failed: {error}"),
    }
}

fn flatten_config(value: &toml::Value) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    flatten_value("", value, &mut rows);
    rows.truncate(64);
    rows
}

fn flatten_value(prefix: &str, value: &toml::Value, rows: &mut Vec<(String, String)>) {
    match value {
        toml::Value::Table(table) => {
            for (key, value) in table {
                let path = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_value(&path, value, rows);
            }
        }
        toml::Value::Array(values) => {
            rows.push((prefix.to_string(), format!("{} entries", values.len())));
        }
        toml::Value::String(value) => rows.push((prefix.to_string(), value.clone())),
        toml::Value::Integer(value) => rows.push((prefix.to_string(), value.to_string())),
        toml::Value::Float(value) => rows.push((prefix.to_string(), value.to_string())),
        toml::Value::Boolean(value) => rows.push((prefix.to_string(), value.to_string())),
        toml::Value::Datetime(value) => rows.push((prefix.to_string(), value.to_string())),
    }
}
