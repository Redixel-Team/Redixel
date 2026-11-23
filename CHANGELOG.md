# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),  
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.7.0]

### Added

- **Core:** `profile`, an opt-in frame profiler behind the `profiling` feature, timing named phases of every frame from input through submit and present. Builds without the feature pay nothing.
- **Runtime:** profiling runs are configured through `REDIXEL_PROFILE*` environment variables, with no rebuild; the app writes a JSON report, prints `REDIXEL_PROFILE_DONE` to stderr and exits. See `docs/profiling.md`.
- **Renderer:** `Renderer::adapter_info` and `Renderer::present_mode`, the mode the surface actually runs with after any fallback.
- **Tooling:** `tools/profile-compare`, which diffs two profile reports phase by phase and says whether the second run is faster, slower or within noise.
- **Renderer:** rectangles and sprites draw as GPU instances, 36 bytes per quad expanded into its four corners by `quad.wgsl`, instead of four vertices and six indices built on the CPU. `Batch2d` keeps quads and triangles in submission order.
- **Runtime:** `RuntimeConfig::with_event_pump_interval` and `window.event_pump_interval_us`: frames run back to back for up to the interval (500 µs by default on desktop) before window events are dispatched, instead of one event-loop round trip per frame.
- **Platform:** `affinity::keep_current_thread_off_gpu_irq_cpus`. On Linux the runtime moves the frame thread off the CPUs servicing GPU interrupts (`RuntimeConfig::with_gpu_irq_avoidance`, `renderer.avoid_gpu_irq_cpus`, on by default), which a frame thread could otherwise share for a whole session at half speed.
- **Tooling:** `FPS_BENCH_QUADS` runs `fps-bench` on a single tier.
- **Renderer:** native targets render directly on `wgpu-hal` instead of `wgpu`, trying Vulkan, Metal, DX12 and GLES in that order (filtered by `renderer.backend`) and falling back to the next when one fails to start. Frames record ahead of the previous present, which runs on a dedicated `redixel-present` thread; Vulkan streams per-frame geometry from write-combined system RAM and reuses swapchain views. The web keeps `wgpu` (WebGPU, WebGL2).
- **Renderer:** `RendererConfig::present_spin` and `renderer.present_spin`: the frame thread busy-waits for at most 2 ms for the previous present instead of sleeping (on by default on desktop, off on Android and iOS).

### Changed

- **Runtime:** 2D draw calls fill a `Batch2d` the runtime swaps into the renderer each frame; `DrawCommand` loses its `Rect`, `Sprite` and `Triangle` variants. Breaking for code matching on them.
- **Renderer:** `SpriteBatch` is replaced by `Batch2d` (`DrawQueue::batch_2d`), `MeshBatch::new` no longer takes a device, and `Renderer::swap_batch_2d` hands over a filled batch without copying. Breaking for direct `redixel-renderer` consumers.
- **Runtime:** `RuntimeConfig` gains public `event_pump_interval` and `avoid_gpu_irq_cpus` fields. Breaking for code that builds it with a struct literal instead of `windowed`/`headless`.
- **Renderer:** the crate's modules are private; everything public is re-exported at the root under the same names (`redixel_renderer::Vertex` rather than `redixel_renderer::pipeline::Vertex`). The CPU-side types (`Vertex`, `GlobalUniforms`, `Batch2d`, `QuadInstance`, `RendererConfig`) no longer depend on `wgpu`, only on `wgpu-types`.
- **Runtime, Math, Tooling:** `Backends`, `PresentMode`, `AdapterInfo` and `Color` come from `wgpu-types`, the crate `wgpu` re-exports them from, so the types are unchanged. `redixel-runtime` and `fps-bench` no longer depend on `wgpu`, and `redixel-math`'s `wgpu` feature pulls in only `wgpu-types`.
- **Core:** `RedixelError::Gpu`, for failures reported as text by a graphics backend.
- **Renderer:** `DrawQueue`, `MeshBatch`, `ShapePipeline`, `TextureRegistry`, `UniformBlock` and `Renderer::draw_queue_mut` exist only on the web, where the renderer is still built on `wgpu`. `RendererConfig` gains a public `present_spin` field. Breaking for direct `redixel-renderer` consumers.
- **Tooling:** `fps-bench` measures each round for 2 seconds instead of 300 frames, which at thousands of FPS spanned a couple of display refreshes and swung with any compositor stall. It also reports the p99 frame time and the 1% low (`median_p99_frame_ms`, `median_low_1pct_fps`), and allocations per frame (`allocs_per_frame`, `bytes_per_frame`) instead of per window; `measure_frames` becomes `measure_ms`.
- **Core:** native builds no longer depend on `wgpu`; the renderer enables `redixel-core`'s `wgpu` feature, and with it the `wgpu` error variants, only on the web.

