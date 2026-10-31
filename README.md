# Redixel

[![Online Playground](https://img.shields.io/badge/Online-Playground-orange?style=for-the-badge)](https://redixel-web.vercel.app/)

Redixel is a high-performance 2D & 3D game engine written in Rust, with a strict layered architecture targeting desktop, web, Android and iOS.

The primary goal of this project is to build a clean, modular, and scalable engine architecture from scratch. It enforces strict software engineering standards—such as unidirectional data flow and strong layer separation—to prevent the technical debt common in growing game engines.

**Highlights:** batched 2D rendering with textured sprites · depth-buffered 3D rendering with perspective projection · WebGPU with a WebGL2 fallback in the browser · authoritative multiplayer over WebTransport, with cross-play between native and browser clients · one `entry_point!` that boots the same game on every platform.

## Technology Stack

| Component        | Technology                              | Description                                                                                                                                                                                                   |
| :--------------- | :-------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Language**     | Rust (2024)                             | Memory safety and performance without garbage collection.                                                                                                                                                     |
| **Windowing**    | Winit                                   | Event loop management and low-level platform abstraction.                                                                                                                                                     |
| **Graphics**     | WGPU                                    | Portable graphics API targeting Vulkan, Metal, DX12, and WebGL/WebGPU.                                                                                                                                        |
| **Networking**   | WebTransport (`wtransport` / `web-sys`) | QUIC/HTTP-3 transport unifying reliable and unreliable delivery on one encrypted connection — native via `wtransport`, browser via `web-sys`'s `WebTransport` bindings, both speaking the same wire protocol. |
| **Build System** | Cargo                                   | Standard Rust package manager and build tool.                                                                                                                                                                 |

## Quick Start

With [Rust](https://www.rust-lang.org/) installed, run the `Shooter` example natively:

```sh
cargo run --release --manifest-path examples/shooter/Cargo.toml
```

Or in the browser (after `rustup target add wasm32-unknown-unknown` and `cargo install wasm-server-runner`):

```sh
cargo run --release --manifest-path examples/shooter/Cargo.toml --target wasm32-unknown-unknown
```

See [Getting Started](./docs/getting-started.md) for the full setup.

## Examples

| Example                                 | Description                                            |
| :-------------------------------------- | :----------------------------------------------------- |
| [`pong`](./examples/pong)               | Classic 2D game demonstrating input and collision      |
| [`shooter`](./examples/shooter)         | 2D top-down shooter with AI, weapons, and particles    |
| [`shooter_mp`](./examples/shooter_mp)   | Authoritative-server multiplayer shooter               |
| [`sprite`](./examples/sprite)           | Textured 3D crate with tinted 2D sprites drawn over it |
| [`triangle`](./examples/triangle)       | Basic 2D rendering example                             |
| [`triangle_3d`](./examples/triangle_3d) | Rotating 3D tetrahedron with perspective and depth     |

Most of them also run in the [online playground](https://redixel-web.vercel.app/).

## Documentation

| Guide                                        | Covers                                                      |
| :------------------------------------------- | :---------------------------------------------------------- |
| [Getting Started](./docs/getting-started.md) | Installing Rust and running natively (Windows/Linux/macOS)  |
| [Web (WASM)](./docs/platforms/web.md)        | Browser builds and enabling WebGPU on Linux                 |
| [Android](./docs/platforms/android.md)       | SDK/NDK setup, running on a device, building the APK        |
| [iOS](./docs/platforms/ios.md)               | Building the static library and embedding it in Xcode       |
| [Multiplayer](./docs/multiplayer.md)         | Dedicated server, clients, cross-play, browser cert pinning |
| [Architecture](./docs/architecture.md)       | Crate layers and directory structure                        |
| [Configuration](./config/CONFIG.md)          | `config.json` options                                       |
| [Roadmap](./ROADMAP.md)                      | What is done and what is planned, phase by phase            |
| [Changelog](./CHANGELOG.md)                  | Release history                                             |

## Contributing

To maintain architectural integrity, we enforce strict coding standards and a specific development workflow. Please read our [CONTRIBUTING](./CONTRIBUTING.md) guide before opening a Merge Request.

## License

Redixel is distributed under the terms of the Apache License (Version 2.0).

See [LICENSE](./LICENSE) for details.

Copyright 2026 Redixel Core Team.
