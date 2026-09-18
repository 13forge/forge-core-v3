//! SPEC §5 conformance — the assertion suite, compiled independently of the crate that
//! defines the types. This file is the gate, not a copy of one: if a layout lock inside
//! `src/` is ever deleted, the size/offset it protected still has to answer here.
//!
//! Colour Science was `[ASSUMED-BLOCKED]` here while `colour.rs` did not exist. It does
//! now, so the block is live and the marker is gone rather than left to rot.

use forge_core_v3::arch::{CreativeClock, DetClock};
use forge_core_v3::atom::{Pexil, PexilLine};
use forge_core_v3::colour::{ColorBlindMode, OklchColor};
use forge_core_v3::grid::{GridPixelBuffer, PackedPoint105};
use forge_core_v3::soul::SoulIdentity;

const _: () = {
    // Spatial Primitives
    assert!(core::mem::size_of::<Pexil>() == 8);
    assert!(core::mem::align_of::<Pexil>() == 8);
    assert!(core::mem::size_of::<PexilLine>() == 64);
    assert!(core::mem::align_of::<PexilLine>() == 64);

    // Provenance
    assert!(core::mem::size_of::<SoulIdentity>() == 12);
    assert!(core::mem::align_of::<SoulIdentity>() == 4);
    assert!(core::mem::offset_of!(SoulIdentity, genesis_tick) == 0);
    assert!(core::mem::offset_of!(SoulIdentity, authority) == 10);

    // Clocks
    assert!(core::mem::size_of::<DetClock>() == 16);
    assert!(core::mem::offset_of!(DetClock, epoch) == 8);
    assert!(core::mem::offset_of!(DetClock, authority) == 10);
    assert!(core::mem::size_of::<CreativeClock>() == 12);

    // §3 Hierarchical coordinate space
    assert!(core::mem::size_of::<PackedPoint105>() == 21);
    assert!(core::mem::align_of::<PackedPoint105>() == 1);
    assert!(core::mem::size_of::<GridPixelBuffer>() == 420);

    // Colour Science
    assert!(core::mem::size_of::<OklchColor>() == 8);
    assert!(core::mem::align_of::<OklchColor>() == 8);
    assert!(core::mem::size_of::<ColorBlindMode>() == 1);
};

/// §2 boundary elements. The 5-cube's k-face counts are `C(5,k) * 2^(5-k)` — arithmetic,
/// so it is computed here rather than transcribed from the spec table.
#[test]
fn the_penteract_has_the_boundary_elements_section_two_claims() {
    const fn choose(n: u32, k: u32) -> u32 {
        let (mut num, mut den, mut i) = (1u32, 1u32, 0u32);
        while i < k {
            num *= n - i;
            den *= i + 1;
            i += 1;
        }
        num / den
    }
    let faces = |k: u32| choose(5, k) * 2u32.pow(5 - k);
    assert_eq!(faces(0), 32, "bounding vertices");
    assert_eq!(faces(1), 80, "edges");
    assert_eq!(faces(2), 80, "square faces");
    assert_eq!(faces(3), 40, "cube cells");
    assert_eq!(faces(4), 10, "tesseract facets");
}

/// §1/§2 radix identities. `243 + 13 == 256` is the whole envelope argument.
#[test]
fn the_radix_closes_the_byte() {
    assert_eq!(3usize.pow(5), 243, "interior capacity");
    assert_eq!(256 - 3usize.pow(5), 13, "out-of-band sentinel states");
    assert_eq!(forge_core_v3::thirteen::ARITHMETIC_FORCING as usize, 13);
}

/// §3 scale capacity. The spec states 21 trits/axis and ~10.46e9 steps/axis; both are
/// integer facts, so neither is transcribed.
#[test]
fn the_scale_capacity_is_twenty_one_trits_per_axis() {
    assert_eq!(
        forge_core_v3::grid::CELLS / forge_core_v3::grid::LANES,
        21,
        "105 trits / 5 axes"
    );
    assert_eq!(3u64.pow(21), 10_460_353_203, "steps per axis");
    assert_eq!(
        core::mem::size_of::<GridPixelBuffer>(),
        forge_core_v3::grid::CELLS * 4,
        "one R8G8B8A8 pixel per cell"
    );
}

/// §3 invariance contract, stated by the spec as
/// `pixels_to_point(point_to_pixels(p)) == p`. L07 wants the inverse exercised over the
/// interior *and* the sentinels, not a single happy value.
#[test]
fn the_invariance_contract_holds_over_interior_and_sentinels() {
    use forge_core_v3::atom::TritCell5D;
    use forge_core_v3::grid::{pixels_to_point, point_to_pixels};

    for b in 0u8..=242 {
        let p = PackedPoint105 { slices: [TritCell5D(b); 21] };
        assert_eq!(pixels_to_point(&point_to_pixels(&p)), p, "interior byte {b}");
    }
    for b in 243u8..=255 {
        let p = PackedPoint105 { slices: [TritCell5D(b); 21] };
        assert_eq!(pixels_to_point(&point_to_pixels(&p)), p, "sentinel byte {b}");
    }
    let origin = PackedPoint105::ORIGIN;
    assert_eq!(pixels_to_point(&point_to_pixels(&origin)), origin, "origin");
}
