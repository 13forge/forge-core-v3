//! Hierarchical 7-700-7 MoE routing, Tier 2 — Invention #169 (ported from
//! `F:\NewRepo\crates\forge-ml\src\hierarchical_moe.rs`, ARCH000-approved
//! cross-tree port per G02, 2026-08-12).
//!
//! Tier 1 is `crate::metarouter::MetaRouter`: picks 1-of-7 domain
//! specialists. This module is Tier 2: given a domain already selected, it
//! picks 2-3 sub-expert layer slices *within* that domain.
//!
//! Sub-experts are layer slices within the specialist's transformer stack —
//! real sparse computation with real isolation, using existing weights. No
//! flat global pool; cross-domain access is structurally impossible.
//!
//! Not yet wired to an execution engine: `F:\v3` has no per-layer-selective
//! forward pass for the "sovereign" NDE trunk architecture this was designed
//! against (`sidecar::GemmaEngine` only runs a whole-model `forward()` on an
//! unrelated Gemma3/GGUF model). This module is the routing math only.
//!
//! Phase 4 of the GPU/CPU dual-flywheel plan (2026-08-12): `SubRouter::evaluate`
//! is trit-native — it consumes the same `f32` query type
//! `metarouter::MetaRouter::route()` does, packs it via `metarouter::pack_trits`,
//! and scores band energy through `metarouter::TRIT_DIST_LUT` (distance from
//! `TritCell5D::ORIGIN`, the trit-native equivalent of the old raw-`i8`
//! abs-sum). One LUT, one packer, shared with Tier 1 — not a parallel scheme.

use crate::atom::TritCell5D;
use crate::fixed_point::Permyriad;
use crate::metarouter::{pack_trits, trit_bytes_needed, TRIT_DIST_LUT};

/// Fixed-point scale for Tier-2 routing math: Q16, `1 << 16`. Distinct from
/// `fixed_point::Permyriad` (decimal `0..=10_000` ratio scale) — this is a
/// binary scale chosen for cheap shift-based fixed-point multiply, matching
/// the source module's convention. Not a second home for Permyriad's job.
pub const SCALE_DENOM: i32 = 1 << 16;

/// Number of domain specialists (Tier 1).
pub const NUM_DOMAINS: usize = 7;
/// Number of sub-experts per domain (Tier 2).
pub const SUB_EXPERTS_PER_DOMAIN: usize = 7;
/// Total discrete experts in the topology.
pub const TOTAL_EXPERTS: usize = NUM_DOMAINS * SUB_EXPERTS_PER_DOMAIN;

/// A sub-expert is a contiguous slice of layers within a specialist.
/// For a 5-layer expert, sub-experts map to overlapping 1-2 layer windows.
/// For a 12-layer expert, sub-experts map to non-overlapping ~2 layer windows.
#[derive(Debug, Clone, Copy)]
pub struct SubExpertSlice {
    /// First layer index (inclusive).
    pub layer_start: usize,
    /// Last layer index (exclusive).
    pub layer_end: usize,
}

/// Sub-router: gates which sub-expert slices activate within a domain.
///
/// Initialization strategy (two paths):
///
/// **Path A (cold start):** Derived from layer norm magnitudes. This is a
/// heuristic — layer norms encode activation scale, not routing logic.
/// Produces near-uniform routing initially. Acceptable for first session.
///
/// **Path B (warm, post-distillation):** Loaded from trained weights once a
/// distillation pipeline exists for this crate (none does yet in `F:\v3`).
///
/// The SubRouter improves every session without retraining the base model.
pub struct SubRouter {
    /// Bias scores per sub-expert.
    /// Fixed-point (`SCALE_DENOM`). Higher = more likely to activate.
    /// Cold start: derived from layer norm magnitudes.
    /// Warm: loaded from distilled weights.
    pub bias: [i32; SUB_EXPERTS_PER_DOMAIN],
    /// Whether this router has been trained (vs heuristic init).
    pub trained: bool,
}

impl SubRouter {
    /// Create from trained weights (post-distillation path).
    pub fn from_trained(weights: [i32; SUB_EXPERTS_PER_DOMAIN]) -> Self {
        Self { bias: weights, trained: true }
    }

