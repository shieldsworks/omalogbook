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

The markers only count on a line of their own, so prose that mentions one is
just prose.

Four rules keep your writing safe:

1. **Every save re-reads the file and merges.** An entry written by another
   process — `omalogbook note`, on watch — is never overwritten by the running
   log, and the day's totals only ever move forward.
2. **A note that isn't text is never rewritten.** If the file can't be read as
   UTF-8, omalogbook reports it and leaves it exactly as it is.
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

Local time first, then UTC when the boat isn't on Greenwich, then position in
degrees and decimal minutes, course over ground, speed over ground, and what
happened. Anything the fix didn't carry is left out rather than guessed.

Entries are written when:

- the log opens, or the fix comes back after being lost
- the boat gets under way, and when the passage ends
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
