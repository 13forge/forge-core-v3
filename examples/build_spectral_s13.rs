//! Builds and verifies the canonical `spectral_s13_weight.s13` file for the 7-domain
//! Spectral MoE Router and Harmonic Formant Sieve engine.

use forge_core_v3::metarouter::{build_s13_bytes, pack_trits, trit_bytes_needed, MetaRouter};
use std::path::Path;

fn main() {
    println!("=== Building Canonical Spectral MoE S13 Weight File ===");

    const D_MODEL: u16 = 64;
    let bpc = trit_bytes_needed(D_MODEL) as usize; // 13 bytes per centroid

    // 7 Spectral Domains:
    // 0: Vocal / Formant Engine (mid-range formants, pitch tracked)
    // 1: Sub-Bass & Kick Core (sub-150Hz energy)
    // 2: Percussive & Transient Sibilance (HF noise, consonants)
    // 3: Harmonic Pad & Plains Cree Sieve (resonant pentatonic partials)
    // 4: Game Voice Morph & Throat Modeler (extreme formant shifts)
    // 5: Chladni Cymatics & Visual Synesthesia (standing wave modes)
    // 6: Master Summing & Dynamic Limiter (full spectrum bus)

    let mut domains: [Vec<f32>; 7] = Default::default();
    for d in 0..7 {
        domains[d] = vec![0.0f32; D_MODEL as usize];
    }

    // Domain 0: Vocal Formants (concentrated in bins 10..28)
    for i in 10..28 {
        domains[0][i] = 1.0;
    }

    // Domain 1: Sub-Bass (concentrated in bins 0..6)
    for i in 0..6 {
        domains[1][i] = 1.0;
    }

    // Domain 2: Percussive / Sibilance (concentrated in bins 35..60)
    for i in 35..60 {
        domains[2][i] = 1.0;
    }

    // Domain 3: Harmonic Sieve (alternating harmonic combs)
    for i in (0..64).step_by(4) {
        domains[3][i] = 1.0;
        if i + 1 < 64 {
            domains[3][i + 1] = 0.5;
        }
    }

    // Domain 4: Voice Morph (throat resonance in bins 8..18 and 28..40)
    for i in 8..18 {
        domains[4][i] = 1.0;
    }
    for i in 28..40 {
        domains[4][i] = 0.8;
    }

    // Domain 5: Chladni Cymatics (orthogonal modal peaks in 12, 24, 36, 48)
    domains[5][12] = 1.0;
    domains[5][24] = 1.0;
    domains[5][36] = 1.0;
    domains[5][48] = 1.0;

    // Domain 6: Master Summing (broad flat distribution)
    for i in 0..64 {
        domains[6][i] = 0.5;
    }

    // Pack all 7 domains into centroids (13 bytes each)
    let mut all_centroids = Vec::with_capacity(7 * bpc);
    for d in 0..7 {
        let packed = pack_trits(&domains[d], bpc);
        assert_eq!(packed.len(), bpc);
        all_centroids.extend_from_slice(&packed);
    }

    let bias = [0.0f32; 7];

    // Build raw .s13 file bytes
    let s13_bytes = build_s13_bytes(D_MODEL, bias, &all_centroids);

    let targets = [
        Path::new("F:/v3/assets/spectral_s13_weight.s13"),
        Path::new("F:/v3/crates/forge-core-v3/assets/spectral_s13_weight.s13"),
    ];

    for output_path in &targets {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).expect("create assets dir");
        }
        std::fs::write(output_path, &s13_bytes).expect("write spectral_s13_weight.s13");
        println!("Successfully wrote {} bytes to {}", s13_bytes.len(), output_path.display());

        // Verify round-trip load
        let router = MetaRouter::<7>::load(output_path).expect("loaded .s13 file must be valid");
        assert_eq!(router.d_model, D_MODEL);
        assert_eq!(router.num_experts, 7);
        assert_eq!(router.bytes_per_centroid as usize, bpc);
        assert_eq!(router.centroids.len(), 7 * bpc);
        println!("Verification passed for {}", output_path.display());
    }

    let router = MetaRouter::<7>::load(targets[0]).expect("load target[0]");

    // Test a sample query against the loaded router
    let mut vocal_query = vec![0.0f32; 64];
    for i in 12..24 {
        vocal_query[i] = 1.0;
    }
    let (routed_domain, dist) = router.route(&vocal_query).expect("route query");
    println!("Sample vocal query routed to Domain {} with distance {}", routed_domain, dist);
    assert_eq!(routed_domain, 0, "Vocal query should route to Domain 0 (Vocal/Formants)");

    println!("=== All assertions passed. Spectral S13 weight is live and ready! ===");
}
