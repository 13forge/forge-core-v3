//! The proof ladder and one row of the unified stack.
//!
//! `ProofState` has **four** states, not three. `Authored` already shipped into
//! `forge-book` (`crates/forge-book/src/unified_stack.rs`, HANDOFF §3); v3 matches its
//! discriminants exactly or the two ladders diverge on day one and no tally can be
//! compared across them.

/// How well a claim is held. Discriminants are wire-compatible with `forge-book`'s
/// `Proof` — the numbering is a contract between two crates, not a local choice.
///
/// Deliberately **not** `Ord`. `Authored = 3` is the highest discriminant but it is not
/// the strongest proof; it is a *different kind* of claim — one that carries human
/// authorship instead of a machine anchor. Deriving `Ord` would quietly assert that
/// `Authored > Proven`, which is exactly the confusion this enum exists to prevent.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProofState {
    /// No proof yet.
    Unproven = 0,
    /// An estimate or approximation.
    Estimate = 1,
    /// Proven by measurement or calculation.
    Proven = 2,
    /// Authored doctrine, exempt from machine anchors.
    Authored = 3,
}

/// The ladder is closed. Every tally is 4-wide; a fifth state breaks `tally()`.
pub const PROOF_STATES: usize = 4;

impl ProofState {
    /// Every state, in discriminant order. `tally()`'s index space.
    pub const ALL: [ProofState; PROOF_STATES] = [
        ProofState::Unproven,
        ProofState::Estimate,
        ProofState::Proven,
        ProofState::Authored,
    ];

    /// May this claim stand without a machine anchor? `Authored` only (CLAUDE.md L12).
    /// This is the whole reason the fourth state exists: authored doctrine has no
    /// `file:line` to point at, and forcing one produces a fabricated receipt.
    #[inline(always)]
    pub const fn exempt_from_anchor(self) -> bool {
        matches!(self, ProofState::Authored)
    }

    /// Tally index. `as u8` on the discriminant, named so callers do not cast inline.
    #[inline(always)]
    pub const fn rung(self) -> u8 {
        self as u8
    }

    /// Decode a stored discriminant. `None` outside `0..=3` — a fifth value is not a
    /// new state, it is corruption of a persisted row.
    #[inline(always)]
    pub const fn from_u8(b: u8) -> Option<Self> {
        match b {
            0 => Some(ProofState::Unproven),
            1 => Some(ProofState::Estimate),
            2 => Some(ProofState::Proven),
            3 => Some(ProofState::Authored),
            _ => None,
        }
    }
}

/// One row of the unified stack: what layer, what it claims, where the proof lives.
///
/// 56 B — three fat pointers (48) + one tag + 7 pad. This size was stated wrong in
/// three successive drafts before it was measured. Interning the three strings to `u32`
/// arena offsets would give 16 B and 4 rows per cache line instead of 1; that is
/// HANDOFF §9.2 and is **not decided**, so the fat-pointer layout is pinned as measured.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StackRow {
    /// Which layer of the stack this row belongs to.
    pub layer: &'static str,
    /// What the row claims.
    pub spec: &'static str,
    /// Where the claim is held — `file:line` or a measurement. Empty only when
    /// `proof` is anchor-exempt.
    pub anchor: &'static str,
    /// How well it is held.
    pub proof: ProofState,
    /// Explicit tail. Named, not hand-sized by eye: the offset locks below prove it
    /// occupies 49..56 and the compiler rejects any other width.
    pub _pad: [u8; 7],
}

impl StackRow {
    /// Build a row. `_pad` is zeroed here so no caller ever writes it.
    #[inline]
    pub const fn new(
        layer: &'static str,
        spec: &'static str,
        anchor: &'static str,
        proof: ProofState,
    ) -> Self {
        Self { layer, spec, anchor, proof, _pad: [0; 7] }
    }

    /// True when this row points at something.
    #[inline(always)]
    pub const fn is_anchored(&self) -> bool {
        !self.anchor.is_empty()
    }

    /// A row is well-formed when it is anchored, or exempt from needing one.
    /// An unanchored `Unproven` row is the defect this predicate exists to catch:
    /// it claims nothing and proves nothing, and it inflates the tally denominator.
    #[inline(always)]
    pub const fn is_well_formed(&self) -> bool {
        self.is_anchored() || self.proof.exempt_from_anchor()
    }
}

/// Count rows per rung. Four counts, matching `forge-book`'s 4-tuple `tally()`.
/// Indexed by `ProofState::rung()`, so `[Unproven, Estimate, Proven, Authored]`.
pub fn tally(rows: &[StackRow]) -> [usize; PROOF_STATES] {
    let mut counts = [0usize; PROOF_STATES];
    for row in rows {
        counts[row.proof.rung() as usize] += 1;
    }
    counts
}

// LAYOUT LOCKS. A `&str` is two words: 16 B on 64-bit, 8 B on wasm32 — both pinned.
const _: () = assert!(core::mem::size_of::<ProofState>() == 1);
const _: () = assert!(core::mem::offset_of!(StackRow, layer) == 0);
const _: () = assert!(ProofState::ALL.len() == PROOF_STATES);

