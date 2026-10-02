//! `seldon completions <bash|zsh|fish>` and `seldon mangen` (WP-049):
//! generated from the clap definition that `main.rs` passes in, so they
//! cannot drift from `--help`. Neither reads a config, a logbook or the
//! home directory.
//!
//! The man page is seldon(1) alone: clap_mangen's name, synopsis,
//! description and global options, then a COMMANDS section of our own with
//! every command and subcommand (usage, one sentence, its arguments) in
//! place of clap_mangen's list of `seldon-<command>(1)` pages that do not
//! exist, then exit status, environment and files (SPEC-ENGINE §2 §3).

use clap::Command;
use clap_complete::Shell;
use serde_json::json;

use super::Output;
use crate::VERSION;
use crate::error::{Error, Result};

/// The shells `seldon completions` takes.
pub const SHELLS: [&str; 3] = ["bash", "zsh", "fish"];

/// `seldon completions <shell>`: the script; with `--json`
/// `{"shell", "script"}`.
pub fn completions(mut cmd: Command, shell: &str) -> Result<Output> {
    let shell = match shell {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        "fish" => Shell::Fish,
        other => return Err(Error::user(format!("no completions for `{other}`"))),
    };
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut cmd, "seldon", &mut script);
    let script = String::from_utf8(script).map_err(anyhow::Error::from)?;
    Ok(Output::ok(
        script.trim_end(),
        json!({ "shell": shell.to_string(), "script": script }),
    ))
}

/// `seldon mangen`: seldon(1) in roff; with `--json` `{"manPage"}`.
pub fn mangen(cmd: Command) -> Result<Output> {
    let page = man_page(cmd)?;
    Ok(Output::ok(page.trim_end(), json!({ "manPage": page })))
}

/// seldon(1) for the CLI `cmd`.
pub fn man_page(cmd: Command) -> Result<String> {
    let mut cmd = cmd.version(VERSION);
    cmd.build();
    let man = clap_mangen::Man::new(cmd.clone())
        .title("SELDON")
        .section("1")
        .source(format!("seldon {VERSION}"))
        .manual("Seldon Manual");
    let mut page = Vec::new();
    let render = |page: &mut Vec<u8>| -> std::io::Result<()> {
        man.render_title(page)?;
        man.render_name_section(page)?;
        man.render_synopsis_section(page)?;
        man.render_description_section(page)?;
        man.render_options_section(page)
    };
    render(&mut page).map_err(anyhow::Error::from)?;
    let mut page = String::from_utf8(page).map_err(anyhow::Error::from)?;
    page.push_str(".SH COMMANDS\n");
    commands(&cmd, &mut page);
    page.push_str(MAN_TAIL);
    Ok(page)
}

/// One `.TP` paragraph per command below `cmd`, depth first: the usage
/// line in bold, the help's first line, then its own arguments (the global
/// options are in OPTIONS). `help` and hidden commands are left out.
fn commands(cmd: &Command, page: &mut String) {
    for sub in cmd
        .get_subcommands()
        .filter(|s| s.get_name() != "help" && !s.is_hide_set())
    {
        let usage = sub.clone().render_usage().to_string();
        let usage = usage.strip_prefix("Usage: ").unwrap_or(&usage);
        for (i, line) in usage.lines().enumerate() {
            page.push_str(if i == 0 { ".TP\n" } else { ".br\n" });
            page.push_str(&format!("\\fB{}\\fR\n", escape(line.trim())));
        }
        if let Some(about) = sub.get_about() {
            page.push_str(&format!("{}\n", escape(&about.to_string())));
        }
        let args: Vec<_> = sub
            .get_arguments()
            .filter(|a| !a.is_global_set() && !a.is_hide_set())
            .filter(|a| !matches!(a.get_id().as_str(), "help" | "version"))
            .collect();
        if !args.is_empty() {
            page.push_str(".RS\n");
            for a in args {
                page.push_str(&format!(".TP\n\\fB{}\\fR\n", escape(&arg_name(a))));
                if let Some(help) = a.get_help() {
                    page.push_str(&format!("{}\n", escape(&help.to_string())));
                }
                if a.get_action().takes_values() {
                    page.push_str(&value_notes(a));
                }
            }
            page.push_str(".RE\n");
        }
        commands(sub, page);
    }
}

