//! Vixel automata — cellular rules on VixelAtom storage buffers.
//!
//! Fire spread, fluid flow, gravity, and sand emergence/collapse. Integer-only logic
//! with CPU-parity determinism: same function, same result, every time. The GPU compute
//! kernel reads/writes VixelAtom[] in-place; this module provides the CPU-side proof
//! function that ensures bit-identical behaviour across platforms.
//!
//! # Drain
//!
//! Ported from v2 `F:\NewRepo\crates\forge-shaders\src\vixel_automata.rs` (git tag v0.2).
//! This CPU port is the TRUTH function — a WGSL compute twin will later be proven
//! against it word-for-word. No HashMap iteration order, no platform RNG, no wall-clock:
//! determinism is the whole point.

/// A single vixel atom — the fundamental renderable unit.
///
/// 1 vixel = 1 flat pixel = 1 forgeAtom. Integer-only on CPU; float at GPU boundary.
/// 28 bytes (pos_x, pos_y, pos_z, material, opacity, size, flags). All fields i32/u32
/// for SPIR-V compatibility (no Int16 capability needed). CPU packs from u16 sources;
/// GPU reads directly.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct VixelAtom {
    /// Screen-space position (MilliUnit i32). Integer until projection.
    pub pos_x: i32,
    /// Screen-space position (MilliUnit i32). Integer until projection.
    pub pos_y: i32,
    /// 3D depth, MilliUnit (1000 = 1 vixel). `z = 0` is the 2D sprite plane
    /// ("a 2D sprite is just z=0"); the 32^3 StructuralBox spans z in [0, 32_000).
    pub pos_z: i32,
    /// Material palette ID (indexes GpuMaterialEntry[]).
    pub material: u32,
    /// Opacity in permyriad (0=transparent, 10000=opaque).
    pub opacity: u32,
    /// Splat radius in sub-pixels (permyriad of a pixel).
    pub size: u32,
    /// Bit flags: bit0=alive, bit1=lit, bit2=fluid, bit3=flammable, bit4=burning, bit5=ui.
    pub flags: u32,
}

const _: () = assert!(core::mem::size_of::<VixelAtom>() == 28);
const _: () = assert!(core::mem::align_of::<VixelAtom>() == 4);

/// Atom is present and has geometry/state. Fire, gravity, and fluid rules key off this.
pub const FLAG_ALIVE: u32 = 0x01;
/// Atom is emitting light (lit or burning).
pub const FLAG_LIT: u32 = 0x02;
/// Atom flows under fluid-flow rules instead of gravity.
pub const FLAG_FLUID: u32 = 0x04;
/// Atom will catch fire if adjacent to a burning atom.
pub const FLAG_FLAMMABLE: u32 = 0x08;
/// Atom is actively on fire.
pub const FLAG_BURNING: u32 = 0x10;
/// Structural UI atom — a vixel that belongs to a UI element. It DEFIES gravity
/// (the settle loop skips it) until heat melts it: clearing `FLAG_UI` and setting
/// `FLAG_FLUID` lets the atom fall/flow into the world. This single bit is what
/// makes "UI = World = one atom" true.
pub const FLAG_UI: u32 = 0x20;

/// Empty-cell sentinel for the GPU material-id chunk (`[u32; 32768]` — the
/// widened upload of `[MaterialId; 32768]`). 0 = air, nonzero = a grain; matches
/// `forge_vix::chunk_bake::AIR`. `u32` not `u16`: SPIR-V requires `OpCapability
/// Int16` for `u16`, and the storage buffers are `u32` (mirrors `bake_ao_compute`).
pub const AIR_ID: u32 = 0;

/// Check if an atom has a specific flag.
#[inline]
pub fn has_flag(atom: &VixelAtom, flag: u32) -> bool {
    atom.flags & flag != 0
}

