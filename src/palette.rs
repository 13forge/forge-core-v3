//! The 16-state machine palette. Every match is a full 32-bit word compare —
//! channel predicates like `R==255 && G==0` also match `0xFF00FF` and are banned.

use crate::sentinel::{breach, MAX_PACKED, SENTINEL_COUNT};

/// One exact RGBA8 readback value. Compared as a word, never per channel.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineColor(/// The RGBA8 bytes.
pub [u8; 4]);

impl MachineColor {
    /// Build an opaque RGB color.
    #[inline(always)]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 0xFF])
    }

    /// Endian-independent packed form. `0xRRGGBBAA`. The only legal comparison.
    #[inline(always)]
    pub const fn to_u32(self) -> u32 {
        u32::from_be_bytes(self.0)
    }
}

/// Trit states. Index is `trit + 1`, so `-1 -> 0`, `0 -> 1`, `+1 -> 2`.
pub const TRIT_PALETTE: [MachineColor; 3] = [
    MachineColor::rgb(0xFF, 0x00, 0x00), // -1 NEG
    MachineColor::rgb(0x80, 0x80, 0x80), //  0 ZERO (Sovereign Anchor)
    MachineColor::rgb(0x00, 0xFF, 0x00), // +1 POS
];

/// The 13-state envelope, index `byte - 243`. Four named, nine reserved-but-distinct
/// so a reserved byte still round-trips through pixels before it is refused.
pub const SENTINEL_PALETTE: [MachineColor; SENTINEL_COUNT] = [
    MachineColor::rgb(0x00, 0x00, 0xFF), // 243 NullNode
    MachineColor::rgb(0xFF, 0x00, 0xFF), // 244 MersenneOverflow
    MachineColor::rgb(0xFF, 0xFF, 0x00), // 245 Tombstone
    MachineColor::rgb(0x00, 0xFF, 0xFF), // 246 BusIndirect
    MachineColor::rgb(0x00, 0x00, 0x00), // 247 reserved
    MachineColor::rgb(0xFF, 0xFF, 0xFF), // 248 reserved
    MachineColor::rgb(0x80, 0x00, 0x00), // 249 reserved
    MachineColor::rgb(0x00, 0x80, 0x00), // 250 reserved
    MachineColor::rgb(0x00, 0x00, 0x80), // 251 reserved
    MachineColor::rgb(0x80, 0x80, 0x00), // 252 reserved
    MachineColor::rgb(0x80, 0x00, 0x80), // 253 reserved
    MachineColor::rgb(0x00, 0x80, 0x80), // 254 reserved
    MachineColor::rgb(0xFF, 0x80, 0x00), // 255 reserved
];

/// Colour for one balanced trit.
#[inline(always)]
pub const fn trit_colour(t: i8) -> MachineColor {
    TRIT_PALETTE[(t + 1) as usize]
}

/// Colour for one out-of-band byte. Aborts if handed a coordinate.
#[inline(always)]
pub fn sentinel_colour(byte: u8) -> MachineColor {
    if byte < MAX_PACKED {
        breach("sentinel_colour on a coordinate byte", byte);
    }
    SENTINEL_PALETTE[(byte - MAX_PACKED) as usize]
}

/// Exact word match against the trit states. `None` means "not a trit colour".
#[inline(always)]
pub fn trit_of(c: MachineColor) -> Option<i8> {
    let w = c.to_u32();
    let mut i = 0;
    while i < 3 {
        if TRIT_PALETTE[i].to_u32() == w {
            return Some(i as i8 - 1);
        }
        i += 1;
    }
    None
}

/// Exact word match against the envelope. `None` means "not a sentinel colour".
#[inline(always)]
pub fn sentinel_byte_of(c: MachineColor) -> Option<u8> {
    let w = c.to_u32();
    let mut i = 0;
    while i < SENTINEL_COUNT {
        if SENTINEL_PALETTE[i].to_u32() == w {
            return Some(MAX_PACKED + i as u8);
        }
        i += 1;
    }
    None
}

const _: () = assert!(core::mem::size_of::<MachineColor>() == 4);
const _: () = assert!(TRIT_PALETTE.len() + SENTINEL_PALETTE.len() == 16);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_sixteen_entries_are_distinct() {
        let mut all: Vec<u32> = TRIT_PALETTE.iter().map(|c| c.to_u32()).collect();
        all.extend(SENTINEL_PALETTE.iter().map(|c| c.to_u32()));
        assert_eq!(all.len(), 16);
        let mut sorted = all.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 16, "palette collision: {all:?}");
    }

    #[test]
    fn every_channel_is_on_the_quantised_grid() {
        for c in TRIT_PALETTE.iter().chain(SENTINEL_PALETTE.iter()) {
            for ch in c.0 {
                assert!(matches!(ch, 0x00 | 0x80 | 0xFF), "off-grid channel {ch:#04X} in {c:?}");
            }
        }
    }

    // The bug this whole palette exists to prevent: two-channel predicates match
    // both a trit state and a sentinel. Word compare must keep them apart.
    #[test]
    fn word_compare_separates_neg_from_overflow_and_pos_from_bus() {
        let neg = trit_colour(-1);
        let overflow = sentinel_colour(244);
        assert_eq!(neg.0[0], overflow.0[0], "same R — a channel predicate would collide");
        assert_eq!(neg.0[1], overflow.0[1], "same G — a channel predicate would collide");
        assert_ne!(neg.to_u32(), overflow.to_u32(), "word compare must separate them");
        assert_eq!(trit_of(overflow), None, "a sentinel must never decode as a trit");

        let pos = trit_colour(1);
        let bus = sentinel_colour(246);
        assert_eq!(pos.0[0], bus.0[0]);
        assert_eq!(pos.0[1], bus.0[1]);
        assert_ne!(pos.to_u32(), bus.to_u32());
        assert_eq!(trit_of(bus), None);
    }

    #[test]
    fn round_trip_every_palette_entry() {
        for t in -1i8..=1 {
            assert_eq!(trit_of(trit_colour(t)), Some(t));
        }
        for b in MAX_PACKED..=255 {
            assert_eq!(sentinel_byte_of(sentinel_colour(b)), Some(b));
        }
    }

    /// The decode direction `pixels_to_point` actually depends on. It asks
    /// `sentinel_byte_of` **first** (`grid.rs:64`), so a trit colour that matched any
    /// envelope entry would be read as out-of-band and the coordinate lost — without
    /// ever reaching `trit_of`. The existing separation test covers the reverse
    /// direction only; this one closes the loop over all 3 x 13 pairs.
    #[test]
    fn no_trit_colour_decodes_as_a_sentinel_byte() {
        for t in -1i8..=1 {
            assert_eq!(
                sentinel_byte_of(trit_colour(t)),
                None,
                "trit {t} collides with the envelope and grid decode would take it first"
            );
        }
        for b in MAX_PACKED..=255 {
            assert_eq!(trit_of(sentinel_colour(b)), None, "sentinel {b} decodes as a trit");
        }
    }

    /// Both tables are total over their own domain, so neither decode can fall through
    /// to a default. 16 entries, 16 successful decodes, no overlap.
    #[test]
    fn the_two_tables_partition_the_sixteen_words() {
        let trits = (-1i8..=1).filter(|t| trit_of(trit_colour(*t)).is_some()).count();
        let sentinels =
            (MAX_PACKED..=255).filter(|b| sentinel_byte_of(sentinel_colour(*b)).is_some()).count();
        assert_eq!(trits, 3);
        assert_eq!(sentinels, SENTINEL_COUNT);
        assert_eq!(trits + sentinels, 16);
    }
}
