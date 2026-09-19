//! A day's note: plain markdown with YAML front matter, the shape Obsidian
//! and every other markdown editor already understand.
//!
//! Omalogbook owns exactly one block, between two HTML comments, and the day's
//! numbers in the front matter. Everything else in the file is the crew's, and
//! is copied through untouched. When in doubt the file is left alone: a note
//! that can't be parsed is never rewritten.

use crate::{geo, time};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const BEGIN: &str = "<!-- omalogbook:begin -->";
pub const END: &str = "<!-- omalogbook:end -->";
const BANNER: &str = "<!-- Written by omalogbook. Your own notes belong outside this block. -->";

/// The day's note, held open across a watch.
#[derive(Debug)]
pub struct Day {
    pub date: String,
    pub path: PathBuf,
    boat: String,
    /// Rendered entry lines, oldest first.
    entries: Vec<String>,
    pub distance_nm: f64,
    pub max_sog_kn: f64,
    pub underway_secs: i64,
    /// Vault-relative track files, linked from the note.
    tracks: Vec<String>,
}

/// `<vault>/2026/09/2026-09-18.md`, a year and month deep so a long voyage
/// stays navigable in a file manager.
pub fn path_for(vault: &Path, date: &str) -> PathBuf {
    let (year, month) = (&date[..4], &date[5..7]);
    vault.join(year).join(month).join(format!("{date}.md"))
}

