---
name: verify-omalogbook
description: "Drive omalogbook the way a sailor does, from the CLI and the Quickshell window when it can run, and keep proof. Use before calling any behavior change done, when reviewing someone else's change, or when asked to verify omalogbook."
---

# Verify omalogbook

`scripts/verify.sh` proves the code is sound. This skill proves the log works. It runs the binary from this checkout, drives a feature from the sailor's side, and keeps evidence. Run both. The agent that built a change does not sign off on this run.

## Launch

Run the drive in a bash you start for that purpose. Do not source `drive.sh` and do not paste it into the shell you are already using.

```sh
env -u BASH_ENV bash --noprofile --norc .cursor/skills/verify-omalogbook/drive.sh [features]
```

`bash --noprofile --norc` still sources `BASH_ENV` before it reads `drive.sh`. A `BASH_ENV` file that runs `exit 0` exits 0 and prints nothing. That is not a pass. `env -u BASH_ENV` clears it. Do not source `drive.sh`. If `drive.sh` still sees `BASH_ENV`, it unsets that name and re-execs with `env -u BASH_ENV bash --noprofile --norc`. When unset fails, the process exits 1 and does not exec.

That process is the whole drive. It sets `set -euo pipefail` and `set -m`. On EXIT, INT, and TERM it ignores further INT and TERM, then sends TERM and KILL to each pid it recorded from `setsid`. It copies the artifacts out, and deletes the run directory only after that copy includes a marker for this run. Its exit status is the result. The shell that started it keeps the options and traps it had before the command.

A pass is exit status 0 and a last line `evidence:` followed by a directory. That directory contains `.copy-marker`. The line `evidence: none` is not a pass. Exit 0 with no `evidence:` line is not a pass.

With no arguments the drive runs doctor, note, today, totals, correct, confine, window, and run. Name the features you want.

```sh
env -u BASH_ENV bash --noprofile --norc .cursor/skills/verify-omalogbook/drive.sh doctor note today totals correct confine
env -u BASH_ENV bash --noprofile --norc .cursor/skills/verify-omalogbook/drive.sh window
env -u BASH_ENV bash --noprofile --norc .cursor/skills/verify-omalogbook/drive.sh pause
```

`pause` sleeps in the foreground and starts a sleeper in its own session. The sleeper ignores TERM. It does not build. Ctrl-C during that sleep is how you see the trap. A second INT during cleanup is ignored, and the KILL still runs. The sleeper is gone when the drive exits.

On a pass the drive prints `evidence: /tmp/omalogbook-verify-evidence.XXXXXX` last. That directory remains. When the copy did not land, the last line is `evidence: none` and the exit status is not 0. The run directory remains only when the copy failed. The failure line names that path.

`note`, `today`, `amend`, `strike`, and `path` ignore `--vault`. They read `$XDG_CONFIG_HOME/omalogbook/config.toml`. The drive writes that file under the run directory and passes `HOME`, `XDG_CONFIG_HOME`, and `XDG_RUNTIME_DIR` on each command. It does not export them. `TZ` is `UTC`. At start the drive unsets `OMALOGBOOK_BIN`, `OMALOGBOOK_THEME_DIR`, `OMALOGBOOK_WIDTH`, and `OMALOGBOOK_HEIGHT` when the parent exported them, and each launch passes `env -u` for those names. The window command then sets `OMALOGBOOK_BIN` to the realpath of `./target/debug/omalogbook`.

Before the binary starts, the drive canonicalizes the vault with `realpath -m`. That resolves symlinks, `..`, and missing directories. `$run/../x`, `$run/no-such/../../x`, and a `$run/vault` symlink whose target sits outside `$run` are rejected by doctor and by every launch. The binary is not executed, and nothing is written at the escaped path. `confirm_drive` runs before the run directory is removed. A git status change, or a new or changed file under `/tmp` or the sailor's home, fails the drive. Cargo, rustup, npm, and editor caches are left out of the home walk. The checkout's `.git` directory is left out too, because `git status` rewrites `.git/index` while the porcelain output stays empty. The run directory and the evidence directory are left out of the `/tmp` walk. A directory the walk cannot read is left out as well. `find` exits 1 for that directory, and the drive keeps the files `find` could read instead of aborting.

`cargo` on `PATH` may be older than `Cargo.toml` `rust-version`. The drive prepends `$HOME/.local/bin` and runs `mise exec -- cargo build --locked`. `bash --noprofile --norc` still inherits `PATH` from the shell that started it.

Do not export `HOME`, `XDG_CONFIG_HOME`, or `XDG_RUNTIME_DIR` from the shell that starts the drive.

## Doctor

Doctor refuses a vault that does not canonicalize inside the run directory. It then runs `--version` and `vault`. The version line is this checkout. `vault` prints `$run/vault`.

## Drive

Follow `features/`. Start at `features/README.md`. A proof that drives one entry point is incomplete when the map lists others.

`confine` is the attack on the vault check, on `confirm_drive`, and on the socket predicate. It is not a sailor-facing command. `pause` is the signal check. It is not a sailor-facing command either.

## Evidence

`record` saves the command, stdout, stderr, and the exit code under `$run/artifacts/verify/<id>/`. An empty stdout or stderr file is removed. The EXIT trap copies that tree into the evidence directory.

Check a write with a second read of the file on disk. Exercise the real command. There is no test-only endpoint.

A `grim` screenshot is the window's proof when Quickshell is up and the window process is still alive at the start and the end of `grim`. The `./run.sh` transcript, and `today --json` plus `presets --json`, are what you can capture without Quickshell. Name them as substitutes. Do not call them a screenshot. A missing `quickshell` or `grim`, or a Qt6 skip from `scripts/verify.sh engine`, makes the window feature exit non-zero.

If a path cannot be reached, the drive writes the command and the missing precondition and exits non-zero. Do not mark that path passed.

## Cleanup

The trap is the cleanup. It runs on a normal exit, on Ctrl-C, and on TERM. Ctrl-C and TERM ignore further INT and TERM before they exit, so a second Ctrl-C does not leave a sleeper, a hub, or the window running. The EXIT trap then sends TERM and KILL, with no sleep between those two signals, to each pid recorded when `setsid` started it and to each session `note_tree` recorded under that pid. That pid is the process group. Cleanup does not ask the socket which process is listening. After those signals, the drive checks `pgrep -g` for each group and kills any process whose cwd, command, or `XDG_RUNTIME_DIR` is under `$run`. It copies the artifacts and `outside-diff.txt` when that file exists, checks the marker, and only then deletes the run directory. A pass ends with `evidence:` and a directory. A failure ends with that directory when the copy includes the artifacts and, if the drive wrote outside the run, `outside-diff.txt`. When that copy does not land, the last line is `evidence: none` and the run directory stays.

When omalogbook's commands or the lines it prints change, run `/maintain-verification-skill` over this directory.
