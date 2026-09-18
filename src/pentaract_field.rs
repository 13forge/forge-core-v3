//! The 32-channel sensory field sampled at one [`Pentaract`] — the S⁴
//! mood-field point. A `Pentaract` is the POINT; this is the field value there.
//!
//! [`CONSTRAINT BOUNDARY`] Additive and self-contained, per `pentaract.rs`'s own
//! charter. [`Pentaract`]'s `repr(C)` layout is NOT touched: its offset locks are
//! the contract that `forge-ocular-v3`'s GPU/CPU parity lane rests on, its
//! `accent` is already an RRGGBBAA swatch, and its `_pad` is documented "never a
//! payload channel". 32 `i32` channels is 128 bytes and lives out here instead.
//!
//! Integer-only by law (CLAUDE.md L08 machine-first): gains are permyriad
//! `i16`, never `f32` — the loom that reads this field must weave the same
//! bytes on every machine.

use core::ops::{Add, AddAssign, Index, IndexMut};

use crate::pentaract::{Pentaract, TRUTH_CREATIVE};

/// Channels per field. One `u32` mask bit each, exactly.
pub const SENSE_COUNT: usize = 32;

/// Permyriad unity gain: `10_000` reads a channel at 1.0x.
pub const GAIN_UNITY_Q: i16 = 10_000;

/// Bands per field. One polar axis each for the first three, the azimuth for
/// the fourth — see [`mood_point`].
pub const BAND_COUNT: usize = 4;

/// The four authored bands the 32 channels fall into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SenseGroup {
    /// Light and the rest of the spectrum a body can be built to see.
    Optical,
    /// Motion carried through rock, air and water.
    Kinetic,
    /// What is alive, what is rotting, and what a soul weighs.
    Anima,
    /// Magic, mind, and the seams between planes.
    Arcane,
}

impl SenseGroup {
    /// Every band, in the lane order the share vectors follow.
    pub const ALL: [Self; BAND_COUNT] = [Self::Optical, Self::Kinetic, Self::Anima, Self::Arcane];

    /// This band's lane in a share vector.
    pub const fn lane(self) -> usize {
        match self {
            Self::Optical => 0,
            Self::Kinetic => 1,
            Self::Anima => 2,
            Self::Arcane => 3,
        }
    }
}

/// Per-band permyriad shares of a total. All-zero in, all-zero out.
const fn share_of(sum: [i64; BAND_COUNT]) -> [i32; BAND_COUNT] {
    let total = sum[0] + sum[1] + sum[2] + sum[3];
    if total == 0 {
        return [0; BAND_COUNT];
    }
    let mut out = [0i32; BAND_COUNT];
    let mut i = 0;
    while i < BAND_COUNT {
        out[i] = (sum[i] * 10_000 / total) as i32;
        i += 1;
    }
    out
}

/// Quadrature rest angle. Every polar axis starts at `π/2` so a band holding
/// no share still leaves the axes below it alive — at `θ1 == 0` the
/// hyperspherical parametrization collapses `x2..x5` to zero.
const REST_POLAR: i32 = 32_768;
/// A polar axis swings a quarter turn across the full share range.
const POLAR_SWING: i32 = 16_384;
/// The azimuth swings a half turn — `0` and `10_000` share sit antipodal.
const AZIMUTH_SWING: i32 = 32_768;

/// The S⁴ point a band share sits at: optical, kinetic and anima tilt the
/// three polar axes off quadrature, arcane carries the azimuth. Two readings
/// with the same band shape land on the same point, so
/// [`Pentaract::cos_similarity`] between them scores how alike they are.
pub const fn mood_point(key: u64, share: [i32; BAND_COUNT]) -> Pentaract {
    const fn tilt(s: i32, swing: i32) -> u16 {
        let clamped = if s < 0 {
            0
        } else if s > 10_000 {
            10_000
        } else {
            s
        };
        (REST_POLAR + clamped * swing / 10_000) as u16
    }
    Pentaract::new(
        key,
        tilt(share[0], POLAR_SWING),
        tilt(share[1], POLAR_SWING),
        tilt(share[2], POLAR_SWING),
        tilt(share[3], AZIMUTH_SWING),
        0,
        TRUTH_CREATIVE,
    )
}

