//! `seldon dossier` (WP-035, SPEC-ENGINE §3): the generated fences of
//! `system/*.md` on a copy of `fixtures/logbook/`. Every host query is a
//! shim on the test PATH (`Env::query_shims`, inputs in `fixtures/logs/`):
//! no package manager, `systemctl` or `omarchy` of the host ever runs, and
//! the shims log their arguments so the tests prove that only read-only
//! queries were asked. Omarchy's package lists are the fixture copies in
//! `fixtures/logs/omarchy-packages/` (`SELDON_OMARCHY_PACKAGES`, set by
//! `common::Env`). Always through `common::Env` (temp home,
//! `SELDON_TEST_GUARD`), never the real XDG dirs.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use common::{Env, Snapper, copy_dir, fixture_logbook, hardware_root, json, read, stderr, stdout};
use seldon::index::load::fences;
use seldon::index::views::replace_fence;

/// The sample index's clock (`fixtures/index.sample.json`).
const NOW: &str = "2026-10-01T17:05:12+02:00";
const LATER: &str = "2026-10-02T08:00:00+02:00";

const ALL_FENCES: [&str; 8] = [
    "deviations.table",
    "hardware.summary",
    "omarchy.summary",
    "packages.explicit",
    "packages.history",
    "packages.summary",
    "plugins.list",
    "services.enabled",
];

/// The only calls the shims may see: read-only queries.
const QUERIES: [&str; 6] = [
    "pacman -Qqe",
    "pacman -Qqm",
    "pacman -Q",
    "systemctl --system list-unit-files --state=enabled --no-legend --no-pager",
    "systemctl --user list-unit-files --state=enabled --no-legend --no-pager",
    "omarchy plugin list --json",
];

fn fixture_copy(env: &Env) -> PathBuf {
    let lb = env.tmp.path().join("logbook");
    copy_dir(&fixture_logbook(), &lb);
    lb
}

/// `seldon --logbook <lb> args…` at `now` with the fixture hardware.
fn command(env: &Env, lb: &Path, now: &str, args: &[&str]) -> std::process::Output {
    let mut all = vec!["--logbook", lb.to_str().unwrap()];
    all.extend_from_slice(args);
    env.command(&all)
        .env("SELDON_NOW", now)
        .env("SELDON_HARDWARE_ROOT", hardware_root())
        .output()
        .expect("run seldon")
}

/// `seldon --logbook <lb> --json args…` at `now`; asserts `code`.
fn run_at(env: &Env, lb: &Path, now: &str, args: &[&str], code: i32) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let out = command(env, lb, now, &all);
    assert_eq!(
        out.status.code(),
        Some(code),
        "{args:?}: {}{}",
        stdout(&out),
        stderr(&out)
    );
    json(&out)
}

fn dossier(env: &Env, lb: &Path, args: &[&str]) -> Value {
    let mut all = vec!["dossier"];
    all.extend_from_slice(args);
    run_at(env, lb, NOW, &all, 0)
}

/// `system/*.md` by file name.
fn system_files(lb: &Path) -> BTreeMap<String, String> {
    std::fs::read_dir(lb.join("system"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                read(&p),
            )
        })
        .collect()
}

/// What the user owns: the text with every fence body taken out.
fn outside(text: &str) -> String {
    fences(text)
        .into_iter()
        .fold(text.to_string(), |t, (name, _)| {
            replace_fence(&t, &name, "").unwrap()
        })
}

/// Every fence body of the dossier, by fence name.
fn bodies(lb: &Path) -> BTreeMap<String, String> {
    system_files(lb).values().flat_map(|t| fences(t)).collect()
}

fn git_init(env: &Env, lb: &Path) {
    for args in [
        &["init", "-q"][..],
        &["-c", "user.name=t", "-c", "user.email=t@t", "add", "-A"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "fixture",
        ],
    ] {
        let out = env.git(lb, args);
        assert!(out.status.success(), "git {args:?}: {}", stderr(&out));
    }
}

