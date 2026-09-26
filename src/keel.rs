//! The boat's position from omakeel: its socket and `state` messages, version
//! 1, as omakeel's docs/protocol.md describes them. Read only, as every
//! Omahoy app is.

use serde_json::Value;
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    net::UnixStream,
    sync::mpsc,
    time::{Duration, sleep},
};

/// omakeel's longest line, a thousand vessels' worth, is well under this.
const MAX_LINE: u64 = 4 << 20;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fix {
    pub lat: f64,
    pub lon: f64,
    pub sog_kn: Option<f64>,
    pub cog_deg: Option<f64>,
    /// The receiver's own time, when it gave one.
    pub utc: Option<i64>,
    /// How good the fix was, from GGA, for the line that says it was lost.
    pub satellites: Option<u32>,
    pub hdop: Option<f64>,
}

/// Why there is no current fix, as far as the log can tell.
///
/// A position goes missing for very different reasons — the hub stopped, the
/// link to the GPS broke, the GPS is connected but silent, or the receiver is
/// talking and simply can't see enough sky — and each is fixed in a different
/// place. "no fix" alone sends the crew looking everywhere, so the log says
/// which it was. omakeel already knows; this is its `state` read closely.
#[derive(Clone, Debug, PartialEq)]
pub enum Why {
    /// No connection to omakeel at all.
    Hub,
    /// Sources that aren't delivering: the link, not the receiver.
    Links(Vec<Link>),
    /// The receiver is talking but reports no fix: sky, antenna, or the
    /// receiver itself. omakeel keeps the satellite count of the last good
    /// GGA, so there is no count here that belongs to this moment.
    Receiver,
    /// Every source is delivering, and none of it is a position.
    Silent,
}

/// One source that isn't delivering, as omakeel names it.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub name: String,
    /// `down` (omakeel's `error`, which it stays in while it retries),
    /// `connecting` (reached, or starting, and nothing heard yet), `quiet` or
    /// `ended`.
    pub state: &'static str,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    /// A current fix: less than five seconds old, and the sources that
    /// weren't delivering even so — an AIS receiver on an empty bay, say —
    /// which are no reason for the fix to go when it later does.
    Fix(Fix, Vec<String>),
    /// No current fix, and why. A stale position is dropped here: it says
    /// where the boat was, not where it is, and the log has no use for it.
    NoFix(Why),
    /// Not connected to omakeel.
    Lost,
    /// omakeel speaks another protocol version.
    Incompatible(u64),
}

impl Why {
    /// Two reasons are the same for the log when they would send the crew to
    /// the same place, so a source flipping between `error` and `connecting`
    /// every two seconds while it retries is one reason, not a stream of them.
    pub fn same_as(&self, other: &Why) -> bool {
        match (self, other) {
            (Why::Links(a), Why::Links(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|(x, y)| x.name == y.name && x.state == y.state)
            }
            _ => std::mem::discriminant(self) == std::mem::discriminant(other),
        }
    }

    /// This reason without the sources that were already idle while the fix
    /// was good. What is left is what changed, and that is the cause; with
    /// nothing left, the receiver's own link was fine.
    pub fn without(self, idle: &[String]) -> Why {
        match self {
            Why::Links(links) => {
                let links: Vec<Link> = links
                    .into_iter()
                    .filter(|l| !idle.contains(&l.name))
                    .collect();
                if links.is_empty() {
                    Why::Silent
                } else {
                    Why::Links(links)
                }
            }
            other => other,
        }
    }

    /// The reason in words, for the log.
    pub fn say(&self) -> String {
        match self {
            Why::Hub => "omakeel, the hub, isn't answering".into(),
            Why::Links(links) => links
                .iter()
                .map(|l| match (l.state, &l.message) {
                    ("down", Some(m)) => format!("{} is down ({m})", l.name),
                    ("down", None) => format!("{} is down", l.name),
                    ("connecting", _) => format!("{} is connecting", l.name),
                    ("quiet", _) => format!("{} is connected but sending nothing", l.name),
                    _ => format!("{} has ended", l.name),
                })
                .collect::<Vec<_>>()
                .join("; "),
            Why::Receiver => "the receiver has no fix".into(),
            Why::Silent => "the receiver is talking but sends no position".into(),
        }
    }
}

/// `$XDG_RUNTIME_DIR/omakeel/keel.sock`.
pub fn default_socket() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_absolute())
        .map(|d| d.join("omakeel").join("keel.sock"))
}

