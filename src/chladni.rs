//! Chladni nodal plate: a 32x32 integer standing-wave field for the Astrolabe
//! dial. Cosine comes from the crate's BAM table (`pentaract::bam_cos`); this
//! module owns no trigonometric table of its own.

use crate::pentaract::bam_cos;

/// Width of the plate grid.
pub const GRID_WIDTH: usize = 32;
/// Height of the plate grid.
pub const GRID_HEIGHT: usize = 32;
/// Cells in one full plate.
pub const GRID_CELLS: usize = GRID_WIDTH * GRID_HEIGHT;

/// BAM angle step per grid cell for one mode index: a half turn (pi) spans the
/// plate, matching the classical square-plate figure `cos(n*pi*x/L)`.
const CELL_STEP: u32 = 32768 / GRID_WIDTH as u32;

/// Resonant mode pair of a square plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChladniPlate {
    /// Wave cycles across the X axis.
    pub n: u8,
    /// Wave cycles across the Y axis.
    pub m: u8,
}

impl ChladniPlate {
    /// A plate in modes `(n, m)`.
    pub const fn new(n: u8, m: u8) -> Self {
        Self { n, m }
    }

    #[inline]
    fn angle(mode: u8, coord: usize) -> u16 {
        ((mode as u32).wrapping_mul(coord as u32).wrapping_mul(CELL_STEP) & 0xFFFF) as u16
    }

    /// The unscaled standing wave at `(x, y)`, Q15:
    /// `cos(n x) cos(m y) - cos(m x) cos(n y)`. Nodal lines are where this is
    /// near zero, so nodal extraction reads THIS, never the energised field.
    #[inline]
    pub fn sample_raw(&self, x: usize, y: usize) -> i32 {
        let cos_nx = bam_cos(Self::angle(self.n, x));
        let cos_my = bam_cos(Self::angle(self.m, y));
        let cos_mx = bam_cos(Self::angle(self.m, x));
        let cos_ny = bam_cos(Self::angle(self.n, y));
        ((cos_nx * cos_my) >> 15) - ((cos_mx * cos_ny) >> 15)
    }

    /// [`Self::sample_raw`] scaled by drive energy. Amplitude only: a quiet
    /// plate is a flat plate, not a plate that is everywhere nodal.
    #[inline]
    pub fn sample(&self, x: usize, y: usize, energy: u16) -> i32 {
        // i64 intermediate: raw reaches +-65532 and energy +-65535, whose
        // product overflows i32 before the shift brings it back in range.
        ((self.sample_raw(x, y) as i64 * energy as i64) >> 8) as i32
    }

    /// Fill a caller-owned buffer with the energised field. No allocation.
    pub fn update_buffer(&self, energy: u16, buffer: &mut [i32; GRID_CELLS]) {
        for y in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                buffer[y * GRID_WIDTH + x] = self.sample(x, y, energy);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Negative controls. Each of these FAILS against the discarded 256-entry
    // COS_LUT, whose real period was ~154: it put +122 at the half turn and
    // broke by 403/256 of full scale across the wrap.
    #[test]
    fn cosine_wrap_is_continuous() {
        let d = (bam_cos(u16::MAX) - bam_cos(0)).abs();
        assert!(d < 300, "wrap discontinuity of {d} Q15 units");
    }

    #[test]
    fn cosine_is_zero_at_the_quarter_turn() {
        assert!(bam_cos(16384).abs() < 300, "got {}", bam_cos(16384));
    }

    #[test]
    fn cosine_is_minimal_at_the_half_turn() {
        assert!(bam_cos(32768) < -32000, "got {}", bam_cos(32768));
    }

    // Reads real cosine values rather than cancelling them: with m=0 the field
    // collapses to cos(pi x/32) - cos(pi y/32), so these two cells pin the
    // table's turn and quarter-turn against hand-computed values.
    #[test]
    fn mode_one_zero_is_a_cosine_difference() {
        let p = ChladniPlate::new(1, 0);
        assert!((p.sample_raw(0, 16) - 32767).abs() < 300, "got {}", p.sample_raw(0, 16));
        assert!((p.sample_raw(16, 0) + 32767).abs() < 300, "got {}", p.sample_raw(16, 0));
        assert!(p.sample_raw(0, 0).abs() < 300, "got {}", p.sample_raw(0, 0));
    }

    #[test]
    fn full_drive_does_not_overflow() {
        let p = ChladniPlate::new(2, 5);
        let mut buf = [0i32; GRID_CELLS];
        p.update_buffer(u16::MAX, &mut buf);
        assert!(buf.iter().any(|&v| v != 0));
    }

    #[test]
    fn swapping_modes_negates_the_field() {
        let a = ChladniPlate::new(2, 5);
        let b = ChladniPlate::new(5, 2);
        for y in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                assert_eq!(a.sample_raw(x, y), -b.sample_raw(x, y), "at ({x},{y})");
            }
        }
    }

    #[test]
    fn zero_energy_flattens_without_erasing_the_nodal_structure() {
        let p = ChladniPlate::new(2, 5);
        let mut buf = [0i32; GRID_CELLS];
        p.update_buffer(0, &mut buf);
        assert!(buf.iter().all(|&v| v == 0));
        assert!(
            (0..GRID_CELLS).any(|i| p.sample_raw(i % GRID_WIDTH, i / GRID_WIDTH) != 0),
            "raw field must still carry the pattern at zero drive"
        );
    }
}
