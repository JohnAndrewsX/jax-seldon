//! Recently edited under `~/.config`, outside the watch paths (ADR-0045,
//! WP-139): the scan a capture runs, `system.recentConfig` in the index,
//! and `seldon config watch`, the desk's *Watch* click. Everything lives in
//! a temp home; nothing reads or writes the real `~/.config` (AGENTS.md §6).

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use common::{Env, Snapper, read};
use seldon::config::Config;

const HOUR: Duration = Duration::from_secs(3600);
const DAY: Duration = Duration::from_secs(86_400);

/// `~/rel` in `env`, modified `age` ago.
fn file(env: &Env, rel: &str, age: Duration) -> PathBuf {
    let path = env.home.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "x = 1\n").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(SystemTime::now() - age)
        .unwrap();
    path
}

fn json_file(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap()
}

fn index(env: &Env) -> Value {
    json_file(&env.home.join(".local/state/seldon/index.json"))
}

fn recent_paths(index: &Value) -> Vec<String> {
    index["system"]["recentConfig"]["files"]
        .as_array()
        .unwrap_or_else(|| panic!("no recentConfig: {}", index["system"]))
        .iter()
        .map(|f| f["path"].as_str().unwrap().to_string())
        .collect()
}

fn ok(out: &std::process::Output) -> Value {
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        common::stdout(out),
        common::stderr(out)
    );
    common::json(out)
}

fn edit_config(env: &Env, edit: impl FnOnce(&mut Config)) {
    let mut config = Config::load(&env.config_file()).unwrap().unwrap();
    edit(&mut config);
    config.save(&env.config_file()).unwrap();
}

/// A logbook whose first capture is done, so later captures diff.
fn setup() -> (Env, PathBuf) {
    let env = Env::new(Snapper::Missing);
    let lb = env.init_logbook();
    ok(&env.seldon(&["capture", "--json"]));
    (env, lb)
}

#[test]
fn a_capture_lists_recent_files_outside_the_watch_paths_and_writes_no_event() {
    let (env, lb) = setup();
    let before = common::ledger(&lb).len();
    file(&env, ".config/alacritty/alacritty.toml", HOUR);
    file(&env, ".config/git/config", 2 * DAY);
    file(&env, ".config/hypr/input.lua", HOUR);
    file(&env, ".config/old/app.conf", 8 * DAY);
    file(&env, ".config/chromium/Local State", HOUR);
    file(&env, ".config/chromium/Default/Preferences", HOUR);
    file(&env, ".config/app/app.log", HOUR);
    let out = ok(&env.seldon(&["capture", "--json"]));
    let index = index(&env);
    common::assert_valid_index(&index);
    assert_eq!(
        recent_paths(&index),
        ["~/.config/alacritty/alacritty.toml", "~/.config/git/config"]
    );
    // the watched file is the config collector's event; the list writes none
    let new: Vec<Value> = common::ledger(&lb).split_off(before);
    assert_eq!(out["written"], json!(new.len()));
    let subjects: Vec<&str> = new.iter().filter_map(|e| e["subject"].as_str()).collect();
    assert_eq!(subjects, ["~/.config/hypr/input.lua"], "{new:?}");
    // paths and times only, in the state directory
    let saved = json_file(&env.home.join(".local/state/seldon/recent-config.json"));
    assert_eq!(
        saved["files"][0]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["mtime", "path"]
    );
    assert_eq!(
        saved["scannedAt"],
        index["system"]["recentConfig"]["scannedAt"]
    );
}

#[test]
fn a_config_file_elsewhere_under_dot_config_is_not_listed() {
    let (env, _) = setup();
    let elsewhere = env.home.join(".config/mine/seldon.toml");
    std::fs::create_dir_all(elsewhere.parent().unwrap()).unwrap();
    std::fs::copy(env.config_file(), &elsewhere).unwrap();
    file(&env, ".config/mine/other.conf", HOUR);
    let out = env
        .command(&["capture", "--json"])
        .env("SELDON_CONFIG", &elsewhere)
        .output()
        .unwrap();
    ok(&out);
    assert_eq!(recent_paths(&index(&env)), ["~/.config/mine/other.conf"]);
}

#[test]
fn only_a_capture_that_runs_the_config_collector_scans() {
    let (env, _) = setup();
    let state = env.home.join(".local/state/seldon/recent-config.json");
    std::fs::remove_file(&state).unwrap();
    ok(&env.seldon(&["capture", "--source", "theme", "--json"]));
    assert!(!state.exists());
    // without a scan the index has no list
    ok(&env.seldon(&["index", "--check", "--json"]));
    assert!(index(&env)["system"].get("recentConfig").is_none());
    edit_config(&env, |c| c.collectors.config = false);
    ok(&env.seldon(&["capture", "--json"]));
    assert!(!state.exists());
}

