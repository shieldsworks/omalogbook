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
    assert_eq!(output.stderr, b"omalogbook: HOME is unset\n");
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
