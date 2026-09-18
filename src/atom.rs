//! The 8-byte Pexil and its three field types. Sizes are asserted, never assumed —
//! every number here was measured with `rustc` before it was written down.
//!
//! `TritCell5D`'s 5-lane balanced-ternary shape is not incidental: `PARARITY.md`
//! (repo root) proves a lane can carry a balanced trit `{-1,0,+1}` iff its arity is
//! exactly 3 with one fixed point — no even-arity lane, at any width, can ever hold
//! one. Composing 5 such lanes (`3^5 = 243 <= 256`, one byte, true origin at
//! all-zero) is that law applied five times, not a byte-packing convenience.

use crate::sentinel::MAX_PACKED;

/// Trits per packed byte. `3^5 = 243 <= 256` is the whole reason the byte works.
pub const TRITS_PER_BYTE: usize = 5;
/// Balanced ternary. Not configurable — the sentinel envelope depends on it.
pub const RADIX: u8 = 3;

/// One 5-trit lattice address, radix-3 packed. `0..=242` interior, `243..=255` sentinel.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TritCell5D(/// The packed trit byte.
pub u8);

/// Kleene three-valued validity, one state per axis, packed in the same radix-3 byte.
/// Absence lives here, never in the lattice zero — `TritCell5D`'s zero means *both*.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValidityMask(/// The packed validity byte.
pub u8);

/// Intra-cell identity index. `u16` caps identity at 65_536 — widening breaks the
/// 8-byte face, so this ceiling is load-bearing, not incidental.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellOrdinal(/// The cell ordinal value.
pub u16);

/// The atom. 1 pexil = 8 bytes = 1/8th of an L1 line.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pexil {
    /// The 5-trit lattice address.
    pub lattice: TritCell5D,
    /// The Kleene three-valued validity mask.
    pub validity: ValidityMask,
    /// The intra-cell identity index.
    pub ordinal: CellOrdinal,
    /// The payload.
    pub payload: [u8; 4],
}

const _: () = assert!(core::mem::offset_of!(Pexil, lattice) == 0);
const _: () = assert!(core::mem::offset_of!(Pexil, validity) == 1);
const _: () = assert!(core::mem::offset_of!(Pexil, ordinal) == 2);
const _: () = assert!(core::mem::offset_of!(Pexil, payload) == 4);

/// Eight pexils, one cache line. `align(64)` is the guarantee — `[Pexil; 8]` alone is
/// 64 bytes at align 8 and can still straddle a line.
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PexilLine(/// The eight pexils.
pub [Pexil; 8]);

impl TritCell5D {
    /// The origin `(0,0,0,0,0)`: every digit at balanced zero. `1+3+9+27+81`.
    pub const ORIGIN: Self = Self(121);

    /// True when this byte is an out-of-band control state rather than a coordinate.
    #[inline(always)]
    pub const fn is_sentinel(self) -> bool {
        self.0 >= MAX_PACKED
    }
}

impl TritCell5D {
    /// Decode to five balanced trits, or `None` when this byte is a sentinel.
    /// Returning `Option` is the whole point: a sentinel that decoded to `[0;5]`
    /// would re-encode as 121 and silently break the bijection.
    #[inline(always)]
    pub const fn trits(self) -> Option<[i8; 5]> {
        if self.is_sentinel() {
            return None;
        }
        let b = self.0;
        Some([
            (b % 3) as i8 - 1,
            (b / 3 % 3) as i8 - 1,
            (b / 9 % 3) as i8 - 1,
            (b / 27 % 3) as i8 - 1,
            (b / 81 % 3) as i8 - 1,
        ])
    }

    /// Encode five balanced trits. Digits outside `-1..=1` are a programming fault.
    #[inline(always)]
    pub const fn from_trits(t: [i8; 5]) -> Self {
        debug_assert!(
            t[0] >= -1 && t[0] <= 1 && t[1] >= -1 && t[1] <= 1 && t[2] >= -1
                && t[2] <= 1 && t[3] >= -1 && t[3] <= 1 && t[4] >= -1 && t[4] <= 1
        );
        Self(
            (t[0] + 1) as u8
                + (t[1] + 1) as u8 * 3
                + (t[2] + 1) as u8 * 9
                + (t[3] + 1) as u8 * 27
                + (t[4] + 1) as u8 * 81,
        )
    }

    /// Substrate pillar, derived rather than stored (ARCH-020 §6, `lattice.0 % 5`).
    #[inline(always)]
    pub const fn essence(self) -> u8 {
        self.0 % 5
    }

    /// The pararity involution `PARARITY.md` §3 Corollary 2 proves this lane shape
    /// admits: negate all five trits (`digit d -> 2-d`). `None` for sentinels
    /// (`243..=255`), which stay out-of-band by construction, never a coordinate.
    #[inline(always)]
    pub const fn fold(self) -> Option<Self> {
        if let Some(t) = self.trits() {
            Some(Self::from_trits([-t[0], -t[1], -t[2], -t[3], -t[4]]))
        } else {
            None
        }
    }
}

