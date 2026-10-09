//! `seldon preview` (WP-138, ADR-0047): read-only, no logbook, nothing
//! written; its JSON follows `schema/preview.schema.json`, and
//! `fixtures/preview.sample.json` is what it prints for the fixture home
//! below (`SELDON_WRITE_PREVIEW_SAMPLE=1` rewrites the sample).

mod common;

use std::collections::BTreeMap;
use std::fs::{File, FileTimes};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::LazyLock;
use std::time::SystemTime;

use chrono::DateTime;
use common::{Env, Snapper};
use serde_json::Value;

const NOW: &str = "2026-10-08T12:00:00+02:00";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

static SCHEMA: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let text = std::fs::read_to_string(repo().join("schema/preview.schema.json")).unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .expect("preview.schema.json compiles")
});

fn assert_valid(v: &Value) {
    let errors: Vec<String> = SCHEMA
        .iter_errors(v)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect();
    assert!(errors.is_empty(), "{v}: {}", errors.join("; "));
}

fn time(s: &str) -> SystemTime {
    SystemTime::from(DateTime::parse_from_rfc3339(s).unwrap())
}

/// Writes `rel` under `home` with its mtime at `at` (RFC 3339).
fn file(home: &Path, rel: &str, at: &str) {
    let path = home.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "x = 1\n").unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(time(at)))
        .unwrap();
}

/// The fixture home: four files edited in the window, one before it, and
/// one of each kind the scan ignores.
fn fixture_home(env: &Env) {
    let h = &env.home;
    file(
        h,
        ".config/alacritty/alacritty.toml",
        "2026-10-07T21:15:00+02:00",
    );
    file(h, ".config/hypr/bindings.conf", "2026-10-06T09:30:00+02:00");
    file(h, ".config/git/config", "2026-10-03T18:02:11+02:00");
    file(h, ".config/starship.toml", "2026-10-01T12:00:00+02:00");
    file(h, ".config/fish/config.fish", "2026-09-20T08:00:00+02:00");
    for rel in [
        ".config/chromium/Local State",
        ".config/chromium/Default/Preferences",
        ".config/Code/Cache/data_0",
        ".config/omarchy/shell.json",
        ".config/omarchy/plugins/jax.seldon/manifest.json",
        ".config/omarchy/current/history.json",
        ".config/app/app.log",
        ".config/app/places.sqlite",
        ".config/app/icon.png",
        ".config/repo/.git/index",
    ] {
        file(h, rel, "2026-10-07T10:00:00+02:00");
    }
}

fn preview_in(env: &Env, log: &Path, args: &[&str]) -> Output {
    let mut all = vec!["preview"];
    all.extend_from_slice(args);
    env.command(&all)
        .env("SELDON_NOW", NOW)
        .env("SELDON_PACMAN_LOG", log)
        .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
        .env("TZ", "Europe/Berlin")
        .output()
        .expect("run seldon preview")
}

fn preview_log() -> PathBuf {
    repo().join("fixtures/logs/pacman-preview.log")
}

fn json(out: &Output) -> Value {
    assert!(
        out.status.success(),
        "exit {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).expect("JSON on stdout");
    assert_valid(&v);
    v
}

/// Every path under `dir` with its size and mtime.
fn tree(dir: &Path) -> BTreeMap<PathBuf, (u64, SystemTime)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let e = e.unwrap();
            let m = e.metadata().unwrap();
            if m.is_dir() {
                stack.push(e.path());
            }
            out.insert(e.path(), (m.len(), m.modified().unwrap()));
        }
    }
    out
}

