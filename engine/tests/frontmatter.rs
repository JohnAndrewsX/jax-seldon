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
            model::update(&mut doc2, &record);
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
        model::update(&mut doc, &case);
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
