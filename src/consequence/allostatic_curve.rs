//! AllostaticCurve — decay, persistence, learning-progression primitive.
//! Ported verbatim from F:\NewRepo\crates\forge-core\src\spine\allostatic_curve.rs
//! (2026-08-31, sha 2E60F9DB; serde derives feature-gated to match this crate).
//! Decay family (book ch.29): this DESCRIBES; decay.rs EXECUTES; forge-audio vested_decay REMEMBERS.

/// Decay or persistence curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AllostaticCurve {
    /// Linear drift of `slope_q8` per tick (Permyriad/256 per tick), signed: negative decays.
    Linear {
        /// Signed delta per tick, Permyriad/256; negative decays, the value clamps at 0.
        slope_q8: i32,
    },
    /// Exponential decay with the given half-life in ticks.
    Exponential {
        /// Half-life in ticks.
        half_life_ticks: u32,
    },
    /// Holds at full value until `drop_at_tick`, then zero.
    Threshold {
        /// Tick at which the value drops to zero.
        drop_at_tick: u64,
    },
    /// Never decays.
    Permanent,
    /// Six-stage literacy arc — player learns the meaning over many exposures.
    LiteracyArc {
        /// The six stages in progression order.
        stages: [LiteracyStage; 6],
    },
}

/// Stages of literacy progression, from raw sensory horror to active prevention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum LiteracyStage {
    /// Raw sensory horror — no comprehension.
    SensoryHorror = 0,
    /// Pattern noticed.
    Pattern = 1,
    /// Direction understood.
    Direction = 2,
    /// Schedule predicted.
    Schedule = 3,
    /// Interruption attempted.
    Interruption = 4,
    /// Active prevention.
    Prevention = 5,
}

impl LiteracyStage {
    /// Stable integer encoding of the stage.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl AllostaticCurve {
    /// The canonical six-stage literacy arc in order.
    pub const DEFAULT_LITERACY_ARC: Self = Self::LiteracyArc {
        stages: [
            LiteracyStage::SensoryHorror,
            LiteracyStage::Pattern,
            LiteracyStage::Direction,
            LiteracyStage::Schedule,
            LiteracyStage::Interruption,
            LiteracyStage::Prevention,
        ],
    };
}

