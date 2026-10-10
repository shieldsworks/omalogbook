//! `~/.config/omalogbook/config.toml`: where the log lives, the boat's name,
//! and what counts as under way.

use std::ffi::{OsStr, OsString};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoHome;

impl std::fmt::Display for NoHome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HOME is unset")
    }
}

impl std::error::Error for NoHome {}

enum VaultAsk {
    Default,
    Tilde(String),
    Given(PathBuf),
}

const DEFAULT_STOPPED_KN: f64 = 0.5;

impl Settings {
    /// Read `key = value` lines, keeping the defaults for anything missing and
    /// reporting what it could not understand.
    pub fn parse(text: &str) -> Result<(Settings, Vec<String>), NoHome> {
        parse_with(text, None, std::env::var_os("HOME"))
    }
}

pub(crate) fn usable_env_path(value: Option<OsString>) -> Option<PathBuf> {
    match value {
        Some(value) if !value.is_empty() => {
            let path = PathBuf::from(value);
            path.is_absolute().then_some(path)
        }
        _ => None,
    }
}

fn home_from(value: Option<&OsStr>) -> Result<PathBuf, NoHome> {
    value.map(PathBuf::from).ok_or(NoHome)
}

fn classify_vault(value: &str) -> VaultAsk {
    match value.strip_prefix("~/") {
        Some(rest) => VaultAsk::Tilde(rest.to_string()),
        None => VaultAsk::Given(PathBuf::from(value)),
    }
}

fn resolve_vault(
    ask: VaultAsk,
    chosen_vault: Option<&Path>,
    home: Option<&OsStr>,
) -> Result<PathBuf, NoHome> {
    if let Some(path) = chosen_vault {
        return Ok(PathBuf::from(path));
    }
    match ask {
        VaultAsk::Given(path) => Ok(path),
        VaultAsk::Default => Ok(home_from(home)?.join("Logbook")),
        VaultAsk::Tilde(rest) => Ok(home_from(home)?.join(rest)),
    }
}

fn parse_with(
    text: &str,
    chosen_vault: Option<&Path>,
    home: Option<OsString>,
) -> Result<(Settings, Vec<String>), NoHome> {
    let mut boat = String::from("Dash");
    let mut every_minutes: u32 = 60;
    let mut underway_kn = 1.0;
    let mut stopped_kn = DEFAULT_STOPPED_KN;
    let mut stop_after_minutes: u32 = 5;
    let mut point_seconds: u32 = 10;
    let mut git = true;
    let mut ask = VaultAsk::Default;
    let mut problems = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with('[') {
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
            "vault" => ask = classify_vault(value),
            "boat" if !value.is_empty() => boat = value.to_string(),
            "boat" => problems.push("boat: expected a name".into()),
            "every_minutes" => {
                if let Some(v) = number(1.0, 720.0) {
                    every_minutes = v as u32;
                }
            }
            "underway_kn" => {
                if let Some(v) = number(0.1, 20.0) {
                    underway_kn = v;
                }
            }
            "stopped_kn" => {
                if let Some(v) = number(0.0, 20.0) {
                    stopped_kn = v;
                }
            }
            "stop_after_minutes" => {
                if let Some(v) = number(1.0, 240.0) {
                    stop_after_minutes = v as u32;
                }
            }
            "point_seconds" => {
                if let Some(v) = number(1.0, 600.0) {
                    point_seconds = v as u32;
                }
            }
            "git" => match value {
                "true" | "false" => git = value == "true",
                _ => problems.push("git: expected true or false".into()),
            },
            other => problems.push(format!("unknown setting {other}")),
        }
    }
    if stopped_kn > underway_kn {
        problems.push("stopped_kn: must not be above underway_kn; using the default".into());
        stopped_kn = DEFAULT_STOPPED_KN.min(underway_kn);
    }
    let vault = resolve_vault(ask, chosen_vault, home.as_deref())?;
    Ok((
        Settings {
            vault,
            boat,
            every_minutes,
            underway_kn,
            stopped_kn,
            stop_after_minutes,
            point_seconds,
            git,
        },
        problems,
    ))
}

