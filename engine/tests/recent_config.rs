//! Recently edited under `~/.config`, outside the watch paths (ADR-0046,
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
fn a_logbook_under_dot_config_is_not_listed() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook_at("home/.config/logbook", "en");
    ok(&env.seldon(&["capture", "--json"]));
    ok(&env.seldon(&["log", "--json", "--", "a note in the journal"]));
    file(&env, ".config/git/config", HOUR);
    ok(&env.seldon(&["capture", "--json"]));
    assert_eq!(recent_paths(&index(&env)), ["~/.config/git/config"]);
}

fn link(to: &Path, at: &Path) {
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(to, at).unwrap();
}

fn add_watch_path(env: &Env, path: &str) {
    edit_config(env, |c| c.watch_paths.push(path.into()));
}

fn config_events(lb: &Path, subject: &str) -> usize {
    common::ledger(lb)
        .iter()
        .filter(|e| e["source"] == json!("config") && e["subject"] == json!(subject))
        .count()
}

/// WP-139 round 3, B1: a link to a skipped secret is neither listed nor
/// watchable, and a watch path written by hand that is such a link is
/// never opened or hashed; nor is a link to it inside a watched folder.
#[test]
fn a_link_to_a_skipped_secret_is_never_listed_watched_or_hashed() {
    let (env, lb) = setup();
    edit_config(&env, |c| c.redaction.skip_paths.push("~/secrets/".into()));
    let secret = file(&env, "secrets/token", HOUR);
    link(&secret, &env.home.join(".config/app/token.conf"));
    link(&secret, &env.home.join(".config/hypr/token.conf"));
    file(&env, ".config/git/config", HOUR);
    let out = ok(&env.seldon(&["capture", "--json"]));
    assert_eq!(recent_paths(&index(&env)), ["~/.config/git/config"]);
    let refused = env.seldon(&[
        "config",
        "watch",
        "--json",
        "--",
        "~/.config/app/token.conf",
    ]);
    assert_eq!(refused.status.code(), Some(1));
    let message = common::json(&refused)["error"]["message"].to_string();
    assert!(
        message.contains("leads through a link to a path under [redaction] skipPaths"),
        "{message}"
    );
    // written by hand: the collector does not follow it
    add_watch_path(&env, "~/.config/app/token.conf");
    ok(&env.seldon(&["capture", "--json"]));
    std::fs::write(&secret, "token=changed\n").unwrap();
    let out2 = ok(&env.seldon(&["capture", "--json"]));
    assert_eq!(config_events(&lb, "~/.config/app/token.conf"), 0);
    assert_eq!(config_events(&lb, "~/.config/hypr/token.conf"), 0);
    let messages = format!("{out}{out2}");
    assert!(messages.contains("link(s) not followed"), "{messages}");
    // neither hashed nor listed as skipped: never opened
    let manifest = json_file(&env.home.join(".local/state/seldon/manifest.json"));
    let held = format!("{}{}", manifest["files"], manifest["skipped"]);
    assert!(!held.contains("token.conf"), "{held}");
}

/// WP-139 stage 2, B1: a folder link at a persistence path (the default
/// watch path `~/.config/systemd/user`) to a skipped folder is not
/// entered (A), and below a followed link a skipped folder where it really
/// lies is not read (B): no event, nothing in the manifest.
#[test]
fn a_folder_link_in_a_watched_folder_never_reads_a_skipped_file() {
    let (env, lb) = setup();
    edit_config(&env, |c| {
        c.redaction.skip_paths.extend([
            "~/secrets/".into(),
            "~/units/private/".into(),
            "~/units/top.secret".into(),
        ])
    });
    // one refusal per layer, so each counts: the link to a skipped folder
    // (not entered, though it holds two files), a skipped folder below a
    // followed link (not entered, two files), a skipped file directly
    // below it
    let token = file(&env, "secrets/token", HOUR);
    file(&env, "secrets/key", HOUR);
    let pw = file(&env, "units/private/pw.conf", HOUR);
    file(&env, "units/private/pin.conf", HOUR);
    let top = file(&env, "units/top.secret", HOUR);
    file(&env, "units/ok.service", HOUR);
    let user = env.home.join(".config/systemd/user");
    link(&env.home.join("secrets"), &user.join("foo.d"));
    link(&env.home.join("units"), &user.join("bar.d"));
    let first = ok(&env.seldon(&["capture", "--json"]));
    std::fs::write(&token, "token=changed\n").unwrap();
    std::fs::write(&pw, "pw=changed\n").unwrap();
    std::fs::write(&top, "top=changed\n").unwrap();
    let second = ok(&env.seldon(&["capture", "--json"]));
    for subject in [
        "~/.config/systemd/user/foo.d/token",
        "~/.config/systemd/user/bar.d/private/pw.conf",
        "~/.config/systemd/user/bar.d/top.secret",
    ] {
        assert_eq!(config_events(&lb, subject), 0, "{subject}");
    }
    // the allowed part of B is watched
    assert_eq!(
        config_events(&lb, "~/.config/systemd/user/bar.d/ok.service"),
        1
    );
    let manifest = json_file(&env.home.join(".local/state/seldon/manifest.json"));
    let held = format!("{}{}", manifest["files"], manifest["skipped"]);
    assert!(!held.contains("foo.d"), "{held}");
    assert!(!held.contains("private"), "{held}");
    assert!(!held.contains("top.secret"), "{held}");
    for out in [&first, &second] {
        let text = out.to_string();
        assert!(text.contains("3 link(s) not followed"), "{text}");
    }
}