impl Day {
    /// Open the day, continuing the entries and totals already written.
    pub fn open(vault: &Path, date: &str, boat: &str) -> io::Result<Day> {
        let path = path_for(vault, date);
        let mut day = Day {
            date: date.to_string(),
            path: path.clone(),
            boat: boat.to_string(),
            entries: Vec::new(),
            distance_nm: 0.0,
            max_sog_kn: 0.0,
            underway_secs: 0,
            tracks: Vec::new(),
        };
        let Ok(text) = fs::read_to_string(&path) else {
            return Ok(day);
        };
        if let Some(block) = block_of(&text) {
            day.entries = block
                .lines()
                .filter(|l| l.starts_with("- "))
                .map(str::to_string)
                .collect();
        }
        for (key, value) in front_matter(&text) {
            match key.as_str() {
                "distance_nm" => day.distance_nm = value.parse().unwrap_or(0.0),
                "max_sog_kn" => day.max_sog_kn = value.parse().unwrap_or(0.0),
                "hours_underway" => {
                    day.underway_secs = (value.parse::<f64>().unwrap_or(0.0) * 3600.0) as i64;
                }
                "tracks" => {
                    day.tracks = value
                        .trim_matches(['[', ']'])
                        .split(',')
                        .map(|t| t.trim().trim_matches(['"', '\'']).to_string())
                        .filter(|t| !t.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
        Ok(day)
    }

    /// Add an entry, already rendered by [`crate::entry`].
    pub fn push(&mut self, line: String) {
        self.entries.push(line);
    }

    /// Link a track file, once.
    pub fn add_track(&mut self, relative: &str) {
        if !self.tracks.iter().any(|t| t == relative) {
            self.tracks.push(relative.to_string());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Write the note, keeping every line the crew wrote outside the block.
    /// The file is replaced only once it is complete on disk.
    pub fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let existing = fs::read_to_string(&self.path).unwrap_or_default();
        let text = self.render(&existing);
        let temp = self.path.with_extension("md.tmp");
        {
            let mut file = fs::File::create(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&temp, &self.path)
    }

    fn render(&self, existing: &str) -> String {
        let block = self.block();
        if existing.trim().is_empty() {
            return format!(
                "{}\n\n# {} · {}\n\n{block}\n",
                self.front(&[]),
                self.date,
                self.boat
            );
        }
        let (front, body) = split_front(existing);
        let body = match (body.find(BEGIN), body.find(END)) {
            (Some(start), Some(end)) if end > start => {
                let tail = &body[end + END.len()..];
                format!("{}{block}{tail}", &body[..start])
            }
            // No block, or a half-written one: add a fresh block at the end and
            // leave whatever is there alone.
            _ => format!("{}\n\n{block}\n", body.trim_end()),
        };
        format!("{}{}", self.front(&front), body)
    }

    /// The day's numbers, keeping any other front matter the crew added.
    fn front(&self, existing: &[(String, String)]) -> String {
        let mine: Vec<(&str, String)> = vec![
            ("date", self.date.clone()),
            ("boat", self.boat.clone()),
            ("distance_nm", format!("{:.1}", self.distance_nm)),
            ("max_sog_kn", format!("{:.1}", self.max_sog_kn)),
            (
                "hours_underway",
                format!("{:.1}", self.underway_secs as f64 / 3600.0),
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
        for (key, value) in &mine {
            out.push_str(&format!("{key}: {value}\n"));
        }
        for (key, value) in existing {
            if !mine.iter().any(|(k, _)| k == key) {
                out.push_str(&format!("{key}: {value}\n"));
            }
        }
        out.push_str("---\n");
        out
    }

    fn block(&self) -> String {
        let mut out = format!("{BEGIN}\n{BANNER}\n\n## The watch\n\n");
        if self.entries.is_empty() {
            out.push_str("_Nothing logged yet._\n");
        } else {
            for line in &self.entries {
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

/// `3 h 12 min`, or minutes alone for a short hop.
pub fn hours(secs: i64) -> String {
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    if h > 0 {
        format!("{h} h {m:02} min")
    } else {
        format!("{m} min")
    }
}

/// The text between the two markers, if both are there in order.
pub fn block_of(text: &str) -> Option<&str> {
    let start = text.find(BEGIN)? + BEGIN.len();
    let end = text[start..].find(END)? + start;
    Some(&text[start..end])
}

/// `key: value` pairs from the leading `---` block, in file order.
fn front_matter(text: &str) -> Vec<(String, String)> {
    split_front(text).0
}

/// The front matter and the rest, so a rewrite can keep both.
fn split_front(text: &str) -> (Vec<(String, String)>, &str) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return (Vec::new(), text);
    };
    let Some(end) = rest.find("\n---\n") else {
        return (Vec::new(), text);
    };
    let pairs = rest[..end]
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    (pairs, &rest[end + 5..])
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

    #[test]
    fn writes_a_note_that_opens_as_markdown() {
        let dir = tempdir();
        day(&dir).save().unwrap();
        let text = fs::read_to_string(path_for(&dir, "2026-09-18")).unwrap();
        assert!(text.starts_with("---\ndate: 2026-09-18\nboat: Dash\n"));
        assert!(text.contains("distance_nm: 12.4"));
        assert!(text.contains("hours_underway: 3.2"));
        assert!(text.contains("- **09:15**"));
        assert!(text.contains("**Day's run** 12.4 nm"));
        assert!(text.contains("**under way** 3 h 12 min"));
    }

    #[test]
    fn keeps_what_the_crew_wrote() {
        let dir = tempdir();
        day(&dir).save().unwrap();
        let path = path_for(&dir, "2026-09-18");
        let text = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            text.replace(BEGIN, "Reefed at noon, wind up to 22.\n\n{BEGIN}")
                .replace("{BEGIN}", BEGIN),
        )
        .unwrap();

        let mut second = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        second.push("- **14:00** · anchored".into());
        second.save().unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("Reefed at noon, wind up to 22."), "{text}");
        assert!(text.contains("- **09:15**"), "earlier entry lost:\n{text}");
        assert!(text.contains("- **14:00**"));
        assert_eq!(text.matches(BEGIN).count(), 1);
    }

    #[test]
    fn continues_the_days_totals() {
        let dir = tempdir();
        day(&dir).save().unwrap();
        let second = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        assert_eq!(second.distance_nm, 12.4);
        assert_eq!(second.max_sog_kn, 6.1);
        assert_eq!(second.underway_secs, 11_520);
        assert_eq!(second.entries.len(), 1);
    }

    #[test]
    fn keeps_front_matter_the_crew_added() {
        let dir = tempdir();
        day(&dir).save().unwrap();
        let path = path_for(&dir, "2026-09-18");
        let text = fs::read_to_string(&path).unwrap();
        fs::write(&path, text.replacen("---\n", "---\ncrew: Casey\n", 1)).unwrap();
        Day::open(&dir, "2026-09-18", "Dash")
            .unwrap()
            .save()
            .unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("crew: Casey"));
    }

    #[test]
    fn links_each_track_once() {
        let dir = tempdir();
        let mut d = day(&dir);
        d.add_track("tracks/2026-09-18-0915.gpx");
        d.add_track("tracks/2026-09-18-0915.gpx");
        d.save().unwrap();
        let text = fs::read_to_string(path_for(&dir, "2026-09-18")).unwrap();
        // once in the front matter, twice in the markdown link
        assert_eq!(text.matches("tracks/2026-09-18-0915.gpx").count(), 3);
        let reopened = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        assert_eq!(reopened.tracks, vec!["tracks/2026-09-18-0915.gpx"]);
    }

    #[test]
    fn a_half_written_block_is_not_swallowed() {
        let dir = tempdir();
        let path = path_for(&dir, "2026-09-18");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("Notes.\n\n{BEGIN}\nlost marker\n")).unwrap();
        let mut d = Day::open(&dir, "2026-09-18", "Dash").unwrap();
        d.push("- **10:00** · under way".into());
        d.save().unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("Notes."));
        assert!(text.contains("lost marker"));
        assert!(text.contains("- **10:00**"));
    }

    fn tempdir() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "omalogbook-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }
}
