//! pacman's ignore list (ADR-0052, WP-165): the `IgnorePkg` and
//! `IgnoreGroup` names of `/etc/pacman.conf` and the files it includes,
//! read by the pacman collector on every run.
//!
//! - **Names only** (AGENTS.md §6, S9). The files are read as pacman reads
//!   them ([`read`]); of all they hold only the two lists' names are kept.
//!   Every other line, key and value is read past and dropped.
//! - **As pacman reads it.** What `pacman.conf(5)` documents: CamelCase
//!   keys, `Include = <path>` expanded by glob(7) rules, `IgnorePkg =
//!   package ...` (names separated by spaces; a tab is part of a name, as
//!   in pacman's `setrepeatingoption`). Beyond the page, as pacman's
//!   parser (`ini.c`, `conf.c`) does: `#` cuts the rest of a line (the page
//!   says comments start a line; pacman's parser drops end-of-line
//!   comments too), `[name]` starts a section shared across includes,
//!   `Include` is followed wherever it stands, at most [`MAX_DEPTH`]
//!   levels below `pacman.conf`; the two keys count only in `[options]`
//!   and repeated lines add up. An include is read wherever it lies: a
//!   path under `/etc` below `Sources::etc_dir`, any other absolute path
//!   below that directory's parent (`/` on the host, the guard in tests).
//! - **Partial means incomplete** ([`Ignore::partial`]): a file that
//!   cannot be read, a relative include, the depth, or the budget of one
//!   read ([`Limits`]: files, includes, directory entries — counted, never
//!   timed) cut the read. A partial read writes no event.
//! - **Hidden names.** A name Seldon does not show (not of [`is_name`]'s
//!   shape, or changed by the redaction) is still a name: it counts in
//!   the comparison and in the event (as [`HIDDEN`]), and the index counts
//!   it as hidden ([`shown`]).
//! - **Change.** The cursor keeps the list as read (shown in the index),
//!   the last complete one, which a change is measured against, and when
//!   that was read ([`step`]). A change is one pacman `note` ([`event`]),
//!   attention `ignore-list` (ADR-0028 §2 as amended by ADR-0052 §4).

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

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

/// A name the event does not spell out (not of [`is_name`]'s shape).
pub const HIDDEN: &str = "(hidden)";

/// The largest file read (pacman.conf and every include).
pub const FILE_MAX: u64 = 1024 * 1024;

/// Include levels below pacman.conf.
pub const MAX_DEPTH: usize = 10;

/// Names kept per list.
pub const MAX_NAMES: usize = 256;

/// A longer name is kept in the cursor as `sha256:<hex>` of its bytes.
pub const TOKEN_MAX: usize = 512;

/// The longest name shown.
pub const NAME_MAX: usize = 128;

/// The budget of one read: everything counted, nothing timed (WP-174).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Files read, pacman.conf included.
    pub files: usize,
    /// `Include` lines followed.
    pub includes: usize,
    /// Directory entries looked at by every glob of the read together.
    pub entries: usize,
}

impl Limits {
    pub const DEFAULT: Limits = Limits {
        files: 64,
        includes: 64,
        entries: 16_384,
    };
}

/// Whether `name` is a name Seldon shows: a package or group name, or an
/// `fnmatch` pattern of one — 1 to [`NAME_MAX`] characters of
/// `[A-Za-z0-9@._+*?!^[]-]`.
pub fn is_name(name: &str) -> bool {
    (1..=NAME_MAX).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._+*?!^[]-".contains(&b))
}

/// The two lists as pacman holds them (engine state, raw names). As the
/// cursor's `ignore` (what the last run read) it may be partial; as its
/// `ignoreKnown` (the last complete read) never.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ignore {
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    /// The read was incomplete: names may be missing. Written only when
    /// true.
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

/// `system.pacmanIgnore` (ADR-0052 §5): the names Seldon shows, and how
/// many it does not.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Shown {
    pub packages: Vec<String>,
    pub groups: Vec<String>,
    /// Names of both lists not shown. Written only when not 0.
    #[serde(skip_serializing_if = "is_zero")]
    pub hidden: usize,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// The lists as `pacman.conf` under `etc_dir` gives them; `None` when
