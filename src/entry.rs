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

/// The crew's own entry, marked so it reads as theirs.
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
