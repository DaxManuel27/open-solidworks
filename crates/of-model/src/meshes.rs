//! Bridges between B-rep shapes and triangle meshes.

use of_geom::{DVec3, Kind, Shape};
use of_mesh::TriMesh;

/// A shape's faces as triangles, grouped by face (the group is the face's
/// index in `shape`). Faces don't share vertices: weld to join them.
pub fn tessellate(shape: &Shape, deflection: f64) -> Result<TriMesh, String> {
    let m = shape.mesh(deflection, 0.3).map_err(|e| e.0)?;
    let positions: Vec<DVec3> = m
        .positions
        .chunks_exact(3)
        .map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64))
        .collect();
    let triangles: Vec<[u32; 3]> = m
        .indices
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    let mut groups = vec![0u32; triangles.len()];
    for f in 0..shape.count(Kind::Face) as usize {
        let (a, b) = (
            m.face_tri_start[f] as usize,
            m.face_tri_start[f + 1] as usize,
        );
        for g in &mut groups[a..b] {
            *g = f as u32;
        }
    }
    Ok(TriMesh {
        positions,
        triangles,
        colors: None,
        groups,
    })
}

/// A fine tessellation deflection for a shape of this size.
pub fn deflection(shape: &Shape, fraction: f64) -> f64 {
    shape
        .bbox()
        .map_or(0.01, |b| (b.diagonal() * fraction).max(1e-4))
}

/// A flattened surface's triangles in place in the model, with each
/// triangle's stretch, for a deviation plot.
#[derive(Debug, Clone, PartialEq)]
pub struct Flattening {
    pub mesh: TriMesh,
    /// The largest change of length per triangle, as a fraction.
    pub strain: Vec<f64>,
}
