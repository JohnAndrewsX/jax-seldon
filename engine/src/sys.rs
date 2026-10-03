//! Small OS helpers: atomic writes, subprocesses with a timeout, host name.
//!
//! Subprocesses are always fixed programs with fixed argument lists
//! (AGENTS.md §8); nothing here goes through a shell.

use std::fs::{DirBuilder, File, OpenOptions, Permissions};
use std::io::{Read, Write as _};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Context as _;

/// Mode of a file the engine creates (SPEC-ENGINE §2).
pub const NEW_FILE_MODE: u32 = 0o600;
/// Mode of a directory the engine creates (SPEC-ENGINE §2).
pub const NEW_DIR_MODE: u32 = 0o700;
/// Symbolic links followed at most, as the kernel's `MAXSYMLINKS`.
const MAX_LINKS: usize = 40;

/// Creates `dir` and its missing parents with [`NEW_DIR_MODE`]; existing
/// directories keep their mode.
pub fn create_dir_private(dir: &Path) -> std::io::Result<()> {
    DirBuilder::new()
        .recursive(true)
        .mode(NEW_DIR_MODE)
        .create(dir)
}

/// Creates the new file `path` with exactly [`NEW_FILE_MODE`], whatever the
/// umask; an existing file is an `AlreadyExists` error.
pub fn create_new_private(path: &Path) -> std::io::Result<File> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(NEW_FILE_MODE)
        .open(path)?;
    file.set_permissions(Permissions::from_mode(NEW_FILE_MODE))?;
    Ok(file)
}

/// Writes `bytes` to a temp file next to `path` and renames it over `path`.
/// A symbolic link at `path` is followed: its target is replaced and the
/// link stays. The target keeps its permission bits; a new file gets
/// [`NEW_FILE_MODE`] and new directories [`NEW_DIR_MODE`]. File and
/// directory are synced; the temp file is removed when anything fails.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    write_atomic_with(path, bytes, None, true)
}

/// [`write_atomic`] with the file's permission bits set to exactly `mode`.
pub fn write_atomic_mode(path: &Path, bytes: &[u8], mode: u32) -> anyhow::Result<()> {
    write_atomic_with(path, bytes, Some(mode), true)
}

/// [`write_atomic`] without the syncs, for files the engine rebuilds from
/// the ledger and the logbook (`index.json`, `STATUS.md`, the ledger
/// views, `outputs/REBUILD.md`): after a crash the next build writes them
/// again.
pub fn write_generated(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    write_atomic_with(path, bytes, None, false)
}

fn write_atomic_with(
    path: &Path,
    bytes: &[u8],
    mode: Option<u32>,
    sync: bool,
) -> anyhow::Result<()> {
    let target =
        resolve_links(path).with_context(|| format!("cannot resolve {}", path.display()))?;
    let dir = target
        .parent()
        .with_context(|| format!("{} has no parent directory", target.display()))?;
    create_dir_private(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let mode = mode.unwrap_or_else(|| {
        std::fs::metadata(&target).map_or(NEW_FILE_MODE, |m| m.permissions().mode() & 0o777)
    });
    let name = target
        .file_name()
        .map_or_else(Default::default, |n| n.to_string_lossy());
    let tmp = dir.join(format!(".{name}.tmp-{}", std::process::id()));
    let mut file = match create_temp(&tmp, mode) {
        Ok(f) => f,
        Err(e) => return Err(e).with_context(|| format!("cannot write {}", tmp.display())),
    };
    let written = file
        .write_all(bytes)
        .and_then(|()| if sync { file.sync_all() } else { Ok(()) })
        .with_context(|| format!("cannot write {}", tmp.display()))
        .and_then(|()| {
            std::fs::rename(&tmp, &target)
                .with_context(|| format!("cannot replace {}", target.display()))
        });
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return written;
    }
    // the rename is durable once the directory is synced; a file system
    // that cannot sync a directory still has the new file
    if sync && let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// The temp file at `tmp`, new, with exactly `mode`. A leftover of an
/// earlier run with the same process id is removed first.
fn create_temp(tmp: &Path, mode: u32) -> std::io::Result<File> {
    let open = || {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(tmp)
    };
    let file = match open() {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(tmp)?;
            open()?
        }
        other => other?,
    };
    // the umask narrows the mode given to open(2); set it exactly
    if let Err(e) = file.set_permissions(Permissions::from_mode(mode)) {
        let _ = std::fs::remove_file(tmp);
        return Err(e);
    }
    Ok(file)
}

