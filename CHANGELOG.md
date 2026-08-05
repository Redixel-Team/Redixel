# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),  
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.6.0]

### Added

- **Renderer:** `GlobalUniforms`, the uniform block at `@group(0) @binding(0)` — `view` and `projection` matrices plus the frame globals `resolution`, `time`, and `glow`. Replaces `CameraUniform`, which carried only a projection matrix. Laid out field-for-field against the WGSL struct (144 bytes, no implicit padding), with `min_binding_size` pinned so any drift between the two declarations fails at pipeline creation instead of reading garbage.
- **Renderer:** `Renderer::update_uniforms(time)` writes both uniform blocks, driven from inside `render` so no frame can present against a stale surface size.
- **Runtime:** `TimeManager::elapsed_time` and `GameContext::elapsed_time` — seconds of real time since startup, previously unavailable in any form. Accumulated once per frame in `accumulate` (covering the windowed and headless runtimes alike); the spiral-of-death clamp never discards it.
- **Renderer + Core:** `set_glow(amount)` — a time-driven brightness modulation the shader applies to the 3D batch, `0.0` (the default) disabling it, so existing games render exactly as before. Each fragment's phase derives smoothly from its position: moving geometry shimmers instead of strobing, waves roll across large surfaces, and separate objects pulse out of step at no per-object cost. The 2D batch always renders with it off.
- **Renderer + Core:** `draw_triangle_3d_shaded(points, colors)` — a 3D triangle with an independent colour per vertex, interpolated across the face. The batch's `Vertex` always stored a colour per vertex; this exposes it, enabling Gouraud shading and soft alpha gradients.
- **Examples:** `solar_system` — a limb-darkened sun in layered halos, three Gouraud-shaded planets on tilted orbits (one with a moon, one with a soft-edged double ring), an asteroid belt, feathered orbit lines, and a twinkling star field over nebula clouds. Positions and lighting are CPU work driven by `elapsed_time`; every brightness flicker runs on the GPU from the `time` uniform via `set_glow`.

### Changed

- **Renderer:** `Renderer::render` takes the elapsed time in seconds, narrowed to `f32` only at the GPU boundary.
- **Renderer:** the group 0 bind group layout is visible to both shader stages (`VERTEX_FRAGMENT`, previously `VERTEX`), since the fragment stage now reads the frame globals.
- **Renderer:** `CameraUniform`/`Camera` replaced by `GlobalUniforms`/`UniformBlock` (`camera_2d`/`camera_3d` → `globals_2d`/`globals_3d`) — the block is no longer only a camera. Breaking for direct `redixel-renderer` consumers.
- **Renderer:** the vertex shader transforms by `projection * view`; `view` is the identity today, so output is unchanged — the multiply is the seam a camera system (planned) slots into.

## [0.5.0]

### Added

- **Redixel:** `entry_point!`, a declarative macro that generates every platform entry point from a single invocation — `desktop_main`, the unmangled `android_main` the native-activity glue resolves by symbol, the `extern "C" ios_main` an Xcode target is pointed at, the `#[wasm_bindgen(start)]` function the browser runs on module instantiation, and the `fn main()` the native binary calls. A game now boots on all four platforms without writing a single `#[cfg]`, `extern "C"`, `#[wasm_bindgen(start)]`, or logger initialisation of its own. Optional parameters cover the cases the one-liner cannot express: `config:` for an explicit `RuntimeConfig`, `desktop:` to replace the desktop run step (argv-dependent games choosing between hosting a server and joining one), and `log_tag:` for the Android logcat tag, which was hardcoded to `"REDIXEL_ENGINE"` in every example before.

### Changed

- **Examples:** every example boots through `redixel::entry_point!`. Each `lib.rs` lost the ~45-line block of per-platform entry points it carried — byte-for-byte identical across `pong`, `triangle`, `triangle_3d`, `sprite`, and `shooter` — each `main.rs` is now a three-line call with no `#[cfg]`, and each `Cargo.toml` dropped its three `[target.'cfg(...)'.dependencies]` blocks. `shooter_mp` keeps only what is genuinely its own: argv parsing and the headless server (`src/native.rs`), and the fixed server address the argv-less platforms need (`src/preset.rs`).
- **Redixel:** the crate owns the platform glue the entry points need — `env_logger` on desktop, `android_logger` on Android, `console_log`/`console_error_panic_hook`/`wasm-bindgen` on the web — target-gated and re-exported through a hidden module, so a game depends on `redixel` alone. The cost is that `env_logger` is now in the dependency graph of every desktop consumer, including headless servers.
- **`shooter_mp`:** the desktop entry is named `desktop_main` like every other example's, not `native_main`.

