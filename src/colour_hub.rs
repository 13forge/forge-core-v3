//! Integer sRGB8 ⇄ [`OklchColor`] bridge — ported from
//! `E:\13forge-super\crates\forge-core\src\colour_oklch.rs` (v2 quarry,
//! zero-float, Q32 fixed point, Newton cube root, CORDIC atan2/sin/cos).
//! Only the boundary quantization changed: v2 wrote permyriad/centidegree
//! (`OklchPmy`); this crate's own law (`colour.rs`) refuses that type, so
//! the u16/binary-angle `OklchColor` shape is targeted directly instead.

use crate::colour::OklchColor;
use crate::fixed_point::isqrt_i128;
use crate::sky::Spectral;

const ONE: i64 = 1 << 32; // Q32 unity
const HALF: i128 = 1 << 31;
/// `0.4 * 2^32`, integer-exact — `OklchColor.c`'s gamut ceiling in Q32.
const CHROMA_CEILING_Q32: i64 = ((1i128 << 32) * 4 / 10) as i64;

/// sRGB byte → linear Q32 (gamma 2.4 decode, IEC 61966-2-1). LUT[0] = 0,
/// LUT[255] = 2^32.
#[rustfmt::skip]
const SRGB_TO_LINEAR_Q32: [i64; 256] = [
    0, 1303638, 2607277, 3910915, 5214554, 6518192, 7821831, 9125469,
    10429108, 11732746, 13036385, 14373262, 15790479, 17286028, 18861100, 20516859,
    22254445, 24074974, 25979540, 27969217, 30045058, 32208097, 34459351, 36799819,
    39230483, 41752310, 44366252, 47073245, 49874213, 52770066, 55761699, 58849999,
    62035836, 65320072, 68703557, 72187129, 75771618, 79457840, 83246606, 87138715,
    91134956, 95236110, 99442951, 103756241, 108176738, 112705190, 117342336, 122088910,
    126945637, 131913236, 136992419, 142183890, 147488347, 152906483, 158438983, 164086527,
    169849787, 175729433, 181726125, 187840520, 194073270, 200425020, 206896410, 213488075,
    220200647, 227034750, 233991006, 241070029, 248272432, 255598822, 263049800, 270625965,
    278327911, 286156228, 294111500, 302194310, 310405235, 318744849, 327213722, 335812420,
    344541505, 353401536, 362393069, 371516656, 380772844, 390162179, 399685202, 409342452,
    419134464, 429061771, 439124900, 449324378, 459660727, 470134468, 480746117, 491496188,
    502385193, 513413639, 524582032, 535890876, 547340670, 558931911, 570665096, 582540715,
    594559259, 606721216, 619027069, 631477301, 644072393, 656812822, 669699062, 682731588,
    695910869, 709237375, 722711571, 736333921, 750104887, 764024929, 778094504, 792314068,
    806684075, 821204974, 835877217, 850701250, 865677518, 880806466, 896088534, 911524162,
    927113788, 942857848, 958756776, 974811004, 991020962, 1007387080, 1023909783, 1040589497,
    1057426646, 1074421652, 1091574933, 1108886910, 1126357997, 1143988612, 1161779166, 1179730072,
    1197841741, 1216114580, 1234548998, 1253145399, 1271904188, 1290825768, 1309910540, 1329158903,
    1348571255, 1368147994, 1387889515, 1407796211, 1427868476, 1448106700, 1468511273, 1489082583,
    1509821018, 1530726963, 1551800803, 1573042920, 1594453697, 1616033513, 1637782748, 1659701780,
    1681790986, 1704050740, 1726481418, 1749083392, 1771857033, 1794802713, 1817920800, 1841211663,
    1864675669, 1888313183, 1912124571, 1936110195, 1960270418, 1984605602, 2009116106, 2033802290,
    2058664511, 2083703126, 2108918492, 2134310962, 2159880891, 2185628631, 2211554533, 2237658948,
    2263942226, 2290404715, 2317046762, 2343868714, 2370870917, 2398053714, 2425417449, 2452962464,
    2480689102, 2508597703, 2536688606, 2564962150, 2593418673, 2622058511, 2650882000, 2679889476,
    2709081272, 2738457722, 2768019157, 2797765909, 2827698309, 2857816685, 2888121368, 2918612683,
    2949290960, 2980156522, 3011209697, 3042450808, 3073880179, 3105498132, 3137304990, 3169301073,
    3201486702, 3233862197, 3266427875, 3299184056, 3332131055, 3365269190, 3398598775, 3432120126,
    3465833556, 3499739379, 3533837907, 3568129451, 3602614323, 3637292833, 3672165290, 3707232003,
    3742493280, 3777949427, 3813600753, 3849447561, 3885490158, 3921728847, 3958163933, 3994795718,
    4031624505, 4068650595, 4105874288, 4143295886, 4180915687, 4218733990, 4256751094, 4294967296,
];