/// Ignition rule: flammable + adjacent burning → start burning.
///
/// Returns new flags for the target atom. If the target is already burning or not
/// flammable, returns its flags unchanged. Otherwise, if any neighbor is burning,
/// sets both FLAG_BURNING and FLAG_LIT on the target.
#[inline]
pub fn rule_ignite(target: &VixelAtom, neighbors: &[VixelAtom]) -> u32 {
    if !has_flag(target, FLAG_FLAMMABLE) || has_flag(target, FLAG_BURNING) {
        return target.flags;
    }
    let fire_adjacent = neighbors.iter().any(|n| has_flag(n, FLAG_BURNING));
    if fire_adjacent {
        target.flags | FLAG_BURNING | FLAG_LIT
    } else {
        target.flags
    }
}

/// Gravity rule: unsupported atom falls (pos_y += 1000 MilliUnit = 1 pixel).
///
/// Returns new pos_y. `below_empty`: true if the cell below has no alive atom.
/// Fluid atoms are excluded (they use flow rule instead of gravity).
#[inline]
pub fn rule_gravity(atom: &VixelAtom, below_empty: bool) -> i32 {
    if !has_flag(atom, FLAG_ALIVE) || has_flag(atom, FLAG_FLUID) {
        return atom.pos_y; // fluid uses flow rule, not gravity
    }
    if below_empty {
        atom.pos_y + 1000 // fall 1 pixel (1000 MilliUnit)
    } else {
        atom.pos_y
    }
}

/// Fluid flow rule: fluid seeks lowest empty neighbor.
///
/// `neighbors_y`: y-positions of the 4 cardinal neighbors (i32::MAX = no neighbor/occupied).
/// Returns the index (0..3) of the best flow target, or None if nowhere to go.
/// Fluid prefers to flow down (higher y in screen coords).
#[inline]
pub fn rule_fluid_flow(atom: &VixelAtom, neighbors_y: [i32; 4]) -> Option<usize> {
    if !has_flag(atom, FLAG_FLUID) || !has_flag(atom, FLAG_ALIVE) {
        return None;
    }
    // Find lowest empty neighbor (highest y in screen coords)
    let mut best_idx = None;
    let mut best_y = atom.pos_y;
    for (i, &ny) in neighbors_y.iter().enumerate() {
        if ny != i32::MAX && ny > best_y {
            best_y = ny;
            best_idx = Some(i);
        }
    }
    best_idx
}

/// Step one atom through all rules. Returns (new_flags, new_pos_y).
///
/// This is the CPU-side parity function; the compute shader does the same logic.
/// Applies ignition first (spreading fire), then gravity/flow (moving atoms).
#[inline]
pub fn step_atom(
    atom: &VixelAtom,
    neighbors: &[VixelAtom],
    below_empty: bool,
) -> (u32, i32) {
    let flags = rule_ignite(atom, neighbors);
    let pos_y = rule_gravity(&VixelAtom { flags, ..*atom }, below_empty);
    (flags, pos_y)
}

/// Sand-emerge rule — Piece 2's CPU-parity logic (the GPU compute lane).
///
/// Warm sand grains spring up from the floor (y=0) and lock into the baked
/// sand-stencil: the panel macro-structure (border/title-bar/controls) baked by
/// `forge_vix::chunk_bake` into a `[MaterialId; 32768]` chunk. Bottom-up,
/// race-free GATHER — reads only this cell + the column below, so it is safe for
/// a double-buffered compute dispatch (no read/write hazard). A cell fills with
/// its stencil material once the cell beneath it has risen (or it sits on the
/// floor); cells outside the stencil never fill (and stray grains there clear),
/// so the emerged sand traces EXACTLY the authored panel.
///
/// # Arguments
/// - `stencil`: target MaterialId for this cell (`AIR_ID` = outside the panel).
/// - `here`: current state (`AIR_ID` = empty, else an already-locked grain).
/// - `below_locked`: the cell below is filled, or this is the floor row.
///
/// # Returns
/// The cell's next MaterialId. The HUD "lock count" = cells where the return is
/// nonzero; full convergence = every stencil cell locked (Idle beat).
#[inline]
pub fn sand_emerge_cell(stencil: u32, here: u32, below_locked: bool) -> u32 {
    if stencil == AIR_ID {
        return AIR_ID; // outside the panel structure — never fills; clears strays
    }
    if here == stencil {
        return stencil; // already locked in place
    }
    if below_locked {
        stencil // the column beneath has risen → this grain springs up and locks
    } else {
        AIR_ID // bottom-up: wait for the grain below to emerge first
    }
}

