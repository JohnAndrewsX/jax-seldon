//! `seldon index` (WP-007): the golden test against
//! `fixtures/index.sample.json`, schema validation, the ADR-0013/ADR-0015
//! mutation checks of `scripts/validate-fixtures.py`, the banner variants,
//! atomic writes under a concurrent reader, and the ×10 timing.
//!
//! The fixture logbook is only ever read or copied; every write goes to a
//! temp dir (common::Env, AGENTS.md §6).

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use chrono::{DateTime, FixedOffset};
use serde_json::{Value, json};

use common::{Env, Snapper, TempDir, copy_dir, fixture_logbook, read};
use seldon::config::{Config, Dirs};
use seldon::index::{self, Input, build, check, load, model};
use seldon::logbook::Logbook;
use seldon::model::event::{Event, Kind, Resolution};

/// `generatedAt` of the sample: the golden run's `SELDON_NOW`.
const GENERATED_AT: &str = "2026-10-01T17:05:12+02:00";
/// The open caseless `-Syu` of 2026-09-30 (one yellow group of three).
const GROUP_TX: &str = "tx-20260930T214115";

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(rel)
}

fn json_file(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn now() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(GENERATED_AT).unwrap()
}

/// Every difference between `want` and `got`, as JSON pointer lines.
fn diff(want: &Value, got: &Value, path: &str, out: &mut Vec<String>) {
    match (want, got) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for k in keys {
                let p = format!("{path}/{k}");
                match (a.get(k), b.get(k)) {
                    (Some(x), Some(y)) => diff(x, y, &p, out),
                    (Some(x), None) => {
                        out.push(format!("{p}: missing in engine output (fixture {x})"))
                    }
                    (None, Some(y)) => out.push(format!("{p}: not in fixture (engine {y})")),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                out.push(format!(
                    "{path}: fixture has {} items, engine {}",
                    a.len(),
                    b.len()
                ));
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                diff(x, y, &format!("{path}/{i}"), out);
            }
        }
        _ if want != got => out.push(format!("{path}: fixture {want} != engine {got}")),
        _ => {}
    }
}

/// The comparison the WP defines: modulo `generatedAt`, `engineVersion`,
/// `logbook.git`; `logbook.path` is where the copy lives (not derivable
/// either, fixtures/README.md).
fn normalise(v: &mut Value) {
    v["generatedAt"] = json!("<generatedAt>");
    v["engineVersion"] = json!("<engineVersion>");
    let lb = v["logbook"].as_object_mut().unwrap();
    lb.remove("git");
    lb.insert("path".into(), json!("<path>"));
}

/// `cursors.json` of a capture at 17:05:00 with every collector ok (the
/// sample's `state`), bound to `logbook`; one collector optionally
/// degraded with the engine's message: `(name, message)`.
fn write_cursors(env: &Env, logbook: &Path, degraded: Option<(&str, &str)>) {
    let mut collectors = serde_json::Map::new();
    for name in ["snapper", "pacman", "omarchy", "plugins", "theme", "config"] {
        let mut c = json!({ "ok": true, "lastRun": "2026-10-01T17:05:00+02:00", "events": 0 });
        if let Some((which, message)) = degraded
            && which == name
        {
            c["ok"] = json!(false);
            c["message"] = json!(message);
        }
        collectors.insert(name.into(), c);
    }
    let state = env.home.join(".local/state/seldon");
    std::fs::create_dir_all(&state).unwrap();
    let cursors = json!({
        "logbook": std::fs::canonicalize(logbook).unwrap(),
        "collectors": collectors,
    });
    std::fs::write(state.join("cursors.json"), cursors.to_string()).unwrap();
}

