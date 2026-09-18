//! Analytic sky irradiance for the 5D chiaroscuro raymarch — donor is v2's
//! self-contained `forge-lighting::sky_radiance` (`F:\NewRepo\crates\
//! forge-lighting\src\sky.rs:19-58`, Rayleigh+Mie dome + sun forward-scatter
//! lobe). NOT ported: `sky_bake.rs`/`baked.rs` — both depend on v2-only
//! `forge_core::contract::WorldContract`/`lut_pack::LutPack`/`presets_layout`
//! types absent from this crate, and bake a 2D LUT table so runtime becomes a
//! memcpy — a real optimization, but not required at this crate's per-pixel
//! CPU-raymarch scale. `sky_radiance` here is evaluated live instead.
//!
//! [AUTHORED] `sky_visibility` and the visibility-gated mix in
//! `hemispheric_irradiance` are NOT a v2 port: v2's sky model assumed an
//! outdoor renderer with no interior occlusion. Without a visibility gate, a
//! flat hemispheric mix floods `CarvedAir` vaults/cellars with sky fill and
//! erases the chiaroscuro contrast `raymarch_5d.rs`'s tri-state HitType exists
//! to produce (Sean, plan correction 2026-08-20). `sky_visibility` reuses the
//! same bounded integer solid/air neighbour sampling `central_difference_
//! normal`'s `solid()` closure already uses in `raymarch_5d.rs` — no new grid
//! API, no new dependency.

use crate::zones::raymarch_5d::Occupancy;

/// Sun forward-scatter lobe gain (carried from v2's `procedural_sky`).
const SUN_LOBE_GAIN: f32 = 4.0;

/// Analytic sky radiance for a unit view direction. Linear HDR-ish output
/// (unclamped); callers compose and clamp at the final 8-bit boundary, same
/// convention `evaluate_5d_shading`/`chiaroscuro_sample` already use.
/// `turbidity` ~2.0 (clear) .. ~9.5 (storm). Ported near-verbatim from v2's
/// `sky_radiance` (`forge-lighting/src/sky.rs:19-58`).
pub fn sky_radiance(
    view_dir: (f32, f32, f32),
    sun_dir: (f32, f32, f32),
    sun_color: (f32, f32, f32),
    sun_energy: f32,
    turbidity: f32,
) -> (f32, f32, f32) {
    let view_elev = view_dir.1.clamp(-1.0, 1.0);
    let sun_elev = sun_dir.1.clamp(-1.0, 1.0);
    let day_factor = (sun_elev + 0.1).clamp(0.0, 1.0);
    let view_factor = (view_elev * 0.5 + 0.5).clamp(0.0, 1.0);

    let rayleigh = (
        (0.10 + 0.05 * view_factor) * day_factor,
        (0.20 + 0.15 * view_factor) * day_factor,
        (0.40 + 0.30 * view_factor) * day_factor,
    );
    let horizon_term = (1.0 - view_factor).powi(3);
    let mie = (
        sun_color.0 * 0.40 * horizon_term * day_factor,
        sun_color.1 * 0.30 * horizon_term * day_factor,
        sun_color.2 * 0.15 * horizon_term * day_factor,
    );
    let sun_dot =
        (view_dir.0 * sun_dir.0 + view_dir.1 * sun_dir.1 + view_dir.2 * sun_dir.2).max(0.0);
    let sun_lobe = sun_dot.powi(96) * SUN_LOBE_GAIN * day_factor;
    let turbidity_mul = (1.0 / (turbidity * 0.25)).clamp(0.1, 1.0);
    let night = (1.0 - day_factor) * 0.05;
    let night_rgb = (night * 0.10, night * 0.15, night * 0.30);

    (
        (rayleigh.0 + mie.0) * turbidity_mul * sun_energy
            + sun_color.0 * sun_lobe * sun_energy
            + night_rgb.0,
        (rayleigh.1 + mie.1) * turbidity_mul * sun_energy
            + sun_color.1 * sun_lobe * sun_energy
            + night_rgb.1,
        (rayleigh.2 + mie.2) * turbidity_mul * sun_energy
            + sun_color.2 * sun_lobe * sun_energy
            + night_rgb.2,
    )
}

