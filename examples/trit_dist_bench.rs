//! LUT-lookup vs. register-only decode, across growing array sizes, to find where
//! `TRIT_DIST_LUT`'s 64KB (65536 bytes — already bigger than a typical 32KB L1d)
//! stops paying for itself against `TritCell5D::trits()`'s 5-division decode.
//!
//! Not core — wall-clock timing lives here, behind the C14 firewall, never in
//! `route()` itself. `std` only: no `criterion` dep (none exists in this workspace,
//! L19 doesn't clear the bar for a one-off measurement), `Mulberry32` reused from
//! `crate::seed` for the pseudo-random byte stream instead of pulling `rand` (C06).
//!
//! `cargo run --release --example trit_dist_bench -p forge-core-v3`

use forge_core_v3::atom::TritCell5D;
use forge_core_v3::metarouter::TRIT_DIST_LUT;
use forge_core_v3::seed::Mulberry32;
use std::hint::black_box;
use std::time::Instant;

/// Register-only decoder: no table, pure division + subtraction, the exact cost
/// `TritCell5D::trits()` pays on every call (`atom.rs:79-91`).
#[inline]
fn dist_by_decode(c: u8, q: u8) -> u32 {
    let dc = TritCell5D(c).trits().unwrap_or([0i8; 5]);
    let dq = TritCell5D(q).trits().unwrap_or([0i8; 5]);
    let mut dist: u32 = 0;
    for k in 0..5 {
        let d = dc[k] - dq[k];
        dist += d.unsigned_abs() as u32;
    }
    dist
}

#[inline]
fn dist_by_lut(c: u8, q: u8) -> u32 {
    TRIT_DIST_LUT[((c as usize) << 8) | q as usize] as u32
}

/// Valid trit-byte range only (`0..=242`) — matches `route()`'s real domain; the
/// LUT's sentinel rows (`243..=255`) are never read in production either.
fn random_pairs(n: usize, seed: u64) -> Vec<(u8, u8)> {
    let mut rng = Mulberry32::new(seed);
    (0..n)
        .map(|_| {
            let c = rng.below(243) as u8;
            let q = rng.below(243) as u8;
            (c, q)
        })
        .collect()
}

/// Bigger than any plausible L2/L3 on this machine — touched at cache-line stride
/// (64B) so every prior cache resident (the LUT included) is genuinely evicted,
/// not just "probably" pushed out. Returns a sum so the sweep can't be
/// dead-code-eliminated.
const EVICT_BYTES: usize = 64 * 1024 * 1024;

fn evict_cache(evictor: &mut [u8]) -> u64 {
    let mut acc = 0u64;
    let mut i = 0usize;
    while i < evictor.len() {
        evictor[i] = evictor[i].wrapping_add(1);
        acc += evictor[i] as u64;
        i += 64;
    }
    black_box(acc)
}

/// Runs `reps` timed passes over `pairs` with a given per-op fn, evicting the
/// full cache hierarchy immediately before EACH rep when `cold` is set — so
/// `cold=true` measures actual memory-latency cost per access, not warm-cache
/// residency, and `cold=false` reproduces the original (contention-free) numbers
/// for direct before/after comparison.
fn timed_pass(
    pairs: &[(u8, u8)],
    evictor: &mut [u8],
    reps: u32,
    cold: bool,
    f: impl Fn(u8, u8) -> u32,
) -> (u128, u64) {
    let mut total_ns = 0u128;
    let mut sum = 0u64;
    for _ in 0..reps {
        if cold {
            black_box(evict_cache(evictor));
        }
        let t = Instant::now();
        for &(c, q) in pairs {
            sum += black_box(f(black_box(c), black_box(q))) as u64;
        }
        total_ns += t.elapsed().as_nanos();
    }
    (total_ns, sum)
}

fn bench_one(label: &str, n: usize, pairs: &[(u8, u8)], evictor: &mut [u8], reps: u32) {
    let (lut_warm_ns, lut_warm_sum) = timed_pass(pairs, evictor, reps, false, dist_by_lut);
    let (dec_warm_ns, dec_warm_sum) = timed_pass(pairs, evictor, reps, false, dist_by_decode);
    assert_eq!(lut_warm_sum, dec_warm_sum, "warm: LUT and decode must agree");

    let (lut_cold_ns, lut_cold_sum) = timed_pass(pairs, evictor, reps, true, dist_by_lut);
    let (dec_cold_ns, dec_cold_sum) = timed_pass(pairs, evictor, reps, true, dist_by_decode);
    assert_eq!(lut_cold_sum, dec_cold_sum, "cold: LUT and decode must agree");

    let per_op = |ns: u128| ns as f64 / (n as u128 * reps as u128) as f64;
    let (lut_warm, dec_warm) = (per_op(lut_warm_ns), per_op(dec_warm_ns));
    let (lut_cold, dec_cold) = (per_op(lut_cold_ns), per_op(dec_cold_ns));
    let working_set_bytes = n * 2;

    println!(
        "{label:>10}  n={n:>8}  ws~{working_set_bytes:>9}B  \
         WARM  LUT={lut_warm:>7.3} DEC={dec_warm:>7.3} ratio={:>5.2}  |  \
         COLD  LUT={lut_cold:>7.3} DEC={dec_cold:>7.3} ratio={:>5.2}  \
         (LUT cold/warm={:>5.2}x)",
        dec_warm / lut_warm,
        dec_cold / lut_cold,
        lut_cold / lut_warm,
    );
}

fn main() {
    println!(
        "TRIT_DIST_LUT is {} bytes (65536) — typical L1d is 32768-49152 bytes. \
         Evictor sweep is {}MB per cold rep.\n",
        std::mem::size_of_val(&TRIT_DIST_LUT),
        EVICT_BYTES / (1024 * 1024)
    );
    println!(
        "WARM = same run as before (LUT stays cache-resident, nothing evicts it). \
         COLD = a real {}MB touch-sweep evicts L1/L2/L3 before every single rep, \
         forcing genuine memory-latency reloads — this is the forced-contention run.\n",
        EVICT_BYTES / (1024 * 1024)
    );

    // Fewer reps than the warm-only run: each cold rep now pays for a real 64MB
    // sweep (not free), so 6 reps keeps total runtime bounded while still
    // averaging out sweep-to-sweep noise.
    const REPS: u32 = 6;
    let mut evictor = vec![0u8; EVICT_BYTES];

    let sizes: &[(&str, usize)] = &[
        ("tiny", 64),
        ("small", 1_024),
        ("l1-edge", 16_384),   // ~32KB working set, at a typical L1d boundary
        ("l1-full", 24_576),   // ~48KB working set, at a larger L1d boundary
        ("over-lut", 40_000),  // working set alone exceeds the 64KB LUT footprint
        ("l2-ish", 131_072),
        ("large", 1_048_576),
    ];

    for (label, n) in sizes {
        let pairs = random_pairs(*n, 0xC0FFEE ^ *n as u64);
        bench_one(label, *n, &pairs, &mut evictor, REPS);
    }
}
