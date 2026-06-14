pub mod error;
pub mod handle;
pub mod manager;
pub mod pty;
pub mod spawner;

pub use error::ProcessError;
pub use pty::{ChildProcess, PtyMaster, PtyPair, PtySize, PtySlave};