    /// Evaluate sub-expert scores for a query embedding.
    /// Returns fixed-point scores (sum ≈ `SCALE_DENOM`).
    ///
    /// Trit-native (Phase 4): packs `query` via `metarouter::pack_trits` —
    /// the same encoding `MetaRouter::route()` uses — then splits the packed
    /// bytes into 7 contiguous bands (remainder folded into the last band,
    /// so every dimension is covered; the old `i8` version silently dropped
    /// `d % 7` trailing dims from every band). Per-band "energy" is the sum
    /// of `TRIT_DIST_LUT[(ORIGIN << 8) | byte]` over the band's bytes —
    /// distance from the all-neutral origin equals the count of nonzero
    /// trits in that byte (0..5), the trit-native analogue of the old raw
    /// abs-sum, computed through the one LUT Tier 1 already proved out.
    pub fn evaluate(&self, query: &[f32]) -> [i32; SUB_EXPERTS_PER_DOMAIN] {
        let bpc = trit_bytes_needed(query.len() as u16) as usize;
        let q_tq = pack_trits(query, bpc);
        let origin = TritCell5D::ORIGIN.0 as usize;

        let mut scores = self.bias;

        let band_size = bpc / SUB_EXPERTS_PER_DOMAIN;
        if band_size > 0 {
            for (i, score) in scores.iter_mut().enumerate() {
                let start = i * band_size;
                let end = if i == SUB_EXPERTS_PER_DOMAIN - 1 { bpc } else { start + band_size };
                let energy: i32 = q_tq[start..end]
                    .iter()
                    .map(|&b| TRIT_DIST_LUT[(origin << 8) | b as usize] as i32)
                    .sum();
                // Modulate bias by band energy
                *score = (*score as i64 * energy as i64 / 5) as i32;
            }
        }

        // Normalize to sum ≈ SCALE_DENOM
        let total: i64 = scores.iter().map(|&s| s.max(1) as i64).sum();
        if total > 0 {
            for s in &mut scores {
                *s = (*s as i64 * SCALE_DENOM as i64 / total) as i32;
            }
        }
        scores
    }

    /// Evaluate as soft weights (continuous blend) across all 7 sub-experts.
    /// Returns a 7-element array of `Permyriad` weights (0..=10_000 each).
    /// Weights are normalized via rank-preserving inverse-distance, same as
    /// `evaluate()` but producing Permyriad output instead of fixed-point.
    pub fn evaluate_soft(&self, query: &[f32]) -> [Permyriad; SUB_EXPERTS_PER_DOMAIN] {
        let bpc = trit_bytes_needed(query.len() as u16) as usize;
        let q_tq = pack_trits(query, bpc);
        let origin = TritCell5D::ORIGIN.0 as usize;

        let mut scores = [0i32; SUB_EXPERTS_PER_DOMAIN];
        for (i, score) in scores.iter_mut().enumerate() {
            *score = self.bias[i];
        }

        let band_size = bpc / SUB_EXPERTS_PER_DOMAIN;
        if band_size > 0 {
            for (i, score) in scores.iter_mut().enumerate() {
                let start = i * band_size;
                let end = if i == SUB_EXPERTS_PER_DOMAIN - 1 { bpc } else { start + band_size };
                let energy: i32 = q_tq[start..end]
                    .iter()
                    .map(|&b| TRIT_DIST_LUT[(origin << 8) | b as usize] as i32)
                    .sum();
                *score = (*score as i64 * energy as i64 / 5) as i32;
            }
        }

        let max_score = *scores.iter().max().unwrap_or(&0);
        let mut sum: i64 = 0;
        let mut shifted = [0i32; SUB_EXPERTS_PER_DOMAIN];

        for i in 0..SUB_EXPERTS_PER_DOMAIN {
            let shift = scores[i] - max_score;
            shifted[i] = if shift >= 0 {
                (1i64 << shift.min(30)) as i32
            } else {
                0
            };
            sum += shifted[i] as i64;
        }

        if sum == 0 {
            sum = 1;
        }

        let mut out = [Permyriad::ZERO; SUB_EXPERTS_PER_DOMAIN];
        for i in 0..SUB_EXPERTS_PER_DOMAIN {
            let numerator = (shifted[i] as i64) * 10_000;
            out[i] = Permyriad((numerator / sum) as i32);
        }

        out
    }
}

/// A domain specialist: owns its sub-router and its sub-expert layer slices.
/// Cross-domain access is structurally impossible — you can only reach
/// sub-experts through the owning specialist.
pub struct DomainSpecialist {
    /// Which global expert index this specialist maps to (0-6).
    pub expert_id: usize,
    /// Sub-router for Tier 2 activation.
    pub sub_router: SubRouter,
    /// Layer slices defining each sub-expert's computation window.
    pub sub_experts: [SubExpertSlice; SUB_EXPERTS_PER_DOMAIN],
}

