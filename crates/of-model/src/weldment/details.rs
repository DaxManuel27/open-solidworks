//! End caps and gussets: plates added to members as bodies of their own.

use of_geom::{
    BoolOp, Curve2, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Shape, SurfaceKind,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::build::{Body, State};
use crate::naming::Naming;
use crate::sheet_metal::CornerBreak;
use crate::{BodyId, FeatureId, Ref, SolidBody};

/// Which way an end cap's thickness goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CapDirection {
    /// A plate on the end, covering the profile.
    #[default]
    Outward,
    /// A plug in the hollow profile, flush with its end.
    Inward,
    /// A plug in the hollow profile, set in from its end, mm.
    Inset { depth: f64 },
}

/// How far a cap's edge stays in from the profile's edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CapOffset {
    /// A multiple of the profile's wall thickness.
    Ratio { ratio: f64 },
    /// mm.
    Distance { distance: f64 },
}

impl Default for CapOffset {
    fn default() -> Self {
        CapOffset::Ratio { ratio: 0.5 }
    }
}

/// Plates closing members' ends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EndCap {
    /// Square end faces of structural members.
    #[schemars(length(min = 1))]
    pub faces: Vec<Ref>,
    #[serde(default)]
    pub direction: CapDirection,
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub thickness: f64,
    #[serde(default)]
    pub offset: CapOffset,
    /// Chamfer or round the cap's corners.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corners: Option<CornerBreak>,
}

/// A gusset's outline in its plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GussetShape {
    /// Legs `d1` along the first face and `d2` along the second.
    Triangle { d1: f64, d2: f64 },
    /// Legs `d1` and `d2`, then straight ends `d3` (at the end of `d1`, away
    /// from the first face) and `d4` (at the end of `d2`).
    Polygon { d1: f64, d2: f64, d3: f64, d4: f64 },
}

/// Where a gusset's thickness lies from its location.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GussetSide {
    /// Centered on the location.
    #[default]
    Both,
    /// Toward the middle of the faces' shared edge.
    Inner,
    /// Away from it.
    Outer,
}

/// Where along the faces' shared edge a gusset goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GussetLocation {
    Start,
    #[default]
    Middle,
    End,
}

/// A plate in the corner between two members' faces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeldGusset {
    /// Two planar faces meeting at an inside corner.
    pub faces: [Ref; 2],
    pub shape: GussetShape,
    /// Corner cut off where the faces meet, for the weld, mm.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub chamfer: f64,
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub thickness: f64,
    #[serde(default)]
    pub side: GussetSide,
    #[serde(default)]
    pub location: GussetLocation,
    /// Along the shared edge from the location, mm.
    #[serde(default)]
    pub offset: f64,
}

/// A planar face of the part: its outward normal, a point, its vertices and
/// the body index it belongs to.
fn planar_face(state: &State, r: &Ref) -> Result<(DVec3, DVec3, Vec<DVec3>, usize), String> {
    let (Kind::Face, global) = state.index_of(r)? else {
        return Err("select planar faces".into());
    };
    let whole = &state.body.as_ref().ok_or("there's no body")?.shape;
    let info = whole.face_info(global).map_err(|e| e.0)?;
    if info.kind != SurfaceKind::Plane {
        return Err("select planar faces".into());
    }
    let body = state
        .bodies
        .iter()
        .position(|b| {
            b.geometry
                .shape
                .indices_in(whole, Kind::Face)
                .is_ok_and(|ix| ix.contains(&global))
        })
        .ok_or("the face is not on a body")?;
    let face = whole.sub_shape(Kind::Face, global).map_err(|e| e.0)?;
    let verts = (0..face.count(Kind::Vertex))
        .filter_map(|v| face.vertex_point(v).ok())
        .collect();
    Ok((info.normal, info.point, verts, body))
}

/// A planar plate from a closed polygon (model points in a plane), pushed
/// along `dir`.
fn plate(points: &[DVec3], normal: DVec3, dir: DVec3) -> Result<Shape, String> {
    let origin = points[0];
    let plane = Plane::new(origin, (points[1] - origin).normalize(), normal);
    let local: Vec<DVec2> = points.iter().map(|p| plane.to_local(*p)).collect();
    let outer = (0..local.len())
        .map(|i| Curve2::Line {
            a: local[i],
            b: local[(i + 1) % local.len()],
        })
        .collect();
    let face = Profile {
        plane,
        faces: vec![ProfileFace {
            outer,
            holes: Vec::new(),
        }],
    }
    .to_shape()
    .map_err(|e| e.0)?;
    Ok(face.prism(dir).map_err(|e| e.0)?.shape)
}

