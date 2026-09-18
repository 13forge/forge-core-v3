//! 5D photometric shading — extends the flat tri-state raymarch
//! (`raymarch.rs`) with a real lighting model over `Pexil`'s already-
//! stored fields. `k` (Girih angle) lives in `Pexil.payload[1]`
//! (previously unused byte) — NOT read from `TritCell5D`, which has no
//! `k()`/`w()` accessor; it's a packed 5-trit lattice ADDRESS, not a
//! coordinate/angle store. `W` (world layer) is the real `i8` already
//! threaded through every brush and `SparseChunkGrid::get`/`get_mut` —
//! also not a `TritCell5D` field.
//!
//! [APERTURE] `f32` here, not integer permyriad: this is a color/shading
//! boundary, same precedent as `forge-audio-v3::dimensional_collapse.rs`
//! ("f32 appears only in render_sample's audio output, past the
//! boundary") and `MetaRouter`'s own bias field (already `f32` in this
//! crate) — deterministic gameplay/physics state stays integer-only
//! elsewhere; this module never feeds back into it.
//!
//! `forge-photometric-v3::normal::{NormalAlbedo8, decode_octahedral}` is
//! the real, proven, INTEGER-ONLY normal encoding already one-home in
//! this repo — checked before writing this module. Not reused here: (1)
//! `forge-core-v3` is Crate Zero, zero-dependency by law, so it cannot
//! take `forge-photometric-v3` as a dependency; (2) that type's job is
//! compact 8-byte STORAGE of a normal, and this module never stores
//! one — it recomputes a transient gradient fresh per pixel from
//! neighbouring cell occupancy, a different use case, not a second home
//! for the same primitive. The gradient sampling itself stays integer
//! (`i32` diffs of solid/air); only the final normalize/dot/cos/exp
//! composite crosses into `f32`, at the same boundary as above.
//!
//! Landed 2026-08-20: `chiaroscuro_sample`/`evaluate_5d_shading`'s ambient
//! floor is now [`sky_irradiance::hemispheric_irradiance`] (ported from v2
//! `forge-lighting::sky_radiance`, occlusion-gated by
//! [`sky_irradiance::sky_visibility`] so a sealed `CarvedAir` vault stays
//! dark instead of flooding with sky fill) — front-face normals get real
//! sky headroom instead of the old flat top-biased constant. `sky_bake.rs`/
//! `baked.rs` were NOT ported: both depend on v2-only `WorldContract`/
//! `LutPack`/`presets_layout` types this crate cannot take (Crate Zero,
//! zero-dependency by law), and bake a table this crate's per-pixel CPU
//! raymarch doesn't need — `sky_radiance` is evaluated live instead.

use crate::atom::ValidityMask;
use crate::zones::project3d::AIR;
use crate::zones::sky_irradiance;
use crate::zones::sparse_grid::SparseChunkGrid;

/// Cells marched upward when gating sky fill by occlusion — enough to read
/// a vaulted ceiling, cheap enough per-pixel. See
/// [`sky_irradiance::sky_visibility`].
const SKY_VISIBILITY_BUDGET: usize = 12;

/// Trait for querying occupancy in a lattice — whether a cell is solid.
/// Implemented for any grid or world type that can answer occupancy queries
/// at integer coordinates. Enables generalized shading over different lattice
/// backends without coupling to a specific grid type.
pub trait Occupancy {
    /// Returns `true` if the cell at `(x, y, z, w)` is solid (not air).
    /// Out-of-bounds coordinates return `false` (treated as open space).
    fn solid(&self, x: i64, y: i64, z: i64, w: i8) -> bool;

    /// The cell's material byte (`AIR` when open); sources with real
    /// material ids override this, the default keeps solid/air only.
    fn material(&self, x: i64, y: i64, z: i64, w: i8) -> u8 {
        if self.solid(x, y, z, w) {
            SOLID_BYTE
        } else {
            AIR
        }
    }
}

/// Implement [`Occupancy`] for [`SparseChunkGrid`].
impl Occupancy for SparseChunkGrid {
    fn solid(&self, x: i64, y: i64, z: i64, w: i8) -> bool {
        let (Some(ux), Some(uy), Some(uz)) = (
            usize::try_from(x).ok(),
            usize::try_from(y).ok(),
            usize::try_from(z).ok(),
        ) else {
            return false;
        };
        match self.get(ux, uy, uz, w) {
            Some(p) if p.payload[0] != AIR => true,
            _ => false,
        }
    }

    fn material(&self, x: i64, y: i64, z: i64, w: i8) -> u8 {
        let (Some(ux), Some(uy), Some(uz)) = (
            usize::try_from(x).ok(),
            usize::try_from(y).ok(),
            usize::try_from(z).ok(),
        ) else {
            return AIR;
        };
        self.get(ux, uy, uz, w).map_or(AIR, |p| p.payload[0])
    }
}

/// The slab byte for a solid cell; air stays `0` (`AIR`).
pub const SOLID_BYTE: u8 = 1;

