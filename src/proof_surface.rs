//! The L09 proof surface. A surface is proven by readback, not exit code — this
//! module is the readback's landing pad: a raw, linear, uncompressed frame-buffer
//! wrapper that serialises the 5-lane grid to the exact wire bytes an
//! `R8G8B8A8_UINT` copy returns. No colour-space translation, no mipmapping, no
//! multisampling — every one of those is a place a driver could round, and a
//! rounded pixel is a pixel that no longer word-compares.
//!
//! The readback contract lives in `.forge/v3-directives.ron` under `readback:`;
//! the consts here mirror it key for key so drift between the directive and the
//! code is a diff, not a mystery.

use crate::grid::{GridPixelBuffer, CELLS, DEPTH, LANES};
use crate::palette::{sentinel_byte_of, trit_of, MachineColor};
use crate::sentinel::breach;

/// The proof surface: one `GridPixelBuffer`, nothing else. The wrapper exists so
/// "bytes handed to the GPU for L09 readback" is a *type*, not a convention —
/// a function that takes `ProofSurface` cannot be handed an unaudited buffer
/// by accident once construction goes through `audit`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofSurface {
    /// The grid of pixels.
    pub pixels: GridPixelBuffer,
}

// ── Readback contract, mirrored from .forge/v3-directives.ron `readback:` ────
// Any drift here and every pixel read aborts — the .ron holds the tunable, the
// const holds the code's copy, and the two must say the same thing.

/// Mirrors `readback.format`. Raw unsigned bytes — `_UINT`, never `_UNORM`,
/// because a normalised format invites the driver to touch the values.
pub const READBACK_FORMAT: &str = "R8G8B8A8_UINT";
/// Mirrors `readback.sample_count`. One sample; MSAA resolve is arithmetic.
pub const SAMPLE_COUNT: u32 = 1;
/// Mirrors `readback.mip_level_count`. One level; a mip chain is downsampling.
pub const MIP_LEVEL_COUNT: u32 = 1;
/// Mirrors `readback.filter`. Nearest; any interpolation invents new words.
pub const READBACK_FILTER: &str = "nearest";
/// Mirrors `readback.srgb_transform`. False; a gamma curve remaps every channel.
pub const SRGB_TRANSFORM: bool = false;
/// Mirrors `readback.compression`. None; BC/ASTC quantise blocks, not pixels.
pub const READBACK_COMPRESSION: &str = "none";

/// Texture width in texels: one depth slice per column.
pub const SURFACE_WIDTH: usize = DEPTH;
/// Texture height in texels: one lane per row. Lane-major is the grid's own
/// order (`grid.rs` `index = lane * DEPTH + depth`), so a row IS a lane and the
/// readback needs no swizzle.
pub const SURFACE_HEIGHT: usize = LANES;
/// The wire size in bytes: every texel is one 4-byte RGBA8 word.
pub const SURFACE_BYTES: usize = CELLS * core::mem::size_of::<MachineColor>();

impl ProofSurface {
    /// Wrap a rendered grid. Wrapping is free by construction — the layout locks
    /// below prove the wrapper adds no byte.
    #[inline(always)]
    pub const fn new(pixels: GridPixelBuffer) -> Self {
        Self { pixels }
    }

    /// Serialise to the exact bytes an `R8G8B8A8_UINT` readback returns.
    /// `MachineColor` is `[u8; 4]` in R,G,B,A memory order at align 1 with no
    /// padding anywhere in the struct, so this is a plain copy loop — safe code,
    /// because there is nothing an `unsafe` transmute would do that this doesn't.
    pub const fn as_bytes(&self) -> [u8; SURFACE_BYTES] {
        let mut out = [0u8; SURFACE_BYTES];
        let mut i = 0;
        while i < CELLS {
            let c = self.pixels.px[i].0;
            out[i * 4] = c[0];
            out[i * 4 + 1] = c[1];
            out[i * 4 + 2] = c[2];
            out[i * 4 + 3] = c[3];
            i += 1;
        }
        out
    }

