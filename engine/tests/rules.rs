//! `seldon rules update` and the `rules` row of `seldon doctor` (ADR-0027,
//! WP-100), in a throw-away home with a fresh logbook.

mod common;

use std::path::{Path, PathBuf};

use common::{Env, Snapper, json, read, stderr, stdout};

const NOW: &str = "2026-10-05T10:00:00+02:00";
const BEGIN: &str = "<!-- seldon:begin rules v3 -->\n";
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
fn init_writes_the_v3_block_first_and_doctor_calls_it_current() {
    for language in ["en", "de"] {
        let env = Env::new(Snapper::Allowed);
        let root = logbook(&env, language);
        let text = read(&root.join("AGENTS.md"));
        assert!(text.starts_with(BEGIN), "{language}");
        assert!(text.contains("\n## Your rules\n"), "{language}");
        let row = rules_row(&env);
        assert_eq!(row["status"], "ok", "{row}");
        assert_eq!(row["message"], "current (v3)");
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
        diff.contains("\n+<!-- seldon:begin rules v3 -->\n"),
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

    // a v3 block whose text was changed counts as outdated, too
    std::fs::write(
        &path,
        current.replacen("Rules for every agent", "Rules for some agents", 1),
    )
    .unwrap();
    let row = rules_row(&env);
    assert_eq!(row["status"], "degraded");
    assert!(
        row["message"].as_str().unwrap().starts_with("outdated (v3"),
        "{row}"
    );
    assert_eq!(row["fix"], "seldon rules update (archives your copy)");
    let (code, v) = update(&env, &[]);
    assert_eq!(
        (code, v["action"].as_str(), v["from"].as_str()),
        (0, Some("rewritten"), Some("v3"))
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
            "<!-- seldon:begin rules v4 -->\nfuture\n<!-- seldon:end -->\n",
            "newer (v4) than this seldon's rules (v3)",
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
