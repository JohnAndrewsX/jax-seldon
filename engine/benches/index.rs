//! `cargo bench --bench index`: the index build (read the logbook, derive,
//! write `index.json` atomically) must take < 100 ms (SPEC-ENGINE §6).
//! Asserted on the fixture logbook scaled ×10 (WP-007; `just bench`, CI).
//! ×150 (12 750 ledger lines, 1 200 cases: more cases than the 300 of the
//! budget) is always printed and asserted only with `SELDON_BENCH_X150=1`,
//! which `just check-perf` sets (WP-076): on a busy host a run comes out at
//! about twice its usual time. Median of 21 runs after a warm-up; a median
//! over budget is measured once more before the bench fails.
//!
//! Plain `std::time` (no bench crate: AGENTS.md §7). Everything is written
//! to a temp dir; no real XDG directory is touched.

#[path = "../tests/common/scale.rs"]
mod scale;

use std::path::Path;
use std::time::{Duration, Instant};

use chrono::DateTime;

use seldon::config::{Config, Dirs};
use seldon::index;
use seldon::logbook::Logbook;

const RUNS: usize = 21;
const BUDGET: Duration = Duration::from_millis(100);
/// Set to `1` to assert the ×150 case too (`just check-perf`).
const X150_ENV: &str = "SELDON_BENCH_X150";

fn main() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/logbook");
    let tmp = std::env::temp_dir().join(format!("seldon-bench-index-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let x150 = std::env::var(X150_ENV).is_ok_and(|v| v == "1");
    let mut over_budget = Vec::new();
    for (factor, asserted) in [(10, true), (150, x150)] {
        let median = bench(&fixture, &tmp.join(format!("x{factor}")), factor, asserted);
        if asserted && median >= BUDGET {
            over_budget.push(format!("×{factor}"));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    if !over_budget.is_empty() {
        eprintln!(
            "index build {} over the {BUDGET:?} budget",
            over_budget.join(", ")
        );
        std::process::exit(1);
    }
}

/// The median build time of the fixture scaled `factor` times; when
/// `asserted` and over [`BUDGET`], the median of a second measurement.
fn bench(fixture: &Path, dir: &Path, factor: usize, asserted: bool) -> Duration {
    let root = dir.join("logbook");
    let lines = scale::scaled_logbook(fixture, &root, factor);
    let logbook = Logbook::open(&root).expect("scaled logbook");
    let dirs = Dirs {
        home: dir.to_path_buf(),
        xdg_config_home: dir.join("config"),
        state_dir: dir.join("state"),
    };
    let config = Config::default();
    let now = DateTime::parse_from_rfc3339("2026-10-01T17:05:12+02:00").unwrap();
    let out = dirs.index_file();
    let mut median = Duration::MAX;
    for attempt in 1..=2 {
        let mut times: Vec<Duration> = (0..=RUNS)
            .map(|_| {
                let start = Instant::now();
                let built = index::derive_at(&dirs, &config, &logbook, now).expect("index");
                index::write(&out, &built.index).expect("write");
                start.elapsed()
            })
            .skip(1) // warm-up
            .collect();
        times.sort();
        median = times[RUNS / 2];
        let bytes = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
        let mode = if asserted { "asserted" } else { "printed only" };
        println!(
            "index build ×{factor:<3} ({lines} ledger lines, {} cases, {bytes} bytes): median {median:?}, min {:?}, max {:?} ({mode}, attempt {attempt})",
            8 * factor,
            times[0],
            times[RUNS - 1],
        );
        if !asserted || median < BUDGET {
            break;
        }
    }
    median
}
