//! `seldon import task` (WP-102): the user's Markdown task files become
//! cases — one queued case per open `- [ ]` item, a file without items as
//! one case — idempotently, with the source only read, every path checked
//! before the first write and the text redacted (SPEC-ENGINE §3, §7).

mod common;

use std::path::{Path, PathBuf};

use common::{
    Env, Snapper, assert_valid_index, find_file, json, ledger, read, stderr, stdout, tree,
};

const NOW: &str = "2026-10-07T10:00:00+02:00";
const LATER: &str = "2026-10-07T11:30:00+02:00";
const MARKER: &str = ".seldon/imports/tasks.json";

const TODO: &str = "\
---
owner: me
---
# Desk

Things for the machine.

- [ ] Fix the bar flicker. It happens on the second monitor.
  Only after resume.

  - [ ] check hyprland.conf
- [x] Install zed
## Later
* [ ] Try a lighter theme
";

fn setup() -> (Env, PathBuf) {
    let env = Env::new(Snapper::NoPermissions);
    let root = env.init_logbook();
    (env, root)
}

/// Writes `text` to `~/<rel>` and returns the absolute path.
fn task_file(env: &Env, rel: &str, text: &str) -> PathBuf {
    let path = env.home.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

fn import(env: &Env, now: &str, args: &[&str]) -> std::process::Output {
    let mut all = vec!["import", "task"];
    all.extend_from_slice(args);
    all.push("--json");
    env.at(now, &all)
}

fn ok(out: &std::process::Output) -> serde_json::Value {
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout: {}\nstderr: {}",
        stdout(out),
        stderr(out)
    );
    json(out)
}

/// The message of a refused `--json` command (exit 1).
fn refusal(out: &std::process::Output) -> String {
    assert_eq!(
        out.status.code(),
        Some(1),
        "stdout: {}\nstderr: {}",
        stdout(out),
        stderr(out)
    );
    let v = json(out);
    assert_eq!(v["error"]["code"], 1, "{v}");
    v["error"]["message"].as_str().unwrap().to_string()
}

