//! The dossier `system/*.md` (SPEC-LOGBOOK §3, SPEC-ENGINE §3 `seldon
//! dossier`, WP-035): the engine writes the generated fences, the user owns
//! everything outside them.
//!
//! [`Files`] holds the files and replaces fence bodies only
//! ([`views::replace_fence`]), so every byte outside a fence stays as it
//! was. A fence the files lack is appended to its default file under a
//! heading in the logbook language (a missing file is created). [`facts`]
//! reads what the ledger knows (installs, cases, the last Omarchy update)
//! from the index derivation; the renderers below turn facts and host
//! queries ([`query`]) into fence bodies. Nothing here depends on the clock
//! except the date of a new `packages.history` row, so an unchanged system
//! renders the same bytes.

pub mod query;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::collectors::plugins::Listed;
use crate::index::Built;
use crate::index::load::{FENCE_BEGIN, FENCE_END, fence_table, has_table_separator};
use crate::index::views;
use crate::model::Language;
use crate::model::event::{Kind, Resolution, Source, format_ts};
use crate::pkgcmd;
use crate::rebuild::unit_of;
use crate::sys;
use query::{Hardware, Packages, Scope};

/// `--section` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum Section {
    Packages,
    Services,
    Omarchy,
    Hardware,
    Plugins,
    Deviations,
    All,
}

impl Section {
    /// Whether `fence` belongs to one of `selected` (`all`: every fence).
    pub fn selects(selected: &[Section], fence: &Fence) -> bool {
        selected.is_empty()
            || selected
                .iter()
                .any(|s| *s == Section::All || *s == fence.section)
    }
}

/// A fence the engine writes, and where it lives by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fence {
    pub name: &'static str,
    /// The file under `system/` that gets the fence when no file has it.
    pub file: &'static str,
    pub section: Section,
    /// The heading put above the fence when it is appended (en, de).
    heading: [&'static str; 2],
}

/// Every generated fence of the dossier, in the order they are written.
pub const FENCES: [Fence; 8] = [
    Fence {
        name: "packages.summary",
        file: "packages.md",
        section: Section::Packages,
        heading: ["Summary", "Übersicht"],
    },
    Fence {
        name: "packages.history",
        file: "packages.md",
        section: Section::Packages,
        heading: ["History", "Verlauf"],
    },
    Fence {
        name: "packages.explicit",
        file: "packages.md",
        section: Section::Packages,
        heading: ["Explicit packages", "Explizite Pakete"],
    },
    Fence {
        name: "services.enabled",
        file: "services.md",
        section: Section::Services,
        heading: ["Enabled units", "Aktivierte Units"],
    },
    Fence {
        name: "omarchy.summary",
        file: "omarchy.md",
        section: Section::Omarchy,
        heading: ["Summary", "Übersicht"],
    },
    Fence {
        name: "hardware.summary",
        file: "hardware.md",
        section: Section::Hardware,
        heading: ["Summary", "Übersicht"],
    },
    Fence {
        name: "plugins.list",
        file: "plugins.md",
        section: Section::Plugins,
        heading: ["Installed plugins", "Installierte Plugins"],
    },
    Fence {
        name: "deviations.table",
        file: "deviations.md",
        section: Section::Deviations,
        heading: ["Deviations", "Abweichungen"],
    },
];

/// The title of a dossier file the engine has to create (en, de).
fn title(file: &str) -> [&'static str; 2] {
    match file {
        "packages.md" => ["Packages", "Pakete"],
        "services.md" => ["Services", "Dienste"],
        "omarchy.md" => ["Omarchy", "Omarchy"],
        "hardware.md" => ["Hardware", "Hardware"],
        "plugins.md" => ["Plugins", "Plugins"],
        _ => [
            "Deviations from Omarchy defaults",
            "Abweichungen vom Omarchy-Standard",
        ],
    }
}

