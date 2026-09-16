use std::{
    num::ParseFloatError,
    sync::{
        Arc, RwLockWriteGuard, mpsc,
        mpsc::{Receiver, Sender},
    },
};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

#[cfg(not(feature = "audio"))]
use redixel_core::NoOpAudio;
use redixel_core::{AudioConfig, AudioManager, Game, RedixelError, game::GameContext, net::NetworkManager};
#[cfg(feature = "net")]
use redixel_net::NetConfig;
use redixel_platform::{WindowManager, window::WindowConfig};
use redixel_renderer::{Renderer, RendererConfig};

use crate::{
    context::{AudioCommand, Context, DrawCommand, SoundRequest, TextureRequest},
    settings::EngineSettings,
    simulation::{SimulationCore, StepFlow},
    time::TimeManager,
};

pub const DEFAULT_TICKRATE: f64 = 60.0;

#[derive(Clone)]
pub struct RuntimeConfig {
    pub window: WindowConfig,
    pub renderer: RendererConfig,
    pub audio: AudioConfig,
    pub target_fps: f64,
    /// Fixed-update rate in Hz driving `on_fixed_update` (physics/networking).
    pub tickrate: f64,
    /// Networking configuration. `None` installs a zero-cost no-op network.
    #[cfg(feature = "net")]
    pub net: Option<NetConfig>,
}

impl RuntimeConfig {
    /// A windowed configuration with networking off. Enable it with
    /// `with_net`.
    ///
    /// Always construct through this (or [`headless`](Self::headless)) from
    /// another crate, never with a struct literal: the `net` field only exists
    /// when *this* crate's `net` feature is on, and that feature can be enabled
    /// by any crate in the dependency graph. A caller that spells out every
    /// field compiles only when its own `net` feature happens to agree.
    pub fn windowed(window: WindowConfig, renderer: RendererConfig, target_fps: f64, tickrate: f64) -> Self {
        Self {
            window,
            renderer,
            audio: AudioConfig::default(),
            target_fps,
            tickrate,
            #[cfg(feature = "net")]
            net: None,
        }
    }

    /// A configuration for a **headless server**: no window, no GPU, just a
    /// fixed-update tick driving simulation. The window/renderer fields are
    /// placeholders never touched by [`HeadlessRuntime`]. `tickrate` follows
    /// the same `config.json` (`engine.tickrate`) fallback as the windowed
    /// path, defaulting to [`DEFAULT_TICKRATE`] when the file or key is
    /// absent. Enable networking with `with_net`.
    pub fn headless() -> Self {
        EngineSettings::load_config_json();

        let tickrate: f64 = EngineSettings::global_read().get_path("engine.tickrate", DEFAULT_TICKRATE);

        Self {
            target_fps: 0.0,
            tickrate,
            #[cfg(feature = "net")]
            net: None,
            window: WindowConfig {
                title: String::from("redixel-headless"),
                width: 0,
                height: 0,
                fullscreen: false,
            },
            renderer: RendererConfig::default(),
            audio: AudioConfig::default(),
        }
    }

    /// Enables networking on this configuration with the given transport role.
    #[cfg(feature = "net")]
    pub fn with_net(mut self, net: NetConfig) -> Self {
        self.net = Some(net);
        self
    }

    /// Sets the master and per-channel volume the game starts with.
    pub fn with_audio(mut self, audio: AudioConfig) -> Self {
        self.audio = audio;
        self
    }

    /// Builds the network manager described by the config, or a no-op when
    /// networking is disabled (feature off or no [`net`](Self::net) set).
    pub(crate) fn build_network(&self) -> Box<dyn NetworkManager> {
        #[cfg(feature = "net")]
        {
            match &self.net {
                Some(cfg) => redixel_net::build(cfg, self.tickrate),
                None => Box::new(redixel_core::net::NoOpNetwork),
            }
        }
        #[cfg(not(feature = "net"))]
        {
            Box::new(redixel_core::net::NoOpNetwork)
        }
    }

    /// Builds the audio backend described by the config, or a no-op when the
    /// `audio` feature is disabled or the platform's audio output fails to
    /// start.
    pub(crate) fn build_audio(&self) -> Box<dyn AudioManager> {
        #[cfg(feature = "audio")]
        {
            redixel_audio::build(&self.audio)
        }
        #[cfg(not(feature = "audio"))]
        {
            Box::new(NoOpAudio::new(self.audio))
        }
    }
}

