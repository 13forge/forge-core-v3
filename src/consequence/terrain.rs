//! The terrain family's Consequence handler — the missing CONSUMER of
//! `vixel_automata`'s rules. A terrain consequence sweeps a bounded
//! neighbourhood through the landed rule fns instead of stepping one cell.

use crate::consequence::query::{Consequence, ConsequenceKind};
use crate::consequence::tags::TGT_FAMILY_TERRAIN;
use crate::vixel_automata::{
    has_flag, rule_fluid_flow, rule_gravity, rule_ignite, VixelAtom, FLAG_ALIVE,
};

/// Hard ceiling on sweep radius. "Bounded" is the row's own word and this is
/// what makes it true: a consequence cannot walk the whole grid however loud
/// it is. A radius of 8 visits at most a 17x17 block.
pub const MAX_SWEEP_RADIUS: i32 = 8;

/// Permyriad of catalytic energy that buys one cell of radius.
const PMY_PER_RADIUS: u16 = 1_250;

/// What a sweep did. Counts, not cells — a caller that wants the cells reads
/// the grid it just handed over.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SweepReport {
    /// Cells the sweep actually looked at.
    pub visited: usize,
    /// Cells that caught fire this sweep.
    pub ignited: usize,
    /// Cells that fell.
    pub fell: usize,
    /// Fluid cells that found somewhere to flow.
    pub flowed: usize,
}

impl SweepReport {
    /// True when the sweep changed nothing.
    pub fn is_quiet(&self) -> bool {
        self.ignited == 0 && self.fell == 0 && self.flowed == 0
    }
}

/// How far a consequence reaches, in cells. Louder consequences reach further,
/// up to [`MAX_SWEEP_RADIUS`] and never past it.
pub fn sweep_radius(c: &Consequence) -> i32 {
    ((c.catalytic_pmy / PMY_PER_RADIUS) as i32).clamp(0, MAX_SWEEP_RADIUS)
}

/// True when this consequence is one the terrain handler answers at all.
///
/// Two gates, both required: the TARGET FAMILY must be terrain, and the kind
/// must be one the automata rules can actually express. A Sound consequence
/// landing on terrain moves no grains.
pub fn handles(target_family: u8, c: &Consequence) -> bool {
    target_family == TGT_FAMILY_TERRAIN
        && matches!(
            c.kind(),
            ConsequenceKind::Ignite
                | ConsequenceKind::Extinguish
                | ConsequenceKind::VoxelBreak
                | ConsequenceKind::Shatter
        )
}

fn index(x: i32, y: i32, width: usize, height: usize) -> Option<usize> {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return None;
    }
    Some(y as usize * width + x as usize)
}

