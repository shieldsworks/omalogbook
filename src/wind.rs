//! The weather from omawind: its socket and the `state` and `stations`
//! messages, version 1, as omawind's docs/protocol.md describes them. Read
//! only, as every Omahoy app is.
//!
//! Two different things come back and the log keeps them apart. `here` is
//! HRRR's forecast worked out at the boat — a model's opinion, available
//! anywhere the region covers, and the only source of a barometer reading.
//! A station is a real anemometer that really measured that wind, somewhere
//! else. A log that blurred them would be a log you can't trust later.

use crate::geo;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

/// omawind's longest line, a whole forecast field, is well under this.
const MAX_LINE: usize = 4 << 20;

/// A tendency older than this is left out: it is about some other three
/// hours.
const BAROMETER_MAX_AGE_MINUTES: i64 = 180;

/// A station further off than this measured somewhere else's wind.
const NEAR_NM: f64 = 10.0;

/// HRRR at the boat: what the model says was blowing there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Forecast {
    pub speed_kn: f64,
    pub dir_deg: Option<f64>,
    pub gust_kn: Option<f64>,
    pub pressure_hpa: Option<f64>,
}

/// What an anemometer somewhere nearby actually measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    /// NDBC's name for the station when it has one, else its id.
    pub name: String,
    pub nm: f64,
    /// Minutes since the report was taken, when its time could be read.
    pub age_minutes: Option<i64>,
    pub speed_kn: f64,
    pub dir_deg: Option<f64>,
    pub gust_kn: Option<f64>,
}

/// A barometer that really read the pressure, and how it has moved: NDBC's
/// three-hour tendency, the oldest weather sign in a ship's log. A model
/// can't give this; HRRR has no memory of the last three hours.
#[derive(Clone, Debug, PartialEq)]
pub struct Barometer {
    pub name: String,
    pub nm: f64,
    pub pressure_hpa: Option<f64>,
    /// Hectopascals gained (positive) or lost over the three hours up to
    /// its report.
    pub tendency_hpa: f64,
    /// Minutes since that report, when its time could be read.
    pub age_minutes: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    pub forecast: Option<Forecast>,
    pub measured: Option<Measured>,
    pub barometer: Option<Barometer>,
}

impl Weather {
    pub fn is_empty(&self) -> bool {
        self.forecast.is_none() && self.measured.is_none() && self.barometer.is_none()
    }
}

/// `$XDG_RUNTIME_DIR/omawind/wind.sock`.
pub fn default_socket() -> Option<PathBuf> {
    crate::config::usable_env_path(std::env::var_os("XDG_RUNTIME_DIR"))
        .map(|dir| dir.join("omawind").join("wind.sock"))
}

fn number(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64).filter(|n| n.is_finite())
}

/// A compass direction, or nothing. omawind sends 0 to 359; anything else
/// is not a direction and is better left off than written down wrong.
fn direction(v: &Value, key: &str) -> Option<f64> {
    number(v, key).filter(|d| (0.0..360.0).contains(d))
}

/// A wind speed, or nothing. Nothing on this planet blows 300 knots, and a
/// negative speed is a bug somewhere upstream.
fn speed(v: &Value, key: &str) -> Option<f64> {
    number(v, key).filter(|k| (0.0..=300.0).contains(k))
}

/// NDBC's name for a station when it has one, else its id, cleaned for a
/// log line. None when neither leaves anything to write.
fn station_name(s: &Value) -> Option<String> {
    ["name", "id"]
        .iter()
        .filter_map(|key| s.get(key).and_then(Value::as_str))
        .find_map(crate::wire::label)
}

/// Minutes since a station's report was taken, when its time can be read.
fn age_minutes(s: &Value, now: i64) -> Option<i64> {
    s.get("time")
        .and_then(Value::as_str)
        .and_then(crate::time::parse_utc)
        .map(|t| (now - t).max(0) / 60)
}

