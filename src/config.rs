//! `~/.config/omalogbook/config.toml`: where the log lives, the boat's name,
//! and what counts as under way.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// The vault: a folder of markdown, and a git repo if you want one.
    pub vault: PathBuf,
    pub boat: String,
    /// Minutes between position entries while under way.
    pub every_minutes: u32,
    /// Speed over ground that starts a passage, in knots.
    pub underway_kn: f64,
    /// Below this, for [`Settings::stop_after_minutes`], the passage has ended.
    pub stopped_kn: f64,
    pub stop_after_minutes: u32,
    /// Seconds between track points.
    pub point_seconds: u32,
    /// Commit the vault as the log is written.
    pub git: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            vault: home().join("Logbook"),
            boat: "Dash".into(),
            every_minutes: 60,
            underway_kn: 1.0,
            stopped_kn: 0.5,
            stop_after_minutes: 5,
            point_seconds: 10,
            git: true,
        }
    }
}

impl Settings {
    /// Read `key = value` lines, keeping the defaults for anything missing and
    /// reporting what it could not understand rather than stopping.
    pub fn parse(text: &str) -> (Settings, Vec<String>) {
        let mut s = Settings::default();
        let mut problems = Vec::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                problems.push(format!("line {}: expected key = value", n + 1));
                continue;
            };
            let (key, value) = (key.trim(), value.trim().trim_matches('"'));
            let mut number = |min: f64, max: f64| match value.parse::<f64>() {
                Ok(v) if v.is_finite() && (min..=max).contains(&v) => Some(v),
                _ => {
                    problems.push(format!("{key}: expected a number from {min} to {max}"));
                    None
                }
            };
            match key {
                "vault" => s.vault = expand(value),
                "boat" if !value.is_empty() => s.boat = value.to_string(),
                "boat" => problems.push("boat: expected a name".into()),
                "every_minutes" => {
                    if let Some(v) = number(1.0, 720.0) {
                        s.every_minutes = v as u32;
                    }
                }
                "underway_kn" => {
                    if let Some(v) = number(0.1, 20.0) {
                        s.underway_kn = v;
                    }
                }
                "stopped_kn" => {
                    if let Some(v) = number(0.0, 20.0) {
                        s.stopped_kn = v;
                    }
                }
                "stop_after_minutes" => {
                    if let Some(v) = number(1.0, 240.0) {
                        s.stop_after_minutes = v as u32;
                    }
                }
                "point_seconds" => {
                    if let Some(v) = number(1.0, 600.0) {
                        s.point_seconds = v as u32;
                    }
                }
                "git" => match value {
                    "true" | "false" => s.git = value == "true",
                    _ => problems.push("git: expected true or false".into()),
                },
                other => problems.push(format!("unknown setting {other}")),
            }
        }
        if s.stopped_kn > s.underway_kn {
            problems.push("stopped_kn: must not be above underway_kn; using the default".into());
            s.stopped_kn = Settings::default().stopped_kn.min(s.underway_kn);
        }
        (s, problems)
    }
}

/// `~/.config/omalogbook/config.toml`, honoring `XDG_CONFIG_HOME`.
pub fn default_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".config"))
        .join("omalogbook")
        .join("config.toml")
}

pub fn load(path: &Path) -> (Settings, Vec<String>) {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let (s, p) = Settings::parse(&text);
            (
                s,
                p.into_iter().map(|p| format!("config.toml: {p}")).collect(),
            )
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Settings::default(), Vec::new()),
        Err(e) => (Settings::default(), vec![format!("config.toml: {e}")]),
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `~/Logbook` and `$HOME/Logbook` both mean the same folder.
fn expand(value: &str) -> PathBuf {
    match value.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_is_the_defaults() {
        let (s, problems) = Settings::parse("");
        assert_eq!(s, Settings::default());
        assert!(problems.is_empty());
    }

    #[test]
    fn reads_the_settings() {
        let (s, problems) = Settings::parse(
            "# the log\nvault = \"/boat/log\"\nboat = \"Dash\"\nevery_minutes = 30\ngit = false\n",
        );
        assert_eq!(s.vault, PathBuf::from("/boat/log"));
        assert_eq!(s.every_minutes, 30);
        assert!(!s.git);
        assert!(problems.is_empty());
    }

    #[test]
    fn keeps_going_past_a_bad_line() {
        let (s, problems) = Settings::parse("every_minutes = soon\nboat = \"Dash\"\n");
        assert_eq!(s.every_minutes, 60);
        assert_eq!(s.boat, "Dash");
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn a_stop_speed_above_the_start_speed_is_refused() {
        let (s, problems) = Settings::parse("underway_kn = 1.0\nstopped_kn = 4.0\n");
        assert!(s.stopped_kn <= s.underway_kn);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn names_the_unknown_setting() {
        let (_, problems) = Settings::parse("vessel = \"Dash\"\n");
        assert_eq!(problems, vec!["unknown setting vessel"]);
    }
}
