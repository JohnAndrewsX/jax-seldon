//! The Seldon agent skill (WP-094): `seldon hook install|uninstall skills`,
//! the `skills` row of `doctor`, `init --harness skills`, and the skill's
//! text against this engine's commands and the logbook rules.
//!
//! Everything runs in a throw-away home (`common::Env`, with
//! `SELDON_TEST_GUARD`): never the real `~/.claude`, `~/.agents`,
//! `~/.codex`, `~/.pi` or `~/.hermes`.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::SystemTime;

use common::{Env, Snapper, json, read, stderr, stdout};
use seldon::sys::sha256_hex;
use serde_json::{Value, json};

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/skills/seldon");
const FILES: [&str; 5] = [
    "SKILL.md",
    "case.md",
    "drift.md",
    "snapshot.md",
    "update.md",
];
const MANIFEST: &str = ".seldon-skill.json";

fn asset(name: &str) -> String {
    read(&Path::new(ASSETS).join(name))
}

fn run(env: &Env, args: &[&str]) -> Output {
    env.command(args)
        .env("SELDON_PACMAN_LOG", env.tmp.path().join("pacman.log"))
        .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
        .env("SELDON_OMARCHY_PLUGINS_DIR", env.tmp.path().join("plugins"))
        .env("SELDON_THEME_FILE", env.tmp.path().join("theme.name"))
        .env("SELDON_HARDWARE_ROOT", common::hardware_root())
        .output()
        .unwrap()
}

/// `seldon --json args…`, exit 0, the JSON.
fn ok(env: &Env, args: &[&str]) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let out = run(env, &all);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{args:?}: {}{}",
        stdout(&out),
        stderr(&out)
    );
    json(&out)
}

fn install(env: &Env) -> Value {
    ok(env, &["hook", "install", "skills"])
}

fn uninstall(env: &Env) -> Value {
    ok(env, &["hook", "uninstall", "skills"])
}

