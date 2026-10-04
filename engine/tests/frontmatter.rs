//! Frontmatter round-trip against `fixtures/logbook/` (read-only).
//!
//! "Round-trip" is the strong form: parse the file into its typed record,
//! then write a *fresh* canonical frontmatter from the record, append the
//! untouched body, and compare with the original bytes. This proves the
//! engine's writing style is the fixtures' style, not just that unchanged
//! text is passed through.

mod common;

use std::path::{Path, PathBuf};

use seldon::frontmatter::Document;
use seldon::logbook::Logbook;
use seldon::model::{self, Area, Case, Decision, Journal, Memory, Project, Record};

fn logbook() -> Logbook {
    Logbook::open(&common::fixture_logbook()).expect("fixture logbook opens")
}

fn assert_round_trip<R: Record + std::fmt::Debug>(files: &[PathBuf]) -> Vec<R> {
    assert!(!files.is_empty(), "no fixture files found");
    files
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            let (record, doc) =
                model::parse::<R>(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            // lossless pass-through
            assert_eq!(
                doc.render(),
                text,
                "{}: parse/render changed bytes",
                path.display()
            );
            // canonical re-serialisation from the typed record
            let rendered = model::render_new(&record, &doc.body);
            assert_eq!(rendered, text, "{}: canonical form differs", path.display());
            // writing the unchanged record back is a no-op
            let mut doc2 = Document::parse(&text).unwrap();
            model::update(&mut doc2, &record).unwrap();
            assert_eq!(
                doc2.render(),
                text,
                "{}: update of unchanged record",
                path.display()
            );
            record
        })
        .collect()
}

mod round_trip {
    use super::*;

    #[test]
    fn case_files() {
        let files = logbook().case_files().unwrap();
        assert_eq!(files.len(), 8, "fixtures/README.md: 8 cases");
        let cases: Vec<Case> = assert_round_trip(&files);
        for (path, case) in files.iter().zip(&cases) {
            let folder = path.parent().unwrap().file_name().unwrap();
            assert_eq!(folder, case.status.folder(), "{}", path.display());
            let name = path.file_name().unwrap().to_string_lossy();
            assert!(name.starts_with(&format!("{}-", case.id)), "{name}");
        }
    }

    #[test]
    fn journal_files() {
        let files = logbook().journal_files().unwrap();
        assert_eq!(files.len(), 10, "fixtures/README.md: 10 journal days");
        let journals: Vec<Journal> = assert_round_trip(&files);
        for (path, j) in files.iter().zip(&journals) {
            let rel = path.strip_prefix(common::fixture_logbook()).unwrap();
            assert_eq!(Path::new(&Journal::relative_path(j.date)), rel);
        }
    }

    #[test]
    fn decision_files() {
        let files = logbook().decision_files().unwrap();
        assert_eq!(files.len(), 4, "fixtures/README.md: 4 decisions");
        assert_round_trip::<Decision>(&files);
    }

    #[test]
    fn area_and_memory_files() {
        let lb = logbook();
        assert_round_trip::<Area>(&lb.area_files().unwrap());
        assert_round_trip::<Memory>(&lb.memory_files().unwrap());
    }

    #[test]
    fn project_file() {
        let path = common::fixture_logbook().join("PROJECT.md");
        let projects: Vec<Project> = assert_round_trip(&[path]);
        assert_eq!(projects[0].machine_id, "workstation-7f3a");
    }

    #[test]
    fn logbook_toml() {
        let lb = logbook();
        let text = std::fs::read_to_string(common::fixture_logbook().join(".seldon/logbook.toml"))
            .unwrap();
        assert_eq!(lb.meta.to_toml(), text);
        assert_eq!(lb.meta.machine_id, "workstation-7f3a");
    }

    #[test]
    fn every_markdown_file_with_frontmatter_round_trips_losslessly() {
        let mut stack = vec![common::fixture_logbook()];
        let mut seen = 0;
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "md") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    let doc = Document::parse(&text).unwrap();
                    assert_eq!(doc.render(), text, "{}", path.display());
                    seen += 1;
                }
            }
        }
        assert!(seen > 40, "only {seen} markdown files");
    }
}

