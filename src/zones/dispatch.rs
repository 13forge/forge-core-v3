//! Zone dispatch — thin wrapper binding a `TritCell5D` lattice address to
//! `MetaRouter`'s 1-of-7 MoE routing.

use crate::atom::TritCell5D;
use crate::metarouter::MetaRouter;

/// Routes a 5D lattice cell to the best expert using a loaded `MetaRouter`.
///
/// Unpacks the `TritCell5D`'s 5 balanced trits, converts them to f32 values,
/// pads to the router's `d_model` dimensions with zeros, and invokes the
/// router's real `route()` method.
///
/// # Arguments
/// * `router` — pre-loaded `MetaRouter`, must have `d_model >= 5`.
/// * `cell` — a `TritCell5D` lattice address (5 balanced trits packed in one u8).
///
/// # Returns
/// * `Ok((expert_id, margin))` — routing succeeded; expert_id is 0..6, margin is
///   the top-1 minus top-2 score.
/// * `Err(byte)` — a sentinel byte was encountered in the router's centroids
///   during distance computation; decode via `S13::from_byte`.
///
/// # Panics
/// Panics if the cell is a sentinel (out-of-band control state); use
/// `cell.is_sentinel()` to check before calling if the input source permits it.
#[inline]
pub fn route_cell(router: &MetaRouter, cell: TritCell5D) -> Result<(u8, f32), u8> {
    let trits = cell.trits().expect("route_cell requires a non-sentinel lattice cell");
    let d_model = router.d_model as usize;

    // Expand the 5 trits to a full d_model-length f32 query.
    // First 5 dimensions: convert balanced trits to f32 values.
    // Remaining dimensions: pad with zero (neutral).
    let mut query = vec![0.0f32; d_model];
    for (i, &trit) in trits.iter().enumerate() {
        query[i] = trit as f32;
    }

    router.route(&query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metarouter::trit_bytes_needed;

    /// Constructs a minimal test `MetaRouter` with d_model=64.
    fn make_test_router() -> MetaRouter {
        let bpc = trit_bytes_needed(64) as usize;
        let mut centroids = vec![121u8; 7 * bpc];
        for i in 0..7 {
            centroids[i * bpc + i] = 242;
        }
        MetaRouter {
            d_model: 64,
            num_experts: 7,
            bytes_per_centroid: bpc as u16,
            bias: [0.0; 7],
            centroids,
        }
    }

    #[test]
    fn route_cell_returns_valid_expert() {
        let router = make_test_router();
        let cell = TritCell5D::ORIGIN;
        let (expert, margin) = route_cell(&router, cell).expect("origin must not trap");
        assert!(expert < 7, "expert must be 0..6");
        assert!(margin.is_finite(), "margin must be finite");
    }

    #[test]
    fn route_cell_is_deterministic() {
        let router = make_test_router();
        let cell = TritCell5D::ORIGIN;

        let (expert1, margin1) = route_cell(&router, cell).expect("first call must succeed");
        let (expert2, margin2) = route_cell(&router, cell).expect("second call must succeed");

        assert_eq!(expert1, expert2, "same cell must route to same expert");
        assert_eq!(margin1, margin2, "margin must be identical on repeated calls");
    }

    #[test]
    fn route_cell_different_cells_may_differ() {
        let router = make_test_router();
        let origin = TritCell5D::ORIGIN;
        let other = TritCell5D(100);

        let (e1, _) = route_cell(&router, origin).expect("origin must route");
        let (e2, _) = route_cell(&router, other).expect("other must route");

        // Not asserting inequality — just that both succeed and return valid experts.
        assert!(e1 < 7 && e2 < 7, "both must return valid experts");
    }
}