fn pick(pair: [&'static str; 2], language: Language) -> &'static str {
    match language {
        Language::En => pair[0],
        Language::De => pair[1],
    }
}

/// The `*.md` files of `system/`, as read and as they will be written.
#[derive(Debug, Clone)]
pub struct Files {
    dir: PathBuf,
    language: Language,
    files: Vec<File>,
    /// Files that could not be read as text: never written, and a fence
    /// one of them may hold is never appended elsewhere (F-550).
    unread: Vec<Unread>,
}

#[derive(Debug, Clone)]
struct File {
    /// For display and the returned paths (`system/<name>`).
    name: String,
    /// Where it is read from and written to (a name that is not UTF-8
    /// stays as it is).
    path: PathBuf,
    /// The bytes on disk; `None` for a file the engine creates.
    old: Option<String>,
    text: String,
}

/// A `system/*.md` file that is not UTF-8 or cannot be read.
#[derive(Debug, Clone)]
struct Unread {
    name: String,
    /// The bytes, lossily decoded, when they could be read: they tell
    /// which fences the file holds. `None`: it may hold any.
    lossy: Option<String>,
}

impl Unread {
    fn may_hold(&self, fence: &str) -> bool {
        self.lossy
            .as_deref()
            .is_none_or(|t| t.contains(&format!("{FENCE_BEGIN}{fence} -->")))
    }
}

impl Files {
    /// Reads every `*.md` directly in `dir`, sorted by name (the order the
    /// index reads fences in: the first fence of a name wins). A file that
    /// is not UTF-8 or cannot be read is skipped and never written (F-550),
    /// as the index reader skips it; the warning is the index
    /// derivation's, which `seldon dossier` runs on the same files first.
    pub fn read(dir: &Path, language: Language) -> anyhow::Result<Self> {
        let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                return Err(anyhow::Error::new(e).context(format!("cannot read {}", dir.display())));
            }
        };
        paths.sort();
        let mut files = Vec::new();
        let mut unread = Vec::new();
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match std::fs::read_to_string(&path) {
                Ok(text) => files.push(File {
                    name,
                    path,
                    old: Some(text.clone()),
                    text,
                }),
                // not UTF-8: the lossy text still shows its fence markers
                Err(e) => unread.push(Unread {
                    lossy: (e.kind() == std::io::ErrorKind::InvalidData)
                        .then(|| std::fs::read(&path).ok())
                        .flatten()
                        .map(|b| String::from_utf8_lossy(&b).into_owned()),
                    name,
                }),
            }
        }
        Ok(Files {
            dir: dir.to_path_buf(),
            language,
            files,
            unread,
        })
    }

    /// The body of the first fence `name`, if a file has one (found the
    /// way [`views::replace_fence`] finds it).
    pub fn body(&self, name: &str) -> Option<String> {
        self.files
            .iter()
            .find_map(|f| views::fence_body(&f.text, name).map(String::from))
    }

    /// Sets the body of `fence` to `content` ([`views::neutralise`]d, so a
    /// value can neither end the fence nor open one, WP-075); `Ok(true)`
    /// when that changed it. A file with a damaged fence of that name
    /// ([`views::fence_damaged`]: a begin marker without an end marker of
    /// its own) is left alone and the fence skipped; the `Err` is the
    /// warning (WP-050). A fence no readable file has is not appended while
    /// its default file or a file that may hold it could not be read
    /// (F-550): the fence is skipped with a warning.
    pub fn set(&mut self, fence: &Fence, content: &str) -> Result<bool, String> {
        debug_assert!(content.is_empty() || content.ends_with('\n'));
        let content = &*views::neutralise(content);
        if let Some(f) = self
            .files
            .iter()
            .find(|f| views::fence_damaged(&f.text, fence.name))
        {
            return Err(format!(
                "system/{}: the {} fence has no end marker of its own; fence kept",
                f.name, fence.name
            ));
        }
        if self.body(fence.name).as_deref() == Some(content) {
            return Ok(false);
        }
        for f in &mut self.files {
            if let Some(text) = views::replace_fence(&f.text, fence.name, content) {
                f.text = text;
                return Ok(true);
            }
        }
        // no file has it: append it to its default file, unless that file
        // or one that may hold the fence could not be read
        if let Some(u) = self.unread.iter().find(|u| u.name == fence.file) {
            return Err(format!(
                "system/{} (the file of the {} fence) could not be read; fence kept",
                u.name, fence.name
            ));
        }
        if let Some(u) = self.unread.iter().find(|u| u.may_hold(fence.name)) {
            return Err(format!(
                "system/{} could not be read and may hold the {} fence; fence kept",
                u.name, fence.name
            ));
        }
        let language = self.language;
        let index = match self.files.iter().position(|f| f.name == fence.file) {
            Some(i) => i,
            None => {
                self.files.push(File {
                    name: fence.file.to_string(),
                    path: self.dir.join(fence.file),
                    old: None,
                    text: format!("# {}\n", pick(title(fence.file), language)),
                });
                self.files.len() - 1
            }
        };
        let f = &mut self.files[index];
        if !f.text.is_empty() && !f.text.ends_with('\n') {
            f.text.push('\n');
        }
        let _ = write!(
            f.text,
            "\n## {}\n\n{FENCE_BEGIN}{} -->\n{content}{FENCE_END}\n",
            pick(fence.heading, language),
            fence.name
        );
        Ok(true)
    }

    /// The files whose bytes changed, as `system/<name>`.
    pub fn changed(&self) -> Vec<String> {
        self.files
            .iter()
            .filter(|f| f.old.as_deref() != Some(f.text.as_str()))
            .map(|f| format!("system/{}", f.name))
            .collect()
    }

    /// Writes every changed file atomically; returns them as `system/<name>`.
    pub fn write(&self) -> anyhow::Result<Vec<String>> {
        let mut written = Vec::new();
        for f in &self.files {
            if f.old.as_deref() == Some(f.text.as_str()) {
                continue;
            }
            sys::write_atomic(&f.path, f.text.as_bytes())?;
            written.push(format!("system/{}", f.name));
        }
        Ok(written)
    }
}

/// When the ledger saw a package installed, and for which case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    /// `YYYY-MM-DD` of the latest `install`.
    pub date: String,
    pub case: Option<String>,
}

/// A config file the ledger knows a case for (a `deviations.table` row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cased {
    pub path: String,
    pub date: String,
    pub case: String,
}

/// What the ledger contributes to the dossier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// Packages whose latest install is not followed by a removal.
    pub installs: BTreeMap<String, Known>,
    /// `ts` of the newest Omarchy `update`.
    pub last_update: Option<String>,
    /// The newest `theme-set` that was not dismissed.
    pub theme: Option<String>,
    /// Config files whose latest event has a case, oldest first; unit
    /// files (`~/.config/systemd/`) belong to `services.enabled` instead.
    pub cased_config: Vec<Cased>,
    /// The case that set up a unit: a user unit file's config event, or
    /// an agent's `systemctl [--user] enable <unit>` with a case.
    pub unit_cases: BTreeMap<(String, Scope), String>,
}

/// Config files under this prefix are systemd units.
const UNIT_PREFIX: &str = "~/.config/systemd/";
const USER_UNIT_PREFIX: &str = "~/.config/systemd/user/";