/// A station's barometer, when it reports a tendency. A pressure with no
/// tendency says less than HRRR's already does, so it isn't worth a line.
fn barometer(s: &Value, nm: f64, now: i64) -> Option<Barometer> {
    // A real three-hour change past 20 hPa is a hurricane arriving; anything
    // beyond it is a bad report.
    let tendency_hpa = number(s, "tendencyHpa").filter(|t| t.abs() <= 20.0)?;
    // A tendency is the three hours up to the report it came with. NDBC only
    // sends one on the hour, so omawind keeps it with its own time; without
    // that, the station's report time is the best there is. Older than three
    // hours, it describes a different afternoon.
    let age = s
        .get("tendencyTime")
        .and_then(Value::as_str)
        .and_then(crate::time::parse_utc)
        .map(|t| (now - t).max(0) / 60)
        .or_else(|| age_minutes(s, now));
    if age.is_some_and(|m| m > BAROMETER_MAX_AGE_MINUTES) {
        return None;
    }
    Some(Barometer {
        name: station_name(s)?,
        age_minutes: age,
        nm,
        pressure_hpa: number(s, "pressureHpa").filter(|p| (800.0..=1100.0).contains(p)),
        tendency_hpa,
    })
}

/// What one line from omawind says, given where the boat is. Returns None
/// for a line that isn't a `state` or `stations` this reader can use.
fn read(line: &str, at: (f64, f64), now: i64) -> Option<Weather> {
    let m: Value = serde_json::from_str(line).ok()?;
    if m.get("v")?.as_u64()? != 1 {
        return None;
    }
    match m.get("type")?.as_str()? {
        "state" => {
            let here = m.get("here")?;
            // `here` is only the boat's own weather when omawind is following
            // the boat. Pinned to a home position it is somewhere else, and
            // the log would be saying the wrong thing.
            if here.get("at").and_then(Value::as_str) != Some("boat") {
                return None;
            }
            Some(Weather {
                forecast: Some(Forecast {
                    speed_kn: speed(here, "speedKn")?,
                    dir_deg: direction(here, "dirDeg"),
                    gust_kn: speed(here, "gustKn"),
                    pressure_hpa: number(here, "pressureHpa")
                        .filter(|p| (800.0..=1100.0).contains(p)),
                }),
                ..Weather::default()
            })
        }
        "stations" => {
            let list = m.get("stations")?.as_array()?;
            let mut best: Option<Measured> = None;
            let mut barometer: Option<Barometer> = None;
            for s in list {
                let (Some(lat), Some(lon)) = (number(s, "lat"), number(s, "lon")) else {
                    continue;
                };
                let nm = geo::distance_nm(at, (lat, lon));
                if !nm.is_finite() || nm > NEAR_NM {
                    continue;
                }
                // The nearest barometer is chosen on its own: the nearest
                // anemometer often has none.
                if barometer.as_ref().is_none_or(|b| nm < b.nm)
                    && let Some(b) = self::barometer(s, nm, now)
                {
                    barometer = Some(b);
                }
                if best.as_ref().is_some_and(|b| b.nm <= nm) {
                    continue;
                }
                let Some(speed_kn) = speed(s, "speedKn") else {
                    continue;
                };
                let Some(name) = station_name(s) else {
                    continue;
                };
                best = Some(Measured {
                    name,
                    nm,
                    age_minutes: age_minutes(s, now),
                    speed_kn,
                    dir_deg: direction(s, "dirDeg"),
                    gust_kn: speed(s, "gustKn"),
                });
            }
            (best.is_some() || barometer.is_some()).then_some(Weather {
                forecast: None,
                measured: best,
                barometer,
            })
        }
        _ => None,
    }
}

