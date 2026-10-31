//! Internal CI benchmark: renders a fixed synthetic scene and reports median FPS, frame times and allocations per tier.

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
    use wgpu::{Backends, PresentMode};

    const WARMUP: Duration = Duration::from_millis(500);
    const WARMUP_FRAMES: usize = 30;
    const MEASURE_FRAMES: usize = 300;
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
        avg_fps:         f64,
        avg_frame_ms:    f64,
        p95_frame_ms:    f64,
        allocations:     u64,
        allocated_bytes: u64,
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
        allocations:         u64,
        allocated_bytes:     u64,
    }

    struct FpsBenchmark {
        schedule:      Vec<usize>,
        step:          usize,
        step_started:  Option<Instant>,
        warmup_frames: usize,
        samples:       Vec<f64>,
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

    fn median_u64(values: &[u64]) -> u64 {
        let mut sorted: Vec<u64> = values.to_vec();
        sorted.sort_unstable();

        let mid: usize = sorted.len() / 2;
        if sorted.len().is_multiple_of(2) {
            sorted[mid - 1] + (sorted[mid] - sorted[mid - 1]) / 2
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

            self.samples.push(ctx.delta_time());

            if self.samples.len() < MEASURE_FRAMES {
                return;
            }

            let tier: usize = self.tier_index();
            let total_delta: f64 = self.samples.iter().sum();
            let (alloc_count, alloc_bytes): (u64, u64) = alloc_snapshot();

            self.collected.lock().unwrap()[tier].push(Sample {
                avg_fps:         self.samples.len() as f64 / total_delta,
                avg_frame_ms:    (total_delta / self.samples.len() as f64) * 1000.0,
                p95_frame_ms:    percentile_ms(&self.samples, 0.95),
                allocations:     alloc_count - self.alloc_start.0,
                allocated_bytes: alloc_bytes - self.alloc_start.1,
            });

            self.samples.clear();
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
        let frame_ms: Vec<f64> = samples.iter().map(|s: &Sample| s.avg_frame_ms).collect();
        let p95_ms: Vec<f64> = samples.iter().map(|s: &Sample| s.p95_frame_ms).collect();
        let allocations: Vec<u64> = samples.iter().map(|s: &Sample| s.allocations).collect();
        let allocated_bytes: Vec<u64> = samples.iter().map(|s: &Sample| s.allocated_bytes).collect();

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
            median_frame_ms: round2(median(&frame_ms)),
            median_p95_frame_ms: round2(median(&p95_ms)),
            allocations: median_u64(&allocations),
            allocated_bytes: median_u64(&allocated_bytes),
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

        let schedule: Vec<usize> = (0..ROUNDS).flat_map(|_| 0..TIERS.len()).collect();
        let collected: Arc<Mutex<Vec<Vec<Sample>>>> =
            Arc::new(Mutex::new((0..TIERS.len()).map(|_| Vec::with_capacity(ROUNDS)).collect()));

        let game: FpsBenchmark = FpsBenchmark {
            schedule,
            step: 0,
            step_started: None,
            warmup_frames: 0,
            samples: Vec::with_capacity(MEASURE_FRAMES),
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
                backends:     Backends::all(),
                present_mode: PresentMode::Immediate,
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

        let marginal_ms: f64 = round2(marginal_ms_per_1000_quads(&data));

        let tiers: Vec<serde_json::Value> = data
            .iter()
            .map(|r: &TierReport| {
                log::info!(
                    "{}: {:.2} fps median over {} rounds (range {:.2}–{:.2}, spread {:.1}%, frame {:.2} ms, p95 {:.2} ms, {} allocs / {} bytes)",
                    r.scene,
                    r.median_fps,
                    r.rounds,
                    r.fps_min,
                    r.fps_max,
                    r.spread_pct,
                    r.median_frame_ms,
                    r.median_p95_frame_ms,
                    r.allocations,
                    r.allocated_bytes
                );

                serde_json::json!({
                    "scene": r.scene,
                    "rounds": r.rounds,
                    "median_fps": format!("{:.2}", r.median_fps),
                    "fps_min": format!("{:.2}", r.fps_min),
                    "fps_max": format!("{:.2}", r.fps_max),
                    "spread_pct": format!("{:.1}", r.spread_pct),
                    "median_frame_ms": format!("{:.2}", r.median_frame_ms),
                    "median_p95_frame_ms": format!("{:.2}", r.median_p95_frame_ms),
                    "allocations": r.allocations,
                    "allocated_bytes": r.allocated_bytes,
                })
            })
            .collect();

        log::info!("Marginal cost: {marginal_ms:.2} ms per 1000 additional quads");

        let output: serde_json::Value = serde_json::json!({
            "rounds": ROUNDS,
            "warmup_ms": WARMUP.as_millis(),
            "warmup_frames": WARMUP_FRAMES,
            "measure_frames": MEASURE_FRAMES,
            "tiers": tiers,
            "marginal_ms_per_1000_quads": format!("{:.2}", marginal_ms),
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
