//! Capture cost of the config and plugins collectors (WP-113): what the
//! watched files, the toggles directory, the persistence paths and the
//! third-party plugin trees cost a capture, cold (nothing known: every
//! file read) and warm (the stat cache: nothing changed). Release timing,
//! `just check-perf` (`--profile bench --ignored`); the budgets are
//! generous ceilings, the printed medians are the numbers.
//!
//! The synthetic home is the size of a lived-in Omarchy desktop: 115
//! files under the earlier default watch paths, then per line item 6
//! toggles, a 4 MiB binary and a 2 MiB script in a hook directory, an
//! `authorized_keys`; and 8 third-party plugins of 41 files plus one
//! 2 MiB image each. Everything lives in a temp dir; nothing
//! reads the real home (AGENTS.md §6).
//!
//! The scan of recently edited files under `~/.config` (ADR-0045, WP-139)
//! is the capture-cost delta of that WP: the same home plus 40 programs'
//! config folders, a browser profile, an Electron app and a cache, every
//! file modified within the last 7 days (the list's worst case).

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use serde_json::{Value, json};

use common::TempDir;
use seldon::collectors::config::ConfigFiles;
use seldon::collectors::plugins::Plugins;
use seldon::collectors::recent;
use seldon::collectors::{Ctx, Outcome, Sources, Tz};
use seldon::config::{Config, Dirs};
use seldon::ledger::Ledger;
use seldon::redact::Redactor;

const NOW: &str = "2026-10-07T12:00:00+02:00";
const RUNS: usize = 21;

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// `n` bytes of text that differ per `seed`.
fn text(seed: usize, n: usize) -> Vec<u8> {
    let line = format!("-- line of file {seed}: some configuration value = {seed}\n");
    line.bytes().cycle().take(n).collect()
}

/// `n` bytes with NUL bytes in them (an image, a library).
fn binary(seed: usize, n: usize) -> Vec<u8> {
    (0..n).map(|i| ((i * 31 + seed) % 251) as u8).collect()
}

struct Home {
    tmp: TempDir,
    dirs: Dirs,
    config: Config,
    sources: Sources,
    ledger: Ledger,
    omarchy: String,
    plugins_dir: PathBuf,
}

impl Home {
    fn new() -> Self {
        let tmp = TempDir::new("capture-cost");
        let home = tmp.path().join("home");
        let dirs = Dirs {
            xdg_config_home: home.join(".config"),
            state_dir: home.join(".local/state/seldon"),
            home: home.clone(),
        };
        let h = |rel: &str| home.join(rel);
        for i in 0..20 {
            write(&h(&format!(".config/hypr/conf{i}.lua")), text(i, 2048));
        }
        for t in 0..4 {
            for i in 0..15 {
                write(
                    &h(&format!(".config/omarchy/themes/t{t}/file{i}.conf")),
                    text(100 + t * 20 + i, 1024),
                );
            }
        }
        write(&h(".config/omarchy/shell.json"), text(1, 8192));
        for i in 0..30 {
            write(
                &h(&format!(".local/share/applications/app{i}.desktop")),
                text(200 + i, 512),
            );
        }
        for i in 0..3 {
            write(
                &h(&format!(".config/autostart/a{i}.desktop")),
                text(300 + i, 256),
            );
        }
        write(&h(".bashrc"), text(400, 4096));
        // the toggles directory (WP-113 item 2)
        for i in 0..6 {
            write(
                &h(&format!(".local/state/omarchy/toggles/hypr/flag{i}.lua")),
                text(500 + i, 128),
            );
        }
        // hook-path blind spots (WP-113 item 4): binary, over 1 MiB
        write(
            &h(".config/omarchy/hooks/post-update.d/blob"),
            binary(1, 4 << 20),
        );
        write(
            &h(".config/omarchy/hooks/post-update.d/big.sh"),
            text(600, 2 << 20),
        );
        // third-party plugin trees (WP-113 item 1)
        let plugins_dir = h(".config/omarchy/plugins");
        let mut listed = Vec::new();
        for p in 0..8 {
            let id = format!("io.github.example.p{p}");
            let dir = plugins_dir.join(&id);
            write(
                &dir.join("manifest.json"),
                json!({"id": id, "name": id, "version": "1.0.0"}).to_string(),
            );
            for i in 0..40 {
                write(
                    &dir.join(format!("components/C{i}.qml")),
                    text(1000 + p * 50 + i, 4096),
                );
            }
            write(&dir.join("assets/preview.png"), binary(p, 2 << 20));
            listed.push(json!({"id": id, "enabled": true, "firstParty": false}));
        }
        let list = tmp.path().join("list.json");
        write(&list, Value::Array(listed).to_string());
        let omarchy = tmp.path().join("bin/omarchy");
        common::write_executable(
            &omarchy,
            &format!(
                "#!/bin/sh\ncase \"$2\" in\nlist) while IFS= read -r l || [ -n \"$l\" ]; do printf '%s\\n' \"$l\"; done < '{}';;\n*) exit 1;;\nesac\n",
                list.display()
            ),
        );
        let missing = tmp.path().join("no-such-program").display().to_string();
        let sources = Sources {
            pacman_log: tmp.path().join("pacman.log"),
            pacman_db_lock: tmp.path().join("db.lck"),
            snapper: missing.clone(),
            snapshots: tmp.path().join("no-snapshots"),
            omarchy_version: missing.clone(),
            pacman: missing.clone(),
            omarchy: missing,
            plugins_dir: Some(plugins_dir.clone()),
            theme_file: Some(tmp.path().join("theme.name")),
            omarchy_path: tmp.path().join("omarchy"),
        };
        // the files must be older than the stat cache's racy window
        let old = std::time::SystemTime::now() - Duration::from_secs(3600);
        set_mtimes(&home, old);
        Home {
            ledger: Ledger::at(tmp.path().join("logbook/ledger"), Redactor::builtin()),
            config: Config::default(),
            omarchy: omarchy.to_string_lossy().into_owned(),
            tmp,
            dirs,
            sources,
            plugins_dir,
        }
    }

