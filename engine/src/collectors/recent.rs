//! Recently edited files under `~/.config` outside the watch paths
//! (SPEC-ENGINE §4, ADR-0045, WP-139). No collector: it writes no event.
//!
//! The config collector hashes what `watchPaths` names, exactly but
//! narrowly. Edits elsewhere under `~/.config` (a terminal's config,
//! `git/config`, `starship.toml`) are invisible to it. [`scan`] walks
//! `~/.config` once per capture that runs the config collector and keeps
//! the paths and modification times of the newest [`MAX_FILES`] regular
//! files modified in the last [`DAYS`] days — never their content. The
//! result is `$XDG_STATE_HOME/seldon/recent-config.json` ([`Saved`]);
//! the index shows it as `system.recentConfig` ([`shown`]), filtered once
//! more at build time, so a path watched or skipped since the scan leaves
//! the list at the next index build.
//!
//! Left out, never entered or listed:
//!
//! - everything under a watch path and everything matching `[redaction]
//!   skipPaths` (a skipped folder is not entered);
//! - Omarchy's plugin folder and Seldon's own config folder;
//! - the ignore list of [`ignored_dir`] and [`ignored_file`]: `.git`,
//!   caches, state, logs, locks, databases, images, editor temp files,
//!   `shell.json`; and every folder that holds `Cookies` or `Local State`
//!   (a browser or Electron profile);
//! - links to directories (never followed) and anything that is no
//!   regular file; a link to a file counts by its target's time;
//! - a `~`-path with a control, direction or format character, longer than
//!   [`SUBJECT_MAX`] characters, or one the logbook's redaction would
//!   change (it is never shown masked: *Watch* needs the real path).
//!
//! Bounded: at most [`MAX_ENTRIES`] directory entries read and
//! [`MAX_DEPTH`] levels below `~/.config`; a walk that reaches the entry
//! budget stops there and says so (`cut`) in the state file.

use std::cmp::Reverse;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, FixedOffset, Timelike as _, Utc};
use serde::{Deserialize, Serialize};

use super::config::SkipPaths;
use crate::config::{Config, Dirs};
use crate::index::model::{RecentConfig, RecentFile};
use crate::model::event::{SUBJECT_MAX, format_ts};
use crate::redact::Redactor;
use crate::sys;

/// The state file, next to `cursors.json`.
pub const FILE: &str = "recent-config.json";

/// How far back a modification counts.
pub const DAYS: i64 = 7;

/// The newest this many files are kept.
pub const MAX_FILES: usize = 80;

/// Directory entries one walk reads at most.
pub const MAX_ENTRIES: usize = 20_000;

/// Directory levels below `~/.config` walked at most.
pub const MAX_DEPTH: usize = 12;

/// The folder walked, under the home directory.
pub const ROOT: &str = ".config";

/// Folders that are never entered, by exact name: they hold no
/// configuration a person edits.
const DIR_NAMES: [&str; 11] = [
    ".git",
    "state",
    "log",
    "logs",
    "history",
    "databases",
    "indexeddb",
    "leveldb",
    "local storage",
    "session storage",
    "blob_storage",
];

/// File endings that are never listed (lowercase): logs, locks,
/// databases, images, editor temp files.
const FILE_ENDINGS: [&str; 34] = [
    ".log",
    ".lock",
    ".lck",
    ".pid",
    ".db",
    ".db-journal",
    ".db-wal",
    ".db-shm",
    ".sqlite",
    ".sqlite3",
    ".sqlite-journal",
    ".sqlite-wal",
    ".sqlite-shm",
    ".ldb",
    ".kdbx",
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".webp",
    ".bmp",
    ".ico",
    ".svg",
    ".avif",
    ".tif",
    ".tiff",
    ".heic",
    ".jxl",
    ".xpm",
    ".swp",
    ".swo",
    ".swx",
    ".tmp",
    "~",
];

