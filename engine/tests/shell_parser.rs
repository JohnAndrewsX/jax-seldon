//! One shell parser for the hook and attribution (WP-071; SPEC-ENGINE §4,
//! §8). The hook's classification (`commands::hook::classify`) and the
//! package intent attribution reads from a recorded line
//! (`pkgcmd::command_intent`) both read a line through `parse_shell`,
//! `simple_commands` and `command_argv`. One table of lines is read by
//! both entry points in-process, and end to end through `seldon hook
//! generic`: the intent of the line the hook records is the intent of the
//! line the agent ran.
//!
//! The table holds the review's cases: wrappers (`pkexec`, `run0`, `sudo`
//! option clusters), print-only package commands (`yay --version`), `>&
//! file`, heredocs inside `$(…)`, backticks and double quotes, `<<` inside
//! `((…))`, and the directories the classifier follows (`pushd`, `popd`,
//! `env -C`, `git -C`).

mod common;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use common::{Env, Snapper, stderr, stdout};
use seldon::attribution::causes;
use seldon::commands::hook::{Scope, classify};
use seldon::config::{Config, Dirs};
use seldon::model::event::{Event, Zone};
use seldon::pkgcmd::{Intent, command_intent, parse_shell};
use serde_json::{Value, json};

use Zone::{Green, Red, Yellow};

/// A line, the zone the hook records it with while a case is set (`None`:
/// no record), and the package intent attribution reads from it.
struct Row {
    line: &'static str,
    hook: Option<Zone>,
    full_upgrade: bool,
    packages: &'static [&'static str],
}

const fn row(
    line: &'static str,
    hook: Option<Zone>,
    full_upgrade: bool,
    packages: &'static [&'static str],
) -> Row {
    Row {
        line,
        hook,
        full_upgrade,
        packages,
    }
}

