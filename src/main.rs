use omalogbook::{config, day, entry, keel, lock, preset, time, watch::Watch, wind};
use std::{path::PathBuf, process::ExitCode};

const USAGE: &str = "\
omalogbook — the ship's log for Omahoy

usage: omalogbook run [--vault DIR] [--socket PATH] [--no-git]
       omalogbook note TEXT...
       omalogbook presets [--json]
       omalogbook today [--json] [--date YYYY-MM-DD]
       omalogbook path [--date YYYY-MM-DD]
       omalogbook vault
       omalogbook --help | --version

run     follow omakeel and write the log: entries, the day's run, and a GPX
        track for every passage
note    add your own entry to today's note, at the boat's position when
        omakeel has one. A line that opens with a mark — /depart, /anchor,
        /reef and the rest — also carries what it was blowing
presets list the marks
today   print a day's entries and its totals, or --json for a window to read
path    print the path of a day's note, for an editor to open
vault   print the folder the log lives in

Settings live in ~/.config/omalogbook/config.toml: vault, boat, every_minutes,
underway_kn, stopped_kn, stop_after_minutes, point_seconds, git.
";

/// How long a note waits for omakeel to say where the boat is. Long enough
/// for a hub that is up, short enough that a hub that is gone doesn't hold
/// the crew's words on the foredeck.
const FIX_WAIT: std::time::Duration = std::time::Duration::from_millis(600);

/// And how long a mark waits for omawind. Two messages rather than one, so
/// a little longer, and still nothing the crew would notice.
const WIND_WAIT: std::time::Duration = std::time::Duration::from_millis(900);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: Vec<&str> = args.iter().map(String::as_str).collect();
    match rest.split_first() {
        None | Some((&"--help" | &"-h" | &"help", _)) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some((&"--version" | &"-V", _)) => {
            println!("omalogbook {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some((&"run", rest)) => run(rest),
        Some((&"note", rest)) => note(rest),
        Some((&"today", rest)) => today(rest),
        Some((&"presets", rest)) => presets(rest),
        Some((&"path", rest)) => path(rest),
        Some((&"vault", _)) => {
            println!("{}", settings(&[]).0.vault.display());
            ExitCode::SUCCESS
        }
        Some((other, _)) => {
            eprintln!("omalogbook: no such command: {other}\n\n{USAGE}");
            ExitCode::from(64)
        }
    }
}

/// Settings from the config file, with the flags on top.
fn settings(args: &[&str]) -> (config::Settings, Option<PathBuf>) {
    let (mut s, problems) = config::load(&config::default_path());
    for p in problems {
        eprintln!("omalogbook: {p}");
    }
    let mut socket = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "--vault" => {
                if let Some(v) = it.next() {
                    s.vault = PathBuf::from(v);
                }
            }
            "--socket" => socket = it.next().map(PathBuf::from),
            "--no-git" => s.git = false,
            _ => {}
        }
    }
    (s, socket)
}

fn run(args: &[&str]) -> ExitCode {
    let (settings, socket) = settings(args);
    let Some(socket) = socket.or_else(keel::default_socket) else {
        eprintln!("omalogbook: no XDG_RUNTIME_DIR; pass --socket PATH to omakeel's socket");
        return ExitCode::FAILURE;
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("omalogbook: {e}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(async move {
        let mut watch = match Watch::new(settings) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("omalogbook: {e}");
                return ExitCode::FAILURE;
            }
        };
        println!("omalogbook: {}", watch.day_path().display());
        let mut updates = keel::follow(socket);
        let mut interrupt =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("omalogbook: {e}");
                    return ExitCode::FAILURE;
                }
            };
        loop {
            tokio::select! {
                update = updates.recv() => {
                    let Some(update) = update else { break };
                    match watch.update(update, time::now()) {
                        Ok(lines) => for line in lines {
                            println!("{line}");
                        },
                        Err(e) => eprintln!("omalogbook: {e}"),
                    }
                }
                _ = tokio::signal::ctrl_c() => break,
                _ = interrupt.recv() => break,
            }
        }
        if let Err(e) = watch.close() {
            eprintln!("omalogbook: {e}");
            return ExitCode::FAILURE;
        }
        ExitCode::SUCCESS
    })
}