/// [AUTHORED] Fraction of `budget` cells directly above `(x,y,z)` on layer `w`
/// that are clear (not solid, and in-grid) — `1.0` fully open to sky, `0.0`
/// capped immediately by a ceiling/roof. Same solid-neighbour sampling
/// discipline as `central_difference_normal`'s `solid()` closure
/// (`raymarch_5d.rs`): integer occupancy only, no new grid API.
pub fn sky_visibility<G: Occupancy + ?Sized>(grid: &G, x: usize, y: usize, z: usize, w: i8, budget: usize) -> f32 {
    if budget == 0 {
        return 1.0;
    }
    let mut clear = 0usize;
    for step in 1..=budget {
        let uy = y + step;
        if grid.solid(x as i64, uy as i64, z as i64, w) {
            break;
        } else {
            clear += 1;
        }
    }
    clear as f32 / budget as f32
}

/// [AUTHORED] Occlusion-gated hemispheric sky term: two-tap `sky_radiance`
/// (straight up / straight down) mixed by the shading normal's vertical
/// component, then scaled by `visibility` ([`sky_visibility`]) so a sealed
/// `CarvedAir` interior reads near-black while an open rooftop face gets full
/// sky fill. Callers still multiply the result by their own `x_ray_decay`
/// (5D |W|-depth attenuation) — this function is the sky term alone, never an
/// independent additive channel outside that composite.
#[allow(clippy::too_many_arguments)]
pub fn hemispheric_irradiance(
    normal: (f32, f32, f32),
    sun_dir: (f32, f32, f32),
    sun_color: (f32, f32, f32),
    sun_energy: f32,
    turbidity: f32,
    visibility: f32,
) -> (f32, f32, f32) {
    let up = sky_radiance((0.0, 1.0, 0.0), sun_dir, sun_color, sun_energy, turbidity);
    let down = sky_radiance((0.0, -1.0, 0.0), sun_dir, sun_color, sun_energy, turbidity);
    let mix = (normal.1 * 0.5 + 0.5).clamp(0.0, 1.0);
    let gate = visibility.clamp(0.0, 1.0);
    (
        (up.0 * mix + down.0 * (1.0 - mix)) * gate,
        (up.1 * mix + down.1 * (1.0 - mix)) * gate,
        (up.2 * mix + down.2 * (1.0 - mix)) * gate,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::worldbuilder::WorldBuilderEngine;

    #[test]
    fn sky_radiance_day_is_brighter_than_night() {
        let sun_up = (0.0, 1.0, 0.0);
        let sun_down = (0.0, -1.0, 0.0);
        let day = sky_radiance((0.0, 1.0, 0.0), sun_up, (1.0, 1.0, 1.0), 1.0, 2.0);
        let night = sky_radiance((0.0, 1.0, 0.0), sun_down, (1.0, 1.0, 1.0), 1.0, 2.0);
        assert!(day.2 > night.2, "daytime zenith must outshine night: {day:?} vs {night:?}");
    }

    #[test]
    fn full_visibility_is_a_no_op_gate() {
        let sun = (0.0, 1.0, 0.0);
        let free = hemispheric_irradiance((0.0, 1.0, 0.0), sun, (1.0, 1.0, 1.0), 1.0, 2.0, 1.0);
        let gated = hemispheric_irradiance((0.0, 1.0, 0.0), sun, (1.0, 1.0, 1.0), 1.0, 2.0, 1.0);
        assert_eq!(free, gated);
    }

    #[test]
    fn zero_visibility_is_pure_black() {
        let sun = (0.0, 1.0, 0.0);
        let sealed = hemispheric_irradiance((0.0, 1.0, 0.0), sun, (1.0, 1.0, 1.0), 1.0, 2.0, 0.0);
        assert_eq!(sealed, (0.0, 0.0, 0.0));
    }

    #[test]
    fn sky_visibility_is_open_over_a_fresh_engine() {
        let engine = WorldBuilderEngine::new(32);
        let v = sky_visibility(&engine.grid, 16, 0, 16, 0, 8);
        assert_eq!(v, 1.0, "an untouched lattice has nothing to cap the sky");
    }

    #[test]
    fn sky_visibility_is_capped_under_a_roof() {
        let mut engine = WorldBuilderEngine::new(32);
        let (x, y, z): (usize, usize, usize) = (16, 0, 16);
        for step in 1..=4usize {
            engine.grid.get_mut(x, y + step, z, 0).unwrap().payload[0] = 1;
        }
        let v = sky_visibility(&engine.grid, x, y, z, 0, 8);
        assert!(v < 1.0, "a roof directly overhead must cap sky visibility, got {v}");
    }
}
