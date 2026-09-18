//! The second-kind field — `(I − λK) f = g` over the 5D substrate, in FIXED POINT.
//!
//! This is the matrix generalisation of [`crate::decay::LeakyPermyriad`]: a scalar
//! leak becomes an N×N coupling `M = λK` in permyriad, and the scalar equilibrium
//! `rate·PMY/leak` becomes the resolved field `(I − M)⁻¹ g`. A diagonal `M`
//! reproduces N independent leaky integrators exactly (`tests::diagonal_field_is_decay`).
//!
//! WHY FIXED POINT, NOT F_M61 (drain notes part 5, ARCH000 2026-08-10): the Neumann
//! series `Σ Mⁿ g` converges because `‖M‖ < 1` makes higher hops *decay*. Decay is an
//! ordered-field property — a finite field has no order, no norm, no small. Mod-p there
//! is no convergence to ride, only wraparound. So the coupling lives in permyriad
//! (`10_000 == 1.0`) where `‖M‖∞ < PMY` is a real, checkable convergence guard, and
//! `F_M61` stays where it belongs: identity and exact scoring, never magnitudes.
//!
//! TWO DIRECTIONS, and the cheap one is the inverse:
//! - `deproject`  `f → g = (I − M) f`  — ONE exact O(N²) pass. The inverse operator.
//! - `resolve`    `g → f = (I − M)⁻¹ g` — Neumann iteration to a fixed point. Costly.
//!
//! `resolve ∘ deproject` and `deproject ∘ resolve` are the identity within flooring
//! distance — the L07 bijection discipline, now over field dynamics.
//!
//! ## The boundary is deliberate — what this primitive REFUSES
//!
//! `Field5D` is the DAMPED, DENSE, SMALL-HOLON field solver. Its `‖M‖∞ < PMY`
//! guard is a competence boundary, not an incidental bound. Three regimes live
//! outside it, each needing a *different* primitive — never a loosened guard here:
//!
//! 1. CONSERVATIVE / UNDAMPED (`‖M‖∞ = 1`): ideal springs, frictionless orbits,
//!    lossless reflections. Eigenvalues sit ON the unit circle — energy circulates
//!    forever, the Neumann series does not converge, and `(I − M)` has no bounded
//!    inverse. There is NO equilibrium for a resolvent to find, so refusal is
//!    correct. The strict `<` (never `<=`) is load-bearing: admitting `‖M‖∞ == 1`
//!    would let `resolve` spin to `max_iters` on a problem with no fixed point.
//!    Conservative dynamics belong to a SYMPLECTIC integrator (Verlet/leapfrog)
//!    that steps time and conserves energy — a sibling, not a relaxation of this.
//!
//! 2. LARGE & SPARSE (`N > 256`): dense `[[i64;N];N]` is O(N²) memory and per-hop,
//!    right for small holons (audio N=32, a PexilLine N=8) and wrong for a big
//!    graph. A 5D Morton coupling is sparse (a node touches a few Cremantics
//!    neighbours, not all N). That wants a sparse-backed sibling where the same
//!    Neumann iteration `f ← g + Mf` is O(edges) message-passing along the DAG.
//!    Same math, sparse store — not this dense struct.
//!
//! 3. ANISOTROPIC DAMPING (`ρ(M) < 1` but `‖M‖∞ ≥ 1`): genuinely convergent yet
//!    false-rejected, because the infinity norm is a SUFFICIENT not NECESSARY
//!    contraction test (`‖M‖∞ ≥ ρ(M)`). Inf-norm is the cheap honest floor; a
//!    tighter admission test (Gershgorin discs / power-iteration ρ estimate) is a
//!    future refinement if the false-reject ever bites.

use crate::decay::PMY;

/// Truncating `a·b / PMY` in `i128`, deterministic on every target. Truncation
/// toward zero is Rust's integer-division rule; it is the flooring leak of the
/// scalar primitive, one dimension up.
#[inline(always)]
const fn scale(a: i64, b: i64) -> i128 {
    (a as i128 * b as i128) / PMY as i128
}

