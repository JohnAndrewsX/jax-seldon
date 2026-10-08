//! Recently edited files under `~/.config` (WP-138, shared with WP-139).
//!
//! A bounded walk of the config directory (`$XDG_CONFIG_HOME`, default
//! `~/.config`) by modification time only: paths and times, never content,
//! never a hash. It feeds `seldon preview` before the logbook exists; WP-139
//! runs it during capture for the files outside the watched paths.
//!
//! - **What counts.** Regular files modified at or after the window's
//!   start. Symbolic links are never followed (a link to a folder is not
//!   entered, a link to a file is not listed); special files are skipped.
//! - **What is ignored** ([`ignored_dir`], [`ignored_file`]): caches (any
//!   folder whose name holds `cache`), browser and Electron profiles (any
//!   folder holding `Cookies` or `Local State`), state, logs, locks,
//!   databases (SQLite, `*.db`, LevelDB, IndexedDB, Local and Session
//!   Storage, dconf), images, `.git`, Omarchy's plugin folder
//!   (`omarchy/plugins`), `omarchy/shell.json` (the shell's own settings),
//!   editor swap files, `*~` and `*.bak.*` backups, and every
//!   `[redaction] skipPaths` match.
//! - **Bounds** ([`Limits`]): the newest `max_files`; the walk stops at a
//!   deadline, at `max_entries` directory entries or below `MAX_DEPTH`
//!   folders and then says [`Scan::partial`].

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::collectors::config::SkipPaths;

/// Folders below the config directory the walk enters at most.
pub const MAX_DEPTH: usize = 16;

/// How far the walk may go.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Files modified before this are not listed.
    pub since: SystemTime,
    /// The newest this many files are kept.
    pub max_files: usize,
    /// The walk stops here (`None`: no deadline).
    pub deadline: Option<Instant>,
    /// The walk stops after reading this many directory entries.
    pub max_entries: usize,
}

/// One recently modified file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recent {
    pub path: PathBuf,
    pub modified: SystemTime,
}

/// What the walk found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    /// Newest first, at most [`Limits::max_files`]; equal times by path.
    pub files: Vec<Recent>,
    /// Every file in the window the walk saw, also those cut by
    /// `max_files`.
    pub matched: usize,
    /// The walk stopped early (deadline, entry budget or depth): files it
    /// did not reach are missing.
    pub partial: bool,
}

/// Names that mark a browser or Electron profile: the folder holding one
/// is skipped whole.
const PROFILE_MARKERS: [&str; 2] = ["Cookies", "Local State"];

/// Folder names skipped (compared lowercased), besides any holding
/// `cache`.
const IGNORED_DIRS: [&str; 15] = [
    ".git",
    "state",
    "log",
    "logs",
    "crashpad",
    "crash reports",
    "indexeddb",
    "local storage",
    "session storage",
    "databases",
    "blob_storage",
    "leveldb",
    "dconf",
    "sentry",
    "webstorage",
];

/// File extensions skipped (compared lowercased).
const IGNORED_EXTENSIONS: [&str; 30] = [
    "log",
    "lock",
    "lck",
    "pid",
    "state",
    "db",
    "db-wal",
    "db-shm",
    "db-journal",
    "sqlite",
    "sqlite3",
    "sqlite-wal",
    "sqlite-shm",
    "sqlite-journal",
    "ldb",
    "png",
    "jpg",
    "jpeg",
    "gif",
    "webp",
    "svg",
    "ico",
    "bmp",
    "tif",
    "tiff",
    "avif",
    "heic",
    "swp",
    "swo",
    "tmp",
];

/// Whether the folder `name` is skipped by its name alone.
pub fn ignored_dir(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("cache") || IGNORED_DIRS.contains(&lower.as_str())
}

/// Whether the file `name` is skipped by its name alone.
pub fn ignored_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    if lower.ends_with('~') || lower.contains(".bak.") || lower == "lock" {
        return true;
    }
    match lower.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => IGNORED_EXTENSIONS.contains(&ext),
        _ => false,
    }
}

