//! Redaction of secrets before text is written (SPEC-ENGINE §7).
//!
//! Applied by the ledger to `subject`, `detail` and every string value of
//! `meta` of every event, and by the commands that write free text into
//! the logbook (`log`, `plan`, `decide`, `drift explain|dismiss`) before
//! anything is written, through [`Redactor::for_config`]. Built-in rules:
//! URLs with userinfo, `--password` (also wget's `--http-password`),
//! `--token`/`--api-key`/`--secret`/`--pass`/`--oauth2-bearer`-style
//! options, the `pass:…` value of openssl's `-pass`/`-passin`/`-passout`-style
//! options, `token=` and `…KEY=`/`…TOKEN=`/`…SECRET=`/`…PASSWORD=`-style
//! assignments, `Authorization:` and `X-…-Key:`-style headers, AWS access
//! keys, GitHub, GitLab and Slack tokens, `sk-`/`sk_` keys, anything after
//! `-p` for `mysql|psql|smbclient`, the credentials after `curl -u`, the
//! password after `sshpass -p` and `docker|podman … login -p`, proxy
//! credentials (`curl -U`, `--proxy-user`, `user:pass@` after `curl -x`,
//! `--proxy` or `…proxy=`), JSON values of `"…password"`, `"…secret"`,
//! `"…token"`-style keys, `Cookie:`/`Set-Cookie:` header values and the
//! cookies after `curl -b`/`--cookie`, a client certificate with its
//! password after `curl -E`/`--cert`, the value after `http|xh -a`/`--auth`,
//! the local part of an e-mail address (the domain stays); plus the
//! user's regexes from `config.toml [redaction] patterns`, each of which
//! replaces its whole match. The replacement is always [`REDACTED`].
//!
//! Every rule errs towards redacting too much: a value may be quoted, a
//! key may be `GITHUB_TOKEN=`, and `sk-proj-…` keys contain hyphens.
//! A name that can only mean a credential (`--token`, `--secret`,
//! `PASSWORD=`, `…_PWD=`, …) masks any non-empty value. A name that ends
//! in `key` (`--api-key`, `…KEY=`) masks a value only when it looks like
//! a credential ([`looks_like_credential`]), so `sort --key=2` or
//! `hotkey=Super` stay as they are.
//!
//! Redacting twice gives the same text for the built-in rules: a match
//! that lies inside an existing [`REDACTED`] marker is left alone, so text
//! redacted by a command and again by the ledger reads the same in both
//! places.
//!
//! Word boundaries are ASCII (`(?-u:\b)`): with a Unicode `\b` a regex
//! leaves its fast matcher on any non-ASCII text, the marker of an
//! earlier rule included, and a long line took milliseconds (WP-084). The
//! `…=` assignment rules need no boundary at all, since a match starts at
//! the first character of the name anyway, also at a `ſ` or `K` that
//! case-insensitive matching folds.
//!
//! A built-in rule is compiled once per process, and only when a text
//! holds one of its literal triggers ([`triggers`]); a command line
//! without `://`, `=`, a token prefix, … compiles none of them, and a
//! `curl` line compiles a `curl` rule only when it also holds that rule's
//! option (`-u`, `-x`, `-b`, …).
//!
//! The rules for an option of a command (`curl -u`, `sshpass -p`,
//! `docker login -p`) look for it within one command ([`COMMAND_REST`]:
//! a quoted `;`, `&` or `|`, a redirection such as `2>&1` and a line end
//! after `\` or inside quotes do not end it) and find it again when the
//! command gives it twice ([`Rule::matches`]). The value of an option is
//! one shell word ([`WORD`]), so `-u admin:'p w'` is masked whole.

use std::sync::{LazyLock, OnceLock};

use regex::{Captures, Regex};

use crate::config::Config;
use crate::error::{Error, Result};

/// What a secret is replaced with.
pub const REDACTED: &str = "‹redacted›";

/// A quoted or bare value after a key (`token=`, `PASSWORD=`); a double
/// quoted value may hold `\"`. A bare value does not start at a
/// [`REDACTED`] marker: after `TOKEN="a"bob@example.com` is masked to
/// `TOKEN=‹redacted›‹redacted›@example.com`, a second pass must not take
/// the glued markers for a new value.
const VALUE: &str = r#"(?:"(?:[^"\\]|\\.)*"|"[^"]*"|'[^']*'|[^\s'"&;|‹][^\s'"&;|]*)"#;

