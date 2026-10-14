# omalogbook verification map

This directory is the source for verifying omalogbook's user-facing behavior. Read this index, then use the matching feature file as the recipe. A change that adds or alters a user path updates its feature file in the same PR.

## Baseline preconditions

- A debug build of this checkout, launched per `../SKILL.md`, with `HOME`, `XDG_CONFIG_HOME`, and `XDG_RUNTIME_DIR` inside that run's temp directory.
- `config.toml` sets `vault` to `$run/vault`, `boat` to `Dash`, and `git` to `false`. `TZ` is `UTC`.
- Doctor passes, and `omalogbook vault` prints `$run/vault`.
- Never drive a vault, config, or hub this run did not create.

## Driving conventions

- Start every recipe from the baseline unless its preconditions say otherwise.
- Commands are literal. Keep quoted names and flags unchanged.
- Every `omalogbook` command runs as `env $(cat "$run/env") ./target/debug/omalogbook`, written `omalogbook` below.
- Prefer stable handles. Subcommands, flags, and the words a note prints. Not screen coordinates.
- A recipe that reads the pinned morning replaces `$run/vault` with a copy of `tests/golden/morning`. A recipe that writes starts from an empty `$run/vault`. Do not mix the two.
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
