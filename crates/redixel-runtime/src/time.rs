#[cfg(not(target_arch = "wasm32"))]
use std::{
    thread,
    time::{Duration, Instant},
};

#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use crate::runtime::DEFAULT_TICKRATE;

/// How long before the deadline we switch from `thread::sleep` to a spin-loop.
/// 2 ms balances CPU burn against precision on most operating systems.
#[cfg(not(target_arch = "wasm32"))]
const SPIN_THRESHOLD: f64 = 0.002;

/// Number of recent frame times kept for the rolling average used by [`TimeManager::display_fps`].
const FPS_WINDOW: usize = 60;

/// Upper bound on fixed steps run per frame. Prevents the "spiral of death":
/// after a long stall the accumulator is clamped instead of running an
/// unbounded catch-up burst that would stall the frame even further.
const DEFAULT_MAX_SUBSTEPS: u32 = 8;

/// Tracks frame timing and enforces an optional FPS cap.
///
/// Exposes two different FPS readings:
/// - [`fps`](Self::fps): instantaneous, recalculated every single frame.
/// - [`display_fps`](Self::display_fps): rolling average over the last `FPS_WINDOW` (60) frames.
///
/// # Usage
/// ```ignore
/// time.begin_frame();
/// // ... render ...
/// time.end_frame();
/// time.every_seconds(1.0, |fps| window.set_title_stats(fps, rtt_ms));
/// ```
#[derive(Debug)]
pub struct TimeManager {
    fps: f64,
    frame_target: f64,
    frame_start: Instant,
    frame_last: Instant,
    last_interval_tick: Instant,
    frame_times: [f64; FPS_WINDOW],
    frame_times_idx: usize,
    frame_times_filled: usize,
    fixed_step: f64,
    accumulator: f64,
    max_substeps: u32,
    fixed_tick: u64,
    elapsed: f64,
}

impl TimeManager {
    pub fn new() -> Self {
        let now: Instant = Instant::now();
        Self {
            fps: 0.0,
            frame_target: 0.0,
            frame_start: now,
            frame_last: now,
            last_interval_tick: now,
            frame_times: [0.0; FPS_WINDOW],
            frame_times_idx: 0,
            frame_times_filled: 0,
            fixed_step: 1.0 / DEFAULT_TICKRATE,
            accumulator: 0.0,
            max_substeps: DEFAULT_MAX_SUBSTEPS,
            fixed_tick: 0,
            elapsed: 0.0,
        }
    }

    /// Sets the FPS cap. Pass `0.0` (or any non-positive value) to uncap.
    pub fn set_target_fps(&mut self, target_fps: f64) {
        self.frame_target = if target_fps > 0.0 { 1.0 / target_fps } else { 0.0 };
    }

    /// Sets the fixed-update tickrate in Hz (e.g. `60.0`). Non-positive values
    /// are ignored, keeping the previous step.
    ///
    /// Shrinking the step (raising the tickrate) re-clamps the accumulator, so
    /// time banked against a coarser step can never cash out as a catch-up burst
    /// longer than `max_substeps`.
    pub fn set_tickrate(&mut self, hz: f64) {
        if hz > 0.0 {
            self.fixed_step = 1.0 / hz;
            self.clamp_accumulator();
        } else {
            log::warn!("Ignoring non-positive tickrate {hz}; keeping {} Hz.", 1.0 / self.fixed_step);
        }
    }

    /// Caps the number of fixed steps consumed per frame (spiral-of-death guard).
    /// Must be at least 1.
    pub fn set_max_substeps(&mut self, max: u32) {
        self.max_substeps = max.max(1);
        self.clamp_accumulator();
    }

    /// Feeds elapsed real time into the fixed-step accumulator.
    ///
    /// Call once per frame (after measuring the frame delta). Excess time beyond
    /// `max_substeps` worth of steps is discarded so a hitch can never trigger
    /// an unbounded catch-up burst.
    ///
    /// The wall clock read by [`elapsed_time`](Self::elapsed_time) advances here
    /// too, and takes the delta whole: the clamp exists to bound simulation
    /// catch-up, not to pretend a stalled frame took less time than it did.
    pub fn accumulate(&mut self, frame_delta: f64) {
        self.elapsed += frame_delta;
        self.accumulator += frame_delta;
        self.clamp_accumulator();
    }

