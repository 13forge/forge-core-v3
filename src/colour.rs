//! Integer-deterministic OKLCH. No `f32` anywhere in this file — a perceptual channel
//! that a GPU vendor may round differently is a channel that breaks replay, and §3's
//! arithmetic rule bans float across the boundary. Every value here is an integer that
//! compares exactly.
//!
//! Two prior homes exist for this concept in the v2 tree and neither is imported
//! (G02 — law and layout do not cross trees):
//!
//! - `forge-colour/src/lib.rs:21` — `OklchColor { l: f32, c: f32, h: f32 }`, 12 B, float.
//! - `forge-core/src/colour_ir.rs:27` — `OklchPmy { l: i32, c: i32, h: i32 }`, 12 B,
//!   permyriad + centidegrees, with a 6-variant `VisionProfile` at `:72`.
//!
//! This is the v3 home and is 8 B, which is why the encoding differs: four `u16`
//! channels instead of three `i32`. The **variant set is carried over intact**,
//! including `Scotopic` — it is not a blindness mode but the night-vision hue clamp,
//! and the daltonisation transform at `colour_ir.rs:47-63` is written against all six.
//! Shipping five would strand it.

/// One colour, 8 bytes, exact. Channel scales are fixed here and nowhere else.
///
/// `h` is a **binary angle** — the full turn is `u16::MAX + 1`, so hue arithmetic wraps
/// on the integer type itself and no modulus is ever needed. That is the whole reason
/// hue is not stored in degrees: `359° + 2°` needs a branch, `h.wrapping_add(d)` does not.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OklchColor {
    /// Lightness. `0..=65535` maps `0.0..=1.0`.
    pub l: u16,
    /// Chroma. `0..=65535` maps `0.0..=CHROMA_CEILING_PERMYRIAD / 10000`.
    pub c: u16,
    /// Hue, binary angle measure. The type is the modulus.
    pub h: u16,
    /// Alpha. `0..=65535` maps `0.0..=1.0`.
    pub a: u16,
}

/// Chroma's upper bound in permyriad — `0.4` in OKLCH terms, carried from
/// `colour_ir.rs:30` so the two encodings describe the same gamut rather than two.
pub const CHROMA_CEILING_PERMYRIAD: u32 = 4_000;

/// A full turn in binary angle measure. `u16::MAX + 1`, held as `u32` so it is nameable.
pub const TURN: u32 = 1 << 16;

impl OklchColor {
    /// Fully opaque black.
    pub const BLACK: Self = Self { l: 0, c: 0, h: 0, a: u16::MAX };
    /// Fully opaque white, zero chroma.
    pub const WHITE: Self = Self { l: u16::MAX, c: 0, h: 0, a: u16::MAX };

    /// Opaque achromatic grey. Chroma zero makes hue meaningless, so it is pinned to 0 —
    /// two greys of equal lightness must compare equal, and they cannot if hue is free.
    #[inline(always)]
    pub const fn grey(l: u16) -> Self {
        Self { l, c: 0, h: 0, a: u16::MAX }
    }

    /// Rotate the hue. Wraps by construction; there is no out-of-range hue to reject.
    #[inline(always)]
    pub const fn rotate(self, delta: u16) -> Self {
        Self { h: self.h.wrapping_add(delta), ..self }
    }

    /// True when chroma is zero — the colour carries no hue information at all.
    #[inline(always)]
    pub const fn is_achromatic(self) -> bool {
        self.c == 0
    }

    /// Correct for a colour-vision deficiency by pushing hues that sit inside that
    /// mode's confusion band away from the axis, and encoding the same push into
    /// lightness (the confusable channel is not the impaired one). Achromatic input
    /// (`c == 0`) passes through unchanged for every mode except `Achromatopsia`,
    /// which forces `c = 0` on any input. Fresh integer arithmetic against this 8B
    /// shape — v2's float transform (`colour_ir.rs:47-63`) is not imported (G02).
    pub const fn daltonize(self, mode: ColorBlindMode) -> Self {
        match mode {
            ColorBlindMode::Normal => self,
            ColorBlindMode::Achromatopsia => Self { c: 0, ..self },
            _ if self.c == 0 => self,
            ColorBlindMode::Protanopia => self.push_from_axis(PROTAN_AXIS),
            ColorBlindMode::Deuteranopia => self.push_from_axis(DEUTAN_AXIS),
            ColorBlindMode::Tritanopia => self.push_from_axis(TRITAN_AXIS),
            ColorBlindMode::Scotopic => self.scotopic_shift(),
        }
    }

