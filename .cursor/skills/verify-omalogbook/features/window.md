# The log window

`./run.sh` opens the log in its own Quickshell process. The window reads `omalogbook today --json`, lists marks with `omalogbook presets --json`, and files a line with `omalogbook note`. It does not talk to omakeel itself.

## Sub-features

- `window-launch` is `./run.sh` under Quickshell on Hyprland, with the run directory's env on the command.
- `window-missing` records the tools this machine does not have, and the exit from `./run.sh`.
- `window-read` is `today --json` and `presets --json`, the two reads the window makes on open.
- `window-engine` is `scripts/verify.sh engine`. It builds a stub binary under `mktemp` and prints whether the window's start script would exec that stub.

## How to get to it (user POV)

- Run `./run.sh` from the checkout.
- The Omarchy plugin loads the same window. The binary is `OMALOGBOOK_BIN`, or `target/release/omalogbook` beside the `ui` directory, or `omalogbook` on `PATH`.
- Night Watch is the NIGHT button at the top right. The label reads `NIGHT  n`. The text field holds focus, so the `n` key is a letter in the note. Click the button. Escape clears the line, and closes the window when the line is already empty.

## Driving it with the start script

Preconditions:

- Run it with the drive in `../SKILL.md`. `OMALOGBOOK_BIN` is the realpath of `./target/debug/omalogbook`. When `WAYLAND_DISPLAY` is set, the command gets that socket as an absolute path.
- Quickshell, Hyprland, and `grim` are required for the painted window. When they are missing, `window-launch` is unreachable. The other steps are the substitutes, and they are not a screenshot.

The reads use the pinned morning.

- **Look for the window.** The drive records `DISPLAY`, `WAYLAND_DISPLAY`, and whether `quickshell`, `Hyprland`, `grim`, `qmllint`, and Qt6 are present.
- **Open the window.** The command is `env HOME="$run/home" XDG_CONFIG_HOME="$run/config" XDG_RUNTIME_DIR="$run/runtime" TZ=UTC OMALOGBOOK_BIN="$bin" WAYLAND_DISPLAY="$wayland" ./run.sh`, omitting `WAYLAND_DISPLAY` when the shell has none. `./run.sh` is started with `setsid`. When `quickshell` is missing, the exit is 127 and stderr names `quickshell`. `window-painted/miss.txt` says `missing command: quickshell`, and `window-painted/exit-code.txt` is 127. When `quickshell` is present and `grim` is missing, the same file names `grim` and the exit code is 127. When both are present, the window leader has to be alive when `grim` starts and still alive when `grim` returns. Otherwise `miss.txt` says `process was not alive`. The capture command is `timeout -k 5 10 grim -`. A `miss.txt` makes the drive exit non-zero.
- **What the window would read.** `omalogbook today --date 2026-09-13 --json` and `omalogbook presets --json`. Exit 0. The day JSON contains `"v":1` and `under way`. The presets JSON contains `"word":"depart"`.
- **The binary the start script would exec.** `env XDG_RUNTIME_DIR=$run/engine-probe-runtime scripts/verify.sh engine`. That runtime directory is inside the run directory and mode `700`. A pass prints `All engine-path checks passed.` and does not print `Qt6 QML dev packages not installed.`. When Qt6 is missing the script prints that sentence and exits 0. The drive writes `window-engine/miss.txt` and exits non-zero. The summary line `Passed: nothing` is not a pass of the engine path.
- **Proof.** Save `tools.txt`, the `./run.sh` stdout and stderr, `miss.txt` or the `grim` image, the two JSON replies, and the engine transcript. Do not label a transcript as a window screenshot.

```sh
env -u BASH_ENV bash --noprofile --norc .cursor/skills/verify-omalogbook/drive.sh window
```

## Gotchas

- `./run.sh` does not set `OMALOGBOOK_BIN` or the vault. A bare `./run.sh` inherits the sailor's home. The window then uses `OMALOGBOOK_BIN`, or `target/release/omalogbook`, or `omalogbook` on `PATH`, and the config in that home. The default vault is `$HOME/Logbook` and the default `git` is true, so Enter writes that log and commits when the folder is a git repository. This drive passes the run directory on the command, so `OMALOGBOOK_BIN` is the debug binary and the vault is `$run/vault`.
- A relative `WAYLAND_DISPLAY` is looked up under the shell's `XDG_RUNTIME_DIR` before that variable is replaced with `$run/runtime`.
- `scripts/verify.sh engine` exits 0 when Qt6 is missing and lists `engine` under `Not run`. The drive returns non-zero for that transcript. A pass names the temp stub in `started:`, not this checkout's `target/release/omalogbook`.
- Do not install Quickshell to invent a painted window on a machine with no Hyprland session.
- The window and `omalogbook run` are different programs. The window shows the log. `run` is what keeps the watch.
- The drive starts `./run.sh` with `setsid`. Cleanup signals that process group after the leader has exited.
