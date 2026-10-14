//! The drift rules the index applies at build time: the proposal token rule
//! (ADR-0015 §4), the `[drift] alwaysRed` globs and the routine class of a
//! pacman group member (ADR-0013 §3).

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use regex::Regex;

use crate::model::event::{Event, Kind, Source};
use crate::pkgcmd::{parse_command, split_logged};

/// Word characters of the token rule: `[A-Za-z0-9._+-]`.
fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-')
}

/// Whether `subject` occurs in `text` as a whole word (ADR-0015 §4):
/// case-sensitive, no word character before it, and after it either no
/// word character or a single `.` that is followed by no word character
/// (`Install zed.` names `zed`, `zed.conf` does not, `extra/zed` does).
pub fn names_token(text: &str, subject: &str) -> bool {
    if subject.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(i) = text[from..].find(subject) {
        let start = from + i;
        let end = start + subject.len();
        let before_ok = !text[..start].chars().next_back().is_some_and(is_word);
        let mut after = text[end..].chars();
        let after_ok = match after.next() {
            None => true,
            Some('.') => !after.next().is_some_and(is_word),
            Some(c) => !is_word(c),
        };
        if before_ok && after_ok {
            return true;
        }
        // the next candidate starts after the first character of this one
        from = start + text[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Omarchy's channel packages (WP-224): the stable channel's name and its
/// edge twin. Each pair is one package to `alwaysRed`, the stable name
/// standing for both.
pub const OMARCHY_CHANNELS: [(&str, &str); 2] = [
    ("omarchy", "omarchy-dev"),
    ("omarchy-settings", "omarchy-settings-dev"),
];

/// The name that stands for `subject` in `alwaysRed` (WP-224): an Omarchy
/// channel package's stable name ([`OMARCHY_CHANNELS`]), `linux-omarchy`
/// for any of Omarchy's kernels (`linux-omarchy-bore`; not their
/// `-headers` or `-docs`), else `subject` itself.
pub fn family(subject: &str) -> &str {
    if let Some(&(stable, _)) = OMARCHY_CHANNELS.iter().find(|(_, edge)| *edge == subject) {
        return stable;
    }
    let kernel = subject
        .strip_prefix("linux-omarchy-")
        .is_some_and(|variant| !variant.is_empty())
        && !subject.ends_with("-headers")
        && !subject.ends_with("-docs");
    if kernel { "linux-omarchy" } else { subject }
}

/// The pacman transactions among `events` that switched Omarchy's channel
/// (WP-224): each removed a channel package and installed its twin
/// ([`OMARCHY_CHANNELS`]), in either direction. Omarchy's channel command
/// does that in one `pacman -S`, the old package going as a conflict.
pub fn channel_switches<'a>(events: impl IntoIterator<Item = &'a Event>) -> HashSet<&'a str> {
    let mut seen: HashMap<&str, (HashSet<&str>, HashSet<&str>)> = HashMap::new();
    for e in events {
        let Some(tx) = e.tx_id.as_deref().filter(|_| e.source == Source::Pacman) else {
            continue;
        };
        let (removed, installed) = seen.entry(tx).or_default();
        match e.kind {
            Kind::Remove => removed.insert(e.subject.as_str()),
            Kind::Install => installed.insert(e.subject.as_str()),
            _ => false,
        };
    }
    seen.into_iter()
        .filter(|(_, (removed, installed))| {
            OMARCHY_CHANNELS.iter().any(|&(a, b)| {
                (removed.contains(a) && installed.contains(b))
                    || (removed.contains(b) && installed.contains(a))
            })
        })
        .map(|(tx, _)| tx)
        .collect()
}

/// `config.toml [drift] alwaysRed`, compiled: fnmatch-style globs (`*`,
/// `?`, `[…]`), case-sensitive, matched against the whole subject, and
/// against the name that stands for it ([`family`]): `omarchy` covers
/// `omarchy-dev`, `linux-omarchy` every Omarchy kernel.
///
/// A glob is compiled on its first use, and only for a subject that starts
/// with the glob's literal head (the part before its first `*`, `?` or
/// `[`), which every subject it matches does: an index build compiles
/// nothing when no routine pacman group needs the check (WP-092; the
/// defaults cost about 0.25 ms to compile). A pattern without those
/// characters is compared as a string.
#[derive(Debug, Clone)]
pub struct AlwaysRed(Vec<RedGlob>);

#[derive(Debug, Clone)]
struct RedGlob {
    pattern: String,
    /// The literal head; the whole pattern when it holds no glob character.
    head: usize,
    regex: OnceLock<Option<Regex>>,
}

impl AlwaysRed {
    pub fn new(patterns: &[String]) -> Self {
        AlwaysRed(
            patterns
                .iter()
                .map(|p| RedGlob {
                    head: p.find(['*', '?', '[']).unwrap_or(p.len()),
                    pattern: p.clone(),
                    regex: OnceLock::new(),
                })
                .collect(),
        )
    }

    pub fn matches(&self, subject: &str) -> bool {
        let stands_for = family(subject);
        self.matches_name(subject) || (stands_for != subject && self.matches_name(stands_for))
    }

    fn matches_name(&self, subject: &str) -> bool {
        self.0.iter().any(|g| {
            if g.head == g.pattern.len() {
                return subject == g.pattern;
            }
            subject.starts_with(&g.pattern[..g.head])
                && g.regex
                    .get_or_init(|| glob(&g.pattern))
                    .as_ref()
                    .is_some_and(|r| r.is_match(subject))
        })
    }
}

/// A glob as an anchored regex; `None` for an unclosed `[`… that the regex
/// engine refuses (it is then a literal that never matches a package).
fn glob(pattern: &str) -> Option<Regex> {
    let mut re = String::from("^");
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '[' => {
                // fnmatch: `[!…]` negates; `]` first is literal; an
                // unclosed `[` is a literal `[` and the rest stays literal
                let raw: String = chars.clone().collect();
                let negate = raw.starts_with('!');
                let body = if negate { &raw[1..] } else { &raw[..] };
                match body.char_indices().skip(1).find(|&(_, c)| c == ']') {
                    Some((close, _)) if !body.is_empty() => {
                        let mut class = String::from(if negate { "[^" } else { "[" });
                        for n in body[..close].chars() {
                            if matches!(n, '\\' | '[' | ']' | '&' | '~' | '^') {
                                class.push('\\');
                            }
                            class.push(n);
                        }
                        class.push(']');
                        re.push_str(&class);
                        let used = usize::from(negate) + close + 1;
                        for _ in 0..raw[..used].chars().count() {
                            chars.next();
                        }
                    }
                    _ => re.push_str(&regex::escape("[")),
                }
            }
            c => re.push_str(&regex::escape(&c.to_string())),
        }
    }
    re.push('$');
    Regex::new(&re).ok()
}