    /// Caps the accumulator at `max_substeps` worth of the *current* fixed step.
    fn clamp_accumulator(&mut self) {
        let ceiling: f64 = self.fixed_step * self.max_substeps as f64;
        if self.accumulator > ceiling {
            self.accumulator = ceiling;
        }
    }

    /// Drives the fixed-update loop. Returns `true` and consumes one fixed step
    /// (advancing [`fixed_tick`](Self::fixed_tick)) while a full step is owed,
    /// `false` once the accumulator is drained below one step.
    ///
    /// ```ignore
    /// time.accumulate(frame_delta);
    /// while time.next_fixed_step() {
    ///     game.on_fixed_update(ctx);
    /// }
    /// ```
    pub fn next_fixed_step(&mut self) -> bool {
        if self.accumulator >= self.fixed_step {
            self.accumulator -= self.fixed_step;
            self.fixed_tick += 1;
            true
        } else {
            false
        }
    }

    /// The constant duration of one fixed step, in seconds. Pass this as `dt`
    /// to deterministic simulation inside `on_fixed_update`.
    pub fn fixed_delta(&self) -> f64 {
        self.fixed_step
    }

    /// Monotonic count of fixed steps consumed since startup.
    pub fn fixed_tick(&self) -> u64 {
        self.fixed_tick
    }

    /// Seconds of real time since startup, summed from every frame delta fed to
    /// [`accumulate`](Self::accumulate).
    ///
    /// Unlike [`fixed_tick`](Self::fixed_tick) it advances continuously rather
    /// than in discrete steps, which is what a shader animation or any
    /// wall-clock-driven visual needs; unlike [`delta_time`](Self::delta_time)
    /// it is absolute rather than per-frame.
    pub fn elapsed_time(&self) -> f64 {
        self.elapsed
    }

    /// Fraction `[0, 1)` of the way into the next fixed step, for interpolating
    /// rendered visuals between the two most recent simulation states and
    /// avoiding stutter when render and tick rates differ.
    pub fn interpolation_alpha(&self) -> f64 {
        if self.fixed_step > 0.0 {
            (self.accumulator / self.fixed_step).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Call at the **start** of every frame, before any rendering work.
    pub fn begin_frame(&mut self) {
        self.frame_start = Instant::now();
    }

    /// Call at the **end** of every frame, after `present()`.
    ///
    /// Updates both the instantaneous and rolling-average FPS readings, then sleeps/spins to honour the cap.
    pub fn end_frame(&mut self) {
        let now: Instant = Instant::now();
        let delta: f64 = now.duration_since(self.frame_last).as_secs_f64();
        self.frame_last = now;
        self.record_delta(delta);

        #[cfg(not(target_arch = "wasm32"))]
        self.enforce_cap();
    }

    /// Applies one frame's elapsed time to the fps / rolling-average state.
    /// Split out of `end_frame` so tests can feed exact synthetic deltas
    /// instead of depending on real (and platform-jittery) sleeps.
    fn record_delta(&mut self, delta: f64) {
        if delta > 0.0 {
            self.fps = 1.0 / delta;
            self.push_frame_time(delta);
        }
    }

    /// Returns the time in seconds between the last two frames, derived from the **instantaneous** FPS.
    pub fn delta_time(&self) -> f64 {
        if self.fps > 0.0 { 1.0 / self.fps } else { 0.0 }
    }

    /// Returns the **instantaneous** FPS — recalculated every single frame.
    pub fn fps(&self) -> f64 {
        self.fps
    }

    /// Returns a **smoothed** FPS reading averaged over the last `FPS_WINDOW` (60) frames.
    pub fn display_fps(&self) -> f64 {
        if self.frame_times_filled == 0 {
            return 0.0;
        }

        let sum: f64 = self.frame_times[..self.frame_times_filled].iter().sum();
        let avg_delta: f64 = sum / self.frame_times_filled as f64;
        if avg_delta > 0.0 { 1.0 / avg_delta } else { 0.0 }
    }

    /// Invokes `callback` with the current **smoothed** FPS at most once per `interval` seconds.
    pub fn every_seconds<F: FnOnce(f64)>(&mut self, interval: f64, callback: F) {
        let now: Instant = Instant::now();
        if now.duration_since(self.last_interval_tick).as_secs_f64() >= interval {
            self.last_interval_tick = now;
            callback(self.display_fps());
        }
    }

    /// Pushes a new frame delta into the ring buffer, overwriting the oldest sample once the buffer is full.
    fn push_frame_time(&mut self, delta: f64) {
        self.frame_times[self.frame_times_idx] = delta;
        self.frame_times_idx = (self.frame_times_idx + 1) % FPS_WINDOW;

        if self.frame_times_filled < FPS_WINDOW {
            self.frame_times_filled += 1;
        }
    }

    /// Hybrid frame limiter: coarse `sleep` + precision spin-loop.
    #[cfg(not(target_arch = "wasm32"))]
    fn enforce_cap(&self) {
        if self.frame_target == 0.0 {
            return;
        }

        let elapsed: f64 = self.frame_start.elapsed().as_secs_f64();
        let remaining: f64 = self.frame_target - elapsed;

        if remaining <= 0.0 {
            return;
        }

        if remaining > SPIN_THRESHOLD {
            thread::sleep(Duration::from_secs_f64(remaining - SPIN_THRESHOLD));
        }

        while self.frame_start.elapsed().as_secs_f64() < self.frame_target {
            std::hint::spin_loop();
        }
    }
}

impl Default for TimeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let tm: TimeManager = TimeManager::new();
        assert_eq!(tm.fps, 0.0);
        assert_eq!(tm.frame_target, 0.0);
        assert_eq!(tm.display_fps(), 0.0);
    }

