use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::SysopError;
use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::file_service::{FileAdminService, FileDashboard};
use crate::theme::Theme;
use crate::widgets::modal::{ConfirmModal, ModalKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesView {
    Areas,
    Entries,
    Transfers,
}

impl FilesView {
    fn label(self) -> &'static str {
        match self {
            Self::Areas => "Areas",
            Self::Entries => "Entries",
            Self::Transfers => "Transfers",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesPendingAction {
    SetAreaEnabled { area_id: String, enabled: bool },
    SetEntryApproved { entry_id: String, approved: bool },
}

pub struct FilesScreen {
    pub theme: Theme,
    pub view: FilesView,
    pub dashboard: Option<FileDashboard>,
    pub area_state: TableState,
    pub entry_state: TableState,
    pub transfer_state: TableState,
    pub pending_action: Option<FilesPendingAction>,
    pub error: Option<String>,
    pub status: Option<String>,
}

impl FilesScreen {
    pub fn new(theme: Theme) -> Self {
        let mut area_state = TableState::default();
        area_state.select(Some(0));
        let mut entry_state = TableState::default();
        entry_state.select(Some(0));
        let mut transfer_state = TableState::default();
        transfer_state.select(Some(0));
        Self {
            theme,
            view: FilesView::Areas,
            dashboard: None,
            area_state,
            entry_state,
            transfer_state,
            pending_action: None,
            error: None,
            status: None,
        }
    }

    pub fn refresh(&mut self, db: &oxidebbs_db::OxideDb) {
        match FileAdminService::load(db.db()) {
            Ok(dashboard) => {
                select_or_clear(&mut self.area_state, dashboard.areas.len());
                select_or_clear(&mut self.entry_state, dashboard.entries.len());
                select_or_clear(&mut self.transfer_state, dashboard.transfers.len());
                self.dashboard = Some(dashboard);
                self.error = None;
            }
            Err(error) => {
                self.dashboard = None;
                self.error = Some(error.to_string());
                self.area_state.select(None);
                self.entry_state.select(None);
                self.transfer_state.select(None);
            }
        }
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<oxidebbs_db::OxideDb>,
        readonly: bool,
    ) -> UiAction {
        if let UiEvent::Key(key) = event {
            match key.code {
                KeyCode::Char('1') => self.view = FilesView::Areas,
                KeyCode::Char('2') => self.view = FilesView::Entries,
                KeyCode::Char('3') => self.view = FilesView::Transfers,
                KeyCode::Esc => return UiAction::Navigate(ScreenId::Dashboard),
                KeyCode::Up => self.move_selection(false),
                KeyCode::Down => self.move_selection(true),
                KeyCode::Char('d') if self.view == FilesView::Areas && !readonly => {
                    if let Some((area_id, enabled, key)) = self.selected_area_status() {
                        self.pending_action = Some(FilesPendingAction::SetAreaEnabled {
                            area_id,
                            enabled: !enabled,
                        });
                        return confirm(
                            if enabled {
                                "Disable File Area"
                            } else {
                                "Enable File Area"
                            },
                            &format!(
                                "{} file area {key}?",
                                if enabled { "Disable" } else { "Enable" }
                            ),
                        );
                    }
                }
                KeyCode::Char('a') if self.view == FilesView::Entries && !readonly => {
                    if let Some((entry_id, approved, display_name)) = self.selected_entry_status() {
                        self.pending_action = Some(FilesPendingAction::SetEntryApproved {
                            entry_id,
                            approved: !approved,
                        });
                        return confirm(
                            if approved {
                                "Unapprove File Entry"
                            } else {
                                "Approve File Entry"
                            },
                            &format!(
                                "{} file entry {display_name}?",
                                if approved { "Unapprove" } else { "Approve" }
                            ),
                        );
                    }
                }
                _ => {}
            }
        }
        UiAction::None
    }

    pub fn confirm_pending_action(
        &mut self,
        db: &Option<oxidebbs_db::OxideDb>,
    ) -> Result<Option<String>, SysopError> {
        let Some(action) = self.pending_action.take() else {
            return Ok(None);
        };
        let Some(db) = db else {
            return Err(SysopError::Message(
                "database is unavailable for file action".to_string(),
            ));
        };
        let message = match action {
            FilesPendingAction::SetAreaEnabled { area_id, enabled } => {
                FileAdminService::set_area_enabled(db.db(), &area_id, enabled)?;
                if enabled {
                    "File area enabled"
                } else {
                    "File area disabled"
                }
            }
            FilesPendingAction::SetEntryApproved { entry_id, approved } => {
                FileAdminService::set_entry_approved(db.db(), &entry_id, approved)?;
                if approved {
                    "File entry approved"
                } else {
                    "File entry unapproved"
                }
            }
        }
        .to_string();
        self.status = Some(message.clone());
        self.refresh(db);
        Ok(Some(message))
    }

    pub fn cancel_pending_action(&mut self) {
        self.pending_action = None;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Min(8),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_summary(frame, layout[0]);
        match self.view {
            FilesView::Areas => self.render_areas(frame, layout[1]),
            FilesView::Entries => self.render_entries(frame, layout[1]),
            FilesView::Transfers => self.render_transfers(frame, layout[1]),
        }
        let hints = match self.view {
            FilesView::Areas => {
                "1-3 Tabs | Up/Down Move | D Enable/Disable | F5 Refresh | Esc Back"
            }
            FilesView::Entries => {
                "1-3 Tabs | Up/Down Move | A Approve/Unapprove | F5 Refresh | Esc Back"
            }
            FilesView::Transfers => "1-3 Tabs | Up/Down Move | F5 Refresh | Esc Back",
        };
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(layout[2], frame.buffer_mut());
    }

    fn render_summary(&self, frame: &mut Frame, area: Rect) {
        let tabs = [
            (FilesView::Areas, "1"),
            (FilesView::Entries, "2"),
            (FilesView::Transfers, "3"),
        ]
        .into_iter()
        .map(|(view, number)| {
            let text = format!(" {number} {} ", view.label());
            if view == self.view {
                Span::styled(text, self.theme.selected_style())
            } else {
                Span::styled(text, self.theme.muted_style())
            }
        })
        .collect::<Vec<_>>();

        let summary = if let Some(error) = &self.error {
            format!("File admin unavailable: {error}")
        } else if let Some(dashboard) = &self.dashboard {
            format!(
                "areas={} enabled={} files={} pending={} transfers={} failed={}",
                dashboard.areas.len(),
                dashboard.enabled_areas,
                dashboard.entries.len(),
                dashboard.pending_entries,
                dashboard.transfers.len(),
                dashboard.failed_transfers
            )
        } else {
            "File admin unavailable: database is not open".to_string()
        };
        let mut lines = vec![Line::from(tabs), Line::from(summary)];
        if let Some(status) = &self.status {
            lines.push(Line::from(status.as_str()));
        }
        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" Files ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }

    fn render_areas(&self, frame: &mut Frame, area: Rect) {
        let areas = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.areas.as_slice())
            .unwrap_or(&[]);
        let rows = areas
            .iter()
            .map(|area| {
                Row::new(vec![
                    area.key.clone(),
                    area.name.clone(),
                    if area.enabled { "yes" } else { "no" }.to_string(),
                    area.read_security_level.to_string(),
                    area.download_security_level.to_string(),
                    area.upload_security_level.to_string(),
                    area.root_path.clone(),
                ])
                .style(if area.enabled {
                    self.theme.normal_style()
                } else {
                    self.theme.muted_style()
                })
            })
            .collect::<Vec<_>>();
        let mut state = self.area_state;
        render_table(
            frame,
            area,
            &mut state,
            TableSpec {
                rows,
                header: vec!["Key", "Name", "On", "Read", "Down", "Up", "Root"],
                widths: [
                    Constraint::Length(12),
                    Constraint::Length(22),
                    Constraint::Length(4),
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Min(18),
                ],
                title: " File Areas ",
            },
            &self.theme,
        );
    }

    fn render_entries(&self, frame: &mut Frame, area: Rect) {
        let entries = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.entries.as_slice())
            .unwrap_or(&[]);
        let rows = entries
            .iter()
            .map(|entry| {
                Row::new(vec![
                    if entry.approved { "yes" } else { "no" }.to_string(),
                    entry.display_name.clone(),
                    entry.size_bytes.to_string(),
                    entry.download_count.to_string(),
                    entry.original_name.clone().unwrap_or_default(),
                    entry.description.clone(),
                ])
                .style(if entry.approved {
                    self.theme.normal_style()
                } else {
                    self.theme.warning_style()
                })
            })
            .collect::<Vec<_>>();
        let mut state = self.entry_state;
        render_table(
            frame,
            area,
            &mut state,
            TableSpec {
                rows,
                header: vec![
                    "Approved",
                    "Name",
                    "Bytes",
                    "Down",
                    "Original",
                    "Description",
                ],
                widths: [
                    Constraint::Length(10),
                    Constraint::Length(24),
                    Constraint::Length(10),
                    Constraint::Length(7),
                    Constraint::Length(18),
                    Constraint::Min(18),
                ],
                title: " File Entries ",
            },
            &self.theme,
        );
    }

    fn render_transfers(&self, frame: &mut Frame, area: Rect) {
        let transfers = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.transfers.as_slice())
            .unwrap_or(&[]);
        let rows = transfers
            .iter()
            .map(|transfer| {
                Row::new(vec![
                    transfer.started_at.clone(),
                    transfer.direction.clone(),
                    transfer.protocol.clone(),
                    transfer.outcome.clone(),
                    transfer.node_number.to_string(),
                    transfer.transferred_payload_bytes.to_string(),
                    transfer.error_message.clone().unwrap_or_default(),
                ])
                .style(if transfer.outcome == "success" {
                    self.theme.normal_style()
                } else {
                    self.theme.warning_style()
                })
            })
            .collect::<Vec<_>>();
        let mut state = self.transfer_state;
        render_table(
            frame,
            area,
            &mut state,
            TableSpec {
                rows,
                header: vec![
                    "Started", "Dir", "Proto", "Outcome", "Node", "Bytes", "Error",
                ],
                widths: [
                    Constraint::Length(20),
                    Constraint::Length(8),
                    Constraint::Length(8),
                    Constraint::Length(12),
                    Constraint::Length(6),
                    Constraint::Length(10),
                    Constraint::Min(18),
                ],
                title: " Transfer History ",
            },
            &self.theme,
        );
    }

    fn move_selection(&mut self, down: bool) {
        let count = match self.view {
            FilesView::Areas => self
                .dashboard
                .as_ref()
                .map(|dashboard| dashboard.areas.len())
                .unwrap_or(0),
            FilesView::Entries => self
                .dashboard
                .as_ref()
                .map(|dashboard| dashboard.entries.len())
                .unwrap_or(0),
            FilesView::Transfers => self
                .dashboard
                .as_ref()
                .map(|dashboard| dashboard.transfers.len())
                .unwrap_or(0),
        };
        let state = match self.view {
            FilesView::Areas => &mut self.area_state,
            FilesView::Entries => &mut self.entry_state,
            FilesView::Transfers => &mut self.transfer_state,
        };
        move_table_state(state, count, down);
    }

    fn selected_area_status(&self) -> Option<(String, bool, String)> {
        self.dashboard.as_ref().and_then(|dashboard| {
            self.area_state
                .selected()
                .and_then(|index| dashboard.areas.get(index))
                .map(|area| (area.id.clone(), area.enabled, area.key.clone()))
        })
    }

    fn selected_entry_status(&self) -> Option<(String, bool, String)> {
        self.dashboard.as_ref().and_then(|dashboard| {
            self.entry_state
                .selected()
                .and_then(|index| dashboard.entries.get(index))
                .map(|entry| (entry.id.clone(), entry.approved, entry.display_name.clone()))
        })
    }
}

struct TableSpec<'a, const N: usize> {
    rows: Vec<Row<'a>>,
    header: Vec<&'a str>,
    widths: [Constraint; N],
    title: &'a str,
}

