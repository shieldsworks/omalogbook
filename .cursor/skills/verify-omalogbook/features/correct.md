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

- Launch and Doctor from `../SKILL.md` are done. `omalogbook vault` prints `$run/vault`.
- `$run/vault` is empty. No keel socket, so the note has a clock and no position.
- `art="$run/artifacts/verify/correct"; mkdir -p "$art"`.

- **File two lines.** Run `omalogbook note "Dolphins off the port side"`, then `omalogbook note "Crab boat crossed astern"`. Exit 0 both times.
- **Amend the first.** Run `omalogbook today` and copy line 1 without its number. Run `omalogbook amend 1 --expect "$line" "Dolphins off the starboard side"`. Exit 0. Stdout contains `Dolphins off the starboard side` and the same `**` clock as `$line`. A second `omalogbook today` shows the new words on line 1 and `Crab boat crossed astern` on line 2.
- **Strike the first.** Copy the amended line from `today`. Run `omalogbook strike 1 --expect "$line"`. Exit 0. Stdout contains `~~` and `Dolphins off the starboard side`. `today` still lists two entries.
- **Erase the second.** Copy line 2. Run `omalogbook strike 2 --expect "$line" --erase`. Exit 0. Stdout is the note path. `today` no longer contains `Crab boat crossed astern`. The struck dolphin line remains.
- **A stale expect.** Run `omalogbook amend 1 --expect "not the line" "nope"`. Exit 1. Stderr contains `that entry has changed since you read it; look again`. `today` does not contain `nope`.
- **Proof.** Save each command's stdout, stderr, and exit code under `$art`, plus the note after the erase. Copy `$art` to `${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/correct/` before cleanup. Observed on 2026-10-14. Amend printed a bold `07:30` and `Dolphins off the starboard side`. Strike printed `- **07:30** ~~UTC` with those words inside the tildes. Erase printed the note path. The stale `--expect` exited 1 and stderr was `omalogbook: that entry has changed since you read it; look again`. The file kept the struck line and did not contain `nope` or `Crab boat`.

## Gotchas

- Numbers start at 1, in the order `today` prints. An index past the end exits 1 and stderr contains `has N entries`.
- `--expect` matches after collapsing whitespace. A line that only rewrapped still matches. Different words do not.
- The tildes start immediately after the bold clock. They include `UTC`, not only the words.
- Striking a line that is already struck exits 1. Stderr contains `already struck`.
- Amend and strike ignore `--vault`, the same way `note` does.
