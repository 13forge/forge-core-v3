//! Soliton-Phase Context Collapse — the interference kernel handed to [`Field5D`].
//!
//! SPCC is NOT a new engine. It is one kernel definition for the second-kind
//! resolvent (`(I − λK) f = g`, `resolvent.rs`): two context rows interact iff
//! they are spatially collinear on X/Y/Z and phase-related on θ. Phase-coincident
//! rows couple positively (constructive — a restated fact reinforces), phase-
//! inverted rows couple negatively (destructive — a contradicted fact is damped).
//! The settled field is the compacted context; rows driven below the floor are
//! evicted, and every eviction is visible in the receipt. Forgetting is audited,
//! never silent.
//!
//! AXIS PIN (ARCH000 2026-08-10): θ is the harmonic rung = scale rung, so the
//! phase lane is [`AXIS_THETA`](crate::ramus_prime::AXIS_THETA) `== AXIS_S`,
//! never a bare `4`. Amplitude is NOT
//! a lane — weight lives beside the lattice in permyriad, exactly where the
//! resolvent's `f`/`g` vectors live. The old `[x, y, z, θ, w]` reading of weight
//! as a coordinate died with V2.
//!
//! WHY ANNIHILATION IS A FLOOR, NOT A ZERO: exact total cancellation of equal
//! opposed packets is the conservative boundary `‖M‖∞ = 1` — the poltergeist the
//! resolvent refuses by design (`resolvent.rs`, "the strict `<` is load-bearing").
//! Inside the admissible damped regime, destructive interference SUPPRESSES a
//! row's settled weight below its drive; [`WEIGHT_FLOOR_PMY`] decides eviction.
//! A resonance cascade that would need `‖M‖∞ ≥ 1` is unrepresentable: the
//! coupling ceiling makes every lawful holon admissible by construction, and
//! `Field5D::new` stands behind that as defense in depth.
//!
//! THE DEGENERATE-PHASE EDGE (the "harmonic symmetry" failure of the paper
//! spec): a θ lane of all balanced zeros is its own inversion. Constructive is
//! therefore tested FIRST — a zero-phase pair merges, it never annihilates. The
//! origin cannot be its own poltergeist.
//!
//! ORPHANS (the `split_ident → tokens=[]` analog): a point carrying any sentinel
//! slice has no decodable phase. It bypasses the collision field entirely —
//! zero coupling both ways, settled weight = drive, flagged in the receipt.
//! Orphans are reported, never aborted on: `breach` is for corruption, and an
//! orphan is not corrupt, it is merely unaddressable.
//!
//! THE LANDAUER LEDGER (2026-08-12 harvest, fourth-landing wave): erasure is
//! priced, not merely audited. [`CollapseReceipt::erased_drive_pmy`] is the
//! drive mass the caller committed that the field then destroyed (evicted), and
//! [`CollapseReceipt::landauer_margin_pmy`] is `mass_in − 2·erased`. The tested
//! law: THE FIELD CANNOT FORGET MORE THAN IT KEEPS — the margin is never
//! negative for a lawful holon. Why (inferred, held by the tests below, not a
//! proof): flooring a victim needs attacker drive of the same order, because
//! per-pair coupling is capped at [`COUPLING_CEILING_PMY`] and amplification at
//! `1/(1 − 7/8)`; and victims of a common attacker sit within `2·PROXIMITY` of
//! each other (tritwise Hamming obeys the triangle inequality), so co-victims
//! are forced into mutual constructive support. The adversarial optimum — one
//! full-weight attacker flooring seven ceiling-weight victims — erases exactly
//! 7/15 of committed mass; the margin left is one coupling ceiling. Physical
//! Landauer (kT·ln2 joules) is admitted as ordering only, never as heat: the
//! claim holds as integer information accounting and fails if read as
//! thermodynamics.
//!
//! Prior art, same law, fourth landing: `lateral-seam-theory.md:12` (routing as
//! Fredholm 2nd kind), GHOST-CONSTELLATION hauntings (L03, 2026-08-10 harvest),
//! `_quarry/STUDIONOTE-FREDHOLM-SALIENCY-2026-08-12.md` (allostatic OODA as 2nd kind;
//! independent haiku derivation, verdicts + absences ledgered there).