fn note(args: &[&str]) -> ExitCode {
    let text = args.join(" ");
    if text.trim().is_empty() {
        eprintln!("omalogbook: what should the entry say?\n\n{USAGE}");
        return ExitCode::from(64);
    }
    let (settings, _) = settings(&[]);
    let now = time::now();
    let date = time::local(now).date();
    // Where the boat is as the note is written. A hub that is down or has no
    // fix costs the position, never the note: the words are the point.
    let here = keel::default_socket().and_then(|s| keel::once(&s, FIX_WAIT));
    let typed = preset::parse(&text);
    // A mark also asks omawind what it was blowing. Only a mark: a note is
    // the crew's words, and an instrument reading stapled to them would
    // read as the machine talking over the top.
    let weather = match (&typed, here) {
        (preset::Typed::Mark(..), Some(fix)) => wind::default_socket()
            .map(|s| wind::once(&s, (fix.lat, fix.lon), now, WIND_WAIT))
            .unwrap_or_default(),
        _ => wind::Weather::default(),
    };
    let write = || -> std::io::Result<PathBuf> {
        let _lock = lock::Lock::take(&settings.vault)?;
        let mut day = day::Day::open(&settings.vault, &date, &settings.boat)?;
        let (at, utc) = (time::local(now), time::utc(now));
        day.push(match typed {
            preset::Typed::Mark(p, said) => entry::mark(
                at,
                utc,
                here.map(|f| (f.lat, f.lon, f.sog_kn, f.cog_deg)),
                p.label,
                said,
                &weather,
            ),
            _ => match here {
                Some(fix) => entry::note_at(at, utc, fix.lat, fix.lon, &text),
                None => entry::note(at, utc, &text),
            },
        });
        day.save()?;
        Ok(day.path.clone())
    };
    match write() {
        Ok(path) => {
            println!("{}", path.display());
            if let preset::Typed::Unknown(word) = typed {
                eprintln!(
                    "omalogbook: /{word} isn't a mark, so that went in as a note.\nThe marks are: {}",
                    preset::words()
                );
            }
            if settings.git {
                match omalogbook::git::commit(&settings.vault, &format!("log: {date} · a note")) {
                    Ok(_) => {}
                    Err(e) => eprintln!("omalogbook: could not commit ({e}); the note is on disk"),
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("omalogbook: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A day as it stands: what the log says, for the crew or for a window.
/// Reads the note on disk and nothing else, so it works whether or not the
/// log is running.
fn today(args: &[&str]) -> ExitCode {
    let (settings, _) = settings(&[]);
    let mut date = day::today();
    let mut json = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "--json" => json = true,
            "--date" => {
                let Some(value) = it.next() else { continue };
                if !time::valid_date(value) {
                    eprintln!("omalogbook: --date wants a day like 2026-09-18");
                    return ExitCode::from(64);
                }
                date = value.to_string();
            }
            _ => {}
        }
    }
    let path = day::path_for(&settings.vault, &date);
    // A day with no note yet is an empty day, not an error: the boat simply
    // hasn't been anywhere.
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let entries = day::entries_in(&text);
    let day = match day::Day::open(&settings.vault, &date, &settings.boat) {
        Ok(day) => day,
        Err(e) => {
            eprintln!("omalogbook: {e}");
            return ExitCode::FAILURE;
        }
    };
    if json {
        println!(
            "{}",
            serde_json::json!({
                "v": 1,
                "date": date,
                "boat": settings.boat,
                "path": path.to_string_lossy(),
                "entries": entries,
                "distanceNm": day.distance_nm,
                "maxSogKn": day.max_sog_kn,
                "underwaySecs": day.underway_secs,
                "tracks": day.tracks(),
            })
        );
        return ExitCode::SUCCESS;
    }
    println!("{date} · {}", settings.boat);
    for line in &entries {
        println!("{line}");
    }
    if !entries.is_empty() {
        println!(
            "\n{:.1} nm · fastest {:.1} kn · under way {}",
            day.distance_nm,
            day.max_sog_kn,
            day::hours(day.underway_secs)
        );
    }
    ExitCode::SUCCESS
}

/// The marks, for the crew or for the window's completion list.
fn presets(args: &[&str]) -> ExitCode {
    if args.contains(&"--json") {
        println!(
            "{}",
            serde_json::json!({
                "v": 1,
                "presets": preset::PRESETS.iter().map(|p| serde_json::json!({
                    "word": p.word, "label": p.label, "about": p.about,
                })).collect::<Vec<_>>(),
            })
        );
        return ExitCode::SUCCESS;
    }
    let width = preset::PRESETS
        .iter()
        .map(|p| p.word.len())
        .max()
        .unwrap_or(0);
    for p in preset::PRESETS {
        println!("/{:<width$}  {}", p.word, p.about);
    }
    ExitCode::SUCCESS
}

fn path(args: &[&str]) -> ExitCode {
    let (settings, _) = settings(&[]);
    let mut date = day::today();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if *arg == "--date"
            && let Some(value) = it.next()
        {
            if !time::valid_date(value) {
                eprintln!("omalogbook: --date wants a day like 2026-09-18");
                return ExitCode::from(64);
            }
            date = value.to_string();
        }
    }
    println!("{}", day::path_for(&settings.vault, &date).display());
    ExitCode::SUCCESS
}
