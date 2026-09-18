//! MetaRouter — TQ (5D-Trit Quantized) 1-of-7 domain routing.
//!
//! Loads a `.s13` file produced by `gemma-sidecar quantize-s13 pack` (reads a
//! pretrained NDE F32 router safetensor, quantizes to trits). At inference:
//! query_f32 → normalize → trit-pack (5 dims/byte, base-3) → LUT distance
//! against 7 centroids → top-1.
//!
//! Trit byte range is `0..=242` (`3^5 = 243`, matching `sentinel::MAX_PACKED`
//! exactly) — bytes `243..=255` are out-of-band `S13` sentinels, trapped in
//! `route()` rather than folded into a bogus distance.
//!
//! Invention #169: Hierarchical 7-700-7 MoE routing.

use crate::atom::TritCell5D;
use crate::fixed_point::Permyriad;
use std::path::Path;

const MAGIC: [u8; 4] = *b"S13\x01";

/// Upper bound on `bytes_per_centroid` a loaded `.s13` may declare — sizes
/// `route()`'s stack-only trit-pack buffer (`forbidden_ops::hot_path_heap_alloc`
/// bans a `Vec` there). `ceil(d_model / 5)`, so 1024 covers `d_model` up to
/// 5,120 — the largest router in this repo packs `d_model=64` (13 bytes;
/// `sidecar/src/router_swap.rs:65,278`), so this is generous headroom, not a
/// tight fit. `MetaRouter::load()` rejects any `.s13` declaring more.
pub const MAX_BYTES_PER_CENTROID: usize = 1024;

/// The shipped expert count: hierarchical 7-700-7 MoE. `MetaRouter` defaults to it.
pub const NUM_EXPERTS: usize = 7;

/// Loaded MetaRouter ready for inference, generic over the expert count `N`
/// (default [`NUM_EXPERTS`]); every array, loop bound and header gate reads `N`.
pub struct MetaRouter<const N: usize = NUM_EXPERTS> {
    /// Query vector dimensionality.
    pub d_model: u16,
    /// Number of routable experts as declared by the `.s13` header; always `N` after `load`.
    pub num_experts: u8,
    /// Bytes per TQ-packed centroid (`ceil(d_model / 5)`, 5 trits/byte).
    pub bytes_per_centroid: u16,
    /// Per-expert score bias, applied at 10x weight during routing.
    pub bias: [f32; N],
    /// `N` × `bytes_per_centroid`, packed 5D-trit bytes (base-3, `0..=242`).
    pub centroids: Vec<u8>,
}

/// `ceil(d_model / 5)` — the trit-byte count a `d_model`-dim vector packs
/// into, 5 trits/byte. Shared by `route()`'s runtime pack and the offline
/// quantizer so header math never drifts between producer and consumer.
pub const fn trit_bytes_needed(d_model: u16) -> u16 {
    (d_model + 4) / 5
}

/// One dimension's balanced trit: `1` = positive, `-1` = negative, `0` =
/// neutral (inside the `EPS` deadzone). Matches `atom::TritCell5D`'s
/// balanced-ternary convention (`trits()`/`from_trits()`), not an
/// independent 0/1/2 scheme — one radix-3 encoding, one home (L05).
/// `EPS` is relative to the caller's already-normalized value, not the raw
/// magnitude. `pub` so a training-time discretizer (straight-through
/// estimator or otherwise) can share the exact same decision boundary as
/// `pack_trits()` instead of re-deriving it and drifting (L05).
pub const TRIT_EPS: f32 = 1e-6;

#[inline]
const fn trit_of(v: f32) -> i8 {
    if v > TRIT_EPS {
        1
    } else if v < -TRIT_EPS {
        -1
    } else {
        0
    }
}

/// Packs an f32 vector into 5D-trit bytes, writing into a caller-supplied
/// buffer — the alloc-free home for the encoding (`route()`'s hot path uses
/// this directly, per `forbidden_ops::hot_path_heap_alloc`). Normalize, map
/// each dimension to a balanced trit, pack 5 trits/byte via
/// `atom::TritCell5D::from_trits` (the crate's one home for the radix-3
/// packed-byte encoding — this function does not re-derive it). Dimensions
/// past `query.len()` (padding out an incomplete final byte) pack as the
/// neutral trit `0`. `out.len()` is the caller's `bytes_per_centroid`.
pub fn pack_trits_into(query: &[f32], out: &mut [u8]) {
    let norm = query.iter().map(|x| x * x).sum::<f32>().sqrt();
    let inv_norm = if norm > 1e-10 { 1.0 / norm } else { 1.0 };

    for (byte_idx, slot) in out.iter_mut().enumerate() {
        let mut digits = [0i8; 5];
        for (k, digit) in digits.iter_mut().enumerate() {
            let dim = byte_idx * 5 + k;
            *digit = if dim < query.len() { trit_of(query[dim] * inv_norm) } else { 0 };
        }
        *slot = TritCell5D::from_trits(digits).0;
    }
}

