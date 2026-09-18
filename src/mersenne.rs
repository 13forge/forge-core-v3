//! Seven Mersenne numbers that share a nickname and nothing else.
//!
//! **Bare `M31` is banned.** `2^5 - 1 = 31` and `2^31 - 1` are both spoken aloud as
//! "M31" and differ by eight orders of magnitude (HANDOFF §2.4). Wiring the first into
//! a fold written for the second destroys the hash silently — no panic, no wrong type,
//! just a different number. So every constant here is named by its **exponent**, every
//! one asserts `(1 << n) - 1` in its own width, and `tests::m5_is_not_m31` holds the
//! distance between the two.
//!
//! The five roles are unrelated to each other. Sharing the Mersenne form is not a
//! mechanism, and this file claims no connection between them.

/// `2^2 - 1`. The ternary radix. That 3 is Mersenne is coincidence, not mechanism.
pub const M2: u8 = 3;
/// `2^3 - 1`. 3D sub-cell presence mask — 7 neighbours of a cell face.
pub const M3: u8 = 7;
/// `2^5 - 1`. 5D sub-cell presence mask. **Not** `M31`.
pub const M5: u8 = 31;
/// `2^7 - 1`. The `i8::MAX` boundary, asserted below rather than asserted in prose.
pub const M7: u8 = 127;
/// `2^13 - 1`. 13-bit Hamming hypercube router. Unrelated to the 5D lattice.
pub const M13: u16 = 8_191;
/// `2^31 - 1`. 32-bit RAG hash prime. **Not** `M5`.
pub const M31: u32 = 2_147_483_647;
/// `2^61 - 1`. 64-bit Pexil register hash modulus — see [`reduce_m61`].
pub const M61: u64 = 2_305_843_009_213_693_951;

/// The exponents, in order. The name of each constant *is* its exponent, so this is
/// also the list of legal names.
pub const EXPONENTS: [u32; 7] = [2, 3, 5, 7, 13, 31, 61];

/// The shift that pairs with [`M61`]. Mask and shift are a matched pair — a fold that
/// takes one from `M61` and the other from another exponent is not a reduction at all.
pub const M61_SHIFT: u32 = 61;

/// Reduce `x` modulo `2^61 - 1`.
///
/// `2^61 ≡ 1 (mod M61)`, so `x = hi·2^61 + lo ≡ hi + lo`. One conditional subtraction
/// finishes it: `lo <= M61` and `hi <= 7`, so the fold cannot exceed `M61 + 7`.
///
/// The `>=` is load-bearing and not a `>`. `x == M61` folds to exactly `M61`, and a
/// strict comparison would return the modulus itself instead of zero — one value out of
/// `2^64` wrong, which is precisely the kind of defect that survives spot-checking.
/// Overflow of the *input* is not this function's business; `Sentinel::MersenneOverflow`
/// is the out-of-band state for that.
#[inline(always)]
pub const fn reduce_m61(x: u64) -> u64 {
    let folded = (x & M61) + (x >> M61_SHIFT);
    if folded >= M61 {
        folded - M61
    } else {
        folded
    }
}

// VALUE LOCKS. Each constant asserts `(1 << n) - 1` in its own width. A typo in a
// digit group, or a constant swapped for its namesake, fails `cargo check`.
const _: () = assert!(M2 == (1u8 << 2) - 1);
const _: () = assert!(M3 == (1u8 << 3) - 1);
const _: () = assert!(M5 == (1u8 << 5) - 1);
const _: () = assert!(M7 == (1u8 << 7) - 1);
const _: () = assert!(M13 == (1u16 << 13) - 1);
const _: () = assert!(M31 == (1u32 << 31) - 1);
const _: () = assert!(M61 == (1u64 << 61) - 1);

// WIDTH LOCKS. Each constant is held in the narrowest type its role needs; widening
// one is a change to a wire format, not a convenience.
const _: () = assert!(core::mem::size_of_val(&M2) == 1);
const _: () = assert!(core::mem::size_of_val(&M3) == 1);
const _: () = assert!(core::mem::size_of_val(&M5) == 1);
const _: () = assert!(core::mem::size_of_val(&M7) == 1);
const _: () = assert!(core::mem::size_of_val(&M13) == 2);
const _: () = assert!(core::mem::size_of_val(&M31) == 4);
const _: () = assert!(core::mem::size_of_val(&M61) == 8);

// The two that share a spoken name are not the same number.
const _: () = assert!(M5 as u64 != M31 as u64);
const _: () = assert!(M31 as u64 / M5 as u64 > 69_000_000);

// M7 is the signed byte boundary; stated as an assert, not a comment.
const _: () = assert!(M7 == i8::MAX as u8);
const _: () = assert!(EXPONENTS.len() == 7);

#[cfg(test)]
mod tests {
    use super::*;

    /// `(1 << n) - 1` for every exponent, walked as data rather than seven copies.
    #[test]
    fn every_constant_is_one_shifted_less_one() {
        let values: [u64; 7] =
            [M2 as u64, M3 as u64, M5 as u64, M7 as u64, M13 as u64, M31 as u64, M61];
        for (n, value) in EXPONENTS.iter().zip(values.iter()) {
            assert_eq!(*value, (1u64 << n) - 1, "M{n} is not 2^{n} - 1");
            assert_eq!(value.count_ones(), *n, "M{n} must be {n} set bits and nothing else");
            assert_eq!(value.trailing_ones(), *n);
        }
    }