#[test]
fn no_path_under_a_skip_path_appears_also_one_added_after_the_scan() {
    let (env, _) = setup();
    file(&env, ".config/secret/app.conf", HOUR);
    file(&env, ".config/app/id.key", HOUR);
    file(&env, ".config/app/settings.conf", HOUR);
    file(&env, ".config/later/x.conf", 2 * HOUR);
    edit_config(&env, |c| {
        c.redaction.skip_paths = vec!["~/.config/secret/".into(), "*.key".into()];
    });
    ok(&env.seldon(&["capture", "--json"]));
    assert_eq!(
        recent_paths(&index(&env)),
        ["~/.config/app/settings.conf", "~/.config/later/x.conf"]
    );
    // a pattern added after the scan: gone at the next index build
    edit_config(&env, |c| c.redaction.skip_paths.push("later".into()));
    ok(&env.seldon(&["index", "--json"]));
    assert_eq!(recent_paths(&index(&env)), ["~/.config/app/settings.conf"]);
}

#[test]
fn watch_adds_the_path_by_a_minimal_edit_and_the_row_goes() {
    let (env, lb) = setup();
    let alacritty = ".config/alacritty/alacritty.toml";
    file(&env, alacritty, HOUR);
    file(&env, ".config/git/config", HOUR);
    ok(&env.seldon(&["capture", "--json"]));
    // a comment the edit keeps
    let text = format!("# mine\n{}", read(&env.config_file()));
    std::fs::write(env.config_file(), &text).unwrap();
    let out = ok(&env.seldon(&[
        "config",
        "watch",
        "--json",
        "--",
        "~/.config/alacritty/alacritty.toml",
    ]));
    assert_eq!(
        out,
        json!({"added": true, "path": "~/.config/alacritty/alacritty.toml", "coveredBy": null,
               "config": "~/.config/seldon/config.toml"})
    );
    let edited = read(&env.config_file());
    assert!(edited.starts_with("# mine\n"), "{edited}");
    let config = Config::load(&env.config_file()).unwrap().unwrap();
    assert_eq!(
        config.watch_paths.last().unwrap(),
        "~/.config/alacritty/alacritty.toml"
    );
    // the index was rebuilt: the row is gone without a capture
    assert_eq!(recent_paths(&index(&env)), ["~/.config/git/config"]);
    // again (also spelled absolute): nothing changes
    let again = ok(&env.seldon(&[
        "config",
        "watch",
        "--json",
        "--",
        env.home.join(alacritty).to_str().unwrap(),
    ]));
    assert_eq!(again["added"], json!(false));
    assert_eq!(
        again["coveredBy"],
        json!("~/.config/alacritty/alacritty.toml")
    );
    assert_eq!(read(&env.config_file()), edited);
    // a folder a default watch path covers
    let hypr = ok(&env.seldon(&["config", "watch", "--json", "--", "~/.config/hypr/x.lua"]));
    assert_eq!(hypr["coveredBy"], json!("~/.config/hypr"));
    // the next capture takes the file in as it is: no event
    let before = common::ledger(&lb).len();
    let out = ok(&env.seldon(&["capture", "--json"]));
    assert_eq!(
        out["written"],
        json!(0),
        "{:?}",
        common::ledger(&lb).split_off(before)
    );
    // an edit after that is the config collector's
    std::fs::write(env.home.join(alacritty), "x = 2\n").unwrap();
    ok(&env.seldon(&["capture", "--json"]));
    let last = common::ledger(&lb).pop().unwrap();
    assert_eq!(last["kind"], json!("config-change"));
    assert_eq!(last["subject"], json!("~/.config/alacritty/alacritty.toml"));
}

#[test]
fn watch_without_a_config_file_writes_the_defaults_and_the_path() {
    let env = Env::new(Snapper::Missing);
    assert!(!env.config_file().exists());
    let out = ok(&env.seldon(&["config", "watch", "--json", "--", "~/.config/git/config"]));
    assert_eq!(out["added"], json!(true));
    let config = Config::load(&env.config_file()).unwrap().unwrap();
    let mut want = Config::default().watch_paths;
    want.push("~/.config/git/config".into());
    assert_eq!(config.watch_paths, want);
}

