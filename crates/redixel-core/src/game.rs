use winit::{event::MouseButton, keyboard::KeyCode};

use redixel_math::{Color, Vec2, Vec3};

use crate::{InputAction, InputSource, RedixelError, net::NetworkManager, texture::TextureId};

/// The entry point for user game logic.
///
/// The associated type `Action` is your game's input action enum. The engine
/// is generic over it — `GameContext` exposes a typed input API with zero
/// overhead.
///
/// ```rust,ignore
/// #[derive(Clone, PartialEq, Eq, Hash)]
/// enum MyAction { MoveUp, Shoot }
///
/// struct MyGame;
///
/// impl Game for MyGame {
///     type Action = MyAction;
///
///     fn on_start(&mut self, ctx: &mut dyn GameContext<MyAction>) {
///         // Bind keys and mouse buttons using `.into()`
///         ctx.input_mut().bind(MyAction::MoveUp, KeyCode::KeyW.into());
///         ctx.input_mut().bind(MyAction::Shoot, MouseButton::Left.into());
///     }
///
///     fn on_update(&mut self, ctx: &mut dyn GameContext<MyAction>) {
///         if ctx.input().held(MyAction::MoveUp) { /* ... */ }
///         
///         if ctx.input().just_pressed(MyAction::Shoot) {
///             if let Some(pos) = ctx.input().mouse_position() {
///                 // Shoot towards mouse position
///             }
///         }
///     }
///
///     fn on_render(&mut self, ctx: &mut dyn GameContext<MyAction>) {
///         ctx.draw_rect(Vec2::new(0.0, 0.0), Vec2::new(50.0, 50.0), Color::WHITE);
///     }
/// }
/// ```
pub trait Game: 'static {
    /// The action enum that maps to keybinds/mouse for this game.
    ///
    /// Use `type Action = ()` if you don't need input.
    type Action: InputAction;

    /// Called once after the GPU context is ready. Bind keys and load assets here.
    ///
    /// On a headless server there is no GPU context — this still runs first so
    /// the same game code can set up its world and networking.
    fn on_start(&mut self, ctx: &mut dyn GameContext<Self::Action>);

    /// Called on a fixed cadence (the tickrate), decoupled from the render
    /// framerate, using an accumulator in the `TimeManager`. May run zero, one,
    /// or several times per rendered frame.
    ///
    /// Put deterministic simulation here: physics, and **all** networking
    /// (poll inbound events, step the authoritative world, broadcast state).
    /// `ctx.fixed_delta()` is constant; `ctx.fixed_tick()` is a monotonic tick
    /// counter — tag inputs/snapshots with it for prediction/reconciliation.
    ///
    /// This is the **only** game callback invoked in headless/server mode.
    /// Default is a no-op so non-networked games need not implement it.
    fn on_fixed_update(&mut self, _ctx: &mut dyn GameContext<Self::Action>) {}

    /// Called every rendered frame before rendering. Update visual/interpolated
    /// state here. Not invoked in headless/server mode.
    fn on_update(&mut self, ctx: &mut dyn GameContext<Self::Action>);

    /// Called every rendered frame after `on_update`. Issue draw calls here.
    /// Not invoked in headless/server mode.
    fn on_render(&mut self, ctx: &mut dyn GameContext<Self::Action>);
}

/// The interface through which [`Game`] methods talk to the engine each frame.
///
/// Generic over `A: InputAction` so that `input()` returns a typed view
/// without any dynamic dispatch or downcasting.
pub trait GameContext<A: InputAction> {
    /// Requests a clean engine shutdown after the current frame.
    fn exit(&mut self);

    /// Returns `true` if [`exit`](Self::exit) was called this frame.
    fn should_exit(&self) -> bool;

    /// Seconds elapsed between the two most recent frames (delta time).
    ///
    /// Use in `on_update` for frame-rate-dependent visuals. Inside
    /// `on_fixed_update`, prefer [`fixed_delta`](Self::fixed_delta).
    fn delta_time(&self) -> f64;

    /// Seconds of real time since startup.
    ///
    /// The same clock the renderer hands the shader as `globals.time`, so a
    /// value driven from it on the CPU stays in phase with anything the shader
    /// animates from it.
    fn elapsed_time(&self) -> f64;

    /// The constant timestep of the fixed-update loop, in seconds (e.g. `1/60`).
    ///
    /// This is the dt to integrate with inside `on_fixed_update` for
    /// deterministic, framerate-independent simulation.
    fn fixed_delta(&self) -> f64;

    /// Monotonic count of fixed-update ticks since startup.
    ///
    /// Stamp inputs and snapshots with this for client-side prediction and
    /// server reconciliation (see [`SequenceBuffer`](crate::net::SequenceBuffer)).
    fn fixed_tick(&self) -> u64;