/// Runs a game **headless** (server mode): no `winit`, no `wgpu`. Drives only
/// `on_start` then a fixed-rate `on_fixed_update` loop (networking + physics),
/// pacing each tick with a sleep to keep CPU/RAM minimal on a VPS. `on_update`
/// and `on_render` never fire.
///
/// The same `Game` implementation runs here and in the windowed [`Runtime`] —
/// only the rendering callbacks are skipped.
#[cfg(not(target_arch = "wasm32"))]
pub struct HeadlessRuntime<G: Game> {
    sim: SimulationCore<G>,
    tickrate: f64,
}

#[cfg(not(target_arch = "wasm32"))]
impl<G: Game> HeadlessRuntime<G> {
    /// Builds a headless runner from `config`, installing the configured network
    /// transport. A non-positive tickrate falls back to [`DEFAULT_TICKRATE`].
    pub fn new(game: G, config: RuntimeConfig) -> Self {
        let tickrate: f64 = if config.tickrate > 0.0 {
            config.tickrate
        } else {
            DEFAULT_TICKRATE
        };

        let mut time: TimeManager = TimeManager::new();
        time.set_tickrate(tickrate);

        let context: Context<G::Action> = Context::with_network(config.build_network()).with_audio(config.audio);

        Self {
            sim: SimulationCore::new(time, context, game),
            tickrate,
        }
    }

    /// Runs `on_start`, then the fixed-update loop until the game calls
    /// `ctx.exit()`. Blocks the calling thread; returns any fatal game error.
    pub fn run(mut self) -> Result<(), RedixelError> {
        use std::thread::sleep;
        use std::time::Instant;

        self.sim.start()?;

        let mut last: Instant = Instant::now();
        log::info!("Headless runtime started at {} Hz.", self.tickrate);

        loop {
            let now: Instant = Instant::now();
            let frame_delta: f64 = now.duration_since(last).as_secs_f64();
            last = now;

            match self.sim.run_fixed_updates(frame_delta) {
                StepFlow::Exit => {
                    log::info!("Headless runtime stopping (game requested shutdown).");
                    return Ok(());
                }
                StepFlow::Fatal(e) => return Err(e),
                StepFlow::Continue => {}
            }

            self.sim.context.reset_frame();

            let tick_dur: Duration = Duration::from_secs_f64(self.sim.time.fixed_delta());
            let work: Duration = Instant::now().duration_since(now);
            if work < tick_dur {
                sleep(tick_dur - work);
            }
        }
    }
}

/// Thin convenience wrapper around [`HeadlessRuntime::new`] + [`HeadlessRuntime::run`].
#[cfg(not(target_arch = "wasm32"))]
pub fn run_headless<G: Game>(game: G, config: RuntimeConfig) -> Result<(), RedixelError> {
    HeadlessRuntime::new(game, config).run()
}

type BridgePayload = Result<(Renderer, WindowManager), RedixelError>;

struct RunningState<G: Game> {
    renderer: Renderer,
    window: WindowManager,
    sim: SimulationCore<G>,
    audio: Box<dyn AudioManager>,
}

enum AppState<G: Game> {
    Initializing,
    Loading,
    Running(Box<RunningState<G>>),
}

/// Implements [`ApplicationHandler`] and drives the engine from creation to
/// shutdown. Owns the application state and the async initialisation bridge.
pub struct Runtime<G: Game> {
    state: AppState<G>,
    pending_game: Option<G>,
    fatal_error: Option<RedixelError>,
    bridge_tx: Sender<BridgePayload>,
    bridge_rx: Receiver<BridgePayload>,
    config: RuntimeConfig,
    is_suspended: bool,
}

impl<G: Game> Runtime<G> {
    pub fn new(game: G, config: RuntimeConfig) -> Self {
        let (bridge_tx, bridge_rx): (Sender<BridgePayload>, Receiver<BridgePayload>) = mpsc::channel();
        Self {
            state: AppState::Initializing,
            pending_game: Some(game),
            fatal_error: None,
            bridge_tx,
            bridge_rx,
            config,
            is_suspended: false,
        }
    }

