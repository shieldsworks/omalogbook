//! The watch: omakeel's fixes become entries, tracks and a day's run.
//!
//! Every decision here is deliberately dull. A passage starts when the boat
//! has been moving, ends when it has been still for a while, and the log
//! writes what it saw. Nothing is inferred that the crew can't check.

use crate::{
    config::Settings,
    day::{self, Day},
    entry, geo, git, keel, lock, time,
    track::{Point, Track},
};
use std::{io, path::PathBuf};

/// A gap longer than this is the log having been stopped, or the receiver
/// having been off: it does not count as time under way.
const MAX_GAP_SECS: i64 = 300;

/// How far the receiver's clock may be from this machine's before the log
/// stops believing it. An hour covers a wrong time zone in the receiver and
/// any ordinary drift.
const CLOCK_TOLERANCE_SECS: i64 = 3600;

/// Before this, the machine's own clock has plainly never been set.
const CLOCK_LOOKS_UNSET: i64 = 1_577_836_800; // 2020-01-01

/// How long a lost fix is watched before the line saying why is written.
/// When a link breaks, omakeel sees the source go and the fix go stale a
/// second or two apart; waiting a moment names the cause rather than the
/// first symptom.
const SETTLE_SECS: i64 = 3;

/// How many times one outage may change its reason in the log. A link that
/// can't make up its mind would otherwise write a line every few seconds.
const MAX_REASONS: u8 = 4;

/// A fix that has gone missing, from the moment it went.
struct Outage {
    /// When the watch saw it go.
    since: i64,
    /// The reason last written, and the latest seen.
    said: keel::Why,
    /// When the last current fix came, which is when the boat really lost
    /// its position: a link that goes silent only reads as lost once the
    /// fix is five seconds old.
    from: i64,
    why: keel::Why,
    /// Whether the "no fix" line is in the log yet.
    written: bool,
    /// How many times the reason has changed since.
    changed: u8,
}

pub struct Watch {
    settings: Settings,
    vault: PathBuf,
    day: Day,
    track: Option<Track>,
    underway: bool,
    /// Where and when the last counted position was.
    last: Option<(f64, f64, i64)>,
    last_point: i64,
    last_entry: i64,
    /// Since when the boat has been below the stopping speed.
    slow_since: Option<i64>,
    has_fix: bool,
    opened: bool,
    /// Said once, when the receiver's clock and this machine's disagree.
    warned_clock: bool,
    /// Said once, when omakeel speaks a protocol this doesn't know.
    warned_version: bool,
    /// When a current fix last arrived, by this machine's clock, so a passage
    /// can't outlive it. The receiver's own clock is no use here: the two are
    /// allowed to differ, and comparing across them would close a passage on
    /// the first dropout, or never.
    last_fix: i64,
    /// When the day's totals last reached the disk.
    last_save: i64,
    /// The fix being lost, while it is.
    outage: Option<Outage>,
    /// Sources that weren't delivering while the fix was good.
    idle: Vec<String>,
    /// Satellites and HDOP of the last current fix, to say how good the fix
    /// was just before it went.
    quality: (Option<u32>, Option<f64>),
}

impl Watch {
    pub fn new(settings: Settings) -> io::Result<Watch> {
        let vault = settings.vault.clone();
        let day = Day::open(&vault, &day::today(), &settings.boat)?;
        Ok(Watch {
            settings,
            vault,
            day,
            track: None,
            underway: false,
            last: None,
            last_point: 0,
            last_entry: 0,
            slow_since: None,
            has_fix: false,
            opened: false,
            warned_clock: false,
            warned_version: false,
            last_fix: 0,
            last_save: 0,
            outage: None,
            idle: Vec::new(),
            quality: (None, None),
        })
    }

    /// One update from omakeel. Returns what it wrote, for the terminal.
    pub fn update(&mut self, update: keel::Update, now: i64) -> io::Result<Vec<String>> {
        match update {
            keel::Update::Fix(fix, idle) => {
                self.idle = idle;
                self.moved(fix, now)
            }
            keel::Update::NoFix(why) => self.lost(why, now),
            keel::Update::Lost => self.lost(keel::Why::Hub, now),
            keel::Update::Incompatible(v) => {
                // Every message would otherwise be an entry, all day long.
                if self.warned_version {
                    return Ok(Vec::new());
                }
                self.warned_version = true;
                let mut wrote = self.roll_over(now)?;
                wrote.extend(self.say(
                    now,
                    format!("omakeel speaks protocol {v}; the log is paused"),
                )?);
                Ok(wrote)
            }
        }
    }

