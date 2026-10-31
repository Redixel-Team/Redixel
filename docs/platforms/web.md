# Running on Web (WASM)

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

> **Audio:** browsers keep audio blocked until the page receives a user gesture, so sound starts on the first key press, click or touch. Music started before then begins at that moment; sound effects played before then are dropped.

## WebGPU on Linux (Chromium-based browsers)

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