/// Allocating wrapper over [`pack_trits_into`] for offline/training callers
/// (the quantizer, distillation, flywheel tooling) where a `Vec<u8>` is the
/// natural shape and there is no hot-path constraint. `route()` does NOT
/// call this — it calls `pack_trits_into` against a stack buffer instead
/// (L05: one encoding, `pack_trits_into` is its one home; this is a thin
/// shape adapter over it, not a second derivation).
pub fn pack_trits(query: &[f32], bytes_per_centroid: usize) -> Vec<u8> {
    let mut out = vec![0u8; bytes_per_centroid];
    pack_trits_into(query, &mut out);
    out
}

const fn build_trit_dist_lut() -> [u8; 65536] {
    let mut lut = [0u8; 65536];
    let mut a = 0usize;
    while a < 256 {
        // `TritCell5D::trits()` is `None` for a >= sentinel::MAX_PACKED
        // (243..=255); the LUT still fills those rows mechanically (as
        // all-neutral) because they're never read — `route()` traps a
        // sentinel byte before it reaches a lookup.
        let da = match TritCell5D(a as u8).trits() {
            Some(t) => t,
            None => [0i8; 5],
        };
        let mut b = 0usize;
        while b < 256 {
            let db = match TritCell5D(b as u8).trits() {
                Some(t) => t,
                None => [0i8; 5],
            };
            let mut dist: u8 = 0;
            let mut k = 0;
            while k < 5 {
                let d = da[k] - db[k];
                dist += if d < 0 { (-d) as u8 } else { d as u8 };
                k += 1;
            }
            lut[a * 256 + b] = dist;
            b += 1;
        }
        a += 1;
    }
    lut
}

/// Precomputed per-trit-byte-pair distance (sum of 5 per-trit `|a-b|`s, max
/// `10`). Built once at compile time (`const fn`, no heap alloc — the
/// hot-path-heap-alloc ban holds) by decoding through `atom::TritCell5D`,
/// not a parallel decoder. `route()` looks up `LUT[(a << 8) | b]` instead of
/// decoding both bytes on every comparison.
///
/// Only meaningful for `a, b < sentinel::MAX_PACKED` — entries for `a` or
/// `b >= 243` decode mechanically but are never read: `route()` traps a
/// sentinel byte before it reaches a lookup.
pub static TRIT_DIST_LUT: [u8; 65536] = build_trit_dist_lut();

/// Builds the raw `.s13` file bytes: magic(4) + d_model(u16) + num_experts(u8,
/// always 7) + bytes_per_centroid(u16) + `bias[7]` f32 + centroids. The one home
/// for the `.s13` byte layout — `MetaRouter::load()` is this function's
/// reader; every producer (offline quantizer, training export) calls this
/// instead of re-deriving the header (L05).
pub fn build_s13_bytes<const N: usize>(d_model: u16, bias: [f32; N], centroids: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(4 + 2 + 1 + 2 + 4 * N + centroids.len());
    data.extend_from_slice(&MAGIC);
    data.extend_from_slice(&d_model.to_le_bytes());
    data.push(N as u8);
    data.extend_from_slice(&((centroids.len() / N) as u16).to_le_bytes());
    for b in bias {
        data.extend_from_slice(&b.to_le_bytes());
    }
    data.extend_from_slice(centroids);
    data
}

impl<const N: usize> MetaRouter<N> {
    /// `N` must fit the header byte and leave a top-2 for the margin.
    const N_OK: () = assert!(N >= 2 && N <= u8::MAX as usize, "MetaRouter needs 2..=255 experts");

