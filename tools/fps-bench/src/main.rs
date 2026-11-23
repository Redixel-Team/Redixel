//! Internal CI benchmark: renders a fixed synthetic scene and reports median FPS, frame times and allocations per tier.
//!
//! Each round measures for a fixed time rather than a fixed frame count, so a window spans many display refreshes
//! at any frame rate and a one-off stall in the driver or compositor cannot swing it.

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
mod bench {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        sync::{
            Arc, Mutex, MutexGuard,
            atomic::{AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    };

    use redixel::prelude::*;
    use redixel_platform::window::WindowConfig;
    use redixel_renderer::RendererConfig;
    use wgpu_types::{Backends, PresentMode};

    const WARMUP: Duration = Duration::from_millis(500);
    const WARMUP_FRAMES: usize = 30;
    const MEASURE: Duration = Duration::from_secs(2);
    /// Frame times reserved up front, so the measurement does not allocate while it counts allocations.
    const SAMPLE_CAPACITY: usize = 1 << 17;
    const ROUNDS: usize = 3;

    const TIERS: [(usize, &str); 5] = [
        (100, "100 quads"),
        (500, "500 quads"),
        (2000, "2000 quads"),
        (5000, "5000 quads"),
        (8000, "8000 quads"),
    ];

    static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
    static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);

    struct CountingAllocator;

    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static GLOBAL: CountingAllocator = CountingAllocator;

    fn alloc_snapshot() -> (u64, u64) {
        (ALLOC_COUNT.load(Ordering::Relaxed), ALLOC_BYTES.load(Ordering::Relaxed))
    }

    /// One measured window for one tier.
    struct Sample {
        avg_fps:          f64,
        avg_frame_ms:     f64,
        p95_frame_ms:     f64,
        p99_frame_ms:     f64,
        low_1pct_fps:     f64,
        frames:           usize,
        allocs_per_frame: f64,
        bytes_per_frame:  f64,
    }

    /// A tier's samples reduced across rounds.
    ///
    /// Reports the median plus the observed range, because a single figure
    /// cannot distinguish a regression from an outlier.
    struct TierReport {
        quad_count:          usize,
        scene:               &'static str,
        rounds:              usize,
        median_fps:          f64,
        fps_min:             f64,
        fps_max:             f64,
        spread_pct:          f64,
        median_frame_ms:     f64,
        median_p95_frame_ms: f64,
        median_p99_frame_ms: f64,
        median_low_1pct_fps: f64,
        median_frames:       f64,
        allocs_per_frame:    f64,
        bytes_per_frame:     f64,
    }

    struct FpsBenchmark {
        schedule:      Vec<usize>,
        step:          usize,
        step_started:  Option<Instant>,
        warmup_frames: usize,
        samples:       Vec<f64>,
        measured:      f64,
        alloc_start:   (u64, u64),
        collected:     Arc<Mutex<Vec<Vec<Sample>>>>,
    }

    impl FpsBenchmark {
        fn tier_index(&self) -> usize {
            self.schedule[self.step.min(self.schedule.len() - 1)]
        }

        fn quad_count(&self) -> usize {
            TIERS[self.tier_index()].0
        }
    }

    fn round2(value: f64) -> f64 {
        (value * 100.0).round() / 100.0
    }

    fn round3(value: f64) -> f64 {
        (value * 1000.0).round() / 1000.0
    }

    /// The "1% low": frames per second averaged over the slowest 1% of the frames.
    fn low_1pct_fps(samples: &[f64]) -> f64 {
        let mut sorted: Vec<f64> = samples.to_vec();
        sorted.sort_by(|a: &f64, b: &f64| b.total_cmp(a));
        let slowest: &[f64] = &sorted[..samples.len().div_ceil(100)];
        slowest.len() as f64 / slowest.iter().sum::<f64>()
    }

    fn percentile_ms(samples: &[f64], p: f64) -> f64 {
        let mut sorted: Vec<f64> = samples.to_vec();
        sorted.sort_by(|a: &f64, b: &f64| a.total_cmp(b));
        let idx: usize = ((sorted.len() as f64 - 1.0) * p).round() as usize;
        sorted[idx] * 1000.0
    }

    fn median(values: &[f64]) -> f64 {
        let mut sorted: Vec<f64> = values.to_vec();
        sorted.sort_by(|a: &f64, b: &f64| a.total_cmp(b));

        let mid: usize = sorted.len() / 2;
        if sorted.len().is_multiple_of(2) {
            (sorted[mid - 1] + sorted[mid]) / 2.0
        } else {
            sorted[mid]
        }
    }

