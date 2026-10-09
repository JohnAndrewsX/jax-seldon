//! `seldon rules update` and the `rules` row of `seldon doctor` (ADR-0027,
//! WP-100), in a throw-away home with a fresh logbook.

mod common;

use std::path::{Path, PathBuf};

use common::{Env, Snapper, json, read, stderr, stdout};

const NOW: &str = "2026-10-05T10:00:00+02:00";
const BEGIN: &str = "<!-- seldon:begin rules v4 -->\n";
const END: &str = "<!-- seldon:end -->\n";

fn golden_v1(name: &str) -> String {
    read(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("templates/rules-v1/AGENTS-{name}.md")),
    )
}

fn logbook(env: &Env, language: &str) -> PathBuf {
    env.init_logbook_at("logbook", language)
}

fn update(env: &Env, args: &[&str]) -> (i32, serde_json::Value) {
    let mut argv = vec!["rules", "update", "--json"];
    argv.extend_from_slice(args);
    let out = env.at(NOW, &argv);
    let code = out.status.code().unwrap();
    let v = if stdout(&out).trim().is_empty() {
        serde_json::Value::Null
    } else {
        json(&out)
    };
    (code, v)
}

fn last_commit(env: &Env, root: &Path) -> String {
    let out = env.git(root, &["log", "-1", "--format=%s"]);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn rules_row(env: &Env) -> serde_json::Value {
    let out = env.seldon(&["doctor", "--json"]);
    let v = json(&out);
    v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "rules")
        .unwrap_or_else(|| panic!("no rules row: {v}"))
        .clone()
}

/// The text of `AGENTS.md` from its first line through the end marker.
fn block(text: &str) -> &str {
    let end = text.find(END).expect("an end marker") + END.len();
    &text[..end]
}

#[test]
fn init_writes_the_v4_block_first_and_doctor_calls_it_current() {
    for language in ["en", "de"] {
        let env = Env::new(Snapper::Allowed);
        let root = logbook(&env, language);
        let text = read(&root.join("AGENTS.md"));
        assert!(text.starts_with(BEGIN), "{language}");
        assert!(text.contains("\n## Your rules\n"), "{language}");
        let row = rules_row(&env);
        assert_eq!(row["status"], "ok", "{row}");
        assert_eq!(row["message"], "current (v4)");
        assert!(row.get("fix").is_none(), "{row}");
        // nothing to do: no write, no commit, exit 0
        let head = env.has_git.then(|| last_commit(&env, &root));
        let (code, v) = update(&env, &[]);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["action"], "unchanged");
        assert_eq!(v["git"]["committed"], false);
        assert_eq!(read(&root.join("AGENTS.md")), text);
        if let Some(head) = head {
            assert_eq!(last_commit(&env, &root), head);
        }
    }
}