#[test]
fn the_sample_is_what_the_engine_prints() {
    let env = Env::new(Snapper::Missing);
    fixture_home(&env);
    let mut v = json(&preview_in(&env, &preview_log(), &["--json"]));
    v["elapsedMs"] = Value::from(23);
    let path = repo().join("fixtures/preview.sample.json");
    if std::env::var_os("SELDON_WRITE_PREVIEW_SAMPLE").is_some() {
        let mut text = serde_json::to_string_pretty(&v).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let sample: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v, sample, "rewrite with SELDON_WRITE_PREVIEW_SAMPLE=1");
    // what the sample shows, spelled out
    let txs = v["pacman"]["transactions"].as_array().unwrap();
    let at: Vec<&str> = txs.iter().map(|t| t["at"].as_str().unwrap()).collect();
    assert_eq!(
        at,
        [
            "2026-10-07T16:02:55+02:00",
            "2026-10-06T09:41:30+02:00",
            "2026-10-05T21:14:06+02:00",
            "2026-10-04T08:05:10+02:00",
            "2026-10-03T19:22:41+02:00",
            // across the window's start: only its line inside, at that line
            "2026-10-01T12:00:01+02:00",
        ]
    );
    assert_eq!(txs[1]["status"], "failed");
    assert_eq!(txs[2]["count"], 14);
    assert_eq!(txs[2]["packages"].as_array().unwrap().len(), 10);
    assert_eq!(txs[5]["count"], 1);
    assert_eq!(v["truncated"], true);
    let files: Vec<&str> = v["files"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        [
            "~/.config/alacritty/alacritty.toml",
            "~/.config/hypr/bindings.conf",
            "~/.config/git/config",
            "~/.config/starship.toml",
        ]
    );
    assert_eq!(v["files"]["root"], "~/.config");
}

#[test]
fn nothing_is_written_and_no_logbook_is_needed() {
    let env = Env::new(Snapper::Missing);
    fixture_home(&env);
    let before = tree(env.tmp.path());
    let out = preview_in(&env, &preview_log(), &["--json"]);
    json(&out);
    let human = preview_in(&env, &preview_log(), &[]);
    assert!(human.status.success());
    let text = String::from_utf8_lossy(&human.stdout);
    assert!(text.contains("no who, no why"), "{text}");
    assert!(
        text.contains("upgraded 14 packages  (pacman -Syu --noconfirm)"),
        "{text}"
    );
    assert!(
        text.contains("~/.config/alacritty/alacritty.toml"),
        "{text}"
    );
    assert_eq!(tree(env.tmp.path()), before, "preview wrote something");
    assert!(!env.home.join(".local/state/seldon").exists());
    assert!(!env.config_file().exists());
}

#[test]
fn an_initialised_logbook_changes_nothing() {
    let env = Env::new(Snapper::Missing);
    fixture_home(&env);
    let bare = json(&preview_in(&env, &preview_log(), &["--json"]));
    env.init_logbook();
    let before = tree(env.tmp.path());
    let mut v = json(&preview_in(&env, &preview_log(), &["--json"]));
    assert_eq!(tree(env.tmp.path()), before, "preview wrote something");
    // init's config.toml is listed: it is a file edited under ~/.config
    let items = v["files"]["items"].as_array_mut().unwrap();
    items.retain(|f| f["path"] != "~/.config/seldon/config.toml");
    v["elapsedMs"] = bare["elapsedMs"].clone();
    assert_eq!(v["pacman"], bare["pacman"]);
}

#[test]
fn days_are_one_to_seven() {
    let env = Env::new(Snapper::Missing);
    for bad in ["0", "8", "x"] {
        let out = preview_in(&env, &preview_log(), &["--days", bad, "--json"]);
        assert_eq!(out.status.code(), Some(1), "--days {bad}");
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["error"]["code"], 1);
    }
    let v = json(&preview_in(
        &env,
        &preview_log(),
        &["--days", "2", "--json"],
    ));
    assert_eq!(v["days"], 2);
    assert_eq!(v["since"], "2026-10-06T12:00:00+02:00");
    let at: Vec<&str> = v["pacman"]["transactions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["at"].as_str().unwrap())
        .collect();
    assert_eq!(at, ["2026-10-07T16:02:55+02:00"]);
}

#[test]
fn a_missing_pacman_log_is_said_not_fatal() {
    let env = Env::new(Snapper::Missing);
    fixture_home(&env);
    let v = json(&preview_in(
        &env,
        &env.tmp.path().join("no.log"),
        &["--json"],
    ));
    assert_eq!(v["pacman"]["ok"], false);
    assert!(
        v["pacman"]["message"]
            .as_str()
            .unwrap()
            .starts_with("cannot read "),
        "{v}"
    );
    assert_eq!(v["pacman"]["transactions"], Value::Array(vec![]));
    assert_eq!(v["files"]["items"].as_array().unwrap().len(), 4);
}

#[test]
fn a_config_that_cannot_be_used_withholds_files_and_commands() {
    for text in ["not toml = = =\n", "[redaction]\npatterns = [\"(\"]\n"] {
        let env = Env::new(Snapper::Missing);
        fixture_home(&env);
        std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        std::fs::write(env.config_file(), text).unwrap();
        let v = json(&preview_in(&env, &preview_log(), &["--json"]));
        assert_eq!(v["files"]["ok"], false, "{text}");
        assert!(
            v["files"]["message"]
                .as_str()
                .unwrap()
                .starts_with("withheld")
        );
        assert_eq!(v["files"]["items"], Value::Array(vec![]));
        let txs = v["pacman"]["transactions"].as_array().unwrap();
        assert!(!txs.is_empty());
        assert!(txs.iter().all(|t| t.get("command").is_none()), "{text}");
    }
}

