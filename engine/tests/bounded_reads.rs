//! Every file of the logbook the engine reads goes through
//! `sys::read_regular` (or `sys::open_regular`): a FIFO there is never
//! waited on and a link to `/dev/zero` never read (WP-174). Each reader
//! the WP-171 review found unbounded gets a FIFO and a `/dev/zero` link,
//! and the command ends within a time limit, saying why; `doctor` still
//! reaches its `layout` row and names the file. A link to a regular file
//! is read as before (ADR-0049 §2). A grep test keeps new readers on the
//! helper.

mod common;

use std::collections::BTreeMap;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use common::{Env, Snapper, hardware_root, json, stderr};

const NOW: &str = "2026-10-09T10:00:00+02:00";
const MONTH: &str = "ledger/2026-10.jsonl";
/// What a refusal tells the user to do (review F1).
const REMEDY: &str = "not read: make it a regular file and run the command again";

unsafe extern "C" {
    fn mkfifo(path: *const std::ffi::c_char, mode: u32) -> i32;
    fn setrlimit(resource: i32, limit: *const [u64; 2]) -> i32;
}

/// `RLIMIT_AS` on Linux.
const RLIMIT_AS: i32 = 9;

/// What takes the file's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Swap {
    /// A FIFO: opened to read, it blocks until a writer comes.
    Fifo,
    /// A symbolic link to `/dev/zero`: read, it never ends.
    Zero,
}

const BOTH: &[Swap] = &[Swap::Fifo, Swap::Zero];

impl Swap {
    /// What the reader says the file is.
    fn what(self) -> &'static str {
        match self {
            Swap::Fifo => "a FIFO, not a regular file; not read: make it a regular file",
            Swap::Zero => "a device, not a regular file; not read: make it a regular file",
        }
    }

    /// What doctor's `layout` row says of it.
    fn layout(self) -> &'static str {
        match self {
            Swap::Fifo => "no regular file",
            Swap::Zero => "symbolic link",
        }
    }
}

/// Puts `how` where the file `rel` of `root` is.
fn swap(root: &Path, rel: &str, how: Swap) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);
    match how {
        Swap::Fifo => {
            let c = CString::new(path.as_os_str().as_bytes()).unwrap();
            assert_eq!(unsafe { mkfifo(c.as_ptr(), 0o600) }, 0, "mkfifo {rel}");
        }
        Swap::Zero => std::os::unix::fs::symlink("/dev/zero", &path).unwrap(),
    }
}

