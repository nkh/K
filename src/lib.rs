pub mod cli;
pub mod config;
pub mod daemon;
pub mod handles;
pub mod hooks;
pub mod instance;
pub mod interactive;
pub mod ipc;
pub mod logging;
pub mod process;
pub mod trace;
pub mod vtty;

#[cfg(feature = "vrw")]
pub mod web;

// Re-export commonly used types for ergonomic imports.
pub use config::schema::Config;
pub use process::manager::CommandManager;
pub use vtty::emulator::VttyEmulator;
pub use vtty::buffer::Buffer;
pub use ipc::protocol::{ControlCommand, ControlResponse};