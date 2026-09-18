//! Zone-forge — 5D world-storage organs. Wave 1 of `forge-zone-v3-5D`
//! (plan: `elegant-swinging-scone.md`). Landed: `spatial`, `ledger`,
//! `dispatch`, `storage`, `compaction`, `structural` (wire 9, ported
//! architecture-constraint solver), `blueprint` (wire 2, resolved
//! 2026-08-20 — moved here from `forge-zones-v3` under L05, which now
//! re-exports it), `project3d` (wire 3, sphere carve/fill mutation +
//! ledger audit over `PexilChunk`), `pentaract` (bounded 5D brick +
//! CPU raymarch smoke test), `raymarch` (wire 8, 5-PoV orthographic
//! witness renderer).

pub mod spatial;
pub mod ledger;
pub mod dispatch;
pub mod storage;
pub mod compaction;
/// The Blueprint authoring schema (`BlueprintGraph`/`BlueprintNode`/etc.) —
/// canonical home as of 2026-08-20 (moved from `forge-zones-v3` under L05
/// one-home; that crate now re-exports this module for its downstream
/// consumers, `ZoneState`/`svg`/`spiral`/`word` stay there, deliberately
/// f64-walled per ARCH000 precedent, unrelated to the 5D lattice).
pub mod blueprint;
/// Ported architecture-constraint solver (wire 9): Ad Quadratum/Ad
/// Triangulum ratio resolution, octagon/hub/buttress validation. Donor:
/// `F:\NewRepo\crates\forge-tile-crawler\src\architecture\`.
pub mod structural;
/// Sphere carve/fill mutation over `PexilChunk`, audited into
/// `MutationLedger` (wire 3). Algorithm donor: `forge-canvas-v3/src/
/// sphere_brush.rs`.
pub mod project3d;
/// Bounded dense 5D hyper-brick (`FlatPentaract`), slicing to the 3D
/// `PexilChunk`/raymarch path — a local brick, not world storage.
pub mod pentaract;
/// 5-PoV orthographic raymarcher over `PexilChunk` (wire 8, WITNESS).
pub mod raymarch;
/// `WorldBuilderEngine` — named scenes (Thornbell Parish/Bell Pit/
/// Under-Orchard) over dual-layer `PexilChunk`s, wiring the brushes
/// above together.
pub mod worldbuilder;
/// Sparse vertical Y-chunking (`VerticalColumn`) — stacked `PexilChunk`s
/// allocated lazily, proving a 1.6km sky ceiling (`Y=500` modules)
/// resolves without a monolithic dense allocation.
pub mod vertical_column;
/// Full 3D sparse chunk grid (`SparseChunkGrid`) — supersedes
/// `vertical_column` with real X/Y/Z chunking, not just Y. `raymarch`
/// samples this directly via `render_frame_sparse`, never allocating a
/// monolithic chunk to render a sparse scene.
pub mod sparse_grid;
/// Analytic sky irradiance (Rayleigh+Mie dome, ported from v2's self-
/// contained `forge-lighting::sky_radiance`) plus an occlusion-gated
/// hemispheric ambient term for `raymarch_5d`'s chiaroscuro composite.
/// Zero-dependency, same Crate Zero law as the rest of this crate.
pub mod sky_irradiance;
/// 5D photometric shading (Girih phase + X-ray |W| attenuation) over the
/// sparse grid — extends `raymarch`'s flat tri-state color with a real
/// lighting composite. `k`/`w` come from `Pexil.payload[1]`/the real
/// layer `i8`, never `TritCell5D` (no such accessors exist there).
pub mod raymarch_5d;
/// Ulam5D — deterministic O(1) space-filling layout generator, extending
/// the real, landed `UlamSpiral3D` (`forge-zones-v3/src/spiral.rs`,
/// file-mounted here via `#[path]`, same mechanism xtask already uses)
/// with two more stacked axes (Girih angle, world layer).
pub mod ulam5d;
