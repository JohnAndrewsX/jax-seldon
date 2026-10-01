//! Journal appends on real-shaped files: the fixture logbook's days
//! (a copy; `fixtures/logbook/` is read-only), CRLF files, the `plan done`
//! stub (SPEC-LOGBOOK §3).

mod common;

use common::{Env, Snapper, copy_dir, fixture_logbook, ledger, read, stderr};
use seldon::model::{self, Journal};

mod journal {
    use super::*;

    #[test]
    fn appends_to_a_fixture_day() {
        let env = Env::new(Snapper::Missing);
        let root = env.tmp.path().join("fixture");
        copy_dir(&fixture_logbook(), &root);
        let lb = root.to_str().unwrap();
        let path = root.join("journal/2026/2026-10-01.md");
        let original = read(&path);
        let lines_before = ledger(&root).len();

        let out = env.at(
            "2026-10-01T19:30:00+02:00",
            &[
                "--logbook",
                lb,
                "log",
                "--case",
                "C-2026-005",
                "--",
                "Tokyo Night fühlt sich gut an.",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let after = read(&path);
        let expected = original.replace(
            "cases: [C-2026-003, C-2026-004]",
            "cases: [C-2026-003, C-2026-004, C-2026-005]",
        ) + "\n## 19:30 · human · C-2026-005\nTokyo Night fühlt sich gut an.\n";
        assert_eq!(after, expected);

        // an existing case id leaves the frontmatter alone: a pure append
        let out = env.at(
            "2026-10-01T19:45:00+02:00",
            &[
                "--logbook",
                lb,
                "log",
                "--case",
                "C-2026-004",
                "--",
                "noch eins",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let again = read(&path);
        assert!(again.starts_with(&after));
        let (journal, doc) = model::parse::<Journal>(&again).unwrap();
        assert_eq!(journal.cases.len(), 3);
        let entries = Journal::entries(&doc.body);
        assert_eq!(entries.len(), 6, "four fixture entries plus two");
        assert_eq!(entries[5].actor, "human");
        assert_eq!(
            ledger(&root).len(),
            lines_before + 2,
            "the fixture ledger plus two"
        );
    }

    #[test]
    fn crlf_days_stay_crlf() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let path = root.join("journal/2026/2026-10-01.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let day = "---\r\ntype: journal\r\ndate: 2026-10-01\r\ncases: []\r\n---\r\n## 08:00 · human\r\nfrom Windows\r\n";
        std::fs::write(&path, day).unwrap();
        let out = env.at("2026-10-01T09:00:00+02:00", &["log", "--", "from Linux"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(
            read(&path),
            format!("{day}\r\n## 09:00 · human\r\nfrom Linux\r\n")
        );
    }

    #[test]
    fn done_writes_a_stub_in_the_logbook_language() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook_at("de", "de");
        let lb = root.to_str().unwrap();
        let t = "2026-10-05T17:20:00+02:00";
        for args in [
            &["plan", "new", "--", "Monitore"][..],
            &["plan", "start", "C-2026-001"][..],
            &["plan", "verify", "C-2026-001"][..],
            &["plan", "done", "C-2026-001", "--actor", "agent:claude-code"][..],
        ] {
            let mut full = vec!["--logbook", lb];
            full.extend_from_slice(args);
            let out = env.at(t, &full);
            assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        }
        assert_eq!(
            read(&root.join("journal/2026/2026-10-05.md")),
            "---\ntype: journal\ndate: 2026-10-05\ncases: [C-2026-001]\n---\n## 17:20 · agent:claude-code · C-2026-001\nCase abgeschlossen: Monitore\n"
        );
        // the German body template, English section headings (the engine reads them)
        let case = std::fs::read_dir(root.join("work/completed"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "md"))
            .unwrap();
        let text = read(&case);
        assert!(text.contains("<!-- Warum dieser Case?"));
        assert!(text.contains("agents: [agent:claude-code]"));
    }
}
