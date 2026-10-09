# Sample sail

A short morning under way, written as one day's note and one GPX track. The pinned copy is `tests/golden/morning`. It is the passage a person would see in the log after `omalogbook run` followed omakeel for that morning.

## Sub-features

- `morning-note` is `2026/09/2026-09-13.md`, with log opened, under way, and stopped.
- `morning-track` is `tracks/2026-09-13-120100.gpx`, the passage that started at 12:01 UTC.
- `read-back` is `today` and `totals` reading that note from a temp vault.

## How to get to it (user POV)

- Run `omalogbook run --vault DIR --no-git` in a terminal while omakeel is up. The log writes the day's note and a GPX track into DIR.
- Run `omalogbook today --date 2026-09-13` to read that day.
- Run `omalogbook totals` to add the vault up.
- On a Hyprland session, `./run.sh` opens the same day in the window.

## Driving it with the golden

Preconditions:

- Launch from `../SKILL.md` is done. `XDG_CONFIG_HOME` and `XDG_RUNTIME_DIR` are inside `$run`. `omalogbook vault` prints `$run/vault`.
- `cp -a tests/golden/morning/. "$run/vault/"` has been run. The checkout copy is not the vault.
- `mise` is on `PATH`.

- **Compare.** The user trusts the pinned morning. Run `mise goldens`. Exit 0. `git status --porcelain` is unchanged.
- **Read the day.** The user opens 13 September 2026. Run `./target/debug/omalogbook today --date 2026-09-13`. Exit 0. Stdout contains `log opened`, `under way`, `stopped`, `1.0 nm`, and `under way 19 min`.
- **Add up.** The user asks for the whole log. Run `./target/debug/omalogbook totals`. Exit 0. Stdout contains `1.0 nm`, `19 min`, and `4.7 kn on 2026-09-13`.
- **Proof.** Save both commands' stdout, stderr, and exit codes to `$run/artifacts/verify/morning/`.

## Gotchas

- `note` and `today` ignore `--vault`. On `note`, a `--vault` argument becomes part of the sentence. Set the vault in `$XDG_CONFIG_HOME/omalogbook/config.toml`.
- `today` and `totals` read `hours_underway: 0.33`. That prints as 19 min. The watch line in the note says 20 min. Both are the pinned file.
- `mise goldens` sets `TZ=UTC`. `BLESS=1` rewrites `tests/golden/morning` and nothing else. Do not set `BLESS` while verifying.
- `omalogbook run` needs a live omakeel. This recipe does not start `run`, and it does not fake a keel.
- The golden bytes were measured on x86_64 only. The raw day's run is 1.0086797403 nm, which prints as 1.0.
