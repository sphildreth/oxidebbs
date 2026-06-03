use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::door_service::DoorAdminService;
use crate::theme::Theme;
use oxidebbs_db::{DoorDefinitionRecord, DoorRunRecord, OxideDb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorView {
    List,
    Detail,
    RunHistory,
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
}

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
        _readonly: bool,
    ) -> UiAction {
        match self.view {
            DoorView::Detail => return self.handle_detail_event(event),
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
                    self.filter.clear();
                    // In a real TUI this would open a filter modal; for now toggle filter reset
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

    fn handle_detail_event(&mut self, event: UiEvent) -> UiAction {
        match event {
            UiEvent::Key(key) if key.code == KeyCode::Esc => {
                self.view = DoorView::List;
                self.detail_door = None;
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

        let hints = "↑↓ Move | Enter Detail | H History | F Filter | Esc Back";
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

        Paragraph::new(lines)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" Door Detail ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
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
