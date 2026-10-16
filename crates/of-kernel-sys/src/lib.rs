//! The single bridge between Rust and the C++ geometry kernel, OpenCASCADE.
//!
//! This is the only crate that touches C++. Everything above it uses the safe
//! wrappers in `of-geom`. Sub-shapes (faces, edges, vertices) are addressed by
//! their 0-based index in the shape's indexed map, which is stable for a given
//! shape and shared by every function here.

#[cfg(feature = "kernel-probes")]
pub mod probes;

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
    // cxx 1.0.198 clones every field in non-Copy structs, including Copy fields.
    #[allow(clippy::clone_on_copy)]
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

    /// One instance in a STEP product structure: its parent node (-1 for a
    /// root), instance name, definition name, whether it is an assembly and
    /// its row-major 3x4 placement in the parent.
    // cxx 1.0.198 clones every field in non-Copy structs, including Copy fields.
    #[allow(clippy::clone_on_copy)]
    #[derive(Debug, Clone)]
    struct StepNode {
        parent: i64,
        name: String,
        part: String,
        assembly: bool,
        matrix: Vec<f64>,
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

    // cxx 1.0.198 clones every field in non-Copy structs, including Copy fields.
    #[allow(clippy::clone_on_copy)]
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

    /// One edge of a hidden-line projection, in the view plane's 2D
    /// coordinates. Categories: 0 sharp, 1 smooth (tangent), 2 seam or other
    /// higher-continuity edge, 3 outline (silhouette). Kinds: 0 line
    /// `[x0 y0 x1 y1]`, 1 counter-clockwise arc `[cx cy r a0 a1]` with
    /// `a1 > a0`, 2 polyline `[x0 y0 x1 y1 ...]`.
    // cxx 1.0.198 clones every field in non-Copy structs, including Copy fields.
    #[allow(clippy::clone_on_copy)]
    #[derive(Debug, Clone, PartialEq)]
    struct HlrEdge {
        visible: bool,
        category: u8,
        kind: u8,
        data: Vec<f64>,
    }

    /// A face's surface parameterisation. Kinds: 0 other, 1 plane, 2
    /// cylinder, 3 cone. A plane's point at (u, v) is `origin + u x + v y`; a
    /// cylinder's is `origin + radius (cos u x + sin u y) + v z`; a cone's is
    /// `origin + (radius + v sin a)(cos u x + sin u y) + v cos a z` with `a`
    /// the semi-angle. `reversed` faces have their normal opposite to the
    /// surface's.
    #[derive(Debug, Clone, Copy, Default)]
    struct FaceFrame {
        kind: u8,
        origin: Vec3,
        x: Vec3,
        y: Vec3,
        z: Vec3,
        radius: f64,
        semi_angle: f64,
        umin: f64,
        umax: f64,
        vmin: f64,
        vmax: f64,
        reversed: bool,
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
        fn indices_in(part: &Shape, whole: &Shape, kind: u8) -> Result<Vec<u32>>;
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
        fn make_wire3(curves: &[Curve]) -> Result<UniquePtr<Shape>>;
        fn join_wires(curves: &Shape) -> Result<UniquePtr<Shape>>;
        fn reverse_shape(shape: &Shape) -> UniquePtr<Shape>;
        fn make_helix(
            frame: &Frame,
            radius: f64,
            pitch: f64,
            turns: f64,
            start: f64,
            left: bool,
        ) -> Result<UniquePtr<Shape>>;
        /// A helix whose pitch varies linearly in turn count between rows.
        fn make_helix_law(
            frame: &Frame,
            radius: f64,
            turns: &[f64],
            pitches: &[f64],
            start: f64,
            left: bool,
        ) -> Result<UniquePtr<Shape>>;
        /// The edges where two shapes meet, as a compound.
        fn section(a: &Shape, b: &Shape) -> Result<UniquePtr<Shape>>;
        fn project_wire(wire: &Shape, target: &Shape, direction: &Vec3)
        -> Result<UniquePtr<Shape>>;
        fn split_face(
            body: &Shape,
            face: u32,
            wire: &Shape,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn sweep(
            profile: &Shape,
            path: &Shape,
            frame: &Frame,
            orientation: u8,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn loft(
            profiles: &Shape,
            ruled: bool,
            preserve_correspondence: bool,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn half_space(face: &Shape, inside: &Vec3) -> Result<UniquePtr<Shape>>;
        fn offset_face(face: &Shape, distance: f64, toward: &Vec3) -> Result<UniquePtr<Shape>>;
        fn revol_until(
            s: &Shape,
            until: &Shape,
            origin: &Vec3,
            dir: &Vec3,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;

        fn prism(s: &Shape, dir: &Vec3, hist: &mut Vec<HistoryEntry>) -> Result<UniquePtr<Shape>>;
        fn split_plane(
            s: &Shape,
            plane: &Frame,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn plane_face(plane: &Frame) -> Result<UniquePtr<Shape>>;
        fn split_tools(
            s: &Shape,
            tools: &Shape,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn face_orientation(s: &Shape, face: u32) -> Result<i8>;
        /// Closed sketch regions mapped by arc length onto a cylindrical face.
        fn wrap_faces(
            body: &Shape,
            face: u32,
            frame: &Frame,
            curves: &[Curve],
            loop_sizes: &[u32],
            face_loops: &[u32],
        ) -> Result<UniquePtr<Shape>>;
        /// A solid through closed sections (compound of wires, in order) and
        /// direction-2 curves crossing them, patched cell by cell.
        fn boundary(
            sections: &Shape,
            guides: &Shape,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        /// A solid skinned through closed sections (compound of wires, in order)
        /// with optional end tangents: mode 0 free, 1 normal to the end section
        /// (pointing into the loft), 2 along `dir`; `weight` scales the tangent.
        /// Without `preserve`, edges are matched and aligned first.
        #[allow(clippy::too_many_arguments)]
        fn skin(
            sections: &Shape,
            preserve: bool,
            start_mode: u8,
            start_dir: &Vec3,
            start_weight: f64,
            end_mode: u8,
            end_dir: &Vec3,
            end_weight: f64,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        /// Sweep a closed profile along a tangent-continuous open path so the
        /// points where guide curves cross each plane normal to the path keep
        /// their place on the section: one guide rotates it (and with `scale`
        /// scales it uniformly), two guides stretch it. `guide2` may be null.
        fn sweep_scaled(
            profile: &Shape,
            path: &Shape,
            guide1: &Shape,
            guide2: &Shape,
            scale: bool,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        /// A loft whose sections blend along a centerline (a connected wire)
        /// crossing each section's plane in order.
        fn loft_centerline(
            sections: &Shape,
            centerline: &Shape,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        /// The material an ISO-profile thread removes from a cylinder of `radius`.
        /// The frame's origin is the thread start on the axis, its normal the
        /// direction of advance and its x direction the start phase.
        fn thread_tool(
            frame: &Frame,
            radius: f64,
            pitch: f64,
            length: f64,
            left: bool,
            internal: bool,
        ) -> Result<UniquePtr<Shape>>;
        /// The start point, start tangent (as `normal`) and a perpendicular of a path.
        fn wire_start(path: &Shape) -> Result<Frame>;
        /// Sweep with the section's rotation set by an auxiliary curve: `guide`
        /// when it is not null, otherwise a synthesized curve turning `twist`
        /// radians about the path relative to a rotation-minimizing frame.
        fn sweep_aux(
            profile: &Shape,
            path: &Shape,
            guide: &Shape,
            twist: f64,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        /// Wrapped faces thickened radially outward (emboss) or inward (deboss).
        fn wrap_solid(faces: &Shape, depth: f64, outward: bool) -> Result<UniquePtr<Shape>>;
        /// Half an ellipsoid of revolution on a circular disk, tangent to the disk normal at the rim.
        fn dome_elliptical(
            s: &Shape,
            face: u32,
            height: f64,
            inward: bool,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn dome(
            s: &Shape,
            face: u32,
            height: f64,
            inward: bool,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
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
        fn variable_fillet(
            s: &Shape,
            edge: u32,
            stations: &[f64],
            straight: bool,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn draft_planes(
            s: &Shape,
            faces: &[u32],
            planes: &[Frame],
            direction: &Vec3,
            angle: f64,
            hist: &mut Vec<HistoryEntry>,
        ) -> Result<UniquePtr<Shape>>;
        fn draft(
            s: &Shape,
            faces: &[u32],
            plane: &Frame,
            direction: &Vec3,
            angle: f64,
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
        /// Hidden-line removal looking along `-view.normal` (the normal points
        /// at the viewer), with `view.x_dir` to the right on paper. `exact`
        /// uses the analytic algorithm, otherwise the polygonal one on a mesh
        /// of `deflection`. Hidden edges are returned only when `hidden`.
        fn hlr(
            s: &Shape,
            view: &Frame,
            exact: bool,
            hidden: bool,
            deflection: f64,
        ) -> Result<Vec<HlrEdge>>;
        /// Edges a face meets itself along (cylinder seams), possibly
        /// repeated, except those on the silhouette seen along `view`.
        fn seam_edges(s: &Shape, view: &Vec3) -> Vec<u32>;
        fn face_frame(s: &Shape, face: u32) -> Result<FaceFrame>;
        /// The inverse of `flatten`: the parts of `region` (planar faces in
        /// `frame`'s plane) over each face's flat image, mapped back onto the
        /// face's surface. `owners` gets the face each result came from.
        fn fold_faces(
            s: &Shape,
            faces: &[u32],
            maps: &[f64],
            frame: &Frame,
            region: &Shape,
            owners: &mut Vec<u32>,
        ) -> Result<UniquePtr<Shape>>;
        /// A solid bounded by planar polygons: `points` are xyz triples and
        /// `faces` is, per face, its vertex count then its vertex indices.
        fn polyhedron(points: &[f64], faces: &[u32]) -> Result<UniquePtr<Shape>>;
        /// Faces of `s` mapped into `frame`'s plane, each by an affine map of
        /// its (u, v) parameters (six values per face: x = a u + b v + c,
        /// y = d u + e v + f), joined into one region and extruded along
        /// `thickness`. `sources` gets, per face of the result: the index of
        /// the edge of `s` it was extruded from, -1 for the face in the plane,
        /// -2 for the opposite face, -3 when unknown.
        fn flatten(
            s: &Shape,
            faces: &[u32],
            maps: &[f64],
            frame: &Frame,
            thickness: &Vec3,
            sources: &mut Vec<i64>,
            simplify: bool,
        ) -> Result<UniquePtr<Shape>>;

        fn write_step(s: &Shape, path: &str) -> Result<()>;
        fn read_step(path: &str) -> Result<UniquePtr<Shape>>;
        fn write_iges(s: &Shape, path: &str) -> Result<()>;
        fn read_iges(path: &str) -> Result<UniquePtr<Shape>>;
        fn write_stl(s: &Shape, path: &str, linear: f64, angular: f64, ascii: bool) -> Result<()>;

        /// The shape placed by a row-major 3x4 rigid transform, sharing its
        /// geometry (no copy): for interference checks and assembly export.
        fn located(s: &Shape, matrix: &[f64]) -> Result<UniquePtr<Shape>>;
        /// Volume shared by two solids (0 when their boxes don't overlap).
        fn common_volume(a: &Shape, b: &Shape) -> Result<f64>;
        fn common_shape(a: &Shape, b: &Shape) -> Result<UniquePtr<Shape>>;

        /// A STEP file with product structure. Label 0 is the root assembly.
        type StepAssembly;
        fn step_assembly_new(name: &str) -> Result<UniquePtr<StepAssembly>>;
        /// Add a part definition; returns its label.
        fn step_assembly_part(a: Pin<&mut StepAssembly>, name: &str, s: &Shape) -> Result<u32>;
        /// Add an (empty) subassembly definition; returns its label.
        fn step_assembly_node(a: Pin<&mut StepAssembly>, name: &str) -> Result<u32>;
        /// Place `child` inside `parent` by a row-major 3x4 transform.
        fn step_assembly_place(
            a: Pin<&mut StepAssembly>,
            parent: u32,
            child: u32,
            name: &str,
            matrix: &[f64],
        ) -> Result<()>;
        fn step_assembly_write(a: Pin<&mut StepAssembly>, path: &str) -> Result<()>;

        /// A STEP file read with its product structure, as instance nodes.
        type StepRead;
        fn step_read(path: &str) -> Result<UniquePtr<StepRead>>;
        fn step_read_count(r: &StepRead) -> u32;
        fn step_read_node(r: &StepRead, i: u32) -> Result<StepNode>;
        fn step_read_shape(r: &StepRead, i: u32) -> Result<UniquePtr<Shape>>;
    }
}

// OpenCASCADE shapes are reference counted with atomic counters and are
// never mutated in place by the bridge, so moving them between threads is
// sound.
unsafe impl Send for ffi::Shape {}
unsafe impl Sync for ffi::Shape {}

pub use ffi::*;
