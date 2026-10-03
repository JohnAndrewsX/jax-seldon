//! `seldon watch [--interval SECS]` (SPEC-ENGINE §3, ADR-0005, WP-034):
//! rebuilds `index.json` when the logbook changes. Optional and off by
//! default: compiled only with the cargo feature `watch`; without it the
//! command is a user error (exit 1).
//!
//! - Watches `ledger/`, `work/`, `journal/`, `decisions/`, `system/`,
//!   `memory/` and `areas/` recursively, and `.seldon/logbook.toml`. The logbook root is
//!   watched on its own (not recursively) only to pick up one of those
//!   folders when it is created later.
//! - Generated files never trigger a rebuild: `ledger/*.md` and
//!   `STATUS.md` (written by `seldon index`/`status`, which rebuild the
//!   index themselves), hidden files (`write_atomic` temp files, editor
//!   swap files), backups ending in `~`, and pure reads (inotify open and
//!   close-without-write: the rebuild reads every file it watches).
//! - The rebuild writes `index.json` only (state dir, outside the watch),
//!   never a logbook file, so it cannot trigger itself.
//! - One rebuild right after start, so the index reflects edits made while
//!   the watcher was down; after that it reacts to changes.
//! - Debounce: a rebuild runs once the logbook has been quiet for
//!   `--interval` seconds (default and minimum 2), at the latest five
//!   intervals after the first change of a burst.
//! - The rebuild takes the state lock. A held lock delays it (retried every
//!   250 ms); it never fails the watcher.
//! - One line per rebuild: text on stderr, or with `--json` one JSON
//!   object per line on stdout (`watching`, `rebuilt`, `error`, and the
//!   final `stopped`).
//! - SIGTERM and SIGINT end the watcher after the rebuild in progress
//!   (exit 0); a second signal kills it.
//! - Exit 3 when the logbook is not initialised (at start, or when it
//!   disappears); exit 2 when the watches cannot be set up at start (the
//!   inotify watch limit). A watch that fails later (a folder that vanished
//!   or was replaced) is an `error` line; the watcher goes on.
//! - After an inotify overflow (rescan) the seven folders are watched afresh,
//!   so a subfolder whose create event was lost is not left unwatched.

use clap::Args;

#[cfg(not(feature = "watch"))]
use super::{Context, Output};
#[cfg(not(feature = "watch"))]
use crate::error::{Error, Result};

/// The default and smallest `--interval`, in seconds.
pub const MIN_INTERVAL: u64 = 2;

#[derive(Debug, Clone, Args)]
pub struct WatchArgs {
    /// Quiet time before the index is rebuilt, in seconds (at least 2)
    #[arg(
        long,
        value_name = "SECS",
        default_value_t = MIN_INTERVAL,
        value_parser = clap::value_parser!(u64).range(MIN_INTERVAL..=3600)
    )]
    pub interval: u64,
}

#[cfg(not(feature = "watch"))]
pub fn run(_ctx: &Context, _args: WatchArgs) -> Result<Output> {
    Err(Error::user(
        "built without the watch feature; rebuild seldon with `cargo build --release --features watch`",
    ))
}

#[cfg(feature = "watch")]
pub use imp::run;

#[cfg(feature = "watch")]
mod imp {
    use std::collections::BTreeSet;
    use std::io::Write as _;
    use std::path::{Component, Path};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use chrono::{DateTime, FixedOffset, Local, TimeDelta, Timelike as _};
    use notify::{EventKind, RecursiveMode, Watcher as _};
    use serde_json::{Value, json};

    use super::super::{Context, NOW_ENV, Output};
    use super::WatchArgs;
    use crate::error::{Error, Result};
    use crate::index::{self, Built};
    use crate::logbook::Logbook;

    /// Watched recursively (WP-034); everything the index reads (`areas/`
    /// for `system.areas`, WP-075).
    pub(super) const DIRS: [&str; 7] = [
        "ledger",
        "work",
        "journal",
        "decisions",
        "system",
        "memory",
        "areas",
    ];
    /// Watched through its folder: editors and `write_atomic` replace it.
    const META: &str = ".seldon/logbook.toml";
    /// Generated at the root by `seldon status`.
    const GENERATED_ROOT: [&str; 1] = ["STATUS.md"];
    /// How often the loop looks at the signal flag while idle.
    const TICK: Duration = Duration::from_millis(500);
    /// Retry delay while another `seldon` holds the lock.
    const LOCK_RETRY: Duration = Duration::from_millis(250);
    /// A steady stream of changes still rebuilds after this many intervals.
    const MAX_WAIT_INTERVALS: u32 = 5;
    /// Changed paths named in one log line.
    const PATHS_SHOWN: usize = 10;

