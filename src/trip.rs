//! The trip, for the `/berth` mark: from the last `/depart` to now, worked
//! out from what is on disk — the day notes and the GPX tracks — so it reads
//! the same whether or not the log was running the whole way.
//!
//! `trip 20.2 nm in 4 h 25 min (2.5 inferred) · under way 4 h 12 min
//!  · avg 4.8 kn · top 7.8 kn · sail 3 h 40 min, motor 32 min`
//!
//! The track is only what the receiver really saw. Where the fixes stop for
//! a while the boat still went somewhere, and the straight line across the
//! hole is counted, but said apart as inferred: a total that quietly passed
//! a guess off as track would be one nobody could trust later.

use crate::{day, geo, time};
use std::{fs, io::Read, path::Path};

/// How far back a `/depart` is looked for. A month covers any passage this
/// boat will make before the log grows a proper voyage mark.
const MAX_DAYS: i64 = 30;

/// A gap in the fixes at least this long is a hole: its miles are inferred.
/// The same threshold omahelm draws holes at.
const HOLE_SECS: i64 = 90;

/// Nothing a day note or a track should ever reach. A bigger file is not
/// omalogbook's, and is left unread rather than read into memory.
const MAX_NOTE_BYTES: u64 = 4 << 20;
const MAX_TRACK_BYTES: u64 = 32 << 20;
const MAX_TRACK_FILES: usize = 2_000;

/// A GPS speed past this is a glitch, not the boat.
const MAX_SOG_KN: f64 = 60.0;

/// One mark from a day note, with the moment it was made.
#[derive(Clone, Debug, PartialEq)]
struct Mark {
    epoch: i64,
    label: String,
}

/// One point off a GPX track.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    epoch: i64,
    lat: f64,
    lon: f64,
    sog_kn: Option<f64>,
}

/// The labels a trip is built from, as the presets write them.
const DEPARTED: &str = "Departed";
const UNDER: [(&str, Leg); 2] = [("Sailing", Leg::Sail), ("Motoring", Leg::Motor)];
/// Marks that end a trip. Found before a `Departed`, looking back, they
/// mean there is no trip open to sum up.
const ENDED: [&str; 2] = ["Berthed", "Moored"];
/// Marks that end a leg under sail or engine without starting another.
const STILL: [&str; 3] = ["Anchor down", "Moored", "Berthed"];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Leg {
    Sail,
    Motor,
}

/// The trip's figures, worded, one part per item, for the mark's line. Empty
/// when there is no `/depart` to measure from.
pub fn summary(vault: &Path, now: i64, underway_kn: f64) -> Vec<String> {
    let marks = marks_since_departure(vault, now);
    let Some(depart) = marks.first().map(|m| m.epoch) else {
        return Vec::new();
    };
    let points = points_between(vault, depart, now);
    words(depart, now, &marks, &points, underway_kn)
}

/// The latest `Departed` mark and every mark after it up to `now`, oldest
/// first. Empty when there is no departure in the last month, or when the
/// boat was already berthed or moored since the last one: that trip is
/// over, and this one never had its `/depart`.
fn marks_since_departure(vault: &Path, now: i64) -> Vec<Mark> {
    let mut later: Vec<Mark> = Vec::new();
    let today = time::local(now).date();
    let Some(today_index) = time::day_index(&today) else {
        return Vec::new();
    };
    for back in 0..=MAX_DAYS {
        let date = time::utc((today_index - back) * 86_400).date();
        let mut marks = marks_on(vault, &date);
        marks.retain(|m| m.epoch <= now);
        // Newest first, so the first `Departed` found is the latest.
        marks.sort_by_key(|m| std::cmp::Reverse(m.epoch));
        for m in marks {
            if ENDED.contains(&m.label.as_str()) {
                return Vec::new();
            }
            let departed = m.label == DEPARTED;
            later.push(m);
            if departed {
                later.reverse();
                return later;
            }
        }
    }
    Vec::new()
}

