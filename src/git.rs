//! The vault as a git repository. Every commit is a plain `git` call, so the
//! log stays a repo you can pick up with any tool, and omalogbook needs no
//! library to read or write it.

use std::{path::Path, process::Command};

/// Is this a git repository at all? A vault without one is fine; the log is
/// still plain files.
pub fn is_repo(vault: &Path) -> bool {
    run(vault, &["rev-parse", "--git-dir"]).is_some()
}

/// Stage everything and commit. Nothing to commit is success, not a failure.
pub fn commit(vault: &Path, message: &str) -> Result<bool, String> {
    if !is_repo(vault) {
        return Ok(false);
    }
    run(vault, &["add", "-A"]).ok_or("git add failed")?;
    if run(vault, &["diff", "--cached", "--quiet"]).is_some() {
        return Ok(false); // the tree already matches the last commit
    }
    match Command::new("git")
        .arg("-C")
        .arg(vault)
        .args(["commit", "-m", message])
        .output()
    {
        Ok(out) if out.status.success() => Ok(true),
        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Push if there is a remote and it answers. Offshore there will be no
/// connection for weeks, which is not an error worth stopping the log for.
pub fn push(vault: &Path) -> Result<bool, String> {
    if !is_repo(vault) || run(vault, &["remote"]).is_none_or(|out| out.trim().is_empty()) {
        return Ok(false);
    }
    match Command::new("git")
        .arg("-C")
        .arg(vault)
        .arg("push")
        .output()
    {
        Ok(out) if out.status.success() => Ok(true),
        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// stdout when the command succeeded, None when it failed or git is missing.
fn run(vault: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(vault)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn vault(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("omalogbook-git-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_plain_folder_is_left_alone() {
        let dir = vault("plain");
        assert!(!is_repo(&dir));
        assert_eq!(commit(&dir, "nothing to do"), Ok(false));
        assert_eq!(push(&dir), Ok(false));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn commits_only_when_something_changed() {
        let dir = vault("repo");
        if run(&dir, &["init", "-q"]).is_none() {
            return; // no git here; the daemon copes the same way
        }
        run(&dir, &["config", "user.email", "log@example.com"]).unwrap();
        run(&dir, &["config", "user.name", "Omalogbook"]).unwrap();
        assert!(is_repo(&dir));
        assert_eq!(commit(&dir, "empty"), Ok(false));
        fs::write(dir.join("2026-09-18.md"), "under way\n").unwrap();
        assert_eq!(commit(&dir, "the day's log"), Ok(true));
        assert_eq!(commit(&dir, "again"), Ok(false));
        assert_eq!(push(&dir), Ok(false)); // no remote
        let _ = fs::remove_dir_all(&dir);
    }
}
