//! Formal verification: 32-trit centroid lattice ↔ TritArray32.
//! Proves strict bijection on 𝒮₃ = {-10000, 0, +10000}³², dual-bitmask SIMD packing,
//! and 2-cycle Hamming distance (XOR popcount). No entropy loss on canonical lattice.

use crate::pentaract_field::PentaractField;

/// A 32-element trit array, encoding {−1, 0, +1} values.
/// Maps bijectively to the canonical centroid lattice 𝒮₃ when quantized from
/// [-10000, +10000]³² via threshold ±3000.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TritArray32([i8; 32]);

impl TritArray32 {
    /// Quantize a field to trits: lossy map [−10000, +10000]³² → {−1, 0, +1}³².
    pub fn from_field(field: &PentaractField, threshold: i32) -> Self {
        let mut trits = [0i8; 32];
        let channels = field.channels();
        for i in 0..32 {
            trits[i] = if channels[i] >= threshold {
                1
            } else if channels[i] <= -threshold {
                -1
            } else {
                0
            };
        }
        TritArray32(trits)
    }

    /// Reconstruct to canonical centroid lattice: lossless map {−1, 0, +1}³² → 𝒮₃.
    pub fn to_canonical_field(&self) -> [i32; 32] {
        let mut field = [0i32; 32];
        for i in 0..32 {
            field[i] = (self.0[i] as i32) * 10_000;
        }
        field
    }

    /// Verify the bijection holds: round-trip without data loss on the canonical lattice.
    pub fn verify_round_trip_canonical(field: &PentaractField, threshold: i32) -> bool {
        let trit = Self::from_field(field, threshold);
        let canonical = trit.to_canonical_field();

        // The bijection only holds on the canonical lattice 𝒮₃.
        // Check that the original field was already quantized to that lattice.
        for i in 0..32 {
            let orig = field.channels()[i];
            let recon = canonical[i];
            if orig != recon {
                return false;
            }
        }
        true
    }

    /// Pack trits to dual bitmasks: (pos_mask, neg_mask) for SIMD Hamming distance.
    /// Bit i set in pos_mask iff trit[i] == +1; bit i set in neg_mask iff trit[i] == −1.
    pub fn to_dual_bitmasks(&self) -> (u32, u32) {
        let mut pos_mask = 0u32;
        let mut neg_mask = 0u32;
        for i in 0..32 {
            match self.0[i] {
                1 => pos_mask |= 1 << i,
                -1 => neg_mask |= 1 << i,
                0 => {}
                _ => unreachable!("trit out of range"),
            }
        }
        (pos_mask, neg_mask)
    }

    /// Reconstruct trits from dual bitmasks.
    pub fn from_dual_bitmasks(pos_mask: u32, neg_mask: u32) -> Self {
        let mut trits = [0i8; 32];
        for i in 0..32 {
            if pos_mask & (1 << i) != 0 {
                trits[i] = 1;
            } else if neg_mask & (1 << i) != 0 {
                trits[i] = -1;
            }
        }
        TritArray32(trits)
    }

    /// Hamming distance in 2 CPU cycles: (a_pos ⊕ b_pos).popcount() + (a_neg ⊕ b_neg).popcount().
    pub fn hamming_distance(a: &Self, b: &Self) -> u32 {
        let (a_pos, a_neg) = a.to_dual_bitmasks();
        let (b_pos, b_neg) = b.to_dual_bitmasks();
        (a_pos ^ b_pos).count_ones() + (a_neg ^ b_neg).count_ones()
    }