    /// Moves the stored fatal error out of the runtime.
    /// Called by `redixel::run()` after `run_app` returns.
    pub fn take_error(&mut self) -> Option<RedixelError> {
        self.fatal_error.take()
    }

    fn abort(&mut self, event_loop: &dyn ActiveEventLoop, error: RedixelError) {
        self.fatal_error = Some(error);
        event_loop.exit();
    }

    fn transition_to_running(&mut self, mut renderer: Renderer, window: WindowManager) -> Result<(), RedixelError> {
        window.request_redraw();

        let mut time: TimeManager = TimeManager::new();
        time.set_target_fps(self.config.target_fps);
        time.set_tickrate(self.config.tickrate);

        let initial_size: PhysicalSize<u32> = window.surface_size();
        let mut context: Context<G::Action> =
            Context::with_network(self.config.build_network()).with_audio(self.config.audio);
        context.update_state(initial_size.width, initial_size.height);
        context.update_fullscreen(window.is_fullscreen());

        let game: G = self.pending_game.take().expect("pending_game already consumed");
        let mut sim: SimulationCore<G> = SimulationCore::new(time, context, game);
        sim.start()?;

        Self::upload_textures(&mut renderer, &mut sim.context);
        let mut audio: Box<dyn AudioManager> = self.config.build_audio();
        Self::upload_sounds(audio.as_mut(), &mut sim.context);

        self.state = AppState::Running(Box::new(RunningState {
            renderer,
            window,
            sim,
            audio,
        }));
        Ok(())
    }

    /// Hands every queued image to the renderer.
    ///
    /// A decode failure is logged and dropped rather than propagated: the
    /// handle was already issued to game code, and an unfilled slot draws the
    /// checkerboard. Taking the process down over a bad asset would be a worse
    /// trade than drawing it loudly.
    ///
    /// Called at startup, since `on_start` loads assets before the first frame,
    /// and again each frame before the draw commands dispatch, so a texture
    /// requested in `on_update` is resident by that frame's `on_render`.
    fn upload_textures(renderer: &mut Renderer, context: &mut Context<G::Action>) {
        for request in context.drain_texture_requests() {
            let TextureRequest { id, bytes, filter } = request;

            if let Err(e) = renderer.load_texture_filtered(id, &bytes, filter) {
                log::warn!(
                    "Failed to load texture {}: {e}. Drawing the missing-texture checkerboard.",
                    id.index()
                );
            }
        }
    }

    /// Hands every queued sound to the audio backend, which decodes it off
    /// this thread.
    ///
    /// Called at the same two points as [`upload_textures`](Self::upload_textures),
    /// so a sound requested in a frame is loading before any play of it issued
    /// in that frame dispatches.
    fn upload_sounds(audio: &mut dyn AudioManager, context: &mut Context<G::Action>) {
        for request in context.drain_sound_requests() {
            let SoundRequest { id, bytes } = request;
            audio.load_clip(id, bytes);
        }
    }

    async fn init_gpu(
        tx: Sender<BridgePayload>,
        window: Arc<dyn Window>,
        window_mgr: WindowManager,
        proxy: EventLoopProxy,
        config: RendererConfig,
    ) {
        let result: Result<(Renderer, WindowManager), RedixelError> =
            Renderer::new(window, config).await.map(|r: Renderer| (r, window_mgr));

        tx.send(result).ok();
        proxy.wake_up();
    }