    fn moved(&mut self, fix: keel::Fix, now: i64) -> io::Result<Vec<String>> {
        let (at, complaint) = self.stamp(fix.utc, now);
        let mut wrote = Vec::new();
        if let Some(complaint) = complaint {
            wrote.extend(self.say(at, complaint)?);
        }
        // Always this machine's clock, so a receiver a little out of step
        // can't flip the log between two days around midnight.
        wrote.extend(self.before_midnight(now)?);
        wrote.extend(self.roll_over(now)?);

        if !self.has_fix {
            self.has_fix = true;
            if self.outage.as_ref().is_some_and(|o| !o.written) {
                wrote.extend(self.write_outage()?);
            }
            if let Some(outage) = self.outage.take() {
                let mut line = format!("fix again after {}", gone_for(now - outage.from));
                // Past the cap, the reason the fix came back from may never
                // have been written.
                if !outage.said.same_as(&outage.why) {
                    line.push_str(&format!(" · last: {}", outage.why.say()));
                }
                wrote.extend(self.say(at, line)?);
            }
        }
        self.quality = (fix.satellites, fix.hdop);
        if !self.opened {
            self.opened = true;
            let line = format!("log opened · {}", day::position(fix.lat, fix.lon));
            wrote.extend(self.say(at, line)?);
        }

        self.last_fix = now;
        let speed = fix.sog_kn.unwrap_or(0.0);
        if !self.underway && speed >= self.settings.underway_kn {
            self.underway = true;
            self.slow_since = None;
            self.last = Some((fix.lat, fix.lon, at));
            self.last_entry = at;
            self.track = Some(Track::start(&self.vault, at, &self.settings.boat));
            wrote.extend(self.log(
                at,
                entry::fix(
                    time::local(at),
                    time::utc(at),
                    fix.lat,
                    fix.lon,
                    fix.sog_kn,
                    fix.cog_deg,
                    "under way",
                ),
            )?);
        }

        if !self.underway {
            return Ok(wrote);
        }

        // The day's run, and the time it took, from fix to fix.
        if let Some((lat, lon, was)) = self.last {
            let gap = at - was;
            if (0..=MAX_GAP_SECS).contains(&gap) {
                self.day.distance_nm += geo::distance_nm((lat, lon), (fix.lat, fix.lon));
                self.day.underway_secs += gap;
            }
        }
        self.last = Some((fix.lat, fix.lon, at));
        self.day.max_sog_kn = self.day.max_sog_kn.max(speed);

        if at - self.last_point >= i64::from(self.settings.point_seconds)
            && let Some(track) = self.track.as_mut()
        {
            track.push(Point {
                epoch: at,
                lat: fix.lat,
                lon: fix.lon,
                sog_kn: fix.sog_kn,
            });
            self.last_point = at;
            track.save()?;
        }

        if at - self.last_entry >= i64::from(self.settings.every_minutes) * 60 {
            self.last_entry = at;
            wrote.extend(self.log(
                at,
                entry::fix(
                    time::local(at),
                    time::utc(at),
                    fix.lat,
                    fix.lon,
                    fix.sog_kn,
                    fix.cog_deg,
                    "",
                ),
            )?);
        }

        // Still for long enough is the end of the passage. Only making way
        // again clears it, so a boat swinging at anchor stays stopped.
        if speed <= self.settings.stopped_kn {
            let since = *self.slow_since.get_or_insert(at);
            if at - since >= i64::from(self.settings.stop_after_minutes) * 60 {
                wrote.extend(self.stop(fix, at)?);
                return Ok(wrote);
            }
        } else if speed >= self.settings.underway_kn {
            self.slow_since = None;
        }

        // Keep the day's run on disk even when nothing is worth an entry.
        if at - self.last_save >= 300 {
            self.save()?;
        }
        Ok(wrote)
    }

    fn stop(&mut self, fix: keel::Fix, at: i64) -> io::Result<Vec<String>> {
        self.underway = false;
        self.slow_since = None;
        self.last = None;
        if let Some(track) = self.track.take() {
            track.save()?;
            if !track.is_empty() {
                self.day.add_track(&track.relative);
            }
        }
        let line = entry::fix(
            time::local(at),
            time::utc(at),
            fix.lat,
            fix.lon,
            None,
            None,
            "stopped",
        );
        let wrote = self.log(at, line)?;
        self.commit(&format!("log: {} · stopped", self.day.date));
        Ok(wrote)
    }

