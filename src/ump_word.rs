//! `UmpWord` — the 16-byte MoM (Mixture of Musicians) routing word.
//!
//! Ported from `F:\NewRepo\crates\forge-core\src\ump.rs` (2026-08-14, the
//! `mom_router` port). Scope cut (C09/L15, named not silent): only the
//! `UmpWord` type, its 4 event-family encoders, and their own tests cross —
//! `RoutingTag`/`RoutedUmp` (need `crate::scene::MaterialId`, unrelated to
//! routing) and `packet::Ump`/`event::UmpEvent` (the DIFFERENT 16-byte MIDI
//! wire packet, already ported here as `spine::packet::Ump`, `spine.rs:619+`
//! — porting it again under this name would be an L05 second home) do NOT
//! cross. `UmpWord` is the MoM *routing* word; `spine::packet::Ump` is the
//! MIDI 2.0 *wire* packet — same byte width, different faces, v2's own doc
//! comment already names the distinction.
//!
//! Integer-only throughout (XOR + POPCNT via Hamming distance in the
//! consumer, `forge-hal-clockspine::mom_router`), no float, no alloc.

/// The UMP transport-word width in bytes — pinned equal to
/// `forge_hal_clockspine::expert_pool::MOE_QUERY_BYTES` counterpart, the
/// `QUERY_BYTES=16` instantiation `mom_router.rs` uses.
pub const UMP_WORD_BYTES: usize = 16;

/// Pure MoM routing transport word — 128 bits. Feeds directly into
/// `forge_hal_clockspine::expert_pool::MoeRouter<_, 16, _>` as the query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UmpWord(pub [u8; UMP_WORD_BYTES]);

// ---------------------------------------------------------------------------
// Event-family encoders — DAPS sonification (v2 session 2026-07-03 §D.1).
//
// Word layout:
//   bytes 0..8   family signature (repetition code). Hamming distance is
//                additive per byte, so the payload can only ADD distance —
//                any two families sit >= 32 bits apart by construction.
//   bytes 8..16  payload, discriminative bits spread through SPREAD codes.
//                No timestamp, no dead padding (Hamming-separability law).
// ---------------------------------------------------------------------------

/// Family signature byte — CST syllabic voices.
pub const FAMILY_CST_VOICE: u8 = 0x35;
/// Family signature byte — physics events. Bitwise complement of
/// [`FAMILY_CST_VOICE`]: 64-bit signature separation.
pub const FAMILY_PHYSICS: u8 = 0xCA;
/// Family signature byte — mixer health. >= 4 bits from both other
/// signatures (>= 32-bit floor over the region).
pub const FAMILY_ROADIE: u8 = 0x56;
/// Family signature byte — PTY route decisions. Bitwise complement of
/// [`FAMILY_ROADIE`]: 64-bit separation from it, 4 bits from each other.
pub const FAMILY_PTY_ROUTE: u8 = 0xA9;

/// Spread codes for 3-bit quantized payload fields. Every pair is >= 4 bits
/// apart, so a one-bucket quantization step survives POPCNT routing instead
/// of vanishing into a single flipped bit.
const SPREAD: [u8; 8] = [0x0F, 0xF0, 0x33, 0xCC, 0x55, 0xAA, 0x69, 0x96];

impl UmpWord {
    /// Encode a CST node voice from its fields. Raw MIDI note bytes carry
    /// Hamming cliffs, so pitch REGION routes through a spread code and the
    /// raw pitch class (`note % 12`) stays as fine within-tier detail.
    pub fn from_node_voice(material_id: u8, note: u8, voice: u8) -> Self {
        let mat_code = SPREAD[(material_id.min(10) as usize * 7) / 10];
        let note_code = SPREAD[(note.saturating_sub(36) as usize / 8).min(7)];
        let voice_code = SPREAD[(voice & 0x07) as usize];
        let pitch_class = note % 12;
        let mut w = [FAMILY_CST_VOICE; UMP_WORD_BYTES];
        w[8..16].copy_from_slice(&[
            mat_code, note_code, voice_code, pitch_class, mat_code, note_code, voice_code, pitch_class,
        ]);
        Self(w)
    }

