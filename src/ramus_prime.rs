//! Ramus Prime — the RAG/DAG node: 5D Morton keys, M61 hypersphere scoring, and a
//! forward-only edge gate, drained from the v2 quarry (2026-08-10).
//!
//! Provenance, per receipt:
//! - Morton interleave: v2 `forge-game-systems/src/spatial5d.rs:56-90` — which existed
//!   TWICE in v2, byte-identical (`forge-vision/src/poll5d/spatial.rs:29-63`). This is
//!   the one home; both v2 copies are quarry.
//! - Field fold: [`crate::mersenne::reduce_m61`], already proven here. The v2
//!   `sphere_index.rs` fold is the M13 cousin and stays in its own domain.
//! - Verb edge: the ternary-branch traversal of v2 `outland/src/trit_tree.rs`, with its
//!   `f64` routing hint deliberately left behind — nothing in this module floats.
//!
//! The spec's own node did not fit: `CremanticsEdge` (8B target + 1B trit + 7B pad =
//! 16B) plus 8+8+40 is 72 bytes, not 64. The fix is a fold, not a squeeze: the verb
//! trit *is* a [`TritCell5D`], and the node already carries one inside its [`Pexil`].
//! One byte, one home (L05). A sentinel verb byte means "no outgoing edge" — absence
//! lives in the envelope, never in a zeroed coordinate.

use crate::atom::{Pexil, TritCell5D};
use crate::grid::{PackedPoint105, DEPTH, LANES};
use crate::mersenne::{reduce_m61, M61};

/// Bits per axis. `5 * 12 = 60 <= 64`; the top four key bits are always zero.
pub const AXIS_BITS: u32 = 12;
/// The five axes `[X, Y, Z, T, S]`, same lane order as [`crate::grid`].
pub const AXES: usize = 5;
/// One axis, masked. `2^12 - 1`. Not a Mersenne prime and not named like one.
pub const AXIS_MASK: u16 = (1 << AXIS_BITS) - 1;

/// Axis indices, so `T` and `S` are never bare `3` and `4` at a call site.
pub const AXIS_X: usize = 0;
/// Y axis index.
pub const AXIS_Y: usize = 1;
/// Z axis index — the isolation plane.
pub const AXIS_Z: usize = 2;
/// T axis index — non-decreasing along every edge.
pub const AXIS_T: usize = 3;
/// S axis index — with T, strictly increasing in sum along every edge.
pub const AXIS_S: usize = 4;

// AXIS PIN (ARCH000 2026-08-10, "pin the axes W=T and θ=S"). Two live 5-lane
// vocabularies existed: (X,Y,Z,W,θ) in the audio/sphere lineage
// (dimensional_collapse, sphere_index fold_lanes) and (X,Y,Z,T,S) here. They
// are ONE set of lanes: W is chrono-tick lineage = T; θ is the harmonic rung,
// and a harmonic rung is a scale rung = S. Every corpus line and every model
// trained on it inherits this correspondence from these constants, not prose.

/// W, the chrono-tick lineage axis of the audio/sphere vocabulary. `== AXIS_T`.
pub const AXIS_W: usize = AXIS_T;
/// θ, the harmonic-codeword axis of the audio/sphere vocabulary. `== AXIS_S`.
pub const AXIS_THETA: usize = AXIS_S;

const _: () = assert!(AXIS_W == 3);
const _: () = assert!(AXIS_THETA == 4);

/// A 5D Morton key: five 12-bit axes bit-interleaved into the low 60 bits of a `u64`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MortonKey5D(/// The interleaved key. Bits 60..64 are always zero.
pub u64);

impl MortonKey5D {
    /// Interleave five masked axes. Bit `b` of axis `a` lands at `b * AXES + a`,
    /// the exact placement of the v2 pair this drains.
    #[inline]
    pub const fn encode(axes: [u16; AXES]) -> Self {
        let mut key = 0u64;
        let mut bit = 0;
        while bit < AXIS_BITS {
            let mut axis = 0;
            while axis < AXES {
                let v = (axes[axis] & AXIS_MASK) as u64;
                key |= ((v >> bit) & 1) << (bit as usize * AXES + axis);
                axis += 1;
            }
            bit += 1;
        }
        Self(key)
    }

    /// De-interleave back to the five axes. The bijection test holds this against
    /// [`Self::encode`] — L07: every encode has a tested decode.
    #[inline]
    pub const fn axes(self) -> [u16; AXES] {
        let mut axes = [0u16; AXES];
        let mut bit = 0;
        while bit < AXIS_BITS {
            let mut axis = 0;
            while axis < AXES {
                axes[axis] |= (((self.0 >> (bit as usize * AXES + axis)) & 1) as u16) << bit;
                axis += 1;
            }
            bit += 1;
        }
        axes
    }

    /// The Z-plane is the isolation boundary: two keys in different planes never
    /// score against each other, whatever their distance elsewhere.
    #[inline]
    pub const fn same_z_plane(self, other: Self) -> bool {
        self.axes()[AXIS_Z] == other.axes()[AXIS_Z]
    }
}

/// An inclusive 5D bounding box, the pruning predicate of Stage 2. A key outside the
/// box discards its whole sub-DAG before any payload is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Box5D {
    /// Inclusive minimum per axis.
    pub min: [u16; AXES],
    /// Inclusive maximum per axis.
    pub max: [u16; AXES],
}

impl Box5D {
    /// True when the key's every axis lies in `min..=max`.
    #[inline]
    pub const fn contains(&self, key: MortonKey5D) -> bool {
        let a = key.axes();
        let mut axis = 0;
        while axis < AXES {
            if a[axis] < self.min[axis] || a[axis] > self.max[axis] {
                return false;
            }
            axis += 1;
        }
        true
    }

    /// Volume of the box in keys: the product of its five inclusive extents (0 when any
    /// axis is inverted). The range cover of a box sums to exactly this.
    pub const fn volume(&self) -> u64 {
        let mut v = 1u64;
        let mut axis = 0;
        while axis < AXES {
            if self.max[axis] < self.min[axis] {
                return 0;
            }
            v *= (self.max[axis] - self.min[axis]) as u64 + 1;
            axis += 1;
        }
        v
    }
}

/// One contiguous run of Morton keys, inclusive at both ends.
pub type MortonRange = (MortonKey5D, MortonKey5D);