/// Rows whose hook record is red while the intent is empty name no package
/// and are no full upgrade (`-Sy`, `-Yc`): nothing to attribute by name.
const TABLE: &[Row] = &[
    // wrappers (F-530, F-531)
    row("pkexec pacman -S x", Some(Red), false, &["x"]),
    row("pkexec --user root yay -S x", Some(Red), false, &["x"]),
    row("pkexec --version", None, false, &[]),
    row("run0 pacman -S x", Some(Red), false, &["x"]),
    row(
        "run0 --user=root --setenv FOO=1 pacman -S x",
        Some(Red),
        false,
        &["x"],
    ),
    row("run0 --help", None, false, &[]),
    row("sudo -Eu root pacman -S x", Some(Red), false, &["x"]),
    row("sudo -u root pacman -S x", Some(Red), false, &["x"]),
    row("sudo -uroot pacman -S x", Some(Red), false, &["x"]),
    row("sudo --user=root pacman -S x", Some(Red), false, &["x"]),
    row("sudo -k pacman -S x", Some(Red), false, &["x"]),
    row("sudo -l pacman -Syu", None, false, &[]),
    row("doas -nu root pacman -S x", Some(Red), false, &["x"]),
    row("timeout 600 yay -S x", Some(Red), false, &["x"]),
    row("nice -n 10 pacman -S x", Some(Red), false, &["x"]),
    row("time pacman -S x", Some(Red), false, &["x"]),
    row("nohup yay -S x", Some(Red), false, &["x"]),
    row("env -u FOO pacman -S x", Some(Red), false, &["x"]),
    row("bash -c 'yay -S x'", Some(Red), false, &["x"]),
    row("sudo sh -lc \"pacman -S x\"", Some(Red), false, &["x"]),
    row("command -v yay", None, false, &[]),
    row("command -v yay && yay -S x", Some(Red), false, &["x"]),
    // what a package command asks for (F-532)
    row("yay --version", None, false, &[]),
    row("yay -V", None, false, &[]),
    row("yay -h", None, false, &[]),
    row("paru --help", None, false, &[]),
    row("yay -S --help", None, false, &[]),
    row("yay -Y --gendb", None, false, &[]),
    row("yay -Ps", None, false, &[]),
    row("yay -G x", None, false, &[]),
    row("yay -Yc", Some(Red), false, &[]),
    row("yay", Some(Red), true, &[]),
    row("pacman -Syu", Some(Red), true, &[]),
    row("pacman -Sy", Some(Red), false, &[]),
    row("pacman -Qi x", None, false, &[]),
    row("P=zed; pacman -S $P", Some(Red), false, &["zed"]),
    // Omarchy routes, as a command and as a script
    row(
        "omarchy update",
        Some(Red),
        true,
        &["archlinux-keyring", "omarchy-keyring"],
    ),
    row("omarchy update available", None, false, &[]),
    row("omarchy pkg add x y", Some(Red), false, &["x", "y"]),
    row("omarchy-pkg-aur-add x", Some(Red), false, &["x"]),
    row("omarchy pkg present x", None, false, &[]),
    row("omarchy-pkg-present x", None, false, &[]),
    // redirections (F-534)
    row("echo x >& ~/.config/hypr/a.conf", Some(Yellow), false, &[]),
    row("echo x >&~/.config/hypr/a.conf", Some(Yellow), false, &[]),
    row("echo x >&2", None, false, &[]),
    row("echo x 2>&1 >&- | cat", None, false, &[]),
    // heredocs (F-536): bodies are stdin, never commands
    row(
        "echo \"$(cat <<EOF\nsecret_line=1\nEOF\n)\" > ~/.config/hypr/b.conf\npacman -S x",
        Some(Red),
        false,
        &["x"],
    ),
    row(
        "x=`cat <<EOF\npacman -S y )\nEOF\n`; pacman -S x",
        Some(Red),
        false,
        &["x"],
    ),
    row("cat <<EOF\npacman -S y\nEOF", None, false, &[]),
    row(
        "git commit -m \"$(cat <<'EOF'\nfix: don't pacman -S y\nEOF\n)\"",
        Some(Green),
        false,
        &[],
    ),
    row("(( n = 1 << 3 ))\npacman -S x", Some(Red), false, &["x"]),
    row(
        "echo $((1 << 3)) $[1 << 2]; yay -S x",
        Some(Red),
        false,
        &["x"],
    ),
    // directories (cd, pushd, popd, -C): redirections in the shell's
    // directory, a program's operands where it runs
    row(
        "pushd ~/.config/hypr && sed -i s/a/b/ bindings.conf",
        Some(Yellow),
        false,
        &[],
    ),
    row(
        "pushd ~/.config/hypr; popd; sed -i s/a/b/ bindings.conf",
        None,
        false,
        &[],
    ),
    row(
        "env -C ~/.config/hypr sed -i s/a/b/ bindings.conf",
        Some(Yellow),
        false,
        &[],
    ),
    row(
        "sudo -D ~/.config/hypr tee a.conf",
        Some(Yellow),
        false,
        &[],
    ),
    row("env -C ~/.config/hypr cat x > a.conf", None, false, &[]),
    row(
        "cd ~/.config/hypr && git commit -C HEAD",
        Some(Yellow),
        false,
        &[],
    ),
    row(
        "git -C ~/.config/hypr commit -m x",
        Some(Yellow),
        false,
        &[],
    ),
    row(
        "F=~/.config/hypr/a.conf; echo x > $F",
        Some(Yellow),
        false,
        &[],
    ),
];

fn scope() -> Scope {
    let dirs = Dirs {
        home: "/home/user".into(),
        xdg_config_home: "/home/user/.config".into(),
        state_dir: "/home/user/.local/state/seldon".into(),
    };
    Scope::new(&dirs, &Config::default(), Path::new("/home/user/Seldon"))
}

fn expected(r: &Row) -> Intent {
    Intent {
        full_upgrade: r.full_upgrade,
        packages: r.packages.iter().map(|p| p.to_string()).collect(),
    }
}