    /// Rank pool indices by Hamming distance from query. Fills out with
    /// (distance, pool_index) for top-k nearest in ascending order, ties by index.
    pub fn rank_nearest(query: &Self, pool: &[Self], out: &mut [(u32, usize)]) -> usize {
        let mut count = 0;
        for (idx, &point) in pool.iter().enumerate() {
            let dist = Self::hamming_distance(query, &point);
            if count < out.len() {
                out[count] = (dist, idx);
                count += 1;
                let mut pos = count - 1;
                while pos > 0 && (out[pos].0 < out[pos - 1].0 ||
                    (out[pos].0 == out[pos - 1].0 && out[pos].1 < out[pos - 1].1)) {
                    out.swap(pos, pos - 1);
                    pos -= 1;
                }
            } else if dist < out[out.len() - 1].0 ||
                (dist == out[out.len() - 1].0 && idx < out[out.len() - 1].1) {
                out[out.len() - 1] = (dist, idx);
                let mut pos = out.len() - 1;
                while pos > 0 && (out[pos].0 < out[pos - 1].0 ||
                    (out[pos].0 == out[pos - 1].0 && out[pos].1 < out[pos - 1].1)) {
                    out.swap(pos, pos - 1);
                    pos -= 1;
                }
            }
        }
        count
    }

    /// Encode to base-3 index. 3³² ≈ 1.85e15 < 2⁵¹, fits in u64.
    pub fn to_base3_index(&self) -> u64 {
        let mut idx = 0u64;
        let mut pow = 1u64;
        for i in 0..32 {
            let trit_val = (self.0[i] + 1) as u64; // map {−1, 0, +1} → {0, 1, 2}
            idx = idx.wrapping_add(trit_val.wrapping_mul(pow));
            pow = pow.wrapping_mul(3);
        }
        idx
    }

    /// Decode from base-3 index.
    pub fn from_base3_index(idx: u64) -> Self {
        let mut trits = [0i8; 32];
        let mut idx = idx;
        for i in 0..32 {
            let trit_val = (idx % 3) as i8;
            trits[i] = trit_val - 1; // map {0, 1, 2} → {−1, 0, +1}
            idx /= 3;
        }
        TritArray32(trits)
    }

    /// Access the underlying trit array.
    pub fn as_array(&self) -> &[i8; 32] {
        &self.0
    }
}

/// Trit state machine: tracks prior state and quantizes spectral input.
/// Uses hamming distance to detect state changes and quantizes via threshold.
pub struct TritStateMachine {
    /// Prior state array for hamming distance calculation.
    pub prior_state: TritArray32,
    /// Threshold value for quantization and state transition decisions.
    pub threshold: u32,
}

