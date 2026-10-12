//! Geometry that mates, patterns and explode directions read from resolved
//! references, in a component's own frame.

use of_mate::{DVec3, Pose};
use of_model::geom::{CurveKind, Kind, Shape, SurfaceKind};
use of_model::{Resolved, State};

/// Mate geometry in some frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Geo {
    Point(DVec3),
    /// An axis or straight edge; `ends` for finite edges.
    Line {
        origin: DVec3,
        dir: DVec3,
        ends: Option<(DVec3, DVec3)>,
    },
    /// A plane or planar face: a point on it, the outward normal and an
    /// in-plane X direction. `center` is the face centroid.
    Plane {
        origin: DVec3,
        normal: DVec3,
        x: DVec3,
        center: DVec3,
    },
    Cylinder {
        origin: DVec3,
        dir: DVec3,
        radius: f64,
    },
    Cone {
        origin: DVec3,
        dir: DVec3,
    },
    Sphere {
        center: DVec3,
        radius: f64,
    },
    Circle {
        center: DVec3,
        normal: DVec3,
        radius: f64,
    },
    Curve {
        points: Vec<DVec3>,
        closed: bool,
    },
}

impl Geo {
    pub fn name(&self) -> &'static str {
        match self {
            Geo::Point(_) => "point",
            Geo::Line { .. } => "line",
            Geo::Plane { .. } => "plane",
            Geo::Cylinder { .. } => "cylinder",
            Geo::Cone { .. } => "cone",
            Geo::Sphere { .. } => "sphere",
            Geo::Circle { .. } => "circle",
            Geo::Curve { .. } => "curve",
        }
    }

    /// The same geometry seen through a pose.
    pub fn transformed(&self, p: &Pose) -> Geo {
        let pt = |v: DVec3| p.point(v);
        let vc = |v: DVec3| p.vector(v);
        match self {
            Geo::Point(q) => Geo::Point(pt(*q)),
            Geo::Line { origin, dir, ends } => Geo::Line {
                origin: pt(*origin),
                dir: vc(*dir),
                ends: ends.map(|(a, b)| (pt(a), pt(b))),
            },
            Geo::Plane {
                origin,
                normal,
                x,
                center,
            } => Geo::Plane {
                origin: pt(*origin),
                normal: vc(*normal),
                x: vc(*x),
                center: pt(*center),
            },
            Geo::Cylinder {
                origin,
                dir,
                radius,
            } => Geo::Cylinder {
                origin: pt(*origin),
                dir: vc(*dir),
                radius: *radius,
            },
            Geo::Cone { origin, dir } => Geo::Cone {
                origin: pt(*origin),
                dir: vc(*dir),
            },
            Geo::Sphere { center, radius } => Geo::Sphere {
                center: pt(*center),
                radius: *radius,
            },
            Geo::Circle {
                center,
                normal,
                radius,
            } => Geo::Circle {
                center: pt(*center),
                normal: vc(*normal),
                radius: *radius,
            },
            Geo::Curve { points, closed } => Geo::Curve {
                points: points.iter().map(|q| pt(*q)).collect(),
                closed: *closed,
            },
        }
    }

    /// An axis: lines, cylinders, cones and circles.
    pub fn axis(&self) -> Option<(DVec3, DVec3)> {
        match self {
            Geo::Line { origin, dir, .. }
            | Geo::Cylinder { origin, dir, .. }
            | Geo::Cone { origin, dir } => Some((*origin, *dir)),
            Geo::Circle { center, normal, .. } => Some((*center, *normal)),
            _ => None,
        }
    }

    /// A direction: an axis, or a plane's normal.
    pub fn direction(&self) -> Option<DVec3> {
        match self {
            Geo::Plane { normal, .. } => Some(*normal),
            _ => self.axis().map(|(_, d)| d),
        }
    }

    /// A representative point: the point itself, a center, a face centroid
    /// or a point on an axis.
    pub fn point(&self) -> DVec3 {
        match self {
            Geo::Point(p) => *p,
            Geo::Line { origin, ends, .. } => ends.map_or(*origin, |(a, b)| (a + b) * 0.5),
            Geo::Plane { center, .. } => *center,
            Geo::Cylinder { origin, .. } | Geo::Cone { origin, .. } => *origin,
            Geo::Sphere { center, .. } | Geo::Circle { center, .. } => *center,
            Geo::Curve { points, .. } => points.first().copied().unwrap_or_default(),
        }
    }
}

/// Sampling for curved edges: chord tolerance in mm and angle.
const LINEAR: f64 = 0.05;
const ANGULAR: f64 = 0.1;

