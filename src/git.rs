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

/// Stage and commit only `paths`, which must sit inside the vault's work
/// tree. Nothing else in the index is read, staged, or committed, so changes
/// another writer has staged stay staged for that writer's own commit.
/// Nothing to commit for those paths is success, not a failure.
pub fn commit_paths(vault: &Path, paths: &[&Path], message: &str) -> Result<bool, String> {
    if paths.is_empty() || !is_repo(vault) {
        return Ok(false);
    }
    let mut add = Command::new("git");
    add.arg("-C").arg(vault).args(["add", "--"]).args(paths);
    if !add.output().is_ok_and(|out| out.status.success()) {
        return Err("git add failed".to_string());
    }
    let mut diff = Command::new("git");
    diff.arg("-C")
        .arg(vault)
        .args(["diff", "--cached", "--quiet", "--"])
        .args(paths);
    if diff.output().is_ok_and(|out| out.status.success()) {
        return Ok(false); // those paths already match the last commit
    }
    match Command::new("git")
        .arg("-C")
        .arg(vault)
        .args(["commit", "--only", "-m", message, "--"])
        .args(paths)
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

    #[test]
    fn commit_paths_leaves_everything_else_alone() {
        let dir = vault("paths");
        if run(&dir, &["init", "-q"]).is_none() {
            return; // no git here; the daemon copes the same way
        }
        run(&dir, &["config", "user.email", "log@example.com"]).unwrap();
        run(&dir, &["config", "user.name", "Omalogbook"]).unwrap();
        let note = dir.join("2026").join("09").join("2026-09-18.md");
        fs::create_dir_all(note.parent().unwrap()).unwrap();
        fs::write(&note, "under way\n").unwrap();
        fs::write(dir.join("staged.md"), "someone else's\n").unwrap();
        fs::write(dir.join("loose.md"), "not ours\n").unwrap();
        run(&dir, &["add", "staged.md"]).unwrap();

        assert_eq!(commit_paths(&dir, &[&note], "a note"), Ok(true));
        let committed = run(&dir, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
        assert_eq!(committed.trim(), "2026/09/2026-09-18.md");
        let status = run(&dir, &["status", "--porcelain"]).unwrap();
        assert!(status.contains("A  staged.md"), "{status}");
        assert!(status.contains("?? loose.md"), "{status}");

        assert_eq!(commit_paths(&dir, &[&note], "again"), Ok(false));
        assert_eq!(commit_paths(&dir, &[], "none"), Ok(false));
        let _ = fs::remove_dir_all(&dir);
    }
}