/// The LOD ladder cell holding `key` when the low `lod` bits of every axis are dropped:
/// `(first, last)` key of that cell. `lod == 0` is the key itself; `lod >= AXIS_BITS` is
/// the whole space. An exact-match relay answers a cell tag with this one range.
#[inline]
pub const fn ladder_truncate(key: MortonKey5D, lod: u32) -> MortonRange {
    let lod = if lod > AXIS_BITS { AXIS_BITS } else { lod };
    let low_bits = lod as usize * AXES;
    let mask: u64 = if low_bits >= 64 { u64::MAX } else { (1u64 << low_bits) - 1 };
    (MortonKey5D(key.0 & !mask), MortonKey5D(key.0 | mask))
}

/// Decompose an inclusive 5D box into the minimal cover of contiguous Morton runs, in
/// ascending key order, written into `out`. Returns the run count, or `None` when `out`
/// is too small (nothing is allocated; the caller sizes the buffer).
///
/// Walks the Z-order tree top-down: a cell entirely inside the box is one run, a cell
/// disjoint from it is skipped, a straddling cell splits into its 32 children. Runs that
/// touch are merged, so no two emitted runs are adjacent — the LITMAX/BIGMIN cover of the
/// UB-tree literature, produced by descent instead of key-skipping.
///
/// CEILING (measured 2026-09-05): an UNALIGNED box fragments to about `volume / 2` runs,
/// because X is the lowest interleaved bit and in/out alternates key by key. A ladder
/// cell ([`ladder_truncate`]) is always exactly ONE run. Relay queries snap to the ladder
/// and use this fn for the residue, never for a raw large box.
pub fn box5d_to_morton_ranges(b: &Box5D, out: &mut [MortonRange]) -> Option<usize> {
    if b.volume() == 0 {
        return Some(0);
    }
    let mut n = 0usize;
    let ok = descend(b, [0u16; AXES], AXIS_BITS, out, &mut n);
    ok.then_some(n)
}

/// One Z-order cell: `prefix` holds the axis bits above `level`; the cell spans every
/// key whose top bits match, i.e. `2^(level*AXES)` consecutive keys.
fn descend(b: &Box5D, prefix: [u16; AXES], level: u32, out: &mut [MortonRange], n: &mut usize) -> bool {
    let span_mask = (1u16 << level) - 1;
    let mut inside = true;
    let mut axis = 0;
    while axis < AXES {
        let lo = prefix[axis] << level;
        let hi = lo | span_mask;
        if hi < b.min[axis] || lo > b.max[axis] {
            return true; // disjoint: nothing to emit
        }
        if lo < b.min[axis] || hi > b.max[axis] {
            inside = false;
        }
        axis += 1;
    }
    if inside {
        let first = MortonKey5D::encode(shift_axes(prefix, level));
        let last = MortonKey5D(first.0 | ((1u64 << (level as usize * AXES)) - 1));
        return push_merged(out, n, (first, last));
    }
    // level > 0 here: a level-0 cell is a single key and is always inside or disjoint.
    let child_level = level - 1;
    let mut child = 0u32;
    while child < (1 << AXES) {
        let mut p = prefix;
        let mut a = 0;
        while a < AXES {
            p[a] = (p[a] << 1) | ((child >> a) & 1) as u16;
            a += 1;
        }
        if !descend(b, p, child_level, out, n) {
            return false;
        }
        child += 1;
    }
    true
}

const fn shift_axes(prefix: [u16; AXES], level: u32) -> [u16; AXES] {
    let mut axes = [0u16; AXES];
    let mut a = 0;
    while a < AXES {
        axes[a] = prefix[a] << level;
        a += 1;
    }
    axes
}

fn push_merged(out: &mut [MortonRange], n: &mut usize, run: MortonRange) -> bool {
    if *n > 0 && out[*n - 1].1 .0 + 1 == run.0 .0 {
        out[*n - 1].1 = run.1;
        return true;
    }
    if *n >= out.len() {
        return false;
    }
    out[*n] = run;
    *n += 1;
    true
}

/// A residue of the field `F_M61`, held reduced: `0 <= value < M61`. The constructor
/// reduces; arithmetic in [`mersenne_dot`] depends on that bound to fit `u128`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MersenneScalar(/// The reduced residue.
pub u64);

impl MersenneScalar {
    /// The additive identity.
    pub const ZERO: Self = Self(0);

    /// Reduce any `u64` into the field.
    #[inline(always)]
    pub const fn new(x: u64) -> Self {
        Self(reduce_m61(x))
    }
}

/// A 5D vector of field residues — a point of `F_M61^5`, used as an `S^4` direction.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HypersphereVector5D {
    /// The five components, each reduced below `M61`.
    pub components: [MersenneScalar; AXES],
}

impl HypersphereVector5D {
    /// The zero vector.
    pub const ZERO: Self = Self { components: [MersenneScalar::ZERO; AXES] };

    /// True when this vector lies on the sphere of squared radius `r2`:
    /// `sum(x_i^2) == r2` in the field.
    #[inline]
    pub fn is_on_sphere(&self, r2: MersenneScalar) -> bool {
        mersenne_dot(self, self) == r2
    }
}

/// Reduce a `u128` accumulator modulo `M61`. `2^61 ≡ 1`, so the three 61-bit chunks
/// sum congruently; their sum is at most `2*M61 + 7`, which [`reduce_m61`] finishes.
#[inline(always)]
const fn reduce_m61_u128(x: u128) -> u64 {
    let m = M61 as u128;
    let folded = (x & m) + ((x >> 61) & m) + (x >> 122);
    reduce_m61(folded as u64)
}

/// The exact `F_M61` inner product — Stage 4 of the pipeline. Bit-identical on every
/// architecture because nothing here rounds: five `u128` products, one fold.
///
/// The `u128` accumulator is safe *because* components are reduced: five products of
/// 61-bit values are below `5 * 2^122 < 2^125`.
#[inline]
pub fn mersenne_dot(a: &HypersphereVector5D, b: &HypersphereVector5D) -> MersenneScalar {
    let mut acc: u128 = 0;
    let mut i = 0;
    while i < AXES {
        debug_assert!(a.components[i].0 < M61 && b.components[i].0 < M61);
        acc += (a.components[i].0 as u128) * (b.components[i].0 as u128);
        i += 1;
    }
    MersenneScalar(reduce_m61_u128(acc))
}

