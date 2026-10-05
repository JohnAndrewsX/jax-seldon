//! The environment a capture runs in (WP-089): `OMARCHY_PATH` for the
//! Omarchy programs when ssh, cron or a systemd unit did not export it,
//! and the desktop entries in the default watch paths. Always through
//! `common::Env` (temp home, `SELDON_TEST_GUARD`, PATH = stubs only).

mod common;

use std::path::{Path, PathBuf};

use serde_json::Value;

use common::{Env, Snapper, copy_dir, fixture_logbook, json, read, stderr, stdout};
use seldon::collectors::config::Manifest;
use seldon::config::{Config, DEFAULT_WATCH_PATHS};

/// What the stubs log: one line per call, `<OMARCHY_PATH> <program> <args>`.
fn calls(env: &Env) -> PathBuf {
    env.tmp.path().join("omarchy-calls.log")
}

/// `omarchy` and `omarchy-version` as Omarchy's own: they fail without
/// `OMARCHY_PATH` (`omarchy-shell` says "OMARCHY_PATH is not set") and log
/// the value they were given.
fn strict_stubs(env: &Env) {
    let log = calls(env).display().to_string();
    let guard = |name: &str| {
        format!(
            "printf '%s %s %s\\n' \"$OMARCHY_PATH\" {name} \"$*\" >> '{log}'\n\
             [ -n \"$OMARCHY_PATH\" ] || {{ echo 'OMARCHY_PATH is not set' >&2; exit 1; }}\n"
        )
    };
    env.stub(
        "omarchy",
        &format!(
            "{}case \"$*\" in\n\
             \x20 'plugin list --json') echo '[{{\"id\":\"jax.seldon\",\"enabled\":true}}]' ;;\n\
             \x20 'plugin catalog') echo '[]' ;;\n\
             \x20 *) echo \"unexpected: $*\" >&2; exit 64 ;;\n\
             esac",
            guard("omarchy")
        ),
    );
    env.stub(
        "omarchy-version",
        &format!("{}echo 4.0.4-1", guard("omarchy-version")),
    );
}

fn logged(env: &Env) -> Vec<String> {
    std::fs::read_to_string(calls(env))
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect()
}

/// The collector `name` of a `capture --json` result.
fn collector<'a>(out: &'a Value, name: &str) -> &'a Value {
    out["collectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no collector {name} in {out}"))
}

/// `capture --source <source> --json` with `OMARCHY_PATH` as given
/// (`None` = unset, the `Env` clears the environment).
fn capture(env: &Env, source: &str, omarchy_path: Option<&str>) -> Value {
    let mut cmd = env.command(&["capture", "--source", source, "--json"]);
    if let Some(v) = omarchy_path {
        cmd.env("OMARCHY_PATH", v);
    }
    let out = cmd.output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    json(&out)
}

mod omarchy_path {
    use super::*;

    #[test]
    fn the_plugins_collector_runs_without_omarchy_path() {
        // a capture over ssh or from cron: no OMARCHY_PATH in the engine's
        // environment; list and catalog get the package install's root
        for given in [None, Some("")] {
            let env = Env::new(Snapper::NoPermissions);
            env.init_logbook();
            strict_stubs(&env);
            let out = capture(&env, "plugins", given);
            let plugins = collector(&out, "plugins");
            assert_eq!(plugins["ok"], true, "{given:?}: {out}");
            assert_eq!(
                logged(&env),
                [
                    "/usr/share/omarchy omarchy plugin list --json",
                    "/usr/share/omarchy omarchy plugin catalog",
                ],
                "{given:?}"
            );
        }
    }

    #[test]
    fn a_set_omarchy_path_is_passed_on_unchanged() {
        // a dev checkout (`omarchy dev link`) is trusted like PATH
        let env = Env::new(Snapper::NoPermissions);
        env.init_logbook();
        strict_stubs(&env);
        let out = capture(&env, "plugins", Some("/opt/omarchy-dev"));
        assert_eq!(collector(&out, "plugins")["ok"], true, "{out}");
        assert_eq!(
            logged(&env),
            [
                "/opt/omarchy-dev omarchy plugin list --json",
                "/opt/omarchy-dev omarchy plugin catalog",
            ]
        );
    }