    pub fn run(ctx: &Context, args: WatchArgs) -> Result<Output> {
        let config = ctx.load_config()?.unwrap_or_default();
        let (root, _) = ctx.resolve_logbook(None, Some(&config));
        if !Logbook::is_initialised(&root) {
            return Err(Error::NotInitialised(root));
        }
        // the rebuilds use this logbook even if config.toml changes later
        let mut ctx = ctx.clone();
        ctx.logbook_flag = Some(root.clone());
        let clock = Clock::new(&ctx);
        let interval = Duration::from_secs(args.interval);

        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx)
            .map_err(|e| anyhow::anyhow!("cannot start the file watcher: {e}"))?;
        let mut watched = BTreeSet::new();
        let failed = watch_folders(&mut watcher, &root, &mut watched);
        if !failed.is_empty() {
            return Err(anyhow::anyhow!("{}", failed.join("; ")).into());
        }
        signals::install();

        let log = Log { json: ctx.json };
        // what is watched, as the user sees it: `ledger/`, …, the meta file
        let targets: Vec<String> = DIRS
            .iter()
            .filter(|d| watched.contains(**d))
            .map(|d| format!("{d}/"))
            .chain(watched.contains(".seldon").then(|| META.to_string()))
            .collect();
        log.line(
            json!({
                "status": "watching",
                "logbook": root,
                "interval": args.interval,
                "paths": targets,
            }),
            format!(
                "watching {} ({}; rebuild after {} s quiet)",
                root.display(),
                targets.join(" "),
                args.interval
            ),
        );

