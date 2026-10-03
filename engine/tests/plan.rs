//! `seldon plan`: the case state machine, folder moves, frontmatter, the
//! Log section, `.seldon/active-case` (SPEC-LOGBOOK §3, ADR-0012 §9).

mod common;

use std::path::{Path, PathBuf};

use common::{
    Env, Snapper, assert_valid_case, copy_dir, find_file, fixture_logbook, json, ledger, read,
    stderr, stdout,
};
use seldon::model::{self, Case, CaseStatus};

const T0: &str = "2026-10-01T10:12:00+02:00";
const T1: &str = "2026-10-01T11:00:30+02:00";
const T2: &str = "2026-10-02T09:30:00+02:00";
const T3: &str = "2026-10-03T18:45:10+02:00";

fn case_at(root: &Path, id: &str) -> (PathBuf, Case, String) {
    for folder in ["queued", "active", "completed"] {
        let dir = root.join("work").join(folder);
        if let Some(path) = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(&format!("{id}-"))
            })
        {
            let (case, doc) = model::load::<Case>(&path).unwrap();
            return (path, case, doc.body);
        }
    }
    panic!("{id} not found");
}

fn new_case(env: &Env, title: &str, extra: &[&str]) -> String {
    let mut args = vec!["plan", "new", "--json"];
    args.extend_from_slice(extra);
    args.extend(["--", title]);
    let out = env.at(T0, &args);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    json(&out)["case"]["id"].as_str().unwrap().to_string()
}

/// Everything of the body before `## Log`, and from `## Result` on.
fn outside_log(body: &str) -> (String, String) {
    let log = body.find("## Log").expect("a Log section");
    let result = body.rfind("## Result").expect("a Result section");
    (body[..log].to_string(), body[result..].to_string())
}

fn log_lines(body: &str) -> Vec<String> {
    let start = body.find("## Log").unwrap();
    let end = body.rfind("## Result").unwrap();
    body[start..end]
        .lines()
        .filter(|l| l.starts_with("- "))
        .map(String::from)
        .collect()
}

mod plan {
    use super::*;

    #[test]
    fn new_creates_a_queued_case_from_the_template() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let title = r#"Install "zed" -- --json $(touch /tmp/x); `ls` 'q'"#;
        let out = env.at(
            T0,
            &[
                "plan", "new", "--zone", "red", "--risk", "R2", "--area", "editors", "--json",
                "--", title,
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(v["case"]["id"], "C-2026-001");
        assert_eq!(v["case"]["status"], "queued");
        assert_eq!(v["areaCreated"], "areas/editors/README.md");
        assert_valid_case(&v["case"]);

        let (path, case, body) = case_at(&root, "C-2026-001");
        assert_eq!(path.parent().unwrap(), root.join("work/queued"));
        assert_eq!(
            path.file_name().unwrap(),
            "C-2026-001-install-zed-json-touch-tmp-x-ls-q.md"
        );
        assert_eq!(case.title, title, "free text arrives as one argument");
        assert_eq!(case.status, CaseStatus::Queued);
        assert_eq!(case.created.to_string(), "2026-10-01");
        assert_eq!(
            (case.started, case.closed, case.snapshot_before),
            (None, None, None)
        );
        assert!(body.starts_with(&format!("# C-2026-001 — {title}\n\n## Intent\n")));
        assert_eq!(
            log_lines(&body),
            ["- 2026-10-01 10:12 · created (zone red, risk R2) · human"]
        );
        // the canonical frontmatter of fixtures/logbook/
        let text = read(&path);
        assert!(text.starts_with("---\nid: C-2026-001\ntype: case\ntitle: \""));
        assert!(text.contains("\nstatus: queued\nzone: red\nrisk: R2\npriority: normal\narea: editors\ncreated: 2026-10-01\nstarted:\nclosed:\nsnapshotBefore:\nagents: []\nevents: []\ntags: []\n---\n"));
        let (area, _) = model::load::<model::Area>(&root.join("areas/editors/README.md")).unwrap();
        assert_eq!(area.name, "editors");

        let events = ledger(&root);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["source"], "seldon");
        assert_eq!(events[0]["kind"], "case-created");
        assert_eq!(events[0]["subject"], "C-2026-001");
        assert_eq!(events[0]["case"], "C-2026-001");
        assert_eq!(events[0]["detail"], title);
        assert_eq!(events[0]["ts"], T0);
        assert!(events[0].get("zone").is_none());
    }