/// The DAG order: an edge may only point where `ΔT >= 0` and `ΔT + ΔS > 0`. `T + S`
/// then strictly increases along every edge, so no walk can revisit a key — acyclicity
/// by construction, no cycle detector needed (the v2 `forge-dag` Kahn sort solved a
/// mutable task graph; this lattice refuses the cycle at link time instead).
#[inline]
pub const fn edge_is_forward(from: MortonKey5D, to: MortonKey5D) -> bool {
    let a = from.axes();
    let b = to.axes();
    let dt = b[AXIS_T] as i32 - a[AXIS_T] as i32;
    let ds = b[AXIS_S] as i32 - a[AXIS_S] as i32;
    dt >= 0 && dt + ds > 0
}

/// The node. One L1 line: 8 (key) + 8 (pexil) + 8 (edge target) + 40 (vector) = 64.
///
/// There is no separate edge-verb field. The verb *is* `pexil.lattice` — a
/// [`TritCell5D`] holding the five directional trits of the outgoing edge. When that
/// byte is a sentinel ([`crate::sentinel::Sentinel::NullNode`] by convention) the node
/// is terminal and `edge_target` is dead weight, never read.
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamusPrimeNode {
    /// This node's 5D Morton address.
    pub morton_key: MortonKey5D,
    /// The atom: verb trits in `lattice`, Kleene validity, identity in `ordinal`.
    pub pexil: Pexil,
    /// The primary outgoing edge's target key. Meaningless when the verb is sentinel.
    pub edge_target: MortonKey5D,
    /// The node's field vector, scored by [`mersenne_dot`].
    pub hypersphere: HypersphereVector5D,
}

impl RamusPrimeNode {
    /// The outgoing edge, or `None` when the verb byte is a sentinel. The trits are
    /// the Cremantics direction of the transition, one per axis.
    #[inline]
    pub const fn outgoing_edge(&self) -> Option<(MortonKey5D, [i8; AXES])> {
        match self.pexil.lattice.trits() {
            Some(t) => Some((self.edge_target, t)),
            None => None,
        }
    }

    /// The ternary-branch step — Stage 3, drained from the v2 `outland` raycast:
    /// there, a query trit chose `child_neg` / `child_zero` / `child_pos`. Here the
    /// edge is followed only when its verb agrees with `want` on every axis the
    /// query constrains; a zero in `want` constrains nothing. A terminal node
    /// follows nowhere.
    #[inline]
    pub const fn follow(&self, want: [i8; AXES]) -> Option<MortonKey5D> {
        match self.outgoing_edge() {
            Some((target, verb)) => {
                let mut axis = 0;
                while axis < AXES {
                    if want[axis] != 0 && want[axis] != verb[axis] {
                        return None;
                    }
                    axis += 1;
                }
                Some(target)
            }
            None => None,
        }
    }

    /// Point the edge at `target` with direction `verb`. Refused — `false`, no
    /// mutation — when `verb` is a sentinel or the edge would run backward in the
    /// DAG order. A refused link is the cycle that never got to exist.
    #[inline]
    pub fn try_link(&mut self, target: MortonKey5D, verb: TritCell5D) -> bool {
        if verb.is_sentinel() || !edge_is_forward(self.morton_key, target) {
            return false;
        }
        self.pexil.lattice = verb;
        self.edge_target = target;
        true
    }
}

// ---- THE GLASS BRIDGE — index to onscreen ---------------------------------------
//
// The NeuroHUD paints `PackedPoint105` via `grid::point_to_pixels`; this pair puts a
// Ramus key into that contract and back. One 12-bit axis needs 9 balanced trits
// (8 reach only ±3280 < 4095; 9 reach ±9841), so the key occupies the 9 leaf-most
// depths — least significant digit at the leaf, depths 0..12 at the balanced origin.

/// Balanced-trit digits per axis. Minimality is asserted below, not claimed.
pub const KEY_TRIT_DIGITS: usize = 9;

/// `3^d` for each digit position.
const POW3: [i32; KEY_TRIT_DIGITS] = [1, 3, 9, 27, 81, 243, 729, 2187, 6561];

// 9 digits are enough and 8 are not — the choice is arithmetic, not taste.
const _: () = assert!((POW3[8] * 3 - 1) / 2 >= AXIS_MASK as i32);
const _: () = assert!((POW3[8] - 1) / 2 < AXIS_MASK as i32);
// The grid's five lanes are this module's five axes, or the bridge is a transpose.
const _: () = assert!(LANES == AXES);
const _: () = assert!(DEPTH - KEY_TRIT_DIGITS == 12);

/// Serialise a key into the HUD's point: five axes to balanced ternary, leaf-aligned.
pub fn key_to_point(key: MortonKey5D) -> PackedPoint105 {
    let axes = key.axes();
    let mut slices = [TritCell5D::ORIGIN; DEPTH];
    let mut n = [0i32; AXES];
    let mut axis = 0;
    while axis < AXES {
        n[axis] = axes[axis] as i32;
        axis += 1;
    }
    for slice in slices.iter_mut().rev().take(KEY_TRIT_DIGITS) {
        let mut t = [0i8; AXES];
        for (axis, v) in n.iter_mut().enumerate() {
            let mut r = *v % 3;
            *v /= 3;
            if r == 2 {
                r = -1;
                *v += 1;
            }
            t[axis] = r as i8;
        }
        *slice = TritCell5D::from_trits(t);
    }
    PackedPoint105 { slices }
}

/// Reconstruct the key. `None` when the point is not a key at all: a sentinel slice,
/// a non-origin depth above the key band, or a digit sum outside `0..=AXIS_MASK`.
/// The HUD may paint such points; the index refuses to import them.
pub fn point_to_key(p: &PackedPoint105) -> Option<MortonKey5D> {
    for d in 0..DEPTH - KEY_TRIT_DIGITS {
        if p.slices[d] != TritCell5D::ORIGIN {
            return None;
        }
    }
    let mut acc = [0i32; AXES];
    for d in 0..KEY_TRIT_DIGITS {
        let t = p.slices[DEPTH - 1 - d].trits()?;
        for axis in 0..AXES {
            acc[axis] += t[axis] as i32 * POW3[d];
        }
    }
    let mut axes = [0u16; AXES];
    for axis in 0..AXES {
        if acc[axis] < 0 || acc[axis] > AXIS_MASK as i32 {
            return None;
        }
        axes[axis] = acc[axis] as u16;
    }
    Some(MortonKey5D::encode(axes))
}

