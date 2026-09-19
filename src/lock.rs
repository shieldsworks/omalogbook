//! One writer at a time. `omalogbook note` and the running log would otherwise
//! rewrite the same note from two processes, and the last one to finish would
//! quietly drop the other's line.

use std::{fs, io, os::fd::AsRawFd, path::Path};

/// Held for as long as the writing takes; the lock goes when this drops.
#[derive(Debug)]
pub struct Lock(fs::File);

impl Lock {
    /// Wait for the vault's lock. Blocks, because every writer here is quick.
    pub fn take(vault: &Path) -> io::Result<Lock> {
        fs::create_dir_all(vault)?;
        // The lock belongs to this machine, not to the log's history.
        let ignore = vault.join(".gitignore");
        if !ignore.exists() {
            let _ = fs::write(&ignore, ".omalogbook.lock\n");
        }
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(vault.join(".omalogbook.lock"))?;
        // SAFETY: flock only takes the advisory lock on this descriptor.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Lock(file))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // SAFETY: the descriptor is still open; closing would unlock anyway.
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_writer_waits_its_turn() {
        let dir = std::env::temp_dir().join(format!("omalogbook-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let first = Lock::take(&dir).unwrap();
        let taken = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (d, t) = (dir.clone(), taken.clone());
        let waiter = std::thread::spawn(move || {
            let _second = Lock::take(&d).unwrap();
            t.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            !taken.load(std::sync::atomic::Ordering::SeqCst),
            "took the lock twice"
        );
        drop(first);
        waiter.join().unwrap();
        assert!(taken.load(std::sync::atomic::Ordering::SeqCst));
        let _ = fs::remove_dir_all(&dir);
    }
}
