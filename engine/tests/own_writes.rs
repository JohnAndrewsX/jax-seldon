//! The engine's own writes (WP-038, SPEC-ENGINE §5 rule 7): a file that
//! `init --theme-hook` or `hook install` writes under a watched path is
//! reported by the next capture as a config event that is already
//! explained, so no drift opens; the event stays in the ledger. The
//! removal commands (WP-049: `init --remove-theme-hook`, `hook uninstall`)
//! are recorded the same way: their `config-remove` or `config-change` is
//! explained as `removed by <command>`.
//!
//! Everything runs in a throw-away home (`common::Env`, with
//! `SELDON_TEST_GUARD`); the collectors' sources point at temp files.

mod common;

use std::path::PathBuf;
use std::process::Output;

use common::{Env, Snapper, assert_valid_index, json, read, stderr};
use serde_json::Value;

/// Where `omarchy hook install theme-set` puts Seldon's hook.
const HOOK: &str = "~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh";

/// An env whose `omarchy` does what `omarchy-hook-install` does (copy the
/// file into `$HOME/.config/omarchy/hooks/<type>.d/`, mode 755) and
/// nothing for any other call.
fn env() -> Env {
    let env = Env::new(Snapper::NoPermissions);
    env.stub(
        "omarchy",
        "PATH=/usr/bin:/bin\nif [ \"$1 $2\" = 'hook install' ]; then\n\
         \x20 d=\"$HOME/.config/omarchy/hooks/$3.d\"\n\
         \x20 mkdir -p \"$d\" && cp \"$4\" \"$d/${4##*/}\" && chmod 755 \"$d/${4##*/}\"\n\
         fi\nexit 0",
    );
    std::fs::write(env.tmp.path().join("pacman.log"), "").unwrap();
    env
}

/// `seldon args…` with the collectors' sources in the temp dir.
fn run(env: &Env, args: &[&str]) -> Output {
    let tmp = env.tmp.path();
    env.command(args)
        .env("SELDON_PACMAN_LOG", tmp.join("pacman.log"))
        .env("SELDON_PACMAN_DB_LOCK", tmp.join("no-db.lck"))
        .env("SELDON_OMARCHY_PLUGINS_DIR", tmp.join("plugins"))
        .env("SELDON_THEME_FILE", tmp.join("theme.name"))
        .env("SELDON_HARDWARE_ROOT", common::hardware_root())
        .output()
        .unwrap()
}

/// `seldon --json args…`, exit 0, the JSON.
fn ok(env: &Env, args: &[&str]) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let out = run(env, &all);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
    json(&out)
}

fn logbook(env: &Env) -> PathBuf {
    env.tmp.path().join("logbook")
}

