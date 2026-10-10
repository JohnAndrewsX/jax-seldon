//! pacman's ignore list (ADR-0052, WP-165): the `IgnorePkg` and
//! `IgnoreGroup` names of `/etc/pacman.conf` and the files it includes,
//! read by the pacman collector on every run.
//!
//! - **Names only** (AGENTS.md §6, S9). The files are read as pacman reads
//!   them ([`read`]); of all they hold only the two lists' names are kept.
//!   Every other line, key and value is read past and dropped.
//! - **As pacman reads it.** `#` starts a comment; `[name]` starts a
//!   section, shared across includes (pacman keeps one section state);
//!   `Include = <pattern>` is followed wherever it stands, expanded like
//!   `glob(3)`, at most [`MAX_DEPTH`] levels deep; in `[options]` the
//!   values of `IgnorePkg` and `IgnoreGroup` are split on white space and
//!   add up. Only absolute includes under `/etc` are followed, read under
//!   `Sources::etc_dir`.
//! - **Bounds.** Regular files of at most [`FILE_MAX`], at most
//!   [`MAX_FILES`] per read, names of [`is_name`]'s shape, at most
//!   [`MAX_NAMES`] per list. Anything past them makes the list
//!   [`Ignore::partial`]: what was read is kept.
//! - **Change.** The cursor keeps the list as read ([`Ignore`], shown in
//!   the index) and the last complete one, which a change is measured
//!   against ([`step`]). A change is one pacman `note` ([`event`]),
//!   attention `ignore-list` (ADR-0028 §2 as amended by ADR-0052 §4).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::event::{Event, Kind, Source};
use crate::redact::Redactor;

/// The event's subject: the file pacman reads the lists from.
pub const SUBJECT: &str = "/etc/pacman.conf";

/// `meta` key of the event: the new `IgnorePkg` names, space-separated.
pub const META_PKG: &str = "ignorePkg";

/// `meta` key of the event: the new `IgnoreGroup` names, space-separated.
pub const META_GROUP: &str = "ignoreGroup";

/// The largest file read (pacman.conf and every include).
pub const FILE_MAX: u64 = 1024 * 1024;

/// Files read per run, pacman.conf included.
pub const MAX_FILES: usize = 64;

/// Include levels (pacman's `config_max_recursion`).
pub const MAX_DEPTH: usize = 10;

/// Names kept per list.
pub const MAX_NAMES: usize = 256;

/// Directory entries looked at per expanded glob component.
const MAX_ENTRIES: usize = 4096;

/// The longest name kept.
pub const NAME_MAX: usize = 128;

/// Whether `name` is a name as pacman allows it in both lists: a package
/// or group name, or an `fnmatch` pattern of one — 1 to [`NAME_MAX`]
/// characters of `[A-Za-z0-9@._+*?!^[]-]`.
pub fn is_name(name: &str) -> bool {
    (1..=NAME_MAX).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._+*?!^[]-".contains(&b))
}

/// The two lists. As the cursor's `ignore` (what the last run read) it may
/// be partial; as its `ignoreKnown` (the last complete read) never.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ignore {
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    /// Something could not be read, or was left out: the list may be
    /// incomplete. Written only when true.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}

impl Ignore {
    /// Whether both lists hold the same names as `other`'s, in any order.
    pub fn same_names(&self, other: &Ignore) -> bool {
        let set = |v: &[String]| v.iter().cloned().collect::<HashSet<_>>();
        set(&self.packages) == set(&other.packages) && set(&self.groups) == set(&other.groups)
    }
}

/// The lists as `pacman.conf` under `etc_dir` gives them; `None` when
/// `pacman.conf` itself cannot be read.
pub fn read(etc_dir: &Path) -> Option<Ignore> {
    let root = etc_dir.join("pacman.conf");
    let text = read_file(&root).ok()?;
    let mut p = Parser {
        etc_dir,
        section: None,
        files: 1,
        out: Ignore::default(),
    };
    p.parse(&text, 0);
    Some(p.out)
}

/// One file as text, if it is a regular file of at most [`FILE_MAX`].
fn read_file(path: &Path) -> std::io::Result<String> {
    let bytes = crate::sys::read_regular(path, FILE_MAX)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

struct Parser<'a> {
    etc_dir: &'a Path,
    /// The current section's name; shared by every file of the chain.
    section: Option<String>,
    files: usize,
    out: Ignore,
}

