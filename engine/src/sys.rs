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

/// Who runs this process, as far as the upgrades a capture makes in the
/// user's files need to know (the rules block, the agent skill; WP-111).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runner {
    /// A user other than root.
    User,
    /// Root: its home is not the user's, and a package hook runs as root.
    Root,
    /// It cannot be told (the probe cannot be read); why.
    Unknown(String),
}

/// [`Runner`] from the effective user id, the owner of `/proc/self`.
/// Callers treat anything but [`Runner::User`] as "do not touch the
/// user's files" (fail closed, WP-111 round 2). Debug builds only:
/// `SELDON_TEST_ROOT_PROBE` names another path to read the owner of.
pub fn runner() -> Runner {
    use std::os::unix::fs::MetadataExt as _;
    #[cfg(debug_assertions)]
    let probe = std::env::var_os("SELDON_TEST_ROOT_PROBE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "/proc/self".into());
    #[cfg(not(debug_assertions))]
    let probe = std::path::PathBuf::from("/proc/self");
    match std::fs::metadata(&probe) {
        Ok(m) if m.uid() == 0 => Runner::Root,
        Ok(_) => Runner::User,
        Err(e) => Runner::Unknown(format!("{}: {e}", probe.display())),
    }
}

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

/// Most bytes a state file the index reads beside the logbook may hold
/// (`proposals/*.json`, `autocommit.json`; ADR-0035 §6, WP-120 round 3).
pub const STATE_FILE_MAX: u64 = 4 * 1024 * 1024;

/// The text of `path` when it is a regular file (not a symbolic link, a
/// FIFO, a device or a directory) of at most `max` bytes; else why not, in
/// words for a warning. `Ok(None)` when it does not exist. The type is
/// checked with `symlink_metadata` before the file is opened, so a FIFO is
/// never opened (it would block) and `/dev/zero` behind a link never read;
/// the read itself stops after `max + 1` bytes, in case the file grew.
pub fn read_small_file(path: &Path, max: u64) -> Result<Option<String>, String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let kind = meta.file_type();
    if !kind.is_file() {
        let what = if kind.is_symlink() {
            "a symbolic link"
        } else if kind.is_dir() {
            "a directory"
        } else {
            "not a regular file"
        };
        return Err(what.to_string());
    }
    if meta.len() > max {
        return Err(format!("{} bytes, more than {max}", meta.len()));
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max {
        return Err(format!("more than {max} bytes"));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "not UTF-8".to_string())
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

/// Runs `program args…` with stdin closed and a timeout, capturing output,
/// in its own process group ([`run_command`]).
pub fn run(program: &str, args: &[&str], cwd: Option<&Path>, timeout: Duration) -> Run {
    run_with(command(program, args, cwd), timeout, Group::Own)
}

/// [`run`] in the engine's own process group, for `git`: git and what it
/// runs (hooks, a signing prompt) may use the terminal, which a background
/// process group cannot read. At the deadline only `program` is killed;
/// the wait for the output pipes ends at the same deadline (plus
/// [`DRAIN_GRACE`]) all the same.
pub fn run_in_engine_group(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Run {
    run_with(command(program, args, cwd), timeout, Group::Engine)
}

/// Omarchy's install root when `OMARCHY_PATH` is unset or empty: the
/// package install (memory/host.md). A desktop session exports the
/// variable; ssh, cron and systemd units do not, and `omarchy-shell`
/// (behind `omarchy plugin list`) then fails with "OMARCHY_PATH is not
/// set".
pub const OMARCHY_PATH_DEFAULT: &str = "/usr/share/omarchy";

/// `$OMARCHY_PATH`, or [`OMARCHY_PATH_DEFAULT`] when it is unset or empty.
pub fn omarchy_path() -> PathBuf {
    omarchy_path_from(std::env::var_os("OMARCHY_PATH").as_deref())
}

/// [`omarchy_path`] for the engine's own `value`.
fn omarchy_path_from(value: Option<&std::ffi::OsStr>) -> PathBuf {
    match omarchy_path_to_set(value) {
        Some(default) => default.into(),
        None => value.unwrap_or_default().into(),
    }
}

/// The `OMARCHY_PATH` an Omarchy command must be given for the engine's
/// own `value`: [`OMARCHY_PATH_DEFAULT`] when it is unset or empty, else
/// none (a set value is trusted, as `PATH` is).
fn omarchy_path_to_set(value: Option<&std::ffi::OsStr>) -> Option<&'static str> {
    value
        .is_none_or(|v| v.is_empty())
        .then_some(OMARCHY_PATH_DEFAULT)
}

/// `program args…` for a program of Omarchy's (`omarchy`,
/// `omarchy-version`), for [`run_command`]: the child gets `OMARCHY_PATH`
/// through [`Command::env`] when the engine's own is unset or empty. The
/// engine's environment is never changed (`set_var` races with the
/// threads [`run_command`] reads the output pipes with).
pub fn omarchy_command(program: &str, args: &[&str]) -> Command {
    let mut cmd = command(program, args, None);
    if let Some(value) = omarchy_path_to_set(std::env::var_os("OMARCHY_PATH").as_deref()) {
        cmd.env("OMARCHY_PATH", value);
    }
    cmd
}

fn command(program: &str, args: &[&str], cwd: Option<&Path>) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd
}

/// The process group a program runs in.
#[derive(Clone, Copy)]
enum Group {
    /// Its own: killed as a whole at the deadline.
    Own,
    /// The engine's: only the program is killed.
    Engine,
}

/// How long the output pipes may stay open after the program (or its
/// process group) was killed at the deadline.
const DRAIN_GRACE: Duration = Duration::from_millis(200);

/// [`run`] for a command built by the caller (fixed program and argv, plus
/// environment, e.g. `snapper` with `LC_ALL=C`): stdin closed, output
/// captured, in its own process group. The timeout covers the program and
/// its output pipes: at the deadline the whole group is killed, also when
/// the program has exited and something it started still holds a pipe.
pub fn run_command(cmd: Command, timeout: Duration) -> Run {
    run_with(cmd, timeout, Group::Own)
}

/// [`run_command`] in the engine's own process group, for a `git` command
/// the caller built (its environment controlled, `logbook::git`): git and
/// what it runs (hooks, a signing prompt) may use the terminal, as with
/// [`run_in_engine_group`]. At the deadline only the program is killed.
pub fn run_command_in_engine_group(cmd: Command, timeout: Duration) -> Run {
    run_with(cmd, timeout, Group::Engine)
}

fn run_with(mut cmd: Command, timeout: Duration, group: Group) -> Run {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Group::Own = group {
        cmd.process_group(0);
    }
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
                stop(&mut child, group);
                let _ = child.wait();
                return Run::TimedOut;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                stop(&mut child, group);
                return Run::Failed(e.to_string());
            }
        }
    };
    if !(out.wait_until(deadline) && err.wait_until(deadline)) {
        // the program has exited; what it started keeps a pipe open
        stop(&mut child, group);
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

/// Kills `child` and, when it leads its own group, the whole group.
fn stop(child: &mut Child, group: Group) {
    match group {
        Group::Own => kill_group(child),
        Group::Engine => {
            let _ = child.kill();
        }
    }
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
/// crate is on the allowed list (AGENTS.md §7). The whole blocks are
/// hashed in place; only the last one or two, with the padding, are
/// copied. [`Sha256`] hashes a stream (a file of any size, WP-113).
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut s = Sha256::new();
    s.update(bytes);
    s.finish()
}

/// SHA-256 over bytes that arrive in pieces: [`Sha256::update`] as often
/// as needed, then [`Sha256::finish`].
#[derive(Debug, Clone)]
pub struct Sha256 {
    h: [u32; 8],
    /// A block begun by an earlier update.
    buf: [u8; 64],
    buffered: usize,
    len: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Sha256::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Sha256 {
            h: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0; 64],
            buffered: 0,
            len: 0,
        }
    }

    pub fn update(&mut self, mut bytes: &[u8]) {
        self.len = self.len.wrapping_add(bytes.len() as u64);
        if self.buffered > 0 {
            let take = (64 - self.buffered).min(bytes.len());
            self.buf[self.buffered..self.buffered + take].copy_from_slice(&bytes[..take]);
            self.buffered += take;
            bytes = &bytes[take..];
            if self.buffered < 64 {
                return;
            }
            let block = self.buf;
            sha256_block(&mut self.h, &block);
            self.buffered = 0;
        }
        let (blocks, rest) = bytes.as_chunks::<64>();
        for block in blocks {
            sha256_block(&mut self.h, block);
        }
        self.buf[..rest.len()].copy_from_slice(rest);
        self.buffered = rest.len();
    }

    pub fn finish(mut self) -> [u8; 32] {
        // the rest + 0x80 + zero padding + 64-bit big-endian bit length: one
        // block, or two when the rest leaves less than 9 bytes
        let rest = self.buffered;
        let mut tail = [0u8; 128];
        tail[..rest].copy_from_slice(&self.buf[..rest]);
        tail[rest] = 0x80;
        let end = if rest < 56 { 64 } else { 128 };
        tail[end - 8..end].copy_from_slice(&self.len.wrapping_mul(8).to_be_bytes());
        let (blocks, _) = tail[..end].as_chunks::<64>();
        for block in blocks {
            sha256_block(&mut self.h, block);
        }

        let mut out = [0u8; 32];
        let (chunks, _) = out.as_chunks_mut::<4>();
        for (chunk, word) in chunks.iter_mut().zip(self.h) {
            *chunk = word.to_be_bytes();
        }
        out
    }

    /// [`Sha256::finish`] as 64 lowercase hex digits.
    pub fn finish_hex(self) -> String {
        hex(&self.finish())
    }
}

