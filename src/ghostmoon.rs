//! Ghostmoon: the 5D closed-interval box `[X, Y, Z, T, S]` — space in
//! `MilliUnit`, time in `SimTick`, phase in `u32`. Ported from
//! `pp-math/src/ghostmoon.rs` (Wave 1), serde stripped at the firewall.
//!
//! A collision/logic volume that exists ONLY inside its tick window `[t0,t1]`
//! and state window `[s0,s1]`: hitboxes cannot leak across time or phase by
//! construction — the lane that would leak simply fails `intersects`.
//!
//! Five lanes, and `atom.rs` has a 5D trit lattice — that is a coincidence of
//! arity, not of meaning. `TritCell5D` is a radix-3 packed lattice *address*;
//! a Ghostmoon lane is a closed integer *interval*. They share no encoding,
//! no unit, and no type, and must never be aliased into one another.

use crate::fixed_point::{MilliUnit, SimTick};

/// The 5-lane box. `repr(C)` pins the lane order — the offset locks at the
/// bottom are the layout contract, so a field reorder fails `cargo check`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ghostmoon {
    /// X axis lower bound.
    pub x0: MilliUnit,
    /// X axis upper bound.
    pub x1: MilliUnit,
    /// Y axis lower bound.
    pub y0: MilliUnit,
    /// Y axis upper bound.
    pub y1: MilliUnit,
    /// Z axis lower bound.
    pub z0: MilliUnit,
    /// Z axis upper bound.
    pub z1: MilliUnit,
    /// Time window lower bound.
    pub t0: SimTick,
    /// Time window upper bound.
    pub t1: SimTick,
    /// Phase window lower bound.
    pub s0: u32,
    /// Phase window upper bound.
    pub s1: u32,
}

impl Ghostmoon {
    /// Build from unordered per-lane spans — each pair normalises to `(min, max)`,
    /// so a caller cannot construct an inverted lane by mistake.
    pub fn span(
        x: (MilliUnit, MilliUnit),
        y: (MilliUnit, MilliUnit),
        z: (MilliUnit, MilliUnit),
        t: (SimTick, SimTick),
        s: (u32, u32),
    ) -> Self {
        Self {
            x0: x.0.min(x.1),
            x1: x.0.max(x.1),
            y0: y.0.min(y.1),
            y1: y.0.max(y.1),
            z0: z.0.min(z.1),
            z1: z.0.max(z.1),
            t0: t.0.min(t.1),
            t1: t.0.max(t.1),
            s0: s.0.min(s.1),
            s1: s.0.max(s.1),
        }
    }

    /// Zero-extent box: one exact 5D state point.
    pub fn point(x: MilliUnit, y: MilliUnit, z: MilliUnit, t: SimTick, s: u32) -> Self {
        Self::span((x, x), (y, y), (z, z), (t, t), (s, s))
    }

    /// `min <= max` on every lane — always true for `span`/`point` construction,
    /// so a `false` here means the struct was built by hand and built wrong.
    pub const fn is_normalized(&self) -> bool {
        self.x0.0 <= self.x1.0
            && self.y0.0 <= self.y1.0
            && self.z0.0 <= self.z1.0
            && self.t0.0 <= self.t1.0
            && self.s0 <= self.s1
    }

    /// Inclusive membership on all 5 lanes.
    pub const fn contains(&self, x: MilliUnit, y: MilliUnit, z: MilliUnit, t: SimTick, s: u32) -> bool {
        self.x0.0 <= x.0
            && x.0 <= self.x1.0
            && self.y0.0 <= y.0
            && y.0 <= self.y1.0
            && self.z0.0 <= z.0
            && z.0 <= self.z1.0
            && self.t0.0 <= t.0
            && t.0 <= self.t1.0
            && self.s0 <= s
            && s <= self.s1
    }

    /// Closed-interval overlap on ALL 5 lanes — one disjoint lane kills the contact.
    pub const fn intersects(&self, o: &Self) -> bool {
        self.x0.0 <= o.x1.0
            && o.x0.0 <= self.x1.0
            && self.y0.0 <= o.y1.0
            && o.y0.0 <= self.y1.0
            && self.z0.0 <= o.z1.0
            && o.z0.0 <= self.z1.0
            && self.t0.0 <= o.t1.0
            && o.t0.0 <= self.t1.0
            && self.s0 <= o.s1
            && o.s0 <= self.s1
    }

    /// Smallest box covering both.
    pub fn union(&self, o: &Self) -> Self {
        Self {
            x0: self.x0.min(o.x0),
            x1: self.x1.max(o.x1),
            y0: self.y0.min(o.y0),
            y1: self.y1.max(o.y1),
            z0: self.z0.min(o.z0),
            z1: self.z1.max(o.z1),
            t0: self.t0.min(o.t0),
            t1: self.t1.max(o.t1),
            s0: self.s0.min(o.s0),
            s1: self.s1.max(o.s1),
        }
    }

