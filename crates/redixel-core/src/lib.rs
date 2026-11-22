//! Core traits and types shared by every Redixel crate: `Game`, `GameContext`, input actions, the `NetworkManager` and
//! `AudioManager` traits and `RedixelError`.

pub mod audio;
pub mod error;
pub mod game;
pub mod input;
pub mod net;
pub mod texture;

pub use audio::{AudioChannel, AudioConfig, AudioManager, MusicOptions, NoOpAudio, SoundId};
pub use error::RedixelError;
pub use game::{Game, GameContext, InputBind, InputQuery};
pub use input::{InputAction, InputSource, KeyCode, KeyState, MouseButton};
pub use net::{ClientId, NetworkChannel, NetworkEvent, NetworkManager, NoOpNetwork, SERVER_ID, SequenceBuffer};
pub use texture::TextureId;
