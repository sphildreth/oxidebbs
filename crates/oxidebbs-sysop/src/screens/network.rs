use crossterm::event::KeyCode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::input::{ScreenId, UiEvent};
use crate::screens::common::UiAction;
use crate::services::network_service::{NetworkAdminService, NetworkDashboard};
use crate::theme::Theme;

pub struct NetworkScreen {
    pub theme: Theme,
    pub dashboard: Option<NetworkDashboard>,
    pub table_state: TableState,
    pub error: Option<String>,
}

impl NetworkScreen {
    pub fn new(theme: Theme) -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            theme,
            dashboard: None,
            table_state,
            error: None,
        }
    }

    pub fn refresh(&mut self, db: &oxidebbs_db::OxideDb) {
        match NetworkAdminService::load(db) {
            Ok(dashboard) => {
                if dashboard.profiles.is_empty() {
                    self.table_state.select(None);
                } else if self.table_state.selected().is_none() {
                    self.table_state.select(Some(0));
                }
                self.dashboard = Some(dashboard);
                self.error = None;
            }
            Err(error) => {
                self.dashboard = None;
                self.error = Some(error.to_string());
                self.table_state.select(None);
            }
        }
    }

    pub fn handle_event(
        &mut self,
        event: UiEvent,
        _db: &Option<oxidebbs_db::OxideDb>,
        _readonly: bool,
    ) -> UiAction {
        if let UiEvent::Key(key) = event {
            match key.code {
                KeyCode::Up => {
                    let current = self.table_state.selected().unwrap_or(0);
                    self.table_state.select(Some(current.saturating_sub(1)));
                }
                KeyCode::Down => {
                    let current = self.table_state.selected().unwrap_or(0);
                    let max = self
                        .dashboard
                        .as_ref()
                        .map(|dashboard| dashboard.profiles.len().saturating_sub(1))
                        .unwrap_or(0);
                    self.table_state.select(Some((current + 1).min(max)));
                }
                KeyCode::Esc => {
                    return UiAction::Navigate(ScreenId::Dashboard);
                }
                _ => {}
            }
        }
        UiAction::None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Min(6),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_summary(frame, layout[0]);
        self.render_profiles(frame, layout[1]);

        Paragraph::new("Read-only network status | F5 Refresh | Esc Back")
            .style(self.theme.muted_style())
            .block(Block::default().borders(Borders::ALL))
            .render(layout[2], frame.buffer_mut());
    }

    fn render_summary(&self, frame: &mut Frame, area: Rect) {
        let text = if let Some(error) = &self.error {
            format!("Network status unavailable: {error}")
        } else if let Some(dashboard) = &self.dashboard {
            let status_counts = if dashboard.packet_status_counts.is_empty() {
                "packet_status=none".to_string()
            } else {
                dashboard
                    .packet_status_counts
                    .iter()
                    .map(|(status, count)| format!("{status}={count}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            format!(
                "profiles={} links={} areas={} packets={} messages={}\nnodelist_entries={} poll_logs={} failed_polls={} duplicate_events={}\n{}",
                dashboard.profiles.len(),
                dashboard.total_links,
                dashboard.total_areas,
                dashboard.total_packets,
                dashboard.total_messages,
                dashboard.total_nodelist_entries,
                dashboard.total_poll_logs,
                dashboard.failed_polls,
                dashboard.duplicate_events,
                status_counts
            )
        } else {
            "Network status unavailable: database is not open".to_string()
        };

        Paragraph::new(text)
            .style(self.theme.normal_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(false))
                    .title(" Network Summary ")
                    .title_style(self.theme.title_style()),
            )
            .render(area, frame.buffer_mut());
    }

    fn render_profiles(&self, frame: &mut Frame, area: Rect) {
        let profiles = self
            .dashboard
            .as_ref()
            .map(|dashboard| dashboard.profiles.as_slice())
            .unwrap_or(&[]);

        let header = Row::new(vec![
            "Key",
            "Adapter",
            "Address",
            "On",
            "Links",
            "Areas",
            "Pkts",
            "Msgs",
            "Nodes",
            "Last Poll",
        ])
        .style(self.theme.label_style())
        .height(1);

        let rows = profiles
            .iter()
            .map(|profile| {
                Row::new(vec![
                    profile.key.clone(),
                    profile.adapter.clone(),
                    profile.address.clone(),
                    if profile.enabled { "yes" } else { "no" }.to_string(),
                    profile.links.to_string(),
                    profile.areas.to_string(),
                    profile.packets.to_string(),
                    profile.messages.to_string(),
                    profile.nodelist_entries.to_string(),
                    profile
                        .last_poll_status
                        .clone()
                        .unwrap_or_else(|| "-".to_string()),
                ])
                .style(self.theme.normal_style())
            })
            .collect::<Vec<_>>();

        let mut table_state = self.table_state;
        ratatui::prelude::StatefulWidget::render(
            Table::new(
                rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(10),
                    Constraint::Length(14),
                    Constraint::Length(4),
                    Constraint::Length(7),
                    Constraint::Length(7),
                    Constraint::Length(7),
                    Constraint::Length(7),
                    Constraint::Length(8),
                    Constraint::Min(10),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.block_style(true))
                    .title(" Network Profiles ")
                    .title_style(self.theme.title_style()),
            )
            .row_highlight_style(self.theme.selected_style()),
            area,
            frame.buffer_mut(),
            &mut table_state,
        );
    }
}