/// A copy of the fixture logbook in `env` (changed by `prepare`), indexed
/// at the sample's time.
fn golden_run(
    env: &Env,
    degraded: Option<(&str, &str)>,
    prepare: impl FnOnce(&Path),
) -> (PathBuf, Value, Value) {
    let lb = env.tmp.path().join("logbook");
    copy_dir(&fixture_logbook(), &lb);
    prepare(&lb);
    write_cursors(env, &lb, degraded);
    let out = env.at(
        GENERATED_AT,
        &[
            "--logbook",
            lb.to_str().unwrap(),
            "index",
            "--check",
            "--json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        common::stdout(&out),
        common::stderr(&out)
    );
    let index = json_file(&env.home.join(".local/state/seldon/index.json"));
    (lb, common::json(&out), index)
}

fn assert_same(mut want: Value, mut got: Value, what: &str) {
    normalise(&mut want);
    normalise(&mut got);
    let mut d = Vec::new();
    diff(&want, &got, "", &mut d);
    assert!(
        d.is_empty(),
        "{what}: {} difference(s):\n{}",
        d.len(),
        d.join("\n")
    );
}

#[test]
fn golden_index_equals_the_sample() {
    let env = Env::new(Snapper::Missing);
    let (lb, out, index) = golden_run(&env, None, |_| {});
    assert_eq!(out["valid"], json!(true));
    assert_eq!(index["generatedAt"], json!(GENERATED_AT));
    assert_eq!(index["engineVersion"], json!(env!("CARGO_PKG_VERSION")));
    assert_eq!(index["logbook"]["path"], json!(lb.display().to_string()));
    assert!(
        index["logbook"].get("git").is_none(),
        "the copy is no repository"
    );
    common::assert_valid_index(&index);
    assert_same(
        json_file(&repo("fixtures/index.sample.json")),
        index,
        "golden",
    );
}

#[test]
fn ledger_views_equal_the_fixture_views() {
    let env = Env::new(Snapper::Missing);
    let (lb, out, _) = golden_run(&env, None, |_| {});
    for month in ["2026-09", "2026-10"] {
        let rel = format!("ledger/{month}.md");
        assert_eq!(
            read(&lb.join(&rel)),
            read(&fixture_logbook().join(&rel)),
            "{rel}"
        );
    }
    // the copy had them already: identical views are not rewritten
    assert_eq!(out["files"], json!([]));
}

#[test]
fn snapper_degraded_equals_the_variant() {
    let env = Env::new(Snapper::Missing);
    let message = "snapper: No permissions. The snapper config does not list this user in ALLOW_USERS; see `seldon doctor`.";
    let (_, _, index) = golden_run(&env, Some(("snapper", message)), |_| {});
    common::assert_valid_index(&index);
    assert_same(
        json_file(&repo("fixtures/index-variants/snapper-degraded.json")),
        index,
        "snapper-degraded",
    );
}

#[test]
fn plugins_degraded_equals_the_variant() {
    // the plugins collector's own message for a timed-out shell IPC call
    // (collectors/plugins.rs: `{WHAT}: timed out`)
    let env = Env::new(Snapper::Missing);
    let degraded = Some(("plugins", "omarchy plugin list --json: timed out"));
    let (_, _, index) = golden_run(&env, degraded, |_| {});
    common::assert_valid_index(&index);
    assert_same(
        json_file(&repo("fixtures/index-variants/plugins-degraded.json")),
        index,
        "plugins-degraded",
    );
}

#[test]
fn omarchy_git_checkout_equals_the_variant() {
    // `repoHead` comes from the dossier fence `omarchy.summary`, which the
    // omarchy collector fills on a git checkout (SPEC-ENGINE §4)
    let env = Env::new(Snapper::Missing);
    let (_, _, index) = golden_run(&env, None, |lb| {
        let path = lb.join("system/omarchy.md");
        let text = read(&path).replacen(
            "- version: 4.0.7-1\n",
            "- version: 4.0.7-1\n- repoHead: 3f9c2e1\n",
            1,
        );
        assert!(text.contains("repoHead"));
        std::fs::write(&path, text).unwrap();
    });
    common::assert_valid_index(&index);
    assert_same(
        json_file(&repo("fixtures/index-variants/omarchy-git-checkout.json")),
        index,
        "omarchy-git-checkout",
    );
}

#[test]
fn not_initialised_writes_the_banner_index_and_exits_3() {
    let env = Env::new(Snapper::Missing);
    let missing = env.tmp.path().join("nothing-here");
    for cmd in ["index", "status"] {
        let out = env.at(
            GENERATED_AT,
            &["--logbook", missing.to_str().unwrap(), cmd, "--json"],
        );
        assert_eq!(out.status.code(), Some(3), "{cmd}");
        assert_eq!(common::json(&out)["error"]["code"], json!(3));
        let index = json_file(&env.home.join(".local/state/seldon/index.json"));
        common::assert_valid_index(&index);
        assert_eq!(
            index["logbook"]["path"],
            json!(missing.display().to_string())
        );
        assert_same(
            json_file(&repo("fixtures/index-variants/not-initialised.json")),
            index,
            "not-initialised",
        );
        assert!(!missing.exists(), "nothing is created at the logbook path");
    }
}

#[test]
fn the_engine_checker_agrees_with_jsonschema() {
    let v = check::Validator::new();
    let sample = json_file(&repo("fixtures/index.sample.json"));
    assert_eq!(
        v.validate(&sample, "index.schema.json"),
        Vec::<String>::new()
    );
    assert!(common::index_errors(&sample).is_empty());
    // every banner variant, whatever WP-015 adds
    let mut variants = 0;
    for entry in std::fs::read_dir(repo("fixtures/index-variants")).unwrap() {
        let path = entry.unwrap().path();
        let x = json_file(&path);
        assert!(
            v.validate(&x, "index.schema.json").is_empty(),
            "{}",
            path.display()
        );
        assert!(common::index_errors(&x).is_empty(), "{}", path.display());
        variants += 1;
    }
    assert!(variants >= 5, "{variants} variants");
    // every must-fail fixture fails both
    let invalid = repo("fixtures/invalid");
    let mut n = 0;
    for entry in std::fs::read_dir(&invalid).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let schema = format!("{}.schema.json", name.split('.').next().unwrap());
        let x = json_file(&path);
        assert!(
            !v.validate(&x, &schema).is_empty(),
            "{name} must fail the engine checker"
        );
        if schema == "index.schema.json" {
            assert!(
                !common::index_errors(&x).is_empty(),
                "{name} must fail jsonschema"
            );
        }
        n += 1;
    }
    assert!(n >= 8, "fixtures/invalid has {n} files");
    // and both catch a field the closed schema does not know
    let mut extra = sample.clone();
    extra["drift"][0]["note"] = json!("x");
    assert!(!v.validate(&extra, "index.schema.json").is_empty());
    assert!(!common::index_errors(&extra).is_empty());
}

