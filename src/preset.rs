//! The moments worth a mark: `/depart`, `/anchor`, `/reef` and the rest.
//!
//! A preset is the crew saying what happened, which is worth more than the
//! log guessing from speed. `under way` and `stopped` come from a speed
//! threshold and can't tell anchored from moored from tied up alongside; a
//! mark can, because someone said so.
//!
//! Presets are parsed here rather than in the window, so `/anchor 25 ft`
//! means the same thing typed in a terminal as typed on watch.

/// One of the marks, and what it says in the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    /// The word the crew types, without the slash.
    pub word: &'static str,
    /// How the entry reads.
    pub label: &'static str,
    /// What it is for, for `omalogbook presets` and the window's list.
    pub about: &'static str,
}

/// Every mark, in the order a day tends to use them.
pub const PRESETS: &[Preset] = &[
    Preset {
        word: "depart",
        label: "Departed",
        about: "left the berth",
    },
    Preset {
        word: "sail",
        label: "Sailing",
        about: "engine off, under sail",
    },
    Preset {
        word: "motor",
        label: "Motoring",
        about: "engine on",
    },
    Preset {
        word: "reef",
        label: "Reefed",
        about: "a reef in",
    },
    Preset {
        word: "shake",
        label: "Shook out",
        about: "a reef out",
    },
    Preset {
        word: "anchor",
        label: "Anchor down",
        about: "anchored; add the depth and scope",
    },
    Preset {
        word: "aweigh",
        label: "Anchor up",
        about: "anchor off the bottom",
    },
    Preset {
        word: "moor",
        label: "Moored",
        about: "on a mooring",
    },
    Preset {
        word: "berth",
        label: "Berthed",
        about: "tied up alongside",
    },
    Preset {
        word: "watch",
        label: "Watch change",
        about: "the watch handed over",
    },
];

/// Words that mean a preset without being its own: what the crew is likely
/// to reach for. `/mooring` and `/moor` are the same mark.
const ALIASES: &[(&str, &str)] = &[
    ("mooring", "moor"),
    ("returned", "berth"),
    ("return", "berth"),
    ("dock", "berth"),
    ("docked", "berth"),
    ("departed", "depart"),
    ("anchored", "anchor"),
    ("weigh", "aweigh"),
    ("sailing", "sail"),
    ("motoring", "motor"),
    ("reefed", "reef"),
];

pub fn find(word: &str) -> Option<&'static Preset> {
    let word = word.to_ascii_lowercase();
    let word = ALIASES
        .iter()
        .find(|(from, _)| *from == word)
        .map_or(word.as_str(), |(_, to)| to);
    PRESETS.iter().find(|p| p.word == word)
}

/// What the crew typed, split into a mark and whatever they added.
#[derive(Clone, Debug, PartialEq)]
pub enum Typed<'a> {
    /// `/anchor 25 ft, 5:1` — the mark, and the rest of the line.
    Mark(&'static Preset, &'a str),
    /// `/whatever` — a slash word that isn't a mark. The words are kept:
    /// losing what someone wrote on watch is worse than filing it plainly.
    Unknown(&'a str),
    /// Ordinary prose, which is most of a log.
    Note,
}

pub fn parse(text: &str) -> Typed<'_> {
    let text = text.trim();
    let Some(rest) = text.strip_prefix('/') else {
        return Typed::Note;
    };
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    let (word, tail) = rest.split_at(end);
    // A word of letters, so a date written `/12/09` or a path pasted in is
    // prose and not a mark that went wrong.
    if word.is_empty() || !word.chars().all(|c| c.is_ascii_alphabetic()) {
        return Typed::Note;
    }
    match find(word) {
        Some(p) => Typed::Mark(p, tail.trim()),
        None => Typed::Unknown(word),
    }
}

/// `depart, sail, motor, …`, for a message that has to list them.
pub fn words() -> String {
    PRESETS
        .iter()
        .map(|p| p.word)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_is_its_word_and_the_rest_of_the_line() {
        let Typed::Mark(p, tail) = parse("/anchor 25 ft, 5:1 scope") else {
            panic!("not a mark");
        };
        assert_eq!(p.label, "Anchor down");
        assert_eq!(tail, "25 ft, 5:1 scope");
    }

    #[test]
    fn a_mark_on_its_own_has_no_tail() {
        assert_eq!(parse("/depart"), Typed::Mark(&PRESETS[0], ""));
        assert_eq!(parse("  /depart  "), Typed::Mark(&PRESETS[0], ""));
    }

    #[test]
    fn the_words_the_crew_reaches_for_are_the_same_mark() {
        for (word, label) in [
            ("/mooring", "Moored"),
            ("/returned", "Berthed"),
            ("/DEPARTED", "Departed"),
            ("/Anchor", "Anchor down"),
        ] {
            let Typed::Mark(p, _) = parse(word) else {
                panic!("{word} is not a mark");
            };
            assert_eq!(p.label, label, "{word}");
        }
    }

    #[test]
    fn prose_is_prose() {
        for line in [
            "Dolphins off the port side",
            "",
            "the 1/4 berth is wet again",
            "/12/09 was the last haul-out",
            "/",
            "/ anchor",
        ] {
            assert_eq!(parse(line), Typed::Note, "{line}");
        }
    }

    #[test]
    fn a_slash_word_that_is_not_a_mark_is_named_not_lost() {
        assert_eq!(parse("/anchr now"), Typed::Unknown("anchr"));
    }

    #[test]
    fn every_preset_and_alias_resolves() {
        for p in PRESETS {
            assert_eq!(find(p.word).map(|f| f.label), Some(p.label), "{}", p.word);
        }
        for (from, to) in ALIASES {
            assert_eq!(find(from), find(to), "{from} -> {to}");
            assert!(find(to).is_some(), "{to} is not a preset");
        }
    }
}