/// Deterministic per-cell scatter delay for the EXIT cascade.
///
/// Maps `(x, y, z)` → a permyriad threshold (0..=10000) using a low-quality hash.
/// No crypto needed — just uniform spread so each grain clears at a unique moment,
/// producing the staggered shatter effect across the 32³ chunk.
#[inline]
pub fn cell_scatter_pmy(x: u32, y: u32, z: u32) -> u32 {
    let mut h = x.wrapping_mul(2_654_435_761)
        ^ y.wrapping_mul(2_246_822_519)
        ^ z.wrapping_mul(3_266_489_917);
    h ^= h >> 13;
    h = h.wrapping_mul(1_540_483_477);
    h ^= h >> 15;
    h % 10_001 // 0..=10000 permyriad
}

/// EXIT cascade: stagger-clear a grain based on a global permyriad timer.
///
/// Each cell carries a unique delay threshold from `cell_scatter_pmy`. When
/// `timer_pmy` (0..=10000, driven by `VoxelFontAnimator`) exceeds the cell's
/// threshold the grain releases to AIR. CPU-parity twin of the GPU compute kernel.
///
/// The float scatter impulse (velocity + gravity) lives in the GPU kernel
/// lane as a cosmetic visual effect — NOT tracked here. This function
/// only advances the integer occupancy state.
#[inline]
pub fn sand_collapse_cell(here: u32, x: u32, y: u32, z: u32, timer_pmy: u32) -> u32 {
    if here == AIR_ID {
        return AIR_ID;
    }
    if timer_pmy >= cell_scatter_pmy(x, y, z) {
        AIR_ID
    } else {
        here
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_atom(flags: u32, x: i32, y: i32) -> VixelAtom {
        VixelAtom {
            pos_x: x,
            pos_y: y,
            pos_z: 0,
            material: 1,
            opacity: 10000,
            size: 10000,
            flags,
        }
    }

    #[test]
    fn a_flammable_atom_ignites_when_adjacent_to_fire() {
        let target = make_atom(FLAG_ALIVE | FLAG_FLAMMABLE, 0, 0);
        let fire = make_atom(FLAG_ALIVE | FLAG_BURNING, 1000, 0);
        let flags = rule_ignite(&target, &[fire]);
        assert!(flags & FLAG_BURNING != 0);
    }

    #[test]
    fn non_flammable_atoms_do_not_ignite() {
        let target = make_atom(FLAG_ALIVE, 0, 0);
        let fire = make_atom(FLAG_ALIVE | FLAG_BURNING, 1000, 0);
        let flags = rule_ignite(&target, &[fire]);
        assert!(flags & FLAG_BURNING == 0);
    }

    #[test]
    fn unsupported_atoms_fall_by_gravity() {
        let atom = make_atom(FLAG_ALIVE, 0, 5000);
        let new_y = rule_gravity(&atom, true);
        assert_eq!(new_y, 6000);
    }

    #[test]
    fn atoms_do_not_fall_when_supported() {
        let atom = make_atom(FLAG_ALIVE, 0, 5000);
        let new_y = rule_gravity(&atom, false);
        assert_eq!(new_y, 5000);
    }

    #[test]
    fn fluid_atoms_flow_to_the_lowest_neighbor() {
        let atom = make_atom(FLAG_ALIVE | FLAG_FLUID, 0, 5000);
        // neighbor below (idx 2) is empty at y=6000
        let neighbors_y = [i32::MAX, i32::MAX, 6000, i32::MAX];
        assert_eq!(rule_fluid_flow(&atom, neighbors_y), Some(2));
    }

    #[test]
    fn fluid_atoms_stay_when_all_neighbors_are_occupied() {
        let atom = make_atom(FLAG_ALIVE | FLAG_FLUID, 0, 5000);
        let neighbors_y = [i32::MAX, i32::MAX, i32::MAX, i32::MAX];
        assert_eq!(rule_fluid_flow(&atom, neighbors_y), None);
    }

    #[test]
    fn step_atom_produces_deterministic_results() {
        let atom = make_atom(FLAG_ALIVE | FLAG_FLAMMABLE, 0, 5000);
        let fire = make_atom(FLAG_ALIVE | FLAG_BURNING, 1000, 5000);
        let (f1, y1) = step_atom(&atom, &[fire], true);
        let (f2, y2) = step_atom(&atom, &[fire], true);
        assert_eq!(f1, f2);
        assert_eq!(y1, y2);
        assert!(f1 & FLAG_BURNING != 0);
        assert_eq!(y1, 6000); // fell + ignited
    }

    #[test]
    fn sand_collapse_passes_through_air_cells() {
        assert_eq!(sand_collapse_cell(AIR_ID, 0, 0, 0, 0), AIR_ID);
        assert_eq!(sand_collapse_cell(AIR_ID, 7, 3, 2, 10000), AIR_ID);
    }

    #[test]
    fn sand_collapse_clears_all_grains_when_timer_reaches_max() {
        // At timer_pmy = 10000 every threshold (0..=10000) is met.
        assert_eq!(sand_collapse_cell(7, 0, 0, 0, 10000), AIR_ID);
        assert_eq!(sand_collapse_cell(7, 5, 3, 2, 10000), AIR_ID);
        assert_eq!(sand_collapse_cell(7, 31, 31, 31, 10000), AIR_ID);
    }

    #[test]
    fn sand_collapse_scatter_thresholds_have_adequate_spread() {
        // The scatter threshold must cover at least 50% of the permyriad range so
        // the shatter effect has visible spread (not all grains fall at once).
        let thresholds: Vec<u32> = (0u32..8)
            .flat_map(|x| (0u32..8).map(move |y| cell_scatter_pmy(x, y, 0)))
            .collect();
        let min = *thresholds.iter().min().unwrap();
        let max = *thresholds.iter().max().unwrap();
        assert!(max - min > 5000, "poor scatter spread: min={min} max={max}");
    }

    #[test]
    fn sand_cells_outside_stencil_never_fill_and_clear_strays() {
        assert_eq!(sand_emerge_cell(AIR_ID, AIR_ID, true), AIR_ID);
        assert_eq!(sand_emerge_cell(AIR_ID, 7, true), AIR_ID); // stray grain → cleared
    }

    #[test]
    fn sand_cells_lock_in_place_once_filled() {
        assert_eq!(sand_emerge_cell(7, 7, false), 7);
    }

    #[test]
    fn sand_emerges_bottom_up_never_top_down() {
        assert_eq!(sand_emerge_cell(7, AIR_ID, true), 7); // floor / below risen → fills
        assert_eq!(sand_emerge_cell(7, AIR_ID, false), AIR_ID); // below empty → waits
    }

    #[test]
    fn a_sand_column_fills_bottom_up_and_converges_completely() {
        // Height-4 stencil column (all panel material 7), starts empty; floor at idx 0.
        const H: usize = 4;
        let stencil = [7u32; H];
        let step = |s: &[u32; H]| {
            let mut n = [AIR_ID; H];
            let mut y = 0;
            while y < H {
                let below_locked = y == 0 || s[y - 1] != AIR_ID;
                n[y] = sand_emerge_cell(stencil[y], s[y], below_locked);
                y += 1;
            }
            n
        };
        let mut st = [AIR_ID; H];
        st = step(&st);
        assert_eq!(st, [7, 0, 0, 0]);
        st = step(&st);
        assert_eq!(st, [7, 7, 0, 0]);
        st = step(&st);
        assert_eq!(st, [7, 7, 7, 0]);
        st = step(&st);
        assert_eq!(st, [7, 7, 7, 7]); // fully locked (Idle)
        assert_eq!(step(&st), [7, 7, 7, 7]); // idempotent once converged
        assert_eq!(st.iter().filter(|&&c| c != AIR_ID).count(), H); // lock count
    }
}
