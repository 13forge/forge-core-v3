//! Directed traversal of the 5-lane lattice — one integer DDA, any predicate.
//!
//! A ray here is `(origin, direction)` where the direction is a [`TritCell5D`]: five balanced
//! trits, one per lane, each `-1` / `0` / `+1`. The zero is load-bearing and is not "no
//! input" — it is the fixed point of the mirror involution (`PARARITY.md` §3 Corollary 2,
//! `n=3, k=1`), and it means **this lane does not participate in the march**. That is exactly
//! what a partial-axis ray needs, and it is why `3^5 = 243` enumerates every direction in a
//! 5-lane lattice — including all the degenerate ones — inside a single byte.
//!
//! ## Why the predicate is a parameter
//!
//! The same walk serves domains that share this lattice's *shape* but not its meaning:
//!
//! - **geometric** — `hit = |c| world.is_solid_at(cell, tick)`; the answer is terrain.
//! - **semantic** — `hit = |c| squared_distance(c, entry) <= r2`; the answer is a codebook id
//!   (`forge-ml-bqrouter::nearest_neighbor`, `EMBED_DIM = 5`).
//! - **syntactic** — the same, over `embed_river_line`'s five independent syntactic features
//!   (tag / payload / shape / token-order / token-set).
//!
//! ## The lane orderings are a permutation, not a divergence
//!
//! The world grid is `[X, Y, Z, T, S]` (`grid.rs:10`); the semantic box and `Point5D` are
//! `[x, y, z, theta, w]` (`nearest_neighbor.rs:71`,
//! `forge-audio-v3::dimensional_collapse::Point5D`). An earlier draft of this doc called that
//! a divergence at lane 3 (time vs angle) and refused to relate them. That was flat 5-tuple
//! thinking. `collapse_5d_to_stereo` gives EVERY lane a physical decode — X → pan + ITD,
//! Y → gain + air-absorption lowpass, Z → root fundamental, θ → overtone richness + phase,
//! W → modulation rate — so both orderings carry the same five axes, with time at `w` and
//! substrate/overtone at `theta`. The difference is an index permutation a caller knows, not
//! two unrelated spaces.
//!
//! **APERTURE (C09):** what this module still refuses to assume is WHICH lane wraps. `theta`
//! is angular (`rem_euclid(360_000)` mdeg, matching `Point5D::theta_mdeg`); the world's lanes
//! are linear. So the wrap modulus is a per-lane parameter supplied by the caller, and this
//! module asserts no lane meaning of its own. Hardcoding either ordering here is how the two
//! spaces would silently desync.
//!
//! ## One point, several decoders
//!
//! The same 5D coordinate already has independent, landed decoders in this tree — which is
//! why a march over it is worth having:
//! - **sound** — `collapse_5d_to_stereo` / `collapse_5d_to_surround` (5.1, 6 channels).
//! - **identity** — `cree_code_to_point` / `cree_point_to_code`: a `u8` ↔ 5D bijection, the
//!   same shape as [`TritCell5D`]'s own `u8` ↔ `[i8; 5]`, arrived at independently.
//! - **semantic / syntactic** — `nearest_neighbor`'s `EMBED_DIM = 5` box.
//!
//! A ray therefore does not only find a cell; it finds a cell that can be heard, named, and
//! matched. This module supplies the walk and nothing else.

use crate::atom::TritCell5D;
use crate::grid::LANES;

/// A lane that does not wrap. Linear lanes carry this in the `wrap` array.
pub const NO_WRAP: i64 = 0;

/// Where a march stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The coordinate the predicate accepted.
    pub cell: [i64; LANES],
    /// How many steps were taken to reach it. `0` means the origin itself hit.
    pub steps: u8,
}

/// Why a march produced no hit. Every variant is loud — a march never returns a
/// plain `None` that could mean either "nothing there" or "you asked for nothing".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarchEnd {
    /// `max_steps` was exhausted with the predicate never accepting.
    Exhausted,
    /// The direction byte was a sentinel, not a coordinate — it decodes to no trits at all.
    /// Marching it would be marching in a direction that does not exist.
    SentinelDirection,
    /// Every lane sat on its pararity fixed point, so no step could ever change the
    /// coordinate. Walking it would loop forever without moving; refused instead.
    NullDirection,
}

