//! What `seldon dossier` reads from the host. Read-only, always (AGENTS.md
//! §6): the package manager is only ever asked `-Qqe`, `-Qqm` and `-Q`,
//! systemd only `list-unit-files --state=enabled`, the Omarchy CLI only
//! `plugin list --json` and `omarchy-version`, and the hardware comes from
//! files under `/proc` and `/sys` (no `lscpu`). Omarchy's own package
//! lists (`omarchy-base.packages`, `omarchy-other.packages`) are plain file
//! reads. Every program is a fixed name with a fixed argument list
//! ([`crate::sys::run`], no shell).
//!
//! The programs and the file root can be pointed elsewhere for tests and
//! demos: `SELDON_PACMAN`, `SELDON_OMARCHY`, `SELDON_OMARCHY_VERSION`,
//! `SELDON_THEME_FILE` (shared with the collectors), `SELDON_SYSTEMCTL`
//! `SELDON_HARDWARE_ROOT` (default `/`) and `SELDON_OMARCHY_PACKAGES` (the
//! directory of Omarchy's package lists, default `$OMARCHY_PATH/install`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::collectors::plugins::{self, Listed};
use crate::collectors::theme::Theme;
use crate::collectors::{RUN_TIMEOUT, Sources, omarchy};
use crate::model::event::SUBJECT_MAX;
use crate::redact::Redactor;
use crate::sys::{self, Run};

/// Where the dossier's queries go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hosts {
    pub sources: Sources,
    /// `SELDON_SYSTEMCTL`, default `systemctl`.
    pub systemctl: String,
    /// `SELDON_HARDWARE_ROOT`, default `/`: `proc/…` and `sys/…` are read
    /// below it.
    pub hardware_root: PathBuf,
    /// `SELDON_OMARCHY_PACKAGES`, default `$OMARCHY_PATH/install` (and
    /// `/usr/share/omarchy/install` without `OMARCHY_PATH`,
    /// [`sys::omarchy_path`]): the directory of Omarchy's package lists.
    pub omarchy_packages: PathBuf,
}

impl Hosts {
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        Hosts {
            sources: Sources::from_env(),
            systemctl: var("SELDON_SYSTEMCTL").unwrap_or_else(|| "systemctl".into()),
            hardware_root: var("SELDON_HARDWARE_ROOT").map_or_else(|| "/".into(), PathBuf::from),
            omarchy_packages: var("SELDON_OMARCHY_PACKAGES")
                .map_or_else(|| sys::omarchy_path().join("install"), PathBuf::from),
        }
    }
}

/// The installed packages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Packages {
    /// Explicitly installed, sorted (`-Qqe`).
    pub explicit: Vec<String>,
    /// Not in any sync database: AUR or local builds (`-Qqm`).
    pub foreign: BTreeSet<String>,
    /// Every installed package (`-Q`).
    pub total: usize,
    /// The explicit packages Omarchy's package lists name (origin class
    /// `omarchy-base`; [`Packages::classify`]).
    pub omarchy: BTreeSet<String>,
}

impl Packages {
    /// The names through `redactor` (the user's `[redaction]` patterns).
    pub fn redacted(self, redactor: &Redactor) -> Self {
        let explicit: BTreeSet<String> = self.explicit.iter().map(|n| redactor.redact(n)).collect();
        Packages {
            explicit: explicit.into_iter().collect(),
            foreign: self.foreign.iter().map(|n| redactor.redact(n)).collect(),
            total: self.total,
            omarchy: self.omarchy.iter().map(|n| redactor.redact(n)).collect(),
        }
    }

    /// Marks the explicit packages `lists` names as Omarchy's own.
    pub fn classify(mut self, lists: &BTreeSet<String>) -> Self {
        self.omarchy = self
            .explicit
            .iter()
            .filter(|n| lists.contains(*n))
            .cloned()
            .collect();
        self
    }

    /// Foreign packages, explicit or not (the `aur` count).
    pub fn aur(&self) -> usize {
        self.foreign.len()
    }
}

/// `pacman -Qqe`, `-Qqm` and `-Q`: three queries, nothing else.
pub fn packages(pacman: &str) -> Result<Packages, String> {
    let explicit: BTreeSet<String> = names(&query(pacman, &["-Qqe"])?);
    let foreign = names(&query(pacman, &["-Qqm"])?);
    let total = query(pacman, &["-Q"])?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    if explicit.is_empty() || total == 0 {
        return Err(format!("`{pacman} -Qqe` or `-Q` listed no packages"));
    }
    Ok(Packages {
        explicit: explicit.into_iter().collect(),
        foreign,
        total,
        omarchy: BTreeSet::new(),
    })
}

