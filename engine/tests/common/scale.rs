//! A logbook scaled up from `fixtures/logbook/` for timing the index build
//! (SPEC-ENGINE §6, WP-007). Copy `k` of the fixture (k = 1…factor-1) gets
//! fresh event ids (same time part), its own case ids
//! (`C-2026-003` → `C-2026-k003`), its own transactions and, for k < 10,
//! its own decisions (`ADR-0004` → `ADR-k004`). Journals, the dossier and
//! memory stay single: the index reads only two journal days anyway.
//!
//! Used by `tests/index.rs` and `benches/index.rs` (`#[path]` include), so
//! it depends on nothing but the crate's own dependencies.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use regex::Regex;
use ulid::Ulid;

/// Copies `fixture` to `dst` and adds `factor - 1` renamed copies of its
/// ledger events, cases and decisions. Returns the number of ledger lines.
pub fn scaled_logbook(fixture: &Path, dst: &Path, factor: usize) -> usize {
    scaled_logbook_with(fixture, dst, factor, factor)
}

/// The scale SPEC-ENGINE §6 states for its budget (WP-076): the ledger
/// ×141 (10 011 lines), the cases ×38 (304, a copy's events name the
/// cases of copies 38 and later, which do not exist) and a journal file
/// for each of the 365 days up to 2026-10-01. Returns the ledger lines.
pub fn stated_scale(fixture: &Path, dst: &Path) -> usize {
    let lines = scaled_logbook_with(fixture, dst, 141, 38);
    journal_year(dst, chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
    lines
}

/// [`scaled_logbook`] with only `case_factor` copies of the cases.
pub fn scaled_logbook_with(fixture: &Path, dst: &Path, factor: usize, case_factor: usize) -> usize {
    copy_tree(fixture, dst);
    let ulid_re = Regex::new(r#""([0-7][0-9A-HJKMNP-TV-Z]{25})""#).unwrap();
    let case_re = Regex::new(r"C-2026-([0-9]{3})\b").unwrap();
    let adr_re = Regex::new(r"ADR-0([0-9]{3})\b").unwrap();

    let ledger = dst.join("ledger");
    let mut months: Vec<_> = std::fs::read_dir(&ledger)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .collect();
    months.sort();
    let mut lines = 0;
    for month in &months {
        let original = std::fs::read_to_string(month).unwrap();
        let mut text = original.clone();
        for k in 1..factor {
            let mut ids: HashMap<String, String> = HashMap::new();
            for line in original.lines() {
                let line = ulid_re.replace_all(line, |c: &regex::Captures| {
                    let new = ids
                        .entry(c[1].to_string())
                        .or_insert_with(|| renamed_id(&c[1], k))
                        .clone();
                    format!("\"{new}\"")
                });
                let line = case_re
                    .replace_all(&line, |c: &regex::Captures| format!("C-2026-{k}{}", &c[1]));
                let line = line.replace("\"tx-", &format!("\"tx-k{k}-"));
                text.push_str(&line);
                text.push('\n');
            }
        }
        lines += text.lines().count();
        std::fs::write(month, text).unwrap();
    }

    for folder in ["queued", "active", "completed"] {
        let dir = dst.join("work").join(folder);
        let files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "md"))
            .collect();
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            for k in 1..case_factor {
                let rename = |s: &str| {
                    let s = case_re
                        .replace_all(s, |c: &regex::Captures| format!("C-2026-{k}{}", &c[1]));
                    ulid_like(&s, k)
                };
                std::fs::write(dir.join(rename(&name)), rename(&text)).unwrap();
            }
        }
    }

    let decisions = dst.join("decisions");
    let files: Vec<_> = std::fs::read_dir(&decisions)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        for k in 1..factor.min(10) {
            let rename = |s: &str| {
                adr_re
                    .replace_all(s, |c: &regex::Captures| format!("ADR-{k}{}", &c[1]))
                    .into_owned()
            };
            std::fs::write(decisions.join(rename(&name)), rename(&text)).unwrap();
        }
    }
    lines
}

/// A made-up journal file `journal/YYYY/YYYY-MM-DD.md` for each of the
/// 365 days up to `last` that has none.
fn journal_year(root: &Path, last: chrono::NaiveDate) {
    for back in 0..365 {
        let day = last - chrono::Duration::days(back);
        let year = day.format("%Y").to_string();
        let path = root.join(format!("journal/{year}/{day}.md"));
        if path.exists() {
            continue;
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = format!(
            "---\ntype: journal\ndate: {day}\ncases: []\n---\n## 08:00 · human\nFiller entry of {day}.\n"
        );
        std::fs::write(path, text).unwrap();
    }
}

/// The id of copy `k` of `id`: same time part, a different random part.
fn renamed_id(id: &str, k: usize) -> String {
    let u = Ulid::from_string(id).unwrap();
    Ulid::from_parts(
        u.timestamp_ms(),
        u.random() ^ (k as u128 * 0x9E37_79B9_7F4A_7C15),
    )
    .to_string()
}

/// Renames bare ULIDs (case `events: [...]` lists) like [`renamed_id`].
fn ulid_like(text: &str, k: usize) -> String {
    let re = Regex::new(r"\b([0-7][0-9A-HJKMNP-TV-Z]{25})\b").unwrap();
    re.replace_all(text, |c: &regex::Captures| renamed_id(&c[1], k))
        .into_owned()
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Appends `n` made-up journal notes, one a minute from 2026-09-01 00:00
/// (+02:00) on, to `<root>/ledger/2026-09.jsonl`: a ledger of that many
/// lines for the hook budget (SPEC-ENGINE §1, WP-076). Every note lies
/// before the 2026-10-01 clock of the tests.
pub fn filler_notes(root: &Path, n: usize) {
    use std::fmt::Write as _;
    let start = chrono::DateTime::parse_from_rfc3339("2026-09-01T00:00:00+02:00").unwrap();
    let mut text = String::new();
    for i in 0..n {
        let ts = start + chrono::Duration::minutes(i as i64);
        let id = Ulid::from_parts(ts.timestamp_millis() as u64, i as u128 + 1);
        let _ = writeln!(
            text,
            r#"{{"id":"{id}","ts":"{}","source":"manual","kind":"note","subject":"journal","detail":"Filler note {i}.","actor":"human"}}"#,
            ts.format("%Y-%m-%dT%H:%M:%S%:z"),
        );
    }
    let path = root.join("ledger/2026-09.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    std::io::Write::write_all(&mut file, text.as_bytes()).unwrap();
}
