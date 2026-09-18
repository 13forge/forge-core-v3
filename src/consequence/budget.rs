//! Ported verbatim from F:\NewRepo\crates\forge-consequence\src\budget.rs (2026-08-17 truth-hunt lineage port, completing the 2026-08-13 wce-tags-port).
//!
//! `InteractionBudget` — per-tick interaction cap with a defer queue.
//!
//! Mirrors the `forge-hal::VramBudget` API shape but governs interaction
//! count per tick instead of GPU bytes.
//!
//! Defer policy: when the per-tick cap is hit, queries land in a fixed-size
//! deferred ring. When the deferred ring is full, the **weakest** query
//! (lowest intensity) is evicted to make room for stronger incoming queries.
//!
//! The budget stores `PendingInteraction` (query + cell_id + position) so
//! deferred queries don't lose their cell binding when released next tick.

use std::collections::VecDeque;

use super::query::PendingInteraction;

/// Default per-tick interaction cap. Design specced 16..=200 dynamic; MVP
/// uses the upper bound. Tunable at construction.
pub const DEFAULT_CAP_PER_TICK: u16 = 200;

/// Default deferred-queue capacity. Design ceiling = 512.
pub const DEFAULT_DEFER_CAP: u16 = 512;

/// Outcome of an `admit` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetOutcome {
    /// Query admitted and counted against the tick cap.
    Admitted,
    /// Cap hit. Query parked in the deferred queue for a later tick.
    Deferred,
    /// Deferred queue full and the incoming query was weaker than the
    /// weakest already-deferred query. Dropped.
    Dropped,
    /// Deferred queue full but the incoming query was stronger than the
    /// weakest deferred — that weakest one was evicted and the incoming
    /// query took its slot.
    EvictedWeakest,
}

/// Tracks interaction admissions against a per-tick cap, with a defer queue.
#[derive(Clone)]
pub struct InteractionBudget {
    cap_per_tick: u16,
    defer_cap: u16,
    current_tick_admitted: u16,
    deferred: VecDeque<PendingInteraction>,
    total_admitted: u64,
    total_deferred: u64,
    total_dropped: u64,
}

impl InteractionBudget {
    /// Construct with the given caps. Both clamped to >=1.
    pub fn new(cap_per_tick: u16, defer_cap: u16) -> Self {
        Self {
            cap_per_tick: cap_per_tick.max(1),
            defer_cap: defer_cap.max(1),
            current_tick_admitted: 0,
            deferred: VecDeque::with_capacity(defer_cap as usize),
            total_admitted: 0,
            total_deferred: 0,
            total_dropped: 0,
        }
    }

    /// Default budget — 200/tick cap, 512 defer cap.
    pub fn default_budget() -> Self {
        Self::new(DEFAULT_CAP_PER_TICK, DEFAULT_DEFER_CAP)
    }

    /// Admit a pending interaction. Returns the policy outcome.
    pub fn admit(&mut self, p: PendingInteraction) -> BudgetOutcome {
        if self.current_tick_admitted < self.cap_per_tick {
            self.current_tick_admitted += 1;
            self.total_admitted += 1;
            return BudgetOutcome::Admitted;
        }

        if (self.deferred.len() as u16) < self.defer_cap {
            self.deferred.push_back(p);
            self.total_deferred += 1;
            return BudgetOutcome::Deferred;
        }

        // Defer queue full — compare intensities.
        let (weakest_idx, weakest_intensity) = self
            .deferred
            .iter()
            .enumerate()
            .map(|(i, x)| (i, x.intensity_pmy()))
            .min_by_key(|&(_, v)| v)
            .expect("defer queue is non-empty by construction");

        if p.intensity_pmy() > weakest_intensity {
            self.deferred.remove(weakest_idx);
            self.deferred.push_back(p);
            self.total_dropped += 1;
            BudgetOutcome::EvictedWeakest
        } else {
            self.total_dropped += 1;
            BudgetOutcome::Dropped
        }
    }

    /// Roll the tick. Returns deferred interactions that fit under the new
    /// cap; the rest stay parked.
    pub fn next_tick(&mut self) -> Vec<PendingInteraction> {
        self.current_tick_admitted = 0;
        let mut released = Vec::new();
        while self.current_tick_admitted < self.cap_per_tick {
            match self.deferred.pop_front() {
                Some(p) => {
                    released.push(p);
                    self.current_tick_admitted += 1;
                    self.total_admitted += 1;
                }
                None => break,
            }
        }
        released
    }

    /// Queries admitted this tick.
    #[inline]
    pub fn admitted_this_tick(&self) -> u16 { self.current_tick_admitted }

