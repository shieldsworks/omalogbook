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