/// Both entry points, in-process, against the table; every mismatch is
/// listed at once.
#[test]
fn hook_and_intent_read_the_table_alike() {
    assert!(TABLE.len() >= 30);
    let scope = scope();
    let mut wrong = Vec::new();
    for r in TABLE {
        let hook = classify(&parse_shell(r.line), &scope, Path::new("/home/user/Seldon"))
            .and_then(|m| m.zone);
        let intent = command_intent(r.line);
        if hook != r.hook || intent != expected(r) {
            wrong.push(format!("{:?}: hook {hook:?}, intent {intent:?}", r.line));
        }
        // whatever attribution can attribute, the hook records red
        if intent != Intent::default() && hook != Some(Red) {
            wrong.push(format!("{:?}: intent {intent:?} but hook {hook:?}", r.line));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// A scratch logbook with an active case, for `seldon hook generic`.
struct Bench {
    env: Env,
    logbook: PathBuf,
}

impl Bench {
    fn new() -> Self {
        let env = Env::new(Snapper::NoPermissions);
        let logbook = env.init_logbook();
        let bench = Bench { env, logbook };
        let out = bench.run(&["plan", "new", "Parser", "--json"], None);
        let id = common::json(&out)["event"]["subject"]
            .as_str()
            .unwrap()
            .to_string();
        bench.run(&["plan", "start", &id], None);
        bench
    }

    fn run(&self, args: &[&str], stdin: Option<&str>) -> Output {
        let tmp = self.env.tmp.path();
        let mut child = self
            .env
            .command(args)
            .env("SELDON_PACMAN_LOG", tmp.join("pacman.log"))
            .env("SELDON_PACMAN_DB_LOCK", tmp.join("no-db.lck"))
            .env("SELDON_OMARCHY", tmp.join("no-omarchy"))
            .env("SELDON_OMARCHY_PLUGINS_DIR", tmp.join("plugins"))
            .env("SELDON_THEME_FILE", tmp.join("theme.name"))
            .env("SELDON_NOW", "2026-10-03T09:00:00+02:00")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.unwrap_or("").as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        out
    }

    /// The event `seldon hook generic` writes for `command`, if any.
    fn hook(&self, command: &str, n: usize) -> Option<Value> {
        let before = common::ledger(&self.logbook).len();
        let payload = json!({
            "command": command,
            "actor": "agent:codex",
            "cwd": self.logbook,
            "startedAt": format!("2026-10-03T10:{:02}:{:02}+02:00", n / 60, n % 60),
        });
        let out = self.run(&["hook", "generic"], Some(&payload.to_string()));
        assert_eq!(stdout(&out), "", "hooks print nothing");
        let after = common::ledger(&self.logbook);
        assert!(after.len() <= before + 1, "{command:?}: one event at most");
        after.get(before).cloned()
    }

    /// `[redaction] skipPaths`.
    fn skip(&self, patterns: &[&str]) {
        let file = self.env.config_file();
        let mut config = Config::load(&file).unwrap().unwrap();
        config.redaction.skip_paths = patterns.iter().map(|p| p.to_string()).collect();
        config.save(&file).unwrap();
    }
}

/// End to end: what the hook records for each line has the table's zone,
/// and attribution reads the table's intent from the recorded line
/// (`attribution::causes`, the pacman collector's input).
#[test]
fn the_recorded_line_carries_the_intent() {
    let b = Bench::new();
    let mut wrong = Vec::new();
    for (n, r) in TABLE.iter().enumerate() {
        let event = b.hook(r.line, n);
        let zone = event
            .as_ref()
            .and_then(|e| e["zone"].as_str())
            .map(String::from);
        if zone.as_deref() != r.hook.map(|z| z.as_str()) {
            wrong.push(format!("{:?}: recorded zone {zone:?}", r.line));
        }
        let Some(event) = event else { continue };
        assert_eq!(event["kind"], "command");
        assert_eq!(event["actor"], "agent:codex");
        assert!(event["case"].is_string(), "the active case");
        let event: Event = serde_json::from_value(event).unwrap();
        let intent = causes(std::slice::from_ref(&event))[0].intent.clone();
        if intent != expected(r) {
            wrong.push(format!(
                "{:?}: recorded {:?}, intent {intent:?}",
                r.line, event.meta.command
            ));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// F-536: a heredoc body inside `$(…)`, backticks or a double-quoted
/// substitution is stdin and never reaches the record; the command after
/// it is still the one recorded. The delimiter line stays when anything
/// follows it, so the record reads as the same commands. `<<` inside
/// `((…))` starts no heredoc.
#[test]
fn heredoc_bodies_in_substitutions_are_cut() {
    let b = Bench::new();
    for (n, (line, kept)) in [
        (
            "echo \"$(cat <<EOF\nsecret_line_B=1\nEOF\n)\" > ~/.config/hypr/b.conf\npacman -S zedk",
            "echo \"$(cat <<EOF\nEOF\n)\" > ~/.config/hypr/b.conf\npacman -S zedk",
        ),
        (
            "x=`cat <<'EOF'\nsecret_line_B=1 ) `\nEOF\n`\npacman -S zedk",
            "x=`cat <<'EOF'\nEOF\n`\npacman -S zedk",
        ),
        (
            "git commit -q -m \"$(cat <<'EOF'\nsecret_line_B=1 \" '\nEOF\n)\" && pacman -S zedk",
            "git commit -q -m \"$(cat <<'EOF'\nEOF\n)\" && pacman -S zedk",
        ),
        (
            "diff <(cat <<EOF\nsecret_line_B=1\nEOF\n) x; pacman -S zedk",
            "diff <(cat <<EOF\nEOF\n) x; pacman -S zedk",
        ),
        (
            "(( n = 1 << 3 ))\npacman -S zedk",
            "(( n = 1 << 3 ))\npacman -S zedk",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let e = b
            .hook(line, n)
            .unwrap_or_else(|| panic!("{line:?}: not recorded"));
        assert_eq!(
            (&e["subject"], &e["zone"]),
            (&json!("pacman"), &json!("red"))
        );
        assert_eq!(e["meta"]["command"], kept, "{line:?}");
    }
}

/// The orchestrator's addition to WP-071: a path built from a variable the
/// line sets, or written with a glob, is read for `[redaction] skipPaths`
/// like the path it can name; the commands record as `<program>
/// ‹redacted›`. The paths are under the scratch HOME.
#[test]
fn skip_paths_read_variables_and_globs() {
    let b = Bench::new();
    b.skip(&["~/d/private.conf"]);
    let redacted = "sed ‹redacted›";
    for (n, (line, recorded)) in [
        ("F=private.conf; sed -i s/a/b/ ~/d/$F", redacted),
        ("export D=~/d; sed -i s/a/b/ \"$D/private.conf\"", redacted),
        ("sed -i s/a/b/ ~/d/priv*.conf", redacted),
        ("sed -i s/a/b/ ~/d/[p]rivate.conf", redacted),
        ("sed -i s/a/b/ /tmp/x \"$(pwd)\"/private.conf", redacted),
        // a word that is only an unknown value names no path
        (
            "cd ~/d && sed -i s/a/b/ $NAME",
            "cd ~/d && sed -i s/a/b/ $NAME",
        ),
        (
            "sed -i s/a/b/ ~/d/public.conf",
            "sed -i s/a/b/ ~/d/public.conf",
        ),
        ("sed -i s/a/b/ ~/d/*.txt", "sed -i s/a/b/ ~/d/*.txt"),
        (
            "F=public.conf; sed -i s/a/b/ ~/d/$F",
            "F=public.conf; sed -i s/a/b/ ~/d/$F",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let e = b
            .hook(line, n)
            .unwrap_or_else(|| panic!("{line:?}: not recorded"));
        assert_eq!(e["meta"]["command"], recorded, "{line:?}");
    }
}

/// The review's hook-level cases one by one, with the payload a coding
/// agent would send.
#[test]
fn review_cases_through_the_hook() {
    let b = Bench::new();
    let cases = [
        ("pkexec pacman -S x", Some(("pacman", "red"))),
        ("run0 pacman -S x", Some(("pacman", "red"))),
        ("sudo -Eu root pacman -S x", Some(("pacman", "red"))),
        ("yay --version", None),
        ("echo x >& ~/.config/hypr/a", Some(("echo", "yellow"))),
    ];
    for (n, (line, want)) in cases.into_iter().enumerate() {
        let got = b.hook(line, n).map(|e| {
            (
                e["subject"].as_str().unwrap().to_string(),
                e["zone"].as_str().unwrap().to_string(),
            )
        });
        let want = want.map(|(s, z)| (s.to_string(), z.to_string()));
        assert_eq!(got, want, "{line:?}");
    }
}
