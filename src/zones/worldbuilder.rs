//! `WorldBuilderEngine` — ties the already-landed zone-forge organs into
//! real named scenes (Thornbell Parish, The Bell Pit, The Under-Orchard,
//! the cathedral). Backed by one flat [`SparseChunkGrid`] — no
//! `BTreeMap<i8, _>` wrapping N separate grids; the world layer (`W`) is
//! folded directly into the chunk key (see `sparse_grid.rs`), so every
//! layer shares one O(1) lookup and unbounded horizontal/vertical extent.
//!
//! Corrects a pasted spec against real APIs: `TritCell5D` is a packed
//! 3-trit-per-axis (`-1,0,+1`) lattice ADDRESS, not a coordinate container
//! with unbounded `x()`/`y()`/`z()`/`k()`/`w()` accessors — it cannot hold
//! permyriad-scale positions or a world-layer index below `-1`, and Girih
//! angle `k` is an authoring-time rotation parameter, not a spatial
//! partition axis (a cell's chunk doesn't change based on which angle
//! placed it). World position uses real `usize`/`i8` coordinates through
//! [`SparseChunkGrid`] instead.
//!
//! Brush 4 (structural check) reuses `structural::buttress::solve` — the
//! already-tested central-third containment solver — rather than a new,
//! unproven trig FS formula.

use crate::atom::ValidityMask;
use crate::fixed_point::MilliUnit;
use crate::zones::ledger::MutationLedger;
use crate::zones::project3d::AIR;
use crate::zones::sparse_grid::SparseChunkGrid;
use crate::zones::structural::buttress::{self, ButtressProfile, ContainmentVerdict, ThrustVector};

/// al-Kāshī module datum: 1 module = `10_000` permyriad = 3.20m.
pub const MODULE_PERMYRIAD: i64 = 10_000;

/// How many storage cells one module spans. Keeps chunk sizes sane at
/// world scale; a real render/GPU wire would pick this per zoom level.
pub const CELLS_PER_MODULE: i64 = 2;

/// Rational Ad Quadratum-family ratios NOT already in `structural::ratio::
/// ConstructiveRatio` (that enum is the ported irrational/sqrt-derivation
/// ratios only — 2:1, 3:2, 4:3 are plain rationals, no reason to extend a
/// sqrt-derivation enum for them).
pub mod rational_ratio {
    /// 1:1 — nave width to aisle width.
    pub const SQUARE_1_1: i64 = 10_000;
    /// 2:1 — nave height to nave width (double square).
    pub const DOUBLE_SQUARE_2_1: i64 = 20_000;
    /// 3:2 — tower rise to nave height (sesquialtera).
    pub const SESQUIALTERA_3_2: i64 = 15_000;
    /// 4:3 — total height to spire apex (sesquitertia).
    pub const SESQUITERTIA_4_3: i64 = 13_333;
}

fn modules_to_cells(modules: i64) -> i64 {
    modules * CELLS_PER_MODULE
}

/// Named tile vocabulary for brush `material: u8` params — existing brushes
/// keep taking raw `u8` (call sites like `build_parish` keep their literal
/// history), but new callers (studio-shell's edit mode) name their tiles
/// instead of inventing another magic number. `raymarch_5d::tile_tint` reads
/// these same discriminants for real per-tile color.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderTile {
    /// Grey masonry — the default "solid platform" tile.
    Stone = 1,
    /// Warm brown — planks, cart tracks, fallen logs.
    Wood = 2,
    /// Packed earth/soil bank.
    Earth = 3,
    /// Pale ash/decay.
    Ash = 4,
    /// Cool cyan — glass/crystal.
    Glass = 5,
    /// Rust-red — iron/metal.
    Iron = 6,
    /// A single-point marker (spawn/exit/prop anchor), not a solid fill.
    Marker = 7,
}

impl WorldBuilderEngine {
    // ── Brush 5: axis-aligned box fill/carve ────────────────────────

