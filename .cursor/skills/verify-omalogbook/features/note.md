# Write a note

`omalogbook note` files one line in today's note. The line carries the clock. It carries the position only when omakeel has a fix. With no hub, the words still file.

## Sub-features

- `note-words` files a sentence with the time and no position.
- `note-mark` files `/anchor` as `Anchor down`, with no wind when there is no fix.
- `note-unknown` files a slash word that is not a mark, and says so on stderr.
- `note-empty` refuses a blank line and writes nothing.
- `note-flag` treats `--vault` on `note` as words. The file still lands in the config vault.
- `note-presets` lists the marks.

## How to get to it (user POV)

- Run `omalogbook note "Dolphins off the port side"` in a terminal.
- Run `omalogbook note /anchor 25 ft, 5:1` to mark the anchor.
- Run `omalogbook presets` to read the marks before typing one.
- In the window, type the words and press Enter. That runs the same `note` command. See `window.md`.

## Driving it with the isolated vault

Preconditions:

- Run it with the drive in `../SKILL.md`. `vault` prints `$run/vault`.
- No keel socket under `$run/runtime`, so this drive has no fix.

The note path is `$run/vault/${day}/` with `day` from `TZ=UTC date +%F`, laid out as `YYYY/MM/YYYY-MM-DD.md`.

- **File the words.** `omalogbook note "Dolphins off the port side"`. Exit 0. Stdout is the note path and nothing else. Stderr is empty.
- **Read it back.** The file starts with `---`. It contains `Dolphins off the port side` and `<!-- omalogbook:begin -->`, and it does not contain a position in degrees. `omalogbook today` prints `$day · Dash`, a line `1.` that contains the same words, and `0.0 nm · fastest 0.0 kn · under way 0 min`.
- **Mark the anchor.** `omalogbook note /anchor 25 ft, 5:1`. Exit 0. The file contains `Anchor down: 25 ft, 5:1`. It does not contain `wind` or `HRRR`.
- **A word that is not a mark.** `omalogbook note /madeup`. Exit 0. Stderr contains `/madeup isn't a mark`. The file contains `/madeup`.
- **A blank line.** `omalogbook note`. Exit 64. Stderr contains `what should the entry say?`. The file is unchanged from the step before.
- **The flag is words.** `omalogbook note --vault "$run/not-the-vault" flag check`. Exit 0. `$run/not-the-vault` does not exist. The note under `$run/vault` contains `--vault` and `flag check`.
- **List the marks.** `omalogbook presets`. Exit 0. Stdout contains `/depart  left the berth; notes the sunset and starts the trip` and `/berth`.
- **Proof.** `record` saves each command's stdout, stderr, and exit code under `verify/note-*/`, plus a copy of the note. The clock differs on a later run. Match the words.

```sh
env -u BASH_ENV bash --noprofile --norc .cursor/skills/verify-omalogbook/drive.sh note
```

## Gotchas

- `note` and `today` ignore `--vault`. On `note`, a `--vault` argument becomes part of the sentence. Set the vault in the config file. A `--vault` path that canonicalizes outside the run directory never starts the binary, including on `note`.
- A mark asks omawind and omatide only when a fix is in hand. No socket means no wind line and no tide line. Do not invent a hub to produce them.
- `note` waits up to 600 ms when the socket exists and sends no fix. A missing socket returns at once.
- The run config sets `git = false`. A sailor's vault with `git = true` commits the note when that vault is already a git repository. This drive must not.
