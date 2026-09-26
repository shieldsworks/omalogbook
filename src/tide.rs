//! The tide and the stream from omatide: its socket and `state` message,
//! version 1, as omatide's docs/protocol.md describes them. Read only, as
//! every Omahoy app is.
//!
//! Both are predictions from NOAA's harmonic constants at the nearest
//! station of each kind, not measurements, and the entry names the station
//! so nobody later mistakes Emeryville's stream for the one under the keel.

use serde_json::Value;
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// omatide's `state` carries three days of turns and the bay's narrows; this
/// is far more than that.
const MAX_LINE: u64 = 1 << 20;

/// A station further off than this is somewhere else's water. omatide itself
/// looks out to 60 nm, which is right for a chart and wrong for a log.
const NEAR_NM: f64 = 10.0;

/// The height of the tide at the nearest tide station.
#[derive(Clone, Debug, PartialEq)]
pub struct Water {
    pub name: String,
    pub nm: f64,
    /// Meters above mean lower low water, the chart datum.
    pub height_m: f64,
    pub rising: Option<bool>,
}

/// The stream at the nearest current station.
#[derive(Clone, Debug, PartialEq)]
pub struct Stream {
    pub name: String,
    pub nm: f64,
    pub knots: f64,
    /// `flood`, `ebb` or `slack`.
    pub way: String,
    /// Where the stream sets, degrees true, when NOAA says.
    pub set_deg: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tide {
    pub water: Option<Water>,
    pub stream: Option<Stream>,
}

impl Tide {
    pub fn is_empty(&self) -> bool {
        self.water.is_none() && self.stream.is_none()
    }
}

/// `$XDG_RUNTIME_DIR/omatide/tide.sock`.
pub fn default_socket() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_absolute())
        .map(|d| d.join("omatide").join("tide.sock"))
}

fn number(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64).filter(|n| n.is_finite())
}

/// The station's name, or nothing: a reading from an unnamed station can't
/// be checked later, so it isn't written down.
fn name(v: &Value) -> Option<String> {
    let name = v
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| v.get("station").and_then(Value::as_str))?
        .trim();
    // A name is a label in a log line, not a place for markup or a novel.
    let name: String = name.chars().filter(|c| !c.is_control()).take(60).collect();
    (!name.is_empty()).then_some(name)
}

/// How far off the station is, when that is near enough to count.
fn near(v: &Value) -> Option<f64> {
    number(v, "distanceNm").filter(|nm| (0.0..=NEAR_NM).contains(nm))
}

/// What one line from omatide says about the water at the boat. None for a
/// line that isn't a `state` this reader can use.
fn read(line: &str) -> Option<Tide> {
    let m: Value = serde_json::from_str(line).ok()?;
    if m.get("v")?.as_u64()? != 1 || m.get("type")?.as_str()? != "state" {
        return None;
    }
    // Without a fix omatide answers for its home position, which is where
    // the boat usually is and exactly when it matters that it isn't.
    if m.get("here")?.get("at").and_then(Value::as_str) != Some("boat") {
        return None;
    }
    let water = m.get("tide").and_then(|t| {
        Some(Water {
            name: name(t)?,
            nm: near(t)?,
            // The lowest tide ever measured in the Bay is under a meter below
            // datum, and the highest spring tide a few meters above.
            height_m: number(t, "heightM").filter(|h| (-5.0..=20.0).contains(h))?,
            rising: t.get("rising").and_then(Value::as_bool),
        })
    });
    let stream = m.get("current").and_then(|c| {
        let way = c.get("way").and_then(Value::as_str)?;
        if !matches!(way, "flood" | "ebb" | "slack") {
            return None;
        }
        Some(Stream {
            name: name(c)?,
            nm: near(c)?,
            knots: number(c, "knots").filter(|k| (0.0..=20.0).contains(k))?,
            way: way.to_string(),
            set_deg: number(c, "setDeg").filter(|d| (0.0..360.0).contains(d)),
        })
    });
    Some(Tide { water, stream })
}

/// The tide and stream at the boat right now, for a one-shot command like a
/// mark. omatide sends `hello` then `state` on connect, so one line after
/// the greeting is enough. An engine that is down, silent, or answering for
/// its home position gives nothing, and the mark is filed without it.
pub fn once(socket: &Path, wait: Duration) -> Tide {
    let Ok(stream) = std::os::unix::net::UnixStream::connect(socket) else {
        return Tide::default();
    };
    if stream.set_read_timeout(Some(wait)).is_err() {
        return Tide::default();
    }
    let deadline = Instant::now() + wait;
    let mut reader = std::io::BufReader::new(stream.take(MAX_LINE * 4));
    let mut line = String::new();
    while Instant::now() < deadline {
        line.clear();
        match std::io::BufRead::read_line(&mut reader, &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) if line.len() as u64 > MAX_LINE => break,
            Ok(_) => {
                if let Some(t) = read(line.trim_end()) {
                    return t;
                }
            }
        }
    }
    Tide::default()
}

