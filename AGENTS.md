# Working on omalogbook

omalogbook keeps the ship's log for Omahoy. `omalogbook run` follows omakeel and writes one markdown note per day, plus a GPX track for each passage. `omalogbook note`, `omalogbook today`, and `omalogbook totals` read and write that vault from the terminal. The Quickshell window in `ui/` shows the same day. The notes are the log. A person opens them in any editor.

## Toolchain

Run `mise install` first. `mise.toml` pins the Rust toolchain, including rustfmt and clippy. `Cargo.toml`'s `rust-version` is the minimum the code promises. A system cargo older than that fails. `mise tasks` lists every job.

The window runs under Quickshell. `./run.sh` starts it. This repo does not need GTK or other system libraries to build.

## Done means verified

You are not done until this passes from the repo root.

```sh
scripts/verify.sh
```

It runs, in order, `mise lint`, `mise test`, the comment check, qmllint on `ui/` when qmllint is installed, `mise goldens`, and a check that verification left no new files in the tree. A step that cannot run is listed as not run. It is not counted as passed. CI runs `VERIFY_SKIP=qml scripts/verify.sh`, then `cargo build --release --locked`, on x86_64 and aarch64. A separate job runs `scripts/verify.sh qml` with PySide6-Essentials 6.12.0. Fix what a check reports. Do not weaken the check that reported it.

Commands:

- `mise install`
- `mise lint`
- `mise test`
- `scripts/verify.sh`
- `mise goldens` byte-compares `tests/golden/morning` under `TZ=UTC`. `BLESS=1 mise goldens` rewrites that directory only.

To drive the binary the way a user does, use `.cursor/skills/verify/SKILL.md` and the feature map in `.cursor/skills/verify/features/`.

## The gates are not yours to move

These files set the rules. Change them only in a change whose whole purpose is changing them, and have a human review that change.

- `[lints]` in `Cargo.toml`, and `clippy.toml`
- `.github/workflows/`, `scripts/verify.sh`, `scripts/check-comments.sh`
- `mise.toml` task definitions for `lint` and `test`

To silence one lint at one site, use `#[expect(clippy::<lint>, reason = "<the fact that makes this correct>")]` on the smallest item. `#[allow]` without a reason fails the build, and `#[expect]` fails once the code stops needing it. A crate-level allow is never the fix.

## Every behavior change has a test or a golden

- A bug fix starts with a failing test that reproduces the bug.
- New behavior lands with the test that pins it. For a day's note or a GPX file, update the golden and say why the bytes changed.
- Do not edit a golden by hand. Do not regenerate one to make a failing test pass without saying what a person would see.
- Refactors change no test expectations. If one has to change, it was not a refactor.
- Tests write only to a temp directory. `BLESS=1` is the one path that writes into the checkout, and it rewrites only `tests/golden/<name>/`.

## No apologetic comments

Comments state facts about the code and the world it handles. They do not apologize, defer, or hedge. `scripts/check-comments.sh` fails the build on TODO, FIXME, XXX, HACK, workaround, temporary fix, quick fix, for the time being, not ideal, should be fixed, sorry, kludge, and band-aid. If something is wrong, fix it in this change or open an issue and leave the code honest. A constraint from outside the repo is written as the fact. Say what the outside thing does, and what this code does about it.

## Copy the right pattern

You will copy what you see. Before copying, check that the code you copy passes today's lints and has a test. Old code may predate the rules.

- No `unwrap()` outside tests. Return a `Result` with context, or `expect("<invariant that guarantees this>")` when it truly cannot fail.
- Every `unsafe` block carries a `// SAFETY:` comment saying why it is sound. Keep unsafe to the existing libc calls. Do not add a new one without need.
- A bare `as` that can truncate is a real narrowing. Prefer `try_from` or `From` when the range is not already known.
- Take the shortcut only if it is also the right path. If the right path is hard, say so in the change instead of shipping the shortcut.

## Review

The agent that wrote a change does not approve, merge, or mark it verified. A different agent, with fresh context and a clean checkout, or Casey, reviews it and runs `scripts/verify.sh` plus the verify skill for the features the change touches.

The reviewer reports what it ran and saw, not what it assumes.

## Conventions

- Rust, edition 2024, written from scratch. Dependencies stay `serde_json`, `libc`, and `tokio` unless a task says otherwise.
- Always pass `--locked`. Do not change `Cargo.lock` unless the task is a dependency change.
- The on-disk format is `docs/format.md`. Change that file in the same change as a format change. There is no `docs/protocol.md`.
- Distance is `src/geo.rs`. GPX is `src/track.rs`. Do not add GDAL, GEOS, PROJ, or a map crate.
- The window in `ui/` runs under Quickshell as an Omarchy plugin. Keep it in step with the notes `omalogbook today --json` writes.

## Rules specific to omalogbook

- No GDAL, GEOS, PROJ, or map crates.
- Dependencies stay `serde_json`, `libc`, and `tokio` unless a task says otherwise.
- Keep `docs/format.md` in the same change as a format change.
- Every behavior change needs a test or a golden.
- No apologetic or workaround comments.
- The agent that builds a change does not approve it.

## Layout

- `src/lib.rs` declares the library modules.
- `src/main.rs` is the `omalogbook` command.
- `src/config.rs` reads `config.toml`.
- `src/day.rs` owns one day's note.
- `src/entry.rs` formats one log line.
- `src/geo.rs` computes distance and writes positions.
- `src/git.rs` commits the vault when git is on.
- `src/keel.rs` reads omakeel's socket.
- `src/lock.rs` keeps one writer on a note at a time.
- `src/preset.rs` parses marks such as `/depart`.
- `src/sun.rs` computes sunset for `/depart`.
- `src/tide.rs` reads omatide for a mark.
- `src/time.rs` turns an epoch into the boat's date and clock.
- `src/totals.rs` adds up every day in the vault.
- `src/track.rs` writes the GPX track.
- `src/trip.rs` sums the trip for `/berth`.
- `src/watch.rs` turns fixes into entries, tracks, and the day's run.
- `src/wind.rs` reads omawind for a mark.
- `src/wire.rs` reads another engine's socket up to one deadline.
- `ui/` is the Quickshell window.
- `scripts/` holds `verify.sh` and `check-comments.sh`.
- `tests/golden/` is the pinned UTC sail, written by `tests/golden.rs`.