    fn ctx(&self) -> Ctx<'_> {
        let now: DateTime<FixedOffset> = DateTime::parse_from_rfc3339(NOW).unwrap();
        Ctx {
            now,
            baseline: now,
            tz: Tz::Fixed(*now.offset()),
            sources: &self.sources,
            config: &self.config,
            dirs: &self.dirs,
            ledger: &self.ledger,
            earlier: &[],
        }
    }

    fn manifest(&self) -> PathBuf {
        self.tmp.path().join("manifest.json")
    }

    fn config(&self, cursor: Option<&Value>) -> Outcome {
        let roots: Vec<PathBuf> = self
            .config
            .watch_paths
            .iter()
            .filter_map(|p| self.dirs.expand_config(p))
            .collect();
        let excluded = [self.plugins_dir.clone()];
        ConfigFiles.collect_from(&self.ctx(), cursor, &roots, &excluded, &self.manifest())
    }

    fn plugins(&self, cursor: Option<&Value>) -> Outcome {
        Plugins.collect_from(&self.ctx(), cursor, &self.omarchy, &self.plugins_dir)
    }
}

fn set_mtimes(dir: &Path, t: std::time::SystemTime) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let meta = std::fs::symlink_metadata(&path).unwrap();
        if meta.is_dir() {
            set_mtimes(&path, t);
        } else if meta.is_file() {
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
    }
}