    /// Push a hue sitting inside `CONFUSION_BAND_HALF` of `axis` further away from
    /// it, ramping linearly with proximity, and stretch lightness the same way so a
    /// pair that collides in hue still separates in lightness.
    const fn push_from_axis(self, axis: u16) -> Self {
        let diff = hue_diff(self.h, axis);
        let dist = if diff < 0 { -diff } else { diff } as u16;
        if dist >= CONFUSION_BAND_HALF {
            return self;
        }
        let closeness = CONFUSION_BAND_HALF - dist;
        let magnitude = (closeness as u32 * CONFUSION_PUSH as u32 / CONFUSION_BAND_HALF as u32) as u16;
        let new_h = if diff >= 0 {
            self.h.wrapping_add(magnitude)
        } else {
            self.h.wrapping_sub(magnitude)
        };
        let l_push = (closeness as u32 * (u16::MAX as u32 / 4) / CONFUSION_BAND_HALF as u32) as u16;
        let new_l = if self.l >= 32_768 {
            self.l.saturating_add(l_push)
        } else {
            self.l.saturating_sub(l_push)
        };
        Self { l: new_l, h: new_h, ..self }
    }

    /// Night-vision hue clamp: pull hue halfway toward the blue-shifted scotopic
    /// target and halve chroma, matching the dark-adaptation band this variant
    /// documents at its definition.
    const fn scotopic_shift(self) -> Self {
        let diff = hue_diff(self.h, SCOTOPIC_HUE_TARGET);
        let new_h = self.h.wrapping_sub((diff / 2) as u16);
        Self { h: new_h, c: self.c / 2, ..self }
    }
}

/// Half-width of the hue confusion band pulled from each axis (`TURN/12`, ~30°).
const CONFUSION_BAND_HALF: u16 = (TURN / 12) as u16;
/// Maximum hue push applied to a colour sitting exactly on a confusion axis.
const CONFUSION_PUSH: u16 = (TURN / 8) as u16;
/// Protanopia's red/cyan confusion axis.
const PROTAN_AXIS: u16 = 0;
/// Deuteranopia's axis, approximated a third of a turn from protan's.
const DEUTAN_AXIS: u16 = (TURN / 3) as u16;
/// Tritanopia's blue/yellow confusion axis.
const TRITAN_AXIS: u16 = (TURN * 2 / 3) as u16;
/// Scotopic (night-vision) hue target — blue-shifted, off the dark-adaptation band.
const SCOTOPIC_HUE_TARGET: u16 = (TURN * 2 / 3) as u16;

/// Signed hue distance `h - axis`, wrapped into `-32768..=32768` so the sign names
/// a rotation direction instead of always reading as the long way around the turn.
const fn hue_diff(h: u16, axis: u16) -> i32 {
    let raw = h.wrapping_sub(axis) as i32;
    if raw > 32_768 {
        raw - 65_536
    } else {
        raw
    }
}

/// Vision profiles. Six, matching `colour_ir.rs:72` — the transform there is written
/// against this exact set and dropping one silently disables a correction path.
///
/// Daltonisation works by encoding a confusable hue axis into **lightness**, which CVD
/// does not impair. That is why these are corrections and not simulations.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColorBlindMode {
    /// No remap. The identity.
    Normal = 0,
    /// Red-weak. Red-green confusion.
    Protanopia = 1,
    /// Green-weak. Red-green confusion, and the most common.
    Deuteranopia = 2,
    /// Blue-weak. Blue-yellow confusion.
    Tritanopia = 3,
    /// No hue perception. Value only.
    Achromatopsia = 4,
    /// Night / low-light. Not a deficiency — a hue clamp off the dark-adaptation band.
    Scotopic = 5,
}

/// How many profiles exist. The modulus, in one place.
pub const VISION_PROFILES: usize = 6;