/// The weather at the boat right now, for a one-shot command like a mark.
/// omawind sends `hello`, then `state`, then `stations` on connect, so both
/// halves arrive without asking; the read ends as soon as it has them or
/// `wait` runs out. An engine that is down, silent or speaking another
/// version gives no weather at all, and the mark is filed without it.
pub fn once(socket: &Path, at: (f64, f64), now: i64, wait: Duration) -> Weather {
    let mut weather = Weather::default();
    crate::wire::read_lines(socket, wait, MAX_LINE, |line| {
        if let Some(w) = read(line, at, now) {
            if w.forecast.is_some() {
                weather.forecast = w.forecast;
            }
            if w.measured.is_some() {
                weather.measured = w.measured;
            }
            if w.barometer.is_some() {
                weather.barometer = w.barometer;
            }
        }
        weather.forecast.is_some() && weather.measured.is_some()
    });
    weather
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOAT: (f64, f64) = (37.8667, -122.315);
    const NOON: i64 = 1_789_300_800; // 2026-09-13T12:00:00Z

    const STATE: &str = r#"{"type":"state","v":1,"here":{"at":"boat","lat":37.86,"lon":-122.31,"speedKn":12.4,"dirDeg":262,"gustKn":17.9,"pressureHpa":1014.6}}"#;
    const STATIONS: &str = r#"{"type":"stations","v":1,"source":"NDBC","stations":[
        {"id":"AAMC1","name":"Alameda","lat":37.772,"lon":-122.3,"time":"2026-09-13T11:48:00Z","speedKn":2.9,"dirDeg":120,"gustKn":4.1},
        {"id":"FAR","name":"Farallon","lat":37.77,"lon":-123.0,"speedKn":22.0,"dirDeg":280}]}"#;

    #[test]
    fn the_forecast_at_the_boat_reads() {
        let w = read(STATE, BOAT, NOON).expect("a state");
        let f = w.forecast.expect("a forecast");
        assert_eq!(f.speed_kn, 12.4);
        assert_eq!(f.dir_deg, Some(262.0));
        assert_eq!(f.gust_kn, Some(17.9));
        assert_eq!(f.pressure_hpa, Some(1014.6));
    }

    /// Pinned to a home position, `here` is not where the boat is.
    #[test]
    fn a_forecast_somewhere_else_is_not_the_boats() {
        let home = STATE.replace(r#""at":"boat""#, r#""at":"home""#);
        assert_eq!(read(&home, BOAT, NOON), None);
    }

    #[test]
    fn the_nearest_station_wins_and_the_far_one_is_left_out() {
        let w = read(STATIONS, BOAT, NOON).expect("stations");
        let m = w.measured.expect("a station");
        assert_eq!(m.name, "Alameda");
        assert!(m.nm < NEAR_NM, "{} nm", m.nm);
        assert_eq!(m.age_minutes, Some(12));
        assert_eq!(m.speed_kn, 2.9);
    }

    /// The Farallones are 30-odd miles out: that is someone else's wind.
    #[test]
    fn no_station_near_is_no_station() {
        let far = r#"{"type":"stations","v":1,"stations":[{"id":"FAR","name":"Farallon","lat":37.77,"lon":-123.0,"speedKn":22.0}]}"#;
        assert_eq!(read(far, BOAT, NOON), None);
    }

    #[test]
    fn a_station_with_no_wind_is_not_a_report() {
        let calm = r#"{"type":"stations","v":1,"stations":[{"id":"AAMC1","name":"Alameda","lat":37.772,"lon":-122.3}]}"#;
        assert_eq!(read(calm, BOAT, NOON), None);
    }

    #[test]
    fn hostile_numbers_are_refused() {
        for bad in [
            (r#""speedKn":12.4"#, r#""speedKn":-5.0"#),
            (r#""speedKn":12.4"#, r#""speedKn":1e9"#),
        ] {
            assert_eq!(
                read(&STATE.replace(bad.0, bad.1), BOAT, NOON),
                None,
                "{bad:?}"
            );
        }
        let spun = STATE.replace(r#""dirDeg":262"#, r#""dirDeg":999"#);
        let w = read(&spun, BOAT, NOON).expect("a state");
        assert_eq!(w.forecast.expect("a forecast").dir_deg, None);
        let deep = STATE.replace(r#""pressureHpa":1014.6"#, r#""pressureHpa":3.0"#);
        let w = read(&deep, BOAT, NOON).expect("a state");
        assert_eq!(w.forecast.expect("a forecast").pressure_hpa, None);
    }

    #[test]
    fn another_version_is_not_read() {
        assert_eq!(
            read(&STATE.replace(r#""v":1"#, r#""v":2"#), BOAT, NOON),
            None
        );
        for line in [
            "{}",
            "not json",
            "",
            r#"{"type":"hello","v":1,"wind":"0.1.0"}"#,
        ] {
            assert_eq!(read(line, BOAT, NOON), None, "{line}");
        }
    }

    /// The nearest barometer is picked on its own: here the nearest station
    /// has wind and no barometer, and one further off has both.
    #[test]
    fn the_nearest_barometer_is_its_own_choice() {
        let line = r#"{"type":"stations","v":1,"stations":[
            {"id":"AAMC1","name":"Alameda","lat":37.772,"lon":-122.3,"speedKn":2.9,"pressureHpa":1013.8,"tendencyHpa":-1.2},
            {"id":"NEAR","name":"Berkeley Pier","lat":37.866,"lon":-122.33,"speedKn":6.0}]}"#
            .replace('\n', "");
        let w = read(&line, BOAT, NOON).expect("stations");
        assert_eq!(w.measured.expect("wind").name, "Berkeley Pier");
        let b = w.barometer.expect("a barometer");
        assert_eq!(b.name, "Alameda");
        assert_eq!(b.pressure_hpa, Some(1013.8));
        assert_eq!(b.tendency_hpa, -1.2);
    }

    /// A pressure with no tendency is not a barometer worth a line, and a
    /// tendency no weather makes is a bad report.
    #[test]
    fn a_barometer_needs_a_believable_tendency() {
        for tendency in [r#""#, r#","tendencyHpa":99"#, r#","tendencyHpa":"up""#] {
            let line = format!(
                r#"{{"type":"stations","v":1,"stations":[{{"id":"AAMC1","name":"Alameda","lat":37.772,"lon":-122.3,"speedKn":2.9,"pressureHpa":1013.8{tendency}}}]}}"#
            );
            let w = read(&line, BOAT, NOON).expect("stations");
            assert_eq!(w.barometer, None, "{tendency}");
        }
        // A barometer with no anemometer is still weather.
        let line = r#"{"type":"stations","v":1,"stations":[{"id":"AAMC1","name":"Alameda","lat":37.772,"lon":-122.3,"tendencyHpa":0.4}]}"#;
        let w = read(line, BOAT, NOON).expect("stations");
        assert_eq!(w.measured, None);
        assert_eq!(w.barometer.expect("a barometer").pressure_hpa, None);
    }

    /// A tendency is about the three hours before its report; one more than
    /// three hours old is about some other afternoon. Its own time wins over
    /// the station's, and one with no time at all can't be judged, and
    /// stands.
    #[test]
    fn an_old_tendency_is_left_out() {
        let at = |times: &str| {
            format!(
                r#"{{"type":"stations","v":1,"stations":[{{"id":"AAMC1","name":"Alameda","lat":37.772,"lon":-122.3,"tendencyHpa":-1.2{times}}}]}}"#
            )
        };
        let age = |times: &str| {
            read(&at(times), BOAT, NOON)
                .and_then(|w| w.barometer)
                .map(|b| b.age_minutes)
        };
        assert_eq!(age(r#","tendencyTime":"2026-09-13T08:30:00Z""#), None);
        assert_eq!(age(r#","time":"2026-09-13T08:30:00Z""#), None);
        assert_eq!(
            age(r#","time":"2026-09-13T11:54:00Z","tendencyTime":"2026-09-13T10:40:00Z""#),
            Some(Some(80))
        );
        assert_eq!(
            age(r#","time":"2026-09-13T08:00:00Z","tendencyTime":"2026-09-13T11:00:00Z""#),
            Some(Some(60))
        );
        assert_eq!(age(""), Some(None));
    }

    /// Station names go into a log line: control characters out, length cut,
    /// the id when the name is empty.
    #[test]
    fn station_names_are_cleaned() {
        let line = format!(
            r#"{{"type":"stations","v":1,"stations":[{{"id":"AAMC1","name":"Ala\u001bmeda{}","lat":37.772,"lon":-122.3,"speedKn":2.9,"tendencyHpa":0.3}}]}}"#,
            "x".repeat(100)
        );
        let w = read(&line, BOAT, NOON).expect("stations");
        let m = w.measured.expect("wind");
        assert!(m.name.starts_with("Alameda"), "{}", m.name);
        assert_eq!(m.name.chars().count(), 60);
        assert_eq!(w.barometer.expect("barometer").name, m.name);
        let blank = r#"{"type":"stations","v":1,"stations":[{"id":"AAMC1","name":"  ","lat":37.772,"lon":-122.3,"speedKn":2.9}]}"#;
        assert_eq!(
            read(blank, BOAT, NOON)
                .expect("stations")
                .measured
                .expect("wind")
                .name,
            "AAMC1"
        );
    }

    /// An engine dribbling a byte at a time must not hold the mark past its
    /// wait.
    #[test]
    fn a_trickling_engine_is_given_up_on() {
        let path = crate::wire::trickle("wind", b"{\"type\":", Duration::from_millis(100));
        let start = std::time::Instant::now();
        let w = once(&path, BOAT, NOON, Duration::from_millis(400));
        assert!(w.is_empty());
        assert!(
            start.elapsed() < Duration::from_millis(900),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn nothing_listening_is_no_weather() {
        let missing = std::env::temp_dir().join("omalogbook-wind-nothing-here.sock");
        let _ = std::fs::remove_file(&missing);
        assert!(once(&missing, BOAT, NOON, Duration::from_millis(50)).is_empty());
    }
}
