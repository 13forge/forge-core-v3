//! The 21×5 pixel contract. Storage is slice-major (21 depth slices); the render is
//! lane-major (5 rows of 21) so one lane is a contiguous run for SIMD.

use crate::atom::TritCell5D;
use crate::palette::{sentinel_byte_of, sentinel_colour, trit_colour, trit_of, MachineColor};
use crate::sentinel::breach;

/// Hierarchy depth. Index 0 is the root macro-cell, 20 the leaf.
pub const DEPTH: usize = 21;
/// Lanes `[X, Y, Z, T, S]`. S is Scale / substrate depth.
pub const LANES: usize = 5;
/// One cell per lane per depth.
pub const CELLS: usize = DEPTH * LANES;

/// 105 trits as 21 radix-3 bytes. Storage order is depth, not lane.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackedPoint105 {
    /// The 21 packed slices, one per depth level.
    pub slices: [TritCell5D; DEPTH],
}

/// The readback surface. Lane-major: `index = lane * DEPTH + depth`, so lane `n`
/// occupies a contiguous 21-entry run.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPixelBuffer {
    /// The pixels in lane-major order.
    pub px: [MachineColor; CELLS],
}

impl PackedPoint105 {
    /// Every slice at the balanced origin.
    pub const ORIGIN: Self = Self { slices: [TritCell5D::ORIGIN; DEPTH] };
}

/// Serialise to pixels. A sentinel slice paints all five of its lanes with the
/// sentinel colour, so the whole depth reads as out-of-band.
pub fn point_to_pixels(p: &PackedPoint105) -> GridPixelBuffer {
    let mut px = [MachineColor([0, 0, 0, 0]); CELLS];
    for d in 0..DEPTH {
        let cell = p.slices[d];
        match cell.trits() {
            Some(t) => {
                for (lane, &trit) in t.iter().enumerate() {
                    px[lane * DEPTH + d] = trit_colour(trit);
                }
            }
            None => {
                let c = sentinel_colour(cell.0);
                for lane in 0..LANES {
                    px[lane * DEPTH + d] = c;
                }
            }
        }
    }
    GridPixelBuffer { px }
}

/// Reconstruct. Any colour outside the 16-entry palette, or a partially-sentinel
/// depth, is unrecoverable corruption — abort, never panic.
pub fn pixels_to_point(g: &GridPixelBuffer) -> PackedPoint105 {
    let mut slices = [TritCell5D::ORIGIN; DEPTH];
    for d in 0..DEPTH {
        let first = g.px[d];
        if let Some(b) = sentinel_byte_of(first) {
            for lane in 1..LANES {
                if sentinel_byte_of(g.px[lane * DEPTH + d]) != Some(b) {
                    breach("depth is only partly sentinel", b);
                }
            }
            slices[d] = TritCell5D(b);
            continue;
        }
        let mut t = [0i8; LANES];
        for lane in 0..LANES {
            match trit_of(g.px[lane * DEPTH + d]) {
                Some(v) => t[lane] = v,
                None => breach("pixel outside the 16-colour palette", d as u8),
            }
        }
        slices[d] = TritCell5D::from_trits(t);
    }
    PackedPoint105 { slices }
}

