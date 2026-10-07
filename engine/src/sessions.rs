//! The windows the engine opened and that still live (WP-156): an agent
//! `seldon agent start` launched, an editor `seldon open --editor` started.
//!
//! No launcher tells us its window or the pid that ends up owning it:
//! `omarchy-launch-tui` runs `setsid`, which forks because the engine
//! starts the launcher as a process-group leader, and the terminal is a
//! pid the engine never saw. What survives the whole chain is the
//! environment the engine gave the launcher (ADR-0030 §1: `SELDON_CASE`,
//! `SELDON_LOGBOOK`; `SELDON_OPEN` for an editor). So a session is found
//! by its marker in `/proc/<pid>/environ` — readable for the user's own
//! processes only; every other one is skipped — and only the keys asked
//! for are compared; nothing else of any environment is kept. No state
//! file: the marker dies with the session.
//!
//! Its window is the Hyprland client whose pid is a marked process, else
//! the nearest ancestor of one (`hyprctl clients -j`, read-only), focused
//! the way `omarchy-launch-or-focus` does: `hl.dsp.focus({ window =
//! "address:…" })`, then the legacy `focuswindow address:…`. The address
//! goes into that expression only after it is checked as `0x` and hex.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use crate::sys::{self, Run};

/// Where the kernel lists the processes.
pub const PROC: &str = "/proc";

/// Set to the path for an editor `open --editor` started without a
/// terminal: the marker by which a second open of the same file finds it.
pub const OPEN_ENV: &str = "SELDON_OPEN";

/// How long a `hyprctl` call may take.
const HYPRCTL_TIMEOUT: Duration = Duration::from_secs(3);

/// A live process that carries the marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marked {
    pub pid: u32,
    /// The values of the `report` keys it carries, in their order.
    pub values: Vec<Option<String>>,
}

