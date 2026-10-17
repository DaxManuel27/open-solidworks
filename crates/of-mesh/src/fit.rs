//! Fitting analytic surfaces to points: planes, spheres, cylinders, cones and
//! tori by least squares, with the distances of the points from the result.
//! Also a regular grid of points over a region, for freeform fits.

use glam::{DVec2, DVec3};
use nalgebra::{DMatrix, DVector, Matrix3, SymmetricEigen};

use crate::bvh::Bvh;
use crate::{Result, TriMesh, invalid};

/// A fitted surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Primitive {
    Plane {
        origin: DVec3,
        normal: DVec3,
    },
    Sphere {
        center: DVec3,
        radius: f64,
    },
    Cylinder {
        origin: DVec3,
        axis: DVec3,
        radius: f64,
    },
    /// A cone: its apex, axis (pointing into the cone) and half angle.
    Cone {
        apex: DVec3,
        axis: DVec3,
        half_angle: f64,
    },
    Torus {
        center: DVec3,
        axis: DVec3,
        major: f64,
        minor: f64,
    },
}

/// A fit and how well it matches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub primitive: Primitive,
    pub rms: f64,
    pub max: f64,
}

impl Primitive {
    /// Unsigned distance from a point to the surface.
    pub fn distance(&self, p: DVec3) -> f64 {
        match *self {
            Primitive::Plane { origin, normal } => (p - origin).dot(normal).abs(),
            Primitive::Sphere { center, radius } => ((p - center).length() - radius).abs(),
            Primitive::Cylinder {
                origin,
                axis,
                radius,
            } => {
                let d = p - origin;
                ((d - axis * d.dot(axis)).length() - radius).abs()
            }
            Primitive::Cone {
                apex,
                axis,
                half_angle,
            } => {
                let d = p - apex;
                let t = d.dot(axis);
                let r = (d - axis * t).length();
                // Distance to the line at half_angle from the axis in this plane.
                (r * half_angle.cos() - t * half_angle.sin()).abs()
            }
            Primitive::Torus {
                center,
                axis,
                major,
                minor,
            } => {
                let d = p - center;
                let z = d.dot(axis);
                let r = (d - axis * z).length();
                (((r - major).powi(2) + z * z).sqrt() - minor).abs()
            }
        }
    }
}

fn mean(points: &[DVec3]) -> DVec3 {
    points.iter().copied().sum::<DVec3>() / points.len() as f64
}

/// Eigenvectors of the covariance of `v` about `center`, smallest first.
fn principal(v: &[DVec3], center: DVec3) -> ([f64; 3], [DVec3; 3]) {
    let mut m = Matrix3::zeros();
    for p in v {
        let d = *p - center;
        let d = nalgebra::Vector3::new(d.x, d.y, d.z);
        m += d * d.transpose();
    }
    let e = SymmetricEigen::new(m);
    let mut order: Vec<usize> = (0..3).collect();
    order.sort_by(|a, b| e.eigenvalues[*a].total_cmp(&e.eigenvalues[*b]));
    let vec = |i: usize| {
        let c = e.eigenvectors.column(i);
        DVec3::new(c[0], c[1], c[2]).normalize()
    };
    (
        [
            e.eigenvalues[order[0]],
            e.eigenvalues[order[1]],
            e.eigenvalues[order[2]],
        ],
        [vec(order[0]), vec(order[1]), vec(order[2])],
    )
}

fn quality(primitive: Primitive, points: &[DVec3]) -> Fit {
    let d: Vec<f64> = points.iter().map(|p| primitive.distance(*p)).collect();
    let rms = (d.iter().map(|x| x * x).sum::<f64>() / d.len() as f64).sqrt();
    let max = d.iter().copied().fold(0.0, f64::max);
    Fit {
        primitive,
        rms,
        max,
    }
}

pub fn plane(points: &[DVec3]) -> Result<Fit> {
    if points.len() < 3 {
        return Err(invalid("a plane needs three points"));
    }
    let c = mean(points);
    let (_, v) = principal(points, c);
    Ok(quality(
        Primitive::Plane {
            origin: c,
            normal: v[0],
        },
        points,
    ))
}

