//! World Consequence Engine (WCE) — ported from `F:\NewRepo\crates\forge-consequence`.
//!
//! Port 2026-08-17: `budget` and `curves` need no external deps (only
//! `query`+`tags`) and live here in Crate Zero. `quest`, `rule`, `moe`, and
//! `dispatch` all landed in the sibling crate `forge-consequence-v3`, which
//! depends on this module (`serde` for quest/rule, `forge-hal-clockspine`
//! for moe, `forge-physics-v3` for dispatch's `PhysicsEffect` mapping — see
//! `forge-consequence-v3/Cargo.toml` for the full per-module dependency
//! breakdown). The full v2 `forge-consequence` source is now ported; no
//! WCE module remains a named blocker.

pub mod allostatic_curve;
pub mod budget;
pub mod curves;
pub mod query;
pub mod tags;
pub mod terrain;