fn convex_hull(mut pts: Vec<DVec2>) -> Vec<DVec2> {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| (*a - *b).length() < 1e-9);
    if pts.len() < 3 {
        return pts;
    }
    let mut hull: Vec<DVec2> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let order: Vec<DVec2> = if pass == 0 {
            pts.clone()
        } else {
            pts.iter().rev().copied().collect()
        };
        for p in order {
            while hull.len() >= start + 2 {
                let (a, b) = (hull[hull.len() - 2], hull[hull.len() - 1]);
                if (b - a).perp_dot(p - a) > 1e-12 {
                    break;
                }
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

/// A convex counter-clockwise polygon with every side moved in by `d`.
fn inset(poly: &[DVec2], d: f64) -> Result<Vec<DVec2>, String> {
    let n = poly.len();
    // Each side as a point and direction, moved in (to its left).
    let sides: Vec<(DVec2, DVec2)> = (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let t = (b - a).normalize();
            (a + t.perp() * d, t)
        })
        .collect();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (p, t) = sides[(i + n - 1) % n];
        let (q, u) = sides[i];
        let cross = t.perp_dot(u);
        if cross.abs() < 1e-12 {
            out.push(q);
            continue;
        }
        let s = (q - p).perp_dot(u) / cross;
        out.push(p + t * s);
    }
    // Moved past the middle: the outline turned inside out.
    let area: f64 = (0..n).map(|i| out[i].perp_dot(out[(i + 1) % n])).sum();
    if area <= 0.0 {
        return Err("the cap offset leaves no cap".into());
    }
    Ok(out)
}

/// Corners of a closed polygon sharper than 30°, as indices.
fn sharp(poly: &[DVec2]) -> Vec<usize> {
    let n = poly.len();
    (0..n)
        .filter(|i| {
            let (a, b, c) = (poly[(i + n - 1) % n], poly[*i], poly[(i + 1) % n]);
            (b - a).angle_to(c - b).abs() > 30f64.to_radians()
        })
        .collect()
}

pub(crate) fn end_cap(
    fid: FeatureId,
    p: &EndCap,
    prev: &State,
    state: &mut State,
) -> Result<(), String> {
    for (k, r) in p.faces.iter().enumerate() {
        let (normal, point, _, b) = planar_face(prev, r)?;
        let id = &prev.bodies[b].id;
        let record = prev
            .members
            .iter()
            .find(|m| m.body == *id)
            .ok_or("end caps go on structural members")?;
        let [x, y, z] = record.frame.map(DVec3::from);
        if normal.dot(z).abs() < 1.0 - 1e-6 {
            return Err("end caps go on square-cut member ends".into());
        }
        let a = DVec3::from(record.axis[0]);
        let center = a + z * (point - a).dot(z);
        let offset = match p.offset {
            CapOffset::Ratio { ratio } => ratio * record.wall,
            CapOffset::Distance { distance } => distance,
        };
        let outline: Vec<DVec2> = match p.direction {
            CapDirection::Outward => {
                convex_hull(record.outline.iter().map(|q| DVec2::from(*q)).collect())
            }
            CapDirection::Inward | CapDirection::Inset { .. } => {
                if record.hole.is_empty() {
                    return Err("inward caps need a hollow profile".into());
                }
                convex_hull(record.hole.iter().map(|q| DVec2::from(*q)).collect())
            }
        };
        let poly = inset(&outline, offset)?;
        let start = match p.direction {
            CapDirection::Outward | CapDirection::Inward => center,
            CapDirection::Inset { depth } => center - normal * depth,
        };
        let points: Vec<DVec3> = poly.iter().map(|q| start + x * q.x + y * q.y).collect();
        let dir = match p.direction {
            CapDirection::Outward => normal * p.thickness,
            _ => -normal * p.thickness,
        };
        let mut shape = plate(&points, normal, dir)?;
        if let Some(corner) = p.corners {
            // The edges through the thickness at the outline's sharp corners.
            let corners: Vec<DVec3> = sharp(&poly).iter().map(|i| points[*i]).collect();
            let edges: Vec<u32> = (0..shape.count(Kind::Edge))
                .filter(|e| {
                    shape.edge_info(*e).is_ok_and(|i| {
                        (i.end - i.start).normalize().dot(normal).abs() > 1.0 - 1e-9
                            && corners
                                .iter()
                                .any(|c| (i.start - *c).length().min((i.end - *c).length()) < 1e-6)
                    })
                })
                .collect();
            if !edges.is_empty() {
                let r = match corner {
                    CornerBreak::Chamfer { distance } => shape.chamfer(&edges, distance, distance),
                    CornerBreak::Fillet { radius } => shape.fillet(&edges, radius),
                }
                .map_err(|e| format!("can't break the cap's corners: {}", e.0))?;
                shape = r.shape;
            }
        }
        let names = Naming::by_index(&shape, &format!("{fid}.cap{k}"));
        state.bodies.push(SolidBody {
            id: BodyId(format!("b{fid}:cap{k}")),
            geometry: Body { shape, names },
        });
    }
    state.aggregate_bodies()
}

