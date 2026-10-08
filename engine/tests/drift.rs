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

/// The six open drift items of the fixture (ADR-0028 §2), newest first.
const THEME: &str = "01M3VTGNY0NZG4AY80814WSKGR"; // tokyo-night: routine, proposed C-2026-005
const UNIT: &str = "01M3VNJ9JGZ9169T01XCW16FT0"; // ollama.service, crisis
const OLLAMA: &str = "01M3VNFTF8EVHWFFZ687N14Q0C"; // pacman install, attention
const HOOK: &str = "01M3Q7R0Z08ZD5R76DQA3PHQ1G"; // hooks/post-update.d, crisis, yellow zone
const MONITORS: &str = "01M3KVWFR06078ZQTPRZCFYHK0"; // config-remove, attention
const MESA: &str = "01M3H6M720FC6BAG7ETNQTXW9K"; // leader of the 10-01 downgrade group
const LIB32: &str = "01M3H6M8184NVTFDTEGPD71P5H"; // group member
const VULKAN: &str = "01M3H6M818EPKV6HMJ0GN4PGFG"; // group member
/// Routine, history, not drift: the 09-30 `pacman -Syu` group.
const FIREFOX: &str = "01M3SXBQVR7AW8PJQC1YXDCQ14"; // leader
const NOTO: &str = "01M3SXBRV0E702XKBM22HEV1B8"; // member
const LIBINPUT: &str = "01M3SXBRV0WPNQ721VWGG2WXZ1"; // member

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

/// A list item without the field the command adds to the index's
/// (`class`; `rule` is the index's own since ADR-0038 §1).
fn as_index_item(item: &Value) -> Value {
    let mut item = item.clone();
    let map = item.as_object_mut().unwrap();
    map.remove("class");
    item
}