/// The value after an option of a command (`--password`, `curl -u`): one
/// shell word, which may join quoted and bare parts (`admin:'p w'`,
/// `"$U":pw`) and hold `$'…'`, `\"` inside double quotes and backslash
/// escapes (`\;`, `\` before a line end). A quoted part ends at a line end
/// that no `\` escapes: a double-quoted part that never closes as escapes
/// are read is taken up to the next `"` on its line as written, and a
/// quote that the line does not close (`bob's` in a note, `'admin:pw`)
/// takes the rest of the line, so a later line stays.
const WORD: &str = r#"(?:(?:"(?:[^"\\\n]|\\(?s:.))*"|'[^'\n]*'|\$'(?:[^'\\\n]|\\(?s:.))*'|\\(?s:.)|[^\s'"\\&;|]|"[^"\n]*")+(?:['"][^\n]*)?|['"][^\n]*)"#;

/// The value of an openssl pass phrase option that gives the secret
/// itself: a [`WORD`] whose first part starts with `pass:`, bare or inside
/// `'…'`, `"…"` or `$'…'` (`pass:p`, `'pass:p w'`, `pass:'p w'`). The
/// sources `env:`, `file:`, `fd:` and `stdin` name where the secret is
/// and are no match; nor is a flag (`-twopass`) before the option, since
/// the value must hold `pass:` (a check on a [`WORD`] would take the
/// next option as the flag's value and miss its `pass:`).
const PASS_ARG: &str = r#"(?:(?:"pass:(?:[^"\\\n]|\\(?s:.))*"|'pass:[^'\n]*'|\$'pass:(?:[^'\\\n]|\\(?s:.))*'|pass:|"pass:[^"\n]*")(?:"(?:[^"\\\n]|\\(?s:.))*"|'[^'\n]*'|\$'(?:[^'\\\n]|\\(?s:.))*'|\\(?s:.)|[^\s'"\\&;|]|"[^"\n]*")*(?:['"][^\n]*)?|['"]pass:[^\n]*)"#;

/// White space between an option and its value, or a line continuation
/// (`\` before a line end).
const GAP: &str = r"(?:\s|\\\n)";

/// The rest of one command after its command word (`curl`, `sshpass`,
/// `docker login`), up to an option: anything but a line end or an
/// unquoted `;`, `&` or `|`. A quoted string (`'a&b'`, `"x;y"`, with `\"`
/// inside double quotes, also over several lines), an ANSI-C string
/// (`$'a;b\''`), a backslash escape outside quotes (`\;`, and `\` before
/// a line end, which continues the command on the next line) and a
/// redirection (`2>&1`, `&>file`, `>|file`) belong to the command. A
/// quote that the text never closes (`curl's -u …` in a note) is an
/// ordinary character, after which no quote, separator or line end may
/// follow; the quoted strings before it pair up as written, so an
/// unquoted `;` between two of them still ends the command.
///
/// Quotes pair left to right, which is not always the shell's reading
/// (quotes inside `"$(…)"`, an escaped space `\ `). The fragment is
/// therefore the union with the plain form, any characters but a line
/// end, `;`, `&` or `|`: a match needs only one of the two readings, so
/// every command the plain form reaches is still reached.
const COMMAND_REST: &str = r#"(?:[^\n;&|]*?|(?:\$'(?:[^'\\]|\\(?s:.))*'|[^\n;&|'"\\]|\\(?s:.)|[<>]&|&>|>\||'[^']*'|"(?:[^"\\]|\\(?s:.))*")*?(?:['"][^\n;&|'"]*?)?)"#;

/// The shortest value that counts as a credential when it mixes at least
/// two character classes (lower case, upper case, digits, other).
pub const CREDENTIAL_MIN: usize = 8;
/// The shortest value that counts as a credential whatever its classes.
pub const CREDENTIAL_LONG: usize = 16;

/// One rule: matches of `pattern` are replaced by `replacement`, a
/// template that keeps the non-secret groups (e.g. the option name)
/// around [`REDACTED`]. With `check`, a match counts only when its group
/// `v` passes it. With `next`, the rule is an option of a command and
/// finds the option again in the same command ([`Rule::matches`]). The
/// regexes are compiled on first use; a text whose ASCII lower case holds
/// none of `triggers` cannot match (empty: always try; see
/// [`holds_trigger`] for `+`). A match in which one of the groups in
/// `unless` takes part is left as it is: such a group stands for context
/// that the `regex` crate cannot look behind or ahead for, so the pattern
/// matches it and the rule then keeps the match. A group in
/// `unless_followed` does so only for what follows the match
/// ([`Rule::kept`]).
#[derive(Debug, Clone)]
struct Rule {
    name: &'static str,
    pattern: String,
    re: OnceLock<Regex>,
    next: Option<String>,
    next_re: OnceLock<Regex>,
    replacement: String,
    check: Option<fn(&str) -> bool>,
    unless: &'static [&'static str],
    unless_followed: &'static [&'static str],
    triggers: &'static [&'static str],
}