/// Entries that mark a folder as a browser or Electron profile.
const PROFILE_MARKS: [&str; 2] = ["Cookies", "Local State"];

/// Whether the folder `name` is never entered: [`DIR_NAMES`] (any case)
/// or a name holding `cache` (`Cache`, `GPUCache`, `Code Cache`,
/// `__pycache__`).
pub fn ignored_dir(name: &str) -> bool {
    let lower = name.to_lowercase();
    DIR_NAMES.contains(&lower.as_str()) || lower.contains("cache")
}

/// Whether the file `name` is never listed: `shell.json` (Omarchy's shell
/// rewrites it), state and history files, the endings of
/// [`FILE_ENDINGS`], rotated logs (`x.log.1`), a lock by name (`lock`,
/// Chromium's `Singleton*`), and temp files (`.#x`, `#x#`, `x.tmp-…`, GTK's
/// `.goutputstream-…`).
pub fn ignored_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == "shell.json"
        || lower == "state"
        || lower == "lock"
        || lower.ends_with("state.json")
        || lower.ends_with(".state")
        || lower == "history.json"
        || lower.starts_with("singleton")
        || lower.contains(".log.")
        || lower.contains(".tmp-")
        || lower.starts_with(".#")
        || lower.starts_with(".goutputstream-")
        || (lower.starts_with('#') && lower.ends_with('#'))
        || FILE_ENDINGS.iter().any(|e| lower.ends_with(e))
}

/// What a scan found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    /// Newest first, at most [`MAX_FILES`].
    pub files: Vec<(String, DateTime<FixedOffset>)>,
    /// The walk stopped at [`MAX_ENTRIES`].
    pub cut: bool,
    /// Directory entries read.
    pub entries: usize,
}

/// The rules of one scan.
struct Walk<'a> {
    dirs: &'a Dirs,
    /// The watch paths, expanded: nothing below them is listed.
    watched: Vec<PathBuf>,
    /// Folders never entered: the plugin folders, Seldon's config.
    excluded: Vec<PathBuf>,
    skip: SkipPaths,
    redactor: &'a Redactor,
    since: SystemTime,
    now: DateTime<FixedOffset>,
    found: Vec<(SystemTime, String)>,
    entries: usize,
    /// [`MAX_ENTRIES`], smaller in tests.
    max_entries: usize,
    cut: bool,
}

/// Walks `~/.config` at `now` (SPEC-ENGINE §4). `excluded` are folders
/// never entered besides the built-in ones (the capture passes Omarchy's
/// plugin folder as the plugins collector finds it).
pub fn scan(
    dirs: &Dirs,
    config: &Config,
    redactor: &Redactor,
    excluded: &[PathBuf],
    now: DateTime<FixedOffset>,
) -> Scan {
    scan_bounded(dirs, config, redactor, excluded, now, MAX_ENTRIES)
}

/// [`scan`] with its entry budget.
fn scan_bounded(
    dirs: &Dirs,
    config: &Config,
    redactor: &Redactor,
    excluded: &[PathBuf],
    now: DateTime<FixedOffset>,
    max_entries: usize,
) -> Scan {
    let root = dirs.home.join(ROOT);
    let since = SystemTime::from(now - chrono::Duration::days(DAYS));
    let mut walk = Walk {
        dirs,
        watched: watched(dirs, config),
        excluded: own_dirs(dirs)
            .into_iter()
            .chain(excluded.iter().cloned())
            .collect(),
        skip: SkipPaths::new(&dirs.home, &config.redaction.skip_paths),
        redactor,
        since,
        now,
        found: Vec::new(),
        entries: 0,
        max_entries,
        cut: false,
    };
    // `~/.config` itself may be a link (dotfile managers)
    if std::fs::metadata(&root).is_ok_and(|m| m.is_dir()) && !walk.left_out(&root) {
        walk.dir(&root, 0);
    }
    let mut found = walk.found;
    found.sort_by(|a, b| Reverse(a.0).cmp(&Reverse(b.0)).then_with(|| a.1.cmp(&b.1)));
    found.truncate(MAX_FILES);
    Scan {
        files: found
            .into_iter()
            .map(|(t, path)| (path, at(t, now)))
            .collect(),
        cut: walk.cut,
        entries: walk.entries,
    }
}