mod journal_entries {
    use super::*;

    #[test]
    fn fixture_day_has_four_entries() {
        let path = common::fixture_logbook().join("journal/2026/2026-10-01.md");
        let (_, doc) = model::load::<Journal>(&path).unwrap();
        let entries = Journal::entries(&doc.body);
        let headings: Vec<String> = entries.iter().map(|e| e.heading()).collect();
        assert_eq!(
            headings,
            [
                "## 09:25 · agent:claude-code · C-2026-003",
                "## 10:40 · agent:claude-code · C-2026-004",
                "## 14:40 · human",
                "## 17:00 · human · C-2026-004",
            ]
        );
        assert!(entries[2].text.starts_with("Codex hat ollama"));
    }
}

mod lossless_update {
    use super::*;
    use chrono::NaiveDate;
    use seldon::model::CaseStatus;

    #[test]
    fn closing_a_case_changes_two_lines() {
        let path = common::fixture_logbook().join("work/active/C-2026-004-zed.md");
        let text = std::fs::read_to_string(&path).unwrap();
        let (mut case, mut doc) = model::parse::<Case>(&text).unwrap();
        case.status = CaseStatus::Completed;
        case.closed = NaiveDate::from_ymd_opt(2026, 10, 2);
        model::update(&mut doc, &case).unwrap();
        let out = doc.render();
        let changed: Vec<(&str, &str)> = text
            .lines()
            .zip(out.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(
            changed,
            [
                ("status: active", "status: completed"),
                ("closed:", "closed: 2026-10-02")
            ]
        );
        assert_eq!(text.lines().count(), out.lines().count());
    }
}

/// Hand edits and files from other editors (WP-066): a save keeps them,
/// and a save whose frontmatter would not read back is refused.
mod hand_edits {
    use super::*;
    use common::{Env, Snapper, find_file, json, read, stderr};
    use seldon::logbook::cases::CaseFile;
    use seldon::model::CaseStatus;

    const T0: &str = "2026-10-01T10:12:00+02:00";
    const T1: &str = "2026-10-01T11:00:30+02:00";
    const BOM: &str = "\u{feff}";

    /// A fresh logbook with one queued case; its id and path.
    fn new_case(env: &Env, title: &str) -> (PathBuf, String, PathBuf) {
        let root = env.init_logbook();
        let id = add_case(env, title);
        let path = find_file(&root.join("work/queued"), &format!("{id}-"));
        (root, id, path)
    }

