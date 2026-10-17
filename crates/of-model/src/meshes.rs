//! Bridges between B-rep shapes and triangle meshes.

use of_geom::{DVec3, Kind, Shape};
use of_mesh::TriMesh;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BodyId, Ref};

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

/// A graphics mesh body: triangles for display, measuring and mesh tools,
/// outside the B-rep kernel.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshBody {
    pub id: BodyId,
    pub mesh: std::sync::Arc<TriMesh>,
}

fn yes() -> bool {
    true
}

/// What an imported mesh becomes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MeshImportMode {
    /// Triangles for display and mesh tools.
    #[default]
    Graphics,
    /// A surface body with a face per triangle.
    Surface,
    /// A solid with a face per triangle; the mesh must be closed.
    Solid,
}

fn unit_scale() -> f64 {
    1.0
}

fn facet_limit() -> u32 {
    100_000
}

/// How a mesh or point cloud file is imported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MeshImport {
    #[serde(default)]
    pub mode: MeshImportMode,
    /// Millimetres per file unit, for files without units.
    #[serde(default = "unit_scale")]
    pub scale: f64,
    /// Meshes with more triangles than this can only be graphics bodies.
    #[serde(default = "facet_limit")]
    pub max_facets: u32,
    /// Join flat runs of triangles into single faces.
    #[serde(default = "yes")]
    pub group_facets: bool,
}

impl Default for MeshImport {
    fn default() -> Self {
        MeshImport {
            mode: MeshImportMode::Graphics,
            scale: 1.0,
            max_facets: facet_limit(),
            group_facets: true,
        }
    }
}

fn chord() -> f64 {
    0.01
}

fn angle() -> f64 {
    0.3
}

/// Turn a solid or surface body into a graphics mesh body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConvertToMesh {
    pub body: BodyId,
    /// Keep the original body too.
    #[serde(default)]
    pub keep: bool,
    /// The most the triangles may stray from the surface, mm.
    #[serde(default = "chord")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub chord: f64,
    /// The most neighbouring triangles may turn, radians.
    #[serde(default = "angle")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub angle: f64,
}

/// When decimation stops.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecimateTarget {
    /// Keep this fraction of the triangles.
    Fraction {
        #[schemars(range(min = 0.0, max = 1.0))]
        fraction: f64,
    },
    /// Stop before the surface moves further than this, mm.
    Tolerance {
        #[schemars(extend("exclusiveMinimum" = 0))]
        tolerance: f64,
    },
}

/// Reduce a mesh body's triangle count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DecimateMesh {
    pub body: BodyId,
    pub target: DecimateTarget,
    #[serde(default = "yes")]
    pub preserve_boundary: bool,
    /// Only these triangles (a painted region); empty for all.
    #[serde(default)]
    pub facets: Vec<u32>,
}

fn twenty_degrees() -> f64 {
    20f64.to_radians()
}

/// Group a mesh body's triangles into regions by how sharply they fold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SegmentMesh {
    pub body: BodyId,
    /// Triangles meeting at a gentler fold than this join, radians.
    #[serde(default = "twenty_degrees")]
    pub angle: f64,
    /// Regions smaller than this join a neighbour, mm².
    #[serde(default)]
    pub min_area: f64,
}

/// One step of mesh preparation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PrepStep {
    /// Line the mesh up with the axes by its principal directions, about
    /// the origin.
    Orient,
    /// Drop pieces smaller than this fraction of the largest.
    RemovePieces {
        fraction: f64,
    },
    /// Pull spikes further than `sigma` standard deviations back.
    RemoveNoise {
        sigma: f64,
    },
    /// Keep this fraction of the triangles.
    Simplify {
        fraction: f64,
    },
    Smooth {
        iterations: u32,
        #[serde(default)]
        boundary: bool,
    },
    /// Fill holes with at most this many edges around them.
    FillHoles {
        max_edges: u32,
    },
    SmoothBoundaries {
        iterations: u32,
    },
}

/// Clean up scan data, step by step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MeshPrep {
    pub body: BodyId,
    #[schemars(length(min = 1))]
    pub steps: Vec<PrepStep>,
}

/// Turn a graphics mesh body into a surface or solid of triangle faces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MeshToBrep {
    pub body: BodyId,
    /// Make a solid; the mesh must be closed.
    #[serde(default)]
    pub solid: bool,
    #[serde(default = "yes")]
    pub group_facets: bool,
    #[serde(default = "facet_limit")]
    pub max_facets: u32,
}

/// The kind of surface to fit to mesh triangles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FitKind {
    /// The simplest kind that fits well.
    #[default]
    Auto,
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    /// A smooth B-spline surface.
    Freeform,
}

/// A surface fitted to a region of a mesh body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SurfaceFromMesh {
    pub body: BodyId,
    /// The triangles to fit; empty uses `group`, or the whole mesh.
    #[serde(default)]
    pub facets: Vec<u32>,
    /// A region from Segment Mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u32>,
    #[serde(default)]
    pub fit: FitKind,
    /// How far the surface reaches past the triangles, mm.
    #[serde(default)]
    pub extend: f64,
}

/// A solid from a closed mesh: its regions fitted with surfaces, extended to
/// meet, and the enclosed volume kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MeshToSolid {
    pub body: BodyId,
    /// Segmentation fold angle, radians.
    #[serde(default = "twenty_degrees")]
    pub angle: f64,
    /// Regions smaller than this join a neighbour, mm².
    #[serde(default)]
    pub min_area: f64,
}

/// Section curves through a mesh body on evenly spaced planes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MeshSections {
    pub body: BodyId,
    pub plane: Ref,
    #[schemars(range(min = 1, max = 200))]
    pub count: u32,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub spacing: f64,
    #[serde(default)]
    pub offset: f64,
}
