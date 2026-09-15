use std::sync::RwLockReadGuard;

use winit::event_loop::{ControlFlow, EventLoop};

use redixel_core::{Game, RedixelError};
use redixel_platform::window::WindowConfig;
use redixel_renderer::RendererConfig;
use redixel_runtime::{DEFAULT_TICKRATE, EngineSettings, RawBackend, RawPresentMode, Runtime, RuntimeConfig};

#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

mod entry_point;

#[doc(hidden)]
pub use entry_point::__private;

pub mod prelude {
    pub use redixel_core::{
        ClientId, Game, GameContext, InputAction, InputSource, KeyCode, KeyState, MouseButton, NetworkChannel,
        NetworkEvent, NetworkManager, RedixelError, SERVER_ID, SequenceBuffer, TextureFilter, TextureId,
    };

    pub use redixel_math::{Color, Mat4, Vec2, Vec3};
    pub use redixel_runtime::RuntimeConfig;

    #[cfg(feature = "net")]
    pub use redixel_net::{CertSource, DEFAULT_PROTOCOL_ID, NetConfig, NetMode};

    #[cfg(not(target_arch = "wasm32"))]
    pub use redixel_runtime::{HeadlessRuntime, run_headless};
}

/// Reads `config/config.json` into a [`RuntimeConfig`], falling back to sane
/// defaults for any missing or malformed key.
pub fn build_config() -> RuntimeConfig {
    EngineSettings::load_config_json();

    let settings: RwLockReadGuard<EngineSettings> = EngineSettings::global_read();

    RuntimeConfig::windowed(
        WindowConfig {
            width: settings.get_path("window.width", 1280),
            height: settings.get_path("window.height", 720),
            fullscreen: settings.get_path("window.fullscreen", false),
            title: settings.get_path("app.name", String::from("Redixel")),
        },
        RendererConfig {
            backends: settings.get_path("renderer.backend", RawBackend(0)).into(),
            present_mode: settings.get_path("renderer.present_mode", RawPresentMode(0)).into(),
        },
        settings.get_path("window.target_fps", 60.0),
        settings.get_path("engine.tickrate", DEFAULT_TICKRATE),
    )
}

/// Runs `game` as a single-player desktop app, loading [`RuntimeConfig`] from
/// `config/config.json`.
#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop<G: Game>(game: G) -> Result<(), RedixelError> {
    run_desktop_with(game, build_config())
}

/// Like [`run_desktop`] but with an explicit [`RuntimeConfig`] — use this to run
/// a **windowed client** with networking enabled (set `config.net`).
///
/// Uses `EventLoopExtRunOnDemand`, which lets the event loop return control to
/// the caller and be re-entered — a desktop-only capability not available on
/// iOS or the web (see `run_ios_with` / `run_wasm_with`).
#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
pub fn run_desktop_with<G: Game>(game: G, config: RuntimeConfig) -> Result<(), RedixelError> {
    use winit::event_loop::run_on_demand::EventLoopExtRunOnDemand;

    let mut event_loop: EventLoop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut runtime: Runtime<G> = Runtime::new(game, config);
    event_loop.run_app_on_demand(&mut runtime)?;

    if let Some(e) = runtime.take_error() {
        return Err(e);
    }

    Ok(())
}

/// Runs `game` as a single-player Android app, loading [`RuntimeConfig`] from
/// `config/config.json`.
#[cfg(target_os = "android")]
pub fn run_android<G: Game>(game: G, app: AndroidApp) -> Result<(), RedixelError> {
    run_android_with(game, app, build_config())
}

/// Like [`run_android`] but with an explicit [`RuntimeConfig`] — use this to run
/// a **windowed client** with networking enabled (set `config.net`).
#[cfg(target_os = "android")]
pub fn run_android_with<G: Game>(game: G, app: AndroidApp, config: RuntimeConfig) -> Result<(), RedixelError> {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    let event_loop: EventLoop = EventLoop::builder().with_android_app(app).build()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let runtime_ptr: *mut Runtime<G> = Box::into_raw(Box::new(Runtime::new(game, config)));
    let runtime_ref: &'static mut Runtime<G> = unsafe { &mut *runtime_ptr };
    let run_result: Result<(), winit::error::EventLoopError> = event_loop.run_app(runtime_ref);

    let mut owned_runtime: Box<Runtime<G>> = unsafe { Box::from_raw(runtime_ptr) };
    run_result?;

    if let Some(e) = owned_runtime.take_error() {
        return Err(e);
    }

    Ok(())
}

/// Runs `game` as a single-player iOS app, loading [`RuntimeConfig`] from
/// `config/config.json`. Unlike desktop/Android, this is not reached through
/// the crate's own `fn main()` — `UIApplicationMain` must be called before
/// anything else touches UIKit, so call this from a
/// `#[unsafe(no_mangle)] extern "C" fn` exported for an Xcode project to embed
/// (the `ios_main` that [`entry_point!`] generates).
#[cfg(target_os = "ios")]
pub fn run_ios<G: Game + 'static>(game: G) -> Result<(), RedixelError> {
    run_ios_with(game, build_config())
}

/// Like [`run_ios`] but with an explicit [`RuntimeConfig`] — use this to run a
/// **windowed client** with networking enabled (set `config.net`).
///
/// iOS has no `EventLoopExtRunOnDemand`
/// (desktop-only), so this uses the portable [`EventLoop::run_app`], which
/// dispatches to UIKit's `run_app_never_return` under the hood: on success this
/// call never returns to the caller (the OS owns the run loop until the process
/// is terminated), so — unlike [`run_desktop_with`]/[`run_android_with`] —
/// there is no error to recover afterward.
#[cfg(target_os = "ios")]
pub fn run_ios_with<G: Game + 'static>(game: G, config: RuntimeConfig) -> Result<(), RedixelError> {
    let event_loop: EventLoop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let runtime: Runtime<G> = Runtime::new(game, config);
    event_loop.run_app(runtime)?;

    Ok(())
}

/// Runs `game` as a single-player browser app, loading [`RuntimeConfig`] from
/// `config/config.json`.
#[cfg(target_arch = "wasm32")]
pub fn run_wasm<G: Game + 'static>(game: G) -> Result<(), RedixelError> {
    run_wasm_with(game, build_config())
}

/// Like [`run_wasm`] but with an explicit [`RuntimeConfig`] — use this to run a
/// **windowed client** with networking enabled (set `config.net`).
#[cfg(target_arch = "wasm32")]
pub fn run_wasm_with<G: Game + 'static>(game: G, config: RuntimeConfig) -> Result<(), RedixelError> {
    let event_loop: EventLoop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let runtime: Runtime<G> = Runtime::new(game, config);
    event_loop.run_app(runtime)?;

    Ok(())
}