/// Geometry of a resolved reference. `state` supplies edge samples.
pub fn from_resolved(r: &Resolved, state: Option<&State>) -> Result<Geo, String> {
    let shape = state.and_then(|s| s.body.as_ref()).map(|b| &b.shape);
    Ok(match r {
        Resolved::Point(p) | Resolved::Vertex { point: p, .. } => Geo::Point(*p),
        Resolved::Body { center, .. } => Geo::Point(*center),
        Resolved::Plane(p) => Geo::Plane {
            origin: p.origin,
            normal: p.normal,
            x: p.x_dir,
            center: p.origin,
        },
        Resolved::Axis { origin, dir } => Geo::Line {
            origin: *origin,
            dir: dir.normalize_or_zero(),
            ends: None,
        },
        Resolved::CoordSys { origin, x, z, .. } => Geo::Plane {
            origin: *origin,
            normal: *z,
            x: *x,
            center: *origin,
        },
        Resolved::Face { info, .. } => match info.kind {
            SurfaceKind::Plane => Geo::Plane {
                origin: info.point,
                normal: info.normal.normalize_or_zero(),
                x: info.x_dir,
                center: info.centroid,
            },
            SurfaceKind::Cylinder => Geo::Cylinder {
                origin: info.axis_origin,
                dir: info.axis_dir.normalize_or_zero(),
                radius: info.radius,
            },
            SurfaceKind::Cone | SurfaceKind::Torus => Geo::Cone {
                origin: info.axis_origin,
                dir: info.axis_dir.normalize_or_zero(),
            },
            SurfaceKind::Sphere => Geo::Sphere {
                center: info.axis_origin,
                radius: info.radius,
            },
            SurfaceKind::Other => {
                return Err("select a flat, cylindrical, conical or spherical face".into());
            }
        },
        Resolved::Edge { index, info } => match info.kind {
            CurveKind::Line => Geo::Line {
                origin: info.start,
                dir: (info.end - info.start).normalize_or_zero(),
                ends: Some((info.start, info.end)),
            },
            CurveKind::Circle => Geo::Circle {
                center: info.center,
                normal: info.axis.normalize_or_zero(),
                radius: info.radius,
            },
            _ => {
                let shape = shape.ok_or("the edge has no geometry")?;
                let points = shape
                    .edge_samples(*index, LINEAR, ANGULAR)
                    .map_err(|e| e.0)?;
                Geo::Curve {
                    points,
                    closed: info.closed,
                }
            }
        },
    })
}

/// Sample points of an edge or a whole face boundary, in the part frame.
pub fn edge_points(shape: &Shape, edge: u32) -> Result<Vec<DVec3>, String> {
    shape.edge_samples(edge, LINEAR, ANGULAR).map_err(|e| e.0)
}

/// Join polylines end to end into one chain, flipping pieces as needed.
/// Returns the chain and whether it closes.
pub fn chain(mut pieces: Vec<Vec<DVec3>>) -> Result<(Vec<DVec3>, bool), String> {
    pieces.retain(|p| !p.is_empty());
    let Some(first) = pieces.first().cloned() else {
        return Err("select at least one edge or curve".into());
    };
    let size = pieces
        .iter()
        .flatten()
        .fold(0.0f64, |a, p| a.max(p.length()))
        .max(1.0);
    let tol = 1e-5 * size;
    let mut out = first;
    pieces.remove(0);
    while !pieces.is_empty() {
        let (start, end) = (out[0], *out.last().unwrap());
        let found = pieces.iter().enumerate().find_map(|(i, p)| {
            let (a, b) = (p[0], *p.last().unwrap());
            if a.distance(end) < tol {
                Some((i, false, true))
            } else if b.distance(end) < tol {
                Some((i, true, true))
            } else if b.distance(start) < tol {
                Some((i, false, false))
            } else if a.distance(start) < tol {
                Some((i, true, false))
            } else {
                None
            }
        });
        let Some((i, flip, at_end)) = found else {
            return Err("the selected edges do not form one connected chain".into());
        };
        let mut p = pieces.remove(i);
        if flip {
            p.reverse();
        }
        if at_end {
            out.extend(p.into_iter().skip(1));
        } else {
            p.pop();
            p.extend(out);
            out = p;
        }
    }
    let closed = out.len() > 2 && out[0].distance(*out.last().unwrap()) < tol;
    if closed {
        out.pop();
    }
    Ok((out, closed))
}

/// The outer boundary loop of a planar face as a closed polyline, with the
/// face normal. Used for cam profiles.
pub fn face_loop(state: &State, face: u32) -> Result<(Vec<DVec3>, DVec3), String> {
    let shape = &state.body.as_ref().ok_or("the part has no body")?.shape;
    let info = shape.face_info(face).map_err(|e| e.0)?;
    let edges = shape
        .face_edges()
        .get(face as usize)
        .cloned()
        .unwrap_or_default();
    let pieces = edges
        .iter()
        .map(|e| edge_points(shape, *e))
        .collect::<Result<Vec<_>, _>>()?;
    // Inner loops (holes) form separate chains; keep the longest.
    let mut loops: Vec<Vec<DVec3>> = Vec::new();
    let mut rest = pieces;
    while !rest.is_empty() {
        let mut group = vec![rest.remove(0)];
        loop {
            let ends: Vec<DVec3> = group
                .iter()
                .flat_map(|p| [p[0], *p.last().unwrap()])
                .collect();
            let touching = rest.iter().position(|p| {
                ends.iter()
                    .any(|e| e.distance(p[0]) < 1e-6 || e.distance(*p.last().unwrap()) < 1e-6)
            });
            match touching {
                Some(i) => group.push(rest.remove(i)),
                None => break,
            }
        }
        let (points, _) = chain(group)?;
        loops.push(points);
    }
    let length = |p: &Vec<DVec3>| p.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>();
    let outer = loops
        .into_iter()
        .max_by(|a, b| length(a).total_cmp(&length(b)))
        .ok_or("the face has no edges")?;
    Ok((outer, info.normal.normalize_or_zero()))
}

/// Faces adjacent to a face through its edges.
pub fn neighbours(shape: &Shape, face: u32) -> Vec<u32> {
    let edge_faces = shape.edge_faces();
    let mut out: Vec<u32> = shape
        .face_edges()
        .get(face as usize)
        .into_iter()
        .flatten()
        .flat_map(|e| edge_faces.get(*e as usize).cloned().unwrap_or_default())
        .filter(|f| *f != face)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Index of a face reference's kernel face.
pub fn face_index(state: &State, r: &of_model::Ref) -> Result<u32, String> {
    match state.index_of(r)? {
        (Kind::Face, i) => Ok(i),
        _ => Err("select a face".into()),
    }
}