const _: () = assert!(core::mem::size_of::<PackedPoint105>() == 21);
const _: () = assert!(core::mem::align_of::<PackedPoint105>() == 1);
const _: () = assert!(core::mem::size_of::<GridPixelBuffer>() == 420);
const _: () = assert!(CELLS == 105);

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trips(p: PackedPoint105) {
        assert_eq!(pixels_to_point(&point_to_pixels(&p)), p);
    }

    #[test]
    fn origin_round_trips() {
        round_trips(PackedPoint105::ORIGIN);
    }

    #[test]
    fn extremes_round_trip() {
        round_trips(PackedPoint105 { slices: [TritCell5D(0); DEPTH] });
        round_trips(PackedPoint105 { slices: [TritCell5D(242); DEPTH] });
    }

    // The bug this test exists for: a sentinel that decoded to [0;5] would
    // re-encode as 121 and silently become the origin.
    #[test]
    fn every_sentinel_round_trips_as_itself() {
        for b in 243u8..=255 {
            let p = PackedPoint105 { slices: [TritCell5D(b); DEPTH] };
            round_trips(p);
            assert_ne!(pixels_to_point(&point_to_pixels(&p)).slices[0].0, 121);
        }
    }

    #[test]
    fn every_interior_byte_round_trips() {
        for b in 0u8..243 {
            round_trips(PackedPoint105 { slices: [TritCell5D(b); DEPTH] });
        }
    }

    #[test]
    fn mixed_depths_round_trip() {
        let mut slices = [TritCell5D::ORIGIN; DEPTH];
        for (i, s) in slices.iter_mut().enumerate() {
            *s = match i % 4 {
                0 => TritCell5D(0),
                1 => TritCell5D(121),
                2 => TritCell5D(242),
                _ => TritCell5D(243 + (i % 13) as u8),
            };
        }
        round_trips(PackedPoint105 { slices });
    }

    // ---- THE BREACH PATH IS PROVEN, NOT ASSUMED -----------------------------
    //
    // `breach()` calls `std::process::abort()`, so it cannot be tested in-process:
    // there is nothing to catch, which is the entire point of choosing abort over
    // panic (L10 — an unwind could be caught and the corruption swallowed). These two
    // tests re-exec this test binary and assert the child died and said so. Without
    // them the abort wiring is a call site nobody has ever executed.

    const BREACH_ENV: &str = "FORGE_V3_BREACH_UNDER_TEST";

    /// Re-run one test in a child process with the trigger armed.
    fn run_breach_child(test_name: &str) -> std::process::Output {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "--nocapture", test_name])
            .env(BREACH_ENV, test_name)
            .output()
            .expect("could not re-exec the test binary")
    }

    fn armed_for(test_name: &str) -> bool {
        std::env::var(BREACH_ENV).as_deref() == Ok(test_name)
    }

    fn assert_aborted(out: &std::process::Output, what: &str) {
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{what}: child exited 0 — corruption was swallowed");
        assert!(
            stderr.contains("PEXIL BREACH"),
            "{what}: child died without announcing the breach; stderr = {stderr:?}"
        );
    }

    #[test]
    fn an_off_palette_pixel_aborts() {
        const NAME: &str = "grid::tests::an_off_palette_pixel_aborts";
        if armed_for(NAME) {
            let mut g = point_to_pixels(&PackedPoint105::ORIGIN);
            // A colour on no table: not a trit, not an envelope entry.
            g.px[0] = MachineColor::rgb(0x12, 0x34, 0x56);
            assert_eq!(trit_of(g.px[0]), None);
            let _ = pixels_to_point(&g);
            unreachable!("pixels_to_point returned on an off-palette pixel");
        }
        assert_aborted(&run_breach_child(NAME), "off-palette pixel");
    }

    // The corruption grid.rs:67 exists for: one lane of a depth reads as a sentinel and
    // the other four do not, so there is no coordinate and no sentinel to recover.
    #[test]
    fn a_partly_sentinel_depth_aborts() {
        const NAME: &str = "grid::tests::a_partly_sentinel_depth_aborts";
        if armed_for(NAME) {
            let mut g = point_to_pixels(&PackedPoint105::ORIGIN);
            g.px[0] = crate::palette::sentinel_colour(243);
            // Lanes 1..5 of depth 0 are still trit colours — the depth is half converted.
            let _ = pixels_to_point(&g);
            unreachable!("pixels_to_point returned on a partly-sentinel depth");
        }
        assert_aborted(&run_breach_child(NAME), "partly-sentinel depth");
    }

    /// Lane `n` occupies the contiguous run `px[n*21 .. n*21+21]`, in depth order.
    ///
    /// The point that proves this has to have lanes that DIFFER. The previous
    /// version of this test built `[TritCell5D(0); 21]` — 105 pixels of one
    /// colour — so it passed under any indexing whatsoever, including the
    /// transposed one it exists to refuse. It asserted that a constant is
    /// constant.
    #[test]
    fn a_lane_is_a_contiguous_run() {
        // trit(lane, depth) = (depth + lane) % 3 - 1: varies along BOTH axes, so
        // `lane * DEPTH + depth` and `depth * LANES + lane` disagree.
        let trit_at = |lane: usize, d: usize| ((d + lane) % 3) as i8 - 1;
        let mut slices = [TritCell5D::ORIGIN; DEPTH];
        for (d, s) in slices.iter_mut().enumerate() {
            let mut t = [0i8; LANES];
            for (lane, v) in t.iter_mut().enumerate() {
                *v = trit_at(lane, d);
            }
            *s = TritCell5D::from_trits(t);
        }
        let point = PackedPoint105 { slices };
        let g = point_to_pixels(&point);

        for lane in 0..LANES {
            for d in 0..DEPTH {
                assert_eq!(
                    g.px[lane * DEPTH + d],
                    trit_colour(trit_at(lane, d)),
                    "lane {lane} depth {d} is not where lane-major indexing puts it"
                );
            }
        }

        // The transposed reading is REFUSED, not merely unasserted. Without this
        // the loop above would still pass on a point whose lanes coincided —
        // which is exactly how the old test passed while proving nothing.
        let transposed_agrees = (0..LANES).all(|lane| {
            (0..DEPTH).all(|d| g.px[d * LANES + lane] == trit_colour(trit_at(lane, d)))
        });
        assert!(!transposed_agrees, "this point cannot tell the two indexings apart");

        // And it is a real point, not just a pixel pattern.
        round_trips(point);
    }
}
