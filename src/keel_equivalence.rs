use super::{Fix, Link, Update, Why, read};
use serde_json::Value;

/// One input, the update the field walk produced, and what the log says about it.
struct Case {
    name: &'static str,
    line: String,
    update: Option<Update>,
    said: Option<String>,
    detail: u32,
}

fn case(
    name: &'static str,
    line: impl Into<String>,
    update: Option<Update>,
    said: Option<&str>,
    detail: u32,
) -> Case {
    Case {
        name,
        line: line.into(),
        update,
        said: said.map(str::to_string),
        detail,
    }
}

fn position() -> Fix {
    Fix {
        lat: 37.8647,
        lon: -122.3207,
        sog_kn: None,
        cog_deg: None,
        utc: None,
        satellites: None,
        hdop: None,
    }
}

fn kept(idle: &[&str]) -> Option<Update> {
    Some(Update::Fix(
        position(),
        idle.iter().map(|name| (*name).to_string()).collect(),
    ))
}

fn no_fix(why: Why) -> Option<Update> {
    Some(Update::NoFix(why))
}

fn link(name: &str, state: &'static str, message: Option<&str>) -> Link {
    Link {
        name: name.to_string(),
        state,
        message: message.map(str::to_string),
    }
}

fn down(message: Option<&str>) -> Why {
    Why::Links(vec![link("tcp:10.0.2.2:10110", "down", message)])
}

