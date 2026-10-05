//! Seldon updating itself (WP-086, SPEC-ENGINE §5 rule 8): an event of its
//! own plugin (`jax.seldon` updated, enabled, disabled) or of its own
//! package (`jax-seldon` upgraded, reinstalled) is explained by the capture
//! that writes it, so it is no drift; the event stays in the ledger with
//! its own actor. Other plugins and packages, and adding, installing,
//! downgrading or removing Seldon (review F4: nothing checks provenance),
//! stay drift. Own changes an earlier capture left without a resolution
//! are explained by the next capture (WP-088).
//!
//! Everything runs in a throw-away home (`common::Env`, with
//! `SELDON_TEST_GUARD`); the collectors' sources point at temp files.

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;

use common::{Env, Snapper, assert_valid_index, json, read, stderr};
use seldon::attribution::{OWN_PACKAGE, OWN_PLUGIN};
use serde_json::{Value, json};

const OTHER: &str = "io.github.example.tyme";
const T0: &str = "2026-10-01T10:00:00+02:00";

struct Machine {
    env: Env,
    logbook: PathBuf,
}

impl Machine {
    /// A logbook without a first capture; `omarchy plugin list --json`
    /// prints `<tmp>/plugins.json`, every other `omarchy` call fails.
    fn new() -> Self {
        let env = Env::new(Snapper::Missing);
        let tmp = env.tmp.path();
        common::write_executable(
            &tmp.join("omarchy-stub"),
            &format!(
                "#!/bin/sh\n[ \"$2\" = list ] && exec /bin/cat '{}'; exit 1\n",
                tmp.join("plugins.json").display()
            ),
        );
        std::fs::write(tmp.join("pacman.log"), "").unwrap();
        let logbook = tmp.join("logbook");
        let m = Machine { env, logbook };
        m.run(
            T0,
            &[
                "init",
                "--non-interactive",
                "--no-capture",
                "--no-git",
                "--path",
                m.logbook.to_str().unwrap(),
            ],
        );
        m
    }

    fn run(&self, now: &str, args: &[&str]) -> Output {
        let tmp = self.env.tmp.path();
        let out = self
            .env
            .command(args)
            .env("SELDON_NOW", now)
            .env("SELDON_OMARCHY", tmp.join("omarchy-stub"))
            .env("SELDON_OMARCHY_PLUGINS_DIR", tmp.join("plugins"))
            .env("SELDON_PACMAN_LOG", tmp.join("pacman.log"))
            .env("SELDON_PACMAN_DB_LOCK", tmp.join("no-db.lck"))
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        out
    }

    /// The listed plugins `(id, enabled, manifest version)`.
    fn plugins(&self, plugins: &[(&str, bool, &str)]) {
        let dir = self.env.tmp.path().join("plugins");
        let mut list = Vec::new();
        for (id, enabled, version) in plugins {
            let manifest = dir.join(id).join("manifest.json");
            std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
            std::fs::write(
                &manifest,
                json!({ "id": id, "version": version }).to_string(),
            )
            .unwrap();
            list.push(json!({ "id": id, "enabled": enabled, "firstParty": false }));
        }
        std::fs::write(
            self.env.tmp.path().join("plugins.json"),
            Value::from(list).to_string(),
        )
        .unwrap();
    }

    /// `capture --source <source> --json` at `now` (`--since T0` for a
    /// collector without a cursor), asserted ok.
    fn capture(&self, now: &str, source: &str) -> Value {
        let c = json(&self.run(
            now,
            &["capture", "--source", source, "--since", T0, "--json"],
        ));
        assert_eq!(c["ok"], true, "{c}");
        c
    }

    fn ledger(&self) -> Vec<Value> {
        common::ledger(&self.logbook)
    }

    /// The last `kind` event of `subject` and the resolutions that refer
    /// to it.
    fn event(&self, kind: &str, subject: &str) -> (Value, Vec<Value>) {
        let lines = self.ledger();
        let event = lines
            .iter()
            .rev()
            .find(|e| e["kind"] == kind && e["subject"] == subject)
            .unwrap_or_else(|| panic!("{kind} {subject} in {lines:?}"))
            .clone();
        let resolutions = lines
            .iter()
            .filter(|e| e["kind"] == "resolution" && e["refersTo"] == event["id"])
            .cloned()
            .collect();
        (event, resolutions)
    }

