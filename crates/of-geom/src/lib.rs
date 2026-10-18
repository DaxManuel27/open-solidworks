//! Safe Rust wrappers over the geometry kernel: shapes, booleans, fillets,
//! chamfers, shells, and tessellation into meshes and edge lines for the viewport.
//!
//! Faces, edges and vertices are addressed by index ([`FaceId`] style `u32`s)
//! into the shape they belong to. Indices are only meaningful for that exact
//! shape; `of-model` maps them to stable names across rebuilds using the
//! [`History`] every operation returns.

mod assembly;
mod curves;
mod hlr;
mod plane;
mod profile;

pub use assembly::{STEP_ROOT, StepAssemblyWriter, StepInstance, read_step_assembly};
pub use curves::{Curve3, EndTangent};
pub use glam::{DAffine3, DMat3, DVec2, DVec3};
pub use hlr::{Curve2d, EdgeClass, HlrEdge, HlrQuality};
pub use plane::Plane;
pub use profile::{Curve2, Profile, ProfileFace};

use cxx::UniquePtr;
use of_kernel_sys::ffi;
use serde::{Deserialize, Serialize};

/// A geometry kernel error, with a message suitable for showing to users.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct GeomError(pub String);

impl From<cxx::Exception> for GeomError {
    fn from(e: cxx::Exception) -> Self {
        GeomError(e.what().to_string())
    }
}

pub type Result<T> = std::result::Result<T, GeomError>;

/// Kinds of sub-shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Face = 0,
    Edge = 1,
    Vertex = 2,
    Solid = 3,
}

impl Kind {
    fn from_u8(k: u8) -> Kind {
        match k {
            0 => Kind::Face,
            1 => Kind::Edge,
            2 => Kind::Vertex,
            _ => Kind::Solid,
        }
    }
}

/// How an output sub-shape relates to an input sub-shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Relation {
    /// Passed through unchanged.
    Same,
    /// Trimmed, split or otherwise changed, but still the same entity.
    Modified,
    /// Created from it, e.g. an extrude's side face from a profile edge.
    Generated,
    /// The start cap of an extrude or revolve, from a profile face.
    StartCap,
    /// The end cap of an extrude or revolve, from a profile face.
    EndCap,
}

/// One step of an operation's history. `input` is 0 for the main input
/// (or a boolean's first argument) and 1 for a boolean's tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HistoryEntry {
    pub input: u8,
    pub from_kind: Kind,
    pub from: u32,
    pub rel: Relation,
    pub to_kind: Kind,
    pub to: u32,
}

pub type History = Vec<HistoryEntry>;

fn convert_history(raw: Vec<ffi::HistoryEntry>) -> History {
    raw.into_iter()
        .map(|h| HistoryEntry {
            input: h.input,
            from_kind: Kind::from_u8(h.from_kind),
            from: h.from,
            rel: match h.rel {
                0 => Relation::Same,
                1 => Relation::Modified,
                2 => Relation::Generated,
                3 => Relation::StartCap,
                _ => Relation::EndCap,
            },
            to_kind: Kind::from_u8(h.to_kind),
            to: h.to,
        })
        .collect()
}

/// The result of a modeling operation: the new shape and how it relates to
/// the inputs.
pub struct OpResult {
    pub shape: Shape,
    pub history: History,
}

pub(crate) fn v3(v: DVec3) -> ffi::Vec3 {
    ffi::Vec3 {
        x: v.x,
        y: v.y,
        z: v.z,
    }
}

pub(crate) fn dv3(v: ffi::Vec3) -> DVec3 {
    DVec3::new(v.x, v.y, v.z)
}

/// A boundary-representation shape: a solid, a set of faces, a wire, or a
/// compound of them.
pub struct Shape(UniquePtr<ffi::Shape>);

impl Clone for Shape {
    fn clone(&self) -> Self {
        Shape(ffi::clone_shape(&self.0))
    }
}

