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
//! The `--token`-style options and the `…KEY=`-style assignments mask a
//! value only when it looks like a credential ([`looks_like_credential`]),
//! so `sort --key=2` or `hotkey=Super` stay as they are.
//!
//! Redacting twice gives the same text for the built-in rules: a match
//! that lies inside an existing [`REDACTED`] marker is left alone, so text
//! redacted by a command and again by the ledger reads the same in both
//! places.
//!
//! A built-in rule is compiled once per process, and only when a text
//! holds one of its literal triggers ([`triggers`]); a command line
//! without `://`, `=`, a token prefix, … compiles none of them.

use std::sync::{LazyLock, OnceLock};

use regex::{Captures, Regex};

use crate::config::Config;
use crate::error::{Error, Result};

/// What a secret is replaced with.
pub const REDACTED: &str = "‹redacted›";

/// A quoted or bare value after a key.
const VALUE: &str = r#"(?:"[^"]*"|'[^']*'|[^\s'"&;|]+)"#;

/// The shortest value that counts as a credential when it mixes at least
/// two character classes (lower case, upper case, digits, other).
pub const CREDENTIAL_MIN: usize = 8;
/// The shortest value that counts as a credential whatever its classes.
pub const CREDENTIAL_LONG: usize = 16;

/// One rule: matches of `pattern` are replaced by `replacement`, a
/// template that keeps the non-secret groups (e.g. the option name)
/// around [`REDACTED`]. With `check`, a match counts only when its group
/// `v` passes it. The regex is compiled on first use; a text whose ASCII
/// lower case holds none of `triggers` cannot match (empty: always try).
#[derive(Debug, Clone)]
struct Rule {
    name: &'static str,
    pattern: String,
    re: OnceLock<Regex>,
    replacement: String,
    check: Option<fn(&str) -> bool>,
    triggers: &'static [&'static str],
}

impl Rule {
    fn regex(&self) -> &Regex {
        self.re
            .get_or_init(|| Regex::new(&self.pattern).expect("built-in redaction pattern compiles"))
    }

    /// Whether `lower` (the text in ASCII lower case) may hold a match.
    fn triggered(&self, lower: &str) -> bool {
        self.triggers.is_empty() || self.triggers.iter().any(|t| lower.contains(t))
    }

    /// Whether this match is replaced: not inside an existing marker and,
    /// for a checked rule, with a value that passes the check.
    fn applies(&self, caps: &Captures, markers: &[(usize, usize)]) -> bool {
        let m = caps.get(0).expect("group 0 is the match");
        let inside = markers
            .iter()
            .any(|&(start, end)| start <= m.start() && m.end() <= end);
        !inside
            && self
                .check
                .is_none_or(|check| caps.name("v").is_some_and(|v| check(v.as_str())))
    }
}

/// Where [`REDACTED`] already stands in `text`.
fn markers(text: &str) -> Vec<(usize, usize)> {
    text.match_indices(REDACTED)
        .map(|(start, m)| (start, start + m.len()))
        .collect()
}

/// Whether a value after a key or option looks like a credential: without
/// its quotes, at least [`CREDENTIAL_LONG`] characters, or at least
/// [`CREDENTIAL_MIN`] that mix two of lower case, upper case, digits and
/// other characters.
pub fn looks_like_credential(value: &str) -> bool {
    let v = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value);
    let len = v.chars().count();
    let classes = [
        v.chars().any(|c| c.is_lowercase()),
        v.chars().any(|c| c.is_uppercase()),
        v.chars().any(|c| c.is_ascii_digit()),
        v.chars().any(|c| !c.is_alphanumeric()),
    ]
    .into_iter()
    .filter(|&has| has)
    .count();
    len >= CREDENTIAL_LONG || (len >= CREDENTIAL_MIN && classes >= 2)
}

/// Keep group 1, redact the rest of the match.
const KEEP_PREFIX: &str = "${1}‹redacted›";
/// Redact the whole match.
const WHOLE: &str = "‹redacted›";

/// The built-in rules plus user patterns.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    /// `config.toml [redaction] patterns`, after the built-in rules.
    user: Vec<Rule>,
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

/// The built-in rules, shared by every [`Redactor`] of the process; each
/// regex is compiled the first time a text triggers its rule.
static BUILTIN_RULES: LazyLock<Vec<Rule>> = LazyLock::new(builtin_rules);

