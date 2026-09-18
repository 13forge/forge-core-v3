//! O(1) spatial index over `TritCell5D` — port of `SphereIndex`
//! (`F:\NewRepo\crates\forge-ml\src\sphere_index.rs`, v2, integer-only,
//! 7 tests green). `encode`/`decode`/the Morton bit-interleave/the
//! quadtree `parent`/`children` walk are ported verbatim, unchanged. NOT
//! ported: `ang2pix`/`ang2face`/`index_cree_codebook`/`fold_lanes` — the
//! donor's one f32 boundary (celestial-coordinate binning) and its
//! CREE-codebook caller, both out of scope for a 5D lattice index.
//!
//! New here: [`cell_pixel`]/[`pixel_cell`], a bijective adapter from
//! `TritCell5D`'s 5 balanced trits to the donor's `(face, x, y)` input —
//! `(t0,t1) -> face 0..=8`, `(t2,t3) -> x 0..=8`, `t4 -> y 0..=2`. Every
//! interior `TritCell5D` code round-trips exactly (L07); `order` must be
//! `>= 4` (`Nside >= 16`) so `x` fits.

use crate::atom::TritCell5D;

/// The coarse partition before subdivision — 12 base faces, HEALPix-style.
pub const BASE_FACES: u64 = 12;

/// A resolution level of the sphere index: `order` is the quadtree depth
/// per face (`Nside = 1 << order`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SphereIndex {
    /// Quadtree depth per face.
    pub order: u32,
}

impl SphereIndex {
    /// A new index at quadtree depth `order` (`4..=26`).
    pub fn new(order: u32) -> Self {
        debug_assert!(order <= 26, "order {order} overflows a u64 pixel id");
        debug_assert!(order >= 4, "order {order} too coarse for the TritCell5D adapter (needs Nside >= 16)");
        Self { order }
    }

    /// Pixels along one face edge, `2^order`.
    pub fn nside(&self) -> u64 {
        1u64 << self.order
    }

    /// Pixels per base face, `Nside^2`.
    pub fn per_face(&self) -> u64 {
        1u64 << (2 * self.order)
    }

    /// Total pixels, `12 * Nside^2`.
    pub fn npix(&self) -> u64 {
        BASE_FACES * self.per_face()
    }

    /// `(face, x, y)` -> the nested pixel id.
    pub fn encode(&self, face: u32, x: u32, y: u32) -> u64 {
        debug_assert!((face as u64) < BASE_FACES, "face {face} >= 12");
        debug_assert!((x as u64) < self.nside() && (y as u64) < self.nside(), "cell out of face");
        (face as u64) * self.per_face() + morton2(x, y)
    }

    /// The inverse of [`encode`](Self::encode): pixel id -> `(face, x, y)`.
    pub fn decode(&self, pix: u64) -> (u32, u32, u32) {
        let face = (pix / self.per_face()) as u32;
        let local = pix % self.per_face();
        let (x, y) = unmorton2(local);
        (face, x, y)
    }
}

// ── Morton / Z-order bit-interleave ──────────────────────────────────────

fn part1by1(mut n: u64) -> u64 {
    n &= 0x0000_0000_ffff_ffff;
    n = (n | (n << 16)) & 0x0000_ffff_0000_ffff;
    n = (n | (n << 8)) & 0x00ff_00ff_00ff_00ff;
    n = (n | (n << 4)) & 0x0f0f_0f0f_0f0f_0f0f;
    n = (n | (n << 2)) & 0x3333_3333_3333_3333;
    n = (n | (n << 1)) & 0x5555_5555_5555_5555;
    n
}

fn compact1by1(mut n: u64) -> u64 {
    n &= 0x5555_5555_5555_5555;
    n = (n | (n >> 1)) & 0x3333_3333_3333_3333;
    n = (n | (n >> 2)) & 0x0f0f_0f0f_0f0f_0f0f;
    n = (n | (n >> 4)) & 0x00ff_00ff_00ff_00ff;
    n = (n | (n >> 8)) & 0x0000_ffff_0000_ffff;
    n = (n | (n >> 16)) & 0x0000_0000_ffff_ffff;
    n
}

/// Interleave x (even bits) and y (odd bits) into one Z-order code.
pub fn morton2(x: u32, y: u32) -> u64 {
    part1by1(x as u64) | (part1by1(y as u64) << 1)
}

/// De-interleave a Z-order code back into `(x, y)`.
pub fn unmorton2(code: u64) -> (u32, u32) {
    (compact1by1(code) as u32, compact1by1(code >> 1) as u32)
}