impl std::fmt::Debug for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_null() {
            return write!(f, "Shape(null)");
        }
        write!(
            f,
            "Shape(solids: {}, faces: {}, edges: {})",
            self.count(Kind::Solid),
            self.count(Kind::Face),
            self.count(Kind::Edge)
        )
    }
}

/// Surface type of a face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SurfaceKind {
    Other,
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FaceInfo {
    pub kind: SurfaceKind,
    pub area: f64,
    pub centroid: DVec3,
    /// A point on the face and the outward normal there.
    pub point: DVec3,
    pub normal: DVec3,
    /// Plane origin, or the axis of a cylinder, cone or torus.
    pub axis_origin: DVec3,
    pub axis_dir: DVec3,
    /// In-plane X direction for planar faces.
    pub x_dir: DVec3,
    pub radius: f64,
}

/// Curve type of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CurveKind {
    Other,
    Line,
    Circle,
    Ellipse,
    Degenerate,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgeInfo {
    pub kind: CurveKind,
    pub length: f64,
    pub start: DVec3,
    pub end: DVec3,
    pub mid: DVec3,
    /// Circle center.
    pub center: DVec3,
    /// Line direction, or circle axis.
    pub axis: DVec3,
    pub radius: f64,
    pub closed: bool,
}

/// Triangles grouped by face and polylines grouped by edge, for display.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
    /// Face i owns triangles `face_tri_start[i]..face_tri_start[i + 1]`.
    pub face_tri_start: Vec<u32>,
    pub edge_points: Vec<f32>,
    /// Edge i owns points `edge_start[i]..edge_start[i + 1]`.
    pub edge_start: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MassProps {
    pub volume: f64,
    pub area: f64,
    pub center: DVec3,
    /// Inertia tensor about the center of mass, for unit density.
    pub inertia: DMat3,
    pub principal_moments: DVec3,
    pub principal_axes: [DVec3; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub min: DVec3,
    pub max: DVec3,
}

impl BBox {
    pub fn diagonal(&self) -> f64 {
        (self.max - self.min).length()
    }
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Distance {
    pub value: f64,
    pub a: DVec3,
    pub b: DVec3,
}

/// Boolean operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BoolOp {
    Fuse = 0,
    Cut = 1,
    Common = 2,
}

impl Shape {
    pub fn null() -> Shape {
        Shape(ffi::null_shape())
    }

    pub fn is_null(&self) -> bool {
        ffi::is_null(&self.0)
    }

    /// True when the shape passes the kernel's topology and geometry checks.
    pub fn is_valid(&self) -> bool {
        ffi::is_valid(&self.0)
    }

    pub fn count(&self, kind: Kind) -> u32 {
        if self.is_null() {
            return 0;
        }
        ffi::count(&self.0, kind as u8).unwrap_or(0)
    }

    pub fn sub_shape(&self, kind: Kind, index: u32) -> Result<Shape> {
        Ok(Shape(ffi::sub_shape(&self.0, kind as u8, index)?))
    }

    /// Map this shape's sub-shapes to a containing compound by kernel identity.
    pub fn indices_in(&self, whole: &Shape, kind: Kind) -> Result<Vec<u32>> {
        Ok(ffi::indices_in(&self.0, &whole.0, kind as u8)?)
    }

    /// The solids in this shape, each as its own shape.
    pub fn solids(&self) -> Vec<Shape> {
        (0..self.count(Kind::Solid))
            .filter_map(|i| self.sub_shape(Kind::Solid, i).ok())
            .collect()
    }

    pub fn compound<'a>(shapes: impl IntoIterator<Item = &'a Shape>) -> Shape {
        let mut c = ffi::compound_new();
        for s in shapes {
            if !s.is_null() {
                ffi::compound_add(c.pin_mut(), &s.0);
            }
        }
        Shape(c)
    }

    pub fn make_box(dx: f64, dy: f64, dz: f64) -> Result<Shape> {
        Ok(Shape(ffi::make_box(dx, dy, dz)?))
    }

    /// The solid on the `inside` side of a face's surface, unbounded.
    pub fn half_space(face: &Shape, inside: DVec3) -> Result<Shape> {
        Ok(Shape(ffi::half_space(&face.0, &v3(inside))?))
    }

    /// Offset a supporting surface toward a point (negative distance moves away).
    pub fn offset_face(&self, distance: f64, toward: DVec3) -> Result<Shape> {
        Ok(Shape(ffi::offset_face(&self.0, distance, &v3(toward))?))
    }

    pub fn revolve_until(&self, until: &Shape, origin: DVec3, axis: DVec3) -> Result<OpResult> {
        let mut h = Vec::new();
        let shape = ffi::revol_until(&self.0, &until.0, &v3(origin), &v3(axis), &mut h)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(h),
        })
    }

    /// An open or closed wire from 2D curves in a plane.
    pub fn wire(plane: &Plane, curves: &[Curve2]) -> Result<Shape> {
        let raw: Vec<ffi::Curve> = curves.iter().map(Curve2::to_ffi).collect();
        Ok(Shape(ffi::make_wire(&plane.to_ffi(), &raw)?))
    }

    pub fn prism(&self, dir: DVec3) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::prism(&self.0, &v3(dir), &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    pub fn split_plane(&self, plane: &Plane) -> Result<OpResult> {
        let mut h = Vec::new();
        let shape = ffi::split_plane(&self.0, &plane.to_ffi(), &mut h)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(h),
        })
    }

    /// An unbounded planar cutting face, retaining the authored plane normal.
    pub fn plane_face(plane: &Plane) -> Result<Shape> {
        Ok(Shape(ffi::plane_face(&plane.to_ffi())?))
    }

    /// Partition this shape with bounded faces, solids or unbounded plane faces.
    /// History input 1 describes the cutting tools; tool material is not included.
    pub fn split_tools(&self, tools: &Shape) -> Result<OpResult> {
        let mut history = Vec::new();
        let shape = ffi::split_tools(&self.0, &tools.0, &mut history)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }

    /// Topological orientation of a boundary face within this solid (+1 / -1).
    /// Thicken faces from [`Profile::wrap_onto`] radially by `depth`, outward
    /// (emboss) or inward (deboss), into solids.
    pub fn wrap_solid(&self, depth: f64, outward: bool) -> Result<Shape> {
        Ok(Shape(ffi::wrap_solid(&self.0, depth, outward)?))
    }

    pub fn face_orientation(&self, face: u32) -> Result<i8> {
        Ok(ffi::face_orientation(&self.0, face)?)
    }

    pub fn revolve(&self, origin: DVec3, axis: DVec3, angle: f64) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::revol(&self.0, &v3(origin), &v3(axis), angle, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    pub fn boolean(&self, op: BoolOp, tool: &Shape) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::boolean(op as u8, &self.0, &tool.0, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    pub fn fillet(&self, edges: &[u32], radius: f64) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::fillet(&self.0, edges, radius, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Chamfer with distance `d1` on the first face of each edge and `d2` on
    /// the other.
    pub fn chamfer(&self, edges: &[u32], d1: f64, d2: f64) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::chamfer(&self.0, edges, d1, d2, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Hollow the solid, removing `faces`. Positive thickness goes inward.
    pub fn shell(&self, faces: &[u32], thickness: f64) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::shell(&self.0, faces, thickness, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Apply a rigid transform.
    pub fn transform(&self, t: &DAffine3) -> Result<OpResult> {
        let m = t.matrix3;
        let p = t.translation;
        let rows = [
            m.x_axis.x, m.y_axis.x, m.z_axis.x, p.x, //
            m.x_axis.y, m.y_axis.y, m.z_axis.y, p.y, //
            m.x_axis.z, m.y_axis.z, m.z_axis.z, p.z,
        ];
        let mut h = Vec::new();
        let s = ffi::transform(&self.0, &rows, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Mirror across the plane through `origin` with normal `normal`.
    pub fn mirror(&self, origin: DVec3, normal: DVec3) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::mirror(&self.0, &v3(origin), &v3(normal), &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// For each edge, the indices of its adjacent faces.
    pub fn edge_faces(&self) -> Vec<Vec<u32>> {
        unflatten(&ffi::edge_faces(&self.0))
    }

    /// For each face, the indices of its edges.
    pub fn face_edges(&self) -> Vec<Vec<u32>> {
        unflatten(&ffi::face_edges(&self.0))
    }

    /// For each vertex, the indices of the faces that meet there.
    pub fn vertex_faces(&self) -> Vec<Vec<u32>> {
        unflatten(&ffi::vertex_faces(&self.0))
    }

    /// Triangulate for display. `linear` is the max chord deviation in mm,
    /// `angular` the max angle between segments in radians.
    pub fn mesh(&self, linear: f64, angular: f64) -> Result<Mesh> {
        let m = ffi::tessellate(&self.0, linear, angular)?;
        Ok(Mesh {
            positions: m.positions,
            normals: m.normals,
            indices: m.indices,
            face_tri_start: m.face_tri_start,
            edge_points: m.edge_points,
            edge_start: m.edge_start,
        })
    }

    /// Points along an edge.
    pub fn edge_samples(&self, edge: u32, linear: f64, angular: f64) -> Result<Vec<DVec3>> {
        let raw = ffi::edge_samples(&self.0, edge, linear, angular)?;
        Ok(raw
            .chunks_exact(3)
            .map(|c| DVec3::new(c[0], c[1], c[2]))
            .collect())
    }

    pub fn mass_props(&self) -> Result<MassProps> {
        let m = ffi::mass_props(&self.0)?;
        let i = &m.inertia;
        let axes: Vec<DVec3> = m.principal_axes.iter().map(|a| dv3(*a)).collect();
        Ok(MassProps {
            volume: m.volume,
            area: m.area,
            center: dv3(m.center),
            inertia: DMat3::from_cols_array(&[
                i[0], i[3], i[6], i[1], i[4], i[7], i[2], i[5], i[8],
            ]),
            principal_moments: dv3(m.principal_moments),
            principal_axes: [axes[0], axes[1], axes[2]],
        })
    }

    pub fn bbox(&self) -> Option<BBox> {
        let b = ffi::bbox(&self.0);
        b.valid.then(|| BBox {
            min: dv3(b.min),
            max: dv3(b.max),
        })
    }

    pub fn face_info(&self, face: u32) -> Result<FaceInfo> {
        let f = ffi::face_info(&self.0, face)?;
        Ok(FaceInfo {
            kind: match f.kind {
                1 => SurfaceKind::Plane,
                2 => SurfaceKind::Cylinder,
                3 => SurfaceKind::Cone,
                4 => SurfaceKind::Sphere,
                5 => SurfaceKind::Torus,
                _ => SurfaceKind::Other,
            },
            area: f.area,
            centroid: dv3(f.centroid),
            point: dv3(f.point),
            normal: dv3(f.normal),
            axis_origin: dv3(f.axis_origin),
            axis_dir: dv3(f.axis_dir),
            x_dir: dv3(f.x_dir),
            radius: f.radius,
        })
    }

    pub fn edge_info(&self, edge: u32) -> Result<EdgeInfo> {
        let e = ffi::edge_info(&self.0, edge)?;
        Ok(EdgeInfo {
            kind: match e.kind {
                1 => CurveKind::Line,
                2 => CurveKind::Circle,
                3 => CurveKind::Ellipse,
                255 => CurveKind::Degenerate,
                _ => CurveKind::Other,
            },
            length: e.length,
            start: dv3(e.start),
            end: dv3(e.end),
            mid: dv3(e.mid),
            center: dv3(e.center),
            axis: dv3(e.axis),
            radius: e.radius,
            closed: e.closed,
        })
    }

    pub fn vertex_point(&self, vertex: u32) -> Result<DVec3> {
        Ok(dv3(ffi::vertex_point(&self.0, vertex)?))
    }

    /// Minimum distance between two shapes and the closest points.
    pub fn distance(&self, other: &Shape) -> Result<Distance> {
        let d = ffi::distance(&self.0, &other.0)?;
        Ok(Distance {
            value: d.value,
            a: dv3(d.a),
            b: dv3(d.b),
        })
    }

    pub fn write_step(&self, path: &str) -> Result<()> {
        Ok(ffi::write_step(&self.0, path)?)
    }

    pub fn read_step(path: &str) -> Result<Shape> {
        Ok(Shape(ffi::read_step(path)?))
    }

    pub fn write_iges(&self, path: &str) -> Result<()> {
        Ok(ffi::write_iges(&self.0, path)?)
    }

    pub fn read_iges(path: &str) -> Result<Shape> {
        Ok(Shape(ffi::read_iges(path)?))
    }

    pub fn write_stl(&self, path: &str, linear: f64, angular: f64, ascii: bool) -> Result<()> {
        Ok(ffi::write_stl(&self.0, path, linear, angular, ascii)?)
    }
}

fn unflatten(flat: &[u32]) -> Vec<Vec<u32>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < flat.len() {
        let n = flat[i] as usize;
        out.push(flat[i + 1..i + 1 + n].to_vec());
        i += 1 + n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_topology_and_props() {
        let b = Shape::make_box(10.0, 20.0, 30.0).unwrap();
        assert_eq!(b.count(Kind::Face), 6);
        assert_eq!(b.count(Kind::Edge), 12);
        assert_eq!(b.edge_faces().len(), 12);
        assert!(b.edge_faces().iter().all(|f| f.len() == 2));
        assert!(b.face_edges().iter().all(|e| e.len() == 4));
        let m = b.mass_props().unwrap();
        assert!((m.volume - 6000.0).abs() < 1e-6);
        assert!((m.center - DVec3::new(5.0, 10.0, 15.0)).length() < 1e-9);
        let bb = b.bbox().unwrap();
        assert!((bb.max - DVec3::new(10.0, 20.0, 30.0)).length() < 1e-3);
    }

    #[test]
    fn face_info_reports_outward_normals() {
        let b = Shape::make_box(10.0, 10.0, 10.0).unwrap();
        for i in 0..6 {
            let f = b.face_info(i).unwrap();
            assert_eq!(f.kind, SurfaceKind::Plane);
            // Outward: the normal points away from the box center.
            assert!(f.normal.dot(f.centroid - DVec3::splat(5.0)) > 0.0);
        }
    }

    #[test]
    fn transform_moves_shape() {
        let b = Shape::make_box(1.0, 1.0, 1.0).unwrap();
        let moved = b
            .transform(&DAffine3::from_translation(DVec3::new(5.0, 0.0, 0.0)))
            .unwrap();
        let c = moved.shape.mass_props().unwrap().center;
        assert!((c - DVec3::new(5.5, 0.5, 0.5)).length() < 1e-9);
        assert_eq!(
            moved
                .history
                .iter()
                .filter(|h| h.from_kind == Kind::Face)
                .count(),
            6
        );
    }

    #[test]
    fn mirror_keeps_volume() {
        let b = Shape::make_box(1.0, 2.0, 3.0).unwrap();
        let m = b
            .mirror(DVec3::new(-1.0, 0.0, 0.0), DVec3::X)
            .unwrap()
            .shape;
        let p = m.mass_props().unwrap();
        assert!((p.volume - 6.0).abs() < 1e-9);
        assert!((p.center.x + 2.5).abs() < 1e-9);
    }
}