### Fixed

- **Renderer:** the web build no longer dies on browsers that expose `navigator.gpu` without a usable adapter behind it — WebGPU disabled by flag, blocklisted driver, or no GPU at all. WGPU picks its wasm backend from the mere presence of `navigator.gpu` and never falls back, and WGPU 30 reads `requestAdapter()`'s `null` through a `JsOption` that only counts `undefined` as empty, so the null adapter was accepted and the first property read on it threw a JS `TypeError` no Rust code could catch. The renderer now resolves `requestAdapter()` itself before building the instance and drops `Backends::BROWSER_WEBGPU` when it comes back empty, leaving WebGL2 as the candidate instead of a crash.

## [0.4.0]

### Added

- **Renderer:** texture support. PNG files are decoded with the `image` crate (PNG only — `default-features = false` keeps the JPEG/GIF/TIFF/WebP decoders out of every binary), uploaded to `Rgba8UnormSrgb` textures, and sampled through one shared `FilterMode::Nearest` / `ClampToEdge` sampler, so pixel art stays crisp at any magnification. The sRGB format matches the surface, which the shader relies on to encode on write-out since it applies no gamma correction of its own.
- **Renderer:** solid shapes and sprites share one pipeline, one shader, and one batch. The fragment shader is unconditionally `textureSample(...) * color`, and untextured geometry is bound against a 1×1 opaque white texture that makes the multiply an identity. Splitting sprites into their own batch would have split their painter order too — a sprite drawn after a rectangle has to land on top of it, and separate batches cannot promise that.
- **Renderer:** batches record _runs_ — contiguous stretches of the index buffer sharing one texture — and issue one `draw_indexed` per run, coalescing consecutive primitives so a frame that never switches texture stays exactly one draw call. Only the draw is split; the whole frame's geometry still travels in two `write_buffer` calls.
- **Renderer:** a failed texture load is never fatal. A corrupt file, an unreadable path, an image past the device's `max_texture_dimension_2d`, or a handle that was never uploaded all draw as a magenta-and-black checkerboard and log a warning. The size bound goes to the decoder rather than being checked afterwards, since an oversized `create_texture` raises a validation error that WGPU's default handler turns into a panic.
- **Core:** `GameContext::load_texture(&[u8])` and, off the web, `load_texture_file(path)`, returning a `TextureId`. The handle comes back immediately but the decode and upload are deferred to the runtime, since the renderer is not reachable from the context and a headless server has no GPU to upload to. Prefer `include_bytes!`; `load_texture_file` resolves paths against the working directory, the same caveat that applies to `config/config.json`.
- **Core:** `GameContext::draw_sprite` / `draw_sprite_tinted` for 2D, and `draw_triangle_3d_textured` / `draw_triangle_3d_textured_tinted` for 3D. The tint multiplies every sampled texel — its colour modulates the image and its alpha multiplies the image's own — with `Color::WHITE` as the no-op.
- **Examples:** `sprite` — a PNG-textured crate spinning head-on, every face UV-mapped from the same image and occluded by the depth buffer, with a row of 2D hearts over it showing the image untouched, colour-modulated, alpha-faded, and both at once. Their transparent border shows the alpha channel being honoured rather than the sprite's bounding box being painted.
- **Renderer:** `SpriteBatch::index_count()` and `unique_vertex_count()` — the draw call size and the number of distinct vertices, which indexed drawing makes two different figures.
- **Renderer:** the selected GPU adapter is logged at startup, making it visible whether a session ran on hardware or on a software rasteriser. Browsers withhold adapter identity to limit fingerprinting, so the fields WebGPU leaves blank are substituted or omitted rather than logged as empty text.
- **Renderer:** a GPU depth buffer — a `Depth32Float` texture whose view is recreated alongside the swap chain on every resize, since WGPU requires every attachment in a pass to share one resolution. 3D geometry is occluded by the GPU rather than sorted by hand.
- **Renderer:** `GameContext::draw_triangle_3d` / `Renderer::draw_triangle_3d`, and a `MeshBatch` that accumulates 3D triangles the way `SpriteBatch` accumulates 2D shapes. `MeshBatch` is a separate type so that queueing a pixel-space rectangle into the perspective batch is a compile error instead of a rendering mystery. The 3D camera is fixed: field of view, near, and far live in the renderer, and `draw_triangle_3d` takes coordinates already in view space.
- **Renderer:** two shape pipelines over one shader, one vertex layout, and one bind group layout. The 3D pipeline tests and writes depth (`depth_compare: LessEqual`, which keeps coplanar geometry resolving in submission order); the 2D pipeline does neither, since 2D content is painter-ordered by definition and a flat overlay writing depth would occlude the scene behind it. 3D is flushed first, so 2D lands on top and its alpha blends against the rendered scene rather than the clear colour. Nothing is back-face culled — `draw_triangle_3d` accepts its corners in any order, so culling would silently discard triangles wound the "wrong" way.
- **Math:** `Mat4::perspective(fov_y_radians, aspect, near, far)`, a left-handed perspective projection (camera at the origin looking down `+Z`) using the same `[0, 1]` WGPU depth-range convention as `orthographic`. Degenerate arguments — a zero `aspect` from a window reporting no height, a `far` at or below `near` — are clamped rather than trusted, since each divides by zero and a `NaN` reaching the uniform buffer blanks the frame without failing anywhere visible.
- **Math:** `Vec3` — a 3-component float vector mirroring `Vec2`'s arithmetic, `dot`, `length`, `normalise`, `splat`, `abs`, `min`, `max`, and `lerp`, plus `cross`, which has no 2D counterpart.
- **Math:** `Color::srgb()`/`Color::srgba()`, the float counterparts to `from_rgba8()`, and `Color::to_rgba8()`, its inverse.