### Fixed

- **Runtime:** the FPS in the window title counts every frame of the last second, instead of averaging the last 60 frames, which at thousands of FPS spanned a few milliseconds and swung with any single stall.
- **Renderer:** the native present thread starts from the frame thread, so it inherits the frame thread's GPU interrupt avoidance instead of running on any CPU.
- **Renderer:** `wgpu-hal` 30.0.2, whose Vulkan swapchain no longer passes an unreset fence to every image acquire off Windows, which the validation layers reported on every frame.

## [0.6.0]

### Added

- **Renderer:** `GlobalUniforms`, the uniform block at `@group(0) @binding(0)`: `view` and `projection` matrices plus the frame globals `resolution` and `time`, rebuilt every `render` so no frame presents against a stale surface size. Its layout mirrors the WGSL struct field for field, so edit both together.
- **Runtime:** `GameContext::elapsed_time` and `TimeManager::elapsed_time`, seconds of real time since startup, in both the windowed and headless runtimes.
- **Audio:** `redixel-audio`, an optional crate behind the `audio` feature: `cpal` + `symphonia` natively (WAV and OGG Vorbis, decoded off the main thread, up to 32 sound effects at once), the Web Audio API on the web. Falls back to silent `NoOpAudio` when output can't start.
- **Audio:** on the web, sound starts on the page's first key press, click or touch; music started before then waits for it, sound effects are dropped.
- **Core:** `AudioManager`, `SoundId`, `AudioChannel`, `AudioConfig`, `MusicOptions` and `NoOpAudio`, available without the `audio` feature so audio calls need no `#[cfg]`.
- **Core:** `GameContext` gains `load_sound`/`load_sound_file`, `play_sound`/`play_sound_with`, `play_music`/`play_music_with` (looping, fade-in, crossfade), `stop_music`/`stop_music_fade`, and master/per-channel volume getters and setters. Volume and pitch are clamped, and a clip that fails to decode plays silently.
- **Runtime:** `AudioCommand`, buffered by `Context` and dispatched each frame like `DrawCommand`. A sound effect whose clip is still decoding after two seconds is dropped; music waits for its clip. Audio pauses while the app is suspended.
- **Runtime:** `RuntimeConfig::with_audio` and an `audio` section in `config.json`; volumes changed in-game are saved back on a clean exit (desktop only).
- **Runtime:** `EngineSettings::set_path`, `save`, `save_config_json` and `is_loaded`. A `config.json` that failed to load is never overwritten.
- **Examples:** `shooter` plays shot and hit effects and a looping music track.

### Changed

- **Core:** `GameContext` gains `elapsed_time` and the audio methods as required methods. Breaking for out-of-tree implementors.
- **Renderer:** `CameraUniform`/`Camera` are replaced by `GlobalUniforms`/`UniformBlock` (`camera_2d`/`camera_3d` → `globals_2d`/`globals_3d`), and `Renderer::render` takes the elapsed time in seconds. Breaking for direct `redixel-renderer` consumers.
- **Renderer:** the vertex shader transforms by `projection * view`; `view` is the identity for now, so output is unchanged.
- **Runtime:** `RuntimeConfig` gains a public `audio` field. Breaking for code that builds it with a struct literal instead of `windowed`/`headless`.
- **Runtime:** `serde_json` is built with `preserve_order`, so a saved `config.json` keeps its key order.
- **Platform:** the desktop window title shows the configured `app.name` instead of a hardcoded `"Redixel"`.
- **Renderer:** the 3D batch's GPU buffers are labelled `REDIXEL_MESH_BATCH_VB`/`_IB` instead of reusing the sprite batch's labels.
- **Examples:** `shooter` requires Android 8.0 (API level 26), the first release with the AAudio API `cpal` uses.
- **Examples:** `triangle`, `triangle_3d` and `sprite` quit on ESC like the other examples, and `triangle` sets its own clear colour.
- **Examples:** `shooter` times its powerup respawn, dash after-images and bullet trails with countdown clocks instead of `time % period < dt`, which misfired as the clock grew. The powerup returns ten seconds after pickup, as in `shooter_mp`.
- **Tooling:** formatting runs on nightly `rustfmt`, locally and in CI, so the options in `rustfmt.toml` apply. `builder` fails on an example whose `Cargo.toml` does not parse instead of silently skipping it.
- **Docs:** the README is an overview linking to new guides under `docs/` (getting started, web, Android, iOS, multiplayer, architecture), and the ROADMAP matches the code. `CONFIG.md`, the multiplayer guide, `CONTRIBUTING.md` and the crate-level docs are corrected and completed.