impl ColorBlindMode {
    /// Every profile in ordinal order.
    pub const ALL: [ColorBlindMode; VISION_PROFILES] = [
        ColorBlindMode::Normal,
        ColorBlindMode::Protanopia,
        ColorBlindMode::Deuteranopia,
        ColorBlindMode::Tritanopia,
        ColorBlindMode::Achromatopsia,
        ColorBlindMode::Scotopic,
    ];

    /// Decode a stored ordinal. `None` outside `0..=5` — a seventh profile is
    /// corruption, not an extension.
    #[inline(always)]
    pub const fn from_u8(b: u8) -> Option<Self> {
        match b {
            0 => Some(ColorBlindMode::Normal),
            1 => Some(ColorBlindMode::Protanopia),
            2 => Some(ColorBlindMode::Deuteranopia),
            3 => Some(ColorBlindMode::Tritanopia),
            4 => Some(ColorBlindMode::Achromatopsia),
            5 => Some(ColorBlindMode::Scotopic),
            _ => None,
        }
    }

    /// The stored ordinal.
    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

// ---------------------------------------------------------------------------
// LAYOUT LOCKS. Measured with rustc, never hand-computed.
// ---------------------------------------------------------------------------
const _: () = assert!(core::mem::size_of::<OklchColor>() == 8);
const _: () = assert!(core::mem::align_of::<OklchColor>() == 8);
const _: () = assert!(core::mem::size_of::<ColorBlindMode>() == 1);

// OFFSET LOCKS. Size alone is a weak gate here: any four `u16` fields measure 8, so a
// reordering that swapped `c` and `h` would keep the size assert green while silently
// reinterpreting every stored colour.
const _: () = assert!(core::mem::offset_of!(OklchColor, l) == 0);
const _: () = assert!(core::mem::offset_of!(OklchColor, c) == 2);
const _: () = assert!(core::mem::offset_of!(OklchColor, h) == 4);
const _: () = assert!(core::mem::offset_of!(OklchColor, a) == 6);

// Every one of the 8 bytes is a field — no padding hole for a fifth channel to hide in.
const _: () = assert!(4 * core::mem::size_of::<u16>() == core::mem::size_of::<OklchColor>());

// The profile count is the modulus, in one place.
const _: () = assert!(ColorBlindMode::ALL.len() == VISION_PROFILES);

// A colour is not a coordinate. `OklchColor` must never be mistaken for a `Pexil` slot:
// both are 8 bytes, and that coincidence is exactly the kind a type gate has to hold.
const _: () = assert!(core::mem::size_of::<OklchColor>() == core::mem::size_of::<crate::atom::Pexil>());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_profiles_and_nothing_else_decodes() {
        for b in 0u8..=255 {
            match ColorBlindMode::from_u8(b) {
                Some(m) => {
                    assert!(b < VISION_PROFILES as u8, "byte {b} decoded but is out of range");
                    assert_eq!(m.as_u8(), b, "ordinal round-trip");
                }
                None => assert!(b >= VISION_PROFILES as u8, "byte {b} is a profile and must decode"),
            }
        }
    }

    /// The variant `colour_ir.rs:53` depends on. Its absence is the failure this catches.
    #[test]
    fn scotopic_survived_the_port() {
        assert!(ColorBlindMode::ALL.contains(&ColorBlindMode::Scotopic));
        assert_eq!(ColorBlindMode::from_u8(5), Some(ColorBlindMode::Scotopic));
    }

    #[test]
    fn every_profile_is_reachable_and_distinct() {
        for (i, m) in ColorBlindMode::ALL.iter().enumerate() {
            assert_eq!(m.as_u8() as usize, i, "ALL is not in ordinal order at {i}");
        }
        for (i, a) in ColorBlindMode::ALL.iter().enumerate() {
            for b in &ColorBlindMode::ALL[i + 1..] {
                assert_ne!(a, b, "two profiles share an ordinal");
            }
        }
    }

    /// Hue is a binary angle, so rotation is total: every start and every delta lands on
    /// a valid hue and the inverse rotation returns the original. No modulus, no branch.
    #[test]
    fn hue_rotation_wraps_and_is_invertible() {
        for h in (0u32..=u16::MAX as u32).step_by(97) {
            for d in [0u16, 1, 255, 16_384, 32_768, 65_535] {
                let c = OklchColor { l: 100, c: 200, h: h as u16, a: 300 };
                let rotated = c.rotate(d);
                assert_eq!(rotated.rotate(d.wrapping_neg()), c, "h={h} d={d}");
            }
        }
    }