fn render_table<const N: usize>(
    frame: &mut Frame,
    area: Rect,
    state: &mut TableState,
    spec: TableSpec<'_, N>,
    theme: &Theme,
) {
    ratatui::prelude::StatefulWidget::render(
        Table::new(spec.rows, spec.widths)
            .header(Row::new(spec.header).style(theme.label_style()))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.block_style(true))
                    .title(spec.title)
                    .title_style(theme.title_style()),
            )
            .row_highlight_style(theme.selected_style()),
        area,
        frame.buffer_mut(),
        state,
    );
}

fn select_or_clear(state: &mut TableState, count: usize) {
    if count == 0 {
        state.select(None);
    } else if state.selected().is_none_or(|index| index >= count) {
        state.select(Some(0));
    }
}

fn move_table_state(state: &mut TableState, count: usize, down: bool) {
    if count == 0 {
        state.select(None);
        return;
    }
    let current = state.selected().unwrap_or(0);
    let next = if down {
        (current + 1).min(count.saturating_sub(1))
    } else {
        current.saturating_sub(1)
    };
    state.select(Some(next));
}

fn confirm(title: &str, message: &str) -> UiAction {
    UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
        title: title.to_string(),
        message: message.to_string(),
        detail: None,
        confirm_label: "Confirm".to_string(),
        cancel_label: "Cancel".to_string(),
    }))
}