    impl Game for FpsBenchmark {
        type Action = ();

        fn on_start(&mut self, _ctx: &mut dyn GameContext<Self::Action>) {}

        fn on_update(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
            let started: Instant = *self.step_started.get_or_insert_with(Instant::now);

            if started.elapsed() < WARMUP || self.warmup_frames < WARMUP_FRAMES {
                self.warmup_frames += 1;
                return;
            }

            if self.samples.is_empty() {
                self.alloc_start = alloc_snapshot();
            }

            let delta: f64 = ctx.delta_time();
            self.samples.push(delta);
            self.measured += delta;

            if self.measured < MEASURE.as_secs_f64() {
                return;
            }

            let tier: usize = self.tier_index();
            let frames: usize = self.samples.len();
            let (alloc_count, alloc_bytes): (u64, u64) = alloc_snapshot();

            self.collected.lock().unwrap()[tier].push(Sample {
                avg_fps: frames as f64 / self.measured,
                avg_frame_ms: (self.measured / frames as f64) * 1000.0,
                p95_frame_ms: percentile_ms(&self.samples, 0.95),
                p99_frame_ms: percentile_ms(&self.samples, 0.99),
                low_1pct_fps: low_1pct_fps(&self.samples),
                frames,
                allocs_per_frame: (alloc_count - self.alloc_start.0) as f64 / frames as f64,
                bytes_per_frame: (alloc_bytes - self.alloc_start.1) as f64 / frames as f64,
            });

            self.samples.clear();
            self.measured = 0.0;
            self.step_started = None;
            self.warmup_frames = 0;
            self.step += 1;

            if self.step >= self.schedule.len() {
                ctx.exit();
            }
        }

        fn on_render(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
            ctx.clear_color(Color::rgb(0.05, 0.05, 0.08));

            const COLS: usize = 80;
            const CELL: f32 = 16.0;

            for i in 0..self.quad_count() {
                let col: f32 = (i % COLS) as f32;
                let row: f32 = (i / COLS) as f32;
                let position: Vec2 = Vec2::new(col * CELL, row * CELL);
                ctx.draw_rect(position, Vec2::new(CELL - 2.0, CELL - 2.0), Color::rgb(0.9, 0.4, 0.2));
            }
        }
    }

