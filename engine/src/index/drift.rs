//! The drift rules the index applies at build time: the proposal token rule
//! (ADR-0015 §4), the `[drift] alwaysRed` globs and the routine class of a
//! pacman group member (ADR-0013 §3).

use regex::Regex;

use crate::model::event::{Event, Kind};
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

/// `config.toml [drift] alwaysRed`, compiled: fnmatch-style globs (`*`,
/// `?`, `[…]`), case-sensitive, matched against the whole subject.
#[derive(Debug, Clone)]
pub struct AlwaysRed(Vec<Regex>);

impl AlwaysRed {
    pub fn new(patterns: &[String]) -> Self {
        AlwaysRed(patterns.iter().filter_map(|p| glob(p)).collect())
    }

    pub fn matches(&self, subject: &str) -> bool {
        self.0.iter().any(|r| r.is_match(subject))
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
            "linux-firmware",
            "linux-zen",
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
}