    /// Tick-window length (saturating; 0 for a point).
    pub const fn tick_span(&self) -> u64 {
        self.t1.since(self.t0)
    }
}

// ---------------------------------------------------------------------------
// LAYOUT LOCKS. Measured with rustc, never hand-computed. Ten lanes, no
// padding hole: 6×8 + 2×8 + 2×4 fills all 72 bytes.
// ---------------------------------------------------------------------------
const _: () = assert!(core::mem::size_of::<Ghostmoon>() == 72);
const _: () = assert!(core::mem::align_of::<Ghostmoon>() == 8);

// OFFSET LOCKS. Size alone is weak here: swapping any two same-typed bounds
// keeps 72 green while silently mirroring an interval.
const _: () = assert!(core::mem::offset_of!(Ghostmoon, x0) == 0);
const _: () = assert!(core::mem::offset_of!(Ghostmoon, x1) == 8);
const _: () = assert!(core::mem::offset_of!(Ghostmoon, y0) == 16);
const _: () = assert!(core::mem::offset_of!(Ghostmoon, y1) == 24);
const _: () = assert!(core::mem::offset_of!(Ghostmoon, z0) == 32);
const _: () = assert!(core::mem::offset_of!(Ghostmoon, z1) == 40);
const _: () = assert!(core::mem::offset_of!(Ghostmoon, t0) == 48);
const _: () = assert!(core::mem::offset_of!(Ghostmoon, t1) == 56);
const _: () = assert!(core::mem::offset_of!(Ghostmoon, s0) == 64);
const _: () = assert!(core::mem::offset_of!(Ghostmoon, s1) == 68);

#[cfg(test)]
mod tests {
    use super::*;

    fn mu(v: i64) -> MilliUnit {
        MilliUnit(v)
    }

    fn base() -> Ghostmoon {
        Ghostmoon::span(
            (mu(0), mu(1_000)),
            (mu(0), mu(2_000)),
            (mu(0), mu(500)),
            (SimTick(100), SimTick(200)),
            (1, 2),
        )
    }

    #[test]
    fn span_normalizes_reversed_lanes() {
        let b = Ghostmoon::span(
            (mu(1_000), mu(0)),
            (mu(2_000), mu(0)),
            (mu(500), mu(0)),
            (SimTick(200), SimTick(100)),
            (2, 1),
        );
        assert!(b.is_normalized());
        assert_eq!(b, base());
    }

    #[test]
    fn contains_is_inclusive_on_all_bounds() {
        let b = base();
        assert!(b.contains(mu(0), mu(0), mu(0), SimTick(100), 1));
        assert!(b.contains(mu(1_000), mu(2_000), mu(500), SimTick(200), 2));
        assert!(!b.contains(mu(1_001), mu(0), mu(0), SimTick(100), 1));
    }

    /// Rank receipt: EVERY lane is live — disjointness on any single axis
    /// (X, Y, Z, T, S) kills the intersection while the other four still overlap.
    #[test]
    fn each_of_the_five_lanes_can_kill_contact() {
        let b = base();
        let hit = Ghostmoon::span(
            (mu(500), mu(1_500)),
            (mu(500), mu(2_500)),
            (mu(100), mu(900)),
            (SimTick(150), SimTick(250)),
            (2, 3),
        );
        assert!(b.intersects(&hit));
        let mut x = hit;
        x.x0 = mu(2_000);
        x.x1 = mu(3_000);
        assert!(!b.intersects(&x), "X-disjoint");
        let mut y = hit;
        y.y0 = mu(3_000);
        y.y1 = mu(4_000);
        assert!(!b.intersects(&y), "Y-disjoint");
        let mut z = hit;
        z.z0 = mu(600);
        z.z1 = mu(700);
        assert!(!b.intersects(&z), "Z-disjoint");
        let mut t = hit;
        t.t0 = SimTick(300);
        t.t1 = SimTick(400);
        assert!(!b.intersects(&t), "T-disjoint");
        let mut s = hit;
        s.s0 = 5;
        s.s1 = 9;
        assert!(!b.intersects(&s), "S-disjoint");
    }

    #[test]
    fn union_covers_both_and_point_is_zero_extent() {
        let b = base();
        let p = Ghostmoon::point(mu(5_000), mu(-100), mu(9_000), SimTick(999), 7);
        assert_eq!(p.tick_span(), 0);
        let u = b.union(&p);
        assert!(u.contains(mu(5_000), mu(-100), mu(9_000), SimTick(999), 7));
        assert!(u.contains(mu(0), mu(0), mu(0), SimTick(100), 1));
        assert_eq!(base().tick_span(), 100);
    }
}