### Changed

- **Renderer:** `SpriteBatch` submits indexed draw calls (`pass.draw_indexed`) — 4 unique vertices + 6 `u32` indices per quad, instead of 6 duplicated vertices.
- **Renderer:** `Vertex.position` grew from `[f32; 2]` to `[f32; 3]`, and the shape shader now reads a real `vec3<f32>` position instead of hardcoding `z = 0.0` — the depth test needs something to compare. `draw_rect`/`draw_triangle` are unaffected; they still take `Vec2` and now just carry `z = 0.0` through to the GPU. The cost lands on 2D games too: 28 bytes per vertex instead of 24, plus a full-resolution depth attachment (4 bytes per pixel, ~8 MB at 1080p) allocated whether or not anything 3D is drawn.
- **Renderer:** `DrawQueue.batch` is now `DrawQueue.batch_2d`, alongside `batch_3d`.
- **Core:** `GameContext` gained `draw_triangle_3d` as a required method, and `DrawCommand` gained a `Triangle3d` variant. Both break code outside the engine that implements the trait or matches the enum exhaustively. `DrawCommand` is now `#[non_exhaustive]`, so future primitives stop being breaking changes.
- **Core:** `GameContext` gained six more required methods — `load_texture`, `load_texture_file`, `draw_sprite`, `draw_sprite_tinted`, `draw_triangle_3d_textured`, `draw_triangle_3d_textured_tinted` — breaking code outside the engine that implements the trait.
- **Renderer:** `Vertex` gained a `uv: [f32; 2]` field at `@location(2)`, taking a vertex from 28 to 36 bytes. Untextured geometry carries `[0.0, 0.0]` and lands on the white pixel's only texel, so `draw_rect`/`draw_triangle`/`draw_triangle_3d` are unaffected at the call site and unchanged on screen.
- **Renderer:** the shape pipelines declare a second bind group at index 1 (texture + sampler, fragment stage), whose layout the texture registry owns because it also owns every bind group built against it. Both pipelines declare it, so the 3D path binds one too — `MeshBatch` tags untextured geometry as such and resolves to the white pixel, leaving 3D output unchanged.
- **Renderer:** `SpriteBatch::flush()` and `MeshBatch::flush()` take a `&TextureRegistry`, and `ShapePipeline::new()` takes the registry's `&BindGroupLayout`. `MeshBatch::draw_triangle_3d_textured` takes its corners and UVs as `[Vec3; 3]`/`[Vec2; 3]` arrays rather than six positional arguments.
- **Renderer:** `SpriteBatch` no longer caps how much geometry a frame may queue, replacing the fixed `MAX_QUADS` ceiling that silently dropped excess rectangles and triangles. Buffers start empty, grow to fit whatever is drawn, and never shrink.
- **Renderer:** `SpriteBatch::flush()` takes a `&Device` alongside the `&Queue`, and `vertex_count()` is gone in favour of `unique_vertex_count()` — the figure changed from 6 values per quad to 4, so the rename turns a silent behaviour break into a compile error.
- **Math:** `wgpu` is optional, behind a `wgpu` feature carrying the `From<Color> for wgpu::Color` conversion. Without it the crate has no dependencies at all, against 59 with it.
- **Core:** `wgpu` is optional, behind a `wgpu` feature carrying the four error variants that wrap `wgpu` failure types. Without it the crate resolves 78 dependencies instead of 118, so the `Game`, input, and networking surface no longer forces a graphics API on consumers that do not render. `RedixelError` is `#[non_exhaustive]`, since which variants exist depends on a feature Cargo unifies across the whole dependency graph.
- **Examples:** the `pong`, `shooter`, and `shooter_mp` palettes were re-authored in sRGB. The rendered result is unchanged — every value was replaced with the colour already reaching the screen — but the 8-bit literals now match the on-screen pixels. Converted at each origin rather than at the draw calls, so colours travelling through particle systems or over the wire stay consistent. The `Color::rgb()` literals for grids, clear colours, and health bars take linear components and are untouched.
- **`shooter_mp`:** `Effect.color` crosses the wire as sRGB bytes. The example still handshakes on `DEFAULT_PROTOCOL_ID`, so a 0.3.x client and a 0.4.0 server still accept each other and disagree on the brightness of every hit, death, and impact effect. Setting `NetConfig::protocol_id` is what makes the mismatch fail loudly.
- **`tools/fps-bench`:** tiers are interleaved and measured over several rounds, reported as a median with its observed range and spread instead of one figure each. Warmup is whichever comes later, 500 ms or 30 frames.
- **CI:** the benchmark comment reports the measured spread, making it visible when an FPS delta is smaller than the run-to-run noise. The job timeout rises to 15 minutes to fit the added rounds.

