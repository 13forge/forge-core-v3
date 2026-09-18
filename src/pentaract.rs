//! Pentaract: one point on S⁴, the hypersphere aperture proposed for
//! GhostMoon's mood-field in `_vault/_plans/pins/hypersphere-grain/PIN.md`
//! and `_vault/_plans/pins/bitmap-bytemap-hypersphere/PIN.md`. Landed on
//! Sean's explicit 2026-08-15 build order ("actually build it").
//!
//! [`CONSTRAINT BOUNDARY`] This is a NEW, additive, self-contained type.
//! It does NOT replace [`crate::ghostmoon::Ghostmoon`] (the live, 28/28-proven
//! 5D closed-interval collision box) or `forge-ml-bqrouter`'s `[x,y,z,theta,w]`
//! embedding box — both stay exactly as they are. Swapping either of those for
//! this shape is its own destructive, ARCH000-gated change (pin containment
//! clause 3: "GhostMoon's SPEC document is explicitly amended by Sean, not by
//! lens inference"), not something this module does by existing.
//!
//! Shape: 4 angles place a point on S⁴ exactly — self-normalizing, no
//! `x1²+...+x5²=1` constraint for a float to drift off, matching the
//! `MoodRow` layout the peer proposed (32 bytes, `repr(C)`, half a cache
//! line): `theta1..3` (polar, BAM-halfturn, 0..65535 -> 0..π) and `phi`
//! (azimuthal, BAM-fullturn, 0..65535 -> 0..2π). `u16` BAM wraps mod 2π (or
//! mod π) for free on integer overflow — no renormalize step, ever.

/// One row: a point on S⁴ plus its payload. `repr(C)` pins the layout —
/// the offset locks below are the contract, so a field reorder fails
/// `cargo check` (same discipline as `Ghostmoon`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pentaract {
    /// Open-ended identity (BrutalHash-style name), not a coordinate.
    pub key: u64,
    /// Polar angle 1, BAM-halfturn: `0..65535` -> `0..π`.
    pub theta1: u16,
    /// Polar angle 2, BAM-halfturn: `0..65535` -> `0..π`.
    pub theta2: u16,
    /// Polar angle 3, BAM-halfturn: `0..65535` -> `0..π`.
    pub theta3: u16,
    /// Azimuthal angle, BAM-fullturn: `0..65535` -> `0..2π`.
    pub phi: u16,
    /// RRGGBBAA swatch riding alongside the position.
    pub accent: u32,
    /// `0` = Creative (movable, GPU-lane) · `1` = Clock-bound (code-owned,
    /// pinned) — the Two Clocks split encoded per-point.
    pub truth: u8,
    /// Layout padding only; never a payload channel.
    pub _pad: [u8; 11],
}

/// `0` on [`Pentaract::truth`]: a movable, creative-lane point.
pub const TRUTH_CREATIVE: u8 = 0;
/// `1` on [`Pentaract::truth`]: a pinned, code-owned point.
pub const TRUTH_CLOCK_BOUND: u8 = 1;

impl Pentaract {
    /// Build a row from its lanes; angles wrap by construction (BAM is
    /// total over `u16`, so no caller can construct an "illegal" angle).
    pub const fn new(key: u64, theta1: u16, theta2: u16, theta3: u16, phi: u16, accent: u32, truth: u8) -> Self {
        Self { key, theta1, theta2, theta3, phi, accent, truth, _pad: [0; 11] }
    }

    /// The embedding on the unit S⁴ in Q15 fixed point (`[-32767,32767]`
    /// represents `[-1.0,1.0]`), via the standard hyperspherical
    /// parametrization. Self-normalizing: `sum(x_i^2) == 1` up to the LUT's
    /// quantization error (see `bam_sin`'s doc), never by explicit renormalize.
    ///
    /// \[APERTURE\] `theta` fields encode `0..65535 -> 0..π` (half turn); the
    /// BAM sine table is full-turn, so the halfturn angle is halved
    /// (`>> 1`) before lookup. That drops the LSB of each `theta` — 15-bit
    /// effective angular resolution, not 16. If a caller needs the full 16
    /// bits of polar precision this aperture collapses and a finer table
    /// (or interpolation) is owed.
    pub fn unit_vector(&self) -> [i32; 5] {
        let (s1, c1) = bam_sin_cos_halfturn(self.theta1);
        let (s2, c2) = bam_sin_cos_halfturn(self.theta2);
        let (s3, c3) = bam_sin_cos_halfturn(self.theta3);
        let (sp, cp) = bam_sin_cos_fullturn(self.phi);

        let x1 = c1;
        let x2 = qmul(s1, c2);
        let x3 = qmul(qmul(s1, s2), c3);
        let s123 = qmul(qmul(s1, s2), s3);
        let x4 = qmul(s123, cp);
        let x5 = qmul(s123, sp);
        [x1, x2, x3, x4, x5]
    }