/// One perceptual channel. The reader form is a mask over these, never a
/// hardcoded branch — adding a 33rd channel is one variant plus one row.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SenseChannel {
    /// Radiated thermal energy from living bodies and thermal vents.
    HeatGradient = 0,
    /// High-frequency ambient radiation invisible to mortals in pitch dark.
    UvFlux = 1,
    /// Non-thermal monochrome acuity in total darkness.
    LuxZero = 2,
    /// Amplification threshold for ambient starlight and torchlight.
    LumensMultiplier = 3,
    /// Rejection index for illusion, polymorph and phantasm.
    GlamourPhase = 4,
    /// Light bending or air displacement around unseen entities.
    RefractionDelta = 5,
    /// Overlapping density of the Ethereal Plane.
    VeilDensity = 6,
    /// Permeability and depth of ambient shadow pools.
    ShadowDepth = 7,

    /// Low-frequency seismic shockwaves through ground and stone.
    VibrationHz = 8,
    /// High-frequency acoustic bounce-back off solid geometry.
    EchoDelay = 9,
    /// Structural load, hidden compartments, worked-stone age.
    MasonryStress = 10,
    /// Inherent alignment with absolute cardinal magnetic north.
    GeomagneticYaw = 11,
    /// Fluid pressure and movement inside roots, leaves and vines.
    SapVelocity = 12,
    /// Micro-pressure signalling storms, doors opening, air movement.
    AtmospherePa = 13,
    /// Wave displacement and pressure currents in liquid media.
    FluidDisplacement = 14,
    /// Disturbance in airborne dust, spores and suspended ash.
    ParticulateFlux = 15,

    /// Decay age and directional gradient of airborne chemical trails.
    ScentAge = 16,
    /// Iron and hemoglobin PPM in air or water across distance.
    FerrumPpm = 17,
    /// Positive energy aura emitted by living, breathing organisms.
    VitalityLux = 18,
    /// Cellular breakdown, disease load, proximity to death.
    NecroticDecay = 19,
    /// Mass and purity of a soul bound to its vessel.
    SoulMass = 20,
    /// Emotional state broadcast via sweat and pores.
    HormoneBias = 21,
    /// Biological load, parasitic infestation, active viral rot.
    PathogenCount = 22,
    /// Fungal network communication and spore-cloud saturation.
    SporeDensity = 23,

    /// Active spell resonances and school signatures lingering in a cell.
    WeaveFlux = 24,
    /// Concentration of raw, unshaped ley-line energy.
    ManaDensity = 25,
    /// Intentional hostility, target focus, anger directed at a unit.
    HateVector = 26,
    /// Metaphysical moral and ethical charge.
    EthosBias = 27,
    /// Synaptic firing and active conscious thought.
    NeuralHz = 28,
    /// Historical emotional trauma imprinted on inanimate surroundings.
    ResidualTrauma = 29,
    /// Spatial distortion from teleportation or interdimensional gates.
    PlanarTear = 30,
    /// Proximity to holy relics, consecration, deity intervention.
    PietyCharge = 31,
}

impl SenseChannel {
    /// Every channel, declaration order — the order the mask bits follow.
    pub const ALL: [Self; SENSE_COUNT] = [
        Self::HeatGradient, Self::UvFlux, Self::LuxZero, Self::LumensMultiplier,
        Self::GlamourPhase, Self::RefractionDelta, Self::VeilDensity, Self::ShadowDepth,
        Self::VibrationHz, Self::EchoDelay, Self::MasonryStress, Self::GeomagneticYaw,
        Self::SapVelocity, Self::AtmospherePa, Self::FluidDisplacement, Self::ParticulateFlux,
        Self::ScentAge, Self::FerrumPpm, Self::VitalityLux, Self::NecroticDecay,
        Self::SoulMass, Self::HormoneBias, Self::PathogenCount, Self::SporeDensity,
        Self::WeaveFlux, Self::ManaDensity, Self::HateVector, Self::EthosBias,
        Self::NeuralHz, Self::ResidualTrauma, Self::PlanarTear, Self::PietyCharge,
    ];