/// The watch paths as absolute paths.
fn watched(dirs: &Dirs, config: &Config) -> Vec<PathBuf> {
    config
        .watch_paths
        .iter()
        .filter_map(|w| dirs.expand_config(w))
        .collect()
}

/// Folders the scan never enters whatever the config says: Omarchy's
/// plugin folder (the plugins collector's) and Seldon's own config (the
/// *Watch* click edits it).
fn own_dirs(dirs: &Dirs) -> Vec<PathBuf> {
    vec![
        dirs.home.join(super::plugins::PLUGINS_DIR),
        dirs.home.join(ROOT).join("seldon"),
        dirs.config_dir(),
    ]
}

/// `t` in `now`'s offset, whole seconds, never after `now`.
fn at(t: SystemTime, now: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    let t = DateTime::<Utc>::from(t).with_timezone(now.offset());
    t.with_nanosecond(0).unwrap_or(t).min(now)
}

impl Walk<'_> {
    /// Whether `path` is under a watch path, an excluded folder or a
    /// skipPath.
    fn left_out(&self, path: &Path) -> bool {
        self.watched.iter().any(|w| path.starts_with(w))
            || self.excluded.iter().any(|x| path.starts_with(x))
            || self.skip.matches(path)
    }

    fn dir(&mut self, dir: &Path, depth: usize) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries = Vec::new();
        for entry in read {
            if self.entries >= self.max_entries {
                self.cut = true;
                return;
            }
            self.entries += 1;
            if let Ok(entry) = entry {
                entries.push(entry);
            }
        }
        // a browser or Electron profile: nothing in it is listed
        if entries
            .iter()
            .any(|e| PROFILE_MARKS.iter().any(|m| e.file_name() == *m))
        {
            return;
        }
        let mut subdirs = Vec::new();
        for entry in entries {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !ignored_dir(&name) && !self.left_out(&path) {
                    subdirs.push(path);
                }
            } else if (kind.is_file() || kind.is_symlink())
                && !ignored_file(&name)
                && !self.left_out(&path)
            {
                // a link counts by its target, and only a link to a file
                if let Ok(meta) = std::fs::metadata(&path)
                    && meta.is_file()
                    && let Ok(t) = meta.modified()
                    && t >= self.since
                    && let Some(key) = shown_path(self.dirs, self.redactor, &path)
                {
                    self.found.push((t.min(SystemTime::from(self.now)), key));
                }
            }
        }
        if depth >= MAX_DEPTH {
            return;
        }
        subdirs.sort();
        for sub in subdirs {
            if self.cut {
                return;
            }
            self.dir(&sub, depth + 1);
        }
    }
}

/// `path` as a `~`-path the list may show: under `~/.config/`, no `.` or
/// `..` folder and no empty one, no control, direction or format
/// character, at most [`SUBJECT_MAX`] characters, and unchanged by the
/// logbook's redaction. `None` otherwise.
pub fn shown_path(dirs: &Dirs, redactor: &Redactor, path: &Path) -> Option<String> {
    let key = dirs.display(path);
    let ok = key.starts_with("~/.config/")
        && key[2..]
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
        && key.chars().count() <= SUBJECT_MAX
        && !key
            .chars()
            .any(|c| c.is_control() || crate::import::is_direction_or_format(c))
        && redactor.redact(&key) == key;
    ok.then_some(key)
}

/// The state file `recent-config.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub scanned_at: String,
    pub files: Vec<RecentFile>,
    /// The walk stopped at [`MAX_ENTRIES`]: older files may be missing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cut: bool,
}

impl Saved {
    pub fn file(dirs: &Dirs) -> PathBuf {
        dirs.state_dir.join(FILE)
    }

