---
name: verify-omalogbook
description: "Drive omalogbook the way a user does, from the CLI, and capture proof. Use before calling any behavior change done, when reviewing someone else's change, or when asked to verify omalogbook."
---

# Verify omalogbook

`scripts/verify.sh` proves the code is sound. This skill proves the app works. It launches the real binary, drives a feature from the user's side, and keeps evidence. Run both. The agent that built a change should not be the one that signs off on this run.

## Launch

`note` and `today` ignore `--vault`. They read `config.toml` from `XDG_CONFIG_HOME`. `run` follows a socket under `XDG_RUNTIME_DIR`. Put both directories inside a temp dir so a drive never touches the user's vault or a live omakeel socket.

```sh
mise install && cargo build --locked
run=$(mktemp -d /tmp/omalogbook-verify.XXXXXX)
mkdir -p "$run/config/omalogbook" "$run/runtime" "$run/vault" "$run/artifacts"
printf 'vault = "%s"\nboat = "Dash"\ngit = false\n' "$run/vault" > "$run/config/omalogbook/config.toml"
export XDG_CONFIG_HOME="$run/config"
export XDG_RUNTIME_DIR="$run/runtime"
```

`omalogbook run` needs a live omakeel. This skill does not start one and does not fake a socket. The sample sail is driven from `tests/golden/morning` instead. See `features/sample-sail.md`.

Short commands need no server. Build once, then run each drive as its own process with the exports above.

## Doctor

Run this before driving, and again when a result looks wrong.

```sh
./target/debug/omalogbook --version
./target/debug/omalogbook vault
```

The version is this checkout's build. `vault` prints `$run/vault`. If it prints a path under the user's home, the config did not take and you must stop.

## Drive

Follow `features/`. A proof that drives one entry point is incomplete when the map lists others.

Copy `tests/golden/morning` into `$run/vault` before reading it. Never point the config at `tests/golden/` itself. `note` would write into the checkout.

## Evidence

Put the command, stdout, stderr, and exit code in `$run/artifacts/verify/<feature-id>/`.

- Check side effects with a second look at the file on disk, not only the first reply.
- Exercise the real command. No test-only endpoints.
- Report anything you could not reach, with the command tried and the missing precondition. A skipped entry point is never reported as passed.

## Cleanup

This drive starts no daemon. Leave `$run/artifacts` in place. Do not delete the user's vault, config, or runtime directory. Those paths were never used.
