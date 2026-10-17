//! Scaling, capping openings, and deforming shapes through the control
//! points of their B-spline geometry.

use glam::DVec3;
use of_kernel_sys::ffi;

use crate::{OpResult, Result, Shape, convert_history, v3};

impl Shape {
    /// Scale about `center` by `factors` along x, y and z.
    pub fn scale(&self, center: DVec3, factors: DVec3) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::scale_shape(&self.0, &v3(center), &v3(factors), &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Planar faces closing each flat loop of open edges.
    pub fn cap_loops(&self) -> Result<Shape> {
        Ok(Shape(ffi::cap_loops(&self.0)?))
    }

    /// Deform the shape by mapping every control point of its geometry
    /// through `f`. `refine` splits each span that many times first, so the
    /// mapping is followed more closely.
    pub fn deform(&self, refine: u32, f: impl Fn(DVec3) -> DVec3) -> Result<OpResult> {
        let mut d = ffi::deformable_new(&self.0, refine.max(1))?;
        let points = ffi::deformable_points(&d);
        let moved: Vec<f64> = points
            .chunks_exact(3)
            .flat_map(|p| {
                let q = f(DVec3::new(p[0], p[1], p[2]));
                [q.x, q.y, q.z]
            })
            .collect();
        if moved.iter().any(|v| !v.is_finite()) {
            return Err(crate::GeomError("the deformation isn't finite".into()));
        }
        let mut h = Vec::new();
        let s = ffi::deformable_apply(d.pin_mut(), &moved, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }
}