/// Every mark in one day's note that nobody has struck through.
fn marks_on(vault: &Path, date: &str) -> Vec<Mark> {
    let Some(text) = read_bounded(&day::path_for(vault, date), MAX_NOTE_BYTES) else {
        return Vec::new();
    };
    day::entries_in(&text)
        .iter()
        .filter(|line| !day::is_struck(line))
        .filter_map(|line| {
            let epoch = epoch_of(date, line)?;
            let label = label_of(line)?;
            Some(Mark { epoch, label })
        })
        .collect()
}

/// The mark an entry is, if it is one: the preset's label that opens what
/// follows the em dash, `Departed` in `— Departed: 2 aboard · wind …`.
fn label_of(line: &str) -> Option<String> {
    let (_, said) = line.split_once(" — ")?;
    crate::preset::PRESETS
        .iter()
        .map(|p| p.label)
        .find(|label| {
            said.strip_prefix(label).is_some_and(|rest| {
                rest.is_empty() || rest.starts_with(':') || rest.starts_with(" ·")
            })
        })
        .map(str::to_string)
}

/// When an entry was made, from its two clocks: `- **12:04** (19:04 UTC)`.
/// The local clock names the minute on the note's own day, and the gap
/// between the two clocks is the zone it was written in, summer time and all.
fn epoch_of(date: &str, line: &str) -> Option<i64> {
    let rest = line.strip_prefix("- **")?;
    let local = minutes(rest.get(..5)?)?;
    let after = rest.get(5..)?.strip_prefix("**")?;
    let midnight = time::parse_utc(&format!("{date}T00:00:00Z"))?;
    let naive = midnight + local * 60;
    let offset_minutes = if after.starts_with(" UTC") {
        0
    } else if let Some(utc) = after.strip_prefix(" (").and_then(|s| s.get(..5)) {
        // The clocks give the zone only up to a whole day: Hawaii at -10
        // and Kiritimati at +14 show the same pair. Of the zones that fit
        // (12 hours behind to 14 ahead), take the one nearest this machine's
        // at that moment, which is the zone the entry was almost certainly
        // written in, and is only a tie-breaker when it wasn't.
        let gap = local - minutes(utc)?;
        let here = time::local(naive).offset / 60;
        [gap - 1440, gap, gap + 1440]
            .into_iter()
            .filter(|g| (-12 * 60..=14 * 60).contains(g))
            .min_by_key(|g| (g - here).abs())?
    } else {
        // An entry with one clock, written by hand: take the zone as it
        // stands here.
        time::local(naive).offset / 60
    };
    Some(naive - offset_minutes * 60)
}

/// `12:04` → 724.
fn minutes(clock: &str) -> Option<i64> {
    let (h, m) = clock.split_once(':')?;
    let (h, m) = (h.parse::<i64>().ok()?, m.parse::<i64>().ok()?);
    ((0..24).contains(&h) && (0..60).contains(&m) && clock.len() == 5).then_some(h * 60 + m)
}

fn read_bounded(path: &Path, max: u64) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > max {
        return None;
    }
    let mut text = String::new();
    file.take(max).read_to_string(&mut text).ok()?;
    Some(text)
}