## [0.5.0]

### Added

- **Redixel:** `entry_point!`, a macro generating every platform entry point (desktop, Android, iOS, web) from one invocation, so a game boots everywhere without a `#[cfg]` or logger setup of its own. Optional `config:`, `desktop:` and `log_tag:` parameters cover an explicit `RuntimeConfig`, argv-driven desktop runs and the Android logcat tag (default `"REDIXEL_ENGINE"`).

### Changed

- **Examples:** every example boots through `redixel::entry_point!`, dropping its per-platform entry points and target-specific dependencies. `shooter_mp` keeps only its argv parsing and headless server (`src/native.rs`) and its fixed server address (`src/preset.rs`).
- **Redixel:** the crate owns the logging and wasm glue the entry points need, so a game depends on `redixel` alone. `env_logger` is now a dependency of every desktop consumer, headless servers included.
- **`shooter_mp`:** the desktop entry is named `desktop_main` like every other example's, not `native_main`.

### Fixed

- **Renderer:** the web build no longer crashes on browsers that expose `navigator.gpu` without a usable adapter (WebGPU disabled, blocklisted driver, no GPU); it falls back to WebGL2.

## [0.4.0]

### Added

- **Renderer:** texture support: PNG files (no other formats) decoded with the `image` crate and sampled with nearest filtering and clamp-to-edge, so pixel art stays crisp. Shapes and sprites share one batch, keeping their draw order, and a frame that never switches texture is still one draw call.
- **Renderer:** a failed texture load is never fatal: a corrupt file, unreadable path, oversized image or unknown handle draws as a magenta-and-black checkerboard and logs a warning.
- **Core:** `GameContext::load_texture(&[u8])` and, off the web, `load_texture_file(path)`, returning a `TextureId` at once while the upload happens later. Prefer `include_bytes!`; `load_texture_file` resolves paths against the working directory.
- **Core:** `GameContext::draw_sprite`/`draw_sprite_tinted` for 2D and `draw_triangle_3d_textured`/`draw_triangle_3d_textured_tinted` for 3D. The tint multiplies every texel, with `Color::WHITE` as the no-op.
- **Renderer:** a depth buffer, so 3D geometry is occluded by the GPU instead of sorted by hand.
- **Renderer:** `GameContext::draw_triangle_3d`/`Renderer::draw_triangle_3d` and `MeshBatch`, the 3D counterpart of `SpriteBatch`. The 3D camera is fixed and takes coordinates already in view space.
- **Renderer:** separate 2D and 3D pipelines: 3D is depth-tested (`LessEqual`, so coplanar geometry resolves in submission order) and drawn first, 2D lands on top in painter order. Nothing is back-face culled, so triangle winding doesn't matter.
- **Renderer:** `SpriteBatch::index_count()` and `unique_vertex_count()`.
- **Renderer:** the selected GPU adapter is logged at startup, showing whether a session runs on hardware or a software rasteriser.
- **Math:** `Mat4::perspective(fov_y_radians, aspect, near, far)`, a left-handed projection (camera at the origin looking down `+Z`) with WGPU's `[0, 1]` depth range; degenerate arguments are clamped instead of producing `NaN`.
- **Math:** `Vec3`, mirroring `Vec2`'s API, plus `cross`.
- **Math:** `Color::srgb()`/`Color::srgba()` and `Color::to_rgba8()`.
- **Examples:** `sprite`, a textured spinning crate with a row of 2D hearts showing tint and alpha.

### Changed