    /// Encode a physics audio event. `kind` is the event-kind discriminant
    /// (`as u8`; 7 variants — the natural sub-family axis). No timestamp, no
    /// position (no-timestamp law).
    pub fn from_physics_event(kind: u8, material_hash: u64, impulse_q: i32, resonance_hz: i32) -> Self {
        let mat = (material_hash ^ (material_hash >> 16) ^ (material_hash >> 32) ^ (material_hash >> 48)) as u16;
        let [mat_hi, mat_lo] = mat.to_be_bytes();
        let kind_code = SPREAD[(kind & 0x07) as usize];
        let imp_code = SPREAD[impulse_bucket(impulse_q) as usize];
        let res_code = SPREAD[resonance_bucket(resonance_hz) as usize];
        let mut w = [FAMILY_PHYSICS; UMP_WORD_BYTES];
        w[8..16].copy_from_slice(&[
            kind_code,
            mat_hi,
            mat_lo,
            imp_code,
            res_code,
            kind_code ^ mat_lo,
            imp_code ^ res_code,
            kind_code ^ mat_hi,
        ]);
        Self(w)
    }

    /// Encode a mixer-health event: severity + diagnosis discriminant.
    pub fn from_roadie_event(severity: u8, diagnosis: u8) -> Self {
        let sev_code = SPREAD[(severity & 0x07) as usize];
        let diag_code = SPREAD[(diagnosis & 0x07) as usize];
        let mix = sev_code ^ diag_code;
        let mut w = [FAMILY_ROADIE; UMP_WORD_BYTES];
        w[8..16].copy_from_slice(&[sev_code, diag_code, mix, sev_code, diag_code, mix, sev_code, diag_code]);
        Self(w)
    }

    /// Encode a PTY route decision: `sid` (specialist domain, 0..=6) stays
    /// as fine within-family detail; `margin` (confidence gap) routes
    /// through a spread code so a one-bucket confidence step survives
    /// POPCNT routing.
    pub fn from_pty_route(sid: u8, margin: u32) -> Self {
        let sid_code = SPREAD[(sid & 0x07) as usize];
        let margin_code = SPREAD[margin_bucket(margin) as usize];
        let mix = sid_code ^ margin_code;
        let sid_raw = sid & 0x07;
        let mut w = [FAMILY_PTY_ROUTE; UMP_WORD_BYTES];
        w[8..16].copy_from_slice(&[sid_code, margin_code, mix, sid_raw, sid_code, margin_code, mix, sid_raw]);
        Self(w)
    }
}

/// Quantize |impulse| (Permyriad magnitude, 0..=10000 q) into 8 coarse buckets.
fn impulse_bucket(impulse_q: i32) -> u8 {
    let q = impulse_q.unsigned_abs().min(10_000);
    (q * 7 / 10_000) as u8
}

/// Quantize resonance (Hz) into 8 log-spaced buckets over the audible band.
fn resonance_bucket(resonance_hz: i32) -> u8 {
    match resonance_hz.max(0) as u32 {
        0..=60 => 0,
        61..=150 => 1,
        151..=400 => 2,
        401..=1_000 => 3,
        1_001..=2_500 => 4,
        2_501..=6_000 => 5,
        6_001..=12_000 => 6,
        _ => 7,
    }
}

