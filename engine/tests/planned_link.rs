//! Rule 9, the planned-and-active link, and the capture before a case
//! changes state (ADR-0029, WP-115): a change made while exactly one case
//! was open and named it in its Plan is that case's, whoever typed it.
//! The window, uniqueness and actor rules are unit-tested in
//! `reconcile.rs`; these tests run the commands end to end.

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;

use common::{Env, Snapper, find_file, json, read, stderr, stdout};
use serde_json::{Value, json};

/// `init` (the logbook's `created`: the captures' baseline).
const T_INIT: &str = "2026-10-06T13:00:00+02:00";

/// A fresh logbook, a pacman log and the commands' clock (C-2026-004 of
/// the live test of 2026-10-06, here C-2026-001).
struct Bench {
    env: Env,
    lb: PathBuf,
    log: PathBuf,
}

impl Bench {
    fn new() -> Self {
        let env = Env::new(Snapper::Missing);
        let lb = env.tmp.path().join("logbook");
        let log = env.tmp.path().join("pacman.log");
        std::fs::write(&log, "").unwrap();
        let b = Bench { env, lb, log };
        b.ok(
            T_INIT,
            &[
                "init",
                "--non-interactive",
                "--no-capture",
                "--no-git",
                "--path",
                b.lb.to_str().unwrap(),
            ],
        );
        // every collector's baseline
        b.ok("2026-10-06T13:01:00+02:00", &["capture"]);
        b
    }

    /// `seldon args… --json` at `now`, local time CEST (the pacman log's
    /// offset; CI runs in UTC).
    fn run(&self, now: &str, args: &[&str]) -> Output {
        let mut all: Vec<&str> = args.to_vec();
        match all.iter().position(|a| *a == "--") {
            Some(at) => all.insert(at, "--json"),
            None => all.push("--json"),
        }
        self.env
            .command(&all)
            .env("SELDON_NOW", now)
            .env("TZ", "Europe/Berlin")
            .env("SELDON_PACMAN_LOG", &self.log)
            .env(
                "SELDON_PACMAN_DB_LOCK",
                self.env.tmp.path().join("no-db.lck"),
            )
            .output()
            .expect("run seldon")
    }