/// Literal text, in ASCII lower case, that every match of the built-in
/// rule `name` contains (any one of them). A rule is tried only when the
/// text holds one; the replacement `‹redacted›` holds none of them, so
/// the text after an earlier rule needs no new check.
pub fn triggers(name: &str) -> &'static [&'static str] {
    match name {
        "url-userinfo" => &["://"],
        "password-option" => &["--password"],
        "secret-option" => &["token", "key", "secret", "passphrase"],
        "token-assignment" => &["token="],
        "secret-assignment" => &[
            "key=",
            "secret=",
            "password=",
            "passwd=",
            "passphrase=",
            "_pwd=",
            "_pass=",
            "sshpass=",
        ],
        "authorization-header" => &["authorization:"],
        "secret-header" => &["x-", "api-key", "apikey", "private-token"],
        "aws-access-key" => &["akia", "asia"],
        "github-token" => &["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"],
        "gitlab-token" => &["glpat-"],
        "slack-token" => &["xox"],
        "sk-key" => &["sk-", "sk_"],
        "db-client-password" => &["mysql", "psql", "smbclient"],
        "curl-user" => &["curl"],
        "sshpass-password" => &["sshpass"],
        "registry-login-password" => &["login"],
        _ => &[],
    }
}

fn rule(name: &'static str, pattern: &str, replacement: &str) -> Rule {
    Rule {
        name,
        pattern: pattern.to_string(),
        re: OnceLock::new(),
        replacement: replacement.to_string(),
        check: None,
        triggers: triggers(name),
    }
}

/// A rule whose group `v` must look like a credential.
fn credential_rule(name: &'static str, pattern: &str) -> Rule {
    Rule {
        check: Some(looks_like_credential),
        ..rule(name, pattern, KEEP_PREFIX)
    }
}

fn builtin_rules() -> Vec<Rule> {
    vec![
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
        credential_rule(
            "secret-option",
            &format!(
                r"(?i)(--(?:[a-z0-9]+-)*(?:token|api-?key|secret|secret-key|access-key|passphrase)(?:=|\s+))(?P<v>{VALUE})"
            ),
        ),
        rule(
            "token-assignment",
            &format!(r"(?i)(token=)(?:{VALUE})"),
            KEEP_PREFIX,
        ),
        // `API_KEY=…`, `PGPASSWORD=…`, `MYSQL_PWD=…`, `?api_key=…`;
        // not the shell's own `PWD=`/`OLDPWD=` (`…TOKEN=` is the rule
        // above)
        credential_rule(
            "secret-assignment",
            &format!(
                r"(?i)(\b[a-z0-9_]*(?:key|secret|password|passwd|passphrase|_pwd|_pass|sshpass)=)(?P<v>{VALUE})"
            ),
        ),
        // the header value up to a closing quote or the end of the line
        rule(
            "authorization-header",
            r#"(?i)(authorization:\s*)[^'"\n]+"#,
            KEEP_PREFIX,
        ),
        // header names that end in a credential word: `X-Api-Key`,
        // `X-Auth-Token`, `X-Auth`, `Api-Key`, `Private-Token`; not
        // `X-Author`
        rule(
            "secret-header",
            r#"(?i)(\b(?:x-(?:[a-z0-9]+-)*(?:api-?key|key|token|secret|auth)|api-?key|private-token)\s*:\s*)[^'"\n]+"#,
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
    ]
}

impl Redactor {
    /// The built-in rules only.
    pub fn builtin() -> Self {
        Redactor { user: Vec::new() }
    }

    fn rules(&self) -> impl Iterator<Item = &Rule> {
        BUILTIN_RULES.iter().chain(&self.user)
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
            redactor.user.push(Rule {
                name: "user-pattern",
                pattern: p.clone(),
                re: OnceLock::from(re),
                replacement: WHOLE.to_string(),
                check: None,
                triggers: &[],
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
        let lower = text.to_ascii_lowercase();
        for rule in self.rules() {
            if !rule.triggered(&lower) || !rule.regex().is_match(&out) {
                continue;
            }
            let markers = markers(&out);
            out = rule
                .regex()
                .replace_all(&out, |caps: &Captures| {
                    if !rule.applies(caps, &markers) {
                        return caps[0].to_string();
                    }
                    let mut replaced = String::new();
                    caps.expand(&rule.replacement, &mut replaced);
                    replaced
                })
                .into_owned();
        }
        out
    }

    /// Names of the rules that would replace something in `text`
    /// (diagnostics, tests, the import report).
    pub fn matching_rules(&self, text: &str) -> Vec<&'static str> {
        let markers = markers(text);
        let lower = text.to_ascii_lowercase();
        self.rules()
            .filter(|r| {
                r.triggered(&lower)
                    && r.regex()
                        .captures_iter(text)
                        .any(|c| r.applies(&c, &markers))
            })
            .map(|r| r.name)
            .collect()
    }
}