/// Discrete Macaulay bracket `⟨x − a⟩ⁿ` for `n ≥ 0` — the integer-only sibling of
/// SymPy's `SingularityFunction` (verified 2026-08-14: zero prior art for this in
/// `F:\v3`, a genuinely new primitive, not a port). Where SymPy's class carries a
/// symbolic tree over continuous/complex domains to represent a point impulse
/// (`n=-1`), a Heaviside step (`n=0`), or a polynomial ramp (`n≥1`) driving a
/// Fredholm operator, this is the SAME shape — a boundary condition or sudden
/// drive shaping the `g` this module's `resolve`/`deproject` take as input — with
/// zero symbolic overhead: one saturating subtract, one saturating power, no
/// branch on the hot path beyond the sign check the bracket itself IS.
///
/// `n < 0` (the delta impulse `n=-1`, the doublet `n=-2`) is OUT OF SCOPE here —
/// named, not silently dropped: those are distributional objects (they integrate
/// to a value at a single point, they are not a pointwise function), and this
/// repo's own no-alloc/no-symbolic law has no honest integer encoding for "the
/// thing you get when you differentiate a step" without inventing a second,
/// heavier primitive. If a caller needs an impulse response, model it as an
/// explicit lookup-table entry at the boundary tick, not a bracket evaluation.
#[inline(always)]
pub const fn macaulay_pow(x: i64, a: i64, n: u32) -> i64 {
    let diff = x.saturating_sub(a);
    if diff > 0 { diff.saturating_pow(n) } else { 0 }
}

/// An N-channel second-kind coupling `M = λK`, permyriad entries. Constructed only
/// when the Neumann series is guaranteed to converge: the infinity norm (max row
/// absolute sum) is strictly below `PMY`, i.e. `‖M‖∞ < 1`. That bounds the spectral
/// radius below 1, so `(I − M)⁻¹ = Σ Mⁿ` converges — the real convergence the
/// finite field could not provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field5D<const N: usize> {
    /// Row-major coupling. `m[i][j]` = parts-per-myriad of channel `j` folded into `i`.
    m: [[i64; N]; N],
}

impl<const N: usize> Field5D<N> {
    /// Build a field, or refuse a divergent coupling. `None` when any row's absolute
    /// entries sum to `PMY` or more — that coupling has no bounded equilibrium and
    /// `resolve` would not settle.
    pub fn new(m: [[i64; N]; N]) -> Option<Self> {
        let mut i = 0;
        while i < N {
            let mut row_abs: i128 = 0;
            let mut j = 0;
            while j < N {
                row_abs += (m[i][j] as i128).abs();
                j += 1;
            }
            if row_abs >= PMY as i128 {
                return None;
            }
            i += 1;
        }
        Some(Self { m })
    }

    /// The INVERSE operator `g = (I − M) f`. One exact O(N²) pass, no iteration:
    /// `g[i] = f[i] − Σ_j M[i][j]·f[j] / PMY`. Given a settled field, recover its
    /// driving input.
    pub fn deproject(&self, f: &[i64; N]) -> [i64; N] {
        let mut g = [0i64; N];
        for i in 0..N {
            let mut coupled: i128 = 0;
            for j in 0..N {
                coupled += scale(self.m[i][j], f[j]);
            }
            g[i] = f[i] - coupled as i64;
        }
        g
    }