/// Every track point from `from` to `to`, in order, from the tracks named
/// for the days in between. The track still being written counts: it is on
/// disk from its first point, before the note lists it.
fn points_between(vault: &Path, from: i64, to: i64) -> Vec<Point> {
    let (Some(first), Some(last)) = (
        time::day_index(&time::local(from).date()),
        time::day_index(&time::local(to).date()),
    ) else {
        return Vec::new();
    };
    let Ok(dir) = fs::read_dir(vault.join("tracks")) else {
        return Vec::new();
    };
    let mut files: Vec<_> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "gpx"))
        .filter(|p| {
            // `2026-09-21-114330.gpx`, named for the local day it started.
            // A passage can start the day before the departure mark and run
            // past midnight, so one day either side is read too.
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.get(..10))
                .and_then(time::day_index)
                .is_some_and(|d| (first - 1..=last).contains(&d))
        })
        .collect();
    files.sort();
    files.truncate(MAX_TRACK_FILES);
    let mut points: Vec<Point> = files
        .iter()
        .filter_map(|p| read_bounded(p, MAX_TRACK_BYTES))
        .flat_map(|text| gpx_points(&text))
        .filter(|p| (from..=to).contains(&p.epoch))
        .collect();
    points.sort_by_key(|p| p.epoch);
    points.dedup_by_key(|p| p.epoch);
    points
}

/// The points of a GPX track as omalogbook writes it: `<trkpt lat lon>`
/// with a `<time>` and, when the receiver gave one, a `<speed>` in meters
/// per second.
fn gpx_points(text: &str) -> Vec<Point> {
    text.split("<trkpt")
        .skip(1)
        .filter_map(|chunk| {
            let chunk = &chunk[..chunk.find("</trkpt>").unwrap_or(chunk.len())];
            let attr = |name: &str| -> Option<f64> {
                let key = format!("{name}=\"");
                let start = chunk.find(&key)? + key.len();
                let end = start + chunk[start..].find('"')?;
                chunk[start..end]
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
            };
            let element = |name: &str| -> Option<&str> {
                let open = format!("<{name}>");
                let start = chunk.find(&open)? + open.len();
                let end = start + chunk[start..].find(&format!("</{name}>"))?;
                Some(chunk[start..end].trim())
            };
            let lat = attr("lat").filter(|v| (-90.0..=90.0).contains(v))?;
            let lon = attr("lon").filter(|v| (-180.0..=180.0).contains(v))?;
            let epoch = time::parse_utc(element("time")?)?;
            let sog_kn = element("speed")
                .and_then(|s| s.parse::<f64>().ok())
                .map(|ms| ms / 0.514_444)
                .filter(|kn| kn.is_finite() && (0.0..=MAX_SOG_KN).contains(kn));
            Some(Point {
                epoch,
                lat,
                lon,
                sog_kn,
            })
        })
        .collect()
}

/// The trip in words.
fn words(depart: i64, now: i64, marks: &[Mark], points: &[Point], underway_kn: f64) -> Vec<String> {
    let mut track_nm = 0.0;
    let mut inferred_nm = 0.0;
    let mut underway_secs = 0;
    let mut underway_inferred_nm = 0.0;
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let secs = b.epoch - a.epoch;
        let nm = geo::distance_nm((a.lat, a.lon), (b.lat, b.lon));
        if !nm.is_finite() {
            continue;
        }
        if secs < HOLE_SECS {
            track_nm += nm;
            underway_secs += secs;
        } else {
            // A hole the boat plainly crossed — it came out the other side
            // somewhere else, at a speed a boat under way makes — was spent
            // under way. One it came out of where it went in was not.
            inferred_nm += nm;
            if nm / (secs as f64 / 3600.0) >= underway_kn {
                underway_secs += secs;
                underway_inferred_nm += nm;
            }
        }
    }
    let total_nm = track_nm + inferred_nm;
    // The average is miles over the time they took. A hole that didn't
    // count as time under way can't lend the average its miles either, or
    // a slow drift across a long dropout reads as a sprint.
    let timed_nm = track_nm + underway_inferred_nm;
    let mut out = Vec::new();
    let elapsed = day::hours((now - depart).max(0));
    if points.len() >= 2 {
        let inferred = if inferred_nm >= 0.05 {
            format!(" ({inferred_nm:.1} inferred)")
        } else {
            String::new()
        };
        out.push(format!("trip {total_nm:.1} nm in {elapsed}{inferred}"));
        if underway_secs > 0 {
            out.push(format!("under way {}", day::hours(underway_secs)));
            out.push(format!(
                "avg {:.1} kn",
                timed_nm / (underway_secs as f64 / 3600.0)
            ));
        }
    } else {
        out.push(format!("trip {elapsed}, no track"));
    }
    if let Some(top) = points
        .iter()
        .filter_map(|p| p.sog_kn)
        .fold(None, |best: Option<f64>, kn| {
            Some(best.map_or(kn, |b| b.max(kn)))
        })
    {
        out.push(format!("top {top:.1} kn"));
    }
    if let Some(legs) = legs(marks, now) {
        out.push(legs);
    }
    out
}

