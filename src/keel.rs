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
    /// A fix less than five seconds old. A stale one still tells you where the
    /// boat was, but it is not a position to log as current.
    pub current: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Update {
    /// omakeel's latest fix, or None when it has no position at all.
    Fix(Option<Fix>),
    /// Not connected to omakeel.
    Lost,
    /// omakeel speaks another protocol version.
    Incompatible(u64),
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
    let (lat, lon) = (number("lat"), number("lon"));
    let boat = match (lat, lon) {
        (Some(lat), Some(lon))
            if status != "none"
                && (-90.0..=90.0).contains(&lat)
                && (-180.0..=180.0).contains(&lon) =>
        {
            Some(Fix {
                lat,
                lon,
                sog_kn: number("sogKn").filter(|k| (0.0..=100.0).contains(k)),
                cog_deg: number("cogDeg"),
                utc: fix
                    .get("utc")
                    .and_then(Value::as_str)
                    .and_then(crate::time::parse_utc),
                current: status == "ok",
            })
        }
        _ => None,
    };
    Some(Update::Fix(boat))
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
                Some(Update::Fix(fix)) => return fix.filter(|f| f.current),
                Some(Update::Incompatible(_)) => return None,
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

    const STATE: &str = r#"{"type":"state","v":1,"fix":{"status":"ok","lat":37.864711,"lon":-122.3207314,"sogKn":5.0,"cogDeg":255.0,"utc":"2026-09-13T21:00:10Z","ageSeconds":0}}"#;

    #[test]
    fn reads_a_fix() {
        let Some(Update::Fix(Some(fix))) = read(STATE) else {
            panic!("no fix");
        };
        assert_eq!(fix.lat, 37.864_711);
        assert_eq!(fix.sog_kn, Some(5.0));
        assert_eq!(fix.cog_deg, Some(255.0));
        assert_eq!(fix.utc, Some(1_789_333_210));
        assert!(fix.current);
    }

    #[test]
    fn a_stale_fix_is_still_a_position() {
        let line = STATE.replace("\"ok\"", "\"stale\"");
        let Some(Update::Fix(Some(fix))) = read(&line) else {
            panic!("no fix");
        };
        assert!(!fix.current);
    }

    #[test]
    fn no_position_at_all() {
        let line = r#"{"type":"state","v":1,"fix":{"status":"none"}}"#;
        assert_eq!(read(line), Some(Update::Fix(None)));
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
            assert_eq!(read(&line), Some(Update::Fix(None)), "{bad}");
        }
        let line = STATE.replace("\"sogKn\":5.0", "\"sogKn\":1e9");
        let Some(Update::Fix(Some(fix))) = read(&line) else {
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