// ── Quadtree walk ─────────────────────────────────────────────────────────

/// The parent pixel one order coarser.
pub fn parent(pix: u64) -> u64 {
    pix >> 2
}

/// The four child pixels one order finer.
pub fn children(pix: u64) -> [u64; 4] {
    let base = pix << 2;
    [base, base | 1, base | 2, base | 3]
}

// ── TritCell5D adapter (new this wave) ───────────────────────────────────

/// `TritCell5D` -> pixel id. `None` for a sentinel cell (never a coordinate).
pub fn cell_pixel(idx: &SphereIndex, cell: TritCell5D) -> Option<u64> {
    let t = cell.trits()?;
    let face = (t[0] + 1) as u32 + (t[1] + 1) as u32 * 3;
    let x = (t[2] + 1) as u32 + (t[3] + 1) as u32 * 3;
    let y = (t[4] + 1) as u32;
    Some(idx.encode(face, x, y))
}

/// Inverse of [`cell_pixel`]. Only valid for a pixel this adapter produced
/// (`face < 9`, `x < 9`, `y < 3`) — a pixel from the donor's full `0..12`/
/// `0..Nside` range outside that band has no `TritCell5D` preimage.
pub fn pixel_cell(idx: &SphereIndex, pix: u64) -> Option<TritCell5D> {
    let (face, x, y) = idx.decode(pix);
    if face >= 9 || x >= 9 || y >= 3 {
        return None;
    }
    let t0 = (face % 3) as i8 - 1;
    let t1 = (face / 3) as i8 - 1;
    let t2 = (x % 3) as i8 - 1;
    let t3 = (x / 3) as i8 - 1;
    let t4 = y as i8 - 1;
    Some(TritCell5D::from_trits([t0, t1, t2, t3, t4]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbers_hold() {
        let s = SphereIndex::new(7);
        assert_eq!(s.nside(), 128);
        assert_eq!(s.per_face(), 16_384);
        assert_eq!(s.npix(), 196_608);
    }

    #[test]
    fn morton_round_trips() {
        for x in [0u32, 1, 2, 3, 42, 127, 4096, 65_535] {
            for y in [0u32, 1, 5, 63, 128, 9_999, 65_535] {
                let (bx, by) = unmorton2(morton2(x, y));
                assert_eq!((bx, by), (x, y), "morton round-trip {x},{y}");
            }
        }
    }

    #[test]
    fn encode_decode_round_trips_over_all_faces() {
        let s = SphereIndex::new(7);
        for face in 0..12u32 {
            for x in [0u32, 1, 7, 63, 127] {
                for y in [0u32, 3, 64, 127] {
                    let pix = s.encode(face, x, y);
                    assert!(pix < s.npix(), "pixel {pix} out of range");
                    assert_eq!(s.decode(pix), (face, x, y), "decode {face},{x},{y}");
                }
            }
        }
    }

    #[test]
    fn quadtree_parent_child_consistency() {
        let s = SphereIndex::new(6);
        let coarse = SphereIndex::new(5);
        for face in [0u32, 5, 11] {
            for x in [0u32, 1, 30, 63] {
                for y in [0u32, 2, 63] {
                    let pix = s.encode(face, x, y);
                    let kids = children(pix);
                    assert_eq!(kids.len(), 4);
                    for &k in &kids {
                        assert_eq!(parent(k), pix, "child {k} parent != {pix}");
                    }
                    let p = parent(pix);
                    let (pf, _px, _py) = coarse.decode(p);
                    assert_eq!(pf, face, "parent changed face");
                    assert!(children(p).contains(&pix), "pix not a child of its parent");
                }
            }
        }
    }

    /// L07 bijection: every interior `TritCell5D` code round-trips through
    /// `cell_pixel`/`pixel_cell` exactly.
    #[test]
    fn tritcell5d_pixel_round_trips_over_every_interior_code() {
        let s = SphereIndex::new(4);
        for code in 0u8..243 {
            let cell = TritCell5D(code);
            let pix = cell_pixel(&s, cell).expect("interior code, never a sentinel");
            let back = pixel_cell(&s, pix).expect("adapter-produced pixel must decode");
            assert_eq!(back, cell, "f^-1(f(x)) = x for TritCell5D {code}");
        }
    }

    #[test]
    fn sentinel_cells_have_no_pixel() {
        let s = SphereIndex::new(4);
        for code in 243u16..=255 {
            assert!(cell_pixel(&s, TritCell5D(code as u8)).is_none());
        }
    }
}