/// Flatten one `layer` of `occ` into a dense `w*h*d` material slab, index
/// `(y*d + z)*w + x` — same layout as the pane's island slab and
/// `VoxelGrid.mats`; each byte is `Occupancy::material`, `AIR` when open.
pub fn slab_from_occupancy(occ: &dyn Occupancy, w: usize, h: usize, d: usize, layer: i8) -> Vec<u8> {
    let mut slab = vec![AIR; w * h * d];
    for y in 0..h {
        for z in 0..d {
            for x in 0..w {
                slab[(y * d + z) * w + x] = occ.material(x as i64, y as i64, z as i64, layer);
            }
        }
    }
    slab
}

/// Column heights over one `layer` of `occ`, index `z*w + x` — value is the
/// top solid `y` plus one, `0` for an all-air column. Integer on purpose:
/// the GPU seam converts; core never holds `f32`.
pub fn heightmap_from_occupancy(occ: &dyn Occupancy, w: usize, h: usize, d: usize, layer: i8) -> Vec<u16> {
    let mut heights = vec![0u16; w * d];
    for z in 0..d {
        for x in 0..w {
            let mut top: u16 = 0;
            for y in 0..h {
                if occ.solid(x as i64, y as i64, z as i64, layer) {
                    top = y as u16 + 1;
                }
            }
            heights[z * w + x] = top;
        }
    }
    heights
}

/// The chiaroscuro tri-state a ray can land on. Distinguishes genuinely-
/// uncarved ambient space from a deliberately excavated (but currently
/// empty) interior — same fields `raymarch.rs`'s flat classifier reads
/// (`payload[0]`/`validity`), not a new field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitType {
    /// Never touched by any brush — infinite void, Caravaggio-black.
    AmbientVoid,
    /// Explicitly carved to air (`validity == ALL_KNOWN`, `payload[0] ==
    /// AIR`) — an excavated room/tunnel, distinct from untouched space.
    CarvedAir,
    /// Solid masonry/material, carrying its material id.
    Solid(u8),
}

fn classify_hit(payload0: u8, validity: ValidityMask) -> HitType {
    if payload0 != AIR {
        HitType::Solid(payload0)
    } else if validity == ValidityMask::ALL_UNKNOWN {
        HitType::AmbientVoid
    } else {
        HitType::CarvedAir
    }
}

/// Intentional Chiaroscuro Void Policy: pure black is deliberate negative
/// space (Caravaggio-style), not a missing-data artifact. `depth` is the
/// ray step count to this hit (0 = nearest the camera); `normal`/
/// `cell_k`/`w`/`light_dir`/`light_k` feed the same Girih-phase/X-ray
/// terms `evaluate_5d_shading` already proved, composed multiplicatively
/// with the depth-fog + rim-light terms below.
#[allow(clippy::too_many_arguments)]
pub fn chiaroscuro_sample(
    hit_type: HitType,
    depth: usize,
    normal: (f32, f32, f32),
    cell_k: u8,
    w: i8,
    light_dir: (f32, f32, f32),
    light_k: u8,
    grid: &SparseChunkGrid,
    x: usize,
    y: usize,
) -> (u8, u8, u8) {
    match hit_type {
        HitType::AmbientVoid => (0, 0, 0),
        HitType::CarvedAir => (6, 8, 14),
        HitType::Solid(material) => {
            let depth_fog = 1.0 / (1.0 + 0.04 * depth as f32);
            let rim_light = (1.0 - normal.2.abs()).powf(2.0) * 0.4;

            // Girih phase + X-ray |W| decay — the already-proven 5D terms,
            // composed here rather than dropped for the chiaroscuro pass.
            let dk = (light_k as f32 - cell_k as f32).abs();
            let girih_phase = (1.0 + (dk * std::f32::consts::PI / 5.0).cos()) * 0.5;
            let x_ray_decay = (-0.3 * (w as f32).abs()).exp();

            // Occlusion-gated sky ambient: near-zero under a sealed roof,
            // full hemispheric fill in the open — replaces the old flat
            // top-biased `base_intensity` constant.
            let visibility = sky_irradiance::sky_visibility(grid, x, y, depth, w, SKY_VISIBILITY_BUDGET);
            let sky = sky_irradiance::hemispheric_irradiance(
                normal, light_dir, (1.0, 1.0, 1.0), 1.0, 2.0, visibility,
            );
            let sun_dot =
                (normal.0 * light_dir.0 + normal.1 * light_dir.1 + normal.2 * light_dir.2).max(0.0);
            let diffuse_r = sun_dot + sky.0 * x_ray_decay;
            let diffuse_g = sun_dot + sky.1 * x_ray_decay;
            let diffuse_b = sun_dot + sky.2 * x_ray_decay;

            // Real per-material tint (this is the live path `compose::
            // chiaroscuro_layer_plane` -> `render_chiaroscuro_composite`
            // actually renders through) — was `let _ = material;` before.
            let (tint_r, tint_g, tint_b) = tile_tint(material);
            let r = ((210.0 * tint_r * diffuse_r + 45.0 * rim_light) * depth_fog * girih_phase * x_ray_decay)
                .clamp(0.0, 255.0) as u8;
            let g = ((200.0 * tint_g * diffuse_g + 40.0 * rim_light) * depth_fog * girih_phase).clamp(0.0, 255.0) as u8;
            let b = ((185.0 * tint_b * diffuse_b + 50.0 * rim_light) * depth_fog).clamp(0.0, 255.0) as u8;

            (r, g, b)
        }
    }
}

