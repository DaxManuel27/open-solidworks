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
    pub fn split_face(&self, face: u32, wire: &Shape) -> Result<OpResult> {
        let mut history = Vec::new();
        let shape = ffi::split_face(&self.0, face, &wire.0, &mut history)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }
    pub fn variable_fillet(&self, edge: u32, stations: &[(f64, f64)]) -> Result<OpResult> {
        let stations: Vec<_> = stations.iter().flat_map(|(u, r)| [*u, *r]).collect();
        let mut history = Vec::new();
        let shape = ffi::variable_fillet(&self.0, edge, &stations, &mut history)?;
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

    pub fn loft(profiles: &[Shape], ruled: bool) -> Result<OpResult> {
        let combined = Shape::compound(profiles);
        let mut history = Vec::new();
        let shape = ffi::loft(&combined.0, ruled, &mut history)?;
        Ok(OpResult {
            shape: Shape(shape),
            history: convert_history(history),
        })
    }
}