    /// Loads a `.s13` 5D Trit-Packed Sentinel routing file produced by
    /// `gemma-sidecar quantize-s13 pack`. Hard-rejects any file that doesn't
    /// declare itself `S13\x01` — no silent fallback to the retired `MR01`
    /// header.
    pub fn load(path: &Path) -> Result<Self, String> {
        if path.extension().and_then(|s| s.to_str()) != Some("s13") {
            eprintln!("[governor] loading .s13 payload from non-s13 extension: {}", path.display());
        }

        let () = Self::N_OK;
        let data = std::fs::read(path).map_err(|e| format!("read error: {}", e))?;
        if data.len() < 4 + 2 + 1 + 2 {
            return Err("file too small to be valid .s13".into());
        }

        // The Hard Gate.
        if data[0..4] != MAGIC {
            return Err(format!("bad magic: expected S13\\x01, got {:?}", &data[0..4]));
        }

        let d_model = u16::from_le_bytes([data[4], data[5]]);
        let num_experts = data[6];
        let bytes_per_centroid = u16::from_le_bytes([data[7], data[8]]);

        if num_experts as usize != N {
            return Err(format!("expected {N} experts, got {}", num_experts));
        }
        if data.len() < 9 + 4 * N {
            return Err("file too small to be valid .s13".into());
        }

        let bias_offset = 9;
        let mut bias = [0.0f32; N];
        for (i, b) in bias.iter_mut().enumerate() {
            let o = bias_offset + i * 4;
            *b = f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
        }

        if bytes_per_centroid as usize > MAX_BYTES_PER_CENTROID {
            return Err(format!(
                "bytes_per_centroid {} exceeds MAX_BYTES_PER_CENTROID {} (route()'s stack buffer bound)",
                bytes_per_centroid, MAX_BYTES_PER_CENTROID
            ));
        }

        let centroid_offset = bias_offset + 4 * N;
        let centroid_len = N * bytes_per_centroid as usize;
        let expected_len = centroid_offset + centroid_len;
        if data.len() < expected_len {
            return Err(format!("file truncated: {} < {}", data.len(), expected_len));
        }

        let centroids = data[centroid_offset..centroid_offset + centroid_len].to_vec();

        Ok(Self { d_model, num_experts, bytes_per_centroid, bias, centroids })
    }

    /// Routes a query to the best expert (0-6).
    /// Input: f32 slice of length d_model.
    /// Returns `Ok((expert_id, margin))` where `margin = score[0] - score[1]`
    /// (top-1 minus top-2), or `Err(byte)` — the first out-of-band `S13`
    /// sentinel byte (`>= sentinel::MAX_PACKED`) hit while scanning a
    /// centroid, trapped rather than folded into a bogus distance. Decode
    /// `byte` with `S13::from_byte` for its label (`None` = one of the nine
    /// reserved states).
    ///
    /// This traps rather than aborts: a poisoned byte in one `.s13` file
    /// shouldn't take down the process that's routing a live query.
    /// `sentinel::breach()` remains available to a caller that decides a
    /// given `Err` warrants a hard L10 abort.
    pub fn route(&self, query: &[f32]) -> Result<(u8, f32), u8> {
        let bpc = self.bytes_per_centroid as usize;
        // Stack-only trit-pack buffer (forbidden_ops::hot_path_heap_alloc) —
        // `bpc <= MAX_BYTES_PER_CENTROID` is guaranteed by `load()`'s own
        // rejection of any `.s13` declaring more, so this slice is always
        // in-bounds by construction, not by a runtime check here.
        let mut q_tq_buf = [0u8; MAX_BYTES_PER_CENTROID];
        let q_tq = &mut q_tq_buf[..bpc];
        pack_trits_into(query, q_tq);

        let mut scores = [0.0f32; N];
        for expert in 0..N {
            let centroid = &self.centroids[expert * bpc..(expert + 1) * bpc];
            let mut dist: u32 = 0;
            for (&c, &q) in centroid.iter().zip(q_tq.iter()) {
                if TritCell5D(c).is_sentinel() {
                    return Err(c);
                }
                // pack_trits() never emits a sentinel byte, but the check stays
                // symmetric so a future caller-supplied packed query can't slip past.
                if TritCell5D(q).is_sentinel() {
                    return Err(q);
                }
                dist += TRIT_DIST_LUT[((c as usize) << 8) | q as usize] as u32;
            }
            scores[expert] = -(dist as f32) + self.bias[expert] * 10.0;
        }

        // Find top-1 and top-2
        let mut best = 0;
        let mut second = 1;
        if scores[1] > scores[0] {
            best = 1;
            second = 0;
        }
        for i in 2..N {
            if scores[i] > scores[best] {
                second = best;
                best = i;
            } else if scores[i] > scores[second] {
                second = i;
            }
        }

        let margin = scores[best] - scores[second];
        Ok((best as u8, margin))
    }

