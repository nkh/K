use tokio::sync::broadcast;
use std::sync::OnceLock;

/// Global shutdown channel for cross-module signaling.
/// Uses OnceLock to avoid unsafe mutable statics.
static SHUTDOWN_TX: OnceLock<broadcast::Sender<()>> = OnceLock::new();

/// Retrieve a reference to the global shutdown sender, if it has been initialized.
pub fn get_shutdown_tx() -> Option<&'static broadcast::Sender<()>> {
    SHUTDOWN_TX.get()
}

/// Initialize the global shutdown channel. Can only be called once.
pub fn set_shutdown_tx(tx: broadcast::Sender<()>) {
    let _ = SHUTDOWN_TX.set(tx);
}
