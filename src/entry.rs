//! One line of the log. Local time leads, because that is the time the crew
//! kept; UTC follows when the boat is not on Greenwich.

use crate::{geo, time::Local};

/// A position line: `- **09:15** (16:15 UTC) · 37°52.0′N 122°18.9′W · 245° · 5.1 kn — under way`
pub fn fix(
    at: Local,
    utc: Local,
    lat: f64,
    lon: f64,
    sog: Option<f64>,
    cog: Option<f64>,
    note: &str,
) -> String {
    let mut line = format!(
        "- **{}**{} · {} {}",
        at.clock(),
        utc_suffix(at, utc),
        geo::latitude(lat),
        geo::longitude(lon)
    );
    let course = geo::course(cog);
    if !course.is_empty() {
        line.push_str(&format!(" · {course}"));
    }
    if let Some(kn) = sog.filter(|k| k.is_finite()) {
        line.push_str(&format!(" · {kn:.1} kn"));
    }
    if !note.is_empty() {
        line.push_str(&format!(" — {note}"));
    }
    line
}

/// An event with no position to give, such as losing the fix.
pub fn event(at: Local, utc: Local, text: &str) -> String {
    format!("- **{}**{} — {text}", at.clock(), utc_suffix(at, utc))
}

/// What was going on around the boat when a mark was made, gathered from
/// whichever engines are running. Any part may be missing.
#[derive(Clone, Debug, Default)]
pub struct Around {
    pub weather: crate::wind::Weather,
    pub tide: crate::tide::Tide,
    /// What only this mark says, already worded: the sunset on `/depart`,
    /// the trip's figures on `/berth`. They follow the crew's words.
    pub extras: Vec<String>,
}

/// A mark: what the crew said happened, where, and what it was blowing.
/// `- **14:32** (21:32 UTC) · 37°52.0′N 122°18.9′W · 245° · 5.1 kn — Departed
///  · sunset 19:05 · wind 13 kn from 262°, gusting 18 (HRRR) · 1014.5 hPa
///  · barometer 1013.8 hPa, falling 1.2 in 3 h (Alameda) · measured 11 kn
///  from 250° (Alameda, 12 min) · tide 1.37 m falling (Berkeley, 0.4 nm)
///  · ebb 1.4 kn toward 294° (Emeryville Marina, 1.5 nm)`
///
/// Course and speed belong here, unlike on a note: a mark is an event in the
/// day's run, and reads beside `under way` and `stopped`.
pub fn mark(
    at: Local,
    utc: Local,
    fix: Option<(f64, f64, Option<f64>, Option<f64>)>,
    label: &str,
    said: &str,
    around: &Around,
) -> String {
    let mut what = label.to_string();
    if !said.trim().is_empty() {
        what.push_str(": ");
        what.push_str(said.trim());
    }
    let mut line = match fix {
        Some((lat, lon, sog, cog)) => self::fix(at, utc, lat, lon, sog, cog, &what),
        None => event(at, utc, &what),
    };
    for extra in &around.extras {
        line.push_str(&format!(" · {extra}"));
    }
    let weather = &around.weather;
    if let Some(f) = weather.forecast {
        line.push_str(&format!(
            " · wind {} (HRRR)",
            blow(f.speed_kn, f.dir_deg, f.gust_kn)
        ));
        if let Some(hpa) = f.pressure_hpa {
            line.push_str(&format!(" · {hpa:.1} hPa"));
        }
    }
    if let Some(b) = &weather.barometer {
        line.push_str(&format!(" · {}", barometer(b)));
    }
    if let Some(m) = &weather.measured {
        line.push_str(&format!(
            " · measured {} ({}{})",
            blow(m.speed_kn, m.dir_deg, m.gust_kn),
            m.name,
            match m.age_minutes {
                Some(mins) if mins > 0 => format!(", {mins} min"),
                _ => String::new(),
            }
        ));
    }
    line.push_str(&crate::tide::words(&around.tide));
    line
}

/// `barometer 1013.8 hPa, falling 1.2 in 3 h (Alameda)`. A change that
/// rounds to nothing is steady, which is worth saying too.
fn barometer(b: &crate::wind::Barometer) -> String {
    let reading = b
        .pressure_hpa
        .map(|p| format!(" {p:.1} hPa,"))
        .unwrap_or_default();
    let change = if b.tendency_hpa.abs() < 0.05 {
        "steady over 3 h".to_string()
    } else {
        format!(
            "{} {:.1} in 3 h",
            if b.tendency_hpa > 0.0 {
                "rising"
            } else {
                "falling"
            },
            b.tendency_hpa.abs()
        )
    };
    format!("barometer{reading} {change} ({})", b.name)
}

