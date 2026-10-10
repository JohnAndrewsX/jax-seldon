//! The first capture of a large package log (WP-198; SPEC-ENGINE §4
//! pacman, ADR-0033): `init --defaults` reads a synthetic `pacman.log` of
//! [`LOG_MIB`] MiB, nearly all of it older than the 90-day look-back,
//! within a stated time and peak-memory budget, and records exactly the
//! look-back's events; two captures after it write nothing.
//!
//! The log is generated in this run's temp dir (`TMPDIR`, on disk where
//! the host's `/tmp` is a RAM disk) and removed with it; it is never
//! committed. The peak is the largest resident size of a child process
//! this test waited for (`getrusage(RUSAGE_CHILDREN)`), so this binary
//! holds this one test only. `SELDON_PACMAN_STREAM_MIB=<n>` sets another
//! size for a manual run (the review of WP-119 measured 240 MiB).

mod common;

use std::io::{BufWriter, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

use chrono::{DateTime, FixedOffset};
use common::{Env, Snapper, json, stderr};

/// The default size of the log.
const LOG_MIB: u64 = 96;

/// The budget of the `init` run, debug build, the default size: time
/// (the run is killed and the test fails at [`DEADLINE`]) and peak
/// resident size. Before WP-198 a 240 MiB log took 60.6 s and 2.4 GB;
/// the whole log in memory alone is [`LOG_MIB`] MiB.
const TIME_BUDGET: Duration = Duration::from_secs(30);
const PEAK_BUDGET_KB: u64 = 64 * 1024;
const DEADLINE: Duration = Duration::from_secs(180);

/// The clock of the run: the look-back starts at 2026-07-17T00:00:00+02:00.
const NOW: &str = "2026-10-15T12:00:00+02:00";
const START: &str = "2026-07-17T00:00:00+02:00";

/// The packages of one old block, upgraded each time.
const OLD_PACKAGES: usize = 20;

fn ts(s: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s).unwrap()
}

/// pacman's time format.
fn stamp(t: DateTime<FixedOffset>) -> String {
    t.format("%Y-%m-%dT%H:%M:%S%z").to_string()
}

/// Writes the log: blocks of a full upgrade (Running line, transaction,
/// [`OLD_PACKAGES`] upgrades, scriptlet and hook lines) from 2024 up to
/// the evening before the look-back until the file is `mib` MiB, then a
/// transaction open across the look-back's start, then one install with
/// a dependency per day for 60 days. Returns the subjects the look-back
/// must record.
fn write_log(path: &Path, mib: u64) -> Vec<String> {
    let mut out = BufWriter::with_capacity(1 << 20, std::fs::File::create(path).unwrap());
    let mut block = Vec::new();
    for k in 0..OLD_PACKAGES {
        block.push(format!(
            "] [ALPM] upgraded old-package-{k} (1.{k}.0-1 -> 1.{k}.1-1)\n"
        ));
        if k % 4 == 0 {
            block.push(format!(
                "] [ALPM-SCRIPTLET] ==> post-upgrade of old-package-{k}: nothing to do\n"
            ));
        }
    }
    block.push("] [ALPM] running '30-systemd-daemon-reload.hook'...\n".into());
    let block_len: usize = block.iter().map(|l| l.len() + 26).sum::<usize>() + 4 * 26 + 100;
    let blocks = (mib * 1024 * 1024) as usize / block_len;
    let first = ts("2024-01-01T00:00:00+02:00");
    let last = ts("2026-07-16T20:00:00+02:00");
    let step = (last - first) / blocks as i32;
    for b in 0..blocks {
        let t = stamp(first + step * b as i32);
        let w = |out: &mut BufWriter<std::fs::File>, rest: &str| {
            out.write_all(b"[").unwrap();
            out.write_all(t.as_bytes()).unwrap();
            out.write_all(rest.as_bytes()).unwrap();
        };
        w(&mut out, "] [PACMAN] Running 'pacman -Syu --noconfirm'\n");
        w(&mut out, "] [ALPM] transaction started\n");
        for line in &block {
            w(&mut out, line);
        }
        w(&mut out, "] [ALPM] transaction completed\n");
    }
    // open across the look-back's start: only the later install is kept,
    // with the transaction's id and command
    out.write_all(
        b"[2026-07-16T23:59:00+0200] [PACMAN] Running 'pacman -S straddle-early straddle-late'\n\
          [2026-07-16T23:59:01+0200] [ALPM] transaction started\n\
          [2026-07-16T23:59:30+0200] [ALPM] installed straddle-early (1.0-1)\n\
          [2026-07-17T00:00:30+0200] [ALPM] installed straddle-late (1.0-1)\n\
          [2026-07-17T00:00:31+0200] [ALPM] transaction completed\n",
    )
    .unwrap();
    let mut want = vec!["straddle-late".to_string()];
    for d in 0..60 {
        let t = ts("2026-07-18T10:00:00+02:00") + chrono::Duration::days(d);
        let (s0, s1) = (stamp(t), stamp(t + chrono::Duration::seconds(1)));
        write!(
            out,
            "[{s0}] [PACMAN] Running 'pacman -S recent-{d}'\n\
             [{s0}] [ALPM] transaction started\n\
             [{s1}] [ALPM] installed dep-{d} (1.0-1)\n\
             [{s1}] [ALPM] installed recent-{d} (1.0-1)\n\
             [{s1}] [ALPM] transaction completed\n"
        )
        .unwrap();
        want.push(format!("dep-{d}"));
        want.push(format!("recent-{d}"));
    }
    out.flush().unwrap();
    want
}

