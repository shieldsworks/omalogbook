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

- Launch and Doctor from `../SKILL.md` are done. `drive vault` prints `$run/vault`.
- No keel socket under `$run/runtime`, so this drive has no fix.
- `art="$run/artifacts/verify/note"`.

Empty the vault, then take the boat's date from the same env the binary uses.

```sh
reset_note() {
  local saved=$- prev letters
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] || return 1
  [ -n "${art:-}" ] || return 1
  rm -rf "$run/vault" || return 1
  mkdir -p "$run/vault" "$art" || return 1
  env_pins_run && day=$(env $(cat "$run/env") date +%F) || return 1
}
reset_note
```

The note path is `$run/vault/${day:0:4}/${day:5:2}/$day.md`.

- **File the words.** Run `drive note "Dolphins off the port side"`. Exit 0. Stdout is the note path and nothing else. Stderr is empty.
- **Read it back.** Run `cat` on that path, then `drive today`. The file starts with `---`. It contains `Dolphins off the port side` and `<!-- omalogbook:begin -->`, and it does not contain a position in degrees. `today` prints `$day · Dash`, a line `1.` that contains the same words, and `0.0 nm · fastest 0.0 kn · under way 0 min`.
- **Mark the anchor.** Run `drive note /anchor 25 ft, 5:1`. Exit 0. The file contains `Anchor down: 25 ft, 5:1`. It does not contain `wind` or `HRRR`.
- **A word that is not a mark.** Run `drive note /madeup`. Exit 0. Stderr contains `/madeup isn't a mark`. The file contains `/madeup`.
- **A blank line.** Run `drive note`. Exit 64. Stderr contains `what should the entry say?`. The file is unchanged from the step before.
- **The flag is words.** Run `drive note --vault "$run/not-the-vault" flag check`. Exit 0. `$run/not-the-vault` does not exist. The note under `$run/vault` contains `--vault` and `flag check`.
- **List the marks.** Run `drive presets`. Exit 0. Stdout contains `/depart  left the berth; notes the sunset and starts the trip` and `/berth`.
- **Proof.** Save each command's stdout, stderr, and exit code under `$art`, plus a copy of the note. `save_evidence note`. The clock differs on a later run. Match the words.

## Gotchas

- `note` and `today` ignore `--vault`. On `note`, a `--vault` argument becomes part of the sentence. Set the vault in `$XDG_CONFIG_HOME/omalogbook/config.toml`.
- A mark asks omawind and omatide only when a fix is in hand. No socket means no wind line and no tide line. Do not invent a hub to produce them.
- `note` waits up to 600 ms when the socket exists and sends no fix. A missing socket returns at once.
- The run config sets `git = false`. A sailor's vault with `git = true` commits the note when that vault is already a git repository. This drive must not.
