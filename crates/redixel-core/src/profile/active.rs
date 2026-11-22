use std::{
    cell::RefCell,
    env, fs, io,
    time::{Duration, Instant},
};

use serde_json::{Map, Value, json};

use crate::{RedixelError, profile::DONE_MARKER};

const DEFAULT_WARMUP: u64 = 120;
const DEFAULT_FRAMES: u64 = 600;
const DEFAULT_INTERVAL: u64 = 1;
const DEFAULT_OUT: &str = "redixel-profile.json";
const DEFAULT_VARIANT: &str = "unlabeled";
const SCHEMA: u64 = 1;

/// A measurement run's settings, resolved once from the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Config {
    warmup:   u64,
    frames:   u64,
    interval: u64,
    out:      String,
    variant:  String,
}

impl Config {
    /// Resolves the settings through `get`, or `None` when profiling was not requested.
    ///
    /// Takes the lookup as a closure so tests can supply values without touching the
    /// process environment. A value that does not parse is logged and replaced by its
    /// default; `frames` and `interval` are raised to at least 1, since zero of either
    /// would never finish.
    fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let enabled: bool = get("REDIXEL_PROFILE").is_some_and(|value: String| !matches!(value.trim(), "" | "0"));
        if !enabled {
            return None;
        }

        let text = |key: &str, default: &str| -> String {
            get(key)
                .filter(|value: &String| !value.is_empty())
                .unwrap_or_else(|| default.to_string())
        };

        Some(Self {
            warmup:   Self::number(&get, "REDIXEL_PROFILE_WARMUP", DEFAULT_WARMUP),
            frames:   Self::number(&get, "REDIXEL_PROFILE_FRAMES", DEFAULT_FRAMES).max(1),
            interval: Self::number(&get, "REDIXEL_PROFILE_INTERVAL", DEFAULT_INTERVAL).max(1),
            out:      text("REDIXEL_PROFILE_OUT", DEFAULT_OUT),
            variant:  text("REDIXEL_PROFILE_VARIANT", DEFAULT_VARIANT),
        })
    }

    fn number(get: &impl Fn(&str) -> Option<String>, key: &str, default: u64) -> u64 {
        let Some(raw) = get(key) else {
            return default;
        };

        raw.trim().parse::<u64>().unwrap_or_else(|_| {
            log::warn!("Ignoring {key}={raw:?}: expected a whole number. Using {default}.");
            default
        })
    }
}

