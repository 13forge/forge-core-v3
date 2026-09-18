//! CDK — the Cosmic Dissonance Kernel, `Triad` only.
//!
//! Ported 2026-08-13 from `F:\NewRepo\crates\forge-game-systems\src\cdk.rs:14-136`
//! (census.tsv `forge-game-systems` row, GAME-SYSTEMS-CENSUS-2026-08-11.md:88-90).
//! Empedocles made mechanical: LOVE binds, STRIFE separates, ENTROPY is what neither
//! holds. Integer-only in the v2 source and here — the donor's own doc comment: "a
//! float here would put a non-deterministic edge on the 120Hz tick".
//!
//! **Scope cut, stated plainly (C09 aperture):** the v2 file is 503 lines and only
//! `Triad` itself (its fields, `disposition`/`harmony`/`dissonant`/`to_channels`, and
//! the private `norm_signed` helper) is self-contained. Everything else in the donor —
//! `triad()`/`triad_from_fields()` (need `FactionMind`, `Cell`, `forge_zones`,
//! `forge_calligraphy`), `to_vibe_signals`/`to_vibe` (need `forge_core::vibe_matrix`),
//! `TriadHold` (wall-clock crossing, needs a clock bridge), `room_frame`/`tone_of_triad`
//! (need `forge_hal::TripleBuffer`, `forge_sieve::prime_seed`, `forge_calligraphy::
//! audio_bridge`) — none of those crates/types exist in v3 yet. Porting them now would
//! be scaffolding against nothing; this file ports only what has no dangling reference.
//!
//! **CORRECTED (2026-08-13):** the assumption below was live for under a day.
//! `FactionMind` was never missing from v3 — it was already landed in
//! `forge-mud-v3::mind.rs` (8-axis, 5 authored factions), and that crate had
//! independently re-derived its OWN second `Triad` plus a real `triad()`
//! dealer, `colour()`, `bar()`, `report()` (`forge-mud-v3::cdk.rs`) — an L05
//! violation caught by a lateral-criticality pass, not by re-reading this
//! note. Fixed by making `forge_mud_v3::cdk` import `Triad` from HERE
//! (`pub use forge_core_v3::cdk::Triad;` — `forge-mud-v3` already depends on
//! `forge-core-v3`, the correct direction per L06 `dag_root`) and keep only
//! the `FactionMind`-dependent dealer + rendering fns downstream, where
//! `FactionMind` correctly lives (faction cognition is MUD-domain, not
//! core-domain — it does not belong in this crate). `Cell`/`forge_zones`/
//! `forge_calligraphy`/`forge_hal::TripleBuffer`/`forge_sieve::prime_seed`
//! remain genuinely absent from v3 as of this correction — `to_vibe_signals`/
//! `TriadHold`/`room_frame`/`tone_of_triad` stay unported for that reason,
//! unchanged from the original note.
//!
//! Assumes: `Triad` alone is useful without a dealer function IN THIS CRATE.
//! Proven true: the dealer exists, one level downstream, exactly where its
//! one real dependency (`FactionMind`) already lived.

/// The Empedoclean triad, as permyriad-scaled integers. Never float: this rides a
/// deterministic tick like everything else in this workspace (C14 firewall).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Triad {
    /// Philotes — a binding force. Can be large while the room still tears itself
    /// apart, because strife and entropy pull just as hard.
    pub love: i32,
    /// What separates: threat, dominance, the need to close an open question.
    pub strife: i32,
    /// What neither love nor strife holds — never free, always a cut off disposition.
    pub entropy: i32,
}

impl Triad {
    /// Full-scale bound for the signed lanes: three i16 faction axes plus a proximity
    /// pull, in the v2 source. Kept as the same numeric constant so a future port of
    /// `triad()` reproduces byte-identical channel values against this file.
    pub const LANE_SPAN: i32 = 3 * 900 + 1_000;

    /// Entropy arrives as a permyriad strength.
    pub const HAUNT_MAX: i32 = 10_000;

    /// Net disposition. Entropy always takes its cut — a room never breaks even by
    /// standing still.
    #[inline]
    pub const fn disposition(&self) -> i32 {
        self.love - self.strife - self.entropy
    }

    /// The room has come apart: STRIFE and ENTROPY together outweigh LOVE.
    #[inline]
    pub const fn dissonant(&self) -> bool {
        self.disposition() < 0
    }

    /// The three lanes as `0..=1000` — the range every bind surface expects. Raw `love`
    /// spans about ±3700 and `strife` can go negative, so a caller that binds the raw
    /// fields feeds a shader garbage; this is the only lawful hand-off.
    #[inline]
    pub fn to_channels(&self) -> [i32; 3] {
        [
            norm_signed(self.love),
            norm_signed(self.strife),
            self.entropy.clamp(0, Self::HAUNT_MAX) * 1_000 / Self::HAUNT_MAX,
        ]
    }

    /// HARMONY is not LOVE. LOVE is a binding FORCE; HARMONY is the PROPORTION of the
    /// whole pull that actually binds — derived, never stored, so it cannot drift from
    /// the lanes it summarises. Returns `0..=1000`; total silence answers `1000` by
    /// convention (nothing pulling means nothing is out of proportion).
    #[inline]
    pub fn harmony(&self) -> i32 {
        let [l, s, e] = self.to_channels();
        let total = l + s + e;
        if total == 0 {
            return 1_000;
        }
        l * 1_000 / total
    }
}