/// `init --non-interactive --no-git` of `<tmp>/logbook` with `extra`.
fn init(env: &Env, extra: &[&str]) -> Value {
    let root = logbook(env);
    let mut args = vec![
        "init",
        "--non-interactive",
        "--no-git",
        "--path",
        root.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    ok(env, &args)
}

fn owned_file(env: &Env) -> PathBuf {
    env.home.join(".local/state/seldon/owned.json")
}

fn home_path(env: &Env, tilde: &str) -> PathBuf {
    env.home.join(tilde.strip_prefix("~/").unwrap())
}

/// The ledger lines of the logbook.
fn ledger(env: &Env) -> Vec<Value> {
    common::ledger(&logbook(env))
}

/// `(open drift items, the drift rows)` from `seldon drift`.
fn drift(env: &Env) -> (u64, Vec<Value>) {
    let v = ok(env, &["drift"]);
    (
        v["openDrift"].as_u64().unwrap(),
        v["drift"].as_array().unwrap().clone(),
    )
}

/// The config event for `subject` and the resolutions that refer to it.
fn event_and_resolutions(env: &Env, subject: &str) -> (Value, Vec<Value>) {
    let lines = ledger(env);
    let event = lines
        .iter()
        .find(|e| e["source"] == "config" && e["subject"] == subject)
        .unwrap_or_else(|| panic!("no config event for {subject}: {lines:?}"))
        .clone();
    let resolutions = lines
        .iter()
        .filter(|e| e["kind"] == "resolution" && e["refersTo"] == event["id"])
        .cloned()
        .collect();
    (event, resolutions)
}

/// The resolution rule 7 writes for `event`, explained by `by`.
fn assert_explained_by_seldon(event: &Value, resolutions: &[Value], by: &str) {
    assert_explained(event, resolutions, &format!("installed by {by}"));
}

/// The resolution rule 7 writes for `event`, with `detail`.
fn assert_explained(event: &Value, resolutions: &[Value], detail: &str) {
    assert_eq!(resolutions.len(), 1, "{resolutions:?}");
    let r = &resolutions[0];
    assert_eq!(r["source"], "seldon");
    assert_eq!(r["actor"], "system");
    assert_eq!(r["resolution"], "explained");
    assert_eq!(r["subject"], event["subject"]);
    assert_eq!(r["detail"], detail);
    assert!(r.get("case").is_none(), "{r}");
    // the event itself is the collector's, unchanged
    assert_eq!(event["actor"], "system");
    assert!(event.get("case").is_none(), "{event}");
}

fn index(env: &Env) -> Value {
    serde_json::from_str(&read(&env.home.join(".local/state/seldon/index.json"))).unwrap()
}

#[test]
fn init_with_the_theme_hook_then_capture_leaves_no_drift() {
    let env = env();
    let v = init(&env, &["--theme-hook"]);
    assert_eq!(v["themeHook"]["installed"], true, "{}", v["themeHook"]);
    assert_eq!(v["themeHook"]["ownWrites"], serde_json::json!([HOOK]));
    assert!(home_path(&env, HOOK).is_file());
    let owned: Value = serde_json::from_str(&read(&owned_file(&env))).unwrap();
    assert_eq!(owned[HOOK]["by"], "seldon init --theme-hook");
    assert_eq!(owned[HOOK]["hash"].as_str().unwrap().len(), 64);

    // the capture after init: the hook is a new file, explained at once
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["written"], 1, "{c}");
    assert_eq!(c["explainedOwn"], 1, "{c}");
    assert!(!owned_file(&env).exists(), "the record is used up");
    let (event, resolutions) = event_and_resolutions(&env, HOOK);
    assert_eq!(event["kind"], "config-add");
    assert_eq!(event["meta"]["hashTo"], owned[HOOK]["hash"]);
    assert_explained_by_seldon(&event, &resolutions, "seldon init --theme-hook");
    assert_eq!(drift(&env), (0, Vec::new()));

    // the index shows the event, explained, and no drift
    let ix = index(&env);
    assert_valid_index(&ix);
    assert_eq!(ix["summary"]["openDrift"], 0);
    let row = ix["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == event["id"])
        .expect("the event is in the index");
    assert_eq!(row["resolution"], "explained");
    assert_eq!(
        row["resolutionDetail"],
        "installed by seldon init --theme-hook"
    );

    // idempotent: a second capture writes nothing, explains nothing
    let lines = ledger(&env).len();
    let again = ok(&env, &["capture", "--all"]);
    assert_eq!(again["written"], 0, "{again}");
    assert_eq!(again["explainedOwn"], 0, "{again}");
    assert_eq!(ledger(&env).len(), lines);
    assert_eq!(drift(&env).0, 0);

    // the record was used once: a later edit of the hook is drift
    std::fs::write(home_path(&env, HOOK), "#!/bin/bash\necho mine\n").unwrap();
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(
        (c["written"].clone(), c["explainedOwn"].clone()),
        (1.into(), 0.into())
    );
    let (open, rows) = drift(&env);
    assert_eq!(open, 1);
    assert_eq!(rows[0]["subject"], HOOK);
    assert_eq!(rows[0]["kind"], "config-change");
}

#[test]
fn the_human_capture_says_what_it_explained() {
    let env = env();
    init(&env, &["--theme-hook"]);
    let out = run(&env, &["capture", "--all"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = common::stdout(&out);
    assert!(
        text.contains("note: 1 config event(s) explained as written by seldon itself"),
        "{text}"
    );
}

#[test]
fn a_hook_changed_before_the_capture_is_drift() {
    let env = env();
    init(&env, &["--theme-hook"]);
    // someone edits the installed hook before the next capture: what the
    // capture sees is no longer what seldon wrote
    std::fs::write(home_path(&env, HOOK), "#!/bin/bash\n# edited\n").unwrap();
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["written"], 1, "{c}");
    assert_eq!(c["explainedOwn"], 0, "{c}");
    let (event, resolutions) = event_and_resolutions(&env, HOOK);
    assert!(resolutions.is_empty(), "{resolutions:?}");
    let (open, rows) = drift(&env);
    assert_eq!(open, 1);
    assert_eq!(rows[0]["eventId"], event["id"]);
    assert!(!owned_file(&env).exists(), "the record is forgotten anyway");
}

#[test]
fn without_a_first_capture_the_hook_is_part_of_the_baseline() {
    let env = env();
    let v = init(&env, &["--no-capture", "--theme-hook"]);
    assert_eq!(v["themeHook"]["ownWrites"], serde_json::json!([HOOK]));
    // the first capture takes the config baseline: no event, nothing to
    // explain, the record is forgotten
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(
        (c["written"].clone(), c["explainedOwn"].clone()),
        (0.into(), 0.into())
    );
    assert!(!owned_file(&env).exists());
    assert!(ledger(&env).is_empty());
    assert_eq!(drift(&env).0, 0);
}

#[test]
fn a_capture_without_the_config_collector_keeps_the_record() {
    let env = env();
    init(&env, &["--theme-hook"]);
    let c = ok(&env, &["capture", "--source", "pacman"]);
    assert_eq!(c["explainedOwn"], 0, "{c}");
    assert!(
        owned_file(&env).is_file(),
        "config has not seen the file yet"
    );
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["explainedOwn"], 1, "{c}");
    assert_eq!(drift(&env).0, 0);
}

#[test]
fn a_capture_during_the_theme_hook_install_finds_the_lock_held() {
    // WP-052/WP-074: init holds the state lock from the script to the
    // own-write record. The `omarchy` stub copies the hook into place and
    // then starts a capture, as the plugin's timer could: it must get exit
    // 4, not see the copy before its record
    let env = env();
    let tmp = env.tmp.path();
    let (code, out) = (tmp.join("race.code"), tmp.join("race.out"));
    env.stub(
        "omarchy",
        &format!(
            "P=$PATH\nPATH=/usr/bin:/bin\nif [ \"$1 $2\" = 'hook install' ]; then\n\
             \x20 d=\"$HOME/.config/omarchy/hooks/$3.d\"\n\
             \x20 mkdir -p \"$d\" && cp \"$4\" \"$d/${{4##*/}}\" && chmod 755 \"$d/${{4##*/}}\"\n\
             \x20 PATH=$P '{seldon}' --json capture --all > '{out}' 2>&1\n\
             \x20 echo $? > '{code}'\n\
             fi\nexit 0",
            seldon = env!("CARGO_BIN_EXE_seldon"),
            out = out.display(),
            code = code.display(),
        ),
    );
    let v = init(&env, &["--theme-hook"]);
    assert_eq!(v["themeHook"]["installed"], true, "{}", v["themeHook"]);
    assert_eq!(
        read(&code).trim(),
        "4",
        "the capture during the install: {}",
        read(&out)
    );
    assert_eq!(v["themeHook"]["ownWrites"], serde_json::json!([HOOK]));
    // the next capture finds the record: explained, no drift
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["explainedOwn"], 1, "{c}");
    let (event, resolutions) = event_and_resolutions(&env, HOOK);
    assert_explained_by_seldon(&event, &resolutions, "seldon init --theme-hook");
    assert_eq!(drift(&env), (0, Vec::new()));
}

