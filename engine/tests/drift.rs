//! `seldon drift` and its resolving commands (WP-008, SPEC-ENGINE §5,
//! ADR-0008, ADR-0013 §4), on copies of `fixtures/logbook/` and fresh
//! logbooks, always through `common::Env` (never the real XDG dirs).

mod common;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use common::{Env, Snapper, assert_valid_case, assert_valid_index, copy_dir, fixture_logbook};
use common::{find_file, json, read, stderr};

/// The sample index's clock (`fixtures/index.sample.json`).
const GENERATED_AT: &str = "2026-10-01T17:05:12+02:00";

/// Baseline of the pacman captures below (the logbook's `created` is the
/// real clock's, after the log lines).
const T_SINCE: &str = "2026-10-01T09:00:00+02:00";

/// The four open drift items of the fixture.
const THEME: &str = "01M3VTGNY0NZG4AY80814WSKGR"; // tokyo-night, proposed C-2026-005
const UNIT: &str = "01M3VNJ9JGZ9169T01XCW16FT0"; // ollama.service, crisis
const OLLAMA: &str = "01M3VNFTF8EVHWFFZ687N14Q0C"; // pacman install, crisis
const FIREFOX: &str = "01M3SXBQVR7AW8PJQC1YXDCQ14"; // leader of the 09-30 group
const NOTO: &str = "01M3SXBRV0E702XKBM22HEV1B8"; // group member
const LIBINPUT: &str = "01M3SXBRV0WPNQ721VWGG2WXZ1"; // group member

/// A copy of the fixture logbook in `env`.
fn fixture_copy(env: &Env) -> PathBuf {
    let lb = env.tmp.path().join("logbook");
    copy_dir(&fixture_logbook(), &lb);
    lb
}

/// `seldon --logbook <lb> args…` at the sample's time; asserts `code`.
fn run(env: &Env, lb: &Path, args: &[&str], code: i32) -> Value {
    let mut all = vec!["--logbook", lb.to_str().unwrap(), "--json"];
    all.extend_from_slice(args);
    let out = env.at(GENERATED_AT, &all);
    assert_eq!(
        out.status.code(),
        Some(code),
        "{args:?}: {}{}",
        common::stdout(&out),
        stderr(&out)
    );
    json(&out)
}

fn drift(env: &Env, lb: &Path) -> Value {
    run(env, lb, &["drift"], 0)
}

fn ids(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["eventId"].as_str().unwrap().to_string())
        .collect()
}

fn index_file(env: &Env) -> Value {
    serde_json::from_str(&read(&env.home.join(".local/state/seldon/index.json"))).unwrap()
}

/// The case frontmatter as `plan show --json` reports it.
fn case(env: &Env, lb: &Path, id: &str) -> Value {
    run(env, lb, &["plan", "show", id], 0)["case"].clone()
}

#[test]
fn lists_the_four_fixture_items() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let v = drift(&env, &lb);
    let sample: Value = serde_json::from_str(&read(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/index.sample.json"),
    ))
    .unwrap();
    assert_eq!(v["drift"], sample["drift"], "the index's drift model");
    assert_eq!(ids(&v["drift"]), [THEME, UNIT, OLLAMA, FIREFOX]);
    assert_eq!(
        (v["openDrift"].clone(), v["crisis"].clone()),
        (json!(4), json!(2))
    );
    let items = v["drift"].as_array().unwrap();
    assert_eq!(items.iter().filter(|d| d["crisis"] == true).count(), 2);
    assert_eq!(items[0]["proposedCase"], "C-2026-005");
    assert_eq!(items[3]["members"], 3);
    assert_eq!(items[3]["zone"], "yellow");
    assert_eq!(items[3]["txId"], "tx-20260930T214115");

    let crises = run(&env, &lb, &["drift", "--crisis-only"], 0);
    assert_eq!(ids(&crises["drift"]), [UNIT, OLLAMA]);
    assert_eq!(crises["openDrift"], 4, "totals count every item");

    // human output: one line per item, then the totals
    let out = env.at(GENERATED_AT, &["--logbook", lb.to_str().unwrap(), "drift"]);
    let text = common::stdout(&out);
    assert_eq!(text.lines().count(), 5, "{text}");
    assert!(text.contains("firefox (+2 more)"), "{text}");
    assert!(text.ends_with("4 open drift item(s), 2 crisis\n"), "{text}");
    assert!(
        !env.home.join(".local/state/seldon/index.json").exists(),
        "listing writes no index"
    );
}

