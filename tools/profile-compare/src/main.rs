//! Compares two frame-profile reports (`redixel_profile`) phase by phase and says which run is faster.
//!
//! Usage: `profile-compare <before.json> <after.json>`

use std::{env, fs, process::ExitCode};

use serde_json::Value;

const SUPPORTED_SCHEMA: u64 = 1;

/// Report metadata that changes what a timing means. Two reports differing in any of these
/// measured different things, so a delta between them is not a verdict on the code.
const COMPARABLE_KEYS: [&str; 10] = [
    "app",
    "binary",
    "adapter",
    "present_mode",
    "surface",
    "target_fps",
    "tickrate",
    "os",
    "arch",
    "build",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Faster,
    Slower,
    Noise,
}

impl Verdict {
    fn label(self) -> &'static str {
        match self {
            Self::Faster => "faster",
            Self::Slower => "slower",
            Self::Noise => "within noise",
        }
    }
}

/// The figures of one series that the comparison reads.
#[derive(Debug, Clone, Copy)]
struct Stats {
    n:      f64,
    mean:   f64,
    median: f64,
    p95:    f64,
    stddev: f64,
}

impl Stats {
    /// `None` when the series is missing or has no samples.
    fn read(series: &Value) -> Option<Self> {
        let n: f64 = series["n"].as_f64().filter(|n: &f64| *n > 0.0)?;

        Some(Self {
            n,
            mean: series["mean_ms"].as_f64()?,
            median: series["median_ms"].as_f64()?,
            p95: series["p95_ms"].as_f64()?,
            stddev: series["stddev_ms"].as_f64()?,
        })
    }
}

/// Whether `b` is faster or slower than `a`: the means differ by more than twice the
/// standard error of their difference.
///
/// shortcut: consecutive frames are correlated, which makes the standard error optimistic
/// and the verdict overconfident on a single pair of runs; repeat each variant a few times
/// and trust a verdict only when it holds across the repeats.
fn verdict(a: &Stats, b: &Stats) -> Verdict {
    let standard_error: f64 = (a.stddev.powi(2) / a.n + b.stddev.powi(2) / b.n).sqrt();
    let difference: f64 = b.mean - a.mean;

    if difference.abs() <= 2.0 * standard_error {
        Verdict::Noise
    } else if difference < 0.0 {
        Verdict::Faster
    } else {
        Verdict::Slower
    }
}

/// Change from `a` to `b` in percent, or `None` when `a` is zero and no ratio exists.
fn delta_percent(a: f64, b: f64) -> Option<f64> {
    (a.abs() > f64::EPSILON).then(|| (b - a) / a * 100.0)
}

fn percent(delta: Option<f64>) -> String {
    delta.map_or_else(|| String::from("n/a"), |d: f64| format!("{d:+.1}%"))
}

/// Metadata keys on which the two reports disagree.
fn differences(a: &Value, b: &Value) -> Vec<&'static str> {
    COMPARABLE_KEYS
        .iter()
        .copied()
        .filter(|key: &&str| a["meta"][*key] != b["meta"][*key])
        .collect()
}

fn describe(label: &str, report: &Value) -> String {
    let meta: &Value = &report["meta"];
    let text = |key: &str| -> String { meta[key].as_str().unwrap_or("?").to_string() };

    format!(
        "{label}: variant={}  binary={}  app={}  adapter={} ({}, {})  present={}  surface={}  {}",
        text("variant"),
        text("binary"),
        text("app"),
        meta["adapter"]["name"].as_str().unwrap_or("?"),
        meta["adapter"]["backend"].as_str().unwrap_or("?"),
        meta["adapter"]["device_type"].as_str().unwrap_or("?"),
        text("present_mode"),
        meta["surface"],
        text("build"),
    )
}

/// One row of the table: `frame` or a phase, read from both reports.
fn row(name: &str, a: Option<Stats>, b: Option<Stats>) -> String {
    let (Some(a), Some(b)) = (a, b) else {
        return format!("{name:<18}  only in one of the reports");
    };

    format!(
        "{name:<18}  {:>9.4}  {:>9.4}  {:>8}  {:>9.4}  {:>9.4}  {:>8}  {}",
        a.median,
        b.median,
        percent(delta_percent(a.median, b.median)),
        a.p95,
        b.p95,
        percent(delta_percent(a.p95, b.p95)),
        verdict(&a, &b).label(),
    )
}

/// Phase names of `a` in its order, followed by those only `b` has.
fn phase_names(a: &Value, b: &Value) -> Vec<String> {
    let names = |report: &Value| -> Vec<String> {
        report["phases"]
            .as_object()
            .map(|phases: &serde_json::Map<String, Value>| phases.keys().cloned().collect())
            .unwrap_or_default()
    };

    let mut all: Vec<String> = names(a);
    for name in names(b) {
        if !all.contains(&name) {
            all.push(name);
        }
    }

    all
}

fn check_schema(path: &str, report: &Value) -> Result<(), String> {
    match report["schema"].as_u64() {
        Some(SUPPORTED_SCHEMA) => Ok(()),
        other => Err(format!(
            "{path}: unsupported report schema {other:?}, expected {SUPPORTED_SCHEMA}"
        )),
    }
}

/// The conclusion drawn from the `frame` series alone, which is what the whole frame cost.
fn conclusion(a: &Stats, b: &Stats) -> String {
    let change: String = percent(delta_percent(a.mean, b.mean));

    match verdict(a, b) {
        Verdict::Noise => format!(
            "frame: no significant difference (mean {:.4} -> {:.4} ms, {change}, n={}/{})",
            a.mean, b.mean, a.n, b.n
        ),
        found => format!(
            "frame: B is {} than A (mean {:.4} -> {:.4} ms, {change}, n={}/{})",
            found.label(),
            a.mean,
            b.mean,
            a.n,
            b.n
        ),
    }
}

