use omalogbook::{config, day, entry, keel, lock, time, watch::Watch};
use std::{path::PathBuf, process::ExitCode};

const USAGE: &str = "\
omalogbook — the ship's log for Omahoy

usage: omalogbook run [--vault DIR] [--socket PATH] [--no-git]
       omalogbook note TEXT...
       omalogbook path [--date YYYY-MM-DD]
       omalogbook vault
       omalogbook --help | --version

run     follow omakeel and write the log: entries, the day's run, and a GPX
        track for every passage
note    add your own entry to today's note, from anywhere
path    print the path of a day's note, for an editor to open
vault   print the folder the log lives in

Settings live in ~/.config/omalogbook/config.toml: vault, boat, every_minutes,
underway_kn, stopped_kn, stop_after_minutes, point_seconds, git.
";

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
    let write = || -> std::io::Result<PathBuf> {
        let _lock = lock::Lock::take(&settings.vault)?;
        let mut day = day::Day::open(&settings.vault, &date, &settings.boat)?;
        day.push(entry::note(time::local(now), time::utc(now), &text));
        day.save()?;
        Ok(day.path.clone())
    };
    match write() {
        Ok(path) => {
            println!("{}", path.display());
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
