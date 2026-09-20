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
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// omawind's longest line, a whole forecast field, is well under this.
const MAX_LINE: u64 = 4 << 20;

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    pub forecast: Option<Forecast>,
    pub measured: Option<Measured>,
}

impl Weather {
    pub fn is_empty(&self) -> bool {
        self.forecast.is_none() && self.measured.is_none()
    }
}

/// `$XDG_RUNTIME_DIR/omawind/wind.sock`.
pub fn default_socket() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_absolute())
        .map(|d| d.join("omawind").join("wind.sock"))
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
                measured: None,
            })
        }
        "stations" => {
            let list = m.get("stations")?.as_array()?;
            let mut best: Option<Measured> = None;
            for s in list {
                let (Some(lat), Some(lon)) = (number(s, "lat"), number(s, "lon")) else {
                    continue;
                };
                let nm = geo::distance_nm(at, (lat, lon));
                if !nm.is_finite() || nm > NEAR_NM {
                    continue;
                }
                if best.as_ref().is_some_and(|b| b.nm <= nm) {
                    continue;
                }
                let Some(speed_kn) = speed(s, "speedKn") else {
                    continue;
                };
                let id = s.get("id").and_then(Value::as_str).unwrap_or("");
                let name = s.get("name").and_then(Value::as_str).unwrap_or(id);
                if name.is_empty() {
                    continue;
                }
                best = Some(Measured {
                    name: name.to_string(),
                    nm,
                    age_minutes: s
                        .get("time")
                        .and_then(Value::as_str)
                        .and_then(crate::time::parse_utc)
                        .map(|t| (now - t).max(0) / 60),
                    speed_kn,
                    dir_deg: direction(s, "dirDeg"),
                    gust_kn: speed(s, "gustKn"),
                });
            }
            best.map(|m| Weather {
                forecast: None,
                measured: Some(m),
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
    let Ok(stream) = std::os::unix::net::UnixStream::connect(socket) else {
        return weather;
    };
    if stream.set_read_timeout(Some(wait)).is_err() {
        return weather;
    }
    let deadline = Instant::now() + wait;
    let mut reader = std::io::BufReader::new(stream.take(MAX_LINE));
    let mut line = String::new();
    while Instant::now() < deadline {
        line.clear();
        match std::io::BufRead::read_line(&mut reader, &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if let Some(w) = read(line.trim_end(), at, now) {
                    if w.forecast.is_some() {
                        weather.forecast = w.forecast;
                    }
                    if w.measured.is_some() {
                        weather.measured = w.measured;
                    }
                    if weather.forecast.is_some() && weather.measured.is_some() {
                        break;
                    }
                }
            }
        }
    }
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

    #[test]
    fn nothing_listening_is_no_weather() {
        let missing = std::env::temp_dir().join("omalogbook-wind-nothing-here.sock");
        let _ = std::fs::remove_file(&missing);
        assert!(once(&missing, BOAT, NOON, Duration::from_millis(50)).is_empty());
    }
}