/// Walks `config_dir` (the config directory itself; missing: an empty
/// scan) for files modified within `limits`, skipping what the module
/// documentation lists and what `skip` matches.
pub fn scan(config_dir: &Path, skip: &SkipPaths, limits: &Limits) -> Scan {
    let plugins = config_dir.join("omarchy").join("plugins");
    let shell_json = config_dir.join("omarchy").join("shell.json");
    let mut out = Scan::default();
    let mut entries = 0usize;
    // (folder, depth below config_dir)
    let mut stack = vec![(config_dir.to_path_buf(), 0usize)];
    'walk: while let Some((dir, depth)) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children = Vec::new();
        for entry in read {
            entries += 1;
            if entries > limits.max_entries || limits.deadline.is_some_and(|d| Instant::now() >= d)
            {
                out.partial = true;
                break 'walk;
            }
            let Ok(entry) = entry else { continue };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if depth > 0 && PROFILE_MARKERS.contains(&name.as_ref()) {
                // a profile: none of this folder is listed
                children.clear();
                continue 'walk;
            }
            children.push(entry);
        }
        for entry in children {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if skip.matches(&path) {
                continue;
            }
            if kind.is_dir() {
                if ignored_dir(&name) || path == plugins {
                    continue;
                }
                if depth + 1 > MAX_DEPTH {
                    out.partial = true;
                    continue;
                }
                stack.push((path, depth + 1));
            } else if kind.is_file() {
                if ignored_file(&name) || path == shell_json {
                    continue;
                }
                let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
                    continue;
                };
                if modified >= limits.since {
                    out.matched += 1;
                    keep_newest(&mut out.files, Recent { path, modified }, limits.max_files);
                }
            }
        }
    }
    out
}

