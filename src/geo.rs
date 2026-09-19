//! Distance run, and positions written the way a log keeps them.

/// Nautical miles between two positions, on a sphere. The error against an
/// ellipsoid is about 0.3%, far below what a day's run is quoted to.
pub fn distance_nm(from: (f64, f64), to: (f64, f64)) -> f64 {
    const EARTH_NM: f64 = 3440.065;
    let (lat1, lon1) = (from.0.to_radians(), from.1.to_radians());
    let (lat2, lon2) = (to.0.to_radians(), to.1.to_radians());
    let (dlat, dlon) = (lat2 - lat1, lon2 - lon1);
    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_NM * a.sqrt().clamp(0.0, 1.0).asin()
}

/// `37°52.0′N`, degrees and decimal minutes, as a chart and a log use.
pub fn latitude(lat: f64) -> String {
    minutes(lat.abs(), if lat < 0.0 { 'S' } else { 'N' }, 2)
}

/// `122°18.9′W`, with the three-digit degrees longitude always carries.
pub fn longitude(lon: f64) -> String {
    minutes(lon.abs(), if lon < 0.0 { 'W' } else { 'E' }, 3)
}

fn minutes(value: f64, hemisphere: char, width: usize) -> String {
    let mut degrees = value.trunc();
    let mut minutes = (value - degrees) * 60.0;
    // 59.96′ rounds to 60.0′, which is the next whole degree.
    if (minutes * 10.0).round() / 10.0 >= 60.0 {
        degrees += 1.0;
        minutes = 0.0;
    }
    format!("{degrees:0width$.0}°{minutes:04.1}′{hemisphere}")
}

/// `245°`, or blank when the fix carries no course.
pub fn course(cog: Option<f64>) -> String {
    match cog {
        // Round before wrapping, so 359.6° is 000° and never 360°.
        Some(c) if c.is_finite() => format!("{:03}°", (c.round() as i64).rem_euclid(360)),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_degree_of_latitude_is_sixty_miles() {
        let d = distance_nm((37.0, -122.0), (38.0, -122.0));
        assert!((d - 60.0).abs() < 0.2, "{d}");
    }

    #[test]
    fn berkeley_to_the_gate_is_about_seven_miles() {
        let d = distance_nm((37.8663, -122.3148), (37.8199, -122.4783));
        assert!((8.0..9.0).contains(&d), "{d}");
    }

    #[test]
    fn the_same_spot_is_no_distance() {
        assert_eq!(distance_nm((37.8, -122.3), (37.8, -122.3)), 0.0);
    }

    #[test]
    fn positions_read_as_a_log_keeps_them() {
        assert_eq!(latitude(37.866_3), "37°52.0′N");
        assert_eq!(latitude(-37.866_3), "37°52.0′S");
        assert_eq!(longitude(-122.314_8), "122°18.9′W");
        assert_eq!(longitude(5.5), "005°30.0′E");
    }

    #[test]
    fn minutes_never_reach_sixty() {
        assert_eq!(latitude(37.999_95), "38°00.0′N");
    }

    #[test]
    fn course_is_three_digits_or_nothing() {
        assert_eq!(course(Some(5.0)), "005°");
        assert_eq!(course(Some(359.6)), "000°");
        assert_eq!(course(None), "");
        assert_eq!(course(Some(f64::NAN)), "");
    }
}