fn ids(report: &serde_json::Value) -> Vec<String> {
    report["created"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect()
}

fn case_text(root: &Path, folder: &str, id: &str) -> String {
    read(&find_file(&root.join("work").join(folder), id))
}

fn head(env: &Env, root: &Path) -> String {
    let out = env.git(root, &["rev-parse", "HEAD"]);
    String::from_utf8(out.stdout).unwrap()
}

fn index(env: &Env) -> serde_json::Value {
    serde_json::from_str(&read(&env.home.join(".local/state/seldon/index.json"))).unwrap()
}

#[test]
fn open_items_become_queued_cases_and_the_file_is_only_read() {
    let (env, root) = setup();
    let file = task_file(&env, "proj/TODO.md", TODO);
    let before = std::fs::metadata(&file).unwrap().modified().unwrap();

    let report = ok(&import(&env, NOW, &["~/proj/TODO.md"]));
    assert_eq!(report["mode"], "apply");
    assert_eq!(ids(&report), ["C-2026-001", "C-2026-002"]);
    let created = report["created"].as_array().unwrap();
    assert_eq!(created[0]["title"], "Fix the bar flicker");
    assert_eq!(created[0]["status"], "queued");
    assert_eq!(created[0]["source"], "~/proj/TODO.md#8");
    assert_eq!(created[0]["replaces"], serde_json::Value::Null);
    assert_eq!(created[1]["title"], "Try a lighter theme");
    assert_eq!(created[1]["source"], "~/proj/TODO.md#14");
    assert_eq!(
        report["skipped"],
        serde_json::json!([{"source": "~/proj/TODO.md#12", "reason": "done", "case": null}])
    );
    assert_eq!(report["marker"], MARKER);

    // the case: tag, Intent from the item and its block, section, Log line
    let first = case_text(&root, "queued", "C-2026-001");
    assert!(
        first.contains("tags: [imported]") || first.contains("- imported"),
        "{first}"
    );
    assert!(
        first.contains("zone: yellow") && first.contains("risk: R1"),
        "{first}"
    );
    assert!(
        first.contains(
            "## Intent\nFix the bar flicker. It happens on the second monitor.\nOnly after resume.\n\n- [ ] check hyprland.conf\n\nSection: Desk\n"
        ),
        "{first}"
    );
    assert!(
        first.contains("· created (zone yellow, risk R1): imported from ~/proj/TODO.md#8 · human"),
        "{first}"
    );
    let second = case_text(&root, "queued", "C-2026-002");
    assert!(second.contains("Section: Later"), "{second}");

    // ledger: one case-created each, with the risk (contract 2)
    let lines = ledger(&root);
    let created: Vec<_> = lines
        .iter()
        .filter(|e| e["kind"] == "case-created")
        .collect();
    assert_eq!(created.len(), 2);
    assert_eq!(created[0]["detail"], "Fix the bar flicker");
    assert_eq!(created[0]["meta"]["risk"], "R1");

    // one commit, the index lists both with the tag
    if env.has_git {
        let out = env.git(&root, &["log", "-1", "--format=%s"]);
        assert_eq!(
            String::from_utf8(out.stdout).unwrap().trim(),
            "seldon: import task"
        );
        assert_eq!(report["git"]["committed"], true, "{}", report["git"]);
    }
    let index = index(&env);
    assert_valid_index(&index);
    let queued = index["cases"]["queued"].as_array().unwrap();
    assert_eq!(queued.len(), 2);
    assert!(
        queued
            .iter()
            .all(|c| c["tags"] == serde_json::json!(["imported"]))
    );

    // the source is untouched
    assert_eq!(read(&file), TODO);
    assert_eq!(
        std::fs::metadata(&file).unwrap().modified().unwrap(),
        before
    );
}

#[test]
fn a_file_without_items_is_one_case_titled_by_its_heading_or_name() {
    let (env, root) = setup();
    task_file(
        &env,
        "proj/backup.md",
        "Notes first.\n\n# Set up restic backups\n\nDaily, to the NAS.\n## Plan\n- keep 7\n```\n## Log\n```\n",
    );
    let bare = task_file(&env, "proj/fix-wifi.md", "The wifi drops after suspend.\n");
    let report = ok(&import(
        &env,
        NOW,
        &["~/proj/backup.md", bare.to_str().unwrap()],
    ));
    assert_eq!(ids(&report), ["C-2026-001", "C-2026-002"]);
    assert_eq!(report["created"][0]["title"], "Set up restic backups");
    assert_eq!(report["created"][0]["source"], "~/proj/backup.md");
    assert_eq!(report["created"][1]["title"], "fix-wifi");

    // the file's headings and fences are text in the Intent, escaped: the
    // case's own sections stay intact
    let text = case_text(&root, "queued", "C-2026-001");
    assert!(
        text.contains("## Intent\nNotes first.\n\n\nDaily, to the NAS.\n\\## Plan\n- keep 7\n\\```\n\\## Log\n\\```\n"),
        "{text}"
    );
    assert_eq!(text.matches("\n## Log\n").count(), 1, "{text}");
    let out = env.at(LATER, &["plan", "start", "C-2026-001", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = case_text(&root, "active", "C-2026-001");
    assert!(
        text.contains("imported from ~/proj/backup.md · human\n- 2026-10-07 11:30 · started"),
        "{text}"
    );
}

#[test]
fn a_second_run_creates_nothing_and_a_changed_item_names_the_earlier_case() {
    let (env, root) = setup();
    let file = task_file(&env, "TODO.md", TODO);
    ok(&import(&env, NOW, &["~/TODO.md"]));
    let ledger_before = ledger(&root).len();
    let head_before = head(&env, &root);

    // twice: nothing new, each open item reported with its case
    for _ in 0..2 {
        let report = ok(&import(&env, LATER, &["~/TODO.md"]));
        assert_eq!(report["created"], serde_json::json!([]));
        assert_eq!(report["marker"], serde_json::Value::Null);
        assert_eq!(
            report["git"]["reason"], "nothing changed",
            "{}",
            report["git"]
        );
        let skipped = report["skipped"].as_array().unwrap();
        let mut got: Vec<(String, String)> = skipped
            .iter()
            .map(|s| {
                (
                    s["reason"].as_str().unwrap().to_string(),
                    s["case"].as_str().unwrap_or("-").to_string(),
                )
            })
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                ("already-imported".to_string(), "C-2026-001".to_string()),
                ("already-imported".to_string(), "C-2026-002".to_string()),
                ("done".to_string(), "-".to_string()),
            ]
        );
        assert_eq!(ledger(&root).len(), ledger_before);
        assert_eq!(head(&env, &root), head_before);
    }

    // ticking an item does not import it again
    let ticked = TODO.replace("* [ ] Try a lighter theme", "* [x] Try a lighter theme");
    std::fs::write(&file, &ticked).unwrap();
    let report = ok(&import(&env, LATER, &["~/TODO.md", "--include-done"]));
    let reasons: Vec<&str> = report["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["reason"].as_str().unwrap())
        .collect();
    assert_eq!(reasons, ["already-imported", "already-imported"]);
    // only the done item that was never imported is new
    assert_eq!(report["created"][0]["title"], "Install zed");

    // the first item reworded at the same line: a new case naming the old
    let changed = ticked.replace("Fix the bar flicker.", "Fix the bar flicker for good.");
    std::fs::write(&file, &changed).unwrap();
    let report = ok(&import(&env, LATER, &["~/TODO.md"]));
    assert_eq!(ids(&report), ["C-2026-004"]);
    assert_eq!(report["created"][0]["replaces"], "C-2026-001");
    let text = case_text(&root, "queued", "C-2026-004");
    assert!(
        text.contains("imported from ~/TODO.md#8, changed since C-2026-001 · human"),
        "{text}"
    );

    // the marker holds no text, only where, a hash and the case
    let marker: serde_json::Value = serde_json::from_str(&read(&root.join(MARKER))).unwrap();
    assert_eq!(marker["version"], 1);
    assert_eq!(marker["items"].as_array().unwrap().len(), 4);
    assert_eq!(marker["items"][0]["file"], "~/TODO.md");
    assert_eq!(marker["items"][0]["line"], 8);
    assert_eq!(marker["items"][0]["hash"].as_str().unwrap().len(), 64);
    assert!(!read(&root.join(MARKER)).contains("flicker"));
}

#[test]
fn the_same_item_twice_is_imported_once() {
    let (env, _root) = setup();
    task_file(
        &env,
        "a.md",
        "- [ ] Water the plants\n- [ ] Water the plants\n",
    );
    let report = ok(&import(&env, NOW, &["~/a.md", "~/a.md"]));
    assert_eq!(ids(&report), ["C-2026-001"]);
    assert_eq!(report["skipped"][0]["reason"], "duplicate");
    assert_eq!(report["skipped"][0]["source"], "~/a.md#2");
}

#[test]
fn an_empty_file_is_skipped() {
    let (env, root) = setup();
    task_file(&env, "empty.md", "---\nx: 1\n---\n\n  \n");
    task_file(&env, "items.md", "- [ ] .\n- [ ]\n");
    let report = ok(&import(&env, NOW, &["~/empty.md", "~/items.md"]));
    assert_eq!(report["created"], serde_json::json!([]));
    assert_eq!(
        report["skipped"],
        serde_json::json!([
            {"source": "~/empty.md", "reason": "empty", "case": null},
            {"source": "~/items.md#1", "reason": "empty", "case": null},
            {"source": "~/items.md#2", "reason": "empty", "case": null},
        ])
    );
    assert!(!root.join(MARKER).exists());
}

#[test]
fn include_done_makes_completed_cases_but_not_for_an_agent() {
    let (env, root) = setup();
    task_file(&env, "TODO.md", TODO);
    let out = import(
        &env,
        NOW,
        &[
            "~/TODO.md",
            "--include-done",
            "--actor",
            "agent:claude-code",
        ],
    );
    let why = refusal(&out);
    assert!(why.contains("--include-done"), "{why}");
    assert!(!root.join(MARKER).exists());

    let report = ok(&import(
        &env,
        NOW,
        &[
            "~/TODO.md",
            "--include-done",
            "--risk",
            "R0",
            "--zone",
            "green",
            "--area",
            "desk",
        ],
    ));
    assert_eq!(ids(&report), ["C-2026-001", "C-2026-002", "C-2026-003"]);
    assert_eq!(report["created"][1]["status"], "completed");
    assert_eq!(report["areaCreated"], "areas/desk/README.md");
    let done = case_text(&root, "completed", "C-2026-002");
    assert!(
        done.contains("status: completed") && done.contains("closed: 2026-10-07"),
        "{done}"
    );
    assert!(
        done.contains("zone: green") && done.contains("risk: R0") && done.contains("area: desk"),
        "{done}"
    );
    assert!(
        done.contains("imported from ~/TODO.md#12 · human\n- 2026-10-07 10:00 · completed: imported as done · human"),
        "{done}"
    );
    let kinds: Vec<(String, String)> = ledger(&root)
        .iter()
        .filter(|e| e["case"] == "C-2026-002")
        .map(|e| {
            (
                e["kind"].as_str().unwrap().to_string(),
                e["actor"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            ("case-created".to_string(), "human".to_string()),
            ("case-completed".to_string(), "human".to_string())
        ]
    );
    assert_valid_index(&index(&env));

    // an agent may import open items (queued, no close)
    task_file(&env, "more.md", "- [ ] One more\n");
    let report = ok(&import(
        &env,
        NOW,
        &["~/more.md", "--actor", "agent:claude-code"],
    ));
    let text = case_text(
        &root,
        "queued",
        report["created"][0]["id"].as_str().unwrap(),
    );
    assert!(
        text.contains("agents: [agent:claude-code]") || text.contains("- agent:claude-code"),
        "{text}"
    );
}

#[test]
fn a_dry_run_lists_and_writes_nothing() {
    let (env, root) = setup();
    task_file(&env, "TODO.md", TODO);
    let before = tree(env.tmp.path());
    let head_before = head(&env, &root);
    let report = ok(&import(&env, NOW, &["~/TODO.md", "--dry-run"]));
    assert_eq!(report["mode"], "dry-run");
    let created = report["created"].as_array().unwrap();
    assert_eq!(created.len(), 2);
    assert_eq!(created[0]["id"], serde_json::Value::Null);
    assert_eq!(created[0]["title"], "Fix the bar flicker");
    assert_eq!(created[0]["source"], "~/TODO.md#8");
    assert_eq!(report["files"], serde_json::json!([]));
    assert_eq!(tree(env.tmp.path()), before);
    assert_eq!(head(&env, &root), head_before);

    // the human form says what it would do
    let out = env.at(NOW, &["import", "task", "~/TODO.md", "--dry-run"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(
        text.starts_with("Dry run: would create 2 case(s):"),
        "{text}"
    );
    assert!(
        text.contains("\"Fix the bar flicker\" (queued) from ~/TODO.md#8"),
        "{text}"
    );
    assert!(text.contains("~/TODO.md#12 (done)"), "{text}");
    assert_eq!(tree(env.tmp.path()), before);
}

#[test]
fn refused_paths_exit_1_and_write_nothing() {
    let env = Env::new(Snapper::NoPermissions);
    // the logbook inside the home, as `seldon init` makes it by default
    let root = env.init_logbook_at("home/Seldon", "en");
    let good = task_file(&env, "ok.md", "- [ ] fine\n");
    let outside = env.tmp.path().join("outside.md");
    std::fs::write(&outside, "- [ ] not mine\n").unwrap();
    let inside = root.join("work/notes.md");
    std::fs::write(&inside, "- [ ] logbook text\n").unwrap();
    task_file(&env, "notes.txt", "- [ ] text\n");
    task_file(&env, "dir.md/x.md", "- [ ] in a folder\n");
    std::fs::write(env.home.join("latin1.md"), b"- [ ] caf\xe9\n").unwrap();
    std::fs::write(env.home.join("big.md"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
    std::os::unix::fs::symlink(&outside, env.home.join("link.md")).unwrap();
    std::fs::write(env.home.join("ctl\nname.md"), "- [ ] x\n").unwrap();
    let before = tree(env.tmp.path());

    let cases: [(&str, &str); 10] = [
        (outside.to_str().unwrap(), "outside your home directory"),
        ("~/link.md", "outside your home directory"),
        ("../outside.md", "outside your home directory"),
        (inside.to_str().unwrap(), "inside the logbook"),
        ("~/notes.txt", "not a Markdown file"),
        ("~/dir.md", "is a directory"),
        ("~/latin1.md", "not UTF-8"),
        ("~/big.md", "larger than 1024 KiB"),
        ("~/missing.md", "cannot read the task file"),
        ("~/ctl\nname.md", "control character"),
    ];
    for (path, why) in cases {
        // a good file first: every path is checked before anything is written
        let out = env
            .command(&["import", "task", good.to_str().unwrap(), path, "--json"])
            .env("SELDON_NOW", NOW)
            .current_dir(&env.home)
            .output()
            .unwrap();
        let message = refusal(&out);
        assert!(message.contains(why), "{path}: {message}");
        assert_eq!(tree(env.tmp.path()), before, "{path}");
    }
    // the same file through `..` and `~` is fine, and imported once
    std::fs::create_dir(env.home.join("proj")).unwrap();
    let report = ok(&import(&env, NOW, &["~/ok.md", "~/proj/../ok.md"]));
    assert_eq!(ids(&report), ["C-2026-001"]);
}

#[test]
fn more_than_200_new_cases_are_refused() {
    let (env, root) = setup();
    let text: String = (1..=201).map(|n| format!("- [ ] task {n}\n")).collect();
    task_file(&env, "many.md", &text);
    let out = import(&env, NOW, &["~/many.md"]);
    let why = refusal(&out);
    assert!(why.contains("at most 200"), "{why}");
    assert!(!root.join(MARKER).exists());
    assert!(!ledger(&root).iter().any(|e| e["kind"] == "case-created"));
}

#[test]
fn the_text_is_redacted_before_it_reaches_the_logbook() {
    let (env, root) = setup();
    task_file(
        &env,
        "TODO.md",
        "- [ ] Rotate token=s3cr3tvalue123 on the NAS\n  curl -H 'Authorization: Bearer abc.def.ghi' https://nas\n  config in /home/alice/.config/nas\n",
    );
    let report = ok(&import(&env, NOW, &["~/TODO.md"]));
    assert_eq!(report["redactedLines"], 2);
    let id = ids(&report)[0].clone();
    let text = case_text(&root, "queued", &id);
    assert!(
        text.contains("title: \"Rotate token=‹redacted› on the NAS\"")
            || text.contains("title: Rotate token=‹redacted› on the NAS"),
        "{text}"
    );
    assert!(text.contains("Authorization: ‹redacted›"), "{text}");
    assert!(text.contains("config in ~/.config/nas"), "{text}");
    for (name, bytes) in tree(&root) {
        let s = String::from_utf8_lossy(&bytes);
        assert!(!s.contains("s3cr3tvalue123"), "{name}");
        assert!(!s.contains("abc.def.ghi"), "{name}");
        assert!(!s.contains("/home/alice"), "{name}");
    }
    let state = read(&env.home.join(".local/state/seldon/index.json"));
    assert!(!state.contains("s3cr3tvalue123"));
}

#[test]
fn an_unreadable_marker_stops_the_import() {
    let (env, root) = setup();
    task_file(&env, "TODO.md", "- [ ] one\n");
    std::fs::create_dir_all(root.join(".seldon/imports")).unwrap();
    std::fs::write(root.join(MARKER), "{ not json").unwrap();
    let out = import(&env, NOW, &["~/TODO.md"]);
    let why = refusal(&out);
    assert!(why.contains("not a valid import marker"), "{why}");
    assert!(!ledger(&root).iter().any(|e| e["kind"] == "case-created"));
}
