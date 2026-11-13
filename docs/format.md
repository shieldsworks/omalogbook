# The log on disk

Everything omalogbook writes is plain text, in a folder you own. This document
is the whole format; nothing else is hidden anywhere.

```
Logbook/
├── 2026/
│   └── 09/
│       ├── 2026-09-17.md
│       └── 2026-09-18.md
├── tracks/
│   └── 2026-09-18-091504.gpx
├── .gitignore
└── .omalogbook.lock
```

A year and a month deep, so a circumnavigation still opens in a file manager.
Tracks are named for the local time the passage began.

## The day's note

Markdown with YAML front matter, which is what Obsidian, Neovim and every
static site generator already expect.

Omalogbook owns two things in the file:

1. **The block** between `<!-- omalogbook:begin -->` and
   `<!-- omalogbook:end -->`: the watch's entries, the day's totals, and links
   to the day's tracks.
2. **These front matter keys**: `date`, `boat`, `distance_nm`, `max_sog_kn`,
   `hours_underway`, `tracks`.

Everything else is yours. Text above, below or beside the block is copied
through exactly, and front matter omalogbook doesn't own is kept verbatim: same
lines, same order, lists and all. If the block's markers are missing or out of
order, the file is left alone and a fresh block is added at the end, so a
half-edited note is never swallowed.

The markers only count when they start a line of their own, so a marker you
quote in prose or indent in a code block is just text. If a stray begin marker
is left in the file, the block is taken as the last begin before the end.

Four rules keep your writing safe:

1. **Every save re-reads the file and merges.** An entry written by another
   process — `omalogbook note`, on watch — is never overwritten by the running
   log. The day's totals move forward on their own, but a total you correct by
   hand stands: omalogbook remembers what it last wrote, and anything else on
   disk is yours.
2. **A note that isn't text is never rewritten.** If the file can't be read as
   UTF-8, omalogbook reports it and leaves it exactly as it is. Move it aside
   and the log picks up again at the next entry.
3. **Write, then rename.** The note is written to a temporary file and renamed
   into place, so it is either the old one or the new one, never half of
   either.
4. **One writer at a time**, through a lock file in the vault. The lock is
   added to the vault's `.gitignore`, since it belongs to this machine and not
   to the log's history.

### An entry

```
- **09:15** (16:15 UTC) · 37°52.0′N 122°18.9′W · 245° · 5.1 kn — under way
```

Days turn over on this machine's clock, never the receiver's. A step back of a
single day is treated as skew and ignored, so a receiver a little out of step
can't flip the log between two days around midnight; a bigger step back is a
clock that was wrong, and is followed. Near midnight, an entry is stamped with
the day it is filed under, not the receiver's.

Local time first, then UTC when the boat isn't on Greenwich, then position in
degrees and decimal minutes, course over ground, speed over ground, and what
happened. Anything the fix didn't carry is left out rather than guessed.

Entries are written when:

- the log opens, or the fix comes back after being lost
- the boat gets under way, and when the passage ends
- a trip opens (`departed; track started`), is picked up again after a
  restart (`track resumed`), or closes (`berthed; the track is
  closed`), as read from the marks
- every `every_minutes` while under way
- you write one with `omalogbook note`
- the receiver's clock disagrees with this machine's, once
- omakeel speaks a protocol this version doesn't know, once
- a passage is closed because the fix has been gone for `stop_after_minutes`

An entry is always one line: a line break would end the markdown list item, so
newlines in a note become spaces.

### The day's totals

`distance_nm` adds up the distance between fixes while under way, so it is the
distance actually sailed, not the straight line from berth to anchorage. A gap
longer than five minutes is not counted: that is the log having been stopped,
not the boat having teleported.

The totals reach the disk at least every five minutes while under way, so a
power cut costs minutes rather than the day.

A passage ends when the boat has been below `stopped_kn` for
`stop_after_minutes`. Only making way again — `underway_kn` or more — restarts
that countdown, so a boat swinging at anchor doesn't hold a passage open all
night.

A trip is a `Departed` mark with no `Berthed` after it. `omalogbook run` reads
the notes for one every ten seconds, and each look takes the trip from what the
notes say: whether the latest `Departed` has a later `Berthed`, and which berth
that is. While a trip is open the passage starts at the next fix whatever the
speed, and neither stillness nor a lost fix ends it. When the notes show a
closed trip, a passage still recording — started by the trip or by speed — is
closed, and the day's note links it when it began between those two marks.
After a `Berthed`, a speed-based passage does not start again until the boat
has been seen at or below `stopped_kn`, or until a new `Departed`: motoring
into the slip after the mark is still the arrival, not a new passage. A missing
speed is not stillness, and does not lift that hold. Once the boat has been
still, a speed-based passage is not closed again by the same berth. The hold is for that
berth; a later `Departed` and `Berthed` arm it again, even if both marks arrive
between looks. A month of notes with no mark at all closes an open trip too. A mark added to or struck from an
earlier day's note is picked up within ten minutes; today's note is watched for
changes. Marks in one minute are ordered as
the note lists them. Two `Berthed` marks in the same minute cannot be told
apart, because the notes keep only the minute. A note that can't be read, or a note with no entries at
all, changes nothing, and an unreadable note is tried again at the next
ten-second look even if its size and time have not changed. With no speed from the receiver, a trip's speed is worked
out over at least the last 30 seconds of fixes; outside a trip a missing speed
is no speed, and a worked-out speed is never the day's top speed. Fixes below `stopped_kn` during a trip add no distance or time under
way. A `Departed` while a passage is
already open keeps that passage and its track. A recorder started while a trip
is open, after a reboot, starts a new track file at its first fix; the trip
reads every track between its two marks, so the total still covers it.

## The track

GPX 1.1, one `<trk>` per passage, one `<trkpt>` every `point_seconds` with its
UTC time, and speed in meters per second in the standard extension slot. Track
files are named for the local time the passage began, to the second, so two
passages can't land on the same name.

The file is rewritten in full as the passage goes on, so what's on disk is
always valid XML, even if the power goes.

## Git

If the vault is a git repository, omalogbook commits when a passage ends, at
midnight, and when it stops, then pushes if a remote answers. It never rebases,
never force-pushes, and treats a failed push as weather rather than an error.

If the vault is not a repository, nothing changes: the log is still files.
