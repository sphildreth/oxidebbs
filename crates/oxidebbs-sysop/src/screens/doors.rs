use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::SysopError;
use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::door_service::DoorAdminService;
use crate::theme::Theme;
use crate::widgets::modal::{ConfirmModal, FormField, FormModal, ModalKind};
use oxidebbs_db::{DoorDefinitionRecord, DoorRunRecord, OxideDb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorView {
    List,
    Detail,
    RunHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorEditMode {
    Editing { field_index: usize },
    None,
}

pub struct DoorsScreen {
    pub theme: Theme,
    pub view: DoorView,
    pub doors: Vec<DoorDefinitionRecord>,
    pub runs: Vec<DoorRunRecord>,
    pub table_state: TableState,
    pub run_table_state: TableState,
    pub filter: String,
    pub detail_door: Option<String>,
    pub pending_action: Option<DoorPendingAction>,
    pub door_edit: Option<DoorDefinitionRecord>,
    pub door_edit_mode: DoorEditMode,
    pub door_edit_is_new: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DoorPendingAction {
    SetEnabled {
        door_id: String,
        door_key: String,
        enabled: bool,
    },
    SaveDoor {
        door: DoorDefinitionRecord,
        is_new: bool,
    },
}

const DOOR_EDIT_FIELDS: &[(&str, usize)] = &[
    ("key", 14),
    ("name", 30),
    ("runner", 14),
    ("working_dir", 24),
    ("command", 20),
    ("drop_file", 14),
    ("exclusive", 10),
    ("time_limit_minutes", 18),
    ("enabled", 8),
    ("min_security_level", 18),
];

impl DoorsScreen {
    pub fn new(theme: Theme) -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        let mut run_table_state = TableState::default();
        run_table_state.select(Some(0));
        Self {
            theme,
            view: DoorView::List,
            doors: Vec::new(),
            runs: Vec::new(),
            table_state,
            run_table_state,
            filter: String::new(),
            detail_door: None,
            pending_action: None,
            door_edit: None,
            door_edit_mode: DoorEditMode::None,
            door_edit_is_new: false,
        }
    }

    pub fn refresh(&mut self, db: &OxideDb) {
        if let Ok(doors) = DoorAdminService::list(db.db()) {
            self.doors = doors;
        }
        if let Ok(runs) = DoorAdminService::list_runs(db.db(), 50) {
            self.runs = runs;
        }
    }

    fn filtered_doors(&self) -> Vec<&DoorDefinitionRecord> {
        let mut doors: Vec<&DoorDefinitionRecord> = self.doors.iter().collect();
        if !self.filter.is_empty() {
            let f = self.filter.to_ascii_lowercase();
            doors.retain(|d| {
                d.key.to_ascii_lowercase().contains(&f) || d.name.to_ascii_lowercase().contains(&f)
            });
        }
        doors
    }

    fn selected_door_id(&self) -> Option<String> {
        let doors = self.filtered_doors();
        self.table_state
            .selected()
            .and_then(|idx| doors.get(idx))
            .map(|d| d.id.clone())
    }

    fn door_runs(&self, door_id: &str) -> Vec<&DoorRunRecord> {
        self.runs.iter().filter(|r| r.door_id == door_id).collect()
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<OxideDb>,
        readonly: bool,
    ) -> UiAction {
        if self.door_edit.is_some() {
            return self.handle_door_edit_event(event, readonly);
        }

        match self.view {
            DoorView::Detail => return self.handle_detail_event(event, readonly),
            DoorView::RunHistory => return self.handle_run_event(event),
            DoorView::List => {}
        }

        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Up => {
                    let current = self.table_state.selected().unwrap_or(0);
                    if current > 0 {
                        self.table_state.select(Some(current - 1));
                    }
                }
                KeyCode::Down => {
                    let current = self.table_state.selected().unwrap_or(0);
                    let max = self.filtered_doors().len().saturating_sub(1);
                    self.table_state.select(Some((current + 1).min(max)));
                }
                KeyCode::Enter => {
                    if let Some(id) = self.selected_door_id() {
                        self.detail_door = Some(id);
                        self.view = DoorView::Detail;
                    }
                }
                KeyCode::Char('h') => {
                    self.view = DoorView::RunHistory;
                }
                KeyCode::Char('f') | KeyCode::Char('/') => {
                    return UiAction::OpenModal(ModalKind::Form(FormModal {
                        title: "Filter Doors".to_string(),
                        fields: vec![FormField {
                            label: "Filter".to_string(),
                            value: self.filter.clone(),
                            is_password: false,
                        }],
                        active_field: 0,
                    }));
                }
                KeyCode::Char('d') if !readonly => {
                    if let Some(id) = self.selected_door_id()
                        && let Some(door) = self.doors.iter().find(|door| door.id == id)
                    {
                        let enabled = !door.enabled;
                        let (title, verb) = if enabled {
                            ("Enable Door", "Enable")
                        } else {
                            ("Disable Door", "Disable")
                        };
                        self.pending_action = Some(DoorPendingAction::SetEnabled {
                            door_id: door.id.clone(),
                            door_key: door.key.clone(),
                            enabled,
                        });
                        return UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
                            title: title.to_string(),
                            message: format!("{verb} door {}?", door.key),
                            detail: Some(door.name.clone()),
                            confirm_label: verb.to_string(),
                            cancel_label: "Cancel".to_string(),
                        }));
                    }
                }
                KeyCode::Char('a') if !readonly => {
                    self.door_edit = Some(DoorDefinitionRecord {
                        id: String::new(),
                        key: String::new(),
                        name: String::new(),
                        runner: String::new(),
                        working_dir: String::new(),
                        command: String::new(),
                        drop_file: "door.sys".to_string(),
                        exclusive: false,
                        time_limit_minutes: 30,
                        enabled: true,
                        min_security_level: 0,
                    });
                    self.door_edit_mode = DoorEditMode::Editing { field_index: 0 };
                    self.door_edit_is_new = true;
                }
                KeyCode::Char('e') if !readonly => {
                    if let Some(id) = self.selected_door_id()
                        && let Some(door) = self.doors.iter().find(|d| d.id == id)
                    {
                        self.door_edit = Some(door.clone());
                        self.door_edit_mode = DoorEditMode::Editing { field_index: 0 };
                        self.door_edit_is_new = false;
                    }
                }
                KeyCode::Esc => {
                    return UiAction::Navigate(ScreenId::Dashboard);
                }
                KeyCode::PageUp => {
                    let current = self.table_state.selected().unwrap_or(0);
                    self.table_state.select(Some(current.saturating_sub(5)));
                }
                KeyCode::PageDown => {
                    let current = self.table_state.selected().unwrap_or(0);
                    let max = self.filtered_doors().len().saturating_sub(1);
                    self.table_state.select(Some((current + 5).min(max)));
                }
                KeyCode::Home => {
                    self.table_state.select(Some(0));
                }
                KeyCode::End => {
                    let max = self.filtered_doors().len().saturating_sub(1);
                    self.table_state.select(Some(max));
                }
                _ => {}
            },
            UiEvent::Refresh => {
                return UiAction::Refresh;
            }
            _ => {}
        }
        UiAction::None
    }

    fn handle_door_edit_event(&mut self, event: UiEvent, _readonly: bool) -> UiAction {
        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Esc => {
                    self.door_edit = None;
                    self.door_edit_mode = DoorEditMode::None;
                }
                KeyCode::Up => {
                    if let DoorEditMode::Editing {
                        ref mut field_index,
                    } = self.door_edit_mode
                        && *field_index > 0
                    {
                        *field_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if let DoorEditMode::Editing {
                        ref mut field_index,
                    } = self.door_edit_mode
                        && *field_index + 1 < DOOR_EDIT_FIELDS.len()
                    {
                        *field_index += 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(door) = self.door_edit.take() {
                        let is_new = self.door_edit_is_new;
                        self.door_edit_mode = DoorEditMode::None;
                        self.pending_action = Some(DoorPendingAction::SaveDoor { door, is_new });
                        return UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
                            title: if is_new {
                                "Add Door".to_string()
                            } else {
                                "Update Door".to_string()
                            },
                            message: if is_new {
                                "Add this door definition?".to_string()
                            } else {
                                "Save changes to this door?".to_string()
                            },
                            detail: None,
                            confirm_label: "Save".to_string(),
                            cancel_label: "Cancel".to_string(),
                        }));
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(ref mut door) = self.door_edit
                        && let DoorEditMode::Editing { field_index } = self.door_edit_mode
                    {
                        set_field_char(door, field_index, c);
                    }
                }
                KeyCode::Backspace => {
                    if let Some(ref mut door) = self.door_edit
                        && let DoorEditMode::Editing { field_index } = self.door_edit_mode
                    {
                        pop_field_char(door, field_index);
                    }
                }
                KeyCode::Tab => {
                    if let DoorEditMode::Editing {
                        ref mut field_index,
                    } = self.door_edit_mode
                    {
                        *field_index = (*field_index + 1) % DOOR_EDIT_FIELDS.len();
                    }
                }
                _ => {}
            },
            UiEvent::Cancel => {
                self.door_edit = None;
                self.door_edit_mode = DoorEditMode::None;
            }
            _ => {}
        }
        UiAction::None
    }

    pub fn confirm_pending_action(&mut self, db: &Option<OxideDb>) -> Result<(), SysopError> {
        let Some(action) = self.pending_action.take() else {
            return Ok(());
        };
        let Some(db) = db else {
            return Err(SysopError::Message(
                "database is unavailable for door action".to_string(),
            ));
        };

        match action {
            DoorPendingAction::SetEnabled {
                door_id,
                door_key,
                enabled,
            } => DoorAdminService::set_enabled(db.db(), &door_id, &door_key, enabled),
            DoorPendingAction::SaveDoor { door, is_new } => {
                if is_new {
                    DoorAdminService::insert_door(db.db(), &door)
                } else {
                    DoorAdminService::update_door(db.db(), &door)
                }
            }
        }
    }

    pub fn cancel_pending_action(&mut self) {
        self.pending_action = None;
    }

    fn handle_detail_event(&mut self, event: UiEvent, readonly: bool) -> UiAction {
        match event {
            UiEvent::Key(key) if key.code == KeyCode::Esc => {
                self.view = DoorView::List;
                self.detail_door = None;
            }
            UiEvent::Key(key) if key.code == KeyCode::Char('d') && !readonly => {
                if let Some(ref id) = self.detail_door
                    && let Some(door) = self.doors.iter().find(|door| &door.id == id)
                {
                    let enabled = !door.enabled;
                    let (title, verb) = if enabled {
                        ("Enable Door", "Enable")
                    } else {
                        ("Disable Door", "Disable")
                    };
                    self.pending_action = Some(DoorPendingAction::SetEnabled {
                        door_id: door.id.clone(),
                        door_key: door.key.clone(),
                        enabled,
                    });
                    return UiAction::OpenModal(ModalKind::Confirm(ConfirmModal {
                        title: title.to_string(),
                        message: format!("{verb} door {}?", door.key),
                        detail: Some(door.name.clone()),
                        confirm_label: verb.to_string(),
                        cancel_label: "Cancel".to_string(),
                    }));
                }
            }
            UiEvent::Key(key) if key.code == KeyCode::Char('e') && !readonly => {
                if let Some(ref id) = self.detail_door
                    && let Some(door) = self.doors.iter().find(|d| &d.id == id)
                {
                    self.door_edit = Some(door.clone());
                    self.door_edit_mode = DoorEditMode::Editing { field_index: 0 };
                    self.door_edit_is_new = false;
                }
            }
            UiEvent::Cancel => {
                self.view = DoorView::List;
                self.detail_door = None;
            }
            _ => {}
        }
        UiAction::None
    }

    fn handle_run_event(&mut self, event: UiEvent) -> UiAction {
        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Up => {
                    let current = self.run_table_state.selected().unwrap_or(0);
                    if current > 0 {
                        self.run_table_state.select(Some(current - 1));
                    }
                }
                KeyCode::Down => {
                    let current = self.run_table_state.selected().unwrap_or(0);
                    let max = self.runs.len().saturating_sub(1);
                    self.run_table_state.select(Some((current + 1).min(max)));
                }
                KeyCode::Esc => {
                    self.view = DoorView::List;
                }
                _ => {}
            },
            UiEvent::Cancel => {
                self.view = DoorView::List;
            }
            _ => {}
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if self.door_edit.is_some() {
            self.render_door_edit(frame, area);
            return;
        }

        match self.view {
            DoorView::Detail => {
                if let Some(ref id) = self.detail_door {
                    self.render_detail(frame, area, id);
                } else {
                    self.render_list(frame, area);
                }
            }
            DoorView::RunHistory => self.render_run_history(frame, area),
            DoorView::List => self.render_list(frame, area),
        }
    }

    fn render_door_edit(&self, frame: &mut Frame, area: Rect) {
        let title = if self.door_edit_is_new {
            " Add Door "
        } else {
            " Edit Door "
        };

        let field_index = match self.door_edit_mode {
            DoorEditMode::Editing { field_index } => field_index,
            DoorEditMode::None => 0,
        };

        let mut lines = Vec::new();
        if let Some(ref door) = self.door_edit {
            for (i, (field_name, _width)) in DOOR_EDIT_FIELDS.iter().enumerate() {
                let marker = if i == field_index { "> " } else { "  " };
                let value = match *field_name {
                    "key" => &door.key,
                    "name" => &door.name,
                    "runner" => &door.runner,
                    "working_dir" => &door.working_dir,
                    "command" => &door.command,
                    "drop_file" => &door.drop_file,
                    "exclusive" => {
                        let v = if door.exclusive { "Yes" } else { "No" };
                        lines.push(format!("{}{}: {}", marker, field_name, v));
                        continue;
                    }
                    "enabled" => {
                        let v = if door.enabled { "Yes" } else { "No" };
                        lines.push(format!("{}{}: {}", marker, field_name, v));
                        continue;
                    }
                    "time_limit_minutes" => {
                        lines.push(format!(
                            "{}{}: {}",
                            marker, field_name, door.time_limit_minutes
                        ));
                        continue;
                    }
                    "min_security_level" => {
                        lines.push(format!(
                            "{}{}: {}",
                            marker, field_name, door.min_security_level
                        ));
                        continue;
                    }
                    _ => continue,
                };
                lines.push(format!("{}{}: {}", marker, field_name, value));
            }
        }

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(3)])
            .split(area);

        Paragraph::new(lines.join("\n"))
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(title)
                    .title_style(self.theme.title_style()),
            )
            .render(main_layout[0], frame.buffer_mut());

        let hints = "Enter Save | Esc Cancel | Tab Next Field | ↑↓ Navigate Fields";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[1], frame.buffer_mut());
    }

    fn render_list(&self, frame: &mut Frame, area: Rect) {
        let doors = self.filtered_doors();
        let toolbar_text = format!(
            "Doors: {} total | Filter: {}",
            self.doors.len(),
            if self.filter.is_empty() {
                "all".to_string()
            } else {
                self.filter.clone()
            },
        );
        let toolbar = Paragraph::new(toolbar_text).style(self.theme.label_style());

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        toolbar.render(main_layout[0], frame.buffer_mut());

        let header = Row::new(vec![
            "Key", "Name", "Runner", "Dir", "Cmd", "Drop", "T/m", "En",
        ])
        .style(self.theme.label_style())
        .height(1);

        let rows: Vec<Row> = doors
            .iter()
            .map(|d| {
                let style = if d.enabled {
                    self.theme.normal_style()
                } else {
                    self.theme.muted_style()
                };
                Row::new(vec![
                    d.key.clone(),
                    d.name.clone(),
                    d.runner.clone(),
                    d.working_dir.clone(),
                    d.command.clone(),
                    d.drop_file.clone(),
                    d.time_limit_minutes.to_string(),
                    if d.enabled { "Yes" } else { "No" }.to_string(),
                ])
                .style(style)
            })
            .collect();

        let widths = [
            Constraint::Length(8),
            Constraint::Length(18),
            Constraint::Length(10),
            Constraint::Length(16),
            Constraint::Length(14),
            Constraint::Length(10),
            Constraint::Length(4),
            Constraint::Length(3),
        ];

        let mut table_state = self.table_state;
        ratatui::prelude::StatefulWidget::render(
            Table::new(rows, widths)
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.block_style(true))
                        .title(" Doors ")
                        .title_style(self.theme.title_style()),
                )
                .row_highlight_style(self.theme.selected_style()),
            main_layout[1],
            frame.buffer_mut(),
            &mut table_state,
        );

        let hints = "↑↓ Move | Enter Detail | A Add | E Edit | H History | F Filter | D Enable/Disable | Esc Back";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[2], frame.buffer_mut());
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect, door_id: &str) {
        let door = self.doors.iter().find(|d| d.id == door_id);
        let mut lines = vec![Line::from("Door Detail")];
        if let Some(d) = door {
            lines.push(Line::from(vec![
                Span::styled("Key: ", self.theme.label_style()),
                Span::styled(&d.key, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Name: ", self.theme.label_style()),
                Span::styled(&d.name, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Runner: ", self.theme.label_style()),
                Span::styled(&d.runner, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Working Dir: ", self.theme.label_style()),
                Span::styled(&d.working_dir, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Command: ", self.theme.label_style()),
                Span::styled(&d.command, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Drop File: ", self.theme.label_style()),
                Span::styled(&d.drop_file, self.theme.normal_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Exclusive: ", self.theme.label_style()),
                Span::styled(
                    if d.exclusive { "Yes" } else { "No" },
                    self.theme.normal_style(),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Time Limit: ", self.theme.label_style()),
                Span::styled(
                    format!("{} min", d.time_limit_minutes),
                    self.theme.normal_style(),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Enabled: ", self.theme.label_style()),
                Span::styled(
                    if d.enabled { "Yes" } else { "No" },
                    self.theme.normal_style(),
                ),
            ]));
            let run_count = self.door_runs(door_id).len();
            lines.push(Line::from(vec![
                Span::styled("Recent Runs: ", self.theme.label_style()),
                Span::styled(run_count.to_string(), self.theme.normal_style()),
            ]));
        } else {
            lines.push(Line::from("Door not found."));
        }

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(3)])
            .split(area);

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" Door Detail ")
                    .title_style(self.theme.title_style()),
            )
            .render(main_layout[0], frame.buffer_mut());

        let hints = "Esc Back | E Edit | D Enable/Disable";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[1], frame.buffer_mut());
    }

    fn render_run_history(&self, frame: &mut Frame, area: Rect) {
        let toolbar_text = format!("Run History: {} runs", self.runs.len());
        let toolbar = Paragraph::new(toolbar_text).style(self.theme.label_style());

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        toolbar.render(main_layout[0], frame.buffer_mut());

        let header = Row::new(vec![
            "Door ID", "User ID", "Node", "Started", "Ended", "Exit", "Timeout",
        ])
        .style(self.theme.label_style())
        .height(1);

        let rows: Vec<Row> = self
            .runs
            .iter()
            .map(|r| {
                Row::new(vec![
                    r.door_id.clone(),
                    r.user_id.clone(),
                    r.node_number.to_string(),
                    r.started_at.clone(),
                    r.ended_at.clone().unwrap_or("--".to_string()),
                    r.exit_code
                        .map(|e| e.to_string())
                        .unwrap_or("--".to_string()),
                    if r.timed_out { "Yes" } else { "No" }.to_string(),
                ])
                .style(self.theme.normal_style())
            })
            .collect();

        let widths = [
            Constraint::Length(20),
            Constraint::Length(20),
            Constraint::Length(5),
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Length(5),
            Constraint::Length(8),
        ];

        let mut run_table_state = self.run_table_state;
        ratatui::prelude::StatefulWidget::render(
            Table::new(rows, widths)
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.block_style(true))
                        .title(" Run History ")
                        .title_style(self.theme.title_style()),
                )
                .row_highlight_style(self.theme.selected_style()),
            main_layout[1],
            frame.buffer_mut(),
            &mut run_table_state,
        );

        let hints = "↑↓ Move | Esc Back";
        Paragraph::new(hints)
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(main_layout[2], frame.buffer_mut());
    }
}

