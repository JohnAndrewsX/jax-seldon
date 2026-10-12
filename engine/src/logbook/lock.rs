//! `~/.local/state/seldon/lock`: one writer at a time (SPEC-ENGINE §2).
//!
//! An advisory `flock` on the lock file, held until the guard is dropped.
//! A second writer fails at once with exit 4; nothing waits.
//!
//! The flock belongs to the open file description, so it lasts while any
//! descriptor of it is open. The file is opened with `O_CLOEXEC` (Rust's
//! default), yet a child forked by another thread holds a copy of every
//! descriptor from the fork until its `exec`; a drop in that window does
//! not release the lock until the child has exec'd (WP-158). The engine
//! starts its programs from one thread and `Command::spawn` returns only
//! after the child's `exec`, so its own children never outlive a drop
//! with the descriptor; the threads it starts (output pipe readers, the
//! `notify` watcher) never fork. Tests run in parallel threads that do.

use std::fs::{File, OpenOptions, TryLockError};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::error::{Error, Result};

/// Held lock; released on drop (and by the kernel if the process dies).
#[derive(Debug)]
pub struct Lock {
    _file: File,
    pub path: PathBuf,
}

/// Takes the lock at `path`, creating the file and its directory.
pub fn acquire(path: &Path) -> Result<Lock> {
    if let Some(dir) = path.parent() {
        crate::sys::create_dir_private(dir)
            .with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(crate::sys::NEW_FILE_MODE)
        .open(path)
        .with_context(|| format!("cannot open {}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(Lock {
            _file: file,
            path: path.to_path_buf(),
        }),
        Err(TryLockError::WouldBlock) => Err(Error::LockHeld(path.to_path_buf())),
        Err(TryLockError::Error(e)) => Err(anyhow::Error::new(e)
            .context(format!("cannot lock {}", path.display()))
            .into()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    /// [`acquire`], retried while the lock is held: the lib tests run in
    /// parallel threads, and a child another test forks keeps the dropped
    /// lock until its `exec` (WP-158, module docs).
    fn acquire_within(path: &Path, patience: Duration) -> Result<Lock> {
        let deadline = Instant::now() + patience;
        loop {
            match acquire(path) {
                Err(Error::LockHeld(_)) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                taken => return taken,
            }
        }
    }

    #[test]
    fn second_lock_is_refused_until_release() {
        let dir = std::env::temp_dir().join(format!("seldon-lock-test-{}", std::process::id()));
        let path = dir.join("lock");
        let first = acquire(&path).unwrap();
        assert!(matches!(acquire(&path), Err(Error::LockHeld(_))));
        drop(first);
        assert!(acquire_within(&path, Duration::from_secs(10)).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