/// The output of `args` in `env` on the logbook `root` at [`NOW`], with
/// the collectors pointed at nothing. A command that has not finished
/// after 30 s (a FIFO opened) is killed and the test fails; its address
/// space is capped at 2 GiB, so a read of `/dev/zero` fails fast instead
/// of filling the host's memory.
fn within(env: &Env, root: &Path, args: &[&str], what: &str) -> Output {
    let mut cmd: Command = env.command(args);
    cmd.env("SELDON_LOGBOOK", root)
        .env("SELDON_NOW", NOW)
        .env("SELDON_HARDWARE_ROOT", hardware_root())
        .env("SELDON_PACMAN_LOG", env.tmp.path().join("pacman.log"))
        .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
        .env("SELDON_OMARCHY_PLUGINS_DIR", env.tmp.path().join("plugins"))
        .env("SELDON_THEME_FILE", env.tmp.path().join("theme.name"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        cmd.pre_exec(|| {
            let limit = [2 << 30, 2 << 30];
            if setrlimit(RLIMIT_AS, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn().expect("run seldon");
    let deadline = Instant::now() + Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{what}: still running after 30 s (a FIFO opened?)");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

/// `doctor --json` on `root`: its rows by name, and the exit code.
fn doctor(env: &Env, root: &Path, what: &str) -> (BTreeMap<String, String>, Option<i32>) {
    let out = within(env, root, &["--json", "doctor"], what);
    let v = json(&out);
    let rows = v["checks"]
        .as_array()
        .unwrap_or_else(|| panic!("{what}: {v}"))
        .iter()
        .map(|c| {
            let name = c["name"].as_str().unwrap().to_string();
            let mut line = format!(
                "{} {}",
                c["status"].as_str().unwrap(),
                c["message"].as_str().unwrap()
            );
            if let Some(fix) = c["fix"].as_str() {
                line.push_str(" fix: ");
                line.push_str(fix);
            }
            (name, line)
        })
        .fold(BTreeMap::new(), |mut m, (k, v)| {
            m.entry(k)
                .and_modify(|old: &mut String| {
                    old.push_str(" | ");
                    old.push_str(&v);
                })
                .or_insert(v);
            m
        });
    (rows, out.status.code())
}

/// Doctor ends with exit `code`, its row `row` is `status` and names
/// `rel` and why, and its `layout` row names `rel` too.
fn doctor_names(env: &Env, root: &Path, rel: &str, how: Swap, row: &str, status: &str, code: i32) {
    let what = format!("doctor {rel} {how:?}");
    let (rows, exit) = doctor(env, root, &what);
    assert_eq!(exit, Some(code), "{what}: {rows:#?}");
    let line = rows.get(row).unwrap_or_else(|| panic!("{what}: {rows:#?}"));
    assert!(line.starts_with(&format!("{status} ")), "{what}: {line}");
    assert!(line.contains(rel), "{what}: {line}");
    assert!(line.contains(how.what()), "{what}: {line}");
    let layout = rows
        .get("layout")
        .unwrap_or_else(|| panic!("{what}: no layout row: {rows:#?}"));
    assert!(
        layout.contains(&format!("{rel} ({})", how.layout())),
        "{what}: {layout}"
    );
}

#[test]
fn a_ledger_month_that_is_no_regular_file_is_not_read() {
    for &how in BOTH {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        swap(&root, MONTH, how);

        let what = format!("status {how:?}");
        let out = within(&env, &root, &["status"], &what);
        let err = stderr(&out);
        // refused like WP-171's refusals: exit 1, with what to do
        assert_eq!(out.status.code(), Some(1), "{what}: {err}");
        assert!(err.contains(REMEDY), "{what}: {err}");
        assert!(err.contains(MONTH), "{what}: {err}");
        assert!(err.contains(how.what()), "{what}: {err}");

        doctor_names(&env, &root, MONTH, how, "ledger", "error", 1);
        let (rows, _) = doctor(&env, &root, &what);
        assert!(
            rows["ledger"]
                .contains(" fix: make a month file that is no regular file a regular file again"),
            "{what}: {rows:#?}"
        );
    }
}

/// Review F2 and Q4: a ledger month past the read cap (256 MiB) is
/// refused with exit 1 and what to do; doctor warns from 128 MiB on
/// (`degraded`, naming the size and the cap). Sparse files: nothing that
/// large is written to the disk.
#[test]
fn a_ledger_month_past_the_cap_is_refused_and_one_near_it_is_named() {
    const MIB: u64 = 1024 * 1024;
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let month = std::fs::File::create(root.join(MONTH)).unwrap();

    month.set_len(129 * MIB).unwrap();
    let (rows, code) = doctor(&env, &root, "doctor 129 MiB");
    assert_eq!(code, Some(0), "{rows:#?}");
    let line = &rows["ledger"];
    assert!(line.starts_with("degraded "), "{line}");
    assert!(
        line.contains(
            "ledger/2026-10.jsonl is 129 MiB: Seldon reads a ledger month of at most 256 MiB"
        ),
        "{line}"
    );
    assert!(line.contains("before it reaches the limit"), "{line}");

    month.set_len(256 * MIB + 1).unwrap();
    let out = within(&env, &root, &["status"], "status 256 MiB + 1");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains(MONTH), "{err}");
    assert!(
        err.contains("more than 256 MiB; not read: keep a copy of it, remove lines you can do without by hand"),
        "{err}"
    );
    let (rows, code) = doctor(&env, &root, "doctor 256 MiB + 1");
    assert_eq!(code, Some(1), "{rows:#?}");
    assert!(rows["ledger"].starts_with("error "), "{rows:#?}");
    assert!(rows["ledger"].contains(" fix: "), "{rows:#?}");
}

#[test]
fn an_agents_md_that_is_no_regular_file_is_not_read() {
    for &how in BOTH {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        swap(&root, "AGENTS.md", how);

        // degraded, as any rules file that cannot be read; the layout
        // row is the error
        doctor_names(&env, &root, "AGENTS.md", how, "rules", "degraded", 1);

        // the capture's silent rules upgrade reads it first: it is not
        // read, so not upgraded (doctor's rows say why), and the capture
        // goes on
        let what = format!("capture {how:?}");
        let out = within(&env, &root, &["capture", "--all"], &what);
        assert_eq!(out.status.code(), Some(0), "{what}: {}", stderr(&out));
        let kind = root
            .join("AGENTS.md")
            .symlink_metadata()
            .unwrap()
            .file_type();
        assert!(!kind.is_file(), "{what}: AGENTS.md was replaced");
    }
}

#[test]
fn a_status_or_decisions_md_that_is_no_regular_file_is_not_read() {
    for rel in ["STATUS.md", "DECISIONS.md"] {
        for &how in BOTH {
            let env = Env::new(Snapper::Missing);
            let root = env.init_logbook();
            swap(&root, rel, how);
            // a view: `status` skips it, so both rows are degraded
            doctor_names(&env, &root, rel, how, "fences", "degraded", 0);
        }
    }
}

/// A link to a regular file is read through, as before (ADR-0049 §2):
/// `status` shows the ledger's events and doctor's rules row reads the
/// linked `AGENTS.md`; the `layout` row still names both.
#[test]
fn a_link_to_a_regular_file_is_read_as_before() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let out = within(&env, &root, &["log", "a note through the link"], "log");
    assert_eq!(out.status.code(), Some(0), "log: {}", stderr(&out));
    let outside = env.tmp.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    for rel in [MONTH, "AGENTS.md"] {
        let target = outside.join(rel.replace('/', "-"));
        std::fs::rename(root.join(rel), &target).unwrap();
        std::os::unix::fs::symlink(&target, root.join(rel)).unwrap();
    }

    let out = within(&env, &root, &["--json", "status"], "status");
    assert_eq!(out.status.code(), Some(0), "status: {}", stderr(&out));
    assert_eq!(json(&out)["events"], 1);

    let (rows, _) = doctor(&env, &root, "doctor");
    assert!(rows["rules"].starts_with("ok "), "{rows:?}");
    assert_eq!(
        rows["ledger"], "ok 1 month, every line an event",
        "{rows:?}"
    );
    for rel in [MONTH, "AGENTS.md"] {
        assert!(
            rows["layout"].contains(&format!("{rel} (symbolic link)")),
            "{rows:?}"
        );
    }
}

/// The readers of `std` (`fs::read`, `fs::read_to_string`, `File::open`,
/// `File::options`, `OpenOptions` with `.read(true)`, an import of
/// `fs::read`/`fs::read_to_string` for a bare call) are called only for
/// files outside the logbook: every call site in `engine/src` outside the
/// `#[cfg(test)]` modules, per file, is in this list. A new
/// one fails here: a file of the logbook is read with `sys::read_regular`,
/// `sys::read_regular_string` or `sys::open_regular` (WP-174); a file
/// outside the logbook is added below with what it reads.
#[test]
fn only_files_outside_the_logbook_are_read_without_the_bound() {
    const OUTSIDE: &[(&str, usize, &str)] = &[
        (
            "collectors/config.rs",
            6,
            "the watched config files and the collector's state",
        ),
        ("collectors/mod.rs", 1, "cursors.json (state)"),
        ("collectors/pacman.rs", 2, "/proc/stat and pacman.log"),
        ("collectors/plugins.rs", 2, "the Omarchy plugin folders"),
        ("collectors/snapper.rs", 2, "the snapshots' info.xml"),
        ("collectors/theme.rs", 1, "Omarchy's theme name file"),
        (
            "commands/agent.rs",
            2,
            "Omarchy's default agent file, the launch log (state)",
        ),
        ("commands/capture.rs", 2, "config.toml, owned.json (state)"),
        ("commands/doctor.rs", 2, "config.toml, the collector state"),
        ("commands/import/task.rs", 1, "the task file the user names"),
        ("commands/init.rs", 1, "config.toml"),
        ("commands/plan/snapshot.rs", 1, "the snapshots' info.xml"),
        ("commands/preview.rs", 1, "the tail of pacman.log"),
        ("commands/setup.rs", 1, "the setup kit's source files"),
        ("commands/skills.rs", 3, "the agent skill folders"),
        ("config.rs", 3, "user-dirs.dirs, config.toml"),
        (
            "dossier/query.rs",
            3,
            "Omarchy's package lists and theme, the hardware files",
        ),
        (
            "import/omarchy_agent.rs",
            1,
            "the omarchy-agent files to import",
        ),
        ("sessions.rs", 3, "/proc"),
        (
            "sys.rs",
            4,
            "the checked opens (open_checked, the ledger's append), a folder to sync, /etc/hostname",
        ),
    ];
    // the free functions, the opens (`.read(true)` on `OpenOptions`), and
    // an import of the free functions that a bare call would use
    let pattern = regex::Regex::new(
        r"\b(fs::read|fs::read_to_string|File::open|File::options)\(|\.read\(true\)|use std::fs::[^;]*\bread(_to_string)?\b[,;}]",
    )
    .unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found: BTreeMap<String, usize> = BTreeMap::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                // a `#[cfg(test)]` module reads its own files: from the
                // attribute to its closing brace at column 0 (rustfmt);
                // code after it counts again
                let mut calls = 0;
                let mut in_test = false;
                let mut lines = text.lines().peekable();
                while let Some(line) = lines.next() {
                    if in_test {
                        in_test = line != "}";
                        continue;
                    }
                    if line.trim() == "#[cfg(test)]"
                        && lines.peek().is_some_and(|next| next.contains("mod "))
                    {
                        in_test = !lines.peek().is_some_and(|next| next.ends_with(';'));
                        continue;
                    }
                    if !line.trim_start().starts_with("//") {
                        calls += pattern.find_iter(line).count();
                    }
                }
                if calls > 0 {
                    let rel = path
                        .strip_prefix(&src)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    found.insert(rel, calls);
                }
            }
        }
    }
    let listed: BTreeMap<String, usize> = OUTSIDE
        .iter()
        .map(|(file, n, _)| (file.to_string(), *n))
        .collect();
    assert_eq!(
        found, listed,
        "a std reader (fs::read, fs::read_to_string, File::open, File::options, .read(true), an import of fs::read) was added or removed: a logbook file is read with sys::read_regular"
    );
}