    /// Fraction `[0, 1)` of the way into the next fixed step, measured at the
    /// moment `on_update`/`on_render` runs for this frame.
    ///
    /// The render framerate rarely lines up with the fixed-update tickrate,
    /// so a value simulated once per tick (a client-predicted local
    /// position, a physics body) sits still for however many rendered frames
    /// land between two ticks unless something accounts for the gap. Scale a
    /// per-tick delta (e.g. a velocity times [`fixed_delta`](Self::fixed_delta))
    /// by this fraction and add it in `on_render` to extrapolate that value
    /// smoothly across those frames, matching the display's own cadence
    /// instead of the simulation's.
    fn fixed_alpha(&self) -> f64;

    /// Current FPS measurement.
    fn fps(&self) -> f64;

    /// Access the networking transport.
    ///
    /// Always returns a valid manager — a zero-cost
    /// [`NoOpNetwork`](crate::net::NoOpNetwork) when networking is disabled — so
    /// game code never needs to branch on whether the net is configured.
    fn network(&mut self) -> &mut dyn NetworkManager;

    /// Width of the rendering surface in pixels.
    fn surface_width(&self) -> u32;

    /// Height of the rendering surface in pixels.
    fn surface_height(&self) -> u32;

    /// Returns a read-only view of the current input state.
    ///
    /// Use this to query actions, raw keys, and mouse state.
    fn input(&self) -> &dyn InputQuery<A>;

    /// Returns a mutable handle to bind actions to input sources.
    ///
    /// Call this in `on_start` to register your input bindings.
    fn input_mut(&mut self) -> &mut dyn InputBind<A>;

    /// Registers an image for loading and returns the handle to draw it with.
    ///
    /// The handle comes back immediately but the decode and upload happen later
    /// in the frame, since the renderer is not reachable from here. Call this
    /// from `on_start`; a handle is drawable from the frame it was requested in.
    ///
    /// A failed decode is **not** an error: it logs a warning and draws the
    /// missing-texture checkerboard.
    ///
    /// `bytes` is the encoded file, not raw pixels. Prefer `include_bytes!`,
    /// which works identically on desktop, web, and mobile.
    fn load_texture(&mut self, bytes: &[u8]) -> TextureId;

    /// Reads an image from disk and registers it, as [`load_texture`](Self::load_texture) does.
    ///
    /// `path` is resolved **relative to the process's working directory**, the
    /// same caveat that applies to `config/config.json`. A game that must run
    /// from anywhere should use `include_bytes!` instead. Unavailable on web,
    /// which has no filesystem.
    ///
    /// An unreadable path logs a warning and still yields a handle, drawn as
    /// the checkerboard, so game code never has to unwrap a `None`.
    #[cfg(not(target_arch = "wasm32"))]
    fn load_texture_file(&mut self, path: &str) -> TextureId;

    /// Sets the background clear colour for this frame.
    fn clear_color(&mut self, color: Color);

    /// Sets the strength of the time-driven brightness pulse applied to this
    /// frame's 3D geometry, `0.0` (the default) disabling it entirely.
    ///
    /// The pulse runs on the GPU from the engine's elapsed-time uniform — no
    /// CPU code animates it — and each fragment derives its phase smoothly
    /// from its position, so waves of brightness roll across large surfaces
    /// while separate objects pulse out of step with each other, at no
    /// per-object cost. `amount` blends between untouched (`0.0`) and fully
    /// pulsed (`1.0`) brightness. 2D geometry is never affected.
    fn set_glow(&mut self, amount: f32);

    /// Draws a filled triangle.
    ///
    /// - `p1`, `p2`, `p3` — The three vertices of the triangle in world coordinates
    /// - `color`          — fill colour
    fn draw_triangle(&mut self, p1: Vec2, p2: Vec2, p3: Vec2, color: Color);

    /// Draws a filled, axis-aligned rectangle.
    ///
    /// - `position` — top-left corner in world/screen coordinates (y-down)
    /// - `size`     — width × height in pixels
    /// - `color`    — fill colour
    fn draw_rect(&mut self, position: Vec2, size: Vec2, color: Color);

    /// Draws a texture stretched across an axis-aligned rectangle.
    ///
    /// - `position` — top-left corner in world/screen coordinates (y-down)
    /// - `size`     — width × height in pixels
    /// - `texture`  — a handle from [`load_texture`](Self::load_texture)
    ///
    /// The alpha channel is honoured, so a cut-out sprite blends against
    /// whatever was drawn before it. Sprites and solid shapes share one painter
    /// order: whatever is drawn last lands on top.
    fn draw_sprite(&mut self, position: Vec2, size: Vec2, texture: TextureId);

