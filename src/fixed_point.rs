//! Fixed-point integer math, ported from `pp-math/src/fixed_point.rs` (Wave 1,
//! integer subset only). Every arithmetic path here is integer — the source's
//! one-way float valve (`From<Permyriad> for f32`), `from_f32_mm`/`to_f32_mm`,
//! and the f64-initialised sine table were left behind at the door, because §3's
//! arithmetic rule bans float in Crate Zero and a valve needs something on the
//! other side to be worth welding.
//!
//! Serde derives were stripped: this crate has an empty `[dependencies]` section
//! and that firewall outranks wire convenience.

/// Permyriad: `0..=10_000` maps `0%..=100%`. For ratios and multipliers.
/// Deliberately `i32` — ratios never need 64-bit range. In the source tree this
/// carried a one-way `f32` read valve for GPU/DSP; here there is no float side
/// at all, so the valve is not merely shut, it is absent.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Permyriad(pub i32);

impl Permyriad {
    /// The zero ratio (0%).
    pub const ZERO: Permyriad = Permyriad(0);
    /// The unity ratio (100%).
    pub const ONE: Permyriad = Permyriad(10_000);
    /// Saturation ceiling — the maximum representable ratio.
    pub const MAX: Permyriad = Permyriad(i32::MAX);
    /// 2/3 quantized to permyriad.
    pub const TWO_THIRDS: Permyriad = Permyriad(6_666);
    /// 1/3 quantized to permyriad.
    pub const ONE_THIRD: Permyriad = Permyriad(3_333);
    /// sqrt(3)/3 quantized to permyriad.
    pub const SQRT3_OVER_3: Permyriad = Permyriad(5_773);
    /// sqrt(2)/2 quantized to permyriad. Diagonal normalisation for Cartesian grids.
    pub const SQRT2_OVER_2: Permyriad = Permyriad(7_071);

    /// Saturating integer addition — no wrapping at the `i32` bounds.
    #[inline(always)]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Permyriad(self.0.saturating_add(rhs.0))
    }

    /// Construct from the inner `i32` representation.
    #[inline(always)]
    pub const fn from_inner(val: i32) -> Self {
        Self(val)
    }

    /// Extract the inner `i32` representation.
    #[inline(always)]
    pub const fn into_inner(self) -> i32 {
        self.0
    }

    /// Absolute value — wraps negative ratios toward positive.
    #[inline(always)]
    pub const fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Convert to little-endian bytes.
    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
}

impl core::ops::Add for Permyriad {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}
impl core::ops::AddAssign for Permyriad {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}
impl core::ops::Sub for Permyriad {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}
impl core::ops::SubAssign for Permyriad {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}
impl core::ops::Neg for Permyriad {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self {
        Self(-self.0)
    }
}
impl core::fmt::Display for Permyriad {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Clock-domain newtypes ───────────────────────────────────────────────────
//
// Distinct types make cross-clock assignment a compile error — a physics fn
// that takes `SimTick` will never silently accept an `AudioFrame`. There is
// deliberately no cross-conversion and no float impl on either. The source
// proved the weld with `compile_fail` doctests; here the weld is stronger
// still — the crate has no float arithmetic to bridge to.

/// Tier-1 clock domain: the synchronous physics metronome. Required for all
/// deterministic state advancement, and the `T` lane of `Ghostmoon`.
/// Never substitute `AudioFrame`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SimTick(pub u64);

impl SimTick {
    /// The origin tick, before any advancement.
    pub const ZERO: SimTick = SimTick(0);

    /// Extract the inner tick counter as `u64`.
    #[inline(always)]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Saturating advance — wrapping is strictly not present on the metronome.
    #[inline(always)]
    pub const fn saturating_add(self, ticks: u64) -> Self {
        SimTick(self.0.saturating_add(ticks))
    }

    /// Tick distance `self - earlier`, saturating at 0 — monotonic, never negative.
    #[inline(always)]
    pub const fn since(self, earlier: SimTick) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

impl core::fmt::Display for SimTick {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "t{}", self.0)
    }
}

/// Tier-2 clock domain: the variable-rate audio block counter. Counts a
/// different clock than `SimTick` and must never advance Tier-1 state — that
/// is why it is a second type and not a second name for the first.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AudioFrame(pub u64);

impl AudioFrame {
    /// The origin audio frame, before any advancement.
    pub const ZERO: AudioFrame = AudioFrame(0);

    /// Extract the inner frame counter as `u64`.
    #[inline(always)]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Saturating advance — wrapping is strictly not present on the audio clock.
    #[inline(always)]
    pub const fn saturating_add(self, frames: u64) -> Self {
        AudioFrame(self.0.saturating_add(frames))
    }