/// ` · tide 1.37 m falling (Berkeley, 0.4 nm) · ebb 1.4 kn toward 294°
/// (Emeryville Marina, 1.5 nm)`, or nothing when there is nothing to say.
pub fn words(tide: &Tide) -> String {
    let mut out = String::new();
    if let Some(w) = &tide.water {
        let way = match w.rising {
            Some(true) => " rising",
            Some(false) => " falling",
            None => "",
        };
        out.push_str(&format!(
            " · tide {:.2} m{way} ({}, {:.1} nm)",
            w.height_m, w.name, w.nm
        ));
    }
    if let Some(s) = &tide.stream {
        // Below a tenth of a knot the stream has no way worth naming.
        if s.way == "slack" || s.knots < 0.05 {
            out.push_str(&format!(" · slack water ({}, {:.1} nm)", s.name, s.nm));
        } else {
            let set = s
                .set_deg
                .map(|d| format!(" toward {:03.0}°", d.round() % 360.0))
                .unwrap_or_default();
            out.push_str(&format!(
                " · {} {:.1} kn{set} ({}, {:.1} nm)",
                s.way, s.knots, s.name, s.nm
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &str = r#"{"type":"state","v":1,"time":"2026-09-20T17:12:00Z","keel":"connected",
 "here":{"lat":37.8663,"lon":-122.3148,"at":"boat"},
 "tide":{"station":"9414816","name":"Berkeley","lat":37.865,"lon":-122.307,"distanceNm":0.38,"heightM":1.372,"rising":false,"turns":[]},
 "current":{"station":"SFB1218-1","name":"Emeryville Marina","lat":37.84332,"lon":-122.32537,"distanceNm":1.47,"depthM":1.5,"knots":1.38,"way":"ebb","setDeg":294,"turns":[]},
 "bay":[]}"#;

    fn one_line(s: &str) -> String {
        s.replace('\n', "")
    }

    #[test]
    fn the_tide_and_stream_at_the_boat_read() {
        let t = read(&one_line(STATE)).expect("a state");
        assert_eq!(
            words(&t),
            " · tide 1.37 m falling (Berkeley, 0.4 nm) · ebb 1.4 kn toward 294° (Emeryville Marina, 1.5 nm)"
        );
    }

    /// With no fix omatide answers for home, which is not where the boat is.
    #[test]
    fn home_is_not_the_boat() {
        let home = one_line(STATE).replace(r#""at":"boat""#, r#""at":"home""#);
        assert_eq!(read(&home), None);
    }

    #[test]
    fn slack_water_has_no_way() {
        let slack = one_line(STATE)
            .replace(r#""way":"ebb""#, r#""way":"slack""#)
            .replace(r#""knots":1.38"#, r#""knots":0.0"#);
        let t = read(&slack).expect("a state");
        assert!(
            words(&t).ends_with(" · slack water (Emeryville Marina, 1.5 nm)"),
            "{}",
            words(&t)
        );
    }

    #[test]
    fn a_far_station_is_somebody_elses_water() {
        let far = one_line(STATE).replace(r#""distanceNm":0.38"#, r#""distanceNm":42.0"#);
        let t = read(&far).expect("a state");
        assert_eq!(t.water, None);
        assert!(t.stream.is_some());
    }

    #[test]
    fn hostile_values_are_left_out() {
        for (from, to) in [
            (r#""heightM":1.372"#, r#""heightM":1e9"#),
            (r#""heightM":1.372"#, r#""heightM":"high""#),
            (r#""name":"Berkeley""#, r#""name":"""#),
        ] {
            let bad = one_line(STATE).replace(from, to);
            let bad = if to == r#""name":"""# {
                bad.replace(r#""station":"9414816","#, "")
            } else {
                bad
            };
            assert_eq!(read(&bad).expect("a state").water, None, "{to}");
        }
        for (from, to) in [
            (r#""knots":1.38"#, r#""knots":-2"#),
            (r#""way":"ebb""#, r#""way":"<b>sideways</b>""#),
        ] {
            let bad = one_line(STATE).replace(from, to);
            assert_eq!(read(&bad).expect("a state").stream, None, "{to}");
        }
        let spun = one_line(STATE).replace(r#""setDeg":294"#, r#""setDeg":720"#);
        let t = read(&spun).expect("a state");
        assert_eq!(t.stream.expect("a stream").set_deg, None);
    }

    #[test]
    fn other_lines_are_not_read() {
        for line in [
            r#"{"type":"hello","v":1,"tide":"0.1.0"}"#,
            &one_line(STATE).replace(r#""v":1"#, r#""v":2"#),
            "not json",
            "",
        ] {
            assert_eq!(read(line), None, "{line}");
        }
    }

    #[test]
    fn nothing_listening_is_no_tide() {
        let missing = std::env::temp_dir().join("omalogbook-tide-nothing-here.sock");
        let _ = std::fs::remove_file(&missing);
        assert!(once(&missing, Duration::from_millis(50)).is_empty());
    }
}