    pub fn of(scan: &Scan, now: DateTime<FixedOffset>) -> Self {
        Saved {
            scanned_at: format_ts(&now),
            files: scan
                .files
                .iter()
                .map(|(path, t)| RecentFile {
                    path: path.clone(),
                    mtime: format_ts(t),
                })
                .collect(),
            cut: scan.cut,
        }
    }

    pub fn save(&self, dirs: &Dirs) -> anyhow::Result<()> {
        let text = serde_json::to_string(self)?;
        sys::write_atomic(&Self::file(dirs), text.as_bytes())
    }

    /// The saved scan; `Ok(None)` when there is none, `Err` with the reason
    /// when the file cannot be read or parsed.
    pub fn load(dirs: &Dirs) -> Result<Option<Self>, String> {
        let path = Self::file(dirs);
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| format!("{}: unreadable ({e})", dirs.display(&path))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", dirs.display(&path))),
        }
    }
}

/// `system.recentConfig` at index time: the saved scan without the files
/// that are watched or skipped by now, older than [`DAYS`] days at `now`,
/// or no longer a path [`shown_path`] allows, at most [`MAX_FILES`].
/// `None` without a saved scan, or without a redactor (an invalid
/// `[redaction] patterns` entry withholds the list, as ADR-0038 §2 does
/// the texts); a file that cannot be read is a warning.
pub fn shown(
    dirs: &Dirs,
    config: &Config,
    redactor: Option<&Redactor>,
    now: DateTime<FixedOffset>,
    warnings: &mut Vec<String>,
) -> Option<RecentConfig> {
    let saved = match Saved::load(dirs) {
        Ok(saved) => saved?,
        Err(e) => {
            warnings.push(e);
            return None;
        }
    };
    let redactor = redactor?;
    let watched = watched(dirs, config);
    let skip = SkipPaths::new(&dirs.home, &config.redaction.skip_paths);
    let since = now - chrono::Duration::days(DAYS);
    let files = saved
        .files
        .into_iter()
        .filter(|f| {
            let path = dirs.expand(&f.path);
            DateTime::parse_from_rfc3339(&f.mtime).is_ok_and(|t| t >= since)
                && shown_path(dirs, redactor, &path).as_deref() == Some(f.path.as_str())
                && !watched.iter().any(|w| path.starts_with(w))
                && !skipped(&skip, &dirs.home, &path)
        })
        .take(MAX_FILES)
        .collect();
    Some(RecentConfig {
        scanned_at: saved.scanned_at,
        files,
    })
}

