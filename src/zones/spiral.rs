//! `UlamSpiral3D` — the closed-form integer Ulam-spiral ring walk, quarried
//! from `F:\NewRepo\crates\forge-zones\src\worldgen\ulam.rs:1-196` (D4
//! ruling: Ulam IS DONE, integer-only, closed-form ring algebra, bijection
//! test included in the donor). Ported near-verbatim — the ring algebra does
//! not touch trit encoding, it is the address space `UlamCell64::spiral_index`
//! is drawn from.
//!
//! Machine-first (L08): every step is integer arithmetic; no f32/f64 anywhere
//! in this module, matching the donor.
//!
//! ONE HOME (L05): this file is the only definition of the spiral walk. The
//! HUD trace mount (xtask, zero-dependency by its own law) rides this same
//! FILE via `#[path]`, the `sky.rs`/T1 precedent. This module is therefore
//! deliberately self-contained: no `crate::` reference may enter, or the
//! mount stops compiling. The `sky-mount` feature marks the xtask ride and
//! compiles the file data-only, so no test ever has two homes.

/// Integer square root via Newton's method. Returns the largest `k` such that
/// `k*k <= n`.
const fn isqrt(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Maps linear indices onto the 2D Ulam spiral and extends into 3D via layer
/// stacking. A pure function of its inputs — the cache is a speed optimism
/// over `compute_2d`, never a second source of truth.
pub struct UlamSpiral3D {
    /// Number of precomputed 2D positions.
    cache_size: usize,
    /// Precomputed (x, y) pairs for spiral indices `0..cache_size`.
    positions_2d: Vec<(i64, i64)>,
}

impl UlamSpiral3D {
    /// Create a spiral with precomputed cache up to `cache_size` indices.
    /// The cache is a one-time heap allocation at construction, never inside
    /// a hot-path loop (forbidden_ops `hot_path_heap_alloc` scopes to
    /// `MetaRouter::route()`/the governor tick, neither of which this is).
    #[inline]
    pub fn new(cache_size: usize) -> Self {
        let positions_2d: Vec<(i64, i64)> = (0..cache_size as u64).map(Self::compute_2d).collect();
        Self { cache_size, positions_2d }
    }

    /// Closed-form 2D Ulam spiral coordinate for index `n`.
    #[inline]
    pub const fn compute_2d(n: u64) -> (i64, i64) {
        if n == 0 {
            return (0, 0);
        }
        // Ring number: k = ceil((sqrt(n) - 1) / 2)
        let k = isqrt(n - 1).div_ceil(2) as i64;
        // First index in ring k: (2k-1)^2
        let ring_start = (2 * k - 1) * (2 * k - 1);
        let offset = n as i64 - ring_start;
        let side_len = 2 * k;

        if offset < side_len {
            // Right side: moving up
            (k, -k + 1 + offset)
        } else if offset < 2 * side_len {
            // Top side: moving left
            (k - 1 - (offset - side_len), k)
        } else if offset < 3 * side_len {
            // Left side: moving down
            (-k, k - 1 - (offset - 2 * side_len))
        } else {
            // Bottom side: moving right
            (-k + 1 + (offset - 3 * side_len), -k)
        }
    }

    /// Convert a spiral index to 2D (x, y) on the Ulam plane. Uses the cache
    /// for indices below `cache_size`, falls back to computation.
    #[inline]
    pub fn index_to_2d(&self, index: u64) -> (i64, i64) {
        if (index as usize) < self.cache_size {
            self.positions_2d[index as usize]
        } else {
            Self::compute_2d(index)
        }
    }

    /// Project spiral index into 3D: z-layer = index / layer_size, (x, y)
    /// from the 2D spiral of (index % layer_size).
    #[inline]
    pub fn index_to_3d(&self, index: u64, layer_size: u64) -> (i64, i64, i64) {
        let z = (index / layer_size) as i64;
        let layer_index = index % layer_size;
        let (x, y) = self.index_to_2d(layer_index);
        (x, y, z)
    }

    /// Reverse mapping: given 3D coordinates and layer_size, recover the
    /// spiral index.
    #[inline]
    pub fn coord_to_index(&self, x: i64, y: i64, z: i64, layer_size: u64) -> u64 {
        let layer_index = Self::xy_to_index(x, y);
        z as u64 * layer_size + layer_index
    }

    /// Inverse of `compute_2d`: convert (x, y) back to the spiral index
    /// within a layer.
    #[inline]
    pub const fn xy_to_index(x: i64, y: i64) -> u64 {
        if x == 0 && y == 0 {
            return 0;
        }
        let k = if x.abs() > y.abs() { x.abs() } else { y.abs() };
        // First index in ring k: (2k-1)^2
        let ring_start = (2 * k - 1) * (2 * k - 1);
        let side_len = 2 * k;

        let offset = if x == k && y > -k {
            // Right side: y goes from -k+1 to k
            y - (-k + 1)
        } else if y == k && x < k {
            // Top side: x goes from k-1 down to -k
            side_len + (k - 1 - x)
        } else if x == -k && y < k {
            // Left side: y goes from k-1 down to -k
            2 * side_len + (k - 1 - y)
        } else {
            // Bottom side: x goes from -k+1 to k
            3 * side_len + (x - (-k + 1))
        };

        ring_start as u64 + offset as u64
    }
}

#[cfg(all(test, not(feature = "sky-mount")))]
mod tests {
    use super::*;

    #[test]
    fn test_isqrt() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(4), 2);
        assert_eq!(isqrt(8), 2);
        assert_eq!(isqrt(9), 3);
        assert_eq!(isqrt(15), 3);
        assert_eq!(isqrt(16), 4);
        assert_eq!(isqrt(100), 10);
    }

    #[test]
    fn test_index_to_2d_origin() {
        let spiral = UlamSpiral3D::new(100);
        assert_eq!(spiral.index_to_2d(0), (0, 0));
    }

    #[test]
    fn test_index_to_2d_first_ring() {
        let spiral = UlamSpiral3D::new(100);
        // Ring 1: indices 1..=8
        assert_eq!(spiral.index_to_2d(1), (1, 0));
        assert_eq!(spiral.index_to_2d(2), (1, 1));
        assert_eq!(spiral.index_to_2d(3), (0, 1));
        assert_eq!(spiral.index_to_2d(4), (-1, 1));
        assert_eq!(spiral.index_to_2d(5), (-1, 0));
        assert_eq!(spiral.index_to_2d(6), (-1, -1));
        assert_eq!(spiral.index_to_2d(7), (0, -1));
        assert_eq!(spiral.index_to_2d(8), (1, -1));
    }

    #[test]
    fn test_index_to_2d_second_ring_start() {
        let spiral = UlamSpiral3D::new(100);
        // Ring 2 starts at index 9 = (2*2-1)^2 = 9
        assert_eq!(spiral.index_to_2d(9), (2, -1));
        assert_eq!(spiral.index_to_2d(10), (2, 0));
        assert_eq!(spiral.index_to_2d(11), (2, 1));
        assert_eq!(spiral.index_to_2d(12), (2, 2));
    }

    #[test]
    fn test_index_to_3d_basic() {
        let spiral = UlamSpiral3D::new(100);
        let layer_size = 25;
        assert_eq!(spiral.index_to_3d(0, layer_size), (0, 0, 0));
        assert_eq!(spiral.index_to_3d(25, layer_size), (0, 0, 1));
        assert_eq!(spiral.index_to_3d(1, layer_size), (1, 0, 0));
    }

    #[test]
    fn test_coord_to_index_roundtrip() {
        let spiral = UlamSpiral3D::new(1000);
        let layer_size = 100;
        for n in 0..500u64 {
            let (x, y, z) = spiral.index_to_3d(n, layer_size);
            let recovered = spiral.coord_to_index(x, y, z, layer_size);
            assert_eq!(recovered, n, "roundtrip failed for n={n}");
        }
    }

    #[test]
    fn test_xy_to_index_roundtrip() {
        let spiral = UlamSpiral3D::new(1000);
        for n in 0..200u64 {
            let (x, y) = spiral.index_to_2d(n);
            let recovered = UlamSpiral3D::xy_to_index(x, y);
            assert_eq!(recovered, n, "2D roundtrip failed for n={n}: ({x},{y})");
        }
    }

    #[test]
    fn test_bijection_and_tile_crawler_match() {
        let spiral = UlamSpiral3D::new(64);
        // First 25 coords from forge-tile-crawler's iterative ulam_spiral.
        let expected_first_25 = [
            (0i64, 0i64), (1, 0), (1, 1), (0, 1), (-1, 1),
            (-1, 0), (-1, -1), (0, -1), (1, -1), (2, -1),
            (2, 0), (2, 1), (2, 2), (1, 2), (0, 2),
            (-1, 2), (-2, 2), (-2, 1), (-2, 0), (-2, -1),
            (-2, -2), (-1, -2), (0, -2), (1, -2), (2, -2),
        ];
        for (n, &(ex, ey)) in expected_first_25.iter().enumerate() {
            let (x, y) = spiral.index_to_2d(n as u64);
            assert_eq!((x, y), (ex, ey), "coord mismatch at n={n}");
        }
        // Bijection: n=0..64, index->coord->index round-trips.
        for n in 0..64u64 {
            let (x, y) = spiral.index_to_2d(n);
            let recovered = UlamSpiral3D::xy_to_index(x, y);
            assert_eq!(recovered, n, "bijection failed for n={n}");
        }
    }
}