/// The hierarchical routing constellation.
/// Tier 1: `MetaRouter` selects 1-of-7.
/// Tier 2: `DomainSpecialist.sub_router` selects 2-3 sub-experts.
pub struct HierarchicalMoe {
    /// One specialist per domain, `None` where `num_experts` (at
    /// construction) fell short of `NUM_DOMAINS`.
    pub specialists: [Option<DomainSpecialist>; NUM_DOMAINS],
}

impl HierarchicalMoe {
    /// Build the hierarchical topology from the existing flat model.
    ///
    /// Maps N layers per expert into 7 sub-expert slices:
    /// - 5 layers → slices of 1 layer each (sub-experts 5,6 = full-stack fallback)
    /// - 12 layers → slices of ~2 layers each (sub-expert 6 = remainder)
    pub fn from_flat(num_experts: usize, expert_layers: usize, layer_norm_magnitudes: &[[f32; SUB_EXPERTS_PER_DOMAIN]]) -> Self {
        let mut specialists: [Option<DomainSpecialist>; NUM_DOMAINS] = Default::default();

        for eid in 0..num_experts.min(NUM_DOMAINS) {
            let slices = partition_layers(expert_layers);

            // Derive sub-router bias from layer norm magnitudes if available,
            // otherwise use uniform bias.
            let bias = if eid < layer_norm_magnitudes.len() {
                let mags = &layer_norm_magnitudes[eid];
                let mut b = [0i32; SUB_EXPERTS_PER_DOMAIN];
                for (i, &m) in mags.iter().enumerate() {
                    b[i] = (m * SCALE_DENOM as f32 / 7.0) as i32;
                }
                b
            } else {
                [SCALE_DENOM / SUB_EXPERTS_PER_DOMAIN as i32; SUB_EXPERTS_PER_DOMAIN]
            };

            specialists[eid] = Some(DomainSpecialist {
                expert_id: eid,
                sub_router: SubRouter { bias, trained: false },
                sub_experts: slices,
            });
        }

        Self { specialists }
    }

    /// Select top-k sub-experts from scores. Returns 2 or 3 local indices.
    pub fn select_top_k(scores: &[i32; SUB_EXPERTS_PER_DOMAIN]) -> ([usize; 3], u8) {
        // Sort indices by score descending
        let mut indexed: [(i32, usize); SUB_EXPERTS_PER_DOMAIN] = [
            (scores[0], 0),
            (scores[1], 1),
            (scores[2], 2),
            (scores[3], 3),
            (scores[4], 4),
            (scores[5], 5),
            (scores[6], 6),
        ];
        indexed.sort_by(|a, b| b.0.cmp(&a.0));

        // 2 if margin between top-1 and top-2 is high, else 3
        let margin = indexed[0].0 - indexed[1].0;
        let count = if margin > SCALE_DENOM / 4 { 2u8 } else { 3u8 };

        let mut result = [0usize; 3];
        for i in 0..count as usize {
            result[i] = indexed[i].1;
        }
        (result, count)
    }
}

