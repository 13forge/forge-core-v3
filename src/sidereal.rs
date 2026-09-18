//! Real sidereal time — Meeus low-precision GMST, one home (L05).
//! Pure: faces sample their own wall clock into a Julian Date.
//! Lifted 2026-08-26 from shell/src/celestial.rs:135-159 (w4 birth-on-sky).

/// Greenwich Mean Sidereal Time in degrees for Julian Date `jd` (UTC).
/// First-order Meeus term only — a fraction of a second short of exact,
/// ample for a projected sky, not a mount drive.
pub fn gmst_degrees(jd: f64) -> f64 {
    let d = jd - 2451545.0;
    (280.46061837 + 360.98564736629 * d).rem_euclid(360.0)
}

/// Local Sidereal Time in degrees for `longitude_deg` (east-positive) at `jd`.
pub fn lst_degrees(jd: f64, longitude_deg: f64) -> f64 {
    (gmst_degrees(jd) + longitude_deg).rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j2000_noon_gmst_is_the_meeus_constant() {
        assert!((gmst_degrees(2451545.0) - 280.46061837).abs() < 1e-9);
    }

    #[test]
    fn lst_is_gmst_shifted_by_longitude_and_wrapped() {
        let g = gmst_degrees(2451545.0);
        let l = lst_degrees(2451545.0, -113.49);
        assert!((l - (g - 113.49).rem_euclid(360.0)).abs() < 1e-9);
        assert!((0.0..360.0).contains(&lst_degrees(2460000.5, -113.49)));
    }

    #[test]
    fn one_sidereal_day_returns_the_sky() {
        let d = gmst_degrees(2451545.0 + 0.9972695663);
        assert!((d - gmst_degrees(2451545.0)).abs() < 0.01, "sidereal period: {d}");
    }
}