pub fn sphere(points: &[DVec3]) -> Result<Fit> {
    if points.len() < 4 {
        return Err(invalid("a sphere needs four points"));
    }
    // x² + y² + z² + D x + E y + F z + G = 0, about the centroid for conditioning.
    let c0 = mean(points);
    let a = DMatrix::from_fn(points.len(), 4, |r, k| {
        let p = points[r] - c0;
        [p.x, p.y, p.z, 1.0][k]
    });
    let b = DVector::from_fn(points.len(), |r, _| -(points[r] - c0).length_squared());
    let x = a
        .svd(true, true)
        .solve(&b, 1e-12)
        .map_err(|_| invalid("the points don't fit a sphere"))?;
    let center = DVec3::new(-x[0] / 2.0, -x[1] / 2.0, -x[2] / 2.0);
    let radius = (center.length_squared() - x[3]).max(0.0).sqrt();
    Ok(quality(
        Primitive::Sphere {
            center: center + c0,
            radius,
        },
        points,
    ))
}

/// The point nearest, in least squares, to every line (p_i, d_i) projected
/// on the plane square to `axis`.
fn lines_meet(points: &[DVec3], dirs: &[DVec3], axis: DVec3) -> Option<DVec3> {
    let x = axis.any_orthonormal_vector();
    let y = axis.cross(x);
    let mut m = nalgebra::Matrix2::zeros();
    let mut rhs = nalgebra::Vector2::zeros();
    for (p, d) in points.iter().zip(dirs) {
        let p2 = DVec2::new(p.dot(x), p.dot(y));
        let d2 = DVec2::new(d.dot(x), d.dot(y));
        if d2.length() < 1e-9 {
            continue;
        }
        let d2 = d2.normalize();
        let proj = nalgebra::Matrix2::identity()
            - nalgebra::Matrix2::new(d2.x * d2.x, d2.x * d2.y, d2.y * d2.x, d2.y * d2.y);
        m += proj;
        rhs += proj * nalgebra::Vector2::new(p2.x, p2.y);
    }
    let c = m.try_inverse()? * rhs;
    Some(x * c[0] + y * c[1])
}

/// Cylinders and cones need normals; take them from the mesh.
pub fn cylinder(points: &[DVec3], normals: &[DVec3]) -> Result<Fit> {
    if points.len() < 6 {
        return Err(invalid("a cylinder needs more points"));
    }
    // Normals are square to the axis: the axis is their least spread direction.
    let (_, v) = principal(normals, DVec3::ZERO);
    let axis = v[0];
    let origin = lines_meet(points, normals, axis)
        .ok_or_else(|| invalid("the points don't fit a cylinder"))?;
    let radius = points
        .iter()
        .map(|p| {
            let d = *p - origin;
            (d - axis * d.dot(axis)).length()
        })
        .sum::<f64>()
        / points.len() as f64;
    Ok(quality(
        Primitive::Cylinder {
            origin,
            axis,
            radius,
        },
        points,
    ))
}

pub fn cone(points: &[DVec3], normals: &[DVec3]) -> Result<Fit> {
    if points.len() < 6 {
        return Err(invalid("a cone needs more points"));
    }
    // Normals make one angle with the axis: the axis is the direction their
    // spread is smallest along, about their mean.
    let (_, v) = principal(normals, mean(normals));
    let mut axis = v[0];
    let origin =
        lines_meet(points, normals, axis).ok_or_else(|| invalid("the points don't fit a cone"))?;
    // Radius against height along the axis is a line: r = k t + b.
    let (mut st, mut sr, mut stt, mut str) = (0.0, 0.0, 0.0, 0.0);
    for p in points {
        let d = *p - origin;
        let t = d.dot(axis);
        let r = (d - axis * t).length();
        st += t;
        sr += r;
        stt += t * t;
        str += t * r;
    }
    let n = points.len() as f64;
    let den = n * stt - st * st;
    if den.abs() < 1e-12 {
        return Err(invalid("the points don't fit a cone"));
    }
    let k = (n * str - st * sr) / den;
    let b = (sr - k * st) / n;
    if k.abs() < 1e-9 {
        return Err(invalid("the points fit a cylinder, not a cone"));
    }
    // The apex is where the radius is zero; the axis points into the cone.
    let apex = origin + axis * (-b / k);
    if k < 0.0 {
        axis = -axis;
    }
    let k = k.abs();
    Ok(quality(
        Primitive::Cone {
            apex,
            axis,
            half_angle: k.atan(),
        },
        points,
    ))
}