    /// Fill (`material != AIR`) or carve (`material == AIR`) a solid AABB
    /// `size_modules` (w,h,d) modules, anchored at `origin` (grid cells) on
    /// world layer `w`. Unlike [`Self::brush_gothic_nave`] this fills the
    /// WHOLE box, not just its shell — the right shape for a flat platform
    /// or prop footprint, which `brush_gothic_nave`'s hollow-room and
    /// `brush_sphere`'s round carve don't fit. Bounded to the box's own
    /// extent, ledger-audited like every other brush.
    pub fn brush_box(
        &mut self,
        w: i8,
        origin: (usize, usize, usize),
        size_modules: (i64, i64, i64),
        material: u8,
        tick: u64,
    ) -> u32 {
        let (sx, sy, sz) = (
            modules_to_cells(size_modules.0).max(1),
            modules_to_cells(size_modules.1).max(1),
            modules_to_cells(size_modules.2).max(1),
        );
        let (ox, oy, oz) = (origin.0 as i64, origin.1 as i64, origin.2 as i64);
        let grid = &mut self.grid;
        let ledger = &mut self.ledger;
        let mut changed = 0u32;
        for z in 0..sz {
            for y in 0..sy {
                for x in 0..sx {
                    let (wx, wy, wz) = (ox + x, oy + y, oz + z);
                    if wx < 0 || wy < 0 || wz < 0 {
                        continue;
                    }
                    let (uwx, uwy, uwz) = (wx as usize, wy as usize, wz as usize);
                    let Some(cell) = grid.get_mut(uwx, uwy, uwz, w) else { continue };
                    let before = *cell;
                    if before.payload[0] == material {
                        continue;
                    }
                    cell.payload[0] = material;
                    cell.validity = ValidityMask::ALL_KNOWN;
                    let after = *cell;
                    ledger.append(before.ordinal, tick, before, after, (uwx, uwy, uwz, w));
                    changed += 1;
                }
            }
        }
        changed
    }
}

/// A named world: one flat, sparse, multi-layer grid plus one shared
/// audit ledger.
pub struct WorldBuilderEngine {
    /// Every world layer, one flat grid — see `sparse_grid.rs`.
    pub grid: SparseChunkGrid,
    /// Every mutation across every layer, one shared audit trail.
    pub ledger: MutationLedger,
}

impl WorldBuilderEngine {
    /// A fresh engine. `chunk_edge` is the grid's chunk granularity
    /// (default `32` per the sparse-grid design), NOT a world size limit
    /// — the world is unbounded, chunks allocate lazily wherever brushes
    /// actually touch.
    pub fn new(chunk_edge: usize) -> Self {
        Self { grid: SparseChunkGrid::new(chunk_edge), ledger: MutationLedger::new() }
    }

    // ── Brush 1: sphere/vault carver ────────────────────────────────

    /// Carve (`material == AIR`) or fill (nonzero) a sphere of
    /// `radius_modules` centred at `center` (grid cells) on world layer
    /// `w`. Returns cells actually changed. Bounded to the sphere's own
    /// AABB, zero extra allocation beyond the ledger's own row storage
    /// and whatever chunks the sphere itself touches.
    pub fn brush_sphere(
        &mut self,
        w: i8,
        center: (usize, usize, usize),
        radius_modules: i64,
        material: u8,
        tick: u64,
    ) -> u32 {
        let radius = modules_to_cells(radius_modules).max(0);
        let (cx, cy, cz) = (center.0 as i64, center.1 as i64, center.2 as i64);
        let lo = |c: i64| (c - radius).max(0) as usize;
        let hi = |c: i64| (c + radius).max(0) as usize + 1;

        let grid = &mut self.grid;
        let ledger = &mut self.ledger;
        let mut changed = 0u32;
        for z in lo(cz)..hi(cz) {
            for y in lo(cy)..hi(cy) {
                for x in lo(cx)..hi(cx) {
                    let (dx, dy, dz) = (x as i64 - cx, y as i64 - cy, z as i64 - cz);
                    if dx * dx + dy * dy + dz * dz > radius * radius {
                        continue;
                    }
                    let Some(cell) = grid.get_mut(x, y, z, w) else { continue };
                    let before = *cell;
                    if before.payload[0] == material {
                        continue;
                    }
                    cell.payload[0] = material;
                    cell.validity = ValidityMask::ALL_KNOWN;
                    let after = *cell;
                    ledger.append(before.ordinal, tick, before, after, (x, y, z, w));
                    changed += 1;
                }
            }
        }
        changed
    }