// Ottosson OKLab matrices, Q32. Forward: linear rgb → lms; lms' (cbrt) → Lab.
const M1: [[i64; 3]; 3] = [
    [1770477736, 2303530703, 220958857],
    [910118595, 2923582285, 461266416],
    [379256186, 1209973194, 2705737916],
];
const M2: [[i64; 3]; 3] = [
    [903894144, 3408562432, -17489308],
    [8495438848, -10430724096, 1935285248],
    [111256992, 3361979136, -3473235968],
];
// Inverse: Lab → lms' (row = [a-coef, b-coef], L-coef is 1); lms → linear rgb.
const M2I_AB: [[i64; 2]; 3] = [
    [1702257792, 926870080],
    [-453382528, -274251584],
    [-384331616, -5546888192],
];
const M1I: [[i64; 3]; 3] = [
    [17509472113, -14206513109, 992008292],
    [-5447899747, 11208822688, -1465955645],
    [-18022053, -3021159946, 7334149295],
];

// CORDIC: atan(2^-k) in degrees·2^32, k = 0..39, and the gain reciprocal.
#[rustfmt::skip]
const ATAN_DEG_Q32: [i64; 40] = [
    193273528320, 114096026022, 60285206653, 30601712202,
    15360239180, 7687607525, 3844741810, 1922488225,
    961258780, 480631223, 240315841, 120157949,
    60078978, 30039490, 15019745, 7509872,
    3754936, 1877468, 938734, 469367,
    234684, 117342, 58671, 29335,
    14668, 7334, 3667, 1833,
    917, 458, 229, 115,
    57, 29, 14, 7,
    4, 2, 1, 0,
];
const CORDIC_K_Q32: i64 = 2608131496; // 0.6072529350088813 · 2^32

/// Q32 × Q32 → Q32, round-half-up (deterministic for negatives too).
#[inline]
fn mul_q32(a: i64, b: i64) -> i64 {
    ((a as i128 * b as i128 + HALF) >> 32) as i64
}

/// Floor cube root of a non-negative i128 — Newton on integers.
fn icbrt_i128(n: i128) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = 1i128 << ((128 - n.leading_zeros() as i32 + 2) / 3);
    loop {
        let y = (2 * x + n / (x * x)) / 3;
        if y >= x {
            return x as i64;
        }
        x = y;
    }
}

/// Q32 cube root: y/2^32 = (x/2^32)^(1/3) ⇒ y = cbrt(x · 2^64).
#[inline]
fn cbrt_q32(x: i64) -> i64 {
    if x >= 0 {
        icbrt_i128((x as i128) << 64)
    } else {
        -icbrt_i128((-(x as i128)) << 64)
    }
}

/// Signed Q32 cube: x³ (two rounded Q32 multiplies).
#[inline]
fn cube_q32(x: i64) -> i64 {
    mul_q32(mul_q32(x, x), x)
}

/// CORDIC vectoring: atan2(y, x) in degrees·2^32 over [0°, 360°). (0,0) → 0.
fn atan2_deg_q32(y: i64, x: i64) -> i64 {
    if x == 0 && y == 0 {
        return 0;
    }
    let (mut vx, mut vy, mut acc) = if x < 0 {
        (-(x as i128), -(y as i128), 180i64 << 32)
    } else {
        (x as i128, y as i128, 0i64)
    };
    for (k, &a) in ATAN_DEG_Q32.iter().enumerate() {
        if vy > 0 {
            let nx = vx + (vy >> k);
            vy -= vx >> k;
            vx = nx;
            acc += a;
        } else {
            let nx = vx - (vy >> k);
            vy += vx >> k;
            vx = nx;
            acc -= a;
        }
    }
    acc.rem_euclid(360i64 << 32)
}

/// CORDIC rotation: (cos θ, sin θ) in Q32 for θ in degrees·2^32 (any sign).
fn sin_cos_deg_q32(theta: i64) -> (i64, i64) {
    let mut t = (theta + (180i64 << 32)).rem_euclid(360i64 << 32) - (180i64 << 32);
    let mut flip = false;
    if t > 90i64 << 32 {
        t -= 180i64 << 32;
        flip = true;
    } else if t < -(90i64 << 32) {
        t += 180i64 << 32;
        flip = true;
    }
    let (mut vx, mut vy) = (CORDIC_K_Q32 as i128, 0i128);
    let mut acc = t;
    for (k, &a) in ATAN_DEG_Q32.iter().enumerate() {
        if acc >= 0 {
            let nx = vx - (vy >> k);
            vy += vx >> k;
            vx = nx;
            acc -= a;
        } else {
            let nx = vx + (vy >> k);
            vy -= vx >> k;
            vx = nx;
            acc += a;
        }
    }
    if flip {
        (-(vx as i64), -(vy as i64))
    } else {
        (vx as i64, vy as i64)
    }
}