/// Quantize a BqRouter confidence margin into 8 buckets. Buckets 0..=2 span
/// the uncertain band; the edge at 50 matches the live UNCERTAIN threshold
/// (`margin < 50` drains to T2), not an invented boundary.
pub(crate) fn margin_bucket(margin: u32) -> u8 {
    match margin {
        0..=9 => 0,
        10..=24 => 1,
        25..=49 => 2,
        50..=99 => 3,
        100..=249 => 4,
        250..=599 => 5,
        600..=1_499 => 6,
        _ => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ump_word_is_query_width() {
        assert_eq!(std::mem::size_of::<UmpWord>(), UMP_WORD_BYTES);
        assert_eq!(UMP_WORD_BYTES, 16);
    }

    fn hamming(a: &UmpWord, b: &UmpWord) -> u32 {
        a.0.iter().zip(b.0.iter()).map(|(x, y)| (x ^ y).count_ones()).sum()
    }

    #[test]
    fn family_signatures_hold_the_32_bit_floor() {
        for (a, b) in [
            (FAMILY_CST_VOICE, FAMILY_PHYSICS),
            (FAMILY_CST_VOICE, FAMILY_ROADIE),
            (FAMILY_PHYSICS, FAMILY_ROADIE),
            (FAMILY_CST_VOICE, FAMILY_PTY_ROUTE),
            (FAMILY_PHYSICS, FAMILY_PTY_ROUTE),
            (FAMILY_ROADIE, FAMILY_PTY_ROUTE),
        ] {
            assert!((a ^ b).count_ones() >= 4, "{a:#04x} vs {b:#04x}");
        }
    }

    #[test]
    fn spread_codes_pairwise_four_bits() {
        for i in 0..SPREAD.len() {
            for j in (i + 1)..SPREAD.len() {
                let d = (SPREAD[i] ^ SPREAD[j]).count_ones();
                assert!(d >= 4, "SPREAD[{i}] vs SPREAD[{j}]: {d} bits");
            }
        }
    }

    #[test]
    fn families_are_hamming_separated() {
        let nv = UmpWord::from_node_voice(2, 60, 1);
        let ph = UmpWord::from_physics_event(2, 0xDEAD_BEEF_CAFE_F00D, 5_000, 440);
        let rd = UmpWord::from_roadie_event(2, 1);
        assert!(hamming(&nv, &ph) >= 32, "cst<->physics {}", hamming(&nv, &ph));
        assert!(hamming(&nv, &rd) >= 32, "cst<->roadie {}", hamming(&nv, &rd));
        assert!(hamming(&ph, &rd) >= 32, "physics<->roadie {}", hamming(&ph, &rd));
        let pt = UmpWord::from_pty_route(3, 120);
        assert!(hamming(&nv, &pt) >= 32, "cst<->pty {}", hamming(&nv, &pt));
        assert!(hamming(&ph, &pt) >= 32, "physics<->pty {}", hamming(&ph, &pt));
        assert!(hamming(&rd, &pt) >= 32, "roadie<->pty {}", hamming(&rd, &pt));
    }

    #[test]
    fn pty_route_payload_discriminates_and_splits_the_uncertain_band() {
        let certain = UmpWord::from_pty_route(3, 900);
        let uncertain = UmpWord::from_pty_route(3, 12);
        assert_eq!(certain.0[..8], uncertain.0[..8], "signature region is the family");
        assert!(hamming(&certain, &uncertain) > 0, "margin must reach the payload");
        let other_sid = UmpWord::from_pty_route(5, 900);
        let rd = UmpWord::from_roadie_event(3, 2);
        assert!(hamming(&certain, &other_sid) < hamming(&certain, &rd));
        assert_eq!(margin_bucket(49), 2);
        assert_eq!(margin_bucket(50), 3);
    }

    #[test]
    fn same_family_words_stay_nearer_than_cross_family() {
        let a = UmpWord::from_node_voice(2, 60, 1);
        let b = UmpWord::from_node_voice(3, 64, 0);
        let ph = UmpWord::from_physics_event(1, 42, 5_000, 440);
        let rd = UmpWord::from_roadie_event(3, 2);
        assert!(hamming(&a, &b) < hamming(&a, &ph));
        assert!(hamming(&a, &b) < hamming(&a, &rd));
    }

    #[test]
    fn physics_payload_discriminates_within_family() {
        let slide = UmpWord::from_physics_event(1, 42, 200, 440);
        let impact = UmpWord::from_physics_event(2, 42, 9_000, 440);
        assert_eq!(slide.0[..8], impact.0[..8]);
        assert!(hamming(&slide, &impact) > 0);
    }

    #[test]
    fn quantizers_span_their_ranges() {
        assert_eq!(impulse_bucket(0), 0);
        assert_eq!(impulse_bucket(10_000), 7);
        assert_eq!(impulse_bucket(-10_000), 7, "magnitude, sign-blind");
        assert_eq!(impulse_bucket(i32::MAX), 7, "clamped, no overflow");
        assert_eq!(resonance_bucket(0), 0);
        assert_eq!(resonance_bucket(440), 3);
        assert_eq!(resonance_bucket(20_000), 7);
        assert_eq!(resonance_bucket(-5), 0, "negative clamps to floor");
    }
}
