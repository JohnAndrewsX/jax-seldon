//! ADR-0028 through the command line (WP-109): the capture-time evidence
//! marks, the built-in templates as rule-7 evidence, the new default watch
//! paths and their upgrade, `doctor`'s rows, and the index build that
//! writes nothing to the ledger. Always through `common::Env` (a temp
//! HOME; Omarchy's tree is `<tmp>/omarchy`, never the host's).

mod common;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use common::{Env, Snapper, copy_dir, fixture_logbook, json, read, stderr};

/// The sample index's clock (`fixtures/index.sample.json`).
const GENERATED_AT: &str = "2026-10-01T17:05:12+02:00";

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn ok(env: &Env, args: &[&str]) -> Value {
    let out = env.seldon(args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
    json(&out)
}

fn capture_config(env: &Env) -> Value {
    ok(env, &["capture", "--source", "config", "--json"])
}

/// `subject → (meta.matches, resolution)` of the config events in the
/// ledger, the last per subject.
fn config_events(root: &Path) -> Vec<(String, Value, Value)> {
    let ledger = common::ledger(root);
    let resolution = |id: &Value| -> Value {
        ledger
            .iter()
            .rev()
            .find(|e| e["kind"] == "resolution" && &e["refersTo"] == id)
            .map(|e| json!([e["resolution"], e["detail"]]))
            .unwrap_or(Value::Null)
    };
    ledger
        .iter()
        .filter(|e| e["source"] == "config")
        .map(|e| {
            (
                e["subject"].as_str().unwrap().to_string(),
                e["meta"]["matches"].clone(),
                resolution(&e["id"]),
            )
        })
        .collect()
}

fn find<'a>(events: &'a [(String, Value, Value)], subject: &str) -> &'a (String, Value, Value) {
    events
        .iter()
        .find(|e| e.0 == subject)
        .unwrap_or_else(|| panic!("{subject} in {events:?}"))
}

