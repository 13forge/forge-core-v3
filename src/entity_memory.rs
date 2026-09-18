//! Nemesis memory — append-only encounter record chain (P8 LEDGER primitive).
//!
//! Models NPC grudge/promotion-tier memory as a hash-chained sequence of
//! immutable records, reusing [`crate::soul::SoulWord`]'s parent-link mechanism
//! rather than inventing a new lineage format. One `EntityMemoryRecord` per
//! encounter, linked via u32 truncated-hash references (SOUL_BYTES identity).

use crate::soul::{SoulWord, content_hash_fnv1a, seal_soulword};

/// One encounter record: entity_id, tick, grudge_delta, parent, promotion_tier, reserved.
///
/// Minimal shape: enough to walk a chain of encounters and recover
/// entity state deltas (grudge changes, tier promotions) across session boundaries.
/// The wire payload serialized by [`to_soulword`](Self::to_soulword)/
/// [`from_soulword`](Self::from_soulword) is 28 bytes (u64 + u64 + i32 + u32 + u8 +
/// [u8; 3]) field-by-field — `repr(C)` in-memory layout pads that to 32 bytes
/// (u64 alignment), which is what `size_of` reports below.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityMemoryRecord {
    /// Entity this record belongs to (u64).
    pub entity_id: u64,
    /// World tick when encounter ended.
    pub encounter_tick: u64,
    /// Grudge change this encounter inflicted (permyriad delta, signed).
    pub grudge_delta: i32,
    /// Causal parent's truncated hash. `0` at chain head (SoulId::ROOT convention).
    pub parent: u32,
    /// Promotion tier after encounter (0-5, or sentinel).
    pub promotion_tier: u8,
    /// Reserved, must be zero.
    pub _reserved: [u8; 3],
}

const _: () = assert!(core::mem::size_of::<EntityMemoryRecord>() == 32);

impl EntityMemoryRecord {
    /// Seal a record as a SoulWord for chaining. Hash covers all fields.
    pub fn to_soulword(&self) -> Result<SoulWord, String> {
        let mut payload = [0u8; 28];
        payload[0..8].copy_from_slice(&self.entity_id.to_le_bytes());
        payload[8..16].copy_from_slice(&self.encounter_tick.to_le_bytes());
        payload[16..20].copy_from_slice(&self.grudge_delta.to_le_bytes());
        payload[20..24].copy_from_slice(&self.parent.to_le_bytes());
        payload[24] = self.promotion_tier;
        payload[25..28].copy_from_slice(&self._reserved);
        let hash = content_hash_fnv1a(&payload);
        seal_soulword(hash, &payload[..], self.parent)
    }

    /// Decode a record from a SoulWord's trit payload (reverse of `to_soulword`).
    /// Returns `None` if payload is malformed (wrong size or invalid fields).
    pub fn from_soulword(word: &SoulWord) -> Option<Self> {
        if word.trits.len() < 28 {
            return None;
        }
        let entity_id = u64::from_le_bytes([
            word.trits[0], word.trits[1], word.trits[2], word.trits[3],
            word.trits[4], word.trits[5], word.trits[6], word.trits[7],
        ]);
        let encounter_tick = u64::from_le_bytes([
            word.trits[8], word.trits[9], word.trits[10], word.trits[11],
            word.trits[12], word.trits[13], word.trits[14], word.trits[15],
        ]);
        let grudge_delta = i32::from_le_bytes([
            word.trits[16], word.trits[17], word.trits[18], word.trits[19],
        ]);
        let parent = u32::from_le_bytes([
            word.trits[20], word.trits[21], word.trits[22], word.trits[23],
        ]);
        let promotion_tier = word.trits[24];
        let _reserved = [word.trits[25], word.trits[26], word.trits[27]];
        if _reserved != [0, 0, 0] {
            return None;
        }
        Some(Self { entity_id, encounter_tick, grudge_delta, promotion_tier, parent, _reserved })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::truncate_hash_ref;

    #[test]
    fn record_roundtrips_through_soulword() {
        let rec1 = EntityMemoryRecord {
            entity_id: 1,
            encounter_tick: 42,
            grudge_delta: 10,
            promotion_tier: 2,
            parent: 0,
            _reserved: [0, 0, 0],
        };

        let word = rec1.to_soulword().expect("to_soulword failed");
        assert!(word.is_well_formed(), "word must be well-formed");

        let rec2 = EntityMemoryRecord::from_soulword(&word).expect("from_soulword failed");
        assert_eq!(rec1, rec2, "roundtrip must preserve record");
    }

    #[test]
    fn chain_three_records_and_walk() {
        let rec1 = EntityMemoryRecord {
            entity_id: 1,
            encounter_tick: 10,
            grudge_delta: 100,
            promotion_tier: 0,
            parent: 0,
            _reserved: [0, 0, 0],
        };
        let word1 = rec1.to_soulword().expect("word1 failed");
        let parent1 = truncate_hash_ref(word1.hash);

        let rec2 = EntityMemoryRecord {
            entity_id: 1,
            encounter_tick: 20,
            grudge_delta: 200,
            promotion_tier: 1,
            parent: parent1,
            _reserved: [0, 0, 0],
        };
        let word2 = rec2.to_soulword().expect("word2 failed");
        let parent2 = truncate_hash_ref(word2.hash);

        let rec3 = EntityMemoryRecord {
            entity_id: 1,
            encounter_tick: 30,
            grudge_delta: 150,
            promotion_tier: 2,
            parent: parent2,
            _reserved: [0, 0, 0],
        };
        let word3 = rec3.to_soulword().expect("word3 failed");

        assert!(word1.is_well_formed(), "word1 must be well-formed");
        assert!(word2.is_well_formed(), "word2 must be well-formed");
        assert!(word3.is_well_formed(), "word3 must be well-formed");

        let decoded3 = EntityMemoryRecord::from_soulword(&word3).expect("decode rec3");
        assert_eq!(decoded3.entity_id, 1);
        assert_eq!(decoded3.encounter_tick, 30);
        assert_eq!(decoded3.grudge_delta, 150);
        assert_eq!(decoded3.promotion_tier, 2);
        assert_eq!(decoded3.parent, parent2);

        let decoded2 = EntityMemoryRecord::from_soulword(&word2).expect("decode rec2");
        assert_eq!(decoded2.parent, parent1);

        let decoded1 = EntityMemoryRecord::from_soulword(&word1).expect("decode rec1");
        assert_eq!(decoded1.parent, 0);
    }

    #[test]
    fn malformed_soulword_returns_none() {
        let mut word_bad_reserved = SoulWord { hash: 42, parent: 0, trits: [0u8; 52] };
        word_bad_reserved.trits[25] = 1;
        assert!(
            EntityMemoryRecord::from_soulword(&word_bad_reserved).is_none(),
            "non-zero reserved should return None"
        );

        let mut word_bad_reserved_2 = SoulWord { hash: 42, parent: 0, trits: [0u8; 52] };
        word_bad_reserved_2.trits[26] = 5;
        assert!(
            EntityMemoryRecord::from_soulword(&word_bad_reserved_2).is_none(),
            "any non-zero reserved byte should return None"
        );
    }
}