/// Map a signed lane in `-LANE_SPAN..=LANE_SPAN` onto `0..=1000`, saturating at both
/// ends. Widened to `i64` for the multiply.
fn norm_signed(v: i32) -> i32 {
    let span = Triad::LANE_SPAN as i64;
    let clamped = (v as i64).clamp(-span, span);
    (((clamped + span) * 1_000) / (2 * span)) as i32
}

/// Source consequence kind: blessed, cursed, or decay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsequenceSourceKind {
    /// Blessed (favorable) supernatural force → love lane.
    Blessed,
    /// Cursed (hostile) supernatural force → strife lane.
    Cursed,
    /// Decay, season, aging → entropy lane.
    Decay,
}

/// Map source consequence kind + magnitude to Triad lanes.
/// Blessed → love+, cursed → strife+, decay → entropy+. Zero magnitude → zero Triad.
#[inline]
pub fn consequence_to_triad(kind: ConsequenceSourceKind, magnitude_pmy: u16) -> Triad {
    let mag = magnitude_pmy as i32;
    match kind {
        ConsequenceSourceKind::Blessed => Triad { love: mag, strife: 0, entropy: 0 },
        ConsequenceSourceKind::Cursed => Triad { love: 0, strife: mag, entropy: 0 },
        ConsequenceSourceKind::Decay => Triad { love: 0, strife: 0, entropy: mag },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_stay_in_bind_range_across_the_lane_span() {
        for love in [-Triad::LANE_SPAN, 0, Triad::LANE_SPAN] {
            for strife in [-Triad::LANE_SPAN, 0, Triad::LANE_SPAN] {
                for entropy in [0, Triad::HAUNT_MAX / 2, Triad::HAUNT_MAX, 99_999] {
                    let ch = Triad { love, strife, entropy }.to_channels();
                    assert!(
                        ch.iter().all(|c| (0..=1_000).contains(c)),
                        "{love} {strife} {entropy} -> {ch:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn entropy_lane_spans_its_input() {
        assert_eq!(Triad { love: 0, strife: 0, entropy: 0 }.to_channels()[2], 0);
        assert_eq!(Triad { love: 0, strife: 0, entropy: Triad::HAUNT_MAX }.to_channels()[2], 1_000);
    }

    #[test]
    fn harmony_spans_its_range() {
        let pure_love = Triad { love: Triad::LANE_SPAN, strife: -Triad::LANE_SPAN, entropy: 0 };
        let pure_strife = Triad { love: -Triad::LANE_SPAN, strife: Triad::LANE_SPAN, entropy: 0 };
        assert_eq!(pure_love.harmony(), 1_000);
        assert_eq!(pure_strife.harmony(), 0);
        let silent = Triad { love: 0, strife: 0, entropy: 0 };
        assert!((0..=1_000).contains(&silent.harmony()));
    }

    #[test]
    fn harmony_is_not_love() {
        // Same love, entropy alone rising: love (a force) is untouched, harmony (a
        // proportion) must fall as entropy takes a bigger share of the whole pull.
        let held = Triad { love: 900, strife: 0, entropy: 0 };
        let haunted = Triad { love: 900, strife: 0, entropy: Triad::HAUNT_MAX };
        assert_eq!(held.love, haunted.love, "love is a force and did not move");
        assert!(haunted.harmony() < held.harmony(), "harmony must fall when entropy grows");
    }

    #[test]
    fn consequence_to_triad_maps_blessed_to_love() {
        let t = consequence_to_triad(ConsequenceSourceKind::Blessed, 100);
        assert_eq!(t.love, 100);
        assert_eq!(t.strife, 0);
        assert_eq!(t.entropy, 0);
    }

    #[test]
    fn consequence_to_triad_maps_cursed_to_strife() {
        let t = consequence_to_triad(ConsequenceSourceKind::Cursed, 100);
        assert_eq!(t.love, 0);
        assert_eq!(t.strife, 100);
        assert_eq!(t.entropy, 0);
    }

    #[test]
    fn consequence_to_triad_maps_decay_to_entropy() {
        let t = consequence_to_triad(ConsequenceSourceKind::Decay, 100);
        assert_eq!(t.love, 0);
        assert_eq!(t.strife, 0);
        assert_eq!(t.entropy, 100);
    }

    #[test]
    fn consequence_to_triad_zero_magnitude_gives_zero_triad() {
        let t = consequence_to_triad(ConsequenceSourceKind::Blessed, 0);
        assert_eq!(t.love, 0);
        assert_eq!(t.strife, 0);
        assert_eq!(t.entropy, 0);
    }

    #[test]
    fn disposition_and_dissonance_agree() {
        let calm = Triad { love: 1_000, strife: 0, entropy: 0 };
        let torn = Triad { love: 0, strife: 500, entropy: 600 };
        assert!(calm.disposition() > 0 && !calm.dissonant());
        assert!(torn.disposition() < 0 && torn.dissonant());
    }
}
