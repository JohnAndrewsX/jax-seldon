//! CLI smoke tests for the WP-001 scaffold.

use std::process::{Command, Output};

fn seldon(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_seldon"))
        .args(args)
        .output()
        .expect("run seldon")
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("utf-8 stdout")
}

#[test]
fn version_prints_name_and_version() {
    let out = seldon(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(stdout(&out), "seldon 0.1.0\n");
}

#[test]
fn version_json() {
    let out = seldon(&["--version", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["name"], "seldon");
    assert_eq!(v["version"], "0.1.0");
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
