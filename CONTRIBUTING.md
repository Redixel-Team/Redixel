## Contributing to Redixel

To ensure your code passes our CI/CD pipeline, you must configure your local environment correctly.

### 1\. Rust Analyzer & Clippy

We strictly enforce **Clippy** lints in our Continuous Integration (CI) pipeline. If your code generates Clippy warnings, **the Pull Request will fail**.

To prevent this, you **must** configure your editor to use `clippy` as the default check command instead of `cargo check`.

**VS Code Configuration (`settings.json`):**

```json
{
  "[rust]": { "editor.defaultFormatter": "rust-lang.rust-analyzer" },
  "rust-analyzer.check.command": "clippy"
}
```

This ensures you see the same errors locally that the CI will report remotely.

### 2\. Formatting (nightly `rustfmt`)

Our `rustfmt.toml` uses options that only the **nightly** `rustfmt` understands (import grouping and merging, struct field alignment), so formatting runs on nightly while the code itself builds on stable. The CI's format check runs `cargo +nightly fmt -- --check`.

Install it once:

```sh
rustup toolchain install nightly --profile minimal --component rustfmt
```

Then format with:

```sh
cargo +nightly fmt
```

Each example is its own Cargo workspace, so format them too: `cargo +nightly fmt --manifest-path examples/<name>/Cargo.toml`.

To format on save in VS Code, point rust-analyzer at the nightly formatter:

```json
{
  "rust-analyzer.rustfmt.extraArgs": ["+nightly"]
}
```

## Code Style & Conventions

We enforce a strict coding style that differs slightly from standard Rust defaults, particularly regarding imports and type definitions.

### 1\. Import Ordering & Grouping

`cargo +nightly fmt` arranges imports for you: one `use` per crate (merged with `{ ... }`), sorted alphabetically, in three blocks separated by a blank line.

1.  **Standard Library** (`std::...`, `core::...`)
2.  **Other Crates** (external and the workspace's own `redixel_*` crates alike)
3.  **This Crate** (`crate::...`, `super::...`, `self::...`)

**Example:**

```rust
mod engine;

use std::{error::Error, sync::Arc};

use redixel_core::RedixelError;
use redixel_renderer::Renderer;
use wgpu::{Adapter, Device};
use winit::{event::WindowEvent, window::Window};

use crate::engine::Engine;
```

### 2\. Type Definitions & Return Types

Avoid fully qualified paths inside function signatures or variable declarations. Always import the type first. This keeps signatures clean and readable.

**Incorrect:**

```rust
// Hard to read, especially with long paths
fn get_adapter() -> wgpu::util::backend::Adapter { ... }
```

**Correct:**

```rust
use wgpu::Instance;

fn create_instance() -> Instance { ... }
```

### 3\. Explicit Typing

While Rust has type inference, we encourage explicit type annotations for variable declarations when initializing complex structs or engine subsystems. This makes the code self-documenting.

**Example:**

```rust
// Clear intent
let instance: Instance = Instance::new(...);
let surface: Surface = instance.create_surface(...);
```

### 4\. Documentation & Comments

- **Self-Documenting Code:** Prioritize clear naming for variables and functions over comments.
- **Context over Triviality:** Do not comment on trivial logic. Use comments to explain **WHY** a specific approach was taken, especially for complex math, unsafe blocks, or platform-specific hacks.

**Incorrect:**

```rust
// Creates a new instance
let instance = Instance::new();
```

**Correct:**

```rust
// Winit's Windows backend explicitly checks thread identity.
// We use `window_handle_any_thread` to bypass this guard safely.
let surface = unsafe { ... };
```

## Development Workflow

### 1\. Protected Branches

You are **strictly forbidden** from pushing directly to the following branches:

- `main`
- `develop`
- `dev-*` (any branch starting with `dev-`)

All changes must go through a Pull Request (PR).

### 2\. Testing

- **Requirement:** Every new feature or bug fix must include relevant tests.
- **Execution:** Ensure all unit tests pass locally by running `cargo test` before opening a PR.
- **Integration:** If your feature affects the rendering pipeline, ensure it does not break existing examples.

### 3\. Changelog

- **Requirement:** You must update the `CHANGELOG.md` file for every substantive change.
- Add a line describing your change under the section corresponding to the **current relevant version**.
- **Write for the people upgrading**, not the people reviewing:
  - One bullet per change, led by its area (`**Renderer:**`, `**Core:**`, ...) and the item's name. Aim for one sentence, about 25 words; add a second only for something a user must know, such as a cost or a caveat.
  - Say what changed and what it means for a game, not how it works inside. Internal design, data structures and the reasoning behind them belong in doc comments, `docs/architecture.md` or the PR.
  - A breaking change goes under **Changed** and ends with who it breaks (`Breaking for out-of-tree implementors.`).
  - A fix states the symptom users saw and what happens now, not the investigation.
  - Group related items as sub-bullets instead of one long sentence.

### 4\. Pull Request (PR) Process

1.  Push your feature branch to the repository.
2.  Open a Pull Request targeting the relevant version branch (e.g., `dev-0.6.0`).
3.  **CI Pipeline:** Wait for the Continuous Integration pipeline to pass. It runs against the workspace and every example, on Desktop (Linux, Windows, macOS), Web, Android and iOS:
    - Formatting (`cargo +nightly fmt -- --check`)
    - Linter (`cargo clippy -- -D warnings` - no warnings allowed)
    - Tests (`cargo test`, with and without the `net` feature)
    - Build (`cargo build`, plus `cargo apk build` for Android)
4.  **Code Review:** Request a review from a team member. You cannot merge your own code.
5.  Once approved and the pipeline is green, the PR can be merged.

## License

By contributing to Redixel, you agree that your contributions will be licensed under the **Apache License 2.0**, as defined in the `LICENSE` file.
