//! Architecture-constraint kernel, ported from `forge-tile-crawler`
//! (`F:\NewRepo\crates\forge-tile-crawler\src\architecture\`). Deterministic
//! constructive-geometry ratios, socket/octagon/hub validation, and the
//! central-third buttress solver — the concept and the working math, not a
//! new ASP/Clingo engine (Sean, 2026-08-20: "its my invention").

/// Named constructive ratios (Ad Quadratum/Ad Triangulum) and their resolvers.
pub mod ratio;
/// Which axis role a ratio may serve (`AxisRole`), and the policy table.
pub mod ratio_policy;
/// `GeometrySystem`/`ConnectionRole` — the vocabulary structural checks read.
pub mod semantic;
/// `PrimitiveId`/`ArchPrimitiveDef` — the catalog validation checks against.
pub mod catalog;
/// Fail-closed structural verdicts: octagon closure, hub piers, middle-third.
pub mod validate;
/// The central-third buttress solver: scale until thrust is contained.
pub mod buttress;