    #[test]
    fn set_target_fps() {
        let mut tm: TimeManager = TimeManager::new();
        const EPS: f64 = 1e-9;

        tm.set_target_fps(60.0);
        assert!((tm.frame_target - 1.0 / 60.0).abs() < EPS);

        tm.set_target_fps(144.0);
        assert!((tm.frame_target - 1.0 / 144.0).abs() < EPS);

        tm.set_target_fps(0.0);
        assert_eq!(tm.frame_target, 0.0);

        tm.set_target_fps(-1.0);
        assert_eq!(tm.frame_target, 0.0);
    }

    #[test]
    fn elapsed_time_sums_frame_deltas() {
        let mut tm: TimeManager = TimeManager::new();
        const EPS: f64 = 1e-9;

        assert_eq!(tm.elapsed_time(), 0.0);

        for _ in 0..10 {
            tm.accumulate(0.016);
        }

        assert!((tm.elapsed_time() - 0.16).abs() < EPS, "elapsed={}", tm.elapsed_time());
    }

    #[test]
    fn elapsed_time_keeps_time_the_accumulator_clamps() {
        let mut tm: TimeManager = TimeManager::new();
        const EPS: f64 = 1e-9;

        tm.set_tickrate(60.0);
        tm.set_max_substeps(8);
        tm.accumulate(5.0);

        assert!(tm.accumulator < 5.0, "accumulator should be clamped, got {}", tm.accumulator);
        assert!((tm.elapsed_time() - 5.0).abs() < EPS, "elapsed={}", tm.elapsed_time());
    }

    #[test]
    fn fps_measurement() {
        let mut tm: TimeManager = TimeManager::new();
        tm.record_delta(0.016);
        assert!(tm.fps > 50.0 && tm.fps < 80.0, "fps={}", tm.fps);
    }

    #[test]
    fn display_fps_smooths_spikes() {
        let mut tm: TimeManager = TimeManager::new();

        for _ in 0..10 {
            tm.record_delta(0.016);
        }

        tm.record_delta(0.2);
        assert!(tm.fps() < 15.0, "instant fps should reflect the spike, got {}", tm.fps());

        assert!(
            tm.display_fps() > 30.0,
            "rolling average should absorb a single spike, got {}",
            tm.display_fps()
        );
    }

