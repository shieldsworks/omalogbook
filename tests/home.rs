use std::process::Command;

fn command(args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omalogbook"));
    cmd.args(args);
    cmd.env_remove("HOME");
    cmd.env_remove("XDG_CONFIG_HOME");
    cmd.env_remove("XDG_RUNTIME_DIR");
    cmd
}

#[test]
fn vault_without_home_stops() {
    let output = command(&["vault"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"omalogbook: HOME must be set to an absolute path\n"
    );
}

#[test]
fn vault_reads_an_absolute_config_when_home_is_unset() {
    let root = std::env::temp_dir().join(format!(
        "omalogbook-home-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _temp = TempDir(root.clone());
    std::fs::create_dir_all(root.join("omalogbook")).unwrap();
    let vault = root.join("vault");
    std::fs::write(
        root.join("omalogbook").join("config.toml"),
        format!("[boat]\nvault = \"{}\"\n", vault.display()),
    )
    .unwrap();
    let output = command(&["vault"])
        .env("XDG_CONFIG_HOME", &root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, format!("{}\n", vault.display()).into_bytes());
    assert!(output.stderr.is_empty());
}

#[test]
fn an_empty_or_relative_home_stops() {
    for home in ["", "boat"] {
        let output = command(&["vault"]).env("HOME", home).output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{home:?}");
        assert!(output.stdout.is_empty(), "{home:?}");
        assert_eq!(
            output.stderr, b"omalogbook: HOME must be set to an absolute path\n",
            "{home:?}"
        );
    }
}

#[test]
fn presets_do_not_need_home() {
    let output = command(&["presets"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("/depart  left the berth; notes the sunset and starts the trip"));
}

struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn note_home(name: &str) -> std::io::Result<(TempDir, std::path::PathBuf, std::path::PathBuf)> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let root = std::env::temp_dir().join(format!(
        "omalogbook-note-{name}-{}-{nanos}",
        std::process::id()
    ));
    let temp = TempDir(root.clone());
    std::fs::create_dir_all(root.join("omalogbook"))?;
    let vault = root.join("vault");
    std::fs::create_dir_all(&vault)?;
    std::fs::write(
        root.join("omalogbook").join("config.toml"),
        format!("[boat]\nvault = \"{}\"\n", vault.display()),
    )?;
    Ok((temp, root, vault))
}

fn with_git_identity(cmd: &mut Command) -> &mut Command {
    cmd.env("GIT_AUTHOR_NAME", "Omalogbook")
        .env("GIT_AUTHOR_EMAIL", "log@example.com")
        .env("GIT_COMMITTER_NAME", "Omalogbook")
        .env("GIT_COMMITTER_EMAIL", "log@example.com")
}

fn git(vault: &std::path::Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(vault).args(args);
    let out = with_git_identity(&mut cmd).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

fn note(root: &std::path::Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut cmd = command(&[&["note"], args].concat());
    cmd.env("XDG_CONFIG_HOME", root);
    with_git_identity(&mut cmd).output()
}

#[test]
fn a_note_commits_its_day_file_and_nothing_else() {
    let (_temp, root, vault) = note_home("commit").unwrap();
    if git(&vault, &["init", "-q"]).is_none() {
        return; // no git here; the note still lands on disk
    }
    std::fs::write(vault.join("staged.md"), "the timer's\n").unwrap();
    std::fs::write(vault.join("loose.md"), "not the note's\n").unwrap();
    git(&vault, &["add", "staged.md"]).unwrap();

    let output = note(&root, &["anchored", "off", "the", "point"]).unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let path = String::from_utf8(output.stdout).unwrap();
    let path = std::path::Path::new(path.trim());
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("anchored off the point"), "{text}");

    let committed = git(&vault, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
    let relative = path.strip_prefix(&vault).unwrap();
    assert_eq!(committed.trim(), relative.to_string_lossy());
    let status = git(&vault, &["status", "--porcelain"]).unwrap();
    assert!(status.contains("A  staged.md"), "{status}");
    assert!(status.contains("?? loose.md"), "{status}");
}

#[test]
fn note_no_git_writes_without_committing() {
    let (_temp, root, vault) = note_home("nogit").unwrap();
    if git(&vault, &["init", "-q"]).is_none() {
        return; // no git here; the note still lands on disk
    }
    let output = note(&root, &["--no-git", "reefed", "--no-git"]).unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let path = String::from_utf8(output.stdout).unwrap();
    let text = std::fs::read_to_string(path.trim()).unwrap();
    assert!(text.contains("reefed --no-git"), "{text}");
    assert!(git(&vault, &["rev-parse", "--verify", "HEAD"]).is_none());
}