// LAYOUT LOCKS — the 64-byte contract the spec asserted and could not meet. Here it
// is met, and `cargo check` holds it.
const _: () = assert!(core::mem::size_of::<MortonKey5D>() == 8);
const _: () = assert!(core::mem::size_of::<MersenneScalar>() == 8);
const _: () = assert!(core::mem::size_of::<HypersphereVector5D>() == 40);
const _: () = assert!(core::mem::size_of::<RamusPrimeNode>() == 64);
const _: () = assert!(core::mem::align_of::<RamusPrimeNode>() == 64);
const _: () = assert!(core::mem::offset_of!(RamusPrimeNode, morton_key) == 0);
const _: () = assert!(core::mem::offset_of!(RamusPrimeNode, pexil) == 8);
const _: () = assert!(core::mem::offset_of!(RamusPrimeNode, edge_target) == 16);
const _: () = assert!(core::mem::offset_of!(RamusPrimeNode, hypersphere) == 24);
const _: () = assert!(AXIS_BITS as usize * AXES == 60);

/// Manhattan distance over the five integer axes (X, Y, Z, T, S) — ranking only,
/// used for candidate selection in hypersphere blending. Not a field metric.
#[inline]
pub const fn axes_distance(a: MortonKey5D, b: MortonKey5D) -> u32 {
    let a_axes = a.axes();
    let b_axes = b.axes();
    let mut sum = 0u32;
    let mut i = 0;
    while i < AXES {
        let diff = (a_axes[i] as i32) - (b_axes[i] as i32);
        sum += diff.abs() as u32;
        i += 1;
    }
    sum
}

/// Exact field-arithmetic weighted sum of hypersphere vectors.
/// Takes integer weights and a slice of points, computes the weighted sum
/// as an exact `F_M61` operation (no rounding, no interpolation — just scaled
/// addition in the field). The output is a point on the sphere iff all input
/// points and weights conserve the radius: this is a lossy many-to-one reduction,
/// not a reversible transformation.
#[inline]
pub fn mersenne_weighted_sum(weights: &[i32], points: &[HypersphereVector5D]) -> HypersphereVector5D {
    debug_assert_eq!(weights.len(), points.len(), "weights and points must have equal length");
    let mut result = [MersenneScalar::ZERO; AXES];

    for (w, p) in weights.iter().zip(points.iter()) {
        let w_reduced = reduce_m61(*w as u64);
        for i in 0..AXES {
            let term = (p.components[i].0 as u128) * (w_reduced as u128);
            let sum = (result[i].0 as u128) + term;
            result[i] = MersenneScalar(reduce_m61_u128(sum));
        }
    }

    HypersphereVector5D { components: result }
}