    /// The authored `VoxelState`-era field name, kept so the design table and
    /// the code answer to the same string.
    pub const fn field(self) -> &'static str {
        match self {
            Self::HeatGradient => "heat_gradient_q",
            Self::UvFlux => "uv_flux_q",
            Self::LuxZero => "lux_zero_q",
            Self::LumensMultiplier => "lumens_multiplier_q",
            Self::GlamourPhase => "glamour_phase_q",
            Self::RefractionDelta => "refraction_delta_q",
            Self::VeilDensity => "veil_density_q",
            Self::ShadowDepth => "shadow_depth_q",
            Self::VibrationHz => "vibration_hz_q",
            Self::EchoDelay => "echo_delay_t",
            Self::MasonryStress => "masonry_stress_q",
            Self::GeomagneticYaw => "geomagnetic_yaw_q",
            Self::SapVelocity => "sap_velocity_q",
            Self::AtmospherePa => "atmosphere_pa_q",
            Self::FluidDisplacement => "fluid_displacement_q",
            Self::ParticulateFlux => "particulate_flux_q",
            Self::ScentAge => "scent_age_t",
            Self::FerrumPpm => "ferrum_ppm_q",
            Self::VitalityLux => "vitality_lux_q",
            Self::NecroticDecay => "necrotic_decay_q",
            Self::SoulMass => "soul_mass_q",
            Self::HormoneBias => "hormone_bias_q",
            Self::PathogenCount => "pathogen_count_q",
            Self::SporeDensity => "spore_density_q",
            Self::WeaveFlux => "weave_flux_q",
            Self::ManaDensity => "mana_density_q",
            Self::HateVector => "hate_vector_q",
            Self::EthosBias => "ethos_bias_q",
            Self::NeuralHz => "neural_hz_q",
            Self::ResidualTrauma => "residual_trauma_q",
            Self::PlanarTear => "planar_tear_q",
            Self::PietyCharge => "piety_charge_q",
        }
    }

    /// The tabletop lineage the channel was drawn from.
    pub const fn lore(self) -> &'static str {
        match self {
            Self::HeatGradient => "Infravision (EQ / D&D 3.5)",
            Self::UvFlux => "Ultravision (EQ)",
            Self::LuxZero => "Darkvision (Golarion / BG)",
            Self::LumensMultiplier => "Low-Light Vision (Golarion / BG)",
            Self::GlamourPhase => "Truesight (Golarion / BG)",
            Self::RefractionDelta => "See Invisibility (Golarion / BG / EQ)",
            Self::VeilDensity => "Ethereal Sight (Golarion / BG)",
            Self::ShadowDepth => "Gloomvision (Golarion)",
            Self::VibrationHz => "Tremorsense (Golarion / BG)",
            Self::EchoDelay => "Echolocation (Golarion)",
            Self::MasonryStress => "Stonecunning (Golarion / BG)",
            Self::GeomagneticYaw => "Sense Heading (EQ)",
            Self::SapVelocity => "Greensight (Golarion)",
            Self::AtmospherePa => "Barometric Sense (MUD / Golarion)",
            Self::FluidDisplacement => "Hydro-Sense (Golarion)",
            Self::ParticulateFlux => "Dust-Reading (EQ / Golarion)",
            Self::ScentAge => "Scent Tracking (Golarion / BG)",
            Self::FerrumPpm => "Blood Scent (Golarion / BG)",
            Self::VitalityLux => "Lifesense (Golarion)",
            Self::NecroticDecay => "Deathwatch (Golarion / BG)",
            Self::SoulMass => "Anima Sight (Golarion)",
            Self::HormoneBias => "Pheromone Reading (Golarion)",
            Self::PathogenCount => "Blight Sense (Golarion / EQ)",
            Self::SporeDensity => "Mycelial Sense (Golarion)",
            Self::WeaveFlux => "Detect Magic (Golarion / BG)",
            Self::ManaDensity => "Mana Sense (EQ)",
            Self::HateVector => "Aggro / Threat Perception (EQ)",
            Self::EthosBias => "Detect Evil / Good (Golarion / BG)",
            Self::NeuralHz => "Thoughtsense / Mindsight (BG / Golarion)",
            Self::ResidualTrauma => "Psychometry (Golarion Occult)",
            Self::PlanarTear => "Portal Sense (Golarion / BG)",
            Self::PietyCharge => "Divine Grace (Golarion / EQ)",
        }
    }

    /// Which authored band the channel sits in.
    pub const fn group(self) -> SenseGroup {
        match self as u8 {
            0..=7 => SenseGroup::Optical,
            8..=15 => SenseGroup::Kinetic,
            16..=23 => SenseGroup::Anima,
            _ => SenseGroup::Arcane,
        }
    }

    /// This channel's single mask bit.
    pub const fn bit(self) -> u32 {
        1u32 << (self as u32)
    }
}

