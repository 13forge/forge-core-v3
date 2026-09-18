//! Liminal — the half-hanged lifecycle contract, de-gamed.
//!
//! Drained 2026-08-17 from the ironroot-lineage half-hanged mechanism
//! (`crates/forge-arena-v3/src/half_hanged.rs` in this tree; v2
//! `forge-game-systems/src/arena_core/half_hanged.rs`): when a resource hits
//! zero, an entity does not fall straight to Dead — it enters a BOUNDED
//! intermediate state with a hard tick deadline and a degraded capability set,
//! then dies on schedule. Stripped of the game skin that is a
//! graceful-degradation contract for any process/lane/service:
//! `Live -> HalfHanged(deadline) -> Dead`, integer ticks, no wall clock (C14).
//!
//! Doctrinal rhyme (PARARITY.md): this is the trit as a lifecycle — Live = +1,
//! HalfHanged = 0 (the fulcrum: not dead, not alive, bounded moves left),
//! Dead = -1. The liminal state is the fixed point of the kill involution.
//!
//! First consumer: the gemma sidecar's RSS-breach path (`sidecar/src/serve.rs`),
//! which previously abandoned its queue on breach — a cliff wearing a drain's
//! name. Aperture (C09): the donor's cooldown-limited last action is NOT ported
//! yet — no current consumer needs it; a Warden GPU-veto consumer would add it
//! as its own field, not overload the deadline.

/// The three lifecycle phases. `HalfHanged` carries its own countdown so the
/// phase IS the state — no side table to drift from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiminalPhase {
    /// Fully alive; all capabilities.
    Live,
    /// Wounded: bounded window, degraded capabilities, dying on schedule.
    HalfHanged {
        /// Ticks left before the transition to `Dead`. Reaching this state
        /// with `remaining == 0` is legal and dies on the next [`Liminal::tick`].
        remaining: u32,
    },
    /// Terminal. A dead entity never revives (no necromancy in core).
    Dead,
}

/// The lifecycle state machine. All transitions are explicit and one-way:
/// `wound` is the only exit from `Live`, `tick` the only exit from
/// `HalfHanged`, and nothing exits `Dead`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Liminal {
    phase: LiminalPhase,
}

impl Liminal {
    /// A fresh, fully-live entity.
    pub fn new() -> Self {
        Self { phase: LiminalPhase::Live }
    }

    /// Current phase, copyable.
    pub fn phase(&self) -> LiminalPhase {
        self.phase
    }

    /// True only in `Live`.
    pub fn is_live(&self) -> bool {
        self.phase == LiminalPhase::Live
    }

    /// True only in `Dead`.
    pub fn is_dead(&self) -> bool {
        self.phase == LiminalPhase::Dead
    }

    /// Enter the half-hanged window with `deadline_ticks` left to live.
    /// First wound wins: wounding an already-wounded or dead entity changes
    /// nothing — a second breach must never EXTEND a dying window (that would
    /// let repeated wounds keep a zombie alive indefinitely).
    pub fn wound(&mut self, deadline_ticks: u32) {
        if self.phase == LiminalPhase::Live {
            self.phase = LiminalPhase::HalfHanged { remaining: deadline_ticks };
        }
    }

    /// Advance one tick. `Live` ignores ticks (nothing is dying); a
    /// `HalfHanged` window counts down and transitions to `Dead` exactly when
    /// its budget is spent; `Dead` stays dead. Returns the phase AFTER the tick.
    pub fn tick(&mut self) -> LiminalPhase {
        if let LiminalPhase::HalfHanged { remaining } = self.phase {
            // `remaining == 0` still ticks once more before dying, so a
            // deadline of N grants exactly N in-window ticks (proven by
            // `deadline_grants_exactly_n_window_ticks`).
            self.phase = match remaining.checked_sub(1) {
                Some(left) => LiminalPhase::HalfHanged { remaining: left },
                None => LiminalPhase::Dead,
            };
        }
        self.phase
    }
}

impl Default for Liminal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_ignores_ticks() {
        let mut l = Liminal::new();
        for _ in 0..100 {
            assert_eq!(l.tick(), LiminalPhase::Live);
        }
        assert!(l.is_live());
    }

    #[test]
    fn deadline_grants_exactly_n_window_ticks() {
        // wound(3): ticks land on remaining 2, 1, 0 (all still HalfHanged),
        // and the 4th tick is the death — a deadline of N = N in-window ticks.
        let mut l = Liminal::new();
        l.wound(3);
        assert_eq!(l.tick(), LiminalPhase::HalfHanged { remaining: 2 });
        assert_eq!(l.tick(), LiminalPhase::HalfHanged { remaining: 1 });
        assert_eq!(l.tick(), LiminalPhase::HalfHanged { remaining: 0 });
        assert_eq!(l.tick(), LiminalPhase::Dead);
    }

    #[test]
    fn zero_deadline_dies_on_first_tick() {
        let mut l = Liminal::new();
        l.wound(0);
        assert_eq!(l.phase(), LiminalPhase::HalfHanged { remaining: 0 });
        assert_eq!(l.tick(), LiminalPhase::Dead);
    }

    #[test]
    fn first_wound_wins_no_zombie_extension() {
        let mut l = Liminal::new();
        l.wound(2);
        l.wound(1_000_000); // a second breach must not extend the window
        assert_eq!(l.phase(), LiminalPhase::HalfHanged { remaining: 2 });
        l.tick();
        l.tick();
        assert_eq!(l.tick(), LiminalPhase::Dead);
        l.wound(50); // and the dead stay dead
        assert!(l.is_dead());
    }

    #[test]
    fn dead_stays_dead_through_ticks() {
        let mut l = Liminal::new();
        l.wound(0);
        l.tick();
        assert!(l.is_dead());
        for _ in 0..10 {
            assert_eq!(l.tick(), LiminalPhase::Dead);
        }
    }
}
