use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::theme::Theme;
use std::fs;
use std::path::PathBuf;

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
            _ => {}
        }
        UiAction::None
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