/// Inserts `item` into `files` (newest first, equal times by path) and
/// keeps at most `max`.
fn keep_newest(files: &mut Vec<Recent>, item: Recent, max: usize) {
    let at = files.partition_point(|f| {
        f.modified > item.modified || (f.modified == item.modified && f.path < item.path)
    });
    if at >= max {
        return;
    }
    files.insert(at, item);
    files.truncate(max);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, FileTimes};
    use std::time::Duration;

    struct Tmp(PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn tmp(tag: &str) -> Tmp {
        let dir =
            std::env::temp_dir().join(format!("seldon-config-scan-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    /// A day in seconds.
    const DAY: u64 = 86_400;

    fn base() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_000_000)
    }

    /// Writes `rel` under `root` with its mtime `secs` after [`base`].
    fn file(root: &Path, rel: &str, secs: u64) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "x").unwrap();
        let f = File::options().write(true).open(&path).unwrap();
        f.set_times(FileTimes::new().set_modified(base() + Duration::from_secs(secs)))
            .unwrap();
    }

    fn limits() -> Limits {
        Limits {
            since: base() + Duration::from_secs(DAY),
            max_files: 80,
            deadline: None,
            max_entries: 100_000,
        }
    }

    fn names(scan: &Scan, root: &Path) -> Vec<String> {
        scan.files
            .iter()
            .map(|f| {
                f.path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn lists_files_in_the_window_newest_first() {
        let t = tmp("window");
        let root = &t.0;
        file(root, "alacritty/alacritty.toml", 3 * DAY);
        file(root, "git/config", 2 * DAY);
        file(root, "starship.toml", DAY); // the window's first second counts
        file(root, "old.conf", DAY - 1);
        file(root, "b.conf", 2 * DAY);
        let scan = scan(root, &SkipPaths::default(), &limits());
        assert_eq!(
            names(&scan, root),
            [
                "alacritty/alacritty.toml",
                "b.conf",
                "git/config",
                "starship.toml"
            ]
        );
        assert_eq!((scan.matched, scan.partial), (4, false));
    }

    #[test]
    fn the_ignore_list_holds() {
        let t = tmp("ignore");
        let root = &t.0;
        let now = 2 * DAY;
        for rel in [
            "chromium/Local State",
            "chromium/Default/Preferences",
            "Code/User/settings.json",
            "Code/Cookies",
            "app/Cache/data_1",
            "app/GPUCache/index",
            "app/Code Cache/js/x",
            "app/state/now.json",
            "app/logs/main.txt",
            "app/IndexedDB/x/y",
            "app/Local Storage/leveldb/000003.log",
            "dconf/user",
            "app/history.sqlite",
            "app/places.db",
            "app/places.db-wal",
            "app/app.log",
            "app/app.lock",
            "app/lock",
            "app/run.pid",
            "app/window.state",
            "app/icon.png",
            "app/pic.JPG",
            "app/.config.swp",
            "app/config.toml~",
            "hypr/hyprland.conf.bak.1759000000",
            "repo/.git/index",
            "omarchy/plugins/jax.seldon/manifest.json",
            "omarchy/shell.json",
        ] {
            file(root, rel, now);
        }
        // kept: the same names one level off, and ordinary files
        for rel in [
            "omarchy/hooks/theme-set",
            "omarchy/themed/shell.json",
            "nvim/lua/plugins.lua",
            "app/statefile.toml",
            ".dotfile",
            "app/.gitignore",
        ] {
            file(root, rel, now);
        }
        let mut got = names(&scan(root, &SkipPaths::default(), &limits()), root);
        got.sort();
        assert_eq!(
            got,
            [
                ".dotfile",
                "app/.gitignore",
                "app/statefile.toml",
                "nvim/lua/plugins.lua",
                "omarchy/hooks/theme-set",
                "omarchy/themed/shell.json",
            ]
        );
    }

    #[test]
    fn a_profile_marker_skips_its_whole_folder_but_not_the_root() {
        let t = tmp("profile");
        let root = &t.0;
        file(root, "Cookies", 2 * DAY);
        file(root, "a.conf", 2 * DAY);
        file(root, "electron-app/Cookies", 2 * DAY);
        file(root, "electron-app/settings.json", 2 * DAY);
        file(root, "electron-app/sub/deep.json", 2 * DAY);
        let got = names(&scan(root, &SkipPaths::default(), &limits()), root);
        assert_eq!(got, ["Cookies", "a.conf"]);
    }

    #[test]
    fn skip_paths_are_honoured() {
        let t = tmp("skip");
        let root = &t.0;
        file(root, "secrets/token.txt", 2 * DAY);
        file(root, "app/id_ed25519.key", 2 * DAY);
        file(root, "app/ok.toml", 2 * DAY);
        let skip = SkipPaths::new(
            Path::new("/nowhere"),
            &[format!("{}/secrets/", root.display()), "*.key".to_string()],
        );
        let got = names(&scan(root, &skip, &limits()), root);
        assert_eq!(got, ["app/ok.toml"]);
    }

    #[test]
    fn symlinks_are_never_followed() {
        let t = tmp("links");
        let root = &t.0.join("config");
        let outside = &t.0.join("outside");
        file(outside, "big/a.conf", 2 * DAY);
        file(root, "real.conf", 2 * DAY);
        std::os::unix::fs::symlink(outside.join("big"), root.join("linked-dir")).unwrap();
        std::os::unix::fs::symlink(outside.join("big/a.conf"), root.join("linked.conf")).unwrap();
        let got = names(&scan(root, &SkipPaths::default(), &limits()), root);
        assert_eq!(got, ["real.conf"]);
    }

    #[test]
    fn the_newest_max_files_are_kept_and_all_are_counted() {
        let t = tmp("bound");
        let root = &t.0;
        for i in 0..100u64 {
            file(root, &format!("d{}/f{i:03}.conf", i % 7), DAY + i * 60);
        }
        let scan = scan(root, &SkipPaths::default(), &limits());
        assert_eq!(
            (scan.files.len(), scan.matched, scan.partial),
            (80, 100, false)
        );
        assert!(
            scan.files
                .windows(2)
                .all(|w| w[0].modified >= w[1].modified)
        );
        assert!(scan.files[0].path.ends_with("f099.conf"));
        assert!(scan.files[79].path.ends_with("f020.conf"));
    }

    #[test]
    fn budgets_stop_the_walk_and_say_so() {
        let t = tmp("budget");
        let root = &t.0;
        for i in 0..50u64 {
            file(root, &format!("f{i:02}.conf"), 2 * DAY);
        }
        let few = Limits {
            max_entries: 10,
            ..limits()
        };
        let s = scan(root, &SkipPaths::default(), &few);
        assert!(s.partial);
        assert!(s.matched <= 10, "{}", s.matched);
        let past = Limits {
            deadline: Some(Instant::now()),
            ..limits()
        };
        let s = scan(root, &SkipPaths::default(), &past);
        assert_eq!((s.matched, s.partial), (0, true));
        let s = scan(root, &SkipPaths::default(), &limits());
        assert_eq!((s.matched, s.partial), (50, false));
    }

    #[test]
    fn depth_is_bounded() {
        let t = tmp("depth");
        let root = &t.0;
        let deep: Vec<String> = (0..=MAX_DEPTH).map(|i| format!("d{i}")).collect();
        file(root, &format!("{}/deep.conf", deep.join("/")), 2 * DAY);
        file(
            root,
            &format!("{}/ok.conf", deep[..MAX_DEPTH].join("/")),
            2 * DAY,
        );
        let s = scan(root, &SkipPaths::default(), &limits());
        assert_eq!(s.files.len(), 1);
        assert!(s.files[0].path.ends_with("ok.conf"));
        assert!(s.partial);
    }

    #[test]
    fn a_missing_root_is_an_empty_scan() {
        let s = scan(
            Path::new("/nonexistent/seldon-config-scan"),
            &SkipPaths::default(),
            &limits(),
        );
        assert_eq!(s, Scan::default());
    }
}