/// A member that keeps its group yellow (ADR-0013 §3, ADR-0015 §3): kind
/// `upgrade` or `reinstall`, `explicit: false` (absent errs red), the
/// transaction's command is `pacman` with `-S` and `-u` naming no package
/// (parsed as argv by the shared parser, SPEC-ENGINE §4), and the subject
/// matches no `alwaysRed` glob.
pub fn is_routine(e: &Event, always_red: &AlwaysRed) -> bool {
    matches!(e.kind, Kind::Upgrade | Kind::Reinstall)
        && e.explicit == Some(false)
        && e.meta
            .command
            .as_deref()
            .and_then(|c| parse_command(&split_logged(c)))
            .is_some_and(|c| c.program == "pacman" && c.is_plain_full_upgrade())
        && !always_red.matches(&e.subject)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DriftConfig;

    #[test]
    fn token_rule() {
        let yes = [
            "- [ ] Install zed.",
            "`extra/zed` from the repo",
            "zed",
            "(zed)",
            "~/.config/zed/settings.json",
            "zed, then nvim",
            "ä zed ü",
        ];
        for t in yes {
            assert!(names_token(t, "zed"), "{t:?} names zed");
        }
        let no = [
            "Edit zed.conf",
            "zed-theme",
            "zeditor",
            "my_zed",
            "zed..",
            "Zed",
            "",
        ];
        for t in no {
            assert!(!names_token(t, "zed"), "{t:?} does not name zed");
        }
        // a later occurrence counts when an earlier one does not
        assert!(names_token("zed.conf and zed", "zed"));
        assert!(names_token("tokyo-night.", "tokyo-night"));
        assert!(!names_token("x", ""));
    }

    #[test]
    fn always_red_globs() {
        let red = AlwaysRed::new(&DriftConfig::default().always_red);
        for s in [
            "linux",
            "linux-zen",
            "linux-lts",
            "linux-omarchy",
            "systemd",
            "hyprland",
            "omarchy",
            "omarchy-settings",
            "quickshell",
            "limine",
            "limine-snapper-sync",
            "grub",
            "mkinitcpio",
            "mkinitcpio-busybox",
            "filesystem",
            "pam",
            "sddm",
            "uwsm",
            // WP-224: the edge channel's twins and Omarchy's other kernels
            "omarchy-dev",
            "omarchy-settings-dev",
            "linux-omarchy-bore",
            "linux-omarchy-ptl-novrr-mm",
        ] {
            assert!(red.matches(s), "{s}");
        }
        for s in [
            "firefox",
            "systemd-libs",
            "xlinux",
            "glibc-locales",
            "omarchy-nvim",
            "hyprutils",
            "grub-customizer",
            // kernels only: firmware and headers are not R3
            "linux-firmware",
            "linux-firmware-amdgpu",
            "linux-api-headers",
            "linux-omarchy-headers",
            "linux-omarchy-bore-headers",
            "linux-omarchy-docs",
            "linux-omarchy-",
            "omarchy-dev-tools",
            "omarchy-nvim-dev",
            "linux-headers",
            "pambase",
            "sddm-kcm",
        ] {
            assert!(!red.matches(s), "{s}");
        }
        let custom = AlwaysRed::new(&["lib?nput".into(), "mesa[0-9]".into(), "a.b".into()]);
        assert!(custom.matches("libinput"));
        assert!(custom.matches("mesa2"));
        assert!(!custom.matches("mesax"));
        assert!(custom.matches("a.b"));
        assert!(!custom.matches("axb"), "`.` is literal");
        let odd = AlwaysRed::new(&["[".into(), "x[!".into(), "y[!0-9]".into()]);
        assert!(odd.matches("["));
        assert!(odd.matches("x[!"));
        assert!(!odd.matches("x"));
        assert!(odd.matches("ya"));
        assert!(!odd.matches("y1"));
    }

    /// WP-224: a twin is red through the name that stands for it, and only
    /// that one; the user's list decides, the twin's own name still counts.
    #[test]
    fn family_names_stand_for_their_twins() {
        let cases = [
            ("omarchy-dev", "omarchy"),
            ("omarchy-settings-dev", "omarchy-settings"),
            ("linux-omarchy-bore", "linux-omarchy"),
            ("linux-omarchy-bore-headers", "linux-omarchy-bore-headers"),
            ("linux-omarchy-headers", "linux-omarchy-headers"),
            ("linux-omarchy", "linux-omarchy"),
            ("omarchy", "omarchy"),
            ("omarchy-nvim", "omarchy-nvim"),
            ("linux-zen", "linux-zen"),
        ];
        for (subject, want) in cases {
            assert_eq!(family(subject), want, "{subject}");
        }
        let only = AlwaysRed::new(&["omarchy".into()]);
        assert!(only.matches("omarchy-dev"));
        assert!(!only.matches("omarchy-settings-dev"));
        assert!(!only.matches("linux-omarchy-bore"));
        let settings = AlwaysRed::new(&["omarchy-settings".into()]);
        assert!(settings.matches("omarchy-settings-dev"));
        assert!(!settings.matches("omarchy-dev"));
        let kernel = AlwaysRed::new(&["linux-omarchy".into()]);
        assert!(kernel.matches("linux-omarchy-bore"));
        assert!(!kernel.matches("linux-omarchy-bore-headers"));
        let twin = AlwaysRed::new(&["omarchy-dev".into()]);
        assert!(twin.matches("omarchy-dev"));
        assert!(
            !twin.matches("omarchy"),
            "the stable name never stands in for the twin"
        );
        let none = AlwaysRed::new(&["systemd".into()]);
        assert!(!none.matches("omarchy-dev") && !none.matches("linux-omarchy-bore"));
    }

    /// WP-092: a glob compiles only for a subject that starts with its
    /// literal head; literal patterns never compile.
    #[test]
    fn always_red_compiles_on_demand() {
        let red = AlwaysRed::new(&DriftConfig::default().always_red);
        let compiled = |red: &AlwaysRed| -> Vec<String> {
            red.0
                .iter()
                .filter(|g| g.regex.get().is_some())
                .map(|g| g.pattern.clone())
                .collect()
        };
        assert!(red.matches("linux") && !red.matches("firefox") && !red.matches("lim"));
        assert!(compiled(&red).is_empty(), "{:?}", compiled(&red));
        assert!(red.matches("limine-snapper-sync"));
        assert_eq!(compiled(&red), ["limine*"]);
    }
}