- **Core:** `GameContext` gains `draw_triangle_3d` and the six texture methods as required methods, and `DrawCommand` gains `Triangle3d`. Breaking for out-of-tree implementors and exhaustive matches; `DrawCommand` is now `#[non_exhaustive]`.
- **Renderer:** `Vertex.position` is `[f32; 3]` and `Vertex` gains `uv: [f32; 2]`, growing from 24 to 36 bytes. 2D calls are unaffected, but every app now also allocates a full-resolution depth buffer (~8 MB at 1080p).
- **Renderer:** breaking for direct `redixel-renderer` consumers:
  - `SpriteBatch` uses indexed draws (4 vertices and 6 indices per quad), and `vertex_count()` is replaced by `unique_vertex_count()`.
  - `SpriteBatch::flush()` takes a `&Device` and a `&TextureRegistry`, `MeshBatch::flush()` a `&TextureRegistry`, and `ShapePipeline::new()` the registry's `&BindGroupLayout`.
  - `MeshBatch::draw_triangle_3d_textured` takes `[Vec3; 3]`/`[Vec2; 3]` arrays instead of six arguments.
  - `DrawQueue.batch` is renamed `batch_2d`, alongside `batch_3d`.
  - The shape pipelines declare a texture and sampler bind group at index 1.
- **Renderer:** `SpriteBatch` no longer caps a frame's geometry; the fixed `MAX_QUADS` limit that silently dropped shapes is gone. Its buffers grow to fit and never shrink.
- **Core:** `wgpu` is optional, behind a `wgpu` feature carrying its error variants, so crates that don't render no longer pull in a graphics API. `RedixelError` is `#[non_exhaustive]`.
- **Math:** `wgpu` is optional, behind a `wgpu` feature carrying `From<Color> for wgpu::Color`.
- **Examples:** the `pong`, `shooter` and `shooter_mp` palettes are authored in sRGB; on-screen colours are unchanged. `Color::rgb()` takes linear components and is unaffected.
- **`shooter_mp`:** `Effect.color` is sent as sRGB bytes. A 0.3.x client and a 0.4.0 server still connect but disagree on effect brightness; set `NetConfig::protocol_id` to make that mismatch fail.
- **`tools/fps-bench`:** each tier reports a median with its range and spread over several interleaved rounds.
- **CI:** the benchmark comment shows run-to-run spread, and the job timeout is 15 minutes.

### Fixed

- **Math:** `Color::from_rgba8()` and `Color::from_hex()` read sRGB input as linear, so `#FF8000` rendered as roughly `#FFBC00`. They now convert from sRGB; alpha and the named constants are unaffected.
- **Math:** the `Color` docs no longer claim the shader does gamma correction; the sRGB surface does.
- **Docs:** `redixel-core` and `redixel-math` build their docs.rs pages with all features, so the `wgpu` items appear.

## [0.3.0]

### Added

- **Core:** a networking API:
  - `NetworkManager` via `ctx.network()`: connection events, reliable and unreliable send and broadcast, RTT and tickrate.
  - `ClientId`/`SERVER_ID`, `NoOpNetwork`, and `SequenceBuffer<T>`, a ring buffer for prediction and reconciliation.
  - `Game::on_fixed_update` and `GameContext::fixed_delta()`/`fixed_tick()`/`fixed_alpha()`, a fixed-step update decoupled from the render framerate.
- **Net:** `redixel-net`, behind the optional `net` feature:
  - `NetworkManager` over WebTransport (QUIC/HTTP-3): `wtransport` natively and `web-sys` in the browser, speaking one wire protocol.
  - Reliable framed streams and sequenced, fragmented, newest-wins unreliable datagrams, with bounded message sizes.
  - `NetConfig`/`NetMode`/`CertSource` (self-signed or PEM), a protocol-id handshake, and a `max_clients` limit with a shared handshake timeout.
  - Self-signed certificate hash pinning, so the browser client can trust a LAN server.
  - `LoopbackNetwork` for tests and single-process hosting.
- **Runtime:** headless mode: `RuntimeConfig::headless()` and `HeadlessRuntime` run the same `Game` as a dedicated server, with no `winit` or `wgpu`.
- **Runtime:** `EngineSettings::load_config_json()`, an `engine.tickrate` key in `config/config.json`, and `TimeManager::interpolation_alpha()`/`set_max_substeps()`.
- **Redixel:** `run_ios`/`run_ios_with`, exposed by each example as `ios_main`, and the prelude re-exports `CertSource`/`DEFAULT_PROTOCOL_ID`.
- **Examples:** `shooter_mp`, an authoritative-server twin-stick shooter (headless server; desktop, Android, iOS and browser clients) with client-side prediction and reconciliation, and buffered interpolation, without extrapolation, for remote players.
- **Examples:** every example adds `staticlib` to its `crate-type`.
- **Tooling:** `tools/fps-bench`, a CI benchmark of engine-side rendering cost that posts its results as a PR comment.
- **CI:** macOS in the desktop matrix, a new iOS job, and a reusable `cargo-each` action running every job against the root workspace and each example.

