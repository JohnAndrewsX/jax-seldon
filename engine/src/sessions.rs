//! The windows the engine opened and that are still open (WP-156,
//! ADR-0041): an agent `seldon agent start` launched, an editor `seldon
//! open --editor` started.
//!
//! A session is a **Hyprland window** of an Omarchy launcher class whose
//! process, or one of its descendants, carries the engine's marker in its
//! environment (`SELDON_CASE` + `SELDON_LOGBOOK` for an agent,
//! `SELDON_OPEN` for an editor; ADR-0030 §1). No launcher reports its
//! window or a pid that survives (`omarchy-launch-tui` runs `setsid`, which
//! forks), but the environment survives the whole chain into the terminal
//! and what runs in it. Starting from the windows means a process the
//! agent left behind without a window (a daemon, an ssh master) is no
//! session, and only the processes of those windows are read: never the
//! whole process table.
//!
//! Windows come from `hyprctl clients -j` (read-only); without it there is
//! no session tracking. For each window of a matching class the engine
//! walks the window's process and its descendants
//! (`/proc/<pid>/task/<tid>/children`, at most [`TREE_MAX`] processes) and
//! reads each one's `/proc/<pid>/environ` — the user's own processes only;
//! others fail and are skipped — at most [`ENVIRON_MAX`] bytes, keeping
//! only the keys asked for. A window is focused the way
//! `omarchy-launch-or-focus` does it: `hl.dsp.focus({ window = "address:…"
//! })`, then the legacy `focuswindow address:…`; the address goes into that
//! expression only after it is checked as `0x` and hex.

use std::ffi::OsStr;
use std::io::Read as _;
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use crate::sys::{self, Run};

/// Where the kernel lists the processes.
pub const PROC: &str = "/proc";

/// Set to the path for an editor `open --editor` started without a
/// terminal: the marker by which a second open of the same path finds it.
pub const OPEN_ENV: &str = "SELDON_OPEN";

/// The class of every agent window `omarchy agent` opens.
pub const AGENT_CLASS: &str = "org.omarchy.agent";

/// The class prefix of the terminal windows `omarchy-launch-tui` opens
/// (`org.omarchy.<program>`, e.g. `org.omarchy.nvim`).
pub const TUI_CLASS_PREFIX: &str = "org.omarchy.";

/// How much of one process's environment is read.
pub const ENVIRON_MAX: u64 = 64 * 1024;

/// How many processes of one window's tree are looked at.
pub const TREE_MAX: usize = 256;

/// How long a `hyprctl` call may take.
const HYPRCTL_TIMEOUT: Duration = Duration::from_secs(3);

/// One Hyprland window, as `hyprctl clients -j` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Window {
    pub address: String,
    pub pid: i64,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub workspace: Workspace,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Workspace {
    #[serde(default)]
    pub name: String,
}

/// The windows Hyprland has (`hyprctl clients -j`). `Err` says why not:
/// then nothing is tracked.
pub fn windows() -> Result<Vec<Window>, String> {
    // a window list is far below the cap; a cut one is no answer (WP-154)
    match sys::run(
        "hyprctl",
        &["clients", "-j"],
        None,
        HYPRCTL_TIMEOUT,
        sys::OUTPUT_MAX,
    ) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => serde_json::from_str(&stdout)
            .map_err(|e| format!("hyprctl clients -j gave no window list ({e})")),
        Run::NotFound => Err("hyprctl not found: windows are Hyprland's".into()),
        other => Err(format!("hyprctl clients -j failed: {}", failure(&other))),
    }
}

/// A window whose tree carries the marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marked {
    pub window: Window,
    /// The marked processes of its tree, oldest (by start time) first.
    pub pids: Vec<u32>,
    /// The values of the `report` keys the oldest one carries.
    pub values: Vec<Option<String>>,
}

