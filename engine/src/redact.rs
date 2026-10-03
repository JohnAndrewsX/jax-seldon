//! Redaction of secrets before text is written (SPEC-ENGINE §7).
//!
//! Applied by the ledger to `subject`, `detail` and every string value of
//! `meta` of every event, and by the commands that write free text into
//! the logbook (`log`, `plan`, `decide`, `drift explain|dismiss`) before
//! anything is written, through [`Redactor::for_config`]. Built-in rules:
//! URLs with userinfo, `--password`, `--token`/`--api-key`/`--secret`-style
//! options, `token=` and `…KEY=`/`…TOKEN=`/`…SECRET=`/`…PASSWORD=`-style
//! assignments, `Authorization:` and `X-…-Key:`-style headers, AWS access
//! keys, GitHub, GitLab and Slack tokens, `sk-`/`sk_` keys, anything after
//! `-p` for `mysql|psql|smbclient`, the credentials after `curl -u`, the
//! password after `sshpass -p` and `docker|podman … login -p`; plus the
//! user's regexes from `config.toml [redaction] patterns`, each of which
//! replaces its whole match. The replacement is always [`REDACTED`].
//!
//! Every rule errs towards redacting too much: a value may be quoted, a
//! key may be `GITHUB_TOKEN=`, and `sk-proj-…` keys contain hyphens.
//! Redacting twice gives the same text: a match that lies inside an
//! existing [`REDACTED`] marker is left alone, so text redacted by a
//! command and again by the ledger reads the same in both places.

use regex::{Captures, Regex};

use crate::config::Config;
use crate::error::{Error, Result};

/// What a secret is replaced with.
pub const REDACTED: &str = "‹redacted›";

/// A quoted or bare value after a key.
const VALUE: &str = r#"(?:"[^"]*"|'[^']*'|[^\s'"&;|]+)"#;

/// One rule: matches of `re` are replaced by `replacement`, a template
/// that keeps the non-secret groups (e.g. the option name) around
/// [`REDACTED`].
#[derive(Debug, Clone)]
struct Rule {
    name: &'static str,
    re: Regex,
    replacement: String,
}

/// Keep group 1, redact the rest of the match.
const KEEP_PREFIX: &str = "${1}‹redacted›";
/// Redact the whole match.
const WHOLE: &str = "‹redacted›";

/// The built-in rules plus user patterns, compiled once per process.
#[derive(Debug, Clone)]
pub struct Redactor {
    rules: Vec<Rule>,
}

/// Names of the built-in rules, in the order they run (for tests and docs).
pub const BUILTIN: [&str; 16] = [
    "url-userinfo",
    "password-option",
    "secret-option",
    "token-assignment",
    "secret-assignment",
    "authorization-header",
    "secret-header",
    "aws-access-key",
    "github-token",
    "gitlab-token",
    "slack-token",
    "sk-key",
    "db-client-password",
    "curl-user",
    "sshpass-password",
    "registry-login-password",
];