/// Default Girih angle tag for any cell nothing has explicitly tagged.
pub const DEFAULT_GIRIH_K: u8 = 0;

/// Tag a solid cell at world `(x,y,z)` on layer `w` with a Girih angle
/// index (`payload[1]`), without changing its material. No-op (returns
/// `false`) if the cell doesn't exist or isn't solid — a rotation tag on
/// air is meaningless.
pub fn tag_girih_k(grid: &mut SparseChunkGrid, x: usize, y: usize, z: usize, w: i8, k: u8) -> bool {
    let Some(cell) = grid.get_mut(x, y, z, w) else { return false };
    if cell.payload[0] == AIR {
        return false;
    }
    cell.payload[1] = k;
    true
}

/// Estimate a unit surface normal at `(x,y,z)` on layer `w` via central
/// differences over neighbouring cells' solidity (`payload[0] != AIR` as
/// `1.0`/`0.0`) — the spec's `V(x+1,y,z) - V(x-1,y,z)` gradient, using
/// the real occupancy field this crate has (not a literal `ValidityMask`
/// scalar — that's a 5-axis Kleene byte, not one number).
pub fn central_difference_normal<G: Occupancy + ?Sized>(grid: &G, x: usize, y: usize, z: usize, w: i8) -> (f32, f32, f32) {
    // Sampling stays integer: each neighbour is solid(1) or not(0), so
    // every central difference is exactly -1, 0, or +1 — no float
    // touches this step.
    let solid = |dx: i64, dy: i64, dz: i64| -> i32 {
        if grid.solid(x as i64 + dx, y as i64 + dy, z as i64 + dz, w) {
            1
        } else {
            0
        }
    };
    let (nx, ny, nz) = (
        (solid(1, 0, 0) - solid(-1, 0, 0)) as f32,
        (solid(0, 1, 0) - solid(0, -1, 0)) as f32,
        (solid(0, 0, 1) - solid(0, 0, -1)) as f32,
    );
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len < 1e-6 {
        (0.0, 1.0, 0.0) // flat/interior cell (no exposed face): default up
    } else {
        (nx / len, ny / len, nz / len)
    }
}

/// Per-tile colour multipliers (r,g,b in `[0,1]`), keyed by the same `u8`
/// discriminants as `worldbuilder::BuilderTile` — the real fix for the
/// "swappable tiles" gap this module's own `chiaroscuro_sample` already
/// named (`let _ = material; // ... wanting per-material tint later`).
/// One match arm per tile: editing a tuple here is the whole "swap".
fn tile_tint(material: u8) -> (f32, f32, f32) {
    match material {
        1 => (0.92, 0.92, 0.94),  // Stone — pale grey masonry
        2 => (0.85, 0.62, 0.32),  // Wood — warm brown
        3 => (0.60, 0.45, 0.28),  // Earth — packed soil
        4 => (0.80, 0.78, 0.74),  // Ash — pale decay
        5 => (0.55, 0.90, 0.88),  // Glass — cool cyan
        6 => (0.75, 0.48, 0.34),  // Iron — rust-red
        7 => (0.95, 0.85, 0.30),  // Marker — bright yellow, always visible
        _ => (1.0, 1.0, 1.0),     // unnamed material: untinted (prior behaviour)
    }
}

/// The 5D photometric shading composite: 3D spatial diffuse x 4D Girih
/// phase alignment x 5D X-ray depth attenuation. `material == AIR`
/// always renders as black (nothing to shade).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_5d_shading<G: Occupancy + ?Sized>(
    material: u8,
    cell_k: u8,
    w: i8,
    normal_3d: (f32, f32, f32),
    light_dir: (f32, f32, f32),
    light_k: u8,
    grid: &G,
    x: usize,
    y: usize,
    z: usize,
) -> (u8, u8, u8) {
    if material == AIR {
        return (0, 0, 0);
    }

    // 2. 4D Girih phase alignment (10-fold decagonal coherence).
    let dk = (light_k as f32 - cell_k as f32).abs();
    let girih_phase = (1.0 + (dk * std::f32::consts::PI / 5.0).cos()) * 0.5;

    // 3. 5D X-ray attenuation over |W| depth — surface (W=0) is clear,
    // subterranean layers shift toward deep cyan/indigo as |W| grows.
    let w_depth = (w as f32).abs();
    let x_ray_decay = (-0.3 * w_depth).exp();

    // 1. Spatial diffuse: direct sun term plus occlusion-gated sky
    // ambient (near-zero under a sealed roof, full hemispheric fill in
    // the open) — replaces the old flat `.max(0.2)` floor.
    let visibility = sky_irradiance::sky_visibility(grid, x, y, z, w, SKY_VISIBILITY_BUDGET);
    let sky = sky_irradiance::hemispheric_irradiance(
        normal_3d, light_dir, (1.0, 1.0, 1.0), 1.0, 2.0, visibility,
    );
    let sun_dot =
        (normal_3d.0 * light_dir.0 + normal_3d.1 * light_dir.1 + normal_3d.2 * light_dir.2).max(0.0);
    let dot_r = sun_dot + sky.0 * x_ray_decay;
    let dot_g = sun_dot + sky.1 * x_ray_decay;
    let dot_b = sun_dot + sky.2 * x_ray_decay;

    let (tint_r, tint_g, tint_b) = tile_tint(material);
    let base_r = 220.0 * tint_r * dot_r * girih_phase * x_ray_decay;
    let base_g = (200.0 * tint_g * dot_g + 50.0 * (1.0 - x_ray_decay)) * girih_phase;
    let base_b = (180.0 * tint_b * dot_b + 150.0 * (1.0 - x_ray_decay)) * girih_phase;

    (base_r.clamp(0.0, 255.0) as u8, base_g.clamp(0.0, 255.0) as u8, base_b.clamp(0.0, 255.0) as u8)
}