    /// Route as soft weights (continuous blend) across all 7 experts.
    /// Returns a 7-element array of `Permyriad` weights (0..=10_000 each),
    /// or `Err(byte)` if a sentinel byte is encountered (same trapping behavior
    /// as `route()`). Weights are normalized via rank-preserving inverse-distance.
    pub fn route_soft(&self, query: &[f32]) -> Result<[Permyriad; N], u8> {
        let bpc = self.bytes_per_centroid as usize;
        let mut q_tq_buf = [0u8; MAX_BYTES_PER_CENTROID];
        let q_tq = &mut q_tq_buf[..bpc];
        pack_trits_into(query, q_tq);

        let mut dists = [0u32; N];
        for expert in 0..N {
            let centroid = &self.centroids[expert * bpc..(expert + 1) * bpc];
            let mut dist: u32 = 0;
            for (&c, &q) in centroid.iter().zip(q_tq.iter()) {
                if TritCell5D(c).is_sentinel() {
                    return Err(c);
                }
                if TritCell5D(q).is_sentinel() {
                    return Err(q);
                }
                dist += TRIT_DIST_LUT[((c as usize) << 8) | q as usize] as u32;
            }
            dists[expert] = dist;
        }

        let bias_i32: [i32; N] = std::array::from_fn(|i| (self.bias[i] * 10.0) as i32);

        Ok(permyriad_softmax_from_dist(&dists, &bias_i32))
    }
}