    #[test]
    fn the_omarchy_collector_reads_its_version_without_omarchy_path() {
        // without the variable the version must not fall back to the
        // package query: there is no package manager on this PATH
        let env = Env::new(Snapper::NoPermissions);
        env.init_logbook();
        strict_stubs(&env);
        let out = capture(&env, "omarchy", None);
        assert_eq!(collector(&out, "omarchy")["ok"], true, "{out}");
        assert_eq!(logged(&env), ["/usr/share/omarchy omarchy-version "]);
    }

    #[test]
    fn the_doctor_probe_runs_omarchy_version_without_omarchy_path() {
        let env = Env::new(Snapper::NoPermissions);
        let root = env.init_logbook();
        strict_stubs(&env);
        let out = env.seldon(&["doctor", "--path", root.to_str().unwrap(), "--json"]);
        let v = json(&out);
        let omarchy = v["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "omarchy")
            .unwrap();
        assert_eq!(omarchy["status"], "ok", "{v}");
        assert_eq!(omarchy["message"], "Omarchy 4.0.4-1");
        assert_eq!(logged(&env), ["/usr/share/omarchy omarchy-version "]);
    }

    #[test]
    fn the_dossier_queries_omarchy_without_omarchy_path() {
        let env = Env::new(Snapper::Missing);
        let lb = env.tmp.path().join("logbook");
        copy_dir(&fixture_logbook(), &lb);
        strict_stubs(&env);
        let out = env
            .command(&["--logbook", lb.to_str().unwrap(), "--json", "dossier"])
            .env("SELDON_NOW", "2026-10-01T17:05:12+02:00")
            .env("SELDON_HARDWARE_ROOT", common::hardware_root())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        for section in ["plugins.list", "omarchy.summary"] {
            assert_ne!(v["sections"][section], "skipped", "{section}: {v}");
        }
        assert_eq!(
            logged(&env),
            [
                "/usr/share/omarchy omarchy-version ",
                "/usr/share/omarchy omarchy plugin list --json",
            ]
        );
    }

    #[test]
    fn the_dossier_reads_the_package_lists_below_omarchy_path() {
        // without SELDON_OMARCHY_PACKAGES the lists are
        // `$OMARCHY_PATH/install/*.packages`; the default root is the
        // host's, so only a set variable is tested here
        let env = Env::new(Snapper::Missing);
        env.query_shims();
        let root = env.tmp.path().join("omarchy");
        copy_dir(&common::omarchy_packages(), &root.join("install"));
        let run = |omarchy_path: &Path, lists: bool| {
            let lb = env.tmp.path().join(format!("logbook-{lists}"));
            copy_dir(&fixture_logbook(), &lb);
            let mut cmd = env.command(&[
                "--logbook",
                lb.to_str().unwrap(),
                "--json",
                "dossier",
                "--section",
                "packages",
            ]);
            cmd.env("SELDON_NOW", "2026-10-01T17:05:12+02:00")
                .env_remove("SELDON_OMARCHY_PACKAGES")
                .env("OMARCHY_PATH", omarchy_path);
            let out = cmd.output().unwrap();
            assert_eq!(
                out.status.code(),
                Some(0),
                "{}{}",
                stdout(&out),
                stderr(&out)
            );
            (json(&out), read(&lb.join("system/packages.md")))
        };
        let (with, text) = run(&root, true);
        assert!(
            with["warnings"].as_array().unwrap().is_empty(),
            "the lists were found: {with}"
        );
        let (without, other) = run(&env.tmp.path().join("nowhere"), false);
        assert!(
            !without["warnings"].as_array().unwrap().is_empty(),
            "no lists below another root: {without}"
        );
        assert_ne!(text, other);
    }
}

mod desktop_entries {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn entry(env: &Env, name: &str) -> PathBuf {
        env.home.join(".local/share/applications").join(name)
    }

