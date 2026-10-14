use std::process::{Command, Output};
use std::sync::Mutex;

static DRIVE: Mutex<()> = Mutex::new(());

fn guard(case: &str) -> std::io::Result<Output> {
    let _hold = DRIVE.lock().unwrap_or_else(|err| err.into_inner());
    Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/drive_guards.py"
        ))
        .arg(case)
        .output()
}

fn assert_ok(case: &str, output: Output) {
    assert!(
        output.status.success(),
        "{case} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn sweep_kills_a_process_whose_runtime_dir_is_the_run() {
    assert_ok("sweep", guard("sweep").unwrap());
}

#[test]
fn readonly_bash_env_exits_instead_of_reexec() {
    assert_ok("bash-env", guard("bash-env").unwrap());
}

#[test]
fn outside_write_copies_the_diff_before_deletion() {
    assert_ok("outside", guard("outside").unwrap());
}

#[test]
fn outside_write_without_a_copy_prints_evidence_none() {
    assert_ok("outside-unreadable", guard("outside-unreadable").unwrap());
}

#[test]
fn git_index_mtime_is_not_an_outside_write() {
    assert_ok("index", guard("index").unwrap());
}