/// `sail 3 h 40 min, motor 32 min`, from the `/sail` and `/motor` marks. A
/// leg runs from its mark to the next that changes how the boat is moving,
/// or to now. None when the crew marked neither.
fn legs(marks: &[Mark], now: i64) -> Option<String> {
    let mut sail = 0;
    let mut motor = 0;
    let mut on: Option<(Leg, i64)> = None;
    let mut marked = false;
    let mut close = |on: Option<(Leg, i64)>, at: i64| {
        if let Some((leg, since)) = on {
            let secs = (at - since).max(0);
            match leg {
                Leg::Sail => sail += secs,
                Leg::Motor => motor += secs,
            }
        }
    };
    for m in marks {
        if let Some((_, leg)) = UNDER.iter().find(|(label, _)| *label == m.label) {
            close(on, m.epoch);
            on = Some((*leg, m.epoch));
            marked = true;
        } else if STILL.contains(&m.label.as_str()) {
            close(on, m.epoch);
            on = None;
        }
    }
    close(on, now);
    if !marked {
        return None;
    }
    let mut parts = Vec::new();
    if sail > 0 {
        parts.push(format!("sail {}", day::hours(sail)));
    }
    if motor > 0 {
        parts.push(format!("motor {}", day::hours(motor)));
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: i64 = 1_789_300_800; // 2026-09-13T12:00:00Z

    fn vault(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("omalogbook-trip-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tracks")).unwrap();
        dir
    }

    /// A GPX track as omalogbook writes one: every ten seconds, heading
    /// north at `kn`, from `at` for `secs`.
    fn track(vault: &Path, at: i64, secs: i64, lat0: f64, kn: f64) -> f64 {
        let mut t = crate::track::Track::start(vault, at, "Dash");
        let mut lat = lat0;
        let mut when = at;
        while when <= at + secs {
            t.push(crate::track::Point {
                epoch: when,
                lat,
                lon: -122.3148,
                sog_kn: Some(kn),
            });
            lat += kn * 10.0 / 3600.0 / 60.0;
            when += 10;
        }
        t.save().unwrap();
        lat
    }

    /// A day note with these entries in omalogbook's block.
    fn note(vault: &Path, epoch: i64, lines: &[String]) {
        let date = time::local(epoch).date();
        let mut d = day::Day::open(vault, &date, "Dash").unwrap();
        for l in lines {
            d.push(l.clone());
        }
        d.save().unwrap();
    }

    fn mark_line(at: i64, words: &str) -> String {
        crate::entry::event(time::local(at), time::utc(at), words)
    }

    #[test]
    fn an_entrys_clocks_give_back_its_moment() {
        for at in [START, START + 7 * 3600 + 59, START + 86_400 * 100 + 1234] {
            let line = mark_line(at, "Departed");
            let date = time::local(at).date();
            assert_eq!(
                epoch_of(&date, &line),
                Some(at - at.rem_euclid(60)),
                "{line}"
            );
        }
        // Written at Greenwich, one clock and `UTC`.
        assert_eq!(
            epoch_of("2026-09-13", "- **12:00** UTC — Departed"),
            Some(START)
        );
        // Across midnight: 23:30 local is the next day at Greenwich.
        assert_eq!(
            epoch_of("2026-09-13", "- **23:30** (06:30 UTC) — Departed"),
            Some(START + 11 * 3600 + 30 * 60 + 7 * 3600)
        );
    }

    #[test]
    fn a_label_is_the_mark_it_opens_with() {
        assert_eq!(
            label_of("- **12:04** — Departed").as_deref(),
            Some("Departed")
        );
        assert_eq!(
            label_of("- **12:04** · 37°N — Departed: 2 aboard · wind 8 kn").as_deref(),
            Some("Departed")
        );
        assert_eq!(
            label_of("- **12:04** — Berthed · trip 2 nm").as_deref(),
            Some("Berthed")
        );
        // Prose that happens to start with a label's word is not a mark.
        assert_eq!(
            label_of("- **12:04** — Departedness is a state of mind"),
            None
        );
        assert_eq!(label_of("- **12:04** — We Departed late"), None);
    }

    /// Out, a 20-minute hole the boat sailed through, back, and berthed.
    #[test]
    fn a_trip_counts_its_track_and_says_what_it_inferred() {
        let v = vault("trip");
        let depart = START;
        note(
            &v,
            depart,
            &[
                mark_line(depart - 600, "Departed"), // an earlier false start
                mark_line(depart - 300, "Berthed"),
                mark_line(depart, "Departed: second try"),
                mark_line(depart + 60, "Sailing"),
                mark_line(depart + 3600, "Motoring"),
            ],
        );
        // An hour at 5 kn, a hole of 20 min, then 30 min more at 5 kn.
        let lat = track(&v, depart + 60, 3600, 37.8663, 5.0);
        let after = depart + 60 + 3600 + 1200;
        track(&v, after, 1800, lat + 5.0 * 1200.0 / 3600.0 / 60.0, 5.0);
        let now = after + 1800 + 60;

        let parts = summary(&v, now, 1.0);
        let line = parts.join(" · ");
        // 5 nm on the first track, 1.67 across the hole, 2.5 on the second.
        assert!(
            line.starts_with("trip 9.2 nm in 1 h 52 min (1.7 inferred)"),
            "{line}"
        );
        assert!(line.contains("under way 1 h 50 min"), "{line}");
        assert!(line.contains("avg 5.0 kn"), "{line}");
        assert!(line.contains("top 5.0 kn"), "{line}");
        assert!(line.ends_with("sail 59 min, motor 52 min"), "{line}");
    }

    #[test]
    fn a_struck_departure_does_not_count() {
        let v = vault("struck");
        note(
            &v,
            START,
            &[
                mark_line(START, "Departed"),
                mark_line(START + 600, "Departed"),
            ],
        );
        // Strike the second, as the window's STRIKE does.
        let date = time::local(START).date();
        let mut d = day::Day::open(&v, &date, "Dash").unwrap();
        let second = mark_line(START + 600, "Departed");
        d.revise(1, &second, &day::Revision::Strike).unwrap();
        let marks = marks_since_departure(&v, START + 3600);
        assert_eq!(marks.first().map(|m| m.epoch), Some(START), "{marks:?}");
    }

    /// Yesterday's trip ended alongside; today the crew sailed without a
    /// `/depart` and typed `/berth`. Summing back to yesterday's departure
    /// would be a 26-hour trip that never happened.
    #[test]
    fn a_trip_already_berthed_is_not_summed_again() {
        let v = vault("reberth");
        note(
            &v,
            START,
            &[
                mark_line(START, "Departed"),
                mark_line(START + 3600, "Berthed"),
            ],
        );
        let tomorrow = START + 86_400;
        note(&v, tomorrow, &[mark_line(tomorrow, "Sailing")]);
        track(&v, tomorrow, 3600, 37.8663, 5.0);
        assert!(summary(&v, tomorrow + 3600, 1.0).is_empty());

        // A mooring ends a trip just the same.
        let v = vault("moored");
        note(
            &v,
            START,
            &[
                mark_line(START, "Departed"),
                mark_line(START + 3600, "Moored"),
            ],
        );
        assert!(summary(&v, START + 7200, 1.0).is_empty());
    }

    /// An hour sailing at 5 kn, then three hours of dropout that drifted
    /// half a mile: the drift isn't time under way, so it can't lend the
    /// average its miles either.
    #[test]
    fn the_average_only_counts_miles_whose_time_counted() {
        let a = Point {
            epoch: 0,
            lat: 37.0,
            lon: -122.0,
            sog_kn: Some(5.0),
        };
        let b = Point {
            epoch: 3600,
            lat: 37.0 + 5.0 / 60.0,
            ..a
        };
        // The hour as ten-second points, so it is track, not a hole.
        let mut points: Vec<Point> = (0..=360)
            .map(|i| Point {
                epoch: i * 10,
                lat: a.lat + (b.lat - a.lat) * i as f64 / 360.0,
                ..a
            })
            .collect();
        points.push(Point {
            epoch: 3600 + 3 * 3600,
            lat: b.lat + 0.5 / 60.0,
            ..a
        });
        let parts = words(0, 4 * 3600, &[], &points, 1.0);
        assert_eq!(parts[0], "trip 5.5 nm in 4 h 00 min (0.5 inferred)");
        assert_eq!(parts[1], "under way 1 h 00 min");
        assert_eq!(parts[2], "avg 5.0 kn");
    }

    #[test]
    fn no_departure_is_no_trip() {
        let v = vault("none");
        note(&v, START, &[mark_line(START, "Sailing")]);
        assert!(summary(&v, START + 3600, 1.0).is_empty());
    }

    /// A departure at 21:00 local, berthed at 03:00: the `Departed` is in
    /// yesterday's note and the `/berth` is written into today's.
    #[test]
    fn a_trip_can_run_past_midnight() {
        let v = vault("overnight");
        let depart = (0..24)
            .map(|h| START + h * 3600)
            .find(|t| time::local(*t).hour == 21)
            .expect("an hour that is 21:00 here");
        assert_ne!(
            time::local(depart).date(),
            time::local(depart + 6 * 3600).date()
        );
        note(&v, depart, &[mark_line(depart, "Departed")]);
        track(&v, depart, 6 * 3600, 37.8663, 4.0);
        let parts = summary(&v, depart + 6 * 3600, 1.0);
        assert!(
            parts[0].starts_with("trip 24.0 nm in 6 h 00 min"),
            "{parts:?}"
        );
    }

    #[test]
    fn departing_with_no_track_still_says_how_long() {
        let v = vault("notrack");
        note(&v, START, &[mark_line(START, "Departed")]);
        assert_eq!(
            summary(&v, START + 5400, 1.0),
            vec!["trip 1 h 30 min, no track"]
        );
    }

    /// A hole the boat came out of where it went in — the receiver dropped
    /// out at the berth — is neither miles nor time under way.
    #[test]
    fn a_hole_standing_still_is_not_time_under_way() {
        let a = Point {
            epoch: 0,
            lat: 37.0,
            lon: -122.0,
            sog_kn: Some(0.0),
        };
        let b = Point { epoch: 1800, ..a };
        let parts = words(0, 1800, &[], &[a, b], 1.0);
        assert_eq!(
            parts,
            vec!["trip 0.0 nm in 30 min".to_string(), "top 0.0 kn".into()]
        );
    }

    #[test]
    fn hostile_gpx_is_skipped() {
        let text = r#"<trkpt lat="91" lon="0"><time>2026-09-13T12:00:00Z</time></trkpt>
<trkpt lat="37" lon="-122"><time>not a time</time></trkpt>
<trkpt lat="37" lon="-122"><time>2026-09-13T12:00:10Z</time><extensions><speed>999</speed></extensions></trkpt>"#;
        let points = gpx_points(text);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].sog_kn, None);
    }
}