pub fn torus(points: &[DVec3]) -> Result<Fit> {
    if points.len() < 10 {
        return Err(invalid("a torus needs more points"));
    }
    let c = mean(points);
    let (_, v) = principal(points, c);
    let mut center = c;
    let mut axis = v[0];
    let (mut major, mut minor) = {
        let r: Vec<f64> = points
            .iter()
            .map(|p| {
                let d = *p - c;
                (d - axis * d.dot(axis)).length()
            })
            .collect();
        let mean_r = r.iter().sum::<f64>() / r.len() as f64;
        let spread = (r.iter().map(|x| (x - mean_r).powi(2)).sum::<f64>() / r.len() as f64).sqrt();
        (mean_r, (spread * 1.4).max(1e-3))
    };
    // Gauss-Newton on (center, axis tilt, major, minor) with a numeric Jacobian.
    for _ in 0..30 {
        let x = axis.any_orthonormal_vector();
        let y = axis.cross(x);
        let params = [0.0; 7];
        let residual = |q: &[f64; 7]| -> Vec<f64> {
            let c = center + x * q[0] + y * q[1] + axis * q[2];
            let a = (axis + x * q[3] + y * q[4]).normalize();
            let prim = Primitive::Torus {
                center: c,
                axis: a,
                major: major + q[5],
                minor: minor + q[6],
            };
            points
                .iter()
                .map(|p| {
                    let d = *p - c;
                    let z = d.dot(a);
                    let r = (d - a * z).length();
                    let _ = prim;
                    ((r - (major + q[5])).powi(2) + z * z).sqrt() - (minor + q[6])
                })
                .collect()
        };
        let r0 = residual(&params);
        let mut jac = DMatrix::zeros(points.len(), 7);
        for k in 0..7 {
            let mut q = params;
            q[k] = 1e-6 * (1.0 + major);
            let r1 = residual(&q);
            for i in 0..points.len() {
                jac[(i, k)] = (r1[i] - r0[i]) / q[k];
            }
        }
        let r0v = DVector::from_vec(r0.clone());
        let Ok(step) = jac.clone().svd(true, true).solve(&(-r0v), 1e-12) else {
            break;
        };
        center += x * step[0] + y * step[1] + axis * step[2];
        axis = (axis + x * step[3] + y * step[4]).normalize();
        major += step[5];
        minor += step[6];
        if step.norm() < 1e-10 * (1.0 + major) {
            break;
        }
    }
    if !(major > 0.0 && minor > 0.0) {
        return Err(invalid("the points don't fit a torus"));
    }
    Ok(quality(
        Primitive::Torus {
            center,
            axis,
            major,
            minor,
        },
        points,
    ))
}

/// The best of every primitive that fits: the lowest RMS, preferring
/// simpler shapes when they are nearly as good.
pub fn best(points: &[DVec3], normals: &[DVec3]) -> Result<Fit> {
    let mut fits = Vec::new();
    fits.extend(plane(points).ok());
    fits.extend(sphere(points).ok());
    fits.extend(cylinder(points, normals).ok());
    fits.extend(cone(points, normals).ok());
    fits.extend(torus(points).ok());
    let scale = points
        .iter()
        .fold(crate::Bounds::empty(), |mut b, p| {
            b.add(*p);
            b
        })
        .diagonal()
        .max(1e-9);
    // Complexity order: plane, sphere, cylinder, cone, torus. A more complex
    // shape must beat a simpler one clearly.
    fits.into_iter()
        .reduce(|a, b| {
            if b.rms < a.rms * 0.5 - 1e-9 * scale {
                b
            } else {
                a
            }
        })
        .ok_or_else(|| invalid("nothing fits the points"))
}