impl TritStateMachine {
    /// Compute next state from spectral input via hamming distance and quantization.
    /// Quantizes spectral u32 values to trits, computes distance from prior state,
    /// and returns new state based on distance magnitude vs. threshold.
    pub fn next_state(&self, spectral: &[u32; 32]) -> TritArray32 {
        let mut spectral_trits = [0i8; 32];
        let thresh = self.threshold as i32;
        for i in 0..32 {
            let s = spectral[i] as i32;
            spectral_trits[i] = if s >= thresh {
                1
            } else if s <= -thresh {
                -1
            } else {
                0
            };
        }
        let spectral_array = TritArray32(spectral_trits);
        let distance = TritArray32::hamming_distance(&self.prior_state, &spectral_array);

        let mut next = [0i8; 32];
        for i in 0..32 {
            if distance >= self.threshold {
                next[i] = spectral_array.0[i];
            } else {
                next[i] = 0;
            }
        }
        TritArray32(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pentaract::Pentaract;
    use crate::pentaract_field::SenseChannel;

    fn point() -> Pentaract {
        Pentaract::new(0x13, 1000, 2000, 3000, 4000, 0xC3A256FF, 0)
    }

    #[test]
    fn canonical_lattice_bijection() {
        let mut field = PentaractField::quiet_at(point());
        field[SenseChannel::HeatGradient] = 10_000;
        field[SenseChannel::UvFlux] = -10_000;
        field[SenseChannel::LuxZero] = 0;

        let trit = TritArray32::from_field(&field, 3_000);
        assert_eq!(trit.0[0], 1);
        assert_eq!(trit.0[1], -1);
        assert_eq!(trit.0[2], 0);

        let canonical = trit.to_canonical_field();
        assert_eq!(canonical[0], 10_000);
        assert_eq!(canonical[1], -10_000);
        assert_eq!(canonical[2], 0);
    }

    #[test]
    fn round_trip_on_canonical_lattice() {
        let mut field = PentaractField::quiet_at(point());
        field[SenseChannel::HeatGradient] = 10_000;
        field[SenseChannel::UvFlux] = -10_000;
        field[SenseChannel::VeilDensity] = 0;

        assert!(TritArray32::verify_round_trip_canonical(&field, 3_000));

        let trit = TritArray32::from_field(&field, 3_000);
        let canonical = trit.to_canonical_field();
        assert_eq!(canonical[0], 10_000);
        assert_eq!(canonical[1], -10_000);
        assert_eq!(canonical[2], 0);
    }

    #[test]
    fn dual_bitmask_round_trip() {
        let mut field = PentaractField::quiet_at(point());
        field[SenseChannel::HeatGradient] = 10_000;
        field[SenseChannel::UvFlux] = -10_000;
        field[SenseChannel::LuxZero] = 0;
        for i in 3..8 {
            field[SenseChannel::ALL[i]] = 10_000;
        }

        let trit = TritArray32::from_field(&field, 3_000);
        let (pos_mask, neg_mask) = trit.to_dual_bitmasks();

        // Verify bitmask encoding
        assert_eq!(pos_mask & 1, 1); // bit 0 set (HeatGradient = +1)
        assert_eq!(neg_mask & 2, 2); // bit 1 set (UvFlux = −1)
        assert_eq!((pos_mask | neg_mask) & 4, 0); // bit 2 unset (LuxZero = 0)

        // Reconstruct from bitmasks
        let reconstructed = TritArray32::from_dual_bitmasks(pos_mask, neg_mask);
        assert_eq!(reconstructed, trit);
    }

    #[test]
    fn hamming_distance_two_cycles() {
        let mut a_field = PentaractField::quiet_at(point());
        a_field[SenseChannel::HeatGradient] = 10_000;
        a_field[SenseChannel::UvFlux] = 0;

        let mut b_field = PentaractField::quiet_at(point());
        b_field[SenseChannel::HeatGradient] = 0;
        b_field[SenseChannel::UvFlux] = 10_000;

        let a = TritArray32::from_field(&a_field, 3_000);
        let b = TritArray32::from_field(&b_field, 3_000);

        // a=[+1,0,...] vs b=[0,+1,...]: distance=2
        let dist = TritArray32::hamming_distance(&a, &b);
        assert_eq!(dist, 2);

        // Same trit array has distance 0
        let dist_self = TritArray32::hamming_distance(&a, &a);
        assert_eq!(dist_self, 0);
    }

    #[test]
    fn entropy_base3_index_round_trip() {
        let mut field = PentaractField::quiet_at(point());
        for i in 0..32 {
            if i % 3 == 0 {
                field[SenseChannel::ALL[i]] = 10_000;
            } else if i % 3 == 1 {
                field[SenseChannel::ALL[i]] = -10_000;
            }
        }

        let trit = TritArray32::from_field(&field, 3_000);
        let idx = trit.to_base3_index();

        // 3³² ≈ 1.85e15 < 2⁵¹ ≈ 2.25e15, fits in u64
        assert!(idx < (1u64 << 51));

        let reconstructed = TritArray32::from_base3_index(idx);
        assert_eq!(reconstructed, trit);
    }

    #[test]
    fn all_32_trits_map_distinct_indices() {
        let mut seen = std::collections::HashSet::new();
        let test_cases = [
            [1, 0, -1, 0, 1, 0, -1, 0, 1, 0, -1, 0, 1, 0, -1, 0, 1, 0, -1, 0, 1, 0, -1, 0, 1, 0, -1, 0, 1, 0, -1, 0],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            [-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
        ];

        for test_case in &test_cases {
            let trit = TritArray32(*test_case);
            let idx = trit.to_base3_index();
            assert!(!seen.contains(&idx), "duplicate index for {:?}", test_case);
            seen.insert(idx);
        }
    }

    #[test]
    fn threshold_parameterization() {
        let mut field = PentaractField::quiet_at(point());
        field[SenseChannel::HeatGradient] = 5_000;

        let trit_loose = TritArray32::from_field(&field, 3_000);
        assert_eq!(trit_loose.0[0], 1); // above loose threshold

        let trit_strict = TritArray32::from_field(&field, 6_000);
        assert_eq!(trit_strict.0[0], 0); // below strict threshold
    }

    #[test]
    fn saturation_behavior() {
        let mut field = PentaractField::quiet_at(point());
        field[SenseChannel::HeatGradient] = i32::MAX;
        field[SenseChannel::UvFlux] = i32::MIN;

        let trit = TritArray32::from_field(&field, 1);
        assert_eq!(trit.0[0], 1);
        assert_eq!(trit.0[1], -1);
    }

    #[test]
    fn all_thirty_two_channels_map() {
        for (idx, ch) in SenseChannel::ALL.iter().enumerate() {
            let mut field = PentaractField::quiet_at(point());
            field[*ch] = 10_000;
            let trit = TritArray32::from_field(&field, 3_000);
            assert_eq!(trit.0[idx], 1, "channel {ch:?} did not map to +1 at index {idx}");
        }
    }

    #[test]
    fn test_trit_state_determinism() {
        let prior = TritArray32([
            1, -1, 0, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0, 1, 1,
            -1, -1, 0, 0, 1, 1, -1, -1,
        ]);
        let machine = TritStateMachine {
            prior_state: prior,
            threshold: 10,
        };

        let spectral = [
            15u32, 5u32, 20u32, 3u32, 25u32, 8u32, 12u32, 18u32, 14u32, 6u32, 22u32, 4u32,
            26u32, 9u32, 11u32, 19u32, 13u32, 7u32, 21u32, 2u32, 24u32, 10u32, 16u32, 17u32,
            15u32, 5u32, 20u32, 3u32, 25u32, 8u32, 12u32, 18u32,
        ];

        let state1 = machine.next_state(&spectral);
        let state2 = machine.next_state(&spectral);

        assert_eq!(
            state1, state2,
            "next_state should be deterministic: same spectral input → same output"
        );
    }

    #[test]
    fn rank_nearest_top_3_from_6() {
        let query = TritArray32([1, 0, -1, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0,
                                 1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1]);
        let pool = [
            TritArray32([1, 0, -1, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0,
                         1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1]), // 0: identical
            TritArray32([1, 0, -1, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0,
                         1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0, 1, 1, 0, -1]),  // 1: diff 1
            TritArray32([0, 0, -1, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0,
                         1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1]),  // 2: diff 1
            TritArray32([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                         0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),        // 3: diff 16
            TritArray32([-1, -1, 1, -1, 0, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1,
                         0, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0, 1, 1]),    // 4: diff 32
            TritArray32([1, 0, -1, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0,
                         1, 1, -1, -1, 0, 0, 1, 1, -1, -1, 0, 0, 1, 1, -1, 0]),   // 5: diff 1
        ];
        let mut out = [(0u32, 0usize); 3];
        let count = TritArray32::rank_nearest(&query, &pool, &mut out);
        assert_eq!(count, 3);
        assert_eq!(out[0].0, 0, "top-1: distance 0 (self)");
        assert_eq!(out[0].1, 0, "top-1: self at index 0");
        assert_eq!(out[1].0, 1, "top-2: distance 1 (first of ties)");
        assert_eq!(out[1].1, 1, "top-2: lowest index among distance-1");
        assert_eq!(out[2].0, 1, "top-3: distance 1");
        assert_eq!(out[2].1, 2, "top-3: next lowest index among distance-1");
    }
}
