# Add up the log

`omalogbook totals` reads every note in the vault and prints distance, time under way, and the records. It is the only read command that honors `--vault`.

## Sub-features

- `totals-empty` is a vault with no notes.
- `totals-morning` adds up the pinned morning.
- `totals-json` is the same sum as one JSON object.
- `totals-other` reads a second folder with `--vault` and leaves the config vault alone.

## How to get to it (user POV)

- Run `omalogbook totals` for the whole log.
- Run `omalogbook totals --json` for a window or the bar widget.
- Run `omalogbook totals --vault DIR` to add up some other folder.

## Driving it with the isolated vault

Preconditions:

- Launch and Doctor from `../SKILL.md` are done. `omalogbook vault` prints `$run/vault`.
- `art="$run/artifacts/verify/totals"`.

`today.md` leaves the morning in `$run/vault`. This recipe keeps that morning in its own folder and empties the config vault, so the first total is still `Dash · nothing logged yet`.

```sh
: "${run:?}" && rm -rf "$run/vault" "$run/morning" && mkdir -p "$run/vault" "$run/morning" "$art" && cp -a tests/golden/morning/. "$run/morning/"
```

- **Nothing logged.** Run `: "${run:?}" && [ -r "$run/env" ] && env $(cat "$run/env") ./target/debug/omalogbook totals`. Exit 0. Stdout is `Dash · nothing logged yet`.
- **The morning.** Run `: "${run:?}" && [ -r "$run/env" ] && env $(cat "$run/env") ./target/debug/omalogbook totals --vault "$run/morning"`. Exit 0. Stdout contains `Dash · 2026-09-13 to 2026-09-13 · 1 day sailed of 1`, `1.0 nm`, `19 min`, `3.0 kn under way`, and `4.7 kn on 2026-09-13`.
- **JSON.** Run `: "${run:?}" && [ -r "$run/env" ] && env $(cat "$run/env") ./target/debug/omalogbook totals --vault "$run/morning" --json`. Exit 0. Stdout contains `"v":1`, `"boat":"Dash"`, `"distanceNm":1.0`, and `"date":"2026-09-13"`.
- **The config vault is unchanged.** Run `: "${run:?}" && [ -r "$run/env" ] && env $(cat "$run/env") ./target/debug/omalogbook totals` again with no `--vault`. Stdout is still `Dash · nothing logged yet`. `find "$run/vault" -type f` prints nothing.
- **Proof.** Save each command's stdout, stderr, and exit code under `$art`. `: "${run:?}" && mkdir -p "${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/totals" && cp -a "$art/." "${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/totals/"`.

## Gotchas

- `totals --vault` overrides the config for that one command. `today` and `note` do not.
- A note with no distance and no time under way is a day logged, not a day sailed. The heading then says `days logged, none sailed yet`.
- The morning is not today on a later date, so the human totals omit a `Today` row. The JSON carries `"today":null` when the UTC date is not `2026-09-13`. `todayPath` is still filled in, and the file need not exist.