    // ── Brush 2: concentric ring arena carver (The Bell Pit) ────────

    /// Carve a concentric-ring arena on the XZ plane at world layer `w`:
    /// solid witness wall between `r_wall_inner`/`r_outer`, sunken pit
    /// floor (carved to `AIR`) between `r_pit_inner`/`r_wall_inner` up to
    /// `pit_y`, solid altar core inside `r_pit_inner` up to `altar_y`.
    /// Radii in modules. Integer-only — compares squared radii, no `sqrt`.
    /// Bounded to the arena's own AABB (`r_outer`), not a global extent.
    #[allow(clippy::too_many_arguments)]
    pub fn brush_ring_arena(
        &mut self,
        w: i8,
        core: (usize, usize),
        pit_y: usize,
        altar_y: usize,
        r_pit_inner_modules: i64,
        r_wall_inner_modules: i64,
        r_outer_modules: i64,
        wall_material: u8,
        altar_material: u8,
        tick: u64,
    ) -> u32 {
        let r1 = modules_to_cells(r_pit_inner_modules).max(0);
        let r4 = modules_to_cells(r_wall_inner_modules).max(0);
        let r5 = modules_to_cells(r_outer_modules).max(0);
        let (r1_sq, r4_sq, r5_sq) = (r1 * r1, r4 * r4, r5 * r5);

        let (cx, cz) = (core.0 as i64, core.1 as i64);
        let lo = |c: i64| (c - r5).max(0) as usize;
        let hi = |c: i64| (c + r5).max(0) as usize + 1;
        let y_top = pit_y.max(altar_y).max(1);

        let grid = &mut self.grid;
        let ledger = &mut self.ledger;
        let mut changed = 0u32;

        for z in lo(cz)..hi(cz) {
            for x in lo(cx)..hi(cx) {
                let (dx, dz) = (x as i64 - cx, z as i64 - cz);
                let r_sq = dx * dx + dz * dz;

                let target = if r_sq < r1_sq {
                    Some((altar_material, altar_y))
                } else if r_sq < r4_sq {
                    Some((AIR, pit_y))
                } else if r_sq <= r5_sq {
                    Some((wall_material, y_top))
                } else {
                    None
                };
                let Some((material, y_limit)) = target else { continue };
                for y in 0..=y_limit {
                    let Some(cell) = grid.get_mut(x, y, z, w) else { continue };
                    let before = *cell;
                    if before.payload[0] == material {
                        continue;
                    }
                    cell.payload[0] = material;
                    cell.validity = ValidityMask::ALL_KNOWN;
                    let after = *cell;
                    ledger.append(before.ordinal, tick, before, after, (x, y, z, w));
                    changed += 1;
                }
            }
        }
        changed
    }

    // ── Brush 3: Ad Quadratum gothic nave extruder ──────────────────

    /// Extrude a hollow nave box at world layer `w`: footprint
    /// `width_modules` square (1:1), height per `height_ratio_permyriad`
    /// applied to the width (e.g. [`rational_ratio::DOUBLE_SQUARE_2_1`]
    /// for a 2:1 nave). Fills the shell, carves the interior — a real
    /// hollow room, not a solid block. Returns `(cells_changed,
    /// height_modules)`. Bounded to the nave's own footprint/height, not
    /// a global extent — an unbounded world has no such limit to check.
    pub fn brush_gothic_nave(
        &mut self,
        w: i8,
        origin: (usize, usize, usize),
        width_modules: i64,
        height_ratio_permyriad: i64,
        wall_material: u8,
        tick: u64,
    ) -> (u32, i64) {
        let width_cells = modules_to_cells(width_modules).max(1);
        let width_mu = MilliUnit(width_modules * MODULE_PERMYRIAD);
        let height_mu = MilliUnit(width_mu.0 * height_ratio_permyriad / MODULE_PERMYRIAD);
        let height_modules = height_mu.0 / MODULE_PERMYRIAD;
        let height_cells = modules_to_cells(height_modules).max(1);

        let (ox, oy, oz) = (origin.0 as i64, origin.1 as i64, origin.2 as i64);
        let grid = &mut self.grid;
        let ledger = &mut self.ledger;
        let mut changed = 0u32;

        for z in 0..width_cells {
            for y in 0..height_cells {
                for x in 0..width_cells {
                    let (wx, wy, wz) = (ox + x, oy + y, oz + z);
                    if wx < 0 || wy < 0 || wz < 0 {
                        continue;
                    }
                    let (wx, wy, wz) = (wx as usize, wy as usize, wz as usize);

                    let on_shell =
                        x == 0 || x == width_cells - 1 || z == 0 || z == width_cells - 1 || y == height_cells - 1;
                    let material = if on_shell { wall_material } else { AIR };

                    let Some(cell) = grid.get_mut(wx, wy, wz, w) else { continue };
                    let before = *cell;
                    if before.payload[0] == material {
                        continue;
                    }
                    cell.payload[0] = material;
                    cell.validity = ValidityMask::ALL_KNOWN;
                    let after = *cell;
                    ledger.append(before.ordinal, tick, before, after, (wx, wy, wz, w));
                    changed += 1;
                }
            }
        }
        (changed, height_modules)
    }