impl Redactor {
    /// The built-in rules only.
    pub fn builtin() -> Self {
        let rule = |name, pattern: &str, replacement: &str| Rule {
            name,
            re: Regex::new(pattern).expect("built-in redaction pattern compiles"),
            replacement: replacement.to_string(),
        };
        Redactor {
            rules: vec![
                // scheme://user:pass@host → scheme://‹redacted›@host. With a
                // `:` in the userinfo, everything from `://` up to the last
                // `@` before white space or a quote, so a password may hold
                // `/ ? # : @`; without one (a bare token), up to the last
                // `@` before the path
                rule(
                    "url-userinfo",
                    r#"(?i)(\b[a-z][a-z0-9+.-]*://)(?:[^/\s'"@:]*:[^\s'"]*|[^/\s'"]+)(@)"#,
                    "${1}‹redacted›${2}",
                ),
                rule(
                    "password-option",
                    &format!(r"(?i)(--password(?:=|\s+))(?:{VALUE})"),
                    KEEP_PREFIX,
                ),
                // `--token X`, `--api-key=X`, `--with-token X`,
                // `--client-secret X`; not `--token-file X`
                rule(
                    "secret-option",
                    &format!(
                        r"(?i)(--(?:[a-z0-9]+-)*(?:token|api-?key|secret|secret-key|access-key|passphrase)(?:=|\s+))(?:{VALUE})"
                    ),
                    KEEP_PREFIX,
                ),
                rule(
                    "token-assignment",
                    &format!(r"(?i)(token=)(?:{VALUE})"),
                    KEEP_PREFIX,
                ),
                // `API_KEY=…`, `PGPASSWORD=…`, `MYSQL_PWD=…`, `?api_key=…`;
                // not the shell's own `PWD=`/`OLDPWD=` (`…TOKEN=` is the rule
                // above)
                rule(
                    "secret-assignment",
                    &format!(
                        r"(?i)(\b[a-z0-9_]*(?:key|secret|password|passwd|passphrase|_pwd|_pass|sshpass)=)(?:{VALUE})"
                    ),
                    KEEP_PREFIX,
                ),
                // the header value up to a closing quote or the end of the line
                rule(
                    "authorization-header",
                    r#"(?i)(authorization:\s*)[^'"\n]+"#,
                    KEEP_PREFIX,
                ),
                rule(
                    "secret-header",
                    r#"(?i)(\b(?:x-[a-z0-9-]*(?:key|token|secret|auth)[a-z0-9-]*|api-?key|private-token)\s*:\s*)[^'"\n]+"#,
                    KEEP_PREFIX,
                ),
                rule("aws-access-key", r"(?:AKIA|ASIA)[0-9A-Z]{16}", WHOLE),
                // classic (`ghp_`), OAuth, user, server, refresh and
                // fine-grained tokens
                rule(
                    "github-token",
                    r"gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,}",
                    WHOLE,
                ),
                rule("gitlab-token", r"glpat-[A-Za-z0-9_-]{20,}", WHOLE),
                rule("slack-token", r"xox[abposr]-[A-Za-z0-9-]{10,}", WHOLE),
                // `sk-…`, `sk-proj-…`, `sk_live_…`; at a word start, so a
                // name such as `task-…` is not cut
                rule("sk-key", r"\bsk[-_][A-Za-z0-9_-]{20,}", WHOLE),
                // `mysql … -p secret …`, `-psecret`: everything after -p
                rule(
                    "db-client-password",
                    r"(?m)(\b(?:mysql|psql|smbclient)\b[^\n]*?\s-p ?)\S[^\n]*",
                    KEEP_PREFIX,
                ),
                // `curl -u user:pass`, `-uuser:pass`, `--user user:pass`,
                // within one command of the line
                rule(
                    "curl-user",
                    &format!(r"(\bcurl\b[^\n;&|]*?\s(?:-u\s*|--user(?:=|\s+)))(?:{VALUE})"),
                    KEEP_PREFIX,
                ),
                rule(
                    "sshpass-password",
                    &format!(r"(\bsshpass\b[^\n;&|]*?\s-p\s*)(?:{VALUE})"),
                    KEEP_PREFIX,
                ),
                // `docker login -u me -p secret`, also podman, buildah,
                // nerdctl and `helm registry login`
                rule(
                    "registry-login-password",
                    &format!(
                        r"(\b(?:docker|podman|buildah|nerdctl|helm\s+registry)\s+login\b[^\n;&|]*?\s-p\s*)(?:{VALUE})"
                    ),
                    KEEP_PREFIX,
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
                replacement: WHOLE.to_string(),
            });
        }
        Ok(redactor)
    }

    /// The redactor of a logbook's `config.toml`: the built-in rules plus
    /// its `[redaction] patterns`. Every command that writes free text
    /// into the logbook runs that text through it before the first write,
    /// so the ledger and the Markdown files hold the same redacted text.
    pub fn for_config(config: &Config) -> Result<Self> {
        Redactor::with_patterns(&config.redaction.patterns)
    }

    /// `text` with every secret replaced by [`REDACTED`].
    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for rule in &self.rules {
            if !rule.re.is_match(&out) {
                continue;
            }
            let markers: Vec<(usize, usize)> = out
                .match_indices(REDACTED)
                .map(|(start, m)| (start, start + m.len()))
                .collect();
            out = rule
                .re
                .replace_all(&out, |caps: &Captures| {
                    let m = caps.get(0).expect("group 0 is the match");
                    let inside = markers
                        .iter()
                        .any(|&(start, end)| start <= m.start() && m.end() <= end);
                    if inside {
                        return m.as_str().to_string();
                    }
                    let mut replaced = String::new();
                    caps.expand(&rule.replacement, &mut replaced);
                    replaced
                })
                .into_owned();
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
