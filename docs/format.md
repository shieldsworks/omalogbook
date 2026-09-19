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
│   └── 2026-09-18-0915.gpx
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
through exactly. Front matter keys omalogbook doesn't own are kept, in the
order you wrote them. If the block's markers are missing or out of order, the
file is left alone and a fresh block is added at the end, so a half-edited note
is never swallowed.

The note is written to a temporary file and renamed into place, so a note is
either the old one or the new one, never half of either. A lock file in the
vault keeps `omalogbook note` and the running log from writing at the same
moment.

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

### The day's totals

`distance_nm` adds up the distance between fixes while under way, so it is the
distance actually sailed, not the straight line from berth to anchorage. A gap
longer than five minutes is not counted: that is the log having been stopped,
not the boat having teleported.

## The track

GPX 1.1, one `<trk>` per passage, one `<trkpt>` every `point_seconds` with its
UTC time, and speed in meters per second in the standard extension slot. It is
rewritten in full as the passage goes on, so the file on disk is always valid
XML even if the power goes.

## Git

If the vault is a git repository, omalogbook commits when a passage ends, at
midnight, and when it stops, then pushes if a remote answers. It never rebases,
never force-pushes, and treats a failed push as weather rather than an error.

If the vault is not a repository, nothing changes: the log is still files.