    fn ok(&self, now: &str, args: &[&str]) -> Value {
        let out = self.run(now, args);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{args:?}: {}{}",
            stdout(&out),
            stderr(&out)
        );
        json(&out)
    }

    /// A new case started at `at` whose Plan names `step`.
    fn case(&self, at: &str, title: &str, risk: &str, step: &str) -> String {
        let id = self.ok(at, &["plan", "new", "--risk", risk, "--", title])["case"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        self.ok(at, &["plan", "start", &id]);
        let path = self.case_path(&id);
        let text = read(&path).replacen("- Steps:", &format!("- Steps: {step}"), 1);
        std::fs::write(&path, text).unwrap();
        id
    }

    /// Appends one transaction to the pacman log: `pacman -S <named>`,
    /// then `installed` for the dependencies and the named packages.
    fn install(&self, at: &str, named: &[&str], deps: &[&str]) {
        let stamp = format!("[{}]", at.replace("+02:00", "+0200"));
        let mut text = format!(
            "{stamp} [PACMAN] Running 'pacman -S {}'\n{stamp} [ALPM] transaction started\n",
            named.join(" ")
        );
        for p in deps.iter().chain(named) {
            text.push_str(&format!("{stamp} [ALPM] installed {p} (1.0-1)\n"));
        }
        text.push_str(&format!("{stamp} [ALPM] transaction completed\n"));
        let mut all = read(&self.log);
        all.push_str(&text);
        std::fs::write(&self.log, all).unwrap();
    }

    fn capture(&self, now: &str) -> Value {
        self.ok(now, &["capture"])
    }

    fn ledger(&self) -> Vec<Value> {
        common::ledger(&self.lb)
    }

    fn pacman(&self, subject: &str) -> Value {
        self.ledger()
            .into_iter()
            .find(|e| e["source"] == "pacman" && e["subject"] == subject)
            .unwrap_or_else(|| panic!("no pacman event of {subject}"))
    }

    /// The resolution lines that refer to `id`, in ledger order.
    fn resolutions(&self, id: &Value) -> Vec<Value> {
        self.ledger()
            .into_iter()
            .filter(|e| e["kind"] == "resolution" && e["refersTo"] == *id)
            .collect()
    }

    fn case_path(&self, id: &str) -> PathBuf {
        for folder in ["queued", "active", "completed", "dropped"] {
            let dir = self.lb.join("work").join(folder);
            if dir.join(".").exists()
                && std::fs::read_dir(&dir)
                    .unwrap()
                    .any(|e| e.unwrap().file_name().to_string_lossy().starts_with(id))
            {
                return find_file(&dir, &format!("{id}-"));
            }
        }
        panic!("{id} not found");
    }

    fn case_json(&self, id: &str) -> Value {
        self.ok(T_INIT, &["plan", "show", id])["case"].clone()
    }

    fn log_lines(&self, id: &str) -> Vec<String> {
        log_lines(&self.case_path(id))
    }

    fn index(&self) -> Value {
        serde_json::from_str(&read(&self.env.home.join(".local/state/seldon/index.json"))).unwrap()
    }

    fn index_event(&self, id: &Value) -> Value {
        self.index()["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == *id)
            .cloned()
            .unwrap_or_else(|| panic!("{id} not in the index"))
    }
}

fn log_lines(path: &Path) -> Vec<String> {
    let text = read(path);
    let start = text.find("## Log").unwrap();
    let end = text.rfind("## Result").unwrap();
    text[start..end]
        .lines()
        .filter(|l| l.starts_with("- "))
        .map(String::from)
        .collect()
}

/// Acceptance 1: C-2026-004 as it happened. The user installs `glow` in a
/// terminal while the case is open, the agent verifies and closes it
/// without a capture; the next capture links the install to the case.
#[test]
fn an_install_the_user_made_inside_the_case_is_linked_after_the_close() {
    let b = Bench::new();
    let id = b.case(
        "2026-10-06T13:38:00+02:00",
        "install glow",
        "R1",
        "install glow",
    );
    b.install("2026-10-06T13:40:57+02:00", &["glow"], &[]);
    b.ok(
        "2026-10-06T13:41:34+02:00",
        &["plan", "verify", &id, "--no-capture"],
    );
    b.ok(
        "2026-10-06T13:41:34+02:00",
        &["plan", "done", &id, "--no-capture"],
    );
    assert!(
        b.ledger().iter().all(|e| e["source"] != "pacman"),
        "no capture ran before the close"
    );

    let c = b.capture("2026-10-06T13:45:00+02:00");
    assert_eq!(
        (c["written"].clone(), c["linkedPlanned"].clone()),
        (json!(1), json!(1)),
        "{c}"
    );
    let glow = b.pacman("glow");
    assert_eq!(
        (glow["actor"].clone(), glow.get("case")),
        (json!("system"), None)
    );
    let lines = b.resolutions(&glow["id"]);
    assert_eq!(lines.len(), 1);
    let r = &lines[0];
    assert_eq!(
        (
            r["source"].clone(),
            r["actor"].clone(),
            r["resolution"].clone(),
            r["case"].clone(),
            r["detail"].clone(),
            r["subject"].clone(),
            r["ts"].clone(),
        ),
        (
            json!("seldon"),
            json!("system"),
            json!("linked"),
            json!(id),
            json!(format!("planned by {id}; active at the time")),
            json!("glow"),
            json!("2026-10-06T13:45:00+02:00"),
        )
    );
    assert!(r["meta"].get("txId").is_none(), "one line, no group");
    assert_eq!(b.case_json(&id)["events"], json!([glow["id"]]));
    assert_eq!(
        b.log_lines(&id).last().unwrap(),
        "- 2026-10-06 13:45 · linked after the fact: pacman install glow at 13:40:57 \
         (planned here, no capture ran before the close) · system"
    );
    let folded = b.index_event(&glow["id"]);
    assert_eq!(
        (folded["resolution"].clone(), folded["case"].clone()),
        (json!("linked"), json!(id))
    );
    assert_eq!(b.index()["drift"], json!([]));
    assert_eq!(b.index()["summary"]["openDrift"], 0);

    // idempotent: the second capture writes nothing, the case file stays
    let file = b.case_path(&id);
    let text = read(&file);
    let c = b.capture("2026-10-06T13:50:00+02:00");
    assert_eq!(
        (c["written"].clone(), c["linkedPlanned"].clone()),
        (json!(0), json!(0))
    );
    assert_eq!(read(&file), text);
}

/// Acceptance 2: with ADR-0029 §2 the capture inside `plan verify` records
/// and links the install while the case is still active.
#[test]
fn plan_verify_captures_first_and_links_while_the_case_is_open() {
    let b = Bench::new();
    let id = b.case(
        "2026-10-06T13:38:00+02:00",
        "install glow",
        "R1",
        "install glow",
    );
    b.install("2026-10-06T13:40:57+02:00", &["glow"], &[]);
    let v = b.ok("2026-10-06T13:41:30+02:00", &["plan", "verify", &id]);
    assert_eq!(v["capture"]["written"], 1, "{v}");
    assert_eq!(v["capture"]["linkedPlanned"], 1, "{v}");
    assert_eq!(v["to"], "verification");

    let ledger = b.ledger();
    let order: Vec<(String, String)> = ledger
        .iter()
        .filter(|e| e["ts"].as_str().unwrap() >= "2026-10-06T13:40")
        .map(|e| {
            (
                e["kind"].as_str().unwrap().to_string(),
                e["subject"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let s = |k: &str, sub: &str| (k.to_string(), sub.to_string());
    assert_eq!(
        order,
        [
            s("install", "glow"),
            s("resolution", "glow"),
            s("case-verified", &id)
        ]
    );
    assert_eq!(
        b.log_lines(&id)[2..],
        [
            "- 2026-10-06 13:41 · linked after the fact: pacman install glow at 13:40:57 \
             (planned here) · system"
                .to_string(),
            "- 2026-10-06 13:41 · verification · human".to_string()
        ]
    );
    // `plan done` captures too: nothing new
    let d = b.ok("2026-10-06T13:41:34+02:00", &["plan", "done", &id]);
    assert_eq!(d["capture"]["written"], 0, "{d}");
    assert_eq!(b.case_json(&id)["events"], json!([b.pacman("glow")["id"]]));
}

/// Acceptance 7: the named package takes its transaction's dependencies
/// with it, one line each with `meta.txId`; an explicit package the Plan
/// does not name stays drift.
#[test]
fn the_dependencies_follow_the_planned_package() {
    let b = Bench::new();
    let id = b.case(
        "2026-10-06T13:38:00+02:00",
        "install glow",
        "R1",
        "install glow",
    );
    b.install(
        "2026-10-06T13:40:57+02:00",
        &["glow", "htop"],
        &["libyaml", "oniguruma"],
    );
    let c = b.capture("2026-10-06T13:45:00+02:00");
    assert_eq!(
        (c["written"].clone(), c["linkedPlanned"].clone()),
        (json!(4), json!(3)),
        "{c}"
    );
    let tx = b.pacman("glow")["txId"].clone();
    let mut linked = Vec::new();
    for p in ["libyaml", "oniguruma", "glow"] {
        let e = b.pacman(p);
        let lines = b.resolutions(&e["id"]);
        assert_eq!(lines.len(), 1, "{p}");
        assert_eq!(lines[0]["meta"]["txId"], tx, "{p}");
        assert_eq!(lines[0]["case"], json!(id), "{p}");
        linked.push(e["id"].clone());
    }
    assert!(b.resolutions(&b.pacman("htop")["id"]).is_empty());
    let events = b.case_json(&id)["events"].clone();
    let mut listed: Vec<String> = events
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let mut want: Vec<String> = linked
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    listed.sort();
    want.sort();
    assert_eq!(listed, want, "one entry each");
    let drift = b.ok("2026-10-06T13:46:00+02:00", &["drift"])["drift"].clone();
    assert_eq!(drift.as_array().unwrap().len(), 1);
    assert_eq!(drift[0]["subject"], "htop");
    // one Log line, for the package the Plan names
    let lines = b.log_lines(&id);
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.contains("linked after the fact"))
            .count(),
        1,
        "{lines:?}"
    );
}

/// Acceptance 4: two open cases whose Plans both name the package: no
/// link, the open proposal stays (lowest id), each case gets a Log line
/// naming the other.
#[test]
fn two_cases_that_planned_it_link_nothing_and_say_so() {
    let b = Bench::new();
    let first = b.case(
        "2026-10-06T13:38:00+02:00",
        "install glow",
        "R1",
        "install glow",
    );
    let second = b.case(
        "2026-10-06T13:39:00+02:00",
        "markdown tools",
        "R1",
        "glow and mdcat",
    );
    b.install("2026-10-06T13:40:57+02:00", &["glow"], &[]);
    let c = b.capture("2026-10-06T13:45:00+02:00");
    assert_eq!(c["linkedPlanned"], 0, "{c}");
    let glow = b.pacman("glow");
    assert!(b.resolutions(&glow["id"]).is_empty());
    let drift = b.ok("2026-10-06T13:46:00+02:00", &["drift"])["drift"].clone();
    assert_eq!(drift[0]["proposedCase"], json!(first));
    let id = glow["id"].as_str().unwrap();
    for (case, other) in [(&first, &second), (&second, &first)] {
        assert_eq!(
            b.log_lines(case).last().unwrap(),
            &format!(
                "- 2026-10-06 13:45 · not linked: pacman install glow at 13:40:57 is planned here \
                 and in {other}, both active at the time; `seldon drift link {id} <CASE>` links \
                 it · system"
            )
        );
    }
    // said once
    let before = read(&b.case_path(&first));
    b.capture("2026-10-06T13:50:00+02:00");
    assert_eq!(read(&b.case_path(&first)), before);
}

/// Acceptance 6 and 8: an `alwaysRed` package links only to an R3 case;
/// below R3 it stays a proposal and the case gets the R3 advisory. An
/// engine link yields to a human's later line, and an agent still may not
/// explain the crisis it is.
#[test]
fn the_harm_guard_and_a_later_line_that_wins() {
    let b = Bench::new();
    let low = b.case(
        "2026-10-06T13:38:00+02:00",
        "new kernel",
        "R1",
        "install linux-zen",
    );
    b.install("2026-10-06T13:40:57+02:00", &["linux-zen"], &[]);
    let c = b.capture("2026-10-06T13:45:00+02:00");
    assert_eq!(c["linkedPlanned"], 0, "{c}");
    let zen = b.pacman("linux-zen");
    assert!(b.resolutions(&zen["id"]).is_empty());
    assert!(
        b.log_lines(&low).last().unwrap().contains(&format!(
            "advisory: {low} is R1, but its red change `linux-zen` is R3"
        )),
        "{:?}",
        b.log_lines(&low)
    );
    let drift = b.ok("2026-10-06T13:46:00+02:00", &["drift"])["drift"].clone();
    assert_eq!(
        (drift[0]["crisis"].clone(), drift[0]["proposedCase"].clone()),
        (json!(true), json!(low))
    );
    b.ok("2026-10-06T13:47:00+02:00", &["plan", "drop", &low]);

    // R3: linked
    let r3 = b.case(
        "2026-10-06T14:00:00+02:00",
        "lts kernel",
        "R3",
        "install linux-lts",
    );
    b.install("2026-10-06T14:02:00+02:00", &["linux-lts"], &[]);
    let c = b.capture("2026-10-06T14:05:00+02:00");
    assert_eq!(c["linkedPlanned"], 1, "{c}");
    let lts = b.pacman("linux-lts");
    let id = lts["id"].as_str().unwrap();
    assert_eq!(b.case_json(&r3)["events"], json!([id]));

    // an agent may not explain the crisis the engine linked (ADR-0028 §3)
    let out = b.run(
        "2026-10-06T14:06:00+02:00",
        &[
            "drift",
            "explain",
            id,
            "--actor",
            "agent:codex",
            "--",
            "it was planned",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        json(&out)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("is a crisis"),
    );
    // a human's dismissal is a later line and wins
    let show = b.ok("2026-10-06T14:06:00+02:00", &["drift", "show", id]);
    assert_eq!(
        (show["open"].clone(), show["class"].clone()),
        (json!(false), json!("crisis"))
    );
    let d = b.ok(
        "2026-10-06T14:07:00+02:00",
        &["drift", "dismiss", id, "--", "tried it, removed it"],
    );
    assert_eq!(d["resolved"], 1, "{d}");
    let lines = b.resolutions(&lts["id"]);
    assert_eq!(
        lines
            .iter()
            .map(|l| (
                l["actor"].as_str().unwrap(),
                l["resolution"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        [("system", "linked"), ("human", "dismissed")]
    );
    let folded = b.index_event(&lts["id"]);
    assert_eq!(
        (folded["resolution"].clone(), folded.get("case")),
        (json!("dismissed"), None)
    );
    assert_eq!(b.case_json(&r3)["events"], json!([]));
    assert_eq!(
        b.log_lines(&r3).last().unwrap(),
        &format!(
            "- 2026-10-06 14:07 · no longer linked here: {id} (dismissed by human; the engine \
             had linked it) · human"
        )
    );
    // a human's line is never resolved again by the engine, nor by a re-run
    let c = b.capture("2026-10-06T14:10:00+02:00");
    assert_eq!(c["linkedPlanned"], 0);
    let again = b.ok(
        "2026-10-06T14:11:00+02:00",
        &["drift", "dismiss", id, "--", "again"],
    );
    assert_eq!(again["resolved"], 0);
}

/// Acceptance 9: `--no-capture` writes no collector event; a capture that
/// fails is a warning and the step happens; `plan drop` never captures.
#[test]
fn no_capture_and_a_failed_capture() {
    let b = Bench::new();
    let id = b.case(
        "2026-10-06T13:38:00+02:00",
        "install glow",
        "R1",
        "install glow",
    );
    b.install("2026-10-06T13:40:57+02:00", &["glow"], &[]);
    let v = b.ok(
        "2026-10-06T13:41:30+02:00",
        &["plan", "verify", &id, "--no-capture"],
    );
    assert_eq!(v["capture"], Value::Null);
    assert!(b.ledger().iter().all(|e| e["source"] != "pacman"));

    // the capture fails (its cursors cannot be read); the step goes on
    let cursors = b.env.home.join(".local/state/seldon/cursors.json");
    std::fs::remove_file(&cursors).unwrap();
    std::fs::create_dir(&cursors).unwrap();
    let d = b.ok("2026-10-06T13:41:34+02:00", &["plan", "done", &id]);
    assert_eq!(d["to"], "completed");
    assert_eq!(d["capture"], Value::Null);
    let warnings = d["warnings"].to_string();
    assert!(
        warnings.contains("the capture before the step did not run"),
        "{warnings}"
    );
    assert!(b.ledger().iter().all(|e| e["source"] != "pacman"));
    std::fs::remove_dir(&cursors).unwrap();

    // drop: no capture
    let other = b.case("2026-10-06T13:50:00+02:00", "other", "R1", "nothing");
    b.ok("2026-10-06T13:51:00+02:00", &["plan", "drop", &other]);
    assert!(b.ledger().iter().all(|e| e["source"] != "pacman"));
}

/// Acceptance 3 (end to end, the unit tests cover every edge): a change
/// after the close, or by a queued case's plan, is not linked; one in a
/// dropped case's window is.
#[test]
fn the_window_comes_from_the_case_events() {
    let b = Bench::new();
    let closed = b.case("2026-10-06T13:38:00+02:00", "glow", "R1", "install glow");
    b.ok("2026-10-06T13:39:00+02:00", &["plan", "drop", &closed]);
    let queued = b.ok("2026-10-06T13:39:00+02:00", &["plan", "new", "--", "mdcat"])["case"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let path = b.case_path(&queued);
    std::fs::write(
        &path,
        read(&path).replacen("- Steps:", "- Steps: install mdcat", 1),
    )
    .unwrap();
    b.install("2026-10-06T13:40:57+02:00", &["glow"], &[]);
    b.install("2026-10-06T13:41:00+02:00", &["mdcat"], &[]);
    let c = b.capture("2026-10-06T13:45:00+02:00");
    assert_eq!(c["linkedPlanned"], 0, "{c}");

    let dropped = b.case("2026-10-06T14:00:00+02:00", "bat", "R1", "install bat");
    b.install("2026-10-06T14:01:00+02:00", &["bat"], &[]);
    b.ok("2026-10-06T14:02:00+02:00", &["plan", "drop", &dropped]);
    let c = b.capture("2026-10-06T14:05:00+02:00");
    assert_eq!(c["linkedPlanned"], 1, "{c}");
    assert_eq!(
        b.resolutions(&b.pacman("bat")["id"])[0]["case"],
        json!(dropped)
    );
    assert!(
        b.log_lines(&dropped)
            .last()
            .unwrap()
            .ends_with("(planned here, no capture ran before the close) · system")
    );
}