/// Nearest byte whose linear decode is closest to `linear` Q32 — the EXACT
/// gamma encode: binary search on the monotone decode LUT, so encode∘decode
/// is identity on every byte by construction.
fn linear_q32_to_srgb8(linear: i64) -> u8 {
    let x = linear.clamp(0, ONE);
    let mut lo = 0usize;
    let mut hi = 255usize;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if SRGB_TO_LINEAR_Q32[mid + 1] <= x {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo < 255 && (SRGB_TO_LINEAR_Q32[lo + 1] - x) < (x - SRGB_TO_LINEAR_Q32[lo]) {
        (lo + 1) as u8
    } else {
        lo as u8
    }
}

/// sRGB byte triple → [`OklchColor`], opaque (alpha = `u16::MAX`).
pub fn rgb8_to_oklch(r: u8, g: u8, b: u8) -> OklchColor {
    let lin = [
        SRGB_TO_LINEAR_Q32[r as usize],
        SRGB_TO_LINEAR_Q32[g as usize],
        SRGB_TO_LINEAR_Q32[b as usize],
    ];
    let mut lms = [0i64; 3];
    for (i, row) in M1.iter().enumerate() {
        lms[i] = mul_q32(row[0], lin[0]) + mul_q32(row[1], lin[1]) + mul_q32(row[2], lin[2]);
    }
    let lms_c = [cbrt_q32(lms[0]), cbrt_q32(lms[1]), cbrt_q32(lms[2])];
    let mut lab = [0i64; 3];
    for (i, row) in M2.iter().enumerate() {
        lab[i] = mul_q32(row[0], lms_c[0]) + mul_q32(row[1], lms_c[1]) + mul_q32(row[2], lms_c[2]);
    }
    let c_q32 = isqrt_i128(lab[1] as i128 * lab[1] as i128 + lab[2] as i128 * lab[2] as i128);
    let h_deg_q32 = atan2_deg_q32(lab[2], lab[1]);

    let l = ((lab[0].clamp(0, ONE) as i128 * 65_535 + HALF) >> 32) as u16;
    let c = ((c_q32.min(CHROMA_CEILING_Q32) as i128 * 65_535 + CHROMA_CEILING_Q32 as i128 / 2)
        / CHROMA_CEILING_Q32 as i128) as u16;
    let h = if c == 0 {
        0
    } else {
        (((h_deg_q32 as i128 * 65_536 + (180i128 << 32)) / (360i128 << 32)) & 0xFFFF) as u16
    };
    OklchColor { l, c, h, a: u16::MAX }
}

/// [`OklchColor`] → linear Q32 rgb, unclamped — the shared body of
/// [`oklch_to_rgb8`] and the gamut probe.
fn oklch_to_linear_q32(p: OklchColor) -> [i64; 3] {
    let l_q = ((p.l as i128 * ONE as i128 + 32_767) / 65_535) as i64;
    let c_q = ((p.c as i128 * CHROMA_CEILING_Q32 as i128 + 32_767) / 65_535) as i64;
    let h_q = ((p.h as i128 * (360i128 << 32) + 32_768) / 65_536) as i64;
    let (a, b) = if p.c == 0 {
        (0, 0)
    } else {
        let (cos, sin) = sin_cos_deg_q32(h_q);
        (mul_q32(c_q, cos), mul_q32(c_q, sin))
    };
    let lms_c = [
        l_q + mul_q32(M2I_AB[0][0], a) + mul_q32(M2I_AB[0][1], b),
        l_q + mul_q32(M2I_AB[1][0], a) + mul_q32(M2I_AB[1][1], b),
        l_q + mul_q32(M2I_AB[2][0], a) + mul_q32(M2I_AB[2][1], b),
    ];
    let lms = [cube_q32(lms_c[0]), cube_q32(lms_c[1]), cube_q32(lms_c[2])];
    let mut lin = [0i64; 3];
    for (i, row) in M1I.iter().enumerate() {
        lin[i] = mul_q32(row[0], lms[0]) + mul_q32(row[1], lms[1]) + mul_q32(row[2], lms[2]);
    }
    lin
}

/// [`OklchColor`] → sRGB byte triple. Hue is ignored when `c == 0`
/// (achromatic), matching [`OklchColor::is_achromatic`].
pub fn oklch_to_rgb8(p: OklchColor) -> [u8; 3] {
    let lin = oklch_to_linear_q32(p);
    [
        linear_q32_to_srgb8(lin[0]),
        linear_q32_to_srgb8(lin[1]),
        linear_q32_to_srgb8(lin[2]),
    ]
}

/// One 8-bit code's slack: below this, the clamp in [`linear_q32_to_srgb8`]
/// costs less than the encode's own quantization, so it is not a clip.
const GAMUT_EPS_Q32: i64 = 1_303_638;

/// True when every linear channel lands inside sRGB without clamping —
/// the predicate the chroma search binaries on.
fn in_srgb_gamut(p: OklchColor) -> bool {
    oklch_to_linear_q32(p)
        .iter()
        .all(|&v| v >= -GAMUT_EPS_Q32 && v <= ONE + GAMUT_EPS_Q32)
}

/// Largest u16 chroma that stays in sRGB at this lightness and hue.
/// Monotone in `c` for fixed `(l, h)`, so a plain binary search is exact
/// to one u16 step.
pub fn max_chroma_in_gamut(l: u16, h: u16) -> u16 {
    let (mut lo, mut hi) = (0u16, u16::MAX);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if in_srgb_gamut(OklchColor { l, c: mid, h, a: u16::MAX }) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

/// Strictly-increasing squash of `want` into `[0, ceiling)`: linear below
/// 0.6·ceiling, hyperbolic above it. A hard `min` at the ceiling would fuse
/// every already-saturated tint onto one wall colour — the whole K/M half of
/// the Teff LUT — so the top of the range compresses instead of clipping.
fn soft_knee(want: i64, ceiling: i64) -> i64 {
    let knee = ceiling * 6 / 10;
    if want <= knee || ceiling == 0 {
        return want.max(0).min(ceiling);
    }
    let (over, head) = (want - knee, ceiling - knee);
    knee + head * over / (over + head)
}

/// Star ink: keep the source tint's HUE, restate it at `l_pmy` lightness with
/// its own chroma times `chroma_gain_pmy` (10_000 = 1x), held under the gamut
/// ceiling at that lightness instead of clipped. Replaces per-channel
/// gain, which clamps every high-luma tint onto the same clipped face (the
/// Teff LUT above index 40 pins blue at 255, so 119k stars read as one
/// colour). Relative, not absolute: a near-white A-class face stays pale
/// while a K/M ember goes vivid, so the field keeps its middle.
pub fn star_ink_rgb(r: u8, g: u8, b: u8, l_pmy: i32, chroma_gain_pmy: i32) -> [u8; 3] {
    let src = rgb8_to_oklch(r, g, b);
    let l = (l_pmy.clamp(0, 10_000) as i64 * 65_535 / 10_000) as u16;
    if src.c == 0 {
        return oklch_to_rgb8(OklchColor { l, c: 0, h: 0, a: u16::MAX });
    }
    let want = src.c as i64 * chroma_gain_pmy.max(0) as i64 / 10_000;
    let c = soft_knee(want, max_chroma_in_gamut(l, src.h) as i64) as u16;
    oklch_to_rgb8(OklchColor { l, c, h: src.h, a: u16::MAX })
}

/// The sky chart's OKLCH magnitude glow, one home (donor v2 sky_verb::mag_ink
/// :46-56): shimmer breathes lightness itself (±0.06 L), chroma washes
/// 0.05→vivid 0.22, hue walks deep blue 220°→warm gold 85°. pmy lanes cross
/// to the u16 channels here (chroma ceiling 0.4, hue bam) and exit as sRGB.
pub fn mag_glow_rgb(mag_permyriad: i32, shimmer_pmy: i32) -> [u8; 3] {
    let norm = crate::sky::mag_norm(mag_permyriad);
    let l_pmy = (4_500 + 5_100 * norm / 1_000 + 1_800 * shimmer_pmy / 10_000).clamp(0, 10_000);
    let c_pmy = 500 + 1_700 * norm / 1_000;
    let h_cdeg = 22_000 - 13_500 * norm / 1_000;
    oklch_to_rgb8(OklchColor {
        l: (l_pmy * 65_535 / 10_000) as u16,
        c: ((c_pmy * 65_535 / 4_000).min(65_535)) as u16,
        h: ((h_cdeg as i64 * 65_536 / 36_000) & 0xFFFF) as u16,
        a: u16::MAX,
    })
}

/// The spectral class this effective temperature burns at — Morgan-Keenan
/// boundaries, the same ones `sky_chart` reads. F and G share [`Spectral::
/// AskiyGold`], as the authored table does.
pub fn spectral_of_kelvin(kelvin: i32) -> Spectral {
    match kelvin {
        k if k >= 30_000 => Spectral::DeepWinter,
        k if k >= 10_000 => Spectral::BoneStar,
        k if k >= 7_500 => Spectral::Frost,
        k if k >= 5_200 => Spectral::AskiyGold,
        k if k >= 3_700 => Spectral::TheForge,
        _ => Spectral::Wisakedjak,
    }
}

/// Class anchors in Kelvin — the temperature each authored [`Spectral`]
/// colour sits AT, so a star between two classes reads between two inks
/// instead of snapping. Ascending, the order the walk below assumes.
const CLASS_ANCHORS: [(i32, Spectral); 6] = [
    (3_000, Spectral::Wisakedjak),
    (4_400, Spectral::TheForge),
    (5_800, Spectral::AskiyGold),
    (8_500, Spectral::Frost),
    (16_000, Spectral::BoneStar),
    (33_000, Spectral::DeepWinter),
];

/// Natural-log of Kelvin in Q16 — temperature is a LOG axis perceptually
/// (3000K->4400K is a bigger colour step than 16000K->33000K), and integer
/// Q16 keeps the walk deterministic.
fn ln_k_q16(kelvin: i32) -> i64 {
    let k = kelvin.max(1) as f64;
    (k.ln() * 65_536.0) as i64
}

/// Shortest-arc hue interpolation on the u16 hue wheel.
fn lerp_hue(a: u16, b: u16, t_q16: i64) -> u16 {
    let d = (b as i32 - a as i32 + 32_768).rem_euclid(65_536) - 32_768;
    ((a as i64 + (d as i64 * t_q16 >> 16)).rem_euclid(65_536)) as u16
}

/// STAR INK BY TYPE — the one home for what colour a star burns.
///
/// Hue and chroma come from the authored [`Spectral`] anchors interpolated
/// by log-temperature, so the class palette IS the law and the ramp between
/// classes is continuous. Lightness comes from apparent magnitude, so a
/// blaze and a smudge of the same class differ in weight, not in hue.
/// Chroma is then held under the gamut ceiling by [`soft_knee`] — never
/// clipped per channel, which is what fused the whole catalog into two faces.
///
/// `chroma_gain_pmy` is the one aesthetic knob (10_000 = the anchors' own
/// saturation); everything else is physics or authored table.
pub fn star_ink_by_type(kelvin: i32, mag_permyriad: i32, chroma_gain_pmy: i32) -> [u8; 3] {
    let lk = ln_k_q16(kelvin);
    let (mut lo, mut hi) = (CLASS_ANCHORS[0], CLASS_ANCHORS[CLASS_ANCHORS.len() - 1]);
    let mut t_q16: i64 = 0;
    if lk <= ln_k_q16(lo.0) {
        hi = lo;
    } else if lk >= ln_k_q16(hi.0) {
        lo = hi;
    } else {
        for pair in CLASS_ANCHORS.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (la, lb) = (ln_k_q16(a.0), ln_k_q16(b.0));
            if lk >= la && lk <= lb {
                lo = a;
                hi = b;
                t_q16 = ((lk - la) << 16) / (lb - la).max(1);
                break;
            }
        }
    }
    let rgba_lo = lo.1.rgba();
    let rgba_hi = hi.1.rgba();
    let a = rgb8_to_oklch(rgba_lo[0], rgba_lo[1], rgba_lo[2]);
    let b = rgb8_to_oklch(rgba_hi[0], rgba_hi[1], rgba_hi[2]);
    let h = lerp_hue(a.h, b.h, t_q16);
    let c_anchor = a.c as i64 + ((b.c as i64 - a.c as i64) * t_q16 >> 16);

    // Magnitude TILTS lightness, it does not carry it: the consumer already
    // weights a star by flux (sprite alpha/size). Spending the full L range
    // here too double-counts magnitude and sinks the faint field to mud.
    let norm = crate::sky::mag_norm(mag_permyriad) as i64;
    let l = ((6_400 + 1_600 * norm / 1_000) * 65_535 / 10_000) as u16;

    let want = c_anchor * chroma_gain_pmy.max(0) as i64 / 10_000;
    let c = soft_knee(want, max_chroma_in_gamut(l, h) as i64) as u16;
    oklch_to_rgb8(OklchColor { l, c, h, a: u16::MAX })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The glow walks blue→gold as the star brightens: a −6.5 blaze reads
    /// warmer (more red than blue) and lighter than a +3.4 smudge.
    #[test]
    fn brighter_magnitude_glows_warmer_and_lighter() {
        let blaze = mag_glow_rgb(-65_000, 0);
        let smudge = mag_glow_rgb(34_400, 0);
        assert!(blaze[0] > blaze[2], "blaze is warm: {blaze:?}");
        assert!(smudge[2] >= smudge[0], "smudge is cool: {smudge:?}");
        let luma = |c: [u8; 3]| c[0] as u32 + c[1] as u32 + c[2] as u32;
        assert!(luma(blaze) > luma(smudge), "{blaze:?} vs {smudge:?}");
    }

    /// Deterministic LCG (Numerical Recipes constants) — no rand dep.
    struct Lcg(u64);
    impl Lcg {
        fn next_byte(&mut self) -> u8 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 56) as u8
        }
    }

    /// Step-0 probe, re-derived for u16 storage (the v2 codec's "≤1" bound
    /// was measured at permyriad precision — coarser than u16, so it does
    /// not carry over unchanged; this is the honest bound at this crate's
    /// own quantization, not a copy-pasted constant).
    #[test]
    fn step0_probe_10k_random_rgb_round_trip_within_one() {
        let mut rng = Lcg(0x13F0_46E5_5EED);
        let mut max_delta = 0i32;
        let mut worst = ([0u8; 3], [0u8; 3]);
        for _ in 0..10_000 {
            let (r, g, b) = (rng.next_byte(), rng.next_byte(), rng.next_byte());
            let back = oklch_to_rgb8(rgb8_to_oklch(r, g, b));
            for (src, dst) in [r, g, b].into_iter().zip(back) {
                let d = (src as i32 - dst as i32).abs();
                if d > max_delta {
                    max_delta = d;
                    worst = ([r, g, b], back);
                }
            }
        }
        assert!(
            max_delta <= 1,
            "Step-0 probe FAILED: max channel delta {max_delta} (worst {:?} -> {:?})",
            worst.0,
            worst.1
        );
    }

    #[test]
    fn black_and_white_round_trip_exact() {
        assert_eq!(oklch_to_rgb8(rgb8_to_oklch(0, 0, 0)), [0, 0, 0]);
        assert_eq!(oklch_to_rgb8(rgb8_to_oklch(255, 255, 255)), [255, 255, 255]);
        let black = rgb8_to_oklch(0, 0, 0);
        assert_eq!((black.l, black.c), (0, 0), "black is l=0 c=0");
        let white = rgb8_to_oklch(255, 255, 255);
        assert_eq!((white.l, white.c), (65_535, 0), "white is l=max c=0");
    }

    #[test]
    fn all_256_greys_round_trip_within_one() {
        for v in 0..=255u8 {
            let p = rgb8_to_oklch(v, v, v);
            assert!(p.c <= 1, "grey {v} must be (near-)achromatic, c={}", p.c);
            let back = oklch_to_rgb8(p);
            for ch in back {
                assert!((ch as i32 - v as i32).abs() <= 1, "grey {v} -> {back:?}");
            }
        }
    }

    #[test]
    fn primaries_round_trip_within_one() {
        for rgb in [[255u8, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0], [0, 255, 255], [255, 0, 255]] {
            let back = oklch_to_rgb8(rgb8_to_oklch(rgb[0], rgb[1], rgb[2]));
            for (s, d) in rgb.into_iter().zip(back) {
                assert!((s as i32 - d as i32).abs() <= 1, "{rgb:?} -> {back:?}");
            }
        }
    }

    /// sRGB red = OKLCH(0.628, 0.258, 29.23°), Ottosson's published value —
    /// tolerance widened from the v2 permyriad test to u16's own grid.
    #[test]
    fn red_lands_on_the_known_oklch_coordinates() {
        let p = rgb8_to_oklch(255, 0, 0);
        assert!((p.l as i32 - 41_140).abs() <= 40, "red L≈0.628, got {}", p.l);
        assert!((p.c as i32 - 42_218).abs() <= 40, "red C≈0.2577, got {}", p.c);
        assert!((p.h as i32 - 5_322).abs() <= 40, "red h≈29.23°, got {}", p.h);
    }

    #[test]
    fn hue_is_ignored_when_achromatic() {
        assert_eq!(
            oklch_to_rgb8(OklchColor { l: 0, c: 0, h: 1, a: u16::MAX }),
            [0, 0, 0]
        );
        assert_eq!(
            oklch_to_rgb8(OklchColor { l: 30_000, c: 0, h: 40_000, a: u16::MAX }),
            oklch_to_rgb8(OklchColor { l: 30_000, c: 0, h: 0, a: u16::MAX })
        );
    }

    /// The ceiling is a true edge: at it the colour is in gamut, one step
    /// past it is not.
    #[test]
    fn max_chroma_is_the_gamut_edge() {
        for h in [0u16, 5_000, 12_000, 30_000, 47_000, 60_000] {
            for l in [15_000u16, 32_000, 48_000, 58_000] {
                let c = max_chroma_in_gamut(l, h);
                assert!(in_srgb_gamut(OklchColor { l, c, h, a: u16::MAX }), "l{l} h{h} c{c}");
                assert!(
                    !in_srgb_gamut(OklchColor { l, c: c + 1, h, a: u16::MAX }),
                    "l{l} h{h} c{c} was not the edge"
                );
            }
        }
    }

    /// The four Teff LUT faces of `shell/assets/hyg_baked.bin` (0/20/60/255,
    /// read 2026-08-27): per-channel gain clamps them onto ~two clipped
    /// colours; the OKLCH ink keeps four separated hues, none clipped.
    #[test]
    fn teff_lut_faces_stay_distinct_as_star_ink() {
        const LUT: [[u8; 3]; 4] = [[255, 136, 13], [255, 227, 205], [196, 214, 255], [151, 185, 255]];
        let hues: Vec<u16> = LUT
            .iter()
            .map(|c| {
                let ink = star_ink_rgb(c[0], c[1], c[2], 7_400, 35_000);
                rgb8_to_oklch(ink[0], ink[1], ink[2]).h
            })
            .collect();
        for (i, a) in hues.iter().enumerate() {
            for b in &hues[i + 1..] {
                let d = (*a as i32 - *b as i32).abs().min(65_536 - (*a as i32 - *b as i32).abs());
                assert!(d > 200, "hues collapsed: {hues:?}");
            }
        }
        for c in LUT {
            let ink = star_ink_rgb(c[0], c[1], c[2], 7_400, 35_000);
            assert!(ink.iter().filter(|&&v| v == 255 || v == 0).count() <= 1, "{c:?} -> clipped {ink:?}");
        }
    }

    /// The knee never reaches the wall and never reorders. Strictly rising
    /// through 3x the ceiling (where every real Teff face lands); beyond that
    /// integer division flattens the asymptote, which stays non-decreasing.
    #[test]
    fn soft_knee_rises_and_never_hits_the_wall() {
        let ceiling = 20_000i64;
        let mut prev = -1i64;
        for want in (0..200_000).step_by(97) {
            let c = soft_knee(want, ceiling);
            assert!(c < ceiling, "want {want} hit the wall");
            if want <= 3 * ceiling {
                assert!(c > prev, "flat at want {want}: {c} <= {prev}");
            } else {
                assert!(c >= prev, "fell at want {want}: {c} < {prev}");
            }
            prev = c;
        }
    }

    /// The two hottest Teff faces of the real LUT (idx 0 and 8, read from
    /// `shell/assets/hyg_baked.bin` 2026-08-27) must not fuse — 21k stars
    /// live in indices 0-15.
    #[test]
    fn the_hot_teff_faces_do_not_fuse() {
        let a = star_ink_rgb(255, 136, 13, 7_400, 35_000);
        let b = star_ink_rgb(255, 183, 122, 7_400, 35_000);
        let d: i32 = a.iter().zip(b).map(|(x, y)| (*x as i32 - y as i32).abs()).sum();
        assert!(d > 20, "hot faces fused: {a:?} vs {b:?}");
    }

    /// The middle survives: a near-white A-class face stays far paler than a
    /// K-class ember at the same gain — the failure mode of a flat
    /// "everything to the gamut edge" rule.
    #[test]
    fn star_ink_keeps_the_pale_faces_pale() {
        let chroma = |c: [u8; 3]| {
            let ink = star_ink_rgb(c[0], c[1], c[2], 7_400, 35_000);
            rgb8_to_oklch(ink[0], ink[1], ink[2]).c as i32
        };
        let pale = chroma([255, 253, 249]); // LUT idx 30, A-class white
        let ember = chroma([255, 136, 13]); // LUT idx 0, the hot orange face
        assert!(ember > pale * 3, "pale {pale} vs ember {ember}");
    }

    /// Ink honours its requested lightness (within the u16 grid) instead of
    /// riding whatever the source tint's luma happened to be.
    #[test]
    fn star_ink_lands_on_the_requested_lightness() {
        for l_pmy in [4_000, 6_200, 7_400, 8_800] {
            let ink = star_ink_rgb(196, 214, 255, l_pmy, 35_000);
            let got = rgb8_to_oklch(ink[0], ink[1], ink[2]).l as i32;
            let want = l_pmy * 65_535 / 10_000;
            assert!((got - want).abs() <= 400, "l{l_pmy}: want {want}, got {got}");
        }
    }

    #[test]
    fn achromatic_source_yields_achromatic_ink() {
        let ink = star_ink_rgb(200, 200, 200, 7_400, 35_000);
        assert_eq!(ink[0], ink[1]);
        assert_eq!(ink[1], ink[2]);
    }

    /// Morgan-Keenan boundaries, both sides of each edge.
    #[test]
    fn spectral_classes_land_on_the_mk_boundaries() {
        assert_eq!(spectral_of_kelvin(35_000), Spectral::DeepWinter);
        assert_eq!(spectral_of_kelvin(30_000), Spectral::DeepWinter);
        assert_eq!(spectral_of_kelvin(29_999), Spectral::BoneStar);
        assert_eq!(spectral_of_kelvin(10_000), Spectral::BoneStar);
        assert_eq!(spectral_of_kelvin(9_999), Spectral::Frost);
        assert_eq!(spectral_of_kelvin(7_500), Spectral::Frost);
        assert_eq!(spectral_of_kelvin(5_778), Spectral::AskiyGold); // Sol
        assert_eq!(spectral_of_kelvin(4_000), Spectral::TheForge);
        assert_eq!(spectral_of_kelvin(3_100), Spectral::Wisakedjak);
    }

    /// The ramp is CONTINUOUS in temperature: no 1K step may jump the ink,
    /// which is what a class-snapped palette does at every boundary.
    #[test]
    fn the_type_ramp_never_snaps_across_a_class_edge() {
        for edge in [3_700, 5_200, 7_500, 10_000, 30_000] {
            let below = star_ink_by_type(edge - 1, 20_000, 10_000);
            let above = star_ink_by_type(edge + 1, 20_000, 10_000);
            for (x, y) in below.into_iter().zip(above) {
                assert!(
                    (x as i32 - y as i32).abs() <= 3,
                    "snap at {edge}K: {below:?} -> {above:?}"
                );
            }
        }
    }

    /// Hotter reads bluer, cooler reads warmer — the whole point of typing.
    #[test]
    fn hot_types_read_blue_and_cool_types_read_warm() {
        let m = star_ink_by_type(3_000, 20_000, 10_000);
        let g = star_ink_by_type(5_778, 20_000, 10_000);
        let o = star_ink_by_type(33_000, 20_000, 10_000);
        assert!(m[0] > m[2], "M-class is warm: {m:?}");
        assert!(o[2] > o[0], "O-class is blue: {o:?}");
        assert!(g[0] >= g[2], "G-class sits warm-of-neutral: {g:?}");
        let warmth = |c: [u8; 3]| c[0] as i32 - c[2] as i32;
        assert!(warmth(m) > warmth(g), "M warmer than G");
        assert!(warmth(g) > warmth(o), "G warmer than O");
    }

    /// Same class, different magnitude: one hue, two weights.
    #[test]
    fn magnitude_moves_weight_not_hue() {
        let blaze = star_ink_by_type(5_778, -14_600, 10_000);
        let smudge = star_ink_by_type(5_778, 60_000, 10_000);
        let hue = |c: [u8; 3]| rgb8_to_oklch(c[0], c[1], c[2]).h as i32;
        let d = (hue(blaze) - hue(smudge)).abs().min(65_536 - (hue(blaze) - hue(smudge)).abs());
        assert!(d < 900, "hue drifted with magnitude: {blaze:?} vs {smudge:?}");
        let luma = |c: [u8; 3]| c[0] as u32 + c[1] as u32 + c[2] as u32;
        assert!(luma(blaze) > luma(smudge), "{blaze:?} vs {smudge:?}");
    }

    /// Nothing the type lane emits may clip a channel.
    #[test]
    fn typed_ink_never_clips() {
        for k in (2_000..40_000).step_by(311) {
            for mag in [-14_600, 0, 20_000, 45_000, 90_000] {
                let ink = star_ink_by_type(k, mag, 35_000);
                assert!(
                    ink.iter().filter(|&&v| v == 255 || v == 0).count() <= 1,
                    "{k}K mag{mag} clipped: {ink:?}"
                );
            }
        }
    }

    #[test]
    fn codec_is_deterministic() {
        let a = rgb8_to_oklch(137, 42, 209);
        let b = rgb8_to_oklch(137, 42, 209);
        assert_eq!((a.l, a.c, a.h), (b.l, b.c, b.h));
        assert_eq!(oklch_to_rgb8(a), oklch_to_rgb8(b));
    }

    #[test]
    fn gamma_encode_is_exact_inverse_of_the_lut() {
        for v in 0..=255u8 {
            assert_eq!(linear_q32_to_srgb8(SRGB_TO_LINEAR_Q32[v as usize]), v);
        }
    }
}
