use redixel_core::{
    RedixelError, TextureId,
    game::{GameContext, InputBind, InputQuery},
    input::InputAction,
    net::{NetworkManager, NoOpNetwork},
};
use redixel_math::{Color, Vec2, Vec3};
use redixel_platform::InputManager;

/// A draw command buffered during `on_render` and flushed by the runtime.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DrawCommand {
    ClearColor(Color),
    Rect {
        position: Vec2,
        size: Vec2,
        color: Color,
    },
    Sprite {
        position: Vec2,
        size: Vec2,
        texture: TextureId,
        tint: Color,
    },
    Triangle {
        p1: Vec2,
        p2: Vec2,
        p3: Vec2,
        color: Color,
    },
    Triangle3d {
        p1: Vec3,
        p2: Vec3,
        p3: Vec3,
        color: Color,
    },
    Triangle3dTextured {
        points: [Vec3; 3],
        uvs: [Vec2; 3],
        texture: TextureId,
        tint: Color,
    },
}

/// An image the game asked to load, waiting for the runtime to hand it to the
/// renderer.
///
/// The bytes are owned because the request outlives the call that made it:
/// game code may pass a temporary, and the upload happens later in the frame.
#[derive(Debug, Clone)]
pub struct TextureRequest {
    pub id: TextureId,
    pub bytes: Vec<u8>,
}

/// Concrete engine context passed to [`Game`](redixel_core::Game) callbacks each frame.
///
/// Implements [`GameContext`] and is passed as `&mut dyn GameContext` to keep
/// `redixel-core` free of any dependency on `redixel-runtime`.
///
/// Timing fields are updated by the runtime before each `on_update`.
/// Draw commands are accumulated during `on_render` and drained after.
pub struct Context<A: InputAction> {
    should_exit: bool,
    error: Option<RedixelError>,
    delta_time: f64,
    fps: f64,
    fixed_delta: f64,
    fixed_tick: u64,
    fixed_alpha: f64,
    surface_width: u32,
    surface_height: u32,
    surface_size_request: Option<(u32, u32)>,
    fullscreen: bool,
    fullscreen_request: Option<bool>,
    next_texture_id: u32,
    pub(crate) input: InputManager<A>,
    pub(crate) commands: Vec<DrawCommand>,
    pub(crate) texture_requests: Vec<TextureRequest>,
    pub(crate) network: Box<dyn NetworkManager>,
}

impl<A: InputAction> Context<A> {
    pub fn new() -> Self {
        Self::with_network(Box::new(NoOpNetwork))
    }

    /// Builds a context driving the given transport. The runtime installs the
    /// real backend here; offline games get a [`NoOpNetwork`] via [`new`](Self::new).
    pub fn with_network(network: Box<dyn NetworkManager>) -> Self {
        Self {
            should_exit: false,
            error: None,
            delta_time: 0.0,
            fps: 0.0,
            fixed_delta: 0.0,
            fixed_tick: 0,
            fixed_alpha: 0.0,
            surface_width: 0,
            surface_height: 0,
            surface_size_request: None,
            fullscreen: false,
            fullscreen_request: None,
            next_texture_id: 0,
            input: InputManager::new(),
            commands: Vec::with_capacity(1024),
            texture_requests: Vec::new(),
            network,
        }
    }

    /// Reserves a handle with nothing queued against it, which is what a load
    /// that failed before producing bytes needs.
    ///
    /// Ids are sequential and never reused, so the renderer indexes its slots
    /// by id directly, and an unfilled slot draws the checkerboard.
    fn reserve_texture_id(&mut self) -> TextureId {
        let id: TextureId = TextureId::new(self.next_texture_id);
        self.next_texture_id += 1;
        id
    }

    /// Reserves the next handle and queues `bytes` for the runtime to upload.
    fn queue_texture(&mut self, bytes: Vec<u8>) -> TextureId {
        let id: TextureId = self.reserve_texture_id();

        self.texture_requests.push(TextureRequest { id, bytes });
        id
    }

    /// Updates per-frame timing values. Called before `on_update`.
    pub(crate) fn update_timing(&mut self, delta_time: f64, fps: f64) {
        self.delta_time = delta_time;
        self.fps = fps;
    }

    /// Sets the fixed-step timing values. Called before each `on_fixed_update`.
    pub(crate) fn set_fixed(&mut self, fixed_delta: f64, fixed_tick: u64) {
        self.fixed_delta = fixed_delta;
        self.fixed_tick = fixed_tick;
    }

    /// Sets how far into the next fixed step this frame falls. Called once
    /// per frame after the fixed-update loop has consumed every full step
    /// it owed, so the leftover reflects exactly the gap `on_update`/
    /// `on_render` need to extrapolate across.
    pub(crate) fn set_fixed_alpha(&mut self, alpha: f64) {
        self.fixed_alpha = alpha;
    }

    /// Updates the surface dimensions. Called on resize and after init.
    pub(crate) fn update_state(&mut self, width: u32, height: u32) {
        self.surface_width = width;
        self.surface_height = height;
    }

    pub(crate) fn take_surface_size_request(&mut self) -> Option<(u32, u32)> {
        self.surface_size_request.take()
    }