/// The files of Omarchy's package lists: what its installer puts on every
/// machine (`base`) and what it installs outside that list or on certain
/// hardware (`other`).
pub const OMARCHY_LISTS: [&str; 2] = ["omarchy-base.packages", "omarchy-other.packages"];

/// The package names of Omarchy's lists in `dir` (one per line, `#`
/// comments and blank lines skipped), and the lists that could not be
/// read. A plain file read; nothing runs.
pub fn omarchy_packages(dir: &Path) -> (BTreeSet<String>, Vec<PathBuf>) {
    let mut names = BTreeSet::new();
    let mut missing = Vec::new();
    for file in OMARCHY_LISTS {
        let path = dir.join(file);
        match std::fs::read_to_string(&path) {
            Ok(text) => names.extend(
                text.lines()
                    .map(|l| l.split('#').next().unwrap_or_default().trim())
                    .filter(|l| !l.is_empty() && !l.contains(char::is_whitespace))
                    .map(String::from),
            ),
            Err(_) => missing.push(path),
        }
    }
    (names, missing)
}

/// One package name per line; anything with whitespace inside is not one.
fn names(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.contains(char::is_whitespace))
        .map(String::from)
        .collect()
}

/// The scope of a systemd unit (`services.enabled` `scope` column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    System,
    User,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::System => "system",
            Scope::User => "user",
        }
    }

    fn flag(self) -> &'static str {
        match self {
            Scope::System => "--system",
            Scope::User => "--user",
        }
    }
}

/// `systemctl --user|--system list-unit-files --state=enabled`: the unit
/// files enabled in `scope`, sorted.
pub fn enabled_units(systemctl: &str, scope: Scope) -> Result<Vec<String>, String> {
    let out = query(
        systemctl,
        &[
            scope.flag(),
            "list-unit-files",
            "--state=enabled",
            "--no-legend",
            "--no-pager",
        ],
    )?;
    let units: BTreeSet<String> = out
        .lines()
        .filter_map(|l| {
            let mut words = l.split_whitespace();
            match (words.next(), words.next()) {
                (Some(unit), Some("enabled")) if unit.contains('.') && !unit.contains('|') => {
                    Some(unit.to_string())
                }
                _ => None,
            }
        })
        .collect();
    Ok(units.into_iter().collect())
}

/// `omarchy plugin list --json` (the plugins collector's reader), ids
/// and clone sources through `redactor`.
pub fn plugins(omarchy: &str, redactor: &Redactor) -> Result<Vec<Listed>, String> {
    let mut listed = plugins::list(omarchy)?;
    for p in &mut listed {
        p.id = redactor.redact(&p.id);
        p.cloned_from = redactor.redact(&p.cloned_from);
    }
    listed.retain(|p| !p.id.is_empty() && !p.id.contains(['|', '\n']));
    listed.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(listed)
}

/// The Omarchy version (the omarchy collector's reader).
pub fn omarchy_version(sources: &Sources) -> Option<String> {
    omarchy::current_version(sources)
}

/// The current theme slug from Omarchy's state file, when there is one.
pub fn theme(sources: &Sources, home: &Path) -> Option<String> {
    let text = std::fs::read_to_string(Theme::file(sources, home)).ok()?;
    let slug = text.trim();
    (!slug.is_empty() && !slug.contains(char::is_whitespace) && slug.len() <= SUBJECT_MAX)
        .then(|| slug.to_string())
}

/// What the files under `/proc` and `/sys` say about the machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hardware {
    /// `model name` of `/proc/cpuinfo`.
    pub cpu: Option<String>,
    /// `MemTotal` of `/proc/meminfo`, in whole GiB.
    pub memory: Option<String>,
    /// `/sys/class/dmi/id/product_name`, when readable.
    pub machine: Option<String>,
    /// The type of the filesystem mounted at `/` (`/proc/mounts`).
    pub rootfs: Option<String>,
}

