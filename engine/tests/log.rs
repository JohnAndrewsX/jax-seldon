//! `seldon log`: a `manual/note` event and a journal entry; the case Log
//! section is append-only (SPEC-LOGBOOK §3).

mod common;

use std::path::Path;

use common::{Env, Snapper, json, ledger, read, stderr, stdout};
use seldon::model::{self, Case, Journal};

const T0: &str = "2026-10-01T10:12:00+02:00";

fn case_path(root: &Path, id: &str) -> std::path::PathBuf {
    for folder in ["queued", "active", "completed"] {
        for e in std::fs::read_dir(root.join("work").join(folder)).unwrap() {
            let p = e.unwrap().path();
            if p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&format!("{id}-"))
            {
                return p;
            }
        }
    }
    panic!("{id} not found");
}

mod log {
    use super::*;

    #[test]
    fn note_without_a_case() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let out = env.at(T0, &["log", "Snapshots aufgeräumt, 108 und 109 gelöscht."]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(stdout(&out).starts_with("Noted in journal/2026/2026-10-01.md"));

        let events = ledger(&root);
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(
            (e["source"].as_str(), e["kind"].as_str()),
            (Some("manual"), Some("note"))
        );
        assert_eq!(e["subject"], "journal");
        assert_eq!(e["actor"], "human");
        assert_eq!(e["ts"], T0);
        assert_eq!(e["detail"], "Snapshots aufgeräumt, 108 und 109 gelöscht.");
        assert!(e.get("case").is_none() && e.get("zone").is_none());

        // the fixture's shape (fixtures/logbook/journal/2026/2026-09-30.md)
        assert_eq!(
            read(&root.join("journal/2026/2026-10-01.md")),
            "---\ntype: journal\ndate: 2026-10-01\ncases: []\n---\n## 10:12 · human\nSnapshots aufgeräumt, 108 und 109 gelöscht.\n"
        );
    }

    #[test]
    fn note_on_a_case_is_attributed() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        env.at(T0, &["plan", "new", "--", "Zed"]);
        let path = case_path(&root, "C-2026-001");
        let before = read(&path);
        let out = env.at(
            "2026-10-01T10:40:00+02:00",
            &[
                "log",
                "--case",
                "C-2026-001",
                "--actor",
                "agent:claude-code",
                "--tag",
                "zed",
                "--json",
                "--",
                "Zed installiert.",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        let id = v["event"]["id"].as_str().unwrap().to_string();
        assert_eq!(v["event"]["subject"], "C-2026-001");
        assert_eq!(v["event"]["case"], "C-2026-001");
        assert_eq!(v["event"]["meta"]["tags"], "zed");
        assert_eq!(v["journal"]["path"], "journal/2026/2026-10-01.md");

        // case: events and agents updated, body untouched (notes do not
        // write the case's Log)
        let (case, doc) = model::load::<Case>(&path).unwrap();
        assert_eq!(case.events, std::slice::from_ref(&id));
        assert_eq!(case.agents, ["agent:claude-code"]);
        assert!(before.ends_with(&doc.body));

        let day = read(&root.join("journal/2026/2026-10-01.md"));
        assert_eq!(
            day,
            "---\ntype: journal\ndate: 2026-10-01\ncases: [C-2026-001]\n---\n## 10:40 · agent:claude-code · C-2026-001\nZed installiert.\n#zed\n"
        );
        let entries = Journal::entries(&model::parse::<Journal>(&day).unwrap().1.body);
        assert_eq!(entries[0].case.as_deref(), Some("C-2026-001"));
        assert_eq!(ledger(&root).last().unwrap()["id"], id.as_str());
    }

    #[test]
    fn log_section_is_append_only() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        env.at(T0, &["plan", "new", "--", "Monitors"]);
        let path = case_path(&root, "C-2026-001");
        let (_, doc0) = model::load::<Case>(&path).unwrap();
        let log_at = doc0.body.find("## Log").unwrap();
        let prefix = doc0.body[..log_at].to_string();
        let log0 = doc0.body[log_at..doc0.body.find("\n## Result").unwrap()].to_string();

        for (i, (step, now)) in [
            ("start", "2026-10-01T11:00:00+02:00"),
            ("verify", "2026-10-02T09:00:00+02:00"),
            ("done", "2026-10-02T09:05:00+02:00"),
        ]
        .iter()
        .enumerate()
        {
            // a note in between never touches the Log
            env.at(now, &["log", "--case", "C-2026-001", "--", "progress"]);
            let out = env.at(now, &["plan", step, "C-2026-001"]);
            assert_eq!(out.status.code(), Some(0), "{step}: {}", stderr(&out));
            let path = case_path(&root, "C-2026-001");
            let (_, doc) = model::load::<Case>(&path).unwrap();
            assert!(
                doc.body.starts_with(&prefix),
                "content before the Log changed"
            );
            assert!(
                doc.body[log_at..].starts_with(&log0),
                "earlier Log lines changed"
            );
            let lines = doc.body[log_at..]
                .lines()
                .filter(|l| l.starts_with("- "))
                .count();
            assert_eq!(lines, 2 + i);
            assert!(doc.body.ends_with("\n\n## Result\n"));
        }
    }