/// `13 kn from 262°, gusting 18`. A calm has no direction to give, and a
/// gust is only worth saying when it is above the wind. The reading is
/// named by its caller — `wind …` for the model, `measured …` for an
/// anemometer — so the two can never be mistaken for each other.
fn blow(speed_kn: f64, dir_deg: Option<f64>, gust_kn: Option<f64>) -> String {
    let mut out = format!("{speed_kn:.0} kn");
    if let Some(deg) = dir_deg {
        out.push_str(&format!(" from {:03.0}°", deg));
    }
    if let Some(gust) = gust_kn.filter(|g| *g > speed_kn + 0.5) {
        out.push_str(&format!(", gusting {gust:.0}"));
    }
    out
}

/// The crew's own entry with the position it was written at:
/// `- **14:32** (21:32 UTC) · 37°52.0′N 122°18.9′W — Dolphins off the port side`
///
/// Course and speed are left off, though the fix carries them. They are what
/// the app writes in its own hourly lines, and a note that repeated them
/// would read as machine output rather than as the crew's words.
pub fn note_at(at: Local, utc: Local, lat: f64, lon: f64, text: &str) -> String {
    fix(at, utc, lat, lon, None, None, text.trim())
}

/// The crew's own entry, when there is no position to put it at.
pub fn note(at: Local, utc: Local, text: &str) -> String {
    format!(
        "- **{}**{} — {}",
        at.clock(),
        utc_suffix(at, utc),
        text.trim()
    )
}