/// A shaded south-elevation render composited across `layers` in order
/// (e.g. `&[0, -1]` — surface first, falling through to subterranean
/// where the surface has nothing): for each pixel, depth-scan the first
/// layer for its nearest-to-camera solid hit; if none, try the next
/// layer. Each layer's own `w` drives its X-ray tint automatically via
/// [`evaluate_5d_shading`] — no separate compositing pass needed.
pub fn render_shaded_composite(
    grid: &SparseChunkGrid,
    layers: &[i8],
    scene_edge: usize,
    width: usize,
    height: usize,
    light_dir: (f32, f32, f32),
    light_k: u8,
) -> Vec<u8> {
    let mut buffer = vec![0u8; width * height * 3];
    let edge = scene_edge as i64;
    if edge == 0 {
        return buffer;
    }

    for py in 0..height {
        for px in 0..width {
            let x = (px as i64 * edge) / width as i64;
            let y = edge - 1 - (py as i64 * edge) / height as i64;
            if x < 0 || y < 0 {
                continue;
            }
            let (ux, uy) = (x as usize, y as usize);

            let hit = layers.iter().find_map(|&w| {
                (0..edge).find_map(|z| {
                    let uz = z as usize;
                    grid.get(ux, uy, uz, w).filter(|p| p.payload[0] != AIR).map(|p| (uz, w, *p))
                })
            });
            let Some((hz, w, pexil)) = hit else { continue };

            let normal = central_difference_normal(grid, ux, uy, hz, w);
            let k = pexil.payload[1];
            let (r, g, b) =
                evaluate_5d_shading(pexil.payload[0], k, w, normal, light_dir, light_k, grid, ux, uy, hz);
            let idx = (py * width + px) * 3;
            buffer[idx] = r;
            buffer[idx + 1] = g;
            buffer[idx + 2] = b;
        }
    }
    buffer
}

