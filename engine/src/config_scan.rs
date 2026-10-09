//! Recently edited files under `~/.config` (WP-138, WP-139: the one walker
//! of both).
//!
//! A bounded walk of `~/.config` by modification time only: paths and
//! times, never content, never a hash. It feeds `seldon preview` before
//! the logbook exists, and every capture's list of recently edited files
//! outside the watched paths (`collectors::recent`, ADR-0046).
//!
//! - **What counts.** Regular files modified at or after the window's
//!   start, and links to regular files by their target's time (the target
//!   is stat'ed, never opened; the link's own path is listed: stow-style
//!   dotfiles). A link to a folder is never entered; special files are
//!   skipped.
//! - **Order.** Breadth-first, each folder's entries by name: every file
//!   one level down is seen before any two levels down, so one heavy
//!   folder cannot spend the entry budget before the shallow config files
//!   elsewhere are read (WP-139 round 2, B3).
//! - **What is ignored** ([`ignored_dir`], [`ignored_file`]): caches (any
//!   folder whose name holds `cache`), `node_modules`, browser and
//!   Electron profiles (any folder below the root holding `Cookies` or
//!   `Local State`), state, history, logs, locks, crash reports, databases
//!   (SQLite, `*.db`, LevelDB, IndexedDB, Local and Session Storage,
//!   dconf), key stores, images, `.git`, `shell.json`, editor swap and temp
//!   files, `*~` and `*.bak.*` backups, every `[redaction] skipPaths`
//!   match (a name pattern matches a folder's name too), what the caller
//!   excludes ([`Limits::exclude`], before a folder is entered) and what
//!   its `keep` refuses (a folder is then not entered either).
//! - **Bounds** ([`Limits`]): the newest `max_files` the caller keeps; the
//!   walk stops at a deadline or at `max_entries` directory entries and
//!   does not go below [`MAX_DEPTH`] folders; any of the three makes the
//!   scan [`Scan::partial`].
//!
//! The walker never redacts: its caller decides what to show.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::collectors::config::SkipPaths;

/// The folder walked, under the home directory: `~/.config` for every
/// caller, whatever `$XDG_CONFIG_HOME` says (the index's paths start with
/// `~/.config/`; WP-139 round 2).
pub const ROOT: &str = ".config";

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
    /// Paths the walk leaves out: a folder with everything below it, a
    /// file by its path (`Path::starts_with`, whole components).
    pub exclude: Vec<PathBuf>,
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
    /// Directory entries read.
    pub entries: usize,
}

/// Names that mark a browser or Electron profile: the folder holding one
/// is skipped whole.
const PROFILE_MARKERS: [&str; 2] = ["Cookies", "Local State"];

