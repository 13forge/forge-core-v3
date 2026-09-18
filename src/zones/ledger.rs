#![deny(missing_docs)]
//! Append-only, tick-bounded mutation ledger for cell edits.
//!
//! Records the before/after state of every Pexil mutation with a deterministic seal,
//! enabling tamper-evident playback and audit trails. Hybrid gate: always appends a
//! 2-bit triage class (LANDMARK/NEUTRAL/DIFFUSE) computed via NIPR; appends the full
//! mutation payload only when `is_landmark()` is true.

use crate::atom::{CellOrdinal, Pexil};

/// Mutation localization triage: balanced ternary {-1, 0, +1}.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriageClass {
    /// Diffuse / noisy (NIPR < DIFFUSE_PMY=2500).
    Diffuse = 0,
    /// Neutral / equilibrium (2500 <= NIPR < LANDMARK_PMY=7500).
    Neutral = 1,
    /// Landmark / active (NIPR >= LANDMARK_PMY=7500).
    Landmark = 2,
}

const LANDMARK_PMY: u16 = 7500;
const DIFFUSE_PMY: u16 = 2500;
const PERMYRIAD_SCALE: u128 = 10_000;

/// One row in the mutation ledger.
///
/// Each row records a single cell mutation: the ordinal being changed, the engine tick
/// it occurred on, the before and after states, and a deterministic seal computed from
/// these fields to detect tampering. Triage class (LANDMARK/NEUTRAL/DIFFUSE) is always
/// recorded; full before/after payload is stored only for landmarks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerRow {
    /// The cell being mutated.
    pub ordinal: CellOrdinal,
    /// The engine tick this mutation occurred on.
    pub tick: u64,
    /// The cell state before the mutation (stored only when triage is Landmark; may be zeroed otherwise).
    pub before: Pexil,
    /// The cell state after the mutation (stored only when triage is Landmark; may be zeroed otherwise).
    pub after: Pexil,
    /// World coordinate `(x, y, z, w)` this mutation happened at — NOT derivable
    /// from `ordinal` (every cell starts `CellOrdinal(0)` and nothing in this
    /// crate's brushes ever assigns it a per-cell-unique value), so undo/redo
    /// needs this field to know where to write `before`/`after` back.
    pub world: (usize, usize, usize, i8),
    /// FNV-1a-derived seal of `(ordinal, tick, before, after, world)`. Deterministic
    /// and independent of insertion order; tampering with any field breaks this hash.
    pub seal: [u8; 32],
    /// NIPR-based triage class: Landmark, Neutral, or Diffuse.
    pub triage: TriageClass,
}

/// An append-only ledger of cell mutations.
///
/// Stores the history of every Pexil edit in order. Each append is O(1) in wall-clock time
/// (only one Vec push) and produces a deterministic seal without allocation or wall-clock
/// calls.
#[derive(Debug, Clone)]
pub struct MutationLedger {
    /// All mutation rows, in insertion order. Sealed on append, never reordered.
    rows: Vec<LedgerRow>,
}

impl MutationLedger {
    /// Create a fresh, empty ledger.
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Append one mutation row to the ledger.
    ///
    /// Computes the row's seal deterministically from its fields (ordinal, tick, before,
    /// after, world) using FNV-1a hashing. Triages the mutation via NIPR over the payload
    /// delta. Always pushes the triage class (~30 B/s at 120 Hz); stores full before/after
    /// only when triage is Landmark. Non-landmark rows have before/after zeroed.
    pub fn append(&mut self, ordinal: CellOrdinal, tick: u64, before: Pexil, after: Pexil, world: (usize, usize, usize, i8)) {
        let seal = compute_seal(ordinal, tick, before, after, world);
        let triage = compute_triage(&before, &after);

        let (stored_before, stored_after) = if triage == TriageClass::Landmark {
            (before, after)
        } else {
            // Neutral and Diffuse mutations: zero the before/after to save space.
            // Triage class and seal are always preserved for sequence integrity.
            let zero_pexil = Pexil {
                lattice: before.lattice,
                validity: before.validity,
                ordinal: before.ordinal,
                payload: [0u8; 4],
            };
            (zero_pexil, zero_pexil)
        };

        let row = LedgerRow {
            ordinal,
            tick,
            before: stored_before,
            after: stored_after,
            world,
            seal,
            triage,
        };
        self.rows.push(row);
    }

    /// How many mutations this ledger has recorded.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// True if no mutations have been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Borrow all rows in insertion order.
    pub fn rows(&self) -> &[LedgerRow] {
        &self.rows
    }

