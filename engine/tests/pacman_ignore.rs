//! ADR-0052 (WP-165): pacman's ignore list through the command line. The
//! pacman collector reads the `IgnorePkg` and `IgnoreGroup` names of
//! `pacman.conf` and its includes (names only, AGENTS.md §6), keeps them
//! in its cursor, shows them as `system.pacmanIgnore` and records a change
//! as one pacman `note`, attention `ignore-list`. Always through
//! `common::Env`: a temp HOME and `<tmp>/etc` (the guarded default of
//! `SELDON_ETC_DIR`), never the host's.

mod common;

use std::path::{Path, PathBuf};

use serde_json::{Value, json as j};

use common::{Env, Snapper, json, stderr};

fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// Every file below `dir` whose bytes hold one of `needles`.
fn holding(dir: &Path, needles: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries {
        let p = entry.unwrap().path();
        if p.is_dir() {
            found.extend(holding(&p, needles));
        } else if let Ok(bytes) = std::fs::read(&p) {
            let text = String::from_utf8_lossy(&bytes);
            for n in needles {
                if text.contains(n) {
                    found.push(format!("{} holds {n}", p.display()));
                }
            }
        }
    }
    found
}

fn etc(env: &Env) -> PathBuf {
    env.tmp.path().join("etc")
}

