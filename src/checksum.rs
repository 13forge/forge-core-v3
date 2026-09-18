//! FNV-1a checksum for frame desync detection.
//!
//! Non-cryptographic, fast, zero-dependency. Suitable for frame integrity checks.

/// FNV-1a 64-bit offset basis — also the hash of empty input, and the seed a running
/// chain starts from.
pub const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64-bit prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a hash of a byte slice. Fast, non-cryptographic, sufficient for frame checksums.
pub fn hash_bytes_fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Fold one `u64` (little-endian bytes) into a running FNV-1a-64 hash.
///
/// The streaming sibling of [`hash_bytes_fnv1a`]: that one hashes a finished slice from the
/// basis, this one extends a chain a word at a time without ever materialising the slice.
/// Folding word-by-word is what lets a per-tick state chain stay allocation-free.
///
/// Shares [`FNV_OFFSET_BASIS`] and the prime with its sibling — one FNV home (L05).
#[inline]
pub fn fnv1a64_fold(mut hash: u64, word: u64) -> u64 {
    for b in word.to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let data = b"hello world";
        assert_eq!(hash_bytes_fnv1a(data), hash_bytes_fnv1a(data));
    }

    #[test]
    fn different_input_different_hash() {
        assert_ne!(hash_bytes_fnv1a(b"aaa"), hash_bytes_fnv1a(b"bbb"));
    }

    #[test]
    fn empty_input() {
        // Should return the offset basis
        assert_eq!(hash_bytes_fnv1a(b""), 0xcbf29ce484222325);
    }
}
