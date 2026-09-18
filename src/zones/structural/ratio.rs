//! Deterministic constructive-geometry ratio resolver.
//!
//! All math uses MilliUnit (i64) and Permyriad (i32).
//! No floating point in generation paths.

use crate::fixed_point::MilliUnit;

/// Permyriad scale: 10000 = 1.0.
/// Ported from `F:\NewRepo\crates\forge-tile-crawler\src\permyriad.rs`.
pub const SCALE: i64 = 10000;

/// sqrt(2) ≈ 1.4142
/// Ported from `F:\NewRepo\crates\forge-tile-crawler\src\permyriad.rs`.
pub const SQRT2_PERM: i64 = 14142;

/// sqrt(2)/2 ≈ 0.7071
/// Ported from `F:\NewRepo\crates\forge-tile-crawler\src\permyriad.rs`.
pub const SQRT2_OVER_2_PERM: i64 = 7071;

/// sqrt(3) ≈ 1.7320
/// Ported from `F:\NewRepo\crates\forge-tile-crawler\src\permyriad.rs`.
pub const SQRT3_PERM: i64 = 17320;

/// sqrt(3)/2 (equilateral altitude) ≈ 0.8660
/// Ported from `F:\NewRepo\crates\forge-tile-crawler\src\permyriad.rs`.
pub const SQRT3_OVER_2_PERM: i64 = 8660;

/// Named constructive ratios used by architectural generators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstructiveRatio {
    /// 1:1
    One,
    /// 1:2
    Half,
    /// sqrt(2) ≈ 1.4142 → Permyriad 14142 (scaled by 10000)
    Sqrt2,
    /// sqrt(2)/2 ≈ 0.7071 → Permyriad 7071
    Sqrt2Over2,
    /// sqrt(3) ≈ 1.7320 → Permyriad 17320 (scaled by 10000)
    Sqrt3,
    /// sqrt(3)/2 (equilateral altitude) ≈ 0.8660 → Permyriad 8660
    EquilateralAltitude,
    /// Ad quadratum diagonal: side * sqrt(2)
    AdQuadratumDiagonal,
    /// Ad triangulum altitude: span * sqrt(3) / 2
    AdTriangulumAltitude,
}

/// Resolve a constructive ratio applied to a base measurement.
///
/// Returns the scaled result in MilliUnits. Deterministic integer arithmetic only.
///
/// Example: `resolve(Sqrt2, MilliUnit(1000))` → MilliUnit(1414)
pub fn resolve(ratio: ConstructiveRatio, base: MilliUnit) -> MilliUnit {
    let v = base.0;
    MilliUnit(match ratio {
        ConstructiveRatio::One => v,
        ConstructiveRatio::Half => v / 2,
        ConstructiveRatio::Sqrt2 => v * SQRT2_PERM / SCALE,
        ConstructiveRatio::Sqrt2Over2 => v * SQRT2_OVER_2_PERM / SCALE,
        ConstructiveRatio::Sqrt3 => v * SQRT3_PERM / SCALE,
        ConstructiveRatio::EquilateralAltitude => v * SQRT3_OVER_2_PERM / SCALE,
        ConstructiveRatio::AdQuadratumDiagonal => v * SQRT2_PERM / SCALE,
        ConstructiveRatio::AdTriangulumAltitude => v * SQRT3_OVER_2_PERM / SCALE,
    })
}

/// Compute octagonal socket offset from center.
///
/// Given a radius in MilliUnits and an octant index (0..7),
/// returns (x, y) offset in MilliUnits using integer trig lookup.
pub fn octagon_offset(radius: MilliUnit, index: u8) -> (MilliUnit, MilliUnit) {
    debug_assert!(index < 8);
    const H: i64 = SQRT2_OVER_2_PERM;
    const S: i64 = SCALE;
    const COS_TABLE: [i64; 8] = [S, H, 0, -H, -S, -H, 0, H];
    const SIN_TABLE: [i64; 8] = [0, H, S, H, 0, -H, -S, -H];

    let r = radius.0;
    let i = index as usize;
    (
        MilliUnit(r * COS_TABLE[i] / SCALE),
        MilliUnit(r * SIN_TABLE[i] / SCALE),
    )
}

/// Compute pointed arch apex height from span width.
///
/// Uses equilateral altitude: height = span * sqrt(3) / 2
pub fn pointed_arch_apex(span_width: MilliUnit, spring_height: MilliUnit) -> MilliUnit {
    let altitude = resolve(ConstructiveRatio::EquilateralAltitude, span_width);
    MilliUnit(spring_height.0 + altitude.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt2_of_1000() {
        assert_eq!(resolve(ConstructiveRatio::Sqrt2, MilliUnit(1000)).0, 1414);
    }

    #[test]
    fn sqrt3_of_1000() {
        assert_eq!(resolve(ConstructiveRatio::Sqrt3, MilliUnit(1000)).0, 1732);
    }

    #[test]
    fn equilateral_altitude_of_1000() {
        assert_eq!(resolve(ConstructiveRatio::EquilateralAltitude, MilliUnit(1000)).0, 866);
    }

    #[test]
    fn octagon_symmetry() {
        let r = MilliUnit(10000);
        let (x0, y0) = octagon_offset(r, 0);
        let (x4, y4) = octagon_offset(r, 4);
        assert_eq!(x0.0, -x4.0);
        assert_eq!(y0.0, -y4.0);
    }

    #[test]
    fn octagon_45_degree() {
        let r = MilliUnit(10000);
        let (x1, y1) = octagon_offset(r, 1);
        assert_eq!(x1.0, 7071);
        assert_eq!(y1.0, 7071);
    }

    #[test]
    fn deterministic_replay() {
        let a = resolve(ConstructiveRatio::AdQuadratumDiagonal, MilliUnit(7777));
        let b = resolve(ConstructiveRatio::AdQuadratumDiagonal, MilliUnit(7777));
        assert_eq!(a.0, b.0);
    }

    #[test]
    fn pointed_arch_height() {
        let apex = pointed_arch_apex(MilliUnit(2000), MilliUnit(5000));
        assert_eq!(apex.0, 6732);
    }
}
