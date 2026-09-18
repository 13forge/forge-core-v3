//! 3D cubic storage for Pexil cells indexed by linear coordinate.

use crate::atom::{CellOrdinal, Pexil, TritCell5D, ValidityMask};

/// A cubic 3D volume of cells, stored as a flat vector indexed by linear position.
pub struct PexilChunk {
    /// Flat storage of all cells.
    cells: Vec<Pexil>,
    /// Edge length of the cube. Total cells = edge³.
    edge: usize,
}

impl PexilChunk {
    /// Constructs a new cubic chunk with the given edge length.
    ///
    /// Initializes all cells to `Pexil { lattice: TritCell5D::ORIGIN, validity: ValidityMask::ALL_UNKNOWN, ordinal: CellOrdinal(0), payload: [0;4] }`.
    /// Total cells allocated: `edge * edge * edge`.
    pub fn new(edge: usize) -> Self {
        let capacity = edge * edge * edge;
        let default = Pexil {
            lattice: TritCell5D::ORIGIN,
            validity: ValidityMask::ALL_UNKNOWN,
            ordinal: CellOrdinal(0),
            payload: [0; 4],
        };
        let cells = vec![default; capacity];
        Self { cells, edge }
    }

    /// Computes the linear index for coordinates (x, y, z) with bounds checking.
    ///
    /// Returns the 1D index if all coordinates are in bounds `[0, edge)`, or `None` if any coordinate is out of bounds.
    /// The layout is: `index = z * edge * edge + y * edge + x`.
    pub fn index(&self, x: usize, y: usize, z: usize) -> Option<usize> {
        if x >= self.edge || y >= self.edge || z >= self.edge {
            return None;
        }
        Some(z * self.edge * self.edge + y * self.edge + x)
    }

    /// Returns a reference to the cell at (x, y, z), or `None` if out of bounds.
    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<&Pexil> {
        self.index(x, y, z).map(|idx| &self.cells[idx])
    }

    /// Returns a mutable reference to the cell at (x, y, z), or `None` if out of bounds.
    pub fn get_mut(&mut self, x: usize, y: usize, z: usize) -> Option<&mut Pexil> {
        let idx = self.index(x, y, z)?;
        Some(&mut self.cells[idx])
    }

    /// Edge length of the cube (cells per axis).
    pub fn edge(&self) -> usize {
        self.edge
    }

    /// Returns the total byte footprint of all cells: `cells.len() * sizeof(Pexil)`.
    ///
    /// Since `Pexil` is 8 bytes, this is always `cells.len() * 8`.
    pub fn byte_footprint(&self) -> usize {
        self.cells.len() * core::mem::size_of::<Pexil>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_4x4x4_produces_64_cells_and_512_bytes() {
        let chunk = PexilChunk::new(4);
        assert_eq!(chunk.cells.len(), 64);
        assert_eq!(chunk.byte_footprint(), 512);
    }

    #[test]
    fn get_get_mut_round_trip() {
        let mut chunk = PexilChunk::new(3);
        if let Some(cell) = chunk.get_mut(1, 1, 1) {
            cell.ordinal = CellOrdinal(42);
        }
        let retrieved = chunk.get(1, 1, 1).expect("cell at (1,1,1) should exist");
        assert_eq!(retrieved.ordinal, CellOrdinal(42));
    }

    #[test]
    fn get_returns_none_for_out_of_bounds() {
        let chunk = PexilChunk::new(4);
        assert!(chunk.get(4, 0, 0).is_none());
        assert!(chunk.get(0, 4, 0).is_none());
        assert!(chunk.get(0, 0, 4).is_none());
        assert!(chunk.get(5, 5, 5).is_none());
    }

    #[test]
    fn byte_footprint_for_65x65x65() {
        let chunk = PexilChunk::new(65);
        let expected = 65 * 65 * 65 * 8;
        assert_eq!(chunk.byte_footprint(), expected);
        assert_eq!(chunk.byte_footprint(), 2_197_000);
    }
}