        // the first rebuild runs at once: edits made while nothing watched
        let mut pending = Some(Pending::start(Instant::now()));
        let mut rebuilds = 0usize;
        let signal = loop {
            if let Some(signal) = signals::received() {
                break signal;
            }
            let now = Instant::now();
            let wait = pending
                .as_ref()
                .map_or(TICK, |p| p.due(interval).saturating_duration_since(now))
                .min(TICK);
            match rx.recv_timeout(wait) {
                Ok(Ok(event)) => {
                    let Some(paths) = relevant(&root, &event) else {
                        continue;
                    };
                    // a watched folder appeared, went or was replaced: drop
                    // its old watch and watch what is there now; after a
                    // rescan (lost events) all seven, for subfolders whose
                    // create event was lost
                    let moved: Vec<&str> = DIRS
                        .into_iter()
                        .filter(|d| event.need_rescan() || paths.iter().any(|p| p == d))
                        .collect();
                    if !moved.is_empty() {
                        for e in rewatch(&mut watcher, &root, &mut watched, &moved) {
                            log.error(&e);
                        }
                    }
                    pending
                        .get_or_insert_with(|| Pending::new(Instant::now()))
                        .add(paths);
                    continue;
                }
                // a watch error (inotify queue overflow, a watch limit):
                // the next rebuild reads everything anyway
                Ok(Err(e)) => {
                    log.error(&format!("file watcher: {e}"));
                    pending
                        .get_or_insert_with(|| Pending::new(Instant::now()))
                        .add(Vec::new());
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(anyhow::anyhow!("the file watcher stopped").into());
                }
            }
            let Some(p) = pending.as_mut() else {
                continue;
            };
            if Instant::now() < p.due(interval) {
                continue;
            }
            let start = Instant::now();
            match rebuild(&ctx, clock.now()) {
                Ok(built) => {
                    rebuilds += 1;
                    log.rebuilt(&built, p, start.elapsed());
                    pending = None;
                }
                Err(Error::LockHeld(_)) => p.retry_at = Some(Instant::now() + LOCK_RETRY),
                Err(e @ Error::NotInitialised(_)) => return Err(e),
                Err(e) => {
                    log.error(&format!("index.json not rebuilt: {e}"));
                    pending = None;
                }
            }
        };
        drop(watcher);
        // the final line goes through `main` (stdout) with --json
        if !ctx.json {
            log.line(
                Value::Null,
                format!("stopped ({signal}) after {rebuilds} rebuild(s)"),
            );
        }
        Ok(Output::ok(
            "",
            json!({ "status": "stopped", "signal": signal, "rebuilds": rebuilds }),
        ))
    }

    /// Changes seen since the last rebuild.
    struct Pending {
        first: Instant,
        last: Instant,
        /// Relative paths, `/`-separated.
        paths: BTreeSet<String>,
        /// Set while another writer holds the lock.
        retry_at: Option<Instant>,
        /// The rebuild at start (no change behind it).
        at_start: bool,
    }

    impl Pending {
        fn new(now: Instant) -> Self {
            Pending {
                first: now,
                last: now,
                paths: BTreeSet::new(),
                retry_at: None,
                at_start: false,
            }
        }

        fn add(&mut self, paths: Vec<String>) {
            self.last = Instant::now();
            self.paths.extend(paths);
        }

        /// The rebuild at start: due at once.
        fn start(now: Instant) -> Self {
            Pending {
                at_start: true,
                ..Pending::new(now)
            }
        }

        /// When the rebuild runs: at once for the one at start, else
        /// `interval` after the last change, at the latest
        /// [`MAX_WAIT_INTERVALS`] after the first; while the lock is held,
        /// not before the next retry.
        fn due(&self, interval: Duration) -> Instant {
            let due = if self.at_start {
                self.first
            } else {
                (self.last + interval).min(self.first + interval * MAX_WAIT_INTERVALS)
            };
            self.retry_at.map_or(due, |r| r.max(due))
        }
    }

    /// The paths of `event` that matter, relative to `root`, or `None` when
    /// the event cannot change the index. A rescan (inotify overflow: events
    /// were lost) and an event without paths matter.
    pub(super) fn relevant(root: &Path, event: &notify::Event) -> Option<Vec<String>> {
        if matches!(event.kind, EventKind::Access(_)) {
            return None;
        }
        if event.paths.is_empty() || event.need_rescan() {
            return Some(Vec::new());
        }
        let paths: Vec<String> = event
            .paths
            .iter()
            .filter_map(|p| relevant_path(root, p))
            .collect();
        (!paths.is_empty()).then_some(paths)
    }

    /// `path` relative to `root` when a change to it can change the index.
    pub(super) fn relevant_path(root: &Path, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(root).ok()?;
        let parts: Vec<&str> = rel
            .components()
            .map(|c| match c {
                Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .collect::<Option<_>>()?;
        let joined = parts.join("/");
        if joined == META {
            return Some(joined);
        }
        let (first, last) = (*parts.first()?, *parts.last()?);
        if !DIRS.contains(&first) || GENERATED_ROOT.contains(&joined.as_str()) {
            return None;
        }
        let scratch = parts.iter().any(|p| p.starts_with('.'))
            || last.ends_with('~')
            || last.ends_with(".swp")
            || last.ends_with(".swx");
        let view = first == "ledger" && parts.len() == 2 && last.ends_with(".md");
        (!scratch && !view).then_some(joined)
    }

    /// Adds a watch for every watched folder that exists and is not watched
    /// yet; the root and `.seldon/` once, not recursively. Returns the
    /// watches that failed (left out of `watched`, so the next try repeats
    /// them).
    fn watch_folders(
        watcher: &mut notify::RecommendedWatcher,
        root: &Path,
        watched: &mut BTreeSet<String>,
    ) -> Vec<String> {
        let mut failed = Vec::new();
        let flat = [(".", root.to_path_buf()), (".seldon", root.join(".seldon"))];
        let deep = DIRS.iter().map(|d| (*d, root.join(d)));
        for (name, path) in flat.into_iter().chain(deep) {
            if watched.contains(name) || !path.is_dir() {
                continue;
            }
            let mode = if DIRS.contains(&name) {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            match watcher.watch(&path, mode) {
                Ok(()) => {
                    watched.insert(name.to_string());
                }
                Err(e) => failed.push(format!("cannot watch {}: {e}", path.display())),
            }
        }
        failed
    }

    /// Drops the watches of `names` (of [`DIRS`]) and watches what is there
    /// now; returns the watches that failed.
    fn rewatch(
        watcher: &mut notify::RecommendedWatcher,
        root: &Path,
        watched: &mut BTreeSet<String>,
        names: &[&str],
    ) -> Vec<String> {
        for name in names {
            // fails for a folder that is gone (inotify dropped the watch)
            let _ = watcher.unwatch(&root.join(name));
            watched.remove(*name);
        }
        watch_folders(watcher, root, watched)
    }

    /// `index.json` from the logbook, under the state lock. Writes nothing
    /// in the logbook (the views stay with `seldon index`/`status`), so the
    /// watcher never sees its own writes.
    fn rebuild(ctx: &Context, now: DateTime<FixedOffset>) -> Result<Built> {
        let config = ctx.load_config()?.unwrap_or_default();
        let (root, _) = ctx.resolve_logbook(None, Some(&config));
        let _lock = ctx.lock()?;
        if !Logbook::is_initialised(&root) {
            return Err(Error::NotInitialised(root));
        }
        let logbook = Logbook::open(&root)?;
        let mut built = index::derive_at(&ctx.dirs, &config, &logbook, now)?;
        built.index.logbook.git = index::git_info(&logbook.root);
        index::write(&ctx.dirs.index_file(), &built.index)?;
        Ok(built)
    }

    /// The clock of each rebuild: the real time, or `SELDON_NOW` moved on
    /// by the time the watcher has been running (tests).
    struct Clock {
        fixed: Option<(DateTime<FixedOffset>, Instant)>,
    }

    impl Clock {
        fn new(ctx: &Context) -> Self {
            let fixed = std::env::var_os(NOW_ENV).is_some_and(|v| !v.is_empty());
            Clock {
                fixed: fixed.then(|| (ctx.now, Instant::now())),
            }
        }

        fn now(&self) -> DateTime<FixedOffset> {
            let now = match self.fixed {
                Some((at, since)) => {
                    at + TimeDelta::from_std(since.elapsed()).unwrap_or(TimeDelta::zero())
                }
                None => Local::now().fixed_offset(),
            };
            now.with_nanosecond(0).unwrap_or(now)
        }
    }

    struct Log {
        json: bool,
    }

    impl Log {
        /// One JSON line on stdout, or one text line on stderr.
        fn line(&self, json: Value, text: String) {
            if self.json {
                let _ = writeln!(std::io::stdout().lock(), "{json}");
            } else {
                let _ = writeln!(std::io::stderr().lock(), "{} seldon watch: {text}", stamp());
            }
        }

        fn rebuilt(&self, built: &Built, p: &Pending, took: Duration) {
            let ix = &built.index;
            let shown: Vec<&String> = p.paths.iter().take(PATHS_SHOWN).collect();
            let more = p.paths.len().saturating_sub(PATHS_SHOWN);
            let trigger = if p.at_start { "start" } else { "changes" };
            let mut text = format!(
                "index rebuilt ({} event(s), {} open drift) in {} ms ",
                ix.events.len(),
                ix.summary.open_drift,
                took.as_millis(),
            );
            if p.at_start {
                text.push_str("at start");
            } else {
                text.push_str(&format!("after {} change(s)", p.paths.len()));
            }
            if !shown.is_empty() {
                text.push_str(": ");
                text.push_str(
                    &shown
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                if more > 0 {
                    text.push_str(&format!(" and {more} more"));
                }
            }
            for w in &built.warnings {
                text.push_str(&format!("\n  warning: {w}"));
            }
            self.line(
                json!({
                    "status": "rebuilt",
                    "trigger": trigger,
                    "generatedAt": ix.generated_at,
                    "events": ix.events.len(),
                    "summary": ix.summary,
                    "changes": p.paths.len(),
                    "paths": shown,
                    "durationMs": took.as_millis() as u64,
                    "warnings": built.warnings,
                }),
                text,
            );
        }

        fn error(&self, message: &str) {
            self.line(
                json!({ "status": "error", "message": message }),
                format!("error: {message}"),
            );
        }
    }

    /// Local wall-clock time for the text log.
    fn stamp() -> String {
        Local::now().format("%H:%M:%S").to_string()
    }

    /// SIGTERM/SIGINT handling with the C library's `signal(2)` (no extra
    /// crate): the handler records the signal and restores the default
    /// action, so a second signal ends the process at once.
    mod signals {
        use std::os::raw::c_int;
        use std::sync::atomic::{AtomicI32, Ordering};

        const SIGINT: c_int = 2;
        const SIGTERM: c_int = 15;
        const SIG_DFL: usize = 0;

        static RECEIVED: AtomicI32 = AtomicI32::new(0);

        unsafe extern "C" {
            fn signal(signum: c_int, handler: usize) -> usize;
        }

        extern "C" fn on_signal(signum: c_int) {
            RECEIVED.store(signum, Ordering::SeqCst);
            // SAFETY: signal(2) is async-signal-safe (POSIX.1-2008 §2.4.3).
            unsafe { signal(signum, SIG_DFL) };
        }

        pub fn install() {
            for signum in [SIGINT, SIGTERM] {
                // SAFETY: the handler only stores to an atomic and calls an
                // async-signal-safe function.
                unsafe { signal(signum, on_signal as extern "C" fn(c_int) as usize) };
            }
        }

        pub fn received() -> Option<&'static str> {
            match RECEIVED.load(Ordering::SeqCst) {
                SIGINT => Some("SIGINT"),
                SIGTERM => Some("SIGTERM"),
                _ => None,
            }
        }
    }
}

#[cfg(all(test, feature = "watch"))]
mod tests {
    use std::path::{Path, PathBuf};

    use notify::event::{AccessKind, AccessMode, CreateKind, DataChange, Flag, ModifyKind};
    use notify::{Event, EventKind};

    use super::imp::{relevant, relevant_path};

    fn rel(p: &str) -> Option<String> {
        relevant_path(Path::new("/lb"), &Path::new("/lb").join(p))
    }

    #[test]
    fn sources_count_generated_and_scratch_files_do_not() {
        for p in [
            "ledger/2026-10.jsonl",
            "work/active/C-2026-001-x.md",
            "work/C-2026-001/script.sh",
            "journal/2026/2026-10-01.md",
            "decisions/ADR-0001-x.md",
            "system/packages.md",
            "memory/lessons.md",
            "areas/printer/README.md",
            "areas/printer",
            "areas",
            ".seldon/logbook.toml",
            "work",
        ] {
            assert_eq!(rel(p).as_deref(), Some(p), "{p}");
        }
        for p in [
            "ledger/2026-10.md",
            "ledger/.2026-10.md.tmp-42",
            "STATUS.md",
            "PROJECT.md",
            "outputs/REBUILD.md",
            ".seldon/active-case",
            ".git/index",
            "work/active/.C-2026-001-x.md.swp",
            "work/active/C-2026-001-x.md~",
            "journal/.obsidian/x.json",
        ] {
            assert_eq!(rel(p), None, "{p}");
        }
        assert_eq!(
            relevant_path(Path::new("/lb"), Path::new("/elsewhere/x")),
            None
        );
    }

    #[test]
    fn reads_do_not_count_and_a_rescan_does() {
        let event = |kind, p: &str| Event::new(kind).add_path(PathBuf::from("/lb").join(p));
        let root = Path::new("/lb");
        let read = event(
            EventKind::Access(AccessKind::Close(AccessMode::Read)),
            "memory/lessons.md",
        );
        assert_eq!(relevant(root, &read), None);
        let open = event(
            EventKind::Access(AccessKind::Open(AccessMode::Any)),
            "ledger/2026-10.jsonl",
        );
        assert_eq!(relevant(root, &open), None);
        let write = event(
            EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            "memory/lessons.md",
        );
        assert_eq!(
            relevant(root, &write),
            Some(vec!["memory/lessons.md".into()])
        );
        let view = event(EventKind::Create(CreateKind::File), "ledger/2026-10.md");
        assert_eq!(relevant(root, &view), None);
        assert_eq!(
            relevant(root, &Event::new(EventKind::Other)),
            Some(Vec::new())
        );
        let rescan = event(EventKind::Other, ".git/index").set_flag(Flag::Rescan);
        assert_eq!(relevant(root, &rescan), Some(Vec::new()));
    }
}
