use crate::{DVec3, GeomError, OpResult, Plane, Result, Shape, convert_history, v3};
use of_kernel_sys::ffi;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Curve3 {
    Line { a: DVec3, b: DVec3 },
    Bezier { points: [DVec3; 4] },
}

impl Shape {
    /// Replace a planar circular disk face by an attached spherical cap.
    /// Inward removes the cap volume and requires supporting material beneath it.
    /// Half an ellipsoid of revolution on a planar circular disk. It leaves the rim
    /// parallel to the disk normal, so it is tangent to a cylinder the disk closes.
    pub fn dome_elliptical(&self, face: u32, height: f64, inward: bool) -> Result<OpResult> {
        let mut history = Vec::new();
        let shape = ffi::dome_elliptical(&self.0, face, height, inward, &mut history)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }

    pub fn dome(&self, face: u32, height: f64, inward: bool) -> Result<OpResult> {
        let mut history = Vec::new();
        let shape = ffi::dome(&self.0, face, height, inward, &mut history)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }
    pub fn split_face(&self, face: u32, wire: &Shape) -> Result<OpResult> {
        let mut history = Vec::new();
        let shape = ffi::split_face(&self.0, face, &wire.0, &mut history)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }
    /// Fillet the tangent chain through `edge` with radius `stations` given as
    /// (fraction along the chain, radius). Fractions run in the selected edge's
    /// direction. Radii blend smoothly between stations, or linearly if `straight`.
    pub fn variable_fillet(
        &self,
        edge: u32,
        stations: &[(f64, f64)],
        straight: bool,
    ) -> Result<OpResult> {
        let stations: Vec<_> = stations.iter().flat_map(|(u, r)| [*u, *r]).collect();
        let mut history = Vec::new();
        let shape = ffi::variable_fillet(&self.0, edge, &stations, straight, &mut history)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }

    /// Draft each face about its own hinge plane (for parting-line drafts).
    pub fn draft_hinged(
        &self,
        faces: &[(u32, Plane)],
        direction: DVec3,
        angle: f64,
    ) -> Result<OpResult> {
        if !direction.is_finite() || direction.length() < 1e-12 {
            return Err(GeomError(
                "draft needs a nonzero finite pull direction".into(),
            ));
        }
        let indices: Vec<u32> = faces.iter().map(|(f, _)| *f).collect();
        let planes: Vec<ffi::Frame> = faces.iter().map(|(_, p)| p.to_ffi()).collect();
        let mut history = Vec::new();
        let shape = ffi::draft_planes(
            &self.0,
            &indices,
            &planes,
            &v3(direction),
            angle,
            &mut history,
        )?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }

    pub fn draft(
        &self,
        faces: &[u32],
        plane: &Plane,
        direction: DVec3,
        angle: f64,
    ) -> Result<OpResult> {
        if !direction.is_finite() || direction.length() < 1e-12 {
            return Err(GeomError(
                "draft needs a nonzero finite pull direction".into(),
            ));
        }
        let mut history = Vec::new();
        let shape = ffi::draft(
            &self.0,
            faces,
            &plane.to_ffi(),
            &v3(direction),
            angle,
            &mut history,
        )?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }
    pub fn wire3(curves: &[Curve3]) -> Result<Shape> {
        let raw: Vec<_> = curves
            .iter()
            .map(|c| {
                let (kind, points) = match c {
                    Curve3::Line { a, b } => (0, vec![*a, *b]),
                    Curve3::Bezier { points } => (1, points.to_vec()),
                };
                ffi::Curve {
                    kind,
                    data: points.iter().flat_map(|p| [p.x, p.y, p.z]).collect(),
                }
            })
            .collect();
        Ok(Shape(ffi::make_wire3(&raw)?))
    }

    pub fn join_curves(curves: &[Shape]) -> Result<Shape> {
        let combined = Shape::compound(curves);
        Ok(Shape(ffi::join_wires(&combined.0)?))
    }

    pub fn reversed(&self) -> Shape {
        Shape(ffi::reverse_shape(&self.0))
    }

    /// A helix whose pitch changes linearly between `(turn, pitch)` rows; the first
    /// row is at turn 0 and the last sets the total turns.
    pub fn helix_variable(
        frame: &Plane,
        radius: f64,
        rows: &[(f64, f64)],
        start: f64,
        left: bool,
    ) -> Result<Shape> {
        let turns: Vec<f64> = rows.iter().map(|r| r.0).collect();
        let pitches: Vec<f64> = rows.iter().map(|r| r.1).collect();
        Ok(Shape(ffi::make_helix_law(
            &frame.to_ffi(),
            radius,
            &turns,
            &pitches,
            start,
            left,
        )?))
    }

    /// The edges where this shape meets `other`, as a compound.
    pub fn section(&self, other: &Shape) -> Result<Shape> {
        Ok(Shape(ffi::section(&self.0, &other.0)?))
    }

    pub fn helix(
        frame: &Plane,
        radius: f64,
        pitch: f64,
        turns: f64,
        start: f64,
        left: bool,
    ) -> Result<Shape> {
        Ok(Shape(ffi::make_helix(
            &frame.to_ffi(),
            radius,
            pitch,
            turns,
            start,
            left,
        )?))
    }

    pub fn project_onto(&self, target: &Shape, direction: DVec3) -> Result<Shape> {
        if !direction.is_finite() || direction.length() < 1e-12 {
            return Err(GeomError(
                "projection needs a nonzero finite direction".into(),
            ));
        }
        Ok(Shape(ffi::project_wire(
            &self.0,
            &target.0,
            &v3(direction),
        )?))
    }