/// Whether `path` or one of its folders below `home` matches `skip` (a
/// name pattern matches a folder's name too, as in the walk).
pub fn skipped(skip: &SkipPaths, home: &Path, path: &Path) -> bool {
    path.ancestors()
        .take_while(|p| p.starts_with(home) && *p != home)
        .any(|p| skip.matches(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const NOW: &str = "2026-10-08T12:00:00+02:00";

    /// A temp home with `~/.config`; removed on drop.
    struct Home {
        dirs: Dirs,
        now: DateTime<FixedOffset>,
    }

    impl Home {
        fn new(tag: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("seldon-recent-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let home = root.join("home");
            std::fs::create_dir_all(home.join(ROOT)).unwrap();
            Home {
                dirs: Dirs {
                    xdg_config_home: home.join(".config"),
                    state_dir: home.join(".local/state/seldon"),
                    home,
                },
                now: DateTime::parse_from_rfc3339(NOW).unwrap(),
            }
        }

        /// `~/rel`, modified `age` before [`NOW`].
        fn file(&self, rel: &str, age: Duration) -> PathBuf {
            let path = self.dirs.home.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "x = 1\n").unwrap();
            let t = SystemTime::from(self.now) - age;
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(t)
                .unwrap();
            path
        }

        fn scan(&self, config: &Config) -> Scan {
            scan(&self.dirs, config, &Redactor::builtin(), &[], self.now)
        }

        fn paths(&self, config: &Config) -> Vec<String> {
            self.scan(config)
                .files
                .into_iter()
                .map(|(p, _)| p)
                .collect()
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.dirs.home.parent().unwrap());
        }
    }

    /// A saved scan of `paths`, each modified an hour before [`NOW`].
    fn json_files(paths: &[&str]) -> String {
        let files: Vec<_> = paths
            .iter()
            .map(|p| serde_json::json!({"path": p, "mtime": "2026-10-08T11:00:00+02:00"}))
            .collect();
        serde_json::json!({"scannedAt": NOW, "files": files}).to_string()
    }

    const HOUR: Duration = Duration::from_secs(3600);
    const DAY: Duration = Duration::from_secs(86_400);

    #[test]
    fn the_ignore_list() {
        for name in [
            ".git",
            "Cache",
            "GPUCache",
            "Code Cache",
            "__pycache__",
            "cache",
            "state",
            "logs",
            "log",
            "history",
            "databases",
            "IndexedDB",
            "Local Storage",
        ] {
            assert!(ignored_dir(name), "{name}");
        }
        for name in ["alacritty", "git", "nvim", "fish", "zed", "Code", "status"] {
            assert!(!ignored_dir(name), "{name}");
        }
        for name in [
            "shell.json",
            "state",
            "state.json",
            "windowstate.json",
            "app.state",
            "history.json",
            "app.log",
            "app.log.1",
            "x.lock",
            "lock",
            "SingletonLock",
            "app.pid",
            "places.sqlite",
            "data.db",
            "data.db-wal",
            "vault.kdbx",
            "000003.ldb",
            "wall.png",
            "icon.SVG",
            "face.jpeg",
            ".init.lua.swp",
            "notes.tmp",
            "config.toml~",
            ".#init.lua",
            "#init.lua#",
            ".config.toml.tmp-1234",
            ".goutputstream-ABC123",
        ] {
            assert!(ignored_file(name), "{name}");
        }
        for name in [
            "alacritty.toml",
            "config",
            "starship.toml",
            "settings.json",
            "init.lua",
            "user-dirs.dirs",
            "mimeapps.list",
            "statusline.conf",
            "catalog.json",
            "blocklist",
        ] {
            assert!(!ignored_file(name), "{name}");
        }
    }

    #[test]
    fn lists_recent_files_outside_the_watch_paths_newest_first() {
        let h = Home::new("list");
        h.file(".config/git/config", 2 * DAY);
        h.file(".config/alacritty/alacritty.toml", HOUR);
        h.file(".config/starship.toml", 3 * HOUR);
        // watched (a default watch path), old, own, ignored
        h.file(".config/hypr/hyprland.lua", HOUR);
        h.file(".config/omarchy/plugins/x.weather/manifest.json", HOUR);
        h.file(".config/old/app.conf", 7 * DAY + HOUR);
        h.file(".config/seldon/config.toml", HOUR);
        h.file(".config/nvim/.git/config", HOUR);
        h.file(".config/app/Cache/data.conf", HOUR);
        h.file(".config/app/logs/today.conf", HOUR);
        h.file(".config/app/app.log", HOUR);
        h.file(".config/app/wall.png", HOUR);
        // a browser and an Electron profile: nothing in them
        h.file(".config/chromium/Local State", HOUR);
        h.file(".config/chromium/Default/Preferences", HOUR);
        h.file(".config/Code/Cookies", HOUR);
        h.file(".config/Code/User/settings.json", HOUR);
        // a home file outside ~/.config is not scanned
        h.file(".gitconfig", HOUR);
        assert_eq!(
            h.paths(&Config::default()),
            [
                "~/.config/alacritty/alacritty.toml",
                "~/.config/starship.toml",
                "~/.config/git/config",
            ]
        );
        // exactly 7 days back still counts; the times are whole seconds
        h.file(".config/edge/app.conf", 7 * DAY);
        let scan = h.scan(&Config::default());
        assert_eq!(scan.files.last().unwrap().0, "~/.config/edge/app.conf");
        assert_eq!(
            format_ts(&scan.files[0].1),
            "2026-10-08T11:00:00+02:00",
            "in the capture's offset"
        );
        assert!(!scan.cut);
    }

    #[test]
    fn skip_paths_are_honoured_also_through_a_folder_name() {
        let h = Home::new("skip");
        h.file(".config/secret/app.conf", HOUR);
        h.file(".config/app/id.key", HOUR);
        h.file(".config/app/private/creds.conf", HOUR);
        h.file(".config/app/settings.conf", HOUR);
        let mut config = Config::default();
        config.redaction.skip_paths =
            vec!["~/.config/secret/".into(), "*.key".into(), "private".into()];
        assert_eq!(h.paths(&config), ["~/.config/app/settings.conf"]);
    }

    #[test]
    fn a_watch_path_the_user_added_or_removed_counts() {
        let h = Home::new("watch");
        h.file(".config/hypr/hyprland.lua", HOUR);
        h.file(".config/alacritty/alacritty.toml", 2 * HOUR);
        let config = Config {
            watch_paths: vec!["~/.config/alacritty/alacritty.toml".into()],
            ..Config::default()
        };
        assert_eq!(h.paths(&config), ["~/.config/hypr/hyprland.lua"]);
    }

    #[test]
    fn the_newest_eighty() {
        let h = Home::new("eighty");
        for i in 0..85u64 {
            h.file(
                &format!(".config/many/f{i:02}.conf"),
                HOUR + Duration::from_secs(60 * i),
            );
        }
        let paths = h.paths(&Config::default());
        assert_eq!(paths.len(), MAX_FILES);
        assert_eq!(paths[0], "~/.config/many/f00.conf");
        assert_eq!(paths[79], "~/.config/many/f79.conf");
    }

    #[test]
    fn the_entry_budget_and_the_depth() {
        let h = Home::new("budget");
        h.file(".config/a/one.conf", HOUR);
        h.file(".config/b/two.conf", HOUR);
        let scan = scan_bounded(
            &h.dirs,
            &Config::default(),
            &Redactor::builtin(),
            &[],
            h.now,
            3,
        );
        // `a`, `b`, then `a/one.conf`: the budget is spent before `b`
        assert!(scan.cut);
        assert_eq!(scan.entries, 3);
        assert_eq!(scan.files.len(), 1);
        let deep: String = (0..=MAX_DEPTH).map(|i| format!("d{i}/")).collect();
        h.file(&format!(".config/{deep}deep.conf"), HOUR);
        let shallow: String = (0..MAX_DEPTH).map(|i| format!("d{i}/")).collect();
        h.file(&format!(".config/{shallow}level.conf"), HOUR);
        let paths = h.paths(&Config::default());
        assert!(paths.iter().any(|p| p.ends_with("level.conf")), "{paths:?}");
        assert!(!paths.iter().any(|p| p.ends_with("deep.conf")), "{paths:?}");
    }

    #[test]
    fn links_to_folders_are_not_followed_links_to_files_count_by_target() {
        let h = Home::new("links");
        let outside = h.file("dotfiles/kitty/kitty.conf", HOUR);
        h.file("dotfiles/fish/config.fish", HOUR);
        std::os::unix::fs::symlink(
            h.dirs.home.join("dotfiles/fish"),
            h.dirs.home.join(".config/fish"),
        )
        .unwrap();
        std::fs::create_dir_all(h.dirs.home.join(".config/kitty")).unwrap();
        std::os::unix::fs::symlink(&outside, h.dirs.home.join(".config/kitty/kitty.conf")).unwrap();
        // a dangling link is nothing
        std::os::unix::fs::symlink(
            h.dirs.home.join("gone"),
            h.dirs.home.join(".config/gone.conf"),
        )
        .unwrap();
        assert_eq!(h.paths(&Config::default()), ["~/.config/kitty/kitty.conf"]);
    }

    #[test]
    fn a_path_the_redaction_would_change_or_with_a_control_character_is_left_out() {
        let h = Home::new("redact");
        h.file(".config/app/sekrit.conf", HOUR);
        h.file(".config/app/bad\u{1b}name.conf", HOUR);
        h.file(".config/app/bidi\u{202e}name.conf", HOUR);
        h.file(".config/app/fine.conf", HOUR);
        let redactor = Redactor::with_patterns(&["sekrit".into()]).unwrap();
        let scan = scan(&h.dirs, &Config::default(), &redactor, &[], h.now);
        let paths: Vec<_> = scan.files.into_iter().map(|(p, _)| p).collect();
        assert_eq!(paths, ["~/.config/app/fine.conf"]);
    }

    #[test]
    fn shown_filters_the_saved_scan_again() {
        let h = Home::new("shown");
        h.file(".config/a/one.conf", HOUR);
        h.file(".config/b/two.conf", 2 * HOUR);
        h.file(".config/c/three.conf", 3 * HOUR);
        let config = Config::default();
        let mut warnings = Vec::new();
        let redactor = Redactor::builtin();
        let shown_at = |config: &Config, now, warnings: &mut Vec<String>| {
            shown(&h.dirs, config, Some(&redactor), now, warnings)
        };
        assert_eq!(shown_at(&config, h.now, &mut warnings), None, "no scan yet");
        Saved::of(&h.scan(&config), h.now).save(&h.dirs).unwrap();
        let paths = |r: Option<RecentConfig>| -> Vec<String> {
            r.unwrap().files.into_iter().map(|f| f.path).collect()
        };
        assert_eq!(
            paths(shown_at(&config, h.now, &mut warnings)),
            [
                "~/.config/a/one.conf",
                "~/.config/b/two.conf",
                "~/.config/c/three.conf"
            ]
        );
        // watched or skipped since the scan, or older than 7 days by now
        let mut later = config.clone();
        later.watch_paths.push("~/.config/a".into());
        later.redaction.skip_paths = vec!["b".into()];
        assert_eq!(
            paths(shown_at(&later, h.now, &mut warnings)),
            ["~/.config/c/three.conf"]
        );
        let week_later = h.now + chrono::Duration::days(7) - chrono::Duration::minutes(150);
        assert_eq!(
            paths(shown_at(&config, week_later, &mut warnings)),
            ["~/.config/a/one.conf", "~/.config/b/two.conf"]
        );
        // without a redactor (an invalid pattern) the list is withheld
        assert_eq!(shown(&h.dirs, &config, None, h.now, &mut warnings), None);
        // a redaction pattern added since the scan
        let masked = Redactor::with_patterns(&["two".into()]).unwrap();
        assert_eq!(
            paths(shown(&h.dirs, &config, Some(&masked), h.now, &mut warnings)),
            ["~/.config/a/one.conf", "~/.config/c/three.conf"]
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        // a state file written by hand: only paths the scan would keep
        std::fs::write(
            Saved::file(&h.dirs),
            json_files(&[
                "~/.config/../.ssh/id_ed25519",
                "~/.config/./a/one.conf",
                "~/.config//a/one.conf",
                "~/.ssh/config",
                "/etc/passwd",
                "~/.config/a/one.conf",
            ]),
        )
        .unwrap();
        assert_eq!(
            paths(shown_at(&config, h.now, &mut warnings)),
            ["~/.config/a/one.conf"]
        );
        // an unreadable file is a warning, never a failure
        std::fs::write(Saved::file(&h.dirs), "{").unwrap();
        assert_eq!(shown_at(&config, h.now, &mut warnings), None);
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].starts_with("~/.local/state/seldon/recent-config.json: unreadable ("),
            "{warnings:?}"
        );
    }
}