/// What one line from omakeel says about the boat, if anything.
pub fn read(line: &str) -> Option<Update> {
    let m: Value = serde_json::from_str(line).ok()?;
    let v = m.get("v")?.as_u64()?;
    if v != 1 {
        return Some(Update::Incompatible(v));
    }
    if m.get("type")?.as_str()? != "state" {
        return None;
    }
    let fix = m.get("fix")?;
    let status = fix.get("status").and_then(Value::as_str).unwrap_or("none");
    let number = |key: &str| {
        fix.get(key)
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite())
    };
    let satellites = number("satellites")
        .filter(|n| (0.0..=255.0).contains(n))
        .map(|n| n as u32);
    let hdop = number("hdop").filter(|h| (0.0..=99.99).contains(h));
    let (lat, lon) = (number("lat"), number("lon"));
    if status == "ok"
        && let (Some(lat), Some(lon)) = (lat, lon)
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lon)
    {
        return Some(Update::Fix(
            Fix {
                lat,
                lon,
                sog_kn: number("sogKn").filter(|k| (0.0..=100.0).contains(k)),
                cog_deg: number("cogDeg"),
                utc: fix
                    .get("utc")
                    .and_then(Value::as_str)
                    .and_then(crate::time::parse_utc),
                satellites,
                hdop,
            },
            unwell(m.get("sources"))
                .into_iter()
                .map(|l| l.name)
                .collect(),
        ));
    }
    Some(Update::NoFix(why(status, m.get("sources"))))
}

/// Why omakeel has no current fix, from its `fix.status` and `sources`.
///
/// A receiver that says it has no fix is talking, so the link is fine
/// whatever the other sources are doing (an AIS receiver can be quiet on an
/// empty bay). Otherwise the sources that aren't delivering are the reason,
/// and when they all are, the receiver is sending something other than a
/// position.
fn why(status: &str, sources: Option<&Value>) -> Why {
    if status == "nofix" {
        return Why::Receiver;
    }
    let links = unwell(sources);
    if links.is_empty() {
        Why::Silent
    } else {
        Why::Links(links)
    }
}

/// The sources that aren't delivering, as omakeel lists them.
fn unwell(sources: Option<&Value>) -> Vec<Link> {
    sources
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(32)
        .filter_map(|s| {
            let state = match s.get("status").and_then(Value::as_str)? {
                "error" => "down",
                // After an error this is the link coming back, not going: the
                // bridge answered, and the first line hasn't come yet.
                "connecting" => "connecting",
                "quiet" => "quiet",
                "ended" => "ended",
                _ => return None,
            };
            Some(Link {
                name: text(s.get("name"))?,
                state,
                message: text(s.get("message")),
            })
        })
        .collect()
}

