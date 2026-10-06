//! What a capture tells the cases (ADR-0027 §2c, §3, WP-101): the
//! rollback snapshot the agent forgot to record, a pruned rollback, and a
//! red `alwaysRed` change in a case below R3.

mod common;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use common::{Env, Snapper, read, stderr};
use serde_json::{Value, json};

const SINCE: &str = "2026-10-01T09:00:00+02:00";

struct Bench {
    env: Env,
    logbook: PathBuf,
}

impl Bench {
    /// A logbook with C-2026-001 active (yellow, R1); snapper refuses the
    /// list, so the snapshots come from `<tmp>/.snapshots` (the read grant).
    fn new() -> Self {
        let env = Env::new(Snapper::NoPermissions);
        let logbook = env.tmp.path().join("logbook");
        let b = Bench { env, logbook };
        b.run(
            "2026-10-01T09:30:00+02:00",
            &[
                "init",
                "--non-interactive",
                "--no-capture",
                "--no-git",
                "--path",
                b.logbook.to_str().unwrap(),
            ],
            None,
        );
        b.run(
            "2026-10-01T10:00:00+02:00",
            &["plan", "new", "--", "Install zed"],
            None,
        );
        b.run(
            "2026-10-01T10:00:00+02:00",
            &["plan", "start", "C-2026-001"],
            None,
        );
        b.snapshot(1, "2026-08-28 10:41:07", "single", "first root filesystem");
        b.capture("2026-10-01T10:01:00+02:00", "snapper");
        b
    }

