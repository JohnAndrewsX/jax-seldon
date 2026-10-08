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

/// `fixtures/proposals/` in the state directory (the sample's `triage`,
/// ADR-0035 §6), bound to `logbook` instead of the fixture's
/// `/home/user/Seldon`.
fn write_proposals(env: &Env, logbook: &Path) {
    let dir = env.home.join(".local/state/seldon/proposals");
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(repo("fixtures/proposals")).unwrap() {
        let path = entry.unwrap().path();
        let mut p = json_file(&path);
        p["logbook"] = json!(std::fs::canonicalize(logbook).unwrap());
        std::fs::write(dir.join(path.file_name().unwrap()), p.to_string()).unwrap();
    }
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
    write_proposals(env, &lb);
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
    // the dev marker too (WP-098)
    assert_eq!(index["engineVersion"], json!(seldon::VERSION));
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

/// ADR-0028 §5: `[drift] attention = "all"` is the rollback, the
/// derivation before ADR-0028 (computed pacman zone, crisis iff red, every
/// caseless event open). `fixtures/index.attention-all.json` is derived by
/// that path of `scripts/validate-fixtures.py`.
#[test]
fn attention_all_reproduces_the_pre_adr_golden() {
    let env = Env::new(Snapper::Missing);
    std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
    std::fs::write(env.config_file(), "[drift]\nattention = \"all\"\n").unwrap();
    let (_, out, index) = golden_run(&env, None, |_| {});
    assert_eq!(out["valid"], json!(true));
    assert_same(
        json_file(&repo("fixtures/index.attention-all.json")),
        index,
        "attention = all",
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
    let message = seldon::collectors::snapper::NO_PERMISSIONS;
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
        drift: Config::default().drift,
        redactor: Some(seldon::redact::Redactor::builtin()),
    }
}

fn derive(mutate: impl FnOnce(&mut load::Loaded)) -> model::Index {
    derive_with(Config::default().drift, mutate)
}

/// [`derive`] under `[drift] attention = "all"`: the rollback, the rules
/// before ADR-0028 (ADR-0013 §3).
fn derive_all(mutate: impl FnOnce(&mut load::Loaded)) -> model::Index {
    let drift = seldon::config::DriftConfig {
        attention: seldon::config::AttentionMode::All,
        ..Config::default().drift
    };
    derive_with(drift, mutate)
}

fn derive_with(
    drift: seldon::config::DriftConfig,
    mutate: impl FnOnce(&mut load::Loaded),
) -> model::Index {
    let mut loaded = fixture_loaded();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    mutate(&mut loaded);
    build::build(loaded, &Input { drift, ..input() }).index
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

/// ADR-0013 §3, kept as the rollback (`attention = "all"`).
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
        let got = group(&derive_all(mutate));
        if got != want {
            failed.push(format!("{label}: got {got:?}, want {want:?}"));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// ADR-0028 §2 on the sample's open `-Syu` group: `None` = routine,
/// history, not drift; else (zone, crisis, members). The zone is the
/// ledger zone (red for pacman, §7).
#[test]
fn drift_group_classes() {
    let attention = Some(("red".to_string(), false, 3));
    let crisis = Some(("red".to_string(), true, 3));
    type Mutation = Box<dyn FnOnce(&mut load::Loaded)>;
    type Group = Option<(String, bool, usize)>;
    fn first(f: fn(&mut Event)) -> Mutation {
        Box::new(move |l| f(members(l).remove(0)))
    }
    fn command(c: &'static str) -> Mutation {
        Box::new(move |l| {
            for m in members(l) {
                m.meta.command = Some(c.into());
            }
        })
    }
    fn both(a: Mutation, b: Mutation) -> Mutation {
        Box::new(move |l| {
            a(l);
            b(l);
        })
    }
    let cases: Vec<(&str, Mutation, Group)> = vec![
        ("plain -Syu", Box::new(|_| {}), None),
        (
            "a kernel in it",
            first(|e| e.subject = "linux".into()),
            None,
        ),
        ("an install in it", first(|e| e.kind = Kind::Install), None),
        (
            "a :: Replace removal",
            first(|e| e.kind = Kind::Remove),
            None,
        ),
        (
            "a kernel removal",
            first(|e| {
                e.subject = "linux".into();
                e.kind = Kind::Remove;
            }),
            attention.clone(),
        ),
        (
            "a kernel downgrade",
            first(|e| {
                e.subject = "systemd".into();
                e.kind = Kind::Downgrade;
            }),
            attention.clone(),
        ),
        (
            "a downgrade",
            first(|e| e.kind = Kind::Downgrade),
            attention.clone(),
        ),
        ("-Syyuu", command("pacman -Syyuu"), None),
        ("-Su", command("pacman -Su"), None),
        ("bare yay", command("yay"), None),
        ("yay -Syu", command("yay -Syu"), None),
        ("Omarchy's update line", command(OMARCHY_UPDATE_LINE), None),
        (
            "a full upgrade naming a package",
            command("pacman -Syu ollama"),
            attention.clone(),
        ),
        ("no -u", command("pacman -Sy"), attention.clone()),
        (
            "no command line on the lead: nothing says it was plain",
            first(|e| e.meta.command = None),
            attention.clone(),
        ),
        (
            "a named upgrade",
            both(
                command("pacman -S firefox"),
                first(|e| e.explicit = Some(true)),
            ),
            None,
        ),
        (
            "a named kernel upgrade",
            both(
                command("pacman -S linux"),
                first(|e| {
                    e.explicit = Some(true);
                    e.subject = "linux".into();
                }),
            ),
            attention.clone(),
        ),
        (
            "a named kernel install",
            both(
                command("pacman -S linux"),
                first(|e| {
                    e.explicit = Some(true);
                    e.subject = "linux".into();
                    e.kind = Kind::Install;
                }),
            ),
            crisis.clone(),
        ),
        (
            "an upgrade from the cache (yay -Sua)",
            both(
                command(
                    "pacman -U /home/user/.cache/yay/firefox/firefox-143.0.2-1-x86_64.pkg.tar.zst",
                ),
                first(|e| e.explicit = Some(true)),
            ),
            None,
        ),
        (
            "the keyring",
            both(
                command("pacman -Sy --noconfirm archlinux-keyring"),
                first(|e| {
                    e.explicit = Some(true);
                    e.subject = "archlinux-keyring".into();
                    e.kind = Kind::Install;
                }),
            ),
            None,
        ),
        (
            "--only leaves routine members",
            Box::new(|l| {
                let r = resolution(&members(l)[0].clone(), 0, false);
                l.events.push(r);
            }),
            None,
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

/// ADR-0028 §5, the migration: a ledger written before ADR-0028 with
/// open theme, toggle, `omarchy update`, `-Syu` and `shell.json` drift,
/// and an old resolution of a theme switch. Under the default rules the
/// routine rows leave `drift` (nothing is written: the build is a pure
/// function of ledger and config), the old resolutions still fold, and
/// `series.drift` never goes negative; `attention = "all"` brings the
/// rows back (the rollback).
#[test]
fn migration_routine_rows_leave_drift() {
    let line = |v: Value| -> Event { serde_json::from_value(v).unwrap() };
    let syu = "pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*";
    let lines = vec![
        // August: a theme tried and dismissed, in a week of its own
        line(
            json!({"id": "01M20000000000000000000A01", "ts": "2026-08-10T10:00:00+02:00",
            "source": "theme", "kind": "theme-set", "subject": "nord", "actor": "human", "zone": "yellow"}),
        ),
        line(
            json!({"id": "01M20000000000000000000A02", "ts": "2026-08-11T10:00:00+02:00",
            "source": "seldon", "kind": "resolution", "subject": "nord", "actor": "human",
            "refersTo": "01M20000000000000000000A01", "resolution": "dismissed", "detail": "tried"}),
        ),
        // 10-01 evening, all open: an `omarchy update` with a kernel in it
        line(
            json!({"id": "7ZZZZZZZZZZZZZZZZZZZZZZM01", "ts": "2026-10-01T16:40:00+02:00",
            "source": "pacman", "kind": "upgrade", "subject": "omarchy", "detail": "4.0.7-1 → 4.0.8-1",
            "actor": "system", "zone": "red", "explicit": false, "txId": "tx-20261001T164000",
            "meta": {"command": syu, "from": "4.0.7-1", "to": "4.0.8-1"}}),
        ),
        line(
            json!({"id": "7ZZZZZZZZZZZZZZZZZZZZZZM02", "ts": "2026-10-01T16:40:01+02:00",
            "source": "pacman", "kind": "upgrade", "subject": "linux", "detail": "6.17.1-1 → 6.17.2-1",
            "actor": "system", "zone": "red", "explicit": false, "txId": "tx-20261001T164000",
            "meta": {"command": syu, "from": "6.17.1-1", "to": "6.17.2-1"}}),
        ),
        line(
            json!({"id": "7ZZZZZZZZZZZZZZZZZZZZZZM03", "ts": "2026-10-01T16:42:00+02:00",
            "source": "omarchy", "kind": "update", "subject": "omarchy", "detail": "4.0.7-1 → 4.0.8-1",
            "actor": "system", "zone": "red", "meta": {"from": "4.0.7-1", "to": "4.0.8-1"}}),
        ),
        line(
            json!({"id": "7ZZZZZZZZZZZZZZZZZZZZZZM04", "ts": "2026-10-01T16:50:00+02:00",
            "source": "theme", "kind": "theme-set", "subject": "nord", "actor": "human", "zone": "yellow"}),
        ),
        line(
            json!({"id": "7ZZZZZZZZZZZZZZZZZZZZZZM05", "ts": "2026-10-01T16:51:00+02:00",
            "source": "plugins", "kind": "plugin-disable", "subject": "io.github.example.weather-plus",
            "actor": "system", "zone": "yellow", "meta": {"enabled": false}}),
        ),
        line(
            json!({"id": "7ZZZZZZZZZZZZZZZZZZZZZZM06", "ts": "2026-10-01T16:51:01+02:00",
            "source": "config", "kind": "config-change", "subject": "~/.config/omarchy/shell.json",
            "actor": "system", "zone": "yellow", "meta": {"hashFrom": "a", "hashTo": "b"}}),
        ),
        // an Omarchy-default copy from before the marks: classified by path
        line(
            json!({"id": "7ZZZZZZZZZZZZZZZZZZZZZZM07", "ts": "2026-10-01T16:52:00+02:00",
            "source": "config", "kind": "config-change", "subject": "~/.config/hypr/looknfeel.lua",
            "actor": "system", "zone": "yellow", "meta": {"hashFrom": "c", "hashTo": "d"}}),
        ),
    ];
    // only these lines, and no Plan that names them (a proposal would show
    // a routine row as attention, ADR-0028 §3)
    let with = |l: &mut load::Loaded| {
        l.events = lines.clone();
        for c in &mut l.cases {
            c.plan = "- [ ] nothing to see\n".into();
        }
    };
    let new = derive(with);
    let old = derive_all(with);
    let ids = |ix: &model::Index| -> Vec<String> {
        ix.drift.iter().map(|d| d.event_id.clone()).collect()
    };
    assert_eq!(
        ids(&new),
        ["7ZZZZZZZZZZZZZZZZZZZZZZM07"],
        "every routine row left; an old copy without its mark is an override (by path)"
    );
    assert_eq!((new.summary.open_drift, new.summary.crisis), (1, 0));
    for id in ["M01", "M03", "M04", "M05", "M06"] {
        let id = format!("7ZZZZZZZZZZZZZZZZZZZZZZ{id}");
        assert!(
            ids(&old).contains(&id),
            "{id} is open under attention = all"
        );
    }
    assert_eq!(
        old.summary.crisis, 2,
        "the -Syu with a kernel and the update were red"
    );
    // the update stays a release marker
    let timeline = new.series.timeline.as_ref().unwrap();
    assert!(
        timeline
            .iter()
            .any(|t| t.kind == "release" && t.reference == "4.0.8-1")
    );
    // old resolutions still fold
    let folded = new
        .events
        .iter()
        .find(|e| e.event.id.to_string() == "01M20000000000000000000A01")
        .unwrap();
    assert_eq!(folded.event.resolution, Some(Resolution::Dismissed));
    // never negative: the open count after each week
    for (name, ix) in [("default", &new), ("all", &old)] {
        let mut open = 0i64;
        for w in &ix.series.drift {
            open += w.opened as i64 - w.resolved as i64;
            assert!(open >= 0, "{name}: {} goes to {open}", w.week);
        }
    }
    let aug = |ix: &model::Index| -> (usize, usize) {
        let w = ix
            .series
            .drift
            .iter()
            .find(|w| w.week == "2026-W33")
            .unwrap();
        (w.opened, w.resolved)
    };
    assert_eq!(
        aug(&new),
        (0, 0),
        "a routine switch opens nothing, its dismissal counts nothing"
    );
    assert_eq!(aug(&old), (1, 1));
}

/// The system upgrade of `omarchy update` as pacman logs it (quotes gone),
/// from `$OMARCHY_PATH/bin/omarchy-update-system-pkgs`.
const OMARCHY_UPDATE_LINE: &str = "pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*";

/// The pinned line is still Omarchy's, where Omarchy is installed.
#[test]
fn the_omarchy_update_line_is_omarchys() {
    let root = std::env::var("OMARCHY_PATH").unwrap_or_else(|_| "/usr/share/omarchy".into());
    let Ok(script) =
        std::fs::read_to_string(Path::new(&root).join("bin/omarchy-update-system-pkgs"))
    else {
        eprintln!(
            "omarchy-update-system-pkgs not installed; the pinned line is checked by the class test only"
        );
        return;
    };
    // continuation lines joined, the shell's quotes gone: what pacman logs
    let joined = script.replace("\\\n", " ");
    let line = joined
        .lines()
        .find(|l| l.contains("pacman -Syu --noconfirm"))
        .expect("the non-interactive upgrade line");
    let from = line.find("pacman -Syu").unwrap();
    let to = line[from..].find("2>").map_or(line.len(), |i| from + i);
    let logged: Vec<String> = line[from..to]
        .split_whitespace()
        .map(|w| w.replace(['\'', '"'], ""))
        .collect();
    assert_eq!(logged.join(" "), OMARCHY_UPDATE_LINE);
}

/// ADR-0013 §4, under the rollback (in the default rules the group is
/// routine and its resolutions count nowhere, ADR-0028 §5).
#[test]
fn a_fan_out_resolution_counts_once() {
    let w40 = |ix: &model::Index| {
        ix.series
            .drift
            .iter()
            .find(|w| w.week == "2026-W40")
            .unwrap()
            .resolved
    };
    let fan_out = |l: &mut load::Loaded| {
        let targets: Vec<Event> = members(l).into_iter().map(|e| e.clone()).collect();
        for (i, t) in targets.iter().enumerate() {
            l.events.push(resolution(t, i as u8, true));
        }
    };
    let routine = derive(fan_out);
    assert_eq!(
        w40(&routine),
        w40(&derive(|_| {})),
        "a routine target opened nothing"
    );
    let before = w40(&derive_all(|_| {}));
    let ix = derive_all(|l| {
        let targets: Vec<Event> = members(l).into_iter().map(|e| e.clone()).collect();
        for (i, t) in targets.iter().enumerate() {
            l.events.push(resolution(t, i as u8, true));
        }
    });
    assert_eq!(group(&ix), None, "the group is resolved");
    assert_eq!(w40(&ix), before + 1, "one write");
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
                "source": "pacman", "kind": "install", "subject": "zed",
                "detail": "0.198.5-1", "actor": "system", "zone": "red",
                "explicit": true, "txId": "tx-20261001T165500",
                "meta": {"command": "pacman -S zed", "version": "0.198.5-1"},
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
            assert_eq!(v["contractVersion"], json!(seldon::CONTRACT_VERSION));
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
    assert_eq!(lines, 870, "87 ledger lines ×10");
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
    assert_eq!(ix.drift.len(), 60, "6 open items per copy");
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
        // 30 old crises (caseless explicit installs of `alwaysRed`
        // packages) and 220 newer attention items (overrides)
        for i in 0..30u32 {
            let e: Event = serde_json::from_value(json!({
                "id": ulid::Ulid::from_parts(1, u128::from(i) + 1),
                "ts": format!("2026-08-01T10:{i:02}:00+02:00"),
                "source": "pacman", "kind": "install", "subject": format!("limine-pkg{i}"),
                "actor": "system", "zone": "red", "explicit": true,
            }))
            .unwrap();
            l.events.push(e);
        }
        for i in 0..220u32 {
            let e: Event = serde_json::from_value(json!({
                "id": ulid::Ulid::from_parts(2, u128::from(i) + 1),
                "ts": format!("2026-09-25T{:02}:{:02}:00+02:00", i / 60, i % 60),
                "source": "config", "kind": "config-change",
                "subject": format!("~/.config/hypr/theme-{i}.lua"),
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
    assert!(
        ix.drift
            .iter()
            .any(|d| d.subject.ends_with("/theme-219.lua"))
    );
    assert!(!ix.drift.iter().any(|d| d.subject.ends_with("/theme-0.lua")));
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

// --------------------------------------------------------------------------
// Size budget (CONTRACT.md rule 5, WP-076): the free texts of index events
// and drift items are clipped with a visible marker; the ledger keeps them
// whole.
// --------------------------------------------------------------------------

/// What `clip` appends: `… (N more characters in the ledger)`.
fn marked(text: &str) -> Option<usize> {
    let (_, tail) = text.rsplit_once("… (")?;
    let n = tail
        .strip_suffix(" more characters in the ledger)")
        .or_else(|| tail.strip_suffix(" more character in the ledger)"))?;
    n.parse().ok()
}

/// Bytes of `text` as a JSON string, without the quotes.
fn json_bytes(text: &str) -> usize {
    serde_json::to_string(text).unwrap().len() - 2
}

/// `DETAIL_MAX` characters that JSON escaping makes longer still.
fn long_text(tag: &str) -> String {
    let mut text = format!("{tag} ");
    while text.chars().count() < 4096 {
        text.push_str("\"cat\" <<EOF \\ päckage ✓ 🚀\t\u{1}\n");
    }
    text.chars().take(4096).collect()
}

#[test]
fn clip_keeps_short_texts_and_marks_long_ones() {
    use build::{TEXT_MAX, clip};
    for unit in ["a", "ä", "🚀", "\"", "\u{1}", " ", "ab "] {
        for repeat in 0..=300 {
            let text = unit.repeat(repeat);
            let clipped = clip(&text);
            if json_bytes(&text) <= TEXT_MAX {
                assert_eq!(clipped, text, "{unit:?}×{repeat}");
                continue;
            }
            assert!(
                json_bytes(&clipped) <= TEXT_MAX,
                "{unit:?}×{repeat}: {clipped:?}"
            );
            let left = marked(&clipped).unwrap_or_else(|| panic!("no marker: {clipped:?}"));
            let head = clipped.rsplit_once("… (").unwrap().0;
            assert!(text.starts_with(head), "{unit:?}×{repeat}");
            assert_eq!(head.chars().count() + left, text.chars().count());
        }
    }
    assert_eq!(marked(&clip(&"x".repeat(257))), Some(40));
}

#[test]
fn long_texts_keep_the_index_under_its_size_budget() {
    let mut loaded = fixture_loaded();
    let note = loaded
        .events
        .iter()
        .find(|e| e.kind == Kind::Note)
        .unwrap()
        .clone();
    let drift = loaded
        .events
        .iter()
        .find(|e| build::is_linkable(e) && e.source != seldon::model::event::Source::Pacman)
        .unwrap()
        .clone();
    let id = |i: u32| ulid::Ulid::from_parts(1_800_000_000_000, u128::from(i) + 1);
    // newer than every fixture event (the newest is at 17:00)
    let ts = |i: u32| now() - chrono::Duration::milliseconds(100 * i64::from(i));
    let mut targets = Vec::new();
    for i in 0..300u32 {
        let mut e = drift.clone();
        (e.id, e.ts, e.tx_id) = (id(i), ts(i), None);
        e.detail = Some(long_text(&format!("drift {i}")));
        loaded.events.push(e);
    }
    for i in 300..900u32 {
        let mut e = note.clone();
        (e.id, e.ts) = (id(i), ts(i));
        e.detail = Some(long_text(&format!("note {i}")));
        e.meta.command = Some(long_text(&format!("command {i}")));
        e.meta
            .extra
            .insert("tags".into(), json!(long_text(&format!("tags {i}"))));
        targets.push(e.clone());
        loaded.events.push(e);
    }
    for (i, target) in targets.iter().enumerate() {
        let mut r = resolution(target, 0, false);
        r.id = id(1000 + i as u32);
        r.detail = Some(long_text(&format!("resolution {i}")));
        loaded.events.push(r);
    }
    let built = build::build(loaded, &input());
    let ix = &built.index;
    assert_eq!(ix.events.len(), 500);
    assert_eq!(ix.drift.len(), build::MAX_DRIFT);

    let size = index::to_text(ix).len();
    eprintln!("index with 500 long events and 200 long drift items: {size} bytes");
    assert!(size < build::SIZE_BUDGET, "{size} bytes");
    assert!(
        built.warnings.iter().all(|w| !w.contains("budget")),
        "{:?}",
        built.warnings
    );
    common::assert_valid_index(&serde_json::to_value(ix).unwrap());

    let v = serde_json::to_value(ix).unwrap();
    let mut clipped = 0;
    let texts = v["events"].as_array().unwrap().iter().flat_map(|e| {
        [
            &e["detail"],
            &e["resolutionDetail"],
            &e["meta"]["command"],
            &e["meta"]["tags"],
        ]
    });
    for text in texts.chain(v["drift"].as_array().unwrap().iter().map(|d| &d["detail"])) {
        let Some(text) = text.as_str() else { continue };
        assert!(json_bytes(text) <= build::TEXT_MAX, "{text:?}");
        if marked(text).is_some() {
            clipped += 1;
        }
    }
    // the drift items of the test (the fixture's crises come first); the
    // newest 500 events: 300 drift events (detail) and 200 notes (detail,
    // command, tags, resolution detail)
    let items = ix
        .drift
        .iter()
        .filter(|d| d.detail.as_deref().is_some_and(|t| t.starts_with("drift ")))
        .count();
    assert!(items > 190, "{items}");
    assert_eq!(clipped, items + 300 + 4 * 200, "texts clipped");

    // the ledger and the folded events (the Markdown views) keep it all
    let whole = |text: &str| text.chars().count() == 4096 && marked(text).is_none();
    let note_500 = built.ledger.iter().find(|e| e.id == id(500)).unwrap();
    assert!(whole(note_500.detail.as_deref().unwrap()));
    assert!(whole(note_500.meta.command.as_deref().unwrap()));
    let folded = built.folded.iter().find(|f| f.event.id == id(500)).unwrap();
    assert!(whole(folded.resolution_detail.as_deref().unwrap()));
}

#[test]
fn an_index_over_its_budget_warns() {
    let mut loaded = fixture_loaded();
    let open = loaded
        .cases
        .iter()
        .find(|c| c.case.status == seldon::model::CaseStatus::Active)
        .unwrap()
        .clone();
    for i in 0..3000 {
        let mut c = open.clone();
        c.case.id = format!("C-2025-{i:04}");
        c.case.title = format!("{i} {}", "long title ".repeat(20));
        loaded.cases.push(c);
    }
    let built = build::build(loaded, &input());
    let size = index::to_text(&built.index).len();
    assert!(size >= build::SIZE_BUDGET, "{size} bytes");
    let want = format!(
        "index.json is {size} bytes, over its budget of 1000000 (CONTRACT.md rule 5); the largest section is `cases`"
    );
    assert!(
        built.warnings.iter().any(|w| w.starts_with(&want)),
        "{:?}",
        built.warnings
    );
}

#[test]
fn a_long_command_line_is_whole_in_the_ledger_and_clipped_in_the_index() {
    const AT: &str = "2026-10-03T09:00:00+02:00";
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let text = format!("cat <<EOF{}", " line of a heredoc".repeat(200));
    let out = env.at(
        AT,
        &[
            "event",
            "agent",
            "command",
            "--subject",
            "cat",
            "--actor",
            "agent:claude-code",
            "--detail",
            &text,
            "--meta",
            &format!("command={text}"),
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
    let line = common::ledger(&root).pop().unwrap();
    assert_eq!(line["detail"], json!(text));
    assert_eq!(line["meta"]["command"], json!(text));

    let ix = json_file(&env.home.join(".local/state/seldon/index.json"));
    let e = &ix["events"][0];
    assert_eq!(e["id"], line["id"]);
    for text in [&e["detail"], &e["meta"]["command"]] {
        let text = text.as_str().unwrap();
        assert!(marked(text).is_some(), "{text:?}");
        assert!(text.len() <= build::TEXT_MAX, "{text:?}");
    }
}

/// Long texts whose cut lands on every kind of character the clip treats
/// apart: multi-byte, JSON-escaped (2 bytes, `\b` and `\f` among them, and
/// 6), Unicode white space
/// (stripped before the marker) and the separators U+001C..U+001F, which
/// are not white space to Rust but are to Python's `str.rstrip()`.
fn clip_probes() -> Vec<String> {
    let units = [
        "ä✓🚀 ",
        "\"\\\t\u{1}",
        "a\u{1f}",
        "b\u{1c}\u{1d}",
        "c\u{2003}",
        "d\n\r ",
        "e\u{8}\u{c}",
    ];
    let mut out = Vec::new();
    for unit in units {
        for pad in 0..6 {
            let mut text = "x".repeat(pad);
            while text.chars().count() < 600 {
                text.push_str(unit);
            }
            out.push(text);
        }
    }
    out
}

/// WP-140 round 2: the reference's set of direction and format
/// characters (`scripts/validate-fixtures.py`, `DIRECTION_OR_FORMAT` and
/// `BAD_PATH` without the control characters) is the engine's
/// `import::is_direction_or_format`, code point for code point.
#[test]
fn the_reference_drops_the_engines_format_characters() {
    let python = ["python3", "python"].into_iter().find(|p| {
        std::process::Command::new(p)
            .arg("--version")
            .output()
            .is_ok()
    });
    let Some(python) = python else {
        eprintln!("skipped: no python3 on PATH (scripts/validate-fixtures.py needs it)");
        return;
    };
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/validate-fixtures.py");
    // `-B`: no `__pycache__` beside the script
    let probe = r#"
import importlib.util, sys
spec = importlib.util.spec_from_file_location("vf", sys.argv[1])
vf = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vf)
for name, rx in (("format", vf.DIRECTION_OR_FORMAT), ("path", vf.BAD_PATH)):
    for c in range(0x110000):
        if 0xD800 <= c <= 0xDFFF:
            continue
        ch = chr(c)
        if rx.match(ch) and not (name == "path" and (c < 0x20 or 0x7F <= c <= 0x9F)):
            print(name, c)
"#;
    let out = std::process::Command::new(python)
        .args(["-I", "-B", "-c", probe])
        .arg(&script)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let engine: Vec<u32> = (0..=0x10FFFF)
        .filter_map(char::from_u32)
        .filter(|c| seldon::import::is_direction_or_format(*c))
        .map(u32::from)
        .collect();
    assert!(engine.len() > 150, "{}", engine.len());
    for name in ["format", "path"] {
        let reference: Vec<u32> = stdout
            .lines()
            .filter_map(|l| l.strip_prefix(name)?.trim().parse().ok())
            .collect();
        assert_eq!(reference, engine, "{name}");
    }
}

/// ADR-0025 (WP-077): the reference `derive()` of
/// `scripts/validate-fixtures.py` clips as the engine does. A copy of the
/// fixture logbook gets caseless config events whose `detail` and `meta`
/// strings are long texts ([`clip_probes`]); half of them are dismissed
/// with a long reason (`resolutionDetail`), the others stay open drift
/// items. `index.events` and `index.drift` of `seldon index` equal those
/// of `validate-fixtures.py --derive` on the same copy, marker for marker.
#[test]
fn the_reference_derive_clips_texts_as_the_engine_does() {
    let python = ["python3", "python"].into_iter().find(|p| {
        std::process::Command::new(p)
            .arg("--version")
            .output()
            .is_ok()
    });
    let Some(python) = python else {
        eprintln!("skipped: no python3 on PATH (scripts/validate-fixtures.py needs it)");
        return;
    };
    let probes = clip_probes();
    let env = Env::new(Snapper::Missing);
    let (lb, _, index) = golden_run(&env, None, |lb| {
        let mut lines = String::new();
        for (i, text) in probes.iter().enumerate() {
            // after the fixture's newest event (17:00), before the sample's clock
            let at = |s: usize| format!("2026-10-01T17:0{}:{:02}+02:00", s / 60, s % 60);
            let id = |n: usize| ulid::Ulid::from_parts(1_800_000_000_000, n as u128).to_string();
            let subject = format!("~/.config/wp-077/{i}.conf");
            let event = json!({
                "id": id(2 * i + 1), "ts": at(2 * i), "source": "config", "kind": "config-change",
                "subject": subject, "detail": text, "actor": "human", "zone": "yellow",
                "meta": { "command": text, "note": format!("{i} {text}") },
            });
            lines.push_str(&format!("{event}\n"));
            if i % 2 == 0 {
                let reason = &probes[(i + 7) % probes.len()];
                let r = json!({
                    "id": id(2 * i + 2), "ts": at(2 * i + 1), "source": "seldon",
                    "kind": "resolution", "subject": subject, "detail": reason, "actor": "human",
                    "refersTo": id(2 * i + 1), "resolution": "dismissed",
                });
                lines.push_str(&format!("{r}\n"));
            }
        }
        // ADR-0035 §1 (WP-120 round 2, B1): 0.1.x notes with a user
        // `meta.risk`; both sides drop it from the index, the line stays
        for (n, risk) in [(90, "R1"), (91, "banana")] {
            let note = json!({
                "id": ulid::Ulid::from_parts(1_800_000_000_000, 1000 + n).to_string(),
                "ts": "2026-10-01T17:04:00+02:00", "source": "manual",
                "kind": "note", "subject": "journal", "detail": format!("risk {risk}"),
                "actor": "human", "meta": { "risk": risk, "mine": "kept" },
            });
            lines.push_str(&format!("{note}\n"));
        }
        let month = lb.join("ledger/2026-10.jsonl");
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&month)
            .unwrap();
        std::io::Write::write_all(&mut file, lines.as_bytes()).unwrap();
        // ADR-0038: a case's Intent and Result and a decision's Decision
        // hold long texts too, clipped with `in the file`
        for (rel, section, probe) in [
            ("work/queued/C-2026-005-tokyo-night.md", "## Intent\n", 3),
            (
                "work/queued/C-2026-006-snapper-retention.md",
                "## Result\n",
                10,
            ),
            ("work/completed/C-2026-001-init.md", "## Intent\n", 17),
            ("work/completed/C-2026-001-init.md", "## Result\n", 24),
            ("decisions/ADR-0002-snapshots.md", "## Decision\n", 38),
            ("decisions/ADR-0003-zed.md", "## Decision\n", 0),
        ] {
            let path = lb.join(rel);
            let text = read(&path).replacen(section, &format!("{section}{}\n\n", probes[probe]), 1);
            std::fs::write(&path, text).unwrap();
        }
    });
    for risk in ["R1", "banana"] {
        let note = index["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["detail"] == json!(format!("risk {risk}")))
            .unwrap_or_else(|| panic!("the {risk} note is listed"));
        assert_eq!(note["meta"], json!({ "mine": "kept" }), "{note}");
    }

    let out = std::process::Command::new(python)
        .arg(repo("scripts/validate-fixtures.py"))
        .args([
            "--derive",
            lb.to_str().unwrap(),
            "--today",
            &GENERATED_AT[..10],
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "validate-fixtures.py --derive: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let derived: Value = serde_json::from_slice(&out.stdout).unwrap();

    let mut d = Vec::new();
    diff(&derived["events"], &index["events"], "/events", &mut d);
    diff(&derived["drift"], &index["drift"], "/drift", &mut d);
    diff(&derived["cases"], &index["cases"], "/cases", &mut d);
    diff(
        &derived["decisions"],
        &index["decisions"],
        "/decisions",
        &mut d,
    );
    let marked_in_file = |t: &Value| {
        t.as_str()
            .is_some_and(|t| t.ends_with(" more characters in the file)"))
    };
    let c = |g: &str, id: &str| {
        index["cases"][g]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap()
            .clone()
    };
    for t in [
        c("queued", "C-2026-005")["intent"].clone(),
        c("completed", "C-2026-001")["intent"].clone(),
        c("completed", "C-2026-001")["result"].clone(),
        index["decisions"][2]["lead"].clone(),
        index["decisions"][1]["lead"].clone(),
    ] {
        assert!(marked_in_file(&t), "{t}");
    }
    assert!(
        d.is_empty(),
        "reference (fixture side) vs engine: {} difference(s):\n{}",
        d.len(),
        d.join("\n")
    );

    // every probe was clipped: 3 texts per event, a reason per dismissed
    // one, the detail of each open item
    let texts = index["events"].as_array().unwrap().iter().flat_map(|e| {
        [
            &e["detail"],
            &e["resolutionDetail"],
            &e["meta"]["command"],
            &e["meta"]["note"],
        ]
    });
    let all: Vec<&str> = texts
        .chain(
            index["drift"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| &d["detail"]),
        )
        .filter_map(Value::as_str)
        .collect();
    // and the sample's own long note of 2026-09-12 (ADR-0035 §3)
    let n = probes.len();
    assert_eq!(
        all.iter().filter(|t| marked(t).is_some()).count(),
        3 * n + n / 2 + n / 2 + 1,
        "clipped texts"
    );
    for text in &all {
        assert!(json_bytes(text) <= build::TEXT_MAX, "{text:?}");
    }

    // ADR-0035 §3: `meta.truncated` (events) and `truncated` (drift) mark
    // exactly the clipped ones
    for e in index["events"].as_array().unwrap() {
        let cut = [
            &e["detail"],
            &e["resolutionDetail"],
            &e["meta"]["command"],
            &e["meta"]["note"],
        ]
        .into_iter()
        .filter_map(Value::as_str)
        .any(|t| marked(t).is_some());
        let want = if cut { json!(true) } else { Value::Null };
        assert_eq!(e["meta"]["truncated"], want, "{}", e["id"]);
    }
    for d in index["drift"].as_array().unwrap() {
        let cut = d["detail"].as_str().is_some_and(|t| marked(t).is_some());
        let want = if cut { json!(true) } else { Value::Null };
        assert_eq!(d["truncated"], want, "{}", d["eventId"]);
    }
}

/// ADR-0035 §3: `meta.truncated` is index-only. A ledger line that carries
/// one (a hand edit) does not make an unclipped event look cut, and the
/// engine never writes one into the ledger (`seldon event --meta` refuses
/// it; `cli.rs`).
#[test]
fn a_ledger_truncated_mark_is_dropped() {
    let env = Env::new(Snapper::Missing);
    let (_, _, index) = golden_run(&env, None, |lb| {
        let month = lb.join("ledger/2026-10.jsonl");
        let text = std::fs::read_to_string(&month).unwrap();
        let text = text.replace(
            r#""meta":{"command":"omarchy update"}"#,
            r#""meta":{"command":"omarchy update","truncated":true}"#,
        );
        std::fs::write(&month, text).unwrap();
    });
    let e = index["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "01M3V4RY8GW92AWEZ8KFTHZRAW")
        .unwrap();
    assert_eq!(e["meta"], json!({ "command": "omarchy update" }));
}

// --------------------------------------------------------------------------
// SPEC-ENGINE §1 at the stated scale (WP-076): release only, `just
// check-perf` (`--profile bench --ignored`).
// --------------------------------------------------------------------------

/// `seldon status` at the scale of the budget (`scale::stated_scale`:
/// 10 788 ledger lines, 304 cases, 365 journal files): median wall time of
/// 11 runs, process start included, < 100 ms (`assert_within_budget`).
#[test]
#[ignore = "release timing at scale: `just check-perf`"]
fn status_at_10_000_ledger_lines_is_under_100_ms() {
    const BUDGET: Duration = Duration::from_millis(100);
    common::assert_optimised();
    let env = Env::new(Snapper::Missing);
    let root = env.tmp.path().join("logbook");
    let lines = common::scale::stated_scale(&fixture_logbook(), &root);
    assert_eq!(lines, 10_788);
    let args = ["--logbook", root.to_str().unwrap(), "status", "--json"];
    let out = env.at(GENERATED_AT, &args);
    assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
    let v = common::json(&out);
    assert_eq!(v["events"], json!(500));
    assert_eq!(v["warnings"], json!([]));
    let ix = json_file(&env.home.join(".local/state/seldon/index.json"));
    let cases: usize = ["queued", "active", "verification"]
        .iter()
        .map(|g| ix["cases"][g].as_array().unwrap().len())
        .sum();
    assert_eq!(cases, 228, "open cases of the stated scale");
    let files: usize = ["queued", "active", "completed"]
        .iter()
        .map(|f| {
            std::fs::read_dir(root.join("work").join(f))
                .unwrap()
                .count()
        })
        .sum();
    assert_eq!(files, 304, "case files of the stated scale");

    common::assert_within_budget("status at the stated scale", BUDGET, 11, || {
        let out = env.at(GENERATED_AT, &args);
        assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
    });
}

// --------------------------------------------------------------------------
// ADR-0038: what the desk's details show — the drift rule, a case's intent,
// result and source, a decision's lead.
// --------------------------------------------------------------------------

fn find_case<'a>(ix: &'a model::Index, id: &str) -> &'a model::IndexCase {
    ix.cases.all().find(|c| c.id == id).unwrap()
}

#[test]
fn case_and_decision_texts_are_clipped_with_the_file_marker() {
    let long = "Ä".repeat(300);
    let ix = derive(|l| {
        let c1 = l
            .cases
            .iter_mut()
            .find(|c| c.case.id == "C-2026-001")
            .unwrap();
        c1.intent = Some(long.clone());
        c1.result = Some("a\u{1b}[31mb\u{7}c\r\nzwei\tdrei".into());
        // round 2: direction and format characters go (N4), a text of
        // control characters only is no text (R3)
        let c4 = l
            .cases
            .iter_mut()
            .find(|c| c.case.id == "C-2026-004")
            .unwrap();
        c4.intent = Some("x\u{202E}evil\u{200B}zw\u{7}bell\u{2066}\u{FEFF}\u{200F}!".into());
        c4.result = Some("\u{7}\u{1b}\u{200B}\t\u{85}".into());
        let adr = l
            .decisions
            .iter_mut()
            .find(|d| d.1.id == "ADR-0003")
            .unwrap();
        adr.2 = Some(format!("{}\n{}", "x".repeat(200), "y".repeat(100)));
    });
    let c1 = find_case(&ix, "C-2026-001");
    let intent = c1.intent.as_deref().unwrap();
    assert!(intent.starts_with("ÄÄ"), "{intent}");
    assert!(
        intent.ends_with(" more characters in the file)"),
        "{intent}"
    );
    assert!(json_bytes(intent) <= build::TEXT_MAX, "{intent}");
    let shown = intent.chars().take_while(|c| *c == 'Ä').count();
    assert_eq!(
        intent,
        format!(
            "{}… ({} more characters in the file)",
            "Ä".repeat(shown),
            300 - shown
        )
    );
    // control characters other than a line break or a tab are spaces
    assert_eq!(c1.result.as_deref(), Some("a [31mb c \nzwei\tdrei"));
    let lead = ix.decisions.iter().find(|d| d.id == "ADR-0003").unwrap();
    let lead = lead.lead.as_deref().unwrap();
    let (head, marker) = lead.split_once('…').unwrap();
    let left: usize = marker
        .strip_prefix(" (")
        .and_then(|m| m.strip_suffix(" more characters in the file)"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(head.chars().count() + left, 301, "{lead}");
    assert!(
        head.starts_with(&"x".repeat(200)) && head.ends_with('y'),
        "{lead}"
    );
    assert!(json_bytes(lead) <= build::TEXT_MAX);
    let c4 = find_case(&ix, "C-2026-004");
    assert_eq!(c4.intent.as_deref(), Some("xevilzw bell!"));
    assert_eq!(c4.result, None);
    // nothing else changed: the other cases keep the fixture's texts
    assert_eq!(
        find_case(&ix, "C-2026-005").intent.as_deref(),
        Some("Ein Theme überall: Omarchy, Zed und Neovim in Tokyo Night.")
    );
}

#[test]
fn an_imported_cases_intent_is_the_paragraph_after_its_provenance() {
    let ix = derive(|_| {});
    let c7 = find_case(&ix, "C-2026-007");
    assert_eq!(c7.tags, ["imported"]);
    assert_eq!(c7.source.as_deref(), Some("~/Notizen/aufgaben.md#4"));
    assert!(
        c7.intent
            .as_deref()
            .unwrap()
            .starts_with("Herdr-Orchestrator als Default-Agent registrieren — "),
        "{:?}",
        c7.intent
    );
    // the same text without the tag is the user's own first paragraph
    let dir = TempDir::new("wp127-provenance");
    copy_dir(&fixture_logbook(), dir.path());
    let file = dir
        .path()
        .join("work/queued/C-2026-007-herdr-default-agent.md");
    std::fs::write(&file, read(&file).replace("tags: [imported]", "tags: []")).unwrap();
    let own = load::load(&Logbook::open(dir.path()).unwrap(), now().date_naive()).unwrap();
    let own = own
        .cases
        .iter()
        .find(|c| c.case.id == "C-2026-007")
        .unwrap();
    assert_eq!(
        own.intent.as_deref(),
        Some("Imported from ~/Notizen/aufgaben.md#4 — read before you start this case.")
    );
}

#[test]
fn without_a_redaction_the_texts_are_withheld() {
    let built = build::build(
        fixture_loaded(),
        &Input {
            redactor: None,
            ..input()
        },
    );
    let ix = built.index;
    assert!(
        ix.cases
            .all()
            .all(|c| c.intent.is_none() && c.result.is_none() && c.source.is_none())
    );
    assert!(ix.decisions.iter().all(|d| d.lead.is_none()));
    // the rule is the engine's own, never withheld
    assert!(ix.drift.iter().all(|d| d.rule.is_some()));
}

#[test]
fn a_source_out_of_shape_is_dropped_with_a_warning() {
    let wide = format!("~/{}", "ä".repeat(300));
    let long = format!("~/{}.md", "a".repeat(510));
    for bad in [
        "/etc/passwd",
        "notes/todo.md#1",
        "~/a\u{202E}dm.exe",
        "~/a\u{200B}b.md",
        "~/a\nb.md",
        long.as_str(),
        // 302 characters, 602 bytes: the cap is in bytes (round 2)
        wide.as_str(),
    ] {
        let mut loaded = fixture_loaded();
        loaded
            .cases
            .iter_mut()
            .find(|c| c.case.id == "C-2026-007")
            .unwrap()
            .case
            .source = Some(bad.to_string());
        let built = build::build(loaded, &input());
        assert_eq!(
            find_case(&built.index, "C-2026-007").source,
            None,
            "{bad:?}"
        );
        assert!(
            built
                .warnings
                .iter()
                .any(|w| w.starts_with("C-2026-007: its source is not a ~/ path")),
            "{bad:?}: {:?}",
            built.warnings
        );
    }
    // 512 bytes are still a source
    let fits = format!("~/{}", "a".repeat(510));
    let mut loaded = fixture_loaded();
    loaded
        .cases
        .iter_mut()
        .find(|c| c.case.id == "C-2026-007")
        .unwrap()
        .case
        .source = Some(fits.clone());
    let built = build::build(loaded, &input());
    assert_eq!(
        find_case(&built.index, "C-2026-007").source.as_deref(),
        Some(fits.as_str())
    );
    assert!(built.warnings.is_empty(), "{:?}", built.warnings);
}

#[test]
fn the_rule_follows_the_attention_mode() {
    let ix = derive(|_| {});
    let rules: Vec<_> = ix.drift.iter().map(|d| d.rule.clone().unwrap()).collect();
    assert_eq!(
        rules,
        [
            "theme",
            "always-red-paths",
            "package",
            "always-red-paths",
            "config-remove",
            "package"
        ]
    );
    let all = derive_all(|_| {});
    assert!(!all.drift.is_empty());
    assert!(
        all.drift
            .iter()
            .all(|d| d.rule.as_deref() == Some("attention-all"))
    );
}

/// The four fields are optional (an index of an earlier contract-2 build
/// has none) and closed in shape.
#[test]
fn the_new_fields_are_optional_and_checked() {
    let v = check::Validator::new();
    let sample = json_file(&repo("fixtures/index.sample.json"));
    let mut bare = sample.clone();
    for d in bare["drift"].as_array_mut().unwrap() {
        d.as_object_mut().unwrap().remove("rule");
    }
    for group in ["queued", "active", "verification", "completed"] {
        for c in bare["cases"][group].as_array_mut().unwrap() {
            let c = c.as_object_mut().unwrap();
            for k in ["intent", "result", "source"] {
                c.remove(k);
            }
        }
    }
    for d in bare["decisions"].as_array_mut().unwrap() {
        d.as_object_mut().unwrap().remove("lead");
    }
    assert_ne!(bare, sample, "the sample shows the fields");
    assert_eq!(v.validate(&bare, "index.schema.json"), Vec::<String>::new());
    assert!(common::index_errors(&bare).is_empty());
    let bad: [(&str, Value); 6] = [
        ("/drift/0/rule", json!("Always Red")),
        ("/cases/queued/2/source", json!("/etc/passwd")),
        ("/cases/queued/2/source", json!("~/a\u{202E}b")),
        (
            "/cases/queued/2/source",
            json!(format!("~/{}", "a".repeat(511))),
        ),
        ("/cases/queued/0/intent", json!("x".repeat(257))),
        ("/decisions/0/lead", json!("")),
    ];
    for (at, value) in bad {
        let mut x = sample.clone();
        *x.pointer_mut(at).unwrap() = value.clone();
        assert!(
            !v.validate(&x, "index.schema.json").is_empty(),
            "{at} = {value}"
        );
        assert!(!common::index_errors(&x).is_empty(), "{at} = {value}");
    }
}

/// WP-127 round 2 (N5): reading an Intent's first paragraphs stops after
/// them. A whole task file imported as an Intent (up to 1 MiB) costs the
/// index build no more than its first paragraphs, whatever follows.
#[test]
#[ignore = "release timing at scale: `just check-perf`"]
fn the_first_paragraphs_of_a_large_intent_cost_what_they_hold() {
    common::assert_optimised();
    let mut body = String::from(
        "## Intent\nImported from ~/x.md — read before you start this case.\n\nFix it.\n\n",
    );
    while body.len() < 1 << 20 {
        body.push_str(&"word ".repeat(20));
        body.push_str("\n<!-- a comment -->\n\n");
    }
    body.push_str("## Plan\n- x\n");
    let section = seldon::logbook::cases::section(&body, "Intent").unwrap();
    let text = &body[section];
    assert_eq!(
        seldon::logbook::cases::paragraphs(text, 2)[1],
        "Fix it.",
        "the intent after the provenance line"
    );
    common::assert_within_budget(
        "paragraphs(…, 2) of a 1 MiB Intent",
        Duration::from_micros(50),
        21,
        || {
            std::hint::black_box(seldon::logbook::cases::paragraphs(
                std::hint::black_box(text),
                2,
            ));
        },
    );
}
