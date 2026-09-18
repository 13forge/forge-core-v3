//! `ExpertCache` -- Hot (GPU) / Warm (pinned RAM) / Cold (mmap NVMe) residency
//! tracker (Phase 2 of the GPU/CPU dual-flywheel plan). Ported as-is from
//! `F:\NewRepo\crates\forge-ml\src\expert_cache.rs` (P2.1) -- architecture/
//! quantization-agnostic, same class of port as Phase 1's `forge-gpu-warden`.
//!
//! The cache tracks expert-ID residency across three tiers. Actual bytes are
//! owned by callers -- this module only schedules which tier an expert
//! *should* be in, wired against the 7 experts `MetaRouter`'s shipped
//! `.s13` asset routes across (`0..7`, see `metarouter::MetaRouter::route`).

use std::collections::{HashMap, VecDeque};

/// A `MetaRouter`-space expert identifier. Widened to `u32` (vs.
/// `MetaRouter::route`'s `u8` return) to leave room for Tier 2's
/// `SubRouter` sub-expert IDs once Phase 4 wires trit-native routing --
/// today only the 7 top-level expert IDs (`0..7`) are ever passed in.
pub type ExpertId = u32;

/// Residency tier an expert can occupy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// GPU-resident.
    Hot,
    /// Pinned RAM.
    Warm,
    /// mmap NVMe / not cached.
    Cold,
}

/// Cumulative access-pattern counters, read-only via `ExpertCache::stats`.
#[derive(Default, Clone, Copy, Debug)]
pub struct CacheStats {
    /// Accesses that found the expert already in `Tier::Hot`.
    pub hits: u64,
    /// Accesses that found the expert in `Tier::Cold` (first touch or post-eviction).
    pub misses: u64,
    /// Warm-tier entries dropped to `Tier::Cold` to enforce `warm_capacity`.
    pub evictions: u64,
    /// Warm-to-Hot tier promotions.
    pub promotions: u64,
}

#[derive(Clone, Debug)]
struct Entry {
    tier: Tier,
}

/// LRU-tiered residency scheduler for experts. Owns no expert bytes -- only
/// the tier bookkeeping (`Hot`/`Warm`/`Cold`) a caller consults before a
/// dispatch decision.
pub struct ExpertCache {
    hot_capacity: usize,
    warm_capacity: usize,
    hot_lru: VecDeque<ExpertId>,
    warm_lru: VecDeque<ExpertId>,
    entries: HashMap<ExpertId, Entry>,
    stats: CacheStats,
}

impl ExpertCache {
    /// Builds an empty cache with the given per-tier capacities (entry counts,
    /// not bytes).
    pub fn new(hot_capacity: usize, warm_capacity: usize) -> Self {
        Self {
            hot_capacity,
            warm_capacity,
            hot_lru: VecDeque::new(),
            warm_lru: VecDeque::new(),
            entries: HashMap::new(),
            stats: CacheStats::default(),
        }
    }

    /// Current tier of `id`; `Tier::Cold` if never seen or evicted.
    pub fn tier_of(&self, id: ExpertId) -> Tier {
        self.entries.get(&id).map(|e| e.tier).unwrap_or(Tier::Cold)
    }

    /// Snapshot of cumulative access counters.
    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    /// Number of experts currently `Tier::Hot`.
    pub fn hot_count(&self) -> usize {
        self.hot_lru.len()
    }

    /// Number of experts currently `Tier::Warm`.
    pub fn warm_count(&self) -> usize {
        self.warm_lru.len()
    }

    /// Record an access; returns the tier the expert was in BEFORE this access.
    /// Afterwards the expert is always in Hot.
    pub fn access(&mut self, id: ExpertId, _bytes: u64) -> Tier {
        let prev = self.tier_of(id);
        match prev {
            Tier::Hot => {
                self.stats.hits += 1;
                move_to_back(&mut self.hot_lru, id);
            }
            Tier::Warm => {
                self.stats.hits += 1;
                remove_id(&mut self.warm_lru, id);
                self.entries.insert(id, Entry { tier: Tier::Hot });
                self.stats.promotions += 1;
                self.push_hot(id);
            }
            Tier::Cold => {
                self.stats.misses += 1;
                self.entries.insert(id, Entry { tier: Tier::Hot });
                self.stats.promotions += 1;
                self.push_hot(id);
            }
        }
        prev
    }

    /// Prefetch: load expert into Warm tier without promoting to Hot.
    /// Used by SpeculativePrefetcher (P2.2).
    pub fn prefetch_to_warm(&mut self, id: ExpertId, _bytes: u64) {
        if matches!(self.tier_of(id), Tier::Hot | Tier::Warm) {
            return;
        }
        self.entries.insert(id, Entry { tier: Tier::Warm });
        self.warm_lru.push_back(id);
        self.enforce_warm_cap();
    }