fn utc_suffix(at: Local, utc: Local) -> String {
    if at.offset == 0 {
        " UTC".into()
    } else {
        format!(" ({} UTC)", utc.clock())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time;

    const NOON: i64 = 1_789_300_800; // 2026-09-13T12:00:00Z

    fn blowing() -> crate::wind::Weather {
        crate::wind::Weather {
            forecast: Some(crate::wind::Forecast {
                speed_kn: 12.6,
                dir_deg: Some(262.0),
                gust_kn: Some(17.9),
                pressure_hpa: Some(1014.6),
            }),
            measured: Some(crate::wind::Measured {
                name: "Alameda".into(),
                nm: 2.4,
                age_minutes: Some(12),
                speed_kn: 11.0,
                dir_deg: Some(250.0),
                gust_kn: None,
            }),
            barometer: None,
        }
    }

    /// Everything running: the extras lead, then the weather, then the water.
    #[test]
    fn a_mark_with_everything_running_reads_in_order() {
        let at = time::local(NOON);
        let mut weather = blowing();
        weather.barometer = Some(crate::wind::Barometer {
            name: "Alameda".into(),
            nm: 6.4,
            pressure_hpa: Some(1013.8),
            tendency_hpa: -1.2,
        });
        let tide = crate::tide::Tide {
            water: Some(crate::tide::Water {
                name: "Berkeley".into(),
                nm: 0.38,
                height_m: 1.372,
                rising: Some(false),
            }),
            stream: Some(crate::tide::Stream {
                name: "Emeryville Marina".into(),
                nm: 1.47,
                knots: 1.38,
                way: "ebb".into(),
                set_deg: Some(294.0),
            }),
        };
        let line = mark(
            at,
            time::utc(NOON),
            Some((37.8667, -122.315, Some(0.4), Some(11.0))),
            "Departed",
            "",
            &Around {
                weather,
                tide,
                extras: vec!["sunset 19:05".into()],
            },
        );
        let rest = line.split_once("— Departed").expect("the mark").1;
        assert_eq!(
            rest,
            " · sunset 19:05 · wind 13 kn from 262°, gusting 18 (HRRR) · 1014.6 hPa \
             · barometer 1013.8 hPa, falling 1.2 in 3 h (Alameda) \
             · measured 11 kn from 250° (Alameda, 12 min) \
             · tide 1.37 m falling (Berkeley, 0.4 nm) \
             · ebb 1.4 kn toward 294° (Emeryville Marina, 1.5 nm)"
        );
    }

    #[test]
    fn a_barometer_that_hasnt_moved_is_steady() {
        let b = crate::wind::Barometer {
            name: "Alameda".into(),
            nm: 6.4,
            pressure_hpa: None,
            tendency_hpa: 0.0,
        };
        assert_eq!(barometer(&b), "barometer steady over 3 h (Alameda)");
        let up = crate::wind::Barometer {
            tendency_hpa: 2.04,
            pressure_hpa: Some(1020.0),
            ..b
        };
        assert_eq!(
            barometer(&up),
            "barometer 1020.0 hPa, rising 2.0 in 3 h (Alameda)"
        );
    }

    #[test]
    fn a_mark_says_what_happened_where_and_what_it_was_blowing() {
        let at = time::local(NOON);
        let line = mark(
            at,
            time::utc(NOON),
            Some((37.8667, -122.315, Some(5.1), Some(245.0))),
            "Departed",
            "",
            &Around {
                weather: blowing(),
                ..Default::default()
            },
        );
        assert!(line.contains("— Departed ·"), "{line}");
        assert!(line.contains("245°"), "{line}");
        assert!(line.contains("5.1 kn"), "{line}");
        assert!(
            line.contains("wind 13 kn from 262°, gusting 18 (HRRR)"),
            "{line}"
        );
        assert!(line.contains("1014.6 hPa"), "{line}");
        assert!(
            line.contains("measured 11 kn from 250° (Alameda, 12 min)"),
            "{line}"
        );
    }

    #[test]
    fn what_the_crew_added_follows_the_mark() {
        let at = time::local(NOON);
        let line = mark(
            at,
            time::utc(NOON),
            Some((37.8667, -122.315, None, None)),
            "Anchor down",
            "  25 ft, 5:1 scope  ",
            &Default::default(),
        );
        assert!(line.ends_with("— Anchor down: 25 ft, 5:1 scope"), "{line}");
    }

    /// No hub and no wind engine: the mark still says what happened, and
    /// when.
    #[test]
    fn a_mark_with_nothing_running_is_still_a_mark() {
        let at = time::local(NOON);
        let line = mark(
            at,
            time::utc(NOON),
            None,
            "Berthed",
            "",
            &Default::default(),
        );
        assert!(line.ends_with("— Berthed"), "{line}");
        assert!(!line.contains('·'), "{line}");
    }

    /// A gust at or below the wind is not a gust worth writing down.
    #[test]
    fn a_gust_is_only_said_when_it_is_one() {
        assert_eq!(blow(12.0, Some(5.0), Some(12.2)), "12 kn from 005°");
        assert_eq!(blow(12.0, None, Some(20.0)), "12 kn, gusting 20");
    }

    /// The crew's words, where the boat was when they wrote them.
    #[test]
    fn a_note_carries_its_position_but_not_the_instruments() {
        let at = time::local(NOON);
        let line = note_at(
            at,
            time::utc(NOON),
            37.8667,
            -122.315,
            "Dolphins off the port side",
        );
        assert!(line.contains("37°52.0′N"), "{line}");
        assert!(line.contains("122°18.9′W"), "{line}");
        assert!(line.ends_with("— Dolphins off the port side"), "{line}");
        assert!(!line.contains(" kn"), "{line}");
        // One separator: the clock, then the position. No course, no speed.
        assert_eq!(line.matches('·').count(), 1, "{line}");
    }

    #[test]
    fn a_note_without_a_fix_still_reads_as_the_crew_s() {
        let at = time::local(NOON);
        assert!(note(at, time::utc(NOON), "  Dolphins  ").ends_with("— Dolphins"));
    }

    #[test]
    fn a_fix_reads_like_a_log_line() {
        let line = fix(
            time::utc(NOON),
            time::utc(NOON),
            37.8663,
            -122.3148,
            Some(5.14),
            Some(245.0),
            "under way",
        );
        assert_eq!(
            line,
            "- **12:00** UTC · 37°52.0′N 122°18.9′W · 245° · 5.1 kn — under way"
        );
    }

    #[test]
    fn a_fix_without_course_or_speed_still_reads() {
        let line = fix(
            time::utc(NOON),
            time::utc(NOON),
            37.8663,
            -122.3148,
            None,
            None,
            "",
        );
        assert_eq!(line, "- **12:00** UTC · 37°52.0′N 122°18.9′W");
    }

    #[test]
    fn away_from_greenwich_both_clocks_show() {
        let local = Local {
            hour: 5,
            offset: -25_200,
            ..time::utc(NOON)
        };
        let line = event(local, time::utc(NOON), "under way");
        assert_eq!(line, "- **05:00** (12:00 UTC) — under way");
    }

    #[test]
    fn a_note_is_trimmed_but_otherwise_the_crews_own_words() {
        let line = note(
            time::utc(NOON),
            time::utc(NOON),
            "  Reefed. Wind up to 22.  ",
        );
        assert_eq!(line, "- **12:00** UTC — Reefed. Wind up to 22.");
    }
}