    /// The open drift rows as `(kind, subject)`.
    fn drift(&self, now: &str) -> Vec<(String, String)> {
        let v = json(&self.run(now, &["drift", "--json"]));
        v["drift"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                (
                    d["kind"].as_str().unwrap().to_string(),
                    d["subject"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    fn index(&self) -> Value {
        let file = self.env.home.join(".local/state/seldon/index.json");
        serde_json::from_str(&read(&file)).unwrap()
    }
}

/// `resolutions` is the one line rule 8 writes for `event`.
fn assert_explained_as_own(event: &Value, resolutions: &[Value], detail: &str) {
    assert_eq!(resolutions.len(), 1, "{event}: {resolutions:?}");
    let r = &resolutions[0];
    assert_eq!(r["source"], "seldon", "{r}");
    assert_eq!(r["actor"], "system", "{r}");
    assert_eq!(r["resolution"], "explained", "{r}");
    assert_eq!(r["subject"], event["subject"], "{r}");
    assert_eq!(r["detail"], detail, "{r}");
    assert!(r.get("case").is_none(), "{r}");
}

fn pair(kind: &str, subject: &str) -> (String, String) {
    (kind.to_string(), subject.to_string())
}

#[test]
fn updating_its_own_plugin_is_no_drift_another_plugin_is() {
    let m = Machine::new();
    m.plugins(&[(OWN_PLUGIN, true, "0.1.0"), (OTHER, true, "1.0")]);
    assert_eq!(m.capture(T0, "plugins")["written"], 0, "baseline");

    m.plugins(&[(OWN_PLUGIN, true, "0.1.2"), (OTHER, true, "1.1")]);
    let now = "2026-10-01T10:20:00+02:00";
    let c = m.capture(now, "plugins");
    assert_eq!(c["written"], 2, "{c}");
    assert_eq!(c["explainedSelf"], 1, "{c}");
    assert_eq!(c["explainedOwn"], 0, "{c}");

    // the update stays in the ledger as the collector saw it, explained
    let (own, resolutions) = m.event("plugin-update", OWN_PLUGIN);
    assert_eq!(own["actor"], "system", "{own}");
    assert_eq!(own["detail"], "0.1.0 → 0.1.2", "{own}");
    assert!(own.get("case").is_none(), "{own}");
    assert_explained_as_own(&own, &resolutions, "seldon's own plugin");
    let (other, resolutions) = m.event("plugin-update", OTHER);
    assert!(resolutions.is_empty(), "{other}: {resolutions:?}");
    assert_eq!(m.drift(now), [pair("plugin-update", OTHER)]);

    // the index (the Changelog) shows it, explained
    let ix = m.index();
    assert_valid_index(&ix);
    assert_eq!(ix["summary"]["openDrift"], 1);
    let row = ix["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == own["id"])
        .expect("the update is in the index");
    assert_eq!(row["resolution"], "explained", "{row}");
    assert_eq!(row["resolutionDetail"], "seldon's own plugin", "{row}");

    // idempotent: the next capture writes and explains nothing
    let lines = m.ledger().len();
    let again = m.capture("2026-10-01T10:40:00+02:00", "plugins");
    assert_eq!(
        (again["written"].clone(), again["explainedSelf"].clone()),
        (0.into(), 0.into()),
        "{again}"
    );
    assert_eq!(m.ledger().len(), lines);
}

/// Enabling and disabling Seldon's plugin is no drift; adding it (nothing
/// checks where the clone came from) and removing it is (review F4).
#[test]
fn enabling_disabling_its_plugin_is_no_drift_adding_removing_it_is() {
    let m = Machine::new();
    m.plugins(&[(OTHER, true, "1.0")]);
    m.capture(T0, "plugins"); // baseline

    m.plugins(&[(OWN_PLUGIN, false, "0.1.2"), (OTHER, true, "1.0")]);
    let added = "2026-10-01T10:10:00+02:00";
    let c = m.capture(added, "plugins");
    assert_eq!(c["explainedSelf"], 0, "{c}");
    let (_, resolutions) = m.event("plugin-add", OWN_PLUGIN);
    assert!(resolutions.is_empty(), "{resolutions:?}");
    assert_eq!(m.drift(added), [pair("plugin-add", OWN_PLUGIN)]);

    // (capture time, Seldon's plugin enabled, the event it gives)
    let steps = [
        ("2026-10-01T10:20:00+02:00", true, "plugin-enable"),
        ("2026-10-01T10:30:00+02:00", false, "plugin-disable"),
    ];
    for (now, enabled, kind) in steps {
        m.plugins(&[(OWN_PLUGIN, enabled, "0.1.2"), (OTHER, true, "1.0")]);
        let c = m.capture(now, "plugins");
        assert_eq!(c["explainedSelf"], 1, "{kind}: {c}");
        let (event, resolutions) = m.event(kind, OWN_PLUGIN);
        assert_explained_as_own(&event, &resolutions, "seldon's own plugin");
        assert_eq!(m.drift(now), [pair("plugin-add", OWN_PLUGIN)], "{kind}");
    }

    // removing Seldon's panel is a change to the system like any other
    m.plugins(&[(OTHER, true, "1.0")]);
    let now = "2026-10-01T10:40:00+02:00";
    let c = m.capture(now, "plugins");
    assert_eq!(c["explainedSelf"], 0, "{c}");
    let (_, resolutions) = m.event("plugin-remove", OWN_PLUGIN);
    assert!(resolutions.is_empty(), "{resolutions:?}");
    // newest first
    assert_eq!(
        m.drift(now),
        [
            pair("plugin-remove", OWN_PLUGIN),
            pair("plugin-add", OWN_PLUGIN)
        ]
    );
}

/// An agent's `omarchy plugin update jax.seldon` (no active case): the
/// event is the agent's, and still Seldon updating itself.
#[test]
fn an_agents_update_of_its_plugin_keeps_the_actor_and_is_no_drift() {
    let m = Machine::new();
    m.plugins(&[(OWN_PLUGIN, true, "0.1.0")]);
    m.capture(T0, "plugins"); // baseline
    let command = format!("command=omarchy plugin update {OWN_PLUGIN}");
    m.run(
        "2026-10-01T10:05:00+02:00",
        &[
            "event",
            "agent",
            "command",
            "--subject",
            "omarchy",
            "--actor",
            "agent:claude-code",
            "--meta",
            &command,
        ],
    );
    m.plugins(&[(OWN_PLUGIN, true, "0.1.2")]);
    let now = "2026-10-01T10:20:00+02:00";
    m.capture(now, "plugins");
    let (event, resolutions) = m.event("plugin-update", OWN_PLUGIN);
    assert_eq!(event["actor"], "agent:claude-code", "{event}");
    assert_explained_as_own(&event, &resolutions, "seldon's own plugin");
    assert_eq!(m.drift(now), []);
}

/// One `pacman -Syu` that upgrades the engine's package and another one:
/// the engine's member is explained, the other is the drift item.
#[test]
fn upgrading_its_own_package_is_no_drift_another_package_is() {
    let m = Machine::new();
    let log = format!(
        "[2026-10-01T10:03:00+0200] [PACMAN] Running 'pacman -Syu'\n\
         [2026-10-01T10:03:01+0200] [ALPM] transaction started\n\
         [2026-10-01T10:03:02+0200] [ALPM] upgraded {OWN_PACKAGE} (0.1.0-1 -> 0.1.2-1)\n\
         [2026-10-01T10:03:02+0200] [ALPM] upgraded zed (1.0-1 -> 1.1-1)\n\
         [2026-10-01T10:03:02+0200] [ALPM] transaction completed\n"
    );
    std::fs::write(m.env.tmp.path().join("pacman.log"), &log).unwrap();
    let now = "2026-10-01T10:20:00+02:00";
    let c = m.capture(now, "pacman");
    assert_eq!(c["written"], 2, "{c}");
    assert_eq!(c["explainedSelf"], 1, "{c}");
    let (own, resolutions) = m.event("upgrade", OWN_PACKAGE);
    assert_eq!(own["actor"], "system", "{own}");
    assert_explained_as_own(&own, &resolutions, "seldon's own package");
    let (zed, _) = m.event("upgrade", "zed");
    assert_eq!(own["txId"], zed["txId"], "one transaction");
    assert_eq!(m.drift(now), [pair("upgrade", "zed")]);
    assert_valid_index(&m.index());

    // idempotent
    let lines = m.ledger().len();
    let again = m.capture("2026-10-01T10:40:00+02:00", "pacman");
    assert_eq!(again["written"], 0, "{again}");
    assert_eq!(again["explainedSelf"], 0, "{again}");
    assert_eq!(m.ledger().len(), lines);

    // removing the engine's package stays drift
    let log = format!(
        "{log}[2026-10-01T10:50:00+0200] [PACMAN] Running 'pacman -R {OWN_PACKAGE}'\n\
         [2026-10-01T10:50:01+0200] [ALPM] transaction started\n\
         [2026-10-01T10:50:02+0200] [ALPM] removed {OWN_PACKAGE} (0.1.2-1)\n\
         [2026-10-01T10:50:02+0200] [ALPM] transaction completed\n"
    );
    std::fs::write(m.env.tmp.path().join("pacman.log"), log).unwrap();
    let now = "2026-10-01T11:00:00+02:00";
    let c = m.capture(now, "pacman");
    assert_eq!(c["explainedSelf"], 0, "{c}");
    // newest first
    assert_eq!(
        m.drift(now),
        [pair("remove", OWN_PACKAGE), pair("upgrade", "zed")]
    );
}

/// WP-088: own changes left open (an engine stop between the two appends,
/// rows from before rule 8; here written with `seldon event`) are
/// explained by the next capture, whatever it collects; a row that has a
/// resolution keeps it, adding Seldon or choosing an older one stays
/// drift, and nothing creates a case.
#[test]
fn own_changes_left_open_are_explained_by_the_next_capture() {
    let m = Machine::new();
    m.plugins(&[]);
    m.capture(T0, "plugins"); // baseline
    let event = |now, source, kind, subject| own_event(&m, now, source, kind, subject);
    let update = event(T0, "plugins", "plugin-update", OWN_PLUGIN);
    let upgrade = event(
        "2026-10-01T10:01:00+02:00",
        "pacman",
        "upgrade",
        OWN_PACKAGE,
    );
    let dismissed = event(
        "2026-10-01T10:02:00+02:00",
        "plugins",
        "plugin-enable",
        OWN_PLUGIN,
    );
    event(
        "2026-10-01T10:03:00+02:00",
        "plugins",
        "plugin-add",
        OWN_PLUGIN,
    );
    event(
        "2026-10-01T10:04:00+02:00",
        "pacman",
        "downgrade",
        OWN_PACKAGE,
    );
    event(
        "2026-10-01T10:05:00+02:00",
        "plugins",
        "plugin-update",
        OTHER,
    );
    m.run(
        "2026-10-01T10:06:00+02:00",
        &["drift", "dismiss", &dismissed, "--", "tried it"],
    );
    let cases = |m: &Machine| {
        std::fs::read_dir(m.logbook.join("work/active"))
            .unwrap()
            .count()
    };
    let active = cases(&m);
    let lines = m.ledger().len();

    // a capture that collects nothing of its own explains them
    let now = "2026-10-01T10:20:00+02:00";
    let c = m.capture(now, "plugins");
    assert_eq!(c["written"], 0, "{c}");
    assert_eq!(c["explainedSelf"], 2, "{c}");
    assert_eq!(m.ledger().len(), lines + 2);
    let resolutions = |id: &str| -> Vec<Value> {
        m.ledger()
            .into_iter()
            .filter(|e| e["kind"] == "resolution" && e["refersTo"] == id)
            .collect()
    };
    let (own, _) = m.event("plugin-update", OWN_PLUGIN);
    assert_eq!(own["id"], update);
    assert_explained_as_own(&own, &resolutions(&update), "seldon's own plugin");
    let (own, _) = m.event("upgrade", OWN_PACKAGE);
    assert_explained_as_own(&own, &resolutions(&upgrade), "seldon's own package");
    let kept = resolutions(&dismissed);
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept[0]["resolution"], "dismissed", "{kept:?}");
    assert_eq!(
        m.drift(now),
        [
            pair("plugin-update", OTHER),
            pair("downgrade", OWN_PACKAGE),
            pair("plugin-add", OWN_PLUGIN),
        ]
    );
    assert_eq!(cases(&m), active, "no case");
    assert_valid_index(&m.index());

    // idempotent
    let again = m.capture("2026-10-01T10:40:00+02:00", "plugins");
    assert_eq!(again["explainedSelf"], 0, "{again}");
    assert_eq!(m.ledger().len(), lines + 2);
}

/// `seldon event <source> <kind> --subject <subject>` at `now`; the id.
fn own_event(m: &Machine, now: &str, source: &str, kind: &str, subject: &str) -> String {
    let out = m.run(
        now,
        &["event", source, kind, "--subject", subject, "--json"],
    );
    json(&out)["event"]["id"].as_str().unwrap().to_string()
}

/// WP-088 review N1: when the ledger cannot be read after the append,
/// the capture still explains what it wrote and says that it did not
/// check the earlier rows; the next readable capture catches up.
#[test]
fn an_unreadable_ledger_explains_the_new_changes_and_catches_up_later() {
    use std::os::unix::fs::PermissionsExt as _;
    let m = Machine::new();
    m.plugins(&[(OWN_PLUGIN, true, "0.1.0")]);
    m.capture(T0, "plugins"); // baseline
    own_event(
        &m,
        "2026-07-15T10:00:00+02:00",
        "plugins",
        "plugin-enable",
        OWN_PLUGIN,
    );
    let july = m.logbook.join("ledger/2026-07.jsonl");
    let mode =
        |m: u32| std::fs::set_permissions(&july, std::fs::Permissions::from_mode(m)).unwrap();
    mode(0o000);
    if std::fs::read(&july).is_ok() {
        mode(0o644);
        eprintln!("skipped: mode 000 does not keep this user (root) out");
        return;
    }
    m.plugins(&[(OWN_PLUGIN, true, "0.1.2")]);
    let out = m.run(
        "2026-10-01T10:20:00+02:00",
        &["capture", "--source", "plugins", "--json"],
    );
    mode(0o644);
    let c = json(&out);
    assert_eq!(
        (c["written"].clone(), c["explainedSelf"].clone()),
        (1.into(), 1.into()),
        "{c}"
    );
    let err = stderr(&out);
    assert!(
        err.contains("seldon: warning: seldon's earlier own changes not checked: ")
            && err.contains("Permission denied"),
        "{err}"
    );
    let (update, resolutions) = m.event("plugin-update", OWN_PLUGIN);
    assert_explained_as_own(&update, &resolutions, "seldon's own plugin");

    let c = m.capture("2026-10-01T10:40:00+02:00", "plugins");
    assert_eq!(c["explainedSelf"], 1, "the July row: {c}");
    let (enable, resolutions) = m.event("plugin-enable", OWN_PLUGIN);
    assert_explained_as_own(&enable, &resolutions, "seldon's own plugin");
}

/// WP-088 review I1: an own change dated after the capture clock (the
/// clock moved back) gets its resolution at the event's time, so the
/// index folds it; a row resolved by a line that comes before it in the
/// ledger (a dismissal at the earlier clock) keeps that resolution.
#[test]
fn an_own_change_dated_after_the_capture_clock_is_explained_once() {
    let m = Machine::new();
    m.plugins(&[]);
    m.capture(T0, "plugins"); // baseline
    let later = "2026-11-05T10:00:00+01:00";
    let update = own_event(&m, later, "plugins", "plugin-update", OWN_PLUGIN);
    let dismissed = own_event(&m, later, "plugins", "plugin-enable", OWN_PLUGIN);
    m.run(
        "2026-10-01T10:10:00+02:00",
        &["drift", "dismiss", &dismissed, "--", "tried it"],
    );

    let now = "2026-10-01T10:20:00+02:00";
    let c = m.capture(now, "plugins");
    assert_eq!(c["explainedSelf"], 1, "{c}");
    let (own, resolutions) = m.event("plugin-update", OWN_PLUGIN);
    assert_eq!(own["id"], update);
    assert_explained_as_own(&own, &resolutions, "seldon's own plugin");
    assert_eq!(resolutions[0]["ts"], own["ts"], "{resolutions:?}");
    let (_, kept) = m.event("plugin-enable", OWN_PLUGIN);
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept[0]["resolution"], "dismissed", "{kept:?}");

    // the index folds it: no drift, the Changelog row explained
    assert!(
        !m.drift(now).contains(&pair("plugin-update", OWN_PLUGIN)),
        "{:?}",
        m.drift(now)
    );
    let ix = m.index();
    let row = ix["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == update)
        .expect("the update is in the index");
    assert_eq!(row["resolution"], "explained", "{row}");

    let lines = m.ledger().len();
    let again = m.capture("2026-10-01T10:40:00+02:00", "plugins");
    assert_eq!(again["explainedSelf"], 0, "{again}");
    assert_eq!(m.ledger().len(), lines);
}

#[test]
fn the_human_capture_says_what_it_explained() {
    let m = Machine::new();
    m.plugins(&[(OWN_PLUGIN, true, "0.1.0")]);
    m.capture(T0, "plugins"); // baseline
    m.plugins(&[(OWN_PLUGIN, true, "0.1.2")]);
    let out = m.run(
        "2026-10-01T10:20:00+02:00",
        &["capture", "--source", "plugins"],
    );
    let text = common::stdout(&out);
    assert!(
        text.contains("note: 1 event(s) explained as seldon updating itself"),
        "{text}"
    );
}

/// The constants are the ids the repository ships: the plugin manifest's
/// `id` and the PKGBUILD's `pkgname`.
#[test]
fn the_own_ids_are_the_shipped_ones() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let manifest: Value = serde_json::from_str(&read(&repo.join("plugin/manifest.json"))).unwrap();
    assert_eq!(manifest["id"], OWN_PLUGIN);
    let pkgbuild = read(&repo.join("packaging/PKGBUILD"));
    assert!(
        pkgbuild
            .lines()
            .any(|l| l == format!("pkgname={OWN_PACKAGE}")),
        "{pkgbuild}"
    );
}