/// Sample `curve` at `tick` from `initial_value` — the one evaluator (FORGE-HARMONICS-MATH-001, folded here 2026-09-05).
/// Linear: `initial + tick * slope_q8 / 256` clamped ≥ 0 · Exponential: `initial >> (tick / half_life)`, zero half-life → 0
/// · Threshold: `initial` until `drop_at_tick`, then 0 · Permanent: `initial` · LiteracyArc: stage index 0–5, one per tick.
pub fn value_at_tick(curve: &AllostaticCurve, tick: u64, initial_value: i32) -> i32 {
    match *curve {
        AllostaticCurve::Linear { slope_q8 } => {
            let delta = (tick as i64).saturating_mul(slope_q8 as i64) / 256;
            (initial_value as i64 + delta).clamp(0, i32::MAX as i64) as i32
        }
        AllostaticCurve::Exponential { half_life_ticks } => {
            if half_life_ticks == 0 {
                return 0;
            }
            let shifts = (tick / half_life_ticks as u64).min(31);
            initial_value.max(0) >> shifts
        }
        AllostaticCurve::Threshold { drop_at_tick } => {
            if tick >= drop_at_tick { 0 } else { initial_value }
        }
        AllostaticCurve::Permanent => initial_value,
        AllostaticCurve::LiteracyArc { .. } => tick.min(5) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literacy_arc_default_has_six_stages_in_order() {
        match AllostaticCurve::DEFAULT_LITERACY_ARC {
            AllostaticCurve::LiteracyArc { stages } => {
                assert_eq!(stages.len(), 6);
                assert_eq!(stages[0], LiteracyStage::SensoryHorror);
                assert_eq!(stages[1], LiteracyStage::Pattern);
                assert_eq!(stages[2], LiteracyStage::Direction);
                assert_eq!(stages[3], LiteracyStage::Schedule);
                assert_eq!(stages[4], LiteracyStage::Interruption);
                assert_eq!(stages[5], LiteracyStage::Prevention);
            }
            _ => panic!("expected LiteracyArc"),
        }
    }

    #[test]
    fn literacy_stage_as_u8_stable() {
        assert_eq!(LiteracyStage::SensoryHorror.as_u8(), 0);
        assert_eq!(LiteracyStage::Pattern.as_u8(), 1);
        assert_eq!(LiteracyStage::Direction.as_u8(), 2);
        assert_eq!(LiteracyStage::Schedule.as_u8(), 3);
        assert_eq!(LiteracyStage::Interruption.as_u8(), 4);
        assert_eq!(LiteracyStage::Prevention.as_u8(), 5);
    }

    #[test]
    fn linear_zero_tick_returns_initial() {
        let c = AllostaticCurve::Linear { slope_q8: -32 };
        assert_eq!(value_at_tick(&c, 0, 10_000), 10_000);
    }

    #[test]
    fn linear_decay_reduces_over_time() {
        let c = AllostaticCurve::Linear { slope_q8: -32 };
        assert_eq!(value_at_tick(&c, 256, 10_000), 9_968);
    }

    #[test]
    fn linear_clamps_at_zero() {
        let c = AllostaticCurve::Linear { slope_q8: -1000 };
        assert_eq!(value_at_tick(&c, 1_000_000, 10_000), 0);
    }

    #[test]
    fn exponential_halves_each_half_life() {
        let c = AllostaticCurve::Exponential { half_life_ticks: 4096 };
        assert_eq!(value_at_tick(&c, 0, 10_000), 10_000);
        assert_eq!(value_at_tick(&c, 4_096, 10_000), 5_000);
        assert_eq!(value_at_tick(&c, 8_192, 10_000), 2_500);
        assert_eq!(value_at_tick(&c, 12_288, 10_000), 1_250);
    }

    #[test]
    fn exponential_zero_half_life_returns_zero() {
        let c = AllostaticCurve::Exponential { half_life_ticks: 0 };
        assert_eq!(value_at_tick(&c, 0, 10_000), 0);
        assert_eq!(value_at_tick(&c, 100, 10_000), 0);
    }

    #[test]
    fn threshold_drops_at_tick() {
        let c = AllostaticCurve::Threshold { drop_at_tick: 1_000 };
        assert_eq!(value_at_tick(&c, 0, 10_000), 10_000);
        assert_eq!(value_at_tick(&c, 999, 10_000), 10_000);
        assert_eq!(value_at_tick(&c, 1_000, 10_000), 0);
        assert_eq!(value_at_tick(&c, 9_999, 10_000), 0);
    }

    #[test]
    fn permanent_never_changes() {
        let c = AllostaticCurve::Permanent;
        assert_eq!(value_at_tick(&c, 0, 5_000), 5_000);
        assert_eq!(value_at_tick(&c, 1_000_000, 5_000), 5_000);
        assert_eq!(value_at_tick(&c, u64::MAX, 5_000), 5_000);
    }

    #[test]
    fn literacy_arc_advances_and_saturates() {
        let c = AllostaticCurve::DEFAULT_LITERACY_ARC;
        assert_eq!(value_at_tick(&c, 0, 0), 0);
        assert_eq!(value_at_tick(&c, 1, 0), 1);
        assert_eq!(value_at_tick(&c, 3, 0), 3);
        assert_eq!(value_at_tick(&c, 5, 0), 5);
        assert_eq!(value_at_tick(&c, 100, 0), 5);
    }

    #[test]
    fn literacy_arc_ignores_initial_value() {
        let c = AllostaticCurve::DEFAULT_LITERACY_ARC;
        assert_eq!(value_at_tick(&c, 2, 9_999), 2);
        assert_eq!(value_at_tick(&c, 2, 0), 2);
    }
}