/// A regular `n` × `n` grid of points on a mesh region, seen along its
/// best-fit plane's normal, for fitting a freeform surface. Grid points the
/// mesh doesn't cover take the nearest point instead.
pub fn grid(region: &TriMesh, n: usize) -> Result<Vec<Vec<DVec3>>> {
    let pf = plane(&region.positions)?;
    let Primitive::Plane { origin, normal } = pf.primitive else {
        unreachable!()
    };
    let x = normal.any_orthonormal_vector();
    let y = normal.cross(x);
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for p in &region.positions {
        let q = DVec2::new((*p - origin).dot(x), (*p - origin).dot(y));
        lo = lo.min(q);
        hi = hi.max(q);
    }
    let bvh = Bvh::new(region);
    let depth = region.bounds().diagonal() * 2.0 + 1.0;
    let n = n.max(4);
    let mut rows = Vec::with_capacity(n);
    for j in 0..n {
        let mut row = Vec::with_capacity(n);
        for i in 0..n {
            let u = lo.x + (hi.x - lo.x) * i as f64 / (n - 1) as f64;
            let v = lo.y + (hi.y - lo.y) * j as f64 / (n - 1) as f64;
            let p = origin + x * u + y * v;
            let hit = bvh
                .ray(p - normal * depth, normal, 0.0)
                .map(|h| h.point)
                .or_else(|| bvh.closest(p).map(|c| c.point))
                .ok_or_else(|| invalid("the region is empty"))?;
            row.push(hit);
        }
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn on_cylinder() -> (Vec<DVec3>, Vec<DVec3>) {
        let mut p = Vec::new();
        let mut n = Vec::new();
        for i in 0..40 {
            for j in 0..5 {
                let a = TAU * i as f64 / 40.0 * 0.7;
                let d = DVec3::new(a.cos(), a.sin(), 0.0);
                p.push(DVec3::new(1.0, 2.0, 0.0) + d * 4.0 + DVec3::Z * j as f64);
                n.push(d);
            }
        }
        (p, n)
    }

    #[test]
    fn fits_each_primitive() {
        let (p, n) = on_cylinder();
        let f = cylinder(&p, &n).unwrap();
        let Primitive::Cylinder { radius, axis, .. } = f.primitive else {
            panic!()
        };
        assert!((radius - 4.0).abs() < 1e-9 && axis.cross(DVec3::Z).length() < 1e-9);
        assert!(f.max < 1e-9);
        assert!(matches!(
            best(&p, &n).unwrap().primitive,
            Primitive::Cylinder { .. }
        ));

        let sp: Vec<DVec3> = (0..50)
            .map(|i| {
                let a = i as f64 * 0.7;
                let b = i as f64 * 0.31;
                DVec3::new(a.cos() * b.sin(), a.sin() * b.sin(), b.cos()) * 3.0 + DVec3::X
            })
            .collect();
        let s = sphere(&sp).unwrap();
        assert!(s.max < 1e-9, "{s:?}");

        // A cone of half angle 30° with its apex at the origin along +z.
        let mut cp = Vec::new();
        let mut cn = Vec::new();
        let h = 30f64.to_radians();
        for i in 0..36 {
            for j in 1..6 {
                let a = TAU * i as f64 / 36.0;
                let t = j as f64;
                let radial = DVec3::new(a.cos(), a.sin(), 0.0);
                cp.push(DVec3::Z * t + radial * t * h.tan());
                cn.push((radial * h.cos() - DVec3::Z * h.sin()).normalize());
            }
        }
        let c = cone(&cp, &cn).unwrap();
        let Primitive::Cone {
            half_angle, apex, ..
        } = c.primitive
        else {
            panic!()
        };
        assert!((half_angle - h).abs() < 1e-6, "{half_angle}");
        assert!(apex.length() < 1e-6, "{apex:?}");

        let mut tp = Vec::new();
        for i in 0..30 {
            for j in 0..12 {
                let u = TAU * i as f64 / 30.0;
                let v = TAU * j as f64 / 12.0;
                let r = 10.0 + 2.0 * v.cos();
                tp.push(DVec3::new(r * u.cos(), r * u.sin(), 2.0 * v.sin()));
            }
        }
        let t = torus(&tp).unwrap();
        assert!(t.max < 1e-6, "{t:?}");
    }
}
