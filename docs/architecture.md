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

Abstracts the GPU hardware via WGPU.

- **Renderer:** Encapsulates the WGPU Instance, Surface, Device, and Queue, with two render pipelines sharing one shader: a depth-tested 3D pipeline (`Depth32Float`) and a painter-ordered 2D pipeline drawn over it.
- **Batching:** `SpriteBatch` (2D) and `MeshBatch` (3D) gather a whole frame's geometry into one vertex and index buffer, issuing one draw call per run of consecutive draws that share a texture.
- **Uniforms & Textures:** `GlobalUniforms` carries the `view`/`projection` matrices, resolution, and time to the shaders; `TextureRegistry` owns the uploaded PNG textures.
- **Capability:** Manages the swapchain, render passes, clear colors, and command encoding.

## 4\. Platform Layer (`redixel-platform`)

Abstracts Operating System specifics, ensuring the core engine remains platform-agnostic.

- **Window Manager:** Handles window creation, lifecycle events, and safe suspension/resumption natively and on WebAssembly.
- **Input Manager:** Decouples raw OS input events from game logic, sanitizing key states and pointer data.

## 5\. Networking Layer (`redixel-net`, optional `net` feature)

Implements the `NetworkManager` trait defined in `redixel-core`.

- **Transports:** `WebTransportServer`/`WebTransportClient` natively via `wtransport`, a browser client via `web-sys`'s `WebTransport` bindings, and `LoopbackNetwork` for tests and single-process hosting. All of them speak the same wire protocol.

## 6\. Core Layer (`redixel-core`)

The structural foundation of the engine framework.

- **Responsibility:** Defines the main lifecycles (`Game` trait), the shared execution context (`GameContext`), the input action bindings, the `NetworkManager` trait, texture handles (`TextureId`), and the centralized error system (`RedixelError`).
- **Dependencies:** Relies on `redixel-math` for type definitions, but remains completely agnostic of runtime, windowing, or graphics implementations.

## 7\. Mathematics Layer (`redixel-math`)

The absolute bedrock utility crate of the entire workspace.

- **Responsibility:** Implements pure, zero-dependency linear algebra operations, including vectors (`Vec2`, `Vec3`), transformation matrices (`Mat4`) with orthographic and perspective projections, and color representations (`Color`).
- **Behavior:** Completely isolated from engine systems and game logic, ensuring optimal compilation times and maximum portability.

## Directory Structure

```text
redixel/
├── Cargo.toml                  # Workspace Root Configuration
├── config/                     # Default config.json and its reference (CONFIG.md)
├── crates/
│   ├── redixel-core/           # Base types, errors, no heavy dependencies
│   ├── redixel-math/           # Linear algebra (Vec2, Vec3, Mat4, Color)
│   ├── redixel-platform/       # Winit: window, input, web-sys DOM injection
│   ├── redixel-renderer/       # Wgpu: GPU device, 2D/3D pipelines, batches, textures
│   ├── redixel-runtime/        # Loop, AppState, TimeManager, Settings
│   ├── redixel-net/            # NetworkManager transports (WebTransport, loopback)
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