### Fixed

- **Math:** `Color::from_rgba8()` and `Color::from_hex()` treated 8-bit and hex inputs as linear, but such values are sRGB-encoded by convention. They now apply the sRGB transfer function, so a colour authored as `#FF8000` renders as `#FF8000` instead of roughly `#FFBC00`, and alpha is correctly left un-encoded. The named constants are unaffected, sitting at the extremes where both spaces coincide.
- **Math:** the `Color` documentation claimed gamma correction happened "at the shader level". The shape shader applies none, and must not: the renderer selects an sRGB surface format and the GPU performs the encoding on write.
- **Docs:** `redixel-core` and `redixel-math` build their `docs.rs` pages with all features. Neither has default features, so published documentation would otherwise omit the `wgpu` error variants and `From<Color> for wgpu::Color`.

## [0.3.0]

### Added

- **Networking Core (`redixel-core::net`):**
  - `NetworkManager` trait exposed via `ctx.network()` — connection events, reliable/unreliable send and broadcast, RTT, tickrate.
  - `ClientId`/`SERVER_ID`, `NoOpNetwork`, and `SequenceBuffer<T>` (a ring buffer for prediction/reconciliation).
  - `Game::on_fixed_update` plus `GameContext::fixed_delta()`/`fixed_tick()`/`fixed_alpha()`, driven by a deterministic accumulator decoupled from render framerate.