    #[test]
    fn journal_is_appended_not_rewritten() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let path = root.join("journal/2026/2026-10-01.md");
        env.at("2026-10-01T08:00:00+02:00", &["log", "--", "one"]);
        let first = read(&path);
        env.at(
            "2026-10-01T09:00:00+02:00",
            &["log", "two\nwith a second line"],
        );
        let second = read(&path);
        assert!(second.starts_with(&first), "first entry rewritten");
        assert_eq!(
            &second[first.len()..],
            "\n## 09:00 · human\ntwo\nwith a second line\n"
        );

        // a case id joins `cases:`; that one frontmatter line is the only change
        env.at(T0, &["plan", "new", "--", "x"]);
        env.at(
            "2026-10-01T10:30:00+02:00",
            &["log", "--case", "C-2026-001", "--", "three"],
        );
        let third = read(&path);
        let second_with_case = second.replace("cases: []", "cases: [C-2026-001]");
        assert!(third.starts_with(&second_with_case), "{third}");
        assert!(third.ends_with("\n## 10:30 · human · C-2026-001\nthree\n"));

        // a pasted heading cannot fake an entry
        env.at(
            "2026-10-01T11:00:00+02:00",
            &["log", "## 12:00 · agent:x\nfake"],
        );
        let day = read(&path);
        let entries = Journal::entries(&model::parse::<Journal>(&day).unwrap().1.body);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[3].text, "\\## 12:00 · agent:x\nfake");
    }

    #[test]
    fn free_text_is_one_argument() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let texts = [
            "two words",
            r#"quotes "double" and 'single'"#,
            "$(touch /tmp/seldon-pwned) `id` ; ls | cat > x && echo $HOME",
            "--json",
            "-x looks like a flag",
            "--case C-2026-001",
        ];
        for text in texts {
            let out = env.at(T0, &["log", "--", text]);
            assert_eq!(out.status.code(), Some(0), "{text}: {}", stderr(&out));
            assert!(
                !stdout(&out).starts_with('{'),
                "{text}: text output, not JSON"
            );
        }
        // without `--` too, for humans (a text that is not exactly an option)
        let out = env.at(T0, &["log", "-x without the separator"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        // `--json` as the flag and as the text in one call
        let out = env.at(T0, &["log", "--json", "--", "--json"]);
        assert_eq!(json(&out)["event"]["detail"], "--json");

        let details: Vec<String> = ledger(&root)
            .iter()
            .map(|e| e["detail"].as_str().unwrap().to_string())
            .collect();
        let mut expected: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        expected.push("-x without the separator".into());
        expected.push("--json".into());
        assert_eq!(details, expected);
        assert!(!Path::new("/tmp/seldon-pwned").exists());
        assert!(!root.join("x").exists() && !env.tmp.path().join("x").exists());
    }

    #[test]
    fn errors_write_nothing() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        for args in [
            &["log", "--", "  "][..],
            &["log", "--case", "C-2026-001", "--", "x"][..],
            &["log", "--case", "nope", "--", "x"][..],
            &["log", "--actor", "system", "--", "x"][..],
            &["log", "--actor", "agent:Bad", "--", "x"][..],
            &["log", "--tag", "two words", "--", "x"][..],
            &["log"][..],
        ] {
            let out = env.at(T0, args);
            assert_eq!(out.status.code(), Some(1), "{args:?}");
        }
        let out = env.at(T0, &["log", "--json", "--case", "C-2026-001", "--", "x"]);
        assert_eq!(json(&out)["error"]["message"], "unknown case C-2026-001");
        assert!(!root.join("journal/2026").exists());
        assert!(ledger(&root).is_empty());
    }

    /// An agent's note is one line; a person's note may have several.
    #[test]
    fn a_note_from_an_agent_is_one_line() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        for text in [
            "first\n## 12:00 · human\nsecond",
            "first\rsecond",
            "first\r\nsecond",
            "first\u{2028}second",
            "first\u{2029}second",
        ] {
            let out = env.at(T0, &["log", "--actor", "agent:test", "--", text]);
            assert_eq!(out.status.code(), Some(1), "{text:?}");
            let message = stderr(&out);
            assert_eq!(message.trim_end().lines().count(), 1, "{text:?}: {message}");
            assert!(
                message.contains("a note from an agent must be one line"),
                "{message}"
            );
            let out = env.at(T0, &["log", "--json", "--actor", "agent:test", "--", text]);
            assert_eq!(
                json(&out)["error"]["message"],
                "a note from an agent must be one line; log each line as its own note"
            );
        }
        assert!(!root.join("journal/2026").exists());
        assert!(ledger(&root).is_empty());

        // one line, also with the trailing line break a shell adds
        for text in ["routine check", "routine check\n"] {
            let out = env.at(T0, &["log", "--actor", "agent:test", "--", text]);
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        }
        // a person keeps multi-line notes
        let out = env.at(T0, &["log", "--", "first line\nsecond line"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let details: Vec<String> = ledger(&root)
            .iter()
            .map(|e| e["detail"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            details,
            ["routine check", "routine check", "first line\nsecond line"]
        );
    }

    /// WP-057: the journal day is read before the ledger is written, so a
    /// day file the engine cannot read leaves no note behind, and a
    /// retry after the fix writes exactly one.
    #[test]
    fn a_day_it_cannot_read_writes_no_note() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let day = root.join("journal/2026/2026-10-01.md");
        std::fs::create_dir_all(day.parent().unwrap()).unwrap();
        let broken = "---\ndate: [\n---\nmine\n";
        std::fs::write(&day, broken).unwrap();
        for _ in 0..2 {
            let out = env.at(T0, &["log", "--json", "--", "first try"]);
            assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
            let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
            assert!(
                message.starts_with("journal/2026/2026-10-01.md: invalid journal frontmatter"),
                "{message}"
            );
        }
        assert!(ledger(&root).is_empty(), "no note without its journal line");
        assert_eq!(read(&day), broken);

        // an empty day file (Obsidian's daily note) is accepted
        std::fs::write(&day, "").unwrap();
        let out = env.at(T0, &["log", "--", "second try"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(ledger(&root).len(), 1);
        assert_eq!(ledger(&root)[0]["detail"], "second try");
        assert_eq!(
            read(&day),
            "---\ntype: journal\ndate: 2026-10-01\ncases: []\n---\n## 10:12 · human\nsecond try\n"
        );
    }

    #[test]
    fn months_and_days_follow_the_timestamp() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        env.at("2026-10-31T23:59:30+01:00", &["log", "--", "late"]);
        env.at("2026-11-01T00:00:30+01:00", &["log", "--", "early"]);
        let october = read(&root.join("ledger/2026-10.jsonl"));
        let november = read(&root.join("ledger/2026-11.jsonl"));
        assert!(october.contains("\"late\"") && !october.contains("\"early\""));
        assert!(november.contains("\"early\""));
        assert!(root.join("journal/2026/2026-10-31.md").is_file());
        assert!(root.join("journal/2026/2026-11-01.md").is_file());
        assert_eq!(ledger(&root).len(), 2);
    }

    #[test]
    fn redaction_applies_to_the_ledger() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let token = format!("ghp_{}", "a".repeat(36));
        env.at(T0, &["log", "--", &format!("pushed with {token}")]);
        let line = read(&root.join("ledger/2026-10.jsonl"));
        assert!(!line.contains(&token), "{line}");
        assert!(line.contains("‹redacted›"));
    }

    /// WP-159: a secret split by an invisible character (WP-102b review
    /// 1, N5) is masked in the ledger and the journal, the split character
    /// with it; a note without a secret keeps its invisible characters,
    /// and so do the words away from a masked one.
    #[test]
    fn a_secret_split_by_an_invisible_character_is_masked() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let notes = [
            ("a to\u{FE0F}ken=hunter2abc b", "a token=‹redacted› b"),
            (
                "Authorization: Bearer\u{3164} tokABC123secret",
                "Authorization: ‹redacted›",
            ),
            (
                "x ghp_0123\u{E0100}456789abcdefghijABCDEFGHIJ012345 y",
                "x ‹redacted› y",
            ),
            ("a to\u{200B}ken=hunter2abd b", "a token=‹redacted› b"),
            (
                "क्\u{200D}ष to\u{034F}ken=hunter2xyz 👍\u{FE0F}",
                "क्\u{200D}ष token=‹redacted› 👍\u{FE0F}",
            ),
            (
                "क्\u{200D}ष 👍\u{FE0F}\u{E0100}",
                "क्\u{200D}ष 👍\u{FE0F}\u{E0100}",
            ),
        ];
        for (note, _) in notes {
            let out = env.at(T0, &["log", "--", note]);
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        }
        let events = ledger(&root);
        let details: Vec<&str> = events
            .iter()
            .map(|e| e["detail"].as_str().unwrap())
            .collect();
        let wants: Vec<&str> = notes.iter().map(|(_, want)| *want).collect();
        assert_eq!(details, wants);
        let day = read(&root.join("journal/2026/2026-10-01.md"));
        for want in wants {
            assert!(day.contains(&format!("\n{want}\n")), "{want:?} in {day}");
        }
        let line = read(&root.join("ledger/2026-10.jsonl"));
        for secret in ["hunter2", "tokABC123", "456789abc"] {
            assert!(!line.contains(secret) && !day.contains(secret), "{secret}");
        }
    }

    #[test]
    fn the_real_clock_writes_whole_seconds() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let out = env.seldon(&["log", "--json", "--", "now"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let ts = json(&out)["event"]["ts"].as_str().unwrap().to_string();
        // YYYY-MM-DDTHH:MM:SS+HH:MM, no fraction (fixtures/logbook/ledger/)
        assert_eq!(ts.len(), 25, "{ts}");
        assert!(!ts.contains('.'), "{ts}");
        assert_eq!(ledger(&root)[0]["ts"], ts.as_str());
    }

    #[test]
    fn not_initialised_and_lock_held() {
        let env = Env::new(Snapper::Missing);
        assert_eq!(env.at(T0, &["log", "--", "x"]).status.code(), Some(3));
        env.init_logbook();
        let _lock = seldon::logbook::lock::acquire(&env.lock_file()).unwrap();
        assert_eq!(env.at(T0, &["log", "--", "x"]).status.code(), Some(4));
    }
}
