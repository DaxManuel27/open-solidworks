//! Cut models for section views and broken-out sections: the model with
//! material removed by an extruded 2D region, the faces the cut exposed
//! (for hatching) and a map from the cut shape's edges back to model edges.

use of_assembly::CompRef;
use of_assembly::DQuat;
use of_model::geom::{
    BoolOp, Curve2, DAffine3, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Relation, Shape,
};

use crate::model::{Scene, affine};
use crate::views::Frame;

/// A face exposed by a cut, as 3D loops, with what was cut.
#[derive(Debug, Clone)]
pub struct CutFace {
    pub loops: Vec<Vec<DVec3>>,
    pub component: Vec<String>,
    pub material: Option<String>,
}

/// The shape a view projects, with the model edge behind each of its edges.
pub struct ViewShape {
    pub shape: Shape,
    pub key: String,
    pub edges: Vec<Option<CompRef>>,
    pub cut_faces: Vec<CutFace>,
    /// Edges of cut faces seen edge-on (the jogs of an offset section),
    /// which drawings leave out.
    pub edge_on: std::collections::HashSet<u32>,
}

impl ViewShape {
    /// A scene drawn uncut.
    pub fn whole(scene: &Scene) -> ViewShape {
        let n = scene.shape.count(Kind::Edge);
        ViewShape {
            shape: scene.shape.clone(),
            key: scene.key.clone(),
            edges: (0..n).map(|i| scene.edge_ref(i)).collect(),
            cut_faces: Vec::new(),
            edge_on: Default::default(),
        }
    }
}

/// What removes material: a region of a plane (2D, in the plane's
/// coordinates) extruded along the plane normal between two depths.
pub struct Tool {
    pub plane: Plane,
    pub region: Vec<Curve2>,
    pub near: f64,
    pub far: f64,
}

impl Tool {
    fn solid(&self) -> Result<Shape, String> {
        let base = Plane::new(
            self.plane.origin + self.plane.normal * self.near,
            self.plane.x_dir,
            self.plane.normal,
        );
        let face = Profile {
            plane: base,
            faces: vec![ProfileFace {
                outer: self.region.clone(),
                holes: Vec::new(),
            }],
        }
        .to_shape()
        .map_err(|e| e.to_string())?;
        face.prism(self.plane.normal * (self.far - self.near))
            .map(|r| r.shape)
            .map_err(|e| e.to_string())
    }
}

/// A closed polygon as line curves.
pub fn polygon(points: &[DVec2]) -> Vec<Curve2> {
    (0..points.len())
        .map(|i| Curve2::Line {
            a: points[i],
            b: points[(i + 1) % points.len()],
        })
        .collect()
}

/// The region on side `side` (unit) of a polyline, extended `far` beyond
/// its ends along their own directions: what a cutting line removes.
pub fn beyond(points: &[DVec2], side: DVec2, far: f64) -> Result<Vec<DVec2>, String> {
    if points.len() < 2 {
        return Err("a cutting line needs two points".into());
    }
    let d0 = (points[1] - points[0])
        .try_normalize()
        .ok_or("the cutting line has no length")?;
    let n = points.len();
    let d1 = (points[n - 1] - points[n - 2])
        .try_normalize()
        .ok_or("the cutting line has no length")?;
    let start = points[0] - d0 * far;
    let end = points[n - 1] + d1 * far;
    let mut poly = vec![start];
    poly.extend_from_slice(points);
    poly.push(end);
    poly.push(end + side * far);
    poly.push(start + side * far);
    // Collinear end extensions may repeat directions; drop zero-length edges.
    poly.dedup_by(|a, b| a.distance(*b) < 1e-9);
    Ok(poly)
}

/// Options of a cut.
pub struct Cut<'a> {
    /// Tools whose union is removed.
    pub tools: Vec<Tool>,
    /// Component key paths left whole.
    pub excluded: &'a [Vec<String>],
    pub exclude_fasteners: bool,
    /// The direction toward the viewer: exposed faces facing it are hatched.
    pub toward: DVec3,
    /// Extra key for the projection cache.
    pub key: String,
    /// Pieces whose cut is rotated afterwards (aligned sections): a tool
    /// index per piece side and the rotation applied to material that tool
    /// keeps. `None` cuts every piece with all tools.
    pub aligned: Option<Aligned>,
}

/// An aligned section: material on the far side of `split` (a plane through
/// the turning axis) is cut by the second tool and turned into the first
/// arm's plane.
pub struct Aligned {
    pub split: Plane,
    pub rotation: DAffine3,
}

fn is_fastener(part: &of_assembly::cache::Part) -> bool {
    part.doc().properties.contains_key("Hardware")
}

