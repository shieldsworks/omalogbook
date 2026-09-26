//! When the sun sets, for the `/depart` mark: the one time every skipper
//! wants written beside leaving the berth.
//!
//! NOAA's solar calculator (the Meeus-derived equations behind NOAA's
//! spreadsheet), written out here. It is good to about a minute at the
//! latitudes anyone sails, which is all a log needs. Sunset is the moment
//! the sun's upper limb touches a flat horizon, with standard refraction:
//! the sun's center 0.833° below it.

use crate::time;

/// The sun's center at sunset, degrees above the horizon.
const SUNSET_ALTITUDE: f64 = -0.833;

/// The equation of time, minutes, and the sun's declination, degrees, at a
/// moment given as Unix seconds.
fn sun_at(epoch: f64) -> (f64, f64) {
    let jd = epoch / 86_400.0 + 2_440_587.5;
    let t = (jd - 2_451_545.0) / 36_525.0;
    let l0 = (280.466_46 + t * (36_000.769_83 + t * 0.000_303_2)).rem_euclid(360.0);
    let m = 357.529_11 + t * (35_999.050_29 - 0.000_153_7 * t);
    let e = 0.016_708_634 - t * (0.000_042_037 + 0.000_000_126_7 * t);
    let mr = m.to_radians();
    let center = mr.sin() * (1.914_602 - t * (0.004_817 + 0.000_014 * t))
        + (2.0 * mr).sin() * (0.019_993 - 0.000_101 * t)
        + (3.0 * mr).sin() * 0.000_289;
    let omega = (125.04 - 1_934.136 * t).to_radians();
    let lambda = (l0 + center - 0.005_69 - 0.004_78 * omega.sin()).to_radians();
    let eps0 =
        23.0 + (26.0 + (21.448 - t * (46.815 + t * (0.000_59 - t * 0.001_813))) / 60.0) / 60.0;
    let eps = (eps0 + 0.002_56 * omega.cos()).to_radians();
    let decl = (eps.sin() * lambda.sin()).asin();
    let y = (eps / 2.0).tan().powi(2);
    let l0r = l0.to_radians();
    let eot = 4.0
        * (y * (2.0 * l0r).sin() - 2.0 * e * mr.sin() + 4.0 * e * y * mr.sin() * (2.0 * l0r).cos()
            - 0.5 * y * y * (4.0 * l0r).sin()
            - 1.25 * e * e * (2.0 * mr).sin())
        .to_degrees();
    (eot, decl.to_degrees())
}

/// Sunset on a local date (`2026-09-21`) at a place, as Unix seconds. None
/// where the sun doesn't set that day, or doesn't rise: the high latitudes
/// in summer and winter, where a sunset time would be a lie.
pub fn sunset(date: &str, lat: f64, lon: f64) -> Option<i64> {
    if !time::valid_date(date) || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon)
    {
        return None;
    }
    let midnight = time::parse_utc(&format!("{date}T00:00:00Z"))? as f64;
    // Start from the sun's noon at this longitude, then work the sunset out
    // twice more at the moment it was last estimated: the sun moves on in
    // the hours between noon and sunset, and a second pass takes that in.
    let mut minutes = 720.0 - 4.0 * lon;
    for _ in 0..3 {
        let (eot, decl) = sun_at(midnight + minutes * 60.0);
        let (latr, declr) = (lat.to_radians(), decl.to_radians());
        let cos_ha = (SUNSET_ALTITUDE.to_radians().sin() - latr.sin() * declr.sin())
            / (latr.cos() * declr.cos());
        if !(-1.0..=1.0).contains(&cos_ha) {
            return None;
        }
        let ha = cos_ha.acos().to_degrees();
        minutes = 720.0 - 4.0 * lon - eot + 4.0 * ha;
    }
    Some((midnight + minutes * 60.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARINA: (f64, f64) = (37.8663, -122.3148);

    /// Reference sunsets for Berkeley Marina, from the `astral` library
    /// (NOAA's algorithm, independently implemented) in a scratch virtualenv,
    /// never a dependency. Unix seconds, UTC.
    const REFERENCE: &[(&str, i64)] = &[
        ("2026-01-15", 1_768_526_040), // 17:13:59 PST
        ("2026-03-20", 1_774_059_667), // 19:21:06 PDT
        ("2026-06-21", 1_782_099_280), // 20:34:39 PDT
        ("2026-09-21", 1_790_042_835), // 19:07:15 PDT
        ("2026-11-01", 1_793_581_796), // 17:09:55 PST
        ("2026-12-21", 1_797_900_805), // 16:53:25 PST
        ("2030-07-04", 1_909_452_881), // 20:34:41 PDT
    ];

    #[test]
    fn agrees_with_a_reference_to_the_minute() {
        for (date, want) in REFERENCE {
            let got = sunset(date, MARINA.0, MARINA.1).expect("the sun sets");
            assert!((got - want).abs() <= 60, "{date}: {got} vs {want}");
        }
    }

    /// Tromsø in June: the midnight sun has no sunset to give.
    #[test]
    fn the_midnight_sun_never_sets() {
        assert_eq!(sunset("2026-06-21", 69.65, 18.96), None);
        assert_eq!(sunset("2026-12-21", 69.65, 18.96), None);
        assert!(sunset("2026-03-20", 69.65, 18.96).is_some());
    }

    #[test]
    fn nonsense_is_no_sunset() {
        assert_eq!(sunset("2026-02-31", MARINA.0, MARINA.1), None);
        assert_eq!(sunset("2026-09-21", 91.0, 0.0), None);
    }
}
