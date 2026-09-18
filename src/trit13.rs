//! Single-word 13-trit balanced-ternary packer. Sibling to `atom::TritCell5D`'s
//! 5-trit byte, same law (`PARARITY.md`), one radix, wider word: `3^13 =
//! 1_594_323` fits inside 21 bits of a `u32`.

/// Trits packed per word.
pub const TRITS_PER_WORD: usize = 13;

const POW3: [u32; 13] =
    [1, 3, 9, 27, 81, 243, 729, 2_187, 6_561, 19_683, 59_049, 177_147, 531_441];

/// Encode 13 balanced trits (`{-1,0,1}` each) into one `u32`, `0..=1_594_322`.
/// Digits outside `-1..=1` are a programming fault.
#[inline(always)]
pub const fn pack13(t: &[i8; TRITS_PER_WORD]) -> u32 {
    debug_assert!(
        t[0] >= -1 && t[0] <= 1 && t[1] >= -1 && t[1] <= 1 && t[2] >= -1 && t[2] <= 1
            && t[3] >= -1 && t[3] <= 1 && t[4] >= -1 && t[4] <= 1 && t[5] >= -1 && t[5] <= 1
            && t[6] >= -1 && t[6] <= 1 && t[7] >= -1 && t[7] <= 1 && t[8] >= -1 && t[8] <= 1
            && t[9] >= -1 && t[9] <= 1 && t[10] >= -1 && t[10] <= 1 && t[11] >= -1 && t[11] <= 1
            && t[12] >= -1 && t[12] <= 1
    );
    let mut word = 0u32;
    let mut i = 0;
    while i < TRITS_PER_WORD {
        word += (t[i] + 1) as u32 * POW3[i];
        i += 1;
    }
    word
}

/// Decode a packed word (`0..=1_594_322`) back to its 13 balanced trits.
#[inline(always)]
pub const fn unpack13(word: u32) -> [i8; TRITS_PER_WORD] {
    debug_assert!(word <= 1_594_322);
    let mut out = [0i8; TRITS_PER_WORD];
    let mut v = word;
    let mut i = 0;
    while i < TRITS_PER_WORD {
        out[i] = (v % 3) as i8 - 1;
        v /= 3;
        i += 1;
    }
    out
}

const _: () = assert!(POW3[12] == 531_441);
const _: () = assert!(POW3[12] * 3 - 1 == 1_594_322);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_is_a_bijection_over_interior_and_edges() {
        let cases: [[i8; TRITS_PER_WORD]; 4] = [
            [0; 13],
            [-1; 13],
            [1; 13],
            [1, -1, 0, 1, -1, 0, 1, -1, 0, 1, -1, 0, 1],
        ];
        for t in cases {
            assert_eq!(unpack13(pack13(&t)), t, "pack13/unpack13 must round-trip");
        }
    }

    #[test]
    fn packed_range_matches_three_pow_thirteen() {
        assert_eq!(pack13(&[-1; 13]), 0);
        assert_eq!(pack13(&[1; 13]), 1_594_322);
    }
}