/// Asserts that every logged call is one of [`QUERIES`]; returns them.
fn read_only_calls(calls: &Path) -> Vec<String> {
    let lines: Vec<String> = std::fs::read_to_string(calls)
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect();
    for l in &lines {
        assert!(QUERIES.contains(&l.as_str()), "not a read-only query: {l}");
    }
    lines
}

#[test]
fn the_fixture_dossier_is_golden_and_keeps_user_text() {
    let env = Env::new(Snapper::Missing);
    let calls = env.query_shims();
    let lb = fixture_copy(&env);
    if env.has_git {
        git_init(&env, &lb);
    }
    let before = system_files(&lb);

    let out = dossier(&env, &lb, &[]);
    assert_eq!(
        out["sections"],
        json!({
            "deviations.table": "unchanged",
            "hardware.summary": "written",
            "omarchy.summary": "unchanged",
            "packages.explicit": "written",
            "packages.history": "written",
            "packages.summary": "written",
            "plugins.list": "unchanged",
            "services.enabled": "written",
        }),
        "{out}"
    );
    assert_eq!(
        out["files"],
        json!([
            "system/hardware.md",
            "system/packages.md",
            "system/services.md"
        ])
    );
    assert_eq!(
        out["counts"],
        json!({"explicit": 15, "preLogbook": 11, "omarchyBase": 7, "total": 23, "aur": 3, "units": 9, "plugins": 40})
    );
    assert_eq!(out["warnings"], json!([]), "{out}");

    // every query ran once, and nothing but the queries
    let mut seen = read_only_calls(&calls);
    seen.sort();
    let mut expected = QUERIES.map(String::from).to_vec();
    expected.sort();
    assert_eq!(seen, expected);

    // the user's text outside the fences is byte-identical
    let after = system_files(&lb);
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );
    // (the fixture carries an empty packages.explicit fence: nothing is
    // appended, WP-036)
    for (name, text) in &before {
        assert_eq!(outside(text), outside(&after[name]), "{name}");
    }
    let all = bodies(&lb);
    let got: Vec<&str> = all.keys().map(String::as_str).collect();
    assert_eq!(got, ALL_FENCES);
    // both origin classes: Omarchy's lists name git, the user added firefox
    let explicit: Vec<&str> = all["packages.explicit"].lines().collect();
    assert!(explicit.contains(&"- git · repo · omarchy-base · pre-logbook"));
    assert!(explicit.contains(&"- firefox · repo · user · pre-logbook"));
    // the engine's four hardware keys are refreshed; the fixture's
    // hand-written lines stay where they were
    assert_eq!(
        all["hardware.summary"],
        "- cpu: Intel(R) Core(TM) i7-14700K\n- memory: 63 GiB\n- gpu: Intel Arc B580\n- displays: 2 × 2560×1440 @ 144 Hz\n- disk: NVMe 2 TB, btrfs\n- machine: MS-7D91\n- rootfs: btrfs\n"
    );

    // golden: every file of the dossier, in name order
    let mut text = String::new();
    for (name, body) in &after {
        text.push_str(&format!("==> system/{name} <==\n{body}"));
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/dossier.md");
    if std::env::var_os("SELDON_BLESS").is_some() {
        std::fs::write(&path, &text).unwrap();
    }
    let golden = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} missing; run with SELDON_BLESS=1", path.display()));
    assert_eq!(text, golden, "{} differs", path.display());

    let git_log = || stdout(&env.git(&lb, &["log", "--format=%s"]));
    if env.has_git {
        assert_eq!(out["git"]["committed"], json!(true), "{out}");
        assert_eq!(git_log().lines().next(), Some("seldon: dossier"));
        assert_eq!(stdout(&env.git(&lb, &["status", "--porcelain"])), "");
    }

    // again, a day later: nothing changed, nothing written or committed
    let again = run_at(&env, &lb, LATER, &["dossier"], 0);
    assert_eq!(again["files"], json!([]), "{again}");
    assert!(
        again["sections"]
            .as_object()
            .unwrap()
            .values()
            .all(|s| s == "unchanged"),
        "{again}"
    );
    assert_eq!(again["git"]["committed"], json!(false));
    assert_eq!(system_files(&lb), after);
    let human = command(&env, &lb, LATER, &["dossier"]);
    assert_eq!(human.status.code(), Some(0));
    assert!(
        stdout(&human).starts_with("Nothing changed (8 fence(s) checked)"),
        "{}",
        stdout(&human)
    );
    if env.has_git {
        assert_eq!(git_log().matches("seldon: dossier").count(), 1);
    }
    read_only_calls(&calls);
}

