---
name: verify-omalogbook
description: "Drive omalogbook the way a sailor does, from the CLI and the Quickshell window when it can run, and keep proof. Use before calling any behavior change done, when reviewing someone else's change, or when asked to verify omalogbook."
---

# Verify omalogbook

`scripts/verify.sh` proves the code is sound. This skill proves the log works. It launches the binary from this checkout, drives a feature from the sailor's side, and keeps evidence. Run both. The agent that built a change does not sign off on this run.

## Launch

Run from the repo root, in the interactive shell you mean to keep. `note`, `today`, `amend`, `strike`, and `path` ignore `--vault`. They read `$XDG_CONFIG_HOME/omalogbook/config.toml`. `run` follows `$XDG_RUNTIME_DIR/omakeel/keel.sock` unless you pass `--socket`. Put `HOME`, the config directory, and the runtime directory inside one temp dir so a drive never touches the sailor's vault or a live hub.

Resolve `WAYLAND_DISPLAY` before the env file replaces `XDG_RUNTIME_DIR`. A relative name is a socket under the compositor's runtime directory. An absolute name still resolves after that directory changes.

`OMALOGBOOK_BIN` is the realpath of `./target/debug/omalogbook`. The commands below start `./target/debug/omalogbook` from the repo root. The window reads `OMALOGBOOK_BIN`.

Each fenced command is one function. The function saves `set +o` and restores those options with a RETURN trap, then clears that trap. A failed check is `return 1`. Leave `set -e` and `set -u` off. Both skip the RETURN trap, and `set -e` inside a function exits an interactive shell. After the function returns, `$-` matches the shell from before the call, and `trap -p` does not list this function's RETURN trap.

`:?` does not exit an interactive shell. It aborts the rest of that list, and the next line still runs. A guard on one line does not stop a launch on the next line. Put the guard and the launch in the same function.

`stop_pid` stops the pid stored in `$run/pid`. Call it before `return 1` when a launch fence fails, so a background process is not left running.

```sh
stop_pid() {
  if [ -n "${run:-}" ] && [ -f "$run/pid" ]; then
    kill "$(cat "$run/pid")" 2>/dev/null || true
    wait "$(cat "$run/pid")" 2>/dev/null || true
  fi
}
```

```sh
prepare_drive() {
  local opts bin wayland
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
  mise install || return 1
  cargo build --locked || return 1
  run=$(mktemp -d /tmp/omalogbook-verify.XXXXXX) || return 1
  mkdir -p "$run/config/omalogbook" "$run/runtime" "$run/vault" "$run/home" "$run/artifacts/verify" || return 1
  printf 'vault = "%s"\nboat = "Dash"\ngit = false\n' "$run/vault" > "$run/config/omalogbook/config.toml" || return 1
  bin=$(realpath ./target/debug/omalogbook) || return 1
  printf 'HOME=%s\nXDG_CONFIG_HOME=%s\nXDG_RUNTIME_DIR=%s\nTZ=UTC\nOMALOGBOOK_BIN=%s\n' \
    "$run/home" "$run/config" "$run/runtime" "$bin" > "$run/env" || return 1
  if [ -n "${WAYLAND_DISPLAY:-}" ]; then
    case $WAYLAND_DISPLAY in
      /*) wayland=$WAYLAND_DISPLAY ;;
      *) wayland=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/$WAYLAND_DISPLAY ;;
    esac
    printf 'WAYLAND_DISPLAY=%s\n' "$wayland" >> "$run/env" || return 1
  fi
  git status --porcelain --untracked-files=all > "$run/git-before" || return 1
  printf '%s\n' "$HOME/Logbook" > "$run/logbook-path" || return 1
  if [ -d "$HOME/Logbook" ]; then
    find -H "$HOME/Logbook" -printf '%p %T@ %s\n' > "$run/logbook-before.unsorted" || return 1
    sort "$run/logbook-before.unsorted" > "$run/logbook-before" || return 1
  else
    printf 'absent\n' > "$run/logbook-before" || return 1
  fi
}
prepare_drive
```

`find -H` follows `$HOME/Logbook` when that path is a symlink, and lists the files in the target. A write inside the target changes the snapshot. `find` without `-H` lists the symlink and misses that write.

Short commands need no server. Build once, then start each drive as its own process.

`drive` is the prefix for every `omalogbook` command in `features/`. A bare `omalogbook` reads `$HOME/Logbook`.

```sh
drive() {
  local opts
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
  if [ -z "${run:-}" ] || [ ! -r "$run/env" ]; then
    stop_pid
    return 1
  fi
  env $(cat "$run/env") ./target/debug/omalogbook "$@"
}
```