/// A short, single-line string from the hub, or nothing. It goes into a
/// markdown note, so no line breaks and nothing that could run on for pages.
fn text(v: Option<&Value>) -> Option<String> {
    let s: String = v?
        .as_str()?
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(120)
        .collect();
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// The boat's position right now, for a one-shot command like a note. The
/// first message after a connection is a full `state`, so one line is
/// usually enough; anything else on the socket is read past until `wait` runs
/// out. A hub that is down, silent, or speaking another version gives no
/// position at all, and the note is filed without one rather than held up.
///
/// Only a current fix counts. A stale one says where the boat was minutes
/// ago, which would put the crew's words in the wrong place on the chart.
pub fn once(socket: &Path, wait: Duration) -> Option<Fix> {
    let stream = std::os::unix::net::UnixStream::connect(socket).ok()?;
    stream.set_read_timeout(Some(wait)).ok()?;
    let deadline = Instant::now() + wait;
    let mut reader = std::io::BufReader::new(stream.take(MAX_LINE));
    let mut line = String::new();
    while Instant::now() < deadline {
        line.clear();
        match std::io::BufRead::read_line(&mut reader, &mut line) {
            Ok(0) | Err(_) => return None,
            Ok(_) => match read(line.trim_end()) {
                Some(Update::Fix(fix, _)) => return Some(fix),
                Some(Update::NoFix(_) | Update::Incompatible(_)) => return None,
                _ => {}
            },
        }
    }
    None
}

/// Follow omakeel, reconnecting for as long as the log is running. The first
/// message after a connection is a full `state`, so nothing is missed.
pub fn follow(socket: PathBuf) -> mpsc::Receiver<Update> {
    let (tx, rx) = mpsc::channel(16);
    tokio::spawn(async move {
        let mut connected = false;
        loop {
            match UnixStream::connect(&socket).await {
                Ok(stream) => {
                    connected = true;
                    let mut reader = BufReader::new(stream);
                    let mut line = Vec::new();
                    loop {
                        line.clear();
                        // The cap is per line, so a long day on the socket
                        // can't end the connection; only one huge line can.
                        let read_line = (&mut reader)
                            .take(MAX_LINE)
                            .read_until(b'\n', &mut line)
                            .await;
                        match read_line {
                            Ok(0) => break,
                            Ok(_) if !line.ends_with(b"\n") => break, // a line past the cap
                            Ok(_) => {
                                let Ok(text) = std::str::from_utf8(&line) else {
                                    continue;
                                };
                                if let Some(update) = read(text.trim_end())
                                    && tx.send(update).await.is_err()
                                {
                                    return;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
                Err(_) => {
                    if connected {
                        connected = false;
                    }
                }
            }
            if tx.send(Update::Lost).await.is_err() {
                return;
            }
            sleep(Duration::from_secs(2)).await;
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &str = r#"{"type":"state","v":1,"fix":{"status":"ok","lat":37.864711,"lon":-122.3207314,"sogKn":5.0,"cogDeg":255.0,"utc":"2026-09-13T21:00:10Z","satellites":9,"hdop":0.9,"ageSeconds":0}}"#;

    #[test]
    fn reads_a_fix() {
        let Some(Update::Fix(fix, _)) = read(STATE) else {
            panic!("no fix");
        };
        assert_eq!(fix.lat, 37.864_711);
        assert_eq!(fix.sog_kn, Some(5.0));
        assert_eq!(fix.cog_deg, Some(255.0));
        assert_eq!(fix.utc, Some(1_789_333_210));
        assert_eq!((fix.satellites, fix.hdop), (Some(9), Some(0.9)));
    }

    #[test]
    fn a_stale_fix_is_no_fix() {
        let line = STATE.replace("\"ok\"", "\"stale\"");
        assert_eq!(read(&line), Some(Update::NoFix(Why::Silent)));
    }

    #[test]
    fn no_position_at_all() {
        let line = r#"{"type":"state","v":1,"fix":{"status":"none"}}"#;
        assert_eq!(read(line), Some(Update::NoFix(Why::Silent)));
    }

    /// What the log says for each way a fix goes missing. These are the
    /// lines the crew reads back at the marina, so they are pinned here.
    #[test]
    fn says_why_there_is_no_fix() {
        let state = |fix: &str, sources: &str| {
            let line = format!(r#"{{"type":"state","v":1,"fix":{fix},"sources":{sources}}}"#);
            match read(&line) {
                Some(Update::NoFix(why)) => why.say(),
                other => panic!("{other:?}"),
            }
        };
        let stale = r#"{"status":"stale","lat":37.86,"lon":-122.31,"ageSeconds":7}"#;
        assert_eq!(
            state(
                stale,
                r#"[{"name":"tcp:10.0.2.2:10110","status":"error","message":"Connection refused (os error 111)"}]"#
            ),
            "tcp:10.0.2.2:10110 is down (Connection refused (os error 111))"
        );
        assert_eq!(
            state(
                stale,
                r#"[{"name":"tcp:10.0.2.2:10110","status":"connecting"}]"#
            ),
            "tcp:10.0.2.2:10110 is connecting"
        );
        assert_eq!(
            state(
                stale,
                r#"[{"name":"tcp:10.0.2.2:10110","status":"quiet"},{"name":"serial:/dev/ttyACM0:38400","status":"ok"}]"#
            ),
            "tcp:10.0.2.2:10110 is connected but sending nothing"
        );
        assert_eq!(
            state(stale, r#"[{"name":"tcp:10.0.2.2:10110","status":"ok"}]"#),
            "the receiver is talking but sends no position"
        );
        // Talking and saying it has no fix: the link is not the problem, even
        // with a quiet AIS receiver beside it.
        assert_eq!(
            state(
                r#"{"status":"nofix","satellites":3}"#,
                r#"[{"name":"tcp:10.0.2.2:10110","status":"ok"},{"name":"serial:/dev/ttyACM0:38400","status":"quiet"}]"#
            ),
            "the receiver has no fix"
        );
        assert_eq!(Why::Hub.say(), "omakeel, the hub, isn't answering");
    }

    #[test]
    fn a_good_fix_names_the_sources_already_idle() {
        let line = STATE.replace(
            r#""ageSeconds":0}"#,
            r#""ageSeconds":0},"sources":[{"name":"tcp:gps:10110","status":"ok"},{"name":"serial:/dev/ais:38400","status":"quiet"}]"#,
        );
        let Some(Update::Fix(_, idle)) = read(&line) else {
            panic!("no fix");
        };
        assert_eq!(idle, ["serial:/dev/ais:38400"]);
        let quiet = |name: &str| Link {
            name: name.into(),
            state: "quiet",
            message: None,
        };
        let both = Why::Links(vec![quiet("tcp:gps:10110"), quiet("serial:/dev/ais:38400")]);
        assert_eq!(
            both.without(&idle),
            Why::Links(vec![quiet("tcp:gps:10110")])
        );
        let ais = Why::Links(vec![quiet("serial:/dev/ais:38400")]);
        assert_eq!(ais.without(&idle), Why::Silent);
    }

    #[test]
    fn a_retrying_link_is_one_reason() {
        let down = |message: Option<&str>| {
            Why::Links(vec![Link {
                name: "tcp:10.0.2.2:10110".into(),
                state: "down",
                message: message.map(String::from),
            }])
        };
        assert!(down(Some("Connection refused")).same_as(&down(None)));
        let quiet = Why::Links(vec![Link {
            name: "tcp:10.0.2.2:10110".into(),
            state: "quiet",
            message: None,
        }]);
        assert!(!down(None).same_as(&quiet));
        assert!(!down(None).same_as(&Why::Hub));
    }

    #[test]
    fn a_hostile_message_stays_on_one_short_line() {
        let long = "x".repeat(500);
        let line = format!(
            r#"{{"type":"state","v":1,"fix":{{"status":"stale"}},"sources":[{{"name":"tcp:a:1","status":"error","message":"bad\n- **00:00** forged{long}"}}]}}"#
        );
        let Some(Update::NoFix(why)) = read(&line) else {
            panic!("no reason");
        };
        let said = why.say();
        assert!(!said.contains('\n'), "{said}");
        assert!(said.len() < 200, "{said}");
    }

    #[test]
    fn another_version_is_not_guessed_at() {
        let line = STATE.replace("\"v\":1", "\"v\":2");
        assert_eq!(read(&line), Some(Update::Incompatible(2)));
    }

    #[test]
    fn other_messages_are_ignored() {
        for line in [
            r#"{"type":"hello","v":1,"keel":"0.1.0"}"#,
            r#"{"type":"targets","v":1,"targets":[]}"#,
            "not json",
            "",
        ] {
            assert_eq!(read(line), None, "{line}");
        }
    }

    #[test]
    fn hostile_numbers_are_refused() {
        for bad in ["\"lat\":91.0", "\"lat\":null", "\"lon\":-999.0"] {
            let line = STATE.replace("\"lat\":37.864711", bad);
            assert!(matches!(read(&line), Some(Update::NoFix(_))), "{bad}");
        }
        let line = STATE.replace("\"sogKn\":5.0", "\"sogKn\":1e9");
        let Some(Update::Fix(fix, _)) = read(&line) else {
            panic!("no fix");
        };
        assert_eq!(fix.sog_kn, None);
    }

    /// A hub that says `lines`, then closes. Returns the socket's path.
    fn hub(name: &str, lines: &[&str]) -> PathBuf {
        use std::io::Write;
        let dir =
            std::env::temp_dir().join(format!("omalogbook-keel-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keel.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let said: Vec<String> = lines.iter().map(|l| format!("{l}\n")).collect();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                for line in said {
                    if stream.write_all(line.as_bytes()).is_err() {
                        return;
                    }
                }
            }
        });
        path
    }

    const WAIT: Duration = Duration::from_secs(2);

    #[test]
    fn one_shot_takes_the_position_from_the_first_state() {
        let socket = hub("first", &[STATE]);
        let fix = once(&socket, WAIT).expect("a fix");
        assert_eq!(fix.lat, 37.864_711);
        assert_eq!(fix.lon, -122.320_731_4);
    }

    #[test]
    fn one_shot_reads_past_what_it_does_not_understand() {
        let socket = hub("chatter", &["not json", r#"{"type":"hello","v":1}"#, STATE]);
        assert!(once(&socket, WAIT).is_some());
    }

    /// A position from minutes ago would put the crew's words in the wrong
    /// place, so a note is better off with none.
    #[test]
    fn one_shot_refuses_a_stale_fix() {
        let stale = STATE.replace("\"status\":\"ok\"", "\"status\":\"stale\"");
        let socket = hub("stale", &[&stale]);
        assert_eq!(once(&socket, WAIT), None);
    }

    #[test]
    fn one_shot_gives_up_on_a_hub_that_is_not_there() {
        let missing = std::env::temp_dir().join("omalogbook-keel-nothing-here.sock");
        let _ = std::fs::remove_file(&missing);
        assert_eq!(once(&missing, Duration::from_millis(50)), None);
    }

    #[test]
    fn one_shot_gives_up_on_a_hub_with_nothing_to_say() {
        let socket = hub("silent", &[]);
        assert_eq!(once(&socket, Duration::from_millis(50)), None);
    }
}