/// `pacman.conf` itself cannot be read.
pub fn read(etc_dir: &Path) -> Option<Ignore> {
    read_with(etc_dir, Limits::DEFAULT)
}

/// [`read`] within `limits`.
pub fn read_with(etc_dir: &Path, limits: Limits) -> Option<Ignore> {
    let text = read_file(&etc_dir.join("pacman.conf")).ok()?;
    let mut p = Parser {
        etc_dir,
        limits,
        section: None,
        files: 1,
        includes: 0,
        entries: 0,
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

/// A name as the cursor keeps it: itself, or the hash of a long one.
fn token(name: &str) -> String {
    if name.len() <= TOKEN_MAX {
        name.to_string()
    } else {
        format!("sha256:{}", crate::sys::sha256_hex(name.as_bytes()))
    }
}

struct Parser<'a> {
    etc_dir: &'a Path,
    limits: Limits,
    /// The current section's name; shared by every file of the chain.
    section: Option<String>,
    files: usize,
    includes: usize,
    entries: usize,
    out: Ignore,
}

impl Parser<'_> {
    fn parse(&mut self, text: &str, depth: usize) {
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                if line.len() <= 2 {
                    // pacman refuses `[]`
                    self.out.partial = true;
                } else {
                    self.section = Some(line[1..line.len() - 1].to_string());
                }
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
                    // pacman splits on spaces only
                    for name in names.split(' ').filter(|n| !n.is_empty()) {
                        let name = token(name);
                        if list.contains(&name) {
                            continue;
                        }
                        if list.len() >= MAX_NAMES {
                            self.out.partial = true;
                        } else {
                            list.push(name);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// `depth`: the includes above the file that names `pattern` (0 for
    /// pacman.conf).
    fn include(&mut self, pattern: &str, depth: usize) {
        self.includes += 1;
        if depth >= MAX_DEPTH || self.includes > self.limits.includes {
            self.out.partial = true;
            return;
        }
        let Some(paths) = self.expand(pattern) else {
            self.out.partial = true;
            return;
        };
        for path in paths {
            if self.files >= self.limits.files {
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

    /// The files an `Include` pattern names, in glob(3)'s order: `/etc/…`
    /// below `etc_dir`, another absolute path below its parent. `None`
    /// when the pattern is relative (pacman reads it from its working
    /// directory, which Seldon cannot know), a literal part does not
    /// exist, or the entry budget ran out. A pattern that matches nothing
    /// gives no files.
    fn expand(&mut self, pattern: &str) -> Option<Vec<PathBuf>> {
        let path = Path::new(pattern);
        if !path.is_absolute() {
            return None;
        }
        let mut parts: Vec<String> = Vec::new();
        for c in path.components() {
            match c {
                Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
                Component::ParentDir => {
                    parts.pop();
                }
                _ => {}
            }
        }
        let base = if parts.first().is_some_and(|p| p == "etc") {
            parts.remove(0);
            self.etc_dir.to_path_buf()
        } else {
            self.etc_dir
                .parent()
                .unwrap_or(Path::new("/"))
                .to_path_buf()
        };
        if parts.is_empty() {
            return None;
        }
        let mut found = vec![base];
        let mut literal = true;
        for part in &parts {
            let mut next = Vec::new();
            if is_glob(part) {
                literal = false;
                let re = glob_regex(part);
                for dir in &found {
                    let Ok(entries) = std::fs::read_dir(dir) else {
                        continue;
                    };
                    let mut names = Vec::new();
                    for entry in entries {
                        self.entries += 1;
                        if self.entries > self.limits.entries {
                            return None;
                        }
                        let Some(n) = entry.ok().and_then(|e| e.file_name().into_string().ok())
                        else {
                            continue;
                        };
                        if (!n.starts_with('.') || part.starts_with('.')) && re.is_match(&n) {
                            names.push(n);
                        }
                    }
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
        Some(found)
    }
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

/// What the cursor keeps after one read ([`step`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The cursor's `ignore`: what the index shows.
    pub shown: Option<Ignore>,
    /// The cursor's `ignoreKnown`: the last complete read.
    pub known: Option<Ignore>,
    /// Whether this read set `known` (the cursor's `ignoreAt` becomes now).
    pub read_complete: bool,
    pub event: Option<Event>,
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
) -> Step {
    let Some(read) = read else {
        let kept = shown.map(|s| Ignore {
            partial: true,
            ..s.clone()
        });
        return Step {
            shown: kept,
            known: known.cloned(),
            read_complete: false,
            event: None,
        };
    };
    if read.partial {
        return Step {
            shown: Some(read),
            known: known.cloned(),
            read_complete: false,
            event: None,
        };
    }
    let event = known
        .filter(|k| !k.same_names(&read))
        .map(|k| event(k, &read, now));
    Step {
        shown: Some(read.clone()),
        known: Some(read),
        read_complete: true,
        event,
    }
}

/// A name as the event writes it: itself, or [`HIDDEN`].
fn written(name: &str) -> &str {
    if is_name(name) { name } else { HIDDEN }
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
            .map(|n| written(n))
            .collect();
        let removed: Vec<&str> = before
            .iter()
            .filter(|n| !after.contains(n))
            .map(|n| written(n))
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
    let (pkg, group) = meta_values(new);
    let mut e = Event::new(now, Source::Pacman, Kind::Note, SUBJECT).detail(parts.join(" "));
    e.meta.extra.insert(META_PKG.into(), Value::String(pkg));
    e.meta.extra.insert(META_GROUP.into(), Value::String(group));
    e
}

/// `meta.ignorePkg` and `meta.ignoreGroup` of the event for `list`, before
/// the ledger's redaction.
fn meta_values(list: &Ignore) -> (String, String) {
    let join = |v: &[String]| v.iter().map(|n| written(n)).collect::<Vec<_>>().join(" ");
    (join(&list.packages), join(&list.groups))
}

/// Whether `events` (ledger order) hold, at or after `since`, a change
/// that recorded `list` as the ledger writes it (`redactor`): the newest
/// change since then. A note of that capture is in the ledger although
/// the cursor is older (its save failed after the write, or an older
/// state directory was restored); a note from before `since` — when the
/// cursor's known list was read — never counts (ADR-0052 §2).
pub fn recorded_since(
    events: &[Event],
    since: DateTime<FixedOffset>,
    list: &Ignore,
    redactor: &Redactor,
) -> bool {
    let Some(e) = events.iter().rev().find(|e| e.ts >= since && is_change(e)) else {
        return false;
    };
    let (pkg, group) = meta_values(list);
    let value = |key: &str| e.meta.extra.get(key).and_then(Value::as_str);
    value(META_PKG) == Some(redactor.redact(&pkg).as_str())
        && value(META_GROUP) == Some(redactor.redact(&group).as_str())
}

/// Whether `e` is the collector's record of a changed list: a pacman
/// `note` with `meta.ignorePkg` (ADR-0052 §4).
pub fn is_change(e: &Event) -> bool {
    e.source == Source::Pacman
        && e.kind == Kind::Note
        && e.meta.extra.get(META_PKG).is_some_and(Value::is_string)
}

/// `system.pacmanIgnore` at index time (ADR-0052 §5): the cursor's names
/// Seldon shows — of [`is_name`]'s shape and unchanged by `redactor` —
/// and the count of the others.
pub fn shown(ignore: &Ignore, redactor: &Redactor) -> Shown {
    let mut hidden = 0;
    let mut keep = |names: &[String]| -> Vec<String> {
        names
            .iter()
            .filter(|n| {
                let ok = is_name(n) && redactor.redact(n) == **n;
                hidden += usize::from(!ok);
                ok
            })
            .cloned()
            .collect()
    };
    let packages = keep(&ignore.packages);
    let groups = keep(&ignore.groups);
    Shown {
        packages,
        groups,
        hidden,
        partial: ignore.partial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporary root with `etc/` below it, removed when dropped.
    struct Tmp(PathBuf);

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// `files` below the root (`etc/…` for the system configuration);
    /// returns the root guard and the `etc` directory.
    fn with_root(files: &[(&str, &str)]) -> (Tmp, PathBuf) {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("seldon-ignore-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let s = Tmp(root);
        for (rel, text) in files {
            let p = s.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        let etc = s.0.join("etc");
        std::fs::create_dir_all(&etc).unwrap();
        (s, etc)
    }

    /// [`with_root`] for files below `etc/`.
    fn with_etc(files: &[(&str, &str)]) -> (Tmp, PathBuf) {
        let rooted: Vec<(String, &str)> = files
            .iter()
            .map(|(r, t)| (format!("etc/{r}"), *t))
            .collect();
        let refs: Vec<(&str, &str)> = rooted.iter().map(|(r, t)| (r.as_str(), *t)).collect();
        with_root(&refs)
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn list(p: &[&str], g: &[&str]) -> Ignore {
        Ignore {
            packages: names(p),
            groups: names(g),
            partial: false,
        }
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
             IgnorePkg=mesa  lib32-mesa\n\
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
    fn names_are_separated_by_spaces_only() {
        // pacman.conf(5): `IgnorePkg = package ...`; pacman splits on ' '
        // (setrepeatingoption), so a tab or a comma is part of a name
        let (_s, etc) = with_etc(&[(
            "pacman.conf",
            "[options]\nIgnorePkg = mesa\tlib32-mesa linux,nvidia-utils zoom\n",
        )]);
        let got = read(&etc).unwrap();
        assert_eq!(
            got.packages,
            names(&["mesa\tlib32-mesa", "linux,nvidia-utils", "zoom"])
        );
        assert!(
            !got.partial,
            "a name Seldon does not show is no incomplete read"
        );
    }

    #[test]
    fn follows_includes_as_pacman_does() {
        let (_s, etc) = with_root(&[
            (
                "etc/pacman.conf",
                "[options]\nInclude = /etc/pacman.d/*.conf\nIgnorePkg = a\n\
                 Include = /usr/share/pins/../pins/extra.conf\n\
                 [core]\nInclude = /etc/pacman.d/mirrorlist\n",
            ),
            // sorted by name: 10 before 20; a header in an include holds
            // for the rest of the includer too
            (
                "etc/pacman.d/10-pin.conf",
                "IgnorePkg = b c\nIgnoreGroup = kde\n",
            ),
            (
                "etc/pacman.d/20-repo.conf",
                "[custom]\nServer = file:///x\n",
            ),
            ("etc/pacman.d/.hidden.conf", "IgnorePkg = hidden\n"),
            ("etc/pacman.d/notes.txt", "IgnorePkg = not-included\n"),
            (
                "etc/pacman.d/mirrorlist",
                "Server = https://m.example/$repo\n[options]\nIgnorePkg = late\n",
            ),
            // outside /etc: read like any include (below the root)
            (
                "usr/share/pins/extra.conf",
                "[options]\nIgnorePkg = outside\n",
            ),
        ]);
        let got = read(&etc).unwrap();
        // `a` follows the include whose [custom] header took the section
        assert_eq!(got.packages, names(&["b", "c", "outside", "late"]));
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
    fn what_cannot_be_read_makes_the_list_partial() {
        for include in [
            "Include = relative/x.conf",
            "Include = /etc/missing.conf",
            "Include = /usr/share/missing.conf",
            "Include = /etc/pacman.d",
            "Include =",
        ] {
            let (_s, etc) = with_etc(&[
                (
                    "pacman.conf",
                    &format!("[options]\nIgnorePkg = a\n{include}\n"),
                ),
                ("pacman.d/x.conf", ""),
            ]);
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
    fn too_many_and_too_long_names() {
        let many: Vec<String> = (0..MAX_NAMES + 3).map(|i| format!("p{i}")).collect();
        let long = "x".repeat(TOKEN_MAX + 1);
        let (_s, etc) = with_etc(&[(
            "pacman.conf",
            &format!(
                "[options]\nIgnorePkg = ok {long}\nIgnoreGroup = {}\n",
                many.join(" ")
            ),
        )]);
        let got = read(&etc).unwrap();
        assert_eq!(got.packages[0], "ok");
        assert_eq!(
            got.packages[1],
            format!("sha256:{}", crate::sys::sha256_hex(long.as_bytes()))
        );
        assert_eq!(got.groups.len(), MAX_NAMES);
        assert!(got.partial, "names past the bound were left out");
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
    fn one_read_has_one_budget() {
        // files: 65 includes of one file each pass the default 64 files
        let mut files = vec![("pacman.conf".to_string(), "[options]\n".to_string())];
        for i in 0..65 {
            files[0].1.push_str(&format!("Include = /etc/d/{i}.conf\n"));
            files.push((format!("d/{i}.conf"), format!("IgnorePkg = p{i}\n")));
        }
        let refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let (_s, etc) = with_etc(&refs);
        let got = read(&etc).unwrap();
        assert_eq!(got.packages.len(), Limits::DEFAULT.files - 1);
        assert!(got.partial);
        let roomy = Limits {
            files: 100,
            includes: 100,
            entries: 100,
        };
        assert!(!read_with(&etc, roomy).unwrap().partial);

        // includes: lines that match nothing still count
        let lines = "Include = /etc/none/*.conf\n".repeat(5);
        let (_s, etc) = with_etc(&[("pacman.conf", &format!("[options]\nIgnorePkg = a\n{lines}"))]);
        let tight = Limits {
            includes: 4,
            ..Limits::DEFAULT
        };
        assert!(read_with(&etc, tight).unwrap().partial);
        let enough = Limits {
            includes: 5,
            ..Limits::DEFAULT
        };
        assert!(!read_with(&etc, enough).unwrap().partial);

        // entries: every glob of the read looks at the same budget
        let mut files = vec![(
            "pacman.conf".to_string(),
            "[options]\nInclude = /etc/a/*.nomatch\nInclude = /etc/a/*.nomatch\n".to_string(),
        )];
        for i in 0..6 {
            files.push((format!("a/{i}"), String::new()));
        }
        let refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let (_s, etc) = with_etc(&refs);
        let at = |entries| {
            read_with(
                &etc,
                Limits {
                    entries,
                    ..Limits::DEFAULT
                },
            )
            .unwrap()
            .partial
        };
        assert!(at(11), "two globs of six entries need twelve");
        assert!(!at(12));
        assert_eq!(Limits::DEFAULT.entries, 16_384);
        assert_eq!(Limits::DEFAULT.includes, 64);
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
        // baseline: no known list, no event
        let s = step(None, None, Some(list(&["a", "b"], &["g", "h"])), now());
        assert_eq!(s.known, Some(list(&["a", "b"], &["g", "h"])));
        assert!(s.read_complete && s.event.is_none());
        // the same names in another order, in both lists: no event
        let s2 = step(
            s.shown.as_ref(),
            s.known.as_ref(),
            Some(list(&["b", "a"], &["h", "g"])),
            now(),
        );
        assert!(s2.event.is_none(), "{:?}", s2.event);
        // a partial read: shown, known kept, no event
        let partial = Ignore {
            partial: true,
            ..list(&[], &[])
        };
        let s3 = step(
            s.shown.as_ref(),
            s.known.as_ref(),
            Some(partial.clone()),
            now(),
        );
        assert_eq!(
            (s3.shown, s3.known.clone()),
            (Some(partial), s.known.clone())
        );
        assert!(!s3.read_complete && s3.event.is_none());
        // unreadable: the last list, partial
        let s4 = step(s.shown.as_ref(), s.known.as_ref(), None, now());
        assert!(s4.shown.unwrap().partial && s4.known == s.known && !s4.read_complete);
        // a change against the known list, not the partial one
        let s5 = step(
            None,
            s3.known.as_ref(),
            Some(list(&["b", "c"], &["kde"])),
            now(),
        );
        let e = s5.event.unwrap();
        assert!(is_change(&e));
        assert_eq!(e.subject, SUBJECT);
        assert_eq!(
            e.detail.as_deref(),
            Some("IgnorePkg: added c; removed a. IgnoreGroup: added kde; removed g, h.")
        );
        assert_eq!(e.meta.extra[META_PKG], "b c");
        assert_eq!(e.meta.extra[META_GROUP], "kde");
        // emptied
        let e = step(None, Some(&list(&["a"], &[])), Some(list(&[], &[])), now())
            .event
            .unwrap();
        assert_eq!(e.detail.as_deref(), Some("IgnorePkg: removed a."));
        assert_eq!(e.meta.extra[META_PKG], "");
        // a name Seldon does not show is a change too, written as hidden
        let e = step(
            None,
            Some(&list(&["linux"], &[])),
            Some(list(&["linux,nvidia-utils"], &[])),
            now(),
        )
        .event
        .unwrap();
        assert_eq!(
            e.detail.as_deref(),
            Some("IgnorePkg: added (hidden); removed linux.")
        );
        assert_eq!(e.meta.extra[META_PKG], HIDDEN);
    }

    #[test]
    fn only_a_change_since_the_known_list_counts_as_recorded() {
        let at = |t: &str| DateTime::parse_from_rfc3339(t).unwrap();
        let r = Redactor::builtin();
        let new = list(&["linux", "mesa"], &[]);
        let mut note = event(
            &list(&["linux"], &[]),
            &new,
            at("2026-10-10T10:05:00+02:00"),
        );
        // a note of the capture whose cursor save failed: after the read
        let since = at("2026-10-10T10:00:00+02:00");
        assert!(recorded_since(std::slice::from_ref(&note), since, &new, &r));
        // the same note before the known list was read (a state loss in
        // between): no record of this change
        let later = at("2026-10-10T10:10:00+02:00");
        assert!(!recorded_since(
            std::slice::from_ref(&note),
            later,
            &new,
            &r
        ));
        // another list, or no note
        assert!(!recorded_since(
            std::slice::from_ref(&note),
            since,
            &list(&["linux"], &[]),
            &r
        ));
        assert!(!recorded_since(&[], since, &new, &r));
        // the newest note decides
        let back = event(
            &new,
            &list(&["linux"], &[]),
            at("2026-10-10T10:06:00+02:00"),
        );
        assert!(!recorded_since(&[note.clone(), back], since, &new, &r));
        // as the ledger writes it: through the redaction
        let r = Redactor::with_patterns(&["corp-[a-z]+".to_string()]).unwrap();
        let secret = list(&["linux", "corp-agent"], &[]);
        note = event(
            &list(&["linux"], &[]),
            &secret,
            at("2026-10-10T10:05:00+02:00"),
        );
        let written = r.redact(note.meta.extra[META_PKG].as_str().unwrap());
        note.meta
            .extra
            .insert(META_PKG.into(), Value::String(written));
        assert!(recorded_since(
            std::slice::from_ref(&note),
            since,
            &secret,
            &r
        ));
    }

    #[test]
    fn shown_hides_what_it_does_not_show() {
        let r = Redactor::with_patterns(&["secret-[a-z]+".to_string()]).unwrap();
        let got = shown(
            &Ignore {
                packages: names(&["mesa", "secret-pkg", "linux,nvidia-utils"]),
                groups: names(&["kde"]),
                partial: false,
            },
            &r,
        );
        assert_eq!(got.packages, names(&["mesa"]));
        assert_eq!(got.groups, names(&["kde"]));
        assert_eq!((got.hidden, got.partial), (2, false));
        let json = serde_json::to_value(&got).unwrap();
        assert_eq!(json["hidden"], 2);
        assert!(json.get("partial").is_none());
        let none = serde_json::to_value(shown(&list(&["a"], &[]), &r)).unwrap();
        assert!(none.get("hidden").is_none());
    }
}