/// Every file and directory under `dir` with its bytes, sorted.
fn tree(dir: &std::path::Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(dir).unwrap().to_path_buf();
            if path.is_dir() && !path.is_symlink() {
                out.push((rel, None));
                stack.push(path);
            } else {
                out.push((rel, std::fs::read(&path).ok()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_held_lock_stops_init_with_the_theme_hook_before_any_write() {
    // while another seldon holds the state lock: exit 4, no file changed
    // (no logbook, no config, no script, no hook), no owned.json record,
    // `omarchy` never runs
    let env = env();
    let calls = env.tmp.path().join("omarchy.calls");
    env.stub(
        "omarchy",
        &format!("echo \"$*\" >> '{}'\nexit 0", calls.display()),
    );
    let lock = seldon::logbook::lock::acquire(&env.lock_file()).unwrap();
    let before = tree(env.tmp.path());
    let root = logbook(&env);
    let out = run(
        &env,
        &[
            "--json",
            "init",
            "--non-interactive",
            "--no-git",
            "--theme-hook",
            "--path",
            root.to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(4), "{}", stderr(&out));
    assert_eq!(json(&out)["error"]["code"], 4);
    assert!(tree(env.tmp.path()) == before, "files byte-identical");
    assert!(!owned_file(&env).exists());
    assert!(!home_path(&env, HOOK).exists());
    assert!(!calls.exists(), "omarchy ran: {}", read(&calls));
    drop(lock);
}

#[test]
fn hook_install_under_a_watched_path_leaves_no_drift() {
    let env = env();
    // the user also watches Claude Code's global settings
    let config = env.config_file();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        "watchPaths = [\"~/.config/hypr\", \"~/.config/omarchy\", \"~/.claude\"]\n",
    )
    .unwrap();
    init(&env, &[]);
    assert!(
        !owned_file(&env).exists(),
        "nothing to record without the hook"
    );

    let settings = env.home.join(".claude/settings.json");
    let v = ok(
        &env,
        &[
            "hook",
            "install",
            "claude-code",
            "--settings",
            settings.to_str().unwrap(),
        ],
    );
    assert_eq!(v["added"].as_array().unwrap().len(), 3, "{v}");
    assert_eq!(
        v["ownWrites"],
        serde_json::json!(["~/.claude/settings.json"])
    );

    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(
        (c["written"].clone(), c["explainedOwn"].clone()),
        (1.into(), 1.into()),
        "{c}"
    );
    let (event, resolutions) = event_and_resolutions(&env, "~/.claude/settings.json");
    assert_eq!(event["kind"], "config-add");
    assert_explained_by_seldon(&event, &resolutions, "seldon hook install claude-code");
    assert_eq!(drift(&env), (0, Vec::new()));

    // a second install changes nothing and records nothing
    let v = ok(
        &env,
        &[
            "hook",
            "install",
            "claude-code",
            "--settings",
            settings.to_str().unwrap(),
        ],
    );
    assert!(v["added"].as_array().unwrap().is_empty(), "{v}");
    assert_eq!(v["ownWrites"], Value::Null);
    assert!(!owned_file(&env).exists());
    let again = ok(&env, &["capture", "--all"]);
    assert_eq!(
        (again["written"].clone(), again["explainedOwn"].clone()),
        (0.into(), 0.into())
    );
    assert_eq!(drift(&env).0, 0);

    // the logbook's own settings file is not watched: nothing recorded
    let v = ok(&env, &["hook", "install", "claude-code"]);
    assert_eq!(v["added"].as_array().unwrap().len(), 3, "{v}");
    assert_eq!(v["ownWrites"], serde_json::json!([]));
    assert!(!owned_file(&env).exists());
}

#[test]
fn hook_install_merging_into_a_watched_file_is_an_explained_change() {
    let env = env();
    let config = env.config_file();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "watchPaths = [\"~/.claude\"]\n").unwrap();
    let settings = env.home.join(".claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(&settings, "{\"model\": \"opus\"}\n").unwrap();
    init(&env, &[]);

    ok(
        &env,
        &[
            "hook",
            "install",
            "claude-code",
            "--settings",
            settings.to_str().unwrap(),
        ],
    );
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["explainedOwn"], 1, "{c}");
    let (event, resolutions) = event_and_resolutions(&env, "~/.claude/settings.json");
    assert_eq!(event["kind"], "config-change");
    assert_explained_by_seldon(&event, &resolutions, "seldon hook install claude-code");
    assert_eq!(drift(&env).0, 0);
}

/// The config event of `kind` for `subject` and the resolutions that refer
/// to it (a path can have an add and a remove).
fn event_of_kind(env: &Env, subject: &str, kind: &str) -> (Value, Vec<Value>) {
    let lines = ledger(env);
    let event = lines
        .iter()
        .find(|e| e["source"] == "config" && e["subject"] == subject && e["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} for {subject}: {lines:?}"))
        .clone();
    let resolutions = lines
        .iter()
        .filter(|e| e["kind"] == "resolution" && e["refersTo"] == event["id"])
        .cloned()
        .collect();
    (event, resolutions)
}

#[test]
fn removing_the_theme_hook_leaves_no_drift() {
    let env = env();
    init(&env, &["--theme-hook"]);
    let script = env
        .home
        .join(".local/state/seldon/hooks/seldon-theme-set.sh");
    assert!(script.is_file());
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["explainedOwn"], 1, "{c}");

    let v = ok(&env, &["init", "--remove-theme-hook"]);
    assert_eq!(v["removed"], true, "{v}");
    assert_eq!(v["scriptRemoved"], true, "{v}");
    assert_eq!(v["ownWrites"], serde_json::json!([HOOK]));
    assert!(!home_path(&env, HOOK).exists());
    assert!(!script.exists());
    assert!(
        home_path(&env, HOOK).parent().unwrap().is_dir(),
        "Omarchy's hook directory stays"
    );
    let owned: Value = serde_json::from_str(&read(&owned_file(&env))).unwrap();
    assert_eq!(owned[HOOK]["by"], "seldon init --remove-theme-hook");
    assert_eq!(owned[HOOK]["op"], "delete");

    // the next capture sees the removal and explains it at once
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(
        (c["written"].clone(), c["explainedOwn"].clone()),
        (1.into(), 1.into()),
        "{c}"
    );
    assert!(!owned_file(&env).exists(), "the record is used up");
    let (event, resolutions) = event_of_kind(&env, HOOK, "config-remove");
    let (added, _) = event_of_kind(&env, HOOK, "config-add");
    assert_eq!(event["meta"]["hashFrom"], added["meta"]["hashTo"]);
    assert_explained(
        &event,
        &resolutions,
        "removed by seldon init --remove-theme-hook",
    );
    assert_eq!(drift(&env), (0, Vec::new()));
    let ix = index(&env);
    assert_valid_index(&ix);
    assert_eq!(ix["summary"]["openDrift"], 0);

    // idempotent: nothing left to remove, nothing recorded, nothing written
    let v = ok(&env, &["init", "--remove-theme-hook"]);
    assert_eq!(
        (v["removed"].clone(), v["scriptRemoved"].clone()),
        (false.into(), false.into())
    );
    assert_eq!(v["ownWrites"], Value::Null);
    assert!(!owned_file(&env).exists());
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["written"], 0, "{c}");
    assert_eq!(drift(&env).0, 0);
}

#[test]
fn removing_the_theme_hook_before_any_capture_saw_it_writes_nothing() {
    let env = env();
    init(&env, &["--theme-hook"]);
    ok(&env, &["init", "--remove-theme-hook"]);
    // installed and removed between two captures: the collector never saw
    // the file, so there is no event and nothing to explain
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(
        (c["written"].clone(), c["explainedOwn"].clone()),
        (0.into(), 0.into()),
        "{c}"
    );
    assert!(!owned_file(&env).exists());
    assert_eq!(drift(&env).0, 0);
}

#[test]
fn a_hook_changed_by_hand_and_then_removed_is_drift() {
    let env = env();
    init(&env, &["--theme-hook"]);
    ok(&env, &["capture", "--all"]);
    // the user edits the hook; no capture sees the edit before the removal
    std::fs::write(home_path(&env, HOOK), "#!/bin/bash\n# mine\n").unwrap();
    ok(&env, &["init", "--remove-theme-hook"]);
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["explainedOwn"], 0, "{c}");
    let (open, rows) = drift(&env);
    assert_eq!(
        open, 1,
        "the content seldon removed is not what it installed"
    );
    assert_eq!(rows[0]["kind"], "config-remove");
}