    /// Which clock an entry is stamped by.
    ///
    /// The receiver's time is better than this machine's, until it isn't: a
    /// GPS that reports the wrong week, or a recording being replayed, would
    /// otherwise file entries days away from the day they were written. So it
    /// is believed while it agrees with this machine's clock, or while this
    /// machine's clock has obviously never been set.
    fn stamp(&mut self, receiver: Option<i64>, now: i64) -> (i64, Option<String>) {
        let Some(receiver) = receiver else {
            return (now, None);
        };
        if (receiver - now).abs() <= CLOCK_TOLERANCE_SECS || now < CLOCK_LOOKS_UNSET {
            // ...unless the two clocks fall on different days, which happens
            // for an hour around midnight. An entry stamped 23:10 at the top
            // of the next day's note reads as a mistake, because it is.
            if time::local(receiver).date() != time::local(now).date() {
                return (now, None);
            }
            return (receiver, None);
        }
        if self.warned_clock {
            return (now, None);
        }
        self.warned_clock = true;
        let days = (now - receiver) as f64 / 86_400.0;
        (
            now,
            Some(format!(
                "the receiver's clock reads {:.1} days {}; logging by this machine's clock",
                days.abs(),
                if days > 0.0 { "behind" } else { "ahead" }
            )),
        )
    }

    fn lost(&mut self, why: keel::Why, now: i64) -> io::Result<Vec<String>> {
        // Roll the day over here too: past midnight, a lost fix belongs to
        // the new day, not to yesterday's note.
        let mut wrote = self.before_midnight(now)?;
        wrote.extend(self.roll_over(now)?);
        wrote.extend(self.no_fix(why, now)?);
        wrote.extend(self.abandon_passage(now)?);
        Ok(wrote)
    }

    /// Keep the passage open: a receiver drops out for a minute at a time,
    /// and the boat is still sailing. But say why, and say it again if the
    /// reason changes, because the crew reads this back at the marina to
    /// find out which part of the chain let go.
    fn no_fix(&mut self, why: keel::Why, now: i64) -> io::Result<Vec<String>> {
        let why = why.without(&self.idle);
        if self.has_fix {
            self.has_fix = false;
            self.outage = Some(Outage {
                since: now,
                said: why.clone(),
                from: self.last_fix,
                why,
                written: false,
                changed: 0,
            });
            return Ok(Vec::new());
        }
        // Never had a fix: there is nothing to have lost.
        let Some(outage) = self.outage.as_mut() else {
            return Ok(Vec::new());
        };
        outage.why = why;
        if !outage.written {
            if now - outage.since < SETTLE_SECS {
                return Ok(Vec::new());
            }
            return self.write_outage();
        }
        if outage.said.same_as(&outage.why) {
            return Ok(Vec::new());
        }
        // Past the cap, only the hub going is still worth a line: it ends
        // everything else, and it is the one cause that must not be missed.
        if outage.changed >= MAX_REASONS && outage.why != keel::Why::Hub {
            return Ok(Vec::new());
        }
        outage.changed += 1;
        outage.said = outage.why.clone();
        let line = format!("still no fix: {}", outage.why.say());
        self.say(now, line)
    }

    /// The "no fix" line for the outage under way, once.
    fn write_outage(&mut self) -> io::Result<Vec<String>> {
        let Some(mut outage) = self.outage.take() else {
            return Ok(Vec::new());
        };
        let wrote = self.say_lost(&outage)?;
        outage.written = true;
        outage.said = outage.why.clone();
        self.outage = Some(outage);
        Ok(wrote)
    }

    /// A fix lost just before midnight belongs in the day it was lost in,
    /// so its line is written before the day turns, settled or not.
    fn before_midnight(&mut self, now: i64) -> io::Result<Vec<String>> {
        let pending = self.outage.as_ref().is_some_and(|o| !o.written);
        if pending && time::local(now).date() != self.day.date {
            return self.write_outage();
        }
        Ok(Vec::new())
    }

    /// `no fix: tcp:10.0.2.2:10110 is down (Connection refused) · last fix 9
    /// satellites, HDOP 0.9`, stamped when the fix went.
    fn say_lost(&mut self, outage: &Outage) -> io::Result<Vec<String>> {
        let mut line = format!("no fix: {}", outage.why.say());
        match self.quality {
            (Some(n), Some(h)) => {
                line.push_str(&format!(" · last fix {n} satellites, HDOP {h:.1}"))
            }
            (Some(n), None) => line.push_str(&format!(" · last fix {n} satellites")),
            (None, Some(h)) => line.push_str(&format!(" · last fix HDOP {h:.1}")),
            (None, None) => {}
        }
        self.say(outage.since, line)
    }

    /// A passage with no fix behind it ends where the fix did: the receiver
    /// went quiet, and the log will not invent the miles in between.
    fn abandon_passage(&mut self, now: i64) -> io::Result<Vec<String>> {
        let quiet_for = now - self.last_fix;
        if !self.underway || quiet_for < i64::from(self.settings.stop_after_minutes) * 60 {
            return Ok(Vec::new());
        }
        self.underway = false;
        self.slow_since = None;
        self.last = None;
        if let Some(track) = self.track.take() {
            track.save()?;
            if !track.is_empty() {
                self.day.add_track(&track.relative);
            }
        }
        let wrote = self.say(now, "no fix for a while; the passage is closed".into())?;
        self.commit(&format!("log: {} · passage closed", self.day.date));
        Ok(wrote)
    }