/// The processes under `proc` (other than this one) whose environment
/// holds every `(key, value)` of `want`, with the values of `report`.
/// Oldest (lowest pid) first.
pub fn marked(proc: &Path, want: &[(&str, &OsStr)], report: &[&str]) -> Vec<Marked> {
    let me = std::process::id();
    let Ok(entries) = std::fs::read_dir(proc) else {
        return Vec::new();
    };
    let mut out: Vec<Marked> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter(|&pid| pid != me)
        .filter_map(|pid| {
            // gone, or another user's: not ours to see
            let environ = std::fs::read(proc.join(pid.to_string()).join("environ")).ok()?;
            let all = want
                .iter()
                .all(|(k, v)| value(&environ, k) == Some(v.as_bytes()));
            all.then(|| Marked {
                pid,
                values: report
                    .iter()
                    .map(|k| value(&environ, k).map(|v| String::from_utf8_lossy(v).into_owned()))
                    .collect(),
            })
        })
        .collect();
    out.sort_by_key(|m| m.pid);
    out
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

/// The parent of `pid` (`/proc/<pid>/stat`, the field after the command
/// name in parentheses, which may itself hold `)` and spaces).
pub fn parent(proc: &Path, pid: u32) -> Option<u32> {
    let stat = std::fs::read(proc.join(pid.to_string()).join("stat")).ok()?;
    let close = stat.iter().rposition(|b| *b == b')')?;
    let rest = std::str::from_utf8(&stat[close + 1..]).ok()?;
    let mut fields = rest.split_whitespace();
    fields.next()?; // state
    fields.next()?.parse().ok().filter(|&p| p > 1)
}

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

/// The windows Hyprland has (`hyprctl clients -j`). `Err` says why not.
pub fn windows() -> Result<Vec<Window>, String> {
    match sys::run("hyprctl", &["clients", "-j"], None, HYPRCTL_TIMEOUT) {
        Run::Exited {
            code: Some(0),
            stdout,
            ..
        } => serde_json::from_str(&stdout)
            .map_err(|e| format!("hyprctl clients -j gave no window list ({e})")),
        Run::NotFound => Err("hyprctl not found: focusing a window needs Hyprland".into()),
        other => Err(format!("hyprctl clients -j failed: {}", failure(&other))),
    }
}

/// The window of the first of `pids` that has one, else of the nearest
/// ancestor of one that has one (a terminal that runs the marked process
/// as its child). Several windows of one pid (a terminal server): the
/// one of class `prefer`, else the first.
pub fn window_of<'a>(
    proc: &Path,
    windows: &'a [Window],
    pids: &[u32],
    prefer: &str,
) -> Option<&'a Window> {
    let of = |pid: u32| {
        let mine: Vec<&Window> = windows.iter().filter(|w| w.pid == i64::from(pid)).collect();
        mine.iter()
            .find(|w| w.class == prefer)
            .or_else(|| mine.first())
            .copied()
    };
    if let Some(w) = pids.iter().find_map(|&p| of(p)) {
        return Some(w);
    }
    pids.iter().find_map(|&p| {
        let mut at = p;
        // a loop in a hand-made /proc must not hang the engine
        for _ in 0..64 {
            at = parent(proc, at)?;
            if let Some(w) = of(at) {
                return Some(w);
            }
        }
        None
    })
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
        match sys::run("hyprctl", args, None, HYPRCTL_TIMEOUT) {
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

    /// A hand-made `/proc`: `(pid, ppid, environ entries)`.
    fn proc_tree(tag: &str, procs: &[(u32, u32, &[&str])]) -> Proc {
        let dir = std::env::temp_dir().join(format!(
            "seldon-sessions-{tag}-{}-{}",
            std::process::id(),
            sys::random_hex(4)
        ));
        for (pid, ppid, env) in procs {
            let p = dir.join(pid.to_string());
            std::fs::create_dir_all(&p).unwrap();
            let mut block = Vec::new();
            for e in *env {
                block.extend_from_slice(e.as_bytes());
                block.push(0);
            }
            std::fs::write(p.join("environ"), block).unwrap();
            std::fs::write(p.join("stat"), format!("{pid} (a ) b) S {ppid} 1 1 0 -1")).unwrap();
        }
        std::fs::create_dir_all(dir.join("self")).unwrap();
        Proc(dir)
    }

    fn os(s: &str) -> &OsStr {
        OsStr::new(s)
    }

    #[test]
    fn a_session_needs_every_marker_key() {
        let p = proc_tree(
            "keys",
            &[
                (
                    10,
                    1,
                    &[
                        "SELDON_CASE=C-2026-005",
                        "SELDON_LOGBOOK=/l",
                        "SELDON_ACTOR=agent:default",
                    ],
                ),
                (11, 10, &["SELDON_CASE=C-2026-005", "SELDON_LOGBOOK=/other"]),
                (12, 10, &["SELDON_CASE=C-2026-0050", "SELDON_LOGBOOK=/l"]),
                (13, 10, &["XSELDON_CASE=C-2026-005", "SELDON_LOGBOOK=/l"]),
                (14, 10, &["SELDON_LOGBOOK=/l", "SELDON_CASE=C-2026-005"]),
                (15, 10, &[]),
            ],
        );
        let want = [
            ("SELDON_CASE", os("C-2026-005")),
            ("SELDON_LOGBOOK", os("/l")),
        ];
        let got = marked(&p.0, &want, &["SELDON_ACTOR"]);
        assert_eq!(
            got,
            [
                Marked {
                    pid: 10,
                    values: vec![Some("agent:default".into())]
                },
                Marked {
                    pid: 14,
                    values: vec![None]
                },
            ]
        );
        assert!(marked(&p.0.join("missing"), &want, &[]).is_empty());
    }

    #[test]
    fn the_first_entry_of_a_key_wins() {
        assert_eq!(value(b"A=1\0A=2\0", "A"), Some(&b"1"[..]));
        assert_eq!(value(b"AB=1\0A=\0", "A"), Some(&b""[..]));
        assert_eq!(value(b"A\0", "A"), None);
    }

    #[test]
    fn the_parent_reads_past_a_name_with_parentheses() {
        let p = proc_tree("stat", &[(20, 7, &[]), (21, 1, &[])]);
        assert_eq!(parent(&p.0, 20), Some(7));
        assert_eq!(parent(&p.0, 21), None, "pid 1 is no parent to follow");
        assert_eq!(parent(&p.0, 99), None);
    }

    fn window(address: &str, pid: i64, class: &str) -> Window {
        Window {
            address: address.into(),
            pid,
            class: class.into(),
            workspace: Workspace { name: "2".into() },
        }
    }

    #[test]
    fn the_window_is_the_marked_pid_or_its_nearest_ancestor() {
        let p = proc_tree("win", &[(30, 20, &[]), (20, 10, &[]), (10, 5, &[])]);
        let ws = [
            window("0x1", 10, "org.omarchy.terminal"),
            window("0x2", 20, "foot"),
            window("0x3", 20, "org.omarchy.agent"),
            window("0x4", 99, "x"),
        ];
        let addr = |pids: &[u32]| {
            window_of(&p.0, &ws, pids, "org.omarchy.agent").map(|w| w.address.clone())
        };
        assert_eq!(addr(&[99]).as_deref(), Some("0x4"));
        assert_eq!(
            addr(&[30]).as_deref(),
            Some("0x3"),
            "the parent, of the preferred class"
        );
        assert_eq!(addr(&[10]).as_deref(), Some("0x1"));
        assert_eq!(
            addr(&[30, 99]).as_deref(),
            Some("0x4"),
            "a marked pid before an ancestor"
        );
        assert_eq!(addr(&[77]), None);
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