    #[test]
    fn display_fps_converges_to_stable_rate() {
        let mut tm: TimeManager = TimeManager::new();

        for _ in 0..FPS_WINDOW {
            tm.record_delta(0.010);
        }

        let display: f64 = tm.display_fps();
        assert!(
            display > 70.0 && display < 130.0,
            "display_fps should converge near 100, got {display}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn interval_callback() {
        let mut tm: TimeManager = TimeManager::new();
        let mut fired: bool = false;

        tm.every_seconds(0.05, |_: f64| fired = true);
        assert!(!fired);

        thread::sleep(Duration::from_millis(60));
        tm.every_seconds(0.05, |_: f64| fired = true);
        assert!(fired);
    }

    #[test]
    fn fixed_step_defaults_to_60hz() {
        let tm: TimeManager = TimeManager::new();
        const EPS: f64 = 1e-9;
        assert!((tm.fixed_delta() - 1.0 / 60.0).abs() < EPS);
        assert_eq!(tm.fixed_tick(), 0);
    }

    #[test]
    fn set_tickrate_changes_step() {
        let mut tm: TimeManager = TimeManager::new();
        const EPS: f64 = 1e-9;

        tm.set_tickrate(30.0);
        assert!((tm.fixed_delta() - 1.0 / 30.0).abs() < EPS);

        tm.set_tickrate(0.0);
        assert!((tm.fixed_delta() - 1.0 / 30.0).abs() < EPS);
    }

    #[test]
    fn accumulator_yields_expected_step_count() {
        let mut tm: TimeManager = TimeManager::new();
        tm.set_tickrate(60.0);
        tm.accumulate(1.0 / 60.0);
        let mut steps: u32 = 0;

        while tm.next_fixed_step() {
            steps += 1;
        }

        assert_eq!(steps, 1);
        assert_eq!(tm.fixed_tick(), 1);

        tm.accumulate(0.05);
        steps = 0;

        while tm.next_fixed_step() {
            steps += 1;
        }

        assert_eq!(steps, 3);
        assert_eq!(tm.fixed_tick(), 4);
    }

    #[test]
    fn accumulator_carries_remainder_between_frames() {
        let mut tm: TimeManager = TimeManager::new();
        tm.set_tickrate(60.0);

        tm.accumulate(0.010);
        assert!(!tm.next_fixed_step(), "single sub-step frame must not tick");

        tm.accumulate(0.010);
        let mut steps: u32 = 0;

        while tm.next_fixed_step() {
            steps += 1;
        }

        assert_eq!(steps, 1);
    }

    #[test]
    fn accumulator_clamps_under_stall() {
        let mut tm: TimeManager = TimeManager::new();
        tm.set_tickrate(60.0);
        tm.set_max_substeps(8);

        tm.accumulate(10.0);
        let mut steps: u32 = 0;

        while tm.next_fixed_step() {
            steps += 1;
        }

        assert_eq!(steps, 8, "accumulator must clamp to max_substeps");
    }

    #[test]
    fn raising_tickrate_reclamps_banked_time() {
        let mut tm: TimeManager = TimeManager::new();
        tm.set_tickrate(20.0);
        tm.set_max_substeps(8);

        tm.accumulate(10.0);
        tm.set_tickrate(60.0);

        let mut steps: u32 = 0;
        while tm.next_fixed_step() {
            steps += 1;
        }

        assert_eq!(steps, 8, "a tickrate change must not bypass the max_substeps clamp");
    }

    #[test]
    fn lowering_max_substeps_reclamps_banked_time() {
        let mut tm: TimeManager = TimeManager::new();
        tm.set_tickrate(60.0);
        tm.accumulate(10.0);
        tm.set_max_substeps(2);

        let mut steps: u32 = 0;
        while tm.next_fixed_step() {
            steps += 1;
        }

        assert_eq!(steps, 2);
    }

    #[test]
    fn interpolation_alpha_tracks_partial_step() {
        let mut tm: TimeManager = TimeManager::new();
        tm.set_tickrate(100.0);
        tm.accumulate(0.005);
        assert!((tm.interpolation_alpha() - 0.5).abs() < 1e-6);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn frame_cap_accuracy() {
        let mut tm: TimeManager = TimeManager::new();
        tm.set_target_fps(100.0);

        let start: Instant = Instant::now();
        tm.begin_frame();
        tm.end_frame();
        let elapsed: f64 = start.elapsed().as_secs_f64();

        assert!(elapsed >= 0.010, "limiter fired too early: {elapsed:.4}s");
        assert!(elapsed < 0.2, "limiter overslept: {elapsed:.4}s");
    }
}
