//! Dates and clocks. A day in the log is a day in the boat's own time zone,
//! because that is the day the crew lived; every entry also carries UTC.

/// Days from 1970-01-01 to a civil date, by Howard Hinnant's algorithm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// `2026-09-13T21:00:10Z` and the same with a fraction, as NMEA time reaches
/// omakeel. Anything else is None rather than a guess.
pub fn parse_utc(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let num = |from: usize, to: usize| s.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Is this a date the calendar has? `2026-02-31` is not.
pub fn valid_date(date: &str) -> bool {
    let Some(epoch) = parse_utc(&format!("{date}T00:00:00Z")) else {
        return false;
    };
    utc(epoch).date() == date
}

/// A moment in the machine's local time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Local {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    /// Seconds east of UTC, as the zone stood at that moment.
    pub offset: i64,
}

impl Local {
    /// `2026-09-18`, the name of the day's note.
    pub fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// `15:04`, local, for an entry.
    pub fn clock(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    /// `+02:00`, or `Z` at Greenwich, so a note says which clock it kept.
    pub fn zone(&self) -> String {
        if self.offset == 0 {
            return "Z".into();
        }
        let (sign, secs) = if self.offset < 0 {
            ('-', -self.offset)
        } else {
            ('+', self.offset)
        };
        format!("{sign}{:02}:{:02}", secs / 3600, (secs % 3600) / 60)
    }
}

/// The local calendar, from the C library, so the zone and its summer time
/// come from the machine rather than from arithmetic here.
pub fn local(epoch: i64) -> Local {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let t = epoch as libc::time_t;
    // SAFETY: localtime_r fills the caller's tm and touches nothing else.
    let filled = unsafe { libc::localtime_r(&t, &mut tm) };
    if filled.is_null() {
        return utc(epoch);
    }
    Local {
        year: tm.tm_year as i64 + 1900,
        month: tm.tm_mon as u32 + 1,
        day: tm.tm_mday as u32,
        hour: tm.tm_hour as u32,
        minute: tm.tm_min as u32,
        second: tm.tm_sec as u32,
        offset: tm.tm_gmtoff as i64,
    }
}

/// The same moment at Greenwich, the fallback when the zone can't be read.
pub fn utc(epoch: i64) -> Local {
    let days = epoch.div_euclid(86_400);
    let secs = epoch.rem_euclid(86_400);
    // Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    Local {
        year,
        month,
        day,
        hour: (secs / 3600) as u32,
        minute: (secs % 3600 / 60) as u32,
        second: (secs % 60) as u32,
        offset: 0,
    }
}

/// `2026-09-18T15:04:09Z`, for GPX and for an entry's UTC stamp.
pub fn iso_utc(epoch: i64) -> String {
    let t = utc(epoch);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    )
}

/// Seconds since the epoch, now.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_omakeel_stamps() {
        assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_utc("2026-09-13T21:00:10Z"), Some(1_789_333_210));
        assert_eq!(parse_utc("2026-09-13T21:00:10.5Z"), Some(1_789_333_210));
    }

    #[test]
    fn refuses_anything_else() {
        for s in ["", "2026-09-13", "2026-13-01T00:00:00Z", "tomorrow at noon"] {
            assert_eq!(parse_utc(s), None, "{s}");
        }
    }

    #[test]
    fn utc_round_trips() {
        for epoch in [0, 1_789_333_210, 2_000_000_000, -86_400] {
            assert_eq!(parse_utc(&iso_utc(epoch)), Some(epoch), "{epoch}");
        }
    }

    #[test]
    fn a_date_the_calendar_hasnt_is_refused() {
        for good in ["2026-09-18", "2024-02-29", "1970-01-01"] {
            assert!(valid_date(good), "{good}");
        }
        for bad in [
            "2026-02-31",
            "2026-13-01",
            "2026-09-31",
            "x",
            "",
            "2026-9-8",
        ] {
            assert!(!valid_date(bad), "{bad}");
        }
    }

    #[test]
    fn utc_names_the_day() {
        assert_eq!(utc(1_789_333_210).date(), "2026-09-13");
        assert_eq!(utc(1_789_333_210).clock(), "21:00");
        assert_eq!(utc(0).zone(), "Z");
    }
}