    fn spawn_gpu_init(&self, event_loop: &dyn ActiveEventLoop, window_mgr: WindowManager) {
        let tx: Sender<BridgePayload> = self.bridge_tx.clone();
        let window: Arc<dyn Window> = window_mgr.window_arc();
        let proxy: EventLoopProxy = event_loop.create_proxy();
        let config: RendererConfig = self.config.renderer.clone();

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(Self::init_gpu(tx, window, window_mgr, proxy, config));

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || pollster::block_on(Self::init_gpu(tx, window, window_mgr, proxy, config)));
    }

    fn on_can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        match WindowManager::new(event_loop, &self.config.window) {
            Ok(window) => {
                self.spawn_gpu_init(event_loop, window);
                self.state = AppState::Loading;
            }
            Err(e) => self.abort(event_loop, e),
        }
    }

    fn on_proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        let payload: Result<(Renderer, WindowManager), RedixelError> = match self.bridge_rx.try_recv() {
            Ok(p) => p,
            Err(..) => return,
        };

        match payload {
            Ok((renderer, window)) => {
                if let Err(e) = self.transition_to_running(renderer, window) {
                    self.abort(event_loop, e);
                }
            }
            Err(e) => self.abort(event_loop, e),
        }
    }

    fn on_app_suspended(&mut self) {
        if let AppState::Running(state) = &mut self.state {
            state.renderer.suspend();
            state.audio.suspend();
        }
    }

    fn on_app_resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.is_suspended = false;

        let result: Result<(), RedixelError> = if let AppState::Running(state) = &mut self.state {
            state.audio.resume();
            state.renderer.resume(&state.window.window_arc())
        } else {
            Ok(())
        };

        if let Err(e) = result {
            self.abort(event_loop, e);
        }
    }

    fn run_frame(&mut self, event_loop: &dyn ActiveEventLoop) {
        let AppState::Running(state) = &mut self.state else {
            return;
        };

        state.sim.context.tick_input();
        state.sim.time.begin_frame();

        let frame_delta: f64 = state.sim.time.delta_time();
        state.sim.context.update_timing(frame_delta, state.sim.time.fps());

        match state.sim.run_fixed_updates(frame_delta) {
            StepFlow::Exit => {
                event_loop.exit();
                return;
            }
            StepFlow::Fatal(e) => {
                self.fatal_error = Some(e);
                event_loop.exit();
                return;
            }
            StepFlow::Continue => {}
        }

        state.sim.context.set_fixed_alpha(state.sim.time.interpolation_alpha());
        state.sim.game.on_update(&mut state.sim.context);

        if state.sim.context.should_exit() {
            event_loop.exit();
            return;
        }

        state.sim.game.on_render(&mut state.sim.context);

        Self::upload_textures(&mut state.renderer, &mut state.sim.context);
        Self::upload_sounds(state.audio.as_mut(), &mut state.sim.context);

        for cmd in state.sim.context.drain_commands() {
            match cmd {
                DrawCommand::ClearColor(c) => {
                    state.renderer.set_clear_color(c);
                }
                DrawCommand::Rect { position, size, color } => {
                    state.renderer.draw_rect(position, size, color);
                }
                DrawCommand::Sprite {
                    position,
                    size,
                    texture,
                    tint,
                } => {
                    state.renderer.draw_sprite(position, size, texture, tint);
                }
                DrawCommand::Triangle { p1, p2, p3, color } => {
                    state.renderer.draw_triangle(p1, p2, p3, color);
                }
                DrawCommand::Triangle3d { p1, p2, p3, color } => {
                    state.renderer.draw_triangle_3d(p1, p2, p3, color);
                }
                DrawCommand::Triangle3dTextured {
                    points,
                    uvs,
                    texture,
                    tint,
                } => {
                    state.renderer.draw_triangle_3d_textured(points, uvs, texture, tint);
                }
            }
        }

        for cmd in state.sim.context.drain_audio_commands() {
            match cmd {
                AudioCommand::PlaySound { clip, volume, pitch } => {
                    state.audio.play_sound(clip, volume, pitch);
                }
                AudioCommand::PlayMusic { clip, options } => {
                    state.audio.play_music(clip, options);
                }
                AudioCommand::StopMusic { fade_out } => {
                    state.audio.stop_music(fade_out);
                }
                AudioCommand::SetMasterVolume(volume) => {
                    state.audio.set_master_volume(volume);
                }
                AudioCommand::SetChannelVolume { channel, volume } => {
                    state.audio.set_channel_volume(channel, volume);
                }
            }
        }
        state.audio.update(frame_delta as f32);

        match state.renderer.render(state.sim.time.elapsed_time() as f32) {
            Ok(()) => {}
            Err(RedixelError::SurfaceIgnored) => {}
            Err(RedixelError::SurfaceNeedsReconfiguration) => {
                state.renderer.resize(state.window.surface_size());
            }
            Err(e) => {
                self.fatal_error = Some(e);
                event_loop.exit();
                return;
            }
        }

        if let Some(fullscreen) = state.sim.context.take_fullscreen_request() {
            state.window.set_fullscreen(fullscreen);
        }

        if let Some((width, height)) = state.sim.context.take_surface_size_request()
            && let Some(size) = state.window.request_surface_size(width, height)
        {
            state.renderer.resize(size);
            state.sim.context.update_state(size.width, size.height);
        }

        state.sim.context.reset_frame();
        state.sim.time.end_frame();

        state
            .sim
            .time
            .every_seconds(1.0, |fps: f64| state.window.set_title_stats(fps, state.sim.context.rtt_ms()));

        state.window.request_redraw();
    }

    fn on_window_event(&mut self, event_loop: &dyn ActiveEventLoop, event: WindowEvent) {
        let AppState::Running(state) = &mut self.state else {
            return;
        };

        if self.is_suspended {
            return;
        }

        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                event_loop.exit();
            }

            WindowEvent::SurfaceResized(size) => {
                state.renderer.resize(size);
                state.sim.context.update_state(size.width, size.height);
            }

            WindowEvent::RedrawRequested => {
                self.run_frame(event_loop);
            }

            ref e => {
                if state.sim.context.process_input_event(e) {
                    return;
                }

                state.window.process_window_event(e);
            }
        }
    }
}