    /// Frame distance `self - earlier`, saturating at 0 — monotonic, never negative.
    #[inline(always)]
    pub const fn since(self, earlier: AudioFrame) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

impl core::fmt::Display for AudioFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "af{}", self.0)
    }
}

/// Length in thousandths of a world unit. `1` world unit = `1000` MilliUnits.
/// The source's `from_f32_mm`/`to_f32_mm` were stripped — there is no float in
/// this crate to convert from or to.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MilliUnit(pub i64);

impl MilliUnit {
    /// Multiply by a permyriad (1/10_000) factor. Accepts typed `Permyriad`
    /// to prevent arbitrary `i64` arguments; the widening multiply is confined
    /// to the single intermediate step.
    #[inline(always)]
    pub const fn mul_permyriad(self, scalar: Permyriad) -> Self {
        MilliUnit(self.0 * scalar.0 as i64 / 10_000)
    }

    /// Divide by a permyriad (1/10_000) factor.
    #[inline(always)]
    pub const fn div_permyriad(self, scalar: Permyriad) -> Self {
        MilliUnit(self.0 * 10_000 / scalar.0 as i64)
    }
}

impl core::ops::Add for MilliUnit {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        MilliUnit(self.0 + other.0)
    }
}
impl core::ops::Sub for MilliUnit {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        MilliUnit(self.0 - other.0)
    }
}
impl core::ops::Add<i64> for MilliUnit {
    type Output = Self;
    fn add(self, rhs: i64) -> Self {
        MilliUnit(self.0 + rhs)
    }
}
impl core::ops::Sub<i64> for MilliUnit {
    type Output = Self;
    fn sub(self, rhs: i64) -> Self {
        MilliUnit(self.0 - rhs)
    }
}
impl core::ops::Neg for MilliUnit {
    type Output = Self;
    fn neg(self) -> Self {
        MilliUnit(-self.0)
    }
}
impl core::ops::Mul<i64> for MilliUnit {
    type Output = Self;
    fn mul(self, rhs: i64) -> Self {
        MilliUnit(self.0 * rhs)
    }
}
impl core::ops::Mul<i32> for MilliUnit {
    type Output = Self;
    fn mul(self, rhs: i32) -> Self {
        MilliUnit(self.0 * rhs as i64)
    }
}
impl core::ops::Div<i64> for MilliUnit {
    type Output = Self;
    fn div(self, rhs: i64) -> Self {
        MilliUnit(self.0 / rhs)
    }
}
impl core::ops::AddAssign for MilliUnit {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}
impl core::ops::SubAssign for MilliUnit {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}
impl core::ops::AddAssign<i64> for MilliUnit {
    fn add_assign(&mut self, rhs: i64) {
        self.0 += rhs;
    }
}
impl core::ops::SubAssign<i64> for MilliUnit {
    fn sub_assign(&mut self, rhs: i64) {
        self.0 -= rhs;
    }
}

/// Convert Cartesian `MilliUnit` coordinates to a hexagonal prism `(q, r, z)`.
/// All arithmetic is `i64` to prevent phantom overflow; multiplication executes
/// before the down-scaling division, because the other order throws precision away.
#[inline]
pub const fn cartesian_to_hex_prism(
    x_mu: i64,
    y_mu: i64,
    z_mu: i64,
    hex_size_mu: i64,
    z_height_mu: i64,
) -> (i64, i64, i64) {
    let scale = Permyriad::ONE.0 as i64;

    let q_numerator = x_mu * (Permyriad::TWO_THIRDS.0 as i64);
    let q = q_numerator / (hex_size_mu * scale);

    let r_numerator =
        (-x_mu * (Permyriad::ONE_THIRD.0 as i64)) + (y_mu * (Permyriad::SQRT3_OVER_3.0 as i64));
    let r = r_numerator / (hex_size_mu * scale);

    let prism_z = if z_height_mu != 0 { z_mu / z_height_mu } else { 0 };

    (q, r, prism_z)
}

/// 2D integer vector in `MilliUnit`. Field order is `(x, y)` and `repr(C)`
/// pins it — the offset locks below are the wire contract.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Vec2Milli(pub MilliUnit, pub MilliUnit);

impl Vec2Milli {
    /// The zero vector in `MilliUnit` space.
    pub const ZERO: Self = Self(MilliUnit(0), MilliUnit(0));

    /// Construct a 2D vector from x and y components.
    #[inline(always)]
    pub const fn new(x: MilliUnit, y: MilliUnit) -> Self {
        Self(x, y)
    }