#[test]
fn skip_paths_and_redaction_patterns_of_the_config_hold() {
    let env = Env::new(Snapper::Missing);
    fixture_home(&env);
    file(
        &env.home,
        ".config/secret-app/token.conf",
        "2026-10-07T10:00:00+02:00",
    );
    std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
    std::fs::write(
        env.config_file(),
        "[redaction]\nskipPaths = [\"~/.config/secret-app/\"]\npatterns = [\"starship\", \"noconfirm\"]\n",
    )
    .unwrap();
    // the config file itself is older than the window
    File::options()
        .write(true)
        .open(env.config_file())
        .unwrap()
        .set_times(FileTimes::new().set_modified(time("2026-09-01T00:00:00+02:00")))
        .unwrap();
    let v = json(&preview_in(&env, &preview_log(), &["--json"]));
    let text = v.to_string();
    assert!(!text.contains("secret-app"), "{text}");
    assert!(!text.contains("starship"), "{text}");
    assert!(!text.contains("noconfirm"), "{text}");
    // Omarchy's history.json stays out whatever skipPaths say: it is on the
    // walker's own list since the one walker of WP-139 round 2
    let files: Vec<&str> = v["files"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        [
            "~/.config/alacritty/alacritty.toml",
            "~/.config/hypr/bindings.conf",
            "~/.config/git/config",
            "~/.config/‹redacted›.toml",
        ]
    );
}

#[test]
fn the_bounds_hold_and_say_so() {
    let env = Env::new(Snapper::Missing);
    // 150 files in the window: 80 listed
    for i in 0..150 {
        file(
            &env.home,
            &format!(".config/many/f{i:03}.conf"),
            &format!("2026-10-07T{:02}:{:02}:00+02:00", i / 60, i % 60),
        );
    }
    // 300 one-package transactions: 120 listed (200 rows − 80 files)
    let mut log = String::new();
    for i in 0..300 {
        let at = format!(
            "2026-10-0{}T{:02}:{:02}:00+0200",
            2 + i / 60,
            (i % 60) / 3,
            i % 60
        );
        log.push_str(&format!(
            "[{at}] [ALPM] transaction started\n[{at}] [ALPM] installed p{i} (1-1)\n[{at}] [ALPM] transaction completed\n"
        ));
    }
    let log_path = env.tmp.path().join("pacman.log");
    std::fs::write(&log_path, log).unwrap();
    let v = json(&preview_in(&env, &log_path, &["--json"]));
    assert_eq!(v["files"]["items"].as_array().unwrap().len(), 80);
    assert_eq!(v["pacman"]["transactions"].as_array().unwrap().len(), 120);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["files"]["partial"], false);
    // the newest first
    assert_eq!(v["files"]["items"][0]["path"], "~/.config/many/f149.conf");
    assert_eq!(
        v["pacman"]["transactions"][0]["packages"][0]["name"],
        "p299"
    );
}

/// The 0.5 s budget (WP-138) on a busy machine: a 32 MiB log whose window
/// is its last transactions and 5 000 files under `~/.config`. The
/// debug build under a parallel test run is far slower than the release
/// binary; the release figure is measured in the handover.
#[test]
fn a_large_log_and_a_full_config_stay_in_budget() {
    let env = Env::new(Snapper::Missing);
    for d in 0..50 {
        for f in 0..100 {
            file(
                &env.home,
                &format!(".config/app{d}/f{f}.conf"),
                "2026-09-01T00:00:00+02:00",
            );
        }
    }
    let mut log = String::with_capacity(33 << 20);
    let old = "[2026-08-01T10:00:00+0200] [ALPM] upgraded some-old-package (1.0-1 -> 1.0-2)\n";
    while log.len() < 32 << 20 {
        log.push_str(old);
    }
    log.push_str(&std::fs::read_to_string(preview_log()).unwrap());
    let log_path = env.tmp.path().join("pacman.log");
    std::fs::write(&log_path, log).unwrap();
    let started = std::time::Instant::now();
    let v = json(&preview_in(&env, &log_path, &["--json"]));
    let wall = started.elapsed();
    assert_eq!(v["pacman"]["transactions"].as_array().unwrap().len(), 6);
    assert_eq!(v["pacman"]["partial"], false);
    assert!(
        v["elapsedMs"].as_u64().unwrap() < 2000,
        "{}",
        v["elapsedMs"]
    );
    assert!(wall.as_secs() < 5, "{wall:?}");
}