#[test]
fn show_lists_every_open_member_of_a_group() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let v = run(&env, &lb, &["drift", "show", LIBINPUT], 0);
    assert_eq!(v["open"], true);
    assert_eq!(v["event"]["id"], LIBINPUT);
    assert_eq!(v["item"]["eventId"], FIREFOX, "the group's row");
    assert_eq!(v["txId"], "tx-20260930T214115");
    let members: Vec<&str> = v["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["subject"].as_str().unwrap())
        .collect();
    assert_eq!(
        members,
        ["firefox", "noto-fonts", "libinput"],
        "oldest first"
    );

    let single = run(&env, &lb, &["drift", "show", OLLAMA], 0);
    assert_eq!(single["members"].as_array().unwrap().len(), 1);
    assert!(single["txId"].is_null(), "a single event is no group");

    // a resolved event: shown with its resolution, no open members
    let btop = "01M1MB2M1GWZYF485HTGVZ1KS3";
    let done = run(&env, &lb, &["drift", "show", btop], 0);
    assert_eq!(done["open"], false);
    assert_eq!(done["event"]["resolution"], "explained");
    assert_eq!(
        done["event"]["resolutionDetail"],
        "Kleines Monitoring-Tool, bewusst ohne Case."
    );
    assert!(done["item"].is_null());
}

#[test]
fn link_explain_dismiss_append_valid_resolutions_and_the_rows_disappear() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let before = common::ledger(&lb).len();

    let link = run(&env, &lb, &["drift", "link", THEME, "C-2026-005"], 0);
    assert_eq!(link["resolved"], 1);
    let line = &link["events"][0];
    assert_eq!(
        (
            &line["kind"],
            &line["resolution"],
            &line["refersTo"],
            &line["case"]
        ),
        (
            &json!("resolution"),
            &json!("linked"),
            &json!(THEME),
            &json!("C-2026-005")
        )
    );
    assert_eq!(line["subject"], "tokyo-night", "the target's subject");
    assert_eq!(line["source"], "seldon");
    assert!(line.get("zone").is_none() && line.get("meta").is_none());
    assert_eq!(case(&env, &lb, "C-2026-005")["events"], json!([THEME]));

    let explain = run(
        &env,
        &lb,
        &[
            "drift",
            "explain",
            OLLAMA,
            "--risk",
            "R2",
            "--area",
            "dev-env",
            "--",
            "Codex set up ollama for local models",
        ],
        0,
    );
    assert_eq!(explain["resolved"], 1);
    let kinds: Vec<&str> = explain["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["case-created", "resolution", "case-completed"]);
    let line = &explain["events"][1];
    assert_eq!(line["resolution"], "explained");
    assert_eq!(line["detail"], "Codex set up ollama for local models");
    assert_eq!(line["case"], "C-2026-009", "the next free id");
    let new_case = &explain["case"];
    assert_valid_case(new_case);
    assert_eq!(new_case["status"], "completed");
    assert_eq!(new_case["zone"], "red", "the item's zone");
    assert_eq!(new_case["risk"], "R2");
    assert_eq!(new_case["title"], "Codex set up ollama for local models");
    assert_eq!(
        (
            &new_case["created"],
            &new_case["started"],
            &new_case["closed"]
        ),
        (
            &json!("2026-10-01"),
            &json!("2026-10-01"),
            &json!("2026-10-01")
        )
    );
    assert_eq!(new_case["events"], json!([OLLAMA]));
    assert_eq!(new_case["agents"], json!(["agent:codex"]), "who did it");
    let path = find_file(&lb.join("work/completed"), "C-2026-009-");
    let text = read(&path);
    assert!(
        text.contains("## Intent\n<!-- Warum dieser Case? Was soll danach anders sein? -->\nCodex set up ollama for local models\n"),
        "{text}"
    );
    assert!(text.contains("· created retroactively for drift"), "{text}");

    let dismiss = run(
        &env,
        &lb,
        &[
            "drift",
            "dismiss",
            UNIT,
            "--",
            "--user unit, tested by hand",
        ],
        0,
    );
    assert_eq!(dismiss["resolved"], 1);
    assert_eq!(dismiss["events"][0]["resolution"], "dismissed");
    assert_eq!(
        dismiss["events"][0]["detail"],
        "--user unit, tested by hand"
    );
    assert!(dismiss["events"][0].get("case").is_none());

    // every ledger line still validates (the schema's allOf for resolutions)
    let lines = common::ledger(&lb);
    assert_eq!(lines.len(), before + 5);
    for l in &lines[before..] {
        if l["kind"] == "resolution" {
            assert!(
                l["refersTo"].is_string() && l["resolution"].is_string(),
                "{l}"
            );
        }
    }

    // each write rebuilt the index; a full rebuild agrees
    let written = index_file(&env);
    assert_valid_index(&written);
    assert_eq!(ids(&written["drift"]), [FIREFOX]);
    run(&env, &lb, &["index", "--check"], 0);
    let index = index_file(&env);
    assert_valid_index(&index);
    assert_eq!(ids(&index["drift"]), [FIREFOX]);
    assert_eq!(ids(&drift(&env, &lb)["drift"]), [FIREFOX]);
    let event = |id: &str| -> Value {
        index["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == id)
            .cloned()
            .unwrap()
    };
    assert_eq!(event(THEME)["case"], "C-2026-005");
    assert_eq!(event(OLLAMA)["resolution"], "explained");
    assert_eq!(
        event(UNIT)["resolutionDetail"],
        "--user unit, tested by hand"
    );
    let completed = &index["cases"]["completed"];
    assert!(
        completed
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "C-2026-009")
    );
}

