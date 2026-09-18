//! S13: the `.s13` router-file aperture onto the shared out-of-band envelope.
//!
//! `crate::sentinel` owns the envelope itself — `MAX_PACKED`, `SENTINEL_COUNT`, the
//! `3^5 = 243` forcing — for the Pexil-cell domain. `S13` is the same coin (same
//! 243..=255 range, same forcing) read through the router-file domain: a byte here
//! means "out-of-band routing signal," not "out-of-band Pexil control." One envelope,
//! two apertures — this module must never redefine the arithmetic, only the labels.
//!
//! IDIOM-ECHO: `crates/forge-cart-brain-v3/src/state.rs`'s `EntityState.kind: u8`
//! independently reserves the top of its tag space (`KIND_RESERVED_START = 243`) —
//! same idiom, no shared value, no cross-crate coupling. Found via lateral-criticality;
//! S13 and EntityState.kind do not share arithmetic.

use crate::sentinel::MAX_PACKED;

/// The 13 out-of-band sentinel states in a 5D trit-packed `.s13` router byte.
/// Valid trits occupy `0..=242`. S13 occupies `243..=255`, same range as `Sentinel`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum S13 {
    /// 243: End of sequence / boundary marker
    Boundary = 243,
    /// 244: Instruction to drop/mask attention for this block
    MaskAttention = 244,
    /// 245: Marks a corrupted or intentionally poisoned tensor block
    Poisoned = 245,
    /// 246: Forces a fallback to the next inference tier (L2)
    TierFallback = 246,
    /// 247: Reserved sentinel state
    Reserved247 = 247,
    /// 248: Reserved sentinel state
    Reserved248 = 248,
    /// 249: Reserved sentinel state
    Reserved249 = 249,
    /// 250: Reserved sentinel state
    Reserved250 = 250,
    /// 251: Reserved sentinel state
    Reserved251 = 251,
    /// 252: Reserved sentinel state
    Reserved252 = 252,
    /// 253: Reserved sentinel state
    Reserved253 = 253,
    /// 254: Reserved sentinel state
    Reserved254 = 254,
    /// 255: Reserved sentinel state
    Reserved255 = 255,
}

impl S13 {
    /// Fast hardware-level check: is this byte a sentinel? Delegates to the shared
    /// envelope boundary (`sentinel::MAX_PACKED`) rather than re-deriving `243`.
    #[inline(always)]
    pub const fn is_sentinel(byte: u8) -> bool {
        byte >= MAX_PACKED
    }

    /// Decode a router-domain sentinel byte. Maps `243..=255` to the 13 named sentinels.
    #[inline(always)]
    pub const fn from_byte(b: u8) -> Option<Self> {
        match b {
            243 => Some(S13::Boundary),
            244 => Some(S13::MaskAttention),
            245 => Some(S13::Poisoned),
            246 => Some(S13::TierFallback),
            247 => Some(S13::Reserved247),
            248 => Some(S13::Reserved248),
            249 => Some(S13::Reserved249),
            250 => Some(S13::Reserved250),
            251 => Some(S13::Reserved251),
            252 => Some(S13::Reserved252),
            253 => Some(S13::Reserved253),
            254 => Some(S13::Reserved254),
            255 => Some(S13::Reserved255),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_sentinel_matches_shared_envelope_boundary() {
        assert!(!S13::is_sentinel(MAX_PACKED - 1));
        assert!(S13::is_sentinel(MAX_PACKED));
        assert!(S13::is_sentinel(255));
    }

    #[test]
    fn all_thirteen_sentinels_distinct() {
        let named = (MAX_PACKED..=255).filter(|&b| S13::from_byte(b).is_some()).count();
        assert_eq!(named, 13);
        for b in 243..=255 {
            assert!(S13::from_byte(b).is_some(), "byte {} should have a named sentinel", b);
        }
    }
}