    // ── Spire tower: stacked, narrowing gothic naves ─────────────────

    /// A tapered spire/tower centred on `(center_x, center_z)`: `segments`
    /// stacked hollow boxes (via [`brush_gothic_nave`](Self::
    /// brush_gothic_nave)), each narrower than the last by
    /// `taper_modules` and re-centred so the tower narrows symmetrically
    /// inward (not shrinking from one corner). Returns total cells
    /// changed.
    #[allow(clippy::too_many_arguments)]
    pub fn brush_tapered_spire(
        &mut self,
        w: i8,
        center_x: usize,
        base_y: usize,
        center_z: usize,
        base_width_modules: i64,
        segments: usize,
        taper_modules: i64,
        wall_material: u8,
        tick: u64,
    ) -> u32 {
        let mut total = 0u32;
        let mut width = base_width_modules;
        let mut y_cursor = base_y;
        for _ in 0..segments {
            if width < 1 {
                break;
            }
            let half = modules_to_cells(width).max(1) as usize / 2;
            let origin_x = center_x.saturating_sub(half);
            let origin_z = center_z.saturating_sub(half);
            let (changed, height_modules) = self.brush_gothic_nave(
                w,
                (origin_x, y_cursor, origin_z),
                width,
                rational_ratio::SQUARE_1_1, // each segment: 1:1, tapering does the "spire" shape
                wall_material,
                tick,
            );
            total += changed;
            y_cursor += modules_to_cells(height_modules).max(1) as usize;
            width -= taper_modules;
        }
        total
    }

    // ── Brush 4: flying buttress structural gate ────────────────────

    /// Real structural check, reusing the already-tested central-third
    /// buttress solver (`structural::buttress::solve`) rather than a new,
    /// unproven trig FS formula. `true` iff the solve contains the thrust
    /// vector within `max_width`.
    pub fn brush_buttress_check(&self, profile: ButtressProfile, thrust: &ThrustVector, max_width: MilliUnit) -> bool {
        matches!(
            buttress::solve(profile, thrust, max_width).verdict,
            ContainmentVerdict::Contained
        )
    }

    /// A single-point marker at world `(x,y,z)` on layer `w` — used to
    /// reach an extreme coordinate (e.g. a spire apex at `Y=1000` cells /
    /// 1.6km) without filling solid material through the entire empty
    /// span between it and the nearest built structure. The gap in
    /// between stays completely unallocated — that IS the sparse claim,
    /// not a shortcut around it. Also `studio-shell`'s undo/redo single-
    /// cell write primitive — this already IS "write one cell, ledger it".
    pub fn brush_marker(&mut self, w: i8, at: (usize, usize, usize), material: u8, tick: u64) -> u32 {
        let Some(cell) = self.grid.get_mut(at.0, at.1, at.2, w) else { return 0 };
        let before = *cell;
        if before.payload[0] == material {
            return 0;
        }
        cell.payload[0] = material;
        cell.validity = ValidityMask::ALL_KNOWN;
        let after = *cell;
        self.ledger.append(before.ordinal, tick, before, after, (at.0, at.1, at.2, w));
        1
    }