    /// `(kind, subject)` of the config events in the ledger.
    fn config_events(root: &Path) -> Vec<(String, String)> {
        common::ledger(root)
            .into_iter()
            .filter(|e| e["source"] == "config")
            .map(|e| {
                (
                    e["kind"].as_str().unwrap().to_string(),
                    e["subject"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn the_defaults_watch_the_desktop_entries() {
        assert!(DEFAULT_WATCH_PATHS.contains(&"~/.local/share/applications"));
        // `init` writes the defaults into config.toml
        let env = Env::new(Snapper::NoPermissions);
        env.init_logbook();
        let config = Config::load(&env.config_file()).unwrap().unwrap();
        assert_eq!(config.watch_paths, DEFAULT_WATCH_PATHS);
    }

    #[test]
    fn a_desktop_entry_is_recorded_and_the_mime_cache_is_not() {
        let env = Env::new(Snapper::NoPermissions);
        let root = env.init_logbook();
        write(&entry(&env, "HEY.desktop"), "[Desktop Entry]\nName=HEY\n");
        write(&entry(&env, "mimeinfo.cache"), "[MIME Cache]\n");
        assert_eq!(capture(&env, "config", None)["written"], 0, "baseline");

        write(&entry(&env, "Zoom.desktop"), "[Desktop Entry]\nName=Zoom\n");
        write(
            &entry(&env, "mimeinfo.cache"),
            "[MIME Cache]\nx-scheme-handler/zoommtg=Zoom.desktop;\n",
        );
        let out = capture(&env, "config", None);
        assert_eq!(collector(&out, "config")["ok"], true, "{out}");
        assert_eq!(
            config_events(&root),
            [(
                "config-add".to_string(),
                "~/.local/share/applications/Zoom.desktop".to_string()
            )]
        );
        let manifest: Manifest =
            serde_json::from_str(&read(&env.home.join(".local/state/seldon/manifest.json")))
                .unwrap();
        let current = &manifest.current;
        let names: Vec<&String> = current.files.keys().chain(&current.skipped).collect();
        assert_eq!(
            names,
            [
                "~/.local/share/applications/HEY.desktop",
                "~/.local/share/applications/Zoom.desktop",
            ],
            "the cache is neither hashed nor listed"
        );
    }

    #[test]
    fn the_desktop_entries_entering_the_scope_are_no_additions() {
        // a config.toml without `watchPaths` takes the defaults: after the
        // upgrade the desktop entries enter the scope of a manifest taken
        // with the old list
        let env = Env::new(Snapper::NoPermissions);
        let root = env.init_logbook();
        let mut config = Config::load(&env.config_file()).unwrap().unwrap();
        config.watch_paths = DEFAULT_WATCH_PATHS
            .iter()
            .filter(|p| **p != "~/.local/share/applications")
            .map(|p| p.to_string())
            .collect();
        config.save(&env.config_file()).unwrap();
        write(&env.home.join(".bashrc"), "export A=1\n");
        write(&entry(&env, "HEY.desktop"), "[Desktop Entry]\nName=HEY\n");
        write(&entry(&env, "X.desktop"), "[Desktop Entry]\nName=X\n");
        write(&entry(&env, "mimeinfo.cache"), "[MIME Cache]\n");
        assert_eq!(capture(&env, "config", None)["written"], 0, "baseline");

        let mut table: toml::Table = read(&env.config_file()).parse().unwrap();
        assert!(table.remove("watchPaths").is_some());
        std::fs::write(env.config_file(), toml::to_string(&table).unwrap()).unwrap();
        let out = capture(&env, "config", None);
        assert_eq!(out["written"], 0, "{out}");
        assert_eq!(
            collector(&out, "config")["message"],
            "watch scope changed: 0 file(s) left it, 2 entered it; no events for them"
        );
        assert!(config_events(&root).is_empty());

        // from now on they are watched
        write(&entry(&env, "X.desktop"), "[Desktop Entry]\nName=X 2\n");
        let out = capture(&env, "config", None);
        assert_eq!(out["written"], 1, "{out}");
        assert_eq!(
            config_events(&root),
            [(
                "config-change".to_string(),
                "~/.local/share/applications/X.desktop".to_string()
            )]
        );
    }
}
