//! `seldon watch` (WP-034, ADR-0005). With the `watch` feature
//! (`cargo test --features watch`, `just check-watch`): a change triggers
//! exactly one debounced rebuild after the one at start, a burst one,
//! generated files and reads none, a held lock delays the rebuild without
//! failing, a replaced folder is watched again, a new area counts (WP-075),
//! SIGTERM/SIGINT end it with exit 0, and the resident size stays under 10 MB on the ×10 fixture. Without the feature: a clear user error (exit 1).
//! Everything runs in a temp home (common::Env, SELDON_TEST_GUARD).

mod common;

#[cfg(not(feature = "watch"))]
#[test]
fn without_the_feature_watch_is_a_user_error() {
    use common::{Env, Snapper};

    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let out = env.seldon(&["watch"]);
    assert_eq!(out.status.code(), Some(1), "{}", common::stderr(&out));
    assert!(
        common::stderr(&out).contains("built without the watch feature"),
        "{}",
        common::stderr(&out)
    );
    let out = env.seldon(&["watch", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(common::json(&out)["error"]["code"], serde_json::json!(1));
}

#[cfg(feature = "watch")]
mod with_feature {
    use std::io::{BufRead as _, BufReader, Read as _};
    use std::path::Path;
    use std::process::{Child, Command, Stdio};
    use std::sync::mpsc::{self, Receiver};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use serde_json::{Value, json};

    use super::common::{self, Env, Snapper, TempDir, fixture_logbook, read};

    /// The default (and smallest) `--interval`.
    const INTERVAL: Duration = Duration::from_secs(2);
    /// Slack for a loaded machine (`just check` runs the suites in parallel).
    const SLACK: Duration = Duration::from_secs(4);

    /// A running `seldon watch --json` and its output lines.
    struct Watch {
        child: Child,
        lines: Receiver<Value>,
        stderr: Arc<Mutex<String>>,
    }

    impl Watch {
        /// Starts `seldon watch --json extra…` and waits for its
        /// `watching` line and the rebuild at start.
        fn start(env: &Env, extra: &[&str]) -> Self {
            let w = Self::spawn(env.command(&watch_args(extra)));
            let first = w.rebuilt(SLACK);
            assert_eq!(first["trigger"], json!("start"), "{first}");
            w
        }

        /// Runs `cmd` and waits for the `watching` line only.
        fn spawn(mut cmd: Command) -> Self {
            let mut child = cmd
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn seldon watch");
            let (tx, lines) = mpsc::channel();
            let stdout = child.stdout.take().unwrap();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let line = line.unwrap();
                    let v = serde_json::from_str(&line)
                        .unwrap_or_else(|e| panic!("not JSON ({e}): {line}"));
                    if tx.send(v).is_err() {
                        break;
                    }
                }
            });
            let stderr = Arc::new(Mutex::new(String::new()));
            let mut pipe = child.stderr.take().unwrap();
            let sink = Arc::clone(&stderr);
            std::thread::spawn(move || {
                let mut text = String::new();
                let _ = pipe.read_to_string(&mut text);
                sink.lock().unwrap().push_str(&text);
            });
            let w = Watch {
                child,
                lines,
                stderr,
            };
            let first = w
                .next(Duration::from_secs(10))
                .unwrap_or_else(|| panic!("no `watching` line; stderr: {}", w.stderr()));
            assert_eq!(first["status"], json!("watching"), "{first}");
            w
        }

        fn stderr(&self) -> String {
            self.stderr.lock().unwrap().clone()
        }

        fn next(&self, timeout: Duration) -> Option<Value> {
            self.lines.recv_timeout(timeout).ok()
        }

        /// The next `rebuilt` line within `timeout` (any other line fails).
        fn rebuilt(&self, timeout: Duration) -> Value {
            let line = self
                .next(timeout)
                .unwrap_or_else(|| panic!("no rebuild in {timeout:?}; stderr: {}", self.stderr()));
            assert_eq!(line["status"], json!("rebuilt"), "{line}");
            line
        }

        /// Asserts that nothing is printed for `quiet`.
        fn assert_quiet(&self, quiet: Duration) {
            if let Some(line) = self.next(quiet) {
                panic!("unexpected line: {line}");
            }
        }

        fn pid(&self) -> u32 {
            self.child.id()
        }

        /// Sends `signal` and returns the exit code and the last line.
        fn stop(mut self, signal: &str) -> (Option<i32>, Option<Value>) {
            let sent = std::process::Command::new("kill")
                .args([&format!("-{signal}"), &self.pid().to_string()])
                .status()
                .expect("run kill");
            assert!(sent.success());
            let status = wait(&mut self.child, Duration::from_secs(10));
            let last = std::iter::from_fn(|| self.next(Duration::from_secs(2))).last();
            (status, last)
        }
    }

    impl Drop for Watch {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    fn watch_args<'a>(extra: &[&'a str]) -> Vec<&'a str> {
        let mut args = vec!["watch", "--json"];
        args.extend_from_slice(extra);
        args
    }

    /// The exit code of `child`, or a panic after `timeout`.
    fn wait(child: &mut Child, timeout: Duration) -> Option<i32> {
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                return status.code();
            }
            assert!(start.elapsed() < timeout, "the process did not exit");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn memory_file(topic: &str) -> String {
        format!("---\ntype: memory\ntopic: {topic}\nupdated: 2026-10-01\n---\n# {topic}\n")
    }

    fn index(env: &Env) -> Value {
        serde_json::from_str(&read(&env.home.join(".local/state/seldon/index.json"))).unwrap()
    }

    /// The memory topics in `index.json` (none before the first rebuild).
    fn topics(env: &Env) -> Vec<String> {
        let path = env.home.join(".local/state/seldon/index.json");
        if !path.exists() {
            return Vec::new();
        }
        index(env)["memory"]["topics"]
            .as_array()
            .map(|t| {
                t.iter()
                    .map(|t| t["topic"].as_str().unwrap().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_change_triggers_one_rebuild_after_the_quiet_interval() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        // an edit made while nothing watched: in the rebuild at start
        std::fs::write(root.join("memory/before.md"), memory_file("before")).unwrap();
        let w = Watch::start(&env, &[]);
        assert!(
            topics(&env).contains(&"before".to_string()),
            "{:?}",
            topics(&env)
        );

        let changed = Instant::now();
        std::fs::write(root.join("memory/watched.md"), memory_file("watched")).unwrap();
        let line = w.rebuilt(INTERVAL + SLACK);
        let took = changed.elapsed();
        assert!(
            took >= INTERVAL - Duration::from_millis(100),
            "debounced: {took:?}"
        );
        assert_eq!(line["paths"], json!(["memory/watched.md"]), "{line}");
        assert_eq!(line["changes"], json!(1));
        assert_eq!(line["warnings"], json!([]), "{line}");
        assert!(
            topics(&env).contains(&"watched".to_string()),
            "{:?}",
            topics(&env)
        );
        common::assert_valid_index(&index(&env));

        // its own rebuild (reading every file, writing index.json) is silent
        w.assert_quiet(INTERVAL + SLACK);
    }

    #[test]
    fn a_burst_of_changes_triggers_one_rebuild() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let w = Watch::start(&env, &[]);

        // a folder created while watching is watched as well (notify adds
        // the watch on its create event; give it that moment)
        let journal = root.join("journal/2025");
        std::fs::create_dir(&journal).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        for n in 0..20 {
            std::fs::write(
                root.join(format!("memory/burst-{n}.md")),
                memory_file(&format!("burst-{n}")),
            )
            .unwrap();
            if n % 5 == 0 {
                std::fs::write(journal.join(format!("2025-12-{:02}.md", n + 1)), "# note\n")
                    .unwrap();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let line = w.rebuilt(INTERVAL + SLACK);
        assert_eq!(
            line["changes"],
            json!(24 + 1),
            "20 memory, 4 journal files, journal/2025: {line}"
        );
        let topics = topics(&env);
        assert!(
            (0..20).all(|n| topics.contains(&format!("burst-{n}"))),
            "{topics:?}"
        );
        w.assert_quiet(INTERVAL + SLACK);

        // a later write in the new folder is seen (recursive watch)
        std::fs::write(journal.join("2025-12-31.md"), "# later\n").unwrap();
        let line = w.rebuilt(INTERVAL + SLACK);
        assert_eq!(
            line["paths"],
            json!(["journal/2025/2025-12-31.md"]),
            "{line}"
        );
    }

    #[test]
    fn generated_files_reads_and_other_commands_do_not_trigger() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let w = Watch::start(&env, &[]);

        // generated views and STATUS.md, temp and backup files
        std::fs::write(root.join("ledger/2026-10.md"), "<!-- generated -->\n").unwrap();
        std::fs::write(root.join("STATUS.md"), "<!-- generated -->\n").unwrap();
        std::fs::write(root.join("work/active/.C-2026-001-x.md.tmp-1"), "x").unwrap();
        std::fs::write(root.join("memory/lessons.md~"), "x").unwrap();
        std::fs::write(root.join("PROJECT.md"), read(&root.join("PROJECT.md"))).unwrap();
        // reads of every watched file
        for dir in [
            "ledger",
            "work",
            "journal",
            "decisions",
            "system",
            "memory",
            "areas",
        ] {
            read_tree(&root.join(dir));
        }
        // `seldon index` and `seldon status` write the views, STATUS.md and
        // index.json, and commit
        for cmd in [["index", "--json"], ["status", "--json"]] {
            let out = env.seldon(&cmd);
            assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        }
        w.assert_quiet(INTERVAL + SLACK);

        // a source edit afterwards still counts
        std::fs::write(
            root.join(".seldon/logbook.toml"),
            read(&root.join(".seldon/logbook.toml")),
        )
        .unwrap();
        let line = w.rebuilt(INTERVAL + SLACK);
        assert_eq!(line["paths"], json!([".seldon/logbook.toml"]), "{line}");
    }

    /// The area names in `index.json`.
    fn areas(env: &Env) -> Vec<String> {
        index(env)["system"]["areas"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|a| a["name"].as_str().unwrap().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_new_area_triggers_a_rebuild() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let w = Watch::start(&env, &[]);
        assert!(!areas(&env).contains(&"printer".to_string()));

        // the index reads areas/*/README.md into system.areas (F-135)
        std::fs::create_dir(root.join("areas/printer")).unwrap();
        std::fs::write(root.join("areas/printer/README.md"), "# printer\n").unwrap();
        let line = w.rebuilt(INTERVAL + SLACK);
        assert!(
            line["paths"]
                .as_array()
                .unwrap()
                .iter()
                .all(|p| p.as_str().unwrap().starts_with("areas/printer")),
            "{line}"
        );
        assert!(
            areas(&env).contains(&"printer".to_string()),
            "{:?}",
            areas(&env)
        );
        w.assert_quiet(INTERVAL + SLACK);

        // a folder `areas/` made after the start is watched too
        std::fs::rename(root.join("areas"), root.join("areas.old")).unwrap();
        let line = w.rebuilt(INTERVAL + SLACK);
        assert_eq!(line["paths"], json!(["areas"]), "{line}");
        assert_eq!(areas(&env), Vec::<String>::new());
        std::fs::create_dir(root.join("areas")).unwrap();
        let line = w.rebuilt(INTERVAL + SLACK);
        assert_eq!(line["paths"], json!(["areas"]), "{line}");
        std::fs::create_dir(root.join("areas/scanner")).unwrap();
        std::fs::write(root.join("areas/scanner/README.md"), "# scanner\n").unwrap();
        w.rebuilt(INTERVAL + SLACK);
        assert_eq!(areas(&env), ["scanner"]);
    }

    fn read_tree(dir: &Path) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                read_tree(&path);
            } else {
                let _ = std::fs::read(&path).unwrap();
            }
        }
    }

    #[test]
    fn a_held_lock_delays_the_rebuild() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let w = Watch::start(&env, &[]);

        let lock = seldon::logbook::lock::acquire(&env.lock_file()).unwrap();
        std::fs::write(root.join("memory/locked.md"), memory_file("locked")).unwrap();
        w.assert_quiet(INTERVAL + SLACK);
        let mut w = w;
        assert!(
            w.child.try_wait().unwrap().is_none(),
            "the watcher keeps running"
        );
        assert!(!topics(&env).contains(&"locked".to_string()));

        let released = Instant::now();
        drop(lock);
        let line = w.rebuilt(SLACK);
        assert!(
            released.elapsed() < Duration::from_secs(2),
            "retried within 250 ms"
        );
        assert_eq!(line["paths"], json!(["memory/locked.md"]), "{line}");
        assert!(topics(&env).contains(&"locked".to_string()));
    }

    #[test]
    fn a_replaced_folder_is_watched_again() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let w = Watch::start(&env, &[]);

        std::fs::rename(root.join("memory"), root.join("memory.old")).unwrap();
        std::fs::create_dir(root.join("memory")).unwrap();
        let line = w.rebuilt(INTERVAL + SLACK);
        assert_eq!(line["paths"], json!(["memory"]), "{line}");
        // the old folder is no longer watched (under its old name or any)
        std::fs::write(root.join("memory.old/stale.md"), memory_file("stale")).unwrap();
        w.assert_quiet(INTERVAL + SLACK);
        // the new one is
        std::fs::write(root.join("memory/fresh.md"), memory_file("fresh")).unwrap();
        let line = w.rebuilt(INTERVAL + SLACK);
        assert_eq!(line["paths"], json!(["memory/fresh.md"]), "{line}");
        assert_eq!(topics(&env), ["fresh"]);
    }

    #[test]
    fn sigterm_and_sigint_end_the_watcher_cleanly() {
        let env = Env::new(Snapper::Missing);
        env.init_logbook();
        for signal in ["TERM", "INT"] {
            let w = Watch::start(&env, &[]);
            let (code, last) = w.stop(signal);
            assert_eq!(code, Some(0), "SIG{signal}");
            assert_eq!(
                last,
                Some(json!({"status": "stopped", "signal": format!("SIG{signal}"), "rebuilds": 1}))
            );
        }
    }

    #[test]
    fn not_initialised_exits_3_and_a_short_interval_exits_1() {
        let env = Env::new(Snapper::Missing);
        let empty = env.tmp.path().join("nothing");
        std::fs::create_dir_all(&empty).unwrap();
        let mut child = env
            .command(&["watch", "--json", "--logbook", empty.to_str().unwrap()])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        assert_eq!(wait(&mut child, Duration::from_secs(10)), Some(3));
        let mut out = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut out)
            .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["error"]["code"], json!(3), "{v}");

        env.init_logbook();
        let out = env.seldon(&["watch", "--interval", "1"]);
        assert_eq!(out.status.code(), Some(1), "{}", common::stderr(&out));
    }

    /// `VmRSS` and `VmHWM` of `pid`, in kB.
    fn memory_kb(pid: u32) -> (u64, u64) {
        let status = read(Path::new(&format!("/proc/{pid}/status")));
        let field = |name: &str| {
            status
                .lines()
                .find_map(|l| l.strip_prefix(name))
                .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
                .unwrap_or_else(|| panic!("{name} in /proc/{pid}/status"))
        };
        (field("VmRSS:"), field("VmHWM:"))
    }

    /// The PLAN.md bound (RSS < 10 MB) is about the shipped, optimised
    /// binary: the peak (VmHWM, never below VmRSS) must stay under it. The
    /// test profile's binary carries ~6 MB more unoptimised code, so there
    /// only the growth over the idle watcher is bounded; `just check-watch`
    /// runs this test again under the `bench` profile for the absolute
    /// bound. `SELDON_WATCH_BIN=<path>` measures another binary (the musl
    /// release build, docs/TESTING.md).
    #[test]
    fn rss_stays_under_10_mb_on_the_x10_fixture() {
        const LIMIT_KB: u64 = 10 * 1024;
        let env = Env::new(Snapper::Missing);
        let tmp = TempDir::new("watch-x10");
        let root = tmp.path().join("logbook");
        common::scale::scaled_logbook(&fixture_logbook(), &root, 10);
        let args = watch_args(&["--logbook", root.to_str().unwrap()]);
        let other = std::env::var_os("SELDON_WATCH_BIN").filter(|b| !b.is_empty());
        // the lock holds the rebuild at start back until the idle size is read
        let lock = seldon::logbook::lock::acquire(&env.lock_file()).unwrap();
        let w = match &other {
            // the same environment as `Env::command`, another program
            Some(bin) => {
                let template = env.command(&args);
                let mut cmd = Command::new(bin);
                cmd.args(&args).env_clear();
                for (key, value) in template.get_envs() {
                    if let Some(value) = value {
                        cmd.env(key, value);
                    }
                }
                cmd.current_dir(template.get_current_dir().unwrap());
                Watch::spawn(cmd)
            }
            None => Watch::spawn(env.command(&args)),
        };
        let optimised = other.is_some() || !cfg!(debug_assertions);
        let (idle, _) = memory_kb(w.pid());
        drop(lock);
        let line = w.rebuilt(SLACK);
        assert_eq!(line["trigger"], json!("start"), "{line}");
        assert_eq!(line["events"], json!(500), "the ×10 ledger: {line}");

        std::fs::write(root.join("memory/x10.md"), memory_file("x10")).unwrap();
        let line = w.rebuilt(INTERVAL + SLACK);
        assert_eq!(line["trigger"], json!("changes"), "{line}");
        let (rss, peak) = memory_kb(w.pid());
        eprintln!(
            "watch on ×10 ({}): idle {idle} kB, after rebuild {rss} kB, peak {peak} kB",
            other
                .as_ref()
                .map_or(if optimised { "optimised" } else { "debug" }.into(), |b| b
                    .to_string_lossy())
        );
        if optimised {
            assert!(peak < LIMIT_KB, "peak {peak} kB ≥ {LIMIT_KB} kB");
        } else {
            // optimised: ~3 MB on the ×10 fixture
            let growth = peak - idle;
            assert!(growth < 6 * 1024, "growth {growth} kB over idle {idle} kB");
        }
        assert!(rss <= peak);
    }
}