/// One match of a rule: its groups, whose positions count from `offset`
/// in the text (a match of [`Rule::next`] is searched for in the rest of
/// the text after the previous match).
struct Found<'t> {
    offset: usize,
    caps: Captures<'t>,
}

impl Found<'_> {
    /// Where the match stands in the whole text.
    fn range(&self) -> (usize, usize) {
        let m = self.caps.get(0).expect("group 0 is the match");
        (self.offset + m.start(), self.offset + m.end())
    }
}

impl Rule {
    fn regex(&self) -> &Regex {
        self.re
            .get_or_init(|| Regex::new(&self.pattern).expect("built-in redaction pattern compiles"))
    }

    fn next_regex(&self) -> Option<&Regex> {
        self.next.as_ref().map(|next| {
            self.next_re
                .get_or_init(|| Regex::new(next).expect("built-in redaction pattern compiles"))
        })
    }

    /// The matches of this rule in `text`, left to right.
    ///
    /// A rule for an option of a command (`curl -u`) scans on from each
    /// match that starts at the command word (group `cmd`): its `next`
    /// pattern is anchored at the end of the previous match, as `\G`
    /// would anchor it in other regex dialects (the `regex` crate has
    /// none), and matches the rest of the same command up to the option
    /// once more. It repeats until the command holds no further option,
    /// so `curl -u a:b … -u c:d` yields both without a second command
    /// word, and the scan never leaves the command. The search for the
    /// next command word goes on after the last of them.
    fn matches<'t>(&self, text: &'t str) -> Vec<Found<'t>> {
        let Some(next) = self.next_regex() else {
            return self
                .regex()
                .captures_iter(text)
                .map(|caps| Found { offset: 0, caps })
                .collect();
        };
        let mut found = Vec::new();
        let mut pos = 0;
        while let Some(caps) = self.regex().captures_at(text, pos) {
            let first = Found { offset: 0, caps };
            let (start, mut end) = first.range();
            let in_command = first.caps.name("cmd").is_some();
            found.push(first);
            while in_command && let Some(caps) = next.captures(&text[end..]) {
                let again = Found { offset: end, caps };
                end = again.range().1;
                found.push(again);
            }
            // the option and its value are never empty
            debug_assert!(end > start, "{}: empty match", self.name);
            pos = end;
        }
        found
    }

    /// Whether `lower` (the text through [`trigger_text`]) may hold a
    /// match.
    fn triggered(&self, lower: &str) -> bool {
        self.triggers.is_empty() || self.triggers.iter().any(|t| holds_trigger(lower, t))
    }

    /// Whether a group of `unless` keeps this match as it is. A group of
    /// `unless_followed` (the `:` after an address) keeps it only when the
    /// text goes on right after the match with a character other than
    /// white space that starts no further match (`next`): `host:path`
    /// does, `a@b.co: hi` and `a@b.co:c@d.example` do not. The group
    /// holds only that `:`, so the next match may start right after it.
    fn kept(&self, found: &Found, text: &str, next: Option<&Found>) -> bool {
        let named = |g: &&str| found.caps.name(g).is_some();
        if self.unless.iter().any(named) {
            return true;
        }
        let end = found.range().1;
        self.unless_followed.iter().any(named)
            && text[end..]
                .chars()
                .next()
                .is_some_and(|c| !c.is_whitespace())
            && next.is_none_or(|n| n.range().0 != end)
    }

    /// Whether this match is replaced: not inside an existing marker, not
    /// kept by its context ([`Rule::kept`]; `next` is the rule's following
    /// match in `text`) and, for a checked rule, with a value that passes
    /// the check.
    fn applies(
        &self,
        found: &Found,
        next: Option<&Found>,
        text: &str,
        markers: &[(usize, usize)],
    ) -> bool {
        let (m_start, m_end) = found.range();
        // only the last marker that starts at or before the match can
        // hold it (see [`markers`]): a binary search, so a long line with
        // many masked values stays linear
        let i = markers.partition_point(|&(start, _)| start <= m_start);
        let inside = i > 0 && m_end <= markers[i - 1].1;
        !inside
            && !self.kept(found, text, next)
            && self
                .check
                .is_none_or(|check| found.caps.name("v").is_some_and(|v| check(v.as_str())))
    }

    /// `text` with every match this rule applies to replaced.
    fn replace(&self, text: &str) -> String {
        let markers = markers(text);
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        let all = self.matches(text);
        for (i, found) in all.iter().enumerate() {
            let (start, end) = found.range();
            out.push_str(&text[last..start]);
            if self.applies(found, all.get(i + 1), text, &markers) {
                found.caps.expand(&self.replacement, &mut out);
            } else {
                out.push_str(&text[start..end]);
            }
            last = end;
        }
        out.push_str(&text[last..]);
        out
    }
}