impl ValidityMask {
    /// All five axes resolved.
    pub const ALL_KNOWN: Self = Self(242);
    /// All five axes unresolved — distinct from a lattice zero, which means *both*.
    pub const ALL_UNKNOWN: Self = Self(121);
}

// LAYOUT LOCKS. An agent that changes a field type or order fails `cargo check`,
// not review. These are the crate's actual contract.
const _: () = assert!(core::mem::size_of::<TritCell5D>() == 1);
const _: () = assert!(core::mem::size_of::<ValidityMask>() == 1);
const _: () = assert!(core::mem::size_of::<CellOrdinal>() == 2);
const _: () = assert!(core::mem::size_of::<Pexil>() == 8);
const _: () = assert!(core::mem::align_of::<Pexil>() == 8);
const _: () = assert!(core::mem::size_of::<PexilLine>() == 64);
const _: () = assert!(core::mem::align_of::<PexilLine>() == 64);
const _: () = assert!((RADIX as usize).pow(TRITS_PER_BYTE as u32) == 243);
const _: () = assert!((RADIX as usize).pow(TRITS_PER_BYTE as u32) <= 256);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_is_all_balanced_zero() {
        assert_eq!(TritCell5D::ORIGIN.0, 1 + 3 + 9 + 27 + 81);
        assert!(!TritCell5D::ORIGIN.is_sentinel());
    }

    #[test]
    fn the_sentinel_boundary_is_exactly_thirteen_states() {
        let sentinels = (0u16..=255).filter(|b| TritCell5D(*b as u8).is_sentinel()).count();
        assert_eq!(sentinels, 13, "256 - 243 = 13; the control envelope is not negotiable");
    }

    #[test]
    fn a_line_holds_eight_pexils_and_cannot_straddle() {
        assert_eq!(core::mem::size_of::<PexilLine>() / core::mem::size_of::<Pexil>(), 8);
        assert_eq!(core::mem::align_of::<PexilLine>(), 64);
    }

    // PARARITY.md §3 Corollary 2: fold() must be an involution over every interior
    // code, and its only fixed point must be the origin (all five trits at 0).
    #[test]
    fn fold_is_an_involution_with_fix_at_origin() {
        for code in 0u8..(MAX_PACKED) {
            let cell = TritCell5D(code);
            let folded = cell.fold().expect("interior code, never a sentinel");
            assert_eq!(folded.fold().expect("fold of an interior code stays interior"), cell, "f(f(x)) = x");
            if folded == cell {
                assert_eq!(cell, TritCell5D::ORIGIN, "the only fixed point must be the origin");
            }
        }
    }

    // Exhaustive bijection proof: 3^5 = 243 trits encode to [0..242] with zero collisions.
    // This is the foundation for 5D lattice traversal: every ray must uniquely address a cell.
    #[test]
    fn trit_cell_5d_bijection_exhaustive_all_243_states() {
        let mut index_seen = [false; 243];
        let mut encode_decode_pairs = Vec::new();

        for tx in -1i8..=1 {
            for ty in -1i8..=1 {
                for tz in -1i8..=1 {
                    for tt in -1i8..=1 {
                        for ts in -1i8..=1 {
                            let trits = [tx, ty, tz, tt, ts];
                            let cell = TritCell5D::from_trits(trits);

                            // Verify cell is not a sentinel (interior only).
                            assert!(
                                !cell.is_sentinel(),
                                "Trit combination {:?} unexpectedly encoded to sentinel {}",
                                trits,
                                cell.0
                            );

                            let idx = cell.0 as usize;
                            assert!(
                                idx < 243,
                                "Index out of bounds: trit {:?} encoded to {}",
                                trits,
                                idx
                            );

                            // Check for collision: two distinct trits mapping to same code.
                            assert!(
                                !index_seen[idx],
                                "Collision at index {}: trit {:?} conflicts with earlier encoding",
                                idx,
                                trits
                            );
                            index_seen[idx] = true;

                            // Verify decode round-trip (injectivity + surjectivity proof).
                            let decoded = cell
                                .trits()
                                .expect("interior cell, never a sentinel");
                            assert_eq!(
                                decoded, trits,
                                "Round-trip decode failed for {:?}: got {:?}",
                                trits, decoded
                            );

                            encode_decode_pairs.push((cell.0, trits));
                        }
                    }
                }
            }
        }

        // Verify surjectivity: every interior code [0..242] was visited exactly once.
        for (idx, &seen) in index_seen.iter().enumerate() {
            assert!(
                seen,
                "Bijection incomplete: interior index {} never visited",
                idx
            );
        }

        // Summary: 3^5 = 243 distinct trits map bijectively to [0..242].
        assert_eq!(
            encode_decode_pairs.len(),
            243,
            "Expected 243 unique mappings; got {}",
            encode_decode_pairs.len()
        );
    }
}