use crate::atom::{Pexil, PexilLine, TritCell5D};
use crate::decay::PMY;
use crate::grid::{PackedPoint105, DEPTH};
use crate::mersenne::M3;
use crate::ramus_prime::{AXIS_THETA, AXIS_X, AXIS_Y, AXIS_Z};
use crate::resolvent::Field5D;
use crate::sentinel::MAX_PACKED;

/// Rows per collapse holon: one cache line of context. The dense resolvent is
/// the small-holon solver (`resolvent.rs` boundary #2); eight is its home size.
pub const HOLON: usize = 8;
const _: () = assert!(HOLON == core::mem::size_of::<PexilLine>() / core::mem::size_of::<Pexil>());

/// Max tritwise Hamming distance on the θ lane for two rows to count as
/// phase-coincident (or, against the inverted lane, phase-opposed). Derivation:
/// one M3-th of the lane, `DEPTH / M3 = 21 / 7 = 3`.
pub const PHASE_EPS_TRITS: usize = DEPTH / M3 as usize;
const _: () = assert!(PHASE_EPS_TRITS * M3 as usize == DEPTH);

/// Max summed X/Y/Z tritwise distance for two rows to be collinear enough to
/// interact. Derivation: one M3-th of the three spatial lanes, `3·DEPTH / M3`.
pub const PROXIMITY_MAX_TRITS: usize = 3 * DEPTH / M3 as usize;
const _: () = assert!(PROXIMITY_MAX_TRITS * M3 as usize == 3 * DEPTH);

/// Coupling ceiling per pair, permyriad. Derivation: `PMY / HOLON`, so a row's
/// worst case `(HOLON − 1)` full-strength partners sum to `‖M‖∞ = 8_750 < PMY`
/// and every lawful holon is admissible BY CONSTRUCTION — the cascade regime
/// is unreachable, not merely checked.
pub const COUPLING_CEILING_PMY: u64 = PMY / HOLON as u64;
const _: () = assert!(COUPLING_CEILING_PMY * HOLON as u64 == PMY);
const _: () = assert!((HOLON as u64 - 1) * COUPLING_CEILING_PMY < PMY);

/// Eviction floor: one centiunit of mass, `PMY / 100`. A settled weight at or
/// below this is context the field has cancelled; the receipt ledgers it.
pub const WEIGHT_FLOOR_PMY: i64 = (PMY / 100) as i64;
const _: () = assert!(WEIGHT_FLOOR_PMY * 100 == PMY as i64);

/// Phase inversion of one packed cell: negate all five trits. In radix-3
/// packing that is the complement `(MAX_PACKED − 1) − b`, a pure integer
/// involution. `None` for a sentinel — control states have no phase.
#[inline(always)]
pub const fn phase_inverse(c: TritCell5D) -> Option<TritCell5D> {
    if c.is_sentinel() {
        return None;
    }
    Some(TritCell5D(MAX_PACKED - 1 - c.0))
}

/// One context row: an address in the 5D substrate plus its permyriad mass.
/// The mass rides beside the lattice, never inside it (AXIS PIN).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextRow {
    /// Where the row lives — the mailbox, not the letter.
    pub point: PackedPoint105,
    /// Driving weight `g`, permyriad. `PMY` is one unit of context mass.
    pub weight: i64,
}

/// How two rows relate under the kernel. Strengths are permyriad couplings,
/// already scaled by spatial proximity, bounded by [`COUPLING_CEILING_PMY`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    /// Different districts, too far apart, or unaddressable: zero coupling.
    None,
    /// Phase-coincident restatement: positive coupling, the rows reinforce.
    Constructive(i64),
    /// Phase-inverted contradiction: negative coupling, the rows damp.
    Destructive(i64),
}

