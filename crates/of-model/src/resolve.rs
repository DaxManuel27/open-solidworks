//! Turning [`Ref`]s into geometry, against the model as it is at a point in
//! the feature tree.

use of_geom::{CurveKind, DVec3, EdgeInfo, FaceInfo, Kind, Plane, SurfaceKind};
use of_sketch::Geometry;

use crate::build::{Datum, State};
use crate::{Ref, unit};

/// What a reference points at.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolved {
    Body {
        id: crate::BodyId,
        center: DVec3,
    },
    Plane(Plane),
    Axis {
        origin: DVec3,
        dir: DVec3,
    },
    Point(DVec3),
    Face {
        index: u32,
        info: FaceInfo,
    },
    Edge {
        index: u32,
        info: EdgeInfo,
    },
    Vertex {
        index: u32,
        point: DVec3,
    },
    CoordSys {
        origin: DVec3,
        x: DVec3,
        y: DVec3,
        z: DVec3,
    },
}

pub(crate) type RResult<T> = Result<T, String>;

/// Resolve a reference against a model state.
pub fn resolve(state: &State, r: &Ref) -> RResult<Resolved> {
    let missing = |what: &str, name: &str| format!("{what} {name} no longer exists");
    Ok(match r {
        Ref::Body { id } => {
            let body = state.solid(id)?;
            Resolved::Body {
                id: id.clone(),
                center: body.geometry.shape.mass_props().map_err(|e| e.0)?.center,
            }
        }
        Ref::BodyTopology { body, kind, name } => {
            let selected = &state.solid(body)?.geometry;
            let local = match kind {
                Kind::Face => selected
                    .names
                    .faces
                    .iter()
                    .position(|n| n.first() == Some(name))
                    .map(|i| i as u32),
                Kind::Edge => selected
                    .names
                    .edges
                    .iter()
                    .position(|n| n == name)
                    .map(|i| i as u32),
                Kind::Vertex => selected
                    .names
                    .vertices
                    .iter()
                    .position(|n| n == name)
                    .map(|i| i as u32),
                _ => None,
            }
            .ok_or_else(|| missing("body topology", name))?;
            let aggregate = state.body.as_ref().ok_or("there is no body")?;
            let indices = selected
                .shape
                .indices_in(&aggregate.shape, *kind)
                .map_err(|e| e.0)?;
            let index = indices[local as usize];
            match kind {
                Kind::Face => Resolved::Face {
                    index,
                    info: aggregate.shape.face_info(index).map_err(|e| e.0)?,
                },
                Kind::Edge => Resolved::Edge {
                    index,
                    info: aggregate.shape.edge_info(index).map_err(|e| e.0)?,
                },
                Kind::Vertex => Resolved::Vertex {
                    index,
                    point: aggregate.shape.vertex_point(index).map_err(|e| e.0)?,
                },
                _ => return Err("expected a face, edge or vertex".into()),
            }
        }
        Ref::Origin => Resolved::Point(DVec3::ZERO),
        Ref::FrontPlane => Resolved::Plane(Plane::front()),
        Ref::TopPlane => Resolved::Plane(Plane::top()),
        Ref::RightPlane => Resolved::Plane(Plane::right()),
        Ref::Face { name } => {
            let body = state.body.as_ref().ok_or_else(|| missing("face", name))?;
            let index = body.names.face(name).ok_or_else(|| missing("face", name))?;
            Resolved::Face {
                index,
                info: body.shape.face_info(index).map_err(|e| e.0)?,
            }
        }
        Ref::Edge { name } => {
            let body = state.body.as_ref().ok_or_else(|| missing("edge", name))?;
            let index = body.names.edge(name).ok_or_else(|| missing("edge", name))?;
            Resolved::Edge {
                index,
                info: body.shape.edge_info(index).map_err(|e| e.0)?,
            }
        }
        Ref::Vertex { name } => {
            let body = state.body.as_ref().ok_or_else(|| missing("vertex", name))?;
            let index = body
                .names
                .vertex(name)
                .ok_or_else(|| missing("vertex", name))?;
            Resolved::Vertex {
                index,
                point: body.shape.vertex_point(index).map_err(|e| e.0)?,
            }
        }
        Ref::Feature { id } => {
            if let Some(d) = state.datums.get(id) {
                match *d {
                    Datum::Plane(p) => Resolved::Plane(p),
                    Datum::Axis { origin, dir } => Resolved::Axis { origin, dir },
                    Datum::Point(p) => Resolved::Point(p),
                    Datum::CoordSys { origin, x, y, z } => Resolved::CoordSys { origin, x, y, z },
                }
            } else if let Some((plane, _)) = state.sketches.get(id) {
                Resolved::Plane(*plane)
            } else {
                return Err(format!("referenced feature {id} is missing or failed"));
            }
        }
        Ref::SketchEntity { sketch, entity } => {
            if let Some((sk, report)) = state.sketches3d.get(sketch) {
                if report.status == of_sketch::SolveStatus::Failed {
                    return Err("the referenced 3D sketch cannot be solved".into());
                }
                return match sk.entity(*entity)?.geometry {
                    of_sketch::spatial::Geometry3::Point { at } => Ok(Resolved::Point(at)),
                    of_sketch::spatial::Geometry3::Line { a, b } => Ok(Resolved::Axis {
                        origin: sk.point(a)?,
                        dir: unit(sk.point(b)? - sk.point(a)?),
                    }),
                    _ => Err("select a 3D sketch point or straight line".into()),
                };
            }
            let (plane, sk) = state
                .sketches
                .get(sketch)
                .ok_or_else(|| format!("sketch {sketch} is missing"))?;
            let e = sk.entity(*entity).map_err(|e| e.to_string())?;
            match &e.geo {
                Geometry::Point { p } => Resolved::Point(plane.to_world(*p)),
                Geometry::Line { a, b } => {
                    let (a, b) = (sk.point(*a).unwrap(), sk.point(*b).unwrap());
                    let origin = plane.to_world(a);
                    Resolved::Axis {
                        origin,
                        dir: unit(plane.to_world(b) - origin),
                    }
                }
                Geometry::Circle { center, .. } | Geometry::Arc { center, .. } => Resolved::Axis {
                    origin: plane.to_world(sk.point(*center).unwrap()),
                    dir: plane.normal,
                },
                Geometry::Spline { .. } | Geometry::Bezier { .. } => {
                    return Err("a spline can't be used here".into());
                }
            }
        }
        Ref::CurveSegment { .. } => {
            return Err("a curve segment is a path, not a datum reference".into());
        }
        Ref::OrderedSketchContour(c) => {
            let (plane, _) = crate::curves::ordered_contour(state, c, false)?;
            Resolved::Plane(plane)
        }
        Ref::SketchContour { sketch, entities } => {
            let (plane, _) = crate::curves::sketch_contour(state, *sketch, entities)?;
            Resolved::Plane(plane)
        }
    })
}