fn set_field_char(door: &mut DoorDefinitionRecord, field_index: usize, c: char) {
    match DOOR_EDIT_FIELDS[field_index].0 {
        "key" => door.key.push(c),
        "name" => door.name.push(c),
        "runner" => door.runner.push(c),
        "working_dir" => door.working_dir.push(c),
        "command" => door.command.push(c),
        "drop_file" => door.drop_file.push(c),
        "exclusive" => {
            if c == 'y' || c == 'Y' {
                door.exclusive = true;
            } else if c == 'n' || c == 'N' {
                door.exclusive = false;
            }
        }
        "time_limit_minutes" if c.is_ascii_digit() => {
            door.time_limit_minutes = door
                .time_limit_minutes
                .saturating_mul(10)
                .saturating_add(i64::from(c.to_digit(10).unwrap_or(0)));
        }
        "time_limit_minutes" => {}
        "enabled" => {
            if c == 'y' || c == 'Y' {
                door.enabled = true;
            } else if c == 'n' || c == 'N' {
                door.enabled = false;
            }
        }
        "min_security_level" if c.is_ascii_digit() => {
            door.min_security_level = door
                .min_security_level
                .saturating_mul(10)
                .saturating_add(i64::from(c.to_digit(10).unwrap_or(0)));
        }
        "min_security_level" => {}
        _ => {}
    }
}

fn pop_field_char(door: &mut DoorDefinitionRecord, field_index: usize) {
    match DOOR_EDIT_FIELDS[field_index].0 {
        "key" => {
            door.key.pop();
        }
        "name" => {
            door.name.pop();
        }
        "runner" => {
            door.runner.pop();
        }
        "working_dir" => {
            door.working_dir.pop();
        }
        "command" => {
            door.command.pop();
        }
        "drop_file" => {
            door.drop_file.pop();
        }
        "exclusive" => {
            door.exclusive = false;
        }
        "enabled" => {
            door.enabled = false;
        }
        "time_limit_minutes" => {
            door.time_limit_minutes /= 10;
        }
        "min_security_level" => {
            door.min_security_level /= 10;
        }
        _ => {}
    }
}