/// The audit of one collapse. Nothing leaves the holon without a line here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollapseReceipt {
    /// Settled weights `f = (I − M)⁻¹ g`, one per row, evicted rows included.
    pub settled: [i64; HOLON],
    /// True where the settled weight fell to [`WEIGHT_FLOOR_PMY`] or below.
    pub evicted: [bool; HOLON],
    /// True where the row carried a sentinel slice and bypassed the field.
    pub orphan: [bool; HOLON],
    /// Total driving mass in.
    pub mass_in: i64,
    /// Total settled mass of surviving rows.
    pub mass_out: i64,
    /// Total settled mass of evicted rows — the ledgered loss.
    pub mass_evicted: i64,
    /// Drive mass committed to rows the field then evicted — the information
    /// DESTROYED, priced at its input weight (the Landauer ledger). Distinct
    /// from `mass_evicted`, which is the settled residue at eviction time.
    pub erased_drive_pmy: i64,
}

impl CollapseReceipt {
    /// Net interference: positive when constructive coupling dominated,
    /// negative when destructive did. Zero for an inert field.
    pub const fn interference_gain(&self) -> i64 {
        self.mass_out + self.mass_evicted - self.mass_in
    }

    /// The Landauer margin `mass_in − 2·erased_drive_pmy`: how far this
    /// collapse sat from forgetting more than it kept. Never negative for a
    /// lawful holon (module doc, "THE LANDAUER LEDGER"); a caller that sees a
    /// shrinking margin across beats is watching the field approach amnesia
    /// and should treat it as dauer pressure.
    pub const fn landauer_margin_pmy(&self) -> i64 {
        self.mass_in - 2 * self.erased_drive_pmy
    }
}

/// True when the point carries any sentinel slice: no decodable phase, no
/// participation in the field.
pub fn is_orphan(p: &PackedPoint105) -> bool {
    let mut d = 0;
    while d < DEPTH {
        if p.slices[d].is_sentinel() {
            return true;
        }
        d += 1;
    }
    false
}

/// One axis of a point as 21 balanced trits, or `None` if any slice is sentinel.
fn axis_trits(p: &PackedPoint105, axis: usize) -> Option<[i8; DEPTH]> {
    let mut out = [0i8; DEPTH];
    for d in 0..DEPTH {
        out[d] = p.slices[d].trits()?[axis];
    }
    Some(out)
}

/// Tritwise Hamming distance between two decoded lanes.
fn lane_distance(a: &[i8; DEPTH], b: &[i8; DEPTH]) -> usize {
    let mut n = 0;
    for d in 0..DEPTH {
        if a[d] != b[d] {
            n += 1;
        }
    }
    n
}

/// Distance from `a` to the PHASE INVERSE of `b`: trit-negate `b` on the fly.
fn lane_distance_inverted(a: &[i8; DEPTH], b: &[i8; DEPTH]) -> usize {
    let mut n = 0;
    for d in 0..DEPTH {
        if a[d] != -b[d] {
            n += 1;
        }
    }
    n
}

/// Proximity-scaled coupling strength: full ceiling at zero spatial distance,
/// decaying linearly, never zero inside the collinearity radius.
const fn coupling_pmy(d_xyz: usize) -> i64 {
    (COUPLING_CEILING_PMY as i64 * (PROXIMITY_MAX_TRITS + 1 - d_xyz) as i64)
        / (PROXIMITY_MAX_TRITS + 1) as i64
}

/// The SPCC kernel: classify one pair of addresses. Constructive is tested
/// before destructive so the degenerate zero-phase lane merges rather than
/// annihilates (see module doc).
pub fn interaction(a: &PackedPoint105, b: &PackedPoint105) -> Interaction {
    if is_orphan(a) || is_orphan(b) {
        return Interaction::None;
    }
    // Unwraps cannot fire past the orphan gate; keep them as Options anyway so
    // a future sentinel path refuses rather than panics.
    let (Some(ax), Some(ay), Some(az), Some(at)) = (
        axis_trits(a, AXIS_X),
        axis_trits(a, AXIS_Y),
        axis_trits(a, AXIS_Z),
        axis_trits(a, AXIS_THETA),
    ) else {
        return Interaction::None;
    };
    let (Some(bx), Some(by), Some(bz), Some(bt)) = (
        axis_trits(b, AXIS_X),
        axis_trits(b, AXIS_Y),
        axis_trits(b, AXIS_Z),
        axis_trits(b, AXIS_THETA),
    ) else {
        return Interaction::None;
    };

    let d_xyz = lane_distance(&ax, &bx) + lane_distance(&ay, &by) + lane_distance(&az, &bz);
    if d_xyz > PROXIMITY_MAX_TRITS {
        return Interaction::None;
    }

    if lane_distance(&at, &bt) <= PHASE_EPS_TRITS {
        return Interaction::Constructive(coupling_pmy(d_xyz));
    }
    if lane_distance_inverted(&at, &bt) <= PHASE_EPS_TRITS {
        return Interaction::Destructive(coupling_pmy(d_xyz));
    }
    Interaction::None
}