fn ok_at(env: &Env, now: &str, args: &[&str]) -> Value {
    let out = env.at(now, args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
    json(&out)
}

fn capture(env: &Env, now: &str) -> Value {
    let c = ok_at(env, now, &["capture", "--source", "pacman", "--json"]);
    assert_eq!(c["ok"], true, "{c}");
    c
}

/// The pacman events of the ledger.
fn pacman_events(env: &Env) -> Vec<Value> {
    common::ledger(&env.tmp.path().join("logbook"))
        .into_iter()
        .filter(|e| e["source"] == "pacman")
        .collect()
}

fn index(env: &Env) -> Value {
    let path = env.home.join(".local/state/seldon/index.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// What only a line other than the two lists holds: never in the record.
const NEVER: [&str; 4] = ["secret-mirror", "SigLevel", "HoldPkg", "custom-repo"];

/// Omarchy's layout: the lists in `[options]`, an include of drop-ins
/// above `[core]` (Omarchy's `add-custom-repo` hook sample), the
/// mirrorlist included by every repository.
fn omarchy_conf(env: &Env, ignore: &str) {
    let etc = etc(env);
    write(
        etc.join("pacman.conf"),
        format!(
            "# See the pacman.conf(5) manpage\n[options]\nHoldPkg = pacman glibc\n{ignore}\n\
             SigLevel = Required DatabaseOptional\nInclude = /etc/pacman.d/*.conf\n\n\
             [core]\nInclude = /etc/pacman.d/mirrorlist\n[extra]\nInclude = /etc/pacman.d/mirrorlist\n"
        ),
    );
    write(
        etc.join("pacman.d/mirrorlist"),
        "Server = https://secret-mirror.example/$repo/os/$arch\n",
    );
}

fn setup() -> Env {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    // the guarded pacman.log (`<guard>/pacman.log`): nothing installed
    write(env.tmp.path().join("pacman.log"), "");
    env
}

#[test]
fn the_list_is_a_baseline_then_a_change_is_one_attention_note() {
    let env = setup();
    omarchy_conf(&env, "IgnorePkg = linux linux-headers # the kernel stays");
    write(
        etc(&env).join("pacman.d/10-pins.conf"),
        "IgnoreGroup = kde-applications\n",
    );

    // the first read: shown, no event
    capture(&env, "2026-10-10T10:00:00+02:00");
    assert_eq!(pacman_events(&env), Vec::<Value>::new());
    assert_eq!(
        index(&env)["system"]["pacmanIgnore"],
        j!({"packages": ["linux", "linux-headers"], "groups": ["kde-applications"]})
    );

    // the same names again: nothing
    capture(&env, "2026-10-10T10:05:00+02:00");
    assert_eq!(pacman_events(&env), Vec::<Value>::new());

    // `omarchy refresh pacman` copies the template: the pins are gone, and
    // the user pins mesa again by hand
    let cursors = env.home.join(".local/state/seldon/cursors.json");
    let before = std::fs::read(&cursors).unwrap();
    omarchy_conf(&env, "IgnorePkg = mesa");
    std::fs::remove_file(etc(&env).join("pacman.d/10-pins.conf")).unwrap();
    capture(&env, "2026-10-10T11:00:00+02:00");
    let events = pacman_events(&env);
    assert_eq!(events.len(), 1, "{events:?}");
    let e = &events[0];
    assert_eq!(e["kind"], "note");
    assert_eq!(e["subject"], "/etc/pacman.conf");
    assert_eq!(e["actor"], "system");
    assert_eq!(e["ts"], "2026-10-10T11:00:00+02:00");
    assert_eq!(
        e["detail"],
        "IgnorePkg: added mesa; removed linux, linux-headers. IgnoreGroup: removed kde-applications."
    );
    assert_eq!(e["meta"], j!({"ignorePkg": "mesa", "ignoreGroup": ""}));
    assert_eq!(
        index(&env)["system"]["pacmanIgnore"],
        j!({"packages": ["mesa"], "groups": []})
    );

    // idempotent: a second capture writes nothing
    capture(&env, "2026-10-10T11:05:00+02:00");
    assert_eq!(pacman_events(&env).len(), 1);
    // nor one with the cursors from before the change (a failed cursor
    // save, an older state directory restored): the ledger has it
    std::fs::write(&cursors, &before).unwrap();
    capture(&env, "2026-10-10T11:05:30+02:00");
    assert_eq!(pacman_events(&env).len(), 1);

    // attention, never a crisis
    let drift = ok_at(
        &env,
        "2026-10-10T11:06:00+02:00",
        &["drift", "--all", "--json"],
    );
    let item = drift["drift"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["subject"] == "/etc/pacman.conf")
        .expect("an open drift item");
    assert_eq!(
        (&item["class"], &item["rule"]),
        (&j!("attention"), &j!("ignore-list"))
    );
    let shown = index(&env);
    let row = shown["drift"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["subject"] == "/etc/pacman.conf")
        .unwrap();
    assert_eq!(
        (&row["crisis"], &row["rule"]),
        (&j!(false), &j!("ignore-list"))
    );
    assert_eq!(shown["summary"]["crisis"], 0);
    let check = ok_at(
        &env,
        "2026-10-10T11:07:00+02:00",
        &["index", "--check", "--json"],
    );
    assert_eq!(check["valid"], true, "{check}");

    // names only: nothing else of the files reaches the logbook or the state
    for dir in [
        env.tmp.path().join("logbook"),
        env.home.join(".local/state/seldon"),
    ] {
        assert_eq!(
            holding(&dir, &NEVER),
            Vec::<String>::new(),
            "{}",
            dir.display()
        );
    }
}

#[test]
fn a_partial_read_is_shown_but_never_reported_as_a_change() {
    let env = setup();
    omarchy_conf(&env, "IgnorePkg = mesa");
    write(
        etc(&env).join("pacman.d/10-pins.conf"),
        "IgnorePkg = linux\n",
    );
    capture(&env, "2026-10-10T10:00:00+02:00");

    // the drop-in becomes a directory pacman cannot read either: the
    // list lacks `linux`, but that is no removal
    std::fs::remove_file(etc(&env).join("pacman.d/10-pins.conf")).unwrap();
    std::fs::create_dir_all(etc(&env).join("pacman.d/10-pins.conf")).unwrap();
    capture(&env, "2026-10-10T11:00:00+02:00");
    assert_eq!(pacman_events(&env), Vec::<Value>::new());
    assert_eq!(
        index(&env)["system"]["pacmanIgnore"],
        j!({"packages": ["mesa"], "groups": [], "partial": true})
    );

    // pacman.conf itself gone: the last list, still partial, no event
    std::fs::remove_file(etc(&env).join("pacman.conf")).unwrap();
    capture(&env, "2026-10-10T11:30:00+02:00");
    assert_eq!(pacman_events(&env), Vec::<Value>::new());
    assert_eq!(index(&env)["system"]["pacmanIgnore"]["partial"], true);

    // readable again with the same names: measured against the last
    // complete list, so still nothing
    omarchy_conf(&env, "IgnorePkg = mesa");
    std::fs::remove_dir(etc(&env).join("pacman.d/10-pins.conf")).unwrap();
    write(
        etc(&env).join("pacman.d/10-pins.conf"),
        "IgnorePkg = linux\n",
    );
    capture(&env, "2026-10-10T12:00:00+02:00");
    assert_eq!(pacman_events(&env), Vec::<Value>::new());
    assert_eq!(
        index(&env)["system"]["pacmanIgnore"],
        j!({"packages": ["mesa", "linux"], "groups": []})
    );
}

#[test]
fn without_the_collector_or_a_list_the_field_is_absent() {
    let env = setup();
    // no pacman.conf before the first capture: no list
    capture(&env, "2026-10-10T10:00:00+02:00");
    assert!(index(&env)["system"].get("pacmanIgnore").is_none());

    omarchy_conf(&env, "IgnorePkg = mesa");
    capture(&env, "2026-10-10T10:05:00+02:00");
    assert_eq!(
        index(&env)["system"]["pacmanIgnore"]["packages"],
        j!(["mesa"])
    );

    // the collector disabled: its list is not shown
    let config = env.config_file();
    let text = std::fs::read_to_string(&config).unwrap();
    assert_eq!(text.matches("\npacman = true\n").count(), 1, "{text}");
    write(
        &config,
        text.replace("\npacman = true\n", "\npacman = false\n"),
    );
    ok_at(&env, "2026-10-10T10:10:00+02:00", &["index", "--json"]);
    assert!(index(&env)["system"].get("pacmanIgnore").is_none());
}

#[test]
fn seldon_event_refuses_the_collectors_keys() {
    let env = setup();
    for meta in ["ignorePkg=mesa", "ignoreGroup=kde"] {
        let out = env.at(
            "2026-10-10T10:00:00+02:00",
            &[
                "event",
                "pacman",
                "note",
                "--subject",
                "/etc/pacman.conf",
                "--meta",
                meta,
                "--json",
            ],
        );
        assert_eq!(out.status.code(), Some(1), "{meta}: {}", stderr(&out));
        let said = format!("{}{}", common::stdout(&out), stderr(&out));
        assert!(
            said.contains("written by the pacman collector only"),
            "{said}"
        );
    }
    assert_eq!(pacman_events(&env), Vec::<Value>::new());
}

/// The review's F1: after a state loss the cursor's known list is newer
/// than the ledger's last note; a later change must still be recorded.
#[test]
fn a_change_after_a_state_loss_is_recorded() {
    let env = setup();
    omarchy_conf(&env, "IgnorePkg = linux");
    capture(&env, "2026-10-10T10:00:00+02:00");
    omarchy_conf(&env, "IgnorePkg = linux mesa");
    capture(&env, "2026-10-10T10:05:00+02:00");
    assert_eq!(pacman_events(&env).len(), 1);

    // the state directory's cursors are lost: a new baseline, no event
    std::fs::remove_file(env.home.join(".local/state/seldon/cursors.json")).unwrap();
    omarchy_conf(&env, "IgnorePkg = linux");
    capture(&env, "2026-10-10T10:10:00+02:00");
    assert_eq!(pacman_events(&env).len(), 1);

    // mesa pinned again: the 10:05 note is older than the known list
    omarchy_conf(&env, "IgnorePkg = linux mesa");
    capture(&env, "2026-10-10T10:15:00+02:00");
    let events = pacman_events(&env);
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(events[1]["detail"], "IgnorePkg: added mesa.");
    assert_eq!(events[1]["ts"], "2026-10-10T10:15:00+02:00");
}

/// The review's F2: the ledger redacts the note; the check compares what
/// the ledger wrote.
#[test]
fn a_redacted_change_is_not_written_twice() {
    let env = setup();
    let config = env.config_file();
    let text = std::fs::read_to_string(&config).unwrap();
    assert!(text.contains("\npatterns = []"), "{text}");
    write(
        &config,
        text.replace("\npatterns = []", "\npatterns = [\"corp-[a-z]+\"]"),
    );
    omarchy_conf(&env, "IgnorePkg = linux");
    capture(&env, "2026-10-10T10:00:00+02:00");
    let cursors = env.home.join(".local/state/seldon/cursors.json");
    let before = std::fs::read(&cursors).unwrap();
    omarchy_conf(&env, "IgnorePkg = linux corp-agent mesa");
    capture(&env, "2026-10-10T10:30:00+02:00");
    let events = pacman_events(&env);
    assert_eq!(events.len(), 1);
    assert!(
        !events[0].to_string().contains("corp-agent"),
        "{}",
        events[0]
    );
    // the cursor from before the change (a failed save): no second note
    std::fs::write(&cursors, &before).unwrap();
    capture(&env, "2026-10-10T10:35:00+02:00");
    assert_eq!(pacman_events(&env).len(), 1);
    // the masked name is counted, not shown
    assert_eq!(
        index(&env)["system"]["pacmanIgnore"],
        j!({"packages": ["linux", "mesa"], "groups": [], "hidden": 1})
    );
}

/// The review's F4: the same names in another order, across lines and
/// files, are no change.
#[test]
fn another_order_across_lines_and_files_is_no_change() {
    let env = setup();
    omarchy_conf(&env, "IgnorePkg = linux mesa zoom\nIgnoreGroup = kde gnome");
    capture(&env, "2026-10-10T10:00:00+02:00");
    omarchy_conf(
        &env,
        "IgnorePkg = zoom\nIgnorePkg = mesa\nIgnoreGroup = gnome",
    );
    write(
        etc(&env).join("pacman.d/10-pins.conf"),
        "IgnoreGroup = kde\nIgnorePkg = linux\n",
    );
    capture(&env, "2026-10-10T10:05:00+02:00");
    assert_eq!(pacman_events(&env), Vec::<Value>::new());
    assert_eq!(
        index(&env)["system"]["pacmanIgnore"],
        j!({"packages": ["zoom", "mesa", "linux"], "groups": ["gnome", "kde"]})
    );
}

/// The review's F6: a name Seldon does not show is no incomplete read; a
/// change of the others is still recorded.
#[test]
fn a_name_seldon_does_not_show_silences_nothing() {
    let env = setup();
    omarchy_conf(&env, "IgnorePkg = linux,nvidia-utils");
    capture(&env, "2026-10-10T10:00:00+02:00");
    assert_eq!(
        index(&env)["system"]["pacmanIgnore"],
        j!({"packages": [], "groups": [], "hidden": 1})
    );
    omarchy_conf(&env, "IgnorePkg = linux,nvidia-utils mesa");
    capture(&env, "2026-10-10T10:05:00+02:00");
    let events = pacman_events(&env);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["detail"], "IgnorePkg: added mesa.");
    assert_eq!(
        events[0]["meta"],
        j!({"ignorePkg": "(hidden) mesa", "ignoreGroup": ""})
    );
    // the typo fixed: one name gone, one added
    omarchy_conf(&env, "IgnorePkg = linux nvidia-utils mesa");
    capture(&env, "2026-10-10T10:10:00+02:00");
    let events = pacman_events(&env);
    assert_eq!(
        events[1]["detail"],
        "IgnorePkg: added linux, nvidia-utils; removed (hidden)."
    );
}

/// The review's F4: while `[redaction] patterns` do not compile the index
/// withholds the list. Every command refuses such a config, so the derive
/// runs in-process with the config as loaded and one pattern broken.
#[test]
fn an_invalid_pattern_withholds_the_list() {
    let env = setup();
    omarchy_conf(&env, "IgnorePkg = mesa");
    capture(&env, "2026-10-10T10:00:00+02:00");
    assert_eq!(
        index(&env)["system"]["pacmanIgnore"]["packages"],
        j!(["mesa"])
    );

    let dirs = seldon::config::Dirs {
        home: env.home.clone(),
        xdg_config_home: env.home.join(".config"),
        state_dir: env.home.join(".local/state/seldon"),
    };
    let mut config = seldon::config::Config::load(&env.config_file())
        .unwrap()
        .unwrap();
    let logbook = seldon::logbook::Logbook::open(&env.tmp.path().join("logbook")).unwrap();
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-10T10:05:00+02:00").unwrap();
    let derive = |config: &seldon::config::Config| {
        seldon::index::derive_at(&dirs, config, &logbook, now)
            .unwrap()
            .index
            .system
            .pacman_ignore
    };
    assert!(
        derive(&config).is_some(),
        "the list with a working redaction"
    );
    config.redaction.patterns = vec!["(".into()];
    assert_eq!(derive(&config), None);
}