/// The largest resident size of a child process this process waited
/// for, in kB (`getrusage(RUSAGE_CHILDREN)`, `ru_maxrss`; 64-bit Linux).
fn children_peak_kb() -> u64 {
    use std::ffi::c_long;
    #[repr(C)]
    struct Rusage {
        /// `ru_utime`, `ru_stime`
        times: [c_long; 4],
        maxrss: c_long,
        rest: [c_long; 13],
    }
    unsafe extern "C" {
        fn getrusage(who: i32, usage: *mut Rusage) -> i32;
    }
    const RUSAGE_CHILDREN: i32 = -1;
    let mut usage = Rusage {
        times: [0; 4],
        maxrss: 0,
        rest: [0; 13],
    };
    // SAFETY: `usage` is a valid, writable `struct rusage`
    assert_eq!(unsafe { getrusage(RUSAGE_CHILDREN, &mut usage) }, 0);
    u64::try_from(usage.maxrss).unwrap()
}

/// `seldon args…` with the sources stubbed (as `init.rs` `defaults`),
/// the package log at `log`, killed and failed at [`DEADLINE`]. Returns
/// the output and how long the run took.
fn run(env: &Env, log: &Path, args: &[&str]) -> (Output, Duration) {
    let tmp = env.tmp.path();
    let started = Instant::now();
    let mut child = env
        .command(args)
        .env("SELDON_NOW", NOW)
        .env("SELDON_PACMAN_LOG", log)
        .env("SELDON_PACMAN_DB_LOCK", tmp.join("no-db.lck"))
        .env("SELDON_OMARCHY", tmp.join("no-omarchy"))
        .env("SELDON_OMARCHY_PLUGINS_DIR", tmp.join("plugins"))
        .env("SELDON_THEME_FILE", tmp.join("theme.name"))
        .env("SELDON_HARDWARE_ROOT", common::hardware_root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // the outputs are small; read after the exit
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("seldon {args:?} did not end within {DEADLINE:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let took = started.elapsed();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    (
        Output {
            status,
            stdout,
            stderr,
        },
        took,
    )
}

#[test]
fn a_large_log_is_read_as_a_stream() {
    let mib = std::env::var("SELDON_PACMAN_STREAM_MIB")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(LOG_MIB);
    let env = Env::new(Snapper::NoPermissions);
    let log: PathBuf = env.tmp.path().join("pacman.log");
    let want = write_log(&log, mib);
    let size = std::fs::metadata(&log).unwrap().len();
    assert!(size >= mib * 1024 * 1024 * 9 / 10, "{size} bytes");

    let root = env.tmp.path().join("logbook");
    let before = children_peak_kb();
    let (out, took) = run(
        &env,
        &log,
        &[
            "init",
            "--defaults",
            "--path",
            root.to_str().unwrap(),
            "--json",
        ],
    );
    let peak = children_peak_kb();
    eprintln!(
        "init --defaults on a {} MiB log: {took:?}, peak {peak} kB (children before: {before} kB)",
        size / (1024 * 1024)
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["capture"]["since"], START);

    // exactly the look-back's events
    let ledger = common::ledger(&root);
    let pacman: Vec<&serde_json::Value> =
        ledger.iter().filter(|e| e["source"] == "pacman").collect();
    let mut got: Vec<&str> = pacman
        .iter()
        .filter_map(|e| e["subject"].as_str())
        .collect();
    got.sort_unstable();
    let mut want_sorted: Vec<&str> = want.iter().map(String::as_str).collect();
    want_sorted.sort_unstable();
    assert_eq!(got, want_sorted);
    let straddle = pacman
        .iter()
        .find(|e| e["subject"] == "straddle-late")
        .unwrap();
    assert_eq!(straddle["txId"], "tx-20260716T235901", "{straddle}");
    assert_eq!(
        straddle["meta"]["command"], "pacman -S straddle-early straddle-late",
        "{straddle}"
    );
    assert_eq!(straddle["explicit"], true, "{straddle}");

    // idempotency: two more captures write nothing
    for n in 1..=2 {
        let (out, took) = run(&env, &log, &["capture", "--all", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(
            json(&out)["written"],
            0,
            "capture {n}: {}",
            common::stdout(&out)
        );
        assert!(took < TIME_BUDGET, "capture {n} took {took:?}");
    }
    assert_eq!(
        common::ledger(&root).len(),
        ledger.len(),
        "the ledger is unchanged"
    );

    // the budget holds at the default size; a manual run at another size
    // reports the numbers only
    if mib == LOG_MIB {
        assert!(took < TIME_BUDGET, "init took {took:?}");
        assert!(peak < PEAK_BUDGET_KB, "peak {peak} kB");
    }
}