/// Collapse one holon: build the coupling, hand it to the resolvent, audit the
/// settlement. Pure — the receipt reports, the CALLER mutates its store
/// (tombstoning an evicted row is `Sentinel::Tombstone`'s job, not ours).
///
/// `None` only when the field refuses or fails to settle in `max_iters` — a
/// non-settling collapse is a defect surfaced, never a best-effort guess.
pub fn collapse(rows: &[ContextRow; HOLON], max_iters: u32) -> Option<CollapseReceipt> {
    let mut orphan = [false; HOLON];
    for (i, row) in rows.iter().enumerate() {
        orphan[i] = is_orphan(&row.point);
    }

    let mut m = [[0i64; HOLON]; HOLON];
    for i in 0..HOLON {
        for j in 0..HOLON {
            if i == j || orphan[i] || orphan[j] {
                continue;
            }
            m[i][j] = match interaction(&rows[i].point, &rows[j].point) {
                Interaction::None => 0,
                Interaction::Constructive(s) => s,
                Interaction::Destructive(s) => -s,
            };
        }
    }

    let field = Field5D::new(m)?;
    let mut g = [0i64; HOLON];
    for i in 0..HOLON {
        g[i] = rows[i].weight;
    }
    let settled = field.resolve(&g, max_iters)?;

    let mut evicted = [false; HOLON];
    let (mut mass_in, mut mass_out, mut mass_evicted) = (0i64, 0i64, 0i64);
    let mut erased_drive_pmy = 0i64;
    for i in 0..HOLON {
        mass_in += g[i];
        // An orphan is never evicted: it never entered the field.
        evicted[i] = !orphan[i] && settled[i] <= WEIGHT_FLOOR_PMY;
        if evicted[i] {
            mass_evicted += settled[i];
            erased_drive_pmy += g[i];
        } else {
            mass_out += settled[i];
        }
    }
    Some(CollapseReceipt {
        settled,
        evicted,
        orphan,
        mass_in,
        mass_out,
        mass_evicted,
        erased_drive_pmy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A point whose X/Y/Z lanes are all balanced zero and whose θ lane holds
    /// `theta` at every depth. S stays zero throughout — scale is not phase.
    fn point_with_theta(theta: i8) -> PackedPoint105 {
        let mut t = [0i8; 5];
        t[AXIS_THETA] = theta;
        PackedPoint105 { slices: [TritCell5D::from_trits(t); DEPTH] }
    }

    fn row(point: PackedPoint105, weight: i64) -> ContextRow {
        ContextRow { point, weight }
    }

    fn inert_rows() -> [ContextRow; HOLON] {
        // Spread far apart on X so nothing interacts: each row's X lane holds a
        // distinct constant trit pattern beyond the proximity radius.
        core::array::from_fn(|i| {
            let mut slices = [TritCell5D::ORIGIN; DEPTH];
            for d in 0..DEPTH {
                let mut t = [0i8; 5];
                t[AXIS_X] = ((i as i8) % 3) - 1;
                t[AXIS_Y] = ((i as i8 / 3) % 3) - 1;
                // Differ on MOST depths per row index so pairwise d_xyz > radius.
                if (d + i) % 2 == 0 {
                    t[AXIS_Z] = 1;
                }
                slices[d] = TritCell5D::from_trits(t);
            }
            row(PackedPoint105 { slices }, PMY as i64)
        })
    }

    #[test]
    fn phase_inversion_is_the_complement_and_an_involution_over_all_243_cells() {
        for b in 0..MAX_PACKED {
            let c = TritCell5D(b);
            let inv = phase_inverse(c).expect("interior cell has a phase");
            assert_eq!(inv.0, 242 - b);
            assert_eq!(phase_inverse(inv).unwrap(), c, "involution failed at {b}");
            // The complement really is tritwise negation.
            let t = c.trits().unwrap();
            let ti = inv.trits().unwrap();
            for k in 0..5 {
                assert_eq!(ti[k], -t[k]);
            }
        }
        for b in MAX_PACKED..=254 {
            assert!(phase_inverse(TritCell5D(b)).is_none(), "sentinel {b} must have no phase");
        }
        assert!(phase_inverse(TritCell5D(255)).is_none());
    }

    #[test]
    fn a_phase_inverted_twin_is_destructive_and_a_coincident_one_constructive() {
        let up = point_with_theta(1);
        let down = point_with_theta(-1);
        assert!(matches!(interaction(&up, &down), Interaction::Destructive(s) if s > 0));
        assert!(matches!(interaction(&up, &up), Interaction::Constructive(s) if s > 0));
    }

    #[test]
    fn the_degenerate_zero_phase_lane_merges_rather_than_annihilates() {
        // θ all zeros is its own inversion; constructive-first decides merge.
        let zero = point_with_theta(0);
        assert!(matches!(interaction(&zero, &zero), Interaction::Constructive(_)));
    }

    #[test]
    fn rows_beyond_the_collinearity_radius_do_not_interact() {
        let near = point_with_theta(1);
        let mut far = near;
        // Push X off by every depth: 21 trits of X distance > PROXIMITY_MAX_TRITS.
        for d in 0..DEPTH {
            let mut t = far.slices[d].trits().unwrap();
            t[AXIS_X] = 1;
            far.slices[d] = TritCell5D::from_trits(t);
        }
        assert_eq!(interaction(&near, &far), Interaction::None);
    }

    #[test]
    fn a_destructive_pair_settles_below_its_drive_and_a_constructive_pair_above() {
        let mut rows = inert_rows();
        rows[0] = row(point_with_theta(1), PMY as i64);
        rows[1] = row(point_with_theta(-1), PMY as i64);
        let r = collapse(&rows, 100_000).expect("admissible by construction");
        assert!(r.settled[0] < PMY as i64, "opposed row must be suppressed");
        assert!(r.settled[1] < PMY as i64);
        assert!(!r.evicted[0] && !r.evicted[1], "a lone twin suppresses, only a chorus evicts");
        assert!(r.interference_gain() < 0, "destruction reads as negative gain");

        let mut rows = inert_rows();
        rows[0] = row(point_with_theta(1), PMY as i64);
        rows[1] = row(point_with_theta(1), PMY as i64);
        let r = collapse(&rows, 100_000).expect("admissible");
        assert!(r.settled[0] > PMY as i64, "restatement must reinforce");
        assert!(r.interference_gain() > 0);
    }

    #[test]
    fn a_chorus_of_inverted_twins_drives_the_lone_voice_below_the_floor() {
        // Row 0 opposed by seven mutually-coincident twins: the cascade the old
        // paper spec feared, held lawful by the coupling ceiling, resolved to an
        // eviction with a receipt instead of a poltergeist.
        let mut rows = [row(point_with_theta(-1), PMY as i64); HOLON];
        rows[0] = row(point_with_theta(1), PMY as i64);
        let r = collapse(&rows, 1_000_000).expect("admissible by construction");
        assert!(r.evicted[0], "the contradicted voice falls below the floor: {}", r.settled[0]);
        assert!(r.settled[0] <= WEIGHT_FLOOR_PMY);
        for i in 1..HOLON {
            assert!(!r.evicted[i], "the chorus survives");
            assert!(r.settled[i] > PMY as i64, "and is amplified");
        }
        assert_eq!(r.mass_evicted, r.settled[0], "the loss is ledgered, not silent");
        assert_eq!(r.erased_drive_pmy, PMY as i64, "the destroyed drive is priced at input weight");
        assert!(r.landauer_margin_pmy() > 0, "one voice lost to seven kept is far from amnesia");
    }

    #[test]
    fn the_adversarial_amnesia_optimum_still_keeps_more_than_it_forgets() {
        // The Landauer ledger's worst case: ONE full-weight attacker flooring
        // SEVEN victims, each victim weighted exactly at the coupling ceiling —
        // the largest drive the attacker can null per pair. Co-located victims
        // are mutually coincident (triangle inequality leaves them no escape),
        // but at the floor their support is worthless: all seven evict, and the
        // erased fraction hits its supremum 7/15 of committed mass.
        let mut rows = [row(point_with_theta(-1), COUPLING_CEILING_PMY as i64); HOLON];
        rows[0] = row(point_with_theta(1), PMY as i64);
        let r = collapse(&rows, 1_000_000).expect("admissible by construction");
        for i in 1..HOLON {
            assert!(r.evicted[i], "victim {i} must fall: settled {}", r.settled[i]);
        }
        assert_eq!(r.erased_drive_pmy, 7 * COUPLING_CEILING_PMY as i64);
        // mass_in = PMY + 7·ceiling = 15·ceiling; erased = 7·ceiling;
        // margin = 15·ceiling − 14·ceiling = exactly one coupling ceiling.
        assert_eq!(r.landauer_margin_pmy(), COUPLING_CEILING_PMY as i64);
    }

    #[test]
    fn an_orphan_bypasses_the_field_with_its_weight_intact() {
        let mut rows = inert_rows();
        let mut p = point_with_theta(1);
        p.slices[0] = TritCell5D(245); // Tombstone sentinel: unaddressable phase
        rows[3] = row(p, 7_777);
        let r = collapse(&rows, 10_000).expect("orphans never poison the field");
        assert!(r.orphan[3]);
        assert!(!r.evicted[3], "an orphan is reported, never evicted");
        assert_eq!(r.settled[3], 7_777, "identity pass-through");
    }

    #[test]
    fn the_inert_holon_is_the_identity_and_gains_nothing() {
        let rows = inert_rows();
        let r = collapse(&rows, 10).expect("zero field settles immediately");
        for i in 0..HOLON {
            assert_eq!(r.settled[i], rows[i].weight);
            assert!(!r.evicted[i]);
        }
        assert_eq!(r.interference_gain(), 0);
        assert_eq!(r.erased_drive_pmy, 0, "nothing forgotten, nothing priced");
        assert_eq!(r.landauer_margin_pmy(), r.mass_in, "full margin when nothing is erased");
    }

    #[test]
    fn an_unsettled_collapse_refuses_rather_than_guesses() {
        let mut rows = inert_rows();
        rows[0] = row(point_with_theta(1), PMY as i64);
        rows[1] = row(point_with_theta(1), PMY as i64);
        assert!(collapse(&rows, 0).is_none(), "zero budget cannot settle a live coupling");
    }

    #[test]
    fn every_lawful_holon_is_admissible_by_construction() {
        // Deterministic LCG points: whatever geometry falls out, the coupling
        // ceiling keeps ‖M‖∞ < PMY, so Field5D::new never refuses a lawful holon.
        let mut state = 0x13u64;
        let mut lcg = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 33) % MAX_PACKED as u64) as u8
        };
        for _ in 0..32 {
            let rows: [ContextRow; HOLON] = core::array::from_fn(|_| {
                let mut slices = [TritCell5D::ORIGIN; DEPTH];
                for s in slices.iter_mut() {
                    *s = TritCell5D(lcg());
                }
                row(PackedPoint105 { slices }, PMY as i64)
            });
            let r = collapse(&rows, 1_000_000)
                .expect("a lawful holon must never hit the cascade boundary");
            // Second oracle for the Landauer law: it never emerges broken in
            // simulation, whatever geometry the LCG deals.
            assert!(
                r.landauer_margin_pmy() >= 0,
                "the field forgot more than it kept: margin {}",
                r.landauer_margin_pmy()
            );
        }
    }

    #[test]
    fn the_collapse_is_deterministic() {
        let mut rows = inert_rows();
        rows[0] = row(point_with_theta(1), 12_345);
        rows[1] = row(point_with_theta(-1), 6_789);
        assert_eq!(collapse(&rows, 100_000), collapse(&rows, 100_000));
    }
}
