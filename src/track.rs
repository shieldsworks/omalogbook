//! Where the boat actually went, as GPX 1.1: the one track format every
//! chartplotter, phone app and mapping site already reads. Written by hand,
//! because it is a dozen lines of XML.

use crate::time;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub epoch: i64,
    pub lat: f64,
    pub lon: f64,
    pub sog_kn: Option<f64>,
}

/// One passage: from getting under way to stopping.
#[derive(Debug)]
pub struct Track {
    pub relative: String,
    path: PathBuf,
    name: String,
    points: Vec<Point>,
}

impl Track {
    /// A track named for when it started: `tracks/2026-09-18-0915.gpx`.
    pub fn start(vault: &Path, epoch: i64, boat: &str) -> Track {
        let at = time::local(epoch);
        // Seconds as well as minutes: two passages can start in one minute.
        let relative = format!(
            "tracks/{}-{:02}{:02}{:02}.gpx",
            at.date(),
            at.hour,
            at.minute,
            at.second
        );
        Track {
            path: vault.join(&relative),
            name: format!("{boat} {} {}", at.date(), at.clock()),
            relative,
            points: Vec::new(),
        }
    }

    /// Keep a point. Repeats of the same second are dropped, so a chatty
    /// receiver can't pad the file.
    pub fn push(&mut self, point: Point) {
        if self.points.last().is_some_and(|p| p.epoch == point.epoch) {
            return;
        }
        self.points.push(point);
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Distance along the track so far, in nautical miles.
    pub fn distance_nm(&self) -> f64 {
        self.points
            .windows(2)
            .map(|w| crate::geo::distance_nm((w[0].lat, w[0].lon), (w[1].lat, w[1].lon)))
            .sum()
    }

    /// Write the whole track. Short enough to rewrite each time, which keeps
    /// the file valid XML even if the power goes mid-passage.
    pub fn save(&self) -> io::Result<()> {
        if self.points.is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = self.path.with_extension("gpx.tmp");
        {
            let mut file = fs::File::create(&temp)?;
            file.write_all(self.gpx().as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&temp, &self.path)
    }

    fn gpx(&self) -> String {
        let mut out = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <gpx version=\"1.1\" creator=\"omalogbook\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n",
        );
        out.push_str(&format!(
            "  <trk>\n    <name>{}</name>\n    <trkseg>\n",
            escape(&self.name)
        ));
        for p in &self.points {
            out.push_str(&format!(
                "      <trkpt lat=\"{:.6}\" lon=\"{:.6}\">\n        <time>{}</time>\n",
                p.lat,
                p.lon,
                time::iso_utc(p.epoch)
            ));
            if let Some(kn) = p.sog_kn.filter(|k| k.is_finite() && *k >= 0.0) {
                // GPX speed is meters per second, in the standard extension slot.
                out.push_str(&format!(
                    "        <extensions>\n          <speed>{:.2}</speed>\n        </extensions>\n",
                    kn * 0.514_444
                ));
            }
            out.push_str("      </trkpt>\n");
        }
        out.push_str("    </trkseg>\n  </trk>\n</gpx>\n");
        out
    }
}

/// XML's five, so a boat named `Jack & Jill <2>` can't break the file.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> Track {
        let mut t = Track::start(Path::new("/tmp/vault"), 1_789_300_800, "Dash");
        t.push(Point {
            epoch: 1_789_300_800,
            lat: 37.8663,
            lon: -122.3148,
            sog_kn: Some(5.1),
        });
        t.push(Point {
            epoch: 1_789_646_410,
            lat: 37.8700,
            lon: -122.3200,
            sog_kn: None,
        });
        t
    }

    #[test]
    fn writes_gpx_with_a_point_per_fix() {
        let gpx = track().gpx();
        assert!(gpx.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert_eq!(gpx.matches("<trkpt").count(), 2);
        assert!(gpx.contains("lat=\"37.866300\" lon=\"-122.314800\""));
        assert!(gpx.contains("<time>2026-09-13T12:00:00Z</time>"));
        assert!(gpx.contains("<speed>2.62</speed>"));
        assert!(gpx.trim_end().ends_with("</gpx>"));
    }

    #[test]
    fn a_repeated_second_is_dropped() {
        let mut t = track();
        t.push(Point {
            epoch: 1_789_646_410,
            lat: 37.9,
            lon: -122.4,
            sog_kn: None,
        });
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn measures_the_passage() {
        assert!(
            (track().distance_nm() - 0.35).abs() < 0.05,
            "{}",
            track().distance_nm()
        );
    }

    #[test]
    fn a_name_cannot_break_the_xml() {
        let mut t = Track::start(Path::new("/tmp/vault"), 1_789_300_800, "Jack & Jill <2>");
        t.push(Point {
            epoch: 1_789_300_800,
            lat: 1.0,
            lon: 2.0,
            sog_kn: None,
        });
        assert!(t.gpx().contains("Jack &amp; Jill &lt;2&gt;"));
        assert!(!t.gpx().contains("<2>"));
    }

    #[test]
    fn an_empty_track_writes_nothing() {
        let t = Track::start(Path::new("/tmp/omalogbook-never"), 0, "Dash");
        assert!(t.is_empty());
        t.save().unwrap();
        assert!(!Path::new("/tmp/omalogbook-never").exists());
    }
}
