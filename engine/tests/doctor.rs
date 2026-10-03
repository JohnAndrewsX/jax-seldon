//! `seldon doctor` in a throw-away home, and read-only on the fixture logbook.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{Env, Snapper, json, stderr, stdout};

fn check<'a>(report: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no check {name} in {report}"))
}

fn init(env: &Env) -> PathBuf {
    let root = env.tmp.path().join("logbook");
    let out = env.seldon(&[
        "init",
        "--non-interactive",
        "--no-capture",
        "--path",
        root.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    root
}

/// Every file under `dir` with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(p.clone(), std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

mod doctor {
    use super::*;

    #[test]
    fn green_after_init_with_snapper_degraded() {
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        let out = env.seldon(&["doctor", "--path", root.to_str().unwrap(), "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
        let v = json(&out);
        assert_eq!(v["ok"], true);
        for name in ["engine", "config", "logbook", "omarchy"] {
            assert_eq!(check(&v, name)["status"], "ok", "{name}: {v}");
        }
        assert_eq!(check(&v, "omarchy")["message"], "Omarchy 4.0.4-1");
        let snapper = check(&v, "snapper");
        assert_eq!(snapper["status"], "degraded");
        assert_eq!(
            snapper["fix"],
            "sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes"
        );
        if env.has_git {
            assert_eq!(check(&v, "git")["status"], "ok", "{v}");
        }
    }

    #[test]
    fn human_output_lists_every_check() {
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        let out = env.seldon(&["--logbook", root.to_str().unwrap(), "doctor"]);
        assert_eq!(out.status.code(), Some(0));
        let text = stdout(&out);
        for name in ["engine", "config", "logbook", "omarchy", "snapper", "git"] {
            assert!(text.contains(name), "{name} missing in:\n{text}");
        }
        assert!(text.contains("fix: sudo snapper"));
        assert!(text.trim_end().ends_with("doctor: ok"));
    }

    /// Issue #1: snapper's permission error is recognised in a German
    /// locale. The stub answers in German unless `LC_ALL=C`, like snapper's
    /// own catalog; the collector state, `init`'s hint and `doctor`'s fix
    /// line must all appear.
    #[test]
    fn snapper_permission_error_is_recognised_in_any_locale() {
        let env = Env::new(Snapper::NoPermissionsLocalized);
        let german = |args: &[&str]| {
            env.command(args)
                .env("LANG", "de_DE.UTF-8")
                .env("LANGUAGE", "de")
                .output()
                .expect("run seldon")
        };
        let root = env.tmp.path().join("logbook");
        let out = german(&[
            "init",
            "--non-interactive",
            "--path",
            root.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let text = stdout(&out);
        assert!(
            text.contains("Snapper: degraded — No permissions."),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "{}   # optional: snapshots in the timeline (ADR-0011)",
                seldon::commands::doctor::SNAPPER_FIX
            )),
            "{text}"
        );
        assert!(!text.contains("Keine"), "{text}");

        let cursors: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(env.home.join(".local/state/seldon/cursors.json")).unwrap(),
        )
        .unwrap();
        let state = &cursors["collectors"]["snapper"];
        assert_eq!(state["ok"], false, "{state}");
        assert_eq!(
            state["message"],
            seldon::collectors::snapper::NO_PERMISSIONS,
            "{state}"
        );
        assert_eq!(state["fix"], seldon::commands::doctor::SNAPPER_FIX);

        let out = german(&["doctor", "--path", root.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let text = stdout(&out);
        assert!(
            text.contains(&format!("fix: {}", seldon::commands::doctor::SNAPPER_FIX)),
            "{text}"
        );
        assert!(!text.contains("Keine"), "{text}");
    }

    /// WP-050: the `logbook` row shortens the home to `~` like the header.
    #[test]
    fn logbook_row_shows_paths_like_the_header() {
        let env = Env::new(Snapper::Allowed);
        let root = env.home.join("Seldon");
        let out = env.seldon(&[
            "init",
            "--non-interactive",
            "--no-capture",
            "--path",
            root.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let text = stdout(&env.seldon(&["doctor"]));
        assert!(text.starts_with("seldon doctor · ~/Seldon\n"), "{text}");
        assert!(text.contains(" logbook  ~/Seldon · machine "), "{text}");
        assert!(!text.contains(env.home.to_str().unwrap()), "{text}");

        let missing = env.home.join("Elsewhere");
        let v = json(&env.seldon(&["doctor", "--path", missing.to_str().unwrap(), "--json"]));
        assert_eq!(
            v["logbook"],
            missing.to_str().unwrap(),
            "JSON keeps it absolute"
        );
        let logbook = check(&v, "logbook");
        assert!(
            logbook["message"]
                .as_str()
                .unwrap()
                .starts_with("not initialised at ~/Elsewhere (path from "),
            "{logbook}"
        );
        assert_eq!(logbook["fix"], "seldon init --path ~/Elsewhere");
    }

    #[test]
    fn logbook_from_config_after_init() {
        let env = Env::new(Snapper::Allowed);
        let root = init(&env);
        let v = json(&env.seldon(&["doctor", "--json"]));
        assert_eq!(v["logbook"], root.to_str().unwrap());
        assert_eq!(v["ok"], true);
        assert_eq!(check(&v, "snapper")["status"], "ok");
        assert_eq!(check(&v, "snapper")["message"], "2 snapshots (config root)");
    }

    #[test]
    fn not_initialised_exits_3() {
        let env = Env::new(Snapper::Missing);
        let out = env.seldon(&["doctor", "--json"]);
        assert_eq!(out.status.code(), Some(3));
        let v = json(&out);
        assert_eq!(v["ok"], false);
        assert_eq!(check(&v, "logbook")["status"], "error");
        assert_eq!(check(&v, "config")["status"], "degraded");
        assert_eq!(check(&v, "snapper")["status"], "degraded");
        assert!(
            check(&v, "logbook")["fix"]
                .as_str()
                .unwrap()
                .starts_with("seldon init --path ")
        );
    }

    #[test]
    fn invalid_frontmatter_is_an_error() {
        let env = Env::new(Snapper::Allowed);
        let root = init(&env);
        std::fs::write(
            root.join("work/queued/C-2026-001-broken.md"),
            "---\nid: C-2026-001\ntype: case\ntitle: \"x\"\nstatus: nonsense\nzone: red\nrisk: R1\ncreated: 2026-10-01\n---\n",
        )
        .unwrap();
        let out = env.seldon(&["doctor", "--json"]);
        assert_eq!(out.status.code(), Some(1));
        let logbook = json(&out)["checks"][2].clone();
        assert_eq!(logbook["status"], "error");
        assert!(
            logbook["message"]
                .as_str()
                .unwrap()
                .contains("work/queued/C-2026-001-broken.md"),
            "{logbook}"
        );
    }

    #[test]
    fn case_in_the_wrong_folder_is_degraded() {
        let env = Env::new(Snapper::Allowed);
        let root = init(&env);
        std::fs::write(
            root.join("work/queued/C-2026-001-early.md"),
            "---\nid: C-2026-001\ntype: case\ntitle: \"x\"\nstatus: active\nzone: red\nrisk: R1\ncreated: 2026-10-01\n---\n",
        )
        .unwrap();
        let v = json(&env.seldon(&["doctor", "--json"]));
        assert_eq!(v["ok"], true);
        let logbook = check(&v, "logbook");
        assert_eq!(logbook["status"], "degraded");
        assert!(
            logbook["message"]
                .as_str()
                .unwrap()
                .contains("status active")
        );
    }

    #[test]
    fn fixture_logbook_is_valid_and_untouched() {
        let env = Env::new(Snapper::NoPermissions);
        let fixture = common::fixture_logbook();
        let before = snapshot(&fixture);
        let out = env.seldon(&["doctor", "--path", fixture.to_str().unwrap(), "--json"]);
        let v = json(&out);
        assert_eq!(out.status.code(), Some(0), "{v}");
        let logbook = check(&v, "logbook");
        assert_eq!(logbook["status"], "ok", "{logbook}");
        let message = logbook["message"].as_str().unwrap();
        assert!(
            message.contains("8 cases, 4 decisions, 10 journal days"),
            "{message}"
        );
        assert!(message.contains("workstation-7f3a"));
        assert_eq!(
            before,
            snapshot(&fixture),
            "doctor wrote into fixtures/logbook"
        );
    }
}
