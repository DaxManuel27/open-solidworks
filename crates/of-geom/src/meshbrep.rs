//! Meshes as B-rep, and faces fitted to scan data.

use glam::DVec3;
use of_kernel_sys::ffi;

use crate::{Plane, Result, Shape};

/// An analytic surface for a fitted face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Analytic {
    Plane,
    Cylinder {
        radius: f64,
    },
    /// `radius` where the cone crosses the frame's plane.
    Cone {
        radius: f64,
        half_angle: f64,
    },
    Sphere {
        radius: f64,
    },
    Torus {
        major: f64,
        minor: f64,
    },
}

impl Shape {
    /// A shell of planar triangles sharing their edges.
    pub fn mesh_brep(points: &[DVec3], triangles: &[[u32; 3]]) -> Result<Shape> {
        let p: Vec<f64> = points.iter().flat_map(|v| [v.x, v.y, v.z]).collect();
        let t: Vec<u32> = triangles.iter().flatten().copied().collect();
        Ok(Shape(ffi::mesh_brep(&p, &t)?))
    }

    /// A solid of a closed mesh B-rep shell whose triangles all face the
    /// same way, turned outward. Unlike [`Shape::solidify`] it doesn't
    /// repair the shell, which takes minutes at 100k faces.
    pub fn oriented_solid(&self) -> Result<crate::OpResult> {
        let mut h = Vec::new();
        let s = ffi::oriented_solid(&self.0, &mut h)?;
        Ok(crate::OpResult {
            shape: Shape(s),
            history: crate::convert_history(h),
        })
    }

    /// An analytic face in `frame` (origin, axis as normal, x direction)
    /// over the parameter box `[u0, u1, v0, v1]`.
    pub fn analytic_face(surface: Analytic, frame: &Plane, bounds: [f64; 4]) -> Result<Shape> {
        let (kind, r1, r2) = match surface {
            Analytic::Plane => (1, 0.0, 0.0),
            Analytic::Cylinder { radius } => (2, radius, 0.0),
            Analytic::Cone { radius, half_angle } => (3, radius, half_angle),
            Analytic::Sphere { radius } => (4, radius, 0.0),
            Analytic::Torus { major, minor } => (5, major, minor),
        };
        Ok(Shape(ffi::primitive_face(
            kind,
            &frame.to_ffi(),
            r1,
            r2,
            &bounds,
        )?))
    }

    /// A smooth face through a grid of points given row by row.
    pub fn bspline_face(rows: &[Vec<DVec3>]) -> Result<Shape> {
        let nv = rows.len() as u32;
        let nu = rows.first().map_or(0, |r| r.len()) as u32;
        let p: Vec<f64> = rows
            .iter()
            .flatten()
            .flat_map(|v| [v.x, v.y, v.z])
            .collect();
        Ok(Shape(ffi::bspline_face(&p, nu, nv)?))
    }
}
