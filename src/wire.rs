//! Reading another engine's socket for a one-shot command, and the labels
//! that come off it.
//!
//! A mark is the crew's words and must never be held up by an engine that is
//! slow, stuck, or trickling a byte at a time: the whole read runs against
//! one deadline, re-armed before every read, rather than a timeout per read
//! that a slow sender can renew forever.

use std::{
    io::Read,
    path::Path,
    time::{Duration, Instant},
};

/// Connect to `socket` and hand each line to `take` until it returns true,
/// the engine hangs up, a line grows past `max_line`, or `wait` runs out —
/// whichever comes first. Nothing is ever read past the deadline.
pub fn read_lines(
    socket: &Path,
    wait: Duration,
    max_line: usize,
    mut take: impl FnMut(&str) -> bool,
) {
    let deadline = Instant::now() + wait;
    let Ok(mut stream) = std::os::unix::net::UnixStream::connect(socket) else {
        return;
    };
    let mut pending: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        // A zero timeout means "block forever" to the socket, so the end of
        // the wait has to be caught here.
        if left.is_zero() || stream.set_read_timeout(Some(left)).is_err() {
            return;
        }
        let n = match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        pending.extend_from_slice(&chunk[..n]);
        while let Some(end) = pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            if let Ok(text) = std::str::from_utf8(&line)
                && take(text.trim_end())
            {
                return;
            }
        }
        if pending.len() > max_line {
            return;
        }
    }
}

/// A station's name as a log can carry it: control characters gone, cut to
/// a sensible length. None when nothing is left, because a reading from a
/// station nobody can name can't be checked later.
pub fn label(name: &str) -> Option<String> {
    let name: String = name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(60)
        .collect();
    let name = name.trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// For tests: a server that sends `bytes` one at a time, `gap` apart,
/// round and round until the reader hangs up.
#[cfg(test)]
pub(crate) fn trickle(name: &str, bytes: &'static [u8], gap: Duration) -> std::path::PathBuf {
    use std::io::Write;
    {
        let dir =
            std::env::temp_dir().join(format!("omalogbook-wire-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("engine.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                for b in bytes.iter().cycle() {
                    if s.write_all(&[*b]).is_err() {
                        return;
                    }
                    std::thread::sleep(gap);
                }
            }
        });
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A byte every 100 ms renews any per-read timeout forever; the
    /// deadline has to hold anyway.
    #[test]
    fn a_trickling_engine_cannot_hold_the_mark() {
        let path = trickle("trickle", b"xxxxxxxxxx", Duration::from_millis(100));
        let start = Instant::now();
        read_lines(&path, Duration::from_millis(400), 1 << 20, |_| false);
        assert!(
            start.elapsed() < Duration::from_millis(900),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn lines_arrive_until_the_reader_has_enough() {
        let path = trickle("lines", b"one\ntwo\nthree\n", Duration::from_millis(1));
        let mut seen = Vec::new();
        read_lines(&path, Duration::from_secs(2), 1 << 20, |l| {
            seen.push(l.to_string());
            seen.len() == 2
        });
        assert_eq!(seen, ["one", "two"]);
    }

    #[test]
    fn a_line_past_the_cap_ends_the_read() {
        let path = trickle("long", b"y", Duration::ZERO);
        let start = Instant::now();
        let mut lines = 0;
        read_lines(&path, Duration::from_secs(5), 1024, |_| {
            lines += 1;
            false
        });
        assert_eq!(lines, 0);
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_label_is_cleaned_and_cut() {
        assert_eq!(label("  Alameda\u{7}\n "), Some("Alameda".into()));
        assert_eq!(label(&"x".repeat(200)).map(|l| l.len()), Some(60));
        assert_eq!(label(" \u{1b} "), None);
    }
}