/// A set of channels a body is built to hear. A newtype, not a raw `u32`, so
/// no call site does its own bit arithmetic — and taking [`SenseChannel`]
/// rather than an index means there is no out-of-range case to assert on
/// (CLAUDE.md L10 bans panic; this makes the bad state unrepresentable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SenseMask(u32);

impl SenseMask {
    /// Hears nothing.
    pub const EMPTY: Self = Self(0);
    /// Hears everything — the wraith's shape, not a body's.
    pub const ALL: Self = Self(u32::MAX);

    /// The mask of exactly one channel.
    pub const fn of(c: SenseChannel) -> Self {
        Self(c.bit())
    }

    /// Both masks together.
    pub const fn union(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }

    /// This mask plus one more channel.
    pub const fn with(self, c: SenseChannel) -> Self {
        Self(self.0 | c.bit())
    }

    /// Whether the channel is open to this body.
    pub const fn listens_to(self, c: SenseChannel) -> bool {
        self.0 & c.bit() != 0
    }

    /// How many channels are open.
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// The raw bits, for wire encoding only.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Permyriad share of the open channels sitting in each band — the shape
    /// of what this body is built to hear, comparable to a cell's own shape.
    pub const fn band_share(self) -> [i32; BAND_COUNT] {
        let mut sum = [0i64; BAND_COUNT];
        let mut i = 0;
        while i < SENSE_COUNT {
            let c = SenseChannel::ALL[i];
            if self.listens_to(c) {
                sum[c.group().lane()] += 1;
            }
            i += 1;
        }
        share_of(sum)
    }
}

/// The 32-channel reading at one mood-field point.
///
/// `q` is private: [`SenseChannel`] is the only key, so reordering or adding a
/// channel cannot silently repoint a caller the way a raw offset would.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PentaractField {
    /// Where on S⁴ this reading was taken.
    pub at: Pentaract,
    q: [i32; SENSE_COUNT],
}

impl PentaractField {
    /// An all-quiet field at a point — every channel zero.
    pub const fn quiet_at(at: Pentaract) -> Self {
        Self { at, q: [0; SENSE_COUNT] }
    }

    /// Build from a full channel vector.
    pub const fn new(at: Pentaract, q: [i32; SENSE_COUNT]) -> Self {
        Self { at, q }
    }

    /// The whole channel vector, read-only.
    pub const fn channels(&self) -> &[i32; SENSE_COUNT] {
        &self.q
    }

    /// Permyriad share of this reading's total magnitude sitting in each band
    /// — the shape of what the cell is doing, sign discarded.
    pub const fn band_share(&self) -> [i32; BAND_COUNT] {
        let mut sum = [0i64; BAND_COUNT];
        let mut i = 0;
        while i < SENSE_COUNT {
            sum[SenseChannel::ALL[i].group().lane()] += (self.q[i] as i64).abs();
            i += 1;
        }
        share_of(sum)
    }

    /// This reading re-pointed at the S⁴ point its own band shape sits at.
    pub const fn oriented(self, key: u64) -> Self {
        Self { at: mood_point(key, self.band_share()), q: self.q }
    }

    /// The channel's value, or `None` when this body cannot hear it. Isolation
    /// is enforced here by the type — no branch reaches outside its own sense.
    pub const fn read_masked(&self, mask: SenseMask, c: SenseChannel) -> Option<i32> {
        if mask.listens_to(c) {
            Some(self.q[c as usize])
        } else {
            None
        }
    }
}

impl Index<SenseChannel> for PentaractField {
    type Output = i32;
    #[inline]
    fn index(&self, c: SenseChannel) -> &i32 {
        &self.q[c as usize]
    }
}

impl IndexMut<SenseChannel> for PentaractField {
    #[inline]
    fn index_mut(&mut self, c: SenseChannel) -> &mut i32 {
        &mut self.q[c as usize]
    }
}