#[test]
fn empty_fences_are_filled_and_the_ledger_marks_known_packages() {
    let env = Env::new(Snapper::Missing);
    env.query_shims();
    let lb = fixture_copy(&env);
    for (name, text) in system_files(&lb) {
        std::fs::write(lb.join("system").join(name), outside(&text)).unwrap();
    }
    let out = dossier(&env, &lb, &["--section", "all"]);
    let sections: BTreeMap<String, Value> =
        serde_json::from_value(out["sections"].clone()).unwrap();
    assert!(sections.values().all(|s| s == "written"), "{out}");
    assert_eq!(sections.len(), 8);

    let b = bodies(&lb);
    assert!(b.values().all(|body| !body.is_empty()), "{b:#?}");
    // the ledger knows four installs; the rest predate the logbook
    let explicit: Vec<&str> = b["packages.explicit"].lines().collect();
    assert_eq!(explicit.len(), 15);
    for line in [
        "- btop · repo · omarchy-base · since 2026-09-03",
        "- ollama · repo · user · since 2026-10-01",
        "- tailscale · repo · user · since 2026-10-01 [[C-2026-008]]",
        "- zed · repo · user · since 2026-10-01 [[C-2026-004]]",
        "- brave-bin · aur · user · pre-logbook",
        "- yay · aur · omarchy-base · pre-logbook",
        "- omarchy · repo · user · pre-logbook",
    ] {
        assert!(explicit.contains(&line), "{line} missing: {explicit:#?}");
    }
    assert_eq!(
        explicit
            .iter()
            .filter(|l| l.ends_with("pre-logbook"))
            .count(),
        11
    );
    assert_eq!(
        explicit
            .iter()
            .filter(|l| l.contains(" · omarchy-base · "))
            .count(),
        7,
        "base, base-devel, btop, git, hyprland, linux-firmware, yay"
    );
    assert_eq!(
        b["packages.summary"],
        "- explicit: 15\n- total: 23\n- aur: 3\n"
    );
    assert_eq!(
        b["packages.history"],
        "| date | explicit | total |\n|---|---|---|\n| 2026-10-01 | 15 | 23 |\n"
    );
    assert_eq!(
        b["omarchy.summary"],
        "- version: 4.0.7-1\n- theme: tokyo-night\n- lastUpdate: 2026-10-01T09:21:00+02:00\n"
    );
    assert_eq!(
        b["hardware.summary"],
        "- cpu: Intel(R) Core(TM) i7-14700K\n- memory: 63 GiB\n- machine: MS-7D91\n- rootfs: btrfs\n"
    );
    // the cased config events of the ledger, oldest first; uncased
    // changes (the user's own deviations) are not the engine's to add
    assert_eq!(
        b["deviations.table"],
        "| path | reason | date | case |\n|---|---|---|---|\n| ~/.config/hypr/monitors.conf |  | 2026-09-12 | [[C-2026-002]] |\n| ~/.config/hypr/bindings.conf |  | 2026-10-01 | [[C-2026-004]] |\n"
    );
    // without the old rows, no case survives for tailscaled
    assert!(
        b["services.enabled"].contains("| tailscaled.service | system | — |"),
        "{}",
        b["services.enabled"]
    );
}

