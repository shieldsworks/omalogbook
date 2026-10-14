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

Each fenced command is one function. The function saves `$-` and restores the settable letters with a RETURN trap. `set` rejects `i` and `s`, so those two stay as the shell already has them. The trap puts back the RETURN trap `trap -p` showed on the way in. A failed check is `return 1`. Leave `set -e` and `set -u` off in the function body. Both skip the RETURN trap, and `set -e` inside a function exits an interactive shell. After the function returns, `$-` matches the shell from before the call, and `trap -p` shows the RETURN trap from before the call.

`$(set +o)` runs in a subshell. With `inherit_errexit` off, that subshell prints `set +o errexit` even when this shell has `set -e`. Evaluating that print clears the caller's errexit. Save `$-` in this shell.

`:?` does not exit an interactive shell. It aborts the rest of that list, and the next line still runs. A guard on one line does not stop a launch on the next line. Put the guard and the launch in the same function, in one `&&` list.

`$run/pids` is one pid per line. A fence appends the pid it started. A guard that fails before that line does not kill a pid already listed. `drop_pid` kills one pid and removes that line. `stop_drive` kills every line.

```sh
drop_pid() {
  local pid=$1
  [ -n "$pid" ] || return 0
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  if [ -n "${run:-}" ] && [ -f "$run/pids" ]; then
    grep -v -x -F -- "$pid" "$run/pids" > "$run/pids.tmp" || true
    mv "$run/pids.tmp" "$run/pids"
  fi
}
```

`env_pins_run` is the guard for every `env $(cat "$run/env")` launch. It does not set a trap. The file has to be a regular non-empty file. `HOME` and `XDG_CONFIG_HOME` have to sit under `$run`, and so does `vault` in `$XDG_CONFIG_HOME/omalogbook/config.toml`. `$run-evil` does not match `$run/*`. An empty file, a directory, and a partial file all fail, and the launch in the same `&&` list does not run.

```sh
env_pins_run() {
  local home config vault
  [ -n "${run:-}" ] || return 1
  [ -f "$run/env" ] || return 1
  [ -s "$run/env" ] || return 1
  home=$(sed -n 's/^HOME=//p' "$run/env" | head -1)
  config=$(sed -n 's/^XDG_CONFIG_HOME=//p' "$run/env" | head -1)
  case "$home" in
    "$run"/*) ;;
    *) return 1 ;;
  esac
  case "$config" in
    "$run"/*) ;;
    *) return 1 ;;
  esac
  [ -f "$config/omalogbook/config.toml" ] || return 1
  vault=$(sed -n 's/^vault = "\([^"]*\)"/\1/p' "$config/omalogbook/config.toml" | head -1)
  case "$vault" in
    "$run"/*) ;;
    *) return 1 ;;
  esac
}
```

```sh
prepare_drive() {
  local saved=$- prev letters bin wayland
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
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
  local saved=$- prev letters
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] && env_pins_run && env $(cat "$run/env") ./target/debug/omalogbook "$@"
}
```

`drive` returns 1 when `run` is unset or `env_pins_run` fails, and it does not start the binary. The shell stays up. `env $(cat "$run/env")` with no guard runs the binary in the ambient environment. An empty file makes `cat` print nothing, `HOME` stays the sailor's home, and the default vault is `~/Logbook`. A directory and a partial file do the same. The `&&` list refuses all three.

Ready when Doctor passes. There is no port and no prompt.

`omalogbook run` needs a live omakeel. This skill does not start one and does not invent a socket. See `features/run.md`.

## Doctor

Run this before the first drive, and again when a result looks wrong.

```sh
doctor() {
  local saved=$- prev letters vault resolved
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  realpath ./target/debug/omalogbook || return 1
  drive --version || return 1
  vault=$(drive vault) || return 1
  resolved=$(realpath "$vault" 2>/dev/null || printf '%s' "$vault")
  case "$resolved" in
    "$run"/*) ;;
    *) return 1 ;;
  esac
}
doctor
```

The binary path is inside this checkout. The version line is this build. `vault` prints a path under `$run`. `realpath` follows a symlink. A target outside `$run` makes `doctor` return 1. The config did not take.

## Drive

Follow `features/`. Start at `features/README.md`. A proof that drives one entry point is incomplete when the map lists others. Run the recipes in that index order. Each recipe resets the vault it reads.

The window is `open_window` in `features/window.md`. That function exports `$run/env` and backgrounds `./run.sh`. `OMALOGBOOK_BIN` in that file is the absolute path `prepare_drive` wrote. When a compositor is up, the same file holds an absolute `WAYLAND_DISPLAY`. `./run.sh` execs `quickshell -p ui/shell.qml`. That needs Quickshell on a Hyprland session. When `quickshell` is not on `PATH`, still drive `features/window.md`. `open_window` records the miss and returns 1. That is not a pass of the painted window.

Copy `tests/golden/morning` only when a recipe says so. Never point the config at `tests/golden/` itself. A later `note` would write into the checkout.

## Evidence

Write under `$run/artifacts/verify/<feature-id>/`. `save_evidence` copies that directory. It returns 1 if `run` is unset.

```sh
save_evidence() {
  local saved=$- prev letters dest
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
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

A recipe may leave Quickshell or `omalogbook run` in the background. Both append a line to `$run/pids`. `stop_drive` kills every line and returns 1 only when `run` is unset. A guard that fails before the append leaves the list alone. The shell stays up so you can still run this.

```sh
stop_drive() {
  local saved=$- prev letters pid
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] || return 1
  if [ -f "$run/pids" ]; then
    while IFS= read -r pid || [ -n "$pid" ]; do
      [ -n "$pid" ] || continue
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    done < "$run/pids"
    : > "$run/pids"
  fi
}
stop_drive
```

Leave `${EVIDENCE_DIR:-/tmp/omalogbook-evidence}` in place. Do not delete the sailor's vault, config, or runtime directory.

Confirm the evidence directory still holds the feature's files. Then confirm the checkout and `$HOME/Logbook` match the snapshots Launch wrote. `confirm_drive` returns 1 if either comparison differs. The shell stays up. The files `$run/git-after` and `$run/logbook-after` are the mismatch.

```sh
confirm_drive() {
  local saved=$- prev letters git_diff=0 log_diff=0 logbook
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
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