/// Stacking readings is per-lane saturating addition — a room debuff plus a
/// stance modifier plus a dialogue delta is `.sum()`, never a bespoke fold.
/// The left point is kept: you stack ONTO where you are standing.
impl Add for PentaractField {
    type Output = Self;
    fn add(mut self, rhs: Self) -> Self {
        self += rhs;
        self
    }
}

impl AddAssign for PentaractField {
    fn add_assign(&mut self, rhs: Self) {
        let mut i = 0;
        while i < SENSE_COUNT {
            self.q[i] = self.q[i].saturating_add(rhs.q[i]);
            i += 1;
        }
    }
}

/// Infer trit (-1, 0, +1) from a permyriad value with default threshold ±3000.
/// Threshold-based: q > +3k → +1 (aligned), q < -3k → -1 (corrupted), else 0 (neutral).
/// Per-channel meanings: see CHANNEL_SEMANTICS.md. Channels may override thresholds per their semantics.
pub const fn trit_from_permyriad(q: i32) -> i8 {
    const THRESHOLD: i32 = 3_000;
    if q > THRESHOLD {
        1
    } else if q < -THRESHOLD {
        -1
    } else {
        0
    }
}

/// Custom threshold trit inference. Useful for channels with different sensitivity.
pub const fn trit_from_permyriad_threshold(q: i32, threshold: i32) -> i8 {
    if q > threshold {
        1
    } else if q < -threshold {
        -1
    } else {
        0
    }
}

/// Per-channel gains a body applies to what it hears, in permyriad. Integer by
/// law: float gain would make the woven room depend on FMA order and target ISA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SenseGain(pub [i16; SENSE_COUNT]);

impl SenseGain {
    /// Every channel at 1.0x.
    pub const UNITY: Self = Self([GAIN_UNITY_Q; SENSE_COUNT]);

    /// Every channel at one gain read off an angular similarity: aligned
    /// (`32767`) hears at unity, orthogonal at half, antipodal deaf. This is
    /// the whole of a body's sensitivity — no per-channel table is authored.
    pub const fn attuned(cos_q: i32) -> Self {
        let half = GAIN_UNITY_Q as i32 / 2;
        let c = if cos_q > 32_767 {
            32_767
        } else if cos_q < -32_767 {
            -32_767
        } else {
            cos_q
        };
        Self([(half + c * half / 32_767) as i16; SENSE_COUNT])
    }

    /// Apply this channel's gain to a raw reading.
    pub const fn apply(&self, c: SenseChannel, raw: i32) -> i32 {
        (raw as i64 * self.0[c as usize] as i64 / GAIN_UNITY_Q as i64) as i32
    }
}

impl Index<SenseChannel> for SenseGain {
    type Output = i16;
    #[inline]
    fn index(&self, c: SenseChannel) -> &i16 {
        &self.0[c as usize]
    }
}

