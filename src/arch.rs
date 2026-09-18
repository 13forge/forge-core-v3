//! Authority and the two clocks. Sizes here are the ones that were stated wrong in
//! three successive drafts (HANDOFF §3) — every one is asserted, none is re-derived.
//!
//! The two clocks are separate types on purpose: `DetClock` indexes a replay and must
//! be reproducible from a seed; `CreativeClock` is free-running and must never be
//! mistaken for it. A single `Clock` with a flag would let one be passed as the other.

/// Authority handle. `ARCH000` is a *person*, not a machine role (CLAUDE.md L17,
/// HANDOFF §6.2), so the role table is deliberately **not** enumerated here —
/// enumerating it would relocate authority into this file. 2 bytes, so it fits the
/// trailing `u16` slot of `DetClock` for free.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchRole(pub u16);

impl ArchRole {
    /// Prime Authority. Ordinal zero, and the only ordinal this crate may name.
    pub const ARCH000: Self = Self(0);

    /// True only for the Prime Authority. Every gated verb in CLAUDE.md L17 asks this.
    #[inline(always)]
    pub const fn is_prime_authority(self) -> bool {
        self.0 == Self::ARCH000.0
    }
}

/// The deterministic clock. A replay index, not a wall time: `(epoch, tick)` orders
/// totally and reproduces from a seed. 16 B — `u64 + u16 + u16` pads to 16, **not 12**.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DetClock {
    /// Monotonic step count inside `epoch`. Never decreases.
    pub tick: u64,
    /// Replay generation. Bumped on reseed, so `tick` may restart without ambiguity.
    pub epoch: u16,
    /// Who advanced it. Authority travels with the tick, not beside it.
    pub authority: ArchRole,
}

/// The free-running clock. 12 B **as declared** — field order is load-bearing: moving
/// `seed` first would make it 8 B. That trade is HANDOFF §3's open item, so the layout
/// is pinned as measured rather than quietly improved; `tests::the_eight_byte_ordering`
/// proves the 8 B claim instead of asserting it in prose.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreativeClock {
    /// Beat index. Wraps; no replay guarantee attaches to it.
    pub beat: u16,
    /// Generator seed for this beat. 32 bits, and the reason for the padding.
    pub seed: u32,
    /// Sub-beat phase.
    pub phase: u16,
}

/// Ordering is `(epoch, tick, authority)` — **not** declaration order. `epoch` must
/// dominate or a reseed to `tick = 0` compares as a rewind of history. `tick` cannot be
/// the first field of the ordering and the first field of the struct at once: `epoch`
/// first would cost 24 bytes (`tests::epoch_outranks_tick` holds the ordering,
/// the layout lock holds the 16). `authority` is the final tiebreak only so that
/// `cmp == Equal` agrees with `==`.
impl Ord for DetClock {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (self.epoch, self.tick, self.authority.0).cmp(&(other.epoch, other.tick, other.authority.0))
    }
}

impl PartialOrd for DetClock {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl DetClock {
    /// Genesis: epoch 0, tick 0, Prime Authority.
    pub const GENESIS: Self = Self { tick: 0, epoch: 0, authority: ArchRole::ARCH000 };

    /// Advance one step. `None` on `u64` exhaustion — a wrapped replay index would
    /// silently re-order history, so the caller must bump `epoch` instead.
    #[inline(always)]
    pub const fn next(self) -> Option<Self> {
        match self.tick.checked_add(1) {
            Some(tick) => Some(Self { tick, ..self }),
            None => None,
        }
    }
}

impl CreativeClock {
    /// Genesis: beat 0, seed 0, phase 0.
    pub const GENESIS: Self = Self { beat: 0, seed: 0, phase: 0 };

    /// Free-running step: advance `dphase` sub-beat units (`phase` is a u16
    /// binary turn — `65_536` = one beat); `beat` increments on phase wrap,
    /// wrapping itself by design (no replay guarantee attaches). `dphase = 0`
    /// is a pause; the driver owns rate and pause, the value stays 12 B.
    #[inline(always)]
    pub const fn advanced(self, dphase: u16) -> Self {
        let (phase, wrapped) = self.phase.overflowing_add(dphase);
        Self { beat: self.beat.wrapping_add(wrapped as u16), seed: self.seed, phase }
    }