    fn run(&self, now: &str, args: &[&str], stdin: Option<&str>) -> Output {
        let tmp = self.env.tmp.path();
        let mut child = self
            .env
            .command(args)
            .env("SELDON_NOW", now)
            .env("SELDON_PACMAN_LOG", tmp.join("pacman.log"))
            .env("SELDON_PACMAN_DB_LOCK", tmp.join("no-db.lck"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.unwrap_or("").as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        out
    }

    fn capture(&self, now: &str, source: &str) -> Output {
        self.run(
            now,
            &["capture", "--source", source, "--since", SINCE],
            None,
        )
    }

    /// `<tmp>/.snapshots/<n>/info.xml`, `date` in UTC.
    fn snapshot(&self, n: u64, date: &str, kind: &str, description: &str) {
        let dir = self.env.tmp.path().join(".snapshots").join(n.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        let pre = if kind == "post" {
            format!("\n  <pre_num>{}</pre_num>", n - 1)
        } else {
            String::new()
        };
        std::fs::write(
            dir.join("info.xml"),
            format!(
                "<?xml version=\"1.0\"?>\n<snapshot>\n  <type>{kind}</type>\n  <num>{n}</num>{pre}\n  \
                 <date>{date}</date>\n  <description>{description}</description>\n  \
                 <cleanup>number</cleanup>\n</snapshot>\n"
            ),
        )
        .unwrap();
    }

    fn delete_snapshot(&self, n: u64) {
        std::fs::remove_dir_all(self.env.tmp.path().join(".snapshots").join(n.to_string()))
            .unwrap();
    }

    /// `seldon hook generic` for an agent's command started at `at`.
    fn hook(&self, command: &str, at: &str) {
        let payload = json!({
            "command": command,
            "actor": "agent:codex",
            "cwd": self.logbook,
            "startedAt": at,
        });
        self.run(at, &["hook", "generic"], Some(&payload.to_string()));
    }

    fn case(&self, id: &str) -> Value {
        let out = self.run(SINCE, &["plan", "show", id, "--json"], None);
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["case"].clone()
    }

    fn log(&self, id: &str) -> Vec<String> {
        let path = case_file(&self.logbook, id);
        let text = read(&path);
        let start = text.find("## Log").unwrap();
        let end = text.rfind("## Result").unwrap();
        text[start..end]
            .lines()
            .filter(|l| l.starts_with("- "))
            .map(String::from)
            .collect()
    }
}

fn case_file(root: &Path, id: &str) -> PathBuf {
    for folder in ["queued", "active", "completed", "dropped"] {
        if let Ok(entries) = std::fs::read_dir(root.join("work").join(folder)) {
            for e in entries {
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
    }
    panic!("{id} not found");
}

mod snapshot_before {
    use super::*;

    #[test]
    fn a_snapshot_named_by_the_case_id_fills_the_empty_field() {
        let b = Bench::new();
        // 08:30 UTC = 10:30 local; `-d "<ID>"` as the rules say
        b.snapshot(42, "2026-10-01 08:30:00", "single", "C-2026-001");
        b.capture("2026-10-01T10:40:00+02:00", "snapper");
        assert_eq!(b.case("C-2026-001")["snapshotBefore"], 42);
        assert_eq!(
            b.log("C-2026-001").last().unwrap(),
            "- 2026-10-01 10:40 · snapshot 42 (its description names the case) · system"
        );
        // a later one names the case too: the first stays
        b.snapshot(43, "2026-10-01 08:45:00", "single", "C-2026-001");
        b.capture("2026-10-01T10:50:00+02:00", "snapper");
        assert_eq!(b.case("C-2026-001")["snapshotBefore"], 42);
        assert_eq!(b.log("C-2026-001").len(), 3, "{:?}", b.log("C-2026-001"));
    }

    #[test]
    fn the_recorded_snapshot_command_fills_it() {
        let b = Bench::new();
        b.hook(
            "sudo snapper -c root create -c number -p -d before-zed",
            "2026-10-01T10:29:50+02:00",
        );
        let commands: Vec<Value> = common::ledger(&b.logbook)
            .into_iter()
            .filter(|e| e["kind"] == "command")
            .collect();
        assert_eq!(commands.len(), 1, "the hook records it with the case");
        assert_eq!(commands[0]["subject"], "snapper");
        assert_eq!(commands[0]["zone"], "green");
        assert_eq!(commands[0]["case"], "C-2026-001");
        b.snapshot(42, "2026-10-01 08:30:00", "single", "before-zed");
        b.capture("2026-10-01T10:40:00+02:00", "snapper");
        assert_eq!(b.case("C-2026-001")["snapshotBefore"], 42);
        assert_eq!(
            b.log("C-2026-001").last().unwrap(),
            "- 2026-10-01 10:40 · snapshot 42 (from the recorded snapshot command) · agent:codex"
        );
    }

    #[test]
    fn omarchys_snapshot_command_counts_too() {
        let b = Bench::new();
        b.hook("omarchy-snapshot create", "2026-10-01T10:29:50+02:00");
        b.snapshot(42, "2026-10-01 08:30:05", "single", "4.0.4");
        b.capture("2026-10-01T10:40:00+02:00", "snapper");
        assert_eq!(b.case("C-2026-001")["snapshotBefore"], 42);
    }

    #[test]
    fn nothing_fills_it_without_a_match() {
        let b = Bench::new();
        // a command outside the window, a post snapshot, a closed case's id
        b.hook(
            "sudo snapper -c root create -d early",
            "2026-10-01T10:10:00+02:00",
        );
        b.snapshot(42, "2026-10-01 08:30:00", "single", "unrelated");
        b.snapshot(43, "2026-10-01 08:31:00", "post", "C-2026-001");
        b.capture("2026-10-01T10:40:00+02:00", "snapper");
        assert_eq!(b.case("C-2026-001")["snapshotBefore"], Value::Null);
        assert_eq!(b.log("C-2026-001").len(), 2);
        // a snapshot list command is no snapshot command
        b.hook("snapper -c create list", "2026-10-01T10:44:00+02:00");
        b.snapshot(44, "2026-10-01 08:45:00", "single", "x");
        b.capture("2026-10-01T10:50:00+02:00", "snapper");
        assert_eq!(b.case("C-2026-001")["snapshotBefore"], Value::Null);
    }

    #[test]
    fn a_recorded_number_is_never_replaced() {
        let b = Bench::new();
        b.run(
            "2026-10-01T10:20:00+02:00",
            &["plan", "snapshot", "C-2026-001", "1"],
            None,
        );
        b.snapshot(42, "2026-10-01 08:30:00", "single", "C-2026-001");
        b.capture("2026-10-01T10:40:00+02:00", "snapper");
        assert_eq!(b.case("C-2026-001")["snapshotBefore"], 1);
    }
}

mod pruned {
    use super::*;

    #[test]
    fn a_deleted_rollback_is_named_in_the_case_once() {
        let b = Bench::new();
        b.snapshot(42, "2026-10-01 08:30:00", "single", "C-2026-001");
        b.capture("2026-10-01T10:40:00+02:00", "snapper");
        assert_eq!(b.case("C-2026-001")["snapshotBefore"], 42);
        b.delete_snapshot(42);
        b.capture("2026-10-01T11:00:00+02:00", "snapper");
        assert_eq!(
            b.log("C-2026-001").last().unwrap(),
            "- 2026-10-01 11:00 · rollback for C-2026-001 pruned (snapshot 42) · system"
        );
        let lines = b.log("C-2026-001").len();
        b.capture("2026-10-01T11:10:00+02:00", "snapper");
        assert_eq!(b.log("C-2026-001").len(), lines);
        // doctor names it
        let out = b.run("2026-10-01T11:10:00+02:00", &["doctor", "--json"], None);
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        let row = v["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "rollbacks")
            .cloned()
            .unwrap();
        assert_eq!(row["status"], "degraded", "{row}");
        assert!(
            row["message"]
                .as_str()
                .unwrap()
                .contains("C-2026-001: snapshot 42 pruned"),
            "{row}"
        );
    }

    #[test]
    fn a_completed_case_is_told_too_a_dropped_one_is_not() {
        let b = Bench::new();
        b.snapshot(42, "2026-10-01 08:30:00", "single", "C-2026-001");
        b.capture("2026-10-01T10:40:00+02:00", "snapper");
        let at = "2026-10-01T10:45:00+02:00";
        b.run(at, &["plan", "new", "--", "Other"], None);
        b.run(at, &["plan", "start", "C-2026-002"], None);
        b.run(at, &["plan", "snapshot", "C-2026-002", "42"], None);
        b.run(at, &["plan", "drop", "C-2026-002", "--reason", "x"], None);
        b.run(at, &["plan", "verify", "C-2026-001"], None);
        b.run(at, &["plan", "done", "C-2026-001"], None);
        b.delete_snapshot(42);
        b.capture("2026-10-01T11:00:00+02:00", "snapper");
        assert!(
            b.log("C-2026-001")
                .last()
                .unwrap()
                .contains("rollback for C-2026-001 pruned (snapshot 42)")
        );
        assert!(!b.log("C-2026-002").iter().any(|l| l.contains("pruned")));
    }
}

mod r3_advisory {
    use super::*;

    /// One install transaction of `package` at 10:05:30 local.
    fn install(package: &str) -> String {
        format!(
            "[2026-10-01T10:05:30+0200] [PACMAN] Running 'pacman -S {package}'\n\
             [2026-10-01T10:05:31+0200] [ALPM] transaction started\n\
             [2026-10-01T10:05:32+0200] [ALPM] installed {package} (1.0-1)\n\
             [2026-10-01T10:05:32+0200] [ALPM] transaction completed\n"
        )
    }

    fn installed(b: &Bench, package: &str) -> Output {
        b.hook(
            &format!("sudo pacman -S {package}"),
            "2026-10-01T10:05:00+02:00",
        );
        std::fs::write(b.env.tmp.path().join("pacman.log"), install(package)).unwrap();
        b.capture("2026-10-01T10:10:00+02:00", "pacman")
    }

    #[test]
    fn a_red_always_red_change_in_a_case_below_r3_is_noted_and_warned() {
        let b = Bench::new();
        let out = installed(&b, "linux");
        let pkg: Vec<Value> = common::ledger(&b.logbook)
            .into_iter()
            .filter(|e| e["source"] == "pacman")
            .collect();
        assert_eq!(pkg[0]["case"], "C-2026-001", "{pkg:?}");
        let advisory = "C-2026-001 is R1, but its red change `linux` is R3 (`[drift] \
                        alwaysRed`): an R3 step needs the user's explicit go and a snapshot; \
                        raise it with `seldon plan set C-2026-001 --risk R3` (ADR-0027 §2c)";
        assert_eq!(
            b.log("C-2026-001").last().unwrap(),
            &format!("- 2026-10-01 10:10 · advisory: {advisory} · system")
        );
        // the index build warns (the existing warnings channel)
        assert!(
            stderr(&out).contains(&format!("seldon: warning: {advisory}")),
            "{}",
            stderr(&out)
        );
        let out = b.run("2026-10-01T10:20:00+02:00", &["index"], None);
        assert!(
            common::stdout(&out).contains(&format!("warning: {advisory}")),
            "{}",
            common::stdout(&out)
        );
        // raised to R3: no warning any more
        b.run(
            "2026-10-01T10:20:00+02:00",
            &["plan", "set", "C-2026-001", "--risk", "R3"],
            None,
        );
        let out = b.run("2026-10-01T10:21:00+02:00", &["index"], None);
        assert!(
            !common::stdout(&out).contains("alwaysRed"),
            "{}",
            common::stdout(&out)
        );
    }

    #[test]
    fn other_packages_are_no_advisory() {
        let b = Bench::new();
        let out = installed(&b, "zed");
        assert!(!stderr(&out).contains("alwaysRed"), "{}", stderr(&out));
        assert!(!b.log("C-2026-001").iter().any(|l| l.contains("advisory")));
    }
}
