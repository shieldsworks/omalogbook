# omalogbook verification map

This directory is the source for verifying omalogbook's user-facing behavior. Read this index, then use the matching feature file as the recipe. A change that adds or alters a user path updates its feature file in the same PR.

## Baseline preconditions

- A debug build of this checkout, launched by `env -u BASH_ENV bash --noprofile --norc ../drive.sh`. `HOME`, `XDG_CONFIG_HOME`, and `XDG_RUNTIME_DIR` are passed on each command and point inside that run's temp directory. The shell that starts the drive does not export them.
- `config.toml` sets `vault` to `$run/vault`, `boat` to `Dash`, and `git` to `false`. `TZ` is `UTC`.
- The binary is the realpath of `./target/debug/omalogbook`. The window gets that path as `OMALOGBOOK_BIN`. When `WAYLAND_DISPLAY` was set, the window command gets the absolute socket path.
- Doctor passes, and `vault` prints `$run/vault`. A vault that canonicalizes outside the run directory never starts the binary.
- The drive snapshots `git status --porcelain` and the files under `/tmp` and the sailor's home. `confirm_drive` fails the drive if either comparison differs. Cargo, rustup, npm, and editor caches are not part of the home walk.
- Never drive a vault, config, or hub this run did not create. The one exception is a read-only look at the live `keel.sock`, in `run.md`.

## Driving conventions

- The drive is one child process. Do not paste the recipes into the shell you mean to keep, and do not source `drive.sh`.
- Run the recipes in the index order below. Each recipe resets the vault it is about to use, so a later recipe does not inherit the previous note.
- Commands inside `drive.sh` are literal. Keep the quoted names and the flags unchanged.
- A failed feature stops the drive. `window` and `run` exit non-zero when Quickshell or the live hub is absent, so name the features you can finish when you need a zero exit.
- `today` and `window` copy `tests/golden/morning` into `$run/vault`. `totals` copies that morning to `$run/morning` and empties `$run/vault`. `note`, `correct`, and a live `run` each empty `$run/vault`.
- Prefer stable handles. Subcommands, flags, and the words a note prints. Not screen coordinates.
- Do not delete proof during cleanup. The evidence directory stays. The run directory is removed only after the copy contains this run's marker.

## Proof and skip reporting

- CLI proof is the command, stdout, stderr, and the exit code.
- Mutation proof is a second read of the note on disk.
- Window proof, when Quickshell runs on Hyprland and the window is alive for the whole `grim`, is a `grim` image. When `quickshell` or `grim` is missing, the drive writes `miss.txt` and exits non-zero. That file is not a screenshot.
- `scripts/verify.sh engine` passes this map only when `All engine-path checks passed.` is in the transcript and `Qt6 QML dev packages not installed.` is in neither stream.
- Record the feature id with every artifact.
- An unreachable path is reported with the command tried and the unmet precondition. It is never reported as verified through a different path.

## Feature entry contract

Each feature file starts with an H1 and one paragraph, then these four H2s, in order.

1. `Sub-features`
2. `How to get to it (user POV)`
3. `Driving it with the isolated vault`, or `Driving it with the start script` for the window, or `Driving it with a live hub` for the watch
4. `Gotchas`

## Features

- [Write a note](./note.md) files the sailor's words, a mark, and a slash word that is not a mark.
- [Read the day](./today.md) prints one day, including the pinned morning, and the path an editor opens.
- [Add up the log](./totals.md) sums every note, and can point `--vault` at another folder.
- [Correct an entry](./correct.md) amends words, strikes a line, or erases one.
- [The log window](./window.md) is `./run.sh`. Without Quickshell, grim, or Qt6 the painted window is unreachable. The drive records that and exits non-zero. The substitute is not a pass.
- [Keep the watch](./run.md) is `omalogbook run`. It needs a live omakeel. A socket this drive created is not that proof. `confine`, inside `drive.sh`, checks the listener and a protocol line with a stand-in.
