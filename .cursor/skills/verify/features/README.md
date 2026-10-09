# omalogbook verification map

This directory is the maintained source for verifying omalogbook's user-facing behavior. Read this index before driving the app, then use the matching feature file as the recipe. Keep it current. A change that adds or alters a user path updates its feature file in the same change.

## Baseline preconditions

- A build of this checkout, launched per `../SKILL.md`, with `XDG_CONFIG_HOME` and `XDG_RUNTIME_DIR` inside that run's temp directory.
- For the sample sail, a copy of `tests/golden/morning` in `$run/vault`.
- Doctor passes, and `omalogbook vault` prints `$run/vault`.
- Never drive an instance this run did not start, and never write the user's own log.

## Driving conventions

- Start every recipe from the baseline unless its preconditions say otherwise.
- Commands are literal. Keep quoted names and flags unchanged.
- Prefer stable handles. CLI subcommands, flags, and file paths. Not screen coordinates.
- Do not delete proof during cleanup.

## Proof and skip reporting

- CLI proof is the command, stdout, stderr, and exit code.
- Window proof needs a Hyprland session and `grim`. This box does not have that session, so a window path is reported as not run.
- Mutation proof is a second read of the file that was written.
- Record the feature ID and entry point with every artifact.
- An unreachable path is reported with the command tried and the unmet precondition, never as verified through a different path.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior, then exactly these four H2s, in order.

1. `Sub-features`, short IDs, one line each.
2. `How to get to it (user POV)`, every user entry point.
3. `Driving it with the golden`, starting with `Preconditions:`, then labeled bullets pairing each user action with an exact command and the observable result.
4. `Gotchas`, traps that waste or invalidate a run.

Keep implementation details out of the map. Name user paths, stable handles, required state, commands, and observable proof.

## Features

- [Sample sail](./sample-sail.md) covers the pinned morning note, its GPX, and reading both back with `today` and `totals`.