#[test]
fn lists_the_six_fixture_items() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let v = drift(&env, &lb);
    let sample: Value = serde_json::from_str(&read(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/index.sample.json"),
    ))
    .unwrap();
    let items = v["drift"].as_array().unwrap();
    let plain: Vec<Value> = items.iter().map(as_index_item).collect();
    assert_eq!(json!(plain), sample["drift"], "the index's drift model");
    assert_eq!(
        ids(&v["drift"]),
        [THEME, UNIT, OLLAMA, HOOK, MONITORS, MESA]
    );
    assert_eq!(
        (
            v["openDrift"].clone(),
            v["crisis"].clone(),
            v["routine"].clone()
        ),
        (json!(6), json!(2), json!(8))
    );
    let class = |i: usize| (items[i]["class"].clone(), items[i]["rule"].clone());
    assert_eq!(
        class(0),
        (json!("attention"), json!("theme")),
        "routine, but proposed"
    );
    assert_eq!(items[0]["proposedCase"], "C-2026-005");
    assert_eq!(class(1), (json!("crisis"), json!("always-red-paths")));
    assert_eq!(class(2), (json!("attention"), json!("package")));
    assert_eq!(class(3), (json!("crisis"), json!("always-red-paths")));
    assert_eq!(items[3]["zone"], "yellow", "the ledger zone");
    assert_eq!(class(4), (json!("attention"), json!("config-remove")));
    assert_eq!(class(5), (json!("attention"), json!("package")));
    assert_eq!(items[5]["members"], 3);
    assert_eq!(items[5]["zone"], "red");
    assert_eq!(items[5]["txId"], "tx-20260927T123000");

    let crises = run(&env, &lb, &["drift", "--crisis-only"], 0);
    assert_eq!(ids(&crises["drift"]), [UNIT, HOOK]);
    assert_eq!(crises["openDrift"], 6, "totals count every item");

    // --all: routine items too, uncapped, newest first
    let all = run(&env, &lb, &["drift", "--all"], 0);
    let routine: Vec<(String, String)> = all["drift"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["class"] == "routine")
        .map(|d| {
            (
                d["subject"].as_str().unwrap().to_string(),
                d["rule"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let want = [
        ("firefox", "sysupgrade"),
        ("io.github.example.weather-plus", "plugin-toggle"),
        ("~/.config/omarchy/shell.json", "routine-paths"),
        ("io.github.example.weather-plus", "plugin-toggle"),
        ("kanagawa", "theme"),
        ("catppuccin", "theme"),
        ("gtk4", "sysupgrade"),
        ("pulseaudio", "sysupgrade"),
    ];
    assert_eq!(routine, want.map(|(s, r)| (s.to_string(), r.to_string())));
    assert_eq!(all["drift"].as_array().unwrap().len(), 14);

    // human output: one line per item, then the totals
    let out = env.at(GENERATED_AT, &["--logbook", lb.to_str().unwrap(), "drift"]);
    let text = common::stdout(&out);
    assert_eq!(text.lines().count(), 7, "{text}");
    assert!(text.contains("mesa (+2 more)"), "{text}");
    assert!(
        text.lines().nth(1).unwrap().starts_with("CRISIS "),
        "{text}"
    );
    assert!(text.ends_with("6 open drift item(s), 2 crisis\n"), "{text}");
    let out = env.at(
        GENERATED_AT,
        &["--logbook", lb.to_str().unwrap(), "drift", "--all"],
    );
    let text = common::stdout(&out);
    assert!(
        text.contains("routine    2026-09-30 21:41  pacman/upgrade  firefox (+2 more)"),
        "{text}"
    );
    assert!(
        text.ends_with("6 open drift item(s), 2 crisis; 8 routine (history, not drift)\n"),
        "{text}"
    );
    assert!(
        !env.home.join(".local/state/seldon/index.json").exists(),
        "listing writes no index"
    );
}

#[test]
fn show_lists_every_open_member_of_a_group() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let v = run(&env, &lb, &["drift", "show", VULKAN], 0);
    assert_eq!(v["open"], true);
    assert_eq!(
        (v["class"].clone(), v["rule"].clone()),
        (json!("attention"), json!("package"))
    );
    assert_eq!(v["event"]["id"], VULKAN);
    assert_eq!(v["item"]["eventId"], MESA, "the group's row");
    assert_eq!(v["txId"], "tx-20260927T123000");
    let members: Vec<&str> = v["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["subject"].as_str().unwrap())
        .collect();
    assert_eq!(
        members,
        ["mesa", "lib32-mesa", "vulkan-radeon"],
        "oldest first"
    );

    let single = run(&env, &lb, &["drift", "show", OLLAMA], 0);
    assert_eq!(single["members"].as_array().unwrap().len(), 1);
    assert!(single["txId"].is_null(), "a single event is no group");
    let hook = run(&env, &lb, &["drift", "show", HOOK], 0);
    assert_eq!(
        (hook["class"].clone(), hook["item"]["crisis"].clone()),
        (json!("crisis"), json!(true))
    );

    // a routine group: not open drift, still linkable, with its rule
    let routine = run(&env, &lb, &["drift", "show", LIBINPUT], 0);
    assert_eq!(routine["open"], false);
    assert_eq!(
        (routine["class"].clone(), routine["rule"].clone()),
        (json!("routine"), json!("sysupgrade"))
    );
    assert_eq!(routine["item"]["eventId"], FIREFOX);
    assert_eq!(routine["members"].as_array().unwrap().len(), 3);
    let out = env.at(
        GENERATED_AT,
        &["--logbook", lb.to_str().unwrap(), "drift", "show", NOTO],
    );
    assert!(
        common::stdout(&out).contains(
            "Routine, history, not drift; `drift link` still takes it (rule sysupgrade):"
        ),
        "{}",
        common::stdout(&out)
    );

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
    assert!(done["class"].is_null());
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
    assert_eq!(new_case["zone"], "red", "the item's (ledger) zone");
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
    let left = [HOOK, MONITORS, MESA];
    let written = index_file(&env);
    assert_valid_index(&written);
    assert_eq!(ids(&written["drift"]), left);
    run(&env, &lb, &["index", "--check"], 0);
    let index = index_file(&env);
    assert_valid_index(&index);
    assert_eq!(ids(&index["drift"]), left);
    assert_eq!(ids(&drift(&env, &lb)["drift"]), left);
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
        event(OLLAMA)["case"],
        "C-2026-009",
        "ADR-0021: the explained event carries its new case"
    );
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
        &["drift", "dismiss", VULKAN, "--", "GPU driver rolled back"],
        0,
    );
    assert_eq!(v["resolved"], 3);
    assert_eq!(v["txId"], "tx-20260927T123000");
    let lines = common::ledger(&lb);
    assert_eq!(lines.len(), before + 3);
    let new = &lines[before..];
    let targets: Vec<&str> = new
        .iter()
        .map(|l| l["refersTo"].as_str().unwrap())
        .collect();
    assert_eq!(targets, [MESA, LIB32, VULKAN]);
    for l in new {
        for key in ["ts", "actor", "detail"] {
            assert_eq!(l[key], new[0][key], "{key}");
        }
        assert_eq!(l["meta"], json!({ "txId": "tx-20260927T123000" }));
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
    for (id, only) in [(VULKAN, false), (MESA, false), (LIB32, true)] {
        let mut args = vec!["drift", "dismiss", id];
        if only {
            args.push("--only");
        }
        args.extend(["--", "again"]);
        let again = run(&env, &lb, &args, 0);
        assert_eq!(again["resolved"], 0, "{again}");
        assert_eq!(again["already"]["resolution"], "dismissed");
    }
    let link = run(&env, &lb, &["drift", "link", MESA, "C-2026-004"], 0);
    assert_eq!(link["resolved"], 0);
    assert_eq!(common::ledger(&lb).len(), before + 3);
    assert!(
        !case(&env, &lb, "C-2026-004")["events"]
            .as_array()
            .unwrap()
            .contains(&json!(MESA))
    );
}

#[test]
fn only_leaves_the_other_members_open() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let v = run(
        &env,
        &lb,
        &["drift", "link", MESA, "C-2026-004", "--only"],
        0,
    );
    assert_eq!(v["resolved"], 1);
    assert!(v["txId"].is_null());
    assert!(v["events"][0].get("meta").is_none(), "no group write");
    // the case lists the event in time order among its own
    let events = case(&env, &lb, "C-2026-004")["events"].clone();
    let events = events.as_array().unwrap();
    assert_eq!(events.len(), 8);
    assert_eq!(events[0], MESA, "09-27 is before the 10-01 events");

    let left = drift(&env, &lb);
    let group = left["drift"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["source"] == "pacman" && d["kind"] == "downgrade")
        .cloned()
        .unwrap();
    assert_eq!(group["eventId"], LIB32, "the new leader");
    assert_eq!(group["members"], 2);
    assert_eq!(left["openDrift"], 6);
    let show = run(&env, &lb, &["drift", "show", VULKAN], 0);
    assert_eq!(show["members"].as_array().unwrap().len(), 2);

    // the rest goes in one write
    let rest = run(&env, &lb, &["drift", "link", LIB32, "C-2026-004"], 0);
    assert_eq!(rest["resolved"], 2);
    assert_eq!(rest["txId"], "tx-20260927T123000");
    assert_eq!(drift(&env, &lb)["openDrift"], 5);
    let events = case(&env, &lb, "C-2026-004")["events"].clone();
    assert_eq!(
        events.as_array().unwrap()[0..3],
        [json!(MESA), json!(LIB32), json!(VULKAN)]
    );
}

/// ADR-0028 §3, §8: a routine event is history. `link` still ties it to a
/// case (the whole transaction, or one event with `--only`); `explain` and
/// `dismiss` exit 1 and write nothing.
#[test]
fn routine_events_link_but_never_explain_or_dismiss() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let before = common::ledger(&lb);
    for args in [
        &["drift", "dismiss", NOTO, "--", "routine"][..],
        &["drift", "explain", FIREFOX, "--", "routine"][..],
    ] {
        let v = run(&env, &lb, args, 1);
        let message = v["error"]["message"].as_str().unwrap();
        assert!(
            message.contains("is routine (rule `sysupgrade`"),
            "{message}"
        );
        assert!(message.contains("drift link"), "{message}");
    }
    assert_eq!(common::ledger(&lb), before, "nothing written");

    let link = run(&env, &lb, &["drift", "link", LIBINPUT, "C-2026-004"], 0);
    assert_eq!(link["resolved"], 3);
    assert_eq!(link["txId"], "tx-20260930T214115");
    let events = case(&env, &lb, "C-2026-004")["events"].clone();
    assert_eq!(
        events.as_array().unwrap()[0..3],
        [json!(FIREFOX), json!(NOTO), json!(LIBINPUT)],
        "09-30, before the case's own"
    );
    // open drift is untouched; the group is linked, no longer linkable
    let v = drift(&env, &lb);
    assert_eq!(v["openDrift"], 6);
    assert_eq!(v["routine"], 7);
    let again = run(&env, &lb, &["drift", "link", FIREFOX, "C-2026-004"], 0);
    assert_eq!(again["resolved"], 0);
    assert_eq!(again["already"]["case"], "C-2026-004");
}

