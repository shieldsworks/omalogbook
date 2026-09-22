use omalogbook::{config, day, entry, keel, lock, preset, time, totals, watch::Watch, wind};
use std::{path::PathBuf, process::ExitCode};

const USAGE: &str = "\
omalogbook — the ship's log for Omahoy

usage: omalogbook run [--vault DIR] [--socket PATH] [--no-git]
       omalogbook note TEXT...
       omalogbook amend N TEXT... [--date D] [--expect LINE]
       omalogbook strike N [--erase] [--date D] [--expect LINE]
       omalogbook presets [--json]
       omalogbook today [--json] [--date YYYY-MM-DD]
       omalogbook totals [--json] [--vault DIR]
       omalogbook path [--date YYYY-MM-DD]
       omalogbook vault
       omalogbook --help | --version

run     follow omakeel and write the log: entries, the day's run, and a GPX
        track for every passage
note    add your own entry to today's note, at the boat's position when
        omakeel has one. A line that opens with a mark — /depart, /anchor,
        /reef and the rest — also carries what it was blowing
amend   change the words of entry N, keeping its clock and its fix
strike  rule a line through entry N, or --erase to take it out
presets list the marks
today   print a day's entries and its totals, or --json for a window to read
totals  add up every day in the log: distance, time under way, average
        speed, and the records
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
        Some((&"totals", rest)) => lifetime(rest),
        Some((&"presets", rest)) => presets(rest),
        Some((&"amend", rest)) => amend(rest),
        Some((&"strike", rest)) => strike(rest),
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
    // Numbered, because `amend` and `strike` ask which one. Counted from 1:
    // the crew reads a log, not an array.
    let width = entries.len().to_string().len();
    for (n, line) in entries.iter().enumerate() {
        println!("{:>width$}. {line}", n + 1);
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

/// `N`, and the flags an entry-changing command shares: which day, and the
/// entry as the caller last saw it. What's left is the words.
struct Which {
    index: usize,
    date: String,
    expect: String,
    erase: bool,
    words: String,
}

fn which(args: &[&str]) -> Result<Which, String> {
    let mut w = Which {
        index: 0,
        date: day::today(),
        expect: String::new(),
        erase: false,
        words: String::new(),
    };
    let mut rest: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "--date" => {
                let Some(value) = it.next() else { continue };
                if !time::valid_date(value) {
                    return Err("--date wants a day like 2026-09-18".into());
                }
                w.date = value.to_string();
            }
            "--expect" => w.expect = it.next().map(|s| s.to_string()).unwrap_or_default(),
            "--erase" => w.erase = true,
            other => rest.push(other),
        }
    }
    let Some((first, words)) = rest.split_first() else {
        return Err("which entry? `omalogbook today` numbers them".into());
    };
    // Entries are counted as `today` prints them, from 1: the crew reads a
    // log, not an array.
    match first.parse::<usize>() {
        Ok(n) if n >= 1 => w.index = n - 1,
        _ => return Err(format!("{first} isn't an entry number")),
    }
    w.words = words.join(" ");
    Ok(w)
}