/// One SHA-256 compression step: `h` after the 64-byte `block`.
fn sha256_block(h: &mut [u32; 8], block: &[u8; 64]) {
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
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = *h;
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

/// [`sha256`] as 64 lowercase hex digits.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&sha256(bytes))
}

fn hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {

    /// WP-120 round 3: [`read_small_file`] reads a small regular file,
    /// refuses a link, a directory and a file over the limit, and says
    /// nothing for a missing one.
    #[test]
    fn small_regular_files_only() {
        let dir = std::env::temp_dir().join(format!("seldon-small-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.json");
        std::fs::write(&file, "{}").unwrap();
        assert_eq!(read_small_file(&file, 2), Ok(Some("{}".to_string())));
        // the size is checked before the file is opened (the read's own cap
        // would say "more than 1 bytes")
        assert_eq!(
            read_small_file(&file, 1),
            Err("2 bytes, more than 1".to_string())
        );
        let link = dir.join("b.json");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(
            read_small_file(&link, 10)
                .unwrap_err()
                .contains("symbolic link")
        );
        assert!(read_small_file(&dir, 10).unwrap_err().contains("directory"));
        assert_eq!(read_small_file(&dir.join("none.json"), 10), Ok(None));
        let _ = std::fs::remove_dir_all(&dir);
    }

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
    fn omarchy_path_is_set_only_when_unset_or_empty() {
        use std::ffi::OsStr;
        assert_eq!(omarchy_path_to_set(None), Some("/usr/share/omarchy"));
        assert_eq!(
            omarchy_path_to_set(Some(OsStr::new(""))),
            Some("/usr/share/omarchy")
        );
        assert_eq!(omarchy_path_to_set(Some(OsStr::new("/opt/omarchy"))), None);
        for unset in [None, Some(OsStr::new(""))] {
            assert_eq!(omarchy_path_from(unset), Path::new("/usr/share/omarchy"));
        }
        assert_eq!(
            omarchy_path_from(Some(OsStr::new("/opt/omarchy"))),
            Path::new("/opt/omarchy")
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
        // padding edges: the length fits into the last block (55), needs
        // a block of its own (56, 63, 119, 120), whole blocks (64, 128)
        for (n, hex) in [
            (
                55,
                "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
            ),
            (
                56,
                "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
            ),
            (
                63,
                "7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34",
            ),
            (
                65,
                "635361c48bb9eab14198e76ea8ab7f1a41685d6ad62aa9146d301d4f17eb0ae0",
            ),
            (
                119,
                "31eba51c313a5c08226adf18d4a359cfdfd8d2e816b13f4af952f7ea6584dcfb",
            ),
            (
                120,
                "2f3d335432c70b580af0e8e1b3674a7c020d683aa5f73aaaedfdc55af904c21c",
            ),
            (
                128,
                "6836cf13bac400e9105071cd6af47084dfacad4e5e302c94bfed24e013afb73e",
            ),
        ] {
            assert_eq!(sha256_hex(&vec![b'a'; n]), hex, "{n} bytes");
        }
        assert_eq!(
            sha256_hex(&[0u8; 64]),
            "f5a5fd42d16a20302798ef6ed309979b43003d2320d9f0e8ea9831a92759fb4b"
        );
    }

    /// WP-113: the stream gives the one-shot digest wherever the pieces
    /// are cut (inside a block, at its edge, empty pieces).
    #[test]
    fn sha256_stream_equals_one_shot() {
        let bytes: Vec<u8> = (0..1000u32).map(|i| (i * 7 % 256) as u8).collect();
        for cut in [0, 1, 55, 63, 64, 65, 127, 128, 500, 999, 1000] {
            for step in [1, 3, 64, 100, 1000] {
                let mut s = Sha256::new();
                s.update(&bytes[..cut]);
                s.update(&[]);
                for piece in bytes[cut..].chunks(step) {
                    s.update(piece);
                }
                assert_eq!(s.finish_hex(), sha256_hex(&bytes), "cut {cut}, step {step}");
            }
        }
    }
}