/// ADR-0028 §3, enforced: an agent may not explain or dismiss a crisis,
/// links one only to an active case that lists it in `agents`; attention
/// stays open to agents; a human is never refused. `SELDON_ACTOR` counts
/// like `--actor`.
#[test]
fn an_agent_never_whitewashes_a_crisis() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let before = common::ledger(&lb);
    let refused: [(&[&str], &str); 5] = [
        (
            &[
                "drift",
                "dismiss",
                UNIT,
                "--actor",
                "agent:codex",
                "--",
                "fine",
            ],
            "may not explain or dismiss",
        ),
        (
            &[
                "drift",
                "explain",
                HOOK,
                "--actor",
                "agent:codex",
                "--",
                "backup",
            ],
            "may not explain or dismiss",
        ),
        (
            &[
                "drift",
                "link",
                UNIT,
                "C-2026-004",
                "--actor",
                "agent:codex",
            ],
            "only to an active case that lists agent:codex",
        ),
        (
            // C-2026-002 lists the agent, but it is completed
            &[
                "drift",
                "link",
                UNIT,
                "C-2026-002",
                "--actor",
                "agent:claude-code",
            ],
            "only to an active case",
        ),
        (
            // C-2026-008 lists no agent and is in verification
            &[
                "drift",
                "link",
                HOOK,
                "C-2026-008",
                "--actor",
                "agent:claude-code",
            ],
            "only to an active case",
        ),
    ];
    for (args, message) in refused {
        let v = run(&env, &lb, args, 1);
        let got = v["error"]["message"].as_str().unwrap();
        assert!(got.contains(message), "{args:?}: {got}");
        assert!(got.contains("crisis (ADR-0028 §3)"), "{got}");
    }
    // K13: a case in verification that lists the agent is not active
    let c8 = find_file(&lb.join("work/active"), "C-2026-008-");
    let text = read(&c8);
    std::fs::write(
        &c8,
        text.replace("agents: []", "agents: [agent:claude-code]"),
    )
    .unwrap();
    let v = run(
        &env,
        &lb,
        &[
            "drift",
            "link",
            HOOK,
            "C-2026-008",
            "--actor",
            "agent:claude-code",
        ],
        1,
    );
    let got = v["error"]["message"].as_str().unwrap();
    assert!(got.contains("only to an active case"), "{got}");
    std::fs::write(&c8, text).unwrap();
    let mut all = vec!["--logbook", lb.to_str().unwrap(), "--json"];
    all.extend(["drift", "dismiss", HOOK, "--", "fine"]);
    let out = env
        .command(&all)
        .env("SELDON_NOW", GENERATED_AT)
        .env("SELDON_ACTOR", "agent:codex")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert_eq!(common::ledger(&lb), before, "nothing written");

    // attention: an agent may explain it
    let ok = run(
        &env,
        &lb,
        &[
            "drift",
            "explain",
            OLLAMA,
            "--actor",
            "agent:codex",
            "--",
            "Local models",
        ],
        0,
    );
    assert_eq!(ok["resolved"], 1);
    // its own active case: an agent may link the crisis
    let ok = run(
        &env,
        &lb,
        &[
            "drift",
            "link",
            UNIT,
            "C-2026-004",
            "--actor",
            "agent:claude-code",
        ],
        0,
    );
    assert_eq!(ok["resolved"], 1);
    // a human is never refused
    let ok = run(
        &env,
        &lb,
        &["drift", "dismiss", HOOK, "--", "my backup hook"],
        0,
    );
    assert_eq!(ok["resolved"], 1);
    assert_eq!(drift(&env, &lb)["crisis"], 0);
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
            "--no-capture",
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

