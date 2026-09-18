//! Mulberry32 PRNG, ported verbatim from `F:\NewRepo\crates\forge-core\src\seed.rs`
//! (algorithm section only — the file's other seed-derivation helpers stay out
//! of scope). `Serialize`/`Deserialize` derives dropped: forge-core-v3 is
//! Crate Zero (zero deps by law), and nothing in the v3 port serializes this
//! type — same call as `Material` in `forge-correspondence-v3` and `TypeFace`
//! in `forge-canvas-v3`.

/// Fast 32-bit generator for combat/gameplay randomness. Canonical
/// implementation; algorithm: additive constant `0x6D2B79F5`.
///
/// FROZEN STREAM: v1's `ironroot-web/src/lib.rs:21` mirrors these constants
/// for the WASM build and asserts against baseline values — a change here is
/// a change there, in the same pass. The methods below are ADDITIVE (they
/// only consume `next_u32`), so folding new callers on costs the stream
/// nothing. L05 (one home): `forge-mud-v3::rng` re-exports this; no duplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mulberry32 {
    /// The generator's internal 64-bit state.
    pub state: u64,
}

impl Mulberry32 {
    /// A new generator seeded with `seed`.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// A value in `0..bound`. `bound == 0` yields 0.
    pub fn below(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            0
        } else {
            self.next_u32() % bound
        }
    }

    /// Permyriad roll — integer `0..=10000`, the engine's parts-per-ten-thousand
    /// unit (the float-free stand-in for a `0..1` fraction).
    pub fn permyriad(&mut self) -> u32 {
        self.below(10_001)
    }

    /// The next raw 32-bit value in the stream.
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6D2B79F5);
        let mut z = self.state as u32;
        z = (z ^ (z >> 15)).wrapping_mul(z | 1);
        z ^= z.wrapping_add((z ^ (z >> 7)).wrapping_mul(z | 61));
        z ^ (z >> 14)
    }

    /// The next value in the stream as a float in `0.0..=1.0`.
    pub fn next_f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    /// Fork an independent child stream keyed by `domain`, ported from
    /// `F:\NewRepo\crates\forge-core\src\seed.rs::ForgeRng::fork` (same
    /// domain-hash construction, retargeted at this generator's `next_u32`
    /// instead of `next_u64` — one RNG home in this crate, not two).
    pub fn fork(&mut self, domain: &str) -> Self {
        let mut hash = self.next_u32() as u64;
        for byte in domain.as_bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(*byte as u64);
        }
        Self::new(hash)
    }
}

#[cfg(test)]
mod mulberry32_tests {
    use super::*;

    /// Canonical matches an independent local reimplementation of the same algorithm.
    #[test]
    fn mulberry32_sequence_matches_local() {
        struct LocalMulberry32 {
            state: u64,
        }
        impl LocalMulberry32 {
            fn new(seed: u64) -> Self {
                Self { state: seed }
            }
            fn next_u32(&mut self) -> u32 {
                self.state = self.state.wrapping_add(0x6D2B79F5);
                let mut z = self.state as u32;
                z = (z ^ (z >> 15)).wrapping_mul(z | 1);
                z ^= z.wrapping_add((z ^ (z >> 7)).wrapping_mul(z | 61));
                z ^ (z >> 14)
            }
        }

        for seed in [0u64, 1, 42, 0xDEADBEEF, u64::MAX] {
            let mut canonical = Mulberry32::new(seed);
            let mut local = LocalMulberry32::new(seed);
            for _ in 0..1000 {
                assert_eq!(
                    canonical.next_u32(),
                    local.next_u32(),
                    "Mulberry32 sequence mismatch at seed {}",
                    seed
                );
            }
        }
    }

    #[test]
    fn mulberry32_deterministic() {
        let mut a = Mulberry32::new(42);
        let mut b = Mulberry32::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn fork_is_deterministic() {
        let mut a = Mulberry32::new(7);
        let mut b = Mulberry32::new(7);
        assert_eq!(a.fork("domain").state, b.fork("domain").state);
    }

    #[test]
    fn fork_produces_different_stream() {
        let mut rng = Mulberry32::new(7);
        let mut base = rng.clone();
        let mut forked = rng.fork("domain");
        assert_ne!(base.next_u32(), forked.next_u32());
    }
}