/// Cut a scene.
pub fn cut(scene: &Scene, c: &Cut) -> Result<ViewShape, String> {
    let tools: Vec<Shape> = c.tools.iter().map(Tool::solid).collect::<Result<_, _>>()?;
    let mut shapes = Vec::new();
    let mut edges = Vec::new();
    let mut cut_faces = Vec::new();
    let mut edge_on = std::collections::HashSet::new();
    for piece in &scene.pieces {
        let Some(shape) = piece.part.shape() else {
            continue;
        };
        let placed = if scene.is_assembly {
            shape
                .located(&affine(&piece.pose))
                .map_err(|e| e.to_string())?
        } else {
            shape.clone()
        };
        let refs: Vec<Option<CompRef>> = (0..shape.count(Kind::Edge))
            .map(|i| {
                piece
                    .part
                    .state()
                    .ref_for(Kind::Edge, i)
                    .map(|entity| CompRef {
                        component: piece.component.clone(),
                        entity,
                    })
            })
            .collect();
        let skip = c
            .excluded
            .iter()
            .any(|e| !e.is_empty() && piece.component.starts_with(e))
            || (c.exclude_fasteners && scene.is_assembly && is_fastener(&piece.part));
        if skip {
            shapes.push(placed);
            edges.extend(refs);
            continue;
        }
        let material = piece.part.doc().material.as_ref().map(|m| m.name.clone());
        // Each part of the piece and the tools that cut it.
        let parts: Vec<(Shape, Vec<&Shape>, Option<DAffine3>)> = match &c.aligned {
            None => vec![(placed, tools.iter().collect(), None)],
            Some(a) => {
                let half = |inside: DVec3| -> Result<Shape, String> {
                    let face = Shape::plane_face(&a.split).map_err(|e| e.to_string())?;
                    let hs = Shape::half_space(&face, inside).map_err(|e| e.to_string())?;
                    placed
                        .boolean(BoolOp::Common, &hs)
                        .map(|r| r.shape)
                        .map_err(|e| e.to_string())
                };
                let o = a.split.origin;
                let n = a.split.normal;
                vec![
                    (half(o - n)?, vec![&tools[0]], None),
                    (half(o + n)?, vec![&tools[1]], Some(a.rotation)),
                ]
            }
        };
        for (body, cutters, turn) in parts {
            if body.count(Kind::Face) == 0 {
                continue;
            }
            let (mut result, mut map) = cut_one(&body, &cutters)?;
            if result.count(Kind::Face) == 0 {
                continue;
            }
            // Turned material faces the viewer once turned.
            let toward = turn
                .map(|t| t.inverse().transform_vector3(c.toward))
                .unwrap_or(c.toward);
            let exposed = exposed_faces(&result, &map.tool_faces, toward);
            let base = edges.len() as u32;
            let face_edges = result.face_edges();
            for f in &map.tool_faces {
                let side_on = result.face_info(*f).is_ok_and(|i| {
                    i.kind == of_model::geom::SurfaceKind::Plane
                        && i.normal.dot(toward).abs() < 1e-6
                });
                if side_on && let Some(es) = face_edges.get(*f as usize) {
                    edge_on.extend(es.iter().map(|e| base + e));
                }
            }
            let mut face_sets: Vec<Vec<Vec<DVec3>>> =
                exposed.iter().map(|f| face_loops(&result, *f)).collect();
            if let Some(t) = turn {
                let r = result.transform(&t).map_err(|e| e.to_string())?;
                let mut next = std::collections::HashMap::new();
                for h in &r.history {
                    if h.from_kind == Kind::Edge
                        && h.to_kind == Kind::Edge
                        && let Some(orig) = map.edge_from.get(&h.from)
                    {
                        next.insert(h.to, *orig);
                    }
                }
                if !r.history.is_empty() {
                    map.edge_from = next;
                }
                result = r.shape;
                for loops in &mut face_sets {
                    for l in loops.iter_mut() {
                        for p in l.iter_mut() {
                            *p = t.transform_point3(*p);
                        }
                    }
                }
            }
            for loops in face_sets {
                if !loops.is_empty() {
                    cut_faces.push(CutFace {
                        loops,
                        component: piece.component.clone(),
                        material: material.clone(),
                    });
                }
            }
            // Edges of the cut piece name the model edges they came from.
            let n = result.count(Kind::Edge);
            for i in 0..n {
                edges.push(
                    map.edge_from
                        .get(&i)
                        .and_then(|from| refs.get(*from as usize).cloned().flatten()),
                );
            }
            shapes.push(result);
        }
    }
    if shapes.is_empty() {
        return Err("the cut removes the whole model".into());
    }
    let shape = Shape::compound(&shapes);
    if shape.count(Kind::Edge) as usize != edges.len() {
        return Err("two cut pieces share edges; hide one component to draw this view".into());
    }
    Ok(ViewShape {
        shape,
        key: format!("{}|{}", scene.key, c.key),
        edges,
        cut_faces,
        edge_on,
    })
}