/// Reads [`Facts`] from the index derivation (folded events: each carries
/// its winning resolution and the case it resolved to, ADR-0021).
pub fn facts(built: &Built) -> Facts {
    let mut f = Facts::default();
    let dismissed = |r: Option<Resolution>| r == Some(Resolution::Dismissed);
    // the latest config event per path, and whether the file existed
    // before the logbook saw it (a file added and removed again is gone)
    let mut config: BTreeMap<String, (usize, bool)> = BTreeMap::new();
    let chrono: Vec<_> = built.folded.iter().rev().collect();
    for (i, ie) in chrono.iter().enumerate() {
        let e = &ie.event;
        match (e.source, e.kind) {
            (Source::Pacman, Kind::Install) => {
                f.installs.insert(
                    e.subject.clone(),
                    Known {
                        date: e.ts.date_naive().to_string(),
                        case: e.case.clone(),
                    },
                );
            }
            (Source::Pacman, Kind::Remove) => {
                f.installs.remove(&e.subject);
            }
            (Source::Omarchy, Kind::Update) => f.last_update = Some(format_ts(&e.ts)),
            (Source::Theme, Kind::ThemeSet) if !dismissed(e.resolution) => {
                f.theme = Some(e.subject.clone());
            }
            (Source::Config, Kind::ConfigAdd | Kind::ConfigChange | Kind::ConfigRemove) => {
                let existed = config
                    .get(&e.subject)
                    .map_or(e.kind != Kind::ConfigAdd, |(_, existed)| *existed);
                config.insert(e.subject.clone(), (i, existed));
            }
            (Source::Agent, Kind::Command) => {
                if let (Some(case), Some(cmd)) = (&e.case, &e.meta.command) {
                    for (unit, scope) in enabled_by(cmd) {
                        f.unit_cases.insert((unit, scope), case.clone());
                    }
                }
            }
            _ => {}
        }
    }
    let mut cased: Vec<(usize, Cased)> = Vec::new();
    for (path, (i, existed)) in &config {
        let e = &chrono[*i].event;
        let gone = e.kind == Kind::ConfigRemove && !existed;
        let Some(case) = e.case.clone().filter(|_| !gone && !dismissed(e.resolution)) else {
            continue;
        };
        if let Some(rest) = path.strip_prefix(USER_UNIT_PREFIX) {
            if e.kind != Kind::ConfigRemove && !rest.is_empty() {
                let (unit, _) = unit_of(path);
                f.unit_cases.insert((unit, Scope::User), case);
            }
            continue;
        }
        if path.starts_with(UNIT_PREFIX) {
            continue;
        }
        cased.push((
            *i,
            Cased {
                path: path.clone(),
                date: e.ts.date_naive().to_string(),
                case,
            },
        ));
    }
    cased.sort_by_key(|(i, _)| *i);
    f.cased_config = cased.into_iter().map(|(_, c)| c).collect();
    f
}

/// The units a recorded command line enables: `systemctl [--user] enable
/// [--now] <unit>…` in any simple command of the line (wrappers such as
/// `sudo` stripped). A bare name gets `.service`.
pub fn enabled_by(command: &str) -> Vec<(String, Scope)> {
    let mut out = Vec::new();
    for segment in pkgcmd::simple_commands(&pkgcmd::parse_shell(command)) {
        let argv = segment.argv();
        let Some((program, args)) = argv.split_first() else {
            continue;
        };
        if program.rsplit('/').next() != Some("systemctl") {
            continue;
        }
        let scope = if args.iter().any(|a| a == "--user") {
            Scope::User
        } else {
            Scope::System
        };
        let mut words = args.iter().filter(|a| !a.starts_with('-'));
        if words.next().map(String::as_str) != Some("enable") {
            continue;
        }
        // a path (`systemctl enable /etc/…/x.service`) names no unit here
        for unit in words.filter(|w| !w.contains('/')) {
            let unit = if unit.contains('.') {
                unit.clone()
            } else {
                format!("{unit}.service")
            };
            out.push((unit, scope));
        }
    }
    out
}

/// `packages.summary`: `- explicit: N`, `- total: N`, `- aur: N`.
pub fn packages_summary(p: &Packages) -> String {
    format!(
        "- explicit: {}\n- total: {}\n- aur: {}\n",
        p.explicit.len(),
        p.total,
        p.aur()
    )
}

/// `packages.history`: the rows as they are, plus a row for `today` when
/// the counts differ from the last row (a row of `today` is replaced).
pub fn packages_history(body: Option<&str>, today: &str, p: &Packages) -> String {
    const HEAD: &str = "| date | explicit | total |\n|---|---|---|\n";
    let row = format!("| {today} | {} | {} |", p.explicit.len(), p.total);
    let body = body.filter(|b| has_table_separator(b)).unwrap_or(HEAD);
    let mut lines: Vec<&str> = body.lines().collect();
    let rows = fence_table(body);
    // the table line of the last row: header and separator come first
    let last_line = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with('|'))
        .map(|(i, _)| i)
        .nth(rows.len() + 1)
        .filter(|_| !rows.is_empty());
    if let (Some(last), Some(i)) = (rows.last(), last_line) {
        let same = |k: &str, v: usize| last.get(k).is_some_and(|x| *x == v.to_string());
        if same("explicit", p.explicit.len()) && same("total", p.total) {
            return ensure_newline(body);
        }
        if last.get("date").is_some_and(|d| d == today) {
            lines[i] = &row;
            return lines.join("\n") + "\n";
        }
    }
    let mut out = ensure_newline(body);
    out.push_str(&row);
    out.push('\n');
    out
}

fn ensure_newline(s: &str) -> String {
    if s.is_empty() || s.ends_with('\n') {
        s.to_string()
    } else {
        format!("{s}\n")
    }
}

/// `packages.explicit`: one line per explicit package, sorted:
/// `- <name> · repo|aur · omarchy-base|user · since <date> [[C-…]]` when
/// the ledger saw the install, else `… · pre-logbook`. The class is
/// `omarchy-base` when Omarchy's package lists name the package.
pub fn packages_explicit(p: &Packages, known: &BTreeMap<String, Known>) -> String {
    let mut t = String::new();
    for name in &p.explicit {
        let origin = if p.foreign.contains(name) {
            "aur"
        } else {
            "repo"
        };
        let class = if p.omarchy.contains(name) {
            OMARCHY_BASE
        } else {
            USER
        };
        let _ = write!(t, "- {name} · {origin} · {class} · ");
        match known.get(name) {
            Some(k) => {
                let _ = write!(t, "since {}", k.date);
                if let Some(case) = &k.case {
                    let _ = write!(t, " [[{case}]]");
                }
            }
            None => t.push_str(PRE_LOGBOOK),
        }
        t.push('\n');
    }
    t
}

/// The mark of a package the ledger does not know.
pub const PRE_LOGBOOK: &str = "pre-logbook";
/// The class of a package Omarchy's package lists name.
pub const OMARCHY_BASE: &str = "omarchy-base";
/// The class of every other explicit package: the user's own addition.
pub const USER: &str = "user";

/// An entry of the `packages.explicit` fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explicit {
    pub name: String,
    pub aur: bool,
    /// Class `omarchy-base` (a line without a class, as WP-035 wrote
    /// them, counts as `user`).
    pub omarchy: bool,
    pub pre_logbook: bool,
    /// The name passed [`is_package_name`](crate::rebuild::shell_arg::is_package_name);
    /// one that fails is kept so the
    /// rebuild can list it as not reproduced.
    pub valid_name: bool,
}