/// Gathers the samples of one run.
///
/// Every method that needs the time takes it as a closure and calls it only on a frame
/// being sampled, so an unsampled frame costs no clock read and a test can drive the
/// collector with a fake clock.
#[derive(Debug)]
struct Collector {
    config:      Config,
    frame_index: u64,
    collected:   u64,
    recording:   bool,
    frame_start: Option<Instant>,
    open:        Option<(&'static str, Instant)>,
    current:     Vec<(&'static str, Duration)>,
    totals:      Vec<Duration>,
    phases:      Vec<(&'static str, Vec<Duration>)>,
}

impl Collector {
    fn new(config: Config) -> Self {
        Self {
            config,
            frame_index: 0,
            collected: 0,
            recording: false,
            frame_start: None,
            open: None,
            current: Vec::new(),
            totals: Vec::new(),
            phases: Vec::new(),
        }
    }

    /// Decides whether this frame is sampled: past the warmup, on the sampling interval,
    /// and before the requested count is reached.
    fn begin_frame(&mut self, now: impl FnOnce() -> Instant) {
        let past_warmup: u64 = self.frame_index.saturating_sub(self.config.warmup);
        self.recording = self.collected < self.config.frames
            && self.frame_index >= self.config.warmup
            && past_warmup.is_multiple_of(self.config.interval);

        self.frame_index += 1;
        self.open = None;
        self.current.clear();
        self.frame_start = self.recording.then(now);
    }

    fn phase(&mut self, name: &'static str, now: impl FnOnce() -> Instant) {
        if !self.recording {
            return;
        }

        let now: Instant = now();
        self.close(now);
        self.open = Some((name, now));
    }

    /// Adds the open phase's elapsed time to this frame's tally for its name.
    fn close(&mut self, now: Instant) {
        let Some((name, start)) = self.open.take() else {
            return;
        };

        let elapsed: Duration = now.saturating_duration_since(start);
        match self
            .current
            .iter_mut()
            .find(|(known, _): &&mut (&'static str, Duration)| *known == name)
        {
            Some((_, total)) => *total += elapsed,
            None => self.current.push((name, elapsed)),
        }
    }

    /// Closes the frame and reports whether the requested sample count is now reached.
    fn end_frame(&mut self, now: impl FnOnce() -> Instant) -> bool {
        if self.recording {
            let now: Instant = now();
            self.close(now);

            if let Some(start) = self.frame_start.take() {
                self.totals.push(now.saturating_duration_since(start));
            }

            for (name, elapsed) in self.current.drain(..) {
                match self
                    .phases
                    .iter_mut()
                    .find(|(known, _): &&mut (&'static str, Vec<Duration>)| *known == name)
                {
                    Some((_, samples)) => samples.push(elapsed),
                    None => self.phases.push((name, vec![elapsed])),
                }
            }

            self.collected += 1;
            self.recording = false;
        }

        self.collected >= self.config.frames
    }

    /// Builds the report. The caller's `meta` is kept as given, with the variant label and
    /// the build environment added to it.
    fn report(&self, meta: Value) -> Value {
        let mut meta: Map<String, Value> = match meta {
            Value::Object(map) => map,
            other => {
                log::warn!("Profile metadata must be a JSON object, got {other}. Recording the run without it.");
                Map::new()
            }
        };

        meta.insert("variant".to_string(), json!(self.config.variant));
        meta.insert("os".to_string(), json!(env::consts::OS));
        meta.insert("arch".to_string(), json!(env::consts::ARCH));
        meta.insert(
            "build".to_string(),
            json!(if cfg!(debug_assertions) { "debug" } else { "release" }),
        );
        meta.insert("redixel_version".to_string(), json!(env!("CARGO_PKG_VERSION")));

        let mut phases: Map<String, Value> = Map::new();
        for (name, samples) in &self.phases {
            phases.insert((*name).to_string(), summarize(samples));
        }

        json!({
            "schema": SCHEMA,
            "meta": meta,
            "config": {
                "warmup_frames": self.config.warmup,
                "frames": self.config.frames,
                "interval": self.config.interval,
            },
            "frame": summarize(&self.totals),
            "phases": phases,
        })
    }
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

/// Nearest-rank percentile of an ascending, non-empty slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() as f64 - 1.0) * p).round() as usize]
}

/// Reduces one series to the figures a comparison needs. Milliseconds, four decimals.
///
/// The standard deviation is the sample one (`n - 1`), which is what a difference-of-means
/// test over two reports expects.
fn summarize(samples: &[Duration]) -> Value {
    if samples.is_empty() {
        return json!({ "n": 0 });
    }

    let mut ms: Vec<f64> = samples.iter().map(|d: &Duration| d.as_secs_f64() * 1000.0).collect();
    ms.sort_by(|a: &f64, b: &f64| a.total_cmp(b));

    let n: usize = ms.len();
    let mean: f64 = ms.iter().sum::<f64>() / n as f64;
    let variance: f64 = if n > 1 {
        ms.iter().map(|x: &f64| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64
    } else {
        0.0
    };
    let median: f64 = if n.is_multiple_of(2) {
        (ms[n / 2 - 1] + ms[n / 2]) / 2.0
    } else {
        ms[n / 2]
    };

    json!({
        "n": n,
        "mean_ms": round4(mean),
        "median_ms": round4(median),
        "p95_ms": round4(percentile(&ms, 0.95)),
        "p99_ms": round4(percentile(&ms, 0.99)),
        "min_ms": round4(ms[0]),
        "max_ms": round4(ms[n - 1]),
        "stddev_ms": round4(variance.sqrt()),
    })
}

fn write_report(collector: &Collector, meta: Value) -> Result<(), RedixelError> {
    let json: String = serde_json::to_string_pretty(&collector.report(meta))?;
    fs::write(&collector.config.out, json).map_err(|e: io::Error| {
        io::Error::new(
            e.kind(),
            format!("cannot write the frame profile to {}: {e}", collector.config.out),
        )
    })?;

    log::info!("Frame profile written to {}", collector.config.out);
    eprintln!("{DONE_MARKER}");

    Ok(())
}

enum State {
    Uninit,
    Off,
    On(Box<Collector>),
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::Uninit) };
}

fn with_collector<R>(f: impl FnOnce(&mut Collector) -> R) -> Option<R> {
    STATE.with(|cell: &RefCell<State>| match &mut *cell.borrow_mut() {
        State::On(collector) => Some(f(collector)),
        State::Uninit | State::Off => None,
    })
}

pub(super) fn begin_frame() {
    STATE.with(|cell: &RefCell<State>| {
        let mut state = cell.borrow_mut();

        if matches!(*state, State::Uninit) {
            *state = match Config::from_lookup(|key: &str| env::var(key).ok()) {
                Some(config) => State::On(Box::new(Collector::new(config))),
                None => State::Off,
            };
        }

        if let State::On(collector) = &mut *state {
            collector.begin_frame(Instant::now);
        }
    });
}

pub(super) fn phase(name: &'static str) {
    with_collector(|collector: &mut Collector| collector.phase(name, Instant::now));
}

pub(super) fn end_frame() -> bool {
    with_collector(|collector: &mut Collector| collector.end_frame(Instant::now)).unwrap_or(false)
}

/// Takes the collector out before writing, so a frame that still runs before the event loop
/// stops neither samples nor writes the report a second time.
pub(super) fn finish(meta: Value) -> Result<(), RedixelError> {
    STATE.with(|cell: &RefCell<State>| match cell.replace(State::Off) {
        State::On(collector) => write_report(&collector, meta),
        State::Uninit | State::Off => Ok(()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(warmup: u64, frames: u64, interval: u64) -> Config {
        Config {
            warmup,
            frames,
            interval,
            out: String::new(),
            variant: String::from("test"),
        }
    }

    fn lookup(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key: &str| {
            pairs
                .iter()
                .find(|(name, _): &&(&str, &str)| *name == key)
                .map(|(_, value): &(&str, &str)| value.to_string())
        }
    }

    /// Plays one frame against a fake clock: each `(name, ms)` opens a phase that lasts
    /// `ms`, and the frame ends when the last one does.
    fn play(collector: &mut Collector, base: Instant, phases: &[(&'static str, u64)]) -> bool {
        let mut elapsed: u64 = 0;

        collector.begin_frame(|| base);
        for &(name, ms) in phases {
            let at: Instant = base + Duration::from_millis(elapsed);
            collector.phase(name, || at);
            elapsed += ms;
        }

        let end: Instant = base + Duration::from_millis(elapsed);
        collector.end_frame(|| end)
    }

    fn series(collector: &Collector, name: &str) -> Vec<Duration> {
        collector
            .phases
            .iter()
            .find(|(known, _): &&(&'static str, Vec<Duration>)| *known == name)
            .map(|(_, samples): &(&'static str, Vec<Duration>)| samples.clone())
            .unwrap_or_default()
    }

    #[test]
    fn stays_off_unless_requested() {
        assert_eq!(Config::from_lookup(lookup(&[])), None);
        assert_eq!(Config::from_lookup(lookup(&[("REDIXEL_PROFILE", "0")])), None);
        assert_eq!(Config::from_lookup(lookup(&[("REDIXEL_PROFILE", "")])), None);
    }

    #[test]
    fn defaults_apply_when_only_enabled() {
        let resolved: Config = Config::from_lookup(lookup(&[("REDIXEL_PROFILE", "1")])).unwrap();

        assert_eq!(
            resolved,
            Config {
                warmup:   DEFAULT_WARMUP,
                frames:   DEFAULT_FRAMES,
                interval: DEFAULT_INTERVAL,
                out:      DEFAULT_OUT.to_string(),
                variant:  DEFAULT_VARIANT.to_string(),
            }
        );
    }

    #[test]
    fn every_setting_is_read_from_its_variable() {
        let resolved: Config = Config::from_lookup(lookup(&[
            ("REDIXEL_PROFILE", "1"),
            ("REDIXEL_PROFILE_WARMUP", "5"),
            ("REDIXEL_PROFILE_FRAMES", "50"),
            ("REDIXEL_PROFILE_INTERVAL", "4"),
            ("REDIXEL_PROFILE_OUT", "out.json"),
            ("REDIXEL_PROFILE_VARIANT", "wgpu-hal"),
        ]))
        .unwrap();

        assert_eq!(
            resolved,
            Config {
                warmup:   5,
                frames:   50,
                interval: 4,
                out:      String::from("out.json"),
                variant:  String::from("wgpu-hal"),
            }
        );
    }

    #[test]
    fn a_bad_number_falls_back_and_a_zero_count_is_raised_to_one() {
        let resolved: Config = Config::from_lookup(lookup(&[
            ("REDIXEL_PROFILE", "1"),
            ("REDIXEL_PROFILE_WARMUP", "many"),
            ("REDIXEL_PROFILE_FRAMES", "0"),
            ("REDIXEL_PROFILE_INTERVAL", "0"),
        ]))
        .unwrap();

        assert_eq!(resolved.warmup, DEFAULT_WARMUP);
        assert_eq!(resolved.frames, 1);
        assert_eq!(resolved.interval, 1);
    }

    #[test]
    fn warmup_frames_are_dropped_and_the_run_ends_on_the_last_sample() {
        let base: Instant = Instant::now();
        let mut collector: Collector = Collector::new(config(2, 3, 1));
        let frame: [(&'static str, u64); 1] = [("update", 4)];

        let done: Vec<bool> = (0..5).map(|_| play(&mut collector, base, &frame)).collect();

        assert_eq!(done, [false, false, false, false, true]);
        assert_eq!(series(&collector, "update").len(), 3);
        assert_eq!(collector.totals.len(), 3);
    }

    #[test]
    fn unsampled_frames_never_read_the_clock() {
        let mut collector: Collector = Collector::new(config(1, 1, 1));
        let no_clock = || -> Instant { panic!("the clock was read on an unsampled frame") };

        collector.begin_frame(no_clock);
        collector.phase("update", no_clock);

        assert!(!collector.end_frame(no_clock));
    }

    #[test]
    fn the_interval_samples_one_frame_in_n() {
        let base: Instant = Instant::now();
        let mut collector: Collector = Collector::new(config(0, 2, 3));
        let frame: [(&'static str, u64); 1] = [("update", 1)];

        let done: Vec<bool> = (0..4).map(|_| play(&mut collector, base, &frame)).collect();

        assert_eq!(done, [false, false, false, true]);
        assert_eq!(collector.totals.len(), 2);
    }

    #[test]
    fn a_repeated_phase_adds_up_into_one_sample_per_frame() {
        let base: Instant = Instant::now();
        let mut collector: Collector = Collector::new(config(0, 1, 1));

        play(&mut collector, base, &[("draw", 10), ("submit", 5), ("draw", 5)]);

        assert_eq!(series(&collector, "draw"), [Duration::from_millis(15)]);
        assert_eq!(series(&collector, "submit"), [Duration::from_millis(5)]);
    }

    #[test]
    fn the_phases_add_up_to_the_frame() {
        let base: Instant = Instant::now();
        let mut collector: Collector = Collector::new(config(0, 1, 1));

        play(&mut collector, base, &[("update", 4), ("draw", 6)]);

        assert_eq!(collector.totals, [Duration::from_millis(10)]);
        assert_eq!(series(&collector, "update"), [Duration::from_millis(4)]);
        assert_eq!(series(&collector, "draw"), [Duration::from_millis(6)]);
    }

    #[test]
    fn summary_figures() {
        let samples: Vec<Duration> = (1..=4).map(Duration::from_millis).collect();
        let summary: Value = summarize(&samples);

        assert_eq!(summary["n"], 4);
        assert_eq!(summary["mean_ms"], 2.5);
        assert_eq!(summary["median_ms"], 2.5);
        assert_eq!(summary["p95_ms"], 4.0);
        assert_eq!(summary["min_ms"], 1.0);
        assert_eq!(summary["max_ms"], 4.0);
        assert_eq!(summary["stddev_ms"], 1.291);
    }

    #[test]
    fn an_empty_series_summarises_to_zero_samples() {
        assert_eq!(summarize(&[]), json!({ "n": 0 }));
    }

    #[test]
    fn the_report_keeps_the_phase_order_and_adds_the_run_metadata() {
        let base: Instant = Instant::now();
        let mut collector: Collector = Collector::new(config(0, 1, 1));
        play(&mut collector, base, &[("update", 1), ("draw", 1), ("present", 1)]);

        let report: Value = collector.report(json!({ "app": "Demo" }));

        let order: Vec<&String> = report["phases"].as_object().unwrap().keys().collect();
        assert_eq!(order, ["update", "draw", "present"]);
        assert_eq!(report["schema"], SCHEMA);
        assert_eq!(report["meta"]["app"], "Demo");
        assert_eq!(report["meta"]["variant"], "test");
        assert_eq!(report["meta"]["os"], env::consts::OS);
        assert_eq!(report["config"]["frames"], 1);
        assert_eq!(report["frame"]["n"], 1);
    }

    #[test]
    fn write_report_leaves_a_parseable_file() {
        let base: Instant = Instant::now();
        let path: std::path::PathBuf =
            env::temp_dir().join(format!("redixel-profile-test-{}.json", std::process::id()));
        let mut settings: Config = config(0, 1, 1);
        settings.out = path.to_string_lossy().into_owned();

        let mut collector: Collector = Collector::new(settings);
        play(&mut collector, base, &[("update", 2)]);

        write_report(&collector, json!({})).unwrap();

        let written: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        fs::remove_file(&path).ok();

        assert_eq!(written["phases"]["update"]["n"], 1);
    }

    #[test]
    fn finish_switches_the_profiler_off_so_the_report_is_written_once() {
        let base: Instant = Instant::now();
        let path: std::path::PathBuf =
            env::temp_dir().join(format!("redixel-profile-once-{}.json", std::process::id()));
        let mut settings: Config = config(0, 1, 1);
        settings.out = path.to_string_lossy().into_owned();

        let mut collector: Collector = Collector::new(settings);
        play(&mut collector, base, &[("update", 2)]);
        STATE.with(|cell: &RefCell<State>| cell.replace(State::On(Box::new(collector))));

        finish(json!({})).unwrap();
        fs::remove_file(&path).ok();

        finish(json!({})).unwrap();
        assert!(!path.exists(), "a second finish must not write the report again");
        assert!(!end_frame(), "no frame may complete a run after finish");
    }

    #[test]
    fn a_failed_write_names_the_report_path() {
        let mut settings: Config = config(0, 1, 1);
        settings.out = String::from("/nonexistent-redixel-dir/profile.json");
        let collector: Collector = Collector::new(settings);

        let message: String = write_report(&collector, json!({})).unwrap_err().to_string();

        assert!(message.contains("/nonexistent-redixel-dir/profile.json"), "{message}");
    }
}