/// `path` with every symbolic link in its last component followed (a
/// relative link is relative to the link's directory). The result may not
/// exist yet: a link to a missing file resolves to that file.
fn resolve_links(path: &Path) -> std::io::Result<PathBuf> {
    let mut p = path.to_path_buf();
    for _ in 0..MAX_LINKS {
        match std::fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {
                let to = std::fs::read_link(&p)?;
                p = match p.parent() {
                    Some(dir) => dir.join(to),
                    None => to,
                };
            }
            Ok(_) => return Ok(p),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(p),
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("too many levels of symbolic links"))
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

/// `ETXTBSY` on Linux ("Text file busy").
const ETXTBSY: i32 = 26;

/// Runs `program args…` with stdin closed and a timeout, capturing output.
pub fn run(program: &str, args: &[&str], cwd: Option<&Path>, timeout: Duration) -> Run {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    run_command(cmd, timeout)
}

/// How long the output pipes may stay open after the process group was
/// killed at the deadline.
const DRAIN_GRACE: Duration = Duration::from_millis(200);

/// [`run`] for a command built by the caller (fixed program and argv, plus
/// environment, e.g. `snapper` with `LC_ALL=C`): stdin closed, output
/// captured, in its own process group. The timeout covers the program and
/// its output pipes: at the deadline the whole group is killed, also when
/// the program has exited and something it started still holds a pipe.
pub fn run_command(mut cmd: Command, timeout: Duration) -> Run {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    // ETXTBSY: the program was just written and another thread's forked
    // child still holds the write descriptor until it execs. Brief; retry.
    let mut spawned = cmd.spawn();
    for _ in 0..20 {
        match &spawned {
            Err(e) if e.raw_os_error() == Some(ETXTBSY) => {
                thread::sleep(Duration::from_millis(5));
                spawned = cmd.spawn();
            }
            _ => break,
        }
    }
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Run::NotFound,
        Err(e) => return Run::Failed(e.to_string()),
    };
    let mut out = Drain::start(child.stdout.take().map(|p| Box::new(p) as _));
    let mut err = Drain::start(child.stderr.take().map(|p| Box::new(p) as _));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                kill_group(&mut child);
                let _ = child.wait();
                return Run::TimedOut;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                kill_group(&mut child);
                return Run::Failed(e.to_string());
            }
        }
    };
    if !(out.wait_until(deadline) && err.wait_until(deadline)) {
        // the program has exited; what it started keeps a pipe open
        kill_group(&mut child);
        let grace = Instant::now() + DRAIN_GRACE;
        if !(out.wait_until(grace) && err.wait_until(grace)) {
            return Run::TimedOut;
        }
    }
    Run::Exited {
        code: status.code(),
        stdout: out.text(),
        stderr: err.text(),
    }
}

/// One output pipe read to its end by a thread.
struct Drain {
    rx: Receiver<Vec<u8>>,
    bytes: Option<Vec<u8>>,
}

impl Drain {
    fn start(pipe: Option<Box<dyn Read + Send>>) -> Drain {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            let _ = tx.send(buf);
        });
        Drain { rx, bytes: None }
    }

    /// Whether the pipe reached its end by `deadline`. A thread still
    /// reading after that is left behind; it ends with the pipe.
    fn wait_until(&mut self, deadline: Instant) -> bool {
        if self.bytes.is_none() {
            let left = deadline.saturating_duration_since(Instant::now());
            self.bytes = self.rx.recv_timeout(left).ok();
        }
        self.bytes.is_some()
    }

    fn text(&mut self) -> String {
        String::from_utf8_lossy(&self.bytes.take().unwrap_or_default()).into_owned()
    }
}

/// `SIGKILL` on Linux.
const SIGKILL: i32 = 9;

unsafe extern "C" {
    /// kill(2); a negative `pid` signals the process group `-pid`.
    fn kill(pid: i32, sig: i32) -> i32;
}

/// Kills the process group `child` leads (it was spawned with
/// `process_group(0)`, so the group id is its pid), then `child` itself in
/// case the group is already gone.
fn kill_group(child: &mut Child) {
    if let Ok(pid) = i32::try_from(child.id()) {
        // SAFETY: kill(2) takes plain integers and touches no memory; the
        // group id is the child's own pid, which stays reserved while the
        // group has members or the child is not yet waited for.
        unsafe { kill(-pid, SIGKILL) };
    }
    let _ = child.kill();
}

/// Runs `program args…` on the user's terminal (stdin, stdout and stderr
/// inherited) and waits for it, without a timeout: for `$EDITOR`. The
/// result has empty `stdout`/`stderr`.
pub fn run_attached(program: &str, args: &[&str]) -> Run {
    match Command::new(program).args(args).status() {
        Ok(status) => Run::Exited {
            code: status.code(),
            stdout: String::new(),
            stderr: String::new(),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Run::NotFound,
        Err(e) => Run::Failed(e.to_string()),
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

/// SHA-256 (FIPS 180-4) of `bytes`. Implemented here because no hashing
/// crate is on the allowed list (AGENTS.md §7); the config collector hashes
/// files of at most 1 MB, so a plain one-shot implementation is enough.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    // message + 0x80 + zero padding + 64-bit big-endian bit length, a
    // multiple of 64 bytes
    let mut msg = bytes.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((bytes.len() as u64).wrapping_mul(8)).to_be_bytes());

    let (blocks, _) = msg.as_chunks::<64>();
    for block in blocks {
        let mut w = [0u32; 64];
        let (words, _) = block.as_chunks::<4>();
        for (i, word) in words.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }

    let mut out = [0u8; 32];
    let (chunks, _) = out.as_chunks_mut::<4>();
    for (chunk, word) in chunks.iter_mut().zip(h) {
        *chunk = word.to_be_bytes();
    }
    out
}

/// [`sha256`] as 64 lowercase hex digits.
pub fn sha256_hex(bytes: &[u8]) -> String {
    sha256(bytes).iter().map(|b| format!("{b:02x}")).collect()
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

    #[test]
    fn sha256_matches_the_fips_vectors() {
        // FIPS 180-4 examples and the NIST one-million-`a` vector
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            sha256_hex(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        // padding edges: 55, 56 and 64 bytes
        for n in [55, 56, 63, 64, 65] {
            assert_eq!(sha256_hex(&vec![0u8; n]).len(), 64);
        }
        assert_eq!(
            sha256_hex(&[0u8; 64]),
            "f5a5fd42d16a20302798ef6ed309979b43003d2320d9f0e8ea9831a92759fb4b"
        );
    }
}
