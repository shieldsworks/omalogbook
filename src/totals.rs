//! Every day's run, added up.
//!
//! The numbers come from the notes themselves, read fresh each time, not
//! from a cache kept alongside them. A cache would be a second copy of the
//! truth, and the first thing to go wrong when the crew corrects a total by
//! hand or restores a note from a backup. A vault is a folder of small text
//! files; reading it is cheap, and it is always right.

use crate::{day, time};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

/// As much of a note as is read looking for front matter. The front matter
/// omalogbook writes is a dozen lines; anything past this is the crew's
/// prose, and reading a whole voyage's worth of it to add up six numbers
/// would be work for nothing.
const HEAD: u64 = 64 * 1024;

/// A day, as its note reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct DayRun {
    pub date: String,
    pub distance_nm: f64,
    pub max_sog_kn: f64,
    pub underway_secs: i64,
    pub passages: u32,
}

impl DayRun {
    /// A day counts as sailed if the boat went anywhere or spent time under
    /// way. A note written at the dock is a day logged, not a day sailed.
    pub fn sailed(&self) -> bool {
        self.distance_nm > 0.0 || self.underway_secs > 0
    }
}

/// The whole log, and the same shape for a single year.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub days: u32,
    pub days_sailed: u32,
    pub distance_nm: f64,
    pub underway_secs: i64,
    pub passages: u32,
}

impl Run {
    fn add(&mut self, day: &DayRun) {
        self.days += 1;
        if day.sailed() {
            self.days_sailed += 1;
        }
        self.distance_nm += day.distance_nm;
        self.underway_secs += day.underway_secs;
        self.passages += day.passages;
    }

    /// Speed made good over the time actually under way — not over the days,
    /// which would count every night at anchor as sailing slowly.
    pub fn average_kn(&self) -> Option<f64> {
        (self.underway_secs > 0).then(|| self.distance_nm / (self.underway_secs as f64 / 3600.0))
    }
}

/// The best single day at something, and when it was.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Best {
    pub value: f64,
    pub date: String,
}

impl Best {
    fn offer(&mut self, value: f64, date: &str) {
        // Strictly greater, so the first day to reach a number keeps it.
        if value > self.value {
            self.value = value;
            self.date = date.to_string();
        }
    }
    pub fn known(&self) -> bool {
        self.value > 0.0 && !self.date.is_empty()
    }
}

/// Everything the log adds up to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Totals {
    pub all: Run,
    /// This calendar year so far, which is the season a sailor is in.
    pub year: Run,
    pub this_year: i64,
    pub fastest: Best,
    pub best_day: Best,
    /// Hours, so one `Best` can carry it; seconds are kept for the clock.
    pub longest: Best,
    pub longest_secs: i64,
    pub first: String,
    pub last: String,
    /// Today, if the log has a note for it. The day in progress belongs
    /// beside the lifetime numbers: a passage under way is the one figure
    /// that changes while you are looking at it.
    pub today: Option<DayRun>,
}

impl Totals {
    /// Days from the first note to the last, counting both ends. The span a
    /// log covers, which is not the number of days in it.
    pub fn span_days(&self) -> Option<i64> {
        let (first, last) = (time::day_index(&self.first)?, time::day_index(&self.last)?);
        Some(last - first + 1)
    }
}

/// Read the vault and add it up. Unreadable notes are skipped rather than
/// reported: one file the crew saved as a PDF should not cost the totals.
pub fn read(vault: &Path, this_year: i64) -> Totals {
    let mut totals = Totals {
        this_year,
        ..Totals::default()
    };
    let mut days = notes(vault);
    // In date order, so first and last are the log's ends whatever the file
    // system handed back.
    days.sort_by(|a, b| a.date.cmp(&b.date));
    for day in &days {
        totals.all.add(day);
        if day.date.starts_with(&format!("{this_year}-")) {
            totals.year.add(day);
        }
        totals.fastest.offer(day.max_sog_kn, &day.date);
        totals.best_day.offer(day.distance_nm, &day.date);
        if day.underway_secs > totals.longest_secs {
            totals.longest_secs = day.underway_secs;
            totals.longest = Best {
                value: day.underway_secs as f64 / 3600.0,
                date: day.date.clone(),
            };
        }
    }
    if let Some(first) = days.iter().find(|d| d.sailed()) {
        totals.first = first.date.clone();
    }
    if let Some(last) = days.iter().rev().find(|d| d.sailed()) {
        totals.last = last.date.clone();
    }
    let today = day::today();
    totals.today = days.into_iter().find(|d| d.date == today);
    totals
}

