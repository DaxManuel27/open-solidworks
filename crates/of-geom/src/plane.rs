use glam::{DAffine3, DMat3, DVec2, DVec3};
use of_kernel_sys::ffi;
use serde::{Deserialize, Serialize};

use crate::v3;

/// A plane with its own 2D coordinate system, used for sketches.
/// `y()` is `normal × x_dir`, so the plane is right-handed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Plane {
    pub origin: DVec3,
    pub x_dir: DVec3,
    pub normal: DVec3,
}

impl Plane {
    /// Build a plane, making the axes unit length and orthogonal. Picks an
    /// X direction when `x_dir` is missing or parallel to the normal.
    pub fn new(origin: DVec3, x_dir: DVec3, normal: DVec3) -> Plane {
        let n = normal.normalize_or(DVec3::Z);
        let mut x = x_dir - n * x_dir.dot(n);
        if x.length_squared() < 1e-18 {
            x = n.any_orthonormal_vector();
        }
        Plane {
            origin,
            x_dir: x.normalize(),
            normal: n,
        }
    }

    /// The XY plane, normal +Z.
    pub fn front() -> Plane {
        Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z)
    }

    /// The XZ plane, normal +Y.
    pub fn top() -> Plane {
        Plane::new(DVec3::ZERO, DVec3::X, DVec3::Y)
    }

    /// The YZ plane, normal +X.
    pub fn right() -> Plane {
        Plane::new(DVec3::ZERO, DVec3::NEG_Z, DVec3::X)
    }

    pub fn y_dir(&self) -> DVec3 {
        self.normal.cross(self.x_dir)
    }

    pub fn to_world(&self, p: DVec2) -> DVec3 {
        self.origin + self.x_dir * p.x + self.y_dir() * p.y
    }

    pub fn to_local(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        DVec2::new(d.dot(self.x_dir), d.dot(self.y_dir()))
    }

    /// Signed distance from the plane along its normal.
    pub fn distance(&self, p: DVec3) -> f64 {
        (p - self.origin).dot(self.normal)
    }

    pub fn project(&self, p: DVec3) -> DVec3 {
        p - self.normal * self.distance(p)
    }

    /// The plane moved `d` along its normal.
    pub fn offset(&self, d: f64) -> Plane {
        Plane {
            origin: self.origin + self.normal * d,
            ..*self
        }
    }

    /// The same plane facing the other way (X kept, so Y flips).
    pub fn flipped(&self) -> Plane {
        Plane {
            normal: -self.normal,
            ..*self
        }
    }

    /// Maps plane-local coordinates (x, y, along normal) to world space.
    pub fn to_affine(&self) -> DAffine3 {
        DAffine3::from_mat3_translation(
            DMat3::from_cols(self.x_dir, self.y_dir(), self.normal),
            self.origin,
        )
    }

    pub fn transformed(&self, t: &DAffine3) -> Plane {
        Plane::new(
            t.transform_point3(self.origin),
            t.transform_vector3(self.x_dir),
            t.transform_vector3(self.normal),
        )
    }

    pub(crate) fn to_ffi(self) -> ffi::Frame {
        ffi::Frame {
            origin: v3(self.origin),
            x_dir: v3(self.x_dir),
            normal: v3(self.normal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_planes_are_right_handed() {
        assert_eq!(Plane::front().y_dir(), DVec3::Y);
        assert_eq!(Plane::top().y_dir(), DVec3::NEG_Z);
        assert_eq!(Plane::right().y_dir(), DVec3::Y);
    }

    #[test]
    fn local_world_round_trip() {
        let p = Plane::new(
            DVec3::new(1.0, 2.0, 3.0),
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::Z,
        );
        let q = DVec2::new(3.5, -2.0);
        assert!((p.to_local(p.to_world(q)) - q).length() < 1e-12);
    }
}