    /// Close the day at local midnight and start the next one. The day is
    /// this machine's.
    ///
    /// Going back a single day is skew around midnight, and following it would
    /// write the same hours into two notes. A bigger step back is a clock that
    /// was plainly wrong — set at boot, or stepped by NTP — and has to be
    /// followed, or the log would stay on the wrong day for good.
    fn roll_over(&mut self, at: i64) -> io::Result<Vec<String>> {
        let date = time::local(at).date();
        if date == self.day.date {
            return Ok(Vec::new());
        }
        let step_back = match (time::day_index(&self.day.date), time::day_index(&date)) {
            (Some(was), Some(now)) => was - now,
            _ => 0,
        };
        if step_back == 1 {
            return Ok(Vec::new());
        }
        if let Some(track) = self.track.take() {
            track.save()?;
            if !track.is_empty() {
                self.day.add_track(&track.relative);
            }
        }
        // A day with nothing in it leaves no note behind.
        if !self.day.is_empty() {
            self.save()?;
            self.commit(&format!("log: {}", self.day.date));
        }
        self.day = Day::open(&self.vault, &date, &self.settings.boat)?;
        self.last_entry = 0;
        self.last_point = 0;
        if self.underway {
            self.track = Some(Track::start(&self.vault, at, &self.settings.boat));
        }
        Ok(vec![format!("{date} · a new day")])
    }

    fn say(&mut self, at: i64, text: String) -> io::Result<Vec<String>> {
        let line = entry::event(time::local(at), time::utc(at), &text);
        self.log(at, line)
    }

    fn log(&mut self, _at: i64, line: String) -> io::Result<Vec<String>> {
        self.day.push(line.clone());
        self.save()?;
        Ok(vec![line])
    }

    fn save(&mut self) -> io::Result<()> {
        let _lock = lock::Lock::take(&self.vault)?;
        self.last_save = time::now();
        self.day.save()
    }

    /// Commit, when the vault is a repo and the settings allow it. A failure
    /// is reported once and never stops the log.
    pub fn commit(&self, message: &str) {
        if !self.settings.git {
            return;
        }
        match git::commit(&self.vault, message) {
            Ok(true) => {
                if let Err(e) = git::push(&self.vault) {
                    eprintln!("omalogbook: could not push ({e}); the log is safe on disk");
                }
            }
            Ok(false) => {}
            Err(e) => eprintln!("omalogbook: could not commit ({e}); the log is safe on disk"),
        }
    }

    /// Write and commit everything before stopping.
    pub fn close(&mut self) -> io::Result<()> {
        // A fix lost moments before the log stopped still gets its line.
        if self.outage.as_ref().is_some_and(|o| !o.written) {
            self.write_outage()?;
        }
        if let Some(track) = self.track.take() {
            track.save()?;
            if !track.is_empty() {
                self.day.add_track(&track.relative);
            }
        }
        if self.day.is_empty() {
            return Ok(());
        }
        self.save()?;
        self.commit(&format!("log: {}", self.day.date));
        Ok(())
    }

    pub fn day_path(&self) -> &std::path::Path {
        &self.day.path
    }
}