/// Folder names skipped (compared lowercased), besides any holding
/// `cache`.
const IGNORED_DIRS: [&str; 17] = [
    ".git",
    "node_modules",
    "state",
    "history",
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
const IGNORED_EXTENSIONS: [&str; 34] = [
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
    "kdbx",
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
    "jxl",
    "xpm",
    "swp",
    "swo",
    "swx",
    "tmp",
];

/// Whether the folder `name` is skipped by its name alone.
pub fn ignored_dir(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("cache") || IGNORED_DIRS.contains(&lower.as_str())
}

/// Whether the file `name` is skipped by its name alone: the extensions
/// above; backups (`*~`, `*.bak.*`); `lock`, `state`, `*state.json`,
/// `history.json`, `shell.json` (Omarchy's shell rewrites it), Chromium's
/// `Singleton*`; rotated logs (`x.log.1`); temp files (`.#x`, `#x#`,
/// `x.tmp-…`, GTK's `.goutputstream-…`).
pub fn ignored_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    if lower.ends_with('~')
        || lower.contains(".bak.")
        || lower == "lock"
        || lower == "state"
        || lower == "shell.json"
        || lower == "history.json"
        || lower.ends_with("state.json")
        || lower.starts_with("singleton")
        || lower.contains(".log.")
        || lower.contains(".tmp-")
        || lower.starts_with(".#")
        || lower.starts_with(".goutputstream-")
        || (lower.len() > 1 && lower.starts_with('#') && lower.ends_with('#'))
    {
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
    scan_keeping(config_dir, skip, limits, &|_| true)
}

/// [`scan`] that also leaves out every path `keep` refuses, before the cut
/// to `max_files` (a folder it refuses is not entered): the caller's own
/// rules for what it may show (`collectors::recent`: UTF-8, no invisible
/// character, the redaction).
pub fn scan_keeping(
    config_dir: &Path,
    skip: &SkipPaths,
    limits: &Limits,
    keep: &dyn Fn(&Path) -> bool,
) -> Scan {
    let excluded = |path: &Path| limits.exclude.iter().any(|e| path.starts_with(e));
    let mut out = Scan::default();
    // (folder, depth below config_dir), breadth-first
    let mut queue = VecDeque::from([(config_dir.to_path_buf(), 0usize)]);
    'walk: while let Some((dir, depth)) = queue.pop_front() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children = Vec::new();
        for entry in read {
            out.entries += 1;
            if out.entries > limits.max_entries
                || limits.deadline.is_some_and(|d| Instant::now() >= d)
            {
                out.entries -= 1;
                out.partial = true;
                break 'walk;
            }
            let Ok(entry) = entry else { continue };
            let name = entry.file_name();
            if depth > 0 && PROFILE_MARKERS.iter().any(|m| name == *m) {
                // a profile: none of this folder is listed
                continue 'walk;
            }
            children.push(entry);
        }
        children.sort_by_key(|e| e.file_name());
        for entry in children {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if skip.matches(&path) || excluded(&path) {
                continue;
            }
            if kind.is_dir() {
                if ignored_dir(&name) || !keep(&path) {
                    continue;
                }
                if depth + 1 > MAX_DEPTH {
                    out.partial = true;
                    continue;
                }
                queue.push_back((path, depth + 1));
            } else if kind.is_file() || kind.is_symlink() {
                if ignored_file(&name) {
                    continue;
                }
                // a link: its target's time, never its content, and only
                // a target inside the root that the walk itself would list
                // (WP-139 round 3); a link to a folder is not entered
                let meta = if kind.is_file() {
                    entry.metadata()
                } else {
                    match resolve_within(config_dir, &path) {
                        Some(target) if listable_target(config_dir, &target, skip, &excluded) => {
                            std::fs::symlink_metadata(&target)
                        }
                        _ => continue,
                    }
                };
                let Ok(modified) = meta.and_then(|m| {
                    if m.is_file() {
                        m.modified()
                    } else {
                        Err(std::io::ErrorKind::InvalidInput.into())
                    }
                }) else {
                    continue;
                };
                if modified >= limits.since && keep(&path) {
                    out.matched += 1;
                    keep_newest(&mut out.files, Recent { path, modified }, limits.max_files);
                }
            }
        }
    }
    out
}

/// Whether the walk itself would list `target` (a link's resolved target,
/// below `root`): neither it nor a folder above it below `root` is skipped
/// by name, matched by `skip` or excluded.
fn listable_target(
    root: &Path,
    target: &Path,
    skip: &SkipPaths,
    excluded: &dyn Fn(&Path) -> bool,
) -> bool {
    let name = target.file_name().map(|n| n.to_string_lossy());
    !name.is_some_and(|n| ignored_file(&n))
        && !target
            .ancestors()
            .take_while(|p| *p != root && p.starts_with(root))
            .any(|p| {
                skip.matches(p)
                    || excluded(p)
                    || (p != target
                        && p.file_name()
                            .is_some_and(|n| ignored_dir(&n.to_string_lossy())))
            })
}

/// Links resolved at most per path (as the kernel's `ELOOP` limit).
const MAX_HOPS: usize = 40;

