# Correct an entry

`omalogbook amend` replaces the words of one entry. `omalogbook strike` rules a line through it. `omalogbook strike --erase` removes it. The clock on the line stays.

## Sub-features

- `correct-amend` changes the words and leaves the clock.
- `correct-strike` wraps the body in `~~`.
- `correct-erase` removes the entry. The path is what the command prints.
- `correct-expect` refuses a change when `--expect` is not the line on disk.

## How to get to it (user POV)

- Run `omalogbook today` and read the numbers.
- Run `omalogbook amend 1 "Dolphins off the starboard side"`.
- Run `omalogbook strike 1` to rule a line through entry 1.
- Run `omalogbook strike 2 --erase` to take entry 2 out.
- The window sends `--expect` with the line it last showed. Pass the same flag from the terminal when the log may still be writing.

## Driving it with the isolated vault

Preconditions:

- Launch and Doctor from `../SKILL.md` are done. `drive vault` prints `$run/vault`.
- `art="$run/artifacts/verify/correct"`.

`totals.md` leaves `$run/vault` empty. Empty it again so this note has a clock and no position, and no morning entries underneath.

```sh
reset_correct() {
  local saved=$- prev letters
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] || return 1
  [ -n "${art:-}" ] || return 1
  rm -rf "$run/vault" || return 1
  mkdir -p "$run/vault" "$art" || return 1
  env_pins_run && day=$(env $(cat "$run/env") date +%F) || return 1
}
reset_correct
```

There is no keel socket under `$run/runtime`.

- **File two lines.** Run `drive note "Dolphins off the port side"`, then `drive note "Crab boat crossed astern"`. Exit 0 both times.
- **An index past the end.** Run `drive strike 9`. Exit 1. Stderr is `omalogbook: the log for $day has 2 entries`. Both lines are still in the note.
- **Amend the first.** Run `drive today` and copy line 1 without its number. Run `drive amend 1 --expect "$line" "Dolphins off the starboard side"`. Exit 0. Stdout contains `Dolphins off the starboard side` and the same `**` clock as `$line`. A second `today` shows the new words on line 1 and `Crab boat crossed astern` on line 2.
- **Strike the first.** Copy the amended line from `today`. Run `drive strike 1 --expect "$line"`. Exit 0. Stdout contains `~~` and `Dolphins off the starboard side`. `today` still lists two entries.
- **Erase the second.** Copy line 2. Run `drive strike 2 --expect "$line" --erase`. Exit 0. Stdout is the note path. `today` no longer contains `Crab boat crossed astern`. The struck dolphin line remains.
- **A stale expect.** Run `drive amend 1 --expect "not the line" "nope"`. Exit 1. Stderr contains `that entry has changed since you read it; look again`. `today` does not contain `nope`.
- **Proof.** Save each command's stdout, stderr, and exit code under `$art`, plus the note after the erase. `save_evidence correct`.

## Gotchas

- Numbers start at 1, in the order `today` prints. An index past the end exits 1. Stderr is `omalogbook: the log for DATE has COUNT entries`, with that day's date and the count on disk.
- `--expect` matches after collapsing whitespace. A line that only rewrapped still matches. Different words do not.
- The tildes start immediately after the bold clock. They include `UTC`, not only the words.
- Striking a line that is already struck exits 1. Stderr is `omalogbook: that entry is already struck`.
- Amend and strike ignore `--vault`, the same way `note` does.
