//! Recently edited files under `~/.config` outside the watch paths
//! (SPEC-ENGINE §4, ADR-0046, WP-139). No collector: it writes no event.
//!
//! The config collector hashes what `watchPaths` names, exactly but
//! narrowly. Edits elsewhere under `~/.config` (a terminal's config,
//! `git/config`, `starship.toml`) are invisible to it. [`scan`] walks
//! `~/.config` once per capture that runs the config collector, with the
//! engine's one walker ([`crate::config_scan`], shared with `seldon
//! preview`), and keeps the paths and modification times of the newest
//! [`MAX_FILES`] files modified in the last [`DAYS`] days — never their
//! content. The result is `$XDG_STATE_HOME/seldon/recent-config.json`
//! ([`Saved`]); the index shows it as `system.recentConfig` ([`shown`]),
//! filtered once more at build time, so a path watched or skipped since
//! the scan leaves the list at the next index build.
//!
//! Left out, never entered or listed, besides the walker's own ignore
//! list, links and `[redaction] skipPaths` ([`crate::config_scan`]):
//!
//! - everything under a watch path, Omarchy's plugin folder, Seldon's own
//!   config folder and config file, and the logbook ([`exclusions`],
//!   before a folder is entered);
//! - a path [`shown_path`] refuses — not UTF-8, a character a path may
//!   not hold (`import::bad_path_char`), longer than [`SUBJECT_MAX`]
//!   characters, a `.` or `..` folder, or one the logbook's redaction
//!   would change (it is never shown masked: *Watch* needs the real path)
//!   — before the cut to [`MAX_FILES`];
//! - a link whose target is not a file inside `~/.config` the walk would
//!   list itself ([`crate::config_scan::resolve_within`], WP-139 round 3).
//!
//! Bounded by the walker: at most [`MAX_ENTRIES`] directory entries read,
//! [`crate::config_scan::MAX_DEPTH`] levels below `~/.config` and
//! [`DEADLINE`] of wall time; any of them marks the result `partial`, in
//! the state file and in the index.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, FixedOffset, Timelike as _, Utc};
use serde::{Deserialize, Serialize};

use super::config::SkipPaths;
use crate::config::{Config, Dirs};
use crate::config_scan::{self, Limits};
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

/// Wall time one walk may take; it stops there, `partial`.
pub const DEADLINE: Duration = Duration::from_millis(500);

/// The folder walked, under the home directory.
pub const ROOT: &str = config_scan::ROOT;

/// What a scan found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    /// Newest first, at most [`MAX_FILES`].
    pub files: Vec<(String, DateTime<FixedOffset>)>,
    /// The walk stopped early ([`MAX_ENTRIES`], [`DEADLINE`]) or left
    /// folders below its depth out: files it did not reach are missing.
    pub partial: bool,
    /// Directory entries read.
    pub entries: usize,
}

/// Walks `~/.config` at `now` (SPEC-ENGINE §4). `excluded` are paths never
/// entered or listed besides the built-in ones ([`exclusions`]; the
/// capture passes Omarchy's plugin folder as the plugins collector finds
/// it, the config file and the logbook).
pub fn scan(
    dirs: &Dirs,
    config: &Config,
    redactor: &Redactor,
    excluded: &[PathBuf],
    now: DateTime<FixedOffset>,
) -> Scan {
    let deadline = Instant::now() + DEADLINE;
    scan_bounded(dirs, config, redactor, excluded, now, MAX_ENTRIES, deadline)
}