#[test]
fn hook_uninstall_under_a_watched_path_leaves_no_drift() {
    let env = env();
    let config = env.config_file();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "watchPaths = [\"~/.claude\"]\n").unwrap();
    init(&env, &[]);
    let settings = env.home.join(".claude/settings.json");
    let path = settings.to_str().unwrap();
    ok(
        &env,
        &["hook", "install", "claude-code", "--settings", path],
    );
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["explainedOwn"], 1, "{c}");

    // the file held only Seldon's hooks: it goes, and the removal is ours
    let v = ok(
        &env,
        &["hook", "uninstall", "claude-code", "--settings", path],
    );
    assert_eq!(v["deleted"], true, "{v}");
    assert_eq!(
        v["ownWrites"],
        serde_json::json!(["~/.claude/settings.json"])
    );
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(
        (c["written"].clone(), c["explainedOwn"].clone()),
        (1.into(), 1.into()),
        "{c}"
    );
    let (event, resolutions) = event_of_kind(&env, "~/.claude/settings.json", "config-remove");
    assert_explained(
        &event,
        &resolutions,
        "removed by seldon hook uninstall claude-code",
    );
    assert_eq!(drift(&env), (0, Vec::new()));
}

#[test]
fn hook_uninstall_from_a_shared_watched_file_is_an_explained_change() {
    let env = env();
    let config = env.config_file();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "watchPaths = [\"~/.claude\"]\n").unwrap();
    let settings = env.home.join(".claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(&settings, "{\"model\": \"opus\"}\n").unwrap();
    init(&env, &[]);
    let path = settings.to_str().unwrap();
    ok(
        &env,
        &["hook", "install", "claude-code", "--settings", path],
    );
    ok(&env, &["capture", "--all"]);

    let v = ok(
        &env,
        &["hook", "uninstall", "claude-code", "--settings", path],
    );
    assert_eq!(v["deleted"], false, "{v}");
    assert_eq!(
        serde_json::from_str::<Value>(&read(&settings)).unwrap(),
        serde_json::json!({"model": "opus"})
    );
    let owned: Value = serde_json::from_str(&read(&owned_file(&env))).unwrap();
    assert_eq!(owned["~/.claude/settings.json"]["op"], "remove");
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["explainedOwn"], 1, "{c}");
    let lines = ledger(&env);
    let changes: Vec<&Value> = lines
        .iter()
        .filter(|e| e["kind"] == "config-change")
        .collect();
    assert_eq!(changes.len(), 2, "install and uninstall: {lines:?}");
    let last = changes[1];
    let resolutions: Vec<Value> = lines
        .iter()
        .filter(|e| e["kind"] == "resolution" && e["refersTo"] == last["id"])
        .cloned()
        .collect();
    assert_explained(
        last,
        &resolutions,
        "removed by seldon hook uninstall claude-code",
    );
    assert_eq!(drift(&env).0, 0);
}