/// Every day note in the vault: `<vault>/YYYY/MM/YYYY-MM-DD.md`. Only that
/// shape, so `tracks/`, a README, or the crew's own folders are passed over
/// without being read at all.
fn notes(vault: &Path) -> Vec<DayRun> {
    let mut out = Vec::new();
    for year in subdirs(vault, 4) {
        for month in subdirs(&year, 2) {
            let Ok(entries) = std::fs::read_dir(&month) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let Some(date) = name.strip_suffix(".md") else {
                    continue;
                };
                if !time::valid_date(date) || !path.is_file() {
                    continue;
                }
                if let Some(run) = run_of(&path, date) {
                    out.push(run);
                }
            }
        }
    }
    out
}

/// Subdirectories whose names are exactly `digits` digits: the year and month
/// folders, and nothing else.
fn subdirs(dir: &Path, digits: usize) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.len() == digits && n.bytes().all(|b| b.is_ascii_digit()))
                && p.is_dir()
        })
        .collect()
}

/// One note's numbers. A note with no front matter omalogbook recognizes is
/// still a day logged, with nothing sailed — the crew wrote something that
/// day, which is worth counting as a day in the log.
fn run_of(path: &Path, date: &str) -> Option<DayRun> {
    let text = head(path)?;
    let mut run = DayRun {
        date: date.to_string(),
        distance_nm: 0.0,
        max_sog_kn: 0.0,
        underway_secs: 0,
        passages: 0,
    };
    for (key, value) in day::front_pairs(&text) {
        match key.as_str() {
            // A number the crew corrected reads exactly as they left it;
            // anything that isn't a number is not a correction, and is
            // passed over rather than read as zero.
            "distance_nm" => run.distance_nm = positive(&value).unwrap_or(run.distance_nm),
            "max_sog_kn" => run.max_sog_kn = positive(&value).unwrap_or(run.max_sog_kn),
            "hours_underway" => {
                if let Some(h) = positive(&value) {
                    run.underway_secs = (h * 3600.0).round() as i64;
                }
            }
            "tracks" => run.passages = count_tracks(&value),
            _ => {}
        }
    }
    Some(run)
}

/// A finite, non-negative number. A negative run or a NaN is a typo, not a
/// day sailed backwards.
fn positive(value: &str) -> Option<f64> {
    match value.trim().parse::<f64>() {
        Ok(v) if v.is_finite() && v >= 0.0 => Some(v),
        _ => None,
    }
}

/// How many tracks a `tracks: ["a.gpx", "b.gpx"]` line names. Counting the
/// quoted names rather than the commas, so a trailing comma or an empty list
/// counts what is there.
fn count_tracks(value: &str) -> u32 {
    value.matches('"').count() as u32 / 2
}

