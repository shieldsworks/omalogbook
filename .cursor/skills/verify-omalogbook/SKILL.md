---
name: verify-omalogbook
description: "Drive omalogbook the way a sailor does, from the CLI and the Quickshell window when it can run, and keep proof. Use before calling any behavior change done, when reviewing someone else's change, or when asked to verify omalogbook."
---

# Verify omalogbook

`scripts/verify.sh` proves the code is sound. This skill proves the log works. It launches the binary from this checkout, drives a feature from the sailor's side, and keeps evidence. Run both. The agent that built a change does not sign off on this run.

## Launch

Run from the repo root. `note`, `today`, `amend`, `strike`, and `path` ignore `--vault`. They read `$XDG_CONFIG_HOME/omalogbook/config.toml`. `run` follows `$XDG_RUNTIME_DIR/omakeel/keel.sock` unless you pass `--socket`. Put `HOME`, the config directory, and the runtime directory inside one temp dir so a drive never touches the sailor's vault or a live hub.

Resolve `WAYLAND_DISPLAY` before the env file replaces `XDG_RUNTIME_DIR`. A relative name is a socket under the compositor's runtime directory. An absolute name still resolves after that directory changes.

```sh
mise install
cargo build --locked
run=$(mktemp -d /tmp/omalogbook-verify.XXXXXX)
mkdir -p "$run/config/omalogbook" "$run/runtime" "$run/vault" "$run/home" "$run/artifacts/verify"
printf 'vault = "%s"\nboat = "Dash"\ngit = false\n' "$run/vault" > "$run/config/omalogbook/config.toml"
bin=$(realpath ./target/debug/omalogbook)
cat > "$run/env" <<EOF
HOME=$run/home
XDG_CONFIG_HOME=$run/config
XDG_RUNTIME_DIR=$run/runtime
TZ=UTC
OMALOGBOOK_BIN=$bin
EOF
if [ -n "${WAYLAND_DISPLAY:-}" ]; then
  case $WAYLAND_DISPLAY in
    /*) wayland=$WAYLAND_DISPLAY ;;
    *) wayland=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/$WAYLAND_DISPLAY ;;
  esac
  printf 'WAYLAND_DISPLAY=%s\n' "$wayland" >> "$run/env"
fi
git status --porcelain --untracked-files=all > "$run/git-before"
printf '%s\n' "$HOME/Logbook" > "$run/logbook-path"
if [ -d "$HOME/Logbook" ]; then
  find "$HOME/Logbook" -printf '%p %T@ %s\n' | sort > "$run/logbook-before"
else
  printf 'absent\n' > "$run/logbook-before"
fi
```

Short commands need no server. Build once, then start each drive as its own process.

Every command in `features/` starts with this prefix. Copy the whole line. A bare `omalogbook` reads `$HOME/Logbook`.

```sh
: "${run:?}" && [ -r "$run/env" ] && env $(cat "$run/env") ./target/debug/omalogbook <subcommand>
```

`: "${run:?}"` exits the shell when `run` is unset. `[ -r "$run/env" ]` skips the binary when the env file is missing. `env $(cat "$run/env")` with neither guard runs the binary in the ambient environment. A failed `cat` leaves the argument list empty, and the shell keeps going.

Ready when Doctor passes. There is no port and no prompt.

`omalogbook run` needs a live omakeel. This skill does not start one and does not invent a socket. See `features/run.md`.

## Doctor

Run this before the first drive, and again when a result looks wrong.

```sh
realpath ./target/debug/omalogbook
: "${run:?}" && [ -r "$run/env" ] && env $(cat "$run/env") ./target/debug/omalogbook --version
: "${run:?}" && [ -r "$run/env" ] && env $(cat "$run/env") ./target/debug/omalogbook vault
```

The binary path is inside this checkout. The version line is this build. `vault` prints `$run/vault`. Stop if it prints a path outside `$run`. The config did not take.

## Drive

Follow `features/`. Start at `features/README.md`. A proof that drives one entry point is incomplete when the map lists others. Run the recipes in that index order. Each recipe resets the vault it reads.

The window is `./run.sh` with `$run/env`. That env sets `OMALOGBOOK_BIN` to `./target/debug/omalogbook` and, when a compositor is up, an absolute `WAYLAND_DISPLAY`. `./run.sh` execs `quickshell -p ui/shell.qml`. That needs Quickshell on a Hyprland session. When `quickshell` is not on `PATH`, still drive `features/window.md` and say the painted window was not driven.

Copy `tests/golden/morning` only when a recipe says so. Never point the config at `tests/golden/` itself. A later `note` would write into the checkout.

## Evidence

Write under `$run/artifacts/verify/<feature-id>/`. Create the evidence directory, then copy.

```sh
: "${run:?}" && mkdir -p "${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/<feature-id>" && cp -a "$run/artifacts/verify/<feature-id>/." "${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/<feature-id>/"
```

Save the command, stdout, stderr, and the exit code. Check a write with a second read of the file on disk, not only the first reply. Exercise the real command. There is no test-only endpoint.

A `grim` screenshot is the window's proof. The `./run.sh` transcript, and `today --json` plus `presets --json`, are what you can capture without Quickshell. Name them as substitutes. Do not call them a screenshot.

If a path cannot be reached, write the command and the missing precondition. Do not mark that path passed.

## Cleanup

A recipe may leave Quickshell or `omalogbook run` in the background. Both write `$run/pid`. Kill that pid.

```sh
: "${run:?}"
if [ -f "$run/pid" ]; then
  kill "$(cat "$run/pid")" 2>/dev/null || true
  wait "$(cat "$run/pid")" 2>/dev/null || true
fi
```

Leave `${EVIDENCE_DIR:-/tmp/omalogbook-evidence}` in place. Do not delete the sailor's vault, config, or runtime directory.

Confirm the evidence directory still holds the feature's files. Then confirm the checkout and `$HOME/Logbook` match the snapshots Launch wrote.

```sh
: "${run:?}"
git status --porcelain --untracked-files=all > "$run/git-after"
cmp -s "$run/git-before" "$run/git-after"
logbook=$(cat "$run/logbook-path")
if [ -d "$logbook" ]; then
  find "$logbook" -printf '%p %T@ %s\n' | sort > "$run/logbook-after"
else
  printf 'absent\n' > "$run/logbook-after"
fi
cmp -s "$run/logbook-before" "$run/logbook-after"
```

Stop if either `cmp` fails. The drive wrote outside `$run`.

## Helpers

No helper script. The binary is the drive. Isolation is `$run/env`, and the prefix on each command refuses to start without it.

When the commands or the lines they print change, run `/maintain-verification-skill` over this directory.