#[test]
fn the_deletion_is_recorded_before_the_file_goes() {
    use std::os::unix::fs::PermissionsExt as _;
    let env = env();
    init(&env, &["--theme-hook"]);
    ok(&env, &["capture", "--all"]);
    assert!(!owned_file(&env).exists());
    // a read-only hook directory: the deletion fails after the record
    let dir = home_path(&env, HOOK).parent().unwrap().to_path_buf();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(dir.join("probe"), "").is_ok() {
        eprintln!("note: running as root, a read-only directory is writable; skipped");
        return;
    }
    let out = run(&env, &["--json", "init", "--remove-theme-hook"]);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(home_path(&env, HOOK).is_file(), "the hook is still there");
    let owned: Value = serde_json::from_str(&read(&owned_file(&env))).unwrap();
    assert_eq!(owned[HOOK]["op"], "delete", "recorded first: {owned}");
    assert_eq!(owned[HOOK]["by"], "seldon init --remove-theme-hook");

    // the stale record is harmless: the file is unchanged, so the next
    // capture writes nothing and forgets it
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(
        (c["written"].clone(), c["explainedOwn"].clone()),
        (0.into(), 0.into()),
        "{c}"
    );
    assert!(!owned_file(&env).exists());
    assert_eq!(drift(&env).0, 0);
}
