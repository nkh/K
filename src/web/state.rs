use std::sync::Arc;
use tokio::sync::broadcast;

use crate::process::manager::CommandManager;

/// Shared application state, threaded through Axum's state extractor.
/// All dependencies are explicit — no globals, no unsafe, no OnceLock hacks.
#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<CommandManager>,
    pub shutdown_tx: broadcast::Sender<()>,
}

impl AppState {
    pub fn new(manager: Arc<CommandManager>, shutdown_tx: broadcast::Sender<()>) -> Self {
        Self { manager, shutdown_tx }
    }
}
