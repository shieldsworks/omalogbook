# The log window

`./run.sh` opens the log in its own Quickshell process. The window reads `omalogbook today --json`, lists marks with `omalogbook presets --json`, and files a line with `omalogbook note`. It does not talk to omakeel itself.

## Sub-features

- `window-launch` is `./run.sh` under Quickshell on Hyprland, with `$run/env`.
- `window-missing` records the tools this machine does not have, and the exit from `./run.sh`.
- `window-read` is `today --json` and `presets --json`, the two reads the window makes on open.
- `window-engine` is `scripts/verify.sh engine`. It builds a stub binary under `mktemp` and prints whether the window's start script would exec that stub.

## How to get to it (user POV)

- Run `./run.sh` from the checkout.
- The Omarchy plugin loads the same window. The binary is `OMALOGBOOK_BIN`, or `target/release/omalogbook` beside the `ui` directory, or `omalogbook` on `PATH`.
- Night Watch is the NIGHT button at the top right. The label reads `NIGHT  n`. The text field holds focus, so the `n` key is a letter in the note. Click the button. Escape clears the line, and closes the window when the line is already empty.

## Driving it with the start script

Preconditions:

- Launch and Doctor from `../SKILL.md` are done, so the debug binary and `$run/env` exist. `$run/env` sets `OMALOGBOOK_BIN` to the realpath of `./target/debug/omalogbook`. When Launch saw `WAYLAND_DISPLAY`, the file holds that socket as an absolute path.
- `art="$run/artifacts/verify/window"`.
- Quickshell, Hyprland, and `grim` are required for the painted window. When they are missing, `window-launch` is unreachable. The other steps are the substitutes, and they are not a screenshot.

The reads below use the pinned morning. `correct.md` has just written today's note into `$run/vault`, so replace it here.

```sh
reset_window() {
  local saved=$- prev letters
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] || return 1
  [ -n "${art:-}" ] || return 1
  rm -rf "$run/vault" || return 1
  mkdir -p "$run/vault" "$art" || return 1
  cp -a tests/golden/morning/. "$run/vault/" || return 1
}
reset_window
```

- **Look for the window.** Record the display and the tools.

```sh
record_window_tools() {
  local saved=$- prev letters
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] || return 1
  [ -n "${art:-}" ] || return 1
  {
    echo "DISPLAY=${DISPLAY:-unset}"
    echo "WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-unset}"
    grep '^WAYLAND_DISPLAY=' "$run/env" || echo "pinned WAYLAND_DISPLAY unset"
    grep '^OMALOGBOOK_BIN=' "$run/env"
    command -v quickshell || echo "quickshell missing"
    command -v Hyprland || echo "Hyprland missing"
    command -v grim || echo "grim missing"
    command -v qmllint || echo "qmllint missing"
    if pkg-config --exists Qt6Core Qt6Gui Qt6Qml Qt6Network; then
      echo "Qt6 present"
    else
      echo "Qt6 missing"
    fi
    echo "ui qml: $(ls ui/*.qml | wc -l)"
  } | tee "$art/tools.txt"
}
record_window_tools
```

- **Open the window.** `open_window` checks `run` and `env_pins_run`, then backgrounds `./run.sh` in the same `&&` list. A failed check returns 1. It does not start `./run.sh`, and it does not kill a pid already in `$run/pids`. Take a `grim` shot only while that pid is alive. A missing `quickshell` or `grim` is written down, and the function returns 1.