/// Reads a `packages.explicit` body (see [`packages_explicit`]); lines in
/// another shape are skipped.
pub fn parse_explicit(body: &str) -> Vec<Explicit> {
    body.lines()
        .filter_map(|l| {
            let mut parts = l.strip_prefix("- ")?.split(" · ").map(str::trim);
            let (name, origin) = (parts.next()?, parts.next()?);
            let (omarchy, since) = match parts.next()? {
                OMARCHY_BASE => (true, parts.next()?),
                USER => (false, parts.next()?),
                since => (false, since),
            };
            if name.is_empty() {
                return None;
            }
            Some(Explicit {
                valid_name: crate::rebuild::shell_arg::is_package_name(name),
                name: name.to_string(),
                aur: match origin {
                    "aur" => true,
                    "repo" => false,
                    _ => return None,
                },
                omarchy,
                pre_logbook: since == PRE_LOGBOOK,
            })
        })
        .collect()
}

/// `services.enabled`: system units, then user units, by name. The case is
/// the ledger's, else the one the existing row had, else `—`.
pub fn services_enabled(
    units: &[(String, Scope)],
    cases: &BTreeMap<(String, Scope), String>,
    body: Option<&str>,
) -> String {
    let old: BTreeMap<(String, String), String> = body
        .map(fence_table)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            let case = r.get("case")?.trim().to_string();
            let known = !case.is_empty() && case != "—" && case != "-";
            if !known {
                return None;
            }
            Some(((r.get("unit")?.clone(), r.get("scope")?.clone()), case))
        })
        .collect();
    let mut sorted: Vec<&(String, Scope)> = units.iter().collect();
    sorted.sort_by(|a, b| (a.1, &a.0).cmp(&(b.1, &b.0)));
    sorted.dedup();
    let mut t = String::from("| unit | scope | case |\n|---|---|---|\n");
    for (unit, scope) in sorted {
        let case = cases
            .get(&(unit.clone(), *scope))
            .map(|c| format!("[[{c}]]"))
            .or_else(|| {
                old.get(&(unit.clone(), scope.as_str().to_string()))
                    .cloned()
            })
            .unwrap_or_else(|| "—".to_string());
        let _ = writeln!(t, "| {unit} | {} | {case} |", scope.as_str());
    }
    t
}

/// `plugins.list`: one row per plugin, by id.
pub fn plugins_list(listed: &[Listed]) -> String {
    let yes = |b: bool| if b { "yes" } else { "no" };
    let mut t = String::from("| id | enabled | firstParty | clonedFrom |\n|---|---|---|---|\n");
    for p in listed {
        let from = p.cloned_from.trim();
        let from = if from.is_empty() || from.contains('|') {
            "—"
        } else {
            from
        };
        let _ = writeln!(
            t,
            "| {} | {} | {} | {from} |",
            p.id,
            yes(p.enabled),
            yes(p.first_party)
        );
    }
    t
}

/// A `- key: value` fence. The engine owns the keys of `pairs`: each old
/// line of such a key gets the new value (or keeps its own when there is
/// none), in place. Every other line of the old body (a hand-written
/// `- gpu: …`) is kept verbatim. Keys of `pairs` the body lacks are
/// appended in `pairs` order; a key without any value is left out.
pub fn key_values(pairs: &[(&str, Option<String>)], body: Option<&str>) -> String {
    let key_of = |line: &str| {
        let (k, _) = line.strip_prefix("- ")?.split_once(": ")?;
        Some(k.to_string())
    };
    let mut t = String::new();
    let mut done: Vec<&str> = Vec::new();
    for line in body.unwrap_or_default().lines() {
        let owned = key_of(line).and_then(|k| pairs.iter().find(|(p, _)| *p == k));
        match owned {
            Some((key, value)) if !done.contains(key) => {
                done.push(key);
                match value {
                    Some(v) => {
                        let _ = writeln!(t, "- {key}: {v}");
                    }
                    None => {
                        let _ = writeln!(t, "{line}");
                    }
                }
            }
            // a second line of an engine key is stale: dropped
            Some(_) => {}
            None => {
                let _ = writeln!(t, "{line}");
            }
        }
    }
    for (key, value) in pairs {
        if let Some(v) = value.as_ref().filter(|_| !done.contains(key)) {
            let _ = writeln!(t, "- {key}: {v}");
        }
    }
    t
}

/// `omarchy.summary`: version, theme, last update.
pub fn omarchy_summary(
    version: Option<String>,
    theme: Option<String>,
    facts: &Facts,
    body: Option<&str>,
) -> String {
    key_values(
        &[
            ("version", version),
            ("theme", theme.or_else(|| facts.theme.clone())),
            ("lastUpdate", facts.last_update.clone()),
        ],
        body,
    )
}

/// `hardware.summary`: the four keys [`query::hardware`] reads; other
/// lines of the old body (hand-written `gpu`, `displays`, `disk`) stay.
pub fn hardware_summary(hw: &Hardware, body: Option<&str>) -> String {
    key_values(
        &[
            ("cpu", hw.cpu.clone()),
            ("memory", hw.memory.clone()),
            ("machine", hw.machine.clone()),
            ("rootfs", hw.rootfs.clone()),
        ],
        body,
    )
}

/// `deviations.table`: the existing lines byte for byte (the user's
/// reasons), plus a row for each cased config file without one, keyed by
/// path. The reason of a new row is empty: the user writes it. A row whose
/// case cell is empty (blank, `—` or `-`) gets the case of its path when
/// that event is not older than the row's date; only that cell changes.
pub fn deviations_table(body: Option<&str>, cased: &[Cased]) -> String {
    const HEAD: &str = "| path | reason | date | case |\n|---|---|---|---|\n";
    let body = body.filter(|b| has_table_separator(b)).unwrap_or(HEAD);
    let listed: Vec<String> = fence_table(body)
        .into_iter()
        .filter_map(|r| r.get("path").cloned())
        .collect();
    // the header is the first table line, the separator the second; a case
    // is filled in only when the columns are path first and case last
    let mut rows = body.split_inclusive('\n').filter(|l| l.starts_with('|'));
    let fillable = rows.next().is_some_and(|head| {
        let cells = cells(head);
        cells.first() == Some(&"path") && cells.last() == Some(&"case")
    });
    let mut t = String::new();
    let mut table = 0;
    for line in body.split_inclusive('\n') {
        if line.starts_with('|') {
            table += 1;
        }
        match (fillable && table > 2 && line.starts_with('|'))
            .then(|| fill_case(line, cased))
            .flatten()
        {
            Some(filled) => t.push_str(&filled),
            None => t.push_str(line),
        }
    }
    let mut t = ensure_newline(&t);
    for c in cased {
        if is_listed(&listed, &c.path) || c.path.contains('|') {
            continue;
        }
        let _ = writeln!(t, "| {} |  | {} | [[{}]] |", c.path, c.date, c.case);
    }
    t
}