// --------------------------------------------------------------------------
// The derivation rules, in-process on the read-only fixture logbook: the
// mutation self-checks of scripts/validate-fixtures.py (ADR-0013 §1-§4,
// ADR-0015 §4).
// --------------------------------------------------------------------------

fn fixture_loaded() -> load::Loaded {
    let logbook = Logbook::open(&fixture_logbook()).unwrap();
    load::load(&logbook, now().date_naive()).unwrap()
}

fn input() -> Input {
    Input {
        now: now(),
        logbook_path: "/home/user/Seldon".into(),
        language: "de".into(),
        machine: "workstation-7f3a".into(),
        git: None,
        state: model::State {
            status: model::Status::Ok,
            last_capture: None,
            collectors: Vec::new(),
        },
        always_red: Config::default().drift.always_red,
    }
}

fn derive(mutate: impl FnOnce(&mut load::Loaded)) -> model::Index {
    let mut loaded = fixture_loaded();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    mutate(&mut loaded);
    build::build(loaded, &input()).index
}

fn members(loaded: &mut load::Loaded) -> Vec<&mut Event> {
    loaded
        .events
        .iter_mut()
        .filter(|e| e.tx_id.as_deref() == Some(GROUP_TX))
        .collect()
}

/// The group's items (at most one may exist): (zone, crisis, members).
fn group(ix: &model::Index) -> Option<(String, bool, usize)> {
    let items: Vec<&model::DriftItem> = ix
        .drift
        .iter()
        .filter(|d| {
            d.tx_id.as_deref() == Some(GROUP_TX)
                || (d.source == "pacman" && d.ts.starts_with("2026-09-30"))
        })
        .collect();
    assert!(items.len() <= 1, "{} items for the group", items.len());
    items
        .first()
        .map(|d| (d.zone.clone().unwrap(), d.crisis, d.members.unwrap_or(1)))
}

fn resolution(target: &Event, i: u8, fan_out: bool) -> Event {
    let mut r: Event = serde_json::from_value(json!({
        "id": format!("7ZZZZZZZZZZZZZZZZZZZZZZZ{i:02}"),
        "ts": "2026-10-01T16:50:00+02:00", "source": "seldon", "kind": "resolution",
        "subject": target.subject, "detail": "Routine.", "actor": "human",
        "refersTo": target.id, "resolution": "explained",
    }))
    .unwrap();
    if fan_out {
        r.meta.tx_id = Some(GROUP_TX.into());
    }
    r
}