impl<G: Game> Drop for Runtime<G> {
    /// Writes the volumes back to `config/config.json` if the game changed
    /// them during the session, so they carry over to the next run.
    fn drop(&mut self) {
        if let AppState::Running(state) = &self.state {
            persist_volumes(self.config.audio.clamped(), state.sim.context.volumes());
        }
    }
}

/// Writes `current` over the `audio` section of the global settings and saves
/// them, unless it equals `initial`, the volumes the session started with.
fn persist_volumes(initial: AudioConfig, current: AudioConfig) {
    if current == initial {
        return;
    }

    let mut settings: RwLockWriteGuard<'static, EngineSettings> = EngineSettings::global_write();
    settings.set_path("audio.master_volume", json_volume(current.master_volume));
    settings.set_path("audio.sfx_volume", json_volume(current.sfx_volume));
    settings.set_path("audio.music_volume", json_volume(current.music_volume));
    drop(settings);

    EngineSettings::save_config_json();
}

/// `volume` as the `f64` with the same shortest decimal form, so `0.4` is
/// written as `0.4` rather than as the `0.4000000059604645` a plain widening
/// cast produces.
fn json_volume(volume: f32) -> f64 {
    volume
        .to_string()
        .parse()
        .unwrap_or_else(|_: ParseFloatError| f64::from(volume))
}

impl<G: Game> ApplicationHandler for Runtime<G> {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        match &mut self.state {
            AppState::Initializing => {
                log::info!("OS requested a surface. Initializing graphics bridge...");
                self.on_can_create_surfaces(event_loop);
            }
            AppState::Loading => {
                log::debug!("OS requested a surface, but ignored (waiting for GPU init).");
            }
            AppState::Running(..) => {
                log::info!("App Resumed. Waking up engine and reconstructing GPU surface.");
                self.on_app_resumed(event_loop);
            }
        }
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
        log::info!("OS requested suspension. Halting engine updates.");
        self.is_suspended = true;