    fn push_hot(&mut self, id: ExpertId) {
        self.hot_lru.push_back(id);
        while self.hot_lru.len() > self.hot_capacity {
            if let Some(victim) = self.hot_lru.pop_front() {
                if let Some(entry) = self.entries.get_mut(&victim) {
                    entry.tier = Tier::Warm;
                }
                self.warm_lru.push_back(victim);
                self.enforce_warm_cap();
            }
        }
    }

    fn enforce_warm_cap(&mut self) {
        while self.warm_lru.len() > self.warm_capacity {
            if let Some(victim) = self.warm_lru.pop_front() {
                self.entries.remove(&victim);
                self.stats.evictions += 1;
            }
        }
    }
}

fn remove_id(q: &mut VecDeque<ExpertId>, id: ExpertId) {
    if let Some(pos) = q.iter().position(|&x| x == id) {
        q.remove(pos);
    }
}

fn move_to_back(q: &mut VecDeque<ExpertId>, id: ExpertId) {
    if let Some(pos) = q.iter().position(|&x| x == id) {
        q.remove(pos);
    }
    q.push_back(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cache_is_empty() {
        let c = ExpertCache::new(4, 8);
        assert_eq!(c.hot_count(), 0);
        assert_eq!(c.warm_count(), 0);
        assert_eq!(c.tier_of(42), Tier::Cold);
    }

    #[test]
    fn first_access_is_miss_into_hot() {
        let mut c = ExpertCache::new(4, 8);
        let prev = c.access(1, 100);
        assert_eq!(prev, Tier::Cold);
        assert_eq!(c.tier_of(1), Tier::Hot);
        assert_eq!(c.stats().misses, 1);
        assert_eq!(c.stats().hits, 0);
    }

    #[test]
    fn repeat_access_is_hot_hit() {
        let mut c = ExpertCache::new(4, 8);
        c.access(1, 100);
        let prev = c.access(1, 100);
        assert_eq!(prev, Tier::Hot);
        assert_eq!(c.stats().hits, 1);
    }

    #[test]
    fn hot_overflow_demotes_lru_to_warm() {
        let mut c = ExpertCache::new(2, 8);
        c.access(1, 100);
        c.access(2, 100);
        c.access(3, 100);
        assert_eq!(c.tier_of(1), Tier::Warm, "oldest Hot moves to Warm");
        assert_eq!(c.tier_of(2), Tier::Hot);
        assert_eq!(c.tier_of(3), Tier::Hot);
    }

    #[test]
    fn warm_overflow_evicts_to_cold() {
        let mut c = ExpertCache::new(1, 2);
        c.access(1, 100);
        c.access(2, 100);
        c.access(3, 100);
        c.access(4, 100);
        assert_eq!(c.tier_of(1), Tier::Cold, "oldest Warm evicted");
        assert!(c.stats().evictions >= 1);
    }

    #[test]
    fn warm_hit_promotes_to_hot() {
        let mut c = ExpertCache::new(1, 4);
        c.access(1, 100);
        c.access(2, 100); // pushes 1 to Warm
        assert_eq!(c.tier_of(1), Tier::Warm);
        let prev = c.access(1, 100);
        assert_eq!(prev, Tier::Warm);
        assert_eq!(c.tier_of(1), Tier::Hot);
        assert_eq!(c.stats().hits, 1);
    }

    #[test]
    fn evicted_expert_is_miss_again() {
        let mut c = ExpertCache::new(1, 1);
        c.access(1, 100);
        c.access(2, 100);
        c.access(3, 100);
        // 1 evicted to Cold. Re-accessing counts as miss.
        let misses_before = c.stats().misses;
        let prev = c.access(1, 100);
        assert_eq!(prev, Tier::Cold);
        assert_eq!(c.stats().misses, misses_before + 1);
    }

    #[test]
    fn prefetch_skips_promotion() {
        let mut c = ExpertCache::new(2, 4);
        c.prefetch_to_warm(7, 100);
        assert_eq!(c.tier_of(7), Tier::Warm);
        assert_eq!(c.stats().promotions, 0);
        // An actual access then promotes
        let prev = c.access(7, 100);
        assert_eq!(prev, Tier::Warm);
        assert_eq!(c.tier_of(7), Tier::Hot);
    }

    #[test]
    fn shipped_sovereign_asset_expert_ids_are_valid_cache_keys() {
        // Plan Phase 2 verify step: the 7 expert IDs MetaRouter::route()
        // returns from the shipped sovereign.s13 asset must all be usable
        // ExpertCache keys (0..7, matching MetaRouter::num_experts == 7).
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sovereign.s13");
        let router = crate::metarouter::MetaRouter::<{ crate::metarouter::NUM_EXPERTS }>::load(&path).expect("shipped .s13 asset must load");
        assert_eq!(router.num_experts, 7);

        let mut c = ExpertCache::new(7, 7);
        for expert in 0u32..7 {
            let prev = c.access(expert, 0);
            assert_eq!(prev, Tier::Cold, "first touch of a fresh expert id must be a cold miss");
            assert_eq!(c.tier_of(expert), Tier::Hot);
        }
        assert_eq!(c.hot_count(), 7);
        assert_eq!(c.stats().misses, 7);
    }
}