/// Step `origin` along `dir` until `hit` accepts, or `max_steps` is spent.
///
/// `wrap[l]` is lane `l`'s modulus, or [`NO_WRAP`] for a linear lane. A wrapping lane uses
/// `rem_euclid`, so `-1` from `0` lands on `modulus - 1` rather than going negative — the same
/// convention `forge-audio`'s `theta_mdeg` and `nearest_neighbor`'s angular lane already use.
///
/// The origin is tested FIRST: a ray that starts inside a solid should report that solid, not
/// step over it. Integer throughout, no allocation, no `sqrt`.
pub fn march(
    origin: [i64; LANES],
    dir: TritCell5D,
    wrap: [i64; LANES],
    max_steps: u8,
    mut hit: impl FnMut([i64; LANES]) -> bool,
) -> Result<Hit, MarchEnd> {
    let Some(step) = dir.trits() else {
        return Err(MarchEnd::SentinelDirection);
    };
    if hit(origin) {
        return Ok(Hit { cell: origin, steps: 0 });
    }
    if step.iter().all(|&s| s == 0) {
        // Refused, not looped: with no participating lane the coordinate is a constant, so
        // any further step is the same failing test repeated `max_steps` times.
        return Err(MarchEnd::NullDirection);
    }
    let mut cell = origin;
    for n in 1..=max_steps {
        for l in 0..LANES {
            cell[l] += step[l] as i64;
            if wrap[l] != NO_WRAP {
                cell[l] = cell[l].rem_euclid(wrap[l]);
            }
        }
        if hit(cell) {
            return Ok(Hit { cell, steps: n });
        }
    }
    Err(MarchEnd::Exhausted)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINEAR: [i64; LANES] = [NO_WRAP; LANES];

    fn dir(t: [i8; LANES]) -> TritCell5D {
        TritCell5D::from_trits(t)
    }

    #[test]
    fn the_origin_is_tested_before_any_step() {
        let h = march([5, 0, 0, 0, 0], dir([1, 0, 0, 0, 0]), LINEAR, 8, |c| c[0] == 5).unwrap();
        assert_eq!(h.steps, 0, "a ray starting inside the target must report it, not step over");
    }

    #[test]
    fn it_walks_one_lane_until_the_predicate_accepts() {
        let h = march([0; LANES], dir([1, 0, 0, 0, 0]), LINEAR, 16, |c| c[0] == 4).unwrap();
        assert_eq!(h.steps, 4);
        assert_eq!(h.cell, [4, 0, 0, 0, 0]);
    }

    #[test]
    fn a_diagonal_advances_every_participating_lane_together() {
        let h = march([0; LANES], dir([1, -1, 0, 0, 0]), LINEAR, 8, |c| c[1] == -3).unwrap();
        assert_eq!(h.cell, [3, -3, 0, 0, 0], "lanes must step in lockstep, not sequentially");
    }

    /// The pararity zero means "hold this lane" — a lane at its fixed point must never move.
    #[test]
    fn a_zero_lane_never_advances() {
        let h = march([0; LANES], dir([0, 0, 1, 0, 0]), LINEAR, 8, |c| c[2] == 5).unwrap();
        assert_eq!(h.cell[0], 0);
        assert_eq!(h.cell[1], 0);
        assert_eq!(h.cell[4], 0);
    }

    #[test]
    fn exhaustion_is_loud() {
        assert_eq!(
            march([0; LANES], dir([1, 0, 0, 0, 0]), LINEAR, 3, |_| false),
            Err(MarchEnd::Exhausted)
        );
    }

    /// An all-zero direction cannot move, so it is refused rather than silently spun.
    #[test]
    fn the_null_direction_is_refused_not_looped() {
        assert_eq!(
            march([0; LANES], TritCell5D::ORIGIN, LINEAR, 200, |_| false),
            Err(MarchEnd::NullDirection)
        );
    }

    /// A sentinel byte is a control state, not a coordinate — it has no direction to walk.
    #[test]
    fn a_sentinel_is_not_a_direction() {
        let sentinel = TritCell5D(250);
        assert!(sentinel.is_sentinel());
        assert_eq!(
            march([0; LANES], sentinel, LINEAR, 8, |_| true),
            Err(MarchEnd::SentinelDirection)
        );
    }

    /// The angular lane wraps instead of running negative — 1 step back from 0 is the top of
    /// the circle, matching `theta_mdeg`'s `rem_euclid(360_000)` convention.
    #[test]
    fn a_wrapping_lane_comes_round_rather_than_going_negative() {
        let mut wrap = LINEAR;
        wrap[3] = 360_000;
        let h = march([0; LANES], dir([0, 0, 0, -1, 0]), wrap, 2, |c| c[3] == 359_999).unwrap();
        assert_eq!(h.steps, 1, "one step back from 0 must land on 359_999, not -1");
    }

    /// THE POINT OF THE MODULE: one walk, two domains, only the predicate changes.
    ///
    /// This is a UNIT proof, not an integration one, and the distinction is deliberate —
    /// Crate Zero imports nothing, so it cannot call `World5D::is_solid_at` or
    /// `nearest_neighbor::squared_distance` here. Each predicate below models its domain's
    /// SHAPE (a solid set; a sqrt-free radius test) rather than invoking the real one. The
    /// live geometric caller is the shell's brush; a live semantic caller does not exist yet
    /// and is not claimed to (C11: one oracle so far, not two).
    #[test]
    fn one_march_serves_a_geometric_and_a_semantic_predicate() {
        // Geometric: a solid slab at z >= 3. The answer is "where does the ray enter rock".
        let solid = |c: [i64; LANES]| c[2] >= 3;
        let g = march([0, 0, 0, 0, 0], dir([0, 0, 1, 0, 0]), LINEAR, 16, solid).unwrap();
        assert_eq!(g.cell[2], 3, "geometric ray must stop on the first solid cell");
        assert_eq!(g.steps, 3);

        // Semantic: a codebook entry accepted inside a squared radius — no sqrt, exactly how
        // `nearest_neighbor` compares. The ray reports the first cell that ENTERS the entry's
        // neighbourhood, which is the nearest-neighbour answer; it does not walk on to the
        // centre. (Measured, not assumed: with r2=2, [3,3] is already at d2 = 1+1 = 2.)
        let entry = [4_i64, 4, 0, 0, 0];
        let r2 = 2_i64;
        let near = |c: [i64; LANES]| {
            let mut d2 = 0;
            for l in 0..LANES {
                let d = c[l] - entry[l];
                d2 += d * d;
            }
            d2 <= r2
        };
        let s = march([0; LANES], dir([1, 1, 0, 0, 0]), LINEAR, 16, near).unwrap();
        assert_eq!(s.cell, [3, 3, 0, 0, 0], "semantic ray stops on entry to the neighbourhood");
        assert_eq!(s.steps, 3);

        // Tighten the radius to an exact match and the same walk lands on the entry itself.
        let exact = |c: [i64; LANES]| c == entry;
        let e = march([0; LANES], dir([1, 1, 0, 0, 0]), LINEAR, 16, exact).unwrap();
        assert_eq!(e.cell, entry);
        assert_eq!(e.steps, 4);
    }

    /// Marching forward `n` and back `n` returns to the origin — the walk is reversible
    /// because the direction's mirror is its own inverse (L07 bijection).
    #[test]
    fn the_reverse_march_returns_to_the_origin() {
        let out = march([0; LANES], dir([1, 1, -1, 0, 0]), LINEAR, 8, |c| c[0] == 5).unwrap();
        let back = march(out.cell, dir([-1, -1, 1, 0, 0]), LINEAR, 8, |c| c[0] == 0).unwrap();
        assert_eq!(back.cell, [0; LANES], "f-inverse(f(x)) must be x");
    }
}
