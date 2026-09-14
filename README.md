# Redixel

[![Online Playground](https://img.shields.io/badge/Online-Playground-orange?style=for-the-badge)](https://redixel-web.vercel.app/)

Redixel is a high-performance, strict 2D game engine written in Rust.

The primary goal of this project is to build a clean, modular, and scalable engine architecture from scratch. It enforces strict software engineering standards—such as unidirectional data flow and strong layer separation—to prevent the technical debt common in growing game engines.

## Technology Stack

Redixel is built on top of the modern Rust ecosystem, prioritizing safety and cross-platform compatibility (Desktop, Web, Android & iOS).

| Component        | Technology                              | Description                                                                                                                                                                                                   |
| :--------------- | :-------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Language**     | Rust (2024)                             | Memory safety and performance without garbage collection.                                                                                                                                                     |
| **Windowing**    | Winit                                   | Event loop management and low-level platform abstraction.                                                                                                                                                     |
| **Graphics**     | WGPU                                    | Portable graphics API targeting Vulkan, Metal, DX12, and WebGL/WebGPU.                                                                                                                                        |
| **Networking**   | WebTransport (`wtransport` / `web-sys`) | QUIC/HTTP-3 transport unifying reliable and unreliable delivery on one encrypted connection — native via `wtransport`, browser via `web-sys`'s `WebTransport` bindings, both speaking the same wire protocol. |
| **Build System** | Cargo                                   | Standard Rust package manager and build tool.                                                                                                                                                                 |

## Getting Started

### Prerequisites

Before you can build and run the project, you'll need to have the Rust compiler and Cargo (Rust's package manager and build tool) installed on your system. If you don't have Rust installed, follow these steps:

1. Visit [https://www.rust-lang.org/](https://www.rust-lang.org/) and follow the installation instructions for your operating system.

2. Install Rust directly via the command line:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. After installation, verify that Rust is installed by running:

   ```bash
   rustc --version
   ```

> This should output the installed version of the Rust compiler.

### Running Native (Windows/Linux/macOS)

To run the engine and the included `Shooter` game natively on your local machine:

```sh
cargo run --release --manifest-path examples/shooter/Cargo.toml
```

> **Note:** The `--release` flag compiles the engine with maximum optimizations, which is highly recommended to ensure stable framerates. For faster compilation times during development, you can omit this flag.

### Running on Web (WASM)

Redixel uses a pure-Rust pipeline for WebAssembly, requiring no manual HTML or JS files.

1.  **Install the WASM target and server runner:**

    ```sh
    rustup target add wasm32-unknown-unknown
    cargo install wasm-server-runner
    ```

2.  **Run the example:**
    ```sh
    cargo run --release --manifest-path examples/shooter/Cargo.toml --target wasm32-unknown-unknown
    ```

> This will automatically compile, generate bindings and start a local server at `http://127.0.0.1:1334`.

### WebGPU on Linux (Chromium-based browsers)

On Linux, **WebGPU is not fully enabled by default** on Chrome, Edge, Chromium, Opera, or Brave.
As documented in the official GPUWeb Implementation Status, Linux support is **behind flags**.

To run Redixel with WebGPU enabled on Linux, launch your browser with:

```sh
microsoft-edge \
  --enable-unsafe-webgpu \
  --ozone-platform=x11 \
  --use-angle=vulkan \
  --enable-features=Vulkan,VulkanFromANGLE
```

Or for Chrome/Chromium:

```sh
chromium \
  --enable-unsafe-webgpu \
  --ozone-platform=x11 \
  --use-angle=vulkan \
  --enable-features=Vulkan,VulkanFromANGLE
```

> Reference: [WebGPU Implementation Status](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status#implementation-status)

> **Note (Wayland):** These flags force XWayland, which can hurt performance. For best results, log into an **Xorg/X11 session** first.

### Running on Android

Redixel provides native support for Android, leveraging JNI to maintain high performance and hardware access.

1. **Install Android prerequisites:**
   Ensure you have the [Android SDK](https://developer.android.com/studio) and `cargo-apk` installed:

   ```sh
   cargo install cargo-apk
   ```

<details>
<summary><b>Show SDK & Environment Setup Details</b></summary>
   
To successfully build the APK, `cargo-apk` requires specific SDK packages and environment variables to be explicitly set.
   
**1. Android Studio SDK Manager**
Open Android Studio, navigate to **Tools > SDK Manager**, and install:

- **SDK Platforms:** Android API 33 or 34 (Android 13 or 14).
- **SDK Tools:**
  - Android SDK Build-Tools.
  - NDK (Side by side) - e.g., version `xx.x.xxxxxxx`.
  - Android SDK Command-line Tools (latest).

**2. Environment Variables & PATH**
You must set the SDK paths and ensure `adb` and `keytool` (Java) are accessible in your terminal.

_Linux/macOS_ (Add to `~/.bashrc` or `~/.zshrc`):

```sh
export ANDROID_HOME=$HOME/Android/Sdk
export NDK_HOME=$ANDROID_HOME/ndk/xx.x.xxxxxxx # Match your downloaded version
export PATH=$PATH:$ANDROID_HOME/platform-tools
```

> **Note:** Ensure you have a JDK installed (e.g., sudo apt install default-jdk).

_Windows_ (Add to System Environment Variables):

- `ANDROID_HOME`: `C:\Users\YOUR_USER\AppData\Local\Android\Sdk`
- `NDK_HOME`: `%ANDROID_HOME%\ndk\xx.x.xxxxxxx`
- Edit your `Path` variable and append these two directories:
- `%ANDROID_HOME%\platform-tools`
- `C:\Program Files\Android\Android Studio\jbr\bin` (Required for `keytool`)

**3. Accept SDK Licenses**
Make sure you have accepted all licenses (you can do this in Android Studio or by running `sdkmanager --licenses` in your terminal).

</details>

2. **Add the target:**

   ```sh
   rustup target add aarch64-linux-android
   ```

3. **Run on your device:**
   Before executing the command, ensure your phone has "Developer Options" enabled, turn on "USB Debugging", and accept the RSA key fingerprint prompt on your screen when connected via USB.

   ```sh
   cargo apk run --manifest-path examples/shooter/Cargo.toml --lib
   ```

> **Note:** Logs can be inspected in real-time using `adb logcat -s REDIXEL_ENGINE`.

### Manual Installation

If you prefer to install the compiled APK manually without using USB Debugging, you can build it and find the output in the `target` directory:

```sh
cargo apk build --manifest-path examples/shooter/Cargo.toml --lib
```

_The generated `.apk` will be located at `examples/shooter/target/debug/apk/shooter.apk`. You can transfer this file to your device and install it directly._

### Running on iOS

Redixel supports iOS through the same `Game`/`GameContext` API as every other platform, exposed via an `ios_main` entry point — the same idea as Android's JNI-loaded `android_main`, just embedded into an Xcode project instead of loaded by the OS directly.

> **Requires a Mac with Xcode.** Building for iOS needs Apple's proprietary SDK and frameworks (Foundation, UIKit, Metal), which only ship with Xcode — there is no cross-compilation path from Linux or Windows.

1. **Add the target:**

   ```sh
   rustup target add aarch64-apple-ios
   ```

2. **Build the library:**

   ```sh
   cargo build --release --manifest-path examples/shooter/Cargo.toml --target aarch64-apple-ios --lib
   ```

3. **Embed it in an Xcode project:** link the static library (`libshooter.a`, in `examples/shooter/target/aarch64-apple-ios/release/`) into an Xcode app target with no competing `main.swift`/`AppDelegate`, set `ios_main` as that target's entry point, then build/run from Xcode as usual.

> **Note:** Redixel doesn't generate the `.xcodeproj`/`.ipa` itself — that project lives in Xcode.

### Multiplayer

Redixel supports an **authoritative server** architecture via WebTransport (QUIC over UDP). The headless server owns all simulation; clients send input and render server snapshots. Works on LAN out of the box. For internet play, forward **UDP 5000** on the server's router.

```bash
# Server — binds to 0.0.0.0:5000 by default, or pass [bind_addr:port]
cargo run --release --manifest-path examples/shooter_mp/Cargo.toml -- server [bind_addr:port]

# Client — connects to 127.0.0.1:5000 by default, or pass [server_addr:port]
cargo run --release --manifest-path examples/shooter_mp/Cargo.toml -- client [server_addr:port]
```

> If clients on other machines can't connect, check that **UDP 5000** is open on the server's firewall.

**Cross-play:** every platform — Windows, Linux, macOS, Android, iOS, and the browser — shares the exact same `NetworkManager`/WebTransport wire protocol; only the transport binding differs (`wtransport` natively, `web-sys` in the browser). A headless server on Linux, a windowed client on Windows, and a client running in a browser tab can all join the same match interchangeably.

#### Playing from the browser

The browser client needs the WASM toolchain from [Running on Web](#running-on-web-wasm) installed first. Browsers cannot simply disable certificate validation the way native/mobile clients do, so connecting to a self-signed LAN server requires **pinning its certificate hash**:

1. **Start the server** and copy the hash it logs on startup:

   ```bash
   cargo run --release --manifest-path examples/shooter_mp/Cargo.toml -- server [bind_addr:port]
   # WebTransport server certificate hash (sha-256): <64 hex characters>
   ```

2. **Paste it** into `SERVER_CERT_HASH_HEX` in `examples/shooter_mp/src/lib.rs` (the `wasm` module), replacing the `"SERVER_CERT_HASH_HEX"` placeholder. If the server binds to anything other than `127.0.0.1:5000`, also update `SERVER_ADDR` in the same file — the browser client has no argv, so the address is fixed at build time.

3. **Run the client:**

   ```bash
   cargo run --release --manifest-path examples/shooter_mp/Cargo.toml --target wasm32-unknown-unknown
   ```

> Restart the server and the pinned hash goes stale — a fresh self-signed certificate is generated each run, so re-copy it and rebuild the client. Connecting to a CA-trusted `server_name` deployment needs no pinning: leave `SERVER_CERT_HASH_HEX` empty.

## Architecture

The engine adheres to a strict **Layered Architecture** divided into isolated crates within a Cargo Workspace. Dependencies flow downwards; circular dependencies between crates are strictly forbidden to ensure modularity.

### 1\. Public API Facade (`redixel`)

The outermost layer. It re-exports the essential types and traits from the internal crates via a unified `prelude`, providing a clean and ergonomic interface for the end-user.

### 2\. Runtime Layer (`redixel-runtime`)

The "Brain" of the engine. It implements the `winit::ApplicationHandler` trait.

- **Responsibility:** Orchestrates the application lifecycle (Initialization, Update Loop, Render Loop, Shutdown) and manages the `TimeManager`.
- **Behavior:** Owns the sub-systems and safely routes OS events to the platform and graphics modules.

### 3\. Graphics Layer (`redixel-renderer`)

Abstracts the GPU hardware via WGPU.

- **Renderer:** Encapsulates the WGPU Instance, Surface, Device, Queue, and Render Pipeline.
- **Capability:** Manages the swapchain, render passes, clear colors, and command encoding.

### 4\. Platform Layer (`redixel-platform`)

Abstracts Operating System specifics, ensuring the core engine remains platform-agnostic.

- **Window Manager:** Handles window creation, lifecycle events, and safe suspension/resumption natively and on WebAssembly.
- **Input Manager:** Decouples raw OS input events from game logic, sanitizing key states and pointer data.

### 5\. Core Layer (`redixel-core`)

The structural foundation of the engine framework.

- **Responsibility:** Defines the main lifecycles (`Game` trait), the shared execution context (`GameContext`), the primitive rendering command queues, and the centralized error system (`RedixelError`).
- **Dependencies:** Relies on `redixel-math` for type definitions, but remains completely agnostic of runtime, windowing, or graphics implementations.

### 6\. Mathematics Layer (`redixel-math`)

The absolute bedrock utility crate of the entire workspace.

- **Responsibility:** Implements pure, zero-dependency linear algebra operations, including vectors (`Vec2`), transformation matrices (`Mat4`), projection calculations, and color representations (`Color`).
- **Behavior:** Completely isolated from engine systems and game logic, ensuring optimal compilation times and maximum portability.

## Directory Structure

```text
redixel/
├── Cargo.toml                  # Workspace Root Configuration
├── crates/
│   ├── redixel-core/           # Base types, errors, no heavy dependencies
│   ├── redixel-math/           # Linear algebra (Vec2, Mat4, Color)
│   ├── redixel-platform/       # Winit: window, input, web-sys DOM injection
│   ├── redixel-renderer/       # Wgpu: GPU device, render pass, commands
│   ├── redixel-runtime/        # Loop, AppState, TimeManager, Settings
│   ├── redixel-net/            # NetworkManager transports (WebTransport, loopback)
│   └── redixel/                # Public facade API (pub use ...)
└── examples/
    ├── pong/                   # Classic 2D game demonstrating input and physics
    ├── shooter/                # 2D top-down shooter with AI, weapons, and particles
    ├── shooter_mp/             # Authoritative-server multiplayer shooter
    ├── sprite/                 # Texture loading and UV-mapped 3D geometry
    ├── triangle/               # Basic 2D rendering example
    └── triangle_3d/            # Basic 3D rendering and camera example
```

## Roadmap

For a detailed breakdown of upcoming features, including Batch Rendering, ECS, and Physics, please refer to the [ROADMAP](./ROADMAP.md).

## Contributing

To maintain architectural integrity, we enforce strict coding standards and a specific development workflow. Please read our [CONTRIBUTING](./CONTRIBUTING.md) guide before opening a Merge Request.

## License

Redixel is distributed under the terms of the Apache License (Version 2.0).

See [LICENSE](./LICENSE) for details.

Copyright 2026 Redixel Core Team.
