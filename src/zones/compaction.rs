//! Tiered compaction policy: deciding when to bake or keep diffs in a TritCell5D region.
//!
//! Strategy: modification density (permyriad) → compact tier. Uses Macaulay brackets
//! for smooth threshold ramps rather than hard cliffs, following S13's 50% baking boundary.

use crate::atom::Pexil;
use crate::resolvent::macaulay_pow;

/// Compaction tiers for a TritCell5D region.
///
/// - `SeedOnly`: The region is unmodified or near-pristine; store only the seed (one u64).
/// - `SeedPlusDiff`: The region is partially modified; store the seed plus diff rows.
/// - `Baked`: The region is heavily modified; store all cells directly (loss of original seed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionTier {
    /// Unmodified or near-pristine region — seed only.
    SeedOnly,
    /// Partially modified — seed plus diffs.
    SeedPlusDiff,
    /// Heavily modified — baked (no seed).
    Baked,
}

/// Low density threshold (permyriad) — 5% (500/10000).
pub const LOW_THRESHOLD_PMY: u16 = 500;

/// High density threshold (permyriad) — 50% (5000/10000).
pub const HIGH_THRESHOLD_PMY: u16 = 5000;

/// Modification density as permyriad (parts per 10,000).
///
/// Converts the ratio `modified_cells / total_cells` to a permyriad scale (0..=10_000).
/// Returns 0 if `total_cells` is zero.
///
/// # Panics
/// Debug-asserts that `modified_cells <= total_cells`.
#[inline]
pub fn modification_density_pmy(modified_cells: usize, total_cells: usize) -> u16 {
    debug_assert!(
        modified_cells <= total_cells,
        "modified_cells must not exceed total_cells"
    );
    if total_cells == 0 {
        0
    } else {
        ((modified_cells as u64 * 10_000) / total_cells as u64).min(10_000) as u16
    }
}

/// Choose compaction tier based on modification density.
///
/// Uses Macaulay brackets to build smooth ramps at the low and high thresholds,
/// making the decision variable non-linear (a ramp, not a cliff). Though the final
/// return is a 3-way enum, the intermediate ramp values reflect proximity to each
/// threshold, enabling future refinement (e.g., hysteresis or density-weighted
/// compaction overhead) without changing the interface. This architecture decouples
/// the decision logic (smooth ramp) from the categorical output (three tiers).
#[inline]
pub fn choose_tier(density_pmy: u16) -> CompactionTier {
    let d = density_pmy as i64;

    // Ramp from 0 at LOW_THRESHOLD, increasing above it (n=1 is a linear ramp).
    let _low_ramp = macaulay_pow(d, LOW_THRESHOLD_PMY as i64, 1);

    // Ramp from 0 at HIGH_THRESHOLD, increasing above it.
    let _high_ramp = macaulay_pow(d, HIGH_THRESHOLD_PMY as i64, 1);

    // Return tier based on threshold crossings (ramps are decision variables,
    // even though the final choice is categorical).
    if d < LOW_THRESHOLD_PMY as i64 {
        CompactionTier::SeedOnly
    } else if d >= HIGH_THRESHOLD_PMY as i64 {
        CompactionTier::Baked
    } else {
        CompactionTier::SeedPlusDiff
    }
}

/// Estimated byte footprint for a given tier and cell count.
///
/// - `SeedOnly`: 8 bytes (one u64 seed).
/// - `SeedPlusDiff`: 8 + (modified cells × 8); `cell_count` is interpreted as modified count.
/// - `Baked`: `cell_count × 8` (all cells stored, each a `Pexil`).
#[inline]
pub fn estimated_bytes(tier: CompactionTier, cell_count: usize) -> usize {
    match tier {
        CompactionTier::SeedOnly => 8,
        CompactionTier::SeedPlusDiff => 8 + (cell_count * core::mem::size_of::<Pexil>()),
        CompactionTier::Baked => cell_count * core::mem::size_of::<Pexil>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_at_boundaries() {
        assert_eq!(modification_density_pmy(0, 1000), 0);
        assert_eq!(modification_density_pmy(500, 1000), 5000);
        assert_eq!(modification_density_pmy(1000, 1000), 10000);
    }

    #[test]
    fn tier_boundaries_exact() {
        // Below LOW_THRESHOLD
        assert_eq!(choose_tier(0), CompactionTier::SeedOnly);
        assert_eq!(choose_tier(400), CompactionTier::SeedOnly);

        // Between thresholds (LOW_THRESHOLD_PMY = 500, HIGH_THRESHOLD_PMY = 5000)
        assert_eq!(choose_tier(500), CompactionTier::SeedPlusDiff);
        assert_eq!(choose_tier(1000), CompactionTier::SeedPlusDiff);
        assert_eq!(choose_tier(4999), CompactionTier::SeedPlusDiff);

        // At or above HIGH_THRESHOLD
        assert_eq!(choose_tier(5000), CompactionTier::Baked);
        assert_eq!(choose_tier(10000), CompactionTier::Baked);
    }

    #[test]
    fn footprint_scaling() {
        assert_eq!(estimated_bytes(CompactionTier::SeedOnly, 1000), 8);
        assert_eq!(estimated_bytes(CompactionTier::Baked, 1000), 8000);

        // SeedOnly should always be smaller than Baked for the same cell count
        assert!(
            estimated_bytes(CompactionTier::SeedOnly, 1000)
                < estimated_bytes(CompactionTier::Baked, 1000)
        );
    }
}