impl Hardware {
    /// Every value through `redactor` (the user's `[redaction]` patterns).
    pub fn redacted(self, redactor: &Redactor) -> Self {
        let r = |v: Option<String>| v.map(|v| redactor.redact(&v));
        Hardware {
            cpu: r(self.cpu),
            memory: r(self.memory),
            machine: r(self.machine),
            rootfs: r(self.rootfs),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Hardware::default()
    }
}

/// Reads the hardware facts below `root` (`/` on a real host).
pub fn hardware(root: &Path) -> Hardware {
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).ok();
    let clean = |s: &str| {
        let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
        (!s.is_empty() && !s.contains('|')).then_some(s)
    };
    let cpu = read("proc/cpuinfo").and_then(|t| {
        t.lines()
            .filter_map(|l| l.split_once(':'))
            .find(|(k, _)| k.trim() == "model name")
            .and_then(|(_, v)| clean(v))
    });
    let memory = read("proc/meminfo").and_then(|t| {
        let kib: u64 = t
            .lines()
            .find_map(|l| l.strip_prefix("MemTotal:"))?
            .split_whitespace()
            .next()?
            .parse()
            .ok()?;
        let gib = (kib as f64 / (1024.0 * 1024.0)).round() as u64;
        Some(format!("{gib} GiB"))
    });
    let machine = read("sys/class/dmi/id/product_name").and_then(|t| clean(&t));
    // the last mount at `/` is the one in effect
    let rootfs = read("proc/mounts").and_then(|t| {
        t.lines().rev().find_map(|l| {
            let mut f = l.split_whitespace();
            let (_, at, kind) = (f.next()?, f.next()?, f.next()?);
            (at == "/").then(|| kind.to_string())
        })
    });
    Hardware {
        cpu,
        memory,
        machine,
        rootfs,
    }
}

/// Runs a query; its stdout, or why it failed. Exit 1 with no output at
/// all is an empty answer (`-Qqm` on a system without foreign packages).
fn query(program: &str, args: &[&str]) -> Result<String, String> {
    let what = format!("`{program} {}`", args.join(" "));
    // the whole answer: a package list may be long, and a cut one would
    // read as packages removed
    match sys::run(program, args, None, RUN_TIMEOUT, sys::WHOLE_OUTPUT) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => Ok(stdout),
        Run::Exited {
            code: Some(1),
            stdout,
            stderr,
        } if stdout.trim().is_empty() && stderr.trim().is_empty() => Ok(String::new()),
        Run::Exited { code, stderr, .. } => {
            let reason = stderr
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map_or_else(
                    || {
                        format!(
                            "exit code {}",
                            code.map_or("none".into(), |c| c.to_string())
                        )
                    },
                    str::to_string,
                );
            Err(format!("{what}: {reason}"))
        }
        Run::NotFound => Err(format!("{what}: `{program}` not found")),
        Run::TimedOut => Err(format!("{what}: timed out")),
        Run::Cut => Err(format!("{what}: output over the limit")),
        Run::Failed(e) => Err(format!("{what}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_from_files_only() {
        let dir = std::env::temp_dir().join(format!("seldon-hw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("proc")).unwrap();
        std::fs::create_dir_all(dir.join("sys/class/dmi/id")).unwrap();
        std::fs::write(
            dir.join("proc/cpuinfo"),
            "processor\t: 0\nvendor_id\t: GenuineIntel\nmodel name\t: Intel(R)  Core(TM) i7\n\nprocessor\t: 1\nmodel name\t: Intel(R) Core(TM) i7\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("proc/meminfo"),
            "MemTotal:       65536000 kB\nMemFree: 1 kB\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("proc/mounts"),
            "sysfs /sys sysfs rw 0 0\n/dev/nvme0n1p2 / ext4 rw 0 0\n/dev/nvme0n1p2 / btrfs rw,subvol=/@ 0 0\n",
        )
        .unwrap();
        std::fs::write(dir.join("sys/class/dmi/id/product_name"), "\n").unwrap();
        let hw = hardware(&dir);
        assert_eq!(hw.cpu.as_deref(), Some("Intel(R) Core(TM) i7"));
        assert_eq!(hw.memory.as_deref(), Some("63 GiB"));
        assert_eq!(hw.rootfs.as_deref(), Some("btrfs"));
        assert_eq!(hw.machine, None, "blank product name");
        assert!(hardware(&dir.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn omarchy_lists_skip_comments_and_report_missing_files() {
        let dir = std::env::temp_dir().join(format!("seldon-lists-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("omarchy-base.packages"),
            "# Omarchy core\n\nbtop\ngit  # inline\nnot a name\n",
        )
        .unwrap();
        let (names, missing) = omarchy_packages(&dir);
        assert_eq!(names.into_iter().collect::<Vec<_>>(), ["btop", "git"]);
        assert_eq!(missing, [dir.join("omarchy-other.packages")]);
        std::fs::write(dir.join("omarchy-other.packages"), "snapper\n").unwrap();
        let (names, missing) = omarchy_packages(&dir);
        assert_eq!(names.len(), 3);
        assert!(missing.is_empty());
        let (names, missing) = omarchy_packages(&dir.join("absent"));
        assert!(names.is_empty());
        assert_eq!(missing.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_skip_blank_and_odd_lines() {
        let n = names("zed\n\n  btop  \nnot a name\n");
        assert_eq!(n.into_iter().collect::<Vec<_>>(), ["btop", "zed"]);
    }
}
