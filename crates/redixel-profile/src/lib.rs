//! Opt-in frame profiler: named phases inside a frame, per-phase statistics, and a JSON report.
//!
//! Compiled in by the `profiling` feature and switched on at run time by `REDIXEL_PROFILE=1`.
//! Without the feature, or on `wasm32` where `std::time::Instant` is unavailable, every function
//! here is an empty `#[inline(always)]` body and the whole crate costs nothing.
//!
//! # Model
//!
//! Phases are laps on one stopwatch: [`phase`] closes the phase that was open and opens the
//! one it names, so phases never nest or overlap and their durations add up to the frame. A
//! name repeated within one frame accumulates into a single sample for that frame.
//!
//! State is per thread, so every call of a frame must come from the thread that called
//! [`begin_frame`]: the event-loop thread, in the windowed runtime.
//!
//! # Environment
//!
//! | Variable                   | Default                | Meaning                                    |
//! | :------------------------- | :--------------------- | :----------------------------------------- |
//! | `REDIXEL_PROFILE`          | off                    | any value other than empty or `0` enables |
//! | `REDIXEL_PROFILE_WARMUP`   | `120`                  | frames discarded before sampling starts    |
//! | `REDIXEL_PROFILE_FRAMES`   | `600`                  | samples to collect after the warmup        |
//! | `REDIXEL_PROFILE_INTERVAL` | `1`                    | sample one frame out of every N            |
//! | `REDIXEL_PROFILE_OUT`      | `redixel-profile.json` | report path                                |
//! | `REDIXEL_PROFILE_VARIANT`  | `unlabeled`            | label recorded in the report               |

use redixel_core::RedixelError;
use serde_json::Value;

/// The line [`finish`] writes to stderr once the report is on disk. A script waiting on a
/// measurement run can match it exactly.
pub const DONE_MARKER: &str = "REDIXEL_PROFILE_DONE";

#[cfg(all(feature = "profiling", not(target_arch = "wasm32")))]
#[path = "active.rs"]
mod imp;

#[cfg(not(all(feature = "profiling", not(target_arch = "wasm32"))))]
mod imp {
    use redixel_core::RedixelError;
    use serde_json::Value;

    #[inline(always)]
    pub(super) fn begin_frame() {}

    #[inline(always)]
    pub(super) fn phase(_name: &'static str) {}

    #[inline(always)]
    pub(super) fn end_frame() -> bool {
        false
    }

    #[inline(always)]
    pub(super) fn finish(_meta: Value) -> Result<(), RedixelError> {
        Ok(())
    }
}

/// Marks the start of a frame. Call it first thing in the frame, then [`phase`] straight away:
/// time between the two belongs to no phase.
#[inline(always)]
pub fn begin_frame() {
    imp::begin_frame();
}

/// Closes the open phase and opens `name`. A no-op on frames that are not being sampled.
#[inline(always)]
pub fn phase(name: &'static str) {
    imp::phase(name);
}

/// Marks the end of a frame and closes the open phase. Returns `true` once the requested
/// number of samples is collected, which is the caller's cue to call [`finish`] and stop.
/// Always `false` when profiling is compiled out or not enabled.
#[inline(always)]
pub fn end_frame() -> bool {
    imp::end_frame()
}

/// Writes the JSON report and then prints [`DONE_MARKER`] to stderr.
///
/// `meta` must be a JSON object describing the run (application, adapter, present mode, ...);
/// the profiler adds the variant label, OS, architecture, build profile and engine version to it.
/// The marker is printed only after the file is fully written.
///
/// Profiling stops here: later frames are not sampled, [`end_frame`] returns `false`, and a
/// second call does nothing, so the report is written once even if another frame runs before
/// the caller's exit takes effect.
///
/// # Errors
///
/// Returns [`RedixelError::Io`] when the report cannot be written.
#[inline(always)]
pub fn finish(meta: Value) -> Result<(), RedixelError> {
    imp::finish(meta)
}