    // The whole reason bare `M31` is banned: eight orders of magnitude, one nickname.
    #[test]
    fn m5_is_not_m31() {
        assert_ne!(M5 as u64, M31 as u64);
        assert_eq!(M5 as u64, 31);
        assert_eq!(M31 as u64 / M5 as u64, 69_273_666);
        // "Eight orders of magnitude" is a measurement, so it is measured.
        let orders = (M31 as f64).log10() - (M5 as f64).log10();
        assert!(orders > 7.8 && orders < 8.0, "{orders} orders apart");
    }

    // The failure HANDOFF §2.4 describes: substitute M5 for M61 in the mask and the
    // fold still runs, still returns a u64, and is no longer a reduction.
    #[test]
    fn the_wrong_mask_silently_stops_reducing() {
        fn sabotaged(x: u64) -> u64 {
            (x & M5 as u64) + (x >> M61_SHIFT)
        }
        let x = 0x1234_5678_9ABC_DEF0u64;
        assert_eq!(reduce_m61(x), x % M61);
        assert_ne!(sabotaged(x), x % M61);
        // It does not even stay inside its own modulus' range in any useful sense —
        // it is congruent to nothing the caller asked for.
        assert!(sabotaged(x) < M5 as u64 + 8);
        assert!(reduce_m61(x) > M5 as u64, "a real reduction lands in 0..M61, not 0..31");
    }

    #[test]
    fn reduce_m61_agrees_with_the_modulo_on_the_edges() {
        for x in [0u64, 1, 2, M61 - 2, M61 - 1, M61, M61 + 1, M61 + 2, u64::MAX, u64::MAX - 1] {
            assert_eq!(reduce_m61(x), x % M61, "x = {x}");
            assert!(reduce_m61(x) < M61);
        }
    }

    // The one value a non-conditional fold gets wrong.
    #[test]
    fn the_modulus_folds_to_zero_not_to_itself() {
        assert_eq!(reduce_m61(M61), 0);
        assert_eq!((M61 & M61) + (M61 >> M61_SHIFT), M61, "the raw fold really does return M61");
    }

    #[test]
    fn reduce_m61_agrees_with_the_modulo_over_a_deterministic_spread() {
        // A fixed LCG, not a random source — the same 100_000 inputs every run.
        let mut s = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..100_000 {
            s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            assert_eq!(reduce_m61(s), s % M61, "x = {s}");
        }
        // Powers of two either side of the shift, where the fold's two halves swap roles.
        for n in 0..64u32 {
            let x = 1u64 << n;
            assert_eq!(reduce_m61(x), x % M61, "x = 2^{n}");
        }
    }

    #[test]
    fn reduce_m61_is_additively_homomorphic() {
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..10_000 {
            s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let a = reduce_m61(s) % (M61 / 2);
            let b = reduce_m61(!s) % (M61 / 2);
            assert_eq!(reduce_m61(a + b), (a + b) % M61);
        }
    }

    /// Lucas–Lehmer, `u128` arithmetic: for odd prime `p`, `2^p - 1` is prime iff
    /// `s_{p-2} == 0` with `s_0 = 4`, `s_i = s_{i-1}^2 - 2 (mod M_p)`.
    /// Trial division cannot reach `M61`; this can, and it runs in microseconds.
    fn mersenne_is_prime(p: u32) -> bool {
        let m = (1u128 << p) - 1;
        if p == 2 {
            return true; // M2 = 3. Lucas–Lehmer is stated for odd p only.
        }
        let mut s = 4u128;
        for _ in 0..p - 2 {
            s = (s * s + m - 2) % m;
        }
        s == 0
    }

    #[test]
    fn every_named_mersenne_is_prime() {
        for n in EXPONENTS {
            assert!(mersenne_is_prime(n), "M{n} is not prime");
        }
    }

    // Lucas–Lehmer must be able to say no, or the test above proves nothing.
    #[test]
    fn lucas_lehmer_rejects_a_composite_mersenne() {
        // 2^11 - 1 = 2047 = 23 x 89, the classic counterexample to "prime p => prime M_p".
        assert!(!mersenne_is_prime(11));
        assert_eq!((1u32 << 11) - 1, 23 * 89);
        for n in [4, 6, 8, 9, 10, 11, 12] {
            assert!(!mersenne_is_prime(n), "M{n} must not pass");
            assert!(!EXPONENTS.contains(&n), "{n} is not a named exponent");
        }
    }

    #[test]
    fn m7_is_the_signed_byte_boundary() {
        assert_eq!(M7, i8::MAX as u8);
        assert_eq!(M7 as i16 + 1, i8::MAX as i16 + 1);
    }

    #[test]
    fn m5_masks_five_dimensions_and_m3_masks_three() {
        assert_eq!(M3.count_ones(), 3);
        assert_eq!(M5.count_ones(), 5);
        assert_eq!(M5 as usize, (1 << crate::atom::TRITS_PER_BYTE) - 1);
    }
}