    /// The resolvent `f = (I − M)⁻¹ g`, summed numerically by Neumann iteration
    /// `f ← g + M f` to a fixed point. `None` if it has not settled within
    /// `max_iters` (a converging field settles fast; a non-settling one is a defect,
    /// never a silent best-effort). Deterministic: same inputs, same iterate count.
    pub fn resolve(&self, g: &[i64; N], max_iters: u32) -> Option<[i64; N]> {
        let mut f = *g;
        for _ in 0..max_iters {
            let mut next = [0i64; N];
            for i in 0..N {
                let mut coupled: i128 = 0;
                for j in 0..N {
                    coupled += scale(self.m[i][j], f[j]);
                }
                next[i] = g[i] + coupled as i64;
            }
            if next == f {
                return Some(next);
            }
            f = next;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decay::LeakyPermyriad;

    // n=0 is the discrete Heaviside step: 0 at and before the boundary, 1 strictly after.
    #[test]
    fn n_zero_is_the_heaviside_step() {
        assert_eq!(macaulay_pow(4, 5, 0), 0, "x < a: below the step");
        assert_eq!(macaulay_pow(5, 5, 0), 0, "x == a: the bracket's own convention is 0 at the boundary, not 1/2");
        assert_eq!(macaulay_pow(6, 5, 0), 1, "x > a: past the step");
    }

    // n=1 is a ramp: 0 up to and at the boundary, then (x-a) linearly.
    #[test]
    fn n_one_is_a_ramp_from_the_boundary() {
        assert_eq!(macaulay_pow(3, 5, 1), 0);
        assert_eq!(macaulay_pow(5, 5, 1), 0);
        assert_eq!(macaulay_pow(6, 5, 1), 1);
        assert_eq!(macaulay_pow(9, 5, 1), 4);
    }

    // The docstring's own worked example: SingularityFunction(4,1,5) -> 243 = 3^5,
    // this repo's own trit-per-byte capacity (TritCell5D, 3^5=243). Same arithmetic,
    // proven here rather than just asserted in prose.
    #[test]
    fn the_243_worked_example_matches_trit_capacity() {
        assert_eq!(macaulay_pow(4, 1, 5), 243);
        assert_eq!(macaulay_pow(4, 1, 5), 3i64.pow(5));
    }

    // Never panics on inputs that would overflow plain subtraction or plain pow —
    // the saturating ops are load-bearing, not decorative. Sabotage-style proof:
    // swap saturating_sub/saturating_pow for the bare operators and this test
    // panics in a debug build instead of returning a value.
    #[test]
    fn overflow_saturates_instead_of_panicking() {
        assert_eq!(macaulay_pow(i64::MAX, i64::MIN, 2), i64::MAX, "the subtract saturates");
        assert_eq!(macaulay_pow(1_000_000, 0, 10), i64::MAX, "the power saturates");
    }

    // ‖M‖∞ >= PMY is refused; just under is accepted.
    #[test]
    fn a_divergent_coupling_is_refused() {
        // Row sums to exactly PMY -> refused.
        assert!(Field5D::new([[6_000i64, 4_000], [0, 0]]).is_none());
        // Row sums to PMY-1 -> accepted.
        assert!(Field5D::new([[6_000i64, 3_999], [0, 0]]).is_some());
        // Negative entries count by absolute value.
        assert!(Field5D::new([[-6_000i64, -4_000], [0, 0]]).is_none());
        assert!(Field5D::new([[-5_000i64, 4_999], [0, 0]]).is_some());
    }

    // The conservative boundary is refused ON PURPOSE, not by accident. A lossless
    // reflection [[0,1],[1,0]] (energy-preserving permutation, ‖M‖∞ = 1 exactly)
    // has no equilibrium — it must be refused, and the strict `<` is what does it.
    // If this test ever "fails" by the field accepting it, the guard was wrongly
    // loosened to `<=` and `resolve` will hang on undamped input. Do not fix that
    // by widening tolerance; conservative dynamics need a symplectic stepper.
    #[test]
    fn the_conservative_boundary_is_refused_on_purpose() {
        // Lossless swap: ‖M‖∞ = PMY exactly.
        assert!(Field5D::new([[0i64, 10_000], [10_000, 0]]).is_none());
        // Undamped oscillator-shaped coupling, rows sum to PMY: also refused.
        assert!(Field5D::new([[0i64, 10_000], [-10_000, 0]]).is_none());
        // One permyriad of damping tips it back into the admissible half.
        assert!(Field5D::new([[0i64, 9_999], [-9_999, 0]]).is_some());
    }

    // M = 0: the field is inert. resolve and deproject are both the identity.
    #[test]
    fn the_zero_field_is_the_identity_both_ways() {
        let f = Field5D::new([[0i64; 4]; 4]).unwrap();
        let v = [7, -3, 100, 0];
        assert_eq!(f.deproject(&v), v);
        assert_eq!(f.resolve(&v, 8).unwrap(), v);
    }

    // deproject is exact and matches the hand calculation of (I - M) f.
    #[test]
    fn deproject_is_the_exact_operator() {
        // M = [[2000, 1000],[500, 3000]] permyriad, f = [10000, 20000].
        let field = Field5D::new([[2_000i64, 1_000], [500, 3_000]]).unwrap();
        let f = [10_000i64, 20_000];
        // g0 = 10000 - (0.2*10000 + 0.1*20000) = 10000 - 4000 = 6000
        // g1 = 20000 - (0.05*10000 + 0.3*20000) = 20000 - 6500 = 13500
        assert_eq!(field.deproject(&f), [6_000, 13_500]);
    }

    // The round trip, the field's L07 bijection: settle a driving input, then
    // un-settle it, and get it back within flooring distance.
    #[test]
    fn deproject_inverts_resolve_within_flooring_distance() {
        let field = Field5D::new([[3_000i64, -1_500, 500], [800, 2_000, 1_200], [-400, 600, 2_500]])
            .unwrap();
        for g in [[10_000i64, 0, 0], [1, 2, 3], [50_000, -20_000, 30_000], [-1, -1, -1]] {
            let f = field.resolve(&g, 200).expect("converges");
            let g_back = field.deproject(&f);
            for k in 0..3 {
                assert!((g_back[k] - g[k]).abs() <= 3, "channel {k}: {} vs {}", g_back[k], g[k]);
            }
        }
    }

    // And the other order: deproject a settled state, resolve it back.
    #[test]
    fn resolve_inverts_deproject_within_flooring_distance() {
        let field = Field5D::new([[2_500i64, 1_000], [-900, 2_000]]).unwrap();
        for f in [[10_000i64, 5_000], [0, 100_000], [-7_000, 3_000]] {
            let g = field.deproject(&f);
            let f_back = field.resolve(&g, 200).expect("converges");
            for k in 0..2 {
                assert!((f_back[k] - f[k]).abs() <= 3, "channel {k}: {} vs {}", f_back[k], f[k]);
            }
        }
    }

    // A diagonal field is N independent leaky integrators — the resolved value of
    // a constant drive g on channel i is decay's scalar equilibrium g·PMY/leak.
    #[test]
    fn diagonal_field_is_decay() {
        // M[i][i] = keep = PMY - leak. resolve(g) settles at g·PMY/leak.
        let leaks = [100u16, 250, 500, 2_000];
        let mut m = [[0i64; 4]; 4];
        for i in 0..4 {
            m[i][i] = (PMY - leaks[i] as u64) as i64; // keep
        }
        let field = Field5D::new(m).unwrap();
        let g = [500i64, 500, 500, 500];
        let f = field.resolve(&g, 5_000).unwrap();
        for i in 0..4 {
            let want = LeakyPermyriad::equilibrium(500, leaks[i]);
            // fixed point lands at or within flooring distance below the scalar resolvent
            assert!(f[i] as u64 <= want, "ch {i}: {} > {want}", f[i]);
            assert!(want - f[i] as u64 <= PMY / leaks[i] as u64 + 1, "ch {i} too far below");
        }
    }

    // A non-settling budget returns None, never a half-converged guess. (Constructed
    // by asking for far too few iterations on a slow-settling near-critical field.)
    #[test]
    fn an_unsettled_field_refuses_rather_than_guesses() {
        let field = Field5D::new([[9_990i64]]).unwrap(); // leak of 10 pmy: very slow
        assert!(field.resolve(&[10_000], 3).is_none(), "3 iters cannot settle a 0.1% leak");
        assert!(field.resolve(&[10_000], 100_000).is_some(), "given room, it settles");
    }

    // Determinism: identical inputs, identical output, every run.
    #[test]
    fn the_field_is_deterministic() {
        let field = Field5D::new([[3_000i64, 900, -400], [1_100, 2_200, 700], [200, -600, 2_800]])
            .unwrap();
        let g = [12_345i64, -6_789, 4_242];
        assert_eq!(field.resolve(&g, 500), field.resolve(&g, 500));
        assert_eq!(field.deproject(&g), field.deproject(&g));
    }
}
