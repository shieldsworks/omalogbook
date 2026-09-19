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
        };
        match read(&path) {
            Ok(Some(text)) => {
                day.had_entries = !entries_in(&text).is_empty();
                day.merge_totals(&text);
            }
            Ok(None) => {}
            Err(Unreadable) => day.readable = false,
        }
        Ok(day)
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
                "distance_nm" => {
                    self.distance_nm = self.distance_nm.max(value.parse().unwrap_or(0.0))
                }
                "max_sog_kn" => self.max_sog_kn = self.max_sog_kn.max(value.parse().unwrap_or(0.0)),
                "hours_underway" => {
                    let secs = (value.parse::<f64>().unwrap_or(0.0) * 3600.0).round() as i64;
                    self.underway_secs = self.underway_secs.max(secs);
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
            return Err(self.left_alone());
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
        Ok(())
    }

    fn left_alone(&self) -> io::Error {
        io::Error::other(format!(
            "{} is not text; leaving it alone",
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
        out.push_str("---\n\n");
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
        let trimmed = line.trim();
        if start.is_none() && trimmed == BEGIN {
            start = Some(offset);
        } else if start.is_some() && trimmed == END {
            return start.map(|s| (s, offset + line.len() - trailing_newline(line)));
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

    fn tempdir(name: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("omalogbook-day-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }
}
