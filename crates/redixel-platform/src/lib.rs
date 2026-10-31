//! OS abstraction layer for the Redixel engine: window management and input handling over `winit`.

pub mod input;
pub mod window;

pub use input::InputManager;
pub use window::{WindowConfig, WindowManager};
