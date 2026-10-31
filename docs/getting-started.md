# Getting Started

## Prerequisites

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

> **Linux only:** with the `audio` feature enabled (the `Shooter` game enables it), the native audio backend links against ALSA, so install its development headers before building:
>
> ```sh
> # Debian/Ubuntu
> sudo apt-get install libasound2-dev
>
> # Fedora
> sudo dnf install alsa-lib-devel
> ```

## Running Native (Windows/Linux/macOS)

To run the engine and the included `Shooter` game natively on your local machine:

```sh
cargo run --release --manifest-path examples/shooter/Cargo.toml
```

> **Note:** The `--release` flag compiles the engine with maximum optimizations, which is highly recommended to ensure stable framerates. For faster compilation times during development, you can omit this flag.

## Next Steps

All commands in these guides are run from the repository root.

- [Running on Web (WASM)](./platforms/web.md)
- [Running on Android](./platforms/android.md)
- [Running on iOS](./platforms/ios.md)
- [Multiplayer](./multiplayer.md)
- [Configuration](../config/CONFIG.md)