### Changed

- **Workspace:** examples are no longer root workspace members; each is its own workspace, targeted with `--manifest-path`.
- **Renderer:** `wgpu` v27 → v30, presenting through `queue.present()`.
- **Runtime:** `Runtime` and `HeadlessRuntime` share one fixed-step loop, `SimulationCore`.
- **Docs:** the README has updated example commands and new Multiplayer and Running on iOS sections.
- **CI:** the web deploy finds WASM examples by grepping `examples/*/Cargo.toml` instead of `cargo metadata`, and a shared `CARGO_TARGET_DIR` reuses build artifacts across example workspaces.

## [0.2.0]

### Added

- **Core:** `InputSource`, merging `KeyCode` and `MouseButton` bindings, and mouse queries on `InputQuery`: `mouse_just_pressed`, `mouse_held`, `mouse_just_released`, `mouse_position` and `scroll_delta`.
- **Runtime:** `RuntimeConfig`, passing the window, renderer and target-FPS settings into the engine explicitly.
- **Runtime:** `TimeManager::display_fps()`, a smoothed rolling average of the framerate.
- **Platform:** Android support (`aarch64-linux-android`): an `android_main` entry point, Logcat logging through `android_logger`, and suspend/resume handling that releases graphics resources in the background.
- **Platform:** the desktop, web and Android entry points share one `Game` implementation.
- **CI:** a workflow compiling the examples to WASM and syncing them to the frontend repository.

### Changed

- **Core:** `InputBind::bind` takes an `InputSource` instead of a `KeyCode`. Breaking for existing bindings.
- **Platform:** input events are buffered between frames, so a press and release inside one frame are both seen instead of producing phantom clicks.
- **Runtime:** `Runtime` no longer reads the global `EngineSettings`; `redixel::run` builds the `RuntimeConfig` and passes it in.
- **Runtime:** `every_seconds()` yields the smoothed `display_fps()` instead of the raw FPS, so FPS counters and window titles stop jittering. The simulation still uses the raw `delta_time()`.

## [0.1.0]

### Added

- **Workspace:** the engine as a Cargo workspace (`redixel-core`, `redixel-platform`, `redixel-renderer`, `redixel-runtime`, `redixel-math`) behind the `redixel` facade crate, started with `redixel::init()`, whose `redixel::prelude::*` is the one import a game needs.
- **Core:** the `Game` trait (`on_start`, `on_update`, `on_render`) and `GameContext`, the game's interface to the engine, with a buffered, deduplicated draw queue (`ClearColor`, `Rect`, `Triangle`).
- **Core:** type-safe input: a game-defined `InputAction`, `JustPressed`/`Held`/`JustReleased` states with OS key repeat filtered out, and separate `InputQuery`/`InputBind` traits.
- **Core:** `RedixelError`, one error type wrapping `winit`, `wgpu` and `web-sys` failures. Fatal errors are logged on desktop and thrown as exceptions in the browser instead of panicking.
- **Runtime:** the main loop on `winit::ApplicationHandler`, and `TimeManager` for delta time, FPS and target-framerate limiting through a hybrid sleep/spin wait, precise with little CPU.
- **Runtime:** `EngineSettings`, loading `config.json` with dot-notation lookups (`get_path("renderer.present_mode", ...)`) and mapping integer codes to `wgpu` backends and present modes. A missing or malformed file falls back to defaults with a logged error; `CONFIG.md` documents the schema.
- **Platform:** `WindowManager` and `InputManager` for the window lifecycle and keyboard, mouse and scroll input.
- **Renderer:** a WGPU renderer with clear-colour rendering, a `HighPerformance` adapter preference, automatic sRGB surface format and present-mode selection, and resize handling.
- **Math:** `Vec2`, `Mat4` (column-major, with orthographic projection) and `Color`.
- **Web:** WASM support: the engine injects its own `<canvas>` with no HTML or JavaScript setup, maps panics to the browser console and renders through WebGL.
- **Logging:** `log`, through `env_logger` on desktop and `console_log` on the web.
- **CI:** formatting and clippy checks, and tests of graphics code on CPU-emulated Vulkan. `rust-toolchain.toml` and `rustfmt.toml` pin the toolchain and formatting.
- **Docs:** `CONTRIBUTING.md`, `README.md` and `ROADMAP.md`.