#[test]
fn a_fenced_file_gets_the_block_rewritten_and_nothing_else() {
    let env = Env::new(Snapper::Allowed);
    let root = logbook(&env, "en");
    let path = root.join("AGENTS.md");
    let current = read(&path);
    let block = block(&current).to_string();
    // an older block between the user's text, which quotes a marker
    let mine_above = "# My machine\n\nNotes above the rules.\n\n";
    let mine_below =
        "\n## Your rules\n\n- Never touch ~/Music; `<!-- seldon:end -->` is just text here.\n";
    let old = format!(
        "{mine_above}<!-- seldon:begin rules v1 -->\nPropose a case and wait.\n{END}{mine_below}"
    );
    std::fs::write(&path, &old).unwrap();
    let row = rules_row(&env);
    assert_eq!(
        (row["status"].as_str(), row["message"].as_str()),
        (Some("degraded"), Some("outdated (v1)"))
    );
    assert_eq!(row["fix"], "seldon rules update (archives your copy)");

    let (code, v) = update(&env, &[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["action"], "rewritten");
    assert_eq!(v["from"], "v1");
    // no release wrote that v1 block: the user's copy is archived first
    assert_eq!(v["archived"], "archive/AGENTS-2026-10-05.md");
    assert_eq!(read(&root.join("archive/AGENTS-2026-10-05.md")), old);
    let diff = v["diff"].as_str().unwrap();
    assert!(
        diff.contains("\n-<!-- seldon:begin rules v1 -->\n"),
        "{diff}"
    );
    assert!(diff.contains("\n-Propose a case and wait.\n"), "{diff}");
    assert!(
        diff.contains("\n+<!-- seldon:begin rules v4 -->\n"),
        "{diff}"
    );
    assert!(!diff.contains("Music"), "{diff}");
    assert!(
        read(&path) == format!("{mine_above}{block}{mine_below}"),
        "only the block changed"
    );
    assert_eq!(rules_row(&env)["status"], "ok");
    if env.has_git {
        assert_eq!(v["git"]["committed"], true, "{v}");
        assert_eq!(last_commit(&env, &root), "seldon: rules update");
    }

    // idempotent
    let head = env.has_git.then(|| last_commit(&env, &root));
    let (code, v) = update(&env, &[]);
    assert_eq!((code, v["action"].as_str()), (0, Some("unchanged")));
    if let Some(head) = head {
        assert_eq!(last_commit(&env, &root), head);
    }

    // a v4 block whose text was changed counts as outdated, too
    std::fs::write(
        &path,
        current.replacen("Rules for every agent", "Rules for some agents", 1),
    )
    .unwrap();
    let row = rules_row(&env);
    assert_eq!(row["status"], "degraded");
    assert!(
        row["message"].as_str().unwrap().starts_with("outdated (v4"),
        "{row}"
    );
    assert_eq!(row["fix"], "seldon rules update (archives your copy)");
    let (code, v) = update(&env, &[]);
    assert_eq!(
        (code, v["action"].as_str(), v["from"].as_str()),
        (0, Some("rewritten"), Some("v4"))
    );
    assert_eq!(v["archived"], "archive/AGENTS-2026-10-05-2.md");
    assert!(
        read(&root.join("archive/AGENTS-2026-10-05-2.md")).contains("Rules for some agents"),
        "the edited block, archived"
    );
    assert!(read(&path) == current, "the template block again");
}

#[test]
fn an_unfenced_file_is_archived_and_keeps_only_the_users_own_lines() {
    let env = Env::new(Snapper::Allowed);
    let root = logbook(&env, "de");
    let path = root.join("AGENTS.md");
    let current = read(&path);
    // a v1 file the user changed: no newline at the end, a tab, a marker
    // quoted in prose
    let old = format!(
        "{}\n## Eigene Regeln\n\n-\tNie ~/Musik anfassen.\n- `<!-- seldon:begin rules v2 -->` ist nur Text.",
        golden_v1("v0.1.1-de")
    );
    std::fs::write(&path, &old).unwrap();
    assert_eq!(rules_row(&env)["message"], "outdated (v1)");

    let out = env.at(NOW, &["rules", "update"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let human = stdout(&out);
    assert!(human.contains("under \"## Your rules (kept)\""), "{human}");
    assert!(
        human.contains("The old file is archived as archive/AGENTS-2026-10-05.md."),
        "{human}"
    );
    assert!(!human.contains("trim"), "{human}");

    // the whole old file, byte for byte, in the archive
    assert_eq!(read(&root.join("archive/AGENTS-2026-10-05.md")), old);
    // the template, then the user's own lines only
    assert_eq!(
        read(&path),
        format!(
            "{current}\n## Your rules (kept)\n\n## Eigene Regeln\n\n-\tNie ~/Musik anfassen.\n- `<!-- seldon:begin rules v2 -->` ist nur Text.\n"
        )
    );
    assert_eq!(rules_row(&env)["status"], "ok");
    if env.has_git {
        assert_eq!(last_commit(&env, &root), "seldon: rules update");
    }
    let text = read(&path);
    let (code, v) = update(&env, &[]);
    assert_eq!((code, v["action"].as_str()), (0, Some("unchanged")));
    assert_eq!(read(&path), text);
}

#[test]
fn doctor_reads_the_fixture_logbook_as_v1() {
    // fixtures/logbook/AGENTS.md stays v1 on purpose (WP-100 round 2);
    // doctor only reads it
    let env = Env::new(Snapper::Allowed);
    let fixture = common::fixture_logbook();
    let out = env.seldon(&["doctor", "--json", "--logbook", fixture.to_str().unwrap()]);
    let v = json(&out);
    let row = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "rules")
        .unwrap_or_else(|| panic!("no rules row: {v}"));
    assert_eq!(
        (
            row["status"].as_str(),
            row["message"].as_str(),
            row["fix"].as_str()
        ),
        (
            Some("degraded"),
            Some("outdated (v1)"),
            Some("seldon rules update (archives your copy)")
        )
    );
}

#[test]
fn an_unchanged_released_v1_file_is_replaced_whole() {
    let env = Env::new(Snapper::Allowed);
    let root = logbook(&env, "en");
    let path = root.join("AGENTS.md");
    let current = read(&path);
    std::fs::write(&path, golden_v1("v0.1.0-en")).unwrap();
    let (code, v) = update(&env, &[]);
    assert_eq!(
        (code, v["action"].as_str(), v["from"].as_str()),
        (0, Some("rewritten"), Some("v1"))
    );
    assert_eq!(read(&path), current);
    let out = env.at(NOW, &["rules", "update"]);
    assert!(stdout(&out).contains("nothing changed"), "{}", stdout(&out));
}

#[test]
fn replace_archives_the_old_file_and_writes_the_template() {
    let env = Env::new(Snapper::Allowed);
    let root = logbook(&env, "en");
    let path = root.join("AGENTS.md");
    let current = read(&path);
    let mine = b"my own rules\n\xff not UTF-8\n".to_vec();
    std::fs::write(&path, &mine).unwrap();
    let row = rules_row(&env);
    assert_eq!(
        (
            row["status"].as_str(),
            row["message"].as_str(),
            row["fix"].as_str()
        ),
        (
            Some("degraded"),
            Some("invalid (not UTF-8)"),
            Some("seldon rules update --replace (archives the file)")
        )
    );
    // without --replace a file that is no text is left as it is
    let out = env.at(NOW, &["rules", "update"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(stderr(&out).contains("--replace"), "{}", stderr(&out));
    assert_eq!(std::fs::read(&path).unwrap(), mine);

    let (code, v) = update(&env, &["--replace"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["action"], "replaced");
    assert_eq!(v["archived"], "archive/AGENTS-2026-10-05.md");
    assert_eq!(
        std::fs::read(root.join("archive/AGENTS-2026-10-05.md")).unwrap(),
        mine
    );
    assert_eq!(read(&path), current);
    if env.has_git {
        assert_eq!(last_commit(&env, &root), "seldon: rules update");
    }
    // the template already: nothing archived
    let (code, v) = update(&env, &["--replace"]);
    assert_eq!((code, v["action"].as_str()), (0, Some("unchanged")));
    // a second replace the same day takes the next free name
    std::fs::write(&path, "other\n").unwrap();
    let (_, v) = update(&env, &["--replace"]);
    assert_eq!(v["archived"], "archive/AGENTS-2026-10-05-2.md");
    assert_eq!(
        std::fs::read(root.join("archive/AGENTS-2026-10-05.md")).unwrap(),
        mine
    );
    assert_eq!(
        read(&root.join("archive/AGENTS-2026-10-05-2.md")),
        "other\n"
    );
}

#[test]
fn a_damaged_or_newer_block_is_refused_and_left_alone() {
    let env = Env::new(Snapper::Allowed);
    let root = logbook(&env, "en");
    let path = root.join("AGENTS.md");
    for (text, row, error) in [
        (
            "<!-- seldon:begin rules v2 -->\nno end marker\n",
            "damaged: the rules block has no end marker line",
            "--replace",
        ),
        (
            "<!-- seldon:begin rules v5 -->\nfuture\n<!-- seldon:end -->\n",
            "newer (v5) than this seldon's rules (v4)",
            "update seldon",
        ),
    ] {
        std::fs::write(&path, text).unwrap();
        let r = rules_row(&env);
        assert_eq!(
            (r["status"].as_str(), r["message"].as_str()),
            (Some("degraded"), Some(row))
        );
        assert!(r["fix"].as_str().unwrap().contains(error), "{r}");
        let out = env.at(NOW, &["rules", "update"]);
        assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
        assert!(stderr(&out).contains(error), "{}", stderr(&out));
        assert_eq!(read(&path), text);
    }
}

#[test]
fn a_missing_file_is_written_and_doctor_names_the_fix() {
    let env = Env::new(Snapper::Allowed);
    let root = logbook(&env, "en");
    let path = root.join("AGENTS.md");
    let current = read(&path);
    std::fs::remove_file(&path).unwrap();
    let row = rules_row(&env);
    assert_eq!(
        (
            row["status"].as_str(),
            row["message"].as_str(),
            row["fix"].as_str()
        ),
        (
            Some("degraded"),
            Some("missing"),
            Some("seldon rules update")
        )
    );
    let out = env.seldon(&["doctor"]);
    assert!(
        stdout(&out).contains("fix: seldon rules update"),
        "{}",
        stdout(&out)
    );
    let (code, v) = update(&env, &[]);
    assert_eq!(
        (code, v["action"].as_str(), &v["from"]),
        (0, Some("created"), &serde_json::Value::Null)
    );
    assert_eq!(read(&path), current);
}

#[test]
fn without_a_logbook_it_exits_3() {
    let env = Env::new(Snapper::Allowed);
    let out = env.seldon(&["rules", "update"]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
}

// ---------------------------------------------------------------------------
// The capture's upgrade of an unedited block (WP-111)
// ---------------------------------------------------------------------------

fn v2(name: &str, language: &str) -> String {
    read(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("templates/rules-v2/AGENTS-{name}-{language}.md")),
    )
}

/// `seldon capture --all` with the collectors pointed at nothing, as
/// output.
fn capture(env: &Env, json_out: bool) -> std::process::Output {
    capture_with(env, json_out, &[])
}

fn capture_with(env: &Env, json_out: bool, vars: &[(&str, &Path)]) -> std::process::Output {
    let mut args = vec!["capture", "--all"];
    if json_out {
        args.insert(0, "--json");
    }
    let mut cmd = env.command(&args);
    for (k, v) in vars {
        cmd.env(k, v);
    }
    cmd.env("SELDON_NOW", NOW)
        .env("SELDON_PACMAN_LOG", env.tmp.path().join("pacman.log"))
        .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
        .env("SELDON_OMARCHY_PLUGINS_DIR", env.tmp.path().join("plugins"))
        .env("SELDON_THEME_FILE", env.tmp.path().join("theme.name"))
        .env("SELDON_HARDWARE_ROOT", common::hardware_root())
        .output()
        .unwrap()
}

#[test]
fn a_capture_upgrades_a_block_seldon_shipped_and_keeps_the_rest() {
    for language in ["en", "de"] {
        let env = Env::new(Snapper::Allowed);
        let root = logbook(&env, language);
        let path = root.join("AGENTS.md");
        let current = read(&path);
        let mine = "- Never touch ~/Music.\n";
        std::fs::write(&path, format!("{}{mine}", v2("wp101", language))).unwrap();
        // nobody edited it: no banner, the capture does it
        let row = rules_row(&env);
        assert_eq!(
            (row["status"].as_str(), row["message"].as_str()),
            (
                Some("ok"),
                Some("v2 as Seldon wrote it; the next capture updates it to v4")
            ),
            "{row}"
        );
        assert!(row.get("fix").is_none(), "{row}");

        let out = capture(&env, true);
        assert!(out.status.code().is_some(), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(
            (&v["rulesUpdated"]["from"], &v["rulesUpdated"]["version"]),
            (&serde_json::json!("v2"), &serde_json::json!(4))
        );
        assert_eq!(read(&path), format!("{current}{mine}"), "{language}");
        let archived: Vec<_> = std::fs::read_dir(root.join("archive"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() != ".gitkeep")
            .collect();
        assert!(archived.is_empty(), "nothing to archive: {archived:?}");
        assert_eq!(rules_row(&env)["message"], "current (v4)");
        // once
        let v = json(&capture(&env, true));
        assert_eq!(v["rulesUpdated"], serde_json::Value::Null);
    }
}

#[test]
fn a_capture_says_so_in_one_line_and_replaces_a_released_v1_file() {
    let env = Env::new(Snapper::Allowed);
    let root = logbook(&env, "de");
    let path = root.join("AGENTS.md");
    let current = read(&path);
    std::fs::write(&path, golden_v1("v0.1.1-de")).unwrap();
    let out = capture(&env, false);
    let human = stdout(&out);
    assert!(
        human.contains(
            "note: AGENTS.md: Seldon's agent rules updated from v1 to v4 (Seldon's text was unedited; your own rules are kept)"
        ),
        "{human}"
    );
    assert_eq!(read(&path), current);
}

#[test]
fn a_capture_leaves_edited_missing_and_damaged_rules_alone() {
    let env = Env::new(Snapper::Allowed);
    let root = logbook(&env, "en");
    let path = root.join("AGENTS.md");
    let edited = v2("wp101", "en").replacen("Rules for every agent", "Rules for my agents", 1);
    for text in [
        Some(edited.as_str()),
        Some("<!-- seldon:begin rules v2 -->\nno end\n"),
        Some("# My own rules\n"),
        None,
    ] {
        match text {
            Some(t) => std::fs::write(&path, t).unwrap(),
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
        let v = json(&capture(&env, true));
        assert_eq!(v["rulesUpdated"], serde_json::Value::Null, "{text:?}");
        match text {
            Some(t) => assert_eq!(read(&path), t),
            None => assert!(!path.exists(), "a missing file stays missing"),
        }
    }
    // the edited block keeps the banner and its fix
    std::fs::write(&path, &edited).unwrap();
    let row = rules_row(&env);
    assert_eq!(
        (
            row["status"].as_str(),
            row["message"].as_str(),
            row["fix"].as_str()
        ),
        (
            Some("degraded"),
            Some("outdated (v2)"),
            Some("seldon rules update (archives your copy)")
        )
    );
}

/// Round 2, N6: when the user cannot be told, the capture fails closed:
/// no write, one warning; a probe owned by this user lets it write.
#[test]
fn a_capture_that_cannot_tell_the_user_changes_nothing() {
    let env = Env::new(Snapper::Allowed);
    let root = logbook(&env, "en");
    let path = root.join("AGENTS.md");
    let current = read(&path);
    let old = v2("wp101", "en");
    std::fs::write(&path, &old).unwrap();
    let missing = env.tmp.path().join("no-such-probe");
    let v = json(&capture_with(
        &env,
        true,
        &[("SELDON_TEST_ROOT_PROBE", &missing)],
    ));
    assert_eq!(v["rulesUpdated"], serde_json::Value::Null, "{v}");
    assert_eq!(read(&path), old);
    let warnings = v["warnings"].to_string();
    assert!(
        warnings.contains("cannot tell which user runs this capture"),
        "{warnings}"
    );
    // a probe this user owns: a user, so the upgrade runs
    let mine = env.tmp.path().join("probe");
    std::fs::write(&mine, "").unwrap();
    common::user_owned(&mine);
    let v = json(&capture_with(
        &env,
        true,
        &[("SELDON_TEST_ROOT_PROBE", &mine)],
    ));
    assert_eq!(
        (&v["rulesUpdated"]["from"], &v["rulesUpdated"]["version"]),
        (&serde_json::json!("v2"), &serde_json::json!(4))
    );
    assert_eq!(read(&path), current);
}

fn v3(name: &str, language: &str) -> String {
    read(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("templates/rules-v3/AGENTS-{name}-{language}.md")),
    )
}

/// WP-116 (WP-111's D7 reopened): the unedited v3 block of 0.1.3 becomes
/// v4 in a commit of its own, which holds `AGENTS.md` and nothing else;
/// the user's other changes stay uncommitted.
#[test]
fn a_silent_upgrade_is_committed_alone() {
    let env = Env::new(Snapper::Allowed);
    if !env.has_git {
        return;
    }
    let root = logbook(&env, "en");
    let path = root.join("AGENTS.md");
    let current = read(&path);
    let mine = "- Never touch ~/Music.\n";
    std::fs::write(&path, format!("{}{mine}", v3("wp111", "en"))).unwrap();
    let out = env.git(
        &root,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qam",
            "my rule",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    // the user's own change, not committed, one staged and one not
    std::fs::write(root.join("memory/notes.md"), "mine\n").unwrap();
    let lessons = root.join("memory/lessons.md");
    std::fs::write(&lessons, format!("{}- staged\n", read(&lessons))).unwrap();
    let out = env.git(&root, &["add", "memory/lessons.md"]);
    assert_eq!(out.status.code(), Some(0));

    let out = capture(&env, true);
    let v = json(&out);
    assert_eq!(v["rulesUpdated"]["from"], "v3", "{v}");
    assert_eq!(v["rulesUpdated"]["git"]["committed"], true, "{v}");
    assert_eq!(read(&path), format!("{current}{mine}"));
    assert_eq!(
        last_commit(&env, &root),
        "seldon: rules update (unedited, v3 → v4)"
    );
    let files = env.git(&root, &["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(String::from_utf8_lossy(&files.stdout), "AGENTS.md\n");
    let status = env.git(&root, &["status", "--porcelain"]);
    let status = String::from_utf8_lossy(&status.stdout).to_string();
    assert!(
        status.contains("M  memory/lessons.md"),
        "still staged: {status}"
    );
    assert!(status.contains("?? memory/notes.md"), "{status}");
    assert!(!status.contains("AGENTS.md"), "{status}");

    // the human output says it once; a second capture finds nothing
    let v = json(&capture(&env, true));
    assert_eq!(v["rulesUpdated"], serde_json::Value::Null);
    assert_eq!(
        last_commit(&env, &root),
        "seldon: rules update (unedited, v3 → v4)"
    );

    // --no-commit and `git.autocommit = false` leave it to the next commit
    std::fs::write(&path, v3("wp111r2", "en")).unwrap();
    let out = env
        .command(&["--no-commit", "--json", "capture", "--all"])
        .env("SELDON_NOW", NOW)
        .env("SELDON_PACMAN_LOG", env.tmp.path().join("pacman.log"))
        .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
        .env("SELDON_OMARCHY_PLUGINS_DIR", env.tmp.path().join("plugins"))
        .env("SELDON_THEME_FILE", env.tmp.path().join("theme.name"))
        .output()
        .unwrap();
    let v = json(&out);
    assert_eq!(v["rulesUpdated"]["from"], "v3", "{v}");
    assert_eq!(v["rulesUpdated"]["git"]["committed"], false, "{v}");
    assert_eq!(read(&path), current);
}

/// Stage-1 N7: when `AGENTS.md` holds an uncommitted change of the
/// user's (below the block), the update is written but not committed on
/// its own: a commit named "unedited" never carries the user's edit; the
/// capture line says so.
#[test]
fn a_silent_upgrade_of_a_file_with_own_changes_is_not_committed() {
    let env = Env::new(Snapper::Allowed);
    if !env.has_git {
        return;
    }
    let root = logbook(&env, "en");
    let path = root.join("AGENTS.md");
    let current = read(&path);
    std::fs::write(&path, v3("wp111", "en")).unwrap();
    let out = env.git(
        &root,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qam",
            "v3",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let head = last_commit(&env, &root);
    // the user's own line, not committed
    let mine = "- Never touch ~/Music.\n";
    std::fs::write(&path, format!("{}{mine}", v3("wp111", "en"))).unwrap();

    let out = capture(&env, false);
    let human = stdout(&out);
    assert!(
        human.contains(
            "updated from v3 to v4 (Seldon's text was unedited; your own rules are kept); not committed: AGENTS.md has uncommitted changes of yours; the update goes with your next commit"
        ),
        "{human}"
    );
    assert_eq!(read(&path), format!("{current}{mine}"));
    assert_eq!(last_commit(&env, &root), head, "no commit");
    let status = env.git(&root, &["status", "--porcelain", "--", "AGENTS.md"]);
    assert_eq!(String::from_utf8_lossy(&status.stdout), " M AGENTS.md\n");
}

/// WP-171, ADR-0049 §2: an `AGENTS.md` that is a link is not upgraded
/// through it; the capture goes on and says why in a warning, the link and
/// the file it points to stay as they were.
#[test]
fn a_capture_does_not_upgrade_rules_through_a_link() {
    let env = Env::new(Snapper::Allowed);
    let root = logbook(&env, "en");
    let path = root.join("AGENTS.md");
    let outside = env.tmp.path().join("outside-AGENTS.md");
    let text = format!("{}- Never touch ~/Music.\n", v2("wp101", "en"));
    std::fs::write(&outside, &text).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();

    let out = capture(&env, true);
    assert!(out.status.code().is_some(), "{}", stderr(&out));
    let v = json(&out);
    assert_eq!(v["rulesUpdated"], serde_json::Value::Null, "{v}");
    let warnings = v["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|w| w
            == "AGENTS.md: Seldon's agent rules were not updated: AGENTS.md is a symbolic link, not a file of the logbook; make it a file and run the command again"),
        "{v}"
    );
    assert_eq!(read(&outside), text);
    assert_eq!(std::fs::read_link(&path).unwrap(), outside);
}