fn state(fix: &str, sources: &str) -> String {
    format!(r#"{{"type":"state","v":1,"fix":{fix},"sources":{sources}}}"#)
}

const OK: &str = r#"{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0}"#;
const STALE: &str = r#"{"status":"stale","lat":37.8647,"lon":-122.3207,"ageSeconds":7}"#;
const NONE: &str = r#"{"status":"none"}"#;
const GPS: &str = "tcp:10.0.2.2:10110";
const AIS: &str = "serial:/dev/ais:38400";
const REFUSED: &str = "Connection refused (os error 111)";
const DOWN_SAID: &str = "tcp:10.0.2.2:10110 is down (Connection refused (os error 111))";
const SILENT: &str = "the receiver is talking but sends no position";
const RECEIVER: &str = "the receiver has no fix";

fn down_source(extra: &str) -> String {
    format!(r#"[{{"name":"{GPS}","status":"error","message":"{REFUSED}"{extra}}}]"#)
}

fn bare_down() -> String {
    down_source("")
}

fn full_down() -> String {
    down_source(r#","sentences":0,"rejected":0"#)
}

/// The field walk from before `omakeel-protocol`. The table is its result.
fn read_before_protocol(line: &str) -> Option<Update> {
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
            .filter(|value| value.is_finite())
    };
    let satellites = number("satellites")
        .filter(|n| (0.0..=255.0).contains(n))
        .map(|n| n as u32);
    let hdop = number("hdop").filter(|h| (0.0..=99.99).contains(h));
    let (lat, lon) = (number("lat"), number("lon"));
    if status == "ok"
        && let (Some(lat), Some(lon)) = (lat, lon)
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lon)
    {
        return Some(Update::Fix(
            Fix {
                lat,
                lon,
                sog_kn: number("sogKn").filter(|knots| (0.0..=100.0).contains(knots)),
                cog_deg: number("cogDeg"),
                utc: fix
                    .get("utc")
                    .and_then(Value::as_str)
                    .and_then(crate::time::parse_utc),
                satellites,
                hdop,
            },
            unwell(m.get("sources"))
                .into_iter()
                .map(|item| item.name)
                .collect(),
        ));
    }
    Some(Update::NoFix(why(status, m.get("sources"))))
}

fn why(status: &str, sources: Option<&Value>) -> Why {
    if status == "nofix" {
        return Why::Receiver;
    }
    let links = unwell(sources);
    if links.is_empty() {
        Why::Silent
    } else {
        Why::Links(links)
    }
}

fn unwell(sources: Option<&Value>) -> Vec<Link> {
    sources
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(32)
        .filter_map(|source| {
            let state = match source.get("status").and_then(Value::as_str)? {
                "error" => "down",
                "connecting" => "connecting",
                "quiet" => "quiet",
                "ended" => "ended",
                _ => return None,
            };
            Some(Link {
                name: text(source.get("name"))?,
                state,
                message: text(source.get("message")),
            })
        })
        .collect()
}

fn text(value: Option<&Value>) -> Option<String> {
    let raw: String = value?
        .as_str()?
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(120)
        .collect();
    let raw = raw.trim();
    (!raw.is_empty()).then(|| raw.to_string())
}

fn check(
    misses: &mut Vec<String>,
    name: &str,
    line: &str,
    update: &Option<Update>,
    said: Option<&str>,
    detail: u32,
) {
    let before = read_before_protocol(line);
    let after = read(line);
    if before != *update {
        misses.push(format!("{name}: pasted walk {before:?}, table {update:?}"));
    }
    if after != *update {
        misses.push(format!("{name}: read {after:?}, table {update:?}"));
    }
    for (who, got) in [("pasted walk", &before), ("read", &after)] {
        let (got_said, got_detail) = match got {
            Some(Update::NoFix(why)) => (Some(why.say()), why.detail()),
            _ => (None, 0),
        };
        if got_said.as_deref() != said {
            misses.push(format!(
                "{name}: {who} says {got_said:?}, table says {said:?}"
            ));
        }
        if got_detail != detail {
            misses.push(format!(
                "{name}: {who} detail {got_detail}, table detail {detail}"
            ));
        }
    }
}

#[test]
fn read_matches_the_walk_it_replaced() {
    let mut cases = vec![
        case(
            "a complete ok fix is the boat",
            state(OK, "[]"),
            kept(&[]),
            None,
            0,
        ),
        case(
            "latitude 90 and longitude 180 are on the chart",
            state(
                r#"{"status":"ok","lat":90.0,"lon":180.0,"ageSeconds":0}"#,
                "[]",
            ),
            Some(Update::Fix(
                Fix {
                    lat: 90.0,
                    lon: 180.0,
                    ..position()
                },
                vec![],
            )),
            None,
            0,
        ),
        case(
            "a latitude past 90 is not the boat's position",
            state(
                r#"{"status":"ok","lat":91.0,"lon":-122.3207,"ageSeconds":0}"#,
                "[]",
            ),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "a longitude past 180 is not the boat's position",
            state(
                r#"{"status":"ok","lat":37.8647,"lon":181.0,"ageSeconds":0}"#,
                "[]",
            ),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "an out-of-range fix still names a source that is down",
            state(
                r#"{"status":"ok","lat":91.0,"lon":-122.3207,"ageSeconds":0}"#,
                &full_down(),
            ),
            no_fix(down(Some(REFUSED))),
            Some(DOWN_SAID),
            2,
        ),
        case(
            "a stale fix with coordinates is not the boat's position",
            state(STALE, "[]"),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "a stale fix names the source that is down",
            state(STALE, &full_down()),
            no_fix(down(Some(REFUSED))),
            Some(DOWN_SAID),
            2,
        ),
        case(
            "a stale fix whose source omits sentence counts still names the link",
            state(STALE, &bare_down()),
            no_fix(down(Some(REFUSED))),
            Some(DOWN_SAID),
            2,
        ),
        case(
            "a stale fix with no coordinates still names a bare down source",
            state(r#"{"status":"stale"}"#, &bare_down()),
            no_fix(down(Some(REFUSED))),
            Some(DOWN_SAID),
            2,
        ),
        case(
            "a connecting source reads as connecting",
            state(
                STALE,
                r#"[{"name":"tcp:10.0.2.2:10110","status":"connecting"}]"#,
            ),
            no_fix(Why::Links(vec![link(GPS, "connecting", None)])),
            Some("tcp:10.0.2.2:10110 is connecting"),
            0,
        ),
        case(
            "a quiet source is not a down source",
            state(
                STALE,
                r#"[{"name":"tcp:10.0.2.2:10110","status":"quiet"},{"name":"serial:/dev/ttyACM0:38400","status":"ok"}]"#,
            ),
            no_fix(Why::Links(vec![link(GPS, "quiet", None)])),
            Some("tcp:10.0.2.2:10110 is connected but sending nothing"),
            0,
        ),
        case(
            "every source up, and a stale fix, is a silent receiver",
            state(STALE, r#"[{"name":"tcp:10.0.2.2:10110","status":"ok"}]"#),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "a replay that ended is the reason",
            state(STALE, r#"[{"name":"replay:sail.nmea","status":"ended"}]"#),
            no_fix(Why::Links(vec![link("replay:sail.nmea", "ended", None)])),
            Some("replay:sail.nmea has ended"),
            0,
        ),
        case(
            "nofix is the receiver, even beside a quiet source",
            state(
                r#"{"status":"nofix","satellites":3}"#,
                r#"[{"name":"tcp:10.0.2.2:10110","status":"ok"},{"name":"serial:/dev/ttyACM0:38400","status":"quiet"}]"#,
            ),
            no_fix(Why::Receiver),
            Some(RECEIVER),
            0,
        ),
        case(
            "nofix with coordinates and no age is still the receiver",
            state(r#"{"status":"nofix","lat":37.8647,"lon":-122.3207}"#, "[]"),
            no_fix(Why::Receiver),
            Some(RECEIVER),
            0,
        ),
        case(
            "nofix with coordinates, no age, and a quiet source is still the receiver",
            state(
                r#"{"status":"nofix","lat":37.8647,"lon":-122.3207}"#,
                r#"[{"name":"serial:/dev/ais:38400","status":"quiet"}]"#,
            ),
            no_fix(Why::Receiver),
            Some(RECEIVER),
            0,
        ),
        case(
            "nofix with coordinates and an age is still the receiver",
            state(
                r#"{"status":"nofix","lat":37.8647,"lon":-122.3207,"ageSeconds":4}"#,
                "[]",
            ),
            no_fix(Why::Receiver),
            Some(RECEIVER),
            0,
        ),
        case(
            "nofix with one coordinate is the receiver",
            state(r#"{"status":"nofix","lat":37.8647}"#, &full_down()),
            no_fix(Why::Receiver),
            Some(RECEIVER),
            0,
        ),
        case(
            "status none is not a position",
            state(NONE, "[]"),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "status none with coordinates is not a position",
            state(
                r#"{"status":"none","lat":37.8647,"lon":-122.3207,"ageSeconds":0}"#,
                "[]",
            ),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "status none with coordinates still names a source that is down",
            state(
                r#"{"status":"none","lat":37.8647,"lon":-122.3207,"ageSeconds":0}"#,
                &full_down(),
            ),
            no_fix(down(Some(REFUSED))),
            Some(DOWN_SAID),
            2,
        ),
        case(
            "a missing status is none, so coordinates are not a fix",
            state(r#"{"lat":37.8647,"lon":-122.3207,"ageSeconds":0}"#, "[]"),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "status NONE is not ok, so the coordinates stay off the chart",
            state(
                r#"{"status":"NONE","lat":37.8647,"lon":-122.3207,"ageSeconds":0}"#,
                "[]",
            ),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "an unknown fix status with a down source names the link",
            state(
                r#"{"status":"survey","lat":37.8647,"lon":-122.3207,"ageSeconds":0}"#,
                &full_down(),
            ),
            no_fix(down(Some(REFUSED))),
            Some(DOWN_SAID),
            2,
        ),
        case(
            "an unknown fix status with no bad source is silent",
            state(
                r#"{"status":"survey","lat":37.8647,"lon":-122.3207,"ageSeconds":0}"#,
                "[]",
            ),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "an unknown source status is ignored and the fix stays",
            state(
                OK,
                r#"[{"name":"gps","status":"asleep","sentences":1,"rejected":0}]"#,
            ),
            kept(&[]),
            None,
            0,
        ),
        case(
            "an unknown source status does not hide a source that is down",
            state(
                STALE,
                &format!(
                    r#"[{{"name":"gps","status":"asleep","sentences":1,"rejected":0}},{{"name":"{GPS}","status":"error","message":"{REFUSED}","sentences":0,"rejected":0}}]"#
                ),
            ),
            no_fix(down(Some(REFUSED))),
            Some(DOWN_SAID),
            2,
        ),
        case(
            "nofix ignores an unknown source status",
            state(
                r#"{"status":"nofix"}"#,
                r#"[{"name":"gps","status":"asleep","sentences":0,"rejected":0}]"#,
            ),
            no_fix(Why::Receiver),
            Some(RECEIVER),
            0,
        ),
        case(
            "a missing sources array still carries the fix",
            r#"{"type":"state","v":1,"fix":{"status":"ok","lat":37.864711,"lon":-122.3207314,"sogKn":5.0,"cogDeg":255.0,"utc":"2026-09-13T21:00:10Z","satellites":9,"hdop":0.9,"ageSeconds":0}}"#,
            Some(Update::Fix(
                Fix {
                    lat: 37.864_711,
                    lon: -122.320_731_4,
                    sog_kn: Some(5.0),
                    cog_deg: Some(255.0),
                    utc: Some(1_789_333_210),
                    satellites: Some(9),
                    hdop: Some(0.9),
                },
                vec![],
            )),
            None,
            0,
        ),
        case(
            "a missing sources array on status none is silent",
            format!(r#"{{"type":"state","v":1,"fix":{NONE}}}"#),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "sources that are not an array are ignored",
            state(OK, "{}"),
            kept(&[]),
            None,
            0,
        ),
        case(
            "a source missing its name is ignored",
            state(
                OK,
                r#"[{"status":"error","message":"nope","sentences":0,"rejected":0}]"#,
            ),
            kept(&[]),
            None,
            0,
        ),
        case(
            "a source missing its sentence count is still a link",
            state(STALE, r#"[{"name":"tcp:10.0.2.2:10110","status":"error"}]"#),
            no_fix(down(None)),
            Some("tcp:10.0.2.2:10110 is down"),
            1,
        ),
        case(
            "one coordinate and a down source names the link",
            state(
                r#"{"status":"ok","lat":37.8647,"ageSeconds":0}"#,
                &full_down(),
            ),
            no_fix(down(Some(REFUSED))),
            Some(DOWN_SAID),
            2,
        ),
        case(
            "longitude without latitude and no bad source is silent",
            state(r#"{"status":"ok","lon":-122.3207,"ageSeconds":0}"#, "[]"),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "coordinates stored as strings are not a fix",
            state(
                r#"{"status":"ok","lat":"37.8647","lon":"-122.3207","ageSeconds":0}"#,
                "[]",
            ),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "an ok fix with no age is still a fix",
            state(r#"{"status":"ok","lat":37.8647,"lon":-122.3207}"#, "[]"),
            kept(&[]),
            None,
            0,
        ),
        case(
            "a fractional age is ignored",
            state(
                r#"{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":1.5}"#,
                "[]",
            ),
            kept(&[]),
            None,
            0,
        ),
        case(
            "an age of 0.0 is ignored",
            state(
                r#"{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0.0}"#,
                "[]",
            ),
            kept(&[]),
            None,
            0,
        ),
        case(
            "a null age is ignored",
            r#"{"type":"state","v":1,"fix":{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":null},"sources":[]}"#,
            kept(&[]),
            None,
            0,
        ),
        case(
            "a null extra field is ignored",
            r#"{"type":"state","v":1,"fix":{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0},"sources":[],"note":null}"#,
            kept(&[]),
            None,
            0,
        ),
        case(
            "a null latitude is not a fix",
            state(
                r#"{"status":"ok","lat":null,"lon":-122.3207,"ageSeconds":0}"#,
                "[]",
            ),
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "a satellite count past 255 is dropped and the fix stays",
            state(
                r#"{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0,"satellites":300}"#,
                "[]",
            ),
            kept(&[]),
            None,
            0,
        ),
        case(
            "a fractional satellite count keeps the whole part",
            state(
                r#"{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0,"satellites":9.7}"#,
                "[]",
            ),
            Some(Update::Fix(
                Fix {
                    satellites: Some(9),
                    ..position()
                },
                vec![],
            )),
            None,
            0,
        ),
        case(
            "a speed past 100 knots is dropped and the fix stays",
            state(
                r#"{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0,"sogKn":101.0}"#,
                "[]",
            ),
            kept(&[]),
            None,
            0,
        ),
        case(
            "a course past 360 is kept",
            state(
                r#"{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0,"cogDeg":720.0}"#,
                "[]",
            ),
            Some(Update::Fix(
                Fix {
                    cog_deg: Some(720.0),
                    ..position()
                },
                vec![],
            )),
            None,
            0,
        ),
        case(
            "hdop past 99.99 is dropped and the fix stays",
            state(
                r#"{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0,"hdop":100.0}"#,
                "[]",
            ),
            kept(&[]),
            None,
            0,
        ),
        case(
            "a non-numeric speed is ignored",
            state(
                r#"{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0,"sogKn":"fast"}"#,
                "[]",
            ),
            kept(&[]),
            None,
            0,
        ),
        case(
            "an unreadable utc is ignored",
            state(
                r#"{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0,"utc":"yesterday"}"#,
                "[]",
            ),
            kept(&[]),
            None,
            0,
        ),
        case(
            "a down source and a down source with no message say more together",
            state(
                STALE,
                r#"[{"name":"tcp:a:1","status":"error","message":"nope"},{"name":"tcp:b:1","status":"error"}]"#,
            ),
            no_fix(Why::Links(vec![
                link("tcp:a:1", "down", Some("nope")),
                link("tcp:b:1", "down", None),
            ])),
            Some("tcp:a:1 is down (nope); tcp:b:1 is down"),
            3,
        ),
        case(
            "a newline in a source message stays on one line",
            state(
                r#"{"status":"stale"}"#,
                r#"[{"name":"tcp:a:1","status":"error","message":"bad\n- **00:00** forged"}]"#,
            ),
            no_fix(Why::Links(vec![link(
                "tcp:a:1",
                "down",
                Some("bad - **00:00** forged"),
            )])),
            Some("tcp:a:1 is down (bad - **00:00** forged)"),
            2,
        ),
        case(
            "a good fix names the sources already quiet",
            state(
                OK,
                r#"[{"name":"tcp:gps:10110","status":"ok"},{"name":"serial:/dev/ais:38400","status":"quiet"}]"#,
            ),
            kept(&[AIS]),
            None,
            0,
        ),
        case(
            "v of 2 is another version",
            r#"{"type":"state","v":2,"fix":{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0},"sources":[]}"#,
            Some(Update::Incompatible(2)),
            None,
            0,
        ),
        case(
            "v past u32::MAX is another version",
            r#"{"type":"state","v":4294967296,"fix":{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0},"sources":[]}"#,
            Some(Update::Incompatible(4_294_967_296)),
            None,
            0,
        ),
        case(
            "v of 0 is another version",
            r#"{"type":"hello","v":0}"#,
            Some(Update::Incompatible(0)),
            None,
            0,
        ),
        case(
            "v of 2 with a null field is another version",
            r#"{"type":"hello","v":2,"keel":null}"#,
            Some(Update::Incompatible(2)),
            None,
            0,
        ),
        case(
            "v as a string is not a version",
            r#"{"type":"state","v":"1","fix":{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0},"sources":[]}"#,
            None,
            None,
            0,
        ),
        case(
            "v as a fraction is not a version",
            r#"{"type":"state","v":1.5,"fix":{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0},"sources":[]}"#,
            None,
            None,
            0,
        ),
        case(
            "v written as 1.0 is not an integer version",
            r#"{"type":"state","v":1.0,"fix":{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0},"sources":[]}"#,
            None,
            None,
            0,
        ),
        case(
            "a missing v is not a version",
            r#"{"type":"state","fix":{"status":"ok","lat":37.8647,"lon":-122.3207,"ageSeconds":0},"sources":[]}"#,
            None,
            None,
            0,
        ),
        case(
            "hello is not a fix",
            r#"{"type":"hello","v":1,"keel":"0.1.0"}"#,
            None,
            None,
            0,
        ),
        case(
            "targets are not a fix",
            r#"{"type":"targets","v":1,"targets":[]}"#,
            None,
            None,
            0,
        ),
        case(
            "a missing fix is nothing",
            r#"{"type":"state","v":1,"sources":[]}"#,
            None,
            None,
            0,
        ),
        case(
            "a fix that is not an object is silent",
            r#"{"type":"state","v":1,"fix":"ok","sources":[]}"#,
            no_fix(Why::Silent),
            Some(SILENT),
            0,
        ),
        case(
            "a null fix with a down source names the link",
            state("null", &bare_down()),
            no_fix(down(Some(REFUSED))),
            Some(DOWN_SAID),
            2,
        ),
    ];

    let mut sources = Vec::new();
    for i in 0..33 {
        sources.push(format!(
            r#"{{"name":"s{i}","status":"error","message":"m","sentences":0,"rejected":0}}"#
        ));
    }
    let links: Vec<Link> = (0..32)
        .map(|i| link(&format!("s{i}"), "down", Some("m")))
        .collect();
    let said = links
        .iter()
        .map(|item| format!("{} is down (m)", item.name))
        .collect::<Vec<_>>()
        .join("; ");
    cases.push(Case {
        name: "only the first 32 sources are the reason",
        line: state(STALE, &format!("[{}]", sources.join(","))),
        update: no_fix(Why::Links(links)),
        said: Some(said),
        detail: 64,
    });
    let mut capped = Vec::new();
    for i in 0..32 {
        capped.push(format!(
            r#"{{"name":"ok{i}","status":"ok","sentences":1,"rejected":0}}"#
        ));
    }
    capped.push(
        r#"{"name":"serial:/dev/ais:38400","status":"quiet","sentences":0,"rejected":0}"#
            .to_string(),
    );
    cases.push(case(
        "a source past the first 32 is ignored",
        state(OK, &format!("[{}]", capped.join(","))),
        kept(&[]),
        None,
        0,
    ));

    let mut misses = Vec::new();
    for case in &cases {
        check(
            &mut misses,
            case.name,
            &case.line,
            &case.update,
            case.said.as_deref(),
            case.detail,
        );
    }
    check_diagnostics(&mut misses);
    assert!(misses.is_empty(), "{}", misses.join("\n"));
}

fn why_from(line: &str) -> Why {
    match read_before_protocol(line) {
        Some(Update::NoFix(why)) => why,
        other => panic!("expected a reason, got {other:?} for {line}"),
    }
}

fn check_diagnostics(misses: &mut Vec<String>) {
    let connecting = state(
        STALE,
        r#"[{"name":"tcp:10.0.2.2:10110","status":"connecting"}]"#,
    );
    let down_message = state(STALE, &bare_down());
    let down_plain = state(STALE, r#"[{"name":"tcp:10.0.2.2:10110","status":"error"}]"#);
    let quiet = state(STALE, r#"[{"name":"tcp:10.0.2.2:10110","status":"quiet"}]"#);
    for line in [&connecting, &down_message, &down_plain, &quiet] {
        let before = read_before_protocol(line);
        let after = read(line);
        if before != after {
            misses.push(format!(
                "diagnostic line disagrees: pasted {before:?}, read {after:?}, line {line}"
            ));
        }
    }
    let connecting = why_from(&connecting);
    let down_message = why_from(&down_message);
    let down_plain = why_from(&down_plain);
    let quiet = why_from(&quiet);
    if !connecting.same_as(&down_message) || !connecting.same_as(&down_plain) {
        misses.push(format!(
            "connecting and down are one place to look: {connecting:?} vs {down_message:?} vs {down_plain:?}"
        ));
    }
    if down_message.same_as(&quiet) || down_plain.same_as(&Why::Hub) {
        misses.push("a quiet source or the hub is a different place".to_string());
    }
    if connecting.detail() != 0 || down_plain.detail() != 1 || down_message.detail() != 2 {
        misses.push(format!(
            "detail connecting {} plain {} message {}",
            connecting.detail(),
            down_plain.detail(),
            down_message.detail()
        ));
    }
    if down_message.detail() <= connecting.detail() {
        misses.push("a down source with an error says more than connecting".to_string());
    }

    let idle_line = state(
        OK,
        r#"[{"name":"tcp:gps:10110","status":"ok"},{"name":"serial:/dev/ais:38400","status":"quiet"}]"#,
    );
    let both_line = state(
        STALE,
        r#"[{"name":"tcp:gps:10110","status":"quiet"},{"name":"serial:/dev/ais:38400","status":"quiet"}]"#,
    );
    let before_idle = read_before_protocol(&idle_line);
    let after_idle = read(&idle_line);
    if before_idle != after_idle {
        misses.push(format!(
            "idle sources disagree: pasted {before_idle:?}, read {after_idle:?}"
        ));
    }
    let Some(Update::Fix(_, idle)) = before_idle else {
        misses.push(format!("a quiet source on a good fix was {before_idle:?}"));
        return;
    };
    if idle != [AIS] {
        misses.push(format!("idle sources {idle:?}"));
    }
    let both = why_from(&both_line);
    let left = both.clone().without(&idle);
    let expected = Why::Links(vec![link("tcp:gps:10110", "quiet", None)]);
    if left != expected {
        misses.push(format!(
            "without the idle AIS source: {left:?}, table {expected:?}"
        ));
    }
    if left.say() != "tcp:gps:10110 is connected but sending nothing" {
        misses.push(format!("without says {}", left.say()));
    }
    let only_ais = Why::Links(vec![link(AIS, "quiet", None)]);
    if only_ais.clone().without(&idle) != Why::Silent {
        misses.push(format!(
            "an already-idle source left {}",
            only_ais.without(&idle).say()
        ));
    }
    if !matches!(Why::Receiver.without(&idle), Why::Receiver) {
        misses.push("without changed the receiver reason".to_string());
    }
    if read(&idle_line) != Some(Update::Fix(position(), idle)) {
        misses.push("read dropped the idle source on a good fix".to_string());
    }
}