/// `[possible values: …]` and `[default: …]` lines as `--help` shows them.
fn value_notes(a: &clap::Arg) -> String {
    let mut notes = String::new();
    let possible: Vec<_> = a
        .get_possible_values()
        .iter()
        .filter(|v| !v.is_hide_set())
        .map(|v| v.get_name().to_string())
        .collect();
    if !possible.is_empty() {
        notes.push_str(&format!(
            "[possible values: {}]\n",
            escape(&possible.join(", "))
        ));
    }
    let defaults: Vec<_> = a
        .get_default_values()
        .iter()
        .map(|v| v.to_string_lossy())
        .collect();
    if !defaults.is_empty() {
        notes.push_str(&format!("[default: {}]\n", escape(&defaults.join(", "))));
    }
    notes
}

/// `--name <VALUE>`, `-s, --name`, or `<NAME>` for a positional.
fn arg_name(a: &clap::Arg) -> String {
    let value = a
        .get_value_names()
        .and_then(|v| v.first())
        .map(|v| format!("<{v}>"))
        .unwrap_or_else(|| format!("<{}>", a.get_id().as_str().to_uppercase()));
    if a.is_positional() {
        return value;
    }
    let mut name = match (a.get_short(), a.get_long()) {
        (Some(s), Some(l)) => format!("-{s}, --{l}"),
        (Some(s), None) => format!("-{s}"),
        (None, Some(l)) => format!("--{l}"),
        (None, None) => value.clone(),
    };
    if a.get_action().takes_values() {
        name.push(' ');
        name.push_str(&value);
    }
    name
}

/// Text for a roff line: backslashes, hyphens, backticks (which roff
/// prints as an opening quote) and non-ASCII characters (`…`, `→`; troff
/// reads bytes unless `preconv` runs first) escaped, a leading `.` or `'`
/// (a request) neutralised.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\e"),
            '-' => out.push_str("\\-"),
            '`' => out.push_str("\\(ga"),
            c if c.is_ascii() => out.push(c),
            c => out.push_str(&format!("\\[u{:04X}]", c as u32)),
        }
    }
    if out.starts_with(['.', '\'']) {
        out.insert_str(0, "\\&");
    }
    out
}

/// The sections of seldon(1) that clap does not know.
const MAN_TAIL: &str = r#".SH "EXIT STATUS"
.TP
.B 0
ok
.TP
.B 1
user error: bad arguments, an unknown case or event
.TP
.B 2
engine error
.TP
.B 3
the logbook is not initialised (run \fBseldon init\fR)
.TP
.B 4
the state lock is held by another seldon
.SH ENVIRONMENT
.TP
.B SELDON_LOGBOOK
the logbook directory, unless \fB\-\-logbook\fR is given
.TP
.B SELDON_CONFIG
the config file, unless \fB\-\-config\fR is given
.TP
.B SELDON_NOW
an RFC 3339 time that replaces the clock (tests and demos)
.SH FILES
.TP
.I ~/.config/seldon/config.toml
the config
.TP
.I ~/.local/state/seldon/index.json
the index the Omarchy plugin reads
.TP
.I ~/Seldon
the default logbook
.SH "SEE ALSO"
\fBseldon\fR \fIcommand\fR \fB\-\-help\fR prints the same help with examples.
\fBomarchy\fR(1), \fBsnapper\fR(8), \fBgit\fR(1)
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_keeps_roff_requests_out() {
        assert_eq!(escape("--since <TS>"), "\\-\\-since <TS>");
        assert_eq!(escape(".seldon/active-case"), "\\&.seldon/active\\-case");
        assert_eq!(escape("a\\b"), "a\\eb");
        assert_eq!(escape("`x`"), "\\(gax\\(ga");
        assert_eq!(
            escape("queued → active …"),
            "queued \\[u2192] active \\[u2026]"
        );
    }

    #[test]
    fn unknown_shells_are_user_errors() {
        let err = completions(Command::new("seldon"), "tcsh").unwrap_err();
        assert_eq!(err.exit(), crate::error::Exit::UserError);
    }
}
