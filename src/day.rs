//! A day's note: plain markdown with YAML front matter, the shape Obsidian
//! and every other markdown editor already understand.
//!
//! Omalogbook owns exactly one block, between two marker lines, and six keys
//! in the front matter. Everything else in the file is the crew's.
//!
//! Two rules keep it that way. Every save re-reads the file and merges, so an
//! entry written by another process (`omalogbook note`, on watch) is never
//! overwritten by the running log. And a file that cannot be read as text is
//! never rewritten at all.

use crate::{geo, time};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const BEGIN: &str = "<!-- omalogbook:begin -->";
pub const END: &str = "<!-- omalogbook:end -->";
const BANNER: &str = "<!-- Written by omalogbook. Your own notes belong outside this block. -->";
/// The front matter keys omalogbook owns; every other key is the crew's.
const OURS: [&str; 6] = [
    "date",
    "boat",
    "distance_nm",
    "max_sog_kn",
    "hours_underway",
    "tracks",
];

/// The day's note, held open across a watch.
#[derive(Debug)]
pub struct Day {
    pub date: String,
    pub path: PathBuf,
    boat: String,
    /// Entries added since the last save, waiting to be merged into the file.
    pending: Vec<String>,
    /// The file already had entries when it was opened or last saved.
    had_entries: bool,
    pub distance_nm: f64,
    pub max_sog_kn: f64,
    pub underway_secs: i64,
    /// Vault-relative track files to link.
    tracks: Vec<String>,
    /// False when the file on disk isn't text: then it is left alone.
    readable: bool,
    /// The totals as omalogbook last wrote them: distance, fastest, seconds.
    /// A value on disk that differs from these was changed by the crew.
    written: (f64, f64, i64),
}

/// The day's total, given what is on disk and what was last written there.
/// Anything else on disk is the crew's own correction, and it stands.
fn merge(mine: f64, written: f64, disk: &str) -> f64 {
    // A value that isn't a number isn't a correction; keep what we have
    // rather than reading it as zero.
    let Ok(disk_value) = disk.parse::<f64>() else {
        return mine;
    };
    if (disk_value - written).abs() > f64::EPSILON {
        disk_value
    } else {
        mine.max(disk_value)
    }
}

/// `<vault>/2026/09/2026-09-18.md`, a year and month deep so a long voyage
/// stays navigable in a file manager.
pub fn path_for(vault: &Path, date: &str) -> PathBuf {
    match (date.get(..4), date.get(5..7)) {
        (Some(year), Some(month)) if time::valid_date(date) => {
            vault.join(year).join(month).join(format!("{date}.md"))
        }
        // Never build a path out of a date that isn't one.
        _ => vault.join(format!("{date}.md")),
    }
}

impl Day {
    /// Open the day, continuing the entries and totals already written.
    pub fn open(vault: &Path, date: &str, boat: &str) -> io::Result<Day> {
        let path = path_for(vault, date);
        let mut day = Day {
            date: date.to_string(),
            path: path.clone(),
            boat: boat.to_string(),
            pending: Vec::new(),
            had_entries: false,
            distance_nm: 0.0,
            max_sog_kn: 0.0,
            underway_secs: 0,
            tracks: Vec::new(),
            readable: true,
            written: (0.0, 0.0, 0),
        };
        match read(&path) {
            Ok(Some(text)) => {
                day.had_entries = !entries_in(&text).is_empty();
                day.merge_totals(&text);
                // What is on disk is what was last written, so the first save
                // adds to it instead of being read as a correction.
                day.remember();
            }
            Ok(None) => {}
            Err(Unreadable) => day.readable = false,
        }
        Ok(day)
    }

    /// The day's GPX tracks, vault-relative, in the order they were sailed.
    pub fn tracks(&self) -> &[String] {
        &self.tracks
    }

    /// The note exists but isn't text, so omalogbook will not touch it.
    pub fn unreadable(&self) -> bool {
        !self.readable
    }

