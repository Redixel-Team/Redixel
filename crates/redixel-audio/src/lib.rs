//! Native and web audio backends for the Redixel engine.
//!
//! `redixel-core::audio` defines the [`AudioManager`] trait and its handle
//! types with no dependency on a real backend, so `ctx.play_sound`/
//! `ctx.play_music` always compile. This crate provides the two concrete
//! implementations — `cpal` + `symphonia` natively, the Web Audio API on
//! wasm — selected by [`build`], mirroring `redixel-net`.

mod pending;
mod registry;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeAudioManager;
#[cfg(target_arch = "wasm32")]
pub use wasm::WasmAudioManager;

use redixel_core::{AudioConfig, AudioManager, NoOpAudio};

/// The shortest fade either backend applies when it stops or replaces a
/// track, so a cut lands on silence instead of clicking.
const MIN_FADE_SECS: f32 = 0.005;

/// Builds the [`AudioManager`] for `config` on the current platform.
///
/// Never fails fatally: if the platform's audio output (native) or the Web
/// Audio API (wasm) cannot be initialised, this logs an error and degrades to
/// a [`NoOpAudio`] so the game keeps running — the same contract
/// `redixel_net::build` has when a transport fails to start.
pub fn build(config: &AudioConfig) -> Box<dyn AudioManager> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match NativeAudioManager::new(config) {
            Ok(manager) => Box::new(manager),
            Err(e) => {
                log::error!("Failed to start native audio output: {e}. Audio will be silent.");
                Box::new(NoOpAudio::new(*config))
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        match WasmAudioManager::new(config) {
            Ok(manager) => Box::new(manager),
            Err(e) => {
                log::error!("Failed to create the Web Audio context: {e:?}. Audio will be silent.");
                Box::new(NoOpAudio::new(*config))
            }
        }
    }
}