pub(crate) fn gusset(
    fid: FeatureId,
    p: &WeldGusset,
    prev: &State,
    state: &mut State,
) -> Result<(), String> {
    let (n1, q1, v1, _) = planar_face(prev, &p.faces[0])?;
    let (n2, q2, v2, _) = planar_face(prev, &p.faces[1])?;
    let l = n1.cross(n2);
    if l.length() < 1e-6 {
        return Err("the gusset's faces are parallel".into());
    }
    let l = l.normalize();
    // A point on both planes: least squares on n1·x = n1·q1, n2·x = n2·q2.
    let (c1, c2) = (n1.dot(q1), n2.dot(q2));
    let k = n1.dot(n2);
    let det = 1.0 - k * k;
    let base = n1 * ((c1 - k * c2) / det) + n2 * ((c2 - k * c1) / det);
    // Where both faces run along their shared line.
    let range = |v: &[DVec3]| {
        v.iter()
            .map(|q| (*q - base).dot(l))
            .fold((f64::MAX, f64::MIN), |(lo, hi), s| (lo.min(s), hi.max(s)))
    };
    let ((lo1, hi1), (lo2, hi2)) = (range(&v1), range(&v2));
    let (lo, hi) = (lo1.max(lo2), hi1.min(hi2));
    if hi - lo < p.thickness {
        return Err("the faces don't share enough of an edge for the gusset".into());
    }
    let mid = (lo + hi) / 2.0;
    let at = match p.location {
        GussetLocation::Start => lo + p.offset,
        GussetLocation::Middle => mid + p.offset,
        GussetLocation::End => hi - p.offset,
    };
    // Toward the middle of the shared edge.
    let inward = if at <= mid { 1.0 } else { -1.0 };
    let t = p.thickness;
    let from = match p.side {
        GussetSide::Both => at - t / 2.0,
        GussetSide::Inner if inward > 0.0 => at,
        GussetSide::Inner => at - t,
        GussetSide::Outer if inward > 0.0 => at - t,
        GussetSide::Outer => at,
    };
    let corner = base + l * from;
    // Along each face, away from the other.
    let a1 = (n2 - n1 * k).normalize();
    let a2 = (n1 - n2 * k).normalize();
    let mut pts: Vec<DVec3> = match p.shape {
        GussetShape::Triangle { d1, d2 } => vec![corner, corner + a1 * d1, corner + a2 * d2],
        GussetShape::Polygon { d1, d2, d3, d4 } => vec![
            corner,
            corner + a1 * d1,
            corner + a1 * d1 + a2 * d3,
            corner + a2 * d2 + a1 * d4,
            corner + a2 * d2,
        ],
    };
    if p.chamfer > 0.0 {
        // The corner sits between the face 2 leg (last) and the face 1 leg.
        pts.splice(0..1, [corner + a2 * p.chamfer, corner + a1 * p.chamfer]);
    }
    let shape = plate(&pts, l, l * t)?;
    // It must sit in the corner, not inside the members.
    let others = Shape::compound(prev.bodies.iter().map(|b| &b.geometry.shape));
    let overlap = shape
        .boolean(BoolOp::Common, &others)
        .map_err(|e| e.0)?
        .shape;
    if overlap.count(Kind::Solid) > 0 && overlap.mass_props().map_or(0.0, |m| m.volume) > 1e-6 * t {
        return Err("the gusset runs into the members; pick the faces of an inside corner".into());
    }
    let names = Naming::by_index(&shape, &format!("{fid}.gusset"));
    state.bodies.push(SolidBody {
        id: BodyId(format!("b{fid}:gusset")),
        geometry: Body { shape, names },
    });
    state.aggregate_bodies()
}