/// [`scan`] with its entry budget and deadline.
fn scan_bounded(
    dirs: &Dirs,
    config: &Config,
    redactor: &Redactor,
    excluded: &[PathBuf],
    now: DateTime<FixedOffset>,
    max_entries: usize,
    deadline: Instant,
) -> Scan {
    let limits = Limits {
        since: SystemTime::from(now - chrono::Duration::days(DAYS)),
        max_files: MAX_FILES,
        deadline: Some(deadline),
        max_entries,
        exclude: exclusions(dirs, config, excluded),
    };
    let skip = SkipPaths::new(&dirs.home, &config.redaction.skip_paths);
    let keep = |path: &Path| shown_path(dirs, redactor, path).is_some();
    let walked = config_scan::scan_keeping(&dirs.home.join(ROOT), &skip, &limits, &keep);
    Scan {
        files: walked
            .files
            .into_iter()
            .filter_map(|f| Some((shown_path(dirs, redactor, &f.path)?, at(f.modified, now))))
            .collect(),
        partial: walked.partial,
        entries: walked.entries,
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

/// What the walk never enters or lists: the watch paths (the config
/// collector's), Omarchy's plugin folder (the plugins collector's),
/// Seldon's own config (the *Watch* click edits it) and state directory,
/// and `extra`.
fn exclusions(dirs: &Dirs, config: &Config, extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = watched(dirs, config);
    out.extend([
        dirs.home.join(super::plugins::PLUGINS_DIR),
        dirs.home.join(ROOT).join("seldon"),
        dirs.config_dir(),
        // `$XDG_STATE_HOME` under `~/.config`: `index.json` changes with
        // every capture (WP-139 stage 2, N1)
        dirs.state_dir.clone(),
    ]);
    out.extend(extra.iter().cloned());
    out
}

/// `t` in `now`'s offset, whole seconds, never after `now`.
fn at(t: SystemTime, now: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    let t = DateTime::<Utc>::from(t).with_timezone(now.offset());
    t.with_nanosecond(0).unwrap_or(t).min(now)
}

/// `path` as a `~`-path the list may show: UTF-8, under `~/.config/`, no
/// `.` or `..` folder and no empty one, no character a path may not hold
/// ([`crate::import::bad_path_char`], WP-159: control, invisible, U+2028,
/// U+2029; the plugin's `BAD_PATH_CHARS`), at most [`SUBJECT_MAX`]
/// characters, and unchanged by the logbook's redaction. `None`
/// otherwise.
pub fn shown_path(dirs: &Dirs, redactor: &Redactor, path: &Path) -> Option<String> {
    // `display` would replace what is not UTF-8 with U+FFFD (B1)
    path.to_str()?;
    let key = dirs.display(path);
    let ok = key.starts_with("~/.config/")
        && key[2..]
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
        && key.chars().count() <= SUBJECT_MAX
        && !key.chars().any(crate::import::bad_path_char)
        && redactor.redact(&key) == key;
    ok.then_some(key)
}

/// The state file `recent-config.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub scanned_at: String,
    pub files: Vec<RecentFile>,
    /// The walk stopped early or left deep folders out ([`Scan::partial`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
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
            partial: scan.partial,
        }
    }

    pub fn save(&self, dirs: &Dirs) -> anyhow::Result<()> {
        let text = serde_json::to_string(self)?;
        sys::write_atomic(&Self::file(dirs), text.as_bytes())
    }

    /// The saved scan; `Ok(None)` when there is none, `Err` with the reason
    /// when the file cannot be read or parsed. Read only when it is a
    /// regular file of at most [`sys::STATE_FILE_MAX`] (no link, FIFO or
    /// device: B2, as `autocommit.json` and the proposals).
    pub fn load(dirs: &Dirs) -> Result<Option<Self>, String> {
        let path = Self::file(dirs);
        let shown = dirs.display(&path);
        match sys::read_small_file(&path, sys::STATE_FILE_MAX) {
            Ok(Some(text)) => serde_json::from_str(&text)
                .map(Some)
                .map_err(|e| format!("{shown}: unreadable ({e})")),
            Ok(None) => Ok(None),
            Err(why) => Err(format!("{shown}: not read ({why})")),
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
        partial: saved.partial,
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
        assert!(!scan.partial);
    }

    #[test]
    fn a_time_after_the_scan_is_the_scan_time() {
        let h = Home::new("future");
        let path = h.file(".config/app/ahead.conf", Duration::ZERO);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::from(h.now) + 2 * HOUR)
            .unwrap();
        h.file(".config/app/now.conf", Duration::from_secs(1));
        // a file system's nanoseconds are cut to whole seconds
        h.file(
            ".config/app/fraction.conf",
            HOUR - Duration::from_millis(300),
        );
        let scan = h.scan(&Config::default());
        assert_eq!(scan.files[0].0, "~/.config/app/ahead.conf");
        assert_eq!(scan.files[0].1, h.now);
        assert_eq!(scan.files[2].0, "~/.config/app/fraction.conf");
        assert_eq!(format_ts(&scan.files[2].1), "2026-10-08T11:00:00+02:00");
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
        // Omarchy's plugin folder stays out without its watch path too
        h.file(".config/omarchy/plugins/x.weather/manifest.json", HOUR);
        let config = Config {
            watch_paths: vec!["~/.config/alacritty/alacritty.toml".into()],
            ..Config::default()
        };
        assert_eq!(h.paths(&config), ["~/.config/hypr/hyprland.lua"]);
    }

    /// The path rules run before the cut: ten newer files the redaction
    /// would change do not push older ones out of the 80.
    #[test]
    fn refused_paths_take_no_place_in_the_eighty() {
        let h = Home::new("keepcut");
        for i in 0..10u64 {
            h.file(
                &format!(".config/app/sekrit-{i}.conf"),
                Duration::from_secs(60 + i),
            );
        }
        for i in 0..85u64 {
            h.file(
                &format!(".config/many/f{i:02}.conf"),
                HOUR + Duration::from_secs(60 * i),
            );
        }
        let redactor = Redactor::with_patterns(&["sekrit".into()]).unwrap();
        let scan = scan(&h.dirs, &Config::default(), &redactor, &[], h.now);
        assert_eq!(scan.files.len(), MAX_FILES);
        assert_eq!(scan.files[0].0, "~/.config/many/f00.conf");
    }

    /// N1 (stage 2): a state directory under `~/.config` is not listed.
    #[test]
    fn a_state_directory_under_dot_config_is_not_listed() {
        let mut h = Home::new("state");
        h.dirs.state_dir = h.dirs.home.join(".config/seldon-state");
        h.file(".config/seldon-state/index.json", HOUR);
        h.file(".config/app/a.conf", HOUR);
        assert_eq!(h.paths(&Config::default()), ["~/.config/app/a.conf"]);
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
    fn the_entry_budget_the_deadline_and_the_depth() {
        let h = Home::new("budget");
        h.file(".config/a/one.conf", HOUR);
        h.file(".config/b/two.conf", HOUR);
        let bounded = |max: usize, deadline: Instant| {
            scan_bounded(
                &h.dirs,
                &Config::default(),
                &Redactor::builtin(),
                &[],
                h.now,
                max,
                deadline,
            )
        };
        let later = Instant::now() + Duration::from_secs(60);
        assert!(!bounded(MAX_ENTRIES, later).partial);
        // `a`, `b`, then `a/one.conf`: the budget is spent before `b`
        let scan = bounded(3, later);
        assert!(scan.partial);
        assert_eq!(scan.entries, 3);
        assert_eq!(scan.files.len(), 1);
        // the state file says so, and the index after it
        assert!(Saved::of(&scan, h.now).partial);
        assert!(!Saved::of(&bounded(MAX_ENTRIES, later), h.now).partial);
        // a deadline already past: nothing read
        let scan = bounded(MAX_ENTRIES, Instant::now());
        assert!(scan.partial);
        assert_eq!((scan.entries, scan.files.len()), (0, 0));
        // folders below the depth are not read, and the scan says so
        let shallow: String = (0..config_scan::MAX_DEPTH)
            .map(|i| format!("d{i}/"))
            .collect();
        h.file(&format!(".config/{shallow}level.conf"), HOUR);
        let scan = h.scan(&Config::default());
        assert!(!scan.partial, "a file at the last level is read");
        assert!(scan.files.iter().any(|(p, _)| p.ends_with("level.conf")));
        let deep: String = (0..=config_scan::MAX_DEPTH)
            .map(|i| format!("d{i}/"))
            .collect();
        h.file(&format!(".config/{deep}deep.conf"), HOUR);
        let scan = h.scan(&Config::default());
        assert!(scan.partial);
        assert!(!scan.files.iter().any(|(p, _)| p.ends_with("deep.conf")));
    }

    #[test]
    fn a_name_that_is_not_utf8_is_left_out() {
        use std::os::unix::ffi::OsStrExt as _;
        let h = Home::new("latin1");
        let app = h.dirs.home.join(".config/app");
        h.file(".config/app/fine.conf", HOUR);
        // `é` and `è` in Latin-1: two files that a lossy name would merge
        for byte in [0xe9u8, 0xe8] {
            let name = [b"latin1-".as_slice(), &[byte], b".conf"].concat();
            std::fs::write(app.join(std::ffi::OsStr::from_bytes(&name)), "x\n").unwrap();
        }
        // and a folder whose name is not UTF-8
        let folder = app.join(std::ffi::OsStr::from_bytes(b"dir-\xe9"));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("inside.conf"), "x\n").unwrap();
        let scan = h.scan(&Config::default());
        let paths: Vec<_> = scan.files.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(paths, ["~/.config/app/fine.conf"]);
        // `app` at the top, then its four entries: the folder is not entered
        assert_eq!(scan.entries, 5);
        let redactor = Redactor::builtin();
        assert_eq!(
            shown_path(&h.dirs, &redactor, &folder.join("inside.conf")),
            None
        );
    }

    #[test]
    fn a_link_counts_only_with_a_target_the_list_would_show_inside_dot_config() {
        let h = Home::new("links");
        let home = &h.dirs.home;
        let link = |to: &Path, at: &str| {
            let at = home.join(at);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(to, at).unwrap();
        };
        // outside ~/.config: a dotfile tree, Seldon's state (B2), a skipped
        // secret (B1), /proc (N1)
        h.file("dotfiles/kitty/kitty.conf", HOUR);
        h.file("dotfiles/fish/config.fish", HOUR);
        h.file(".local/state/seldon/index.json", HOUR);
        h.file("secrets/token", HOUR);
        link(&home.join("dotfiles/fish"), ".config/fish");
        link(
            &home.join("dotfiles/kitty/kitty.conf"),
            ".config/kitty/kitty.conf",
        );
        link(
            &home.join(".local/state/seldon/index.json"),
            ".config/ownlink.json",
        );
        link(&home.join("secrets/token"), ".config/app/token.conf");
        link(Path::new("/proc/self/status"), ".config/procfile.conf");
        // inside: a skipped folder, a watched file, Seldon's config, and one
        // the list shows
        h.file(".config/private/key.conf", HOUR);
        h.file(".config/hypr/input.lua", HOUR);
        h.file(".config/seldon/config.toml", HOUR);
        h.file(".config/dots/foot.ini", 2 * HOUR);
        link(
            &home.join(".config/private/key.conf"),
            ".config/app/key.conf",
        );
        link(
            &home.join(".config/hypr/input.lua"),
            ".config/app/input.lua",
        );
        link(
            &home.join(".config/seldon/config.toml"),
            ".config/app/seldon.toml",
        );
        link(Path::new("../dots/foot.ini"), ".config/foot/foot.ini");
        link(&home.join("gone"), ".config/gone.conf");
        let config = Config {
            redaction: crate::config::Redaction {
                skip_paths: vec!["~/secrets/".into(), "private".into()],
                ..Default::default()
            },
            ..Config::default()
        };
        assert_eq!(
            h.paths(&config),
            ["~/.config/dots/foot.ini", "~/.config/foot/foot.ini"]
        );
    }

    #[test]
    fn a_path_the_redaction_would_change_or_with_a_control_character_is_left_out() {
        let h = Home::new("redact");
        h.file(".config/app/sekrit.conf", HOUR);
        h.file(".config/app/bad\u{1b}name.conf", HOUR);
        h.file(".config/app/bidi\u{202e}name.conf", HOUR);
        h.file(".config/app/line\u{2028}sep.conf", HOUR);
        h.file(".config/app/para\u{2029}sep.conf", HOUR);
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
        // a FIFO or a link to /dev/zero is never opened or read (B2)
        std::fs::remove_file(Saved::file(&h.dirs)).unwrap();
        let fifo = std::process::Command::new("mkfifo")
            .arg(Saved::file(&h.dirs))
            .status()
            .is_ok_and(|s| s.success());
        if fifo {
            assert_eq!(shown_at(&config, h.now, &mut warnings), None);
            assert_eq!(
                warnings.pop().as_deref(),
                Some("~/.local/state/seldon/recent-config.json: not read (not a regular file)")
            );
            std::fs::remove_file(Saved::file(&h.dirs)).unwrap();
        }
        std::os::unix::fs::symlink("/dev/zero", Saved::file(&h.dirs)).unwrap();
        assert_eq!(shown_at(&config, h.now, &mut warnings), None);
        assert_eq!(
            warnings.pop().as_deref(),
            Some("~/.local/state/seldon/recent-config.json: not read (a symbolic link)")
        );
        std::fs::remove_file(Saved::file(&h.dirs)).unwrap();
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
