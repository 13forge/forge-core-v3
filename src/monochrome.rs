//! The monochrome law: a drained era renders achromatic, and colour returns to
//! ONE thing at a time as its fact is earned. Colour is the reward channel, so
//! nothing here invents a hue — restoration returns exactly what was drained.

use crate::colour::OklchColor;
use crate::organs::creation_spine::LoreFactId;

/// Drain a colour to its own grey: chroma to zero, LIGHTNESS UNTOUCHED.
///
/// Preserving lightness is what keeps a drained scene readable — a greyscale
/// world still has form. Hue goes to 0 with the chroma because
/// [`OklchColor::grey`] already argues the point: with no chroma, hue carries
/// nothing, and two greys of equal lightness must compare equal.
///
/// Deliberately NOT `daltonize(ColorBlindMode::Achromatopsia)`, which produces
/// the same bytes for a completely different reason. That is an accessibility
/// profile describing an EYE; this is a world state describing an ERA. Folding
/// them together would mean a player's vision setting and the era's arc share
/// one switch, and no later reader could tell which one drained the frame.
#[inline]
pub fn drained(colour: OklchColor) -> OklchColor {
    OklchColor { c: 0, h: 0, ..colour }
}

/// Which things have earned their colour back.
///
/// Holds facts, not palettes: the law never stores a colour, so it can never
/// hand back a different one than was drained.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonochromeLaw {
    restored: Vec<LoreFactId>,
}

impl MonochromeLaw {
    /// A fully drained era. Nothing has earned colour yet.
    pub const fn drained_era() -> Self {
        Self { restored: Vec::new() }
    }

    /// Mark one thing's colour as earned. Idempotent — a fact restored twice is
    /// still one restored thing, so a replayed ledger cannot inflate the count.
    pub fn restore(&mut self, fact: LoreFactId) {
        if !self.is_restored(fact) {
            self.restored.push(fact);
        }
    }

    /// Has this thing earned its colour?
    pub fn is_restored(&self, fact: LoreFactId) -> bool {
        self.restored.contains(&fact)
    }

    /// How many things have come back. The era's arc, as one number.
    pub fn restored_count(&self) -> usize {
        self.restored.len()
    }

    /// True while nothing has been restored — the era at its greyest.
    pub fn is_fully_drained(&self) -> bool {
        self.restored.is_empty()
    }

    /// Render one thing's colour under the law.
    ///
    /// `fact` is the thing's own restoration fact. `None` means the thing has
    /// no fact to earn — scenery, not a reward — and it stays drained for as
    /// long as the law is in force.
    pub fn render(&self, colour: OklchColor, fact: Option<LoreFactId>) -> OklchColor {
        match fact {
            Some(f) if self.is_restored(f) => colour,
            _ => drained(colour),
        }
    }

    /// Rebuild the law from a ledger's own facts: every candidate fact the
    /// ledger holds is restored, in the order given.
    ///
    /// Takes the held-check as a closure rather than a `&Ledger` so the law
    /// stays in Crate Zero without reaching for a consumer's ledger type.
    pub fn from_facts(candidates: &[LoreFactId], holds: impl Fn(LoreFactId) -> bool) -> Self {
        let mut law = Self::drained_era();
        for &fact in candidates {
            if holds(fact) {
                law.restore(fact);
            }
        }
        law
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: OklchColor = OklchColor { l: 30_000, c: 20_000, h: 4_000, a: u16::MAX };
    const BLUE: OklchColor = OklchColor { l: 30_000, c: 20_000, h: 40_000, a: u16::MAX };

    fn fact(n: u64) -> LoreFactId {
        LoreFactId(n)
    }

    #[test]
    fn draining_keeps_the_light_and_takes_the_colour() {
        let grey = drained(RED);
        assert_eq!(grey.l, RED.l, "a drained world still has form");
        assert_eq!(grey.a, RED.a, "draining is not fading");
        assert!(grey.is_achromatic());
    }

    /// The flattening has to be real: two things that differ only in hue must
    /// become indistinguishable, or the era is not black and white.
    #[test]
    fn two_hues_of_one_lightness_drain_to_the_same_grey() {
        assert_ne!(RED, BLUE);
        assert_eq!(drained(RED), drained(BLUE));
    }

    #[test]
    fn a_drained_era_shows_nothing_in_colour() {
        let law = MonochromeLaw::drained_era();
        assert!(law.is_fully_drained());
        assert_eq!(law.render(RED, Some(fact(1))), drained(RED));
        assert_eq!(law.render(RED, None), drained(RED));
    }

    /// Colour returns EXACTLY as it was. The law stores facts, never palettes,
    /// so it has nothing to hand back but the original.
    #[test]
    fn a_restored_thing_gets_its_own_colour_back_unchanged() {
        let mut law = MonochromeLaw::drained_era();
        law.restore(fact(1));
        assert_eq!(law.render(RED, Some(fact(1))), RED, "not a repaint — the same colour");
    }

    /// One thing at a time. Healing the bell does not repaint the sky.
    #[test]
    fn restoring_one_thing_leaves_every_other_thing_grey() {
        let mut law = MonochromeLaw::drained_era();
        law.restore(fact(1));
        assert_eq!(law.render(BLUE, Some(fact(2))), drained(BLUE));
        assert_eq!(law.restored_count(), 1);
        assert!(!law.is_fully_drained(), "one restored thing ends the full drain");
    }

    /// Scenery has no fact to earn and stays drained however far the era comes.
    #[test]
    fn a_thing_with_no_fact_never_comes_back() {
        let mut law = MonochromeLaw::drained_era();
        for n in 0..8 {
            law.restore(fact(n));
        }
        assert_eq!(law.render(RED, None), drained(RED));
    }

    /// A replayed ledger must not inflate the arc.
    #[test]
    fn restoring_twice_is_still_one_thing() {
        let mut law = MonochromeLaw::drained_era();
        law.restore(fact(7));
        law.restore(fact(7));
        assert_eq!(law.restored_count(), 1);
    }

    #[test]
    fn the_law_rebuilds_from_the_facts_a_ledger_holds() {
        let candidates = [fact(1), fact(2), fact(3)];
        let held = [fact(1), fact(3)];
        let law = MonochromeLaw::from_facts(&candidates, |f| held.contains(&f));
        assert_eq!(law.restored_count(), 2);
        assert!(law.is_restored(fact(1)) && law.is_restored(fact(3)));
        assert!(!law.is_restored(fact(2)));
        assert_eq!(law.render(RED, Some(fact(2))), drained(RED));
    }

    /// The era's arc is one number, and it only ever climbs.
    #[test]
    fn the_arc_is_countable_and_monotonic() {
        let mut law = MonochromeLaw::drained_era();
        let mut last = 0;
        for n in 0..5 {
            law.restore(fact(n));
            assert!(law.restored_count() > last, "each new thing advances the era");
            last = law.restored_count();
        }
    }

    /// This law is a WORLD state, not a vision profile. If someone ever routes
    /// it through `daltonize`, this test says why they should not.
    #[test]
    fn draining_is_not_an_accessibility_mode() {
        use crate::colour::ColorBlindMode;
        let by_law = drained(RED);
        let by_eye = RED.daltonize(ColorBlindMode::Achromatopsia);
        assert_eq!(by_law.c, by_eye.c, "the bytes agree — that is the trap");
        assert_eq!(
            RED.daltonize(ColorBlindMode::Normal),
            RED,
            "a normal eye sees the era's colour untouched; only the LAW drains it"
        );
    }
}