    /// Draws a texture as [`draw_sprite`](Self::draw_sprite) does, multiplying
    /// every sampled texel by `tint`.
    ///
    /// `Color::WHITE` is the no-op tint. Useful for flashing a sprite on damage
    /// or fading one out through the tint's alpha.
    fn draw_sprite_tinted(&mut self, position: Vec2, size: Vec2, texture: TextureId, tint: Color);

    /// Draws a filled triangle in 3D view space.
    ///
    /// - `p1`, `p2`, `p3` — the three vertices, in the engine's perspective
    ///   camera's view space: camera at the origin, looking down `+Z`, `+Y`
    ///   up (see [`Mat4::perspective`](redixel_math::Mat4::perspective))
    /// - `color`          — fill colour
    fn draw_triangle_3d(&mut self, p1: Vec3, p2: Vec3, p3: Vec3, color: Color);

    /// Draws a triangle in 3D view space with an independent colour per
    /// vertex, interpolated across the face by the rasteriser.
    ///
    /// The building block for smooth (Gouraud) shading — light each vertex of
    /// a mesh instead of each face and the facets disappear — and for soft
    /// gradients: a corner whose colour carries zero alpha fades the face out
    /// towards it.
    ///
    /// - `points` — the three vertices, in the perspective camera's view space
    /// - `colors` — the colour at each vertex, in the same order
    fn draw_triangle_3d_shaded(&mut self, points: [Vec3; 3], colors: [Color; 3]);

    /// Draws a textured triangle in 3D view space.
    ///
    /// - `points`  — the three vertices, in the perspective camera's view space
    /// - `uvs`     — where each vertex lands on the texture, `(0, 0)` at the
    ///   top-left and `(1, 1)` at the bottom-right
    /// - `texture` — a handle from [`load_texture`](Self::load_texture)
    fn draw_triangle_3d_textured(&mut self, points: [Vec3; 3], uvs: [Vec2; 3], texture: TextureId);

    /// Draws a textured triangle as
    /// [`draw_triangle_3d_textured`](Self::draw_triangle_3d_textured) does,
    /// multiplying every sampled texel by `tint`.
    fn draw_triangle_3d_textured_tinted(&mut self, points: [Vec3; 3], uvs: [Vec2; 3], texture: TextureId, tint: Color);

    /// Extracts any pending engine error out of the context.
    fn take_error(&mut self) -> Option<RedixelError>;
}

/// Read-only input queries for the current frame.
pub trait InputQuery<A: InputAction> {
    /// Returns `true` on the exact frame the action's bound input went down.
    fn just_pressed(&self, action: A) -> bool;

    /// Returns `true` every frame the action's bound input is held down.
    fn held(&self, action: A) -> bool;

    /// Returns `true` on the exact frame the action's bound input came up.
    fn just_released(&self, action: A) -> bool;

    /// Returns `true` if the action is down in any capacity.
    fn is_down(&self, action: A) -> bool {
        self.just_pressed(action.clone()) || self.held(action)
    }

    /// Returns `true` if a raw `KeyCode` is currently held, bypassing bindings.
    /// Useful for debug keys or engine-level shortcuts.
    fn key_held(&self, key: KeyCode) -> bool;

    /// Returns `true` if a raw `KeyCode` was just pressed this frame.
    fn key_just_pressed(&self, key: KeyCode) -> bool;

    /// Returns `true` if a raw `KeyCode` was just released this frame.
    fn key_just_released(&self, key: KeyCode) -> bool;

    /// Returns `true` if a raw `MouseButton` is currently held, bypassing bindings.
    fn mouse_held(&self, button: MouseButton) -> bool;

    /// Returns `true` if a raw `MouseButton` was just pressed this frame.
    fn mouse_just_pressed(&self, button: MouseButton) -> bool;

    /// Returns `true` if a raw `MouseButton` was just released this frame.
    fn mouse_just_released(&self, button: MouseButton) -> bool;

    /// Returns the current cursor position in surface pixels.
    /// Returns `None` if the cursor is outside the window.
    fn mouse_position(&self) -> Option<Vec2>;

    /// Returns the accumulated mouse scroll delta for the current frame.
    /// `x` represents horizontal scrolling, `y` represents vertical.
    fn scroll_delta(&self) -> Vec2;
}

/// Mutable binding configuration — call only in `on_start`.
pub trait InputBind<A: InputAction> {
    /// Binds an action to a source (Keyboard Key or Mouse Button).
    /// Multiple sources can share the same action.
    fn bind(&mut self, action: A, source: InputSource);

    /// Removes all bindings for `action`.
    fn unbind(&mut self, action: A);

    /// Removes all bindings entirely.
    fn clear_bindings(&mut self);
}