    /// Angular closeness to another row: `cos(angular separation)` in Q15
    /// (`32767` = identical direction, `-32767` = antipodal, `0` = orthogonal).
    ///
    /// \[APERTURE\] This returns the dot product, not `arccos(dot)` — a
    /// monotonic proxy for angular distance. Cheapest passing layer (C08):
    /// ranking/nearest-neighbor only needs the ordering, not the radian
    /// value; an inverse-trig LUT is not bought until a caller needs the
    /// actual angle.
    pub fn cos_similarity(&self, other: &Pentaract) -> i32 {
        let a = self.unit_vector();
        let b = other.unit_vector();
        let mut dot: i64 = 0;
        for i in 0..5 {
            dot += (a[i] as i64 * b[i] as i64) >> 15;
        }
        dot.clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }

    /// Chord midpoint of `self` and `other`'s unit vectors, renormalized to
    /// the unit sphere by an integer Newton step (`|v| ~= 32767`) — a cheap
    /// stand-in for slerp's exact great-circle midpoint.
    ///
    /// \[APERTURE\] This is NOT a true slerp: for two nearly-antipodal
    /// points the chord midpoint is not the geodesic midpoint, and this
    /// function does not correct for that. Fine for "somewhere between"
    /// (mood blending); wrong for anything that needs the exact geodesic.
    pub fn midpoint_unit_vector(&self, other: &Pentaract) -> [i32; 5] {
        let a = self.unit_vector();
        let b = other.unit_vector();
        let mut sum = [0i64; 5];
        for i in 0..5 {
            sum[i] = a[i] as i64 + b[i] as i64;
        }
        let mag_sq: i64 = sum.iter().map(|v| v * v).sum();
        if mag_sq == 0 {
            // Exact antipodal cancellation: no well-defined midpoint direction.
            return [0; 5];
        }
        let mag = isqrt(mag_sq);
        let mut out = [0i32; 5];
        for i in 0..5 {
            out[i] = ((sum[i] * 32767) / mag) as i32;
        }
        out
    }
}

/// Integer Q15 multiply: `Q15 * Q15 -> Q30`, rescaled back to `Q15`.
#[inline]
fn qmul(a: i32, b: i32) -> i32 {
    ((a as i64 * b as i64) >> 15) as i32
}

