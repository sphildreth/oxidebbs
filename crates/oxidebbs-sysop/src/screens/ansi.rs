use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::theme::Theme;
use std::fs;
use std::path::{Path, PathBuf};

pub struct AnsiScreen {
    pub theme: Theme,
    pub screens_path: PathBuf,
    pub entries: Vec<AnsiAssetEntry>,
    pub status: String,
}

pub struct AnsiAssetEntry {
    pub path: String,
    pub bytes: u64,
    pub kind: String,
}

impl AnsiScreen {
    pub fn new(theme: Theme, screens_path: PathBuf) -> Self {
        Self {
            theme,
            screens_path,
            entries: Vec::new(),
            status: "Not loaded".to_string(),
        }
    }

    pub fn refresh(&mut self) {
        match collect_assets(&self.screens_path) {
            Ok(entries) => {
                self.status = "OK".to_string();
                self.entries = entries;
            }
            Err(error) => {
                self.status = format!("Read error: {error}");
                self.entries.clear();
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
                    self.screens_path.display().to_string(),
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
            Line::from(vec![
                Span::styled("Assets: ", self.theme.label_style()),
                Span::styled(self.entries.len().to_string(), self.theme.normal_style()),
            ]),
            Line::from(""),
        ];
        lines.extend(self.entries.iter().take(30).map(|entry| {
            Line::from(vec![
                Span::styled(format!("{:<8}", entry.kind), self.theme.label_style()),
                Span::styled(
                    format!("{:>8} bytes  ", entry.bytes),
                    self.theme.muted_style(),
                ),
                Span::styled(&entry.path, self.theme.normal_style()),
            ])
        }));

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" ANSI Preview ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }
}

fn collect_assets(root: &Path) -> std::io::Result<Vec<AnsiAssetEntry>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    collect_assets_recursive(root, root, &mut entries)?;
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

fn collect_assets_recursive(
    root: &Path,
    path: &Path,
    entries: &mut Vec<AnsiAssetEntry>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_assets_recursive(root, &path, entries)?;
        } else if path.is_file() {
            let metadata = fs::metadata(&path)?;
            let relative = path.strip_prefix(root).unwrap_or(&path);
            entries.push(AnsiAssetEntry {
                path: relative.display().to_string(),
                bytes: metadata.len(),
                kind: asset_kind(&path).to_string(),
            });
        }
    }
    Ok(())
}

fn asset_kind(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("ans") | Some("ansi") => "ANSI",
        Some("asc") | Some("txt") => "TEXT",
        Some("bin") => "BINARY",
        _ => "SCREEN",
    }
}