fn compare(a: &Value, b: &Value) -> Result<String, String> {
    let mut lines: Vec<String> = vec![describe("A", a), describe("B", b)];

    let differing: Vec<&'static str> = differences(a, b);
    if !differing.is_empty() {
        lines.push(format!(
            "warning: the runs differ in {}; the numbers are not comparable.",
            differing.join(", ")
        ));
    }

    lines.push(String::new());
    lines.push(format!(
        "{:<18}  {:>9}  {:>9}  {:>8}  {:>9}  {:>9}  {:>8}  verdict (B vs A, ms)",
        "phase", "median A", "median B", "delta", "p95 A", "p95 B", "delta"
    ));

    let frame_a: Option<Stats> = Stats::read(&a["frame"]);
    let frame_b: Option<Stats> = Stats::read(&b["frame"]);
    lines.push(row("frame", frame_a, frame_b));

    for name in phase_names(a, b) {
        lines.push(row(&name, Stats::read(&a["phases"][&name]), Stats::read(&b["phases"][&name])));
    }

    let (Some(frame_a), Some(frame_b)) = (frame_a, frame_b) else {
        return Err(String::from("a report has no frame samples to compare"));
    };

    lines.push(String::new());
    lines.push(conclusion(&frame_a, &frame_b));

    Ok(lines.join("\n"))
}

fn read(path: &str) -> Result<Value, String> {
    let text: String = fs::read_to_string(path).map_err(|e: std::io::Error| format!("{path}: {e}"))?;
    let report: Value = serde_json::from_str(&text).map_err(|e: serde_json::Error| format!("{path}: {e}"))?;
    check_schema(path, &report)?;

    Ok(report)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let [before, after] = args.as_slice() else {
        eprintln!("usage: profile-compare <before.json> <after.json>");
        return ExitCode::from(2);
    };

    let result: Result<String, String> =
        read(before).and_then(|a: Value| read(after).and_then(|b: Value| compare(&a, &b)));

    match result {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("profile-compare: {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn stats(mean: f64, stddev: f64, n: f64) -> Stats {
        Stats {
            n,
            mean,
            median: mean,
            p95: mean,
            stddev,
        }
    }

    fn series(mean: f64, stddev: f64) -> Value {
        json!({ "n": 600, "mean_ms": mean, "median_ms": mean, "p95_ms": mean, "stddev_ms": stddev })
    }

    fn report(variant: &str, adapter: &str, frame_mean: f64) -> Value {
        json!({
            "schema": 1,
            "meta": { "variant": variant, "app": "Demo", "binary": "shooter", "adapter": { "name": adapter }, "present_mode": "Immediate" },
            "frame": series(frame_mean, 0.1),
            "phases": { "update": series(0.5, 0.05), "present": series(frame_mean - 0.5, 0.1) },
        })
    }

    #[test]
    fn a_clear_drop_in_the_mean_is_faster() {
        assert_eq!(verdict(&stats(2.0, 0.1, 600.0), &stats(1.5, 0.1, 600.0)), Verdict::Faster);
    }

    #[test]
    fn a_clear_rise_in_the_mean_is_slower() {
        assert_eq!(verdict(&stats(1.5, 0.1, 600.0), &stats(2.0, 0.1, 600.0)), Verdict::Slower);
    }

    #[test]
    fn a_difference_inside_the_spread_is_noise() {
        assert_eq!(verdict(&stats(2.00, 1.0, 10.0), &stats(2.05, 1.0, 10.0)), Verdict::Noise);
    }

    #[test]
    fn identical_zero_variance_runs_are_noise() {
        assert_eq!(verdict(&stats(2.0, 0.0, 10.0), &stats(2.0, 0.0, 10.0)), Verdict::Noise);
    }

    #[test]
    fn a_zero_baseline_has_no_percent_change() {
        assert_eq!(delta_percent(0.0, 1.0), None);
        assert_eq!(delta_percent(2.0, 1.0), Some(-50.0));
    }

    #[test]
    fn the_variant_label_is_not_a_difference_but_the_adapter_is() {
        let a: Value = report("before", "GPU one", 2.0);

        assert!(differences(&a, &report("after", "GPU one", 2.0)).is_empty());
        assert_eq!(differences(&a, &report("after", "GPU two", 2.0)), ["adapter"]);
    }

    #[test]
    fn the_report_concludes_from_the_frame_and_lists_every_phase() {
        let text: String = compare(&report("before", "GPU", 2.0), &report("after", "GPU", 1.5)).unwrap();

        assert!(text.contains("frame: B is faster than A"), "{text}");
        assert!(text.contains("update"), "{text}");
        assert!(text.contains("present"), "{text}");
        assert!(!text.contains("warning"), "{text}");
    }

    #[test]
    fn runs_of_different_games_are_flagged_even_under_the_same_app_name() {
        let a: Value = report("before", "GPU", 2.0);
        let mut b: Value = report("after", "GPU", 2.0);
        b["meta"]["binary"] = json!("pong");

        assert_eq!(differences(&a, &b), ["binary"]);
    }

    #[test]
    fn incomparable_runs_are_flagged() {
        let text: String = compare(&report("before", "GPU one", 2.0), &report("after", "GPU two", 1.5)).unwrap();

        assert!(text.contains("warning: the runs differ in adapter"), "{text}");
    }

    #[test]
    fn a_report_of_another_schema_is_rejected() {
        let mut other: Value = report("x", "GPU", 1.0);
        other["schema"] = json!(2);

        assert!(check_schema("x.json", &other).is_err());
    }
}