#[test]
fn a_failing_query_keeps_its_fences() {
    // no shims: the package manager, systemctl and omarchy are not on PATH
    let env = Env::new(Snapper::Missing);
    let lb = fixture_copy(&env);
    let before = system_files(&lb);
    let out = dossier(&env, &lb, &[]);
    assert_eq!(out["sections"]["packages.summary"], "skipped", "{out}");
    assert_eq!(out["sections"]["packages.history"], "skipped");
    assert_eq!(out["sections"]["packages.explicit"], "skipped");
    assert_eq!(out["sections"]["services.enabled"], "skipped");
    assert_eq!(out["sections"]["plugins.list"], "skipped");
    // the Env's omarchy-version stub prints 4.0.4-1
    assert_eq!(out["sections"]["omarchy.summary"], "written");
    let warnings: Vec<&str> = out["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    assert!(warnings[0].starts_with("packages: `pacman -Qqe`: `pacman` not found"));
    assert!(
        warnings
            .iter()
            .all(|w| w.contains("; fence") && w.ends_with(" kept")),
        "{warnings:?}"
    );
    let after = system_files(&lb);
    for name in ["packages.md", "services.md", "plugins.md", "deviations.md"] {
        assert_eq!(before[name], after[name], "{name}");
    }
    assert!(after["omarchy.md"].contains("- version: 4.0.4-1\n"));
    assert_eq!(out["counts"]["explicit"], Value::Null);
}

#[test]
fn a_section_writes_only_its_fences() {
    let env = Env::new(Snapper::Missing);
    let calls = env.query_shims();
    let lb = fixture_copy(&env);
    let before = system_files(&lb);
    let out = dossier(&env, &lb, &["--section", "services"]);
    assert_eq!(out["sections"], json!({"services.enabled": "written"}));
    assert_eq!(out["files"], json!(["system/services.md"]));
    assert_eq!(
        read_only_calls(&calls),
        [QUERIES[3], QUERIES[4]],
        "only the unit queries ran"
    );
    let after = system_files(&lb);
    for (name, text) in &before {
        if name != "services.md" {
            assert_eq!(text, &after[name], "{name}");
        }
    }

    let out = dossier(
        &env,
        &lb,
        &["--section", "packages,plugins", "--section", "hardware"],
    );
    let names: Vec<&String> = out["sections"].as_object().unwrap().keys().collect();
    assert_eq!(
        names,
        [
            "hardware.summary",
            "packages.explicit",
            "packages.history",
            "packages.summary",
            "plugins.list"
        ]
    );
    // the package queries ran once for three fences
    let calls = read_only_calls(&calls);
    assert_eq!(calls.iter().filter(|c| *c == "pacman -Qqe").count(), 1);

    let bad = command(
        &env,
        &lb,
        NOW,
        &["--json", "dossier", "--section", "kernel"],
    );
    assert_eq!(bad.status.code(), Some(1), "{}", stderr(&bad));
    assert_eq!(json(&bad)["error"]["code"], 1);
}

#[test]
fn cased_changes_add_deviation_rows_and_unit_cases() {
    let env = Env::new(Snapper::Missing);
    env.query_shims();
    let lb = fixture_copy(&env);
    let deviations = read(&lb.join("system/deviations.md"));
    run_at(
        &env,
        &lb,
        NOW,
        &[
            "event",
            "config",
            "config-change",
            "--subject",
            "~/.config/hypr/looknfeel.conf",
            "--case",
            "C-2026-004",
        ],
        0,
    );
    run_at(
        &env,
        &lb,
        NOW,
        &[
            "event",
            "agent",
            "command",
            "--subject",
            "systemctl",
            "--actor",
            "agent:claude-code",
            "--case",
            "C-2026-004",
            "--meta",
            "command=systemctl --user enable --now pipewire.socket",
        ],
        0,
    );
    let out = dossier(&env, &lb, &["--section", "deviations,services"]);
    assert_eq!(out["sections"]["deviations.table"], "written", "{out}");
    let new = read(&lb.join("system/deviations.md"));
    // the old file, byte for byte, plus one row inside the fence
    let row = "| ~/.config/hypr/looknfeel.conf |  | 2026-10-01 | [[C-2026-004]] |\n";
    assert_eq!(
        new,
        deviations.replace("<!-- seldon:end -->", &format!("{row}<!-- seldon:end -->"))
    );
    let services = read(&lb.join("system/services.md"));
    assert!(
        services.contains("| pipewire.socket | user | [[C-2026-004]] |"),
        "{services}"
    );
    // the old row's case is kept where the ledger knows none
    assert!(
        services.contains("| tailscaled.service | system | [[C-2026-008]] |"),
        "{services}"
    );
    assert!(services.contains("| ollama.service | user | — |"));
}

#[test]
fn without_omarchys_lists_every_package_is_the_users() {
    let env = Env::new(Snapper::Missing);
    let calls = env.query_shims();
    let lb = fixture_copy(&env);
    let lists = env.tmp.path().join("no-lists");
    let run = || {
        let out = env
            .command(&[
                "--logbook",
                lb.to_str().unwrap(),
                "--json",
                "dossier",
                "--section",
                "packages",
            ])
            .env("SELDON_NOW", NOW)
            .env("SELDON_OMARCHY_PACKAGES", &lists)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        json(&out)
    };
    let out = run();
    let warnings = out["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{out}");
    let w = warnings[0].as_str().unwrap();
    assert!(
        w.starts_with("packages: Omarchy's package list(s) ")
            && w.contains("omarchy-base.packages")
            && w.contains("omarchy-other.packages")
            && w.ends_with("their packages count as `user`"),
        "{w}"
    );
    assert_eq!(out["counts"]["omarchyBase"], 0);
    let explicit = bodies(&lb)["packages.explicit"].clone();
    assert_eq!(explicit.lines().count(), 15);
    assert!(
        explicit.lines().all(|l| l.contains(" · user · ")),
        "{explicit}"
    );
    // only the package queries ran: the lists are files, not programs
    assert_eq!(read_only_calls(&calls).len(), 3);
    // the same answer again: nothing changed
    let again = run();
    assert_eq!(again["files"], json!([]), "{again}");
}

#[test]
fn a_later_cased_event_fills_only_an_empty_case_cell() {
    let env = Env::new(Snapper::Missing);
    env.query_shims();
    let lb = fixture_copy(&env);
    let deviations = read(&lb.join("system/deviations.md"));
    let event = |path: &str, case: &str| {
        run_at(
            &env,
            &lb,
            NOW,
            &[
                "event",
                "config",
                "config-change",
                "--subject",
                path,
                "--case",
                case,
            ],
            0,
        );
    };
    // ~/.bashrc has a row without a case; monitors.conf has one with
    event("~/.bashrc", "C-2026-004");
    event("~/.config/hypr/monitors.conf", "C-2026-004");
    let out = dossier(&env, &lb, &["--section", "deviations"]);
    assert_eq!(out["sections"]["deviations.table"], "written", "{out}");
    let new = read(&lb.join("system/deviations.md"));
    assert_eq!(
        new,
        deviations.replace(
            "| ~/.bashrc | mise-Aktivierung | 2026-09-01 | — |",
            "| ~/.bashrc | mise-Aktivierung | 2026-09-01 | [[C-2026-004]] |"
        ),
        "only the empty case cell changed"
    );
    assert!(new.contains("| ~/.config/hypr/monitors.conf | Dual-WQHD, Skalierung 1.25 | 2026-09-12 | [[C-2026-002]] |"));
    let again = dossier(&env, &lb, &["--section", "deviations"]);
    assert_eq!(again["files"], json!([]), "{again}");
    assert_eq!(read(&lb.join("system/deviations.md")), new);
}

#[test]
fn exit_codes_follow_the_spec() {
    let env = Env::new(Snapper::Missing);
    env.query_shims();
    let missing = env.tmp.path().join("nothing-here");
    let out = command(&env, &missing, NOW, &["--json", "dossier"]);
    assert_eq!(out.status.code(), Some(3), "{}", stdout(&out));

    let lb = fixture_copy(&env);
    let _lock = seldon::logbook::lock::acquire(&env.lock_file()).unwrap();
    let out = command(&env, &lb, NOW, &["--json", "dossier"]);
    assert_eq!(out.status.code(), Some(4), "{}", stdout(&out));
}

#[test]
fn capture_and_status_leave_the_dossier_alone() {
    let env = Env::new(Snapper::Missing);
    let calls = env.query_shims();
    let lb = fixture_copy(&env);
    let before = system_files(&lb);
    let empty = env.tmp.path().join("empty.log");
    std::fs::write(&empty, "").unwrap();
    for args in [&["capture", "--all"][..], &["status"]] {
        let mut all = vec!["--logbook", lb.to_str().unwrap(), "--json"];
        all.extend_from_slice(args);
        let out = env
            .command(&all)
            .env("SELDON_NOW", NOW)
            .env("SELDON_PACMAN_LOG", &empty)
            .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
    }
    assert_eq!(system_files(&lb), before);
    let calls = std::fs::read_to_string(calls).unwrap_or_default();
    assert!(!calls.contains("-Qqe"), "{calls}");
    assert!(!calls.contains("list-unit-files"), "{calls}");
}

#[test]
fn a_damaged_marker_above_a_fence_never_appends_it_again() {
    let env = Env::new(Snapper::Missing);
    env.query_shims();
    let lb = fixture_copy(&env);
    let path = lb.join("system/packages.md");
    let damaged = read(&path).replacen(
        "<!-- seldon:begin packages.summary -->",
        "<!-- seldon:begin notes -->\nkaputt\n\n<!-- seldon:begin packages.summary -->",
        1,
    );
    std::fs::write(&path, &damaged).unwrap();
    let first = dossier(&env, &lb, &["--section", "packages"]);
    assert_eq!(first["sections"]["packages.summary"], "written", "{first}");
    let text = read(&path);
    for name in ["packages.summary", "packages.history", "packages.explicit"] {
        assert_eq!(
            text.matches(&format!("seldon:begin {name} -->")).count(),
            1,
            "{name}: {text}"
        );
    }
    assert!(text.starts_with("# Pakete\n\n<!-- seldon:begin notes -->\nkaputt\n"));
    let again = dossier(&env, &lb, &["--section", "packages"]);
    assert_eq!(again["files"], json!([]), "{again}");
    assert_eq!(read(&path), text);
}

#[test]
fn host_strings_go_through_the_users_redaction_patterns() {
    let env = Env::new(Snapper::Missing);
    env.query_shims();
    let lb = fixture_copy(&env);
    let config = env.config_file();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        "[redaction]\npatterns = [\"MS-7D\\\\d+\", \"pipewire\", \"weather-plus\"]\n",
    )
    .unwrap();
    let out = dossier(&env, &lb, &["--section", "hardware,services,plugins"]);
    assert_eq!(out["warnings"], json!([]), "{out}");
    let hardware = read(&lb.join("system/hardware.md"));
    assert!(hardware.contains("- machine: ‹redacted›\n"), "{hardware}");
    assert!(!hardware.contains("MS-7D91"));
    let services = read(&lb.join("system/services.md"));
    assert!(!services.contains("pipewire"), "{services}");
    assert!(
        services.contains("| ‹redacted›.socket | user | — |"),
        "{services}"
    );
    let plugins = read(&lb.join("system/plugins.md"));
    assert!(!plugins.contains("weather-plus"), "{plugins}");

    // an invalid pattern refuses to write anything (exit 1)
    std::fs::write(&config, "[redaction]\npatterns = [\"(\"]\n").unwrap();
    let before = system_files(&lb);
    let bad = command(&env, &lb, NOW, &["--json", "dossier"]);
    assert_eq!(bad.status.code(), Some(1), "{}", stdout(&bad));
    assert_eq!(system_files(&lb), before);
}