/// WP-139 round 3, B2: a link into Seldon's own state is neither listed
/// nor watchable; written by hand into watchPaths, it is not followed, so
/// captures stay idempotent: three in a row, nothing after the first.
#[test]
fn a_link_into_seldons_state_keeps_captures_idempotent() {
    let (env, lb) = setup();
    let index_json = env.home.join(".local/state/seldon/index.json");
    link(&index_json, &env.home.join(".config/ownlink.json"));
    ok(&env.seldon(&["capture", "--json"]));
    assert!(recent_paths(&index(&env)).is_empty());
    let refused = env.seldon(&["config", "watch", "--json", "--", "~/.config/ownlink.json"]);
    assert_eq!(refused.status.code(), Some(1));
    let message = common::json(&refused)["error"]["message"].to_string();
    assert!(message.contains("into Seldon's own files"), "{message}");
    add_watch_path(&env, "~/.config/ownlink.json");
    let first = ok(&env.seldon(&["capture", "--json"]));
    for _ in 0..2 {
        let again = ok(&env.seldon(&["capture", "--json"]));
        assert_eq!(again["written"], json!(0), "{again}");
    }
    assert_eq!(config_events(&lb, "~/.config/ownlink.json"), 0, "{first}");
}

/// WP-139 round 3, N1: a link out of `~/.config` is not listed; `config
/// watch` refuses one out of the home directory.
#[test]
fn a_link_out_of_the_home_is_not_listed_and_not_watchable() {
    let (env, _) = setup();
    let outside = env.tmp.path().join("outside/app.conf");
    std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
    std::fs::write(&outside, "x\n").unwrap();
    link(&outside, &env.home.join(".config/out.conf"));
    let dotfile = file(&env, "dotfiles/foot.ini", HOUR);
    link(&dotfile, &env.home.join(".config/foot/foot.ini"));
    ok(&env.seldon(&["capture", "--json"]));
    assert!(recent_paths(&index(&env)).is_empty());
    let refused = env.seldon(&["config", "watch", "--json", "--", "~/.config/out.conf"]);
    assert_eq!(refused.status.code(), Some(1));
    let message = common::json(&refused)["error"]["message"].to_string();
    assert!(message.contains("outside your home directory"), "{message}");
    // inside the home but outside ~/.config: watchable, not listed
    let added = ok(&env.seldon(&["config", "watch", "--json", "--", "~/.config/foot/foot.ini"]));
    assert_eq!(added["added"], json!(true));
}

/// WP-139 round 3b (orchestrator decision): `~/.config` itself a link to a
/// folder outside the home (a dotfile setup): that folder counts as
/// `~/.config`, so `config watch` takes a path there, as the collector
/// does; skipPaths, Seldon's own files and a link out of it still refuse.
#[test]
fn a_dot_config_that_links_out_of_the_home_counts_as_dot_config() {
    let env = Env::new(Snapper::Missing);
    let dots = env.tmp.path().join("dotfiles");
    std::fs::create_dir_all(&dots).unwrap();
    link(&dots, &env.home.join(".config"));
    file(&env, ".config/app/x.conf", HOUR);
    file(&env, ".config/secret/token", HOUR);
    let elsewhere = env.tmp.path().join("elsewhere/y.conf");
    std::fs::create_dir_all(elsewhere.parent().unwrap()).unwrap();
    std::fs::write(&elsewhere, "y\n").unwrap();
    link(&elsewhere, &env.home.join(".config/app/out.conf"));
    std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
    std::fs::write(
        env.config_file(),
        "watchPaths = [\"~/.config/hypr\"]\n\n[redaction]\nskipPaths = [\"~/.config/secret/\"]\n",
    )
    .unwrap();
    let added = ok(&env.seldon(&["config", "watch", "--json", "--", "~/.config/app/x.conf"]));
    assert_eq!(added["added"], json!(true));
    for (path, says) in [
        ("~/.config/secret/token", "matches [redaction] skipPaths"),
        ("~/.config/seldon/config.toml", "Seldon's own files"),
        ("~/.config/app/out.conf", "outside your home directory"),
    ] {
        let out = env.seldon(&["config", "watch", "--json", "--", path]);
        assert_eq!(out.status.code(), Some(1), "{path}");
        let message = common::json(&out)["error"]["message"].to_string();
        assert!(message.contains(says), "{path}: {message}");
    }
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
    // watched by a default path from the start
    file(&env, ".config/hypr/x.lua", HOUR);
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
    file(&env, ".config/git/config", HOUR);
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
    file(&env, "starship.toml", HOUR);
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
    file(&env, ".config/secret/token.conf", HOUR);
    file(&env, ".config/app/private/a.conf", HOUR);
    let text = read(&env.config_file());
    let control = "~/.config/a\u{1b}b";
    let bidi = "~/.config/a\u{202e}b";
    let separator = "~/.config/a\u{2028}b";
    let long = format!("~/.config/{}", "x".repeat(510));
    let cases: [(&str, &str); 14] = [
        ("/etc/pacman.conf", "is not below your home directory"),
        ("~/.config/nope.conf", "does not exist"),
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
        (separator, "cannot be a watch path"),
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
    file(&env, ".config/git/config", HOUR);
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
    file(&env, ".config/git/config", HOUR);
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
    file(&env, ".config/git/config", HOUR);
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
    assert!(!saved.partial);
    assert!(saved.files.len() <= seldon::collectors::recent::MAX_FILES);
}
