//! CREATE tab tools wired onto real forge-geo producers (2026-07-18, Sean).
//! Each tool below calls a genuine forge-geo fn. Defaults are placeholder params.
//!
//! TODO: forge-geo v3 excludes the mesh core (primitives/builder/sdf/csg) as
//! quarantined per BLUEPRINT-SUBSTRATE-CENSUS-2026-08-11.md (2026-08-17).
//! Until those generators are ported or re-authored, this module stubs all
//! tool dispatch — the caller never reaches unreachable! at runtime, but the
//! function signatures exist for compilation.

/// Minimal stub for a voxel: RGBA color in a 3D grid.
#[derive(Clone, Copy, Debug)]
pub struct Voxel {
    /// Grid X coordinate.
    pub x: u32,
    /// Grid Y coordinate.
    pub y: u32,
    /// Grid Z coordinate.
    pub z: u32,
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel.
    pub a: u8,
}

/// Minimal stub for a mesh: vertex and triangle counts only.
///
/// **Twin definition (L05 one-home): Production home is
/// `forge-geo-v3/src/mesh.rs:7` (fuller implementation with normals/UVs and glb
/// parsing). This crate is Crate Zero (zero-dependency) and cannot depend on
/// forge-geo-v3. Both homes are noted and tracked in DEAD-LEDGER (2026-08-17).**
#[derive(Clone, Debug)]
pub struct ForgeMesh {
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
}

impl ForgeMesh {
    /// Stub constructor.
    pub fn new() -> Self {
        Self { vertices: Vec::new(), triangles: Vec::new() }
    }

    /// Stub: add a vertex, return its index.
    pub fn add_vertex(&mut self, pos: [f32; 3]) -> u32 {
        let idx = self.vertices.len() as u32;
        self.vertices.push(pos);
        idx
    }

    /// Stub: add a triangle.
    pub fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.triangles.push([a, b, c]);
    }

    /// Vertex count.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Triangle count.
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }
}

impl Default for ForgeMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Dispatch a CREATE-tab tool name to its real forge-geo mesh.
/// All tool names are stubs until forge-geo v3's mesh core is ported.
/// Panics on an unknown tool — the caller only ever passes names it
/// pattern-matched, so an unknown name here is a wiring bug.
pub fn generate(tool: &str) -> ForgeMesh {
    match tool {
        "mesh" => {
            let mut mesh = ForgeMesh::new();
            // Stub: box_mesh(Vec3::splat(1.0))
            mesh.add_vertex([-0.5, -0.5, -0.5]);
            mesh.add_vertex([0.5, -0.5, -0.5]);
            mesh.add_vertex([0.5, 0.5, -0.5]);
            mesh.add_vertex([-0.5, 0.5, -0.5]);
            mesh.add_vertex([-0.5, -0.5, 0.5]);
            mesh.add_vertex([0.5, -0.5, 0.5]);
            mesh.add_vertex([0.5, 0.5, 0.5]);
            mesh.add_vertex([-0.5, 0.5, 0.5]);
            // Bottom face
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            // Top face
            mesh.add_triangle(4, 6, 5);
            mesh.add_triangle(4, 7, 6);
            // Front face
            mesh.add_triangle(0, 5, 1);
            mesh.add_triangle(0, 4, 5);
            // Back face
            mesh.add_triangle(2, 7, 3);
            mesh.add_triangle(2, 6, 7);
            // Left face
            mesh.add_triangle(0, 3, 7);
            mesh.add_triangle(0, 7, 4);
            // Right face
            mesh.add_triangle(1, 5, 6);
            mesh.add_triangle(1, 6, 2);
            mesh
        }
        "csg" => {
            // Stub: union of a box and ellipsoid
            let mut mesh = ForgeMesh::new();
            mesh.add_vertex([0.0, 0.0, 0.0]);
            mesh.add_vertex([1.0, 0.0, 0.0]);
            mesh.add_vertex([0.0, 1.0, 0.0]);
            mesh.add_triangle(0, 1, 2);
            mesh
        }
        "sdf" => {
            // Stub: smooth_union of sphere + box
            let mut mesh = ForgeMesh::new();
            mesh.add_vertex([0.0, 0.0, 0.0]);
            mesh.add_vertex([1.0, 0.0, 0.0]);
            mesh.add_vertex([0.0, 1.0, 0.0]);
            mesh.add_triangle(0, 1, 2);
            mesh
        }
        "voxel" => {
            // Stub: voxels_to_mesh of a 3x3x3 grid
            let mut mesh = ForgeMesh::new();
            mesh.add_vertex([0.0, 0.0, 0.0]);
            mesh.add_vertex([1.0, 0.0, 0.0]);
            mesh.add_vertex([0.0, 1.0, 0.0]);
            mesh.add_triangle(0, 1, 2);
            mesh
        }
        "refinery" => {
            // Stub: refinery_layout(1.0)
            let mut mesh = ForgeMesh::new();
            mesh.add_vertex([0.0, 0.0, 0.0]);
            mesh.add_vertex([1.0, 0.0, 0.0]);
            mesh.add_vertex([0.0, 1.0, 0.0]);
            mesh.add_triangle(0, 1, 2);
            mesh
        }
        other => unreachable!("create_geo::generate called with unwired tool {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_create_tool_produces_a_mesh() {
        for tool in ["mesh", "csg", "sdf", "voxel", "refinery"] {
            let mesh = generate(tool);
            assert!(
                mesh.vertex_count() > 0,
                "{tool} produced an empty mesh (stub mode)"
            );
            assert!(
                mesh.triangle_count() > 0,
                "{tool} produced zero triangles (stub mode)"
            );
        }
    }
}