    pub fn sweep(&self, path: &Shape, frame: &Plane, orientation: u8) -> Result<OpResult> {
        let mut history = Vec::new();
        let shape = ffi::sweep(&self.0, &path.0, &frame.to_ffi(), orientation, &mut history)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }

    /// Sweep with the section's rotation following `guide` (an auxiliary curve beside
    /// the path), or, without a guide, turning by `twist` radians from start to end.
    /// Sweep along a tangent-continuous open path so the points where the
    /// guides cross each plane normal to the path stay on the section: one
    /// guide rotates it (and scales it uniformly with `scale`), two guides
    /// stretch it. The surface passes through exact sections at stations
    /// refined until sampled profile points between them lie within
    /// 1e-5 × the model size.
    pub fn sweep_scaled(
        &self,
        path: &Shape,
        guide: &Shape,
        second: Option<&Shape>,
        scale: bool,
    ) -> Result<OpResult> {
        let mut history = Vec::new();
        let null = Shape::null();
        let shape = ffi::sweep_scaled(
            &self.0,
            &path.0,
            &guide.0,
            &second.unwrap_or(&null).0,
            scale,
            &mut history,
        )?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }

    pub fn sweep_guided(
        &self,
        path: &Shape,
        guide: Option<&Shape>,
        twist: f64,
    ) -> Result<OpResult> {
        let mut history = Vec::new();
        let null = Shape::null();
        let shape = ffi::sweep_aux(
            &self.0,
            &path.0,
            &guide.unwrap_or(&null).0,
            twist,
            &mut history,
        )?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }

    /// The start point of a path, its unit tangent there and a unit perpendicular.
    pub fn path_start(&self) -> Result<Plane> {
        let f = ffi::wire_start(&self.0)?;
        Ok(Plane::new(
            DVec3::new(f.origin.x, f.origin.y, f.origin.z),
            DVec3::new(f.x_dir.x, f.x_dir.y, f.x_dir.z),
            DVec3::new(f.normal.x, f.normal.y, f.normal.z),
        ))
    }

    pub fn loft(profiles: &[Shape], ruled: bool) -> Result<OpResult> {
        Self::loft_ordered(profiles, ruled, false)
    }

    /// A boundary solid: closed `sections` in order, with every `guides` curve
    /// crossing each section. Patch faces report `Generated` from index
    /// `span * guides.len() + gap`, where gaps follow the guides' order around
    /// the first section; end sections report start and end caps.
    pub fn boundary(sections: &[Shape], guides: &[Shape]) -> Result<OpResult> {
        let mut history = Vec::new();
        let shape = ffi::boundary(
            &Shape::compound(sections).0,
            &Shape::compound(guides).0,
            &mut history,
        )?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }

    /// The groove of an ISO 68-1 basic-profile thread on a cylinder of `radius`
    /// (the major radius outside, the minor radius inside), starting on the axis
    /// at `frame.origin`, advancing along `frame.normal`, phase at `frame.x_dir`.
    pub fn thread_tool(
        frame: &Plane,
        radius: f64,
        pitch: f64,
        length: f64,
        left_handed: bool,
        internal: bool,
    ) -> Result<Shape> {
        Ok(Shape(ffi::thread_tool(
            &frame.to_ffi(),
            radius,
            pitch,
            length,
            left_handed,
            internal,
        )?))
    }

    /// Preserve supplied section direction and seam when correspondence is explicit.
    pub fn loft_ordered(
        profiles: &[Shape],
        ruled: bool,
        preserve_correspondence: bool,
    ) -> Result<OpResult> {
        let combined = Shape::compound(profiles);
        let mut history = Vec::new();
        let shape = ffi::loft(&combined.0, ruled, preserve_correspondence, &mut history)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }

    /// Skin through closed sections with optional end tangents. The sections'
    /// own edge order and seams are kept when `preserve_correspondence` is set.
    pub fn loft_tangent(
        profiles: &[Shape],
        preserve_correspondence: bool,
        start: EndTangent,
        end: EndTangent,
    ) -> Result<OpResult> {
        let mut history = Vec::new();
        let (sm, sd, sw) = start.raw();
        let (em, ed, ew) = end.raw();
        let shape = ffi::skin(
            &Shape::compound(profiles).0,
            preserve_correspondence,
            sm,
            &v3(sd),
            sw,
            em,
            &v3(ed),
            ew,
            &mut history,
        )?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }

    /// A loft whose sections blend along `centerline`, which must cross each
    /// section's plane in order. Side faces report `Generated` from section edges.
    pub fn loft_centerline(profiles: &[Shape], centerline: &Shape) -> Result<OpResult> {
        let mut history = Vec::new();
        let shape =
            ffi::loft_centerline(&Shape::compound(profiles).0, &centerline.0, &mut history)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }
}

/// How a skinned loft leaves an end section.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EndTangent {
    Free,
    /// Along the end section's plane normal, pointing into the loft, scaled by the weight.
    Normal(f64),
    /// Along a direction, scaled by the weight.
    Direction(DVec3, f64),
}

impl EndTangent {
    fn raw(self) -> (u8, DVec3, f64) {
        match self {
            EndTangent::Free => (0, DVec3::ZERO, 1.0),
            EndTangent::Normal(w) => (1, DVec3::ZERO, w),
            EndTangent::Direction(d, w) => (2, d, w),
        }
    }
}