#[test]
fn watch_appends_to_an_empty_list() {
    let env = Env::new(Snapper::Missing);
    std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
    std::fs::write(env.config_file(), "watchPaths = [] # none\n").unwrap();
    ok(&env.seldon(&["config", "watch", "--json", "--", "starship.toml"]));
    assert_eq!(
        read(&env.config_file()),
        "watchPaths = [\"~/starship.toml\"] # none\n"
    );
}

#[test]
fn watch_refuses_and_writes_nothing() {
    let env = Env::new(Snapper::Missing);
    // a logbook under the home directory
    env.init_logbook_at("home/Seldon", "en");
    edit_config(&env, |c| {
        c.redaction.skip_paths = vec!["~/.config/secret/".into(), "private".into()];
    });
    let text = read(&env.config_file());
    let control = "~/.config/a\u{1b}b";
    let bidi = "~/.config/a\u{202e}b";
    let long = format!("~/.config/{}", "x".repeat(510));
    let cases: [(&str, &str); 12] = [
        ("/etc/pacman.conf", "is not below your home directory"),
        ("~", "is not below your home directory"),
        (
            "~/.config/secret/token.conf",
            "matches [redaction] skipPaths",
        ),
        (
            "~/.config/app/private/a.conf",
            "matches [redaction] skipPaths",
        ),
        ("~/.config/seldon/config.toml", "Seldon's own files"),
        ("~/.local/state/seldon", "Seldon's own files"),
        ("~/.local", "Seldon's own files"),
        ("~/Seldon/journal", "Seldon's own files"),
        ("Seldon", "Seldon's own files"),
        (control, "cannot be a watch path"),
        (bidi, "cannot be a watch path"),
        (&long, "cannot be a watch path"),
    ];
    for (path, says) in cases {
        let out = env.seldon(&["config", "watch", "--json", "--", path]);
        assert_eq!(
            out.status.code(),
            Some(1),
            "{path:?}: {}",
            common::stdout(&out)
        );
        let message = common::json(&out)["error"]["message"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(message.contains(says), "{path:?}: {message}");
        assert_eq!(read(&env.config_file()), text, "{path:?}");
    }
    let out = env.seldon(&["config", "watch", "--json", "--", ""]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn watch_leaves_a_file_it_cannot_edit_minimally() {
    let env = Env::new(Snapper::Missing);
    std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
    let text = "\"watchPaths\" = [\"~/.config/hypr\"]\n";
    std::fs::write(env.config_file(), text).unwrap();
    let out = env.seldon(&["config", "watch", "--json", "--", "~/.config/git/config"]);
    assert_eq!(out.status.code(), Some(1));
    let message = common::json(&out)["error"]["message"].to_string();
    assert!(
        message.contains("add \\\"~/.config/git/config\\\" to watchPaths by hand"),
        "{message}"
    );
    assert_eq!(read(&env.config_file()), text);
}

#[test]
fn watch_without_a_watch_paths_key_names_the_defaults() {
    let env = Env::new(Snapper::Missing);
    std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
    let text = "logbook = \"~/Seldon\"\n";
    std::fs::write(env.config_file(), text).unwrap();
    let out = env.seldon(&["config", "watch", "--json", "--", "~/.config/git/config"]);
    assert_eq!(out.status.code(), Some(1));
    let message = common::json(&out)["error"]["message"].to_string();
    assert!(
        message.contains("no top-level watchPaths, so the defaults apply"),
        "{message}"
    );
    assert_eq!(read(&env.config_file()), text);
}

#[test]
fn watch_waits_for_no_one_a_held_lock_is_exit_4() {
    let env = Env::new(Snapper::Missing);
    std::fs::create_dir_all(env.lock_file().parent().unwrap()).unwrap();
    let _lock = seldon::logbook::lock::acquire(&env.lock_file()).unwrap();
    let out = env.seldon(&["config", "watch", "--json", "--", "~/.config/git/config"]);
    assert_eq!(out.status.code(), Some(4));
    assert!(!env.config_file().exists());
}

/// The fixture's scan in the sample index (`fixtures/state/`) holds
/// paths the engine's own walk would also list: the sample is not a
/// made-up shape.
#[test]
fn the_fixture_scan_is_a_scan_the_engine_writes() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/state/recent-config.json");
    let saved: seldon::collectors::recent::Saved = serde_json::from_str(&read(&fixture)).unwrap();
    assert!(!saved.cut);
    assert!(saved.files.len() <= seldon::collectors::recent::MAX_FILES);
}