/// Without the agent's command the same transaction is one drift group:
/// a package installed by name without a case is quiet attention, not a
/// crisis (ADR-0028 §4c); linking any member links both.
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
            "--no-capture",
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
    assert_eq!(item["crisis"], false);
    assert_eq!(
        (item["class"].clone(), item["rule"].clone()),
        (json!("attention"), json!("package"))
    );
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

/// Rule 1 through the hook and the shared attribution pass (WP-009): an
/// agent's in-place edit of a watched config file under the active case.
/// After `capture` the config change carries the case, and the case file
/// lists the hook's command and the change, oldest first. Real clock:
/// `capture` has no `SELDON_NOW`.
#[test]
fn a_hooked_config_edit_lands_in_the_case_file() {
    use std::io::Write as _;
    use std::process::Stdio;

    let env = Env::new(Snapper::Missing);
    let lb = env.init_logbook();
    let conf = env.home.join(".config/hypr/bindings.conf");
    std::fs::create_dir_all(conf.parent().unwrap()).unwrap();
    std::fs::write(&conf, "bindd = SUPER, E, Editor, exec, nvim\n").unwrap();
    let seldon = |args: &[&str], stdin: &str| {
        let mut child = env
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        out
    };
    seldon(&["plan", "new", "--", "Zed as editor"], "");
    seldon(&["plan", "start", "C-2026-001"], "");
    seldon(&["capture", "--source", "config"], ""); // baseline
    let payload = json!({
        "session_id": "s-1",
        "cwd": lb,
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_use_id": "toolu_1",
        "tool_input": {"command": "sed -i 's/nvim/zeditor/' ~/.config/hypr/bindings.conf"},
    });
    seldon(&["hook", "claude-code"], &payload.to_string());
    std::fs::write(&conf, "bindd = SUPER, E, Editor, exec, zeditor\n").unwrap();
    let out = seldon(&["capture", "--source", "config", "--json"], "");
    assert_eq!(json(&out)["written"], 1);

    let lines = common::ledger(&lb);
    let command = lines.iter().find(|l| l["source"] == "agent").unwrap();
    let change = lines.iter().find(|l| l["source"] == "config").unwrap();
    assert_eq!(change["kind"], "config-change");
    assert_eq!(change["actor"], "agent:claude-code");
    assert_eq!(change["case"], "C-2026-001");
    let c = case(&env, &lb, "C-2026-001");
    assert_eq!(c["events"], json!([command["id"], change["id"]]));
    assert_eq!(c["agents"], json!(["agent:claude-code"]));
    assert_eq!(drift(&env, &lb)["openDrift"], 0);
}

/// WP-101 round 2 (ADR-0027 §5): the retroactive case an agent's
/// `explain` completes is an agent's close: tag `closed-by-agent`; a
/// person's explain gets none.
#[test]
fn an_agents_explain_tags_the_completed_case() {
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let v = run(
        &env,
        &lb,
        &[
            "drift",
            "explain",
            OLLAMA,
            "--actor",
            "agent:codex",
            "--",
            "local models",
        ],
        0,
    );
    assert_eq!(v["case"]["tags"], json!(["closed-by-agent"]));
    assert_eq!(v["case"]["status"], "completed");
    let v = run(
        &env,
        &lb,
        &["drift", "explain", UNIT, "--", "tested by hand"],
        0,
    );
    assert_eq!(v["case"]["tags"], json!([]));
}