fn mkdir(env: &Env, rel: &str) -> PathBuf {
    let p = env.home.join(rel);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Every entry under `dir` (not following links): kind, bytes or link
/// target, and mtime, so a test can prove nothing was created, written or
/// rewritten.
fn snapshot(dir: &Path) -> BTreeMap<String, (String, Vec<u8>, Option<SystemTime>)> {
    fn walk(
        root: &Path,
        dir: &Path,
        out: &mut BTreeMap<String, (String, Vec<u8>, Option<SystemTime>)>,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            let mtime = meta.modified().ok();
            if meta.file_type().is_symlink() {
                let target = std::fs::read_link(&path).unwrap();
                out.insert(
                    rel,
                    (
                        "link".into(),
                        target.to_string_lossy().into_owned().into_bytes(),
                        mtime,
                    ),
                );
            } else if meta.is_dir() {
                out.insert(rel, ("dir".into(), Vec::new(), mtime));
                walk(root, &path, out);
            } else {
                out.insert(rel, ("file".into(), std::fs::read(&path).unwrap(), mtime));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The home without `~/.local` (the engine's state directory, the lock).
fn home_snapshot(env: &Env) -> BTreeMap<String, (String, Vec<u8>, Option<SystemTime>)> {
    snapshot(&env.home)
        .into_iter()
        .filter(|(k, _)| !k.starts_with(".local"))
        .collect()
}

fn dir_report<'a>(v: &'a Value, dir: &str) -> &'a Value {
    v["dirs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["dir"] == dir)
        .unwrap_or_else(|| panic!("no report for {dir}: {v}"))
}

fn assert_installed(folder: &Path) {
    let target = folder.join("seldon");
    for name in FILES {
        assert_eq!(
            read(&target.join(name)),
            asset(name),
            "{}",
            target.display()
        );
    }
    let manifest: Value = serde_json::from_str(&read(&target.join(MANIFEST))).unwrap();
    assert_eq!(manifest["files"].as_object().unwrap().len(), FILES.len());
}

fn skills_row(env: &Env) -> Value {
    let out = run(env, &["--json", "doctor"]);
    assert!(out.status.code().is_some());
    json(&out)["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "skills")
        .cloned()
        .unwrap_or_else(|| panic!("no skills row: {}", stdout(&out)))
}

#[test]
fn installs_only_into_skill_folders_that_exist_and_creates_none() {
    let env = Env::new(Snapper::Missing);
    let claude = mkdir(&env, ".claude/skills");
    let pi = mkdir(&env, ".pi/agent/skills");
    // an agent home without a skills folder gets none
    mkdir(&env, ".codex");
    let profile = mkdir(&env, ".hermes/profiles/work/skills");
    mkdir(&env, ".hermes/profiles/empty");

    let v = install(&env);
    assert_eq!(v["skill"], "seldon");
    assert_eq!(dir_report(&v, "~/.claude/skills")["action"], "installed");
    assert_eq!(dir_report(&v, "~/.pi/agent/skills")["action"], "installed");
    assert_eq!(
        dir_report(&v, "~/.hermes/profiles/work/skills")["action"],
        "installed"
    );
    assert_eq!(v["dirs"].as_array().unwrap().len(), 3, "{v}");
    assert_eq!(
        v["absent"],
        json!(["~/.agents/skills", "~/.codex/skills", "~/.hermes/skills"])
    );
    let written = dir_report(&v, "~/.claude/skills")["written"].clone();
    assert_eq!(written, json!(FILES));
    for folder in [&claude, &pi, &profile] {
        assert_installed(folder);
    }
    for never in [
        ".agents",
        ".codex/skills",
        ".hermes/skills",
        ".hermes/profiles/empty/skills",
    ] {
        assert!(!env.home.join(never).exists(), "{never} was created");
    }
    // the human output names what is not there
    let out = run(&env, &["hook", "install", "skills"]);
    let text = stdout(&out);
    assert!(
        text.contains("unchanged  ~/.claude/skills/seldon"),
        "{text}"
    );
    assert!(
        text.contains("Not there, not created: ~/.agents/skills, ~/.codex/skills"),
        "{text}"
    );
}

#[test]
fn no_skill_folder_installs_nothing_and_says_so() {
    let env = Env::new(Snapper::Missing);
    let before = home_snapshot(&env);
    let v = install(&env);
    assert_eq!(v["dirs"], json!([]));
    assert_eq!(v["absent"].as_array().unwrap().len(), 5);
    assert_eq!(home_snapshot(&env), before);
    let out = run(&env, &["hook", "install", "skills"]);
    assert!(
        stdout(&out).starts_with("No agent skill folder exists ("),
        "{}",
        stdout(&out)
    );
    let row = skills_row(&env);
    assert_eq!(row["status"], "ok", "{row}");
    assert!(row.get("fix").is_none(), "{row}");
    let v = uninstall(&env);
    assert_eq!(v["dirs"], json!([]));
}

#[test]
fn a_second_install_and_a_second_uninstall_change_nothing() {
    let env = Env::new(Snapper::Missing);
    let claude = mkdir(&env, ".claude/skills");
    mkdir(&env, ".agents/skills");
    install(&env);
    let after_first = home_snapshot(&env);
    let v = install(&env);
    for d in v["dirs"].as_array().unwrap() {
        assert_eq!(d["state"], "current", "{v}");
        assert_eq!(d["action"], "unchanged", "{v}");
        assert_eq!(d["written"], json!([]), "{v}");
    }
    assert_eq!(v["ownWrites"], json!([]));
    assert_eq!(home_snapshot(&env), after_first, "a second install wrote");

    let v = uninstall(&env);
    assert_eq!(dir_report(&v, "~/.claude/skills")["action"], "removed");
    let mut removed = FILES.to_vec();
    removed.sort();
    let mut got: Vec<String> = dir_report(&v, "~/.claude/skills")["removed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    got.sort();
    assert_eq!(got, removed);
    assert!(!claude.join("seldon").exists());
    // the agent's folder stays
    assert!(claude.is_dir());
    let after_uninstall = home_snapshot(&env);
    let v = uninstall(&env);
    assert_eq!(dir_report(&v, "~/.claude/skills")["action"], "absent");
    assert_eq!(home_snapshot(&env), after_uninstall);
    let out = run(&env, &["hook", "uninstall", "skills"]);
    assert_eq!(
        stdout(&out).trim(),
        "The Seldon agent skill is not installed; nothing changed."
    );
}

#[test]
fn a_foreign_seldon_folder_link_or_file_is_never_touched() {
    let env = Env::new(Snapper::Missing);
    // a folder of someone else's
    let agents = mkdir(&env, ".agents/skills");
    std::fs::create_dir(agents.join("seldon")).unwrap();
    std::fs::write(agents.join("seldon/SKILL.md"), "mine\n").unwrap();
    // a symbolic link to a folder elsewhere, which holds a manifest
    let claude = mkdir(&env, ".claude/skills");
    let elsewhere = env.tmp.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::write(elsewhere.join(MANIFEST), "{\"files\":{}}").unwrap();
    std::os::unix::fs::symlink(&elsewhere, claude.join("seldon")).unwrap();
    // a plain file
    let codex = mkdir(&env, ".codex/skills");
    std::fs::write(codex.join("seldon"), "a file\n").unwrap();
    let before = (home_snapshot(&env), snapshot(&elsewhere));

    let v = install(&env);
    for dir in ["~/.agents/skills", "~/.claude/skills", "~/.codex/skills"] {
        let d = dir_report(&v, dir);
        assert_eq!(
            (d["state"].as_str(), d["action"].as_str()),
            (Some("foreign"), Some("kept")),
            "{v}"
        );
    }
    assert_eq!((home_snapshot(&env), snapshot(&elsewhere)), before);
    let out = run(&env, &["hook", "install", "skills"]);
    assert!(
        stdout(&out)
            .contains("kept       ~/.agents/skills/seldon was not written by seldon; left alone"),
        "{}",
        stdout(&out)
    );

    let v = uninstall(&env);
    for d in v["dirs"].as_array().unwrap() {
        assert_eq!(d["action"], "kept", "{v}");
    }
    assert_eq!((home_snapshot(&env), snapshot(&elsewhere)), before);

    let row = skills_row(&env);
    assert_eq!(row["status"], "degraded", "{row}");
    assert!(
        row["message"]
            .as_str()
            .unwrap()
            .contains("another skill named seldon in"),
        "{row}"
    );
    assert!(
        row["fix"]
            .as_str()
            .unwrap()
            .contains("move the folder named seldon away")
    );
}

#[test]
fn a_file_changed_by_hand_is_kept_by_install_and_uninstall() {
    let env = Env::new(Snapper::Missing);
    let claude = mkdir(&env, ".claude/skills");
    install(&env);
    let case = claude.join("seldon/case.md");
    std::fs::write(&case, "my own notes\n").unwrap();
    let before = home_snapshot(&env);

    let v = install(&env);
    let d = dir_report(&v, "~/.claude/skills");
    assert_eq!(d["state"], "changed");
    assert_eq!(d["action"], "kept");
    assert_eq!(d["kept"], json!(["case.md"]));
    assert_eq!(
        home_snapshot(&env),
        before,
        "install rewrote a changed folder"
    );
    let row = skills_row(&env);
    assert_eq!(row["status"], "degraded", "{row}");
    assert!(
        row["message"]
            .as_str()
            .unwrap()
            .contains("changed by hand in ~/.claude/skills"),
        "{row}"
    );

    let v = uninstall(&env);
    let d = dir_report(&v, "~/.claude/skills");
    assert_eq!(d["action"], "kept");
    assert_eq!(d["kept"], json!(["case.md"]));
    assert_eq!(read(&case), "my own notes\n");
    // the manifest stays with it: the folder is still Seldon's
    assert!(claude.join("seldon").join(MANIFEST).is_file());
    for name in FILES.iter().filter(|n| **n != "case.md") {
        assert!(!claude.join("seldon").join(name).exists(), "{name}");
    }
    // once the user's file is gone, uninstall takes the rest
    std::fs::remove_file(&case).unwrap();
    let v = uninstall(&env);
    assert_eq!(dir_report(&v, "~/.claude/skills")["action"], "removed");
    assert!(!claude.join("seldon").exists());
}

#[test]
fn an_older_skill_is_updated_and_a_dropped_file_removed() {
    let env = Env::new(Snapper::Missing);
    let claude = mkdir(&env, ".claude/skills");
    install(&env);
    let target = claude.join("seldon");
    // what an older engine wrote: another case.md, and a file this one no
    // longer ships; the manifest names both as written
    let old_case = "# Cases (old)\n";
    let old_extra = "# Gone\n";
    std::fs::write(target.join("case.md"), old_case).unwrap();
    std::fs::write(target.join("old.md"), old_extra).unwrap();
    let mut manifest: Value = serde_json::from_str(&read(&target.join(MANIFEST))).unwrap();
    manifest["files"]["case.md"] = json!(sha256_hex(old_case.as_bytes()));
    manifest["files"]["old.md"] = json!(sha256_hex(old_extra.as_bytes()));
    // a manifest name that leaves the folder is never followed
    let outside = env.home.join(".claude/escape.md");
    std::fs::write(&outside, old_extra).unwrap();
    manifest["files"]["../escape.md"] = json!(sha256_hex(old_extra.as_bytes()));
    std::fs::write(target.join(MANIFEST), manifest.to_string()).unwrap();

    let row = skills_row(&env);
    assert_eq!(row["status"], "degraded", "{row}");
    assert_eq!(row["fix"], "seldon hook install skills");
    assert!(
        row["message"]
            .as_str()
            .unwrap()
            .contains("outdated in ~/.claude/skills")
    );

    let v = install(&env);
    let d = dir_report(&v, "~/.claude/skills");
    assert_eq!(d["state"], "outdated");
    assert_eq!(d["action"], "updated");
    assert_eq!(d["written"], json!(["case.md"]));
    assert_eq!(d["removed"], json!(["old.md"]));
    assert_installed(&claude);
    assert!(!target.join("old.md").exists());
    assert_eq!(
        read(&outside),
        old_extra,
        "a file outside the folder was touched"
    );
    let manifest: Value = serde_json::from_str(&read(&target.join(MANIFEST))).unwrap();
    assert!(manifest["files"].get("old.md").is_none(), "{manifest}");
    assert_eq!(skills_row(&env)["status"], "ok");
}

#[test]
fn an_interrupted_install_is_completed_by_the_next() {
    let env = Env::new(Snapper::Missing);
    let claude = mkdir(&env, ".claude/skills");
    install(&env);
    std::fs::remove_file(claude.join("seldon/drift.md")).unwrap();
    let skill_mtime = std::fs::metadata(claude.join("seldon/SKILL.md"))
        .unwrap()
        .modified()
        .unwrap();
    let v = install(&env);
    let d = dir_report(&v, "~/.claude/skills");
    assert_eq!(
        (d["state"].as_str(), d["action"].as_str()),
        (Some("outdated"), Some("updated"))
    );
    assert_eq!(d["written"], json!(["drift.md"]));
    assert_installed(&claude);
    assert_eq!(
        std::fs::metadata(claude.join("seldon/SKILL.md"))
            .unwrap()
            .modified()
            .unwrap(),
        skill_mtime,
        "an unchanged file was rewritten"
    );
}

#[test]
fn an_empty_seldon_folder_is_used_and_files_as_shipped_are_not_rewritten() {
    // what an install that stopped right after creating the folder leaves
    let env = Env::new(Snapper::Missing);
    let claude = mkdir(&env, ".claude/skills");
    std::fs::create_dir(claude.join("seldon")).unwrap();
    assert_eq!(
        skills_row(&env)["message"],
        "seldon agent skill: not installed in ~/.claude/skills"
    );
    let v = install(&env);
    assert_eq!(dir_report(&v, "~/.claude/skills")["action"], "installed");
    assert_installed(&claude);
    // an update that stopped after the files, before the manifest: the
    // files are as shipped, the manifest is older; only the manifest is
    // written, nothing counts as changed by hand
    let target = claude.join("seldon");
    let mut manifest: Value = serde_json::from_str(&read(&target.join(MANIFEST))).unwrap();
    manifest["files"]["case.md"] = json!(sha256_hex(b"# Cases (old)\n"));
    std::fs::write(target.join(MANIFEST), manifest.to_string()).unwrap();
    let v = install(&env);
    let d = dir_report(&v, "~/.claude/skills");
    assert_eq!(
        (d["state"].as_str(), d["action"].as_str()),
        (Some("outdated"), Some("updated"))
    );
    assert_eq!(d["written"], json!([]));
    assert_installed(&claude);
    assert_eq!(skills_row(&env)["status"], "ok");
}

#[test]
fn uninstall_leaves_a_file_the_user_added() {
    let env = Env::new(Snapper::Missing);
    let claude = mkdir(&env, ".claude/skills");
    install(&env);
    std::fs::write(claude.join("seldon/notes.txt"), "mine\n").unwrap();
    let out = run(&env, &["hook", "uninstall", "skills"]);
    assert!(
        stdout(&out)
            .contains("kept       ~/.claude/skills/seldon: holds files that are not seldon's"),
        "{}",
        stdout(&out)
    );
    assert_eq!(read(&claude.join("seldon/notes.txt")), "mine\n");
    for name in FILES.iter().chain(&[MANIFEST]) {
        assert!(!claude.join("seldon").join(name).exists(), "{name}");
    }
    // what is left is the user's folder now
    let v = uninstall(&env);
    assert_eq!(dir_report(&v, "~/.claude/skills")["state"], "foreign");
}

#[test]
fn doctor_says_installed_or_missing_and_the_fix() {
    let env = Env::new(Snapper::Missing);
    mkdir(&env, ".claude/skills");
    mkdir(&env, ".agents/skills");
    let row = skills_row(&env);
    assert_eq!(row["status"], "ok", "{row}");
    assert_eq!(
        row["message"],
        "seldon agent skill: not installed in ~/.agents/skills, ~/.claude/skills"
    );
    assert_eq!(row["fix"], "seldon hook install skills");
    install(&env);
    let row = skills_row(&env);
    assert_eq!(row["status"], "ok", "{row}");
    assert_eq!(
        row["message"],
        "seldon agent skill: installed in ~/.agents/skills, ~/.claude/skills"
    );
    assert!(row.get("fix").is_none(), "{row}");
    // the rules-only doctor the panel calls has no skills row
    let root = env.init_logbook();
    let out = run(&env, &["--json", "doctor", "--only", "rules"]);
    let names: Vec<String> = json(&out)["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, ["engine", "rules"], "{}", root.display());
}

#[test]
fn settings_is_refused_for_the_skill() {
    let env = Env::new(Snapper::Missing);
    let claude = mkdir(&env, ".claude/skills");
    for verb in ["install", "uninstall"] {
        let out = run(&env, &["hook", verb, "skills", "--settings", "/tmp/x.json"]);
        assert_eq!(out.status.code(), Some(1), "{verb}");
        assert!(
            stderr(&out).contains("--settings is for claude-code"),
            "{}",
            stderr(&out)
        );
    }
    assert!(!claude.join("seldon").exists());
}

#[test]
fn own_writes_under_a_watched_path_leave_no_drift() {
    let env = Env::new(Snapper::NoPermissions);
    std::fs::write(env.tmp.path().join("pacman.log"), "").unwrap();
    let config = env.config_file();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        "watchPaths = [\"~/.config/omarchy\", \"~/.claude/skills\"]\n",
    )
    .unwrap();
    mkdir(&env, ".claude/skills");
    mkdir(&env, ".agents/skills");
    let root = env.tmp.path().join("logbook");
    ok(
        &env,
        &[
            "init",
            "--non-interactive",
            "--no-git",
            "--path",
            root.to_str().unwrap(),
        ],
    );

    let v = install(&env);
    let mut own: Vec<String> = v["ownWrites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    own.sort();
    let mut want: Vec<String> = FILES
        .iter()
        .chain(&[MANIFEST])
        .map(|n| format!("~/.claude/skills/seldon/{n}"))
        .collect();
    want.sort();
    // only the watched folder is recorded
    assert_eq!(own, want, "{v}");
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["explainedOwn"], 6, "{c}");
    let d = ok(&env, &["drift"]);
    assert_eq!(d["openDrift"], 0, "{d}");

    let v = uninstall(&env);
    assert_eq!(v["ownWrites"].as_array().unwrap().len(), 6, "{v}");
    let c = ok(&env, &["capture", "--all"]);
    assert_eq!(c["explainedOwn"], 6, "{c}");
    let d = ok(&env, &["drift"]);
    assert_eq!(d["openDrift"], 0, "{d}");
}

#[test]
fn init_offers_the_skill_as_a_harness() {
    let env = Env::new(Snapper::Missing);
    let claude = mkdir(&env, ".claude/skills");
    let root = env.tmp.path().join("logbook");
    let v = ok(
        &env,
        &[
            "init",
            "--non-interactive",
            "--no-git",
            "--no-capture",
            "--path",
            root.to_str().unwrap(),
            "--harness",
            "skills",
        ],
    );
    assert_eq!(v["harnesses"], json!(["skills"]));
    let setup = &v["harnessSetup"]["skills"];
    assert_eq!(dir_report(setup, "~/.claude/skills")["action"], "installed");
    assert_installed(&claude);
    assert!(
        !v["nextSteps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "seldon hook install skills"),
        "{v}"
    );
    assert!(read(&env.config_file()).contains("harnesses = [\"skills\"]"));
    // a changed folder leaves the step to the user
    std::fs::write(claude.join("seldon/SKILL.md"), "mine\n").unwrap();
    let root2 = env.tmp.path().join("logbook2");
    let v = ok(
        &env,
        &[
            "init",
            "--non-interactive",
            "--no-git",
            "--no-capture",
            "--path",
            root2.to_str().unwrap(),
            "--harness",
            "skills",
        ],
    );
    assert!(
        v["nextSteps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "seldon hook install skills"),
        "{v}"
    );
}

// ---------------------------------------------------------------------------
// The text
// ---------------------------------------------------------------------------

/// Every `seldon …` command line in `text`: inline code spans, and lines
/// (or `|` segments of lines) of fenced code blocks.
fn seldon_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fenced = false;
    let mut prose = String::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            for seg in line.split('|') {
                let seg = seg.trim().trim_end_matches('\\').trim();
                if seg.starts_with("seldon ") {
                    out.push(seg.to_string());
                }
            }
        } else {
            prose.push_str(line);
            prose.push('\n');
        }
    }
    for span in prose.split('`').skip(1).step_by(2) {
        let span = span.replace('\n', " ");
        if span.starts_with("seldon ") {
            out.push(span);
        }
    }
    out
}

#[test]
fn every_command_in_the_skill_is_one_this_engine_has() {
    // as for AGENTS.md (WP-100): a skill naming a missing command or option
    // fails here
    let env = Env::new(Snapper::Missing);
    let mut checked = 0;
    for name in FILES {
        for line in seldon_lines(&asset(name)) {
            let rest = &line["seldon ".len()..];
            let mut argv: Vec<&str> = rest
                .split_whitespace()
                .take_while(|w| {
                    !w.starts_with('-') && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })
                .collect();
            argv.push("--help");
            let out = env.seldon(&argv);
            assert_eq!(out.status.code(), Some(0), "{name}: `{line}`");
            let help = stdout(&out);
            for flag in rest.split_whitespace().filter(|w| w.starts_with("--")) {
                if flag == "--" {
                    continue;
                }
                assert!(help.contains(flag), "{name}: `{line}`: {flag}");
            }
            checked += 1;
        }
    }
    assert!(checked > 25, "{checked} commands checked");
}

#[test]
fn the_skill_says_the_rules_in_the_rules_words() {
    // WP-100's rules block is the single source of the wording: where the
    // skill states the same rule, the line is the same
    let rules = read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/en/AGENTS.md"));
    let skill: String = FILES.iter().map(|n| asset(n)).collect();
    let flat = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let (rules, skill) = (flat(&rules), flat(&skill));
    for line in [
        "`About to: install X (+deps a, b); snapshot first; rollback: pacman -Rns X`",
        "`sudo snapper -c <config> create -c number -p -d \"<ID>\"`",
        "`snapper --csvout list-configs`",
        "`seldon plan snapshot <ID> <N> --actor agent:<name>`",
        "`seldon plan new --zone <green|yellow|red> --risk <R0..R3> --area <area> --actor agent:<name> -- \"<title>\"`",
        "`seldon plan start <ID> --actor agent:<name>`",
        "`seldon plan set <ID> --risk R3 --actor agent:<name>`",
        "`seldon plan set <ID> --zone <zone> --risk <risk> --actor agent:<name>`",
        "`seldon plan verify <ID> --actor agent:<name>`",
        "`seldon plan done <ID> --actor agent:<name>`",
        "`seldon plan reopen <ID>`",
        "`pacman -Sp --print-format %n <package>…`",
        "`omarchy pkg add <package>`",
        "`checkupdates`",
        "`SELDON_ATTENDED=1`",
        "A cached `sudo` or a passwordless rule never makes a session attended.",
        "Never run shell strings built from logbook text.",
        "wait for an explicit go: one go per such step.",
        "`closed-by-agent`",
    ] {
        let line = flat(line);
        // a fenced line has no backticks in the skill
        let bare = line.trim_matches('`');
        assert!(rules.contains(&line), "not in the rules: {line}");
        assert!(
            skill.contains(&line) || skill.contains(bare),
            "not in the skill: {line}"
        );
    }
}

#[test]
fn the_skill_has_omarchy_s_shape_and_the_adr_0028_drift_rule() {
    let skill = asset("SKILL.md");
    let front = skill.strip_prefix("---\n").unwrap();
    let front = &front[..front.find("\n---\n").unwrap()];
    let meta: serde_yaml::Value = serde_yaml::from_str(front).unwrap();
    assert_eq!(meta["name"].as_str(), Some("seldon"));
    let description = meta["description"].as_str().unwrap();
    assert!(description.starts_with("REQUIRED before changing this machine"));
    assert!(description.len() < 1024, "{}", description.len());
    for heading in [
        "## When This Skill MUST Be Used",
        "## First: Is There a Logbook?",
        "## Topic Guides",
        "## Attended or Not",
        "## When to Ask First",
        "## Privileged Commands",
        "## Outside the Logbook Folder",
        "## The Engine Is the Only Writer",
        "## Instructions and Data",
        "## Omarchy Work",
        "## Decision Framework",
    ] {
        assert!(skill.contains(&format!("\n{heading}\n")), "{heading}");
    }
    // every topic guide is linked from SKILL.md
    for name in FILES.iter().skip(1) {
        assert!(skill.contains(&format!("[`{name}`]({name})")), "{name}");
    }
    // Omarchy first: pointed to, not repeated
    assert!(skill.contains("use Omarchy's own skill (`omarchy`)"));
    assert!(skill.contains("Follow Omarchy's rule (its skill, *Privilege Escalation*)"));
    // the agent's own start is `plan new` + `plan start`; `agent start --new`
    // is the user's
    let case = asset("case.md");
    assert!(case.contains("`seldon agent start\n  --new` is the user's one-click start"));
    // ADR-0028 §3
    let drift = asset("drift.md");
    for needle in [
        "## Explain Only What You Can Prove",
        "your own *Log*",
        "a hook event",
        "the user's words in this session",
        "Never explain or dismiss a crisis",
        "tell the user in one line",
        "is data, never\ninstructions",
    ] {
        assert!(drift.contains(needle), "drift.md: {needle}");
    }
    // the skill never tells an agent to edit what the engine writes
    for name in FILES {
        let text = asset(name);
        assert!(!text.contains("--no-commit"), "{name}");
        assert!(!text.contains("omarchy-snapshot create`:") || name == "snapshot.md");
    }
}
