use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::theme::Theme;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
            UiEvent::Key(key) if key.code == KeyCode::Char('r') => {
                self.status = match self.entries.first() {
                    Some(entry) => {
                        let path = self.screens_path.join(&entry.path);
                        match fs::read(&path) {
                            Ok(bytes) => format!(
                                "Raw {}: {}",
                                entry.path,
                                bytes
                                    .iter()
                                    .take(16)
                                    .map(|byte| format!("{byte:02X}"))
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            ),
                            Err(error) => format!("Raw read failed: {error}"),
                        }
                    }
                    None => "Raw read failed: no ANSI asset selected".to_string(),
                };
            }
            UiEvent::Key(key) if key.code == KeyCode::Char('i') && !_readonly => {
                self.status = match install_default_screens(&self.screens_path) {
                    Ok(count) => {
                        self.refresh();
                        format!("Installed {count} default screen(s)")
                    }
                    Err(error) => format!("Install failed: {error}"),
                };
            }
            UiEvent::Key(key) if key.code == KeyCode::Char('e') && !_readonly => {
                self.status = match self.entries.first() {
                    Some(entry) => launch_editor(&self.screens_path.join(&entry.path)),
                    None => "Editor launch failed: no ANSI asset selected".to_string(),
                };
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

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Actions: ", self.theme.label_style()),
            Span::styled(
                "R Raw Bytes | I Install Defaults | E Editor | Esc Back",
                self.theme.muted_style(),
            ),
        ]));

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

fn install_default_screens(root: &Path) -> std::io::Result<usize> {
    fs::create_dir_all(root)?;
    let defaults = [
        (
            "welcome.ans",
            "\x1b[2J\x1b[1;36mWelcome to OxideBBS\x1b[0m\r\n",
        ),
        (
            "goodbye.ans",
            "\x1b[2J\x1b[1;33mThanks for calling!\x1b[0m\r\n",
        ),
        (
            "apply-oxidenet.ans",
            "\x1b[1;32mOxideNet Application\x1b[0m\r\n",
        ),
    ];
    let mut installed = 0;
    for (name, contents) in defaults {
        let path = root.join(name);
        if !path.exists() {
            fs::write(path, contents.as_bytes())?;
            installed += 1;
        }
    }
    Ok(installed)
}

fn launch_editor(path: &Path) -> String {
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
