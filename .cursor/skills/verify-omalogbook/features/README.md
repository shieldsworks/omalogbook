# omalogbook verification map

This directory is the source for verifying omalogbook's user-facing behavior. Read this index, then use the matching feature file as the recipe. A change that adds or alters a user path updates its feature file in the same PR.

## Baseline preconditions

- A debug build of this checkout, launched per `../SKILL.md`, with `HOME`, `XDG_CONFIG_HOME`, and `XDG_RUNTIME_DIR` inside that run's temp directory.
- `config.toml` sets `vault` to `$run/vault`, `boat` to `Dash`, and `git` to `false`. `TZ` is `UTC`.
- `$run/env` sets `OMALOGBOOK_BIN` to the realpath of this checkout's debug binary. When `WAYLAND_DISPLAY` was set at launch, the file holds the absolute socket path.
- Doctor passes, and `drive vault` prints `$run/vault`.
- Launch has saved `git status --porcelain` and a `find -H` listing of `$HOME/Logbook`. `confirm_drive` returns 1 if either comparison differs.
- Never drive a vault, config, or hub this run did not create. The one exception is a read-only look at the live `keel.sock`, in `run.md`.

## Driving conventions

- Run the recipes in the index order below. Each recipe's first command resets the vault it is about to use, so a later recipe does not inherit the previous note.
- Commands are literal. Keep the function, the quoted names, and the flags unchanged.
- Every `omalogbook` command is `drive` plus the subcommand. `drive` is defined in `../SKILL.md`. The guard and the command are that one function. A failed guard returns 1 and does not start the binary.

```sh
drive note "Dolphins off the port side"
```

`:?` does not exit an interactive shell. The next line would still run, so do not split the guard onto its own line. Do not shorten a command to `omalogbook`.
- A command that stays running is one function. `open_window` backgrounds `./run.sh`. `follow_hub` backgrounds `omalogbook run`. See `window.md` and `run.md`.
- `today` and `window` copy `tests/golden/morning` into `$run/vault`. `totals` copies that morning to `$run/morning` and empties `$run/vault`. `note`, `correct`, and a live `run` each empty `$run/vault`.
- Prefer stable handles. Subcommands, flags, and the words a note prints. Not screen coordinates.
- Do not delete proof during cleanup.

## Proof and skip reporting

- CLI proof is the command, stdout, stderr, and the exit code.
- Mutation proof is a second read of the note on disk.
- Window proof, when Quickshell runs on Hyprland, is a `grim` screenshot with the log visible.
- Window proof, when Quickshell does not run, is the `./run.sh` transcript plus `today --json` and `presets --json`. Those do not stand in for the screenshot.
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
- [The log window](./window.md) is `./run.sh`. Without Quickshell the painted window is unreachable. The file names the substitute that was run.
- [Keep the watch](./run.md) is `omalogbook run`. It needs a live omakeel and is not driven by inventing a socket.
