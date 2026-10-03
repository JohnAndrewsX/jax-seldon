//! `seldon import omarchy-agent` (WP-043) on the synthetic vault
//! `fixtures/vaults/omarchy-agent/`: the dry-run report is golden
//! (`tests/golden/IMPORT-omarchy-agent.md`, `SELDON_BLESS=1` rewrites it),
//! `--apply` writes what the report lists in one commit, and a second
//! apply changes nothing. The vault is copied into the test home, so the
//! report shows it as `~/omarchy-agent-vault`; it is never written.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{Env, Snapper, copy_dir, find_file, json, read, stderr, stdout};

const NOW: &str = "2026-10-02T10:00:00+02:00";
const REPORT: &str = "outputs/IMPORT-omarchy-agent.md";
const MARKER: &str = ".seldon/imports/omarchy-agent.json";

fn fixture_vault() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/vaults/omarchy-agent")
}

/// Every file under `dir` (except `.git/`) with its bytes.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// A German logbook that already has C-2026-001 (the kit's C-2026-001
/// collides) and a journal entry on 2026-08-20 (a kit session day), and
/// the vault copied into the home.
fn setup() -> (Env, PathBuf, PathBuf) {
    let env = Env::new(Snapper::NoPermissions);
    let root = env.init_logbook_at("logbook", "de");
    let out = env.at(
        "2026-10-01T09:00:00+02:00",
        &[
            "plan",
            "new",
            "Seldon einrichten",
            "--zone",
            "green",
            "--risk",
            "R0",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let out = env.at(
        "2026-08-20T18:00:00+02:00",
        &["log", "Abends noch die Maschine angesehen."],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let vault = env.home.join("omarchy-agent-vault");
    copy_dir(&fixture_vault(), &vault);
    (env, root, vault)
}

fn import(env: &Env, vault: &Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["import", "omarchy-agent", vault.to_str().unwrap()];
    args.extend_from_slice(extra);
    env.at(NOW, &args)
}

fn last_commit(env: &Env, root: &Path) -> String {
    stdout(&env.git(root, &["log", "-1", "--format=%s"]))
        .trim()
        .to_string()
}

fn commit_count(env: &Env, root: &Path) -> usize {
    stdout(&env.git(root, &["rev-list", "--count", "HEAD"]))
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn the_dry_run_report_is_golden_and_nothing_else_is_written() {
    let (env, root, vault) = setup();
    let vault_before = tree(&vault);
    let before = tree(&root);

    let out = import(&env, &vault, &["--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let j = json(&out);
    assert_eq!(j["mode"], "dry-run");
    assert_eq!(j["changed"], true);
    assert_eq!(j["errors"], 0);
    assert_eq!(j["vault"], "~/omarchy-agent-vault");
    assert_eq!(j["files"], serde_json::json!([REPORT]));
    assert_eq!(j["counts"]["cases"], 6);
    assert_eq!(j["counts"]["renumbered"], 1);
    assert_eq!(j["counts"]["journalSessions"], 4);
    assert_eq!(j["counts"]["journalDays"], 3);
    assert_eq!(j["counts"]["memorySections"], 5);
    assert_eq!(j["counts"]["deviationRows"], 2);
    assert_eq!(j["counts"]["redactedLines"], 2);
    assert_eq!(j["counts"]["privatePaths"], 2);
    assert_eq!(j["counts"]["rewrittenLinks"], 1);
    assert_eq!(j["counts"]["rewrittenIds"], 4);
    assert_eq!(j["counts"]["assumptions"], 1);
    assert_eq!(j["assumptions"][0]["case"], "C-2026-005");
    assert_eq!(
        j["collisions"],
        serde_json::json!([{
            "from": "C-2026-001",
            "to": "C-2026-007",
            "takenBy": "work/queued/C-2026-001-seldon-einrichten.md"
        }])
    );

    // the report is the only change, committed on its own
    let after = tree(&root);
    let changed: Vec<&String> = after
        .keys()
        .filter(|k| before.get(*k) != after.get(*k))
        .collect();
    assert_eq!(changed, [REPORT]);
    assert!(before.keys().all(|k| after.contains_key(k)));
    if env.has_git {
        assert_eq!(
            last_commit(&env, &root),
            "seldon: import omarchy-agent (dry run)"
        );
    }
    assert_eq!(tree(&vault), vault_before, "the vault is read only");

    let text = read(&root.join(REPORT));
    assert!(text.starts_with("<!-- generated by seldon; do not edit -->\n"));
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/IMPORT-omarchy-agent.md");
    if std::env::var_os("SELDON_BLESS").is_some() {
        std::fs::write(&golden, &text).unwrap();
    }
    let expected = std::fs::read_to_string(&golden)
        .unwrap_or_else(|_| panic!("{} missing; run with SELDON_BLESS=1", golden.display()));
    assert_eq!(text, expected, "{} differs", golden.display());
    // neither secrets nor the home path reach the report
    assert!(!text.contains("abc123geheim") && !text.contains("/home/user"));

    // the same dry run again changes nothing
    let commits = env.has_git.then(|| commit_count(&env, &root));
    let out = import(&env, &vault, &["--dry-run", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["changed"], false);
    assert_eq!(tree(&root), after);
    if let Some(n) = commits {
        assert_eq!(commit_count(&env, &root), n);
    }
}

#[test]
fn apply_writes_the_plan_once_and_a_second_apply_changes_nothing() {
    let (env, root, vault) = setup();
    // an older memory file: the import moves its `updated`
    let lessons = root.join("memory/lessons.md");
    let text = read(&lessons);
    let line = text
        .lines()
        .find(|l| l.starts_with("updated: "))
        .unwrap()
        .to_string();
    std::fs::write(&lessons, text.replacen(&line, "updated: 2026-09-01", 1)).unwrap();
    if env.has_git {
        let out = env.git(
            &root,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-qam",
                "older lessons",
            ],
        );
        assert!(out.status.success(), "{}", stderr(&out));
    }
    let vault_before = tree(&vault);
    let commits = env.has_git.then(|| commit_count(&env, &root));

    let out = import(&env, &vault, &["--apply", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let j = json(&out);
    assert_eq!(j["mode"], "apply");
    assert_eq!(j["marker"], MARKER);
    if let Some(n) = commits {
        assert_eq!(commit_count(&env, &root), n + 1, "one commit");
        assert_eq!(last_commit(&env, &root), "seldon: import omarchy-agent");
        let status = stdout(&env.git(&root, &["status", "--porcelain"]));
        assert_eq!(status, "", "everything committed");
    }
    assert_eq!(tree(&vault), vault_before, "the vault is read only");

    // cases: ids kept unless taken; folders by mapped status
    let completed = root.join("work/completed");
    let queued = root.join("work/queued");
    let renumbered = read(&find_file(&completed, "C-2026-007-"));
    assert!(renumbered.contains("id: C-2026-007\n"), "{renumbered}");
    assert!(
        renumbered.contains("tags: [setup, dossier, omarchy-agent, omarchy-agent/C-2026-001]\n")
    );
    assert!(renumbered.contains("closed: 2026-08-15\n"));
    assert!(renumbered.contains("*Imported from omarchy-agent as C-2026-001; renumbered"));
    assert!(renumbered.contains("## Intent\nDie Maschine einmal vollständig erfassen"));
    assert!(renumbered.contains("### Befund\n"));
    assert!(renumbered.contains("#### Einzelheiten\n"));
    assert!(renumbered.contains("```\n## keine Überschrift, nur Ausgabe\n"));
    // Ergebnis is the Result; the case's own old id in its text is the new one
    assert!(
        renumbered.ends_with(
            "## Result\nDas Dossier steht unter `system/`. Siehe [[C-2026-005-hostname-prompt]].\n"
        ),
        "{renumbered}"
    );
    assert!(!renumbered.contains("Ergebnis"));
    assert!(renumbered.contains("status done; C-2026-007 abgeschlossen\n"));
    assert_eq!(
        find_file(&completed, "C-2026-007-").file_name().unwrap(),
        "C-2026-007-erste-schritte.md"
    );
    let ours = read(&find_file(&queued, "C-2026-001-"));
    assert!(ours.contains("title: \"Seldon einrichten\""), "untouched");
    let verification = read(&find_file(&queued, "C-2026-004-"));
    assert!(verification.contains("status: queued\n"));
    assert!(
        verification.contains("(status verification → queued); imported cases are never active")
    );
    assert!(verification.contains("token=‹redacted›") && !verification.contains("abc123geheim"));
    assert!(
        verification.contains("`~/.config/hypr/input.lua`") && !verification.contains("/home/user")
    );
    let dropped = read(&find_file(&completed, "C-2026-006-"));
    assert!(dropped.contains("status: dropped\n"));
    for prefix in ["C-2026-002-", "C-2026-003-"] {
        assert!(read(&find_file(&queued, prefix)).contains("status: queued\n"));
    }
    // done without `closed`: closed = created, said in the Log
    let no_closed = read(&find_file(&completed, "C-2026-005-"));
    assert!(
        no_closed.contains("status: completed\n") && no_closed.contains("closed: 2026-08-20\n")
    );
    assert!(no_closed.contains("; closed date assumed: 2026-08-20 · human\n"));
    assert!(
        no_closed.contains("baut auf C-2026-007 auf\n"),
        "a mention of a renumbered id is rewritten"
    );
    let active = std::fs::read_dir(root.join("work/active"))
        .map(|r| {
            r.filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("C-")
            })
            .count()
        })
        .unwrap_or(0);
    assert_eq!(active, 0, "an imported case is never active");

    // every case file is a valid case (the index reads them all)
    let out = env.at(NOW, &["index", "--check", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let index = json(&out);
    assert_eq!(index["valid"], true);
    assert!(
        index["warnings"].as_array().unwrap().is_empty(),
        "{}",
        index["warnings"]
    );

    // one note per case at its created date, attached to the case
    let notes: Vec<serde_json::Value> = common::ledger(&root)
        .into_iter()
        .filter(|e| e["meta"]["import"] == "omarchy-agent")
        .collect();
    assert_eq!(notes.len(), 6);
    let note = notes.iter().find(|e| e["case"] == "C-2026-007").unwrap();
    assert_eq!(note["kind"], "note");
    assert_eq!(note["source"], "manual");
    assert_eq!(note["actor"], "human");
    assert_eq!(note["meta"]["originalId"], "C-2026-001");
    assert!(
        note["ts"]
            .as_str()
            .unwrap()
            .starts_with("2026-08-14T00:00:00")
    );
    assert!(renumbered.contains(&format!("events: [{}]", note["id"].as_str().unwrap())));
    assert!(root.join("ledger/2026-08.jsonl").is_file());

    // journal: sessions by day, appended to the existing day
    let day = read(&root.join("journal/2026/2026-08-20.md"));
    assert!(day.contains("## 18:00 · human\nAbends noch die Maschine angesehen.\n"));
    assert!(day.contains("\n## Imported from omarchy-agent\n*Source: `journal/2026-08.md`.*\n\n### 2026-08-20 — Prompt und Editor\n"));
    assert!(day.contains("### 2026-08-20 — Nachtrag: Sitzungsabschluss\n"));
    assert!(day.contains("cases: [C-2026-005, C-2026-002]\n"), "{day}");
    let day = read(&root.join("journal/2026/2026-08-24.md"));
    assert!(day.contains("das Dossier aus [[C-2026-007-erste-schritte]]."));
    assert!(
        day.contains("cases: [C-2026-003, C-2026-006, C-2026-007]\n"),
        "{day}"
    );
    assert!(day.contains("#### Nachtrag zum selben Tag\n"));
    assert!(
        day.contains("## 2026-08-25 ist keine Sitzung"),
        "fenced text stays"
    );
    let day = read(&root.join("journal/2026/2026-09-03.md"));
    assert!(day.contains("https://‹redacted›@example.org/check"));
    assert!(day.contains("`~/.config/hypr/input.lua`"));

    // memory: lessons appended, topics new
    let lessons = read(&root.join("memory/lessons.md"));
    assert!(lessons.contains("[[C-2026-005-hostname-prompt]] und C-2026-007."));
    assert!(
        lessons.starts_with("---\ntype: memory\ntopic: lessons\nupdated: 2026-10-02\n---\n"),
        "{lessons}"
    );
    assert!(lessons.contains("\n## Verifikation heißt: den ganzen Pfad prüfen\n*Imported from omarchy-agent: `knowledge/lessons/ganzen-pfad-pruefen.md`.*\n\nDestilliert"));
    assert!(lessons.contains("### Der Kern\n"));
    let omarchy = read(&root.join("memory/omarchy.md"));
    assert!(omarchy.starts_with("---\ntype: memory\ntopic: omarchy\nupdated: 2026-10-02\n---\n# Omarchy\n\n## Snapshots vor Updates\n"));
    assert!(omarchy.contains("## Themes und ihre Ordner\n*Imported from omarchy-agent: `knowledge/omarchy/themes.md`, updated 2026-08-24.*"));
    assert!(root.join("memory/arch.md").is_file() && root.join("memory/scripts.md").is_file());
    assert!(
        !root.join("memory/hyprland.md").exists(),
        "an empty topic adds nothing"
    );

    // deviations: rows without a case, the duplicate and resolved left out
    let dev = read(&root.join("system/deviations.md"));
    assert!(dev.contains("| ~/.config/hypr/bindings.lua | Q1 — Eigene Tastenkürzel in `~/.config/hypr/bindings.lua` (omarchy-agent) | 2026-08-20 | — |\n"), "{dev}");
    assert!(dev.contains("| /etc/vconsole.conf | Q2 — Konsole mit deutscher Belegung (seit C-2026-007) (omarchy-agent) | 2026-08-14 | — |\n"), "the reason names the new id");
    assert_eq!(dev.matches("bindings.lua |").count(), 1);
    assert!(!dev.contains("alter-dateimanager") && !dev.contains("snapper"));

    // the report says applied; the marker names the mapping
    let report = read(&root.join(REPORT));
    assert!(report.contains("- Mode: applied 2026-10-02 10:00."));
    let marker: serde_json::Value = serde_json::from_str(&read(&root.join(MARKER))).unwrap();
    assert_eq!(marker["cases"]["C-2026-001"], "C-2026-007");
    assert_eq!(marker["cases"]["C-2026-004"], "C-2026-004");

    // a second apply (and a dry run) changes nothing
    let snapshot = tree(&root);
    let commits = env.has_git.then(|| commit_count(&env, &root));
    for extra in [&["--apply"][..], &["--apply", "--json"], &["--json"]] {
        let out = import(&env, &vault, extra);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        if extra.contains(&"--json") {
            let j = json(&out);
            assert_eq!(j["changed"], false);
            assert_eq!(j["alreadyImported"]["by"], MARKER);
        } else {
            assert!(
                stdout(&out).starts_with("Nothing changed"),
                "{}",
                stdout(&out)
            );
        }
    }
    assert_eq!(tree(&root), snapshot);
    if let Some(n) = commits {
        assert_eq!(commit_count(&env, &root), n);
    }

    // import notes without the marker: refused, with the way back
    std::fs::remove_file(root.join(MARKER)).unwrap();
    let before = tree(&root);
    let out = import(&env, &vault, &["--apply", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(
        message.contains(".seldon/imports/omarchy-agent.json is missing"),
        "{message}"
    );
    assert!(
        message.contains(&format!("`git -C {} status`", root.display())),
        "{message}"
    );
    assert!(!message.contains("git clean"), "{message}");
    assert_eq!(tree(&root), before);
}

#[test]
fn an_editor_formatted_deviations_table_keeps_its_rows() {
    let (env, root, vault) = setup();
    let path = root.join("system/deviations.md");
    let text = read(&path).replace(
        "|---|---|---|---|\n",
        "| --- | --- | --- | --- |\n| ~/.zshrc | eigene Shell | 2026-09-01 | — |\n",
    );
    assert!(text.contains("| --- |"));
    std::fs::write(&path, &text).unwrap();
    let out = import(&env, &vault, &["--apply"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let dev = read(&path);
    assert!(
        dev.contains("| path | reason | date | case |\n| --- | --- | --- | --- |\n| ~/.zshrc | eigene Shell | 2026-09-01 | — |\n| ~/.config/hypr/bindings.lua |"),
        "{dev}"
    );
    assert_eq!(dev.matches("| path | reason |").count(), 1);
}

/// WP-050: a deviations fence without its own end marker plans no rows;
/// it is an error (apply is blocked) and the file is never touched.
#[test]
fn a_deviations_fence_without_its_end_blocks_the_rows() {
    let (env, root, vault) = setup();
    let path = root.join("system/deviations.md");
    let open = read(&path).replace("<!-- seldon:end -->", "Meine Notiz.");
    assert!(!open.contains("<!-- seldon:end -->"));
    std::fs::write(&path, &open).unwrap();
    let out = import(&env, &vault, &["--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let j = json(&out);
    assert_eq!(j["counts"]["deviationRows"], 0, "{j}");
    let skipped = j["skipped"].as_array().unwrap();
    assert!(
        skipped.iter().any(|s| s["path"] == "system/deviations.md"
            && s["error"] == true
            && s["reason"]
                .as_str()
                .unwrap()
                .contains("deviations.table fence has no end marker")),
        "{j}"
    );
    let out = import(&env, &vault, &["--apply"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert_eq!(read(&path), open);
}

#[test]
fn a_failed_apply_says_how_to_undo_it() {
    use std::os::unix::fs::PermissionsExt as _;
    let (env, root, vault) = setup();
    if !env.has_git {
        return;
    }
    // the deviation rows cannot be written: after the ledger, the cases,
    // the journal and the memory files
    let system = root.join("system");
    std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(system.join("probe"), "x").is_ok() {
        // running as root: permissions do not stop the write
        std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let commits = commit_count(&env, &root);
    let out = import(&env, &vault, &["--apply", "--json"]);
    std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(message.contains("nothing was committed"), "{message}");
    let undo = undo_command(&message);
    assert!(
        undo.starts_with("git --literal-pathspecs checkout "),
        "{undo}"
    );
    assert!(!undo.contains(" . ") && !undo.contains("clean"), "{undo}");
    assert_eq!(commit_count(&env, &root), commits, "nothing committed");
    assert!(!root.join(MARKER).exists());
    assert!(
        common::ledger(&root)
            .iter()
            .any(|e| e["meta"]["import"] == "omarchy-agent")
    );

    // not reported as done: refused until the logbook is restored, with
    // the same undo
    let out = import(&env, &vault, &["--apply"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(stderr(&out).contains("is missing"), "{}", stderr(&out));
    assert_eq!(undo_command(&stderr(&out)), undo);

    // the hint works: restore, then the import runs once
    run_undo(&env, &root, &undo);
    assert_eq!(stdout(&env.git(&root, &["status", "--porcelain"])), "");
    let out = import(&env, &vault, &["--apply"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        common::ledger(&root)
            .iter()
            .filter(|e| e["meta"]["import"] == "omarchy-agent")
            .count(),
        6
    );
    assert_eq!(commit_count(&env, &root), commits + 1);
}

/// The command between the backticks after "undo it with".
fn undo_command(message: &str) -> String {
    let start = message
        .find("undo it with `")
        .unwrap_or_else(|| panic!("no undo in {message}"))
        + "undo it with `".len();
    let len = message[start..].find('`').unwrap();
    message[start..start + len].to_string()
}

/// Runs the printed undo in the logbook, as the user would.
fn run_undo(env: &Env, root: &Path, undo: &str) {
    let git_dir = env.tmp.path().join("bin");
    let out = std::process::Command::new("sh")
        .args(["-c", undo])
        .env_clear()
        .env("HOME", &env.home)
        .env("PATH", format!("{}:/usr/bin:/bin", git_dir.display()))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(root)
        .output()
        .unwrap();
    assert!(out.status.success(), "{undo}: {}", stderr(&out));
}

/// WP-061 (F-142): pending changes are committed before the first write,
/// and the undo of a failed apply touches only the files the import wrote:
/// a hook's ledger line, an inbox note and an edit of a file the import
/// also writes survive it, and so do changes made after the failure.
#[test]
fn a_dirty_logbook_is_committed_first_and_the_undo_keeps_other_changes() {
    use std::os::unix::fs::PermissionsExt as _;
    let (env, root, vault) = setup();
    if !env.has_git {
        return;
    }
    // pending: a note (ledger line and journal day), an inbox note, and an
    // edit of memory/lessons.md, which the import appends to
    let out = env.at(
        "2026-10-01T12:00:00+02:00",
        &["--no-commit", "log", "Pending note before the import."],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    std::fs::create_dir_all(root.join("inbox")).unwrap();
    std::fs::write(root.join("inbox/idee.md"), "Eine Idee.\n").unwrap();
    let lessons = root.join("memory/lessons.md");
    let edited = format!("{}\nMeine eigene Lektion.\n", read(&lessons));
    std::fs::write(&lessons, &edited).unwrap();
    let pending = tree(&root);
    let commits = commit_count(&env, &root);

    let system = root.join("system");
    std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(system.join("probe"), "x").is_ok() {
        // running as root: permissions do not stop the write
        std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let out = import(&env, &vault, &["--apply", "--json"]);
    std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();

    // the pending changes are one commit before the import's first write
    assert_eq!(commit_count(&env, &root), commits + 1);
    assert_eq!(
        last_commit(&env, &root),
        "seldon: before import omarchy-agent"
    );
    assert_ne!(read(&lessons), edited, "the import did not write lessons");

    // after the failure, a new inbox note and an edit of an unrelated file
    std::fs::write(root.join("inbox/later.md"), "Später.\n").unwrap();
    let project = root.join("PROJECT.md");
    let project_text = format!("{}\nNach dem Import.\n", read(&project));
    std::fs::write(&project, &project_text).unwrap();

    run_undo(&env, &root, &undo_command(&message));
    let mut expected = pending;
    expected.insert("inbox/later.md".into(), "Später.\n".into());
    expected.insert("PROJECT.md".into(), project_text.into_bytes());
    assert_eq!(
        tree(&root).keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>()
    );
    assert!(tree(&root) == expected, "the undo changed other files");
    assert!(
        !common::ledger(&root)
            .iter()
            .any(|e| e["meta"]["import"] == "omarchy-agent")
    );
    let status = stdout(&env.git(&root, &["status", "--porcelain"]));
    assert_eq!(status, " M PROJECT.md\n?? inbox/later.md\n");
}

/// WP-061: with `--no-commit` the pending changes cannot be committed, so
/// the apply is refused before it writes anything.
#[test]
fn a_dirty_logbook_without_a_commit_is_refused() {
    let (env, root, vault) = setup();
    if !env.has_git {
        return;
    }
    std::fs::create_dir_all(root.join("inbox")).unwrap();
    std::fs::write(root.join("inbox/idee.md"), "Eine Idee.\n").unwrap();
    let before = tree(&root);
    let commits = commit_count(&env, &root);
    let out = import(&env, &vault, &["--no-commit", "--apply", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(
        message.starts_with(
            "the logbook has uncommitted changes (not committed: --no-commit); commit them first"
        ),
        "{message}"
    );
    assert_eq!(tree(&root), before, "the refusal wrote something");
    assert_eq!(commit_count(&env, &root), commits);
}

/// An apply that fails at the deviation rows (read-only `system/`); its
/// error message, or `None` when permissions do not stop the write (root).
fn failed_apply(env: &Env, root: &Path, vault: &Path) -> Option<String> {
    use std::os::unix::fs::PermissionsExt as _;
    let system = root.join("system");
    std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(system.join("probe"), "x").is_ok() {
        std::fs::remove_file(system.join("probe")).unwrap();
        std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o755)).unwrap();
        return None;
    }
    let out = import(env, vault, &["--apply", "--json"]);
    std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    Some(json(&out)["error"]["message"].as_str().unwrap().to_string())
}

/// WP-061: the kept undo lives in the logbook, which agents write too. A
/// tampered one (no hash base, a path outside the import's folders,
/// pathspec magic) is never printed as a command; the refusal points at
/// `git status` instead.
#[test]
fn a_tampered_undo_file_is_not_offered_as_a_command() {
    let (env, root, vault) = setup();
    if !env.has_git || failed_apply(&env, &root, &vault).is_none() {
        return;
    }
    let undo_file = root.join(".seldon/imports/omarchy-agent.undo.json");
    let kept: serde_json::Value = serde_json::from_str(&read(&undo_file)).unwrap();
    let base = kept["base"].clone();
    assert!(base.as_str().is_some_and(|b| b.len() == 40), "{kept}");

    // the real one is offered
    let out = import(&env, &vault, &["--apply", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(
        message.contains("undo it with `git --literal-pathspecs checkout "),
        "{message}"
    );

    let tampered = [
        serde_json::json!({ "base": null, "restore": ["memory/lessons.md"], "remove": [] }),
        serde_json::json!({ "base": "HEAD", "restore": ["memory/lessons.md"], "remove": [] }),
        serde_json::json!({ "base": base, "restore": [".git/config"], "remove": [] }),
        serde_json::json!({ "base": base, "restore": [], "remove": [".git/HEAD"] }),
        serde_json::json!({ "base": base, "restore": [], "remove": ["PROJECT.md"] }),
        serde_json::json!({ "base": base, "restore": [], "remove": ["../outside"] }),
        serde_json::json!({ "base": base, "restore": [":/"], "remove": [] }),
        serde_json::json!({ "base": base, "restore": ["memory/*"], "remove": [] }),
    ];
    for undo in tampered {
        std::fs::write(&undo_file, undo.to_string()).unwrap();
        let before = tree(&root);
        let out = import(&env, &vault, &["--apply", "--json"]);
        assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
        let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
        assert!(
            !message.contains("undo it with"),
            "{undo} offered: {message}"
        );
        assert!(
            message.contains(&format!("`git -C {} status`", root.display())),
            "{message}"
        );
        assert_eq!(tree(&root), before);
    }
}

/// WP-061: an undo file left behind by an earlier failure is removed by
/// the apply that succeeds, and the import commit records that.
#[test]
fn a_successful_apply_removes_a_leftover_undo_file() {
    let (env, root, vault) = setup();
    let undo_file = root.join(".seldon/imports/omarchy-agent.undo.json");
    std::fs::create_dir_all(undo_file.parent().unwrap()).unwrap();
    std::fs::write(
        &undo_file,
        r#"{"base":null,"restore":[],"remove":["ledger/2026-08.jsonl"]}"#,
    )
    .unwrap();
    let out = import(&env, &vault, &["--apply"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(!undo_file.exists(), "the leftover undo file is still there");
    assert!(root.join(MARKER).exists());
    if env.has_git {
        assert_eq!(stdout(&env.git(&root, &["status", "--porcelain"])), "");
        let tracked = env.git(
            &root,
            &["ls-files", ".seldon/imports/omarchy-agent.undo.json"],
        );
        assert_eq!(stdout(&tracked), "", "the undo file is in HEAD");
    }
}

#[test]
fn errors_block_apply_and_are_reported() {
    let (env, root, vault) = setup();
    let case = vault.join("pipeline/cases/C-2026-003-dunkles-theme.md");
    let text = read(&case).replace("status: in-progress", "status: waiting");
    std::fs::write(&case, text).unwrap();
    let ledger_before = common::ledger(&root).len();

    let out = import(&env, &vault, &["--apply", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(
        message.contains("1 error(s)") && message.contains(REPORT),
        "{message}"
    );
    assert_eq!(
        common::ledger(&root).len(),
        ledger_before,
        "nothing imported"
    );
    assert!(!root.join(MARKER).exists());
    assert!(
        !root
            .join("work/completed")
            .join("C-2026-005-hostname-prompt.md")
            .exists()
    );
    let report = read(&root.join(REPORT));
    assert!(report.contains("- Errors: 1\n"));
    assert!(report.contains("| pipeline/cases/C-2026-003-dunkles-theme.md | **error:** status `waiting` is not new\\|planned"), "{report}");
}

#[test]
fn a_directory_that_is_not_a_vault_is_a_user_error() {
    let env = Env::new(Snapper::NoPermissions);
    let not_init = import(&env, &env.home, &["--json"]);
    assert_eq!(not_init.status.code(), Some(3), "{}", stdout(&not_init));
    env.init_logbook();
    let empty = env.home.join("leer");
    std::fs::create_dir_all(&empty).unwrap();
    for dir in [empty.clone(), env.home.join("fehlt")] {
        let out = import(&env, &dir, &["--json"]);
        assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    }
    let out = import(&env, &empty, &["--apply", "--dry-run"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "--apply conflicts with --dry-run"
    );
}