#[test]
#[ignore = "release timing: `just check-perf`"]
fn capture_cost_of_the_watched_files_and_plugin_trees() {
    common::assert_optimised();
    let home = Home::new();

    // config per line item (WP-113): what the files under the earlier
    // default watch paths cost, then each item added on top
    let toggles = "~/.local/state/omarchy/toggles".to_string();
    let hooks = home.dirs.home.join(".config/omarchy/hooks/post-update.d");
    let blobs = ["blob", "big.sh"].map(|n| (hooks.join(n), std::fs::read(hooks.join(n)).unwrap()));
    let keys = home.dirs.home.join(".ssh/authorized_keys");
    let mut home = home;
    let full = home.config.watch_paths.clone();
    let step = |home: &Home, what: &str, cold: u64, warm: u64| {
        common::assert_within_budget(
            &format!("config capture, cold, {what}"),
            Duration::from_millis(cold),
            RUNS,
            || {
                let _ = std::fs::remove_file(home.manifest());
                assert!(home.config(None).ok);
            },
        );
        let _ = std::fs::remove_file(home.manifest());
        let cursor = home.config(None).cursor.unwrap();
        common::assert_within_budget(
            &format!("config capture, warm, {what}"),
            Duration::from_millis(warm),
            RUNS,
            || {
                let out = home.config(Some(&cursor));
                assert!(out.ok && out.events.is_empty(), "{:?}", out.message);
            },
        );
    };
    home.config.watch_paths.retain(|p| *p != toggles);
    for (path, _) in &blobs {
        std::fs::remove_file(path).unwrap();
    }
    step(&home, "earlier default paths", 50, 20);
    home.config.watch_paths = full.clone();
    step(&home, "+ toggles directory", 50, 20);
    for (path, bytes) in &blobs {
        std::fs::write(path, bytes).unwrap();
    }
    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    set_mtimes(&hooks, old);
    step(
        &home,
        "+ a 4 MiB binary and a 2 MiB script in a hook directory",
        100,
        20,
    );
    write(&keys, text(700, 2048));
    set_mtimes(&home.dirs.home.join(".ssh"), old);
    home.config
        .watch_paths
        .push("~/.ssh/authorized_keys".into());
    step(&home, "+ ~/.ssh/authorized_keys (opt-in)", 100, 20);
    home.config.watch_paths = full;
    let home = home;

    // plugins: the stub `omarchy` (one process) plus the trees
    let first = home.plugins(None);
    assert!(first.ok, "{:?}", first.message);
    let cursor = first.cursor.clone().unwrap();
    let budget = Duration::from_millis(60);
    common::assert_within_budget("plugins capture, warm", budget, RUNS, || {
        let out = home.plugins(Some(&cursor));
        assert!(out.ok && out.events.is_empty(), "{:?}", out.message);
    });
    // cold trees: a cursor without the stat fingerprints (absent before
    // WP-113, so this equals the warm run there)
    let mut cold = cursor.clone();
    if let Some(o) = cold.as_object_mut() {
        o.remove("stats");
    }
    let budget = Duration::from_millis(150);
    common::assert_within_budget("plugins capture, cold trees", budget, RUNS, || {
        let out = home.plugins(Some(&cold));
        assert!(out.ok && out.events.is_empty(), "{:?}", out.message);
    });
}

#[test]
#[ignore = "release timing: `just check-perf`"]
fn capture_cost_of_the_recent_config_scan() {
    common::assert_optimised();
    let home = Home::new();
    let h = |rel: &str| home.dirs.home.join(rel);
    for a in 0..40 {
        for i in 0..8 {
            write(
                &h(&format!(".config/app{a}/conf{i}.toml")),
                text(2000 + a * 8 + i, 512),
            );
        }
    }
    // entered only to its first level: the profile marks and the cache
    write(&h(".config/chromium/Local State"), text(1, 64));
    write(&h(".config/Code/Cookies"), binary(2, 64));
    for i in 0..1500 {
        write(
            &h(&format!(".config/chromium/Default/f{}/x{i}", i % 30)),
            text(i, 64),
        );
        write(
            &h(&format!(".config/Code/User/f{}/y{i}", i % 30)),
            text(i, 64),
        );
        write(&h(&format!(".config/app0/Cache/z{i}")), text(i, 64));
    }
    let recent = std::time::SystemTime::now() - Duration::from_secs(3600);
    set_mtimes(&h(".config"), recent);
    let now = DateTime::parse_from_rfc3339(&chrono::Local::now().to_rfc3339()).unwrap();
    let redactor = Redactor::builtin();
    let excluded = [home.plugins_dir.clone()];
    let scan = recent::scan(&home.dirs, &home.config, &redactor, &excluded, now);
    assert_eq!(scan.files.len(), recent::MAX_FILES);
    assert!(!scan.cut);
    eprintln!("recent-config scan: {} entries read", scan.entries);
    common::assert_within_budget(
        "recent-config scan and save (the capture-cost delta)",
        Duration::from_millis(10),
        RUNS,
        || {
            let scan = recent::scan(&home.dirs, &home.config, &redactor, &excluded, now);
            recent::Saved::of(&scan, now).save(&home.dirs).unwrap();
        },
    );
    // what every index build adds: the saved 80 filtered once more
    common::assert_within_budget(
        "recent-config in the index build (80 files)",
        Duration::from_millis(2),
        RUNS,
        || {
            let mut warnings = Vec::new();
            let shown = recent::shown(
                &home.dirs,
                &home.config,
                Some(&redactor),
                now,
                &mut warnings,
            );
            assert_eq!(shown.unwrap().files.len(), recent::MAX_FILES);
        },
    );
}