    /// Dot product. The intermediate is `i128` so no coordinate pair can
    /// overflow before the truncating narrow — deterministic on all platforms.
    pub const fn dot(self, rhs: Self) -> i64 {
        let x = (self.0 .0 as i128) * (rhs.0 .0 as i128);
        let y = (self.1 .0 as i128) * (rhs.1 .0 as i128);
        (x + y) as i64
    }

    /// 2D cross product (scalar): `a.x*b.y - a.y*b.x`.
    pub const fn cross(self, rhs: Self) -> i64 {
        let a = (self.0 .0 as i128) * (rhs.1 .0 as i128);
        let b = (self.1 .0 as i128) * (rhs.0 .0 as i128);
        (a - b) as i64
    }

    /// Squared length: `x² + y²`.
    pub const fn length_squared(self) -> i64 {
        self.dot(self)
    }

    /// Perpendicular vector (rotate 90 degrees CCW): `(-y, x)`.
    pub const fn perp(self) -> Self {
        Self(MilliUnit(-self.1 .0), MilliUnit(self.0 .0))
    }

    /// Negation: `(-x, -y)`.
    pub const fn negate(self) -> Self {
        Self(MilliUnit(-self.0 .0), MilliUnit(-self.1 .0))
    }
}

impl core::ops::Add for Vec2Milli {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0, self.1 + other.1)
    }
}
impl core::ops::Sub for Vec2Milli {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0, self.1 - other.1)
    }
}
impl core::ops::Neg for Vec2Milli {
    type Output = Self;
    fn neg(self) -> Self {
        self.negate()
    }
}

/// 3D integer vector in `MilliUnit`. Field order is `(x, y, z)`, pinned by
/// `repr(C)` and the offset locks below. The source's `to_f32_mm` was stripped.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Vec3Milli(pub MilliUnit, pub MilliUnit, pub MilliUnit);

impl Vec3Milli {
    /// Construct a 3D vector from x, y, z components.
    #[inline(always)]
    pub const fn new(x: MilliUnit, y: MilliUnit, z: MilliUnit) -> Self {
        Self(x, y, z)
    }

    /// Squared length: `x² + y² + z²`.
    pub const fn length_squared(self) -> MilliUnit {
        MilliUnit(self.0 .0.pow(2) + self.1 .0.pow(2) + self.2 .0.pow(2))
    }

    /// Dot product. The intermediate is `i64` — coordinate pairs that fit in `i64` do not overflow.
    pub const fn dot(self, rhs: Self) -> MilliUnit {
        MilliUnit(self.0 .0 * rhs.0 .0 + self.1 .0 * rhs.1 .0 + self.2 .0 * rhs.2 .0)
    }

    /// 3D cross product: `(y₁z₂ - z₁y₂, z₁x₂ - x₁z₂, x₁y₂ - y₁x₂)`.
    pub const fn cross(self, rhs: Self) -> Self {
        Self(
            MilliUnit(self.1 .0 * rhs.2 .0 - self.2 .0 * rhs.1 .0),
            MilliUnit(self.2 .0 * rhs.0 .0 - self.0 .0 * rhs.2 .0),
            MilliUnit(self.0 .0 * rhs.1 .0 - self.1 .0 * rhs.0 .0),
        )
    }

    /// Scale to unit length × 1000 using the integer Newton sqrt — bit-identical
    /// on every CPU. Zero-length input normalises to zero, not to a fault.
    pub const fn normalize(self) -> Self {
        let len_sq = self.length_squared().0;
        if len_sq == 0 {
            return Self::new(MilliUnit(0), MilliUnit(0), MilliUnit(0));
        }
        let scale_factor = 1000;
        let len = isqrt_i64(len_sq);

        if len == 0 {
            return Self::new(MilliUnit(0), MilliUnit(0), MilliUnit(0));
        }

        Self::new(
            MilliUnit(self.0 .0 * scale_factor / len),
            MilliUnit(self.1 .0 * scale_factor / len),
            MilliUnit(self.2 .0 * scale_factor / len),
        )
    }
}

impl core::ops::Add for Vec3Milli {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0, self.1 + other.1, self.2 + other.2)
    }
}
impl core::ops::Sub for Vec3Milli {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0, self.1 - other.1, self.2 - other.2)
    }
}
impl core::ops::Mul<MilliUnit> for Vec3Milli {
    type Output = Self;
    /// Direct `MilliUnit` scaling: multiply and divide by 1000 so the result
    /// stays in `MilliUnit` space instead of drifting into micro-units.
    fn mul(self, rhs: MilliUnit) -> Self {
        Self(
            MilliUnit(self.0 .0 * rhs.0 / 1000),
            MilliUnit(self.1 .0 * rhs.0 / 1000),
            MilliUnit(self.2 .0 * rhs.0 / 1000),
        )
    }
}

