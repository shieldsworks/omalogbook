---
name: verify-omalogbook
description: "Drive omalogbook the way a sailor does, from the CLI and the Quickshell window when it can run, and keep proof. Use before calling any behavior change done, when reviewing someone else's change, or when asked to verify omalogbook."
---

# Verify omalogbook

`scripts/verify.sh` proves the code is sound. This skill proves the log works. It launches the binary from this checkout, drives a feature from the sailor's side, and keeps evidence. Run both. The agent that built a change does not sign off on this run.

## Launch

Run from the repo root. `note`, `today`, `amend`, `strike`, and `path` ignore `--vault`. They read `$XDG_CONFIG_HOME/omalogbook/config.toml`. `run` follows `$XDG_RUNTIME_DIR/omakeel/keel.sock`. Put `HOME`, the config directory, and the runtime directory inside one temp dir so a drive never touches the sailor's vault or a live hub.

```sh
mise install
cargo build --locked
run=$(mktemp -d /tmp/omalogbook-verify.XXXXXX)
mkdir -p "$run/config/omalogbook" "$run/runtime" "$run/vault" "$run/home" "$run/artifacts/verify"
printf 'vault = "%s"\nboat = "Dash"\ngit = false\n' "$run/vault" > "$run/config/omalogbook/config.toml"
cat > "$run/env" <<EOF
HOME=$run/home
XDG_CONFIG_HOME=$run/config
XDG_RUNTIME_DIR=$run/runtime
TZ=UTC
EOF
```

Short commands need no server. Build once, then start each drive as its own process.

```sh
env $(cat "$run/env") ./target/debug/omalogbook <subcommand>
```

Ready when Doctor passes. There is no port and no prompt.

`omalogbook run` needs a live omakeel. This skill does not start one and does not invent a socket. See `features/run.md`.

## Doctor

Run this before the first drive, and again when a result looks wrong.

```sh
realpath ./target/debug/omalogbook
env $(cat "$run/env") ./target/debug/omalogbook --version
env $(cat "$run/env") ./target/debug/omalogbook vault
```

The binary path is inside this checkout. The version line is this build. `vault` prints `$run/vault`. Stop if it prints a path outside `$run`. The config did not take.

## Drive

Follow `features/`. Start at `features/README.md`. A proof that drives one entry point is incomplete when the map lists others.

The terminal command is `env $(cat "$run/env") ./target/debug/omalogbook`. Feature files write that as `omalogbook`.

The window is `./run.sh`. It execs `quickshell -p ui/shell.qml`. That needs Quickshell on a Hyprland session. When `quickshell` is not on `PATH`, still drive `features/window.md` and say the painted window was not driven.

Copy `tests/golden/morning` into `$run/vault` only when a recipe says so. Never point the config at `tests/golden/` itself. A later `note` would write into the checkout.

## Evidence

Write under `$run/artifacts/verify/<feature-id>/`, then copy that directory to `${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/<feature-id>/` before cleanup.

Save the command, stdout, stderr, and the exit code. Check a write with a second read of the file on disk, not only the first reply. Exercise the real command. There is no test-only endpoint.

A `grim` screenshot is the window's proof. The `./run.sh` transcript, and `today --json` plus `presets --json`, are what you can capture without Quickshell. Name them as substitutes. Do not call them a screenshot.

If a path cannot be reached, write the command and the missing precondition. Do not mark that path passed.

## Cleanup

This drive starts no daemon unless a recipe started `omalogbook run`. Kill only that pid.

```sh
if [ -f "$run/pid" ]; then
  kill "$(cat "$run/pid")" 2>/dev/null || true
fi
```

Leave `${EVIDENCE_DIR:-/tmp/omalogbook-evidence}` in place. Do not delete the sailor's vault, config, or runtime directory. The launch never used them. After cleanup, confirm the evidence directory still holds the feature's files, and that `git status --porcelain` in the checkout matches the status from before the run.

## Helpers

No helper script. The binary is the drive. Isolation is the file `$run/env`.

When the commands or the lines they print change, run `/maintain-verification-skill` over this directory.