    pub(crate) fn update_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
    }

    pub(crate) fn take_fullscreen_request(&mut self) -> Option<bool> {
        self.fullscreen_request.take()
    }

    /// Advances input state machine. Called at the start of every frame,
    /// before OS events are processed.
    pub(crate) fn tick_input(&mut self) {
        self.input.tick();
    }

    /// Feeds a raw OS event into the input manager.
    pub(crate) fn process_input_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        self.input.process_event(event)
    }

    /// Drains queued draw commands for the renderer to consume.
    pub(crate) fn drain_commands(&mut self) -> impl Iterator<Item = DrawCommand> + '_ {
        self.commands.drain(..)
    }

    /// Drains queued texture loads for the renderer to upload.
    pub(crate) fn drain_texture_requests(&mut self) -> impl Iterator<Item = TextureRequest> + '_ {
        self.texture_requests.drain(..)
    }

    /// Resets transient per-frame flags. Called after the renderer flushes.
    ///
    /// Texture requests are cleared here too: the windowed runtime has already
    /// drained them, but headless never does, and the queue would otherwise
    /// grow for the life of the process.
    pub(crate) fn reset_frame(&mut self) {
        self.should_exit = false;
        self.surface_size_request = None;
        self.fullscreen_request = None;
        self.commands.clear();
        self.texture_requests.clear();
    }

    /// Round-trip time to the server in milliseconds, once known — `None`
    /// when offline, unconnected, or acting as the server itself.
    pub(crate) fn rtt_ms(&self) -> Option<f64> {
        (!self.network.is_server() && self.network.is_connected()).then(|| self.network.rtt() as f64 * 1000.0)
    }
}

impl<A: InputAction> Default for Context<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: InputAction> GameContext<A> for Context<A> {
    fn exit(&mut self) {
        self.should_exit = true;
    }

    fn should_exit(&self) -> bool {
        self.should_exit
    }

    fn delta_time(&self) -> f64 {
        self.delta_time
    }

    fn fixed_delta(&self) -> f64 {
        self.fixed_delta
    }

    fn fixed_tick(&self) -> u64 {
        self.fixed_tick
    }

    fn fixed_alpha(&self) -> f64 {
        self.fixed_alpha
    }

    fn fps(&self) -> f64 {
        self.fps
    }

    fn network(&mut self) -> &mut dyn NetworkManager {
        self.network.as_mut()
    }

    fn surface_width(&self) -> u32 {
        self.surface_width
    }

    fn surface_height(&self) -> u32 {
        self.surface_height
    }

    fn request_surface_size(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_size_request = Some((width, height));
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }

    fn request_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        self.fullscreen_request = Some(fullscreen);
    }

    fn input(&self) -> &dyn InputQuery<A> {
        &self.input
    }

    fn input_mut(&mut self) -> &mut dyn InputBind<A> {
        &mut self.input
    }

    fn load_texture(&mut self, bytes: &[u8]) -> TextureId {
        self.queue_texture(bytes.to_vec())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_texture_file(&mut self, path: &str) -> TextureId {
        match std::fs::read(path) {
            Ok(bytes) => self.queue_texture(bytes),
            Err(e) => {
                log::warn!("Failed to read texture '{path}': {e}. Drawing the missing-texture checkerboard.");
                self.reserve_texture_id()
            }
        }
    }

    fn clear_color(&mut self, color: Color) {
        self.commands
            .retain(|c: &DrawCommand| !matches!(c, DrawCommand::ClearColor(..)));
        self.commands.push(DrawCommand::ClearColor(color));
    }

    fn draw_triangle(&mut self, p1: Vec2, p2: Vec2, p3: Vec2, color: Color) {
        self.commands.push(DrawCommand::Triangle { p1, p2, p3, color });
    }

    fn draw_rect(&mut self, position: Vec2, size: Vec2, color: Color) {
        self.commands.push(DrawCommand::Rect { position, size, color });
    }

    fn draw_sprite(&mut self, position: Vec2, size: Vec2, texture: TextureId) {
        self.draw_sprite_tinted(position, size, texture, Color::WHITE);
    }

    fn draw_sprite_tinted(&mut self, position: Vec2, size: Vec2, texture: TextureId, tint: Color) {
        self.commands.push(DrawCommand::Sprite {
            position,
            size,
            texture,
            tint,
        });
    }

    fn draw_triangle_3d(&mut self, p1: Vec3, p2: Vec3, p3: Vec3, color: Color) {
        self.commands.push(DrawCommand::Triangle3d { p1, p2, p3, color });
    }

    fn draw_triangle_3d_textured(&mut self, points: [Vec3; 3], uvs: [Vec2; 3], texture: TextureId) {
        self.draw_triangle_3d_textured_tinted(points, uvs, texture, Color::WHITE);
    }

    fn draw_triangle_3d_textured_tinted(&mut self, points: [Vec3; 3], uvs: [Vec2; 3], texture: TextureId, tint: Color) {
        self.commands.push(DrawCommand::Triangle3dTextured {
            points,
            uvs,
            texture,
            tint,
        });
    }

    fn take_error(&mut self) -> Option<RedixelError> {
        self.error.take()
    }
}