    #[test]
    fn a_half_turn_twice_is_the_identity() {
        let half = (TURN / 2) as u16;
        let c = OklchColor { l: 1, c: 2, h: 3, a: 4 };
        assert_eq!(c.rotate(half).rotate(half), c);
    }

    #[test]
    fn grey_is_achromatic_and_hue_free() {
        for l in [0u16, 1, 32_768, u16::MAX] {
            let g = OklchColor::grey(l);
            assert!(g.is_achromatic());
            assert_eq!(g.h, 0, "a grey with a live hue compares unequal to its own twin");
            assert_eq!(g, OklchColor::grey(l));
        }
        assert!(OklchColor::BLACK.is_achromatic());
        assert!(OklchColor::WHITE.is_achromatic());
    }

    /// `Ord` is derived, so it is field order. Lightness must dominate — sorting a
    /// palette by luminance is the only ordering that means anything perceptually.
    #[test]
    fn ordering_is_lightness_first() {
        let dark_vivid = OklchColor { l: 10, c: u16::MAX, h: u16::MAX, a: u16::MAX };
        let light_dull = OklchColor { l: 11, c: 0, h: 0, a: 0 };
        assert!(dark_vivid < light_dull, "chroma outranked lightness");
    }

    #[test]
    fn the_gamut_ceiling_is_the_one_from_the_v2_bus() {
        assert_eq!(CHROMA_CEILING_PERMYRIAD, 4_000, "colour_ir.rs:30 says 0.0..0.4");
        assert_eq!(TURN, 65_536);
    }

    #[test]
    fn daltonize_normal_is_identity() {
        for c in [
            OklchColor { l: 100, c: 3_000, h: 0, a: u16::MAX },
            OklchColor { l: 50_000, c: 1_000, h: 20_000, a: 100 },
            OklchColor::WHITE,
        ] {
            assert_eq!(c.daltonize(ColorBlindMode::Normal), c);
        }
    }

    #[test]
    fn daltonize_leaves_achromatic_input_alone_except_achromatopsia() {
        for l in [0u16, 1, 32_768, u16::MAX] {
            let grey = OklchColor::grey(l);
            for mode in [
                ColorBlindMode::Protanopia,
                ColorBlindMode::Deuteranopia,
                ColorBlindMode::Tritanopia,
                ColorBlindMode::Scotopic,
            ] {
                assert_eq!(grey.daltonize(mode), grey, "grey must pass through mode {mode:?}");
            }
            // Achromatopsia is a no-op on already-achromatic input too.
            assert_eq!(grey.daltonize(ColorBlindMode::Achromatopsia), grey);
        }
    }

    #[test]
    fn daltonize_achromatopsia_always_zeroes_chroma() {
        for c in [
            OklchColor { l: 100, c: 3_000, h: 0, a: u16::MAX },
            OklchColor { l: 50_000, c: u16::MAX, h: 20_000, a: 100 },
        ] {
            let out = c.daltonize(ColorBlindMode::Achromatopsia);
            assert_eq!(out.c, 0);
            assert_eq!(out.l, c.l, "lightness must survive achromatopsia correction");
        }
    }

    #[test]
    fn daltonize_is_deterministic() {
        let c = OklchColor { l: 12_345, c: 2_500, h: 8_000, a: u16::MAX };
        for mode in ColorBlindMode::ALL {
            assert_eq!(c.daltonize(mode), c.daltonize(mode), "mode {mode:?} must be pure");
        }
    }

    #[test]
    fn daltonize_pushes_hue_only_inside_the_confusion_band() {
        // A hue exactly on the protan axis (0) sits inside its own confusion band
        // and must move; a hue a half turn away must not.
        let near = OklchColor { l: 32_768, c: 3_000, h: 0, a: u16::MAX };
        let far = OklchColor { l: 32_768, c: 3_000, h: (TURN / 2) as u16, a: u16::MAX };
        assert_ne!(near.daltonize(ColorBlindMode::Protanopia).h, near.h);
        assert_eq!(far.daltonize(ColorBlindMode::Protanopia).h, far.h);
    }
}
