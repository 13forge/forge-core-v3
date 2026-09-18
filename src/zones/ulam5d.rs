//! Ulam5D — deterministic O(1) space-filling layout generator, extending
//! the real, landed `UlamSpiral3D` (`forge-zones-v3/src/spiral.rs`) with
//! two more stacked axes (Girih angle index, world layer index), the
//! same division-cascade pattern that file already uses to stack 2D -> 3D
//! via z. NOT a re-derivation of the ring-walk algebra — that stays in
//! its one home, file-mounted here via `#[path]`, the SAME mechanism
//! xtask's HUD trace already uses to ride this exact file (`spiral.rs`'s
//! own doc: "deliberately self-contained... so the mount stops
//! compiling" if a `crate::` reference ever entered it).
//!
//! [KNOWN FRICTION] `sky-mount` (declared in this crate's Cargo.toml)
//! exists for a DIFFERENT, unrelated purpose — suppressing this crate's
//! OWN `sky.rs` tests when XTASK mounts *them*. Reusing it here to
//! suppress `spiral.rs`'s mounted tests would also suppress `sky.rs`'s,
//! a collision this module does not attempt to resolve. Left as-is:
//! `spiral.rs`'s own unit tests compile and run a second time inside
//! this crate's test binary — harmless duplication (same assertions, no
//! build conflict), named here rather than hidden.

#[path = "spiral.rs"]
mod spiral;

pub use spiral::UlamSpiral3D;

/// A deterministic 5D coordinate on the Ulam5D layout spiral. `k`/`w`
/// are raw, unsigned axis indices (`0..k_count`/`0..w_count`) — mapping
/// `w` to a signed world layer (e.g. for `WorldBuilderEngine`) is the
/// caller's job, same as the donor's own `z` is unsigned through this
/// forward cascade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ulam5DCoord {
    /// Spiral X.
    pub x: i64,
    /// Spiral Y.
    pub y: i64,
    /// Stacked layer index (donor's own `z`).
    pub z: i64,
    /// Girih angle index, `0..k_count`.
    pub k: u64,
    /// World-layer index, `0..w_count`.
    pub w: u64,
}

/// Map a linear procedural seed `n` onto `(X,Y,Z,k,W)` — the same
/// division-cascade `index_to_3d` already uses for `z`, extended two
/// more axes: `w` outermost, then `k`, then `z`, then the 2D spiral
/// itself within each `(z,k,w)` cell. `layer_size`/`z_count`/`k_count`
/// bound each axis's span before wrapping outward to the next.
pub fn index_to_5d(spiral: &UlamSpiral3D, n: u64, layer_size: u64, z_count: u64, k_count: u64) -> Ulam5DCoord {
    let per_k = layer_size * z_count;
    let per_w = per_k * k_count;

    let w = n / per_w;
    let rem_w = n % per_w;
    let k = rem_w / per_k;
    let rem_k = rem_w % per_k;
    let z = (rem_k / layer_size) as i64;
    let layer_index = rem_k % layer_size;
    let (x, y) = spiral.index_to_2d(layer_index);

    Ulam5DCoord { x, y, z, k, w }
}

/// Inverse of [`index_to_5d`]: recover the linear seed from a 5D coordinate.
pub fn coord_to_index(coord: Ulam5DCoord, layer_size: u64, z_count: u64, k_count: u64) -> u64 {
    let per_k = layer_size * z_count;
    let per_w = per_k * k_count;
    let layer_index = UlamSpiral3D::xy_to_index(coord.x, coord.y);
    coord.w * per_w + coord.k * per_k + (coord.z as u64) * layer_size + layer_index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_5d_round_trips_through_coord_to_index() {
        let spiral = UlamSpiral3D::new(200);
        let (layer_size, z_count, k_count) = (100, 4, 10);
        for n in [0u64, 1, 50, 99, 150, 399, 1_000, 3_999, 39_999] {
            let coord = index_5d_helper(&spiral, n, layer_size, z_count, k_count);
            let back = coord_to_index(coord, layer_size, z_count, k_count);
            assert_eq!(back, n, "n={n} did not round-trip through 5D coord");
        }
    }

    fn index_5d_helper(spiral: &UlamSpiral3D, n: u64, layer_size: u64, z_count: u64, k_count: u64) -> Ulam5DCoord {
        index_to_5d(spiral, n, layer_size, z_count, k_count)
    }

    #[test]
    fn w_and_k_advance_only_after_their_inner_axes_wrap() {
        let spiral = UlamSpiral3D::new(50);
        let (layer_size, z_count, k_count) = (10, 3, 2);
        let per_k = layer_size * z_count; // 30
        let per_w = per_k * k_count; // 60

        let c0 = index_to_5d(&spiral, 0, layer_size, z_count, k_count);
        assert_eq!((c0.z, c0.k, c0.w), (0, 0, 0));

        let c_last_z = index_to_5d(&spiral, per_k - 1, layer_size, z_count, k_count);
        assert_eq!((c_last_z.k, c_last_z.w), (0, 0), "still inside k=0,w=0 just before wrap");

        let c_first_k = index_to_5d(&spiral, per_k, layer_size, z_count, k_count);
        assert_eq!((c_first_k.z, c_first_k.k, c_first_k.w), (0, 1, 0), "k advances exactly at the wrap");

        let c_first_w = index_to_5d(&spiral, per_w, layer_size, z_count, k_count);
        assert_eq!((c_first_w.z, c_first_w.k, c_first_w.w), (0, 0, 1), "w advances exactly at its wrap");
    }

    /// Real use case: deterministic district placement for a town/dungeon
    /// layout — every seed in a real range lands on a distinct, balanced
    /// 5D cell (no spatial overlap within the same layer_size/z/k/w cell).
    #[test]
    fn deterministic_district_placement_has_no_collisions_within_a_layer() {
        let spiral = UlamSpiral3D::new(500);
        let (layer_size, z_count, k_count) = (441, 5, 10); // 21x21-ish ring budget
        let mut seen = std::collections::HashSet::new();
        for n in 0..per_layer_sample(layer_size, z_count, k_count) {
            let c = index_to_5d(&spiral, n, layer_size, z_count, k_count);
            assert!(seen.insert((c.x, c.y, c.z, c.k, c.w)), "collision at seed {n}");
        }
    }

    fn per_layer_sample(layer_size: u64, z_count: u64, k_count: u64) -> u64 {
        (layer_size * z_count * k_count).min(2_000)
    }
}