impl IndexMut<SenseChannel> for SenseGain {
    #[inline]
    fn index_mut(&mut self, c: SenseChannel) -> &mut i16 {
        &mut self.0[c as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point() -> Pentaract {
        Pentaract::new(0x13, 1000, 2000, 3000, 4000, 0xC3A256FF, 0)
    }

    /// Adding a channel means adding a row, and the tables must not fall behind.
    #[test]
    fn all_thirty_two_channels_carry_their_authored_data() {
        assert_eq!(SenseChannel::ALL.len(), SENSE_COUNT);
        for (i, c) in SenseChannel::ALL.iter().enumerate() {
            assert_eq!(*c as usize, i, "declaration order is the bit order");
            assert!(!c.field().is_empty());
            assert!(!c.lore().is_empty());
            let f = c.field();
            assert!(
                f.ends_with("_q") || f.ends_with("_t"),
                "{f} must carry its unit suffix"
            );
        }
        for (i, a) in SenseChannel::ALL.iter().enumerate() {
            for b in &SenseChannel::ALL[i + 1..] {
                assert_ne!(a.field(), b.field(), "two channels share a field name");
            }
        }
    }

    /// Eight per band, four bands.
    #[test]
    fn the_four_bands_hold_eight_each() {
        for g in [SenseGroup::Optical, SenseGroup::Kinetic, SenseGroup::Anima, SenseGroup::Arcane] {
            let n = SenseChannel::ALL.iter().filter(|c| c.group() == g).count();
            assert_eq!(n, 8, "{g:?} must hold eight channels");
        }
    }

    /// L07: one channel, one bit, no collisions and none spilled out of u32.
    #[test]
    fn every_channel_owns_exactly_one_distinct_bit() {
        let mut seen = 0u32;
        for c in SenseChannel::ALL {
            let b = c.bit();
            assert_eq!(b.count_ones(), 1, "{c:?} is not a single bit");
            assert_eq!(seen & b, 0, "{c:?} collides with an earlier channel");
            seen |= b;
        }
        assert_eq!(seen, u32::MAX, "the 32 channels must tile the whole mask");
        assert_eq!(SenseMask::ALL.count(), SENSE_COUNT as u32);
        assert_eq!(SenseMask::EMPTY.count(), 0);
    }

    /// A masked-out channel is unreadable whatever the field holds — the
    /// isolation umwelt_loom states in prose, now held by the type.
    #[test]
    fn a_masked_out_channel_cannot_be_read_at_any_value() {
        let mut f = PentaractField::quiet_at(point());
        for c in SenseChannel::ALL {
            f[c] = 9_999;
        }
        let ears = SenseMask::of(SenseChannel::ScentAge).with(SenseChannel::MasonryStress);
        assert_eq!(f.read_masked(ears, SenseChannel::ScentAge), Some(9_999));
        assert_eq!(f.read_masked(ears, SenseChannel::MasonryStress), Some(9_999));
        for c in SenseChannel::ALL {
            if !ears.listens_to(c) {
                assert_eq!(f.read_masked(ears, c), None, "{c:?} leaked through the mask");
            }
        }
    }

    /// The enum is the only key: writing through it reads back through it.
    #[test]
    fn typed_indexing_round_trips() {
        let mut f = PentaractField::quiet_at(point());
        f[SenseChannel::PietyCharge] = 4_200;
        assert_eq!(f[SenseChannel::PietyCharge], 4_200);
        assert_eq!(f.channels()[SenseChannel::PietyCharge as usize], 4_200);
        assert_eq!(f[SenseChannel::HeatGradient], 0);
    }

    /// Unity gain is identity; half gain halves.
    #[test]
    fn unity_gain_is_identity() {
        let g = SenseGain::UNITY;
        for c in SenseChannel::ALL {
            assert_eq!(g.apply(c, 7_777), 7_777);
            assert_eq!(g.apply(c, -1_234), -1_234);
        }
        let mut half = SenseGain::UNITY;
        half[SenseChannel::EchoDelay] = GAIN_UNITY_Q / 2;
        assert_eq!(half.apply(SenseChannel::EchoDelay, 1_000), 500);
        assert_eq!(half.apply(SenseChannel::UvFlux, 1_000), 1_000);
    }

    /// Stacking is a monoid: per-lane, saturating, and order does not matter.
    #[test]
    fn readings_stack_per_lane_and_saturate() {
        let mut a = PentaractField::quiet_at(point());
        let mut b = PentaractField::quiet_at(point());
        a[SenseChannel::HateVector] = 300;
        b[SenseChannel::HateVector] = 45;
        b[SenseChannel::SoulMass] = 7;
        assert_eq!((a + b)[SenseChannel::HateVector], 345);
        assert_eq!((b + a)[SenseChannel::HateVector], 345, "order-independent");
        assert_eq!((a + b)[SenseChannel::SoulMass], 7);

        let mut hi = PentaractField::quiet_at(point());
        hi[SenseChannel::ManaDensity] = i32::MAX;
        let mut more = PentaractField::quiet_at(point());
        more[SenseChannel::ManaDensity] = 1;
        assert_eq!((hi + more)[SenseChannel::ManaDensity], i32::MAX, "saturates, never wraps");
    }

    /// The gain a body brings is one number off the angle, and it spans the
    /// whole range: aligned unity, orthogonal half, antipodal deaf.
    #[test]
    fn attuned_gain_tracks_similarity() {
        assert_eq!(SenseGain::attuned(32_767).0[0], GAIN_UNITY_Q);
        assert_eq!(SenseGain::attuned(0).0[0], GAIN_UNITY_Q / 2);
        assert_eq!(SenseGain::attuned(-32_767).0[0], 0);
        assert_eq!(SenseGain::attuned(999_999).0[0], GAIN_UNITY_Q, "clamps above");
        assert_eq!(SenseGain::attuned(-999_999).0[0], 0, "clamps below");
        let mut prev = i16::MIN;
        for cos in (-32_767..=32_767).step_by(1_024) {
            let g = SenseGain::attuned(cos).0[0];
            assert!(g >= prev, "gain must not fall as alignment rises");
            prev = g;
        }
        for c in SenseChannel::ALL {
            assert_eq!(SenseGain::attuned(16_000)[c], SenseGain::attuned(16_000).0[0]);
        }
    }

    /// A share is the shape of a reading, not its scale.
    #[test]
    fn a_band_share_is_the_shape_not_the_scale() {
        let mut small = PentaractField::quiet_at(point());
        small[SenseChannel::NecroticDecay] = 40;
        small[SenseChannel::AtmospherePa] = 60;
        let mut large = PentaractField::quiet_at(point());
        large[SenseChannel::NecroticDecay] = 4_000;
        large[SenseChannel::AtmospherePa] = 6_000;
        assert_eq!(small.band_share(), large.band_share());
        assert_eq!(small.band_share(), [0, 6_000, 4_000, 0]);
        assert_eq!(PentaractField::quiet_at(point()).band_share(), [0; BAND_COUNT]);

        let mut negative = PentaractField::quiet_at(point());
        negative[SenseChannel::NecroticDecay] = -4_000;
        negative[SenseChannel::AtmospherePa] = 6_000;
        assert_eq!(negative.band_share(), large.band_share(), "sign is not shape");
    }

    /// A mask's shape reads on the same scale a cell's does.
    #[test]
    fn a_masks_shape_is_read_the_same_way() {
        assert_eq!(SenseMask::of(SenseChannel::NecroticDecay).band_share(), [0, 0, 10_000, 0]);
        assert_eq!(SenseMask::EMPTY.band_share(), [0; BAND_COUNT]);
        assert_eq!(SenseMask::ALL.band_share(), [2_500; BAND_COUNT]);
        let two = SenseMask::of(SenseChannel::NecroticDecay).with(SenseChannel::AtmospherePa);
        assert_eq!(two.band_share(), [0, 5_000, 5_000, 0]);
    }

    /// The whole mechanic in one assert: a body sits closer to a cell shaped
    /// like it than to a cell shaped like something else.
    #[test]
    fn a_body_sits_closest_to_a_cell_shaped_like_itself() {
        let mut death = PentaractField::quiet_at(point());
        death[SenseChannel::NecroticDecay] = 9_000;
        let mut stone = PentaractField::quiet_at(point());
        stone[SenseChannel::MasonryStress] = 9_000;

        let lich = mood_point(0, SenseMask::of(SenseChannel::NecroticDecay).band_share());
        let near = lich.cos_similarity(&death.oriented(1).at);
        let far = lich.cos_similarity(&stone.oriented(1).at);
        assert!(near > 32_767 - 200, "same shape should read as aligned, got {near}");
        assert!(far < near, "a foreign shape must read further off: {far} vs {near}");
        assert!(
            SenseGain::attuned(near).0[0] > SenseGain::attuned(far).0[0],
            "and that distance must cost the body gain"
        );
    }

    /// L18-style sabotage: quadrature is load-bearing. Tilting off zero instead
    /// of off `π/2` collapses every axis past the first.
    #[test]
    fn sabotaged_rest_angle_would_collapse_the_lower_axes() {
        let live = mood_point(0, [0, 0, 10_000, 0]).unit_vector();
        let alive = live.iter().filter(|&&x| x.abs() > 1_000).count();
        assert!(alive >= 2, "quadrature must keep more than one axis alive: {live:?}");

        let collapsed = Pentaract::new(0, 0, 0, 16_384, 0, 0, TRUTH_CREATIVE).unit_vector();
        assert!(
            collapsed[1..].iter().all(|&x| x.abs() <= 1_000),
            "sabotaged zero-rest angle should collapse x2..x5, but did not: {collapsed:?}"
        );
    }

    /// This module is additive: the ARCH000 point it samples is untouched.
    #[test]
    fn the_pentaract_layout_did_not_move() {
        assert_eq!(core::mem::size_of::<Pentaract>(), 32);
    }
}