/// Where `path` (below `root`) leads once every link on the way is
/// resolved, without looking at anything outside `root` (AGENTS.md §6,
/// E41: paths and times under `~/.config` only): a link whose target
/// leaves `root` — an absolute path elsewhere, or `..` above it — ends the
/// walk with `None`, before anything there is touched, as do a loop
/// ([`MAX_HOPS`]) and a part that cannot be read. An absolute target may
/// name `root` as written or as its canonical form (`~/.config` may itself
/// be a link). The result is written below `root` as given.
pub fn resolve_within(root: &Path, path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let canonical_root = std::fs::canonicalize(root).ok()?;
    // components still to resolve, the next one last
    let push = |pending: &mut Vec<std::ffi::OsString>, rest: &Path| -> Option<()> {
        let mut parts = Vec::new();
        for c in rest.components() {
            match c {
                Component::Normal(n) => parts.push(n.to_os_string()),
                Component::ParentDir => parts.push("..".into()),
                Component::CurDir => {}
                Component::RootDir | Component::Prefix(_) => return None,
            }
        }
        pending.extend(parts.into_iter().rev());
        Some(())
    };
    let mut pending = Vec::new();
    push(&mut pending, path.strip_prefix(root).ok()?)?;
    let mut resolved: Vec<std::ffi::OsString> = Vec::new();
    let mut hops = 0;
    while let Some(part) = pending.pop() {
        if part == ".." {
            // above the root: outside
            resolved.pop()?;
            continue;
        }
        let candidate: PathBuf = std::iter::once(root.as_os_str())
            .chain(resolved.iter().map(|p| p.as_os_str()))
            .chain(std::iter::once(part.as_os_str()))
            .collect();
        let meta = std::fs::symlink_metadata(&candidate).ok()?;
        if !meta.file_type().is_symlink() {
            resolved.push(part);
            continue;
        }
        hops += 1;
        if hops > MAX_HOPS {
            return None;
        }
        let target = std::fs::read_link(&candidate).ok()?;
        if target.is_absolute() {
            let rest = target
                .strip_prefix(root)
                .or_else(|_| target.strip_prefix(&canonical_root))
                .ok()?;
            resolved.clear();
            push(&mut pending, rest)?;
        } else {
            push(&mut pending, &target)?;
        }
    }
    Some(
        std::iter::once(root.as_os_str())
            .chain(resolved.iter().map(|p| p.as_os_str()))
            .collect(),
    )
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
            exclude: Vec::new(),
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

    /// The union of WP-138's and WP-139's lists, plus `node_modules`
    /// (WP-139 round 2).
    #[test]
    fn the_union_ignore_list() {
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
            "node_modules",
            "crashpad",
            "Crash Reports",
            "dconf",
            "Sentry",
            "WebStorage",
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
            "hyprland.conf.bak.1728",
            "cover.jxl",
            "icons.xpm",
            ".init.lua.swx",
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
            // shell.json anywhere, node_modules (WP-139 round 2)
            "omarchy/themed/shell.json",
            "coc/extensions/node_modules/x/index.js",
        ] {
            file(root, rel, now);
        }
        // kept: the same names one level off, and ordinary files
        for rel in [
            "omarchy/hooks/theme-set",
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
            ]
        );
    }

    #[test]
    fn the_caller_excludes_folders_and_files() {
        let t = tmp("exclude");
        let root = &t.0;
        for rel in [
            "omarchy/plugins/jax.seldon/manifest.json",
            "omarchy/plugins-old/a.conf",
            "omarchy/settings.json",
            "omarchy/settings.json.d/x.conf",
            "omarchy/themed/settings.json",
        ] {
            file(root, rel, 2 * DAY);
        }
        let mut all = names(&scan(root, &SkipPaths::default(), &limits()), root);
        all.sort();
        assert_eq!(all.len(), 5, "{all:?}");
        let excluding = Limits {
            exclude: vec![
                root.join("omarchy/plugins"),
                root.join("omarchy/settings.json"),
            ],
            ..limits()
        };
        let mut got = names(&scan(root, &SkipPaths::default(), &excluding), root);
        got.sort();
        // whole components: plugins-old and settings.json.d stay
        assert_eq!(
            got,
            [
                "omarchy/plugins-old/a.conf",
                "omarchy/settings.json.d/x.conf",
                "omarchy/themed/settings.json",
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

    /// WP-139 round 3: a link to a folder is never entered; a link to a
    /// file is listed under its own path with its target's time only when
    /// the target, resolved without leaving the root, is a regular file the
    /// walk itself would list (not skipped, not excluded, not ignored by
    /// name); a dangling link, a loop and a link out of the root are
    /// nothing.
    #[test]
    fn a_file_link_counts_only_with_a_listable_target_inside_the_root() {
        let t = tmp("links");
        let root = &t.0.join("config");
        let outside = &t.0.join("outside");
        file(outside, "big/a.conf", 2 * DAY);
        file(outside, "dotfiles/kitty.conf", 2 * DAY);
        file(root, "real.conf", 2 * DAY - 120);
        file(root, "dots/foot.ini", 2 * DAY - 60);
        file(root, "secret/token", 2 * DAY);
        file(root, "own/index.json", 2 * DAY);
        file(root, "keys/vault.kdbx", 2 * DAY);
        let link = |to: &Path, at: &str| std::os::unix::fs::symlink(to, root.join(at)).unwrap();
        link(&outside.join("big"), "linked-dir");
        link(&outside.join("dotfiles/kitty.conf"), "out-absolute.conf");
        link(
            Path::new("../outside/dotfiles/kitty.conf"),
            "out-relative.conf",
        );
        link(Path::new("/proc/self/status"), "proc.conf");
        link(&root.join("dots/foot.ini"), "in-absolute.ini");
        link(Path::new("dots/foot.ini"), "in-relative.ini");
        link(Path::new("in-relative.ini"), "in-chain.ini");
        link(Path::new("secret/token"), "token.conf");
        link(Path::new("own/index.json"), "ownlink.json");
        link(Path::new("keys/vault.kdbx"), "vault.conf");
        link(Path::new("loop-b.conf"), "loop-a.conf");
        link(Path::new("loop-a.conf"), "loop-b.conf");
        link(&outside.join("gone"), "dangling.conf");
        let limits = Limits {
            exclude: vec![root.join("own")],
            ..limits()
        };
        let skip = SkipPaths::new(&t.0, &["secret".to_string()]);
        let scan = scan(root, &skip, &limits);
        let mut got = names(&scan, root);
        got.sort();
        assert_eq!(
            got,
            [
                "dots/foot.ini",
                "in-absolute.ini",
                "in-chain.ini",
                "in-relative.ini",
                "real.conf"
            ]
        );
        let chain = scan
            .files
            .iter()
            .find(|f| f.path.ends_with("in-chain.ini"))
            .unwrap();
        assert_eq!(chain.modified, base() + Duration::from_secs(2 * DAY - 60));
        // the resolution itself never leaves the root
        assert_eq!(resolve_within(root, &root.join("out-relative.conf")), None);
        assert_eq!(resolve_within(root, &root.join("out-absolute.conf")), None);
        assert_eq!(resolve_within(root, &root.join("loop-a.conf")), None);
        assert_eq!(
            resolve_within(root, &root.join("in-chain.ini")),
            Some(root.join("dots/foot.ini"))
        );
        // a root that is itself a link: targets in either spelling
        let linked_root = t.0.join("config-link");
        std::os::unix::fs::symlink(root, &linked_root).unwrap();
        assert_eq!(
            resolve_within(&linked_root, &linked_root.join("in-absolute.ini")),
            Some(linked_root.join("dots/foot.ini"))
        );
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

    /// WP-139 round 2 (B3): breadth-first, so one heavy folder cannot spend
    /// the entry budget before a shallow config file elsewhere is read
    /// (depth-first, `zzz` would be walked to its end first).
    #[test]
    fn breadth_first_reads_shallow_files_before_a_heavy_folder() {
        let t = tmp("breadth");
        let root = &t.0;
        for i in 0..50 {
            file(root, &format!("zzz/x/y/f{i:02}.js"), 2 * DAY);
        }
        file(root, "aaa/app.conf", 2 * DAY - 60);
        let tight = Limits {
            max_entries: 10,
            ..limits()
        };
        let scan = scan(root, &SkipPaths::default(), &tight);
        assert!(scan.partial);
        assert_eq!(scan.entries, 10);
        assert!(names(&scan, root).contains(&"aaa/app.conf".to_string()));
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
