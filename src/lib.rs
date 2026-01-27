pub mod config;
pub mod hangul;
pub mod input_engine;
pub mod keycode;
pub mod keystroke;
pub mod status;

/// Compatibility alias for the old korean module
pub use crate::hangul as korean;