- **`redixel-net` crate:**
  - `NetworkManager` over WebTransport (QUIC/HTTP-3) — `wtransport` natively, `web-sys` in the browser, both speaking the same wire protocol.
  - Reliable framed streams and sequenced, fragmented, newest-wins unreliable datagrams, with bounded frame/fragment/message sizes.
  - `NetConfig`/`NetMode`/`CertSource` (self-signed or PEM), a protocol-id handshake, and `max_clients` admission with a shared handshake timeout.
  - Self-signed certificate hash pinning, for the browser client to trust a LAN server.
  - `LoopbackNetwork` for tests and single-process hosting.
- **Headless / Dedicated Server mode:** `RuntimeConfig::headless()` + `HeadlessRuntime` run the engine with no `winit`/`wgpu` — the same `Game` runs unmodified as a dedicated server.
- **Multiplayer example (`shooter_mp`):**
  - Authoritative-server twin-stick shooter demo — headless server, desktop/Android/iOS/browser clients.
  - Reliable channel for one-shot effects, unreliable for continuous world state.
  - Client-side prediction and reconciliation for local player movement.
  - Delay-buffered interpolation for remote agents, absorbing snapshot jitter/loss without extrapolating.
- **`redixel::prelude`** now re-exports `CertSource`/`DEFAULT_PROTOCOL_ID`.
- **`net` Cargo feature:** `redixel-net` is optional, feature-gated in `redixel`/`redixel-runtime`.
- `EngineSettings::load_config_json()` and a new `engine.tickrate` key in `config/config.json`.
- `TimeManager::interpolation_alpha()` and `set_max_substeps()`.
- **`tools/fps-bench`:** CI benchmark tool for engine-side rendering cost, with a dedicated `benchmark` job posting results as a PR comment.
- **CI:** `macOS` added to the `Desktop` matrix; new `iOS` job; reusable `cargo-each` composite action running every job against the root workspace and each `examples/*`.
- `staticlib` added to every example's `crate-type`.
- **iOS entry point:** `redixel::run_ios`/`run_ios_with`, exposed per example as `ios_main`.

### Changed

- **Workspace restructuring:** examples are no longer root workspace members — each is its own standalone workspace, targeted via `--manifest-path`.
- **`redixel-runtime` internals:** shared fixed-step loop extracted into `SimulationCore`, used by both `Runtime` and `HeadlessRuntime`.
- `README.md`: updated example commands, new **Multiplayer** and **Running on iOS** sections.
- `deploy-frontend.yml`: WASM example discovery now via `grep` over `examples/*/Cargo.toml` instead of `cargo metadata`.
- **Graphics Backend Upgrade:** `wgpu` v27 → v30, migrated to `queue.present()`.
- **CI/CD Pipeline:** global `CARGO_TARGET_DIR` shares compiled artifacts across example workspaces.

## [0.2.0]

### Added

- **Unified InputSource System**:
  - Introduced the `InputSource` enum in `redixel-core` to seamlessly merge `KeyCode` and `MouseButton` bindings.
  - Added native mouse querying methods to `InputQuery` (`mouse_just_pressed`, `mouse_held`, `mouse_just_released`).
  - Added support for real-time cursor position tracking (`mouse_position`) and mouse wheel delta accumulation (`scroll_delta`).
- **Automated Web Deployment Pipeline:**
  - Implemented CI/CD workflow to compile examples into WASM and sync artifacts to the frontend repository.
- **Runtime:** Introduced the `RuntimeConfig` struct to explicitly inject Window, Renderer, and target FPS settings into the engine.
- **Time:** Introduced `display_fps()` to `TimeManager`, which calculates a smoothed rolling average of the framerate using a zero-allocation, fixed-size ring buffer.
- **Android Support:** - Integrated `android_logger` for native Logcat integration.
  - Implemented platform-specific entry point via `#[unsafe(no_mangle)] android_main`.
  - Added JNI-based lifecycle management (suspend/resume) ensuring graphics resource safety.
  - Configured `Cargo.toml` targets for `aarch64-linux-android` support.
- **Cross-Platform Architecture:**
  - Standardized entry points for PC, Web, and Android to ensure platform-agnostic `Game` trait implementation.
  - Optimized memory management for mobile constraints using `Box::into_raw` and reborrowing strategies.

### Changed