/// The start of a file, as text. A file that isn't UTF-8 is not a note.
fn head(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut text = String::new();
    // Lossy: a note that is text apart from one bad byte still has its front
    // matter, and `read_to_string` would refuse the whole file for it.
    let mut bytes = Vec::new();
    file.take(HEAD).read_to_end(&mut bytes).ok()?;
    text.push_str(&String::from_utf8_lossy(&bytes));
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn vault(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("omalogbook-totals-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn note(vault: &Path, date: &str, front: &str) {
        let path = day::path_for(vault, date);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!("---\ndate: {date}\n{front}---\n\nThe watch.\n"),
        )
        .unwrap();
    }

    fn sailed(vault: &Path, date: &str, nm: f64, max: f64, hours: f64, tracks: &str) {
        note(
            vault,
            date,
            &format!(
                "boat: Dash\ndistance_nm: {nm:.1}\nmax_sog_kn: {max:.1}\nhours_underway: {hours:.2}\ntracks: [{tracks}]\n"
            ),
        );
    }

    #[test]
    fn adds_up_every_day() {
        let dir = vault("adds");
        sailed(&dir, "2026-09-18", 12.4, 6.1, 3.2, "\"tracks/a.gpx\"");
        sailed(
            &dir,
            "2026-09-19",
            20.0,
            7.5,
            4.0,
            "\"tracks/b.gpx\", \"tracks/c.gpx\"",
        );
        let t = read(&dir, 2026);
        assert_eq!(t.all.days, 2);
        assert_eq!(t.all.days_sailed, 2);
        assert!((t.all.distance_nm - 32.4).abs() < 1e-9);
        assert_eq!(t.all.underway_secs, (7.2_f64 * 3600.0).round() as i64);
        assert_eq!(t.all.passages, 3);
        assert_eq!(t.fastest.value, 7.5);
        assert_eq!(t.fastest.date, "2026-09-19");
        assert_eq!(t.best_day.date, "2026-09-19");
        assert_eq!(t.longest.date, "2026-09-19");
        assert_eq!(t.first, "2026-09-18");
        assert_eq!(t.last, "2026-09-19");
        assert_eq!(t.span_days(), Some(2));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn average_is_over_the_time_under_way() {
        let dir = vault("average");
        // Ten miles in two hours, and a day at the dock that must not drag
        // the average down.
        sailed(&dir, "2026-09-18", 10.0, 6.0, 2.0, "");
        note(&dir, "2026-09-19", "boat: Dash\n");
        let t = read(&dir, 2026);
        assert_eq!(t.all.average_kn(), Some(5.0));
        assert_eq!(t.all.days, 2);
        assert_eq!(t.all.days_sailed, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn today_is_picked_out_of_the_log() {
        let dir = vault("today");
        sailed(&dir, "2026-09-18", 4.0, 3.0, 1.0, "");
        assert_eq!(read(&dir, 2026).today, None, "an old log has no day in it");
        let now = day::today();
        sailed(&dir, &now, 7.5, 5.2, 2.0, "\"tracks/now.gpx\"");
        let t = read(&dir, 2026);
        let today = t.today.expect("today's run");
        assert_eq!(today.date, now);
        assert_eq!(today.distance_nm, 7.5);
        assert_eq!(today.passages, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_vault_totals_nothing() {
        let dir = vault("empty");
        let t = read(&dir, 2026);
        assert_eq!(t.all, Run::default());
        assert_eq!(t.all.average_kn(), None);
        assert!(!t.fastest.known());
        assert_eq!(t.span_days(), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_year_is_its_own_total() {
        let dir = vault("year");
        sailed(&dir, "2025-12-31", 8.0, 5.0, 2.0, "\"tracks/old.gpx\"");
        sailed(&dir, "2026-01-01", 5.0, 4.0, 1.0, "\"tracks/new.gpx\"");
        let t = read(&dir, 2026);
        assert_eq!(t.all.distance_nm, 13.0);
        assert_eq!(t.year.distance_nm, 5.0);
        assert_eq!(t.year.days, 1);
        assert_eq!(t.year.passages, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_but_day_notes_is_read() {
        let dir = vault("only-notes");
        sailed(&dir, "2026-09-18", 4.0, 3.0, 1.0, "");
        // A track folder, a stray file, and a note that isn't a date.
        fs::create_dir_all(dir.join("tracks")).unwrap();
        fs::write(dir.join("tracks/2026-09-18-0915.gpx"), "<gpx/>").unwrap();
        fs::write(dir.join("README.md"), "---\ndistance_nm: 999\n---\n").unwrap();
        fs::write(dir.join("2026/09/notes.md"), "---\ndistance_nm: 999\n---\n").unwrap();
        fs::write(
            dir.join("2026/09/2026-09-31.md"),
            "---\ndistance_nm: 999\n---\n",
        )
        .unwrap();
        let t = read(&dir, 2026);
        assert_eq!(t.all.days, 1);
        assert_eq!(t.all.distance_nm, 4.0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_total_that_isnt_a_number_is_passed_over() {
        let dir = vault("junk");
        note(
            &dir,
            "2026-09-18",
            "distance_nm: about twelve\nmax_sog_kn: -3\nhours_underway: NaN\n",
        );
        let t = read(&dir, 2026);
        assert_eq!(t.all.days, 1);
        assert_eq!(t.all.distance_nm, 0.0);
        assert_eq!(t.all.underway_secs, 0);
        assert!(!t.fastest.known(), "a negative speed is not a record");
        assert_eq!(
            t.all.days_sailed, 0,
            "a day with no numbers is logged, not sailed"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_note_that_isnt_text_is_skipped_not_fatal() {
        let dir = vault("binary");
        sailed(&dir, "2026-09-18", 4.0, 3.0, 1.0, "");
        let path = day::path_for(&dir, "2026-09-19");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, [0xff, 0xfe, 0x00, 0x01]).unwrap();
        let t = read(&dir, 2026);
        assert_eq!(t.all.days, 2);
        assert_eq!(t.all.distance_nm, 4.0);
        let _ = fs::remove_dir_all(&dir);
    }
}