```sh
open_window() {
  local saved=$- prev letters child exit_code
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] && [ -n "${art:-}" ] && env_pins_run && {
    env $(cat "$run/env") ./run.sh >"$art/run.out" 2>"$art/run.err" &
    child=$!
    printf '%s\n' "$child" >> "$run/pids" || { drop_pid "$child"; return 1; }
    window_pid=$child
  } || return 1
  sleep 1
  if kill -0 "$window_pid" 2>/dev/null; then
    if command -v grim >/dev/null; then
      grim "$art/window.png" || {
        echo "grim failed" | tee "$art/grim.txt"
        return 1
      }
    else
      echo "grim missing" | tee "$art/grim.txt"
      return 1
    fi
  else
    exit_code=0
    wait "$window_pid" || exit_code=$?
    {
      echo "painted window not shot"
      command -v quickshell || echo "quickshell missing"
      echo "exit:$exit_code"
    } | tee "$art/grim.txt"
    return 1
  fi
}
open_window
```

When `quickshell` is missing, the exit is 127 and stderr names `quickshell`. The painted window was not driven. `open_window` returns 1. `stop_drive` kills every pid in `$run/pids`.
- **What the window would read.** Run `drive today --date 2026-09-13 --json` and `drive presets --json`. Exit 0. The day JSON contains `"v":1` and `under way`. The presets JSON contains `"word":"depart"`.
- **The binary the start script would exec.** Run `drive_engine` from the repo root, after `run` is set. The step writes a stub to `target/release/omalogbook` under a `mktemp` directory, for a checkout and for a plugin tree, and deletes that directory before it returns. The stub records its own path. It is not this checkout's binary. `PATH` inside the step is `/usr/bin:/bin`.

  When the probe loads Qt Quick, each of the four blocks prints `qml: REPO=` and `repo:` and `want:` and `started:` for that temp tree, then `ok`. `started:` is `<temp>/checkout/target/release/omalogbook` or the plugin copy. The step then prints `window surfaces book.lastLog` and `All engine-path checks passed.` The summary line is `Passed: engine`. That transcript is still not a screenshot.

  When `pkg-config` cannot see Qt6, the step prints `Qt6 QML dev packages not installed.` and the summary says `Passed: nothing` and `Not run: engine`. The script can still exit 0. `drive_engine` records the transcript and returns 1 unless the transcript contains `All engine-path checks passed.`

  When `pkg-config` sees Qt6 and the QML modules are missing, `repo:` is empty, `started:` is `<none>`, the log line is `exec: omalogbook: not found`, each block prints `FAIL`, and the summary is `FAILED: engine`. The exit is 1.

```sh
drive_engine() {
  local saved=$- prev letters
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] && [ -n "${art:-}" ] || return 1
  scripts/verify.sh engine >"$art/engine.out" 2>"$art/engine.err" || true
  cat "$art/engine.out"
  grep -F "All engine-path checks passed." "$art/engine.out" >/dev/null
}
```
- **Proof.** Save `tools.txt`, the `./run.sh` stdout and stderr, `grim.txt` or `window.png`, the two JSON replies, and the engine transcript under `$art`. `save_evidence window`. Do not label a transcript as a window screenshot.

## Gotchas

- `./run.sh` does not set `OMALOGBOOK_BIN` or the vault. A bare `./run.sh` inherits the sailor's home. The window then uses `OMALOGBOOK_BIN`, or `target/release/omalogbook`, or `omalogbook` on `PATH`, and the config in that home. The default vault is `$HOME/Logbook` and the default `git` is true, so Enter writes that log and commits when the folder is a git repository. This drive exports `$run/env`, so `OMALOGBOOK_BIN` is the realpath of `./target/debug/omalogbook` and the vault is `$run/vault`.
- A relative `WAYLAND_DISPLAY` is looked up under `XDG_RUNTIME_DIR`. `$run/env` points `XDG_RUNTIME_DIR` at the empty temp runtime, so Launch stores the compositor socket as an absolute path before that replacement.
- `scripts/verify.sh engine` exits 0 when Qt6 is missing and lists `engine` under `Not run`. `drive_engine` returns 1 for that transcript. A pass names the temp stub in `started:`, not this checkout's `target/release/omalogbook`.
- Do not install Quickshell to invent a painted window on a machine with no Hyprland session.
- The window and `omalogbook run` are different programs. The window shows the log. `run` is what keeps the watch.