#[test]
fn a_group_resolves_in_one_write_and_a_rerun_writes_nothing() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let before = common::ledger(&lb).len();
    // any member names the group
    let v = run(
        &env,
        &lb,
        &["drift", "dismiss", LIBINPUT, "--", "Routine upgrade"],
        0,
    );
    assert_eq!(v["resolved"], 3);
    assert_eq!(v["txId"], "tx-20260930T214115");
    let lines = common::ledger(&lb);
    assert_eq!(lines.len(), before + 3);
    let new = &lines[before..];
    let targets: Vec<&str> = new
        .iter()
        .map(|l| l["refersTo"].as_str().unwrap())
        .collect();
    assert_eq!(targets, [FIREFOX, NOTO, LIBINPUT]);
    for l in new {
        for key in ["ts", "actor", "detail"] {
            assert_eq!(l[key], new[0][key], "{key}");
        }
        assert_eq!(l["meta"], json!({ "txId": "tx-20260930T214115" }));
        assert!(l.get("case").is_none());
    }
    // one write: ledger appended in one go, and series.drift counts it once
    let index = index_file(&env);
    let resolved: u64 = index["series"]["drift"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["week"] == "2026-W40")
        .map(|w| w["resolved"].as_u64().unwrap())
        .sum();
    let sample: Value = serde_json::from_str(&read(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/index.sample.json"),
    ))
    .unwrap();
    let was: u64 = sample["series"]["drift"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["week"] == "2026-W40")
        .map(|w| w["resolved"].as_u64().unwrap())
        .sum();
    assert_eq!(resolved, was + 1, "one group write counts once");

    // re-runs on any member, with or without --only: nothing open, nothing written
    for (id, only) in [(LIBINPUT, false), (FIREFOX, false), (NOTO, true)] {
        let mut args = vec!["drift", "dismiss", id];
        if only {
            args.push("--only");
        }
        args.extend(["--", "again"]);
        let again = run(&env, &lb, &args, 0);
        assert_eq!(again["resolved"], 0, "{again}");
        assert_eq!(again["already"]["resolution"], "dismissed");
    }
    let link = run(&env, &lb, &["drift", "link", FIREFOX, "C-2026-004"], 0);
    assert_eq!(link["resolved"], 0);
    assert_eq!(common::ledger(&lb).len(), before + 3);
    assert!(
        !case(&env, &lb, "C-2026-004")["events"]
            .as_array()
            .unwrap()
            .contains(&json!(FIREFOX))
    );
}