    /// Build the canonical Thornbell Parish complex on surface layer (W=0):
    /// the market-row ring arena and the gothic forge nave.
    /// Returns total cells changed.
    pub fn build_parish(&mut self) -> u32 {
        let mut total = 0u32;
        total += self.brush_ring_arena(0, (20, 20), 2, 2, 3, 8, 10, 7, 3, 1);
        let (nave_changed, _) = self.brush_gothic_nave(0, (2, 0, 2), 5, rational_ratio::SQUARE_1_1, 4, 2);
        total += nave_changed;
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::raymarch::{CameraMode, PovCamera};
    use crate::zones::structural::ratio::octagon_offset;

    #[test]
    fn brush_sphere_carves_and_fills_on_the_named_layer_only() {
        let mut engine = WorldBuilderEngine::new(32);
        let filled = engine.brush_sphere(0, (10, 10, 10), 3, 9, 1);
        assert!(filled > 0);
        // A different layer is untouched by that call.
        assert_eq!(engine.grid.allocated_chunk_count_on_layer(-1), 0);
    }

    #[test]
    fn brush_box_fills_the_whole_aabb_and_carves_it_back() {
        let mut engine = WorldBuilderEngine::new(32);
        let filled = engine.brush_box(0, (4, 0, 4), (2, 1, 1), BuilderTile::Stone as u8, 1);
        assert!(filled > 0, "box fill must touch cells");
        assert_eq!(
            engine.grid.get(4, 0, 4, 0).unwrap().payload[0],
            BuilderTile::Stone as u8,
            "the box's own origin cell must carry the fill material"
        );
        let carved = engine.brush_box(0, (4, 0, 4), (2, 1, 1), AIR, 2);
        assert!(carved > 0, "carve pass must touch the same cells again");
        assert_eq!(engine.grid.get(4, 0, 4, 0).unwrap().payload[0], AIR);
    }

    /// L18-style sabotage: a box brush that only fills its origin corner
    /// (not the whole AABB) must fail this width check.
    #[test]
    fn brush_box_fills_full_width_not_just_the_corner() {
        let mut engine = WorldBuilderEngine::new(32);
        engine.brush_box(0, (0, 0, 0), (3, 1, 1), BuilderTile::Stone as u8, 1);
        let far_x = (3 * CELLS_PER_MODULE - 1) as usize;
        assert_eq!(
            engine.grid.get(far_x, 0, 0, 0).unwrap().payload[0],
            BuilderTile::Stone as u8,
            "the box's far edge must be filled, not just its origin"
        );
    }

    /// Scene 1: Thornbell Parish — a small market-row ring plus one
    /// Ad Quadratum forge nave, both on the surface layer (W=0).
    #[test]
    fn thornbell_parish_market_and_forge() {
        let mut engine = WorldBuilderEngine::new(32);
        let ring_changed = engine.brush_ring_arena(0, (20, 20), 2, 2, 3, 8, 10, 7, 3, 1);
        assert!(ring_changed > 0, "market row ring must carve/build something");

        let (nave_changed, height_modules) =
            engine.brush_gothic_nave(0, (2, 0, 2), 5, rational_ratio::SQUARE_1_1, 4, 2);
        assert!(nave_changed > 0, "forge nave shell must be built");
        assert_eq!(height_modules, 5, "1:1 ratio keeps height == width in modules");
    }

    /// Scene 2: The Bell Pit — concentric rings down to a sunken floor
    /// and a solid altar core, using the same brush the market row used.
    #[test]
    fn the_bell_pit_concentric_rings() {
        let mut engine = WorldBuilderEngine::new(32);
        let changed = engine.brush_ring_arena(0, (25, 25), 1, 0, 2, 6, 10, 5, 9, 10);
        assert!(changed > 0);
        // Altar core cell must be solid.
        assert_eq!(engine.grid.get(25, 0, 25, 0).unwrap().payload[0], 9);
    }

    /// Scene 3: The Under-Orchard — two real subterranean layers
    /// (W=-1 Root Cellar, W=-2 Bell Vein), same XYZ, distinct chunks.
    #[test]
    fn the_under_orchard_dual_subterranean_layers() {
        let mut engine = WorldBuilderEngine::new(32);
        let cellar = engine.brush_sphere(-1, (15, 15, 15), 4, 6, 1);
        let vein = engine.brush_sphere(-2, (15, 15, 15), 6, 5, 2);
        assert!(cellar > 0);
        assert!(vein > 0);
        assert!(engine.grid.allocated_chunk_count_on_layer(-1) > 0);
        assert!(engine.grid.allocated_chunk_count_on_layer(-2) > 0);
        assert_eq!(engine.grid.allocated_chunk_count_on_layer(0), 0);
    }

    /// Brush 4: reuse the real, already-tested buttress solver — an
    /// octagon-derived thrust vector either contains or breaches budget.
    #[test]
    fn brush_buttress_check_reuses_the_real_solver() {
        let engine = WorldBuilderEngine::new(1);
        let r = MilliUnit(10_000);
        let (ox, oy) = octagon_offset(r, 1);
        let thrust = ThrustVector {
            origin_x: ox,
            origin_y: oy,
            direction_x: MilliUnit(500),
            direction_y: MilliUnit(0),
            magnitude: MilliUnit(10_000),
        };
        let profile = ButtressProfile { width: MilliUnit(600), height: MilliUnit(3_000), position_x: 0, position_y: 0 };
        // Just confirm this calls through and returns a real bool, not a
        // panic — the solver's own tests (buttress.rs) already prove its
        // math; this proves the wiring.
        let _ = engine.brush_buttress_check(profile, &thrust, MilliUnit(20_000));
    }

    /// Replace the mound: a real Gothic silhouette (base plinth + nave +
    /// twin tapered spires + crossing tower), same shape family as the
    /// Ironroot MVP blueprint. Rendered from the same south-elevation
    /// angle as `island_gate.png` for a real comparison.
    #[test]
    fn cathedral_replaces_the_mound() {
        let mut engine = WorldBuilderEngine::new(32);
        let (cx, cz) = (48usize, 48usize);
        let tick_seq = &mut (1u64..);
        let mut tick = || tick_seq.next().unwrap();

        let (plinth_changed, plinth_h) =
            engine.brush_gothic_nave(0, (cx - 10, 0, cz - 10), 10, rational_ratio::SESQUITERTIA_4_3, 8, tick());
        assert!(plinth_changed > 0);

        let plinth_cells = (plinth_h * CELLS_PER_MODULE).max(1) as usize;
        let (nave_changed, nave_h) = engine.brush_gothic_nave(
            0,
            (cx - 6, plinth_cells, cz - 6),
            12,
            rational_ratio::DOUBLE_SQUARE_2_1,
            7,
            tick(),
        );
        assert!(nave_changed > 0);

        let nave_top = plinth_cells + (nave_h * CELLS_PER_MODULE).max(1) as usize;
        let tower_changed = engine.brush_tapered_spire(0, cx, nave_top, cz, 6, 3, 2, 6, tick());
        assert!(tower_changed > 0);

        let north_spire = engine.brush_tapered_spire(0, cx - 12, plinth_cells, cz - 6, 3, 3, 1, 5, tick());
        let south_spire = engine.brush_tapered_spire(0, cx + 12, plinth_cells, cz - 6, 3, 3, 1, 5, tick());
        assert!(north_spire > 0);
        assert!(south_spire > 0);

        let total_changed = plinth_changed + nave_changed + tower_changed + north_spire + south_spire;
        assert!(total_changed > 500, "a cathedral silhouette should touch a real number of cells");

        let camera = PovCamera::new(CameraMode::SouthElevation, 0);
        let frame = camera.render_frame_sparse(&engine.grid, 0, 96, 256, 256);
        let has_solid = frame.chunks_exact(3).any(|p| p == [220, 220, 220]);
        assert!(has_solid, "elevation view must show solid masonry");

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.forge/photons");
        std::fs::create_dir_all(&dir).expect("create .forge/photons");
        let mut file = std::fs::File::create(dir.join("cathedral_south_elevation.ppm")).expect("create ppm");
        use std::io::Write;
        writeln!(file, "P6\n256 256\n255").expect("write ppm header");
        file.write_all(&frame).expect("write ppm body");
    }

    /// Full-world composite WITNESS: a real 1.6km cathedral (ground to a
    /// Y=500-module apex marker) plus Thornbell Parish on the surface
    /// layer, and the Under-Orchard (root cellars + Bell Pit drop) on a
    /// subterranean layer — all through ONE flat `SparseChunkGrid`, all
    /// under a hard chunk-count budget proving sparsity is real, not
    /// just a design intent.
    #[test]
    fn test_full_world_sparse_composite() {
        let mut engine = WorldBuilderEngine::new(32);
        let tick_seq = &mut (1u64..);
        let mut tick = || tick_seq.next().unwrap();
        let (cx, cz) = (200usize, 200usize);

        // Surface (W=0): cathedral fortress, ground through a modest
        // tower, THEN a single marker cell at the literal 1.6km apex
        // (Y=500 modules = 1000 cells at CELLS_PER_MODULE=2) — the huge
        // gap between tower-top and apex stays entirely unallocated.
        let (plinth_changed, plinth_h) =
            engine.brush_gothic_nave(0, (cx - 10, 0, cz - 10), 10, rational_ratio::SESQUITERTIA_4_3, 8, tick());
        let plinth_cells = (plinth_h * CELLS_PER_MODULE).max(1) as usize;
        let (nave_changed, nave_h) =
            engine.brush_gothic_nave(0, (cx - 6, plinth_cells, cz - 6), 12, rational_ratio::DOUBLE_SQUARE_2_1, 7, tick());
        let nave_top = plinth_cells + (nave_h * CELLS_PER_MODULE).max(1) as usize;
        let tower_changed = engine.brush_tapered_spire(0, cx, nave_top, cz, 6, 3, 2, 6, tick());
        let apex_y = (500 * CELLS_PER_MODULE) as usize; // 1000 cells = 1600m exactly
        let apex_changed = engine.brush_marker(0, (cx, apex_y, cz), 3, tick());
        assert!(plinth_changed > 0 && nave_changed > 0 && tower_changed > 0);
        assert_eq!(apex_changed, 1, "the 1.6km cross finial must actually be placed");

        // Surface (W=0), a different location: Thornbell Parish town loop.
        let market_changed = engine.brush_ring_arena(0, (cx + 60, cz + 60), 2, 2, 3, 8, 10, 7, 3, tick());
        assert!(market_changed > 0);

        // Subterranean (W=-1): Under-Orchard root cellars + Bell Pit
        // drop (a local depth of 100 cells within this layer's own grid
        // — W already carries the surface/subterranean sign, so a local
        // Y here means "100 cells down," never a signed world Y).
        let cellar_changed = engine.brush_sphere(-1, (cx, 10, cz), 4, 6, tick());
        let bell_pit_changed = engine.brush_ring_arena(-1, (cx + 30, cz), 100, 95, 4, 10, 14, 9, 5, tick());
        assert!(cellar_changed > 0);
        assert!(bell_pit_changed > 0);

        // Memory audit: the real receipt, not a design claim.
        let allocated = engine.grid.allocated_chunk_count();
        let footprint = engine.grid.byte_footprint();
        assert!(
            allocated < 50,
            "expected under 50 allocated chunks across both layers, found {allocated}"
        );
        assert!(
            footprint < 13 * 1024 * 1024,
            "expected under 13MB total, found {footprint} bytes across {allocated} chunks"
        );

        // Render the composite surface layer.
        let camera = PovCamera::new(CameraMode::SouthElevation, 0);
        let frame = camera.render_frame_sparse(&engine.grid, 0, apex_y + 32, 512, 512);
        let has_solid = frame.chunks_exact(3).any(|p| p == [220, 220, 220]);
        assert!(has_solid, "composite elevation must show real masonry");

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.forge/photons");
        std::fs::create_dir_all(&dir).expect("create .forge/photons");
        let mut file = std::fs::File::create(dir.join("sparse_world_full_elevation.ppm")).expect("create ppm");
        use std::io::Write;
        writeln!(file, "P6\n512 512\n255").expect("write ppm header");
        file.write_all(&frame).expect("write ppm body");
    }
}
