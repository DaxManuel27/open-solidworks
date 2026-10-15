//! Surface parameter frames and flat patterns for sheet metal.

use glam::{DVec2, DVec3};
use of_kernel_sys::ffi;

use crate::{Plane, Result, Shape, dv3, v3};

/// The surface a face lies on, with its (u, v) parameterisation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FaceSurface {
    /// `origin + u x + v y`.
    Plane,
    /// `origin + radius (cos u x + sin u y) + v z`.
    Cylinder {
        radius: f64,
    },
    /// `origin + (radius + v sin a)(cos u x + sin u y) + v cos a z`.
    Cone {
        radius: f64,
        semi_angle: f64,
    },
    Other,
}

/// A face's surface frame and parameter bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceFrame {
    pub surface: FaceSurface,
    pub origin: DVec3,
    pub x: DVec3,
    pub y: DVec3,
    pub z: DVec3,
    pub u: (f64, f64),
    pub v: (f64, f64),
    /// The face normal is opposite to the surface normal (`x × y` for a
    /// plane, outward from the axis for a cylinder).
    pub reversed: bool,
}

impl FaceFrame {
    /// The point at parameters (u, v).
    pub fn point(&self, u: f64, v: f64) -> DVec3 {
        match self.surface {
            FaceSurface::Plane => self.origin + self.x * u + self.y * v,
            FaceSurface::Cylinder { radius } => {
                self.origin + (self.x * u.cos() + self.y * u.sin()) * radius + self.z * v
            }
            FaceSurface::Cone { radius, semi_angle } => {
                self.origin
                    + (self.x * u.cos() + self.y * u.sin()) * (radius + v * semi_angle.sin())
                    + self.z * (v * semi_angle.cos())
            }
            FaceSurface::Other => self.origin,
        }
    }

    /// The outward normal of the face at (u, v).
    pub fn normal(&self, u: f64, v: f64) -> DVec3 {
        let around = self.y * u.cos() - self.x * u.sin();
        let n = match self.surface {
            FaceSurface::Plane => self.x.cross(self.y),
            FaceSurface::Cylinder { .. } => around.cross(self.z),
            FaceSurface::Cone { radius, semi_angle } => {
                let du = around * (radius + v * semi_angle.sin());
                let dv = (self.x * u.cos() + self.y * u.sin()) * semi_angle.sin()
                    + self.z * semi_angle.cos();
                du.cross(dv)
            }
            FaceSurface::Other => DVec3::Z,
        }
        .normalize_or_zero();
        if self.reversed { -n } else { n }
    }

    /// The angle parameter of a point around a cylinder or cone axis.
    pub fn angle_of(&self, p: DVec3) -> f64 {
        let d = p - self.origin;
        d.dot(self.y).atan2(d.dot(self.x))
    }

    /// The axial parameter of a point on a cylinder.
    pub fn height_of(&self, p: DVec3) -> f64 {
        (p - self.origin).dot(self.z)
    }
}

/// A 2D affine map from a face's (u, v) parameters to a plane's (x, y):
/// `x = a u + b v + c`, `y = d u + e v + f`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvMap(pub [f64; 6]);

impl UvMap {
    pub fn apply(&self, uv: DVec2) -> DVec2 {
        let [a, b, c, d, e, f] = self.0;
        DVec2::new(a * uv.x + b * uv.y + c, d * uv.x + e * uv.y + f)
    }
}

/// Where each face of a flat pattern came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatSource {
    /// Extruded from this edge of the formed shape.
    Edge(u32),
    /// The face in the flat pattern's plane.
    Plane,
    /// The face opposite it, one thickness away.
    Back,
    Unknown,
}

impl Shape {
    /// The inverse of [`Shape::flatten`]: the parts of `region` (planar faces
    /// in `plane`) over each face's flat image, mapped back onto that face's
    /// surface, with the face each piece lies on.
    pub fn fold(
        &self,
        faces: &[(u32, UvMap)],
        plane: &Plane,
        region: &Shape,
    ) -> Result<Vec<(Shape, u32)>> {
        let indices: Vec<u32> = faces.iter().map(|f| f.0).collect();
        let maps: Vec<f64> = faces.iter().flat_map(|f| f.1.0).collect();
        let mut owners = Vec::new();
        let all = ffi::fold_faces(
            &self.0,
            &indices,
            &maps,
            &plane.to_ffi(),
            &region.0,
            &mut owners,
        )?;
        let all = Shape(all);
        owners
            .iter()
            .enumerate()
            .map(|(i, o)| Ok((all.sub_shape(crate::Kind::Face, i as u32)?, *o)))
            .collect()
    }

    /// A solid bounded by planar polygons, each a list of indices into
    /// `points`. Faces are sewn together; their order in the result may differ.
    pub fn polyhedron(points: &[DVec3], faces: &[Vec<u32>]) -> Result<Shape> {
        let xyz: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
        let mut list = Vec::new();
        for f in faces {
            list.push(f.len() as u32);
            list.extend(f);
        }
        Ok(Shape(ffi::polyhedron(&xyz, &list)?))
    }

    pub fn face_frame(&self, face: u32) -> Result<FaceFrame> {
        let f = ffi::face_frame(&self.0, face)?;
        Ok(FaceFrame {
            surface: match f.kind {
                1 => FaceSurface::Plane,
                2 => FaceSurface::Cylinder { radius: f.radius },
                3 => FaceSurface::Cone {
                    radius: f.radius,
                    semi_angle: f.semi_angle,
                },
                _ => FaceSurface::Other,
            },
            origin: dv3(f.origin),
            x: dv3(f.x),
            y: dv3(f.y),
            z: dv3(f.z),
            u: (f.umin, f.umax),
            v: (f.vmin, f.vmax),
            reversed: f.reversed,
        })
    }

    /// Map `faces` into `plane` by their (u, v) maps, join them into one
    /// region and extrude it by `thickness`. Returns the solid and, per face
    /// of it, where it came from.
    pub fn flatten(
        &self,
        faces: &[(u32, UvMap)],
        plane: &Plane,
        thickness: DVec3,
    ) -> Result<(Shape, Vec<FlatSource>)> {
        let indices: Vec<u32> = faces.iter().map(|f| f.0).collect();
        let maps: Vec<f64> = faces.iter().flat_map(|f| f.1.0).collect();
        let mut sources = Vec::new();
        let s = ffi::flatten(
            &self.0,
            &indices,
            &maps,
            &plane.to_ffi(),
            &v3(thickness),
            &mut sources,
        )?;
        let sources = sources
            .into_iter()
            .map(|s| match s {
                -1 => FlatSource::Plane,
                -2 => FlatSource::Back,
                s if s >= 0 => FlatSource::Edge(s as u32),
                _ => FlatSource::Unknown,
            })
            .collect();
        Ok((Shape(s), sources))
    }
}
