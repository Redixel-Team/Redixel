# Architecture

The engine adheres to a strict **Layered Architecture** divided into isolated crates within a Cargo Workspace. Dependencies flow downwards; circular dependencies between crates are strictly forbidden to ensure modularity.

## 1\. Public API Facade (`redixel`)

The outermost layer. It re-exports the essential types and traits from the internal crates via a unified `prelude`, providing a clean and ergonomic interface for the end-user.

- **Entry Point:** `redixel::entry_point!` generates every platform entry point (desktop, Android, iOS, and web) from a single invocation, so a game never writes its own `#[cfg]` bootstrap.

## 2\. Runtime Layer (`redixel-runtime`)

The "Brain" of the engine. It implements the `winit::ApplicationHandler` trait.

- **Responsibility:** Orchestrates the application lifecycle (Initialization, Update Loop, Render Loop, Shutdown) and manages the `TimeManager`.
- **Behavior:** Owns the sub-systems and safely routes OS events to the platform and graphics modules.
- **Commands:** Buffers the game's draw calls as `DrawCommand`s and texture loads as `TextureRequest`s, then dispatches them to the renderer each frame.
- **Headless Mode:** `HeadlessRuntime` drives only the fixed-update loop, with no window or GPU, so the same `Game` runs unmodified as a dedicated server.

## 3\. Graphics Layer (`redixel-renderer`)

Abstracts the GPU hardware. Natively it is built directly on `wgpu-hal`, trying Vulkan, Metal, DX12 and GLES in that order (filtered by `renderer.backend`); on the web it runs on `wgpu`, over WebGPU or WebGL2.

- **Shared CPU side:** `Batch2d` keeps rectangles and sprites as 36-byte GPU instances and triangles as indexed vertices, in submission order; 3D triangles accumulate in an indexed geometry queue. Each run of consecutive draws sharing a texture is one draw call. `GlobalUniforms` carries the `view`/`projection` matrices, resolution, and time to the shaders. None of this depends on a GPU API.
- **Pipelines:** a depth-tested 3D pipeline (`Depth32Float`), and painter-ordered 2D triangle and instanced-quad pipelines drawn over it.
- **Native frame loop:** two frames in flight on one monotonic fence; per-frame streaming buffers (write-combined system RAM on Vulkan); uploads and encoding start before the previous present is joined; presents run on a dedicated `redixel-present` thread; swapchain views are cached per image on Vulkan.
- **Textures:** PNGs decode to `Rgba8UnormSrgb` and upload synchronously; untextured geometry samples a white pixel, and a failed load draws a checkerboard.

## 4\. Platform Layer (`redixel-platform`)

Abstracts Operating System specifics, ensuring the core engine remains platform-agnostic.

- **Window Manager:** Handles window creation, lifecycle events, and safe suspension/resumption natively and on WebAssembly.
- **Input Manager:** Decouples raw OS input events from game logic, sanitizing key states and pointer data.

## 5\. Networking Layer (`redixel-net`, optional `net` feature)

Implements the `NetworkManager` trait defined in `redixel-core`.

- **Transports:** `WebTransportServer`/`WebTransportClient` natively via `wtransport`, a browser client via `web-sys`'s `WebTransport` bindings, and `LoopbackNetwork` for tests and single-process hosting. All of them speak the same wire protocol.

## 6\. Audio Layer (`redixel-audio`, optional `audio` feature)

Implements the `AudioManager` trait defined in `redixel-core`.

- **Backends:** natively, `cpal` drives the output device and `symphonia` decodes WAV and OGG Vorbis, mixed by the engine's own multi-voice mixer; in the browser, the Web Audio API via `web-sys`. Both degrade to a silent no-op when output cannot start.

## 7\. Core Layer (`redixel-core`)

The structural foundation of the engine framework.

- **Responsibility:** Defines the main lifecycles (`Game` trait), the shared execution context (`GameContext`), the input action bindings, the `NetworkManager` and `AudioManager` traits, texture and sound handles (`TextureId`, `SoundId`), and the centralized error system (`RedixelError`). It also hosts the opt-in frame profiler (`profile`, behind the `profiling` feature), which the runtime and renderer mark phases through without depending on each other; see [Frame Profiling](./profiling.md).
- **Dependencies:** Relies on `redixel-math` for type definitions, and on `winit` only for its input types (`KeyCode`, `MouseButton`) and window errors. It knows nothing of the runtime or the renderer: the `wgpu` error variants sit behind an optional `wgpu` feature, enabled only by the web renderer.

## 8\. Mathematics Layer (`redixel-math`)

The absolute bedrock utility crate of the entire workspace.

- **Responsibility:** Implements pure linear algebra operations, with no dependencies unless the optional `wgpu` feature (the `Color` → `wgpu::Color` conversion) is enabled, including vectors (`Vec2`, `Vec3`), transformation matrices (`Mat4`) with orthographic and perspective projections, and color representations (`Color`).
- **Behavior:** Completely isolated from engine systems and game logic, ensuring optimal compilation times and maximum portability.

## Directory Structure

```text
redixel/
├── Cargo.toml                  # Workspace Root Configuration
├── config/                     # Default config.json and its reference (CONFIG.md)
├── crates/
│   ├── redixel-core/           # Game/GameContext traits, input, errors, net trait
│   ├── redixel-math/           # Linear algebra (Vec2, Vec3, Mat4, Color)
│   ├── redixel-platform/       # Winit: window, input, web-sys DOM injection
│   ├── redixel-renderer/       # wgpu-hal (native) / wgpu (web): pipelines, batches, textures
│   ├── redixel-runtime/        # Loop, AppState, TimeManager, Settings
│   ├── redixel-net/            # NetworkManager transports (WebTransport, loopback)
│   ├── redixel-audio/          # AudioManager backends (cpal+symphonia, Web Audio API)
│   └── redixel/                # Public facade API (pub use ..., entry_point!)
├── docs/                       # Platform, multiplayer and architecture guides
├── examples/
│   ├── pong/                   # Classic 2D game demonstrating input and collision
│   ├── shooter/                # 2D top-down shooter with AI, weapons, and particles
│   ├── shooter_mp/             # Authoritative-server multiplayer shooter
│   ├── sprite/                 # Textured 3D crate with tinted 2D sprites drawn over it
│   ├── triangle/               # Basic 2D rendering example
│   └── triangle_3d/            # Rotating 3D tetrahedron with perspective and depth
└── tools/
    ├── builder/                # Generates the games manifest for the web playground
    └── fps-bench/              # Frame-time and allocation benchmark run in CI
```