impl Resolved {
    /// A plane: reference planes, planar faces, sketches, or a coordinate
    /// system's XY plane.
    pub fn plane(&self) -> RResult<Plane> {
        match self {
            Resolved::Plane(p) => Ok(*p),
            Resolved::Face { info, .. } if info.kind == SurfaceKind::Plane => {
                // Keep the face's orientation; put the origin where the world
                // origin projects onto it so sketches line up with the model.
                let n = unit(info.normal);
                let origin = DVec3::ZERO - n * (DVec3::ZERO - info.point).dot(n);
                Ok(Plane::new(origin, info.x_dir, n))
            }
            Resolved::CoordSys { origin, x, z, .. } => Ok(Plane::new(*origin, *x, *z)),
            _ => Err("select a plane or a flat face".into()),
        }
    }

    /// An infinite line: axes, straight or circular edges, sketch lines,
    /// cylindrical or conical faces.
    pub fn axis(&self) -> RResult<(DVec3, DVec3)> {
        match self {
            Resolved::Axis { origin, dir } => Ok((*origin, unit(*dir))),
            Resolved::Edge { info, .. } => match info.kind {
                CurveKind::Line => Ok((info.start, unit(info.end - info.start))),
                CurveKind::Circle => Ok((info.center, unit(info.axis))),
                _ => Err("select a straight or circular edge".into()),
            },
            Resolved::Face { info, .. }
                if matches!(
                    info.kind,
                    SurfaceKind::Cylinder | SurfaceKind::Cone | SurfaceKind::Torus
                ) =>
            {
                Ok((info.axis_origin, unit(info.axis_dir)))
            }
            Resolved::CoordSys { origin, z, .. } => Ok((*origin, *z)),
            _ => Err("select an axis, a straight edge or a cylindrical face".into()),
        }
    }

    /// A direction: an axis-like reference, or a plane's normal.
    pub fn direction(&self) -> RResult<DVec3> {
        if let Ok((_, d)) = self.axis() {
            return Ok(d);
        }
        self.plane()
            .map(|p| p.normal)
            .map_err(|_| "select an edge, axis or plane for the direction".into())
    }

    /// A point: vertices, points, circle centers, face centroids.
    pub fn point(&self) -> RResult<DVec3> {
        match self {
            Resolved::Body { center, .. } => Ok(*center),
            Resolved::Point(p) | Resolved::Vertex { point: p, .. } => Ok(*p),
            Resolved::Edge { info, .. } if info.kind == CurveKind::Circle => Ok(info.center),
            Resolved::Edge { info, .. } => Ok(info.mid),
            Resolved::Face { info, .. } => Ok(info.centroid),
            Resolved::CoordSys { origin, .. } => Ok(*origin),
            Resolved::Axis { origin, .. } => Ok(*origin),
            Resolved::Plane(p) => Ok(p.origin),
        }
    }
}

/// Kernel index and kind for a face/edge/vertex reference in a body.
pub(crate) fn index_of(state: &State, r: &Ref) -> RResult<(Kind, u32)> {
    match resolve(state, r)? {
        Resolved::Face { index, .. } => Ok((Kind::Face, index)),
        Resolved::Edge { index, .. } => Ok((Kind::Edge, index)),
        Resolved::Vertex { index, .. } => Ok((Kind::Vertex, index)),
        _ => Err("expected a face, edge or vertex".into()),
    }
}