/// The windows of `windows` whose class `class` accepts and whose process
/// or a descendant (other than this process) carries every `(key, value)`
/// of `want`, with the `report` values of the oldest marked process.
pub fn marked_windows(
    proc: &Path,
    windows: &[Window],
    class: impl Fn(&str) -> bool,
    want: &[(&str, &OsStr)],
    report: &[&str],
) -> Vec<Marked> {
    let me = std::process::id();
    windows
        .iter()
        .filter(|w| class(&w.class))
        .filter_map(|w| {
            let root = u32::try_from(w.pid).ok().filter(|&p| p > 1)?;
            let mut found: Vec<(u64, u32, Vec<u8>)> = tree(proc, root, TREE_MAX)
                .into_iter()
                .filter(|&pid| pid != me)
                .filter_map(|pid| {
                    let environ = environ(proc, pid)?;
                    want.iter()
                        .all(|(k, v)| value(&environ, k) == Some(v.as_bytes()))
                        .then(|| (start_time(proc, pid).unwrap_or(u64::MAX), pid, environ))
                })
                .collect();
            found.sort_by_key(|(start, pid, _)| (*start, *pid));
            let (_, _, oldest) = found.first()?;
            let values = report
                .iter()
                .map(|k| value(oldest, k).map(|v| String::from_utf8_lossy(v).into_owned()))
                .collect();
            Some(Marked {
                window: w.clone(),
                pids: found.iter().map(|(_, pid, _)| *pid).collect(),
                values,
            })
        })
        .collect()
}

/// `root` and its descendants, breadth first, at most `max`.
pub fn tree(proc: &Path, root: u32, max: usize) -> Vec<u32> {
    let mut out = vec![root];
    let mut i = 0;
    while i < out.len() && out.len() < max {
        let pid = out[i];
        i += 1;
        let Ok(tasks) = std::fs::read_dir(proc.join(pid.to_string()).join("task")) else {
            continue;
        };
        for task in tasks.filter_map(|t| t.ok()) {
            let Ok(text) = std::fs::read_to_string(task.path().join("children")) else {
                continue;
            };
            for child in text
                .split_whitespace()
                .filter_map(|c| c.parse::<u32>().ok())
            {
                if out.len() >= max {
                    break;
                }
                // a hand-made /proc with a loop must not run forever
                if !out.contains(&child) {
                    out.push(child);
                }
            }
        }
    }
    out
}

/// The first [`ENVIRON_MAX`] bytes of `pid`'s environment; `None` when it
/// is gone or not the user's.
fn environ(proc: &Path, pid: u32) -> Option<Vec<u8>> {
    let file = std::fs::File::open(proc.join(pid.to_string()).join("environ")).ok()?;
    let mut bytes = Vec::new();
    file.take(ENVIRON_MAX).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// The value of `key` in a NUL-separated environment block (the first
/// entry wins, as `getenv` reads it).
fn value<'a>(environ: &'a [u8], key: &str) -> Option<&'a [u8]> {
    environ.split(|b| *b == 0).find_map(|entry| {
        entry
            .strip_prefix(key.as_bytes())
            .and_then(|rest| rest.strip_prefix(b"="))
    })
}

/// When `pid` started, in clock ticks after boot (`/proc/<pid>/stat` field
/// 22, read past the command name in parentheses, which may itself hold
/// `)` and spaces).
pub fn start_time(proc: &Path, pid: u32) -> Option<u64> {
    let stat = std::fs::read(proc.join(pid.to_string()).join("stat")).ok()?;
    let close = stat.iter().rposition(|b| *b == b')')?;
    let rest = std::str::from_utf8(&stat[close + 1..]).ok()?;
    // field 3 (state) is the first after the name: field 22 is the 20th
    rest.split_whitespace().nth(19)?.parse().ok()
}