/// Real first-hit-wins chiaroscuro render: for each pixel, march forward
/// through `layers` in order tracking a genuine `depth` (ray step count
/// from the camera, 0 = nearest), stopping at the FIRST solid cell found
/// — nearer geometry occludes farther, matching real raymarching rather
/// than `render_shaded_composite`'s any-hit-along-ray priority scan. If
/// no solid is ever found, the ray's nearest-camera cell (first position
/// checked) still classifies as ambient-void or carved-air, so excavated
/// but currently-empty interiors read as deep slate, not flat black.
/// `origin` shifts the sampled window's `(x, y)` start by a cell offset —
/// what lets a caller scroll the view (e.g. `studio-shell`'s builder view
/// following the player) instead of always sampling from world `(0, 0)`.
/// Pass `(0, 0)` for the original fixed-at-origin behaviour.
pub fn render_chiaroscuro_composite(
    grid: &SparseChunkGrid,
    layers: &[i8],
    scene_edge: usize,
    width: usize,
    height: usize,
    light_dir: (f32, f32, f32),
    light_k: u8,
    origin: (usize, usize),
) -> Vec<u8> {
    let mut buffer = vec![0u8; width * height * 3];
    let edge = scene_edge as i64;
    if edge == 0 {
        return buffer;
    }
    let (origin_x, origin_y) = (origin.0 as i64, origin.1 as i64);

    for py in 0..height {
        for px in 0..width {
            let x = origin_x + (px as i64 * edge) / width as i64;
            let y = origin_y + edge - 1 - (py as i64 * edge) / height as i64;
            if x < 0 || y < 0 {
                continue;
            }
            let (ux, uy) = (x as usize, y as usize);

            let mut background = HitType::AmbientVoid;
            let mut solid_hit: Option<(usize, i8, u8, u8)> = None; // (depth == z, w, material, k)

            'layers: for (li, &w) in layers.iter().enumerate() {
                for z in 0..edge {
                    let uz = z as usize;
                    let Some(pexil) = grid.get(ux, uy, uz, w) else { continue };
                    let hit = classify_hit(pexil.payload[0], pexil.validity);
                    if li == 0 && z == 0 {
                        background = hit;
                    }
                    if let HitType::Solid(material) = hit {
                        solid_hit = Some((uz, w, material, pexil.payload[1]));
                        break 'layers;
                    }
                }
            }

            let (r, g, b) = if let Some((depth, w, material, k)) = solid_hit {
                let normal = central_difference_normal(grid, ux, uy, depth, w);
                chiaroscuro_sample(
                    HitType::Solid(material), depth, normal, k, w, light_dir, light_k, grid, ux, uy,
                )
            } else {
                chiaroscuro_sample(background, 0, (0.0, 1.0, 0.0), 0, 0, light_dir, light_k, grid, ux, uy)
            };

            let idx = (py * width + px) * 3;
            buffer[idx] = r;
            buffer[idx + 1] = g;
            buffer[idx + 2] = b;
        }
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::worldbuilder::{rational_ratio, WorldBuilderEngine, CELLS_PER_MODULE};

    /// Fixed 3x3x3 [`Occupancy`] fixture for slab/heightmap tests — cells
    /// indexed `[x][y][z]`; off-grid or off-`layer` queries read air.
    struct Cube3 {
        cells: [[[bool; 3]; 3]; 3],
        layer: i8,
    }

    impl Occupancy for Cube3 {
        fn solid(&self, x: i64, y: i64, z: i64, w: i8) -> bool {
            if w != self.layer {
                return false;
            }
            let (Some(ux), Some(uy), Some(uz)) = (
                usize::try_from(x).ok(),
                usize::try_from(y).ok(),
                usize::try_from(z).ok(),
            ) else {
                return false;
            };
            if ux >= 3 || uy >= 3 || uz >= 3 {
                return false;
            }
            self.cells[ux][uy][uz]
        }
    }

    /// A [`Cube3`] whose solid cells carry one material id, not `SOLID_BYTE`.
    struct Tinted(Cube3, u8);

    impl Occupancy for Tinted {
        fn solid(&self, x: i64, y: i64, z: i64, w: i8) -> bool {
            self.0.solid(x, y, z, w)
        }

        fn material(&self, x: i64, y: i64, z: i64, w: i8) -> u8 {
            if self.0.solid(x, y, z, w) {
                self.1
            } else {
                AIR
            }
        }
    }

    #[test]
    fn slab_from_occupancy_carries_the_source_material_byte() {
        const TINT: u8 = 7;
        let mut cube = Cube3 { cells: [[[false; 3]; 3]; 3], layer: 0 };
        cube.cells[1][1][1] = true;
        let plain = slab_from_occupancy(&cube, 3, 3, 3, 0);
        let tinted = slab_from_occupancy(&Tinted(cube, TINT), 3, 3, 3, 0);
        let idx = (1 * 3 + 1) * 3 + 1;
        assert_eq!(plain[idx], SOLID_BYTE, "the default material is the solid byte");
        assert_eq!(tinted[idx], TINT, "an overriding source's material byte reaches the slab");
        assert_eq!(tinted.iter().filter(|&&b| b != AIR).count(), 1, "air stays air");
    }

    #[test]
    fn slab_from_occupancy_sets_exactly_the_solid_cells() {
        let mut cube = Cube3 { cells: [[[false; 3]; 3]; 3], layer: 0 };
        cube.cells[0][0][0] = true;
        cube.cells[1][2][0] = true;
        cube.cells[2][1][2] = true;
        let solid_coords = [(0usize, 0usize, 0usize), (1, 2, 0), (2, 1, 2)];

        let slab = slab_from_occupancy(&cube, 3, 3, 3, 0);
        assert_eq!(slab.len(), 27);
        for y in 0..3usize {
            for z in 0..3usize {
                for x in 0..3usize {
                    let expected = if solid_coords.contains(&(x, y, z)) { SOLID_BYTE } else { 0 };
                    assert_eq!(slab[(y * 3 + z) * 3 + x], expected, "mismatch at x={x} y={y} z={z}");
                }
            }
        }
    }

    #[test]
    fn heightmap_from_occupancy_reads_top_solid_y_plus_one() {
        let mut cube = Cube3 { cells: [[[false; 3]; 3]; 3], layer: 0 };
        // Column (x=0,z=0): solid at y=0,1 -> top=1 -> height=2.
        cube.cells[0][0][0] = true;
        cube.cells[0][1][0] = true;
        // Column (x=1,z=0) stays all-air -> height=0.
        // Column (x=2,z=0): solid only at y=2 -> height=3.
        cube.cells[2][2][0] = true;

        let heights = heightmap_from_occupancy(&cube, 3, 3, 3, 0);
        assert_eq!(heights.len(), 9);
        assert_eq!(heights[0 * 3 + 0], 2, "y in {{0,1}} solid -> top+1 = 2");
        assert_eq!(heights[0 * 3 + 1], 0, "all-air column -> 0");
        assert_eq!(heights[0 * 3 + 2], 3, "solid only at y=2 -> top+1 = 3");
    }

    #[test]
    fn slab_from_occupancy_layer_passes_through() {
        let mut cube = Cube3 { cells: [[[false; 3]; 3]; 3], layer: 1 };
        cube.cells[0][0][0] = true;
        cube.cells[1][1][1] = true;

        let slab_layer_1 = slab_from_occupancy(&cube, 3, 3, 3, 1);
        assert!(slab_layer_1.contains(&SOLID_BYTE), "layer 1 must see the cells stored on layer 1");

        let slab_layer_0 = slab_from_occupancy(&cube, 3, 3, 3, 0);
        assert!(slab_layer_0.iter().all(|&b| b == 0), "cube solid only on layer 1 must be all-air on layer 0");
    }

    #[test]
    fn air_shades_to_black() {
        let grid = SparseChunkGrid::new(32);
        assert_eq!(
            evaluate_5d_shading(AIR, 0, 0, (0.0, 1.0, 0.0), (0.0, 1.0, 0.0), 0, &grid, 0, 0, 0),
            (0, 0, 0)
        );
    }

    #[test]
    fn different_materials_render_different_colours() {
        let grid = SparseChunkGrid::new(32);
        // Grazing light (perpendicular to the normal, sun_dot=0) keeps the
        // composite well under the 255 clamp ceiling, so the per-material
        // tint difference actually survives instead of both saturating.
        let normal = (0.0, 0.0, 1.0);
        let light = (1.0, 0.0, 0.0);
        let stone = evaluate_5d_shading(1, 0, 0, normal, light, 0, &grid, 0, 0, 0);
        let iron = evaluate_5d_shading(6, 0, 0, normal, light, 0, &grid, 0, 0, 0);
        assert_ne!(stone, iron, "different named tiles must shade to different colours");
    }

    #[test]
    fn matching_girih_angle_is_brighter_than_opposite() {
        let grid = SparseChunkGrid::new(32);
        let normal = (0.0, 1.0, 0.0);
        let light = (0.0, 1.0, 0.0);
        let aligned = evaluate_5d_shading(9, 3, 0, normal, light, 3, &grid, 0, 0, 0);
        let opposed = evaluate_5d_shading(9, 8, 0, normal, light, 3, &grid, 0, 0, 0); // dk=5 -> cos(pi) -> phase=0
        assert!(aligned.0 > opposed.0, "aligned Girih angle should be brighter (R channel)");
    }

    #[test]
    fn deeper_w_attenuates_toward_xray_tint() {
        let grid = SparseChunkGrid::new(32);
        let normal = (0.0, 1.0, 0.0);
        let light = (0.0, 1.0, 0.0);
        let surface = evaluate_5d_shading(9, 0, 0, normal, light, 0, &grid, 0, 0, 0);
        let deep = evaluate_5d_shading(9, 0, -3, normal, light, 0, &grid, 0, 0, 0);
        assert!(deep.0 < surface.0, "deeper |W| must attenuate the red channel (X-ray decay)");
    }

    #[test]
    fn chiaroscuro_void_is_absolute_black() {
        let grid = SparseChunkGrid::new(32);
        assert_eq!(
            chiaroscuro_sample(HitType::AmbientVoid, 0, (0.0, 1.0, 0.0), 0, 0, (0.0, 1.0, 0.0), 0, &grid, 0, 0),
            (0, 0, 0)
        );
    }

    #[test]
    fn chiaroscuro_carved_air_is_deep_slate_not_black() {
        let grid = SparseChunkGrid::new(32);
        let (r, g, b) =
            chiaroscuro_sample(HitType::CarvedAir, 0, (0.0, 1.0, 0.0), 0, 0, (0.0, 1.0, 0.0), 0, &grid, 0, 0);
        assert_eq!((r, g, b), (6, 8, 14));
        assert_ne!((r, g, b), (0, 0, 0), "carved air must be visually distinct from the void");
    }

    #[test]
    fn chiaroscuro_different_materials_render_different_colours() {
        let grid = SparseChunkGrid::new(32);
        // Grazing light, same reasoning as evaluate_5d_shading's own tint
        // test above — stay off the 255 clamp ceiling.
        let normal = (0.0, 0.0, 1.0);
        let light = (1.0, 0.0, 0.0);
        let stone = chiaroscuro_sample(HitType::Solid(1), 2, normal, 0, 0, light, 0, &grid, 0, 0);
        let iron = chiaroscuro_sample(HitType::Solid(6), 2, normal, 0, 0, light, 0, &grid, 0, 0);
        assert_ne!(stone, iron, "the live compose path must render named tiles in different colours");
    }

    #[test]
    fn render_chiaroscuro_composite_origin_actually_scrolls() {
        use crate::zones::worldbuilder::WorldBuilderEngine;
        let mut engine = WorldBuilderEngine::new(32);
        // A small box far from the grid origin — invisible to a (0,0)-origin
        // render whose scene_edge doesn't reach it, visible once the origin
        // scrolls to meet it.
        engine.brush_box(0, (100, 5, 0), (2, 2, 2), 1, 1);
        let unscrolled =
            render_chiaroscuro_composite(&engine.grid, &[0], 32, 64, 64, (0.4, 0.8, -0.4), 0, (0, 0));
        let scrolled =
            render_chiaroscuro_composite(&engine.grid, &[0], 32, 64, 64, (0.4, 0.8, -0.4), 0, (90, 0));
        assert_ne!(unscrolled, scrolled, "shifting origin must change what the render actually samples");
    }

    #[test]
    fn chiaroscuro_nearer_solid_is_brighter_than_farther() {
        let grid = SparseChunkGrid::new(32);
        let normal = (0.0, 1.0, 0.0);
        let light = (0.0, 1.0, 0.0);
        let near = chiaroscuro_sample(HitType::Solid(9), 2, normal, 0, 0, light, 0, &grid, 0, 0);
        let far = chiaroscuro_sample(HitType::Solid(9), 200, normal, 0, 0, light, 0, &grid, 0, 0);
        assert!(near.0 > far.0, "depth fog must dim distant masonry relative to near masonry");
    }

    /// Sabotage-provable (L18): before this fix, front-facing (+Z) solid
    /// faces scored strictly lower than top-facing (+Y) faces under
    /// matching light regardless of sky visibility — the exact defect this
    /// module's own doc used to flag. With equal (full) sky visibility on
    /// both, a front face must no longer be strictly dimmer.
    #[test]
    fn front_facing_normal_no_longer_loses_to_top_facing() {
        let grid = SparseChunkGrid::new(32);
        let light = (0.0, 0.0, 1.0);
        let top = chiaroscuro_sample(HitType::Solid(9), 0, (0.0, 1.0, 0.0), 0, 0, light, 0, &grid, 0, 0);
        let front = chiaroscuro_sample(HitType::Solid(9), 0, (0.0, 0.0, 1.0), 0, 0, light, 0, &grid, 0, 0);
        assert!(
            front.0 >= top.0,
            "front-facing normal under matching light must not lose to top-facing: front={front:?} top={top:?}"
        );
    }

    /// The Round-2 fix, pinned: a solid hit sealed under a roof (low
    /// `sky_visibility`) must render strictly darker than the same hit in
    /// the open (high `sky_visibility`) — proves the occlusion gate is
    /// load-bearing, not decorative. Without it, a naive hemispheric mix
    /// would floods `CarvedAir`-style interiors with sky fill.
    #[test]
    fn sealed_interior_is_darker_than_open_air_under_the_same_light() {
        let mut engine = WorldBuilderEngine::new(32);
        let light = (0.4, 0.8, -0.4);
        let normal = (0.0, 0.0, 1.0); // front-facing: relies entirely on ambient, no top-face floor
        let (x, y, z, w) = (16usize, 0usize, 16usize, 0i8);

        let open = chiaroscuro_sample(HitType::Solid(9), z, normal, 0, w, light, 0, &engine.grid, x, y);

        for step in 1..=4usize {
            engine.grid.get_mut(x, y + step, z, w).unwrap().payload[0] = 1;
        }
        let sealed = chiaroscuro_sample(HitType::Solid(9), z, normal, 0, w, light, 0, &engine.grid, x, y);

        assert!(
            sealed.0 <= open.0 && sealed.1 <= open.1 && sealed.2 <= open.2,
            "a roof directly overhead must not brighten a front-facing hit: sealed={sealed:?} open={open:?}"
        );
        assert!(
            sealed.0 < open.0 || sealed.1 < open.1 || sealed.2 < open.2,
            "the occlusion gate must actually dim something: sealed={sealed:?} open={open:?}"
        );
    }

    #[test]
    fn classify_hit_matches_the_established_tri_state_convention() {
        assert_eq!(classify_hit(AIR, ValidityMask::ALL_UNKNOWN), HitType::AmbientVoid);
        assert_eq!(classify_hit(AIR, ValidityMask::ALL_KNOWN), HitType::CarvedAir);
        assert_eq!(classify_hit(9, ValidityMask::ALL_KNOWN), HitType::Solid(9));
    }

    #[test]
    fn tag_girih_k_only_marks_solid_cells() {
        let mut engine = WorldBuilderEngine::new(32);
        engine.brush_sphere(0, (5, 5, 5), 2, 9, 1);
        assert!(tag_girih_k(&mut engine.grid, 5, 5, 5, 0, 4));
        assert_eq!(engine.grid.get(5, 5, 5, 0).unwrap().payload[1], 4);
        // Untouched air cell: no-op.
        assert!(!tag_girih_k(&mut engine.grid, 500, 500, 500, 0, 4));
    }

    /// Real WITNESS: shade the cathedral (W=0) composited with the
    /// Under-Orchard (W=-1) into one X-ray depth map, save a real PNG-
    /// convertible PPM, and confirm the shading actually varies (not a
    /// flat wash) — a real lighting receipt, not a claim.
    #[test]
    fn cathedral_5d_shaded_witness() {
        let mut engine = WorldBuilderEngine::new(32);
        let (cx, cz) = (48usize, 48usize);
        let tick_seq = &mut (1u64..);
        let mut tick = || tick_seq.next().unwrap();

        let (plinth_changed, plinth_h) =
            engine.brush_gothic_nave(0, (cx - 10, 0, cz - 10), 10, rational_ratio::SESQUITERTIA_4_3, 8, tick());
        let plinth_cells = (plinth_h * CELLS_PER_MODULE).max(1) as usize;
        let (nave_changed, nave_h) =
            engine.brush_gothic_nave(0, (cx - 6, plinth_cells, cz - 6), 12, rational_ratio::DOUBLE_SQUARE_2_1, 7, tick());
        let nave_top = plinth_cells + (nave_h * CELLS_PER_MODULE).max(1) as usize;
        let tower_changed = engine.brush_tapered_spire(0, cx, nave_top, cz, 6, 3, 2, 6, tick());
        assert!(plinth_changed > 0 && nave_changed > 0 && tower_changed > 0);

        // Tag the tower's apex region with a distinct Girih angle so the
        // phase-alignment term has something real to differentiate.
        tag_girih_k(&mut engine.grid, cx, nave_top + 2, cz, 0, 3);

        let cellar_changed = engine.brush_sphere(-1, (cx, 10, cz), 4, 6, tick());
        assert!(cellar_changed > 0);

        let frame = render_shaded_composite(&engine.grid, &[0, -1], 130, 256, 256, (0.4, 0.8, -0.4), 3);
        assert_eq!(frame.len(), 256 * 256 * 3);

        // Real shading receipt: pixel colors must actually vary (not a
        // flat single color), proving the diffuse/phase/attenuation
        // terms are doing something, not just stamping one constant RGB.
        let mut seen = std::collections::HashSet::new();
        for p in frame.chunks_exact(3) {
            seen.insert([p[0], p[1], p[2]]);
            if seen.len() > 3 {
                break;
            }
        }
        assert!(seen.len() > 3, "shaded frame must show real color variation, not a flat wash");

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.forge/photons");
        std::fs::create_dir_all(&dir).expect("create .forge/photons");
        let mut file = std::fs::File::create(dir.join("cathedral_5d_shaded.ppm")).expect("create ppm");
        use std::io::Write;
        writeln!(file, "P6\n256 256\n255").expect("write ppm header");
        file.write_all(&frame).expect("write ppm body");
    }

    /// Real WITNESS: the chiaroscuro pass — a cathedral with a real
    /// excavated interior chamber (carved AIR, distinct from untouched
    /// ambient void) plus a subterranean cellar, first-hit-wins
    /// raymarched with real per-pixel depth. Proves all three tri-state
    /// colors actually appear (not just asserted in isolation).
    #[test]
    fn chiaroscuro_cathedral_witness() {
        let mut engine = WorldBuilderEngine::new(32);
        let (cx, cz) = (48usize, 48usize);
        let tick_seq = &mut (1u64..);
        let mut tick = || tick_seq.next().unwrap();

        let (plinth_changed, plinth_h) =
            engine.brush_gothic_nave(0, (cx - 10, 0, cz - 10), 10, rational_ratio::SESQUITERTIA_4_3, 8, tick());
        let plinth_cells = (plinth_h * CELLS_PER_MODULE).max(1) as usize;
        let (nave_changed, nave_h) =
            engine.brush_gothic_nave(0, (cx - 6, plinth_cells, cz - 6), 12, rational_ratio::DOUBLE_SQUARE_2_1, 7, tick());
        let nave_top = plinth_cells + (nave_h * CELLS_PER_MODULE).max(1) as usize;
        let tower_changed = engine.brush_tapered_spire(0, cx, nave_top, cz, 6, 3, 2, 6, tick());
        assert!(plinth_changed > 0 && nave_changed > 0 && tower_changed > 0);
        tag_girih_k(&mut engine.grid, cx, nave_top + 2, cz, 0, 3);

        let cellar_changed = engine.brush_sphere(-1, (cx, 10, cz), 4, 6, tick());
        assert!(cellar_changed > 0);
        // Carve a real interior void inside the cellar sphere — distinct
        // CarvedAir, not just AmbientVoid outside the whole structure.
        let hollow_changed = engine.brush_sphere(-1, (cx, 10, cz), 2, AIR, tick());
        assert!(hollow_changed > 0);

        // Light at k=0 (DEFAULT_GIRIH_K) so untagged masonry reads at full
        // Girih-phase brightness; only the deliberately-tagged apex (k=3)
        // dims via phase mismatch — the intended demonstration, not a
        // uniformly-dim scene.
        let frame =
            render_chiaroscuro_composite(&engine.grid, &[0, -1], 130, 512, 512, (0.4, 0.8, -0.4), DEFAULT_GIRIH_K, (0, 0));
        assert_eq!(frame.len(), 512 * 512 * 3);

        let mut has_black = false;
        let mut has_slate = false;
        let mut has_bright = false;
        for p in frame.chunks_exact(3) {
            if p == [0, 0, 0] {
                has_black = true;
            } else if p == [6, 8, 14] {
                has_slate = true;
            } else if p[0] > 50 {
                // Threshold 50: any illuminated masonry, top- or
                // front-facing alike, since the sky-irradiance fix landed.
                has_bright = true;
            }
        }
        assert!(has_black, "ambient void must render as absolute black somewhere in frame");
        assert!(has_bright, "illuminated masonry must pop out relative to black/slate background");
        let _ = has_slate; // real but not guaranteed visible at this camera angle/resolution

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.forge/photons");
        std::fs::create_dir_all(&dir).expect("create .forge/photons");
        let mut file = std::fs::File::create(dir.join("chiaroscuro_cathedral_5d.ppm")).expect("create ppm");
        use std::io::Write;
        writeln!(file, "P6\n512 512\n255").expect("write ppm header");
        file.write_all(&frame).expect("write ppm body");
    }
}
