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

- Launch and Doctor from `../SKILL.md` are done. `drive vault` prints `$run/vault`.
- `art="$run/artifacts/verify/today"`.

Replace the vault with the pinned morning. The checkout copy stays untouched.

```sh
reset_today() {
  local opts
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
  [ -n "${run:-}" ] || return 1
  [ -n "${art:-}" ] || return 1
  rm -rf "$run/vault" || return 1
  mkdir -p "$run/vault" "$art" || return 1
  cp -a tests/golden/morning/. "$run/vault/" || return 1
}
reset_today
```

`git status --porcelain -- tests/golden/morning` is empty after the drive.

- **Read 13 September 2026.** Run `drive today --date 2026-09-13`. Exit 0. Stdout contains `2026-09-13 · Dash`, `log opened`, `under way`, `stopped`, `1.0 nm`, and `under way 19 min`.
- **The track is still there.** Run `test -f "$run/vault/tracks/2026-09-13-120100.gpx"`. Exit 0. The note links that file. The command does not rewrite it.
- **JSON for the window.** Run `drive today --date 2026-09-13 --json`. Exit 0. Stdout contains `"v":1`, `"date":"2026-09-13"`, `"boat":"Dash"`, `"distanceNm":1.0`, and `under way`.
- **Where the editor opens.** Run `drive path --date 2026-09-13`. Exit 0. Stdout is `$run/vault/2026/09/2026-09-13.md`.
- **A day with no note.** Run `drive today --date 2026-01-02`. Exit 0. Stdout is `2026-01-02 · Dash` and nothing else. `$run/vault/2026/01/2026-01-02.md` does not exist.
- **The path of a day with no note.** Run `drive path --date 2026-01-02`. Exit 0. Stdout is `$run/vault/2026/01/2026-01-02.md` and nothing else. That file still does not exist.
- **A bad date.** Run `drive today --date Tuesday`. Exit 64. Stderr contains `--date wants a day like 2026-09-18`.
- **Proof.** Save each command's stdout, stderr, and exit code under `$art`. `save_evidence today`.

## Gotchas

- `today` ignores `--vault`. A `--vault` argument is skipped, not applied. The config vault is the one that is read.
- The morning's front matter says `hours_underway: 0.33`. `today` prints that as `19 min`. The watch line in the note says `20 min`. Both are the pinned file.
- `mise goldens` compares `tests/golden/morning` under `TZ=UTC`. `BLESS=1` rewrites that directory and nothing else. Do not set `BLESS` while verifying.
- An empty day is not an error. `today` does not create the note. `path` prints where the file would be and does not create it either.
