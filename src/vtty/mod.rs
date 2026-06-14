pub mod buffer;
pub mod cell;
pub mod color;
pub mod display;
pub mod emulator;
pub mod rate_limiter;
pub mod renderer;
pub mod sink;

// Re-export commonly used types
pub use emulator::CursorStyle;