    /// The inverse of `as_bytes` (L07). Total over all byte patterns — decoding
    /// never fails, because a byte-level bijection has no illegal input; whether
    /// the *pixels* are legal is `audit`'s question, not this function's.
    pub const fn from_bytes(bytes: &[u8; SURFACE_BYTES]) -> Self {
        let mut px = [MachineColor([0, 0, 0, 0]); CELLS];
        let mut i = 0;
        while i < CELLS {
            px[i] = MachineColor([
                bytes[i * 4],
                bytes[i * 4 + 1],
                bytes[i * 4 + 2],
                bytes[i * 4 + 3],
            ]);
            i += 1;
        }
        Self { pixels: GridPixelBuffer { px } }
    }

    /// Word-compare every pixel against the 16 legal palette words — the 3 trit
    /// states and the 13-state envelope, via the palette's own decoders so no
    /// colour literal is restated here (L05). Any pixel outside the 16 is
    /// corruption of the proof surface itself: `breach()` → `abort`, unswallowable
    /// (L10). There is no `Result` on this path by design — a caller cannot `?`
    /// away a corrupt frame. A passing audit returns the number of pixels checked.
    pub fn audit(&self) -> usize {
        for (i, px) in self.pixels.px.iter().enumerate() {
            if trit_of(*px).is_none() && sentinel_byte_of(*px).is_none() {
                breach("proof-surface pixel off the 16-word palette", i as u8);
            }
        }
        CELLS
    }
}

// ---------------------------------------------------------------------------
// LAYOUT LOCKS. Measured with rustc, never hand-computed.
// ---------------------------------------------------------------------------
const _: () = assert!(core::mem::size_of::<ProofSurface>() == 420);
const _: () = assert!(core::mem::align_of::<ProofSurface>() == 1);
const _: () = assert!(core::mem::offset_of!(ProofSurface, pixels) == 0);

// The wrapper is the buffer — same size, so `as_bytes` covers every byte and
// no padding hole exists for un-serialised state to hide in.
const _: () = assert!(core::mem::size_of::<ProofSurface>() == core::mem::size_of::<GridPixelBuffer>());

// The texture geometry IS the byte count: 21 x 5 texels x 4 bytes, lane-major.
const _: () = assert!(SURFACE_WIDTH * SURFACE_HEIGHT * 4 == core::mem::size_of::<ProofSurface>());
const _: () = assert!(SURFACE_BYTES == core::mem::size_of::<ProofSurface>());

// The readback contract's numeric half. The strings are compared by the
// directive-sync tooling; the counts are compile-time facts.
const _: () = assert!(SAMPLE_COUNT == 1);
const _: () = assert!(MIP_LEVEL_COUNT == 1);
const _: () = assert!(!SRGB_TRANSFORM);

