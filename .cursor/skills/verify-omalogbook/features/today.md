# Read the day

`omalogbook today` prints one day's entries and its run. `omalogbook path` prints the note an editor should open. Neither command needs the hub to be up.

## Sub-features

- `today-morning` reads the pinned morning in `tests/golden/morning`.
- `today-json` is the same day as one JSON object, which the window reads.
- `today-path` prints the note path and does not create the file.
- `today-empty` prints the boat and the date when that day has no note.
- `today-bad-date` refuses a `--date` that is not a day.

## How to get to it (user POV)

- Run `omalogbook today` for the day the boat is on.
- Run `omalogbook today --date 2026-09-13` for an earlier day.
- Run `omalogbook today --json` for the window.
- Run `omalogbook path` and open that file.

## Driving it with the isolated vault

Preconditions:

- Launch and Doctor from `../SKILL.md` are done. `omalogbook vault` prints `$run/vault`.
- Replace the vault with the pinned morning. `rm -rf "$run/vault" && mkdir -p "$run/vault" && cp -a tests/golden/morning/. "$run/vault/"`.
- The checkout copy stays untouched. `git status --porcelain -- tests/golden/morning` is empty after the drive.
- `art="$run/artifacts/verify/today"; mkdir -p "$art"`.

- **Read 13 September 2026.** Run `omalogbook today --date 2026-09-13`. Exit 0. Stdout contains `2026-09-13 · Dash`, `log opened`, `under way`, `stopped`, `1.0 nm`, and `under way 19 min`.
- **The track is still there.** Run `test -f "$run/vault/tracks/2026-09-13-120100.gpx"`. Exit 0. The note links that file. The command does not rewrite it.
- **JSON for the window.** Run `omalogbook today --date 2026-09-13 --json`. Exit 0. Stdout contains `"v":1`, `"date":"2026-09-13"`, `"boat":"Dash"`, `"distanceNm":1.0`, and `under way`.
- **Where the editor opens.** Run `omalogbook path --date 2026-09-13`. Exit 0. Stdout is `$run/vault/2026/09/2026-09-13.md`.
- **A day with no note.** Run `omalogbook today --date 2026-01-02`. Exit 0. Stdout is `2026-01-02 · Dash` and nothing else. `$run/vault/2026/01/2026-01-02.md` does not exist.
- **A bad date.** Run `omalogbook today --date Tuesday`. Exit 64. Stderr contains `--date wants a day like 2026-09-18`.
- **Proof.** Save each command's stdout, stderr, and exit code under `$art`. Copy `$art` to `${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/today/` before cleanup. Observed on 2026-10-14. The morning ended with `1.0 nm · fastest 4.7 kn · under way 19 min`. The JSON carried `"v":1`, `"distanceNm":1.0`, and `"underwaySecs":1188`. The empty day printed only `2026-01-02 · Dash` and created no file. `--date Tuesday` exited 64.

## Gotchas

- `today` ignores `--vault`. A `--vault` argument is skipped, not applied. The config vault is the one that is read.
- The morning's front matter says `hours_underway: 0.33`. `today` prints that as `19 min`. The watch line in the note says `20 min`. Both are the pinned file.
- `mise goldens` compares `tests/golden/morning` under `TZ=UTC`. `BLESS=1` rewrites that directory and nothing else. Do not set `BLESS` while verifying.
- An empty day is not an error. `today` does not create the note.