/// `7 s`, `29 min`, `1 h 5 min`.
fn gone_for(secs: i64) -> String {
    let secs = secs.max(0);
    match secs {
        0..60 => format!("{secs} s"),
        60..3600 => format!("{} min", secs / 60),
        _ => format!("{} h {} min", secs / 3600, secs % 3600 / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn settings(name: &str) -> Settings {
        let vault =
            std::env::temp_dir().join(format!("omalogbook-watch-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&vault);
        Settings {
            vault,
            boat: "Dash".into(),
            every_minutes: 60,
            underway_kn: 1.0,
            stopped_kn: 0.5,
            stop_after_minutes: 5,
            point_seconds: 10,
            git: false,
        }
    }

    fn fix(lat: f64, lon: f64, sog: f64, at: i64) -> keel::Update {
        keel::Update::Fix(
            keel::Fix {
                lat,
                lon,
                sog_kn: Some(sog),
                cog_deg: Some(245.0),
                utc: Some(at),
                satellites: Some(9),
                hdop: Some(0.9),
            },
            Vec::new(),
        )
    }

    /// A short sail: away from the berth, half an hour out, then still.
    fn sail(watch: &mut Watch, start: i64) {
        watch
            .update(fix(37.8663, -122.3148, 0.1, start), start)
            .unwrap();
        for step in 1..=180 {
            let at = start + step * 10;
            watch
                .update(fix(37.8663 + step as f64 * 0.0005, -122.3148, 5.0, at), at)
                .unwrap();
        }
        for step in 1..=40 {
            let at = start + 1800 + step * 10;
            watch.update(fix(37.9563, -122.3148, 0.1, at), at).unwrap();
        }
    }

    #[test]
    fn writes_a_days_log_from_a_sail() {
        let s = settings("sail");
        let vault = s.vault.clone();
        let mut watch = Watch::new(s).unwrap();
        sail(&mut watch, 1_789_300_800);
        watch.close().unwrap();

        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(text.contains("log opened"), "{text}");
        assert!(text.contains("under way"), "{text}");
        assert!(text.contains("stopped"), "{text}");
        assert!(
            text.contains("**Day's run** 5."),
            "day's run wrong:\n{text}"
        );

        let tracks: Vec<_> = fs::read_dir(vault.join("tracks"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(tracks.len(), 1);
        let gpx = fs::read_to_string(tracks[0].path()).unwrap();
        assert!(gpx.contains("<trkpt"));
        assert!(
            text.contains("tracks/"),
            "the day should link its track:\n{text}"
        );
    }

    #[test]
    fn a_passage_survives_a_dropped_fix() {
        let s = settings("dropout");
        let mut watch = Watch::new(s).unwrap();
        let start = 1_789_300_800;
        watch
            .update(fix(37.8663, -122.3148, 5.0, start), start)
            .unwrap();
        watch.update(keel::Update::Lost, start + 30).unwrap();
        watch
            .update(fix(37.8700, -122.3148, 5.0, start + 60), start + 60)
            .unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(text.contains("no fix"));
        assert!(text.contains("fix again"));
        assert!(
            !text.contains("stopped"),
            "a dropout is not the end of a passage"
        );
    }

    #[test]
    fn a_long_gap_is_not_counted_as_sailing() {
        let s = settings("gap");
        let mut watch = Watch::new(s).unwrap();
        let start = 1_789_300_800;
        watch
            .update(fix(37.8663, -122.3148, 5.0, start), start)
            .unwrap();
        // An hour later, 5 nm away: the log was off, so neither counts.
        let later = start + 3600;
        watch
            .update(fix(37.9500, -122.3148, 5.0, later), later)
            .unwrap();
        watch.close().unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(text.contains("**Day's run** 0.0 nm"), "{text}");
        assert!(text.contains("**under way** 0 min"), "{text}");
    }

    #[test]
    fn a_receiver_days_out_does_not_move_the_log() {
        let s = settings("clock");
        let vault = s.vault.clone();
        let mut watch = Watch::new(s).unwrap();
        let now = time::now();
        let today = watch.day_path().to_path_buf();
        // A replayed sail, or a receiver with the wrong week: five days back.
        let stale = now - 5 * 86_400;
        watch
            .update(fix(37.8663, -122.3148, 5.0, stale), now)
            .unwrap();
        assert_eq!(watch.day_path(), today, "the log jumped to another day");
        let text = fs::read_to_string(&today).unwrap();
        assert!(text.contains("5.0 days behind"), "{text}");
        assert!(text.contains("under way"));
        // Said once, not on every fix.
        watch
            .update(fix(37.8700, -122.3148, 5.0, stale + 10), now + 10)
            .unwrap();
        let text = fs::read_to_string(&today).unwrap();
        assert_eq!(text.matches("days behind").count(), 1, "{text}");
        let _ = fs::remove_dir_all(&vault);
    }

    #[test]
    fn a_receiver_in_step_keeps_its_own_time() {
        let s = settings("in-step");
        let mut watch = Watch::new(s).unwrap();
        let now = time::now();
        watch
            .update(fix(37.8663, -122.3148, 5.0, now - 30), now)
            .unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(!text.contains("logging by this machine's clock"), "{text}");
    }

    #[test]
    fn anchor_jitter_does_not_hold_a_passage_open() {
        let s = settings("jitter");
        let mut watch = Watch::new(s).unwrap();
        let start = time::now();
        watch
            .update(fix(37.8663, -122.3148, 5.0, start), start)
            .unwrap();
        // Swinging at anchor: mostly still, with the odd 0.8 kn sample.
        for step in 1..=60 {
            let at = start + step * 10;
            let speed = if step % 7 == 0 { 0.8 } else { 0.1 };
            watch
                .update(fix(37.8663, -122.3148, speed, at), at)
                .unwrap();
        }
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(text.contains("stopped"), "the passage never ended:\n{text}");
    }

    #[test]
    fn a_passage_does_not_outlive_the_fix() {
        let s = settings("abandoned");
        let mut watch = Watch::new(s).unwrap();
        let start = time::now();
        watch
            .update(fix(37.8663, -122.3148, 5.0, start), start)
            .unwrap();
        watch.update(keel::Update::Lost, start + 60).unwrap();
        watch.update(keel::Update::Lost, start + 400).unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(text.contains("the passage is closed"), "{text}");
        assert!(
            text.contains("tracks/"),
            "the track should still be linked:\n{text}"
        );
    }

    #[test]
    fn an_unknown_protocol_is_said_once() {
        let s = settings("protocol");
        let mut watch = Watch::new(s).unwrap();
        let now = time::now();
        for step in 0..20 {
            watch
                .update(keel::Update::Incompatible(2), now + step)
                .unwrap();
        }
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert_eq!(text.matches("speaks protocol 2").count(), 1, "{text}");
    }

    #[test]
    fn a_receiver_out_of_step_does_not_close_the_passage() {
        let s = settings("skew");
        let mut watch = Watch::new(s).unwrap();
        let now = time::now();
        // Twenty-five minutes behind, inside the hour the log tolerates.
        let receiver = now - 1500;
        watch
            .update(fix(37.8663, -122.3148, 5.0, receiver), now)
            .unwrap();
        // A ten-second dropout is not the end of a passage.
        watch.update(keel::Update::Lost, now + 10).unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(!text.contains("the passage is closed"), "{text}");
    }

    #[test]
    fn a_receiver_out_of_step_does_not_flip_the_day() {
        let s = settings("flip");
        let vault = s.vault.clone();
        let mut watch = Watch::new(s).unwrap();
        let midnight = {
            let now = time::now();
            let l = time::local(now);
            now - i64::from(l.hour) * 3600 - i64::from(l.minute) * 60 - i64::from(l.second) + 86_400
        };
        // The receiver is half an hour behind as the day turns.
        for step in 0..10 {
            let now = midnight - 300 + step * 120;
            watch
                .update(fix(37.8663, -122.3148, 5.0, now - 1800), now)
                .unwrap();
            watch.update(keel::Update::Lost, now + 1).unwrap();
        }
        let days: Vec<_> = fs::read_dir(vault.join("2026").join("09"))
            .map(|d| d.flatten().collect())
            .unwrap_or_default();
        assert!(days.len() <= 2, "the log flipped between days: {days:?}");
        let tracks: Vec<_> = fs::read_dir(vault.join("tracks"))
            .map(|d| d.flatten().collect())
            .unwrap_or_default();
        assert!(
            tracks.len() <= 2,
            "one passage made {} tracks",
            tracks.len()
        );
        let _ = fs::remove_dir_all(&vault);
    }

    #[test]
    fn a_clock_set_wrong_at_boot_is_followed_later() {
        let s = settings("stepped");
        let vault = s.vault.clone();
        let mut watch = Watch::new(s).unwrap();
        // Anchored to midday rather than to whatever time the tests are run.
        // This scenario steps an hour, and an hour after 23:30 is tomorrow —
        // the day would move for an honest reason and the check below would
        // read it as the log flipping.
        let now = time::now();
        let l = time::local(now);
        let midday =
            now - i64::from(l.hour) * 3600 - i64::from(l.minute) * 60 - i64::from(l.second)
                + 12 * 3600;
        // The machine boots a week ahead, then is corrected.
        let ahead = midday + 7 * 86_400;
        watch
            .update(fix(37.8663, -122.3148, 5.0, ahead), ahead)
            .unwrap();
        let wrong_day = watch.day_path().to_path_buf();
        // Skew inside a day is ignored...
        let soon = ahead + 3600;
        watch
            .update(fix(37.8663, -122.3148, 5.0, soon - 90_000), soon)
            .unwrap();
        assert_eq!(watch.day_path(), wrong_day);
        // ...but a day later on this clock, the correction is followed.
        let corrected = ahead + 86_401;
        watch
            .update(
                fix(37.8663, -122.3148, 5.0, corrected - 8 * 86_400),
                corrected - 8 * 86_400,
            )
            .unwrap();
        assert_ne!(
            watch.day_path(),
            wrong_day,
            "the log is stuck on a wrong day"
        );
        let _ = fs::remove_dir_all(&vault);
    }

    #[test]
    fn an_entry_is_stamped_with_the_day_it_is_filed_under() {
        let s = settings("stamp-day");
        let mut watch = Watch::new(s).unwrap();
        let now = time::now();
        let l = time::local(now);
        let midnight =
            now - i64::from(l.hour) * 3600 - i64::from(l.minute) * 60 - i64::from(l.second)
                + 86_400;
        // Just past midnight here; the receiver is still on yesterday.
        let at = midnight + 60;
        watch
            .update(fix(37.8663, -122.3148, 5.0, at - 3000), at)
            .unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(
            text.contains("**00:01**"),
            "yesterday's clock in today's note:\n{text}"
        );
    }

    #[test]
    fn midnight_starts_a_new_note() {
        let s = settings("midnight");
        let vault = s.vault.clone();
        let mut watch = Watch::new(s).unwrap();
        // 23:59:30 local and a minute later, whatever the machine's zone.
        let midnight = {
            let now = time::now();
            let l = time::local(now);
            now - i64::from(l.hour) * 3600 - i64::from(l.minute) * 60 - i64::from(l.second) + 86_400
        };
        watch
            .update(fix(37.8663, -122.3148, 5.0, midnight - 30), midnight - 30)
            .unwrap();
        let first = watch.day_path().to_path_buf();
        watch
            .update(fix(37.8700, -122.3148, 5.0, midnight + 30), midnight + 30)
            .unwrap();
        watch.close().unwrap();
        assert_ne!(first, watch.day_path());
        assert!(first.exists() && watch.day_path().exists());
        let _ = fs::remove_dir_all(&vault);
    }

    fn link(state: &'static str, message: Option<&str>) -> keel::Update {
        keel::Update::NoFix(keel::Why::Links(vec![keel::Link {
            name: "tcp:10.0.2.2:10110".into(),
            state,
            message: message.map(String::from),
        }]))
    }

    /// Under way, then the fix goes: the watch as the next few seconds see it.
    fn under_way(name: &str) -> (Watch, i64) {
        let mut watch = Watch::new(settings(name)).unwrap();
        let start = 1_789_300_800;
        for step in 0..3 {
            let at = start + step * 10;
            watch
                .update(fix(37.8663 + step as f64 * 0.0005, -122.3148, 5.0, at), at)
                .unwrap();
        }
        (watch, start + 20)
    }

    #[test]
    fn a_dropout_says_why_and_for_how_long() {
        let (mut watch, t) = under_way("why");
        let refused = Some("Connection refused (os error 111)");
        for second in 1..=6 {
            watch.update(link("down", refused), t + second).unwrap();
        }
        watch
            .update(fix(37.8700, -122.3148, 5.0, t + 8), t + 8)
            .unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(
            text.contains(
                "no fix: tcp:10.0.2.2:10110 is down (Connection refused (os error 111)) · last fix 9 satellites, HDOP 0.9"
            ),
            "{text}"
        );
        // From the last good fix at t, not from when the loss was noticed.
        assert!(text.contains("fix again after 8 s"), "{text}");
        assert_eq!(text.matches("no fix").count(), 1, "{text}");
    }

    /// A broken link turns the fix stale a moment after the source goes, or
    /// before. The line names what was wrong once things settled.
    #[test]
    fn the_reason_is_the_one_that_settled() {
        let (mut watch, t) = under_way("settle");
        watch
            .update(keel::Update::NoFix(keel::Why::Silent), t + 1)
            .unwrap();
        watch.update(link("quiet", None), t + 2).unwrap();
        watch.update(link("quiet", None), t + 4).unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(
            text.contains("no fix: tcp:10.0.2.2:10110 is connected but sending nothing"),
            "{text}"
        );
        assert!(!text.contains("sends no position"), "{text}");
    }

    /// A fix back inside the settling time still leaves both lines.
    #[test]
    fn a_blink_is_still_logged() {
        let (mut watch, t) = under_way("blink");
        watch
            .update(keel::Update::NoFix(keel::Why::Receiver), t + 1)
            .unwrap();
        watch
            .update(fix(37.8700, -122.3148, 5.0, t + 2), t + 2)
            .unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(text.contains("no fix: the receiver has no fix"), "{text}");
        assert!(text.contains("fix again after 2 s"), "{text}");
    }

    #[test]
    fn a_retrying_link_writes_one_line_and_a_change_writes_another() {
        let (mut watch, t) = under_way("retry");
        for second in 1..=60 {
            let message = (second % 2 == 0).then_some("Connection refused");
            watch.update(link("down", message), t + second).unwrap();
        }
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert_eq!(text.matches("no fix").count(), 1, "{text}");
        watch.update(keel::Update::Lost, t + 62).unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(
            text.contains("still no fix: omakeel, the hub, isn't answering"),
            "{text}"
        );
    }

    #[test]
    fn a_link_that_keeps_changing_its_mind_is_capped() {
        let (mut watch, t) = under_way("flap");
        for second in 1..=100 {
            let update = if second % 2 == 0 {
                link("quiet", None)
            } else {
                link("down", None)
            };
            watch.update(update, t + second).unwrap();
        }
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert_eq!(
            text.matches("still no fix").count(),
            usize::from(MAX_REASONS),
            "{text}"
        );
    }

    #[test]
    fn a_fix_lost_as_the_log_stops_is_still_written() {
        let (mut watch, t) = under_way("closing");
        watch.update(link("down", None), t + 1).unwrap();
        watch.close().unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(
            text.contains("no fix: tcp:10.0.2.2:10110 is down"),
            "{text}"
        );
    }

    #[test]
    fn how_long_reads_plainly() {
        assert_eq!(gone_for(7), "7 s");
        assert_eq!(gone_for(29 * 60 + 5), "29 min");
        assert_eq!(gone_for(3900), "1 h 5 min");
        assert_eq!(gone_for(-3), "0 s");
    }

    fn source(name: &str, state: &'static str) -> keel::Link {
        keel::Link {
            name: name.into(),
            state,
            message: None,
        }
    }

    /// An AIS receiver on an empty bay goes quiet and back all day. It was
    /// quiet while the fix was good, so it is no reason for the fix to go,
    /// and its comings and goings don't spend the outage's lines.
    #[test]
    fn an_idle_ais_receiver_is_not_blamed() {
        let mut watch = Watch::new(settings("ais")).unwrap();
        let t = 1_789_300_800;
        let ais = "serial:/dev/ais:38400";
        let good = |at: i64| {
            keel::Update::Fix(
                keel::Fix {
                    lat: 37.8663,
                    lon: -122.3148,
                    sog_kn: Some(5.0),
                    cog_deg: Some(245.0),
                    utc: Some(at),
                    satellites: Some(9),
                    hdop: Some(0.9),
                },
                vec![ais.to_string()],
            )
        };
        watch.update(good(t), t).unwrap();
        for second in 1..=40 {
            let mut links = vec![source("tcp:gps:10110", "down")];
            if second % 7 < 3 {
                links.push(source(ais, "quiet"));
            }
            watch
                .update(keel::Update::NoFix(keel::Why::Links(links)), t + second)
                .unwrap();
        }
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(text.contains("no fix: tcp:gps:10110 is down ·"), "{text}");
        assert!(!text.contains(ais), "{text}");
        assert!(!text.contains("still no fix"), "{text}");

        // The GPS talking but sending no position, with the AIS quiet beside
        // it, is the receiver's doing, not the AIS's.
        watch.update(good(t + 50), t + 50).unwrap();
        for second in 51..=55 {
            watch
                .update(
                    keel::Update::NoFix(keel::Why::Links(vec![source(ais, "quiet")])),
                    t + second,
                )
                .unwrap();
        }
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(
            text.contains("no fix: the receiver is talking but sends no position"),
            "{text}"
        );
    }

    #[test]
    fn a_fix_lost_before_midnight_is_in_that_days_note() {
        let s = settings("late");
        let vault = s.vault.clone();
        let mut watch = Watch::new(s).unwrap();
        let midnight = {
            let now = time::now();
            let l = time::local(now);
            now - i64::from(l.hour) * 3600 - i64::from(l.minute) * 60 - i64::from(l.second) + 86_400
        };
        watch
            .update(fix(37.8663, -122.3148, 5.0, midnight - 10), midnight - 10)
            .unwrap();
        let before = watch.day_path().to_path_buf();
        watch.update(link("down", None), midnight - 2).unwrap();
        watch.update(link("down", None), midnight + 2).unwrap();
        let old = fs::read_to_string(&before).unwrap();
        assert!(old.contains("no fix: tcp:10.0.2.2:10110 is down"), "{old}");
        let new = fs::read_to_string(watch.day_path()).unwrap_or_default();
        assert!(!new.contains("no fix:"), "{new}");
        let _ = fs::remove_dir_all(&vault);
    }

    /// Past the cap, the hub going is still written, and the fix coming
    /// back says what it came back from.
    #[test]
    fn past_the_cap_the_hub_and_the_last_reason_still_show() {
        let (mut watch, t) = under_way("capped");
        for second in 1..=20 {
            let update = if second % 2 == 0 {
                link("quiet", None)
            } else {
                link("down", None)
            };
            watch.update(update, t + second).unwrap();
        }
        watch.update(keel::Update::Lost, t + 21).unwrap();
        watch.update(link("quiet", None), t + 23).unwrap();
        watch
            .update(fix(37.8700, -122.3148, 5.0, t + 25), t + 25)
            .unwrap();
        let text = fs::read_to_string(watch.day_path()).unwrap();
        assert!(
            text.contains("still no fix: omakeel, the hub, isn't answering"),
            "{text}"
        );
        assert!(
            text.contains(
                "fix again after 25 s · last: tcp:10.0.2.2:10110 is connected but sending nothing"
            ),
            "{text}"
        );
    }
}