    fn add_case(env: &Env, title: &str) -> String {
        let out = env.at(T0, &["plan", "new", "--json", "--", title]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        json(&out)["case"]["id"].as_str().unwrap().to_string()
    }

    /// `text` with `from` replaced once; panics when `from` is not there.
    fn edit(text: &str, from: &str, to: &str) -> String {
        assert!(text.contains(from), "{from:?} not in\n{text}");
        text.replacen(from, to, 1)
    }

    fn run_ok(env: &Env, args: &[&str]) -> serde_json::Value {
        let out = env.at(T1, args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        assert!(!stderr(&out).contains("skipped"), "{}", stderr(&out));
        json(&out)
    }

    const TAGS: &str = "tags:\n# hand-written tags, keep sorted\n  - editor\n\n  - tools\n";

    #[test]
    fn a_comment_a_blank_line_and_a_quoted_key_survive_a_save() {
        let env = Env::new(Snapper::Missing);
        let (root, id, path) = new_case(&env, "Try a font");
        let text = edit(&read(&path), "tags: []\n", TAGS);
        let text = edit(
            &text,
            "title: \"Try a font\"\n",
            "\"title\": \"Try a font\"\n",
        );
        std::fs::write(&path, &text).unwrap();

        let show = run_ok(&env, &["plan", "show", &id, "--json"]);
        assert_eq!(show["case"]["tags"], serde_json::json!(["editor", "tools"]));
        assert_eq!(show["case"]["title"], "Try a font");

        run_ok(&env, &["plan", "start", &id, "--json"]);
        run_ok(
            &env,
            &["log", "--case", &id, "--json", "--", "tried a font"],
        );
        let path = find_file(&root.join("work/active"), &format!("{id}-"));
        let after = read(&path);
        // the property first: the file still parses, with one title
        let (case, doc) = model::parse::<Case>(&after).unwrap_or_else(|e| panic!("{e}\n{after}"));
        let keys: Vec<&str> = doc.frontmatter.as_ref().unwrap().keys().collect();
        assert_eq!(keys.iter().filter(|k| **k == "title").count(), 1, "{after}");
        assert_eq!(case.status, CaseStatus::Active);
        assert_eq!(case.tags, ["editor", "tools"]);
        assert_eq!(case.events.len(), 1);
        // and the hand-written lines are still there, byte for byte
        assert!(after.contains(TAGS), "{after}");
        assert!(after.contains("\n\"title\": \"Try a font\"\n"), "{after}");

        let show = run_ok(&env, &["plan", "show", &id, "--json"]);
        assert_eq!(show["case"]["status"], "active");
    }

    /// The engine changes `agents` when an agent writes a note: the list is
    /// rewritten whole, its hand-written comment stays.
    #[test]
    fn a_hand_edited_list_the_engine_changes_stays_valid() {
        let env = Env::new(Snapper::Missing);
        let (root, id, path) = new_case(&env, "Agents");
        let agents = "agents:\n# who worked here\n\n  - agent:claude-code\n";
        std::fs::write(&path, edit(&read(&path), "agents: []\n", agents)).unwrap();

        run_ok(&env, &["plan", "start", &id, "--json"]);
        let args = [
            "log",
            "--case",
            &id,
            "--actor",
            "agent:codex",
            "--json",
            "--",
            "x",
        ];
        run_ok(&env, &args);
        let after = read(&find_file(&root.join("work/active"), &format!("{id}-")));
        let (case, _) = model::parse::<Case>(&after).unwrap_or_else(|e| panic!("{e}\n{after}"));
        assert_eq!(case.agents, ["agent:claude-code", "agent:codex"]);
        assert!(
            after.contains("agents: [agent:claude-code, agent:codex]\n# who worked here\n"),
            "{after}"
        );
    }

    #[test]
    fn a_bom_and_padded_fences_parse_and_the_bom_stays() {
        let env = Env::new(Snapper::Missing);
        let (root, bom_id, bom_path) = new_case(&env, "BOM case");
        let pad_id = add_case(&env, "Padded case");
        let pad_path = find_file(&root.join("work/queued"), &format!("{pad_id}-"));
        let bom_text = format!("{BOM}{}", read(&bom_path));
        std::fs::write(&bom_path, &bom_text).unwrap();
        let pad_text = edit(&read(&pad_path), "---\n", "--- \n");
        let pad_text = edit(&pad_text, "\n---\n", "\n--- \t\n");
        std::fs::write(&pad_path, &pad_text).unwrap();

        let list = run_ok(&env, &["plan", "list", "--json"]);
        let ids: Vec<&str> = list["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, [bom_id.as_str(), pad_id.as_str()]);
        for id in [&bom_id, &pad_id] {
            run_ok(&env, &["plan", "show", id, "--json"]);
            run_ok(&env, &["plan", "start", id, "--json"]);
        }

        let active = root.join("work/active");
        let bom_after = read(&find_file(&active, &format!("{bom_id}-")));
        assert!(
            bom_after.starts_with(&format!("{BOM}---\n")),
            "{bom_after:?}"
        );
        assert_eq!(bom_after.matches(BOM).count(), 1, "{bom_after:?}");
        let pad_after = read(&find_file(&active, &format!("{pad_id}-")));
        assert!(pad_after.starts_with("--- \n"), "{pad_after:?}");
        assert!(pad_after.contains("\n--- \t\n"), "{pad_after:?}");
        for text in [&bom_after, &pad_after] {
            let (case, _) = model::parse::<Case>(text).unwrap();
            assert_eq!(case.status, CaseStatus::Active);
        }
    }

    /// A column-0 line inside a flow list is valid YAML that the
    /// line-by-line entry model cannot place: changing that list would
    /// leave the old tail behind. The save is refused, the file unchanged.
    #[test]
    fn a_save_that_would_not_read_back_is_refused_and_the_file_is_unchanged() {
        let env = Env::new(Snapper::Missing);
        let (root, _, path) = new_case(&env, "Flow list");
        let text = edit(
            &read(&path),
            "agents: []\n",
            "agents: [agent:claude-code,\nagent:codex]\n",
        );
        std::fs::write(&path, &text).unwrap();
        let logbook = Logbook::open(&root).unwrap();
        let mut file = CaseFile::load(&path).unwrap();
        assert_eq!(file.case.agents, ["agent:claude-code", "agent:codex"]);

        // an unchanged list is not rewritten: the save goes through
        file.save(&logbook).unwrap();
        assert_eq!(read(&path), text);

        file.add_agent("agent:zed");
        let err = file.save(&logbook).unwrap_err().to_string();
        assert!(err.contains("refused"), "{err}");
        assert_eq!(read(&path), text);
    }
}

/// A case save that would be refused (WP-066) fails the command before
/// anything is written: no ledger line, no journal entry, no case change
/// (WP-077). `CaseFile::prepare` runs the save's checks before the ledger
/// write in `log`, `event` and `drift link` (`hook` in `tests/hooks.rs`).
mod refused_saves {
    use super::*;
    use common::{Env, Snapper, copy_dir, fixture_logbook, stderr, tree};

    /// The sample index's clock.
    const AT: &str = "2026-10-01T17:05:12+02:00";
    /// The fixture's active case and the leader of its open 09-30 group.
    const CASE: &str = "C-2026-004";
    const FIREFOX: &str = "01M3SXBQVR7AW8PJQC1YXDCQ14";

    /// Runs `args` on a copy of the fixture logbook whose C-2026-004 has
    /// its `events:` flow list continued at column 0 (valid YAML the line
    /// model cannot change), and asserts exit 1 with the refusal named and
    /// every file of the logbook as it was.
    fn refused(args: &[&str]) {
        let env = Env::new(Snapper::Missing);
        let lb = env.tmp.path().join("logbook");
        copy_dir(&fixture_logbook(), &lb);
        let path = lb.join("work/active/C-2026-004-zed.md");
        let text = std::fs::read_to_string(&path).unwrap();
        let first = "events: [01M3V896207DAXT81MX8G98WZW, ";
        assert!(text.contains(first));
        std::fs::write(
            &path,
            text.replacen(first, "events: [01M3V896207DAXT81MX8G98WZW,\n", 1),
        )
        .unwrap();
        let before = tree(&lb);

        let mut all = vec!["--logbook", lb.to_str().unwrap()];
        all.extend_from_slice(args);
        let out = env.at(AT, &all);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stderr(&out));
        assert!(stderr(&out).contains("update refused"), "{}", stderr(&out));
        let after = tree(&lb);
        for (file, bytes) in &after {
            assert!(
                before.get(file) == Some(bytes),
                "{args:?} wrote {file}:\n{}",
                String::from_utf8_lossy(bytes)
            );
        }
        assert_eq!(before.len(), after.len(), "{args:?} removed a file");
    }

    #[test]
    fn a_note_on_a_case_whose_save_is_refused_writes_nothing() {
        refused(&["log", "--case", CASE, "--", "tried a font"]);
    }

    #[test]
    fn an_event_on_a_case_whose_save_is_refused_writes_nothing() {
        refused(&[
            "event",
            "agent",
            "command",
            "--subject",
            "zed",
            "--actor",
            "agent:codex",
            "--case",
            CASE,
        ]);
    }

    #[test]
    fn a_drift_link_to_a_case_whose_save_is_refused_writes_nothing() {
        refused(&["drift", "link", FIREFOX, CASE]);
    }
}

/// A case id is ASCII only (`C-YYYY-NNN`): a control or format character
/// in the id of a case file is refused on load, so no reader of a loaded
/// case ever prints one (WP-066).
mod case_ids {
    use super::*;

    #[test]
    fn control_characters_in_a_case_id_are_refused_on_load() {
        let path = common::fixture_logbook().join("work/queued/C-2026-005-tokyo-night.md");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(model::parse::<Case>(&text).is_ok());
        for escape in [
            "\\0", "\\a", "\\t", "\\n", "\\r", "\\e", "\\x7f", "\\N", "\\L", "\\P", "\\u200b",
            "\\u202e", "\\ufeff",
        ] {
            for id in [
                format!("C-2026-005{escape}"),
                format!("{escape}C-2026-005"),
                format!("C-2026-{escape}005"),
            ] {
                let bad = text.replacen("id: C-2026-005\n", &format!("id: \"{id}\"\n"), 1);
                assert_ne!(bad, text);
                let err = model::parse::<Case>(&bad)
                    .map(|(c, _)| c.id)
                    .expect_err(&id);
                assert!(err.to_string().starts_with("`id`"), "{id}: {err}");
            }
        }
    }

    /// A refused value is named escaped (`\u{1b}`), never with its control
    /// characters, which would reach the terminal (WP-077).
    #[test]
    fn a_refused_value_is_named_escaped() {
        const ESC: &str = "\\e[31mX";
        let fixture =
            |rel: &str| std::fs::read_to_string(common::fixture_logbook().join(rel)).unwrap();
        let case = fixture("work/queued/C-2026-005-tokyo-night.md");
        let decision = fixture("decisions/ADR-0001-language.md");
        let journal = fixture("journal/2026/2026-10-01.md");
        let area = fixture("areas/dev-env/README.md");
        type Parse = fn(&str) -> Option<String>;
        fn err<R: Record>(text: &str) -> Option<String> {
            model::parse::<R>(text).err().map(|e| e.to_string())
        }
        let checks: [(&str, &str, &str, Parse); 14] = [
            ("case", &case, "id: C-2026-005", err::<Case>),
            ("case", &case, "status: queued", err::<Case>),
            ("case", &case, "zone: yellow", err::<Case>),
            ("case", &case, "risk: R1", err::<Case>),
            ("case", &case, "priority: normal", err::<Case>),
            ("case", &case, "area: themes", err::<Case>),
            ("case", &case, "agents: []", err::<Case>),
            ("case", &case, "events: []", err::<Case>),
            ("decision", &decision, "id: ADR-0001", err::<Decision>),
            ("decision", &decision, "supersedes:", err::<Decision>),
            (
                "decision",
                &decision,
                "cases: [C-2026-001]",
                err::<Decision>,
            ),
            ("decision", &decision, "status: accepted", err::<Decision>),
            (
                "journal",
                &journal,
                "cases: [C-2026-003, C-2026-004]",
                err::<Journal>,
            ),
            ("area", &area, "name: dev-env", err::<Area>),
        ];
        for (what, text, line, parse) in checks {
            let (key, value) = line.split_once(':').unwrap();
            let bad_value = if value.trim_start().starts_with('[') {
                format!("[\"{ESC}\"]")
            } else {
                format!("\"{ESC}\"")
            };
            let bad = text.replacen(
                &format!("\n{line}\n"),
                &format!("\n{key}: {bad_value}\n"),
                1,
            );
            assert_ne!(&bad, text, "{what}: {line}");
            let msg = parse(&bad).unwrap_or_else(|| panic!("{what} {key}: accepted"));
            assert!(!msg.chars().any(char::is_control), "{what} {key}: {msg:?}");
            assert!(msg.contains("\\u{1b}[31mX"), "{what} {key}: {msg:?}");
        }
    }
}