#[test]
fn only_leaves_the_other_members_open() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let v = run(
        &env,
        &lb,
        &["drift", "link", FIREFOX, "C-2026-004", "--only"],
        0,
    );
    assert_eq!(v["resolved"], 1);
    assert!(v["txId"].is_null());
    assert!(v["events"][0].get("meta").is_none(), "no group write");
    // the case lists the event, oldest first among its own
    let events = case(&env, &lb, "C-2026-004")["events"].clone();
    assert_eq!(events[0], FIREFOX, "09-30 is before the 10-01 events");
    assert_eq!(events.as_array().unwrap().len(), 8);

    let left = drift(&env, &lb);
    let group = left["drift"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["source"] == "pacman" && d["kind"] == "upgrade")
        .cloned()
        .unwrap();
    assert_eq!(group["eventId"], NOTO, "the new leader");
    assert_eq!(group["members"], 2);
    assert_eq!(left["openDrift"], 4);
    let show = run(&env, &lb, &["drift", "show", LIBINPUT], 0);
    assert_eq!(show["members"].as_array().unwrap().len(), 2);

    // the rest goes in one write
    let rest = run(&env, &lb, &["drift", "link", NOTO, "C-2026-004"], 0);
    assert_eq!(rest["resolved"], 2);
    assert_eq!(rest["txId"], "tx-20260930T214115");
    assert_eq!(drift(&env, &lb)["openDrift"], 3);
    let events = case(&env, &lb, "C-2026-004")["events"].clone();
    assert_eq!(
        events.as_array().unwrap()[0..3],
        [json!(FIREFOX), json!(NOTO), json!(LIBINPUT)]
    );
}

#[test]
fn ids_and_cases_are_checked_before_anything_is_written() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let before = common::ledger(&lb);
    let first = |kind: &str| -> String {
        before
            .iter()
            .find(|l| l["kind"] == kind)
            .map(|l| l["id"].as_str().unwrap().to_string())
            .unwrap()
    };
    let snapshot = first("snapshot");
    let resolution = first("resolution");
    let cases: [(&[&str], &str); 7] = [
        (&["drift", "show", "not-a-ulid"], "not an event id"),
        (
            &["drift", "dismiss", "01m3vnftf8evhwffz687n14q0c", "--", "x"],
            "not an event id",
        ),
        (
            &["drift", "show", "01M3VNFTF8EVHWFFZ687N14Q0D"],
            "unknown event",
        ),
        (&["drift", "dismiss", &snapshot, "--", "x"], "can be drift"),
        (
            &["drift", "dismiss", &resolution, "--", "x"],
            "is a resolution",
        ),
        (
            &["drift", "link", OLLAMA, "C-2026-099"],
            "unknown case C-2026-099",
        ),
        (
            &["drift", "dismiss", OLLAMA, "--", "  "],
            "must not be empty",
        ),
    ];
    for (args, message) in cases {
        let v = run(&env, &lb, args, 1);
        let got = v["error"]["message"].as_str().unwrap();
        assert!(got.contains(message), "{args:?}: {got}");
    }
    let multi = run(&env, &lb, &["drift", "explain", OLLAMA, "--", "a\nb"], 1);
    assert!(
        multi["error"]["message"]
            .as_str()
            .unwrap()
            .contains("one line")
    );
    assert_eq!(common::ledger(&lb), before, "nothing written");
    assert!(!lb.join("work/completed").read_dir().unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("C-2026-009")
    }));
    // a logbook that is not initialised: exit 3
    let none = env.tmp.path().join("nothing");
    run(&env, &none, &["drift"], 3);
}