/// Where [`REDACTED`] already stands in `text`: ascending and without
/// overlap, as `match_indices` finds them ([`Rule::applies`] relies on it).
fn markers(text: &str) -> Vec<(usize, usize)> {
    let markers: Vec<_> = text
        .match_indices(REDACTED)
        .map(|(start, m)| (start, start + m.len()))
        .collect();
    debug_assert!(markers.windows(2).all(|w| w[0].1 <= w[1].0));
    markers
}

/// `value` without its surrounding quotes.
fn unquoted(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value)
}

/// Whether a value after a credential name is not empty (`PASSWORD=""`
/// names no secret).
pub fn has_value(value: &str) -> bool {
    !unquoted(value).is_empty()
}

/// Whether a JSON string value (`"…"`, or `\"…\"` inside a shell string)
/// is not empty.
fn has_json_value(value: &str) -> bool {
    let inner = value
        .strip_prefix("\\\"")
        .and_then(|v| v.strip_suffix("\\\""))
        .or_else(|| value.strip_prefix('"').and_then(|v| v.strip_suffix('"')))
        .unwrap_or(value);
    !inner.is_empty()
}

/// `text` as the triggers see it: ASCII letters in lower case, plus the
/// two other characters that case-insensitive matching folds onto an
/// ASCII letter, the Kelvin sign (U+212A → `k`) and the long s
/// (U+017F → `s`). Other characters stay as they are.
pub fn trigger_text(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{212A}' => 'k',
            '\u{017F}' => 's',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

/// Whether a value after a key or option looks like a credential: without
/// its quotes, at least [`CREDENTIAL_LONG`] characters, or at least
/// [`CREDENTIAL_MIN`] that mix two of lower case, upper case, digits and
/// other characters.
pub fn looks_like_credential(value: &str) -> bool {
    let v = unquoted(value);
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
pub const BUILTIN: [&str; 27] = [
    "url-userinfo",
    "password-option",
    "secret-option",
    "openssl-pass",
    "key-option",
    "token-assignment",
    "secret-assignment",
    "key-assignment",
    "json-secret",
    "authorization-header",
    "secret-header",
    "cookie-header",
    "aws-access-key",
    "github-token",
    "gitlab-token",
    "slack-token",
    "sk-key",
    "db-client-password",
    "curl-user",
    "proxy-option",
    "proxy-userinfo",
    "cookie-option",
    "cert-password",
    "httpie-auth",
    "sshpass-password",
    "registry-login-password",
    "email",
];

/// The built-in rules, shared by every [`Redactor`] of the process; each
/// regex is compiled the first time a text triggers its rule.
static BUILTIN_RULES: LazyLock<Vec<Rule>> = LazyLock::new(builtin_rules);

/// Whether `lower` (the text through [`trigger_text`]) holds `trigger`:
/// every one of its literals, which a `+` joins (`curl+-x`: both `curl`
/// and `-x`, anywhere in the text).
pub fn holds_trigger(lower: &str, trigger: &str) -> bool {
    trigger.split('+').all(|part| lower.contains(part))
}

/// Literal text, in lower case, that every match of the built-in rule
/// `name` contains (any one of them, see [`holds_trigger`]), as
/// [`trigger_text`] spells it. A rule is tried only when the text holds
/// one; the replacement `‹redacted›` holds no literal of them, so the
/// text after an earlier rule needs no new check.
pub fn triggers(name: &str) -> &'static [&'static str] {
    match name {
        "url-userinfo" => &["://"],
        "password-option" => &["--password", "--http-password", "--ftp-password"],
        "secret-option" => &["token", "secret", "passphrase", "-pass", "bearer"],
        "openssl-pass" => &["pass:"],
        "key-option" => &["key"],
        "token-assignment" => &["token="],
        "key-assignment" => &["key="],
        "secret-assignment" => &[
            "secret=",
            "password=",
            "passwd=",
            "passphrase=",
            "_pwd=",
            "_pass=",
            "sshpass=",
        ],
        "json-secret" => &[
            "password\"",
            "password\\\"",
            "passwd\"",
            "passwd\\\"",
            "passphrase\"",
            "passphrase\\\"",
            "secret\"",
            "secret\\\"",
            "token\"",
            "token\\\"",
            "api_key\"",
            "api_key\\\"",
            "apikey\"",
            "apikey\\\"",
        ],
        "authorization-header" => &["authorization:"],
        "secret-header" => &["x-", "api-key", "apikey", "private-token"],
        "cookie-header" => &["cookie"],
        "aws-access-key" => &["akia", "asia"],
        "github-token" => &["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"],
        "gitlab-token" => &["glpat-"],
        "slack-token" => &["xox"],
        "sk-key" => &["sk-", "sk_"],
        "db-client-password" => &["mysql", "psql", "smbclient"],
        // `-U` and `--user` both read `-u` here
        "curl-user" => &["curl+-u"],
        "proxy-option" => &["curl+-u", "--proxy-"],
        "proxy-userinfo" => &["curl+-x", "proxy"],
        "cookie-option" => &["curl+-b", "curl+--cookie"],
        // `-E` reads `-e` here
        "cert-password" => &["curl+-e", "--cert", "--proxy-cert"],
        // the command word and the white space or `\` after it, as the
        // rule requires them (ASCII, so a match always holds one)
        "httpie-auth" => &[
            "http +-a",
            "http\t+-a",
            "http\n+-a",
            "http\\+-a",
            "https +-a",
            "https\t+-a",
            "https\n+-a",
            "https\\+-a",
            "xh +-a",
            "xh\t+-a",
            "xh\n+-a",
            "xh\\+-a",
            "xhs +-a",
            "xhs\t+-a",
            "xhs\n+-a",
            "xhs\\+-a",
        ],
        "sshpass-password" => &["sshpass"],
        "registry-login-password" => &["login"],
        "email" => &["@"],
        _ => &[],
    }
}

fn rule(name: &'static str, pattern: &str, replacement: &str) -> Rule {
    Rule {
        name,
        pattern: pattern.to_string(),
        re: OnceLock::new(),
        next: None,
        next_re: OnceLock::new(),
        replacement: replacement.to_string(),
        check: None,
        unless: &[],
        unless_followed: &[],
        triggers: triggers(name),
    }
}

/// A rule whose group `v` must pass `check`.
fn checked_rule(name: &'static str, pattern: &str, check: fn(&str) -> bool) -> Rule {
    Rule {
        check: Some(check),
        ..rule(name, pattern, KEEP_PREFIX)
    }
}

/// The command word `word` as group `cmd`, then the rest of its command
/// up to an option ([`COMMAND_REST`]).
fn command(word: &str) -> String {
    format!("(?P<cmd>{word}){COMMAND_REST}")
}

/// A rule for an option of a command (`curl -u`, `docker login -p`):
/// `command` is the command word and the text up to the option
/// ([`command`]), `option` the option with the white space before it,
/// `value` what follows the option; group 1 holds everything before the
/// value. `plain`, if not empty, is an option that counts without the
/// command word (`--proxy-user`, also wget's); the command's `option`
/// lists it too, or the context would scan past it. The same option later
/// in the same command is found by `next` ([`Rule::matches`]).
fn option_rule(
    name: &'static str,
    command: &str,
    option: &str,
    plain: &str,
    value: &str,
    replacement: &str,
) -> Rule {
    let mut first = format!("{command}{option}");
    if !plain.is_empty() {
        first = format!("{first}|{plain}");
    }
    Rule {
        next: Some(format!(r"\A({COMMAND_REST}{option}){value}")),
        ..rule(name, &format!("({first}){value}"), replacement)
    }
}

/// The first word of a hyphenated option name, unless it is `no`
/// (`--no-pass` is a flag, not a credential).
const NOT_NO: &str = r"(?:[a-mo-z0-9][a-z0-9]*|n(?:[a-np-z0-9][a-z0-9]*)?|no[a-z0-9]+)";

/// The white space after HTTPie's command word: the characters its
/// triggers name ([`triggers`]), not every `\s`.
const HTTPIE_GAP: &str = r"(?:[ \t\n]|\\\n)";

/// The command word `curl`.
const CURL: &str = r"(?-u:\b)curl(?-u:\b)";

fn builtin_rules() -> Vec<Rule> {
    let rules = vec![
        // scheme://user:pass@host → scheme://‹redacted›@host. With a
        // `:` in the userinfo, everything from `://` up to the last
        // `@` before white space or a quote, so a password may hold
        // `/ ? # : @`; without one (a bare token), up to the last
        // `@` before the path
        rule(
            "url-userinfo",
            r#"(?i)((?-u:\b)[a-z][a-z0-9+.-]*://)(?:[^/\s'"@:]*:[^\s'"]*|[^/\s'"]+)(@)"#,
            "${1}‹redacted›${2}",
        ),
        // `--password X`, wget's `--http-password X` and `--ftp-password X`
        rule(
            "password-option",
            &format!(r"(?i)(--(?:(?:http|ftp)-)?password(?:=|{GAP}+))(?:{WORD})"),
            KEEP_PREFIX,
        ),
        // `--token X`, `--with-token X`, `--secret X`, `--client-secret X`,
        // `--passphrase X`, curl's `--pass X` and `--oauth2-bearer X`, also
        // `--proxy-pass X`: any value; not `--token-file X`, and not a
        // negation such as smbclient's `--no-pass`
        checked_rule(
            "secret-option",
            &format!(
                r"(?i)(--(?:{NOT_NO}-(?:[a-z0-9]+-)*)?(?:token|secret|passphrase|pass|bearer)(?:=|{GAP}+))(?P<v>{WORD})"
            ),
            has_value,
        ),
        // openssl's `-pass pass:X`, `-passin`, `-passout`, `-password`,
        // `-passcerts`, `-keypass`, `-srv_secret`, … (any option whose
        // name holds `pass` or `secret`, also `--passin=pass:X` as
        // easyrsa writes it): the whole value, `pass:` included
        rule(
            "openssl-pass",
            &format!(r"(?i)(-[a-z0-9_-]*(?:pass|secret)[a-z0-9_-]*(?:=|{GAP}+))(?:{PASS_ARG})"),
            KEEP_PREFIX,
        ),
        // `--api-key=X`, `--access-key X`, `--secret-key X`: only a value
        // that looks like a credential
        checked_rule(
            "key-option",
            &format!(
                r"(?i)(--(?:[a-z0-9]+-)*(?:api-?key|access-key|secret-key)(?:=|{GAP}+))(?P<v>{WORD})"
            ),
            looks_like_credential,
        ),
        rule(
            "token-assignment",
            &format!(r"(?i)(token=)(?:{VALUE})"),
            KEEP_PREFIX,
        ),
        // `PASSWORD=…`, `PGPASSWORD=…`, `MYSQL_PWD=…`, `DB_PASS=…`,
        // `SECRET=…`: any value; not the shell's own `PWD=`/`OLDPWD=`
        // (`…TOKEN=` is the rule above)
        checked_rule(
            "secret-assignment",
            &format!(
                r"(?i)([a-z0-9_]*(?:secret|password|passwd|passphrase|_pwd|_pass|sshpass)=)(?P<v>{VALUE})"
            ),
            has_value,
        ),
        // `API_KEY=…`, `?api_key=…`: only a value that looks like a
        // credential, so `hotkey=Super` and `key=value` stay
        checked_rule(
            "key-assignment",
            &format!(r"(?i)([a-z0-9_]*key=)(?P<v>{VALUE})"),
            looks_like_credential,
        ),
        // `"password": "…"`, `"client_secret":"…"`, `"access_token"`,
        // `"api_key"`, `"apiKey"`, also with the quotes escaped inside a
        // shell string (`\"password\":\"…\"`) and with white space,
        // newlines included, around the `:`: a non-empty string value;
        // not `"password_hint"` or `"token_type"`
        checked_rule(
            "json-secret",
            r#"(?i)(\\?"[a-z0-9_-]*(?:password|passwd|passphrase|secret|token|api_?key)\\?"\s*:\s*)(?P<v>"(?:[^"\\\n]|\\.)*"|\\"[^"\n]*?\\")"#,
            has_json_value,
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
            r#"(?i)((?-u:\b)(?:x-(?:[a-z0-9]+-)*(?:api-?key|key|token|secret|auth)|api-?key|private-token)\s*:\s*)[^'"\n]+"#,
            KEEP_PREFIX,
        ),
        // `Cookie: a=b; c=d`, `Set-Cookie: …`: a value that starts with a
        // cookie pair `name=` (RFC 6265), up to a closing quote or the end
        // of the line, on the same line; not `cookie: banner fixed`,
        // `Cookie: $COOKIE` or an empty value
        rule(
            "cookie-header",
            r#"(?i)((?-u:\b)(?:set-)?cookie[ \t]*:[ \t]*)[^'"\s=;]+=[^'"\n]*"#,
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
        rule("sk-key", r"(?-u:\b)sk[-_][A-Za-z0-9_-]{20,}", WHOLE),
        // `mysql … -p secret …`, `-psecret`: everything after -p, up to
        // the end of the command's last continued line (`\` before a line
        // end continues it)
        rule(
            "db-client-password",
            r"(?m)((?-u:\b)(?:mysql|psql|smbclient)(?-u:\b)(?:\\\n|[^\n])*?\s-p ?)\S(?:\\\n|[^\n])*",
            KEEP_PREFIX,
        ),
        // `curl -u user:pass`, `-uuser:pass`, `--user user:pass`,
        // within one command, every time it is given
        option_rule(
            "curl-user",
            &command(CURL),
            &format!(r"\s(?:-u{GAP}*|--user(?:=|{GAP}+))"),
            "",
            &format!("(?:{WORD})"),
            KEEP_PREFIX,
        ),
        // `curl -U user:pass` (not `useradd -U`), `--proxy-user user:pass`
        // (curl, wget), wget's `--proxy-password pass` (the `=` form is
        // `secret-assignment`)
        option_rule(
            "proxy-option",
            &command(CURL),
            &format!(r"\s(?:-U{GAP}*|(?i:--proxy-user(?:=|{GAP}+)|--proxy-password{GAP}+))"),
            &format!(r"(?i:--proxy-user(?:=|{GAP}+)|--proxy-password{GAP}+)"),
            &format!("(?:{WORD})"),
            KEEP_PREFIX,
        ),
        // `user:pass@host` without a scheme after `curl -x`, `--proxy`,
        // `https_proxy=`, `http.proxy=`, up to the last `@` as for a URL;
        // a value with `scheme://` is `url-userinfo`. The match goes on to
        // the end of the value (`host`, with a closing quote), so the scan
        // for a further `-x` starts outside the quotes
        option_rule(
            "proxy-userinfo",
            &command(CURL),
            &format!(r#"\s(?:-x{GAP}*|(?i:--proxy(?:=|{GAP}+)))['"]?"#),
            &format!(r#"(?i:--proxy(?:=|{GAP}+)|[a-z_.]*proxy=)['"]?"#),
            r#"[^\s'"@/:]+:(?:[^/\s'"]|/[^/\s'"])[^\s'"]*@(?P<host>[^\s'"]*['"]?)"#,
            "${1}‹redacted›@${host}",
        ),
        // `curl -b 'session=…'`, `--cookie "a=b; c=d"`: a value with `=`
        // (without one, curl reads cookies from that file)
        Rule {
            check: Some(|v| unquoted(v).contains('=')),
            ..option_rule(
                "cookie-option",
                &command(CURL),
                &format!(r"\s(?:-b{GAP}*|--cookie(?:=|{GAP}+))"),
                "",
                &format!("(?P<v>{WORD})"),
                KEEP_PREFIX,
            )
        },
        // `curl -E cert.pem:pass`, `--cert`, `--proxy-cert` (also without
        // the command word): a value with `:`, the certificate's file name
        // included (without one, there is no password in it)
        Rule {
            check: Some(|v| v.contains(':')),
            ..option_rule(
                "cert-password",
                &command(CURL),
                &format!(r"\s(?:-E{GAP}*|--(?:proxy-)?cert(?:=|{GAP}+))"),
                &format!(r"--(?:proxy-)?cert(?:=|{GAP}+)"),
                &format!("(?P<v>{WORD})"),
                KEEP_PREFIX,
            )
        },
        // `http -a user:pass`, `--auth`, also `https`, `xh` and `xhs`
        // (HTTPie and xh): any value, a bearer token included. The command
        // word is followed by a space, tab, line end or `\`+line end
        // (ASCII, as its triggers spell it), so `http://` is none
        option_rule(
            "httpie-auth",
            &format!(r"(?P<cmd>(?-u:\b)(?:https?|xhs?))(?:{HTTPIE_GAP}{COMMAND_REST})??"),
            &format!(r"{HTTPIE_GAP}(?:-a{GAP}*|--auth(?:=|{GAP}+))"),
            "",
            &format!("(?:{WORD})"),
            KEEP_PREFIX,
        ),
        // only the first `-p`: one after the command that sshpass runs is
        // that command's (`ssh -p 2222`)
        rule(
            "sshpass-password",
            &format!(r"((?-u:\b)sshpass(?-u:\b){COMMAND_REST}\s-p{GAP}*)(?:{WORD})"),
            KEEP_PREFIX,
        ),
        // `docker login -u me -p secret`, also podman, buildah,
        // nerdctl and `helm registry login`
        option_rule(
            "registry-login-password",
            &command(r"(?-u:\b)(?:docker|podman|buildah|nerdctl|helm\s+registry)\s+login(?-u:\b)"),
            &format!(r"\s-p{GAP}*"),
            "",
            &format!("(?:{WORD})"),
            KEEP_PREFIX,
        ),
        // `me@example.com` → `‹redacted›@example.com`: the domain stays,
        // so a file named after an account can still be found. Not an
        // address: `user@host` without a dot, a version (`pkg@1.2.3`),
        // an npm scope (`@scope/pkg`), and, through `unless`, the
        // userinfo of a URL (`url-userinfo`'s) and a systemd unit
        // (`getty@tty1.service`), through `unless_followed` `host:path`
        // or a port after the domain (`git@github.com:owner/repo`)
        Rule {
            unless: &["url", "unit"],
            unless_followed: &["port"],
            ..rule("email", &email(), "‹redacted›@${domain}${port}")
        },
    ];
    debug_assert!(
        rules.iter().map(|r| r.name).eq(BUILTIN),
        "BUILTIN lists the rules"
    );
    rules
}

/// One character of a domain beyond ASCII (`müller.example`,
/// `.испытание`): any. The domain is kept as it stands, so text glued to
/// it stays too.
const NON_ASCII: &str = r"\x{80}-\x{10FFFF}";

/// One character of a local part beyond ASCII (`jürgen@…`): not the
/// control characters and the no-break space (U+0080–U+00A0), the general
/// punctuation (U+2000–U+206F: `—`, `„`, the quotes of [`REDACTED`]), CJK
/// (U+3000–U+9FFF, U+F900–U+FAFF) or the full-width forms
/// (U+FF00–U+FFEF), so text glued to an address (`連絡先：山田さんme@…`,
/// `Kontakt—me@…`) stays. A local part in CJK is not matched. Ranges
/// rather than `\p{L}`, which compiles at twice the cost.
const LOCAL_NON_ASCII: &str =
    r"\x{A1}-\x{1FFF}\x{2070}-\x{2FFF}\x{A000}-\x{F8FF}\x{FB00}-\x{FEFF}\x{10000}-\x{10FFFF}";

/// An e-mail address: a local part, `@`, and a domain of at least two
/// labels whose last one (the top-level domain) holds letters only. The
/// groups that rule `email` keeps as they are come first and last: `url`
/// is the start of a URL with userinfo as `url-userinfo` reads it (with a
/// `:`, a password may hold `/`), so an `@` that rule masks up to is no
/// address; `unit` and `port` (only the `:`) follow the domain.
fn email() -> String {
    format!(
        r#"(?P<url>(?-u:\b)[A-Za-z][A-Za-z0-9+.-]*://(?:[^/\s'"@:]*:[^\s'"]*|[^/\s'"]*))?[A-Za-z0-9._%+\-{LOCAL_NON_ASCII}]+@(?P<domain>(?:[A-Za-z0-9\-{NON_ASCII}]+\.)+(?:(?P<unit>service|socket|target|timer|mount|automount|path|slice|scope|swap|device)(?-u:\b)|[A-Za-z{NON_ASCII}]{{2,}}))(?P<port>:)?"#
    )
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
                next: None,
                next_re: OnceLock::new(),
                replacement: WHOLE.to_string(),
                check: None,
                unless: &[],
                unless_followed: &[],
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
        let lower = trigger_text(text);
        for rule in self.rules() {
            if !rule.triggered(&lower) || !rule.regex().is_match(&out) {
                continue;
            }
            out = rule.replace(&out);
        }
        out
    }

    /// Names of the rules that would replace something in `text`
    /// (diagnostics, tests, the import report).
    pub fn matching_rules(&self, text: &str) -> Vec<&'static str> {
        let markers = markers(text);
        let lower = trigger_text(text);
        self.rules()
            .filter(|r| {
                if !r.triggered(&lower) {
                    return false;
                }
                let all = r.matches(text);
                (0..all.len()).any(|i| r.applies(&all[i], all.get(i + 1), text, &markers))
            })
            .map(|r| r.name)
            .collect()
    }
}