        match self.state {
            AppState::Initializing | AppState::Loading => {
                log::debug!("Backgrounded before initialization completed.");
            }
            AppState::Running(..) => {
                log::info!("Dropping active GPU surface to comply with OS background limits.");
                self.on_app_suspended();
            }
        }
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        if matches!(self.state, AppState::Loading) {
            log::info!("GPU Initialization completed asynchronously. Transitioning to Running state.");
            self.on_proxy_wake_up(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.on_window_event(event_loop, event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mpsc::TryRecvError;

    use redixel_core::{AudioChannel, GameContext, MusicOptions, SoundId, TextureFilter, TextureId, audio::MIN_PITCH};
    use redixel_math::{Color, Vec2};

    use crate::context::{SoundRequest, TextureRequest};

    struct Dummy;
    impl Game for Dummy {
        type Action = ();
        fn on_start(&mut self, _ctx: &mut dyn GameContext<()>) {}
        fn on_update(&mut self, _ctx: &mut dyn GameContext<()>) {}
        fn on_render(&mut self, _ctx: &mut dyn GameContext<()>) {}
    }

    fn mock_config() -> RuntimeConfig {
        RuntimeConfig::windowed(
            WindowConfig {
                width: 800,
                height: 600,
                fullscreen: false,
                title: String::from("TEST_TITLE"),
            },
            RendererConfig {
                backends: wgpu::Backends::all(),
                present_mode: wgpu::PresentMode::AutoVsync,
            },
            60.0,
            60.0,
        )
    }

    #[test]
    fn initial_state_is_initializing() {
        let rt: Runtime<Dummy> = Runtime::new(Dummy, mock_config());
        assert!(matches!(rt.state, AppState::Initializing));
        assert!(rt.fatal_error.is_none());
        assert!(rt.pending_game.is_some());
    }

    #[test]
    fn bridge_channel_is_open() {
        let rt: Runtime<Dummy> = Runtime::new(Dummy, mock_config());
        rt.bridge_tx
            .send(Err(RedixelError::Dummy))
            .expect("channel must be open at construction");
        assert!(rt.bridge_rx.try_recv().is_ok());
    }

    #[test]
    fn bridge_delivers_error_correctly() {
        let rt: Runtime<Dummy> = Runtime::new(Dummy, mock_config());
        rt.bridge_tx.send(Err(RedixelError::Dummy)).unwrap();
        let received: Result<BridgePayload, TryRecvError> = rt.bridge_rx.try_recv();
        assert!(matches!(received.unwrap(), Err(RedixelError::Dummy)));
    }

    #[test]
    fn take_error_moves_and_clears() {
        let mut rt: Runtime<Dummy> = Runtime::new(Dummy, mock_config());
        rt.fatal_error = Some(RedixelError::Dummy);
        assert!(matches!(rt.take_error(), Some(RedixelError::Dummy)));
        assert!(rt.fatal_error.is_none());
    }

    #[test]
    fn context_draw_commands_accumulate() {
        let mut ctx: Context<()> = Context::new();
        ctx.draw_rect(Vec2::new(0.0, 0.0), Vec2::new(100.0, 50.0), Color::RED);
        ctx.draw_rect(Vec2::new(10.0, 10.0), Vec2::new(20.0, 20.0), Color::BLUE);
        assert_eq!(ctx.commands.len(), 2);

        let drained: Vec<DrawCommand> = ctx.drain_commands().collect();
        assert_eq!(drained.len(), 2);
        assert!(ctx.commands.is_empty());
    }

    #[test]
    fn context_clear_color_deduplicates() {
        let mut ctx: Context<()> = Context::new();

        ctx.clear_color(Color::RED);
        ctx.clear_color(Color::BLUE);

        let clears: Vec<&DrawCommand> = ctx
            .commands
            .iter()
            .filter(|c: &&DrawCommand| matches!(c, DrawCommand::ClearColor(..)))
            .collect();

        assert_eq!(clears.len(), 1);
    }

    #[test]
    fn context_exit_flag_roundtrip() {
        let mut ctx: Context<()> = Context::new();
        assert!(!ctx.should_exit());
        ctx.exit();
        assert!(ctx.should_exit());
        ctx.reset_frame();
        assert!(!ctx.should_exit());
    }

    #[test]
    fn context_timing_update() {
        let mut ctx: Context<()> = Context::new();
        ctx.update_timing(0.016, 62.5);
        assert!((ctx.delta_time() - 0.016).abs() < 1e-9);
        assert!((ctx.fps() - 62.5).abs() < 1e-9);
    }

    #[test]
    fn surface_size_request_keeps_the_latest_valid_size_and_drains_once() {
        let mut ctx: Context<()> = Context::new();

        ctx.request_surface_size(960, 540);
        ctx.request_surface_size(0, 720);
        ctx.request_surface_size(1920, 1080);

        assert_eq!(ctx.take_surface_size_request(), Some((1920, 1080)));
        assert_eq!(ctx.take_surface_size_request(), None);
    }

    #[test]
    fn reset_frame_discards_an_unhandled_surface_size_request() {
        let mut ctx: Context<()> = Context::new();
        ctx.request_surface_size(1280, 720);

        ctx.reset_frame();

        assert_eq!(ctx.take_surface_size_request(), None);
    }

    #[test]
    fn fullscreen_request_updates_state_and_drains_once() {
        let mut ctx: Context<()> = Context::new();
        assert!(!ctx.is_fullscreen());

        ctx.request_fullscreen(true);

        assert!(ctx.is_fullscreen());
        assert_eq!(ctx.take_fullscreen_request(), Some(true));
        assert_eq!(ctx.take_fullscreen_request(), None);

        ctx.request_fullscreen(false);
        assert!(!ctx.is_fullscreen());
        ctx.reset_frame();
        assert_eq!(ctx.take_fullscreen_request(), None);
    }

    #[test]
    fn load_texture_assigns_sequential_ids() {
        let mut ctx: Context<()> = Context::new();

        let first: TextureId = ctx.load_texture(b"first");
        let second: TextureId = ctx.load_texture(b"second");
        let third: TextureId = ctx.load_texture(b"third");

        assert_eq!(
            [first.index(), second.index(), third.index()],
            [0, 1, 2],
            "the renderer indexes its slots by id, so they must stay dense and ordered"
        );
    }

    #[test]
    fn load_texture_queues_the_bytes_against_its_id() {
        let mut ctx: Context<()> = Context::new();
        let id: TextureId = ctx.load_texture(b"png bytes");

        let requests: Vec<TextureRequest> = ctx.drain_texture_requests().collect();

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].id, id);
        assert_eq!(requests[0].bytes, b"png bytes");
        assert_eq!(requests[0].filter, TextureFilter::Nearest);
    }

    #[test]
    fn filtered_texture_request_keeps_its_sampling_mode() {
        let mut ctx: Context<()> = Context::new();
        let id: TextureId = ctx.load_texture_filtered(b"smooth png", TextureFilter::Linear);

        let requests: Vec<TextureRequest> = ctx.drain_texture_requests().collect();

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].id, id);
        assert_eq!(requests[0].bytes, b"smooth png");
        assert_eq!(requests[0].filter, TextureFilter::Linear);
    }

    #[test]
    fn texture_requests_drain_once() {
        let mut ctx: Context<()> = Context::new();
        ctx.load_texture(b"first");
        ctx.load_texture(b"second");

        assert_eq!(ctx.drain_texture_requests().count(), 2);
        assert_eq!(
            ctx.drain_texture_requests().count(),
            0,
            "a second drain would re-upload textures that are already resident"
        );
    }

    #[test]
    fn reset_frame_clears_texture_requests() {
        let mut ctx: Context<()> = Context::new();
        ctx.load_texture(b"never drained");

        ctx.reset_frame();

        assert_eq!(
            ctx.drain_texture_requests().count(),
            0,
            "headless never drains the queue; without this it grows forever"
        );
    }

    #[test]
    fn ids_keep_climbing_across_frames() {
        let mut ctx: Context<()> = Context::new();
        ctx.load_texture(b"first");
        ctx.reset_frame();

        let later: TextureId = ctx.load_texture(b"second");

        assert_eq!(later.index(), 1, "reusing an id would alias two images");
    }

    #[test]
    fn draw_sprite_defaults_to_an_untinted_white() {
        let mut ctx: Context<()> = Context::new();
        let id: TextureId = ctx.load_texture(b"png bytes");
        ctx.draw_sprite(Vec2::ZERO, Vec2::ONE, id);

        let drained: Vec<DrawCommand> = ctx.drain_commands().collect();

        match drained.as_slice() {
            [DrawCommand::Sprite { texture, tint, .. }] => {
                assert_eq!(*texture, id);
                assert_eq!(*tint, Color::WHITE, "the default tint leaves the image untouched");
            }
            other => panic!("expected a single sprite command, got {other:?}"),
        }
    }

    #[test]
    fn load_sound_assigns_sequential_ids_and_queues_the_bytes() {
        let mut ctx: Context<()> = Context::new();

        let first: SoundId = ctx.load_sound(b"first");
        let second: SoundId = ctx.load_sound(b"second");

        assert_eq!([first.index(), second.index()], [0, 1]);

        let requests: Vec<SoundRequest> = ctx.drain_sound_requests().collect();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].id, first);
        assert_eq!(requests[0].bytes, b"first");
        assert_eq!(requests[1].bytes, b"second");
    }

    #[test]
    fn play_sound_defaults_to_unity_volume_and_pitch() {
        let mut ctx: Context<()> = Context::new();
        let id: SoundId = ctx.load_sound(b"sfx");

        ctx.play_sound(id);

        let drained: Vec<AudioCommand> = ctx.drain_audio_commands().collect();
        match drained.as_slice() {
            [AudioCommand::PlaySound { clip, volume, pitch }] => {
                assert_eq!(*clip, id);
                assert_eq!(*volume, 1.0);
                assert_eq!(*pitch, 1.0);
            }
            other => panic!("expected a single PlaySound command, got {other:?}"),
        }
    }

    #[test]
    fn play_music_defaults_loop_with_no_fade() {
        let mut ctx: Context<()> = Context::new();
        let id: SoundId = ctx.load_sound(b"music");

        ctx.play_music(id);

        let drained: Vec<AudioCommand> = ctx.drain_audio_commands().collect();
        match drained.as_slice() {
            [AudioCommand::PlayMusic { clip, options }] => {
                assert_eq!(*clip, id);
                assert_eq!(*options, MusicOptions::default());
            }
            other => panic!("expected a single PlayMusic command, got {other:?}"),
        }
    }

    #[test]
    fn stop_music_fade_carries_its_duration() {
        let mut ctx: Context<()> = Context::new();
        ctx.stop_music_fade(2.5);

        let drained: Vec<AudioCommand> = ctx.drain_audio_commands().collect();
        assert!(matches!(drained.as_slice(), [AudioCommand::StopMusic { fade_out }] if *fade_out == 2.5));
    }

    #[test]
    fn volume_setters_update_the_getters_and_queue_a_command() {
        let mut ctx: Context<()> = Context::new();

        ctx.set_master_volume(0.4);
        ctx.set_channel_volume(AudioChannel::Sfx, 0.6);
        ctx.set_channel_volume(AudioChannel::Music, 0.2);

        assert_eq!(ctx.master_volume(), 0.4);
        assert_eq!(ctx.channel_volume(AudioChannel::Sfx), 0.6);
        assert_eq!(ctx.channel_volume(AudioChannel::Music), 0.2);
        assert_eq!(ctx.drain_audio_commands().count(), 3);
    }

    #[test]
    fn audio_values_are_clamped_before_they_are_queued() {
        let mut ctx: Context<()> = Context::new();
        let id: SoundId = ctx.load_sound(b"sfx");

        ctx.set_master_volume(f32::NAN);
        ctx.play_sound_with(id, 4.0, -1.0);

        let drained: Vec<AudioCommand> = ctx.drain_audio_commands().collect();
        match drained.as_slice() {
            [
                AudioCommand::SetMasterVolume(master),
                AudioCommand::PlaySound { volume, pitch, .. },
            ] => {
                assert_eq!(*master, 0.0);
                assert_eq!(*volume, 1.0);
                assert_eq!(*pitch, MIN_PITCH);
            }
            other => panic!("expected a volume change then a play, got {other:?}"),
        }
        assert_eq!(ctx.master_volume(), 0.0);
    }

    #[test]
    fn with_audio_starts_from_the_clamped_volumes() {
        let ctx: Context<()> = Context::new().with_audio(AudioConfig {
            master_volume: 0.5,
            sfx_volume: 2.0,
            music_volume: 0.25,
        });

        assert_eq!(ctx.master_volume(), 0.5);
        assert_eq!(ctx.channel_volume(AudioChannel::Sfx), 1.0);
        assert_eq!(ctx.channel_volume(AudioChannel::Music), 0.25);
    }

    #[test]
    fn json_volume_keeps_the_shortest_decimal_form() {
        assert_eq!(serde_json::to_string(&json_volume(0.4)).unwrap(), "0.4");
        assert_eq!(serde_json::to_string(&json_volume(1.0)).unwrap(), "1.0");
    }

    #[test]
    fn reset_frame_clears_audio_queues_too() {
        let mut ctx: Context<()> = Context::new();
        let id: SoundId = ctx.load_sound(b"never drained");
        ctx.play_sound(id);

        ctx.reset_frame();

        assert_eq!(
            ctx.drain_sound_requests().count(),
            0,
            "headless never drains sound requests either; without this it grows forever"
        );
        assert_eq!(ctx.drain_audio_commands().count(), 0);
    }
}
