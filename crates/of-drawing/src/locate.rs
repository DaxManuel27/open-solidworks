//! Model geometry named by a [`ViewRef`], in model (world) space, and where
//! it lands in its view.

use of_model::Resolved as R;
use of_model::geom::{CurveKind, DVec2, DVec3, SurfaceKind};

use crate::document::{Sheet, ViewRef};
use crate::drawing::{Drawing, Resolved};
use crate::graphics::P2;

/// Geometry an annotation can measure or point at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Geo {
    Point(DVec3),
    Line(DVec3, DVec3),
    /// A circle or arc; `ends` for arcs (counter-clockwise about `axis`).
    Circle {
        center: DVec3,
        radius: f64,
        axis: DVec3,
        ends: Option<(DVec3, DVec3)>,
    },
    Plane {
        point: DVec3,
        normal: DVec3,
    },
    /// A cylindrical face: its axis from `a` to `b`.
    Cylinder {
        a: DVec3,
        b: DVec3,
        radius: f64,
    },
}

impl Geo {
    /// A representative point: the point, a line's middle, a circle's
    /// center, a plane's point.
    pub fn anchor(&self) -> DVec3 {
        match *self {
            Geo::Point(p) => p,
            Geo::Line(a, b) => (a + b) / 2.0,
            Geo::Circle { center, .. } => center,
            Geo::Plane { point, .. } => point,
            Geo::Cylinder { a, b, .. } => (a + b) / 2.0,
        }
    }

    /// The point of the geometry nearest `p` (for leaders).
    pub fn nearest(&self, p: DVec3) -> DVec3 {
        match *self {
            Geo::Point(q) => q,
            Geo::Line(a, b) => {
                let d = b - a;
                let t = ((p - a).dot(d) / d.length_squared().max(1e-24)).clamp(0.0, 1.0);
                a + d * t
            }
            Geo::Circle {
                center,
                radius,
                axis,
                ..
            } => {
                let v = p - center;
                let v = v - axis * v.dot(axis);
                center + v.normalize_or(axis.any_orthonormal_vector()) * radius
            }
            Geo::Plane { point, normal } => p - normal * (p - point).dot(normal),
            Geo::Cylinder { a, b, radius } => {
                let d = b - a;
                let t = ((p - a).dot(d) / d.length_squared().max(1e-24)).clamp(0.0, 1.0);
                let on = a + d * t;
                let v = p - on;
                let v = v - d.normalize_or_zero() * v.dot(d.normalize_or_zero());
                on + v.normalize_or(d.any_orthonormal_vector()) * radius
            }
        }
    }
}

/// Geometry located in a view.
pub struct Located {
    pub view: Resolved,
    pub geo: Geo,
}

impl Located {
    /// View-plane position (model mm, before breaks) of a model point.
    pub fn local(&self, p: DVec3) -> DVec2 {
        self.view.frame.project(p)
    }

    /// Sheet position of a model point.
    pub fn sheet(&self, p: DVec3) -> P2 {
        self.view.to_sheet(self.local(p))
    }
}

impl Drawing {
    /// Find model geometry for an annotation.
    pub fn locate(&mut self, sheet: &Sheet, r: &ViewRef) -> Result<Located, String> {
        let view = self.resolve(sheet, r.view)?;
        let path = self.model_path(&view.model);
        let (part, pose) = self.sources.locate(
            &path,
            view.model.configuration,
            view.model.flat.is_some(),
            view.exploded,
            &r.entity,
        )?;
        let state = part.state();
        let geo = match of_model::resolve(state, &r.entity.entity).map_err(|e| e.to_string())? {
            R::Edge { info, .. } => match info.kind {
                CurveKind::Line => Geo::Line(info.start, info.end),
                CurveKind::Circle => Geo::Circle {
                    center: info.center,
                    radius: info.radius,
                    axis: info.axis.normalize_or(DVec3::Z),
                    ends: (!info.closed).then_some((info.start, info.end)),
                },
                _ => Geo::Line(info.start, info.end),
            },
            R::Vertex { point, .. } | R::Point(point) => Geo::Point(point),
            R::Face { index, info } => match info.kind {
                SurfaceKind::Cylinder | SurfaceKind::Cone => {
                    // The axis over the face's extent along it.
                    let axis = info.axis_dir.normalize_or(DVec3::Z);
                    let o = info.axis_origin;
                    let face = state
                        .body
                        .as_ref()
                        .and_then(|b| b.shape.sub_shape(of_model::geom::Kind::Face, index).ok());
                    let (lo, hi) = face
                        .and_then(|f| f.bbox())
                        .map(|bb| {
                            let mut lo = f64::MAX;
                            let mut hi = f64::MIN;
                            for i in 0..8 {
                                let c = DVec3::new(
                                    if i & 1 == 0 { bb.min.x } else { bb.max.x },
                                    if i & 2 == 0 { bb.min.y } else { bb.max.y },
                                    if i & 4 == 0 { bb.min.z } else { bb.max.z },
                                );
                                let t = (c - o).dot(axis);
                                lo = lo.min(t);
                                hi = hi.max(t);
                            }
                            (lo, hi)
                        })
                        .unwrap_or((0.0, 0.0));
                    Geo::Cylinder {
                        a: o + axis * lo,
                        b: o + axis * hi,
                        radius: info.radius,
                    }
                }
                SurfaceKind::Plane => Geo::Plane {
                    point: info.centroid,
                    normal: info.normal,
                },
                _ => Geo::Point(info.centroid),
            },
            R::Plane(p) => Geo::Plane {
                point: p.origin,
                normal: p.normal,
            },
            R::Axis { origin, dir } => Geo::Line(origin - dir * 10.0, origin + dir * 10.0),
            R::Body { center, .. } => Geo::Point(center),
            R::CoordSys { origin, .. } => Geo::Point(origin),
        };
        // Into assembly space.
        let m = |p: DVec3| pose.point(p);
        let v = |d: DVec3| pose.vector(d);
        let geo = match geo {
            Geo::Point(p) => Geo::Point(m(p)),
            Geo::Line(a, b) => Geo::Line(m(a), m(b)),
            Geo::Circle {
                center,
                radius,
                axis,
                ends,
            } => Geo::Circle {
                center: m(center),
                radius,
                axis: v(axis),
                ends: ends.map(|(a, b)| (m(a), m(b))),
            },
            Geo::Plane { point, normal } => Geo::Plane {
                point: m(point),
                normal: v(normal),
            },
            Geo::Cylinder { a, b, radius } => Geo::Cylinder {
                a: m(a),
                b: m(b),
                radius,
            },
        };
        Ok(Located { view, geo })
    }
}