/// `seldon drift [--all] --json`: (subject, class, rule) of each item.
fn items(env: &Env, all: bool) -> Vec<(String, String, String)> {
    let mut args = vec!["drift", "--json"];
    if all {
        args.push("--all");
    }
    ok(env, &args)["drift"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["subject"].as_str().unwrap().to_string(),
                d["class"].as_str().unwrap().to_string(),
                d["rule"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// ADR-0028 §5: a new config event carries the evidence it had at
/// capture (`omarchy-default`, `system-link`, `theme-repo`), and the
/// class follows it. `omarchy refresh` writes `<file>.bak.<epoch>` and
/// Omarchy's copy: both routine. A new hook is a crisis.
#[test]
fn capture_marks_the_evidence_and_the_class_follows_it() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let home = &env.home;
    let omarchy = env.tmp.path().join("omarchy");
    let lua = home.join(".config/hypr/hyprland.lua");
    write(&lua, "-- mine\n");
    capture_config(&env); // baseline

    // `omarchy refresh config hypr/hyprland.lua`
    write(&omarchy.join("config/hypr/hyprland.lua"), "-- Omarchy's\n");
    write(
        &home.join(".config/hypr/hyprland.lua.bak.1786539345"),
        "-- mine\n",
    );
    write(&lua, "-- Omarchy's\n");
    // `omarchy-refresh-applications`
    write(
        &omarchy.join("applications/HEY.desktop"),
        "[Desktop Entry]\nName=HEY\n",
    );
    write(
        &home.join(".local/share/applications/HEY.desktop"),
        "[Desktop Entry]\nName=HEY\n",
    );
    write(
        &omarchy.join("default/alacritty/Alacritty.desktop"),
        "[Desktop Entry]\nName=Alacritty\n",
    );
    write(
        &home.join(".local/share/applications/Alacritty.desktop"),
        "[Desktop Entry]\nName=Alacritty\n",
    );
    // a shipped sample hook, byte-equal
    write(
        &omarchy.join("config/omarchy/hooks/post-update.d/x.sample"),
        "#!/bin/bash\n",
    );
    write(
        &home.join(".config/omarchy/hooks/post-update.d/x.sample"),
        "#!/bin/bash\n",
    );
    // `omarchy theme install`: a clone with its .git; Omarchy strips its code
    write(
        &home.join(".config/omarchy/themes/repo/.git/HEAD"),
        "ref: refs/heads/main\n",
    );
    write(
        &home.join(".config/omarchy/themes/repo/hyprland.lua"),
        "-- theme\n",
    );
    // a theme the user wrote: code is attention, the colours are routine
    write(
        &home.join(".config/omarchy/themes/mine/hyprland.lua"),
        "-- theme\n",
    );
    write(
        &home.join(".config/omarchy/themes/mine/colors.toml"),
        "accent = \"#fff\"\n",
    );
    // `systemctl --user enable` of a packaged unit: a link into /usr
    let usr = Path::new("/usr/lib/os-release");
    let link = home.join(".config/systemd/user/default.target.wants/packaged.service");
    if usr.is_file() {
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(usr, &link).unwrap();
    }
    // persistence nobody asked for
    write(
        &home.join(".config/omarchy/hooks/post-update.d/backup.sh"),
        "#!/bin/bash\nrsync\n",
    );
    // the shell rewrites its state file
    write(&home.join(".config/omarchy/shell.json"), "{}\n");

    let c = capture_config(&env);
    assert_eq!(c["ok"], true, "{c}");
    let events = config_events(&env.tmp.path().join("logbook"));
    let mark = |s: &str| find(&events, s).1.clone();
    assert_eq!(mark("~/.config/hypr/hyprland.lua"), "omarchy-default");
    assert_eq!(
        mark("~/.config/hypr/hyprland.lua.bak.1786539345"),
        Value::Null
    );
    assert_eq!(
        mark("~/.local/share/applications/HEY.desktop"),
        "omarchy-default"
    );
    assert_eq!(
        mark("~/.local/share/applications/Alacritty.desktop"),
        "omarchy-default"
    );
    assert_eq!(
        mark("~/.config/omarchy/hooks/post-update.d/x.sample"),
        "omarchy-default"
    );
    assert_eq!(
        mark("~/.config/omarchy/themes/repo/hyprland.lua"),
        "theme-repo"
    );
    assert_eq!(
        mark("~/.config/omarchy/themes/mine/hyprland.lua"),
        Value::Null
    );
    assert_eq!(
        mark("~/.config/omarchy/hooks/post-update.d/backup.sh"),
        Value::Null
    );
    if usr.is_file() {
        assert_eq!(
            mark("~/.config/systemd/user/default.target.wants/packaged.service"),
            "system-link"
        );
    }

    let open = items(&env, false);
    assert_eq!(
        open,
        [
            (
                "~/.config/omarchy/themes/mine/hyprland.lua".to_string(),
                "attention".to_string(),
                "config".to_string()
            ),
            (
                "~/.config/omarchy/hooks/post-update.d/backup.sh".to_string(),
                "crisis".to_string(),
                "always-red-paths".to_string()
            ),
        ],
        "everything else is routine"
    );
    let rule = |s: &str| -> String {
        items(&env, true)
            .into_iter()
            .find(|i| i.0 == s)
            .map(|i| format!("{} {}", i.1, i.2))
            .unwrap_or_else(|| panic!("{s}"))
    };
    assert_eq!(
        rule("~/.config/hypr/hyprland.lua"),
        "routine omarchy-default"
    );
    assert_eq!(
        rule("~/.config/hypr/hyprland.lua.bak.1786539345"),
        "routine routine-paths"
    );
    assert_eq!(
        rule("~/.config/omarchy/shell.json"),
        "routine routine-paths"
    );
    assert_eq!(
        rule("~/.config/omarchy/themes/mine/colors.toml"),
        "routine theme-assets"
    );
    assert_eq!(
        rule("~/.config/omarchy/themes/repo/hyprland.lua"),
        "routine theme-repo"
    );
    if usr.is_file() {
        assert_eq!(
            rule("~/.config/systemd/user/default.target.wants/packaged.service"),
            "routine system-link"
        );
    }
    // the ledger holds the fact, never the target or the content
    let ledger = read(&env.tmp.path().join("logbook/ledger").join(month(&env)));
    assert!(!ledger.contains("os-release"), "{ledger}");
}

/// The ledger month file a real-clock capture wrote.
fn month(env: &Env) -> PathBuf {
    let dir = env.tmp.path().join("logbook/ledger");
    std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .map(|p| PathBuf::from(p.file_name().unwrap()))
        .unwrap()
}

/// ADR-0028 §2 rule 7: the engine's built-in templates explain a config
/// event without `owned.json` (a lost state directory, `install.sh
/// --unit` with any prefix); a unit that differs in more than its
/// `ExecStart` stays a crisis.
#[test]
fn built_in_templates_explain_without_owned_json() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let home = &env.home;
    capture_config(&env); // baseline
    let owned = home.join(".local/state/seldon/owned.json");
    assert!(!owned.exists());

    let unit = seldon::collectors::config::WATCH_UNIT;
    assert!(unit.contains("\nExecStart=%h/.local/bin/seldon watch\n"));
    let prefixed = unit.replace(
        "ExecStart=%h/.local/bin/seldon watch",
        "ExecStart=/opt/tools/bin/seldon watch",
    );
    write(
        &home.join(".config/systemd/user/seldon-watch.service"),
        &prefixed,
    );
    let tampered = unit.replace(
        "ExecStart=%h/.local/bin/seldon watch",
        "ExecStartPre=/tmp/x\nExecStart=%h/.local/bin/seldon watch",
    );
    write(
        &home.join(".config/systemd/user/other.target.wants/seldon-watch.service"),
        &tampered,
    );
    let spaced = unit.replace(
        "ExecStart=%h/.local/bin/seldon watch",
        "ExecStart=/bin/sh -c x; /seldon watch",
    );
    write(
        &home.join(".config/systemd/user/third.target.wants/seldon-watch.service"),
        &spaced,
    );
    write(
        &home.join(".config/omarchy/hooks/theme-set.d/seldon-theme-set.sh"),
        seldon::commands::setup::THEME_HOOK_SCRIPT,
    );
    // the same bytes under another name are no template
    write(
        &home.join(".config/omarchy/hooks/theme-set.d/copy.sh"),
        seldon::commands::setup::THEME_HOOK_SCRIPT,
    );

    let c = capture_config(&env);
    assert_eq!(c["explainedOwn"], 2, "{c}");
    let events = config_events(&env.tmp.path().join("logbook"));
    let res = |s: &str| find(&events, s).2.clone();
    assert_eq!(
        res("~/.config/systemd/user/seldon-watch.service"),
        json!([
            "explained",
            "installed by install.sh --unit (built-in template)"
        ])
    );
    assert_eq!(
        res("~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh"),
        json!([
            "explained",
            "installed by seldon init --theme-hook (built-in template)"
        ])
    );
    for s in [
        "~/.config/systemd/user/other.target.wants/seldon-watch.service",
        "~/.config/systemd/user/third.target.wants/seldon-watch.service",
        "~/.config/omarchy/hooks/theme-set.d/copy.sh",
    ] {
        assert_eq!(res(s), Value::Null, "{s}");
    }
    let crises: Vec<String> = items(&env, false)
        .into_iter()
        .filter(|i| i.1 == "crisis")
        .map(|i| i.0)
        .collect();
    assert_eq!(crises.len(), 3, "{crises:?}");
    assert!(!owned.exists(), "nothing recorded, nothing left behind");
}

/// ADR-0028 §4d: `init` writes the six persistence paths; a config whose
/// list is an earlier engine's default gains them at the next capture,
/// which says so once; a list the user wrote is never widened, and
/// `doctor` names what it lacks with the line to add.
#[test]
fn default_watch_paths_and_their_upgrade() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let config = || -> toml::Table { read(&env.config_file()).parse().unwrap() };
    let watch = |t: &toml::Table| -> Vec<String> {
        t["watchPaths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(watch(&config()), seldon::config::DEFAULT_WATCH_PATHS);

    let new: Vec<String> = seldon::config::DEFAULT_WATCH_PATHS[6..]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for earlier in seldon::config::EARLIER_DEFAULT_WATCH_PATHS {
        let mut t = config();
        t["watchPaths"] = toml::Value::Array(
            earlier
                .iter()
                .rev()
                .map(|p| toml::Value::String(p.to_string()))
                .collect(),
        );
        std::fs::write(env.config_file(), toml::to_string(&t).unwrap()).unwrap();
        let d = ok(&env, &["doctor", "--json"]);
        let row = row(&d, "watch");
        assert_eq!(row["status"], "ok", "{row}");
        assert!(
            row["message"]
                .as_str()
                .unwrap()
                .contains("the next capture adds"),
            "{row}"
        );

        let c = capture_config(&env);
        let added: Vec<String> = c["watchPathsAdded"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let missing: Vec<String> = seldon::config::DEFAULT_WATCH_PATHS
            .iter()
            .filter(|p| !earlier.contains(p))
            .map(|p| p.to_string())
            .collect();
        assert_eq!(added, missing);
        assert!(new.iter().all(|p| added.contains(p)));
        let mut have = watch(&config());
        have.sort();
        let mut want: Vec<String> = seldon::config::DEFAULT_WATCH_PATHS
            .iter()
            .map(|s| s.to_string())
            .collect();
        want.sort();
        assert_eq!(have, want);
        // once
        let again = capture_config(&env);
        assert_eq!(again["watchPathsAdded"], json!([]), "{again}");
    }
    // the human output says it
    let mut t = config();
    t["watchPaths"] = toml::Value::Array(
        seldon::config::EARLIER_DEFAULT_WATCH_PATHS[0]
            .iter()
            .map(|p| toml::Value::String(p.to_string()))
            .collect(),
    );
    std::fs::write(env.config_file(), toml::to_string(&t).unwrap()).unwrap();
    let out = env.seldon(&["capture", "--source", "config"]);
    let text = common::stdout(&out);
    assert!(
        text.contains("note: config.toml now also watches ~/.local/share/applications, ~/.config/systemd/user,"),
        "{text}"
    );

    // a list of the user's own: kept, and doctor says what it lacks
    let mut t = config();
    t["watchPaths"] = toml::Value::Array(vec![
        toml::Value::String("~/.config/hypr".into()),
        toml::Value::String("~/dotfiles".into()),
    ]);
    std::fs::write(env.config_file(), toml::to_string(&t).unwrap()).unwrap();
    let c = capture_config(&env);
    assert_eq!(c["watchPathsAdded"], json!([]));
    assert_eq!(watch(&config()), ["~/.config/hypr", "~/dotfiles"]);
    let d = ok(&env, &["doctor", "--json"]);
    let row = row(&d, "watch");
    assert_eq!(row["status"], "degraded", "{row}");
    let message = row["message"].as_str().unwrap();
    assert!(
        message.contains("~/.config/systemd/user") && message.contains("~/.bash_profile"),
        "{row}"
    );
    assert!(!message.contains("~/.config/hypr,"), "{row}");
    assert!(
        row["fix"]
            .as_str()
            .unwrap()
            .starts_with("add to watchPaths in config.toml: \"~/.config/omarchy\", "),
        "{row}"
    );
}

fn row(doctor: &Value, name: &str) -> Value {
    doctor["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .cloned()
        .unwrap_or_else(|| panic!("no {name} row: {doctor}"))
}

/// ADR-0028 §4c, §6: `doctor` prints the effective `[drift]` set and marks
/// what differs from the default; an unknown routine rule is degraded.
#[test]
fn doctor_shows_the_drift_rules() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let d = ok(&env, &["doctor", "--json"]);
    let r = row(&d, "drift");
    assert_eq!(r["status"], "ok");
    assert!(
        r["message"].as_str().unwrap().ends_with("; all defaults"),
        "{r}"
    );
    assert_eq!(d["drift"]["attention"], "normal");
    assert_eq!(d["drift"]["nonDefault"], json!([]));
    assert_eq!(
        d["drift"]["routine"].as_array().unwrap().len(),
        seldon::config::ROUTINE_RULES.len()
    );
    // a default config does not write the new keys (a later engine's
    // defaults reach it)
    let text = read(&env.config_file());
    assert!(text.contains("alwaysRed"), "{text}");
    assert!(
        !text.contains("routinePaths") && !text.contains("attention"),
        "{text}"
    );

    let mut t: toml::Table = text.parse().unwrap();
    let drift = t["drift"].as_table_mut().unwrap();
    drift.insert("attention".into(), toml::Value::String("all".into()));
    drift.insert(
        "routine".into(),
        toml::Value::Array(vec![
            toml::Value::String("sysupgrade".into()),
            toml::Value::String("themes".into()),
        ]),
    );
    std::fs::write(env.config_file(), toml::to_string(&t).unwrap()).unwrap();
    let d = env.seldon(&["doctor", "--json"]);
    let d = json(&d);
    let r = row(&d, "drift");
    assert_eq!(r["status"], "degraded", "{r}");
    let message = r["message"].as_str().unwrap();
    assert!(
        message.starts_with("attention all · routine: sysupgrade, themes ·"),
        "{r}"
    );
    assert!(message.contains("non-default: attention, routine"), "{r}");
    assert!(message.ends_with("unknown routine rule(s): themes"), "{r}");
    assert!(r["fix"].as_str().unwrap().contains("theme-assets"), "{r}");
    assert_eq!(d["drift"]["nonDefault"], json!(["attention", "routine"]));
}

/// ADR-0028 §5: the class is computed at index time; nothing is written to
/// the ledger, and two builds from the same ledger and config are byte
/// identical. Reading commands (`drift`, `--all`, `show`, `status`) write
/// no ledger line either.
#[test]
fn index_builds_are_identical_and_write_no_ledger_line() {
    let env = Env::new(Snapper::Missing);
    let lb = env.tmp.path().join("logbook");
    copy_dir(&fixture_logbook(), &lb);
    let ledger = |lb: &Path| -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(lb.join("ledger"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .map(|p| (p.display().to_string(), std::fs::read(&p).unwrap()))
            .collect();
        files.sort();
        files
    };
    let before = ledger(&lb);
    let index_file = env.home.join(".local/state/seldon/index.json");
    let lbs = lb.to_str().unwrap();
    let build = || {
        let out = env.at(GENERATED_AT, &["--logbook", lbs, "index", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        std::fs::read(&index_file).unwrap()
    };
    let first = build();
    for args in [
        &["drift"][..],
        &["drift", "--all"],
        &["drift", "--crisis-only"],
        &["drift", "show", "01M3SXBQVR7AW8PJQC1YXDCQ14"],
        &["status"],
    ] {
        let mut all = vec!["--logbook", lbs];
        all.extend_from_slice(args);
        let out = env.at(GENERATED_AT, &all);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
    }
    let second = build();
    assert_eq!(first, second, "byte-identical");
    assert_eq!(ledger(&lb), before, "no ledger line written");
}