// `i as u8` in `audit` names the breached pixel exactly; this holds only while
// every index fits a byte.
const _: () = assert!(CELLS <= u8::MAX as usize);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atom::TritCell5D;
    use crate::grid::{pixels_to_point, point_to_pixels, PackedPoint105};

    fn uniform(b: u8) -> ProofSurface {
        ProofSurface::new(point_to_pixels(&PackedPoint105 { slices: [TritCell5D(b); DEPTH] }))
    }

    fn byte_round_trips(s: ProofSurface) {
        let bytes = s.as_bytes();
        assert_eq!(ProofSurface::from_bytes(&bytes), s, "f_inv(f(x)) != x");
        // And the other direction: the bytes themselves survive the loop.
        assert_eq!(ProofSurface::from_bytes(&bytes).as_bytes(), bytes, "f(f_inv(y)) != y");
    }

    #[test]
    fn the_origin_surface_survives_the_byte_bijection() {
        byte_round_trips(ProofSurface::new(point_to_pixels(&PackedPoint105::ORIGIN)));
    }

    #[test]
    fn every_interior_byte_surface_survives_the_byte_bijection() {
        for b in 0u8..243 {
            byte_round_trips(uniform(b));
        }
    }

    #[test]
    fn every_sentinel_surface_survives_the_byte_bijection() {
        for b in 243u8..=255 {
            byte_round_trips(uniform(b));
        }
    }

    /// L07 over the full path: point -> pixels -> surface -> wire bytes -> surface
    /// -> pixels -> point. If any stage drops a bit, the original point is gone.
    #[test]
    fn a_point_round_trips_through_the_wire_bytes() {
        let mut points = vec![PackedPoint105::ORIGIN];
        for b in 0u8..=255 {
            points.push(PackedPoint105 { slices: [TritCell5D(b); DEPTH] });
        }
        // And one mixed point, so the test is not fooled by uniform surfaces.
        let mut slices = [TritCell5D::ORIGIN; DEPTH];
        for (i, s) in slices.iter_mut().enumerate() {
            *s = match i % 4 {
                0 => TritCell5D(0),
                1 => TritCell5D(121),
                2 => TritCell5D(242),
                _ => TritCell5D(243 + (i % 13) as u8),
            };
        }
        points.push(PackedPoint105 { slices });

        for p in points {
            let surface = ProofSurface::new(point_to_pixels(&p));
            let recovered = pixels_to_point(&ProofSurface::from_bytes(&surface.as_bytes()).pixels);
            assert_eq!(recovered, p, "point lost through the wire bytes");
        }
    }

    #[test]
    fn audit_passes_every_legal_surface_and_counts_all_pixels() {
        assert_eq!(ProofSurface::new(point_to_pixels(&PackedPoint105::ORIGIN)).audit(), CELLS);
        for b in 0u8..=255 {
            assert_eq!(uniform(b).audit(), CELLS, "legal surface for byte {b} failed audit");
        }
    }

    #[test]
    fn the_geometry_consts_describe_the_grid_not_a_second_grid() {
        assert_eq!(SURFACE_WIDTH, DEPTH);
        assert_eq!(SURFACE_HEIGHT, LANES);
        assert_eq!(SURFACE_WIDTH * SURFACE_HEIGHT, CELLS);
        assert_eq!(READBACK_FORMAT, "R8G8B8A8_UINT");
        assert_eq!(READBACK_FILTER, "nearest");
        assert_eq!(READBACK_COMPRESSION, "none");
    }

    // ---- THE BREACH PATH IS PROVEN, NOT ASSUMED -----------------------------
    //
    // `breach()` calls `std::process::abort()`, so it cannot be tested in-process
    // (L10 — an unwind could be caught and the corruption swallowed). Same
    // re-exec pattern as grid.rs: run this test binary again with the trigger
    // armed, assert the child died non-zero and announced the breach.

    const BREACH_ENV: &str = "FORGE_V3_BREACH_UNDER_TEST";

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

    #[test]
    fn an_off_palette_pixel_fails_audit_by_aborting() {
        const NAME: &str = "proof_surface::tests::an_off_palette_pixel_fails_audit_by_aborting";
        if armed_for(NAME) {
            let mut s = ProofSurface::new(point_to_pixels(&PackedPoint105::ORIGIN));
            // A colour on no table: not a trit, not an envelope entry.
            s.pixels.px[42] = MachineColor::rgb(0x12, 0x34, 0x56);
            assert_eq!(trit_of(s.pixels.px[42]), None);
            assert_eq!(sentinel_byte_of(s.pixels.px[42]), None);
            let _ = s.audit();
            unreachable!("audit returned on an off-palette pixel");
        }
        let out = run_breach_child(NAME);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "child exited 0 — corruption was swallowed");
        assert!(
            stderr.contains("PEXIL BREACH"),
            "child died without announcing the breach; stderr = {stderr:?}"
        );
    }
}