/// Whether `cell` (a path cell of the fence body) is `path`: as written
/// now, [`views::neutralise`]d like every body [`Files::set`] writes
/// (WP-075 review), or as an older run wrote it.
fn same_path(cell: &str, path: &str) -> bool {
    cell == path || cell == views::neutralise(path)
}

fn is_listed(listed: &[String], path: &str) -> bool {
    listed.iter().any(|l| same_path(l, path))
}

/// The trimmed cells of a table line.
fn cells(line: &str) -> Vec<&str> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

/// `line` (a `deviations.table` row, line ending included) with its empty
/// last cell set to the case `cased` knows for its path; `None` when the
/// cell is not empty, the path has no case, or the case event is older
/// than the row's date. The path is the first cell and the date the one
/// before the case, so a reason that contains `|` does not matter. Every
/// other byte of the line stays.
fn fill_case(line: &str, cased: &[Cased]) -> Option<String> {
    let (core, tail) = line.split_at(line.trim_end().len());
    let inner = core.strip_prefix('|')?.strip_suffix('|')?;
    let (path, _) = inner.split_once('|')?;
    let c = cased.iter().find(|c| same_path(path.trim(), &c.path))?;
    let start = inner.rfind('|')?;
    let case = inner[start + 1..].trim();
    if !(case.is_empty() || case == "—" || case == "-") {
        return None;
    }
    let before = &inner[..start];
    let date = before.rsplit('|').next().unwrap_or_default().trim();
    let is_date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok();
    // path, date and case are three different cells
    if before.matches('|').count() < 1 || (is_date && c.date.as_str() < date) {
        return None;
    }
    Some(format!("|{before}| [[{}]] |{tail}", c.case))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkgs(explicit: &[&str], foreign: &[&str], total: usize) -> Packages {
        Packages {
            explicit: explicit.iter().map(|s| s.to_string()).collect(),
            foreign: foreign.iter().map(|s| s.to_string()).collect(),
            total,
            omarchy: Default::default(),
        }
    }

    #[test]
    fn history_appends_replaces_today_or_keeps() {
        let body = "| date | explicit | total |\n|---|---|---|\n| 2026-09-01 | 2 | 10 |\n";
        let p = pkgs(&["a", "b"], &[], 10);
        assert_eq!(packages_history(Some(body), "2026-10-01", &p), body);
        let p3 = pkgs(&["a", "b", "c"], &[], 11);
        let appended = packages_history(Some(body), "2026-10-01", &p3);
        assert_eq!(appended, format!("{body}| 2026-10-01 | 3 | 11 |\n"));
        let p4 = pkgs(&["a", "b", "c", "d"], &[], 12);
        assert_eq!(
            packages_history(Some(&appended), "2026-10-01", &p4),
            format!("{body}| 2026-10-01 | 4 | 12 |\n"),
            "today's row is replaced"
        );
        assert_eq!(
            packages_history(Some(""), "2026-10-01", &p),
            "| date | explicit | total |\n|---|---|---|\n| 2026-10-01 | 2 | 10 |\n"
        );
    }

    #[test]
    fn explicit_lines_round_trip() {
        let lists = std::collections::BTreeSet::from(["btop".to_string(), "yay".to_string()]);
        let p = pkgs(&["btop", "yay", "zed"], &["yay", "yay-debug"], 40).classify(&lists);
        let known = BTreeMap::from([
            (
                "zed".to_string(),
                Known {
                    date: "2026-10-01".into(),
                    case: Some("C-2026-004".into()),
                },
            ),
            (
                "btop".to_string(),
                Known {
                    date: "2026-09-03".into(),
                    case: None,
                },
            ),
        ]);
        let body = packages_explicit(&p, &known);
        assert_eq!(
            body,
            "- btop · repo · omarchy-base · since 2026-09-03\n- yay · aur · omarchy-base · pre-logbook\n- zed · repo · user · since 2026-10-01 [[C-2026-004]]\n"
        );
        let parsed = parse_explicit(&body);
        assert_eq!(parsed.len(), 3);
        assert_eq!(
            parsed[1],
            Explicit {
                name: "yay".into(),
                aur: true,
                omarchy: true,
                pre_logbook: true,
                valid_name: true
            }
        );
        assert!(!parsed[2].pre_logbook);
        assert!(!parsed[2].omarchy);
        // a WP-035 line without a class is the user's
        assert_eq!(
            parse_explicit("- yay · aur · pre-logbook\n- zed · repo · since 2026-10-01\n"),
            [
                Explicit {
                    name: "yay".into(),
                    aur: true,
                    omarchy: false,
                    pre_logbook: true,
                    valid_name: true
                },
                Explicit {
                    name: "zed".into(),
                    aur: false,
                    omarchy: false,
                    pre_logbook: false,
                    valid_name: true
                }
            ]
        );
        assert!(parse_explicit("- yay · aur · user\n- x · git · user · pre-logbook\n").is_empty());
        // a name that is not a package name is kept, marked
        let odd =
            parse_explicit("- -x · repo · user · pre-logbook\n- a b · aur · user · pre-logbook\n");
        assert_eq!(odd.len(), 2);
        assert!(odd.iter().all(|p| !p.valid_name), "{odd:?}");
        assert_eq!(
            packages_summary(&p),
            "- explicit: 3\n- total: 40\n- aur: 2\n"
        );
    }

    #[test]
    fn services_take_the_ledger_case_then_the_old_row() {
        let units = vec![
            ("ollama.service".to_string(), Scope::User),
            ("tailscaled.service".to_string(), Scope::System),
            ("elephant.service".to_string(), Scope::User),
        ];
        let cases = BTreeMap::from([(
            ("ollama.service".to_string(), Scope::User),
            "C-2026-009".to_string(),
        )]);
        let old = "| unit | scope | case |\n|---|---|---|\n| tailscaled.service | system | [[C-2026-008]] |\n| gone.service | user | [[C-1]] |\n";
        assert_eq!(
            services_enabled(&units, &cases, Some(old)),
            "| unit | scope | case |\n|---|---|---|\n| tailscaled.service | system | [[C-2026-008]] |\n| elephant.service | user | — |\n| ollama.service | user | [[C-2026-009]] |\n"
        );
    }

    #[test]
    fn enable_commands_name_their_units() {
        assert_eq!(
            enabled_by("sudo systemctl enable --now tailscaled && tailscale up"),
            [("tailscaled.service".to_string(), Scope::System)]
        );
        assert_eq!(
            enabled_by("systemctl --user enable ollama.service elephant.socket"),
            [
                ("ollama.service".to_string(), Scope::User),
                ("elephant.socket".to_string(), Scope::User)
            ]
        );
        assert!(enabled_by("systemctl --user status ollama").is_empty());
        assert!(enabled_by("systemctl is-enabled ollama").is_empty());
        assert!(
            enabled_by("sudo systemctl enable /etc/systemd/system/x.service").is_empty(),
            "a path is not a unit name"
        );
    }

    #[test]
    fn deviations_keep_rows_and_add_cased_paths() {
        let old = "| path | reason | date | case |\n|---|---|---|---|\n| ~/.bashrc | mise | 2026-09-01 | [[C-0]] |\n";
        let cased = [
            Cased {
                path: "~/.bashrc".into(),
                date: "2026-10-01".into(),
                case: "C-1".into(),
            },
            Cased {
                path: "~/.config/hypr/a.conf".into(),
                date: "2026-10-01".into(),
                case: "C-2".into(),
            },
        ];
        assert_eq!(
            deviations_table(Some(old), &cased),
            format!("{old}| ~/.config/hypr/a.conf |  | 2026-10-01 | [[C-2]] |\n"),
            "a row with a case keeps it"
        );
        assert_eq!(deviations_table(Some(old), &[]), old);
    }

    #[test]
    fn editor_formatted_tables_keep_their_rows() {
        // Obsidian's table editor pads the separator: `| --- | --- |`
        let old = "| path | reason | date | case |\n| --- | --- | --- | --- |\n| ~/.bashrc | mise | 2026-09-01 | — |\n";
        let cased = [Cased {
            path: "~/.zshrc".into(),
            date: "2026-10-01".into(),
            case: "C-2".into(),
        }];
        assert_eq!(
            deviations_table(Some(old), &cased),
            format!("{old}| ~/.zshrc |  | 2026-10-01 | [[C-2]] |\n")
        );
        let history = "| date | explicit | total |\n|:---|---:|---:|\n| 2026-09-01 | 2 | 10 |\n";
        assert_eq!(
            packages_history(Some(history), "2026-10-01", &pkgs(&["a", "b"], &[], 10)),
            history,
            "same counts: unchanged, the old row kept"
        );
        assert_eq!(
            packages_history(Some(history), "2026-10-01", &pkgs(&["a"], &[], 10)),
            format!("{history}| 2026-10-01 | 1 | 10 |\n")
        );
    }

    #[test]
    fn deviations_fill_only_an_empty_case_cell() {
        let cased = |path: &str, date: &str| Cased {
            path: path.into(),
            date: date.into(),
            case: "C-7".into(),
        };
        let head = "| path | reason | date | case |\n|---|---|---|---|\n";
        let old = format!(
            "{head}| ~/.bashrc | mise | a|b  | 2026-09-01 |  —  |\n|  ~/.zshrc |  | 2026-09-01 |  |  \n| ~/.vimrc | x | 2026-09-01 | - |\n| ~/.inputrc | y | 2026-10-05 | — |\n| ~/.profile | z | 2026-09-01 | [[C-3]] |"
        );
        let all = [
            cased("~/.bashrc", "2026-10-01"),
            cased("~/.zshrc", "2026-09-01"),
            cased("~/.vimrc", "2026-10-01"),
            // older than the row: not the row's case
            cased("~/.inputrc", "2026-10-01"),
            cased("~/.profile", "2026-10-01"),
        ];
        let new = deviations_table(Some(&old), &all);
        assert_eq!(
            new,
            format!(
                "{head}| ~/.bashrc | mise | a|b  | 2026-09-01 | [[C-7]] |\n|  ~/.zshrc |  | 2026-09-01 | [[C-7]] |  \n| ~/.vimrc | x | 2026-09-01 | [[C-7]] |\n| ~/.inputrc | y | 2026-10-05 | — |\n| ~/.profile | z | 2026-09-01 | [[C-3]] |\n"
            )
        );
        assert_eq!(deviations_table(Some(&new), &all), new, "idempotent");
        // another column order: the row is listed, and nothing is filled
        let odd = "| case | path |\n|---|---|\n| — | ~/.bashrc |\n";
        assert_eq!(deviations_table(Some(odd), &all[..1]), odd);
    }

    #[test]
    fn key_values_keep_old_values_for_missing_keys() {
        let old =
            "- version: 4.0.7-1\n- theme: tokyo-night\n- lastUpdate: 2026-10-01T09:21:00+02:00\n";
        let facts = Facts::default();
        assert_eq!(omarchy_summary(None, None, &facts, Some(old)), old);
        assert_eq!(
            omarchy_summary(Some("4.0.8-1".into()), None, &facts, Some(old)),
            old.replace("4.0.7-1", "4.0.8-1")
        );
    }

    #[test]
    fn hardware_keeps_hand_written_lines() {
        let old = "- cpu: old cpu\n- memory: 64 GiB\n- gpu: Intel Arc B580\n- displays: 2 × 2560×1440 @ 144 Hz\n- disk: NVMe 2 TB, btrfs\n";
        let hw = Hardware {
            cpu: Some("Intel(R) Core(TM) i7-14700K".into()),
            memory: Some("63 GiB".into()),
            machine: None,
            rootfs: Some("btrfs".into()),
        };
        assert_eq!(
            hardware_summary(&hw, Some(old)),
            "- cpu: Intel(R) Core(TM) i7-14700K\n- memory: 63 GiB\n- gpu: Intel Arc B580\n- displays: 2 × 2560×1440 @ 144 Hz\n- disk: NVMe 2 TB, btrfs\n- rootfs: btrfs\n"
        );
        let once = hardware_summary(&hw, Some(old));
        assert_eq!(hardware_summary(&hw, Some(&once)), once, "idempotent");
        // a stale duplicate of an engine key is dropped
        assert_eq!(
            hardware_summary(&hw, Some("- cpu: a\n- cpu: b\n")),
            "- cpu: Intel(R) Core(TM) i7-14700K\n- memory: 63 GiB\n- rootfs: btrfs\n"
        );
    }

    #[test]
    fn a_damaged_marker_elsewhere_does_not_hide_the_fence() {
        let dir = std::env::temp_dir().join(format!("seldon-dossier-dmg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // an unclosed fence above: a sequential scan reads it up to the
        // real fence's end marker and would not see `packages.summary`
        let text = "# P\n<!-- seldon:begin notes -->\nkaputt\n\n<!-- seldon:begin packages.summary -->\n- explicit: 1\n<!-- seldon:end -->\n";
        std::fs::write(dir.join("packages.md"), text).unwrap();
        let mut files = Files::read(&dir, Language::En).unwrap();
        let fence = *FENCES
            .iter()
            .find(|f| f.name == "packages.summary")
            .unwrap();
        assert_eq!(
            files.body("packages.summary").as_deref(),
            Some("- explicit: 1\n")
        );
        assert!(!files.set(&fence, "- explicit: 1\n").unwrap());
        assert!(files.set(&fence, "- explicit: 2\n").unwrap());
        assert_eq!(files.changed(), ["system/packages.md"]);
        files.write().unwrap();
        let written = std::fs::read_to_string(dir.join("packages.md")).unwrap();
        assert_eq!(written, text.replace("explicit: 1", "explicit: 2"));
        let mut again = Files::read(&dir, Language::En).unwrap();
        assert!(!again.set(&fence, "- explicit: 2\n").unwrap());
        assert!(again.changed().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// F-550: a file that is not UTF-8 is skipped (the index derivation
    /// warns) and never written; the fences of the readable files are
    /// still set. A missing
    /// fence is not appended while its default file, or a file that holds
    /// its begin marker, could not be read.
    #[test]
    fn a_file_that_is_not_utf8_is_skipped_and_never_written() {
        let dir =
            std::env::temp_dir().join(format!("seldon-dossier-latin1-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fence = |name: &str| *FENCES.iter().find(|f| f.name == name).unwrap();
        let notes: &[u8] = b"# Notizen\n\nGr\xfc\xdfe\n";
        // a default fence file in Latin-1, without its fence
        let plugins: &[u8] = b"# Plugins\n\nMein Gr\xfc\xdf.\n";
        // a user file in Latin-1 that holds the hardware fence
        let held: &[u8] = b"# Rechner\n<!-- seldon:begin hardware.summary -->\n- cpu: \xe4\n<!-- seldon:end -->\n";
        let packages =
            "# P\n<!-- seldon:begin packages.summary -->\n- explicit: 1\n<!-- seldon:end -->\n";
        std::fs::write(dir.join("notes.md"), notes).unwrap();
        std::fs::write(dir.join("plugins.md"), plugins).unwrap();
        std::fs::write(dir.join("rechner.md"), held).unwrap();
        std::fs::write(dir.join("packages.md"), packages).unwrap();

        let mut files = Files::read(&dir, Language::En).unwrap();
        let unread: Vec<&str> = files.unread.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(unread, ["notes.md", "plugins.md", "rechner.md"]);
        assert_eq!(
            files.set(&fence("packages.summary"), "- explicit: 2\n"),
            Ok(true)
        );
        // appended to a file that was read (and created)
        assert_eq!(files.set(&fence("services.enabled"), "- a\n"), Ok(true));
        assert_eq!(
            files.set(&fence("plugins.list"), "- x\n"),
            Err(
                "system/plugins.md (the file of the plugins.list fence) could not be read; fence kept"
                    .into()
            )
        );
        assert_eq!(
            files.set(&fence("hardware.summary"), "- cpu: x\n"),
            Err(
                "system/rechner.md could not be read and may hold the hardware.summary fence; fence kept"
                    .into()
            )
        );
        assert_eq!(
            files.changed(),
            ["system/packages.md", "system/services.md"]
        );
        files.write().unwrap();
        assert_eq!(std::fs::read(dir.join("notes.md")).unwrap(), notes);
        assert_eq!(std::fs::read(dir.join("plugins.md")).unwrap(), plugins);
        assert_eq!(std::fs::read(dir.join("rechner.md")).unwrap(), held);
        assert!(!dir.join("hardware.md").exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("packages.md")).unwrap(),
            packages.replace("explicit: 1", "explicit: 2")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file that cannot be read at all may hold any fence: none is
    /// appended, the fences of the other files are set.
    #[test]
    fn an_unreadable_file_blocks_only_appending() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir =
            std::env::temp_dir().join(format!("seldon-dossier-noread-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fence = |name: &str| *FENCES.iter().find(|f| f.name == name).unwrap();
        let secret = dir.join("secret.md");
        std::fs::write(&secret, "x\n").unwrap();
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&secret).is_ok() {
            // root reads it anyway
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let packages =
            "<!-- seldon:begin packages.summary -->\n- explicit: 1\n<!-- seldon:end -->\n";
        std::fs::write(dir.join("packages.md"), packages).unwrap();
        let mut files = Files::read(&dir, Language::En).unwrap();
        assert_eq!(files.unread.len(), 1);
        assert_eq!(files.unread[0].name, "secret.md");
        assert_eq!(files.unread[0].lossy, None);
        assert_eq!(
            files.set(&fence("packages.summary"), "- explicit: 2\n"),
            Ok(true)
        );
        assert!(
            files
                .set(&fence("hardware.summary"), "- cpu: x\n")
                .unwrap_err()
                .starts_with("system/secret.md could not be read and may hold ")
        );
        assert_eq!(files.changed(), ["system/packages.md"]);
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file whose name is not UTF-8 is written back to that name, not to
    /// its lossy spelling.
    #[test]
    fn a_name_that_is_not_utf8_is_written_back_to_itself() {
        use std::os::unix::ffi::OsStrExt as _;
        let dir = std::env::temp_dir().join(format!("seldon-dossier-name-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fence = |name: &str| *FENCES.iter().find(|f| f.name == name).unwrap();
        let path = dir.join(std::ffi::OsStr::from_bytes(b"ger\xe4te.md"));
        std::fs::write(
            &path,
            "<!-- seldon:begin hardware.summary -->\n- cpu: a\n<!-- seldon:end -->\n",
        )
        .unwrap();
        let mut files = Files::read(&dir, Language::En).unwrap();
        assert!(files.unread.is_empty());
        assert_eq!(
            files.set(&fence("hardware.summary"), "- cpu: b\n"),
            Ok(true)
        );
        files.write().unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "<!-- seldon:begin hardware.summary -->\n- cpu: b\n<!-- seldon:end -->\n"
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WP-075 (WP-065's gap): a value with a fence marker can neither end
    /// the fence nor open one; the next run sees the same body.
    #[test]
    fn fence_bodies_are_neutralised() {
        let dir =
            std::env::temp_dir().join(format!("seldon-dossier-marker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fence = |name: &str| *FENCES.iter().find(|f| f.name == name).unwrap();
        let after = "\n## Mine\n\nMeine Notiz.\n";
        std::fs::write(
            dir.join("deviations.md"),
            format!("# D\n<!-- seldon:begin deviations.table -->\n<!-- seldon:end -->\n{after}"),
        )
        .unwrap();
        let content = "| path | reason | date | case |\n|---|---|---|---|\n\
                       | ~/x | a <!-- seldon:end --> b | 2026-10-01 | — |\n\
                       | ~/y | <!-- seldon:begin hardware.summary --> | 2026-10-01 | — |\n";
        let mut files = Files::read(&dir, Language::En).unwrap();
        assert_eq!(files.set(&fence("deviations.table"), content), Ok(true));
        files.write().unwrap();
        let text = std::fs::read_to_string(dir.join("deviations.md")).unwrap();
        assert_eq!(
            crate::index::load::fences(&text),
            vec![(
                "deviations.table".to_string(),
                views::neutralise(content).into_owned()
            )]
        );
        assert!(
            text.ends_with(&format!("<!-- seldon:end -->\n{after}")),
            "{text}"
        );
        let mut again = Files::read(&dir, Language::En).unwrap();
        assert_eq!(again.set(&fence("deviations.table"), content), Ok(false));
        assert_eq!(
            again.set(&fence("hardware.summary"), "- cpu: x\n"),
            Ok(true)
        );
        assert_eq!(again.changed(), ["system/hardware.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WP-050: a fence of the same name without its own end marker is
    /// skipped with a warning, never answered with a second fence.
    #[test]
    fn a_fence_without_its_end_is_skipped_not_appended() {
        let dir = std::env::temp_dir().join(format!("seldon-dossier-open-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fence = |name: &str| *FENCES.iter().find(|f| f.name == name).unwrap();
        // no end marker at all
        let open = "# P\n<!-- seldon:begin packages.summary -->\n- explicit: 1\nMy notes.\n";
        // the end that follows belongs to the next fence
        let borrowed = "# S\n<!-- seldon:begin services.enabled -->\nMine.\n\n\
                        <!-- seldon:begin plugins.list -->\n- x\n<!-- seldon:end -->\n";
        std::fs::write(dir.join("packages.md"), open).unwrap();
        std::fs::write(dir.join("services.md"), borrowed).unwrap();
        let mut files = Files::read(&dir, Language::En).unwrap();
        assert_eq!(
            files.set(&fence("packages.summary"), "- explicit: 2\n"),
            Err(
                "system/packages.md: the packages.summary fence has no end marker of its own; fence kept"
                    .into()
            )
        );
        assert!(
            files
                .set(&fence("services.enabled"), "- a.service\n")
                .unwrap_err()
                .starts_with("system/services.md: the services.enabled fence ")
        );
        // the intact fence in the same file is still written
        assert_eq!(files.set(&fence("plugins.list"), "- y\n"), Ok(true));
        assert_eq!(files.changed(), ["system/services.md"]);
        files.write().unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("packages.md")).unwrap(),
            open
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("services.md")).unwrap(),
            borrowed.replace("- x\n", "- y\n")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn files_replace_bodies_and_append_missing_fences() {
        let dir = std::env::temp_dir().join(format!("seldon-dossier-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let packages = "# Pakete\nMeine Notiz.\n\n<!-- seldon:begin packages.summary -->\n- explicit: 1\n<!-- seldon:end -->\nEnde ohne Zeilenumbruch";
        std::fs::write(dir.join("packages.md"), packages).unwrap();
        let mut files = Files::read(&dir, Language::De).unwrap();
        let fence = |name: &str| *FENCES.iter().find(|f| f.name == name).unwrap();
        assert!(
            !files
                .set(&fence("packages.summary"), "- explicit: 1\n")
                .unwrap()
        );
        assert!(files.changed().is_empty());
        assert!(
            files
                .set(&fence("packages.summary"), "- explicit: 2\n")
                .unwrap()
        );
        assert!(
            files
                .set(&fence("packages.explicit"), "- zed · repo · pre-logbook\n")
                .unwrap()
        );
        assert!(files.set(&fence("hardware.summary"), "- cpu: x\n").unwrap());
        assert_eq!(
            files.changed(),
            ["system/packages.md", "system/hardware.md"]
        );
        files.write().unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("packages.md")).unwrap(),
            "# Pakete\nMeine Notiz.\n\n<!-- seldon:begin packages.summary -->\n- explicit: 2\n<!-- seldon:end -->\nEnde ohne Zeilenumbruch\n\n## Explizite Pakete\n\n<!-- seldon:begin packages.explicit -->\n- zed · repo · pre-logbook\n<!-- seldon:end -->\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("hardware.md")).unwrap(),
            "# Hardware\n\n## Übersicht\n\n<!-- seldon:begin hardware.summary -->\n- cpu: x\n<!-- seldon:end -->\n"
        );
        let again = Files::read(&dir, Language::De).unwrap();
        assert_eq!(
            again.body("packages.explicit").as_deref(),
            Some("- zed · repo · pre-logbook\n")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