#[test]
fn drift_group_zone_rules() {
    let yellow = Some(("yellow".to_string(), false, 3));
    let red = Some(("red".to_string(), true, 3));
    type Mutation = Box<dyn FnOnce(&mut load::Loaded)>;
    type Group = Option<(String, bool, usize)>;
    let first = |f: fn(&mut Event)| -> Mutation { Box::new(move |l| f(members(l).remove(0))) };
    let command = |c: &'static str| -> Mutation {
        Box::new(move |l| {
            for m in members(l) {
                m.meta.command = Some(c.into());
            }
        })
    };
    let cases: Vec<(&str, Mutation, Group)> = vec![
        ("unchanged", Box::new(|_| {}), yellow.clone()),
        (
            "one member explicit",
            first(|e| e.explicit = Some(true)),
            red.clone(),
        ),
        (
            "explicit missing errs red",
            first(|e| e.explicit = None),
            red.clone(),
        ),
        (
            "member subject linux",
            first(|e| e.subject = "linux".into()),
            red.clone(),
        ),
        (
            "linux-firmware is not a kernel",
            first(|e| e.subject = "linux-firmware".into()),
            yellow.clone(),
        ),
        (
            "limine-snapper-sync (glob)",
            first(|e| e.subject = "limine-snapper-sync".into()),
            red.clone(),
        ),
        (
            "member subject sddm (login)",
            first(|e| e.subject = "sddm".into()),
            red.clone(),
        ),
        (
            "member subject quickshell",
            first(|e| e.subject = "quickshell".into()),
            red.clone(),
        ),
        (
            "member kind install",
            first(|e| e.kind = Kind::Install),
            red.clone(),
        ),
        (
            "member kind reinstall",
            first(|e| e.kind = Kind::Reinstall),
            yellow.clone(),
        ),
        (
            "command names a package",
            command("pacman -Syu ollama"),
            red.clone(),
        ),
        (
            "command after --",
            command("pacman -Syu -- ollama"),
            red.clone(),
        ),
        ("command without -u", command("pacman -Sy"), red.clone()),
        (
            "command -S -u --needed",
            command("pacman -S -u --needed"),
            yellow.clone(),
        ),
        (
            "long options",
            command("pacman --sync --sysupgrade --refresh"),
            yellow.clone(),
        ),
        (
            "--overwrite takes its argument",
            command("pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*"),
            yellow.clone(),
        ),
        (
            "-r takes its argument",
            command("pacman -Syur /mnt"),
            yellow.clone(),
        ),
        (
            "unknown option and a word",
            command("pacman -Syu --frobnicate ollama"),
            red.clone(),
        ),
        ("command yay", command("yay -Syu"), red.clone()),
        (
            "--only resolves one member",
            Box::new(|l| {
                let r = resolution(&members(l)[0].clone(), 0, false);
                l.events.push(r);
            }),
            Some(("yellow".to_string(), false, 2)),
        ),
    ];
    let mut failed = Vec::new();
    for (label, mutate, want) in cases {
        let got = group(&derive(mutate));
        if got != want {
            failed.push(format!("{label}: got {got:?}, want {want:?}"));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn a_fan_out_resolution_counts_once() {
    let ix = derive(|l| {
        let targets: Vec<Event> = members(l).into_iter().map(|e| e.clone()).collect();
        for (i, t) in targets.iter().enumerate() {
            l.events.push(resolution(t, i as u8, true));
        }
    });
    assert_eq!(group(&ix), None, "the group is resolved");
    let w40 = ix
        .series
        .drift
        .iter()
        .find(|w| w.week == "2026-W40")
        .unwrap();
    assert_eq!(w40.resolved, 3, "two in the sample, plus one write");
    let folded = ix
        .events
        .iter()
        .filter(|e| e.event.tx_id.as_deref() == Some(GROUP_TX))
        .collect::<Vec<_>>();
    assert_eq!(folded.len(), 3);
    for e in folded {
        assert_eq!(e.event.resolution, Some(Resolution::Explained));
        assert_eq!(e.resolution_detail.as_deref(), Some("Routine."));
    }
}

/// ADR-0021: the winning resolution folds its `case` whether it is linked
/// or explained; an explained line without a case folds none; a later
/// resolution without a case wins and takes it away again.
#[test]
fn a_resolution_folds_its_case_whether_linked_or_explained() {
    let event = |ix: &model::Index, id: &str| {
        ix.events
            .iter()
            .find(|e| e.event.id.to_string() == id)
            .cloned()
            .unwrap()
    };
    let ollama = "01M3VNFTF8EVHWFFZ687N14Q0C";
    let btop = "01M1MB2M1GWZYF485HTGVZ1KS3";
    fn explained(l: &mut load::Loaded, target: &str, i: u8, case: Option<&str>) {
        let t = l
            .events
            .iter()
            .find(|e| e.id.to_string() == target)
            .cloned()
            .unwrap();
        let mut r = resolution(&t, i, false);
        r.case = case.map(String::from);
        l.events.push(r);
    }

    let ix = derive(|l| explained(l, ollama, 0, Some("C-2026-004")));
    let e = event(&ix, ollama);
    assert_eq!(e.event.resolution, Some(Resolution::Explained));
    assert_eq!(e.event.case.as_deref(), Some("C-2026-004"));
    assert!(!ix.drift.iter().any(|d| d.event_id == ollama));
    // the sample's explained lines carry no case and fold none
    let b = event(&ix, btop);
    assert_eq!(
        (b.event.resolution, b.event.case),
        (Some(Resolution::Explained), None)
    );

    let ix = derive(|l| {
        explained(l, ollama, 0, Some("C-2026-004"));
        explained(l, ollama, 1, None);
    });
    assert_eq!(event(&ix, ollama).event.case, None, "the later line wins");
}

#[test]
fn proposal_token_rule_end_to_end() {
    for (line, want) in [
        ("Install zed.", Some("C-2026-007")),
        ("Edit zed.conf", None),
        ("`extra/zed` from the repo", Some("C-2026-007")),
    ] {
        let ix = derive(|l| {
            let zed: Event = serde_json::from_value(json!({
                "id": "7ZZZZZZZZZZZZZZZZZZZZZZZZD", "ts": "2026-10-01T16:55:00+02:00",
                "source": "pacman", "kind": "upgrade", "subject": "zed",
                "detail": "0.198.4-1 → 0.198.5-1", "actor": "system", "zone": "red",
                "explicit": false, "txId": "tx-20261001T165500",
                "meta": {"command": "pacman -Syu", "from": "0.198.4-1", "to": "0.198.5-1"},
            }))
            .unwrap();
            l.events.push(zed);
            for c in &mut l.cases {
                if c.case.status.is_open() {
                    c.plan = if c.case.id == "C-2026-007" {
                        format!("- [ ] {line}\n")
                    } else {
                        "- [ ] nothing to see\n".into()
                    };
                }
            }
        });
        let got: Vec<Option<&str>> = ix
            .drift
            .iter()
            .filter(|d| d.subject == "zed")
            .map(|d| d.proposed_case.as_deref())
            .collect();
        assert_eq!(got, [want], "Plan line {line:?}");
        // the proposal shows on the case too
        let c7 = ix
            .cases
            .queued
            .iter()
            .find(|c| c.id == "C-2026-007")
            .unwrap();
        assert_eq!(
            c7.proposed_events.len(),
            usize::from(want.is_some()),
            "{line:?}"
        );
    }
}

#[test]
fn caps_of_the_view() {
    // more than 500 events and 50 closed cases: the newest are kept
    let ix = derive(|l| {
        let template = l
            .events
            .iter()
            .find(|e| e.kind == Kind::Note)
            .unwrap()
            .clone();
        for i in 0..600u32 {
            let mut e = template.clone();
            e.id = ulid::Ulid::from_parts(1, u128::from(i) + 1);
            e.ts = now() - chrono::Duration::days(400) + chrono::Duration::minutes(i64::from(i));
            l.events.push(e);
        }
        let done = l
            .cases
            .iter()
            .find(|c| c.case.status == seldon::model::CaseStatus::Completed)
            .unwrap()
            .clone();
        for i in 0..60 {
            let mut c = done.clone();
            c.case.id = format!("C-2025-{:03}", i + 100);
            c.case.closed =
                chrono::NaiveDate::from_ymd_opt(2025, 1, 1).map(|d| d + chrono::Duration::days(i));
            l.cases.push(c);
        }
        l.cases.sort_by(|a, b| a.case.id.cmp(&b.case.id));
    });
    assert_eq!(ix.events.len(), 500);
    let newest = DateTime::parse_from_rfc3339("2026-10-01T17:00:00+02:00").unwrap();
    assert_eq!(ix.events[0].event.ts, newest, "newest first");
    assert_eq!(ix.cases.completed.len(), 50);
    assert_eq!(
        ix.cases.completed[0].id, "C-2026-002",
        "newest closed first"
    );
    assert_eq!(ix.series.heatmap.len(), 366);
    assert_eq!(ix.series.heatmap.last().unwrap().date, "2026-10-01");
    let v = serde_json::to_value(&ix).unwrap();
    common::assert_valid_index(&v);
}

// --------------------------------------------------------------------------
// Atomic write (CONTRACT.md rule 2): a reader never sees a partial file.
// --------------------------------------------------------------------------

/// Reads `path` in a loop until `stop`; every read must parse as a whole
/// index. Returns the number of reads.
fn reader(path: PathBuf, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<usize> {
    std::thread::spawn(move || {
        let mut reads = 0;
        while !stop.load(Ordering::Relaxed) {
            let bytes = std::fs::read(&path).expect("the index exists after the first write");
            let v: Value = serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                panic!(
                    "partial index after {reads} reads ({} bytes): {e}",
                    bytes.len()
                )
            });
            assert_eq!(v["contractVersion"], json!(1));
            reads += 1;
        }
        reads
    })
}

#[test]
fn a_concurrent_reader_never_sees_a_partial_index() {
    let tmp = TempDir::new("atomic");
    let path = tmp.path().join("state/index.json");
    let full = derive(|_| {});
    let empty = index::not_initialised(Path::new("/x"), now());
    assert!(
        index::to_text(&full).len() > 30_000,
        "a large file makes torn reads likely"
    );
    index::write(&path, &empty).unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let handle = reader(path.clone(), stop.clone());
    for i in 0..400 {
        index::write(&path, if i % 2 == 0 { &full } else { &empty }).unwrap();
    }
    stop.store(true, Ordering::Relaxed);
    let reads = handle.join().expect("no partial read");
    assert!(reads > 0);
    // nothing left behind but the index
    let left: Vec<_> = std::fs::read_dir(tmp.path().join("state"))
        .unwrap()
        .collect();
    assert_eq!(left.len(), 1);
}

#[test]
fn a_concurrent_reader_never_sees_a_partial_index_from_the_cli() {
    let env = Env::new(Snapper::Missing);
    let lb = env.tmp.path().join("logbook");
    copy_dir(&fixture_logbook(), &lb);
    let path = env.home.join(".local/state/seldon/index.json");
    let args = ["--logbook", lb.to_str().unwrap(), "index"];
    assert_eq!(env.at(GENERATED_AT, &args).status.code(), Some(0));

    let stop = Arc::new(AtomicBool::new(false));
    let handle = reader(path, stop.clone());
    let runs = AtomicUsize::new(0);
    for _ in 0..15 {
        let out = env.at(GENERATED_AT, &args);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
        runs.fetch_add(1, Ordering::Relaxed);
    }
    stop.store(true, Ordering::Relaxed);
    assert!(handle.join().expect("no partial read") > 0);
}

// --------------------------------------------------------------------------
// Speed (SPEC-ENGINE §6): < 100 ms on the fixture logbook scaled ×10,
// asserted in release builds (`cargo test --release`, `cargo bench`).
// --------------------------------------------------------------------------

#[test]
fn index_build_on_x10_fixtures_is_fast() {
    let tmp = TempDir::new("x10");
    let root = tmp.path().join("logbook");
    let lines = common::scale::scaled_logbook(&fixture_logbook(), &root, 10);
    assert_eq!(lines, 710, "71 ledger lines ×10");
    let logbook = Logbook::open(&root).unwrap();
    let dirs = Dirs {
        home: tmp.path().into(),
        xdg_config_home: tmp.path().join("config"),
        state_dir: tmp.path().join("state"),
    };
    let config = Config::default();
    let out = dirs.index_file();
    let run = || {
        let start = Instant::now();
        let built = index::derive_at(&dirs, &config, &logbook, now()).unwrap();
        index::write(&out, &built.index).unwrap();
        (start.elapsed(), built)
    };
    let (_, built) = run();
    assert!(built.warnings.is_empty(), "{:?}", built.warnings);
    let ix = &built.index;
    assert_eq!(ix.events.len(), 500);
    assert_eq!(ix.cases.all().count(), 80);
    assert_eq!(ix.drift.len(), 40, "4 open items per copy");
    assert_eq!(ix.decisions.len(), 40);
    common::assert_valid_index(&serde_json::to_value(ix).unwrap());

    let mut times: Vec<Duration> = (0..5).map(|_| run().0).collect();
    times.sort();
    let median = times[2];
    eprintln!("index build ×10: median {median:?}, all {times:?}");
    if !cfg!(debug_assertions) {
        assert!(median < Duration::from_millis(100), "median {median:?}");
    }
}

#[test]
fn at_most_ten_snapshots_newest_first() {
    let ix = derive(|l| {
        for i in 0..15u32 {
            let ts = format!("2026-09-20T10:{i:02}:00+02:00");
            let e: Event = serde_json::from_value(json!({
                "id": ulid::Ulid::from_parts(1, u128::from(i) + 1), "ts": ts,
                "source": "snapper", "kind": "snapshot", "subject": (200 + i).to_string(),
                "detail": "bulk", "actor": "system", "meta": {"type": "single"},
            }))
            .unwrap();
            l.events.push(e);
        }
    });
    let snaps = ix.system.snapshots.as_ref().unwrap();
    assert_eq!(snaps.len(), 10);
    let numbers: Vec<i64> = snaps.iter().map(|s| s.number).collect();
    assert!(numbers.contains(&214), "the newest added one: {numbers:?}");
    assert!(
        !numbers.contains(&200),
        "the oldest added one is cut: {numbers:?}"
    );
    let ts: Vec<DateTime<FixedOffset>> = snaps
        .iter()
        .map(|s| DateTime::parse_from_rfc3339(&s.ts).unwrap())
        .collect();
    assert!(ts.windows(2).all(|w| w[0] >= w[1]), "newest first");
    // the timeline shows the same ten
    let in_timeline = ix
        .series
        .timeline
        .as_ref()
        .unwrap()
        .iter()
        .filter(|t| t.kind == "snapshot")
        .count();
    assert_eq!(in_timeline, 10);
}

#[test]
fn drift_is_capped_at_200_crises_first() {
    let base = derive(|_| {});
    let (crises0, open0) = (base.summary.crisis, base.summary.open_drift);
    let ix = derive(|l| {
        // 30 old crises (caseless explicit installs) and 220 newer yellow items
        for i in 0..30u32 {
            let e: Event = serde_json::from_value(json!({
                "id": ulid::Ulid::from_parts(1, u128::from(i) + 1),
                "ts": format!("2026-08-01T10:{i:02}:00+02:00"),
                "source": "pacman", "kind": "install", "subject": format!("pkg{i}"),
                "actor": "system", "zone": "red", "explicit": true,
            }))
            .unwrap();
            l.events.push(e);
        }
        for i in 0..220u32 {
            let e: Event = serde_json::from_value(json!({
                "id": ulid::Ulid::from_parts(2, u128::from(i) + 1),
                "ts": format!("2026-09-25T{:02}:{:02}:00+02:00", i / 60, i % 60),
                "source": "theme", "kind": "theme-set", "subject": format!("theme-{i}"),
                "actor": "human", "zone": "yellow",
            }))
            .unwrap();
            l.events.push(e);
        }
    });
    assert_eq!(
        ix.summary.open_drift,
        open0 + 250,
        "the summary counts every item"
    );
    assert_eq!(ix.summary.crisis, crises0 + 30);
    assert_eq!(ix.drift.len(), 200);
    assert_eq!(
        ix.drift.iter().filter(|d| d.crisis).count(),
        crises0 + 30,
        "every crisis is kept, even the oldest"
    );
    let ts: Vec<DateTime<FixedOffset>> = ix
        .drift
        .iter()
        .map(|d| DateTime::parse_from_rfc3339(&d.ts).unwrap())
        .collect();
    assert!(ts.windows(2).all(|w| w[0] >= w[1]), "still newest first");
    // the yellow items cut are the oldest ones
    assert!(ix.drift.iter().any(|d| d.subject == "theme-219"));
    assert!(!ix.drift.iter().any(|d| d.subject == "theme-0"));
    common::assert_valid_index(&serde_json::to_value(&ix).unwrap());
}

/// F-132: a ledger line torn inside `ü` (`0xC3`), and lines whose actor or
/// case `append` would refuse (WP-059 review), are skipped with one warning
/// that names the month and the count; `status`, `index` and `capture`
/// still run and keep the other events.
#[test]
fn a_torn_ledger_line_is_skipped_with_a_warning() {
    use std::io::Write as _;
    const AT: &str = "2026-10-03T09:00:00+02:00";
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let run = |args: &[&str]| {
        let out = env.at(AT, args);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{args:?}: {}{}",
            common::stdout(&out),
            common::stderr(&out)
        );
        out
    };
    run(&["log", "--", "Lüfter getauscht, Prüfung läuft"]);
    let month = root.join("ledger/2026-10.jsonl");
    let good = read(&month);
    let append = |bytes: &[u8]| {
        std::fs::OpenOptions::new()
            .append(true)
            .open(&month)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    };
    append(b"{\"id\":\"01M4TORN\",\"ts\":\"2026-10-03T09:00:00+02:00\",\"source\":\"manual\",\"kind\":\"note\",\"subject\":\"L\xc3");

    let warned = |out: &std::process::Output, count: &str| {
        let json = common::json(out);
        let warnings = json["warnings"].as_array().unwrap();
        assert!(
            warnings.iter().any(|w| w
                .as_str()
                .unwrap()
                .starts_with(&format!("ledger/2026-10.jsonl: {count} skipped"))),
            "{json}"
        );
    };
    let notes = || {
        json_file(&env.home.join(".local/state/seldon/index.json"))["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == json!("note"))
            .count()
    };
    warned(&run(&["status", "--json"]), "1 line");
    assert_eq!(notes(), 1);
    // writing goes on after the torn line, and so does the index
    run(&["log", "--", "second note"]);
    // a line copied by hand with an actor and a case `append` refuses
    let line = good.trim_end();
    for (from, to) in [
        (
            "\"actor\":\"human\"",
            "\"actor\":\"Robot ]] <!-- seldon:end -->\"",
        ),
        (
            "\"actor\":\"human\"",
            "\"actor\":\"human\",\"case\":\"../../x\"",
        ),
    ] {
        let bad = line.replacen(from, to, 1);
        assert_ne!(bad, line, "{line}");
        append(format!("{bad}\n").as_bytes());
    }
    warned(&run(&["index", "--check", "--json"]), "3 lines");
    assert_eq!(notes(), 2);
    let out = run(&["status", "--json"]);
    warned(&out, "3 lines");
    let status = read(&root.join("STATUS.md"));
    assert!(!status.contains("Robot"), "{status}");
    run(&["capture", "--source", "theme"]);
}

/// F-133: a corrupt `cursors.json` makes every capture fail, so every
/// enabled collector is `ok: false` with that message (a disabled one
/// stays as it is), the index carries a load warning, and the index
/// still validates (no contract change).
#[test]
fn a_corrupt_cursors_file_marks_the_collectors_failing() {
    const AT: &str = "2026-10-03T09:00:00+02:00";
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let config = env.config_file();
    let text = read(&config);
    assert!(text.contains("theme = true"), "{text}");
    std::fs::write(&config, text.replace("theme = true", "theme = false")).unwrap();
    let cursors = env.home.join(".local/state/seldon/cursors.json");
    std::fs::write(&cursors, "{").unwrap();

    let out = env.at(AT, &["index", "--check", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        common::stdout(&out),
        common::stderr(&out)
    );
    let warnings = common::json(&out)["warnings"].clone();
    assert!(
        warnings.as_array().unwrap().iter().any(|w| w
            .as_str()
            .unwrap()
            .starts_with("~/.local/state/seldon/cursors.json: corrupt or unreadable (")),
        "{warnings}"
    );
    let index = json_file(&env.home.join(".local/state/seldon/index.json"));
    common::assert_valid_index(&index);
    let rows = index["state"]["collectors"].as_array().unwrap();
    assert_eq!(rows.len(), 6);
    for row in rows {
        if row["name"] == "theme" {
            assert_eq!(row["enabled"], false, "{row}");
            assert_eq!(row["ok"], true, "{row}");
            assert!(row.get("message").is_none(), "{row}");
            continue;
        }
        assert_eq!(row["ok"], false, "{row}");
        assert_eq!(row["lastRun"], Value::Null, "{row}");
        let message = row["message"].as_str().unwrap();
        assert!(
            message.starts_with("cursors.json is corrupt or unreadable, so every capture fails ("),
            "{message}"
        );
    }

    // a readable file again: the rows come from it as before
    std::fs::remove_file(&cursors).unwrap();
    let out = env.at(AT, &["index", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let index = json_file(&env.home.join(".local/state/seldon/index.json"));
    assert!(
        index["state"]["collectors"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["ok"] == true),
        "{index}"
    );
}

/// WP-057 follow-up: a case id in two files is the user's to fix.
/// `index --check` refuses with exit 1 (tests/plan.rs); plain `index` and
/// `status` write the index and warn, with both files and the fix.
#[test]
fn a_case_id_twice_warns_in_index_and_status() {
    const AT: &str = "2026-10-03T09:00:00+02:00";
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let out = env.at(AT, &["plan", "new", "--json", "--", "Twice"]);
    assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
    let id = common::json(&out)["case"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let queued = common::find_file(&root.join("work/queued"), &id);
    let name = queued.file_name().unwrap().to_string_lossy().into_owned();
    std::fs::write(
        root.join("work/completed").join(&name),
        read(&queued).replace("status: queued", "status: completed"),
    )
    .unwrap();
    let want = format!(
        "case {id} exists more than once (work/queued/{name}, work/completed/{name}); keep one file"
    );
    for args in [["index", "--json"], ["status", "--json"]] {
        let out = env.at(AT, &args);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{args:?}: {}",
            common::stderr(&out)
        );
        let warnings = common::json(&out)["warnings"].clone();
        assert!(
            warnings
                .as_array()
                .unwrap()
                .iter()
                .any(|w| *w == json!(want)),
            "{args:?}: {warnings}"
        );
    }
    let out = env.at(AT, &["index"]);
    assert!(
        common::stdout(&out).contains(&format!("\nwarning: {want}")),
        "{}",
        common::stdout(&out)
    );
    let out = env.at(AT, &["index", "--check"]);
    assert_eq!(out.status.code(), Some(1), "{}", common::stderr(&out));
}