impl Parser<'_> {
    fn parse(&mut self, text: &str, depth: usize) {
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            if line.len() > 2 && line.starts_with('[') && line.ends_with(']') {
                self.section = Some(line[1..line.len() - 1].to_string());
                continue;
            }
            let (key, value) = match line.split_once('=') {
                Some((k, v)) => (k.trim(), Some(v.trim())),
                None => (line, None),
            };
            match (key, value) {
                ("Include", Some(pattern)) if !pattern.is_empty() => self.include(pattern, depth),
                ("Include", _) => self.out.partial = true,
                ("IgnorePkg" | "IgnoreGroup", Some(names))
                    if self.section.as_deref() == Some("options") =>
                {
                    let list = if key == "IgnorePkg" {
                        &mut self.out.packages
                    } else {
                        &mut self.out.groups
                    };
                    for name in names.split_whitespace() {
                        if !is_name(name)
                            || (list.len() >= MAX_NAMES && !list.iter().any(|n| n == name))
                        {
                            self.out.partial = true;
                        } else if !list.iter().any(|n| n == name) {
                            list.push(name.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// `depth`: the includes above the file that names `pattern` (0 for
    /// pacman.conf); pacman reads at most [`MAX_DEPTH`] levels below it.
    fn include(&mut self, pattern: &str, depth: usize) {
        if depth >= MAX_DEPTH {
            self.out.partial = true;
            return;
        }
        let Some((paths, cut)) = expand(self.etc_dir, pattern) else {
            self.out.partial = true;
            return;
        };
        self.out.partial |= cut;
        for path in paths {
            if self.files >= MAX_FILES {
                self.out.partial = true;
                return;
            }
            self.files += 1;
            match read_file(&path) {
                Ok(text) => self.parse(&text, depth + 1),
                Err(_) => self.out.partial = true,
            }
        }
    }
}

/// The files an `Include` pattern names, under `etc_dir` for `/etc`, in
/// `glob(3)`'s order, and whether a folder had more than [`MAX_ENTRIES`]
/// entries to look at; `None` when the pattern is not followed (relative,
/// outside `/etc`, `..`) or a literal part of it does not exist. A pattern
/// that matches nothing gives no files.
fn expand(etc_dir: &Path, pattern: &str) -> Option<(Vec<PathBuf>, bool)> {
    let rest = pattern.strip_prefix("/etc/")?;
    let parts: Vec<&str> = rest
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    if parts.is_empty() || parts.contains(&"..") {
        return None;
    }
    let mut found = vec![etc_dir.to_path_buf()];
    let mut literal = true;
    let mut cut = false;
    for part in parts {
        let mut next = Vec::new();
        if is_glob(part) {
            literal = false;
            let re = glob_regex(part);
            for dir in &found {
                let Ok(entries) = std::fs::read_dir(dir) else {
                    continue;
                };
                let mut entries = entries.peekable();
                let mut names: Vec<String> = entries
                    .by_ref()
                    .take(MAX_ENTRIES)
                    .filter_map(|e| e.ok()?.file_name().into_string().ok())
                    .filter(|n| (!n.starts_with('.') || part.starts_with('.')) && re.is_match(n))
                    .collect();
                cut |= entries.peek().is_some();
                names.sort();
                next.extend(names.into_iter().map(|n| dir.join(n)));
            }
        } else {
            next = found
                .iter()
                .map(|d| d.join(part))
                .filter(|p| std::fs::symlink_metadata(p).is_ok())
                .collect();
        }
        found = next;
    }
    if literal && found.is_empty() {
        return None;
    }
    // a directory is no config file; pacman fails to read it as well
    Some((found, cut))
}

fn is_glob(part: &str) -> bool {
    part.contains(['*', '?', '['])
}

/// One path component of a glob as an anchored regex: `*`, `?` and
/// bracket classes (`!` or `^` negates); an unclosed `[` is literal.
fn glob_regex(part: &str) -> Regex {
    let chars: Vec<char> = part.chars().collect();
    let lit = |c: char| format!("\\x{{{:X}}}", c as u32);
    let mut re = String::from("^");
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '[' => {
                let mut j = i + 1;
                if j < chars.len() && (chars[j] == '!' || chars[j] == '^') {
                    j += 1;
                }
                if j < chars.len() && chars[j] == ']' {
                    j += 1;
                }
                while j < chars.len() && chars[j] != ']' {
                    j += 1;
                }
                if j >= chars.len() {
                    re.push_str(&lit('['));
                } else {
                    let mut k = i + 1;
                    re.push('[');
                    if chars[k] == '!' || chars[k] == '^' {
                        re.push('^');
                        k += 1;
                    }
                    let mut first = true;
                    while k < j {
                        let c = chars[k];
                        if c == '-' && !first && k + 1 < j {
                            re.push('-');
                        } else {
                            re.push_str(&lit(c));
                        }
                        first = false;
                        k += 1;
                    }
                    re.push(']');
                    i = j;
                }
            }
            c => re.push_str(&lit(c)),
        }
        i += 1;
    }
    re.push('$');
    Regex::new(&re).unwrap_or_else(|_| Regex::new("^$").expect("valid regex"))
}

/// What the cursor keeps after a run that read `read`, and the event of a
/// change. `shown`/`known`: the cursor's `ignore` and `ignoreKnown`.
///
/// - `read` is `None` (pacman.conf unreadable): the last list, marked
///   partial; no event.
/// - a partial read: shown as read, `known` kept; no event.
/// - a complete read: shown and known; an event when `known` held other
///   names (none without `known`: a baseline).
pub fn step(
    shown: Option<&Ignore>,
    known: Option<&Ignore>,
    read: Option<Ignore>,
    now: DateTime<FixedOffset>,
) -> (Option<Ignore>, Option<Ignore>, Option<Event>) {
    let Some(read) = read else {
        let kept = shown.map(|s| Ignore {
            partial: true,
            ..s.clone()
        });
        return (kept, known.cloned(), None);
    };
    if read.partial {
        return (Some(read), known.cloned(), None);
    }
    let event = known
        .filter(|k| !k.same_names(&read))
        .map(|k| event(k, &read, now));
    (Some(read.clone()), Some(read), event)
}

/// The pacman `note` of a changed list (ADR-0052 §3).
pub fn event(old: &Ignore, new: &Ignore, now: DateTime<FixedOffset>) -> Event {
    let mut parts = Vec::new();
    for (key, before, after) in [
        ("IgnorePkg", &old.packages, &new.packages),
        ("IgnoreGroup", &old.groups, &new.groups),
    ] {
        let added: Vec<&str> = after
            .iter()
            .filter(|n| !before.contains(n))
            .map(String::as_str)
            .collect();
        let removed: Vec<&str> = before
            .iter()
            .filter(|n| !after.contains(n))
            .map(String::as_str)
            .collect();
        let mut what = Vec::new();
        if !added.is_empty() {
            what.push(format!("added {}", added.join(", ")));
        }
        if !removed.is_empty() {
            what.push(format!("removed {}", removed.join(", ")));
        }
        if !what.is_empty() {
            parts.push(format!("{key}: {}.", what.join("; ")));
        }
    }
    let mut e = Event::new(now, Source::Pacman, Kind::Note, SUBJECT).detail(parts.join(" "));
    e.meta
        .extra
        .insert(META_PKG.into(), Value::String(new.packages.join(" ")));
    e.meta
        .extra
        .insert(META_GROUP.into(), Value::String(new.groups.join(" ")));
    e
}

/// Whether `e` is the collector's record of a changed list: a pacman
/// `note` with `meta.ignorePkg` (ADR-0052 §4).
pub fn is_change(e: &Event) -> bool {
    e.source == Source::Pacman
        && e.kind == Kind::Note
        && e.meta.extra.get(META_PKG).is_some_and(Value::is_string)
}

/// `system.pacmanIgnore` at index time (ADR-0052 §5): the cursor's list
/// without the names `redactor` would change (the list is then partial).
pub fn shown(ignore: &Ignore, redactor: &Redactor) -> Ignore {
    let mut partial = ignore.partial;
    let mut keep = |names: &[String]| -> Vec<String> {
        names
            .iter()
            .filter(|n| {
                let ok = is_name(n) && redactor.redact(n) == **n;
                partial |= !ok;
                ok
            })
            .take(MAX_NAMES)
            .cloned()
            .collect()
    };
    let packages = keep(&ignore.packages);
    let groups = keep(&ignore.groups);
    Ignore {
        packages,
        groups,
        partial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporary `/etc`, removed when dropped.
    struct Tmp(PathBuf);

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn with_etc(files: &[(&str, &str)]) -> (Tmp, PathBuf) {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("seldon-ignore-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let s = Tmp(root);
        let etc = s.0.join("etc");
        for (rel, text) in files {
            let p = etc.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        std::fs::create_dir_all(&etc).unwrap();
        (s, etc)
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-10-10T12:00:00+02:00").unwrap()
    }

    #[test]
    fn reads_options_lines_with_comments_and_repeats() {
        let (_s, etc) = with_etc(&[(
            "pacman.conf",
            "# IgnorePkg = commented\n\
             [options]\n\
             HoldPkg = pacman glibc\n\
             IgnorePkg   = linux linux-headers # the kernel stays\n\
             #IgnorePkg = old\n\
             IgnorePkg=mesa\tlib32-mesa\n\
             IgnorePkg = linux\n\
             IgnoreGroup = gnome\n\
             ignorepkg = not-a-key\n\
             IgnorePkg\n\
             [core]\n\
             IgnorePkg = in-a-repo\n\
             Server = https://example.org/$repo\n",
        )]);
        let got = read(&etc).unwrap();
        assert_eq!(
            got.packages,
            names(&["linux", "linux-headers", "mesa", "lib32-mesa"])
        );
        assert_eq!(got.groups, names(&["gnome"]));
        assert!(!got.partial);
    }

    #[test]
    fn follows_includes_as_pacman_does() {
        let (_s, etc) = with_etc(&[
            (
                "pacman.conf",
                "[options]\nInclude = /etc/pacman.d/*.conf\nIgnorePkg = a\n\
                 [core]\nInclude = /etc/pacman.d/mirrorlist\n",
            ),
            // sorted by name: 10 before 20; a header in an include holds
            // for the rest of the includer too
            (
                "pacman.d/10-pin.conf",
                "IgnorePkg = b c\nIgnoreGroup = kde\n",
            ),
            ("pacman.d/20-repo.conf", "[custom]\nServer = file:///x\n"),
            ("pacman.d/.hidden.conf", "IgnorePkg = hidden\n"),
            ("pacman.d/notes.txt", "IgnorePkg = not-included\n"),
            (
                "pacman.d/mirrorlist",
                "Server = https://m.example/$repo\n[options]\nIgnorePkg = late\n",
            ),
        ]);
        let got = read(&etc).unwrap();
        // `a` follows the include whose [custom] header took the section
        assert_eq!(got.packages, names(&["b", "c", "late"]));
        assert_eq!(got.groups, names(&["kde"]));
        assert!(!got.partial);
    }

    #[test]
    fn includes_nest_and_stop_at_pacmans_depth() {
        let mut files = vec![(
            "pacman.conf".to_string(),
            "[options]\nInclude = /etc/n/0.conf\n".to_string(),
        )];
        for i in 0..12 {
            files.push((
                format!("n/{i}.conf"),
                format!("IgnorePkg = p{i}\nInclude = /etc/n/{}.conf\n", i + 1),
            ));
        }
        let refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let (_s, etc) = with_etc(&refs);
        let got = read(&etc).unwrap();
        assert_eq!(got.packages.len(), MAX_DEPTH);
        assert!(got.partial, "the chain was cut");
    }

    #[test]
    fn what_is_not_followed_makes_the_list_partial() {
        for include in [
            "Include = relative/x.conf",
            "Include = /usr/share/x.conf",
            "Include = /etc/missing.conf",
            "Include = /etc/../home/x.conf",
            "Include =",
        ] {
            let (_s, etc) = with_etc(&[(
                "pacman.conf",
                &format!("[options]\nIgnorePkg = a\n{include}\n"),
            )]);
            let got = read(&etc).unwrap();
            assert_eq!(got.packages, names(&["a"]), "{include}");
            assert!(got.partial, "{include}");
        }
        // a pattern that matches nothing is no error
        let (_s, etc) = with_etc(&[(
            "pacman.conf",
            "[options]\nInclude = /etc/pacman.d/*.conf\nIgnorePkg = a\n",
        )]);
        assert!(!read(&etc).unwrap().partial);
    }

    #[test]
    fn bad_and_too_many_names_are_left_out() {
        let many: Vec<String> = (0..MAX_NAMES + 3).map(|i| format!("p{i}")).collect();
        let (_s, etc) = with_etc(&[(
            "pacman.conf",
            &format!(
                "[options]\nIgnorePkg = ok linux-*  b\u{e4}d $(x)\nIgnoreGroup = {}\n",
                many.join(" ")
            ),
        )]);
        let got = read(&etc).unwrap();
        assert_eq!(got.packages, names(&["ok", "linux-*"]));
        assert_eq!(got.groups.len(), MAX_NAMES);
        assert!(got.partial);
    }

    #[test]
    fn an_unreadable_root_is_none_and_a_large_include_partial() {
        let (_s, etc) = with_etc(&[]);
        assert_eq!(read(&etc), None);
        let big = "#".repeat(FILE_MAX as usize + 1);
        let (_s, etc) = with_etc(&[
            (
                "pacman.conf",
                "[options]\nIgnorePkg = a\nInclude = /etc/big.conf\n",
            ),
            ("big.conf", &big),
        ]);
        let got = read(&etc).unwrap();
        assert_eq!(got.packages, names(&["a"]));
        assert!(got.partial);
    }

    #[test]
    fn name_shapes() {
        for ok in [
            "mesa",
            "lib32-mesa",
            "linux-*",
            "python3.12",
            "gtk+",
            "a@b",
            "[!x]?",
            "x".repeat(128).as_str(),
        ] {
            assert!(is_name(ok), "{ok}");
        }
        for bad in [
            "",
            "b\u{e4}d",
            "$(x)",
            "a b",
            "a/b",
            "a\u{202e}b",
            "x".repeat(129).as_str(),
        ] {
            assert!(!is_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn glob_classes() {
        let m = |g: &str, n: &str| glob_regex(g).is_match(n);
        assert!(m("[0-9]*.conf", "10-x.conf"));
        assert!(!m("[!0-9]*.conf", "10-x.conf"));
        assert!(m("[^0-9]*.conf", "x.conf"));
        assert!(m("a?c", "abc"));
        assert!(m("a[.", "a[."), "an unclosed class is literal");
        assert!(m("[]x]", "]"));
        assert!(!m("*.conf", "x.confx"));
    }

    #[test]
    fn steps_take_a_baseline_then_report_changes() {
        let l = |p: &[&str], g: &[&str]| Ignore {
            packages: names(p),
            groups: names(g),
            partial: false,
        };
        // baseline: no known list, no event
        let (s, k, e) = step(None, None, Some(l(&["a"], &[])), now());
        assert_eq!(
            (s.clone(), k.clone(), e.is_none()),
            (Some(l(&["a"], &[])), Some(l(&["a"], &[])), true)
        );
        // the same names in another order: no event
        let (_, _, e) = step(s.as_ref(), k.as_ref(), Some(l(&["a"], &[])), now());
        assert!(e.is_none());
        // a partial read: shown, known kept, no event
        let partial = Ignore {
            partial: true,
            ..l(&[], &[])
        };
        let (s2, k2, e) = step(s.as_ref(), k.as_ref(), Some(partial.clone()), now());
        assert_eq!(
            (s2, k2.clone(), e.is_none()),
            (Some(partial), k.clone(), true)
        );
        // unreadable: the last list, partial
        let (s3, k3, e) = step(s.as_ref(), k.as_ref(), None, now());
        assert!(s3.unwrap().partial && k3 == k && e.is_none());
        // a change against the known list, not the partial one
        let (_, k4, e) = step(None, k2.as_ref(), Some(l(&["b", "c"], &["kde"])), now());
        let e = e.unwrap();
        assert_eq!(k4, Some(l(&["b", "c"], &["kde"])));
        assert!(is_change(&e));
        assert_eq!(e.subject, SUBJECT);
        assert_eq!(
            e.detail.as_deref(),
            Some("IgnorePkg: added b, c; removed a. IgnoreGroup: added kde.")
        );
        assert_eq!(e.meta.extra[META_PKG], "b c");
        assert_eq!(e.meta.extra[META_GROUP], "kde");
        // emptied
        let (_, _, e) = step(None, Some(&l(&["a"], &[])), Some(l(&[], &[])), now());
        let e = e.unwrap();
        assert_eq!(e.detail.as_deref(), Some("IgnorePkg: removed a."));
        assert_eq!(e.meta.extra[META_PKG], "");
    }

    #[test]
    fn shown_drops_what_the_redaction_changes() {
        let r = Redactor::with_patterns(&["secret-[a-z]+".to_string()]).unwrap();
        let got = shown(
            &Ignore {
                packages: names(&["mesa", "secret-pkg"]),
                groups: names(&["kde"]),
                partial: false,
            },
            &r,
        );
        assert_eq!(got.packages, names(&["mesa"]));
        assert_eq!(got.groups, names(&["kde"]));
        assert!(got.partial);
    }
}