    /// Return the deterministic seal of the ledger state (the latest row's seal, or zeroed if empty).
    pub fn seal(&self) -> [u8; 32] {
        self.rows.last().map(|r| r.seal).unwrap_or([0u8; 32])
    }
}

impl Default for MutationLedger {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the FNV-1a-based seal for a row.
///
/// Uses FNV-1a algorithm (from `crate::checksum`) to derive four u64 hashes from the input data,
/// ensuring all fields (ordinal, tick, before, after, world) contribute to the entire 32-byte seal.
fn compute_seal(ordinal: CellOrdinal, tick: u64, before: Pexil, after: Pexil, world: (usize, usize, usize, i8)) -> [u8; 32] {
    use crate::checksum::FNV_OFFSET_BASIS;
    // FNV-1a prime (same value as `crate::checksum`'s private constant)
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    // Fold each field through FNV-1a to generate 4 different hashes with different
    // reduction orders, so that changing any input affects multiple output chunks.
    let mut h0 = FNV_OFFSET_BASIS;
    let mut h1 = FNV_OFFSET_BASIS;
    let mut h2 = FNV_OFFSET_BASIS;
    let mut h3 = FNV_OFFSET_BASIS;

    // Fold ordinal bytes
    for &b in &ordinal.0.to_le_bytes() {
        h0 ^= b as u64;
        h0 = h0.wrapping_mul(FNV_PRIME);
        h1 ^= b.rotate_right(1) as u64;
        h1 = h1.wrapping_mul(FNV_PRIME);
        h2 ^= b.rotate_right(2) as u64;
        h2 = h2.wrapping_mul(FNV_PRIME);
        h3 ^= b.rotate_right(3) as u64;
        h3 = h3.wrapping_mul(FNV_PRIME);
    }

    // Fold tick bytes
    for &b in &tick.to_le_bytes() {
        h0 ^= b as u64;
        h0 = h0.wrapping_mul(FNV_PRIME);
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(FNV_PRIME);
        h2 ^= b.rotate_right(1) as u64;
        h2 = h2.wrapping_mul(FNV_PRIME);
        h3 ^= b.rotate_right(1) as u64;
        h3 = h3.wrapping_mul(FNV_PRIME);
    }

    // Fold before bytes
    let mut before_bytes = [0u8; 8];
    pack_pexil(&before, &mut before_bytes);
    for &b in &before_bytes {
        h0 ^= b as u64;
        h0 = h0.wrapping_mul(FNV_PRIME);
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(FNV_PRIME);
        h2 ^= b as u64;
        h2 = h2.wrapping_mul(FNV_PRIME);
        h3 ^= b.rotate_right(2) as u64;
        h3 = h3.wrapping_mul(FNV_PRIME);
    }

    // Fold after bytes — these must affect all hashes distinctively
    let mut after_bytes = [0u8; 8];
    pack_pexil(&after, &mut after_bytes);
    for &b in &after_bytes {
        h0 ^= b as u64;
        h0 = h0.wrapping_mul(FNV_PRIME);
        h1 ^= b.rotate_right(2) as u64;
        h1 = h1.wrapping_mul(FNV_PRIME);
        h2 ^= b as u64;
        h2 = h2.wrapping_mul(FNV_PRIME);
        h3 ^= b as u64;
        h3 = h3.wrapping_mul(FNV_PRIME);
    }

    // Fold world-coordinate bytes — the field undo/redo actually needs, must be
    // covered by the seal like every other field.
    for &b in &world.0.to_le_bytes() {
        h0 ^= b as u64;
        h0 = h0.wrapping_mul(FNV_PRIME);
    }
    for &b in &world.1.to_le_bytes() {
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(FNV_PRIME);
    }
    for &b in &world.2.to_le_bytes() {
        h2 ^= b as u64;
        h2 = h2.wrapping_mul(FNV_PRIME);
    }
    h3 ^= world.3 as u8 as u64;
    h3 = h3.wrapping_mul(FNV_PRIME);

    // Pack the four u64 hashes into 32 bytes
    let mut seal = [0u8; 32];
    seal[0..8].copy_from_slice(&h0.to_le_bytes());
    seal[8..16].copy_from_slice(&h1.to_le_bytes());
    seal[16..24].copy_from_slice(&h2.to_le_bytes());
    seal[24..32].copy_from_slice(&h3.to_le_bytes());
    seal
}

/// Pack a Pexil into 8 bytes in repr(C) order.
fn pack_pexil(pexil: &Pexil, out: &mut [u8; 8]) {
    out[0] = pexil.lattice.0;
    out[1] = pexil.validity.0;
    out[2..4].copy_from_slice(&pexil.ordinal.0.to_le_bytes());
    out[4..8].copy_from_slice(&pexil.payload);
}

/// Compute the Normalized Inverse Participation Ratio (NIPR) over a payload delta.
/// Returns the permyriad value (0..=10000), computed via the formula:
/// N × IPR = (N·S2 − S1²) / ((N−1)·S1²) × 10000, where S1 = Σv and S2 = Σv².
fn compute_nipr(before: &Pexil, after: &Pexil) -> u16 {
    let n = 4u128; // Payload is 4 bytes

    // Compute S1 (sum of absolute changes) and S2 (sum of squared changes).
    let mut s1: u128 = 0;
    let mut s2: u128 = 0;

    for i in 0..4 {
        let delta = (after.payload[i] as i16 - before.payload[i] as i16).abs() as u128;
        s1 += delta;
        s2 += delta * delta;
    }

    if s1 == 0 || n == 0 {
        return 0;
    }

    if n == 1 {
        return 10_000;
    }

    // NIPR formula: (N·S2 − S1²) / ((N−1)·S1²) × 10000
    let s1_sq = s1 * s1;
    let numerator = (n * s2 - s1_sq) * PERMYRIAD_SCALE;
    let denominator = (n - 1) * s1_sq;

    let pmy = ((numerator / denominator).min(10_000)) as u16;
    pmy
}

/// Triage a mutation based on NIPR thresholds: Landmark (>= 7500), Neutral (2500..7500), Diffuse (< 2500).
fn compute_triage(before: &Pexil, after: &Pexil) -> TriageClass {
    let pmy = compute_nipr(before, after);
    match pmy {
        p if p >= LANDMARK_PMY => TriageClass::Landmark,
        p if p >= DIFFUSE_PMY => TriageClass::Neutral,
        _ => TriageClass::Diffuse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atom::{CellOrdinal, TritCell5D, ValidityMask};

    fn make_pexil(payload: u32) -> Pexil {
        Pexil {
            lattice: TritCell5D::ORIGIN,
            validity: ValidityMask(0),
            ordinal: CellOrdinal(1),
            payload: payload.to_le_bytes(),
        }
    }

    #[test]
    fn append_monotonic_growth() {
        let mut ledger = MutationLedger::new();
        assert_eq!(ledger.len(), 0);
        assert!(ledger.is_empty());

        let before = make_pexil(0);
        let after1 = make_pexil(1);
        let after2 = make_pexil(2);
        let after3 = make_pexil(3);

        ledger.append(CellOrdinal(0), 100, before, after1, (0, 0, 0, 0));
        assert_eq!(ledger.len(), 1);

        ledger.append(CellOrdinal(1), 101, before, after2, (1, 0, 0, 0));
        assert_eq!(ledger.len(), 2);

        ledger.append(CellOrdinal(2), 102, before, after3, (2, 0, 0, 0));
        assert_eq!(ledger.len(), 3);

        assert!(!ledger.is_empty());
    }

    #[test]
    fn seal_determinism() {
        let before = make_pexil(0);
        let after = make_pexil(42);

        let seal1 = compute_seal(CellOrdinal(5), 200, before, after, (0, 0, 0, 0));
        let seal2 = compute_seal(CellOrdinal(5), 200, before, after, (0, 0, 0, 0));

        assert_eq!(seal1, seal2, "identical inputs must produce identical seals");
    }

    #[test]
    fn seal_content_sensitivity() {
        let before = make_pexil(0);
        let after_a = make_pexil(10);
        let after_b = make_pexil(11);

        let seal_a = compute_seal(CellOrdinal(7), 300, before, after_a, (0, 0, 0, 0));
        let seal_b = compute_seal(CellOrdinal(7), 300, before, after_b, (0, 0, 0, 0));

        assert_ne!(
            seal_a, seal_b,
            "differing payload in after must produce different seals"
        );
    }

    #[test]
    fn seal_covers_the_world_coordinate() {
        let before = make_pexil(0);
        let after = make_pexil(10);

        let seal_a = compute_seal(CellOrdinal(7), 300, before, after, (1, 2, 3, 0));
        let seal_b = compute_seal(CellOrdinal(7), 300, before, after, (1, 2, 3, -1));

        assert_ne!(
            seal_a, seal_b,
            "a world-coordinate mismatch alone must produce a different seal — the field \
             undo/redo relies on must be tamper-evident too"
        );
    }

    #[test]
    fn a_row_remembers_where_it_happened() {
        let mut ledger = MutationLedger::new();
        ledger.append(CellOrdinal(0), 1, make_pexil(0), make_pexil(9), (10, 20, 30, -1));
        assert_eq!(ledger.rows()[0].world, (10, 20, 30, -1));
    }
}
