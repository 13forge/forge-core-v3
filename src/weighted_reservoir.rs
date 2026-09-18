//! Weighted single-choice draws (Chao reservoir + closed-form scan), integer-only.
//! Drained from `F:\NewRepo\crates\forge-sieve\src\weighted_reservoir.rs`, RNG
//! retargeted ForgeRng::next_u64 → seed.rs Mulberry32 (one RNG home, two u32 draws).

use crate::seed::Mulberry32;

/// One 64-bit draw from the crate's 32-bit stream: high word first, then low.
fn draw64(rng: &mut Mulberry32) -> u64 {
    ((rng.next_u32() as u64) << 32) | rng.next_u32() as u64
}

/// Streaming weighted reservoir (k = 1), Chao's algorithm: item `i` is chosen
/// with probability `w_i / Σw` using only `u64` arithmetic — the integer-exact
/// cousin of Efraimidis-Spirakis, which needs a fractional power (a float).
/// Single-pass, no alloc, deterministic from the seed.
#[derive(Debug, Clone)]
pub struct WeightedReservoir {
    w_total: u64,
    held: Option<usize>,
    seen: usize,
    rng: Mulberry32,
}

impl WeightedReservoir {
    /// New empty reservoir seeded deterministically.
    pub fn new(seed: u64) -> Self {
        Self { w_total: 0, held: None, seen: 0, rng: Mulberry32::new(seed) }
    }

    /// Reuse an already-forked stream (e.g. a per-domain [`Mulberry32::fork`]).
    pub fn with_rng(rng: Mulberry32) -> Self {
        Self { w_total: 0, held: None, seen: 0, rng }
    }

    /// Offer the next item (its index in the caller's stream) with `weight`.
    /// Zero-weight items can never be picked. Returns the currently held index.
    pub fn offer(&mut self, index: usize, weight: u64) -> Option<usize> {
        if weight == 0 {
            self.seen += 1;
            return self.held;
        }
        self.w_total += weight;
        // Replace with probability weight / w_total: draw r in [0, w_total).
        let r = draw64(&mut self.rng) % self.w_total;
        if r < weight || self.held.is_none() {
            self.held = Some(index);
        }
        self.seen += 1;
        self.held
    }

    /// The currently held pick (`None` until a nonzero-weight item is offered).
    pub fn pick(&self) -> Option<usize> {
        self.held
    }

    /// How many items have been offered (weight zero or not).
    pub fn len(&self) -> usize {
        self.seen
    }

    /// True iff nothing has been offered yet.
    pub fn is_empty(&self) -> bool {
        self.seen == 0
    }
}

/// One-shot weighted pick over a materialised `(item, weight)` slice, by
/// cumulative-weight scan — the closed-form draw when the whole table is in
/// hand. Returns the chosen index, or `None` if the slice is empty or every
/// weight is zero. Deterministic.
pub fn weighted_pick<T>(items: &[(T, u64)], rng: &mut Mulberry32) -> Option<usize> {
    let total: u64 = items.iter().map(|(_, w)| *w).sum();
    if total == 0 {
        return None;
    }
    let mut r = draw64(rng) % total;
    for (i, (_, w)) in items.iter().enumerate() {
        if r < *w {
            return Some(i);
        }
        r -= *w;
    }
    // Unreachable: r < total guarantees a hit. Belt-and-suspenders → last nonzero.
    items.iter().rposition(|(_, w)| *w > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_all_zero_pick_none() {
        let mut rng = Mulberry32::new(1);
        let empty: [(u8, u64); 0] = [];
        assert_eq!(weighted_pick(&empty, &mut rng), None);
        let zeros = [(0u8, 0u64), (1, 0)];
        assert_eq!(weighted_pick(&zeros, &mut rng), None);
    }

    #[test]
    fn single_nonzero_always_wins() {
        let mut rng = Mulberry32::new(42);
        let items = [(b'a', 0u64), (b'b', 7), (b'c', 0)];
        for _ in 0..100 {
            assert_eq!(weighted_pick(&items, &mut rng), Some(1));
        }
    }

    #[test]
    fn deterministic_same_seed() {
        let items = [(0u8, 3u64), (1, 5), (2, 2)];
        let mut a = Mulberry32::new(99);
        let mut b = Mulberry32::new(99);
        for _ in 0..200 {
            assert_eq!(weighted_pick(&items, &mut a), weighted_pick(&items, &mut b));
        }
    }

    #[test]
    fn frequencies_track_weights() {
        // Weights 1:3:6 over 60_000 draws → counts within ~5% of expectation.
        let items = [(0u8, 1u64), (1, 3), (2, 6)];
        let mut rng = Mulberry32::new(7);
        let mut counts = [0u32; 3];
        let n = 60_000;
        for _ in 0..n {
            counts[weighted_pick(&items, &mut rng).unwrap()] += 1;
        }
        // Expected fractions 0.1 / 0.3 / 0.6.
        let frac = |c: u32| c as f64 / n as f64;
        assert!((frac(counts[0]) - 0.10).abs() < 0.02, "w1 {:?}", counts);
        assert!((frac(counts[1]) - 0.30).abs() < 0.03, "w3 {:?}", counts);
        assert!((frac(counts[2]) - 0.60).abs() < 0.03, "w6 {:?}", counts);
    }

    #[test]
    fn reservoir_matches_weighted_distribution() {
        // Streaming Chao reservoir over the same 1:3:6 weights.
        let weights = [1u64, 3, 6];
        let mut counts = [0u32; 3];
        let n = 60_000u64;
        for trial in 0..n {
            let mut res = WeightedReservoir::new(0xC0FFEE ^ trial);
            for (i, &w) in weights.iter().enumerate() {
                res.offer(i, w);
            }
            counts[res.pick().unwrap()] += 1;
        }
        let frac = |c: u32| c as f64 / n as f64;
        assert!((frac(counts[0]) - 0.10).abs() < 0.02, "res w1 {:?}", counts);
        assert!((frac(counts[1]) - 0.30).abs() < 0.03, "res w3 {:?}", counts);
        assert!((frac(counts[2]) - 0.60).abs() < 0.03, "res w6 {:?}", counts);
    }

    #[test]
    fn reservoir_skips_zero_weight() {
        let mut res = WeightedReservoir::new(5);
        assert_eq!(res.offer(0, 0), None); // zero weight can't be held
        assert_eq!(res.pick(), None);
        res.offer(1, 4);
        assert_eq!(res.pick(), Some(1));
        assert_eq!(res.len(), 2);
    }

    /// The fork seam the dialogue tape will use: a per-domain child stream
    /// yields the same pick for the same (seed, domain), a different domain
    /// is free to differ.
    #[test]
    fn forked_stream_pick_is_replayable() {
        let items = [("a", 2u64), ("b", 5), ("c", 3)];
        let pick_for = |domain: &str| {
            let mut world = Mulberry32::new(0xB0A7);
            let mut rng = world.fork(domain);
            weighted_pick(&items, &mut rng)
        };
        assert_eq!(pick_for("node:bell_warden:greet"), pick_for("node:bell_warden:greet"));
    }
}