- **Event Loop Cascading**: Streamlined the main event loop using the _Chain of Responsibility_ pattern, safely delegating OS events between the `Context` and `WindowManager`.
- **InputManager Refactoring & Double Buffering**:
  - `InputBind::bind` now accepts a unified `InputSource` instead of a raw `KeyCode`, altering the public API.
  - Implemented a robust **Double Buffering** system with event queues (`pending_keys`, `pending_mouse`) to prevent dropped OS events.
  - Added a **Deferral Strategy** within the `tick()` lifecycle to completely eliminate "Phantom Clicks" (rapid press/release events within the same frame are now safely buffered and executed across frames).
- **Runtime Architecture Overhaul:** Refactored the core runtime module for better testability and maintainability:
  - Decoupled `Runtime` from the global `EngineSettings` singleton by introducing explicit dependency injection via the new `RuntimeConfig` struct.
  - Extracted the massive core game loop and rendering pipeline into a dedicated, clean `run_frame()` method.
  - Made event delegation explicit in `on_window_event` by replacing implicit match guards with clear `if` statements and early returns.
- **Core:** The main composition root (`redixel::run`) now handles reading the global state and assembling the `RuntimeConfig` prior to engine startup.
- **Tests:** Refactored `runtime.rs` unit tests to use mocked configurations (`mock_config()`), allowing them to run in parallel without shared global state.
- **Time:** The `every_seconds()` callback now yields the smoothed `display_fps()` instead of the raw, instantaneous FPS. This prevents UI counters and window titles from jittering rapidly due to OS context switching, while keeping the internal game physics strictly tied to the raw `delta_time()`.

## [0.1.0]

### Added

- `CONTRIBUTING.md` guide enforcing strict coding styles, environment setup, and CI workflow.
- Initial project structure for the **Redixel Engine**.
- Engine bootstrap (`main.rs`, `lib.rs`) with `redixel::init()` entry point.
- **Runtime system** implementing `winit::ApplicationHandler`, orchestrating:
  - Event processing
  - Surface creation
  - Redraw requests
- **Platform layer**:
  - `WindowManager` for window creation, lifecycle handling, and redraw requests.
  - `InputManager` for basic input event dispatch (keyboard, mouse wheel, pointer movement).
- **Graphics layer**:
  - Implemented **`RendererDevice`**, handling:
    - WGPU instance creation (`Instance`)
    - Surface creation from a `winit` window
    - Adapter selection with `HighPerformance` preference
    - Device & queue creation via `request_device`
    - Automatic surface format and present-mode selection
    - Surface configuration (`SurfaceConfiguration`) including SRGB format detection
  - Implemented **`Renderer`**, providing:
    - Clear-color rendering pipeline (basic render pass)
    - Swapchain acquisition (`get_current_texture`)
    - Command encoder creation & submission
    - Resize handling that updates surface configuration
    - Presentation of rendered frames
- **Web Assembly (WASM) Support**:
  - Enabled `wasm32-unknown-unknown` target support.
  - Integrated `wasm-bindgen` for JavaScript interoperability.
  - Added `console_error_panic_hook` for mapping Rust panics to the browser console.
  - Enabled `wgpu`'s `webgl` feature flag for broad browser compatibility.
  - Implemented DOM manipulation logic to attach the `winit` window to the HTML Canvas.
- **Engine module layout** (`engine`, `runtime`, `platform/input`, `platform/window`, `graphics/renderer`, `graphics/renderer_device`).
- CI pipeline (`.github/workflows/ci.yml`) including toolchain bootstrap, fmt, and clippy checks.
- Repository configuration files (`rust-toolchain.toml`, `rustfmt.toml`).
- **Error Handling System**:
  - Implemented a centralized `RedixelError` enum using `thiserror` to capture and contextually wrap errors from `winit`, `wgpu`, and `web-sys`.
  - Added robust error propagation across the runtime, enabling graceful shutdown on failure.
  - Integrated `log` crate with `env_logger` (Desktop) and `console_log` (WASM) for structured logging and debugging.
- **TimeManager and Limiting**:
  - Implemented `TimeManager` for precise frame timing, delta-time calculation, and performance monitoring.
  - Added a high-precision **hybrid sleep/spin-lock** mechanism to enforce target framerates with minimal CPU overhead.