/// WP-138 round 2 (N2): pacman's grammar takes any `\S+`, so a log can
/// hold names and versions with control and bidi characters, and longer
/// than the schema allows. The output stays one line per value, within
/// the schema, in JSON and on the terminal.
#[test]
fn a_hostile_log_still_gives_schema_valid_one_line_output() {
    let env = Env::new(Snapper::Missing);
    let at = "[2026-10-07T10:00:00+0200]";
    let long_name = "n".repeat(600);
    let long_from = "1".repeat(300);
    let long_version = "v".repeat(300);
    let log = format!(
        "{at} [ALPM] transaction started\n\
         {at} [ALPM] installed ev\u{202E}il\u{1b}[31m (1.0\u{1b}]8;;x-1)\n\
         {at} [ALPM] upgraded {long_name} ({long_from} -> 2.0\u{2066}-1)\n\
         {at} [ALPM] installed long-version ({long_version})\n\
         {at} [ALPM] transaction completed\n\
         {at} [ALPM] transaction started\n\
         {at} [ALPM] installed only\u{1b}[2Jone (1-1)\n\
         {at} [ALPM] transaction completed\n"
    );
    let log_path = env.tmp.path().join("pacman.log");
    std::fs::write(&log_path, log).unwrap();
    let out = preview_in(&env, &log_path, &["--json"]);
    let v = json(&out); // validated against preview.schema.json
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    for bad in ['\u{1b}', '\u{202E}', '\u{2066}'] {
        assert!(!text.contains(bad), "{bad:?} in {text}");
    }
    let txs = v["pacman"]["transactions"].as_array().unwrap();
    let packages = txs[1]["packages"].as_array().unwrap();
    assert_eq!(packages[0]["name"], "ev\u{FFFD}il\u{FFFD}[31m");
    assert_eq!(packages[1]["name"].as_str().unwrap().chars().count(), 512);
    assert_eq!(packages[1]["from"].as_str().unwrap().chars().count(), 256);
    assert_eq!(packages[1]["to"], "2.0\u{FFFD}-1");
    assert_eq!(
        packages[2]["version"].as_str().unwrap().chars().count(),
        256
    );
    // the terminal gets no escape either (a one-package transaction prints its name)
    let human = preview_in(&env, &log_path, &[]);
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("installed only\u{FFFD}[2Jone"), "{text}");
    for bad in ['\u{1b}', '\u{202E}', '\u{2066}'] {
        assert!(!text.contains(bad), "{bad:?} in {text}");
    }
}

/// WP-138 round 2 (N1, N3): a log whose last 8 MiB are all inside the
/// window is read only that far and says so (`pacman.partial`), and the
/// `~/.config` walk, which runs first with its own budget, still lists
/// its files however long the pacman read takes.
#[test]
fn a_log_denser_than_the_tail_says_partial_and_keeps_the_files() {
    let env = Env::new(Snapper::Missing);
    fixture_home(&env);
    let mut log = String::with_capacity(9 << 20);
    let mut i = 0;
    while log.len() < 9 << 20 {
        let at = format!("[2026-10-0{}T12:00:00+0200]", 2 + i % 6);
        log.push_str(&format!(
            "{at} [ALPM] transaction started\n{at} [ALPM] upgraded package-{i} (1.0-1 -> 1.0-2)\n{at} [ALPM] transaction completed\n"
        ));
        i += 1;
    }
    let log_path = env.tmp.path().join("pacman.log");
    std::fs::write(&log_path, log).unwrap();
    let v = json(&preview_in(&env, &log_path, &["--json"]));
    assert_eq!(v["pacman"]["partial"], true);
    assert_eq!(v["pacman"]["ok"], true);
    assert_eq!(v["files"]["items"].as_array().unwrap().len(), 4);
    assert_eq!(v["files"]["partial"], false);
    assert_eq!(v["truncated"], true);
    // the fixture's log reaches back past the window: not partial
    let v = json(&preview_in(&env, &preview_log(), &["--json"]));
    assert_eq!(v["pacman"]["partial"], false);
}