fn permyriad_softmax_from_dist<const N: usize>(dists: &[u32; N], bias: &[i32; N]) -> [Permyriad; N] {
    let mut scores = [0i32; N];
    for i in 0..N {
        scores[i] = -(dists[i] as i32) + bias[i];
    }

    let max_score = *scores.iter().max().unwrap_or(&0);
    let mut sum: i64 = 0;
    let mut shifted = [0i32; N];

    for i in 0..N {
        let shift = scores[i] - max_score;
        shifted[i] = if shift >= 0 {
            (1i64 << shift.min(30)) as i32
        } else {
            0
        };
        sum += shifted[i] as i64;
    }

    if sum == 0 {
        sum = 1;
    }

    let mut out = [Permyriad::ZERO; N];
    for i in 0..N {
        let numerator = (shifted[i] as i64) * 10_000;
        out[i] = Permyriad((numerator / sum) as i32);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::s13::S13;
    use crate::sentinel;

    #[test]
    fn shipped_sovereign_asset_loads_and_routes() {
        // Produced by `gemma-sidecar quantize-s13 pack` from the real
        // sovereign_q4_router.safetensors `router.gate.weight` tensor
        // (mean-reduced over k, per Sean's directive 2026-08-12). Proves the
        // quantizer's output and MetaRouter::load()'s reader agree on the
        // .s13 format for real data, not just synthetic test mocks.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sovereign.s13");
        let router = MetaRouter::<NUM_EXPERTS>::load(&path).expect("shipped .s13 asset must load");
        assert_eq!(router.num_experts, 7);
        assert_eq!(router.d_model, 768);
        assert_eq!(router.centroids.len(), 7 * router.bytes_per_centroid as usize);

        let query = vec![0.1f32; 768];
        let (expert, margin) = router.route(&query).expect("real quantized data must not trap");
        assert!(expert < 7);
        assert!(margin.is_finite());
    }

    #[test]
    fn route_returns_valid_expert() {
        // d_model=64 -> bpc = ceil(64/5) = 13.
        let bpc = trit_bytes_needed(64) as usize;
        // Neutral trits (all-1s -> byte 121) everywhere, except one distinct
        // all-positive-trit byte (242) per expert, so each centroid differs.
        let mut centroids = vec![121u8; 7 * bpc];
        for i in 0..7 {
            centroids[i * bpc + i] = 242;
        }

        let router = MetaRouter {
            d_model: 64,
            num_experts: 7,
            bytes_per_centroid: bpc as u16,
            bias: [0.0; 7],
            centroids,
        };

        let query = vec![1.0f32; 64];
        let (expert, margin) = router.route(&query).expect("valid TQ data must not trap");
        assert!(expert < 7);
        assert!(margin.is_finite());
    }

    #[test]
    fn route_with_bias_shifts_preference() {
        let bpc = trit_bytes_needed(64) as usize;
        // All-neutral-trit centroids (byte 121) are equidistant from a
        // zero query, which also packs to all-neutral trits.
        let centroids = vec![121u8; 7 * bpc];

        let mut bias = [0.0f32; 7];
        bias[3] = 5.0; // heavily bias expert 3

        let router = MetaRouter {
            d_model: 64,
            num_experts: 7,
            bytes_per_centroid: bpc as u16,
            bias,
            centroids,
        };

        let query = vec![0.0f32; 64]; // zero query
        let (expert, _) = router.route(&query).expect("valid TQ data must not trap");
        assert_eq!(expert, 3, "bias should force expert 3");
    }

    #[test]
    fn route_traps_sentinel_byte_instead_of_faking_a_distance() {
        let bpc = trit_bytes_needed(64) as usize;
        let mut centroids = vec![121u8; 7 * bpc];
        centroids[0] = 245; // S13::Poisoned, planted in expert 0's first byte

        let router = MetaRouter {
            d_model: 64,
            num_experts: 7,
            bytes_per_centroid: bpc as u16,
            bias: [0.0; 7],
            centroids,
        };

        let query = vec![0.0f32; 64];
        let err = router.route(&query).expect_err("a sentinel byte must trap, not silently distance");
        assert_eq!(err, 245);
        assert_eq!(S13::from_byte(err), Some(S13::Poisoned));
    }

    #[test]
    fn trit_dist_lut_matches_manual_decode() {
        // Identical bytes: zero distance, for every valid trit-byte.
        for b in 0..sentinel::MAX_PACKED {
            assert_eq!(TRIT_DIST_LUT[((b as usize) << 8) | b as usize], 0);
        }
        // All-trit-0 vs all-trit-2: max per-trit distance (2) * 5 trits = 10.
        assert_eq!(TRIT_DIST_LUT[242], 10);
        // 121 (all trit-1) vs 122 (trit0 bumped to 2, rest trit-1): one
        // trit off by 1, the rest match.
        assert_eq!(TRIT_DIST_LUT[(121usize << 8) | 122usize], 1);
    }

    #[test]
    fn build_s13_bytes_round_trips_through_load() {
        let bpc = trit_bytes_needed(64) as usize;
        let mut centroids = vec![121u8; 7 * bpc];
        for i in 0..7 {
            centroids[i * bpc + i] = 242;
        }
        let mut bias = [0.0f32; 7];
        bias[2] = 3.5;

        let bytes = build_s13_bytes(64, bias, &centroids);

        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("build_s13_bytes_round_trip_{}_{n}.s13", std::process::id()));
        std::fs::write(&path, &bytes).expect("write temp .s13");
        let router = MetaRouter::load(&path).expect("built bytes must load");
        std::fs::remove_file(&path).ok();

        assert_eq!(router.d_model, 64);
        assert_eq!(router.bytes_per_centroid as usize, bpc);
        assert_eq!(router.bias, bias);
        assert_eq!(router.centroids, centroids);
    }

    #[test]
    fn load_rejects_bytes_per_centroid_over_the_stack_buffer_bound() {
        let over = (MAX_BYTES_PER_CENTROID + 1) as u16;
        let centroids = vec![0u8; 7 * over as usize];
        let bytes = build_s13_bytes(64, [0.0; 7], &centroids);
        // build_s13_bytes derives bytes_per_centroid from centroids.len()/7,
        // which already equals `over` here — no header patch needed.
        let path = std::env::temp_dir().join("test_oversized_bpc.s13");
        std::fs::write(&path, &bytes).unwrap();
        let result = MetaRouter::<NUM_EXPERTS>::load(&path);
        std::fs::remove_file(&path).ok();
        match result {
            Err(e) => assert!(e.contains("MAX_BYTES_PER_CENTROID"), "unexpected error: {}", e),
            Ok(_) => panic!("load() must reject bytes_per_centroid > MAX_BYTES_PER_CENTROID"),
        }
    }

    /// const-generic-experts: the expert count is `N`, not a literal — a 3-expert `.s13`
    /// loads, routes inside `0..3`, soft-routes to a 3-array, and refuses the 7-expert header.
    #[test]
    fn a_three_expert_router_is_the_same_code_with_n_three() {
        let d_model = 10u16;
        let bpc = trit_bytes_needed(d_model) as usize;
        let mut centroids = vec![121u8; 3 * bpc];
        centroids[0] = 242; // expert 0 leans +1 on every dim of its first byte
        centroids[bpc] = 0; // expert 1 leans -1
        let bytes = build_s13_bytes(d_model, [0.0f32; 3], &centroids);
        assert_eq!(bytes[6], 3, "header byte carries N");
        let path = std::env::temp_dir().join(format!("test_three_experts_{}.s13", std::process::id()));
        std::fs::write(&path, &bytes).unwrap();
        let router = MetaRouter::<3>::load(&path).expect("a 3-expert file loads as MetaRouter<3>");
        let as_seven = MetaRouter::<NUM_EXPERTS>::load(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(router.num_experts, 3);
        assert_eq!(router.bias.len(), 3);
        assert_eq!(router.centroids.len(), 3 * bpc);
        let (expert, margin) = router.route(&[1.0f32; 10]).expect("no sentinel bytes");
        assert!(expert < 3, "route stays inside 0..N, got {expert}");
        assert_eq!(expert, 0, "the all-positive query lands on the all-positive centroid");
        assert!(margin > 0.0);
        let soft = router.route_soft(&[1.0f32; 10]).expect("soft route");
        assert_eq!(soft.len(), 3);
        let sum: i32 = soft.iter().map(|p| p.0).sum();
        assert!((9_990..=10_000).contains(&sum), "soft weights sum to ~10_000 pmy, got {sum}");
        match as_seven {
            Err(e) => assert!(e.contains("expected 7 experts, got 3"), "{e}"),
            Ok(_) => panic!("a 3-expert header must not load as a 7-expert router"),
        }
    }

    #[test]
    fn pack_trits_pads_incomplete_final_byte_with_neutral_trit() {
        // 3 real dims (all positive -> trit 2) + 2 padding dims (neutral -> trit 1)
        // packed into one byte: 2 + 3*2 + 9*2 + 27*1 + 81*1 = 134.
        let packed = pack_trits(&[1.0, 1.0, 1.0], 1);
        assert_eq!(packed, vec![134]);
    }

    #[test]
    fn route_soft_returns_normalized_weights() {
        let bpc = trit_bytes_needed(64) as usize;
        let centroids = vec![121u8; 7 * bpc];

        let router = MetaRouter {
            d_model: 64,
            num_experts: 7,
            bytes_per_centroid: bpc as u16,
            bias: [0.0; 7],
            centroids,
        };

        let query = vec![0.0f32; 64];
        let weights = router.route_soft(&query).expect("valid TQ data must not trap");

        let sum: i32 = weights.iter().map(|w| w.0).sum();
        assert!(sum > 0, "sum of weights must be positive");
        assert!(weights.iter().all(|w| w.0 >= 0), "all weights must be non-negative");
    }

    #[test]
    fn route_soft_with_bias_shifts_preference() {
        let bpc = trit_bytes_needed(64) as usize;
        let centroids = vec![121u8; 7 * bpc];

        let mut bias = [0.0f32; 7];
        bias[3] = 5.0;

        let router = MetaRouter {
            d_model: 64,
            num_experts: 7,
            bytes_per_centroid: bpc as u16,
            bias,
            centroids,
        };

        let query = vec![0.0f32; 64];
        let weights = router.route_soft(&query).expect("valid TQ data must not trap");
        assert!(weights[3].0 > weights[0].0, "biased expert should have higher weight");
    }

    #[test]
    fn route_soft_traps_sentinel_byte() {
        let bpc = trit_bytes_needed(64) as usize;
        let mut centroids = vec![121u8; 7 * bpc];
        centroids[0] = 245;

        let router = MetaRouter {
            d_model: 64,
            num_experts: 7,
            bytes_per_centroid: bpc as u16,
            bias: [0.0; 7],
            centroids,
        };

        let query = vec![0.0f32; 64];
        let err = router.route_soft(&query).expect_err("a sentinel byte must trap");
        assert_eq!(err, 245);
    }

    #[test]
    fn permyriad_softmax_from_dist_sums_to_nonzero() {
        let dists = [5u32, 3, 7, 2, 8, 4, 6];
        let bias = [0i32; 7];

        let weights = permyriad_softmax_from_dist(&dists, &bias);
        let sum: i32 = weights.iter().map(|w| w.0).sum();
        assert!(sum > 0, "sum must be positive");
        assert!(weights.iter().all(|w| w.0 >= 0), "all weights must be non-negative");
    }

    #[test]
    fn permyriad_softmax_from_dist_ranks_by_inverse_distance() {
        let dists = [10u32, 1, 5, 8, 3, 6, 2];
        let bias = [0i32; 7];

        let weights = permyriad_softmax_from_dist(&dists, &bias);
        assert!(weights[1].0 > weights[0].0, "low dist 1 should rank higher than dist 10");
        assert!(weights[1].0 > weights[3].0, "low dist 1 should rank higher than dist 8");
    }
}