// OFFSET LOCKS. `size_of` alone is a weak gate — `StackRow` carries 7 bytes of tail, so
// widening the tag alone stays at 56 and a size-only assert would not fire.
#[cfg(target_pointer_width = "64")]
mod layout_64 {
    use super::StackRow;
    const _: () = assert!(core::mem::size_of::<StackRow>() == 56);
    const _: () = assert!(core::mem::align_of::<StackRow>() == 8);
    const _: () = assert!(core::mem::offset_of!(StackRow, spec) == 16);
    const _: () = assert!(core::mem::offset_of!(StackRow, anchor) == 32);
    const _: () = assert!(core::mem::offset_of!(StackRow, proof) == 48);
    const _: () = assert!(core::mem::offset_of!(StackRow, _pad) == 49);
    // Three fat pointers, and the row is exactly them plus the tag word.
    const _: () = assert!(core::mem::size_of::<&'static str>() == 16);
    const _: () = assert!(3 * core::mem::size_of::<&'static str>() == 48);
}
#[cfg(target_pointer_width = "32")]
mod layout_32 {
    use super::StackRow;
    const _: () = assert!(core::mem::size_of::<StackRow>() == 32);
    const _: () = assert!(core::mem::align_of::<StackRow>() == 4);
    const _: () = assert!(core::mem::offset_of!(StackRow, spec) == 8);
    const _: () = assert!(core::mem::offset_of!(StackRow, anchor) == 16);
    const _: () = assert!(core::mem::offset_of!(StackRow, proof) == 24);
    const _: () = assert!(core::mem::offset_of!(StackRow, _pad) == 25);
    const _: () = assert!(core::mem::size_of::<&'static str>() == 8);
    const _: () = assert!(3 * core::mem::size_of::<&'static str>() == 24);
}
const _: () = assert!(ProofState::ALL.len() == PROOF_STATES);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_has_four_states_and_forge_book_numbering() {
        assert_eq!(PROOF_STATES, 4);
        assert_eq!(ProofState::Unproven.rung(), 0);
        assert_eq!(ProofState::Estimate.rung(), 1);
        assert_eq!(ProofState::Proven.rung(), 2);
        assert_eq!(ProofState::Authored.rung(), 3);
    }

    #[test]
    fn every_rung_decodes_and_nothing_else_does() {
        for (i, state) in ProofState::ALL.iter().enumerate() {
            assert_eq!(ProofState::from_u8(i as u8), Some(*state));
            assert_eq!(state.rung() as usize, i);
        }
        for b in PROOF_STATES as u8..=255 {
            assert!(ProofState::from_u8(b).is_none(), "byte {b} is corruption, not a state");
        }
    }

    // CLAUDE.md L12: Authored is anchor-exempt, never Unproven.
    #[test]
    fn only_authored_is_anchor_exempt() {
        assert!(ProofState::Authored.exempt_from_anchor());
        for state in ProofState::ALL {
            if state != ProofState::Authored {
                assert!(!state.exempt_from_anchor(), "{state:?} must carry an anchor");
            }
        }
        assert!(!ProofState::Unproven.exempt_from_anchor());
    }

    #[test]
    fn an_unanchored_row_is_malformed_unless_authored() {
        let authored = StackRow::new("doctrine", "13 is forced three ways", "", ProofState::Authored);
        assert!(!authored.is_anchored());
        assert!(authored.is_well_formed());

        for state in ProofState::ALL {
            let row = StackRow::new("atom", "Pexil is 8 bytes", "", state);
            assert_eq!(
                row.is_well_formed(),
                state == ProofState::Authored,
                "{state:?} without an anchor"
            );
        }
    }

    #[test]
    fn an_anchored_row_is_well_formed_on_every_rung() {
        for state in ProofState::ALL {
            let row = StackRow::new("atom", "Pexil is 8 bytes", "src/atom.rs:109", state);
            assert!(row.is_anchored());
            assert!(row.is_well_formed());
        }
    }

    #[test]
    fn tally_is_four_wide_and_totals_the_input() {
        let rows = [
            StackRow::new("a", "s", "src/atom.rs:109", ProofState::Proven),
            StackRow::new("b", "s", "src/grid.rs:87", ProofState::Proven),
            StackRow::new("c", "s", "measured", ProofState::Estimate),
            StackRow::new("d", "s", "", ProofState::Authored),
            StackRow::new("e", "s", "none yet", ProofState::Unproven),
        ];
        let counts = tally(&rows);
        assert_eq!(counts, [1, 1, 2, 1]);
        assert_eq!(counts.iter().sum::<usize>(), rows.len());
        assert_eq!(counts.len(), PROOF_STATES);
    }

    #[test]
    fn tally_of_nothing_is_four_zeroes() {
        assert_eq!(tally(&[]), [0, 0, 0, 0]);
    }

    #[test]
    fn a_row_is_three_fat_pointers_and_a_tag() {
        assert_eq!(core::mem::size_of::<StackRow>(), 56);
        assert_eq!(3 * core::mem::size_of::<&'static str>() + 1 + 7, 56);
        // One row per cache line, which is why HANDOFF §9.2 is still open.
        assert!(core::mem::size_of::<StackRow>() > 32);
    }

    #[test]
    fn new_zeroes_the_tail() {
        let row = StackRow::new("a", "s", "src/lib.rs:25", ProofState::Proven);
        assert_eq!(row._pad, [0; 7]);
    }
}