struct CutMap {
    /// Result edge index → input edge index.
    edge_from: std::collections::HashMap<u32, u32>,
    /// Result faces made by the tools.
    tool_faces: Vec<u32>,
}

/// Cut one body by several tools in turn, composing the histories.
fn cut_one(body: &Shape, tools: &[&Shape]) -> Result<(Shape, CutMap), String> {
    let mut shape = body.clone();
    // Current edge index → original edge index.
    let mut edge_from: std::collections::HashMap<u32, u32> =
        (0..body.count(Kind::Edge)).map(|i| (i, i)).collect();
    let mut tool_faces: std::collections::HashSet<u32> = Default::default();
    for t in tools {
        let r = shape
            .boolean(BoolOp::Cut, t)
            .map_err(|e| format!("the section could not be cut: {e}"))?;
        let mut next = std::collections::HashMap::new();
        let mut next_faces = std::collections::HashSet::new();
        for h in &r.history {
            match (h.input, h.from_kind, h.to_kind, h.rel) {
                (0, Kind::Edge, Kind::Edge, Relation::Same | Relation::Modified) => {
                    if let Some(orig) = edge_from.get(&h.from) {
                        next.insert(h.to, *orig);
                    }
                }
                (0, Kind::Face, Kind::Face, Relation::Same | Relation::Modified) => {
                    if tool_faces.contains(&h.from) {
                        next_faces.insert(h.to);
                    }
                }
                (1, _, Kind::Face, _) => {
                    next_faces.insert(h.to);
                }
                _ => {}
            }
        }
        edge_from = next;
        tool_faces = next_faces;
        shape = r.shape;
    }
    let mut tool_faces: Vec<u32> = tool_faces.into_iter().collect();
    tool_faces.sort();
    Ok((
        shape,
        CutMap {
            edge_from,
            tool_faces,
        },
    ))
}

/// Tool faces that are planar and face the viewer.
fn exposed_faces(shape: &Shape, faces: &[u32], toward: DVec3) -> Vec<u32> {
    faces
        .iter()
        .copied()
        .filter(|f| {
            shape.face_info(*f).is_ok_and(|i| {
                i.kind == of_model::geom::SurfaceKind::Plane && i.normal.dot(toward) > 0.999
            })
        })
        .collect()
}

/// Boundary loops of a face as 3D points.
fn face_loops(shape: &Shape, face: u32) -> Vec<Vec<DVec3>> {
    let fe = shape.face_edges();
    let Some(edges) = fe.get(face as usize) else {
        return Vec::new();
    };
    let size = shape.bbox().map(|b| b.diagonal()).unwrap_or(1.0).max(1e-6);
    let pieces: Vec<Vec<DVec3>> = edges
        .iter()
        .filter_map(|e| shape.edge_samples(*e, size * 1e-3, 0.05).ok())
        .filter(|p| p.len() >= 2)
        .collect();
    chain3(pieces, size * 1e-6 + 1e-7)
}

fn chain3(mut pieces: Vec<Vec<DVec3>>, tol: f64) -> Vec<Vec<DVec3>> {
    let mut loops = Vec::new();
    while let Some(mut cur) = pieces.pop() {
        loop {
            let (s, e) = (cur[0], *cur.last().unwrap());
            if cur.len() > 2 && s.distance(e) <= tol {
                cur.pop();
                loops.push(cur);
                break;
            }
            let next = pieces
                .iter()
                .position(|p| e.distance(p[0]) <= tol || e.distance(*p.last().unwrap()) <= tol);
            match next {
                Some(i) => {
                    let mut p = pieces.swap_remove(i);
                    if e.distance(p[0]) > tol {
                        p.reverse();
                    }
                    cur.extend(p.into_iter().skip(1));
                }
                None => break,
            }
        }
    }
    loops
}

/// Model size used to make tools reach past the model.
pub fn reach(scene: &Scene) -> f64 {
    scene
        .shape
        .bbox()
        .map(|b| b.diagonal() + b.min.length().max(b.max.length()))
        .unwrap_or(1000.0)
        * 2.0
        + 10.0
}

/// The plane of a view frame through the model origin, with paper axes.
pub fn frame_plane(f: &Frame) -> Plane {
    Plane::new(DVec3::ZERO, f.x, f.toward)
}

/// Rotation about an axis through `origin` taking direction `from` to `to`
/// (both perpendicular to the axis).
pub fn turn(origin: DVec3, axis: DVec3, from: DVec3, to: DVec3) -> DAffine3 {
    let a = axis.normalize();
    let (f, t) = (from.normalize(), to.normalize());
    let angle = f.cross(t).dot(a).atan2(f.dot(t));
    let q = DQuat::from_axis_angle(a, angle);
    DAffine3::from_translation(origin)
        * DAffine3::from_quat(q)
        * DAffine3::from_translation(-origin)
}
