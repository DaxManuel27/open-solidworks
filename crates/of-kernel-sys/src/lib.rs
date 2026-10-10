//! The single bridge between Rust and the C++ geometry kernel, OpenCASCADE.
//!
//! This is the only crate that touches C++. Everything above it uses the safe
//! wrappers in `of-geom`. Sub-shapes (faces, edges, vertices) are addressed by
//! their 0-based index in the shape's indexed map, which is stable for a given
//! shape and shared by every function here.

#[cxx::bridge(namespace = "ofk")]
pub mod ffi {
    #[derive(Debug, Clone, Copy, PartialEq, Default)]
    struct Vec3 {
        x: f64,
        y: f64,
        z: f64,
    }

    /// A plane: origin, in-plane X direction and normal.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Frame {
        origin: Vec3,
        x_dir: Vec3,
        normal: Vec3,
    }

    /// A 2D curve in a frame's coordinates.
    ///
    /// Kinds: 0 line `[x0 y0 x1 y1]`, 1 counter-clockwise arc
    /// `[cx cy sx sy ex ey]`, 2 circle `[cx cy r]`, 3 open spline through
    /// points `[x0 y0 ...]`, 4 closed spline through points.
    #[derive(Debug, Clone, PartialEq)]
    struct Curve {
        kind: u8,
        data: Vec<f64>,
    }

    /// One step of an operation's history: sub-shape `from` (of kind
    /// `from_kind`) of input `input` became or produced sub-shape `to` of
    /// the result. Kinds: 0 face, 1 edge, 2 vertex. Relations: 0 unchanged,
    /// 1 modified, 2 generated, 3 start cap, 4 end cap.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct HistoryEntry {
        input: u8,
        from_kind: u8,
        from: u32,
        rel: u8,
        to_kind: u8,
        to: u32,
    }

    /// Triangles grouped by face and polylines grouped by edge.
    /// `face_tri_start[i]..face_tri_start[i + 1]` are face i's triangles;
    /// `edge_start[i]..edge_start[i + 1]` are edge i's points.
    #[derive(Debug, Clone, Default)]
    struct Mesh {
        positions: Vec<f32>,
        normals: Vec<f32>,
        indices: Vec<u32>,
        face_tri_start: Vec<u32>,
        edge_points: Vec<f32>,
        edge_start: Vec<u32>,
    }

    #[derive(Debug, Clone, Default)]
    struct MassProps {
        volume: f64,
        area: f64,
        center: Vec3,
        /// Row-major 3x3 inertia tensor about the center, for unit density.
        inertia: Vec<f64>,
        principal_moments: Vec3,
        principal_axes: Vec<Vec3>,
    }

    #[derive(Debug, Clone, Copy, Default)]
    struct BBox {
        valid: bool,
        min: Vec3,
        max: Vec3,
    }

    /// Kinds: 0 other, 1 plane, 2 cylinder, 3 cone, 4 sphere, 5 torus.
    #[derive(Debug, Clone, Copy, Default)]
    struct FaceInfo {
        kind: u8,
        area: f64,
        centroid: Vec3,
        /// A point on the face and the outward normal there.
        point: Vec3,
        normal: Vec3,
        axis_origin: Vec3,
        axis_dir: Vec3,
        x_dir: Vec3,
        radius: f64,
    }

    /// Kinds: 0 other, 1 line, 2 circle or arc, 3 ellipse, 255 degenerate.
    #[derive(Debug, Clone, Copy, Default)]
    struct EdgeInfo {
        kind: u8,
        length: f64,
        start: Vec3,
        end: Vec3,
        mid: Vec3,
        center: Vec3,
        axis: Vec3,
        radius: f64,
        closed: bool,
    }

    #[derive(Debug, Clone, Copy, Default)]
    struct Distance {
        value: f64,
        a: Vec3,
        b: Vec3,
    }

    unsafe extern "C++" {
        include!("of-kernel-sys/cpp/ofk.h");

        type Shape;

        fn null_shape() -> UniquePtr<Shape>;
        fn clone_shape(s: &Shape) -> UniquePtr<Shape>;
        fn is_null(s: &Shape) -> bool;
        fn is_valid(s: &Shape) -> bool;
        /// Kinds: 0 face, 1 edge, 2 vertex, 3 solid.
        fn count(s: &Shape, kind: u8) -> Result<u32>;
        fn sub_shape(s: &Shape, kind: u8, index: u32) -> Result<UniquePtr<Shape>>;
        fn compound_new() -> UniquePtr<Shape>;
        fn compound_add(compound: Pin<&mut Shape>, s: &Shape);

        fn make_box(dx: f64, dy: f64, dz: f64) -> Result<UniquePtr<Shape>>;
        fn make_faces(
            frame: &Frame,
            curves: &[Curve],
            loop_sizes: &[u32],
            face_loops: &[u32],
        ) -> Result<UniquePtr<Shape>>;
        fn make_wire(frame: &Frame, curves: &[Curve]) -> Result<UniquePtr<Shape>>;
        fn half_space(face: &Shape, inside: &Vec3) -> Result<UniquePtr<Shape>>;

        fn prism(s: &Shape, dir: &Vec3, hist: &mut Vec<HistoryEntry>) -> Result<UniquePtr<Shape>>;
        fn revol(
            s: &Shape,
            origin: &Vec3,
            dir: &Vec3,
            angle: f64,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        /// Ops: 0 fuse, 1 cut, 2 common.
        fn boolean(
            op: u8,
            a: &Shape,
            b: &Shape,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn fillet(
            s: &Shape,
            edges: &[u32],
            radius: f64,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn chamfer(
            s: &Shape,
            edges: &[u32],
            d1: f64,
            d2: f64,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn shell(
            s: &Shape,
            faces: &[u32],
            thickness: f64,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        /// `matrix` is a row-major 3x4 rigid transform.
        fn transform(
            s: &Shape,
            matrix: &[f64],
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn mirror(
            s: &Shape,
            origin: &Vec3,
            normal: &Vec3,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;

        /// For each edge: face count, then that many face indices.
        fn edge_faces(s: &Shape) -> Vec<u32>;
        /// For each face: edge count, then that many edge indices.
        fn face_edges(s: &Shape) -> Vec<u32>;
        /// For each vertex: face count, then that many face indices.
        fn vertex_faces(s: &Shape) -> Vec<u32>;
        fn tessellate(s: &Shape, linear: f64, angular: f64) -> Result<Mesh>;
        fn edge_samples(s: &Shape, edge: u32, linear: f64, angular: f64) -> Result<Vec<f64>>;
        fn mass_props(s: &Shape) -> Result<MassProps>;
        fn bbox(s: &Shape) -> BBox;
        fn face_info(s: &Shape, face: u32) -> Result<FaceInfo>;
        fn edge_info(s: &Shape, edge: u32) -> Result<EdgeInfo>;
        fn vertex_point(s: &Shape, vertex: u32) -> Result<Vec3>;
        fn distance(a: &Shape, b: &Shape) -> Result<Distance>;

        fn write_step(s: &Shape, path: &str) -> Result<()>;
        fn read_step(path: &str) -> Result<UniquePtr<Shape>>;
        fn write_iges(s: &Shape, path: &str) -> Result<()>;
        fn read_iges(path: &str) -> Result<UniquePtr<Shape>>;
        fn write_stl(s: &Shape, path: &str, linear: f64, angular: f64, ascii: bool) -> Result<()>;
    }
}

// OpenCASCADE shapes are reference counted with atomic counters and are
// never mutated in place by the bridge, so moving them between threads is
// sound.
unsafe impl Send for ffi::Shape {}
unsafe impl Sync for ffi::Shape {}

pub use ffi::*;
