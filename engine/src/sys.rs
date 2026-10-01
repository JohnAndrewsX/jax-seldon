//! Small OS helpers: atomic writes, subprocesses with a timeout, host name.
//!
//! Subprocesses are always fixed programs with fixed argument lists
//! (AGENTS.md §8); nothing here goes through a shell.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Context as _;

/// Writes `bytes` to a temp file next to `path` and renames it over `path`.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let dir = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let name = path
        .file_name()
        .map_or_else(Default::default, |n| n.to_string_lossy());
    let tmp = dir.join(format!(".{name}.tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes).with_context(|| format!("cannot write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("cannot replace {}", path.display()))
}

/// The result of running a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    /// The program ran to completion.
    Exited {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    /// The program is not on `PATH`.
    NotFound,
    /// The program did not finish in time and was killed.
    TimedOut,
    /// The program could not be started for another reason.
    Failed(String),
}

impl Run {
    pub fn success(&self) -> bool {
        matches!(self, Run::Exited { code: Some(0), .. })
    }
}

/// Runs `program args…` with stdin closed and a timeout, capturing output.
pub fn run(program: &str, args: &[&str], cwd: Option<&Path>, timeout: Duration) -> Run {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Run::NotFound,
        Err(e) => return Run::Failed(e.to_string()),
    };
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            String::from_utf8_lossy(&buf).into_owned()
        })
    };
    let out = drain(child.stdout.take().map(|p| Box::new(p) as _));
    let err = drain(child.stderr.take().map(|p| Box::new(p) as _));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Run::TimedOut;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(e) => return Run::Failed(e.to_string()),
        }
    };
    Run::Exited {
        code: status.code(),
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    }
}

/// The machine's host name: `/etc/hostname`, else `$HOSTNAME`, else `machine`.
/// Read-only; used for the logbook's `machineId`.
pub fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "machine".to_string())
}

/// `n` random lowercase hex digits, from the 80 random bits of fresh ULIDs
/// (the `ulid` crate's OS-seeded generator; not for cryptography, only for
/// ids that must not collide by accident).
pub fn random_hex(n: usize) -> String {
    let mut out = String::new();
    while out.len() < n {
        out.push_str(&format!("{:020x}", ulid::Ulid::generate().random()));
    }
    out.truncate(n);
    out
}

/// Lowercase slug: ASCII letters and digits, runs of anything else become one
/// hyphen, no leading or trailing hyphen.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_captures_and_reports() {
        let r = run(
            "sh",
            &["-c", "echo out; echo err >&2; exit 3"],
            None,
            Duration::from_secs(5),
        );
        assert_eq!(
            r,
            Run::Exited {
                code: Some(3),
                stdout: "out\n".into(),
                stderr: "err\n".into()
            }
        );
        assert_eq!(
            run("seldon-no-such-program", &[], None, Duration::from_secs(1)),
            Run::NotFound
        );
        assert_eq!(
            run("sleep", &["5"], None, Duration::from_millis(100)),
            Run::TimedOut
        );
    }

    #[test]
    fn slugs_and_hex() {
        assert_eq!(slugify("My Workstation.local"), "my-workstation-local");
        assert_eq!(slugify("--x--"), "x");
        let a = random_hex(4);
        assert_eq!(a.len(), 4);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
