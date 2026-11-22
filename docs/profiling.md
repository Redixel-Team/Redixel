# Frame Profiling

A manual profiler for investigating renderer performance: it times named phases inside a frame, writes the result as JSON, and a small tool compares two such reports. It is for one-off investigations ("is this change faster?"), not for running on every pull request; that is the job of `tools/fps-bench`.

It is **off by default** and compiled out entirely unless the `profiling` feature is on, so a normal build pays nothing for it. It runs on native targets only: the browser has no `std::time::Instant`, so the feature is accepted there and does nothing.

## Running a measurement

Run from the repository root, so `config/config.json` is found. The examples are separate workspaces, so point cargo at the one to measure:

```sh
REDIXEL_PROFILE=1 REDIXEL_PROFILE_VARIANT=before REDIXEL_PROFILE_OUT=before.json \
  cargo run --release --manifest-path examples/shooter/Cargo.toml --features redixel/profiling
```

The app opens, discards the warmup frames, samples, writes the report, and **exits by itself**. The last thing on stderr is the line

```
REDIXEL_PROFILE_DONE
```

printed only after the report is fully on disk, so a script can wait for it (or simply for the process to exit). Closing the window before the run finishes writes no report.

| Variable                   | Default                | Meaning                                                              |
| :------------------------- | :--------------------- | :------------------------------------------------------------------- |
| `REDIXEL_PROFILE`          | off                    | Any value other than empty or `0` turns the profiler on.             |
| `REDIXEL_PROFILE_WARMUP`   | `120`                  | Frames discarded first, so shader compilation and first allocations stay out of the numbers. |
| `REDIXEL_PROFILE_FRAMES`   | `600`                  | Samples collected after the warmup.                                  |
| `REDIXEL_PROFILE_INTERVAL` | `1`                    | Sample one frame out of every N. Unsampled frames read no clock.     |
| `REDIXEL_PROFILE_OUT`      | `redixel-profile.json` | Where the report is written.                                         |
| `REDIXEL_PROFILE_VARIANT`  | `unlabeled`            | Label for the code under test (`before`, `wgpu-hal`, ...).           |

A value that is not a whole number is logged and replaced by its default; `FRAMES` and `INTERVAL` are never below 1.

## Phases

Phases are laps on one stopwatch, so they never overlap and add up to the frame. A phase that runs twice in a frame (the two batch flushes) adds up into one sample for that frame.

| Phase             | Covers                                                                                     |
| :---------------- | :----------------------------------------------------------------------------------------- |
| `input`           | Input tick and frame timing.                                                               |
| `fixed_update`    | The fixed-update loop: network update and flush, and the game's `on_fixed_update`.         |
| `update`          | The game's `on_update`.                                                                    |
| `render_callback` | The game's `on_render`, which only queues draw commands.                                   |
| `resource_upload` | Decoding and uploading queued textures, and handing queued sounds to the audio backend.    |
| `dispatch`        | Turning the queued draw commands into batched geometry.                                    |
| `audio`           | Audio commands and the audio update.                                                       |
| `uniforms`        | Uploading the frame's global uniforms.                                                     |
| `acquire`         | `get_current_texture`. This is where vsync and a GPU that is still behind show up.         |
| `encode`          | Texture view, command encoder, render pass setup.                                          |
| `buffer_upload`   | Growing and writing the vertex and index buffers (3D and 2D batches together).             |
| `draw`            | Recording the draw calls, through the end of the render pass.                              |
| `submit`          | Finishing the encoder and `queue.submit`.                                                  |
| `present`         | `queue.present`.                                                                           |
| `frame_end`       | Per-frame bookkeeping and the FPS cap's sleep, if `target_fps` is above zero.              |

A phase can be absent from a frame (an empty 3D batch records no `buffer_upload`), so each phase carries its own sample count `n`.

## The report

```jsonc
{
  "schema": 1,
  "meta": {
    "app": "Redixel",
    "binary": "shooter",
    "variant": "before",
    "adapter": { "name": "...", "backend": "Vulkan", "device_type": "DiscreteGpu", "driver": "..." },
    "present_mode": "Immediate",
    "surface": [1280, 720],
    "target_fps": 0.0,
    "tickrate": 60.0,
    "os": "linux", "arch": "x86_64", "build": "release", "redixel_version": "0.6.0"
  },
  "config": { "warmup_frames": 120, "frames": 600, "interval": 1 },
  "frame":  { "n": 600, "mean_ms": 0.0, "median_ms": 0.0, "p95_ms": 0.0, "p99_ms": 0.0, "min_ms": 0.0, "max_ms": 0.0, "stddev_ms": 0.0 },
  "phases": { "update": { "...same fields..." }, "...": {} }
}
```

Times are in milliseconds. `frame` spans one pass of the engine's frame function, from `input` to the end of `frame_end`; window events handled between frames are not in it. `app` is `config.json`'s `app.name`, which every example run from the repository root shares, so `binary` (the executable's name) is what identifies the game.

## Comparing two runs

```sh
cargo run --release -p profile-compare -- before.json after.json
```

It prints both runs' metadata, one row per phase (median and p95 for each side, the percentage change, and a verdict of `faster`, `slower` or `within noise`), and finishes with a conclusion on the whole frame. The verdict compares the means against twice the standard error of their difference. If the two reports disagree on the application, executable, adapter, present mode, surface size, FPS cap, tickrate, OS, architecture or build profile, it says so first: such a delta is not a verdict on the code.

The verdict treats frames as independent, which they are not, so on a single pair of runs it is a little overconfident. Measure each variant two or three times, alternating them, and trust a difference only when it repeats.

## Getting numbers worth comparing

- **Turn vsync off.** With `renderer.present_mode` at a vsync mode every frame is padded to the refresh interval and `acquire` or `present` swallows the rest. Use `Immediate`.
- **Remove the FPS cap.** Set `window.target_fps` to `0`, or `frame_end` fills with sleep.
- **Use `--release`.** `meta.build` records which one it was; the comparison flags a mismatch.
- **Same machine, same load, back to back.** A software rasteriser (`llvmpipe`) shows in `meta.adapter` and says nothing about real hardware.
- **Phases are CPU time.** GPU work is invisible except as waiting in `acquire` and `present`.