    /// Merge the file's totals with this session's. The larger wins: another
    /// process only ever writes back what it read, so the running log's newer
    /// numbers are never rolled back by a note written on watch.
    fn merge_totals(&mut self, text: &str) {
        for (key, value) in front_pairs(text) {
            match key.as_str() {
                "distance_nm" => self.distance_nm = merge(self.distance_nm, self.written.0, &value),
                "max_sog_kn" => self.max_sog_kn = merge(self.max_sog_kn, self.written.1, &value),
                "hours_underway" => {
                    let merged = merge(
                        self.underway_secs as f64 / 3600.0,
                        self.written.2 as f64 / 3600.0,
                        &value,
                    );
                    self.underway_secs = (merged * 3600.0).round() as i64;
                }
                "tracks" => {
                    for t in value.trim_matches(['[', ']']).split(',') {
                        let t = t.trim().trim_matches(['"', '\'']);
                        if !t.is_empty() && !self.tracks.iter().any(|k| k == t) {
                            self.tracks.push(t.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Add an entry, already rendered by [`crate::entry`]. A line break would
    /// end the list item, so an entry is always one line.
    pub fn push(&mut self, line: String) {
        self.pending.push(line.replace(['\n', '\r'], " "));
    }

    /// Link a track file, once.
    pub fn add_track(&mut self, relative: &str) {
        if !self.tracks.iter().any(|t| t == relative) {
            self.tracks.push(relative.to_string());
        }
    }

    /// Nothing has ever been logged for this day.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && !self.had_entries
    }

    /// Merge into the note and write it, keeping every line the crew wrote.
    /// The file is replaced only once it is complete on disk.
    pub fn save(&mut self) -> io::Result<()> {
        if !self.readable {
            // It may have been moved aside since; look once more.
            match read(&self.path) {
                Ok(_) => self.readable = true,
                Err(Unreadable) => return Err(self.left_alone()),
            }
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let existing = match read(&self.path) {
            Ok(text) => text.unwrap_or_default(),
            Err(Unreadable) => {
                self.readable = false;
                return Err(self.left_alone());
            }
        };
        // Whatever is on disk wins for order; this session's entries follow.
        let mut entries = entries_in(&existing);
        let had = !entries.is_empty();
        entries.extend(self.pending.iter().cloned());
        self.merge_totals(&existing);

        let text = self.render(&existing, &entries);
        let temp = self.path.with_extension("md.tmp");
        {
            let mut file = fs::File::create(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&temp, &self.path)?;
        self.had_entries = had || !self.pending.is_empty();
        self.pending.clear();
        self.remember();
        Ok(())
    }

    /// Change one entry of the day as it stands on disk.
    ///
    /// The clock and the fix are never rewritten. A log is a record of where
    /// the boat was and when, and omalogbook will not put different numbers
    /// there — amending replaces the words, and the file is yours to edit by
    /// hand if you truly mean to change the rest.
    pub fn revise(&mut self, index: usize, expect: &str, how: &Revision) -> io::Result<String> {
        let existing = match read(&self.path) {
            Ok(Some(text)) => text,
            Ok(None) => return Err(io::Error::other(format!("no log for {}", self.date))),
            Err(Unreadable) => {
                self.readable = false;
                return Err(self.left_alone());
            }
        };
        let mut entries = entries_in(&existing);
        let Some(line) = entries.get(index).cloned() else {
            return Err(io::Error::other(format!(
                "the log for {} has {} entries",
                self.date,
                entries.len()
            )));
        };
        // The day may have moved on since the caller read it: the running log
        // writes an entry an hour, and every one of them shifts what `index`
        // means. Change the entry the caller meant or change nothing.
        if !expect.is_empty() && collapse(expect) != collapse(&line) {
            return Err(io::Error::other(
                "that entry has changed since you read it; look again".to_string(),
            ));
        }
        let changed = match how {
            Revision::Amend(words) => amend(&line, words),
            Revision::Strike => {
                if struck(&line) {
                    return Err(io::Error::other("that entry is already struck".to_string()));
                }
                strike(&line)
            }
            Revision::Erase => String::new(),
        };
        match how {
            Revision::Erase => {
                entries.remove(index);
            }
            _ => entries[index] = changed.clone(),
        }
        self.merge_totals(&existing);
        let text = self.render(&existing, &entries);
        let temp = self.path.with_extension("md.tmp");
        {
            let mut file = fs::File::create(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&temp, &self.path)?;
        self.had_entries = !entries.is_empty();
        self.remember();
        Ok(changed)
    }

    /// Note the totals as the file now has them, rounded as they are written,
    /// so reading them back matches to the bit.
    fn remember(&mut self) {
        self.written = (
            format!("{:.1}", self.distance_nm).parse().unwrap_or(0.0),
            format!("{:.1}", self.max_sog_kn).parse().unwrap_or(0.0),
            (format!("{:.2}", self.underway_secs as f64 / 3600.0)
                .parse::<f64>()
                .unwrap_or(0.0)
                * 3600.0)
                .round() as i64,
        );
    }

    fn left_alone(&self) -> io::Error {
        io::Error::other(format!(
            "{} could not be read as text; leaving it alone",
            self.path.display()
        ))
    }

    fn render(&self, existing: &str, entries: &[String]) -> String {
        let block = self.block(entries);
        if existing.trim().is_empty() {
            return format!(
                "{}\n# {} · {}\n\n{block}\n",
                self.front(&[]),
                self.date,
                self.boat
            );
        }
        let (front, body) = split_front(existing);
        let body = match block_bounds(body) {
            Some((start, end)) => format!("{}{block}{}", &body[..start], &body[end..]),
            // No block, or a half-written one: add a fresh block at the end
            // and leave whatever is there alone.
            None => format!("{}\n\n{block}\n", body.trim_end()),
        };
        format!("{}{}", self.front(&front), body)
    }

    /// The day's numbers, keeping every other front matter line as it was:
    /// same text, same order, lists and all.
    fn front(&self, existing: &[String]) -> String {
        let mine = [
            ("date", self.date.clone()),
            ("boat", self.boat.clone()),
            ("distance_nm", format!("{:.1}", self.distance_nm)),
            ("max_sog_kn", format!("{:.1}", self.max_sog_kn)),
            (
                "hours_underway",
                format!("{:.2}", self.underway_secs as f64 / 3600.0),
            ),
            (
                "tracks",
                format!(
                    "[{}]",
                    self.tracks
                        .iter()
                        .map(|t| format!("\"{t}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ),
        ];
        let mut out = String::from("---\n");
        let mut written = [false; OURS.len()];
        for line in existing {
            match key_of(line) {
                Some(key) if OURS.contains(&key.as_str()) => {
                    let i = OURS.iter().position(|k| *k == key).unwrap_or_default();
                    if !written[i] {
                        out.push_str(&format!("{}: {}\n", mine[i].0, mine[i].1));
                        written[i] = true;
                    }
                }
                _ => {
                    out.push_str(line.trim_end_matches('\r'));
                    out.push('\n');
                }
            }
        }
        for (i, (key, value)) in mine.iter().enumerate() {
            if !written[i] {
                out.push_str(&format!("{key}: {value}\n"));
            }
        }
        out.push_str("---\n");
        out
    }

    fn block(&self, entries: &[String]) -> String {
        let mut out = format!("{BEGIN}\n{BANNER}\n\n## The watch\n\n");
        if entries.is_empty() {
            out.push_str("_Nothing logged yet._\n");
        } else {
            for line in entries {
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push_str(&format!(
            "\n**Day's run** {:.1} nm · **fastest** {:.1} kn · **under way** {}\n",
            self.distance_nm,
            self.max_sog_kn,
            hours(self.underway_secs)
        ));
        for track in &self.tracks {
            out.push_str(&format!("\n**Track** [{track}]({track})\n"));
        }
        out.push_str(&format!("\n{END}"));
        out
    }
}

/// What to do to an entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Revision {
    /// Replace the words, keeping the clock and the fix as they were.
    Amend(String),
    /// Rule a line through it. It happened, and it was withdrawn — which is
    /// what a paper log does, and worth more later than a line that was
    /// quietly never there.
    Strike,
    /// Take it out altogether.
    Erase,
}

/// Whitespace-insensitive, so a line that only got re-wrapped still matches.
fn collapse(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Everything after the clock: what a strike rules through, and what the
/// entry actually says. `- **14:32** (21:32 UTC) · 37°…` splits after the
/// bold clock, so the time an entry was made always stays legible.
fn body_of(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("- **")?;
    let close = rest.find("**")?;
    Some(line.split_at(4 + close + 2))
}

fn struck(line: &str) -> bool {
    body_of(line).is_some_and(|(_, body)| body.trim_start().starts_with("~~"))
}

fn strike(line: &str) -> String {
    match body_of(line) {
        Some((clock, body)) if !body.trim().is_empty() => {
            format!("{clock} ~~{}~~", body.trim())
        }
        _ => format!("~~{}~~", line.trim_start_matches("- ")),
    }
}

/// The crew's words are what follows the em dash. An entry that has none —
/// a bare position line — gains them.
fn amend(line: &str, words: &str) -> String {
    let words = words.trim();
    match line.find(" — ") {
        Some(at) if !words.is_empty() => format!("{}{words}", &line[..at + " — ".len()]),
        Some(at) => line[..at].trim_end().to_string(),
        None if !words.is_empty() => format!("{} — {words}", line.trim_end()),
        None => line.to_string(),
    }
}

/// The note is there but isn't text: a binary file, or another program's.
struct Unreadable;

fn read(path: &Path) -> Result<Option<String>, Unreadable> {
    match fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|_| Unreadable),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        // A note that can't be read is not a note that should be replaced.
        Err(_) => Err(Unreadable),
    }
}

/// `3 h 12 min`, or minutes alone for a short hop.
pub fn hours(secs: i64) -> String {
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    if h > 0 {
        format!("{h} h {m:02} min")
    } else {
        format!("{m} min")
    }
}

/// Where the block starts and ends in the body. The markers count only on a
/// line of their own, so prose that mentions one is just prose.
fn block_bounds(body: &str) -> Option<(usize, usize)> {
    let mut start = None;
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        // trim_end only: an indented marker is quoted prose or a code block.
        let marker = line.trim_end();
        if marker == BEGIN {
            // The last begin before the end wins, so a stray one left in the
            // file takes prose with it no more than once.
            start = Some(offset);
        } else if marker == END
            && let Some(s) = start
        {
            return Some((s, offset + line.len() - trailing_newline(line)));
        }
        offset += line.len();
    }
    None
}

fn trailing_newline(line: &str) -> usize {
    line.len() - line.trim_end_matches(['\n', '\r']).len()
}

/// The entry lines inside the block, in file order.
pub fn entries_in(text: &str) -> Vec<String> {
    let (_, body) = split_front(text);
    let Some((start, end)) = block_bounds(body) else {
        return Vec::new();
    };
    body[start..end]
        .lines()
        .filter(|l| l.starts_with("- "))
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect()
}

/// The front matter's own lines, and the rest of the file. Tolerates CRLF and
/// a file that is front matter and nothing else.
fn split_front(text: &str) -> (Vec<String>, &str) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = text.split_inclusive('\n').next().unwrap_or("");
    if first.trim() != "---" {
        return (Vec::new(), text);
    }
    let mut lines = Vec::new();
    let mut offset = first.len();
    for line in text[first.len()..].split_inclusive('\n') {
        if line.trim() == "---" {
            return (lines, &text[offset + line.len()..]);
        }
        lines.push(line.trim_end_matches(['\n', '\r']).to_string());
        offset += line.len();
    }
    // An unterminated front matter block is not front matter.
    (Vec::new(), text)
}

/// `key: value` pairs from the front matter, for the keys omalogbook owns.
fn front_pairs(text: &str) -> Vec<(String, String)> {
    split_front(text)
        .0
        .iter()
        .filter_map(|line| Some((key_of(line)?, line.split_once(':')?.1.trim().to_string())))
        .collect()
}

/// The key a front matter line sets, if it sets one. An indented line is part
/// of the value above it, not a key.
fn key_of(line: &str) -> Option<String> {
    if line.starts_with([' ', '\t', '-', '#']) {
        return None;
    }
    let (key, _) = line.split_once(':')?;
    let key = key.trim();
    (!key.is_empty() && !key.contains(' ')).then(|| key.to_string())
}

/// The day the log is on now, in the boat's own time zone.
pub fn today() -> String {
    time::local(time::now()).date()
}

/// A one-line position, shared by entries and the note's heading.
pub fn position(lat: f64, lon: f64) -> String {
    format!("{} {}", geo::latitude(lat), geo::longitude(lon))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(dir: &Path) -> Day {
        let mut d = Day::open(dir, "2026-09-18", "Dash").unwrap();
        d.push("- **09:15** · 37°52.0′N 122°18.9′W · under way".into());
        d.distance_nm = 12.4;
        d.max_sog_kn = 6.1;
        d.underway_secs = 11_520;
        d
    }

    fn note(dir: &Path) -> String {
        fs::read_to_string(path_for(dir, "2026-09-18")).unwrap()
    }

    #[test]
    fn writes_a_note_that_opens_as_markdown() {
        let dir = tempdir("markdown");
        day(&dir).save().unwrap();
        let text = note(&dir);
        assert!(text.starts_with("---\ndate: 2026-09-18\nboat: Dash\n"));
        assert!(text.contains("distance_nm: 12.4"));
        assert!(text.contains("hours_underway: 3.20"));
        assert!(text.contains("- **09:15**"));
        assert!(text.contains("**Day's run** 12.4 nm"));
        assert!(text.contains("**under way** 3 h 12 min"));
    }

    #[test]
    fn an_entry_written_by_another_process_is_kept() {
        let dir = tempdir("merge");
        let mut watch = day(&dir);
        watch.save().unwrap();

        // The crew, in another terminal, on watch.
        let mut crew = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        crew.push("- **10:00** — Reefed. Wind up to 22.".into());
        crew.save().unwrap();

        // The running log writes its next entry.
        watch.push("- **11:00** · under way".into());
        watch.save().unwrap();

        let text = note(&dir);
        assert!(
            text.contains("Reefed. Wind up to 22."),
            "crew entry lost:\n{text}"
        );
        assert!(text.contains("- **09:15**"));
        assert!(text.contains("- **11:00**"));
        assert_eq!(text.matches("- **").count(), 3, "{text}");
    }

    #[test]
    fn a_note_that_is_not_text_is_left_alone() {
        let dir = tempdir("binary");
        let path = path_for(&dir, "2026-09-18");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let junk = b"log \xff\xfe not text".to_vec();
        fs::write(&path, &junk).unwrap();

        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        assert!(d.unreadable());
        d.push("- **10:00** · under way".into());
        assert!(d.save().is_err());
        assert_eq!(fs::read(&path).unwrap(), junk, "the file was rewritten");
    }

    #[test]
    fn prose_that_mentions_the_marker_is_only_prose() {
        let dir = tempdir("marker");
        day(&dir).save().unwrap();
        let path = path_for(&dir, "2026-09-18");
        let text = note(&dir).replace(
            "# 2026-09-18 · Dash",
            &format!(
                "# 2026-09-18 · Dash\n\nOmalogbook writes a {BEGIN} marker. Below it I keep my own notes.\n\nSecond paragraph."
            ),
        );
        fs::write(&path, text).unwrap();

        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        d.push("- **12:00** · stopped".into());
        d.save().unwrap();

        let text = note(&dir);
        assert!(text.contains("Below it I keep my own notes."), "{text}");
        assert!(text.contains("Second paragraph."), "prose lost:\n{text}");
        assert!(text.contains("- **09:15**"), "entries lost:\n{text}");
        assert!(text.contains("- **12:00**"));
    }

    #[test]
    fn keeps_front_matter_the_crew_added_exactly() {
        let dir = tempdir("front");
        day(&dir).save().unwrap();
        let path = path_for(&dir, "2026-09-18");
        fs::write(
            &path,
            note(&dir).replacen(
                "---\n",
                "---\ncrew: Casey\ntags:\n  - sailing\n  - bay\n",
                1,
            ),
        )
        .unwrap();

        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        d.push("- **12:00** · stopped".into());
        d.save().unwrap();

        let text = note(&dir);
        assert!(text.contains("crew: Casey"), "{text}");
        assert!(
            text.contains("tags:\n  - sailing\n  - bay\n"),
            "a list was flattened:\n{text}"
        );
        assert!(
            text.starts_with("---\ncrew: Casey\n"),
            "order changed:\n{text}"
        );
        assert_eq!(text.matches("\n---").count(), 1, "{text}");
        assert!(text.contains("distance_nm: 12.4"));
    }

    #[test]
    fn a_note_with_windows_line_endings_keeps_its_totals() {
        let dir = tempdir("crlf");
        day(&dir).save().unwrap();
        let path = path_for(&dir, "2026-09-18");
        fs::write(&path, note(&dir).replace('\n', "\r\n")).unwrap();

        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        assert_eq!(d.distance_nm, 12.4);
        d.push("- **12:00** · stopped".into());
        d.save().unwrap();
        let text = note(&dir);
        assert_eq!(
            text.matches("\n---").count(),
            1,
            "duplicate front matter:\n{text}"
        );
        assert!(text.contains("distance_nm: 12.4"), "{text}");
        assert!(text.contains("- **09:15**"), "entries lost:\n{text}");
    }

    #[test]
    fn the_note_does_not_grow_blank_lines() {
        let dir = tempdir("blanks");
        let mut d = day(&dir);
        d.save().unwrap();
        let first = note(&dir);
        for _ in 0..6 {
            d.save().unwrap();
        }
        assert_eq!(note(&dir), first, "the note changed without an entry");
        assert!(!note(&dir).contains("\n\n\n"), "{}", note(&dir));
    }

    #[test]
    fn a_marker_inside_a_code_block_is_only_prose() {
        let dir = tempdir("fenced");
        day(&dir).save().unwrap();
        let path = path_for(&dir, "2026-09-18");
        let quoted = format!(
            "# 2026-09-18 · Dash\n\nFrom the docs:\n\n    {BEGIN}\n    ...\n    {END}\n\nMy own notes below."
        );
        fs::write(&path, note(&dir).replace("# 2026-09-18 · Dash", &quoted)).unwrap();

        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        d.push("- **12:00** · stopped".into());
        d.save().unwrap();

        let text = note(&dir);
        assert!(text.contains("My own notes below."), "prose lost:\n{text}");
        assert!(text.contains("    <!-- omalogbook:begin -->"), "{text}");
        assert!(text.contains("- **09:15**"), "entries lost:\n{text}");
    }

    #[test]
    fn an_orphan_marker_is_swallowed_no_further_on_a_second_save() {
        let dir = tempdir("orphan");
        let path = path_for(&dir, "2026-09-18");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("Notes.\n\n{BEGIN}\nlost marker\n")).unwrap();
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        d.push("- **10:00** · under way".into());
        d.save().unwrap();
        d.push("- **11:00** · still going".into());
        d.save().unwrap();
        let text = note(&dir);
        assert!(text.contains("Notes."), "{text}");
        assert!(
            text.contains("lost marker"),
            "prose lost on the second save:\n{text}"
        );
        assert!(
            text.contains("- **10:00**") && text.contains("- **11:00**"),
            "{text}"
        );
    }

    #[test]
    fn a_total_the_crew_corrected_stands() {
        let dir = tempdir("corrected");
        let mut d = day(&dir);
        d.max_sog_kn = 42.0; // a bad fix, logged
        d.save().unwrap();
        let path = path_for(&dir, "2026-09-18");
        fs::write(
            &path,
            note(&dir).replace("max_sog_kn: 42.0", "max_sog_kn: 6.2"),
        )
        .unwrap();

        d.push("- **12:00** · stopped".into());
        d.save().unwrap();
        assert!(note(&dir).contains("max_sog_kn: 6.2"), "{}", note(&dir));
    }

    #[test]
    fn a_note_that_becomes_readable_again_is_written_again() {
        let dir = tempdir("unlatch");
        let path = path_for(&dir, "2026-09-18");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"\xff\xfe not text").unwrap();
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        d.push("- **10:00** · under way".into());
        assert!(d.save().is_err());
        fs::remove_file(&path).unwrap(); // the crew moves it aside
        d.save().unwrap();
        assert!(note(&dir).contains("- **10:00**"));
    }

    #[test]
    fn a_total_that_is_not_a_number_is_not_a_correction() {
        let dir = tempdir("nonsense");
        let mut d = day(&dir);
        d.save().unwrap();
        let path = path_for(&dir, "2026-09-18");
        fs::write(
            &path,
            note(&dir).replace("distance_nm: 12.4", "distance_nm: 12.4 nm"),
        )
        .unwrap();
        d.push("- **12:00** · stopped".into());
        d.save().unwrap();
        let text = note(&dir);
        assert!(text.contains("distance_nm: 12.4\n"), "{text}");
        assert!(text.contains("**Day's run** 12.4 nm"), "{text}");
    }

    #[test]
    fn totals_already_on_disk_are_added_to() {
        let dir = tempdir("resume");
        day(&dir).save().unwrap();
        // A new run of the program, continuing the same day.
        let mut second = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        assert_eq!(second.distance_nm, 12.4);
        second.distance_nm += 3.0;
        second.push("- **13:00** · under way".into());
        second.save().unwrap();
        assert!(note(&dir).contains("distance_nm: 15.4"), "{}", note(&dir));
    }

    #[test]
    fn continues_the_days_totals() {
        let dir = tempdir("totals");
        day(&dir).save().unwrap();
        let second = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        assert_eq!(second.distance_nm, 12.4);
        assert_eq!(second.max_sog_kn, 6.1);
        assert_eq!(second.underway_secs, 11_520);
        assert!(!second.is_empty());
    }

    #[test]
    fn links_each_track_once() {
        let dir = tempdir("tracks");
        let mut d = day(&dir);
        d.add_track("tracks/2026-09-18-0915.gpx");
        d.add_track("tracks/2026-09-18-0915.gpx");
        d.save().unwrap();
        let reopened = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        assert_eq!(reopened.tracks, vec!["tracks/2026-09-18-0915.gpx"]);
    }

    #[test]
    fn a_half_written_block_is_not_swallowed() {
        let dir = tempdir("half");
        let path = path_for(&dir, "2026-09-18");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("Notes.\n\n{BEGIN}\nlost marker\n")).unwrap();
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        d.push("- **10:00** · under way".into());
        d.save().unwrap();
        let text = note(&dir);
        assert!(text.contains("Notes."));
        assert!(text.contains("lost marker"));
        assert!(text.contains("- **10:00**"));
    }

    #[test]
    fn an_entry_is_always_one_line() {
        let dir = tempdir("oneline");
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        d.push("- **10:00** — first line\nsecond line".into());
        d.save().unwrap();
        let text = note(&dir);
        assert!(
            text.contains("- **10:00** — first line second line"),
            "{text}"
        );
        assert!(!Day::open(&dir, "2026-09-18", "Dash").unwrap().is_empty());
    }

    #[test]
    fn a_date_that_is_not_a_date_makes_no_deep_path() {
        let dir = Path::new("/tmp/vault");
        assert_eq!(
            path_for(dir, "2026-09-18"),
            dir.join("2026").join("09").join("2026-09-18.md")
        );
        for bad in ["x", "", "2026-13-01", "2026-02-31"] {
            let path = path_for(dir, bad);
            assert_eq!(path.parent(), Some(dir), "{bad} -> {path:?}");
        }
    }

    /// A day with two entries: a machine one and the crew's own.
    fn written(name: &str) -> PathBuf {
        let dir = tempdir(name);
        let mut d = day(&dir);
        d.push(
            "- **14:32** (21:32 UTC) · 37°52.0′N 122°18.9′W — Dolphins off teh port side".into(),
        );
        d.save().unwrap();
        dir
    }

    #[test]
    fn amending_changes_the_words_and_nothing_else() {
        let dir = written("amend");
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        let line = d
            .revise(1, "", &Revision::Amend("Dolphins off the port side".into()))
            .unwrap();
        assert_eq!(
            line,
            "- **14:32** (21:32 UTC) · 37°52.0′N 122°18.9′W — Dolphins off the port side"
        );
        let text = note(&dir);
        assert!(text.contains("— Dolphins off the port side"), "{text}");
        assert!(!text.contains("teh"), "{text}");
        // The day's numbers are untouched by a change to its words.
        assert!(text.contains("distance_nm: 12.4"), "{text}");
    }

    /// A bare position line has no words yet; amending gives it some.
    #[test]
    fn amending_a_line_with_no_words_adds_them() {
        let dir = written("addwords");
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        let line = d
            .revise(0, "", &Revision::Amend("Slipped the lines".into()))
            .unwrap();
        assert!(line.ends_with("· under way — Slipped the lines"), "{line}");
    }

    #[test]
    fn striking_rules_a_line_through_but_keeps_the_clock() {
        let dir = written("strike");
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        let line = d.revise(1, "", &Revision::Strike).unwrap();
        assert_eq!(
            line,
            "- **14:32** ~~(21:32 UTC) · 37°52.0′N 122°18.9′W — Dolphins off teh port side~~"
        );
        assert!(note(&dir).contains("~~"), "{}", note(&dir));
    }

    #[test]
    fn a_struck_entry_is_not_struck_twice() {
        let dir = written("twice");
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        d.revise(1, "", &Revision::Strike).unwrap();
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        assert!(d.revise(1, "", &Revision::Strike).is_err());
    }

    #[test]
    fn erasing_takes_the_line_out() {
        let dir = written("erase");
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        d.revise(1, "", &Revision::Erase).unwrap();
        let text = note(&dir);
        assert!(!text.contains("Dolphins"), "{text}");
        assert_eq!(text.matches("- **").count(), 1, "{text}");
    }

    /// The running log writes an entry an hour, and every one shifts what an
    /// index means. Change the entry the caller meant or change nothing.
    #[test]
    fn an_entry_that_moved_since_it_was_read_is_not_the_one_changed() {
        let dir = written("moved");
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        let err = d
            .revise(1, "- **14:32** something else entirely", &Revision::Erase)
            .unwrap_err();
        assert!(err.to_string().contains("changed since"), "{err}");
        assert!(note(&dir).contains("Dolphins"), "the entry went anyway");
    }

    #[test]
    fn the_entry_the_caller_meant_is_matched_past_rewrapping() {
        let dir = written("expect");
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        let spaced =
            "-  **14:32**  (21:32 UTC) · 37°52.0′N 122°18.9′W  — Dolphins off teh port side";
        assert!(d.revise(1, spaced, &Revision::Strike).is_ok());
    }

    #[test]
    fn an_entry_that_is_not_there_is_refused() {
        let dir = written("range");
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        let err = d.revise(9, "", &Revision::Strike).unwrap_err();
        assert!(err.to_string().contains("2 entries"), "{err}");
    }

    #[test]
    fn a_note_that_is_not_text_is_never_revised() {
        let dir = tempdir("binary-revise");
        let path = path_for(&dir, "2026-09-18");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let junk = b"log \xff\xfe not text".to_vec();
        fs::write(&path, &junk).unwrap();
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        assert!(d.revise(0, "", &Revision::Erase).is_err());
        assert_eq!(fs::read(&path).unwrap(), junk, "the file was rewritten");
    }

    fn tempdir(name: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("omalogbook-day-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }
}