- **Configuration System**:
  - Implemented **`EngineSettings`** as a thread-safe global singleton (`OnceLock`, `RwLock`) enabling concurrent access from any thread.
  - Integrated `serde` and `serde_json` for robust parsing of external `config.json` files with automatic error recovery and logging.
  - Added a generic `get_path<T>` utility for querying nested settings using dot-notation strings (e.g., `"renderer.present_mode"`).
  - Implemented logic to map integer configuration values directly to `wgpu` enums (Backend, PresentMode).
  - Added `CONFIG.md` documentation comprehensively detailing the `app`, `window`, and `renderer` schemas and their default behaviors.
- **Unit Tests for Core Logic:** Implemented comprehensive unit tests across key engine components:
  - **`Runtime`**: Verifies core state management, fatal error capture, and reliable asynchronous communication channel (MPSC bridge) operation.
  - **`TimeManager`**: Validates FPS calculation accuracy, frame limiting precision, correct target duration conversion, and reliable interval callback triggering.
  - **`InputManager`**: Confirms accurate event filtering to distinguish between valid player inputs (Keyboard, Pointer, Scroll) and system events.
  - **`WindowManager`**: Ensures precise FPS title formatting and correct event filtering logic for window-specific events (e.g., Focus, Scaling).
- **Continuous Integration (CI) Enhancements:**
  - Integrated essential Linux graphics dependencies (`xvfb` and `mesa-vulkan-drivers`) to enable integration testing of graphics-dependent code via CPU-emulated Vulkan.
- **Math Library (`redixel-math`)**:
  - Implemented core linear algebra structures: `Vec2`, `Mat4`, and `Color`.
  - Added logic for orthographic projections, matrix multiplication (column-major for GPU), vector normalization, and lerping.
- **Type-Safe Input System**:
  - Upgraded `InputManager` with a generic, zero-overhead `InputAction` trait.
  - Implemented strict state machine tracking (`JustPressed`, `Held`, `JustReleased`).
  - Added OS-level key repeat filtering and decoupled read/write access via `InputQuery` and `InputBind` traits.
- **Game API & Context (`redixel-core`)**:
  - Introduced the main `Game` trait (`on_start`, `on_update`, `on_render`).
  - Implemented `GameContext` to expose a safe, unified interface to the user without exposing internal dependencies.
  - Created the `DrawCommand` queue to buffer rendering primitives (`ClearColor`, `Rect`, `Triangle`) with intelligent deduplication logic.
- **Engine Prelude**:
  - Added `redixel::prelude::*` to drastically improve Developer Experience (DX) and streamline imports for game developers.

### Changed

- Updated `LICENSE` copyright to "Redixel Core Team".
- Updated `README.md` with professional formatting and architecture overview.
- Updated `ROADMAP.md` to reflect the current technical status of Phase 1 and next infrastructure steps.
- Refactored core initialization modules (`WindowManager::new`, `Renderer::new`, `Runtime`) to return `Result<T, RedixelError>`, eliminating fragile `unwrap()` and `expect()` calls in critical paths.
- Updated Application Entry Points:
  - **Desktop (`main`)**: Now returns `Result` and prints formatted fatal errors to `stderr` via the logging system.
  - **WASM (`init`)**: Now implements `From<RedixelError>` for `JsValue`, ensuring Rust errors are correctly mapped and displayed as exceptions in the Browser Console.
- **Architectural Overhaul (Cargo Workspace):** Migrated the monolithic structure into a strict Cargo Workspace (`redixel-core`, `redixel-platform`, `redixel-renderer`, `redixel-runtime`).
- **Public API Facade:** Introduced the `redixel` crate to act as a clean, unified public API for end-users, hiding internal complexity.
- **Pure Rust WebAssembly:** Eliminated external `index.html` and JavaScript bindings setup. The engine now dynamically injects the `<canvas>` into the DOM and enforces styling purely via Rust (`web-sys`).
- Updated `winit` Web API integration to safely build `WindowAttributesWeb` without relying on deprecated trait extensions.
- Removed dead code (e.g., `SetLoggerError` from engine error variants) to ensure the framework remains unopinionated about the consumer's logging setup.
- **Internal API Encapsulation**: Refactored internal crates (`redixel-runtime`, `redixel-platform`) to use private modules and surgical `pub use` exports, preventing namespace pollution and protecting internal structures.
