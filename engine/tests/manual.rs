//! `seldon completions <bash|zsh|fish>` and `seldon mangen` (WP-049): both
//! are generated from the clap definition, need no logbook, config or
//! home, and cover every command `--help` lists. Every run goes through
//! `common::Env`; the host's `bash`, `zsh`, `fish`, `groff` and `man` check
//! the output when they are installed and are skipped (with a note on
//! stderr) when they are not.

mod common;

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use common::{Env, Snapper, json, stderr, stdout};

fn seldon(args: &[&str]) -> Output {
    Env::new(Snapper::Missing).seldon(args)
}

/// `seldon args…`, exit 0, stdout.
fn ok(args: &[&str]) -> String {
    let out = seldon(args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
    stdout(&out)
}

/// The host's `program`, or `None` (the check is skipped).
fn host(program: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(|| {
            eprintln!("note: {program} is not installed; its check is skipped");
            None
        })
}

/// `program args…` with `input` on stdin.
fn pipe(program: &str, args: &[&str], input: &str) -> Output {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// Every command `seldon --help` lists, with its subcommands
/// (`plan new`, …); `help` left out.
fn every_command() -> Vec<String> {
    fn commands(help: &str) -> Vec<String> {
        help.lines()
            .skip_while(|l| *l != "Commands:")
            .skip(1)
            .take_while(|l| !l.trim().is_empty())
            .filter_map(|l| l.split_whitespace().next())
            .filter(|n| *n != "help")
            .map(String::from)
            .collect()
    }
    let mut all = Vec::new();
    for top in commands(&ok(&["--help"])) {
        for sub in commands(&ok(&[&top, "--help"])) {
            all.push(format!("{top} {sub}"));
        }
        all.push(top);
    }
    all
}

#[test]
fn bash_completions_parse_and_know_every_command() {
    let script = ok(&["completions", "bash"]);
    assert!(script.contains("complete -F"), "{script}");
    for cmd in every_command() {
        let last = cmd.rsplit(' ').next().unwrap();
        assert!(script.contains(last), "no `{cmd}` in the bash completions");
    }
    for flag in ["--remove-theme-hook", "--settings", "--since", "--snapshot"] {
        assert!(script.contains(flag), "no {flag}");
    }
    if let Some(bash) = host("bash") {
        let out = pipe(&bash, &["-n"], &script);
        assert!(out.status.success(), "bash -n: {}", stderr(&out));
    }
}

#[test]
fn zsh_and_fish_completions() {
    let zsh = ok(&["completions", "zsh"]);
    assert!(zsh.starts_with("#compdef seldon"), "{zsh}");
    assert!(zsh.contains("uninstall"), "{zsh}");
    let fish = ok(&["completions", "fish"]);
    assert!(fish.contains("complete -c seldon"), "{fish}");
    assert!(fish.contains("remove-theme-hook"), "{fish}");
    for (shell, script) in [("zsh", &zsh), ("fish", &fish)] {
        if let Some(program) = host(shell) {
            let out = pipe(&program, &["-n"], script);
            assert!(out.status.success(), "{shell} -n: {}", stderr(&out));
        }
    }
}

#[test]
fn completions_need_no_home_and_report_unknown_shells() {
    // no HOME, no XDG dirs: nothing is resolved
    let out = Command::new(env!("CARGO_BIN_EXE_seldon"))
        .args(["completions", "bash"])
        .env_clear()
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(!out.stdout.is_empty());

    let out = seldon(&["completions", "tcsh", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
    assert!(message.contains("tcsh"), "{message}");

    let v = json(&seldon(&["completions", "fish", "--json"]));
    assert_eq!(v["shell"], "fish");
    assert!(v["script"].as_str().unwrap().contains("complete -c seldon"));
}

#[test]
fn the_man_page_lists_every_command() {
    let page = ok(&["mangen"]);
    assert!(page.starts_with(".ie \\n(.g"), "roff from clap_mangen");
    assert!(page.contains(".TH SELDON 1 "), "{page}");
    assert!(page.contains("seldon 0.1.0"), "the version in the footer");
    for section in [
        "NAME",
        "SYNOPSIS",
        "OPTIONS",
        "COMMANDS",
        "\"EXIT STATUS\"",
        "ENVIRONMENT",
        "FILES",
    ] {
        assert!(page.contains(&format!(".SH {section}\n")), "no {section}");
    }
    for cmd in every_command() {
        let usage = format!("\\fBseldon {}", cmd.replace('-', "\\-"));
        assert!(page.contains(&usage), "no `seldon {cmd}` in the man page");
    }
    // no references to per-command pages that are not shipped
    assert!(!page.contains("seldon\\-init(1)"), "{page}");
    assert_eq!(
        json(&seldon(&["mangen", "--json"]))["manPage"],
        page.trim_end().to_string() + "\n"
    );
}

#[test]
fn the_man_page_renders_without_warnings() {
    let page = ok(&["mangen"]);
    if let Some(groff) = host("groff") {
        let out = pipe(&groff, &["-man", "-Tutf8", "-ww", "-z"], &page);
        assert!(out.status.success(), "{}", stderr(&out));
        assert_eq!(stderr(&out), "", "groff warnings");
    }
    // the acceptance test: `man -l seldon.1` renders
    if let Some(man) = host("man") {
        let tmp = common::TempDir::new("man");
        let file = tmp.path().join("seldon.1");
        std::fs::write(&file, &page).unwrap();
        let out = Command::new(man)
            .args(["-l", "-P", "cat"])
            .arg(Path::new(&file))
            .env("MANWIDTH", "100")
            .output()
            .unwrap();
        assert!(out.status.success(), "man -l: {}", stderr(&out));
        let text = stdout(&out);
        assert!(text.contains("seldon hook uninstall"), "{text}");
        assert!(text.contains("EXIT STATUS"), "{text}");
    }
}

#[test]
fn a_reader_that_closes_early_is_not_an_error() {
    // the read end is closed before seldon writes: its write gets EPIPE
    // (a panic would abort the release binary)
    let env = Env::new(Snapper::Missing);
    let mut child = env
        .command(&["completions", "bash"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(!stderr(&out).contains("panicked"), "{}", stderr(&out));
}

#[test]
fn a_failed_write_to_stdout_exits_2() {
    // `seldon mangen > /dev/full` must not exit 0 with an empty page
    let Ok(full) = std::fs::OpenOptions::new().write(true).open("/dev/full") else {
        eprintln!("note: no /dev/full; the check is skipped");
        return;
    };
    let env = Env::new(Snapper::Missing);
    let out = env
        .command(&["mangen"])
        .stdout(Stdio::from(full))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("cannot write to stdout"),
        "{}",
        stderr(&out)
    );
}
