use omalogbook::{config::Settings, keel, time, watch::Watch};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

struct Sail {
    name: &'static str,
    steps: Vec<Step>,
}

struct Step {
    now: i64,
    update: keel::Update,
}

const START: i64 = 1_789_300_800;
const MINUTES_UNDER_WAY: i64 = 15;
const SAIL_KN: f64 = 4.7;
const STILL_KN: f64 = 0.2;
const EVERY_MINUTES: u32 = 60;
const UNDERWAY_KN: f64 = 1.0;
const STOPPED_KN: f64 = 0.5;
const STOP_AFTER_MINUTES: u32 = 5;
const POINT_SECONDS: u32 = 300;

fn morning() -> Sail {
    let mut steps = Vec::new();
    steps.push(fix(37.8663, -122.3148, STILL_KN, START));
    for step in 1..=MINUTES_UNDER_WAY {
        let lat = 37.8663 + (step as f64) * 0.0012;
        steps.push(fix(lat, -122.3148, SAIL_KN, START + step * 60));
    }
    let held = 37.8663 + (MINUTES_UNDER_WAY as f64) * 0.0012;
    let still_from = MINUTES_UNDER_WAY + 1;
    let still_until = still_from + i64::from(STOP_AFTER_MINUTES);
    for step in still_from..=still_until {
        steps.push(fix(held, -122.3148, STILL_KN, START + step * 60));
    }
    Sail {
        name: "morning",
        steps,
    }
}

fn fix(lat: f64, lon: f64, sog: f64, now: i64) -> Step {
    Step {
        now,
        update: keel::Update::Fix(
            keel::Fix {
                lat,
                lon,
                sog_kn: Some(sog),
                cog_deg: Some(12.0),
                utc: Some(now),
                satellites: Some(9),
                hdop: Some(0.9),
            },
            Vec::new(),
        ),
    }
}

fn one_segment(name: &str) -> bool {
    !name.is_empty() && name != "." && !name.contains('/') && !name.contains("..")
}

fn golden_dir(name: &str) -> io::Result<PathBuf> {
    if !one_segment(name) {
        return Err(io::Error::other(format!(
            "sail name {name:?} is empty or contains '/' or '..'"
        )));
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if dir.file_name().and_then(|n| n.to_str()) != Some(name) {
        return Err(io::Error::other(format!(
            "sail name {name:?} is not one path segment"
        )));
    }
    Ok(dir)
}

fn write_vault(sail: &Sail, vault: &Path) -> io::Result<()> {
    if !one_segment(sail.name) {
        return Err(io::Error::other(format!(
            "sail name {:?} is empty or contains '/' or '..'",
            sail.name
        )));
    }
    if std::env::var("TZ") != Ok(String::from("UTC")) {
        return Err(io::Error::other("TZ is not UTC; mise goldens sets TZ=UTC"));
    }
    let Some(first) = sail.steps.first() else {
        return Err(io::Error::other("sail has no steps"));
    };
    let offset = time::local(first.now).offset;
    if offset != 0 {
        return Err(io::Error::other(format!(
            "time::local offset is {offset} s, not 0"
        )));
    }
    if vault.exists() {
        let mut entries = std::fs::read_dir(vault)?;
        if let Some(entry) = entries.next() {
            entry?;
            return Err(io::Error::other(format!(
                "{} exists and is not empty",
                vault.display()
            )));
        }
    }
    let settings = Settings {
        vault: vault.to_path_buf(),
        boat: "Dash".into(),
        every_minutes: EVERY_MINUTES,
        underway_kn: UNDERWAY_KN,
        stopped_kn: STOPPED_KN,
        stop_after_minutes: STOP_AFTER_MINUTES,
        point_seconds: POINT_SECONDS,
        git: false,
    };
    let mut watch = Watch::new(settings, first.now)?;
    for step in &sail.steps {
        watch.update(step.update.clone(), step.now)?;
    }
    watch.close()
}

fn vault_files(dir: &Path) -> io::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut out = BTreeMap::new();
    if dir.exists() {
        collect(dir, dir, &mut out)?;
    }
    Ok(out)
}

fn collect(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out)?;
            continue;
        }
        let name = entry.file_name();
        if name == ".gitignore" || name == ".omalogbook.lock" {
            continue;
        }
        let rel = path.strip_prefix(root).map_err(io::Error::other)?;
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("md" | "gpx") => {
                let _ = out.insert(rel.to_path_buf(), std::fs::read(path)?);
            }
            _ => {
                return Err(io::Error::other(format!(
                    "unexpected file {}",
                    rel.display()
                )));
            }
        }
    }
    Ok(())
}

fn bless(dir: &Path, written: &BTreeMap<PathBuf, Vec<u8>>) -> io::Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    for (rel, bytes) in written {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

fn first_diff(rel: &Path, golden: &[u8], written: &[u8]) -> String {
    let golden_text = String::from_utf8_lossy(golden);
    let written_text = String::from_utf8_lossy(written);
    for (n, (left, right)) in golden_text.lines().zip(written_text.lines()).enumerate() {
        if left != right {
            return format!(
                "{} differs at line {}\n  golden:  {left}\n  written: {right}",
                rel.display(),
                n + 1
            );
        }
    }
    format!(
        "{} differs ({} bytes vs {} bytes)",
        rel.display(),
        golden.len(),
        written.len()
    )
}

#[test]
#[ignore = "mise goldens sets TZ=UTC"]
fn the_morning_sail_matches_the_golden() {
    let sail = morning();
    let golden = golden_dir(sail.name).unwrap();
    let vault = std::env::temp_dir().join(format!(
        "omalogbook-golden-{}-{}",
        std::process::id(),
        sail.name
    ));
    let _ = std::fs::remove_dir_all(&vault);
    write_vault(&sail, &vault).unwrap();
    let written = vault_files(&vault).unwrap();
    if std::env::var("BLESS").ok().as_deref() == Some("1") {
        bless(&golden, &written).unwrap();
    }
    let expected = vault_files(&golden).unwrap();
    let mut problems = Vec::new();
    for (rel, bytes) in &written {
        match expected.get(rel) {
            None => problems.push(format!(
                "{} was written and is not in the golden",
                rel.display()
            )),
            Some(gold) if gold != bytes => problems.push(first_diff(rel, gold, bytes)),
            Some(_) => {}
        }
    }
    for rel in expected.keys() {
        if !written.contains_key(rel) {
            problems.push(format!(
                "{} is in the golden and was not written",
                rel.display()
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&vault);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
