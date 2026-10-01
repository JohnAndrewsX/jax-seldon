//! Redaction of secrets before an event is written (SPEC-ENGINE §7).
//!
//! Applied by the ledger to `detail` and `meta.command` of every event.
//! Built-in rules: `--password`, `token=`, `Authorization:`,
//! `AKIA[0-9A-Z]{16}`, `ghp_[A-Za-z0-9]{36}`, `sk-[A-Za-z0-9]{20,}`,
//! anything after `-p` for `mysql|psql|smbclient`, URLs with userinfo; plus
//! the user's regexes from `config.toml [redaction] patterns`, each of which
//! replaces its whole match. The replacement is always [`REDACTED`].
//!
//! Every rule errs towards redacting too much: a value may be quoted, a
//! key may be `GITHUB_TOKEN=`, and `sk-proj-…` keys contain hyphens.

use regex::Regex;

use crate::error::{Error, Result};

/// What a secret is replaced with.
pub const REDACTED: &str = "‹redacted›";

/// A quoted or bare value after a key.
const VALUE: &str = r#"(?:"[^"]*"|'[^']*'|[^\s'"&;|]+)"#;

/// One rule: matches of `re` are replaced by group `keep` (a prefix that
/// stays, e.g. the option name) followed by [`REDACTED`].
#[derive(Debug, Clone)]
struct Rule {
    name: &'static str,
    re: Regex,
    keep_prefix: bool,
}

/// The built-in rules plus user patterns, compiled once per process.
#[derive(Debug, Clone)]
pub struct Redactor {
    rules: Vec<Rule>,
}

/// Names of the built-in rules, in the order they run (for tests and docs).
pub const BUILTIN: [&str; 8] = [
    "url-userinfo",
    "password-option",
    "token-assignment",
    "authorization-header",
    "aws-access-key",
    "github-token",
    "openai-key",
    "db-client-password",
];

impl Redactor {
    /// The built-in rules only.
    pub fn builtin() -> Self {
        let rule = |name, pattern: &str, keep_prefix| Rule {
            name,
            re: Regex::new(pattern).expect("built-in redaction pattern compiles"),
            keep_prefix,
        };
        Redactor {
            rules: vec![
                // scheme://user:pass@host → scheme://‹redacted›@host
                rule(
                    "url-userinfo",
                    r"(?i)(\b[a-z][a-z0-9+.-]*://)[^/\s@'\x22]+@",
                    true,
                ),
                rule(
                    "password-option",
                    &format!(r"(?i)(--password(?:=|\s+))(?:{VALUE})"),
                    true,
                ),
                rule(
                    "token-assignment",
                    &format!(r"(?i)(token=)(?:{VALUE})"),
                    true,
                ),
                // the header value up to a closing quote or the end of the line
                rule(
                    "authorization-header",
                    r#"(?i)(authorization:\s*)[^'"\n]+"#,
                    true,
                ),
                rule("aws-access-key", r"AKIA[0-9A-Z]{16}", false),
                rule("github-token", r"ghp_[A-Za-z0-9]{36}", false),
                rule("openai-key", r"sk-[A-Za-z0-9_-]{20,}", false),
                // `mysql … -p secret …`, `-psecret`: everything after -p
                rule(
                    "db-client-password",
                    r"(?m)(\b(?:mysql|psql|smbclient)\b[^\n]*?\s-p ?)\S[^\n]*",
                    true,
                ),
            ],
        }
    }

    /// The built-in rules plus `patterns` (regexes from `config.toml
    /// [redaction] patterns`). An invalid pattern is a user error: Seldon
    /// refuses to write events rather than leak what the pattern was for.
    pub fn with_patterns(patterns: &[String]) -> Result<Self> {
        let mut redactor = Redactor::builtin();
        for p in patterns {
            let re = Regex::new(p).map_err(|e| {
                Error::user(format!(
                    "config.toml [redaction] patterns: invalid regex `{p}`: {e}"
                ))
            })?;
            redactor.rules.push(Rule {
                name: "user-pattern",
                re,
                keep_prefix: false,
            });
        }
        Ok(redactor)
    }

    /// `text` with every secret replaced by [`REDACTED`].
    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for rule in &self.rules {
            let replacement = if rule.keep_prefix {
                format!("${{1}}{REDACTED}")
            } else {
                REDACTED.to_string()
            };
            if rule.re.is_match(&out) {
                out = rule.re.replace_all(&out, replacement.as_str()).into_owned();
            }
        }
        out
    }

    /// Names of the rules that match `text` (diagnostics, tests).
    pub fn matching_rules(&self, text: &str) -> Vec<&'static str> {
        self.rules
            .iter()
            .filter(|r| r.re.is_match(text))
            .map(|r| r.name)
            .collect()
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Redactor::builtin()
    }
}
