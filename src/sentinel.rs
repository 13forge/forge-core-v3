//! The 13-state control envelope, `243..=255`. Four are named; nine are reserved and
//! abort on read so they cannot be squatted informally later.

/// First out-of-band byte. `3^5 = 243`, so everything below is a coordinate.
pub const MAX_PACKED: u8 = 243;
/// `256 - 243`. The size of the envelope, forced by base-3 meeting base-2.
pub const SENTINEL_COUNT: usize = 256 - MAX_PACKED as usize;

/// An out-of-band control state. Never a coordinate.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sentinel {
    /// Null node sentinel.
    NullNode = 243,
    /// Mersenne overflow sentinel.
    MersenneOverflow = 244,
    /// Tombstone sentinel.
    Tombstone = 245,
    /// Bus indirect sentinel.
    BusIndirect = 246,
}

impl Sentinel {
    /// Decode a byte in the envelope. `None` for `247..=255` — reserved, never valid.
    #[inline(always)]
    pub const fn from_byte(b: u8) -> Option<Self> {
        match b {
            243 => Some(Sentinel::NullNode),
            244 => Some(Sentinel::MersenneOverflow),
            245 => Some(Sentinel::Tombstone),
            246 => Some(Sentinel::BusIndirect),
            _ => None,
        }
    }
}

/// Loud halt. `abort`, not `panic` — an unwind could be caught and the breach swallowed.
/// Mirrors `outland::soulword::breach`, deliberately: one discipline, one verb.
#[cold]
#[inline(never)]
pub fn breach(what: &str, value: u8) -> ! {
    eprintln!("PEXIL BREACH: {what} @ byte={value}");
    std::process::abort()
}

// The envelope is exactly 13 wide, or the byte does not hold a 5-trit cell.
const _: () = assert!(SENTINEL_COUNT == 13);
const _: () = assert!(core::mem::size_of::<Sentinel>() == 1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_named_nine_reserved() {
        let named = (MAX_PACKED..=255).filter(|b| Sentinel::from_byte(*b).is_some()).count();
        let reserved = (MAX_PACKED..=255).filter(|b| Sentinel::from_byte(*b).is_none()).count();
        assert_eq!(named, 4);
        assert_eq!(reserved, 9);
        assert_eq!(named + reserved, SENTINEL_COUNT);
    }

    #[test]
    fn no_coordinate_byte_decodes_as_a_sentinel() {
        for b in 0..MAX_PACKED {
            assert!(Sentinel::from_byte(b).is_none(), "byte {b} is a coordinate, not control");
        }
    }
}
