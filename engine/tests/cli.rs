//! CLI smoke tests: version, contract version, parse errors, exit codes,
//! the config file override. Every run goes through `common::Env` (a fake
//! HOME and XDG dirs), so nothing here can touch the real config or state.

mod common;

use std::process::Output;

use common::{Env, Snapper, json, stderr, stdout};

fn seldon(args: &[&str]) -> Output {
    Env::new(Snapper::Missing).seldon(args)
}

#[test]
fn version_prints_name_and_version() {
    let out = seldon(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(stdout(&out), format!("seldon {}\n", seldon::VERSION));
    assert_version_is_this_build(seldon::VERSION);
}

/// The crate version, plus `+<SELDON_BUILD>` when the build set it
/// (WP-098): `SELDON_BUILD=main.1a2b3c4 cargo test` checks the marked form.
fn assert_version_is_this_build(v: &str) {
    match option_env!("SELDON_BUILD") {
        Some(b) if !b.is_empty() => {
            assert_eq!(v, format!("{}+{b}", env!("CARGO_PKG_VERSION")))
        }
        _ => assert_eq!(v, env!("CARGO_PKG_VERSION")),
    }
}

#[test]
fn version_json() {
    let out = seldon(&["--version", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["name"], "seldon");
    assert_eq!(v["version"], seldon::VERSION);
    assert_version_is_this_build(v["version"].as_str().unwrap());
}

#[test]
fn contract_version_prints_one() {
    let out = seldon(&["contract-version"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(stdout(&out), "1\n");
}

#[test]
fn contract_version_json() {
    let out = seldon(&["contract-version", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["contractVersion"], 1);
}

#[test]
fn contract_version_matches_plugin_manifest() {
    let manifest = include_str!("../../plugin/manifest.json");
    let m: serde_json::Value = serde_json::from_str(manifest).unwrap();
    let out = seldon(&["contract-version"]);
    assert_eq!(
        stdout(&out).trim(),
        m["seldon"]["contractVersion"].to_string()
    );
}

#[test]
fn unknown_command_is_user_error() {
    assert_eq!(seldon(&["no-such-command"]).status.code(), Some(1));
}

#[test]
fn unknown_command_json_is_user_error() {
    let out = seldon(&["no-such-command", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["error"]["code"], 1);
}

#[test]
fn no_command_is_user_error() {
    assert_eq!(seldon(&[]).status.code(), Some(1));
}

// WP-001 review follow-ups (SPEC-ENGINE §3 "JSON shapes").

#[test]
fn json_error_carries_the_full_detail() {
    let out = seldon(&["no-such-command", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let message = v["error"]["message"].as_str().unwrap();
    assert!(message.contains("no-such-command"), "{message}");
    assert!(!message.starts_with("error:"), "{message}");
    assert!(!message.contains("Usage:"), "{message}");
}

#[test]
fn json_after_double_dash_is_not_the_flag() {
    let out = seldon(&["no-such-command", "--", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stdout(&out), "", "must not answer in JSON");
    assert!(String::from_utf8_lossy(&out.stderr).contains("no-such-command"));
}

#[test]
fn json_is_found_past_an_invalid_value() {
    for args in [
        &["init", "--language", "fr", "--json"][..],
        &["--json", "init", "--language", "fr"][..],
        &["init", "--path", "--json"][..],
        &["doctor", "--bogus", "--json"][..],
    ] {
        let out = seldon(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let v: serde_json::Value = serde_json::from_str(&stdout(&out))
            .unwrap_or_else(|e| panic!("{args:?}: {e}: {}", stdout(&out)));
        assert_eq!(v["error"]["code"], 1);
    }
}

#[test]
fn parse_error_without_the_flag_is_text() {
    // `json` is the value of `--path`, not a flag spelled differently.
    let out = seldon(&["init", "--path", "json", "--language", "xx"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stdout(&out), "");
}

#[test]
fn help_exits_zero() {
    for args in [
        &["--help"][..],
        &["init", "--help"][..],
        &["doctor", "--help"][..],
    ] {
        assert_eq!(seldon(args).status.code(), Some(0), "{args:?}");
    }
}

// WP-003 review follow-up: `--config FILE` > `SELDON_CONFIG` > XDG default.

fn config_with_logbook(env: &Env, name: &str, logbook: &std::path::Path) -> std::path::PathBuf {
    let path = env.tmp.path().join(name);
    std::fs::write(&path, format!("logbook = \"{}\"\n", logbook.display())).unwrap();
    path
}

#[test]
fn config_file_precedence() {
    let env = Env::new(Snapper::Missing);
    let a = env.init_logbook_at("a", "en"); // the XDG config points at a
    let b = env.init_logbook_at("b", "en");
    let c = env.init_logbook_at("c", "en");
    // init rewrote the XDG config each time; point it back at a
    let xdg = env.config_file();
    std::fs::write(&xdg, format!("logbook = \"{}\"\n", a.display())).unwrap();
    let from_env = config_with_logbook(&env, "env.toml", &b);
    let from_flag = config_with_logbook(&env, "flag.toml", &c);

    let doctor_logbook = |cmd: &mut std::process::Command| {
        let out = cmd.output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        json(&out)["logbook"].as_str().unwrap().to_string()
    };
    let shown = |p: &std::path::Path| p.display().to_string();
    assert_eq!(
        doctor_logbook(&mut env.command(&["doctor", "--json"])),
        shown(&a)
    );
    assert_eq!(
        doctor_logbook(
            env.command(&["doctor", "--json"])
                .env("SELDON_CONFIG", &from_env)
        ),
        shown(&b)
    );
    assert_eq!(
        doctor_logbook(
            env.command(&["--config", from_flag.to_str().unwrap(), "doctor", "--json"])
                .env("SELDON_CONFIG", &from_env)
        ),
        shown(&c)
    );
    // an empty SELDON_CONFIG is unset
    assert_eq!(
        doctor_logbook(env.command(&["doctor", "--json"]).env("SELDON_CONFIG", "")),
        shown(&a)
    );
}

#[test]
fn init_writes_the_overridden_config_only() {
    let env = Env::new(Snapper::Missing);
    let config = env.tmp.path().join("conf/seldon.toml");
    let root = env.tmp.path().join("lb");
    let out = env.seldon(&[
        "--config",
        config.to_str().unwrap(),
        "init",
        "--non-interactive",
        "--no-capture",
        "--path",
        root.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(json(&out)["config"], config.to_str().unwrap());
    assert!(config.is_file());
    assert!(!env.config_file().exists(), "the XDG config is untouched");
}

#[test]
fn a_broken_overridden_config_is_a_user_error() {
    let env = Env::new(Snapper::Missing);
    let config = env.tmp.path().join("bad.toml");
    std::fs::write(&config, "language = \"fr\"\n").unwrap();
    let out = env.seldon(&[
        "--config",
        config.to_str().unwrap(),
        "plan",
        "list",
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        json(&out)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("bad.toml")
    );
}

#[test]
fn json_detection_skips_free_text() {
    // parse errors after free text: `--json` after `--` is text, an option
    // value that accepts hyphens is text
    for (args, wants_json) in [
        (&["log", "--actor", "Bad", "--", "--json"][..], false),
        (
            &[
                "plan",
                "drop",
                "C-2026-001",
                "--actor",
                "x",
                "--reason",
                "--json",
            ][..],
            false,
        ),
        (&["log", "--actor", "Bad", "--json", "--", "text"][..], true),
    ] {
        let out = seldon(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        assert_eq!(
            stdout(&out).starts_with('{'),
            wants_json,
            "{args:?}: {}",
            stdout(&out)
        );
    }
}