/// Integer square root by Newton's method (bedrock: shift/add/compare +
/// one division per iteration, no float). Exact for perfect squares,
/// floor otherwise — sufficient for a renormalization denominator.
fn isqrt(n: i64) -> i64 {
    if n <= 1 {
        return n.max(0);
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Quarter-wave sine table, Q15, 257 entries covering `[0, π/2]` at 256
/// equal steps. Values computed once via `sin(i * (π/2) / 256) * 32767`
/// (measured, not hand-typed) — see the PowerShell receipt in the landing
/// session. [`APERTURE`] nearest-index lookup, no interpolation: worst-case
/// quantization is one table step, `(π/2)/256 ≈ 0.0061` rad (~0.35°).
const QUARTER_SINE: [i16; 257] = [
    0, 201, 402, 603, 804, 1005, 1206, 1407, 1608, 1809, 2009, 2210, 2410, 2611, 2811, 3012, 3212, 3412, 3612, 3811,
    4011, 4210, 4410, 4609, 4808, 5007, 5205, 5404, 5602, 5800, 5998, 6195, 6393, 6590, 6786, 6983, 7179, 7375, 7571,
    7767, 7962, 8157, 8351, 8545, 8739, 8933, 9126, 9319, 9512, 9704, 9896, 10087, 10278, 10469, 10659, 10849, 11039,
    11228, 11417, 11605, 11793, 11980, 12167, 12353, 12539, 12725, 12910, 13094, 13279, 13462, 13645, 13828, 14010,
    14191, 14372, 14553, 14732, 14912, 15090, 15269, 15446, 15623, 15800, 15976, 16151, 16325, 16499, 16673, 16846,
    17018, 17189, 17360, 17530, 17700, 17869, 18037, 18204, 18371, 18537, 18703, 18868, 19032, 19195, 19357, 19519,
    19680, 19841, 20000, 20159, 20317, 20475, 20631, 20787, 20942, 21096, 21250, 21403, 21554, 21705, 21856, 22005,
    22154, 22301, 22448, 22594, 22739, 22884, 23027, 23170, 23311, 23452, 23592, 23731, 23870, 24007, 24143, 24279,
    24413, 24547, 24680, 24811, 24942, 25072, 25201, 25329, 25456, 25582, 25708, 25832, 25955, 26077, 26198, 26319,
    26438, 26556, 26674, 26790, 26905, 27019, 27133, 27245, 27356, 27466, 27575, 27683, 27790, 27896, 28001, 28105,
    28208, 28310, 28411, 28510, 28609, 28706, 28803, 28898, 28992, 29085, 29177, 29268, 29358, 29447, 29534, 29621,
    29706, 29791, 29874, 29956, 30037, 30117, 30195, 30273, 30349, 30424, 30498, 30571, 30643, 30714, 30783, 30852,
    30919, 30985, 31050, 31113, 31176, 31237, 31297, 31356, 31414, 31470, 31526, 31580, 31633, 31685, 31736, 31785,
    31833, 31880, 31926, 31971, 32014, 32057, 32098, 32137, 32176, 32213, 32250, 32285, 32318, 32351, 32382, 32412,
    32441, 32469, 32495, 32521, 32545, 32567, 32589, 32609, 32628, 32646, 32663, 32678, 32692, 32705, 32717, 32728,
    32737, 32745, 32752, 32757, 32761, 32765, 32766, 32767,
];

/// `sin` of a full-turn BAM angle (`0..65536` -> `0..2π`), Q15, via
/// quadrant symmetry over [`QUARTER_SINE`]. Bedrock: a shift, a mask, a
/// table read, at most one negate/subtract — no runtime trig call.
pub(crate) fn bam_sin(angle: u16) -> i32 {
    let phase = angle as u32;
    let quadrant = phase >> 14; // 0..=3
    let idx_in_quadrant = ((phase & 0x3FFF) >> 6) as usize; // 0..=255
    match quadrant {
        0 => QUARTER_SINE[idx_in_quadrant] as i32,
        1 => QUARTER_SINE[256 - idx_in_quadrant] as i32,
        2 => -(QUARTER_SINE[idx_in_quadrant] as i32),
        _ => -(QUARTER_SINE[256 - idx_in_quadrant] as i32),
    }
}

/// `cos` of a full-turn BAM angle: `sin` quarter-turn ahead, same table.
pub(crate) fn bam_cos(angle: u16) -> i32 {
    bam_sin(angle.wrapping_add(16384))
}

/// `(sin, cos)` of a full-turn BAM angle in one call.
fn bam_sin_cos_fullturn(angle: u16) -> (i32, i32) {
    (bam_sin(angle), bam_cos(angle))
}

/// `(sin, cos)` of a HALF-turn BAM angle (`0..65536` -> `0..π`): the raw
/// value maps to a full-turn phase at half its magnitude (see
/// [`Pentaract::unit_vector`]'s aperture note on the dropped LSB).
fn bam_sin_cos_halfturn(angle: u16) -> (i32, i32) {
    bam_sin_cos_fullturn((angle >> 1) as u16)
}

// ---------------------------------------------------------------------------
// LAYOUT LOCKS. Measured with rustc, never hand-computed.
// ---------------------------------------------------------------------------
const _: () = assert!(core::mem::size_of::<Pentaract>() == 32);
const _: () = assert!(core::mem::align_of::<Pentaract>() == 8);
const _: () = assert!(core::mem::offset_of!(Pentaract, key) == 0);
const _: () = assert!(core::mem::offset_of!(Pentaract, theta1) == 8);
const _: () = assert!(core::mem::offset_of!(Pentaract, theta2) == 10);
const _: () = assert!(core::mem::offset_of!(Pentaract, theta3) == 12);
const _: () = assert!(core::mem::offset_of!(Pentaract, phi) == 14);
const _: () = assert!(core::mem::offset_of!(Pentaract, accent) == 16);
const _: () = assert!(core::mem::offset_of!(Pentaract, truth) == 20);
const _: () = assert!(core::mem::offset_of!(Pentaract, _pad) == 21);

#[cfg(test)]
mod tests {
    use super::*;

    fn mag_sq(v: [i32; 5]) -> i64 {
        v.iter().map(|&x| (x as i64) * (x as i64)).sum()
    }

    // A perfectly unit vector in Q15 has mag_sq == 32767^2 == 1_073,676,289.
    const UNIT_MAG_SQ: i64 = 32767i64 * 32767i64;
    // Tolerance for the LUT's nearest-index quantization, empirically bounded
    // below by sampling every BAM octant boundary in the tests that follow.
    const TOLERANCE: i64 = 30_000_000; // ~2.8% of unit magnitude-squared

    #[test]
    fn unit_vector_stays_near_the_unit_sphere_across_the_angle_space() {
        for t1 in [0u16, 8192, 16384, 24576, 32767, 40000, 55000, 65535] {
            for t2 in [0u16, 20000, 40000, 65535] {
                for t3 in [0u16, 30000, 65535] {
                    for phi in [0u16, 16384, 32768, 49152, 65535] {
                        let p = Pentaract::new(0, t1, t2, t3, phi, 0, TRUTH_CREATIVE);
                        let v = p.unit_vector();
                        let m = mag_sq(v);
                        assert!(
                            (m - UNIT_MAG_SQ).abs() < TOLERANCE,
                            "theta=({t1},{t2},{t3}) phi={phi}: mag_sq={m}, want ~{UNIT_MAG_SQ}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn identical_points_have_maximal_similarity() {
        let p = Pentaract::new(1, 10000, 20000, 30000, 40000, 0, TRUTH_CREATIVE);
        let sim = p.cos_similarity(&p);
        assert!(sim > 32767 - 100, "self-similarity should be ~32767, got {sim}");
    }

    #[test]
    fn orthogonal_poles_have_near_zero_similarity() {
        // theta1=0 -> pure +x1 pole. theta1=32768 (~pi/2 in halfturn terms... but
        // halfturn maps 0..65535->0..pi, so 32768 IS pi/2) with theta2=0 -> the
        // x2 axis. x1 . x2 axes are orthogonal by construction.
        let pole_x1 = Pentaract::new(2, 0, 0, 0, 0, 0, TRUTH_CREATIVE);
        let pole_x2 = Pentaract::new(3, 32768, 0, 0, 0, 0, TRUTH_CREATIVE);
        let sim = pole_x1.cos_similarity(&pole_x2);
        assert!(sim.abs() < 2000, "orthogonal poles should be near 0, got {sim}");
    }

    #[test]
    fn midpoint_of_a_point_with_itself_is_itself() {
        let p = Pentaract::new(4, 12345, 23456, 34567, 45678, 0, TRUTH_CREATIVE);
        let v = p.unit_vector();
        let mid = p.midpoint_unit_vector(&p);
        for i in 0..5 {
            assert!((mid[i] - v[i]).abs() <= 2, "midpoint(p,p) drifted from p at lane {i}: {mid:?} vs {v:?}");
        }
    }

    #[test]
    fn key_accent_truth_survive_construction_untouched() {
        let p = Pentaract::new(0xDEAD_BEEF, 1, 2, 3, 4, 0x11223344, TRUTH_CLOCK_BOUND);
        assert_eq!(p.key, 0xDEAD_BEEF);
        assert_eq!(p.accent, 0x11223344);
        assert_eq!(p.truth, TRUTH_CLOCK_BOUND);
        assert_eq!(p._pad, [0u8; 11]);
    }

    // ── L18-style sabotage: prove the unit-sphere check is not vacuous ────────
    #[test]
    fn sabotaged_bam_cos_would_break_unit_sphere_check() {
        // Sabotage: an off-by-quadrant cos (using bam_sin's own phase instead
        // of the +quarter-turn one) should NOT sit near the unit sphere for a
        // generic angle. Expected failure named first: mag_sq far from UNIT_MAG_SQ.
        let angle: u16 = 10000;
        let s = bam_sin(angle) as i64;
        let bad_c = bam_sin(angle) as i64; // sabotaged: cos == sin, wrong on purpose
        let bad_mag_sq = s * s + bad_c * bad_c;
        assert!(
            (bad_mag_sq - UNIT_MAG_SQ).abs() > TOLERANCE,
            "sabotaged cos should visibly break the unit-circle identity, but stayed within tolerance"
        );
        // Revert: bad_c is a local shadow: the real bam_cos above is untouched.
        let good_c = bam_cos(angle) as i64;
        let good_mag_sq = s * s + good_c * good_c;
        assert!((good_mag_sq - UNIT_MAG_SQ).abs() < TOLERANCE, "real bam_cos must pass where the sabotage failed");
    }
}