/// Blend a caller-owned slice of candidate nodes into a single hypersphere point.
/// Selects the k nearest candidates (by Manhattan distance over integer axes) and
/// returns their weighted average in the `F_M61` field. The caller must supply
/// the candidate slice — this function does not search a store.
///
/// `k` is clamped to `candidates.len()`. Returns `ZERO` if the slice is empty.
/// Limited to `k <= 32` to avoid unbounded stack allocation. Caller selects which
/// candidates to pass; this function is selection-free.
pub fn sample_blend(candidates: &[RamusPrimeNode], query: MortonKey5D, k: usize) -> HypersphereVector5D {
    if candidates.is_empty() {
        return HypersphereVector5D::ZERO;
    }

    let k = k.min(candidates.len()).min(32);
    let num_candidates = candidates.len().min(32);

    let mut scratch = [(0usize, 0u32); 32];
    for (i, candidate) in candidates.iter().enumerate().take(32) {
        scratch[i] = (i, axes_distance(candidate.morton_key, query));
    }

    scratch[..num_candidates].sort_by_key(|(_, d)| *d);

    let weights = [1i32; 32];
    let mut points = [HypersphereVector5D::ZERO; 32];
    for (i, (idx, _)) in scratch[..k].iter().enumerate() {
        points[i] = candidates[*idx].hypersphere;
    }

    mersenne_weighted_sum(&weights[..k], &points[..k])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentinel::Sentinel;

    // ---- terra1-morton-ranges: box → minimal Z-order cover ------------------------

    /// PCG-style step: enough spread for box fixtures, no platform RNG.
    fn box_rng(state: &mut u64) -> u64 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *state >> 33
    }

    fn random_box(state: &mut u64, ceiling: u16) -> Box5D {
        let mut min = [0u16; AXES];
        let mut max = [0u16; AXES];
        for a in 0..AXES {
            let x = (box_rng(state) % ceiling as u64) as u16;
            let y = (box_rng(state) % ceiling as u64) as u16;
            min[a] = x.min(y);
            max[a] = x.max(y);
        }
        Box5D { min, max }
    }

    /// Cover laws that hold for ANY box: ascending, disjoint, never adjacent (minimal),
    /// every run's endpoints inside the box, and the run lengths sum to the volume.
    fn check_cover(b: &Box5D, runs: &[MortonRange]) {
        let mut covered = 0u64;
        for (i, (lo, hi)) in runs.iter().enumerate() {
            assert!(lo <= hi, "run {i} inverted: {lo:?} > {hi:?}");
            assert!(b.contains(*lo) && b.contains(*hi), "run {i} endpoint outside {b:?}");
            if i > 0 {
                let prev_hi = runs[i - 1].1 .0;
                assert!(prev_hi < lo.0, "run {i} overlaps or is out of order");
                assert!(prev_hi + 1 < lo.0, "run {i} is adjacent to run {} — cover is not minimal", i - 1);
            }
            covered += hi.0 - lo.0 + 1;
        }
        assert_eq!(covered, b.volume(), "cover length must equal the box volume for {b:?}");
    }

    #[test]
    fn box_cover_is_minimal_inside_and_sums_to_the_volume_over_the_full_space() {
        // Tiny boxes (extent <= 4 per axis) at random 12-bit offsets. The measured law:
        // an UNALIGNED 5D box fragments to ~volume/2 runs (the X bit is the lowest
        // interleaved bit, so in/out alternates key by key), which is why relay queries
        // snap to the ladder — `ladder_cells_…` below proves an aligned cell is ONE run.
        let mut state = 0x5eed_5eed_5eed_5eedu64;
        let mut out = vec![(MortonKey5D(0), MortonKey5D(0)); 1 << 16];
        let mut worst = 0usize;
        for _ in 0..200 {
            let mut b = random_box(&mut state, 1 << AXIS_BITS);
            for a in 0..AXES {
                let extent = (box_rng(&mut state) % 4) as u16;
                b.max[a] = b.min[a].saturating_add(extent).min(AXIS_MASK);
            }
            let n = box5d_to_morton_ranges(&b, &mut out).expect("2^16 runs must hold a 4-extent box");
            assert!(n > 0 && n as u64 <= b.volume(), "never more runs than keys");
            worst = worst.max(n);
            check_cover(&b, &out[..n]);
        }
        eprintln!("[morton-ranges] worst minimal cover over 200 random 4-extent boxes: {worst} runs");
    }

    #[test]
    fn small_lattice_cover_matches_brute_force_membership() {
        const SIDE: u16 = 8; // 8^5 = 32768 keys enumerated per box
        let mut state = 0xa11ce_u64;
        let mut out = vec![(MortonKey5D(0), MortonKey5D(0)); 4096];
        for _ in 0..40 {
            let b = random_box(&mut state, SIDE);
            let n = box5d_to_morton_ranges(&b, &mut out).unwrap();
            let runs = &out[..n];
            let in_cover = |k: MortonKey5D| runs.iter().any(|(lo, hi)| *lo <= k && k <= *hi);
            let mut inside = 0u64;
            for x in 0..SIDE {
                for y in 0..SIDE {
                    for z in 0..SIDE {
                        for t in 0..SIDE {
                            for s in 0..SIDE {
                                let k = MortonKey5D::encode([x, y, z, t, s]);
                                let want = b.contains(k);
                                assert_eq!(in_cover(k), want, "key {:?} vs box {b:?}", k.axes());
                                inside += want as u64;
                            }
                        }
                    }
                }
            }
            assert_eq!(inside, b.volume());
        }
    }

    #[test]
    fn point_box_is_one_key_degenerate_box_is_empty_and_the_whole_space_is_one_run() {
        let mut out = [(MortonKey5D(0), MortonKey5D(0)); 4];
        let k = MortonKey5D::encode([7, 99, 2048, 4095, 0]);
        let point = Box5D { min: k.axes(), max: k.axes() };
        assert_eq!(box5d_to_morton_ranges(&point, &mut out), Some(1));
        assert_eq!(out[0], (k, k));
        let inverted = Box5D { min: [5; AXES], max: [4; AXES] };
        assert_eq!(inverted.volume(), 0);
        assert_eq!(box5d_to_morton_ranges(&inverted, &mut out), Some(0));
        let all = Box5D { min: [0; AXES], max: [AXIS_MASK; AXES] };
        assert_eq!(box5d_to_morton_ranges(&all, &mut out), Some(1));
        assert_eq!(out[0], (MortonKey5D(0), MortonKey5D((1u64 << (AXIS_BITS as usize * AXES)) - 1)));
        let mut too_small: [MortonRange; 0] = [];
        assert_eq!(box5d_to_morton_ranges(&point, &mut too_small), None, "no room → None, never a panic");
    }

    #[test]
    fn ladder_cells_nest_contain_the_key_and_are_exact_box_covers() {
        let k = MortonKey5D::encode([3000, 17, 4094, 1, 2222]);
        let mut prev = ladder_truncate(k, 0);
        assert_eq!(prev, (k, k));
        for lod in 1..=AXIS_BITS {
            let (lo, hi) = ladder_truncate(k, lod);
            assert!(lo <= k && k <= hi);
            assert!(lo <= prev.0 && prev.1 <= hi, "lod {lod} cell must contain the lod {} cell", lod - 1);
            assert_eq!(hi.0 - lo.0 + 1, 1u64 << (lod as usize * AXES));
            // The cell IS an axis-aligned box: its cover is exactly one run.
            let b = Box5D { min: lo.axes(), max: hi.axes() };
            let mut out = [(MortonKey5D(0), MortonKey5D(0)); 2];
            assert_eq!(box5d_to_morton_ranges(&b, &mut out), Some(1));
            assert_eq!(out[0], (lo, hi));
            prev = (lo, hi);
        }
        assert_eq!(ladder_truncate(k, 99), ladder_truncate(k, AXIS_BITS), "lod clamps at AXIS_BITS");
    }

    /// The fixed LCG the rest of the crate uses — same inputs every run.
    fn lcg(s: &mut u64) -> u64 {
        *s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        *s
    }

    // ---- Morton bijection (L07) ---------------------------------------------------

    #[test]
    fn morton_round_trips_origin_and_boundaries() {
        for v in [0u16, 1, 2, AXIS_MASK - 1, AXIS_MASK] {
            for axis in 0..AXES {
                let mut axes = [0u16; AXES];
                axes[axis] = v;
                let k = MortonKey5D::encode(axes);
                assert_eq!(k.axes(), axes, "axis {axis} value {v}");
            }
            assert_eq!(MortonKey5D::encode([v; AXES]).axes(), [v; AXES]);
        }
        assert_eq!(MortonKey5D::encode([0; AXES]).0, 0, "the origin is key zero");
    }

    #[test]
    fn morton_round_trips_a_deterministic_spread_and_keeps_the_top_bits_clear() {
        let mut s = 0x1357_9BDF_2468_ACE0u64;
        for _ in 0..10_000 {
            let axes = [
                (lcg(&mut s) as u16) & AXIS_MASK,
                (lcg(&mut s) as u16) & AXIS_MASK,
                (lcg(&mut s) as u16) & AXIS_MASK,
                (lcg(&mut s) as u16) & AXIS_MASK,
                (lcg(&mut s) as u16) & AXIS_MASK,
            ];
            let k = MortonKey5D::encode(axes);
            assert_eq!(k.axes(), axes);
            assert_eq!(k.0 >> 60, 0, "bits 60..64 must stay zero");
        }
    }

    // A single axis bit lands at `bit * AXES + axis` — the interleave really does
    // interleave, rather than concatenate and still round-trip.
    #[test]
    fn the_interleave_places_bits_where_the_drain_source_did() {
        for axis in 0..AXES {
            for bit in 0..AXIS_BITS as usize {
                let mut axes = [0u16; AXES];
                axes[axis] = 1 << bit;
                assert_eq!(MortonKey5D::encode(axes).0, 1u64 << (bit * AXES + axis));
            }
        }
    }

    // ---- Pruning predicates -------------------------------------------------------

    #[test]
    fn z_plane_isolation_ignores_every_other_axis() {
        let a = MortonKey5D::encode([1, 2, 7, 4, 5]);
        let b = MortonKey5D::encode([9, 9, 7, 9, 9]);
        let c = MortonKey5D::encode([1, 2, 8, 4, 5]);
        assert!(a.same_z_plane(b), "same z, wildly different elsewhere");
        assert!(!a.same_z_plane(c), "one z step apart is a different plane");
    }

    #[test]
    fn the_box_contains_its_edges_and_refuses_one_past_them() {
        let bx = Box5D { min: [10; AXES], max: [20; AXES] };
        assert!(bx.contains(MortonKey5D::encode([10; AXES])));
        assert!(bx.contains(MortonKey5D::encode([20; AXES])));
        assert!(bx.contains(MortonKey5D::encode([10, 20, 15, 12, 18])));
        assert!(!bx.contains(MortonKey5D::encode([9, 15, 15, 15, 15])));
        assert!(!bx.contains(MortonKey5D::encode([15, 15, 15, 15, 21])));
    }

    // ---- Field scoring (Stage 4) --------------------------------------------------

    fn vec_of(vals: [u64; AXES]) -> HypersphereVector5D {
        HypersphereVector5D {
            components: [
                MersenneScalar::new(vals[0]),
                MersenneScalar::new(vals[1]),
                MersenneScalar::new(vals[2]),
                MersenneScalar::new(vals[3]),
                MersenneScalar::new(vals[4]),
            ],
        }
    }

    /// The oracle: the same sum in `u128`, reduced by `%`, no fold cleverness.
    fn dot_oracle(a: &HypersphereVector5D, b: &HypersphereVector5D) -> u64 {
        let mut acc: u128 = 0;
        for i in 0..AXES {
            acc += (a.components[i].0 as u128) * (b.components[i].0 as u128);
        }
        (acc % (M61 as u128)) as u64
    }

    #[test]
    fn the_dot_agrees_with_the_modulo_oracle_over_a_deterministic_spread() {
        let mut s = 0xFEED_FACE_CAFE_BEEFu64;
        for _ in 0..10_000 {
            let a = vec_of([lcg(&mut s), lcg(&mut s), lcg(&mut s), lcg(&mut s), lcg(&mut s)]);
            let b = vec_of([lcg(&mut s), lcg(&mut s), lcg(&mut s), lcg(&mut s), lcg(&mut s)]);
            assert_eq!(mersenne_dot(&a, &b).0, dot_oracle(&a, &b));
            assert!(mersenne_dot(&a, &b).0 < M61);
        }
    }

    // The accumulator's worst case: every component at the field maximum.
    #[test]
    fn the_dot_survives_five_maximal_products() {
        let top = vec_of([M61 - 1; AXES]);
        assert_eq!(mersenne_dot(&top, &top).0, dot_oracle(&top, &top));
    }

    #[test]
    fn the_dot_is_symmetric_and_zero_annihilates() {
        let mut s = 0x0123_4567_89AB_CDEFu64;
        for _ in 0..1_000 {
            let a = vec_of([lcg(&mut s), lcg(&mut s), lcg(&mut s), lcg(&mut s), lcg(&mut s)]);
            let b = vec_of([lcg(&mut s), lcg(&mut s), lcg(&mut s), lcg(&mut s), lcg(&mut s)]);
            assert_eq!(mersenne_dot(&a, &b), mersenne_dot(&b, &a));
            assert_eq!(mersenne_dot(&a, &HypersphereVector5D::ZERO), MersenneScalar::ZERO);
        }
    }

    #[test]
    fn a_scalar_is_reduced_at_construction() {
        assert_eq!(MersenneScalar::new(M61).0, 0);
        assert_eq!(MersenneScalar::new(u64::MAX).0, u64::MAX % M61);
    }

    #[test]
    fn on_sphere_is_the_self_dot() {
        let v = vec_of([3, 4, 0, 0, 0]);
        assert!(v.is_on_sphere(MersenneScalar::new(25)));
        assert!(!v.is_on_sphere(MersenneScalar::new(24)));
    }

    // ---- DAG order (Layer 1) ------------------------------------------------------

    #[test]
    fn the_edge_gate_holds_the_spec_inequalities() {
        let at = |t: u16, s: u16| MortonKey5D::encode([0, 0, 0, t, s]);
        assert!(edge_is_forward(at(5, 5), at(6, 5)), "ΔT=1 ΔS=0");
        assert!(edge_is_forward(at(5, 5), at(5, 6)), "ΔT=0 ΔS=1");
        assert!(edge_is_forward(at(5, 5), at(7, 4)), "ΔT=2 ΔS=-1 sums above zero");
        assert!(!edge_is_forward(at(5, 5), at(7, 3)), "ΔT=2 ΔS=-2 sums to zero");
        assert!(!edge_is_forward(at(5, 5), at(5, 5)), "the self-edge is the smallest cycle");
        assert!(!edge_is_forward(at(5, 5), at(4, 9)), "ΔT<0 is refused however large ΔS");
        assert!(!edge_is_forward(at(5, 5), at(6, 4)), "ΔT=1 ΔS=-1 sums to zero");
        assert!(!edge_is_forward(at(5, 5), at(6, 2)), "ΔT=1 ΔS=-3 sums below zero");
    }

    // Acyclicity is a consequence, not a search: T+S strictly increases along every
    // accepted edge, so no chain of accepted edges returns to its origin.
    #[test]
    fn every_accepted_edge_strictly_raises_t_plus_s() {
        let mut s = 0xA5A5_5A5A_F00D_D00Du64;
        for _ in 0..10_000 {
            let a = MortonKey5D::encode([0, 0, 0, (lcg(&mut s) as u16) & AXIS_MASK, (lcg(&mut s) as u16) & AXIS_MASK]);
            let b = MortonKey5D::encode([0, 0, 0, (lcg(&mut s) as u16) & AXIS_MASK, (lcg(&mut s) as u16) & AXIS_MASK]);
            if edge_is_forward(a, b) {
                let (aa, bb) = (a.axes(), b.axes());
                assert!(
                    (bb[AXIS_T] as u32 + bb[AXIS_S] as u32) > (aa[AXIS_T] as u32 + aa[AXIS_S] as u32)
                );
            }
        }
    }

    // ---- The node -----------------------------------------------------------------

    fn terminal_node_at(axes: [u16; AXES]) -> RamusPrimeNode {
        RamusPrimeNode {
            morton_key: MortonKey5D::encode(axes),
            pexil: Pexil {
                lattice: TritCell5D(Sentinel::NullNode as u8),
                validity: crate::atom::ValidityMask::ALL_KNOWN,
                ordinal: crate::atom::CellOrdinal(0),
                payload: [0; 4],
            },
            edge_target: MortonKey5D(0),
            hypersphere: HypersphereVector5D::ZERO,
        }
    }

    #[test]
    fn a_sentinel_verb_is_no_edge_at_all() {
        let n = terminal_node_at([1; AXES]);
        assert!(n.pexil.lattice.is_sentinel());
        assert_eq!(n.outgoing_edge(), None);
    }

    #[test]
    fn try_link_accepts_forward_refuses_backward_and_refuses_a_sentinel_verb() {
        let mut n = terminal_node_at([0, 0, 0, 5, 5]);
        let fwd = MortonKey5D::encode([0, 0, 0, 6, 5]);
        let back = MortonKey5D::encode([0, 0, 0, 4, 5]);
        let verb = TritCell5D::from_trits([1, 0, 0, 1, 0]);

        assert!(!n.try_link(back, verb), "backward edge refused");
        assert_eq!(n.outgoing_edge(), None, "a refused link mutates nothing");

        assert!(!n.try_link(fwd, TritCell5D(Sentinel::Tombstone as u8)), "sentinel verb refused");
        assert_eq!(n.outgoing_edge(), None);

        assert!(n.try_link(fwd, verb));
        assert_eq!(n.outgoing_edge(), Some((fwd, [1, 0, 0, 1, 0])));
    }

    // The outland semantics, restated on the flat node: -1/0/+1 per axis, zero is
    // indifference, any disagreement on a constrained axis refuses the branch.
    #[test]
    fn the_ternary_branch_step_follows_agreement_and_refuses_conflict() {
        let mut n = terminal_node_at([0, 0, 0, 5, 5]);
        assert_eq!(n.follow([0; AXES]), None, "a terminal node follows nowhere");

        let fwd = MortonKey5D::encode([0, 0, 0, 6, 5]);
        assert!(n.try_link(fwd, TritCell5D::from_trits([1, -1, 0, 1, 0])));

        assert_eq!(n.follow([0; AXES]), Some(fwd), "an unconstrained query always descends");
        assert_eq!(n.follow([1, 0, 0, 0, 0]), Some(fwd), "agreement on x");
        assert_eq!(n.follow([1, -1, 0, 1, 0]), Some(fwd), "full agreement");
        assert_eq!(n.follow([-1, 0, 0, 0, 0]), None, "conflict on x refuses");
        assert_eq!(n.follow([0, 0, 1, 0, 0]), None, "constraining the verb's zero axis refuses");
    }

    // ---- Hypersphere blending (Part 2) ------------------------------------------------

    #[test]
    fn axes_distance_computes_manhattan_metric() {
        let a = MortonKey5D::encode([0, 0, 0, 0, 0]);
        let b = MortonKey5D::encode([3, 4, 0, 0, 0]);
        assert_eq!(axes_distance(a, b), 7);

        let c = MortonKey5D::encode([0, 0, 0, 5, 5]);
        assert_eq!(axes_distance(a, c), 10);
    }

    #[test]
    fn axes_distance_is_symmetric() {
        let a = MortonKey5D::encode([1, 2, 3, 4, 5]);
        let b = MortonKey5D::encode([6, 7, 8, 9, 10]);
        assert_eq!(axes_distance(a, b), axes_distance(b, a));
    }

    #[test]
    fn mersenne_weighted_sum_sums_linearly() {
        let v1 = vec_of([1, 2, 3, 4, 5]);
        let v2 = vec_of([5, 4, 3, 2, 1]);
        let weights = [1i32, 2i32];
        let points = [v1, v2];

        let result = mersenne_weighted_sum(&weights, &points);
        let oracle_sum: u128 = (1u128 * 1 + 2u128 * 5) % (M61 as u128);
        assert_eq!(result.components[0].0 as u128 % M61 as u128, oracle_sum);
    }

    #[test]
    fn mersenne_weighted_sum_of_empty_is_zero() {
        let weights = [];
        let points = [];
        let result = mersenne_weighted_sum(&weights, &points);
        assert_eq!(result, HypersphereVector5D::ZERO);
    }

    #[test]
    fn sample_blend_on_empty_slice_returns_zero() {
        let query = MortonKey5D::encode([5, 5, 5, 5, 5]);
        let result = sample_blend(&[], query, 3);
        assert_eq!(result, HypersphereVector5D::ZERO);
    }

    #[test]
    fn sample_blend_clamps_k_to_slice_length() {
        let nodes = [
            RamusPrimeNode {
                morton_key: MortonKey5D::encode([0, 0, 0, 0, 0]),
                pexil: Pexil {
                    lattice: TritCell5D::ORIGIN,
                    validity: crate::atom::ValidityMask::ALL_KNOWN,
                    ordinal: crate::atom::CellOrdinal(0),
                    payload: [0; 4],
                },
                edge_target: MortonKey5D(0),
                hypersphere: vec_of([1, 0, 0, 0, 0]),
            },
            RamusPrimeNode {
                morton_key: MortonKey5D::encode([1, 0, 0, 0, 0]),
                pexil: Pexil {
                    lattice: TritCell5D::ORIGIN,
                    validity: crate::atom::ValidityMask::ALL_KNOWN,
                    ordinal: crate::atom::CellOrdinal(0),
                    payload: [0; 4],
                },
                edge_target: MortonKey5D(0),
                hypersphere: vec_of([0, 1, 0, 0, 0]),
            },
        ];

        let query = MortonKey5D::encode([0, 0, 0, 0, 0]);
        let result_k3 = sample_blend(&nodes, query, 3);
        let result_k2 = sample_blend(&nodes, query, 2);
        assert_eq!(result_k3, result_k2, "k=3 on 2-node slice should match k=2");
    }

    #[test]
    fn sample_blend_selects_k_nearest_not_first_k() {
        // Test that with k < candidates.len(), we get the k *nearest*, not first k
        let nodes = [
            RamusPrimeNode {
                morton_key: MortonKey5D::encode([100, 0, 0, 0, 0]),  // far
                pexil: Pexil {
                    lattice: TritCell5D::ORIGIN,
                    validity: crate::atom::ValidityMask::ALL_KNOWN,
                    ordinal: crate::atom::CellOrdinal(0),
                    payload: [0; 4],
                },
                edge_target: MortonKey5D(0),
                hypersphere: vec_of([1, 0, 0, 0, 0]),
            },
            RamusPrimeNode {
                morton_key: MortonKey5D::encode([1, 0, 0, 0, 0]),  // very close
                pexil: Pexil {
                    lattice: TritCell5D::ORIGIN,
                    validity: crate::atom::ValidityMask::ALL_KNOWN,
                    ordinal: crate::atom::CellOrdinal(0),
                    payload: [0; 4],
                },
                edge_target: MortonKey5D(0),
                hypersphere: vec_of([2, 0, 0, 0, 0]),
            },
            RamusPrimeNode {
                morton_key: MortonKey5D::encode([50, 0, 0, 0, 0]),  // medium
                pexil: Pexil {
                    lattice: TritCell5D::ORIGIN,
                    validity: crate::atom::ValidityMask::ALL_KNOWN,
                    ordinal: crate::atom::CellOrdinal(0),
                    payload: [0; 4],
                },
                edge_target: MortonKey5D(0),
                hypersphere: vec_of([3, 0, 0, 0, 0]),
            },
        ];

        let query = MortonKey5D::encode([0, 0, 0, 0, 0]);

        // k=1: should select closest (index 1, distance 1)
        let result_k1 = sample_blend(&nodes, query, 1);
        assert_eq!(result_k1, nodes[1].hypersphere, "k=1 should select nearest neighbor");

        // k=2: should select two nearest (indices 1,2 with distances 1,50)
        let result_k2 = sample_blend(&nodes, query, 2);
        // Since both nodes have positive components, their average should have positive components
        assert!(result_k2.components[0].0 > 0, "blend of positive vectors should be positive");
    }

    // ---- The glass bridge (L07) ---------------------------------------------------

    #[test]
    fn every_boundary_key_round_trips_through_the_hud_point() {
        for v in [0u16, 1, 2, 3, 4, 40, 121, 242, 243, AXIS_MASK - 1, AXIS_MASK] {
            for axis in 0..AXES {
                let mut axes = [0u16; AXES];
                axes[axis] = v;
                let k = MortonKey5D::encode(axes);
                assert_eq!(point_to_key(&key_to_point(k)), Some(k), "axis {axis} value {v}");
            }
            let k = MortonKey5D::encode([v; AXES]);
            assert_eq!(point_to_key(&key_to_point(k)), Some(k));
        }
    }

    #[test]
    fn a_deterministic_spread_of_keys_round_trips_through_the_hud_point() {
        let mut s = 0xBEE5_1DEA_0DDB_A11Du64;
        for _ in 0..10_000 {
            let axes = [
                (lcg(&mut s) as u16) & AXIS_MASK,
                (lcg(&mut s) as u16) & AXIS_MASK,
                (lcg(&mut s) as u16) & AXIS_MASK,
                (lcg(&mut s) as u16) & AXIS_MASK,
                (lcg(&mut s) as u16) & AXIS_MASK,
            ];
            let k = MortonKey5D::encode(axes);
            assert_eq!(point_to_key(&key_to_point(k)), Some(k));
        }
    }

    // The key band is the 9 leaf depths; everything above stays at the origin, and
    // the leaf carries the least significant digit.
    #[test]
    fn the_key_occupies_the_leaf_band_and_nothing_above_it() {
        let mut axes = [0u16; AXES];
        axes[AXIS_Y] = 1; // one unit: a single +1 trit in the leaf slice, lane Y
        let p = key_to_point(MortonKey5D::encode(axes));
        for d in 0..DEPTH - 1 {
            assert_eq!(p.slices[d], TritCell5D::ORIGIN, "depth {d} must be origin");
        }
        assert_eq!(p.slices[DEPTH - 1].trits(), Some([0, 1, 0, 0, 0]));
    }

    #[test]
    fn a_point_that_is_not_a_key_is_refused_not_guessed() {
        // A sentinel slice in the key band.
        let mut p = key_to_point(MortonKey5D::encode([7; AXES]));
        p.slices[DEPTH - 1] = TritCell5D(Sentinel::Tombstone as u8);
        assert_eq!(point_to_key(&p), None);

        // A non-origin depth above the key band.
        let mut p = key_to_point(MortonKey5D::encode([7; AXES]));
        p.slices[0] = TritCell5D::from_trits([1, 0, 0, 0, 0]);
        assert_eq!(point_to_key(&p), None);

        // A negative digit sum: -1 in the most significant key slice, zeros below.
        let mut p = PackedPoint105::ORIGIN;
        p.slices[DEPTH - KEY_TRIT_DIGITS] = TritCell5D::from_trits([-1, 0, 0, 0, 0]);
        assert_eq!(point_to_key(&p), None, "a negative axis is not a coordinate");
    }

    // The whole path the conductor sees: key -> point -> pixels -> point -> key.
    #[test]
    fn a_key_survives_the_full_glass_round_trip() {
        let k = MortonKey5D::encode([1000, 2000, 3000, 4000, 4095]);
        let px = crate::grid::point_to_pixels(&key_to_point(k));
        assert_eq!(point_to_key(&crate::grid::pixels_to_point(&px)), Some(k));
    }

    #[test]
    fn the_node_is_one_cache_line() {
        assert_eq!(core::mem::size_of::<RamusPrimeNode>(), 64);
        assert_eq!(core::mem::align_of::<RamusPrimeNode>(), 64);
    }
}