fn revise(args: &[&str], how: impl Fn(&Which) -> day::Revision) -> ExitCode {
    let w = match which(args) {
        Ok(w) => w,
        Err(e) => {
            eprintln!(
                "omalogbook: {e}

{USAGE}"
            );
            return ExitCode::from(64);
        }
    };
    let (settings, _) = settings(&[]);
    let how = how(&w);
    let done = || -> std::io::Result<(String, PathBuf)> {
        let _lock = lock::Lock::take(&settings.vault)?;
        let mut day = day::Day::open(&settings.vault, &w.date, &settings.boat)?;
        let line = day.revise(w.index, &w.expect, &how)?;
        Ok((line, day.path.clone()))
    };
    match done() {
        Ok((line, path)) => {
            if line.is_empty() {
                println!("{}", path.display());
            } else {
                println!("{line}");
            }
            if settings.git {
                let what = match how {
                    day::Revision::Amend(_) => "an entry amended",
                    day::Revision::Strike => "an entry struck",
                    day::Revision::Erase => "an entry erased",
                };
                match omalogbook::git::commit(&settings.vault, &format!("log: {} · {what}", w.date))
                {
                    Ok(_) => {}
                    Err(e) => eprintln!("omalogbook: could not commit ({e}); the log is on disk"),
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

fn amend(args: &[&str]) -> ExitCode {
    revise(args, |w| day::Revision::Amend(w.words.clone()))
}

fn strike(args: &[&str]) -> ExitCode {
    revise(args, |w| {
        if w.erase {
            day::Revision::Erase
        } else {
            day::Revision::Strike
        }
    })
}

/// Every day in the log, added up. Named `lifetime` here because `totals` is
/// the module that does the adding.
fn lifetime(args: &[&str]) -> ExitCode {
    let (settings, _) = settings(args);
    let now = time::local(time::now());
    let sums = totals::read(&settings.vault, now.year);
    let all = &sums.all;
    if args.contains(&"--json") {
        let best = |b: &totals::Best| {
            serde_json::json!({
                "value": if b.known() { Some(b.value) } else { None },
                "date": b.date,
            })
        };
        let run = |r: &totals::Run| {
            serde_json::json!({
                "days": r.days,
                "daysSailed": r.days_sailed,
                "distanceNm": r.distance_nm,
                "underwaySecs": r.underway_secs,
                "passages": r.passages,
                "averageKn": r.average_kn(),
            })
        };
        let mut longest = best(&sums.longest);
        longest["secs"] = serde_json::json!(sums.longest_secs);
        println!(
            "{}",
            serde_json::json!({
                "v": 1,
                "boat": settings.boat,
                "vault": settings.vault.to_string_lossy(),
                // The day's note, for a window that wants to know when the
                // totals have moved without rereading the whole vault.
                "todayPath": day::path_for(&settings.vault, &day::today()).to_string_lossy(),
                "all": run(all),
                "year": run(&sums.year),
                "thisYear": sums.this_year,
                "fastest": best(&sums.fastest),
                "bestDay": best(&sums.best_day),
                "longestDay": longest,
                "today": sums.today.as_ref().map(|d| serde_json::json!({
                    "date": d.date,
                    "distanceNm": d.distance_nm,
                    "maxSogKn": d.max_sog_kn,
                    "underwaySecs": d.underway_secs,
                    "passages": d.passages,
                })),
                "first": sums.first,
                "last": sums.last,
                "spanDays": sums.span_days(),
            })
        );
        return ExitCode::SUCCESS;
    }

    if all.days == 0 {
        println!("{} · nothing logged yet", settings.boat);
        return ExitCode::SUCCESS;
    }
    let heading = match (sums.first.is_empty(), sums.span_days()) {
        (false, Some(span)) => format!(
            "{} · {} to {} · {} {} sailed of {span}",
            settings.boat,
            sums.first,
            sums.last,
            all.days_sailed,
            if all.days_sailed == 1 { "day" } else { "days" },
        ),
        // Notes, but nothing sailed yet: the log has been opened, no more.
        _ => format!(
            "{} · {} days logged, none sailed yet",
            settings.boat, all.days
        ),
    };
    println!("{heading}\n");
    let row = |label: &str, value: String| println!("  {label:<13}{value}");
    row("Distance", format!("{:.1} nm", all.distance_nm));
    row("Under way", day::hours(all.underway_secs));
    if let Some(kn) = all.average_kn() {
        row("Average", format!("{kn:.1} kn under way"));
    }
    if sums.fastest.known() {
        row(
            "Fastest",
            format!("{:.1} kn on {}", sums.fastest.value, sums.fastest.date),
        );
    }
    row("Passages", all.passages.to_string());
    if sums.best_day.known() || sums.longest.known() {
        println!();
    }
    if sums.best_day.known() {
        row(
            "Biggest day",
            format!("{:.1} nm on {}", sums.best_day.value, sums.best_day.date),
        );
    }
    if sums.longest.known() {
        row(
            "Longest day",
            format!("{} on {}", day::hours(sums.longest_secs), sums.longest.date),
        );
    }
    // Today, while it is still happening. A day that hasn't gone anywhere
    // yet says nothing: the boat is alongside, which the crew can see.
    if let Some(today) = sums.today.as_ref().filter(|d| d.sailed()) {
        println!();
        row(
            "Today",
            format!(
                "{:.1} nm in {}",
                today.distance_nm,
                day::hours(today.underway_secs)
            ),
        );
    }

    // The season so far, which is only worth a line once there is more log
    // than this year.
    if sums.year.days > 0 && sums.year.days < all.days {
        println!();
        row(
            &format!("{}", sums.this_year),
            format!(
                "{:.1} nm in {}",
                sums.year.distance_nm,
                day::hours(sums.year.underway_secs)
            ),
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