    /// OKLCH cycling hue: `phase` read as the u16 binary-turn hue, the same
    /// scale as `OklchColor::h` (`forge_colour_v3::to_oklch`). Presentation
    /// only — never an input to sim state or a sealed record.
    #[inline(always)]
    pub const fn hue_turn16(self) -> u16 {
        self.phase
    }
}

// LAYOUT LOCKS. Measured with rustc, never hand-computed.
const _: () = assert!(core::mem::size_of::<ArchRole>() == 2);
const _: () = assert!(core::mem::size_of::<DetClock>() == 16);
const _: () = assert!(core::mem::align_of::<DetClock>() == 8);
const _: () = assert!(core::mem::size_of::<CreativeClock>() == 12);
const _: () = assert!(core::mem::align_of::<CreativeClock>() == 4);

// OFFSET LOCKS. `size_of` alone is a weak gate on both clocks: `DetClock` has 4 bytes
// of alignment tail, so widening `epoch` from `u16` to `u32` is still 16 bytes and the
// size assert stays green. Offsets close that hole — proven by sabotage, this session.
const _: () = assert!(core::mem::offset_of!(DetClock, tick) == 0);
const _: () = assert!(core::mem::offset_of!(DetClock, epoch) == 8);
const _: () = assert!(core::mem::offset_of!(DetClock, authority) == 10);
const _: () = assert!(core::mem::offset_of!(CreativeClock, beat) == 0);
const _: () = assert!(core::mem::offset_of!(CreativeClock, seed) == 4);
const _: () = assert!(core::mem::offset_of!(CreativeClock, phase) == 8);

#[cfg(test)]
mod tests {
    use super::*;

    /// The claim "`CreativeClock` is 8 B only if `u32` is first" is a measurement, so
    /// it is measured. If this ever stops being 8, the comment on `CreativeClock`
    /// is wrong and the compiler says so here.
    #[test]
    fn the_eight_byte_ordering() {
        #[repr(C)]
        struct SeedFirst {
            seed: u32,
            beat: u16,
            phase: u16,
        }
        assert_eq!(core::mem::size_of::<SeedFirst>(), 8);
        assert_eq!(core::mem::size_of::<CreativeClock>(), 12);
    }

    #[test]
    fn a_det_clock_holds_its_authority_in_the_padding_slot() {
        // 8 (tick) + 2 (epoch) + 2 (ArchRole) = 12 of 16 bytes used; the last 4 are
        // alignment tail. Storing authority inline is free — it costs no size.
        assert_eq!(core::mem::size_of::<DetClock>(), 16);
        assert_eq!(
            core::mem::size_of::<u64>()
                + core::mem::size_of::<u16>()
                + core::mem::size_of::<ArchRole>(),
            12
        );
    }

    #[test]
    fn only_arch000_is_prime_authority() {
        assert!(ArchRole::ARCH000.is_prime_authority());
        for n in 1u16..=1024 {
            assert!(!ArchRole(n).is_prime_authority(), "role {n} claimed Prime Authority");
        }
    }

    #[test]
    fn genesis_advances_and_orders_totally() {
        let a = DetClock::GENESIS;
        let b = a.next().unwrap();
        assert!(b > a);
        assert_eq!(b.tick, 1);
        assert_eq!(b.epoch, a.epoch);
        assert_eq!(b.authority, ArchRole::ARCH000);
    }

    // The bug this test exists for: a wrapping tick makes replay step 0 compare
    // equal to step 2^64, and the two histories become indistinguishable.
    #[test]
    fn an_exhausted_tick_refuses_to_wrap() {
        let end = DetClock { tick: u64::MAX, ..DetClock::GENESIS };
        assert!(end.next().is_none());
    }

    #[test]
    fn creative_phase_wraps_into_beat_and_zero_dphase_pauses() {
        let c = CreativeClock { beat: 0, seed: 7, phase: u16::MAX };
        let stepped = c.advanced(1);
        assert_eq!(stepped.phase, 0, "phase wraps at the beat boundary");
        assert_eq!(stepped.beat, 1, "beat increments on phase wrap");
        assert_eq!(stepped.seed, 7, "seed rides along untouched");
        assert_eq!(stepped.advanced(0), stepped, "dphase 0 is a pause, not a step");
    }

    #[test]
    fn creative_hue_is_the_phase_binary_turn() {
        let c = CreativeClock::GENESIS.advanced(32_768);
        assert_eq!(c.hue_turn16(), 32_768, "half a beat = half a hue turn");
        let mut d = CreativeClock::GENESIS;
        for _ in 0..4 {
            d = d.advanced(16_384);
        }
        assert_eq!(d.phase, 0, "four quarter-steps close the turn exactly");
        assert_eq!(d.beat, 1);
    }

    #[test]
    fn epoch_outranks_tick() {
        let old = DetClock { tick: u64::MAX, epoch: 0, authority: ArchRole::ARCH000 };
        let new = DetClock { tick: 0, epoch: 1, authority: ArchRole::ARCH000 };
        assert!(new > old, "epoch must dominate the ordering, or a reseed rewinds history");
    }

    #[test]
    fn ordering_agrees_with_equality() {
        let a = DetClock::GENESIS;
        let b = DetClock { authority: ArchRole(7), ..DetClock::GENESIS };
        assert_ne!(a, b);
        assert_ne!(a.cmp(&b), core::cmp::Ordering::Equal, "Ord must not call unequal clocks equal");
        assert_eq!(a.cmp(&a), core::cmp::Ordering::Equal);
    }
}
