use crossterm::event::{Event as CrosstermEvent, KeyEvent};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
    ControlSocketData(ControlSocketUpdate),
    Quit,
}

#[derive(Debug)]
pub enum ControlSocketUpdate {
    NodeStatuses(Vec<NodeStatusSnapshot>),
    StatusUpdate(StatusSnapshot),
    ConnectionLost,
    ConnectionRestored,
}

#[derive(Debug, Clone)]
pub struct NodeStatusSnapshot {
    pub node_number: u16,
    pub state: String,
    pub user_alias: Option<String>,
    pub remote_address: Option<String>,
    pub connected_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub heartbeat_age_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct StatusSnapshot {
    pub board_name: String,
    pub uptime_seconds: u64,
    pub node_count: u16,
    pub active_nodes: usize,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    _tx: mpsc::UnboundedSender<AppEvent>,
}

impl EventHandler {
    pub fn new(tick_rate: std::time::Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let _tx = tx.clone();

        // Spawn crossterm event reader
        tokio::spawn(async move {
            loop {
                if crossterm::event::poll(tick_rate).unwrap_or(false) {
                    match crossterm::event::read() {
                        Ok(CrosstermEvent::Key(key)) if tx.send(AppEvent::Key(key)).is_err() => {
                            break;
                        }
                        Ok(CrosstermEvent::Resize(w, h))
                            if tx.send(AppEvent::Resize(w, h)).is_err() =>
                        {
                            break;
                        }
                        _ => {}
                    }
                } else if tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });

        Self { rx, _tx }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }

    pub fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self._tx.clone()
    }
}