/// Sweep a terrain consequence over the neighbourhood of `(ox, oy)`.
///
/// DOUBLE-BUFFERED on purpose: every rule reads the cell state as it was at
/// the START of the sweep, and writes land in the grid. A single-buffer sweep
/// would make each cell's answer depend on the order its neighbours happened
/// to be visited in, which is a different result for the same input — and a
/// cellular automaton that is not order-independent is not deterministic.
///
/// Returns a zero report and touches nothing when [`handles`] says no.
pub fn sweep_terrain(
    cells: &mut [VixelAtom],
    width: usize,
    target_family: u8,
    consequence: &Consequence,
    ox: i32,
    oy: i32,
) -> SweepReport {
    let mut report = SweepReport::default();
    if width == 0 || cells.is_empty() || !handles(target_family, consequence) {
        return report;
    }
    let height = cells.len() / width;
    if height == 0 {
        return report;
    }

    let before: Vec<VixelAtom> = cells.to_vec();
    let radius = sweep_radius(consequence);

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let Some(idx) = index(ox + dx, oy + dy, width, height) else { continue };
            report.visited += 1;

            // Cardinal neighbours, read from the snapshot.
            let cardinals = [(0, -1), (0, 1), (-1, 0), (1, 0)];
            let neighbours: Vec<VixelAtom> = cardinals
                .iter()
                .filter_map(|(nx, ny)| index(ox + dx + nx, oy + dy + ny, width, height))
                .map(|i| before[i])
                .collect();

            let atom = before[idx];

            let flags = rule_ignite(&atom, &neighbours);
            if flags != atom.flags {
                cells[idx].flags = flags;
                report.ignited += 1;
            }

            let below = index(ox + dx, oy + dy + 1, width, height);
            let below_empty = below.map_or(false, |i| !has_flag(&before[i], FLAG_ALIVE));
            let moved = VixelAtom { flags, ..atom };
            let pos_y = rule_gravity(&moved, below_empty);
            if pos_y != atom.pos_y {
                cells[idx].pos_y = pos_y;
                report.fell += 1;
            }

            let neighbours_y = cardinals.map(|(nx, ny)| {
                index(ox + dx + nx, oy + dy + ny, width, height)
                    .filter(|i| !has_flag(&before[*i], FLAG_ALIVE))
                    .map_or(i32::MAX, |i| before[i].pos_y)
            });
            if rule_fluid_flow(&moved, neighbours_y).is_some() {
                report.flowed += 1;
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consequence::tags::TGT_FAMILY_STRUCTURE;
    use crate::vixel_automata::{FLAG_BURNING, FLAG_FLAMMABLE, FLAG_FLUID};

    const W: usize = 5;

    /// `VixelAtom` is a GPU-parity struct with no `Default`/`PartialEq` and is
    /// not given them here — a repr(C) upload shape is not somewhere to add
    /// derives for a test's convenience.
    fn atom(flags: u32) -> VixelAtom {
        VixelAtom {
            pos_x: 0,
            pos_y: 0,
            pos_z: 0,
            material: 1,
            opacity: 10_000,
            size: 1_000,
            flags,
        }
    }

    fn grid() -> Vec<VixelAtom> {
        vec![atom(FLAG_ALIVE); W * W]
    }

    fn same(a: &[VixelAtom], b: &[VixelAtom]) -> bool {
        a.len() == b.len()
            && a.iter().zip(b).all(|(x, y)| {
                x.flags == y.flags && x.pos_y == y.pos_y && x.pos_x == y.pos_x
            })
    }

    fn ignite(pmy: u16) -> Consequence {
        Consequence { kind: 2, catalytic_pmy: pmy, ..Default::default() }
    }

    #[test]
    fn a_non_terrain_family_sweeps_nothing() {
        let mut cells = grid();
        let r = sweep_terrain(&mut cells, W, TGT_FAMILY_STRUCTURE, &ignite(10_000), 2, 2);
        assert_eq!(r, SweepReport::default(), "structure is not terrain");
        assert_eq!(r.visited, 0);
    }

    /// A consequence the automata cannot express moves no grains, even on
    /// terrain — the family gate alone is not enough.
    #[test]
    fn a_sound_consequence_on_terrain_moves_nothing() {
        let sound = Consequence { kind: 6, catalytic_pmy: 10_000, ..Default::default() };
        assert!(!handles(TGT_FAMILY_TERRAIN, &sound));
        let mut cells = grid();
        assert!(sweep_terrain(&mut cells, W, TGT_FAMILY_TERRAIN, &sound, 2, 2).is_quiet());
    }

    #[test]
    fn the_sweep_is_bounded_however_loud_the_consequence() {
        assert_eq!(sweep_radius(&ignite(0)), 0);
        assert_eq!(sweep_radius(&ignite(PMY_PER_RADIUS)), 1);
        assert_eq!(sweep_radius(&ignite(u16::MAX)), MAX_SWEEP_RADIUS, "clamped, not scaled");

        let mut cells = grid();
        let r = sweep_terrain(&mut cells, W, TGT_FAMILY_TERRAIN, &ignite(u16::MAX), 2, 2);
        assert_eq!(r.visited, W * W, "a 5x5 grid caps at its own size, not the radius");
    }

    /// The point of the row: fire PROPAGATES across the neighbourhood instead
    /// of one cell changing state.
    #[test]
    fn fire_spreads_to_the_flammable_neighbourhood() {
        let mut cells = grid();
        for c in cells.iter_mut() {
            c.flags = FLAG_ALIVE | FLAG_FLAMMABLE;
        }
        cells[12].flags = FLAG_ALIVE | FLAG_BURNING; // centre of 5x5

        let r = sweep_terrain(&mut cells, W, TGT_FAMILY_TERRAIN, &ignite(2_500), 2, 2);
        assert!(r.ignited >= 4, "the four cardinal neighbours must catch: {r:?}");
        for i in [7usize, 11, 13, 17] {
            assert!(has_flag(&cells[i], FLAG_BURNING), "cell {i} should be burning");
        }
    }

    /// Double-buffering, stated as a property: fire spreads exactly ONE ring
    /// per sweep. A single-buffer sweep would let a cell that just caught fire
    /// ignite its own neighbour in the same pass.
    #[test]
    fn fire_advances_one_ring_per_sweep_not_a_whole_grid() {
        let mut cells = grid();
        for c in cells.iter_mut() {
            c.flags = FLAG_ALIVE | FLAG_FLAMMABLE;
        }
        cells[12].flags = FLAG_ALIVE | FLAG_BURNING;

        sweep_terrain(&mut cells, W, TGT_FAMILY_TERRAIN, &ignite(u16::MAX), 2, 2);
        // Corners are two steps away and must NOT have caught in one sweep.
        for corner in [0usize, 4, 20, 24] {
            assert!(
                !has_flag(&cells[corner], FLAG_BURNING),
                "corner {corner} caught fire in a single sweep — the sweep is not double-buffered"
            );
        }
    }

    #[test]
    fn a_sweep_over_nothing_is_quiet() {
        let mut empty: Vec<VixelAtom> = Vec::new();
        assert!(sweep_terrain(&mut empty, 0, TGT_FAMILY_TERRAIN, &ignite(9_000), 0, 0).is_quiet());
        let mut cells = grid();
        // Origin far outside the grid: nothing in range.
        let r = sweep_terrain(&mut cells, W, TGT_FAMILY_TERRAIN, &ignite(1_250), 99, 99);
        assert_eq!(r.visited, 0);
        assert!(r.is_quiet());
    }

    /// Same grid, same consequence, same answer — twice running.
    #[test]
    fn the_sweep_is_deterministic() {
        let build = || {
            let mut c = grid();
            for a in c.iter_mut() {
                a.flags = FLAG_ALIVE | FLAG_FLAMMABLE;
            }
            c[12].flags = FLAG_ALIVE | FLAG_BURNING;
            c[6].flags = FLAG_ALIVE | FLAG_FLUID;
            c
        };
        let (mut a, mut b) = (build(), build());
        let ra = sweep_terrain(&mut a, W, TGT_FAMILY_TERRAIN, &ignite(5_000), 2, 2);
        let rb = sweep_terrain(&mut b, W, TGT_FAMILY_TERRAIN, &ignite(5_000), 2, 2);
        assert_eq!(ra, rb);
        assert!(same(&a, &b), "the same input must leave the same grid");
    }
}
