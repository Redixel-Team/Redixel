# **Redixel Roadmap**

Phases are grouped by area, not a strict sequence: work lands where it is needed, so later phases can be further along than earlier ones. Items link to their story on the [project board](https://github.com/orgs/Redixel-Team/projects/1) where one exists, and partially done items stay unchecked with a note on what is missing.

## **Phase 1 — Foundation & Lifecycle**

**Goal:** Establish the OS communication, main loop, user entry point, and platform stability.

- [x] **Project Structure:** Established a strict Cargo Workspace architecture (`redixel-core`, `math`, `platform`, `renderer`, `runtime`, `net`, and the `redixel` facade) enforcing unidirectional dependency flow to support future expansion.
- [x] **Window Creation:** Implemented `WindowManager` with `winit` to handle OS events and cross-platform window initialization (Native/WASM).
- [x] **Renderer Module:** Initialized the `wgpu` context (Instance, Surface, Device) and established the basic Clear Color render pass.
- [x] **Error Handling:** Unified system (`RedixelError`) and graceful propagation (Main/WASM).
- [x] **Time Management:** Delta Time calculation and FPS Counter.
- [x] **Game Loop & User API:** Implement `Game` trait (`on_update`, `on_render`) and `Context`.
- [x] **Basic Window Control:** Clean exit on window close, fullscreen as a startup setting, and cleanup.
- [ ] **Runtime Window Control:** Toggle fullscreen while running, and an engine-level exit binding (ESC is bound by each example today).
- [x] **Logging:** Standardize logs across the engine (`log` crate).
- [x] **Testing Infrastructure:** Establish unit testing patterns and ensure core logic (Time, Config) is covered by `cargo test`.
- [x] **Engine Configuration:** Startup settings struct (Window title, Resolution, VSync, Backend).
- [ ] **Strict Settings Typing:** Remove the global settings singleton in favour of typed configuration ([#56]).

## **Phase 2 — The Graphics Core**

**Goal:** A usable, data-driven rendering API shared by the 2D and 3D renderers.

- [x] **Vertex & Index Buffers:** `Vertex` carries position, colour, and UV; geometry is uploaded through growable vertex and index buffers and drawn indexed ([#40]).
- [x] **Math Library:** Custom Linear Algebra module (`Vec2`, `Vec3`, `Mat4`, orthographic and perspective projections, `Color`).
- [ ] **Math Extensions:** `look_at`, rotations around X/Y, matrix inverse, and quaternions — needed by a 3D camera and model transforms.
- [x] **Shaders & Uniforms:** `GlobalUniforms` passes the `view` and `projection` matrices, resolution, and time to the shaders ([#41]).
- [x] **Depth Buffer:** `Depth32Float` depth testing resolves 3D occlusion per pixel; 2D content stays painter-ordered, drawn over the depth-tested scene ([#68]).
- [ ] **Camera System:** World-to-screen transformation for both renderers — an orthographic 2D camera and a perspective 3D camera — plugged into the `projection * view` seam the shader already has; `view` is the identity today ([#42]).
- [ ] **Render Targets / Framebuffers:** Create intermediate textures to allow offscreen rendering, Pixel-Perfect scaling, and Post-Processing ([#44]).
- [x] **Texture Support:** PNG decoding via the `image` crate, uploaded to `Rgba8UnormSrgb` textures sampled with `FilterMode::Nearest`. Alpha is blended, and a failed load draws a checkerboard instead of failing ([#43]).
- [ ] **Mipmaps:** Generate a mip chain per texture. `FilterMode::Nearest` with no mips keeps pixel art crisp at native scale and above, but a sprite minified below its native size aliases and shimmers as it moves.
- [ ] **Materials & Custom Shaders:** Let games supply their own shaders and materials instead of the single shared `shape.wgsl` ([#84]).
- [ ] **Renderer Backend:** Migrate the renderer to `wgpu-hal` ([#88]).

## **Phase 3 — The 2D Renderer**

**Goal:** Efficiently draw thousands of sprites (The "Draw Call" problem).

- [ ] **Sprite Struct:** Define the data structure for visual objects (Pos, Size, Rotation, UVs). _(partial: `draw_sprite` takes position, size, texture, and tint — rotation and UV regions for sprite sheets/atlases are missing)_
- [x] **Batch Renderer:** `SpriteBatch` groups a whole frame's sprites and shapes into one vertex and index buffer, issuing one draw call per run of consecutive draws sharing a texture ([#35]).
- [ ] **Text & Font Renderer:** Integrate `ab_glyph` to generate font atlases and draw dynamic text via the Batch Renderer ([#72]).
- [ ] **Particle System:** Implement a lightweight engine-level structure to process and render thousands of ephemeral quads efficiently. The `shooter` and `shooter_mp` examples each carry their own copy today.
- [ ] **Primitive Rendering:** Implement logic to draw debug shapes (lines, wireframe rectangles) for physics visualization.

## **Phase 4 — The 3D Renderer**

**Goal:** Move from immediate-mode triangles to real 3D scenes.

- [x] **3D Triangles:** `MeshBatch` draws flat-coloured and textured 3D triangles through a perspective projection, occluded by the depth buffer.
- [ ] **Meshes & Model Transforms:** Upload indexed meshes once and transform them on the GPU with a per-model matrix, instead of re-sending CPU-transformed vertices every frame.
- [ ] **Lighting:** Vertex normals and basic shading ([#71]).
- [ ] **Model Loading:** Load 3D models from glTF ([#70]).
- [ ] **GPU Instancing:** Draw many copies of one mesh in a single draw call ([#69]).
- [ ] **Back-Face Culling & MSAA:** Cull hidden faces and smooth geometry edges.

## **Phase 5 — Input & Camera Control**

**Goal:** Decouple OS events from Game Logic.

- [x] **Input Abstraction:** Create an "Action Mapping" system (bind "Jump" to "Space" or "A button").
- [x] **Mouse Integration & Double Buffering:** Unified Action bindings for Keyboard and Mouse, preventing phantom clicks with a deferred state machine.
- [x] **Input State Machine:** Track "Just Pressed," "Held," and "Just Released" states manually.
- [ ] **Gamepad Integration:** Integrate `gilrs` to support USB/Bluetooth controllers alongside keyboard mappings.
- [ ] **Touchscreen Support:** Map touch events (especially for WASM/Mobile environments) seamlessly into the Input Manager. _(partial: a single touch acts as the left mouse button — no multitouch or gestures)_
- [ ] **Coordinate Conversion:** Convert screen coordinates (pixels) to world coordinates, and unproject screen points into 3D rays.

## **Phase 6 — Platforms**

**Goal:** Run the same game, unmodified, everywhere.

- [x] **Desktop:** Windows, Linux, and macOS through `winit` and `wgpu`.
- [x] **Web (WASM):** Pure-Rust WebAssembly pipeline, WebGPU with a WebGL2 fallback when the browser has no usable adapter ([#81]).
- [x] **Android:** Native activity via `winit` and `wgpu` ([#49]).
- [x] **iOS:** `ios_main` entry point embedded into an Xcode project.
- [x] **Entry Point Macro:** `redixel::entry_point!` generates every platform entry point from one invocation ([#80]).
- [ ] **Surface Loss:** Recover the surface after Alt-Tab on Linux ([#62]).

## **Phase 7 — ECS (Entity Component System)**

**Goal:** Define how game objects are structured and updated ([#76]).

- [ ] **Component Storage:** Implement a custom storage architecture (e.g., Sparse Sets or Archetypes) for high-performance data access.
- [ ] **Entity Management:** ID generation and recycling logic.
- [ ] **System Dispatcher:** Logic to iterate over components and run update loops.
- [ ] **Transform Hierarchy:** Implement local/global matrix calculations to support Parent-Child entity relationships (e.g., a sword attached to a player's hand).
- [ ] **Game State Management:** Create a robust State Machine to handle Scene transitions (Menu -> Gameplay -> Pause -> Game Over).

## **Phase 8 — Core Systems & Resources**

**Goal:** Managing memory and assets efficiently.

- [ ] **Asset Manager:** Implement a caching system to load resources once and reference them by ID/Handle ([#77]). _(partial: textures return an opaque `TextureId` — caching, deduplication, unloading, and a generic `Handle<T>` are missing)_
- [ ] **Async Asset Loading:** Implement non-blocking resource fetching (Promises/Futures) to prevent WASM thread freezing during heavy I/O ([#78]). _(partial: sounds decode off the main thread — a decoder thread natively, `decodeAudioData` on the web — but textures still decode on it)_
- [ ] **Scene Management:** Define a custom file format for saving/loading level data.
- [x] **Audio Engine:** `redixel-audio` (optional `audio` feature) mixes decoded PCM behind the `AudioManager` trait — `cpal` with the engine's own multi-voice mixer natively, the Web Audio API on the web. `GameContext::play_sound`/`play_music` cover one-shot effects and looping, crossfading music; master and per-channel volume persist through `config.json`. Positional/3D audio stays out of scope until a 3D game needs it ([#90]).

## **Phase 9 — Physics**

**Goal:** Movement and Collision logic, 2D first; 3D physics follows once the 3D renderer has meshes and transforms.

- [ ] **AABB Collision:** Implement Axis-Aligned Bounding Box intersection math.
- [ ] **Collision Resolution:** Implement separating axis logic to prevent object overlap.
- [ ] **Raycasting & Triggers:** Implement Line-of-Sight checks and ghost/sensor collisions without physical resolution.
- [ ] **Spatial Partitioning:** Implement a Quadtree or Grid to optimize collision checks.

## **Phase 10 — Developer Tools (UI)**

**Goal:** Runtime inspection and debugging.

- [ ] **Debug UI:** Implement a simple immediate-mode text/button renderer for changing variables at runtime.
- [ ] **Hot-Reloading:** Enable updating of Textures, Shaders, and configs on-the-fly without restarting the engine.
- [ ] **Profiler:** Implement internal timers to measure function execution speed and memory usage ([#89]). _(partial: `tools/fps-bench` measures frame times and allocations in CI, outside the engine)_

## **Phase 11 — Networking (Multiplayer)**

**Goal:** Enable real-time multiplayer with a simple, data-driven API, supporting both Desktop and Web (WASM).

- [x] **Transport Layer:** Implemented an agnostic, backend-swappable network layer behind the `NetworkManager` trait (`redixel-core::net`), with `redixel-net` providing a native **WebTransport (QUIC/HTTP-3)** backend and a `LoopbackNetwork` for tests and single-process hosting ([#63]).
- [x] **Headless Mode:** `RuntimeConfig::headless()` + `HeadlessRuntime` run the engine with no `winit`/`wgpu`, driving only the fixed-update loop — the same `Game` implementation runs unmodified as a dedicated server.
- [x] **Fixed Update Loop:** `on_fixed_update` runs on a deterministic accumulator shared by the windowed and headless runtimes, decoupled from render framerate.
- [x] **Network API:** `ctx.network()` on `GameContext` exposes inbound connection/message events, reliable and unreliable send/broadcast, and connection/tickrate queries to game code.
- [x] **Browser (WASM) Transport:** `redixel-net::wasm` implements the same `NetworkManager` over `web-sys`'s `WebTransport` bindings, speaking the identical wire protocol as the native backend — desktop, Android, iOS, and browser clients all join the same match. Self-signed servers are trusted via certificate hash pinning (`NetConfig::with_server_cert_hash`), since browsers cannot disable certificate validation the way native/mobile clients do ([#64]).
- [ ] **State Serialization:** Still open — `NetworkManager` moves opaque byte payloads with no built-in (de)serialization; a reusable engine-level utility is a candidate for a future story.

[#35]: https://github.com/Redixel-Team/Redixel/issues/35
[#40]: https://github.com/Redixel-Team/Redixel/issues/40
[#41]: https://github.com/Redixel-Team/Redixel/issues/41
[#42]: https://github.com/Redixel-Team/Redixel/issues/42
[#43]: https://github.com/Redixel-Team/Redixel/issues/43
[#44]: https://github.com/Redixel-Team/Redixel/issues/44
[#49]: https://github.com/Redixel-Team/Redixel/issues/49
[#56]: https://github.com/Redixel-Team/Redixel/issues/56
[#62]: https://github.com/Redixel-Team/Redixel/issues/62
[#63]: https://github.com/Redixel-Team/Redixel/issues/63
[#64]: https://github.com/Redixel-Team/Redixel/issues/64
[#68]: https://github.com/Redixel-Team/Redixel/issues/68
[#69]: https://github.com/Redixel-Team/Redixel/issues/69
[#70]: https://github.com/Redixel-Team/Redixel/issues/70
[#71]: https://github.com/Redixel-Team/Redixel/issues/71
[#72]: https://github.com/Redixel-Team/Redixel/issues/72
[#76]: https://github.com/Redixel-Team/Redixel/issues/76
[#77]: https://github.com/Redixel-Team/Redixel/issues/77
[#78]: https://github.com/Redixel-Team/Redixel/issues/78
[#80]: https://github.com/Redixel-Team/Redixel/issues/80
[#81]: https://github.com/Redixel-Team/Redixel/issues/81
[#84]: https://github.com/Redixel-Team/Redixel/issues/84
[#88]: https://github.com/Redixel-Team/Redixel/issues/88
[#89]: https://github.com/Redixel-Team/Redixel/issues/89
[#90]: https://github.com/Redixel-Team/Redixel/issues/90