/// Dependency auto-link (SPEC-ENGINE §5 rule 2): an agent's `yay -S zed`
/// under a case; the pacman transaction installs zed (explicit) and
/// alsa-lib (a dependency). Both carry the case after capture, both are
/// in the case file's `events:` in time order, and neither is drift.
#[test]
fn a_dependency_of_a_cased_explicit_event_is_linked() {
    let env = Env::new(Snapper::Missing);
    let t0 = "2026-10-01T10:00:00+02:00";
    let lb = env.tmp.path().join("logbook");
    let out = env.at(
        t0,
        &[
            "init",
            "--non-interactive",
            "--no-git",
            "--path",
            lb.to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let at = |now: &str, args: &[&str]| {
        let out = env.at(now, args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        out
    };
    at(t0, &["plan", "new", "--zone", "red", "--", "Install zed"]);
    at(t0, &["plan", "start", "C-2026-001"]);
    let cmd = json(&at(
        "2026-10-01T10:00:30+02:00",
        &[
            "event",
            "agent",
            "command",
            "--subject",
            "yay",
            "--actor",
            "agent:claude-code",
            "--case",
            "C-2026-001",
            "--meta",
            "command=yay -S zed",
            "--json",
        ],
    ))["event"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let log = env.tmp.path().join("pacman.log");
    std::fs::write(
        &log,
        "[2026-10-01T10:01:00+0200] [PACMAN] Running 'pacman -S --noconfirm zed'\n\
         [2026-10-01T10:01:01+0200] [ALPM] transaction started\n\
         [2026-10-01T10:01:02+0200] [ALPM] installed alsa-lib (1.2.14-1)\n\
         [2026-10-01T10:01:03+0200] [ALPM] installed zed (0.205.4-1)\n\
         [2026-10-01T10:01:03+0200] [ALPM] transaction completed\n",
    )
    .unwrap();
    let capture = || {
        let out = env
            .command(&[
                "capture", "--source", "pacman", "--since", T_SINCE, "--json",
            ])
            .env("SELDON_PACMAN_LOG", &log)
            .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        json(&out)
    };
    assert_eq!(capture()["written"], 2);

    let lines = common::ledger(&lb);
    let pacman: Vec<&Value> = lines.iter().filter(|l| l["source"] == "pacman").collect();
    let got: Vec<(&str, &Value, &str, &str)> = pacman
        .iter()
        .map(|l| {
            (
                l["subject"].as_str().unwrap(),
                &l["explicit"],
                l["actor"].as_str().unwrap(),
                l["case"].as_str().unwrap_or("-"),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("alsa-lib", &json!(false), "agent:claude-code", "C-2026-001"),
            ("zed", &json!(true), "agent:claude-code", "C-2026-001"),
        ]
    );
    let case_events = case(&env, &lb, "C-2026-001")["events"].clone();
    assert_eq!(
        case_events,
        json!([cmd, pacman[0]["id"], pacman[1]["id"]]),
        "the command, then the transaction, oldest first"
    );
    assert_eq!(drift(&env, &lb)["openDrift"], 0);
    let index = index_file(&env);
    assert_eq!(
        index["summary"]["openDrift"], 0,
        "capture rebuilt the index"
    );

    // a second capture writes nothing and leaves the case file as it is
    let file = find_file(&lb.join("work/active"), "C-2026-001-");
    let text = read(&file);
    assert_eq!(capture()["written"], 0);
    assert_eq!(read(&file), text);
}

/// Without the agent's command the same transaction is one red drift group
/// (an install is never routine); linking any member links both.
#[test]
fn a_caseless_install_is_one_group_and_links_as_one() {
    let env = Env::new(Snapper::Missing);
    let t0 = "2026-10-01T10:00:00+02:00";
    let lb = env.tmp.path().join("logbook");
    let out = env.at(
        t0,
        &[
            "init",
            "--non-interactive",
            "--no-git",
            "--path",
            lb.to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        env.at(t0, &["plan", "new", "--", "Editors"]).status.code(),
        Some(0)
    );
    let log = env.tmp.path().join("pacman.log");
    std::fs::write(
        &log,
        "[2026-10-01T10:01:00+0200] [PACMAN] Running 'pacman -S zed'\n\
         [2026-10-01T10:01:01+0200] [ALPM] transaction started\n\
         [2026-10-01T10:01:02+0200] [ALPM] installed alsa-lib (1.2.14-1)\n\
         [2026-10-01T10:01:03+0200] [ALPM] installed zed (0.205.4-1)\n\
         [2026-10-01T10:01:03+0200] [ALPM] transaction completed\n",
    )
    .unwrap();
    let out = env
        .command(&["capture", "--source", "pacman", "--since", T_SINCE])
        .env("SELDON_PACMAN_LOG", &log)
        .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let v = drift(&env, &lb);
    assert_eq!(v["drift"].as_array().unwrap().len(), 1);
    let item = &v["drift"][0];
    assert_eq!(
        (item["subject"].clone(), item["members"].clone()),
        (json!("zed"), json!(2))
    );
    assert_eq!(item["crisis"], true);
    let alsa = run(
        &env,
        &lb,
        &["drift", "show", item["eventId"].as_str().unwrap()],
        0,
    )["members"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let link = run(&env, &lb, &["drift", "link", &alsa, "C-2026-001"], 0);
    assert_eq!(link["resolved"], 2);
    assert_eq!(
        case(&env, &lb, "C-2026-001")["events"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(drift(&env, &lb)["openDrift"], 0);
}