/// `~/.config/omalogbook/config.toml`, honoring `XDG_CONFIG_HOME`.
pub fn default_path() -> Result<PathBuf, NoHome> {
    default_path_with(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

fn default_path_with(xdg: Option<OsString>, home: Option<OsString>) -> Result<PathBuf, NoHome> {
    let dir = match usable_env_path(xdg) {
        Some(dir) => dir,
        None => home_from(home.as_deref())?.join(".config"),
    };
    Ok(dir.join("omalogbook").join("config.toml"))
}

pub fn load(path: &Path, chosen_vault: Option<&Path>) -> Result<(Settings, Vec<String>), NoHome> {
    let home = std::env::var_os("HOME");
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let (settings, problems) = parse_with(&text, chosen_vault, home)?;
            Ok((
                settings,
                problems
                    .into_iter()
                    .map(|problem| format!("config.toml: {problem}"))
                    .collect(),
            ))
        }
        Err(err) => match parse_with("", chosen_vault, home) {
            Err(missing) => Err(missing),
            Ok((settings, _)) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok((settings, Vec::new()))
            }
            Ok((settings, _)) => Ok((settings, vec![format!("config.toml: {err}")])),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_is_the_defaults() {
        let (s, problems) = parse_with("", None, Some(OsString::from("/home/ada"))).unwrap();
        assert_eq!(
            s,
            Settings {
                vault: PathBuf::from("/home/ada/Logbook"),
                boat: "Dash".into(),
                every_minutes: 60,
                underway_kn: 1.0,
                stopped_kn: 0.5,
                stop_after_minutes: 5,
                point_seconds: 10,
                git: true,
            }
        );
        assert!(problems.is_empty());
    }

    #[test]
    fn reads_the_settings() {
        let (s, problems) = Settings::parse(
            "# the log\nvault = \"/boat/log\"\nboat = \"Dash\"\nevery_minutes = 30\ngit = false\n",
        )
        .unwrap();
        assert_eq!(s.vault, PathBuf::from("/boat/log"));
        assert_eq!(s.every_minutes, 30);
        assert!(!s.git);
        assert!(problems.is_empty());
    }

    #[test]
    fn keeps_going_past_a_bad_line() {
        let (s, problems) = parse_with(
            "every_minutes = soon\nboat = \"Dash\"\n",
            None,
            Some(OsString::from("/home/ada")),
        )
        .unwrap();
        assert_eq!(s.vault, PathBuf::from("/home/ada/Logbook"));
        assert_eq!(s.every_minutes, 60);
        assert_eq!(s.boat, "Dash");
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn a_stop_speed_above_the_start_speed_is_refused() {
        let (s, problems) = parse_with(
            "underway_kn = 1.0\nstopped_kn = 4.0\n",
            None,
            Some(OsString::from("/home/ada")),
        )
        .unwrap();
        assert_eq!(s.vault, PathBuf::from("/home/ada/Logbook"));
        assert!(s.stopped_kn <= s.underway_kn);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn names_the_unknown_setting() {
        let (s, problems) = parse_with(
            "vessel = \"Dash\"\n",
            None,
            Some(OsString::from("/home/ada")),
        )
        .unwrap();
        assert_eq!(s.vault, PathBuf::from("/home/ada/Logbook"));
        assert_eq!(problems, vec!["unknown setting vessel"]);
    }

    #[test]
    fn a_section_line_is_skipped() {
        let text = "\
# comment
point_seconds = 15
  [spaced]
[boat]
vault = \"/boat/log\"
boat = \"Dash\"
[watch] # still a header
every_minutes = 30
not a pair
";
        let (s, problems) = parse_with(text, None, None).unwrap();
        assert_eq!(
            s,
            Settings {
                vault: PathBuf::from("/boat/log"),
                boat: "Dash".into(),
                every_minutes: 30,
                underway_kn: 1.0,
                stopped_kn: 0.5,
                stop_after_minutes: 5,
                point_seconds: 15,
                git: true,
            }
        );
        assert_eq!(problems, vec!["line 9: expected key = value"]);
    }

    #[test]
    fn usable_env_path_keeps_an_absolute_directory() {
        assert_eq!(usable_env_path(None), None);
        assert_eq!(usable_env_path(Some(OsString::new())), None);
        assert_eq!(usable_env_path(Some(OsString::from("run/user"))), None);
        assert_eq!(
            usable_env_path(Some(OsString::from("/run/user/1000"))),
            Some(PathBuf::from("/run/user/1000")),
        );
    }

    #[test]
    fn default_path_with_falls_back_to_home() {
        assert_eq!(default_path_with(None, None), Err(NoHome));
        assert_eq!(
            default_path_with(Some(OsString::from("/cfg")), None).unwrap(),
            PathBuf::from("/cfg/omalogbook/config.toml"),
        );
        assert_eq!(
            default_path_with(
                Some(OsString::from("cfg")),
                Some(OsString::from("/home/ada"))
            )
            .unwrap(),
            PathBuf::from("/home/ada/.config/omalogbook/config.toml"),
        );
    }

    #[test]
    fn a_chosen_vault_does_not_read_home() {
        assert_eq!(
            resolve_vault(
                VaultAsk::Tilde("nope".into()),
                Some(Path::new("/boat/log")),
                None,
            )
            .unwrap(),
            PathBuf::from("/boat/log"),
        );
    }

    #[test]
    fn no_home_says_home_is_unset() {
        assert_eq!(NoHome.to_string(), "HOME is unset");
    }

    #[test]
    fn a_tilde_vault_joins_the_home_directory() {
        let (s, problems) = parse_with(
            "vault = \"~/Logbook\"\n",
            None,
            Some(OsString::from("/home/sailor")),
        )
        .unwrap();
        assert_eq!(s.vault, PathBuf::from("/home/sailor/Logbook"));
        assert!(problems.is_empty());
        assert_eq!(
            parse_with("vault = \"~/Logbook\"\n", None, None).unwrap_err(),
            NoHome
        );
    }

    #[test]
    fn an_empty_home_is_still_a_home() {
        let (s, problems) = parse_with("", None, Some(OsString::from(""))).unwrap();
        assert_eq!(s.vault, PathBuf::from("Logbook"));
        assert!(problems.is_empty());
    }
}
