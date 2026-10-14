# The log window

`./run.sh` opens the log in its own Quickshell process. The window reads `omalogbook today --json`, lists marks with `omalogbook presets --json`, and files a line with `omalogbook note`. It does not talk to omakeel itself.

## Sub-features

- `window-launch` is `./run.sh` under Quickshell on Hyprland.
- `window-missing` records the tools this machine does not have, and the exit from `./run.sh`.
- `window-read` is `today --json` and `presets --json`, the two reads the window makes on open.
- `window-engine` is `scripts/verify.sh engine`, which checks that the window would start this checkout's release binary.

## How to get to it (user POV)

- Run `./run.sh` from the checkout.
- The Omarchy plugin loads the same window. The binary is `OMALOGBOOK_BIN`, or `target/release/omalogbook` beside the `ui` directory, or `omalogbook` on `PATH`.
- Night Watch is the `n` key, or the button, when the line is empty. Escape clears the line, and closes the window when the line is already empty.

## Driving it with the start script

Preconditions:

- Launch and Doctor from `../SKILL.md` are done, so the debug binary and `$run/env` exist.
- The window resolves `OMALOGBOOK_BIN`, or else `target/release/omalogbook`. `./run.sh` only execs Quickshell. Recording a missing `quickshell` does not need the release binary.
- `art="$run/artifacts/verify/window"; mkdir -p "$art"`.
- Quickshell, Hyprland, and `grim` are required for the painted window. When they are missing, `window-launch` is unreachable. The other steps are the substitutes, and they are not a screenshot.

- **Look for the window.** Record the display and the tools.

```sh
{
  echo "DISPLAY=${DISPLAY:-unset}"
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
```

- **Open the window.** Run `./run.sh >"$art/run.out" 2>"$art/run.err"`. When `quickshell` is missing, the exit is 127 and stderr names `quickshell`. The painted window was not driven. Observed on 2026-10-14. Exit 127. Stderr was `./run.sh: line 8: exec: quickshell: not found`.
- **What the window would read.** With the pinned morning copied into `$run/vault` per `today.md`, run `omalogbook today --date 2026-09-13 --json` and `omalogbook presets --json`. Exit 0. The day JSON contains `"v":1` and `under way`. The presets JSON contains `"word":"depart"`.
- **The binary the window would start.** Run `scripts/verify.sh engine`. When `pkg-config` cannot see Qt6, the step prints `Qt6 QML dev packages not installed.` and the summary says `Not run: engine`. The script can still exit 0. That is not a pass of this feature. When Qt6 is present, the transcript says `All engine-path checks passed.` and names this checkout's `target/release/omalogbook`. That transcript is still not a screenshot. Observed on 2026-10-14 with Qt6 missing. The transcript was `Passed: nothing` and `Not run: engine`, exit 0.
- **Proof.** Save `tools.txt`, the `./run.sh` stdout and stderr, the two JSON replies, and the engine transcript under `$art`. Copy `$art` to `${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/window/` before cleanup. Do not label any of them as a window screenshot.

## Gotchas

- `./run.sh` does not set `OMALOGBOOK_BIN`. The window looks for `target/release/omalogbook` first. A debug build alone is not the binary a painted window runs, unless you export `OMALOGBOOK_BIN` to it.
- `scripts/verify.sh engine` exits 0 when Qt6 is missing and lists `engine` under `Not run`. Read that line. The exit code alone is not a pass of `window-engine`.
- Do not install Quickshell to invent a painted window on a machine with no Hyprland session.
- The window and `omalogbook run` are different programs. The window shows the log. `run` is what keeps the watch.