    fn reduce(tier: usize, samples: &[Sample]) -> TierReport {
        let (quad_count, scene): (usize, &'static str) = TIERS[tier];

        let fps: Vec<f64> = samples.iter().map(|s: &Sample| s.avg_fps).collect();
        let field = |f: fn(&Sample) -> f64| -> f64 { median(&samples.iter().map(f).collect::<Vec<f64>>()) };

        let median_fps: f64 = median(&fps);
        let fps_min: f64 = fps.iter().copied().fold(f64::INFINITY, f64::min);
        let fps_max: f64 = fps.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        TierReport {
            quad_count,
            scene,
            rounds: samples.len(),
            median_fps: round2(median_fps),
            fps_min: round2(fps_min),
            fps_max: round2(fps_max),
            spread_pct: (fps_max - fps_min) / median_fps * 100.0,
            median_frame_ms: round3(field(|s: &Sample| s.avg_frame_ms)),
            median_p95_frame_ms: round3(field(|s: &Sample| s.p95_frame_ms)),
            median_p99_frame_ms: round3(field(|s: &Sample| s.p99_frame_ms)),
            median_low_1pct_fps: round2(field(|s: &Sample| s.low_1pct_fps)),
            median_frames: field(|s: &Sample| s.frames as f64),
            allocs_per_frame: round2(field(|s: &Sample| s.allocs_per_frame)),
            bytes_per_frame: round2(field(|s: &Sample| s.bytes_per_frame)),
        }
    }

    fn marginal_ms_per_1000_quads(data: &[TierReport]) -> f64 {
        let first: &TierReport = data.first().expect("fps-bench: no tiers recorded");
        let last: &TierReport = data.last().expect("fps-bench: no tiers recorded");
        let quad_delta: f64 = (last.quad_count - first.quad_count) as f64;
        (last.median_frame_ms - first.median_frame_ms) / (quad_delta / 1000.0)
    }

    pub fn main() {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

        let only_quads: Option<usize> = std::env::var("FPS_BENCH_QUADS")
            .ok()
            .and_then(|v: String| v.parse::<usize>().ok());
        let schedule: Vec<usize> = (0..ROUNDS)
            .flat_map(|_| 0..TIERS.len())
            .filter(|&tier: &usize| only_quads.is_none_or(|q: usize| TIERS[tier].0 == q))
            .collect();
        let collected: Arc<Mutex<Vec<Vec<Sample>>>> =
            Arc::new(Mutex::new((0..TIERS.len()).map(|_| Vec::with_capacity(ROUNDS)).collect()));

        let game: FpsBenchmark = FpsBenchmark {
            schedule,
            step: 0,
            step_started: None,
            warmup_frames: 0,
            samples: Vec::with_capacity(SAMPLE_CAPACITY),
            measured: 0.0,
            alloc_start: (0, 0),
            collected: collected.clone(),
        };

        let config: RuntimeConfig = RuntimeConfig::windowed(
            WindowConfig {
                title:      String::from("redixel-fps-bench"),
                width:      1280,
                height:     720,
                fullscreen: false,
            },
            RendererConfig {
                backends: Backends::all(),
                present_mode: PresentMode::Immediate,
                ..RendererConfig::default()
            },
            0.0,
            60.0,
        );

        if let Err(e) = redixel::run_desktop_with(game, config) {
            eprintln!("fps-bench: the engine failed to run: {e:?}");
            std::process::exit(1);
        }

        let samples: MutexGuard<'_, Vec<Vec<Sample>>> = collected.lock().unwrap();
        let data: Vec<TierReport> = samples
            .iter()
            .enumerate()
            .filter(|(_, s): &(usize, &Vec<Sample>)| !s.is_empty())
            .map(|(tier, s): (usize, &Vec<Sample>)| reduce(tier, s))
            .collect();

        if data.is_empty() {
            eprintln!("fps-bench: no tier completed a measurement window");
            std::process::exit(1);
        }

        let marginal_ms: f64 = round3(marginal_ms_per_1000_quads(&data));

        let tiers: Vec<serde_json::Value> = data
            .iter()
            .map(|r: &TierReport| {
                log::info!(
                    "{}: {:.2} fps median over {} rounds (range {:.2}–{:.2}, spread {:.1}%, 1% low {:.2} fps, frame {:.3} ms, p95 {:.3} ms, p99 {:.3} ms, {:.2} allocs / {:.2} bytes per frame)",
                    r.scene,
                    r.median_fps,
                    r.rounds,
                    r.fps_min,
                    r.fps_max,
                    r.spread_pct,
                    r.median_low_1pct_fps,
                    r.median_frame_ms,
                    r.median_p95_frame_ms,
                    r.median_p99_frame_ms,
                    r.allocs_per_frame,
                    r.bytes_per_frame
                );

                serde_json::json!({
                    "scene": r.scene,
                    "rounds": r.rounds,
                    "median_fps": format!("{:.2}", r.median_fps),
                    "fps_min": format!("{:.2}", r.fps_min),
                    "fps_max": format!("{:.2}", r.fps_max),
                    "spread_pct": format!("{:.1}", r.spread_pct),
                    "median_low_1pct_fps": format!("{:.2}", r.median_low_1pct_fps),
                    "median_frame_ms": format!("{:.3}", r.median_frame_ms),
                    "median_p95_frame_ms": format!("{:.3}", r.median_p95_frame_ms),
                    "median_p99_frame_ms": format!("{:.3}", r.median_p99_frame_ms),
                    "median_frames": r.median_frames,
                    "allocs_per_frame": format!("{:.2}", r.allocs_per_frame),
                    "bytes_per_frame": format!("{:.2}", r.bytes_per_frame),
                })
            })
            .collect();

        log::info!("Marginal cost: {marginal_ms:.3} ms per 1000 additional quads");

        let output: serde_json::Value = serde_json::json!({
            "rounds": ROUNDS,
            "warmup_ms": WARMUP.as_millis(),
            "warmup_frames": WARMUP_FRAMES,
            "measure_ms": MEASURE.as_millis(),
            "tiers": tiers,
            "marginal_ms_per_1000_quads": format!("{:.3}", marginal_ms),
        });

        let out_path: String = std::env::args()
            .nth(1)
            .unwrap_or_else(|| String::from("benchmark_result.json"));
        let json: String = serde_json::to_string_pretty(&output).expect("fps-bench: failed to serialize results");
        std::fs::write(&out_path, json).expect("fps-bench: failed to write output file");
    }
}

fn main() {
    #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
    bench::main();
}