`drive` returns 1 when `run` is unset or `$run/env` is missing, and it does not start the binary. The shell stays up. `env $(cat "$run/env")` with neither check runs the binary in the ambient environment. A failed `cat` leaves the argument list empty, and the shell keeps going.

Ready when Doctor passes. There is no port and no prompt.

`omalogbook run` needs a live omakeel. This skill does not start one and does not invent a socket. See `features/run.md`.

## Doctor

Run this before the first drive, and again when a result looks wrong.

```sh
doctor() {
  local opts
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
  realpath ./target/debug/omalogbook || return 1
  drive --version || return 1
  drive vault || return 1
}
doctor
```

The binary path is inside this checkout. The version line is this build. `vault` prints `$run/vault`. Stop if it prints a path outside `$run`. The config did not take.

## Drive

Follow `features/`. Start at `features/README.md`. A proof that drives one entry point is incomplete when the map lists others. Run the recipes in that index order. Each recipe resets the vault it reads.

The window is `open_window` in `features/window.md`. That function exports `$run/env` and backgrounds `./run.sh`. `OMALOGBOOK_BIN` in that file is the absolute path `prepare_drive` wrote. When a compositor is up, the same file holds an absolute `WAYLAND_DISPLAY`. `./run.sh` execs `quickshell -p ui/shell.qml`. That needs Quickshell on a Hyprland session. When `quickshell` is not on `PATH`, still drive `features/window.md` and say the painted window was not driven.

Copy `tests/golden/morning` only when a recipe says so. Never point the config at `tests/golden/` itself. A later `note` would write into the checkout.

## Evidence

Write under `$run/artifacts/verify/<feature-id>/`. `save_evidence` copies that directory. It returns 1 if `run` is unset.

```sh
save_evidence() {
  local opts dest
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
  [ -n "${run:-}" ] || return 1
  [ -n "${1:-}" ] || return 1
  dest=${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/$1
  mkdir -p "$dest" || return 1
  cp -a "$run/artifacts/verify/$1/." "$dest/"
}
```

Save the command, stdout, stderr, and the exit code. Check a write with a second read of the file on disk, not only the first reply. Exercise the real command. There is no test-only endpoint.

A `grim` screenshot is the window's proof. The `./run.sh` transcript, and `today --json` plus `presets --json`, are what you can capture without Quickshell. Name them as substitutes. Do not call them a screenshot.

If a path cannot be reached, write the command and the missing precondition. Do not mark that path passed.

## Cleanup

A recipe may leave Quickshell or `omalogbook run` in the background. Both write `$run/pid`. `stop_drive` stops that pid and returns 1 only when `run` is unset. A failed launch fence calls `stop_pid` itself, and the shell stays up so you can still run this.

```sh
stop_drive() {
  local opts
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
  [ -n "${run:-}" ] || return 1
  stop_pid
}
stop_drive
```

Leave `${EVIDENCE_DIR:-/tmp/omalogbook-evidence}` in place. Do not delete the sailor's vault, config, or runtime directory.

Confirm the evidence directory still holds the feature's files. Then confirm the checkout and `$HOME/Logbook` match the snapshots Launch wrote. `confirm_drive` returns 1 if either comparison differs. The shell stays up. The files `$run/git-after` and `$run/logbook-after` are the mismatch.

```sh
confirm_drive() {
  local opts git_diff=0 log_diff=0 logbook
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
  [ -n "${run:-}" ] || return 1
  git status --porcelain --untracked-files=all > "$run/git-after" || return 1
  cmp -s "$run/git-before" "$run/git-after" || git_diff=1
  logbook=$(cat "$run/logbook-path") || return 1
  if [ -d "$logbook" ]; then
    find -H "$logbook" -printf '%p %T@ %s\n' > "$run/logbook-after.unsorted" || return 1
    sort "$run/logbook-after.unsorted" > "$run/logbook-after" || return 1
  else
    printf 'absent\n' > "$run/logbook-after" || return 1
  fi
  cmp -s "$run/logbook-before" "$run/logbook-after" || log_diff=1
  [ "$git_diff" -eq 0 ] && [ "$log_diff" -eq 0 ]
}
confirm_drive
```

A failing `confirm_drive` means the drive wrote outside `$run`.

## Helpers

No helper script. The binary is the drive. Isolation is `$run/env`, and `drive` refuses to start without it.

When the commands or the lines they print change, run `/maintain-verification-skill` over this directory.