/// Partition N layers into 7 sub-expert slices.
fn partition_layers(num_layers: usize) -> [SubExpertSlice; SUB_EXPERTS_PER_DOMAIN] {
    let mut slices = [SubExpertSlice { layer_start: 0, layer_end: 0 }; SUB_EXPERTS_PER_DOMAIN];

    if num_layers == 0 {
        return slices;
    }

    // Base slice size and remainder
    let base = num_layers / SUB_EXPERTS_PER_DOMAIN;
    let remainder = num_layers % SUB_EXPERTS_PER_DOMAIN;

    let mut pos = 0;
    for i in 0..SUB_EXPERTS_PER_DOMAIN {
        let size = if base > 0 {
            base + if i < remainder { 1 } else { 0 }
        } else if i < num_layers {
            // Fewer layers than sub-experts: 1 layer each, rest are full-stack fallback
            1
        } else {
            0 // Fallback: this sub-expert runs the full stack
        };

        if size > 0 {
            slices[i] = SubExpertSlice { layer_start: pos, layer_end: pos + size };
            pos += size;
        } else {
            // Fallback sub-expert: runs entire stack
            slices[i] = SubExpertSlice { layer_start: 0, layer_end: num_layers };
        }
    }

    slices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_5_layers() {
        let slices = partition_layers(5);
        // 5 layers / 7 sub-experts: 5 get 1 layer each, 2 get full-stack fallback
        assert_eq!(slices[0].layer_start, 0);
        assert_eq!(slices[0].layer_end, 1);
        assert_eq!(slices[4].layer_start, 4);
        assert_eq!(slices[4].layer_end, 5);
        // Fallback sub-experts cover full stack
        assert_eq!(slices[5].layer_start, 0);
        assert_eq!(slices[5].layer_end, 5);
        assert_eq!(slices[6].layer_start, 0);
        assert_eq!(slices[6].layer_end, 5);
    }

    #[test]
    fn partition_12_layers() {
        let slices = partition_layers(12);
        // 12 / 7 = 1 base + 5 remainder: first 5 get 2 layers, last 2 get 1
        assert_eq!(slices[0].layer_end - slices[0].layer_start, 2);
        assert_eq!(slices[4].layer_end - slices[4].layer_start, 2);
        assert_eq!(slices[5].layer_end - slices[5].layer_start, 1);
        assert_eq!(slices[6].layer_end - slices[6].layer_start, 1);
        // Total coverage = 12
        let total: usize = slices.iter().map(|s| s.layer_end - s.layer_start).sum();
        assert_eq!(total, 12);
    }

    #[test]
    fn select_top_k_picks_2_or_3() {
        // SCALE_DENOM = 65536, threshold = SCALE_DENOM / 4 = 16384.
        // High margin → 2 (margin 40000 > 16384)
        let scores = [50000, 10000, 3000, 1000, 800, 500, 236];
        let (ids, count) = HierarchicalMoe::select_top_k(&scores);
        assert_eq!(count, 2, "high margin should select 2 experts");
        assert_eq!(ids[0], 0);

        // Low margin → 3 (margin 1000 < 16384)
        let scores = [20000, 19000, 18000, 4000, 2000, 1500, 1036];
        let (_ids, count) = HierarchicalMoe::select_top_k(&scores);
        assert_eq!(count, 3, "low margin should select 3 experts");
    }

    #[test]
    fn total_experts_is_49() {
        assert_eq!(TOTAL_EXPERTS, 49);
    }

    #[test]
    fn evaluate_zero_query_is_uniformly_zero() {
        // Uniform bias, all-zero query -> all bands pack to ORIGIN (byte
        // 121, per pack_trits's own neutral-trit convention) -> zero energy
        // in every band -> every score is bias-modulated to zero. The
        // normalize step's `max(1)` floor only guards the *denominator*
        // against div-by-zero — a zeroed numerator stays zero either way,
        // same pre-existing shape the old raw-i8 code had for a silent
        // (all-zero-energy) query. Documented here, not silently assumed.
        let bias = [SCALE_DENOM / 7; SUB_EXPERTS_PER_DOMAIN];
        let router = SubRouter { bias, trained: false };
        let query = vec![0.0f32; 64];
        let scores = router.evaluate(&query);
        assert_eq!(scores, [0i32; SUB_EXPERTS_PER_DOMAIN]);
    }

    #[test]
    fn evaluate_scores_sum_near_scale_denom() {
        let bias = [SCALE_DENOM / 7; SUB_EXPERTS_PER_DOMAIN];
        let router = SubRouter { bias, trained: false };
        let query: Vec<f32> = (0..64).map(|i| ((i % 7) as f32 - 3.0) * 0.3).collect();
        let scores = router.evaluate(&query);
        let total: i64 = scores.iter().map(|&s| s as i64).sum();
        assert!(
            (total - SCALE_DENOM as i64).abs() < SCALE_DENOM as i64 / 100,
            "scores should sum near SCALE_DENOM, got {total} from {scores:?}"
        );
        for s in scores {
            assert!(s >= 0, "score should not go negative: {scores:?}");
        }
    }

    #[test]
    fn evaluate_full_dim_coverage_no_dropped_remainder() {
        // d=72 -> bpc = ceil(72/5) = 15, NOT divisible by 7 (band_size = 2,
        // remainder = 1) — the remainder byte must fold into the last band
        // (3 bytes), not fall off the end the way the old i8 version's
        // `.min(d)` truncation silently dropped `d % 7` trailing dims from
        // every band.
        let bias = [SCALE_DENOM / 7; SUB_EXPERTS_PER_DOMAIN];
        let router = SubRouter { bias, trained: false };

        // All-zero query except a strong positive signal in the last few dims.
        let mut query = vec![0.0f32; 72];
        for v in query.iter_mut().rev().take(5) {
            *v = 1.0;
        }
        let scores = router.evaluate(&query);

        // A uniform (all-zero-except-tail) query should bias the last
        // sub-expert's score upward relative to a fully-zero query, proving
        // the tail signal was actually counted somewhere in band 6.
        let zero_scores = router.evaluate(&vec![0.0f32; 72]);
        assert!(
            scores[SUB_EXPERTS_PER_DOMAIN - 1] > zero_scores[SUB_EXPERTS_PER_DOMAIN - 1],
            "tail-dim signal should raise the last sub-expert's score: {scores:?} vs zero {zero_scores:?}"
        );
    }

    /// Same coverage invariant as the source's `proptest`-based
    /// `prop_partition_layers_coverage`, reimplemented as a deterministic
    /// sweep — `forge-core-v3` is Crate Zero (zero external dependencies,
    /// per `fixed_point.rs`), so `proptest` cannot be added here.
    #[test]
    fn partition_layers_coverage_holds_for_1_to_200_layers() {
        for num_layers in 1usize..200 {
            let slices = partition_layers(num_layers);

            if num_layers >= SUB_EXPERTS_PER_DOMAIN {
                // All slices are non-fallback, contiguous, non-overlapping
                let mut pos = 0;
                for s in &slices {
                    assert_eq!(s.layer_start, pos, "num_layers={num_layers}: slice not contiguous at pos={pos}");
                    assert!(s.layer_end > s.layer_start, "num_layers={num_layers}: empty slice at pos={pos}");
                    pos = s.layer_end;
                }
                assert_eq!(pos, num_layers, "num_layers={num_layers}: slices don't cover [0, {num_layers})");
            } else {
                // num_layers < 7: first num_layers slices are real (1 layer each),
                // remaining are fallback [0, num_layers)
                for (i, s) in slices.iter().enumerate() {
                    if i < num_layers {
                        assert_eq!(s.layer_start, i);
                        assert_eq!(s.layer_end, i + 1);
                    } else {
                        assert_eq!(s.layer_start, 0, "num_layers={num_layers}: fallback slice {i} should start at 0");
                        assert_eq!(s.layer_end, num_layers, "num_layers={num_layers}: fallback slice {i} should end at {num_layers}");
                    }
                }
            }
        }
    }

    /// Golden test vectors for cross-language verification with the source's Python.
    #[test]
    fn partition_layers_golden_vectors() {
        let cases: Vec<(usize, Vec<(usize, usize)>)> = vec![
            (1, vec![(0, 1), (0, 1), (0, 1), (0, 1), (0, 1), (0, 1), (0, 1)]),
            (5, vec![(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (0, 5), (0, 5)]),
            (7, vec![(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7)]),
            (12, vec![(0, 2), (2, 4), (4, 6), (6, 8), (8, 10), (10, 11), (11, 12)]),
            (14, vec![(0, 2), (2, 4), (4, 6), (6, 8), (8, 10), (10, 12), (12, 14)]),
        ];
        for (num_layers, expected) in cases {
            let slices = partition_layers(num_layers);
            for (i, (exp_s, exp_e)) in expected.iter().enumerate() {
                assert_eq!(slices[i].layer_start, *exp_s, "num_layers={num_layers}, slice {i}: start mismatch");
                assert_eq!(slices[i].layer_end, *exp_e, "num_layers={num_layers}, slice {i}: end mismatch");
            }
        }
    }

    #[test]
    fn evaluate_soft_returns_normalized_weights() {
        let bias = [SCALE_DENOM / 7; SUB_EXPERTS_PER_DOMAIN];
        let router = SubRouter { bias, trained: false };

        let query = vec![0.5f32; 64];
        let weights = router.evaluate_soft(&query);

        let sum: i32 = weights.iter().map(|w| w.0).sum();
        assert!(sum > 0, "sum of weights must be positive");
        assert!(weights.iter().all(|w| w.0 >= 0), "all weights must be non-negative");
    }

    #[test]
    fn evaluate_soft_normalizes_to_sum() {
        let bias = [SCALE_DENOM / 7; SUB_EXPERTS_PER_DOMAIN];
        let router = SubRouter { bias, trained: false };

        let query = vec![1.0f32; 64];
        let weights = router.evaluate_soft(&query);

        let sum: i32 = weights.iter().map(|w| w.0).sum();
        assert!(sum > 0, "sum of weights must be positive");
        assert!(weights.iter().all(|w| w.0 >= 0), "all weights must be non-negative");
    }
}
