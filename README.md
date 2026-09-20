# Omalogbook

The ship's log for [Omahoy](https://github.com/shieldsworks/omahoy).

**Status: early.** It writes the log from a real GPS through
[omakeel](https://github.com/shieldsworks/omakeel). It has not yet kept a log at
sea for a season.

Omalogbook writes plain markdown, one note per day, with a GPX track for every
passage. It owns one block in each note and the day's numbers in the front
matter; everything else in the file is yours and is never touched. Nothing here
needs a database, a server, or omalogbook itself to read it again — the log is
a folder of text you can open in any editor, and a git repo you own.

```markdown
---
date: 2026-09-18
boat: Dash
distance_nm: 12.4
max_sog_kn: 6.1
hours_underway: 3.20
tracks: ["tracks/2026-09-18-091504.gpx"]
---

# 2026-09-18 · Dash

Reefed off Angel Island, wind up to 22. Crab boat crossed close astern.

<!-- omalogbook:begin -->
<!-- Written by omalogbook. Your own notes belong outside this block. -->

## The watch

- **09:15** (16:15 UTC) — log opened · 37°52.0′N 122°18.9′W
- **09:15** (16:15 UTC) · 37°52.0′N 122°18.9′W · 245° · 5.1 kn — under way
- **10:15** (17:15 UTC) · 37°49.2′N 122°26.4′W · 262° · 6.1 kn
- **12:27** (19:27 UTC) · 37°51.4′N 122°28.8′W — stopped

**Day's run** 12.4 nm · **fastest** 6.1 kn · **under way** 3 h 12 min

**Track** [tracks/2026-09-18-091504.gpx](tracks/2026-09-18-091504.gpx)

<!-- omalogbook:end -->
```

## What it does

- **Keeps the watch.** A passage starts when the boat has been making way and
  ends when it has been still for a few minutes. In between it logs a position
  every hour, and the day's run, fastest speed and time under way.
- **Records the track.** Every passage becomes a GPX file, the format every
  chartplotter and mapping site already reads.
- **Takes your own entries, where you wrote them.** `omalogbook note "Dolphins
  off the port side"` from any terminal, on watch, without opening an editor.
  The entry carries the time and the boat's position:

  ```markdown
  - **14:32** (21:32 UTC) · 37°52.0′N 122°18.9′W — Dolphins off the port side
  ```

  Course and speed are left off, so a note reads as your words rather than as
  another instrument line. With no hub or no fix the note is filed with its
  time alone — the words are the point, and they are never held up waiting for
  a position.
- **Marks the moments.** A line that opens with a mark — `/depart`,
  `/anchor 25 ft, 5:1`, `/reef` — is filed as an event, with the course and
  speed you were making and what it was blowing:

  ```markdown
  - **14:32** (21:32 UTC) · 37°52.0′N 122°18.9′W · 245° · 5.1 kn — Departed
    · wind 13 kn from 262°, gusting 18 (HRRR) · 1014.6 hPa
    · measured 11 kn from 250° (Alameda, 12 min)
  ```

  The two wind readings are different things and the log never blurs them.
  `wind … (HRRR)` is [omawind](https://github.com/shieldsworks/omawind)'s
  forecast worked out at the boat — a model's opinion, and the only source of
  a barometer reading. `measured …` is an anemometer that really measured
  that wind, at a named NDBC station within 10 nm. With omawind down, or the
  forecast pinned somewhere other than the boat, a mark files without them.

  The marks are `depart`, `sail`, `motor`, `reef`, `shake`, `anchor`,
  `aweigh`, `moor`, `berth` and `watch`; `omalogbook presets` lists them, and
  the words you'd reach for (`/mooring`, `/returned`, `/docked`) find the
  right one. A mark says what the log can't work out for itself: `under way`
  and `stopped` come from a speed threshold, which can't tell anchored from
  moored from tied up alongside.

  A slash word that isn't a mark is filed as an ordinary note and says so.
  Losing what someone wrote on watch would be worse than filing it plainly.
- **Says where the day stands.** `omalogbook today` prints the day's entries
  and totals; `omalogbook today --json` is the same for a window to read.
- **Leaves your writing alone.** Your prose, your headings and any front matter
  you add are copied through untouched, and every save merges with what's on
  disk, so an entry you write while the log is running is never overwritten. A
  note that isn't text is never rewritten at all, and a total you correct by
  hand stands.
- **Commits as it goes**, and pushes when there is a connection. Offshore there
  won't be one for weeks, which is not an error.

## Install

```sh
cargo install --path .           # or: cargo build --release
omalogbook run                   # follows omakeel, writes the log
omalogbook today                 # the day so far
omalogbook presets               # the marks
```

Settings live in `~/.config/omalogbook/config.toml`. Every one is optional:

```toml
vault = "~/Logbook"       # the folder of notes; a git repo if you make one
boat = "Dash"
every_minutes = 60        # how often to log a position while under way
underway_kn = 1.0         # speed over ground that starts a passage
stopped_kn = 0.5          # and what counts as still
stop_after_minutes = 5    # for how long, before the passage has ended
point_seconds = 10        # seconds between track points
git = true                # commit the vault as the log is written
```

To keep the log going with the laptop closed, run it beside omakeel as a user
service, or start it from your session. It writes nothing until omakeel has a
position.

## The window

```sh
./run.sh                         # from this checkout
```

A line to write in, the day's entries under it, and the day's run at the
foot. The field holds the focus, so a note on watch is the words and Enter —
nothing to find with a tiller under your arm. Escape clears the line, and
clears the window when the line is already empty. `n`, or the button, is
Night Watch: red on black, this window's own. A line that opens with `/`
lists the marks it could become, and Tab takes the first.

It reads the day through `omalogbook today --json` and writes with
`omalogbook note`, so the window needs no socket of its own and the position
comes from the same place a note from any terminal gets one. The note on
disk is watched, so an entry the running log writes — or one you type in an
editor — shows without a refresh.

In the Omarchy shell it is the plugin's panel (`manifest.json`, id
`org.omahoy.logbook`); standalone it is its own Quickshell process. Either
way it finds omalogbook next to the checkout, falling back to `omalogbook` on
PATH; `OMALOGBOOK_BIN` names one directly.

The window only shows the log. Keeping the watch — the hourly entries, the
day's run, the GPX — is `omalogbook run`, and it is worth having both.

## In an editor

The vault is an ordinary folder of markdown, so Obsidian opens it as a vault
and Neovim opens it as files. `omalogbook path` prints today's note, which is
all a keybinding needs:

```lua
-- Neovim: <leader>ol opens today's log
vim.keymap.set("n", "<leader>ol", function()
  vim.cmd.edit(vim.fn.system("omalogbook path"):gsub("%s+$", ""))
end, { desc = "Today's log" })
```

`scripts/omalogbook-open` does the same from a desktop launcher, in whatever
terminal `$TERMINAL` names.

## The clocks

Entries lead with the boat's local time, and add UTC when the boat is not on
Greenwich. Days turn over at local midnight, because that is the day the crew
lived.

Times come from the receiver while its clock agrees with this machine's, and
from this machine when they disagree by more than an hour — a receiver with the
wrong week, or a recording being replayed, would otherwise file entries days
away from the day they were written. When that happens the log says so, once.

## Privacy

A log is a record of where the boat actually was, at what time, night by night.
Keep your own vault private, and think before you publish a track.

## What it doesn't do yet

- Routes, which [omahelm](https://github.com/shieldsworks/omahelm) does not have
  yet. Tracks are recorded now; routes will be logged once there are any.
- Weather beyond wind and pressure. A mark carries both, from omawind; air
  and water temperature, sky, visibility and sea state wait until the boat
  has instruments to read them from. Ordinary notes carry no weather at all —
  they are your words, and an instrument reading stapled to them would read
  as the machine talking over the top.
- Engine hours, fuel and tanks, which belong to omabosun.
- Anchoring: an anchor watch will come from omanchor rather than be guessed at
  from speed.

## License

MIT