/// Whether `address` is a Hyprland window address: `0x` and 1–16 hex
/// digits. Nothing else goes into a dispatch.
pub fn is_address(address: &str) -> bool {
    address
        .strip_prefix("0x")
        .is_some_and(|h| (1..=16).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Focuses the window at `address`, Omarchy's way (`omarchy-launch-or-
/// focus`): the Lua dispatcher first, the legacy one when it fails. A
/// dispatch succeeded when hyprctl printed `ok`.
pub fn focus(address: &str) -> Result<(), String> {
    if !is_address(address) {
        return Err(format!(
            "`{}` is not a window address",
            address.escape_debug()
        ));
    }
    let lua = format!("hl.dsp.focus({{ window = \"address:{address}\" }})");
    let legacy = format!("address:{address}");
    let mut last = String::new();
    for args in [
        &["dispatch", lua.as_str()][..],
        &["dispatch", "focuswindow", legacy.as_str()][..],
    ] {
        match sys::run("hyprctl", args, None, HYPRCTL_TIMEOUT, sys::OUTPUT_MAX) {
            Run::Exited {
                code: Some(0),
                stdout,
                ..
            } if stdout.trim() == "ok" => return Ok(()),
            Run::NotFound => {
                return Err("hyprctl not found: focusing a window needs Hyprland".into());
            }
            other => last = failure(&other),
        }
    }
    Err(format!("hyprctl could not focus {address}: {last}"))
}

/// One line for a failed `hyprctl` run.
fn failure(run: &Run) -> String {
    match run {
        Run::Exited {
            code,
            stdout,
            stderr,
        } => {
            let text = format!("{} {}", stdout.trim(), stderr.trim());
            let text: String = text.trim().chars().take(200).collect();
            format!(
                "exit {}{}",
                code.map_or("by a signal".to_string(), |c| c.to_string()),
                if text.is_empty() {
                    String::new()
                } else {
                    format!(": {text}")
                }
            )
        }
        Run::Cut => "answered more than 1 MiB".into(),
        Run::NotFound => "not found".into(),
        Run::TimedOut => "did not return".into(),
        Run::Failed(e) => e.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Proc(PathBuf);
    impl Drop for Proc {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A hand-made `/proc`: `(pid, start time, children, environ)`.
    fn proc_tree(tag: &str, procs: &[(u32, u64, &[u32], &[&str])]) -> Proc {
        let dir = std::env::temp_dir().join(format!(
            "seldon-sessions-{tag}-{}-{}",
            std::process::id(),
            sys::random_hex(4)
        ));
        for (pid, start, children, env) in procs {
            let p = dir.join(pid.to_string());
            std::fs::create_dir_all(p.join("task").join(pid.to_string())).unwrap();
            let mut block = Vec::new();
            for e in *env {
                block.extend_from_slice(e.as_bytes());
                block.push(0);
            }
            std::fs::write(p.join("environ"), block).unwrap();
            let kids: Vec<String> = children.iter().map(|c| c.to_string()).collect();
            std::fs::write(
                p.join("task").join(pid.to_string()).join("children"),
                kids.join(" ") + " ",
            )
            .unwrap();
            // fields 3..22: state, then 18 numbers, then the start time
            let middle = vec!["0"; 18].join(" ");
            std::fs::write(
                p.join("stat"),
                format!("{pid} (a ) b) S {middle} {start} 0 0"),
            )
            .unwrap();
        }
        Proc(dir)
    }

    fn os(s: &str) -> &OsStr {
        OsStr::new(s)
    }

    fn window(address: &str, pid: i64, class: &str) -> Window {
        Window {
            address: address.into(),
            pid,
            class: class.into(),
            workspace: Workspace { name: "2".into() },
        }
    }

    const MARK: [&str; 3] = [
        "SELDON_CASE=C-2026-005",
        "SELDON_LOGBOOK=/l",
        "SELDON_ACTOR=agent:default",
    ];

    fn want() -> [(&'static str, &'static OsStr); 2] {
        [
            ("SELDON_CASE", os("C-2026-005")),
            ("SELDON_LOGBOOK", os("/l")),
        ]
    }

    #[test]
    fn a_window_counts_by_its_own_marker_or_a_descendants() {
        let p = proc_tree(
            "tree",
            &[
                (10, 500, &[11], &[]),
                (
                    11,
                    600,
                    &[12],
                    &["SELDON_LOGBOOK=/l", "SELDON_CASE=C-2026-005"],
                ),
                (12, 550, &[], &MARK),
                (20, 100, &[], &MARK),
                (30, 100, &[], &MARK),
                (
                    40,
                    100,
                    &[],
                    &["SELDON_CASE=C-2026-005", "SELDON_LOGBOOK=/other"],
                ),
            ],
        );
        let ws = [
            window("0x1", 10, AGENT_CLASS),
            window("0x2", 20, AGENT_CLASS),
            window("0x3", 30, "foot"),
            window("0x4", 40, AGENT_CLASS),
            window("0x5", 99, AGENT_CLASS),
        ];
        let got = marked_windows(&p.0, &ws, |c| c == AGENT_CLASS, &want(), &["SELDON_ACTOR"]);
        let short: Vec<(String, Vec<u32>, Vec<Option<String>>)> = got
            .iter()
            .map(|m| (m.window.address.clone(), m.pids.clone(), m.values.clone()))
            .collect();
        assert_eq!(
            short,
            [
                // 12 started before 11 although its pid is higher: oldest first
                (
                    "0x1".into(),
                    vec![12, 11],
                    vec![Some("agent:default".into())]
                ),
                ("0x2".into(), vec![20], vec![Some("agent:default".into())]),
            ]
        );
    }

    #[test]
    fn the_environment_is_read_up_to_its_cap() {
        let pad = format!("PAD={}", "x".repeat(ENVIRON_MAX as usize));
        let early: Vec<&str> = MARK.iter().copied().chain([pad.as_str()]).collect();
        let late: Vec<&str> = [pad.as_str()].into_iter().chain(MARK).collect();
        let p = proc_tree("cap", &[(10, 1, &[], &early), (20, 1, &[], &late)]);
        let ws = [
            window("0x1", 10, AGENT_CLASS),
            window("0x2", 20, AGENT_CLASS),
        ];
        let got = marked_windows(&p.0, &ws, |_| true, &want(), &[]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].window.address, "0x1");
    }

    #[test]
    fn the_tree_is_capped_and_survives_a_loop() {
        let p = proc_tree(
            "loop",
            &[
                (10, 1, &[11, 12], &[]),
                (11, 1, &[10], &[]),
                (12, 1, &[], &[]),
            ],
        );
        assert_eq!(tree(&p.0, 10, 10), [10, 11, 12]);
        assert_eq!(tree(&p.0, 10, 2), [10, 11]);
        assert_eq!(tree(&p.0, 77, 10), [77], "a pid that is gone");
    }

    #[test]
    fn the_start_time_reads_past_a_name_with_parentheses() {
        let p = proc_tree("stat", &[(20, 4242, &[], &[])]);
        assert_eq!(start_time(&p.0, 20), Some(4242));
        assert_eq!(start_time(&p.0, 99), None);
    }

    #[test]
    fn the_first_entry_of_a_key_wins() {
        assert_eq!(value(b"A=1\0A=2\0", "A"), Some(&b"1"[..]));
        assert_eq!(value(b"AB=1\0A=\0", "A"), Some(&b""[..]));
        assert_eq!(value(b"A\0", "A"), None);
    }

    #[test]
    fn only_a_hex_address_is_dispatched() {
        assert!(is_address("0x59f9ac1387b0"));
        assert!(is_address("0xABC"));
        for bad in [
            "",
            "0x",
            "59f9",
            "0x59f9\" }) os.execute(\"id",
            "0xg",
            "0x12345678901234567",
            "address:0x1",
        ] {
            assert!(!is_address(bad), "{bad}");
            assert!(
                focus(bad).unwrap_err().contains("not a window address"),
                "{bad}"
            );
        }
    }
}