    #[test]
    fn title_without_double_dash_and_defaults() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let out = env.at(T0, &["plan", "new", "Snapper retention", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let (_, case, _) = case_at(&root, "C-2026-001");
        assert_eq!(case.title, "Snapper retention");
        assert_eq!(
            (case.zone.as_str(), case.risk.as_str(), case.area),
            ("yellow", "R1", None)
        );
        // a title starting with a hyphen is still the title
        let out = env.at(T0, &["plan", "new", "-x marks the spot"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(case_at(&root, "C-2026-002").1.title, "-x marks the spot");
    }

    #[test]
    fn ids_are_never_reused() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        assert_eq!(new_case(&env, "one", &[]), "C-2026-001");
        assert_eq!(new_case(&env, "two", &[]), "C-2026-002");
        let out = env.at(
            T1,
            &["plan", "drop", "C-2026-002", "--reason", "not needed"],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        // a workpiece folder holds an id too
        std::fs::create_dir_all(root.join("work/C-2026-007")).unwrap();
        assert_eq!(new_case(&env, "three", &[]), "C-2026-008");
        // a new year starts at 001
        let out = env.at(
            "2027-01-01T08:00:00+01:00",
            &["plan", "new", "--json", "--", "y"],
        );
        assert_eq!(json(&out)["case"]["id"], "C-2027-001");
    }

    #[test]
    fn lifecycle_moves_files_and_keeps_the_body() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let id = new_case(&env, "Zed", &["--zone", "red", "--risk", "R2"]);
        // the user writes into the case
        let (path, _, _) = case_at(&root, &id);
        let user_text = read(&path)
            .replace(
                "## Intent\n",
                "## Intent\nA second editor.\n\n```\n## Result inside a fence\n```\n",
            )
            .replace(
                "- Steps:\n",
                "- Steps:\n  - [x] install\n  - [ ] keybinding\n",
            );
        std::fs::write(&path, &user_text).unwrap();
        let (_, _, body0) = case_at(&root, &id);
        let (before0, after0) = outside_log(&body0);

        let out = env.at(T1, &["plan", "start", &id, "--snapshot", "112", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(
            (v["from"].as_str(), v["to"].as_str()),
            (Some("queued"), Some("active"))
        );
        assert_eq!(
            v["movedFrom"],
            format!(
                "work/queued/{}",
                path.file_name().unwrap().to_string_lossy()
            )
        );
        assert_eq!(
            v["case"]["steps"],
            serde_json::json!({ "total": 2, "done": 1 })
        );
        assert_valid_case(&v["case"]);
        let (p, case, _) = case_at(&root, &id);
        assert_eq!(p.parent().unwrap(), root.join("work/active"));
        assert!(!path.exists(), "moved, not copied");
        assert_eq!(case.status, CaseStatus::Active);
        assert_eq!(case.started.unwrap().to_string(), "2026-10-01");
        assert_eq!(case.snapshot_before, Some(112));
        assert_eq!(read(&root.join(".seldon/active-case")), format!("{id}\n"));

        let out = env.at(T2, &["plan", "verify", &id]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let (p, case, _) = case_at(&root, &id);
        assert_eq!(p.parent().unwrap(), root.join("work/active"));
        assert_eq!(case.status, CaseStatus::Verification);
        assert!(
            root.join(".seldon/active-case").exists(),
            "kept during verification"
        );

        let out = env.at(T3, &["plan", "done", &id, "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(json(&out)["activeCase"]["cleared"], true);
        let (p, case, body3) = case_at(&root, &id);
        assert_eq!(p.parent().unwrap(), root.join("work/completed"));
        assert_eq!(case.status, CaseStatus::Completed);
        assert_eq!(case.closed.unwrap().to_string(), "2026-10-03");
        assert_eq!(case.started.unwrap().to_string(), "2026-10-01");
        assert!(!root.join(".seldon/active-case").exists());

        // body: byte-identical outside the Log; the Log only grew
        assert_eq!(outside_log(&body3), (before0, after0));
        assert_eq!(
            log_lines(&body3),
            [
                "- 2026-10-01 10:12 · created (zone red, risk R2) · human",
                "- 2026-10-01 11:00 · started (snapshot 112) · human",
                "- 2026-10-02 09:30 · verification · human",
                "- 2026-10-03 18:45 · completed · human",
            ]
        );

        // frontmatter: only the changed keys differ from the user's file
        let before: Vec<&str> = user_text
            .lines()
            .take_while(|l| !l.starts_with("# "))
            .collect();
        let final_text = read(&p);
        let after: Vec<&str> = final_text
            .lines()
            .take_while(|l| !l.starts_with("# "))
            .collect();
        let changed: Vec<(&str, &str)> = before
            .iter()
            .zip(&after)
            .filter(|(a, b)| a != b)
            .map(|(a, b)| (*a, *b))
            .collect();
        assert_eq!(
            changed,
            [
                ("status: queued", "status: completed"),
                ("started:", "started: 2026-10-01"),
                ("closed:", "closed: 2026-10-03"),
                ("snapshotBefore:", "snapshotBefore: 112"),
            ]
        );

        // the journal stub on done
        let day = read(&root.join("journal/2026/2026-10-03.md"));
        assert!(day.contains("cases: [C-2026-001]"), "{day}");
        assert!(
            day.ends_with("## 18:45 · human · C-2026-001\nCase completed: Zed\n"),
            "{day}"
        );

        let kinds: Vec<String> = ledger(&root)
            .iter()
            .map(|e| e["kind"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            kinds,
            [
                "case-created",
                "case-started",
                "case-verified",
                "case-completed"
            ]
        );
    }

    #[test]
    fn invalid_transitions_are_user_errors_and_change_nothing() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let queued = new_case(&env, "queued", &[]);
        let active = new_case(&env, "active", &[]);
        let done = new_case(&env, "done", &[]);
        for (step, id) in [
            ("start", &active),
            ("start", &done),
            ("verify", &done),
            ("done", &done),
        ] {
            assert_eq!(env.at(T1, &["plan", step, id]).status.code(), Some(0));
        }
        let snapshot = |root: &Path| {
            let mut files = Vec::new();
            for f in ["queued", "active", "completed"] {
                for e in std::fs::read_dir(root.join("work").join(f)).unwrap() {
                    let p = e.unwrap().path();
                    files.push((p.clone(), std::fs::read(&p).unwrap()));
                }
            }
            files.sort();
            (files, ledger(root).len())
        };
        let before = snapshot(&root);
        for (step, id, from) in [
            ("verify", &queued, "queued"),
            ("done", &queued, "queued"),
            ("start", &active, "active"),
            ("done", &active, "active"),
            ("start", &done, "completed"),
            ("verify", &done, "completed"),
            ("done", &done, "completed"),
            ("drop", &done, "completed"),
        ] {
            let out = env.at(T2, &["plan", step, id, "--json"]);
            assert_eq!(out.status.code(), Some(1), "{step} {id}");
            let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
            assert!(
                message.starts_with(&format!("{id} is {from}; `seldon plan {step}`")),
                "{message}"
            );
            let out = env.at(T2, &["plan", step, id]);
            assert_eq!(out.status.code(), Some(1));
            assert!(
                stderr(&out).contains("needs a case that is"),
                "{}",
                stderr(&out)
            );
        }
        assert!(stderr(&env.at(T2, &["plan", "done", &active])).contains("seldon plan verify"));
        assert_eq!(snapshot(&root), before, "a refused step writes nothing");
    }

    #[test]
    fn drop_from_every_open_state() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let a = new_case(&env, "a", &[]);
        let b = new_case(&env, "b", &[]);
        let c = new_case(&env, "c", &[]);
        env.at(T1, &["plan", "start", &b]);
        env.at(T1, &["plan", "start", &c]);
        env.at(T1, &["plan", "verify", &c]);
        // the marker names c (started last); dropping b leaves it
        let out = env.at(
            T2,
            &[
                "plan",
                "drop",
                &b,
                "--json",
                "--reason",
                "--json is fine here",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(json(&out)["activeCase"]["cleared"], false);
        assert_eq!(read(&root.join(".seldon/active-case")), format!("{c}\n"));
        for id in [&a, &c] {
            assert_eq!(env.at(T2, &["plan", "drop", id]).status.code(), Some(0));
        }
        assert!(!root.join(".seldon/active-case").exists());
        for id in [&a, &b, &c] {
            let (p, case, _) = case_at(&root, id);
            assert_eq!(
                p.parent().unwrap(),
                root.join("work/completed"),
                "ADR-0012 §9"
            );
            assert_eq!(case.status, CaseStatus::Dropped);
            assert_eq!(case.closed.unwrap().to_string(), "2026-10-02");
        }
        let (_, _, body) = case_at(&root, &b);
        assert_eq!(
            log_lines(&body).last().unwrap(),
            "- 2026-10-02 09:30 · dropped: --json is fine here · human"
        );
        let dropped: Vec<serde_json::Value> = ledger(&root)
            .into_iter()
            .filter(|e| e["kind"] == "case-dropped")
            .collect();
        assert_eq!(dropped.len(), 3);
        assert_eq!(dropped[0]["detail"], "--json is fine here");
    }

    #[test]
    fn argument_errors() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let id = new_case(&env, "x", &[]);
        for args in [
            &["plan", "verify", id.as_str(), "--snapshot", "1"][..],
            &["plan", "start", "C-2026-099"][..],
            &["plan", "start", "c-2026-001"][..],
            &["plan", "start", "C-26-1"][..],
            &["plan", "new", "--zone", "purple", "--", "t"][..],
            &["plan", "new", "--risk", "R9", "--", "t"][..],
            &["plan", "new", "--area", "Bad Area", "--", "t"][..],
            &["plan", "new", "--", "  "][..],
            &["plan", "new", "--", "two\nlines"][..],
            &["plan", "start", id.as_str(), "--actor", "system"][..],
            &["plan", "start", id.as_str(), "--snapshot", "-1"][..],
        ] {
            let out = env.at(T1, args);
            assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stderr(&out));
        }
        assert_eq!(ledger(&root).len(), 1, "only the first case-created");
        assert!(!root.join("areas/bad area").exists());
    }

    /// ADR-0023: R2/R3 without a snapshot warn on `plan start`, never refuse.
    #[test]
    fn start_warns_on_r2_and_r3_without_a_snapshot() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let start = |id: &str, extra: &[&str]| {
            let mut args = vec!["plan", "start", id, "--json"];
            args.extend_from_slice(extra);
            let out = env.at(T1, &args);
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
            let v = json(&out);
            assert_eq!(v["to"], "active", "never refused");
            v["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|w| w.as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        for risk in ["R0", "R1"] {
            let id = new_case(&env, risk, &["--risk", risk]);
            assert!(start(&id, &[]).is_empty(), "{risk}");
        }

        let r2 = new_case(&env, "r2", &["--zone", "red", "--risk", "R2"]);
        let w = start(&r2, &[]);
        assert_eq!(w.len(), 1);
        assert!(w[0].starts_with(&format!(
            "{r2} is risk R2 and was started without --snapshot"
        )));
        assert!(w[0].contains("snapshot or backup"));
        assert!(!w[0].contains("explicit go"), "{}", w[0]);

        let r3 = new_case(&env, "r3", &["--zone", "red", "--risk", "R3"]);
        let w = start(&r3, &[]);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("risk R3"));
        assert!(
            w[0].contains("the human's explicit go per step"),
            "{}",
            w[0]
        );
        assert_eq!(case_at(&root, &r3).1.status, CaseStatus::Active);

        // with a snapshot: no warning, for either level
        for risk in ["R2", "R3"] {
            let id = new_case(&env, risk, &["--risk", risk]);
            assert!(start(&id, &["--snapshot", "7"]).is_empty(), "{risk}");
        }
        // a snapshotBefore the human set before the start counts too
        let id = new_case(&env, "pre-set", &["--risk", "R3"]);
        let (path, _, _) = case_at(&root, &id);
        let text = read(&path).replace("snapshotBefore:\n", "snapshotBefore: 9\n");
        std::fs::write(&path, text).unwrap();
        assert!(start(&id, &[]).is_empty());

        // the human output carries it as a `warning:` line; other steps none
        let id = new_case(&env, "human", &["--risk", "R2"]);
        let out = env.at(T1, &["plan", "start", &id]);
        assert_eq!(out.status.code(), Some(0));
        let text = stdout(&out);
        assert!(
            text.contains(&format!("\nwarning: {id} is risk R2")),
            "{text}"
        );
        let out = env.at(T2, &["plan", "verify", &id, "--json"]);
        assert_eq!(json(&out)["warnings"], serde_json::json!([]));
    }

    #[test]
    fn list_and_show() {
        let env = Env::new(Snapper::Missing);
        env.init_logbook();
        new_case(&env, "a", &["--area", "shell"]);
        new_case(&env, "b", &["--area", "hyprland"]);
        env.at(T1, &["plan", "start", "C-2026-002"]);
        let out = env.seldon(&["plan", "list", "--json"]);
        let v = json(&out);
        let ids: Vec<&str> = v["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["C-2026-001", "C-2026-002"]);
        for c in v["cases"].as_array().unwrap() {
            assert_valid_case(c);
        }
        let only = json(&env.seldon(&["plan", "list", "--status", "active", "--json"]));
        assert_eq!(only["cases"].as_array().unwrap().len(), 1);
        let only = json(&env.seldon(&["plan", "list", "--area", "shell", "--json"]));
        assert_eq!(only["cases"][0]["id"], "C-2026-001");
        assert!(stdout(&env.seldon(&["plan", "list"])).contains("C-2026-002  active"));
        assert_eq!(
            env.seldon(&["plan", "list", "--status", "open"])
                .status
                .code(),
            Some(1)
        );

        let show = json(&env.seldon(&["plan", "show", "C-2026-002", "--json"]));
        assert_eq!(show["case"]["status"], "active");
        assert_eq!(show["activeCase"], true);
        assert!(show["body"].as_str().unwrap().contains("## Log"));
        assert_valid_case(&show["case"]);
        assert_eq!(
            env.seldon(&["plan", "show", "C-2026-009"]).status.code(),
            Some(1)
        );
    }

    #[test]
    fn works_on_the_fixture_logbook() {
        // a copy: fixtures/logbook/ is read-only input
        let env = Env::new(Snapper::Missing);
        let root = env.tmp.path().join("fixture");
        copy_dir(&fixture_logbook(), &root);
        let lb = root.to_str().unwrap();
        let (path, _, body) = case_at(&root, "C-2026-008");
        let original = read(&path);

        let out = env.at(T2, &["--logbook", lb, "plan", "done", "C-2026-004"]);
        assert_eq!(
            out.status.code(),
            Some(1),
            "C-2026-004 is active, not verification"
        );
        let out = env.at(
            T2,
            &["--logbook", lb, "plan", "done", "C-2026-008", "--json"],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let (p, case, new_body) = case_at(&root, "C-2026-008");
        assert_eq!(
            p,
            root.join("work/completed").join(path.file_name().unwrap())
        );
        assert_eq!(case.status, CaseStatus::Completed);
        let line = "- 2026-10-02 09:30 · completed · human\n";
        let at = body.find("\n\n## Result").unwrap() + 1;
        assert_eq!(new_body, format!("{}{line}{}", &body[..at], &body[at..]));
        // two frontmatter lines changed, nothing else
        let frontmatter = &original[..original.len() - body.len()];
        let expected = frontmatter
            .replace("status: verification", "status: completed")
            .replace("closed:\n", "closed: 2026-10-02\n")
            + &new_body;
        assert_eq!(read(&p), expected);
        assert_eq!(
            env.at(T2, &["--logbook", lb, "plan", "new", "--json", "--", "x"])
                .status
                .code(),
            Some(0)
        );
        assert!(
            find_file(&root.join("work/queued"), "C-2026-009-").is_file(),
            "after C-2026-008"
        );
        ledger(&root);
    }

    #[test]
    fn autocommit_follows_flags_and_config() {
        let env = Env::new(Snapper::Missing);
        if !env.has_git {
            return;
        }
        let root = env.init_logbook();
        let head = |env: &Env| {
            stdout(&env.git(&root, &["log", "-1", "--format=%s"]))
                .trim()
                .to_string()
        };
        new_case(&env, "x", &[]);
        assert_eq!(head(&env), "seldon: C-2026-001 created");
        let status = stdout(&env.git(&root, &["status", "--porcelain"]));
        assert_eq!(status, "", "everything committed");

        assert_eq!(
            env.at(T1, &["--no-commit", "plan", "start", "C-2026-001"])
                .status
                .code(),
            Some(0)
        );
        assert_eq!(head(&env), "seldon: C-2026-001 created");

        let config = env.tmp.path().join("other.toml");
        std::fs::write(
            &config,
            format!(
                "logbook = \"{}\"\n[git]\nautocommit = false\n",
                root.display()
            ),
        )
        .unwrap();
        let out = env.at(
            T1,
            &[
                "--config",
                config.to_str().unwrap(),
                "plan",
                "verify",
                "C-2026-001",
                "--json",
            ],
        );
        assert_eq!(json(&out)["git"]["reason"], "git.autocommit = false");
        assert_eq!(head(&env), "seldon: C-2026-001 created");

        let out = env.at(T2, &["plan", "done", "C-2026-001", "--json"]);
        assert_eq!(json(&out)["git"]["committed"], true);
        assert_eq!(head(&env), "seldon: C-2026-001 completed");
        // the edits of the uncommitted steps went in with it
        assert_eq!(stdout(&env.git(&root, &["status", "--porcelain"])), "");
        let ignored = stdout(&env.git(&root, &["check-ignore", ".seldon/active-case"]));
        assert_eq!(ignored.trim(), ".seldon/active-case");
    }

    /// Everything a plan step may change: every work file, the marker, the
    /// journal folder, the areas.
    fn state(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for e in entries {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push((p.clone(), std::fs::read(&p).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        for dir in ["work", "journal", "areas", "ledger", ".seldon"] {
            walk(&root.join(dir), &mut out);
        }
        out.sort();
        out
    }

    #[test]
    fn a_ledger_failure_transitions_nothing() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let queued = new_case(&env, "queued", &[]);
        let active = new_case(&env, "active", &[]);
        let verifying = new_case(&env, "verifying", &[]);
        for (step, id) in [
            ("start", &active),
            ("start", &verifying),
            ("verify", &verifying),
        ] {
            assert_eq!(env.at(T1, &["plan", step, id]).status.code(), Some(0));
        }

        // 1. the ledger refuses to start: an invalid redaction pattern (exit 1)
        let bad = env.tmp.path().join("bad-redaction.toml");
        std::fs::write(
            &bad,
            format!(
                "logbook = \"{}\"\n[redaction]\npatterns = [\"(\"]\n",
                root.display()
            ),
        )
        .unwrap();
        let before = state(&root);
        let config = bad.to_str().unwrap();
        for args in [
            &[
                "--config", config, "plan", "new", "--area", "new-area", "--", "x",
            ][..],
            &["--config", config, "plan", "start", queued.as_str()][..],
            &["--config", config, "plan", "verify", active.as_str()][..],
            &["--config", config, "plan", "done", verifying.as_str()][..],
            &["--config", config, "plan", "drop", active.as_str()][..],
        ] {
            let out = env.at(T2, args);
            assert_eq!(out.status.code(), Some(1), "{args:?}: {}", stderr(&out));
            assert!(
                stderr(&out).contains("[redaction] patterns"),
                "{}",
                stderr(&out)
            );
        }
        assert_eq!(state(&root), before, "nothing transitioned");

        // 2. the ledger cannot be written: a new month in a read-only ledger/
        //    (exit 2); skipped where permissions do not bind (root)
        let ledger_dir = root.join("ledger");
        let mut perms = std::fs::metadata(&ledger_dir).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o555);
        std::fs::set_permissions(&ledger_dir, perms.clone()).unwrap();
        let probe = ledger_dir.join(".probe");
        if std::fs::write(&probe, "").is_ok() {
            let _ = std::fs::remove_file(&probe);
        } else {
            let november = "2026-11-02T08:00:00+01:00";
            for args in [
                &["plan", "new", "--area", "new-area", "--", "x"][..],
                &["plan", "start", queued.as_str()][..],
                &["plan", "done", verifying.as_str()][..],
                &["plan", "drop", active.as_str()][..],
            ] {
                let out = env.at(november, args);
                assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
            }
            assert_eq!(state(&root), before, "nothing transitioned");
        }
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&ledger_dir, perms).unwrap();

        // and with a working ledger the same steps go through
        assert_eq!(
            env.at(T2, &["plan", "done", &verifying]).status.code(),
            Some(0)
        );
        assert!(root.join("journal/2026/2026-10-02.md").is_file());
    }

    /// WP-057: `plan done` passes a day file without frontmatter (it gets
    /// the block); one with broken frontmatter fails the step before the
    /// ledger, the case or the marker changes.
    #[test]
    fn done_with_a_day_file_without_frontmatter() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let id = new_case(&env, "Fonts", &[]);
        for step in ["start", "verify"] {
            assert_eq!(env.at(T0, &["plan", step, &id]).status.code(), Some(0));
        }
        let day = root.join("journal/2026/2026-10-01.md");
        std::fs::create_dir_all(day.parent().unwrap()).unwrap();
        let events = ledger(&root).len();

        std::fs::write(&day, "---\ndate: [\n---\n").unwrap();
        let out = env.at(T1, &["plan", "done", &id]);
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("invalid journal frontmatter"),
            "{}",
            stderr(&out)
        );
        assert_eq!(ledger(&root).len(), events, "no case-completed event");
        assert_eq!(case_at(&root, &id).1.status, CaseStatus::Verification);
        assert!(root.join(".seldon/active-case").is_file());

        std::fs::write(&day, "").unwrap();
        let out = env.at(T1, &["plan", "done", &id]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(case_at(&root, &id).1.status, CaseStatus::Completed);
        assert_eq!(ledger(&root).len(), events + 1);
        assert_eq!(
            read(&day),
            format!(
                "---\ntype: journal\ndate: 2026-10-01\ncases: [{id}]\n---\n## 11:00 · human · {id}\nCase completed: Fonts\n"
            )
        );
    }

    /// WP-057: a case id in two folders (a stale copy written back) is
    /// reported by `index --check`, with both files; the user's to fix,
    /// so exit 1, not the engine's exit 2 (WP-070).
    #[test]
    fn index_check_reports_a_case_in_two_folders() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let id = new_case(&env, "Twice", &[]);
        let out = env.at(T0, &["index", "--check"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

        let (path, _, _) = case_at(&root, &id);
        let copy = root.join("work/completed").join(path.file_name().unwrap());
        std::fs::write(
            &copy,
            read(&path).replace("status: queued", "status: completed"),
        )
        .unwrap();
        let out = env.at(T0, &["index", "--check", "--json"]);
        assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
        let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            message.starts_with(&format!(
                "case {id} exists more than once (work/queued/{name}, work/completed/{name}); keep one file; "
            )),
            "{message}"
        );
        assert!(!message.contains("schema"), "{message}");
    }

    #[test]
    fn not_initialised_and_lock_held() {
        let env = Env::new(Snapper::Missing);
        let out = env.at(T0, &["plan", "new", "--json", "--", "x"]);
        assert_eq!(out.status.code(), Some(3));
        assert_eq!(json(&out)["error"]["code"], 3);
        env.init_logbook();
        let _lock = seldon::logbook::lock::acquire(&env.lock_file()).unwrap();
        assert_eq!(
            env.at(T0, &["plan", "new", "--", "x"]).status.code(),
            Some(4)
        );
        // reading needs no lock
        assert_eq!(env.seldon(&["plan", "list"]).status.code(), Some(0));
    }
}