    /// Per-tick admission cap.
    #[inline]
    pub fn cap_per_tick(&self) -> u16 { self.cap_per_tick }

    /// Number of queries currently parked in the defer queue.
    #[inline]
    pub fn deferred_count(&self) -> usize { self.deferred.len() }

    /// Defer queue capacity.
    #[inline]
    pub fn defer_cap(&self) -> u16 { self.defer_cap }

    /// Total admitted across all ticks.
    #[inline]
    pub fn total_admitted(&self) -> u64 { self.total_admitted }

    /// Total parked across all ticks.
    #[inline]
    pub fn total_deferred(&self) -> u64 { self.total_deferred }

    /// Total dropped or evicted across all ticks.
    #[inline]
    pub fn total_dropped(&self) -> u64 { self.total_dropped }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consequence::query::InteractionQuery;
    use crate::fixed_point::MilliUnit;

    fn p(intensity: u16) -> PendingInteraction {
        PendingInteraction::new(
            0,
            [MilliUnit(0); 3],
            InteractionQuery { intensity_pmy: intensity, ..InteractionQuery::default() },
        )
    }

    #[test]
    fn admits_up_to_cap() {
        let mut b = InteractionBudget::new(3, 4);
        assert_eq!(b.admit(p(100)), BudgetOutcome::Admitted);
        assert_eq!(b.admit(p(100)), BudgetOutcome::Admitted);
        assert_eq!(b.admit(p(100)), BudgetOutcome::Admitted);
        assert_eq!(b.admitted_this_tick(), 3);
    }

    #[test]
    fn defers_after_cap() {
        let mut b = InteractionBudget::new(2, 4);
        b.admit(p(100));
        b.admit(p(100));
        assert_eq!(b.admit(p(100)), BudgetOutcome::Deferred);
        assert_eq!(b.deferred_count(), 1);
    }

    #[test]
    fn next_tick_releases_deferred() {
        let mut b = InteractionBudget::new(2, 4);
        b.admit(p(100));
        b.admit(p(100));
        b.admit(p(100));
        b.admit(p(100));
        assert_eq!(b.deferred_count(), 2);

        let released = b.next_tick();
        assert_eq!(released.len(), 2);
        assert_eq!(b.deferred_count(), 0);
    }

    #[test]
    fn next_tick_preserves_cell_id_and_position() {
        // Per-cell binding must survive defer/release round-trip.
        let mut b = InteractionBudget::new(1, 4);
        b.admit(p(100)); // admitted
        let parked = PendingInteraction::new(
            777,
            [MilliUnit(1), MilliUnit(2), MilliUnit(3)],
            InteractionQuery { intensity_pmy: 100, ..InteractionQuery::default() },
        );
        b.admit(parked);
        let released = b.next_tick();
        assert_eq!(released.len(), 1);
        assert_eq!(released[0].cell_id, 777);
        assert_eq!(released[0].position, [MilliUnit(1), MilliUnit(2), MilliUnit(3)]);
    }

    #[test]
    fn evicts_weakest_when_defer_full() {
        let mut b = InteractionBudget::new(1, 2);
        b.admit(p(5_000));
        b.admit(p(1_000));
        b.admit(p(2_000));
        assert_eq!(b.deferred_count(), 2);
        assert_eq!(b.admit(p(3_000)), BudgetOutcome::EvictedWeakest);
        assert_eq!(b.deferred_count(), 2);
        let min = b.deferred.iter().map(|x| x.intensity_pmy()).min().unwrap();
        assert_eq!(min, 2_000);
    }

    #[test]
    fn drops_when_weaker_than_weakest_deferred() {
        let mut b = InteractionBudget::new(1, 2);
        b.admit(p(5_000));
        b.admit(p(4_000));
        b.admit(p(3_000));
        assert_eq!(b.admit(p(1_000)), BudgetOutcome::Dropped);
        assert_eq!(b.deferred_count(), 2);
        let min = b.deferred.iter().map(|x| x.intensity_pmy()).min().unwrap();
        assert_eq!(min, 3_000);
    }

    #[test]
    fn stress_8k_admits_respects_cap_and_defer() {
        let mut b = InteractionBudget::default_budget();
        for i in 0..8000u16 {
            let intensity = (i % 10000) + 1;
            let _ = b.admit(p(intensity));
        }
        assert!(b.admitted_this_tick() <= b.cap_per_tick());
        assert!(b.deferred_count() as u16 <= b.defer_cap());
        let total = b.total_admitted() + b.total_deferred() + b.total_dropped();
        assert!(total >= 8000);
    }
}