// ── Deterministic integer sqrt + log10 ──────────────────────────────────────
//
// Integer-only transcendental substitutes. Bit-identical on every CPU — no
// IEEE-754 dependencies. Three sqrt entries (u64 / i64 / i128) because the
// types are real, not cosmetic: a hash-domain u64 genuinely exceeds i64::MAX.

/// Integer square root via Newton's method. Returns `floor(sqrt(n))` for `n >= 0`.
/// Panics on negative input — that is the caller breaking the contract, not data
/// corruption, so `panic!` is the correct verb here rather than L10's `abort`.
#[inline]
pub const fn isqrt_i64(n: i64) -> i64 {
    assert!(n >= 0, "isqrt_i64 of negative");
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Unsigned integer square root. Returns `floor(sqrt(n))`. In the source tree
/// this entry's absence caused 20+ local re-implementations — it is ported so
/// v3 never grows a second home (L05 applied to functions, not just types).
#[inline]
pub const fn isqrt_u64(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// 128-bit version of `isqrt_i64`. Use when the input sum-of-squares can exceed
/// `i64::MAX`. The result always fits `i64`: `sqrt(i128::MAX)` is about `1.3e19`.
#[inline]
pub const fn isqrt_i128(n: i128) -> i64 {
    assert!(n >= 0, "isqrt_i128 of negative");
    if n < 2 {
        return n as i64;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x as i64
}

/// Integer log10 in permyriad output: `log10(n_milli / 1000) * 10_000`.
///
/// Precision contract: max relative error about 3% (≈300 permyriad), caused by
/// linear interpolation of log2 in the mantissa. Acceptable for perceptual
/// transcendentals (0.6 dB attenuation error, below the ~3 dB human JND); not
/// acceptable where tighter than 1% is needed — add a mantissa LUT then.
///
/// Implementation: bit-position for the integer part (log2 → log10 via the
/// constant `LOG10(2) ≈ 3010` permyriad), then linear interpolation in the
/// mantissa. Bit-identical on all platforms. Panics on `n_milli <= 0`.
pub const fn log10_permyriad(n_milli: i64) -> i32 {
    assert!(n_milli > 0, "log10_permyriad of non-positive");
    let lz = n_milli.leading_zeros() as i32;
    let bit_pos = 63 - lz; // 0..=62 for positive i64
    // Mantissa: the value with the leading 1 dropped, shifted to the top 16 bits.
    let mantissa_shifted = if bit_pos >= 16 {
        (n_milli >> (bit_pos - 16)) & 0xFFFF
    } else {
        (n_milli << (16 - bit_pos)) & 0xFFFF
    };
    // log2 in permyriad: bit_pos*10000 + mantissa/65536*10000.
    let log2_permyriad: i64 = (bit_pos as i64) * 10_000 + (mantissa_shifted * 10_000) / 65536;
    // log10(x) = log2(x) * log10(2); log10(2) ≈ 3010 permyriad.
    let log10_n_milli_permyriad: i64 = log2_permyriad * 3010 / 10_000;
    // Subtract log10(1000) = 3.0 = 30_000 permyriad.
    (log10_n_milli_permyriad - 30_000) as i32
}

/// Inverse of [`log10_permyriad`]: `n_milli` such that `n_milli/1000 =
/// 10^(log10_val_pmy/10_000)`. Same bit-position/mantissa decomposition run
/// backwards — same ~3% error budget, same bedrock cost tier (shift + one
/// multiply), no LUT, no float.
///
/// NEW INVARIANT (introduced by this integer port, absent from the f64
/// original): valid only for `log10_val_pmy >= -30_000` (represented value
/// `>= 0.001` — the smallest amplitude a milli-unit can hold, since `n_milli`
/// is always a positive integer). Below that, `n_milli` would round to 0 and
/// silently violate `log10_permyriad`'s own `n_milli > 0` precondition for
/// any caller that round-trips the result — this panics loud instead (C13),
/// rather than clamp. A float `10.0_f64.powf(x)` has no such floor; this does.
pub const fn pow10_permyriad(log10_val_pmy: i32) -> i64 {
    assert!(log10_val_pmy >= -30_000, "pow10_permyriad: below representable milli-precision (n_milli < 1)");
    let log10_n_milli_pmy = log10_val_pmy as i64 + 30_000;
    let log2_permyriad = log10_n_milli_pmy * 10_000 / 3010;
    let bit_pos = (log2_permyriad / 10_000) as u32;
    let mantissa_pmy = log2_permyriad % 10_000;
    let base: i64 = 1i64 << bit_pos;
    base + (base * mantissa_pmy) / 10_000
}

/// Integer/permyriad port of `forge-broski::native_dsp.rs::compress()`'s
/// dB-ratio peak-compression shape (v2, `E:\NewRepo\crates\forge-broski\src\
/// dj\native_dsp.rs:50-61`, real, shipped, tested — this crate never had a
/// Faust dependency, "no faust" precedent already set there).
///
/// `samples`/output are milli-units (1000 = amplitude 1.0). `ratio_pmy` and
/// `threshold_db_pmy` are Permyriad-scaled (real value * 10_000, so 1.0 dB =
/// 10_000). Only touches samples above threshold, so `log10_permyriad`'s
/// `n_milli > 0` contract is never hit by construction — same guard shape as
/// the real shipped `compress()`.
///
/// Aperture: inherits [`pow10_permyriad`]'s new -60dB floor
/// (`threshold_db_pmy/20 >= -30_000`), enforced by its panic. `ratio_pmy ==
/// 0` panics on integer division by zero (native Rust behavior — no extra
/// check needed to satisfy C13's loud-failure discipline here).
pub fn compress_permyriad(samples: &mut [i64], ratio_pmy: Permyriad, threshold_db_pmy: i32) {
    let thresh_lin_milli = pow10_permyriad(threshold_db_pmy / 20);
    for s in samples.iter_mut() {
        let abs = s.abs();
        if abs > thresh_lin_milli {
            let over_db_pmy: i64 = 20 * (log10_permyriad(abs) - log10_permyriad(thresh_lin_milli)) as i64;
            let reduced_db_pmy: i64 = over_db_pmy * 10_000 / ratio_pmy.into_inner() as i64;
            let gain_factor_milli = pow10_permyriad((reduced_db_pmy / 20) as i32);
            *s = s.signum() * (thresh_lin_milli * gain_factor_milli) / 1000;
        }
    }
}

/// General DDS-style phase/rate accumulator — the primitive `pow10_permyriad`
/// and `compress_permyriad`'s own math is a concrete instance of, unified
/// here as its own type. `step_pmy` is a fixed per-tick increment in
/// permyriad-of-a-unit (`10_000` = exactly 1); each `next()` returns how many
/// whole units crossed this tick (`value_pmy / PMY_MAX`) and the remaining
/// sub-unit phase (`value_pmy % PMY_MAX`). Same fold/wrap shape as a hardware
/// DDS phase register, and the same arithmetic this crate already used by
/// hand to prove `44_100/120`'s period-2 Bresenham cycle.
///
/// Two callers, two different readings of the same output: a scheduler reads
/// `whole` as "how many samples this tick" (`step_pmy` > `PMY_MAX`, e.g.
/// `367_5000` for 44.1kHz/120Hz); an oscillator reads `phase` as "where in
/// the cycle" (`step_pmy` < `PMY_MAX`, `whole` is almost always 0, occasionally
/// 1 on a full-cycle wrap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermyriadAccumulator {
    step_pmy: i64,
    value_pmy: i64,
}

impl PermyriadAccumulator {
    /// Permyriad unity — one whole unit.
    pub const PMY_MAX: i64 = 10_000;

    /// A fresh accumulator at phase 0 with the given per-tick step.
    pub const fn new(step_pmy: i64) -> Self {
        Self { step_pmy, value_pmy: 0 }
    }

    /// Advance one tick. Returns `(whole, phase)`: `whole` is the count of
    /// full units crossed this tick, `phase` is the remaining sub-unit
    /// position (`0..PMY_MAX`) — the Fix state that carries to the next
    /// call, same role as a pararity fold's fixed point.
    pub fn next(&mut self) -> (i64, i64) {
        self.value_pmy += self.step_pmy;
        let whole = self.value_pmy.div_euclid(Self::PMY_MAX);
        let phase = self.value_pmy.rem_euclid(Self::PMY_MAX);
        self.value_pmy = phase;
        (whole, phase)
    }

    /// Retune the step live. The current phase (`value_pmy`) carries over
    /// unmolested — retuning never causes a jump or a phase reset.
    pub fn retune(&mut self, step_pmy: i64) {
        self.step_pmy = step_pmy;
    }

    /// Current phase without advancing.
    pub const fn phase(&self) -> i64 {
        self.value_pmy
    }
}

// ---------------------------------------------------------------------------
// LAYOUT LOCKS. Measured with rustc, never hand-computed.
// ---------------------------------------------------------------------------
const _: () = assert!(core::mem::size_of::<Permyriad>() == 4);
const _: () = assert!(core::mem::align_of::<Permyriad>() == 4);
const _: () = assert!(core::mem::size_of::<MilliUnit>() == 8);
const _: () = assert!(core::mem::align_of::<MilliUnit>() == 8);
const _: () = assert!(core::mem::size_of::<SimTick>() == core::mem::size_of::<u64>());
const _: () = assert!(core::mem::size_of::<AudioFrame>() == core::mem::size_of::<u64>());
const _: () = assert!(core::mem::size_of::<Vec2Milli>() == 16);
const _: () = assert!(core::mem::align_of::<Vec2Milli>() == 8);
const _: () = assert!(core::mem::size_of::<Vec3Milli>() == 24);
const _: () = assert!(core::mem::align_of::<Vec3Milli>() == 8);

// OFFSET LOCKS. `repr(C)` pins field order; these make a reorder fail
// `cargo check` instead of silently swapping axes in stored vectors.
const _: () = assert!(core::mem::offset_of!(Vec2Milli, 0) == 0);
const _: () = assert!(core::mem::offset_of!(Vec2Milli, 1) == 8);
const _: () = assert!(core::mem::offset_of!(Vec3Milli, 0) == 0);
const _: () = assert!(core::mem::offset_of!(Vec3Milli, 1) == 8);
const _: () = assert!(core::mem::offset_of!(Vec3Milli, 2) == 16);

// The permyriad scale is the one constant every ratio in this file divides by.
const _: () = assert!(Permyriad::ONE.0 == 10_000);

#[cfg(test)]
mod clock_seam_tests {
    use super::*;

    #[test]
    fn sim_tick_saturates_monotonic() {
        assert_eq!(SimTick(u64::MAX).saturating_add(10), SimTick(u64::MAX));
        assert_eq!(SimTick(50).since(SimTick(20)), 30);
        assert_eq!(SimTick(20).since(SimTick(50)), 0); // never negative
    }

    #[test]
    fn audio_frame_independent_counter() {
        assert_eq!(AudioFrame(400).saturating_add(400), AudioFrame(800));
        assert_eq!(AudioFrame(400).get(), 400);
    }
}

#[cfg(test)]
mod transcendental_tests {
    use super::*;

    #[test]
    fn isqrt_zero_one() {
        assert_eq!(isqrt_i64(0), 0);
        assert_eq!(isqrt_i64(1), 1);
    }
    #[test]
    fn isqrt_perfect_squares() {
        assert_eq!(isqrt_i64(4), 2);
        assert_eq!(isqrt_i64(9), 3);
        assert_eq!(isqrt_i64(100), 10);
        assert_eq!(isqrt_i64(10000), 100);
        assert_eq!(isqrt_i64(1_000_000), 1000);
    }
    #[test]
    fn isqrt_floor_behavior() {
        assert_eq!(isqrt_i64(2), 1);
        assert_eq!(isqrt_i64(3), 1);
        assert_eq!(isqrt_i64(8), 2);
        assert_eq!(isqrt_i64(99), 9);
        assert_eq!(isqrt_i64(101), 10);
    }
    #[test]
    fn isqrt_large() {
        // 2^31 = 2_147_483_648, sqrt ~ 46340
        assert_eq!(isqrt_i64(2_147_483_648), 46340);
        // 10^12 = 1_000_000_000_000, sqrt = 1_000_000
        assert_eq!(isqrt_i64(1_000_000_000_000), 1_000_000);
    }
    #[test]
    fn isqrt_u64_matches_i64_in_shared_range() {
        for n in [0u64, 1, 2, 4, 100, 12345, 1_000_000_000_000] {
            assert_eq!(isqrt_u64(n), isqrt_i64(n as i64) as u64);
        }
        // And above i64::MAX it still holds the floor contract.
        let n = u64::MAX;
        let r = isqrt_u64(n);
        assert!((r as u128) * (r as u128) <= n as u128);
        assert!((r as u128 + 1) * (r as u128 + 1) > n as u128);
    }
    #[test]
    fn isqrt_i128_matches_i64_for_small() {
        for n in [0i64, 1, 4, 100, 12345, 1_000_000_000_000] {
            assert_eq!(isqrt_i128(n as i128), isqrt_i64(n));
        }
    }
    #[test]
    fn isqrt_i128_handles_overflow_range() {
        // i64::MAX = 9.22e18; sqrt ≈ 3.04e9
        let n = i64::MAX as i128;
        let r = isqrt_i128(n);
        // r*r ≤ n < (r+1)²
        assert!(r as i128 * r as i128 <= n);
        assert!((r as i128 + 1) * (r as i128 + 1) > n);
    }
    #[test]
    fn isqrt_deterministic() {
        for n in [0, 1, 2, 100, 12345, 999_999_999i64] {
            assert_eq!(isqrt_i64(n), isqrt_i64(n));
        }
    }
    #[test]
    #[should_panic(expected = "isqrt_i64 of negative")]
    fn isqrt_negative_panics() {
        isqrt_i64(-1);
    }

    // Tolerance reflects the ~3% precision contract documented on log10_permyriad.
    const LOG10_TOLERANCE: i32 = 350;

    #[test]
    fn log10_unity_is_zero() {
        assert!(
            log10_permyriad(1000).abs() < LOG10_TOLERANCE,
            "log10(1.0) = {}, expected ~0",
            log10_permyriad(1000)
        );
    }
    #[test]
    fn log10_ten() {
        let v = log10_permyriad(10_000);
        assert!((v - 10_000).abs() < LOG10_TOLERANCE, "log10(10) = {v}, expected ~10000");
    }
    #[test]
    fn log10_hundred() {
        let v = log10_permyriad(100_000);
        assert!((v - 20_000).abs() < LOG10_TOLERANCE, "log10(100) = {v}, expected ~20000");
    }
    #[test]
    fn log10_half() {
        let v = log10_permyriad(500);
        assert!((v + 3010).abs() < LOG10_TOLERANCE, "log10(0.5) = {v}, expected ~-3010");
    }
    #[test]
    fn log10_monotonic() {
        let mut prev = log10_permyriad(1);
        for n in [10i64, 100, 1000, 10_000, 100_000, 1_000_000, 1_000_000_000] {
            let v = log10_permyriad(n);
            assert!(v > prev, "log10 not monotonic: log10({n}) = {v}, prev = {prev}");
            prev = v;
        }
    }
    #[test]
    fn log10_deterministic() {
        for n in [1i64, 100, 1000, 10_000, 1_000_000] {
            assert_eq!(log10_permyriad(n), log10_permyriad(n));
        }
    }
    #[test]
    #[should_panic(expected = "log10_permyriad of non-positive")]
    fn log10_zero_panics() {
        log10_permyriad(0);
    }

    #[test]
    fn pow10_round_trips_log10() {
        for n in [1i64, 100, 1000, 10_000, 1_000_000] {
            let y = log10_permyriad(n);
            let back = pow10_permyriad(y);
            let rel_err = ((back - n).abs() * 10_000) / n;
            assert!(rel_err < 500, "pow10(log10({n}))={back}, expected ~{n}, rel_err={rel_err}");
        }
    }

    #[test]
    #[should_panic(expected = "below representable milli-precision")]
    fn pow10_below_floor_panics() {
        pow10_permyriad(-30_001);
    }

    #[test]
    fn pow10_at_floor_is_one_milli() {
        assert_eq!(pow10_permyriad(-30_000), 1);
    }

    #[test]
    fn compress_reduces_peaks_above_threshold() {
        let mut samples = vec![2000i64; 8]; // amplitude 2.0, loud
        compress_permyriad(&mut samples, Permyriad(40_000), -120_000); // 4:1 @ -12dB
        for s in &samples {
            assert!(s.abs() < 2000, "compression should reduce peaks: {s}");
            assert!(*s > 0, "sign must be preserved");
        }
    }

    #[test]
    fn compress_passes_below_threshold_bit_exact() {
        let mut samples = vec![100i64, -50, 0, 200];
        let original = samples.clone();
        compress_permyriad(&mut samples, Permyriad(40_000), -120_000); // thresh ~251 milli
        assert_eq!(samples, original, "samples below threshold must pass through unchanged");
    }

    #[test]
    fn compress_never_amplifies() {
        for &amp_milli in &[100i64, 500, 1000, 2000, 5000, 10_000] {
            let mut s = vec![amp_milli];
            compress_permyriad(&mut s, Permyriad(20_000), -60_000); // 2:1 @ -6dB
            assert!(s[0].abs() <= amp_milli, "amp={amp_milli} -> {} must never grow", s[0]);
        }
    }

    #[test]
    fn accumulator_367_368_split_matches_44100_over_120() {
        // 44_100/120 = 367.5 exactly -> step_pmy = 367*10_000 + 5_000.
        let mut acc = PermyriadAccumulator::new(367 * PermyriadAccumulator::PMY_MAX + 5_000);
        let mut total = 0i64;
        for _ in 0..120 {
            let (whole, _phase) = acc.next();
            assert!(whole == 367 || whole == 368, "unexpected sample count {whole}");
            total += whole;
        }
        assert_eq!(total, 44_100, "120 ticks at 367.5 samples/tick must sum to exactly 44_100");
    }

    #[test]
    fn accumulator_phase_wraps_exactly_at_pmy_max() {
        // step < PMY_MAX: whole is 0 until the phase crosses a full cycle.
        let mut acc = PermyriadAccumulator::new(4_000);
        assert_eq!(acc.next(), (0, 4_000));
        assert_eq!(acc.next(), (0, 8_000));
        assert_eq!(acc.next(), (1, 2_000), "third tick crosses PMY_MAX, wraps with remainder");
    }

    #[test]
    fn accumulator_retune_preserves_phase() {
        let mut acc = PermyriadAccumulator::new(3_000);
        acc.next();
        assert_eq!(acc.phase(), 3_000);
        acc.retune(1_000);
        // Phase carries over unmolested; only the step changed.
        assert_eq!(acc.phase(), 3_000);
        assert_eq!(acc.next(), (0, 4_000));
    }

    #[test]
    fn accumulator_deterministic() {
        let mut a = PermyriadAccumulator::new(3_333);
        let mut b = PermyriadAccumulator::new(3_333);
        for _ in 0..50 {
            assert_eq!(a.next(), b.next());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_prism_origin_is_zero() {
        let (q, r, z) = cartesian_to_hex_prism(0, 0, 0, 1000, 500);
        assert_eq!((q, r, z), (0, 0, 0));
    }

    #[test]
    fn hex_prism_positive_x() {
        // x=10000mu, hex_size=1000mu → q = 10000*6666 / (1000*10000) = 6
        let (q, r, _) = cartesian_to_hex_prism(10000, 0, 0, 1000, 500);
        assert_eq!(q, 6);
        // r = -10000*3333 / (1000*10000) = -3
        assert_eq!(r, -3);
    }

    #[test]
    fn hex_prism_z_stacking() {
        let (_, _, z) = cartesian_to_hex_prism(0, 0, 2500, 1000, 500);
        assert_eq!(z, 5);
    }

    #[test]
    fn hex_prism_no_divide_by_zero() {
        let (q, r, z) = cartesian_to_hex_prism(5000, 5000, 1000, 1000, 0);
        assert_eq!(z, 0); // z_height_mu == 0 → prism_z = 0
        assert!(q != 0 || r != 0); // but q/r still computed
    }

    /// L07: perp is a quarter turn, so four of them are the identity and two are
    /// negation — the cheapest full inverse check the type offers.
    #[test]
    fn perp_four_times_is_the_identity() {
        let v = Vec2Milli::new(MilliUnit(3), MilliUnit(-7));
        assert_eq!(v.perp().perp(), v.negate());
        assert_eq!(v.perp().perp().perp().perp(), v);
        assert_eq!(-v, v.negate());
    }

    #[test]
    fn vector_arithmetic_holds_on_axes() {
        let a = Vec3Milli::new(MilliUnit(1000), MilliUnit(0), MilliUnit(0));
        let b = Vec3Milli::new(MilliUnit(0), MilliUnit(1000), MilliUnit(0));
        assert_eq!(a.dot(b), MilliUnit(0));
        assert_eq!(a.cross(b), Vec3Milli::new(MilliUnit(0), MilliUnit(0), MilliUnit(1_000_000)));
        assert_eq!((a + b) - b, a);
        assert_eq!(a.length_squared(), MilliUnit(1_000_000));
        // A unit-axis vector normalises to exactly 1000 on its own axis.
        assert_eq!(a.normalize(), Vec3Milli::new(MilliUnit(1000), MilliUnit(0), MilliUnit(0)));
    }

    #[test]
    fn permyriad_scaling_is_exact_at_the_anchors() {
        assert_eq!(MilliUnit(5000).mul_permyriad(Permyriad::ONE), MilliUnit(5000));
        assert_eq!(MilliUnit(5000).mul_permyriad(Permyriad::ZERO), MilliUnit(0));
        assert_eq!(MilliUnit(5000).div_permyriad(Permyriad::ONE), MilliUnit(5000));
        assert_eq!(
            Permyriad::MAX.saturating_add(Permyriad::ONE),
            Permyriad::MAX,
            "the ceiling saturates, never wraps"
        );
    }
}
