//! Weld beads: cosmetic beads (paths with a size and symbol, drawn and
//! listed, the solid unchanged) and modeled fillet beads (triangular bodies
//! along straight seams).

use of_geom::{CurveKind, DVec3, Kind, Shape, SurfaceKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::build::{Body, State};
use crate::naming::Naming;
use crate::{BodyId, FeatureId, Ref, SolidBody};

/// What a bead follows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BeadPath {
    /// Model edges.
    Edges { edges: Vec<Ref> },
    /// Where faces in `from` meet faces in `to`, on different bodies or one.
    Faces { from: Vec<Ref>, to: Vec<Ref> },
    /// Smart weld selection: every seam where two of these faces, on
    /// different bodies, meet (the faces a stroke across the joint crosses).
    Smart { faces: Vec<Ref> },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BeadKind {
    /// Drawn and listed; the solid is not changed.
    #[default]
    Cosmetic,
    /// A fillet weld modeled as a triangular body along each seam.
    Fillet,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BeadSides {
    #[default]
    One,
    Both,
}

/// Beads in pieces along the path.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Intermittent {
    /// Each piece, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub length: f64,
    /// Between pieces, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub gap: f64,
    /// With both sides: the other side's pieces fall in this side's gaps.
    #[serde(default)]
    pub staggered: bool,
}

/// A weld bead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeldBead {
    pub path: BeadPath,
    #[serde(default)]
    pub kind: BeadKind,
    /// Leg size, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub size: f64,
    #[serde(default)]
    pub sides: BeadSides,
    /// Carry on along edges tangent to the selected ones.
    #[serde(default, skip_serializing_if = "crate::sheet_metal::is_false")]
    pub tangent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intermittent: Option<Intermittent>,
    /// Weld only from `start` to `end` along each seam, mm from its start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_to: Option<[f64; 2]>,
    /// Weld symbol text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Weld folder properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    /// For cosmetic beads, kg per metre of bead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub mass_per_length: Option<f64>,
}

/// A bead in the weld folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeldRecord {
    pub feature: FeatureId,
    pub kind: BeadKind,
    pub size: f64,
    pub sides: BeadSides,
    /// Each welded piece as model points.
    pub pieces: Vec<Vec<[f64; 3]>>,
    /// Total bead length, both sides counted, mm.
    pub length: f64,
    pub symbol: Option<String>,
    pub material: Option<String>,
    pub process: Option<String>,
    pub mass_per_length: Option<f64>,
    /// Modeled beads' bodies.
    pub bodies: Vec<BodyId>,
}

/// A seam: points along it, and for straight seams the two faces' outward
/// normals (for modeled beads).
struct Seam {
    points: Vec<DVec3>,
    normals: Option<(DVec3, DVec3)>,
}

fn global_face(state: &State, r: &Ref) -> Result<u32, String> {
    match state.index_of(r)? {
        (Kind::Face, f) => Ok(f),
        _ => Err("select faces for a bead's from and to sets".into()),
    }
}

fn polyline(shape: &Shape, e: u32) -> Result<Vec<DVec3>, String> {
    shape.edge_samples(e, 0.05, 0.05).map_err(|e| e.0)
}

/// Edges of `from` faces lying on `to` faces, and the other way round.
fn seams_between(state: &State, from: &[Ref], to: &[Ref]) -> Result<Vec<Seam>, String> {
    let whole = &state.body.as_ref().ok_or("there's no body")?.shape;
    let from: Vec<u32> = from
        .iter()
        .map(|r| global_face(state, r))
        .collect::<Result<_, _>>()?;
    let to: Vec<u32> = to
        .iter()
        .map(|r| global_face(state, r))
        .collect::<Result<_, _>>()?;
    let mut seams: Vec<Seam> = Vec::new();
    seams_into(whole, &from, &to, &mut seams)?;
    if seams.is_empty() {
        return Err("the from and to faces don't meet".into());
    }
    Ok(seams)
}

/// Seams between each pair of faces on different bodies.
fn smart_seams(state: &State, faces: &[Ref]) -> Result<Vec<Seam>, String> {
    let whole = &state.body.as_ref().ok_or("there's no body")?.shape;
    let faces: Vec<u32> = faces
        .iter()
        .map(|r| global_face(state, r))
        .collect::<Result<_, _>>()?;
    let owners: Vec<Vec<u32>> = state
        .bodies
        .iter()
        .map(|b| {
            b.geometry
                .shape
                .indices_in(whole, Kind::Face)
                .unwrap_or_default()
        })
        .collect();
    let body_of = |f: u32| owners.iter().position(|o| o.contains(&f));
    let mut seams: Vec<Seam> = Vec::new();
    for (n, &a) in faces.iter().enumerate() {
        for &b in &faces[n + 1..] {
            if body_of(a) != body_of(b) {
                seams_into(whole, &[a], &[b], &mut seams)?;
            }
        }
    }
    if seams.is_empty() {
        return Err("none of the faces meet a face of another body".into());
    }
    Ok(seams)
}

fn seams_into(
    whole: &Shape,
    from: &[u32],
    to: &[u32],
    seams: &mut Vec<Seam>,
) -> Result<(), String> {
    let face_edges = whole.face_edges();
    for (a_set, b_set) in [(from, to), (to, from)] {
        for &a in a_set {
            let ai = whole.face_info(a).map_err(|e| e.0)?;
            for &b in b_set {
                let bi = whole.face_info(b).map_err(|e| e.0)?;
                if bi.kind != SurfaceKind::Plane {
                    continue;
                }
                let b_face = whole.sub_shape(Kind::Face, b).map_err(|e| e.0)?;
                for &e in &face_edges[a as usize] {
                    let pts = polyline(whole, e)?;
                    // On b's plane all along, and touching b itself.
                    let on_plane = pts
                        .iter()
                        .all(|p| (*p - bi.point).dot(bi.normal).abs() < 1e-6);
                    if !on_plane {
                        continue;
                    }
                    let edge = whole.sub_shape(Kind::Edge, e).map_err(|e| e.0)?;
                    if edge.distance(&b_face).map_or(true, |d| d.value > 1e-6) {
                        continue;
                    }
                    let mid = pts[pts.len() / 2];
                    if seams.iter().any(|s| {
                        s.points.len() == pts.len()
                            && (s.points[s.points.len() / 2] - mid).length() < 1e-6
                    }) {
                        continue;
                    }
                    let straight = whole.edge_info(e).is_ok_and(|i| i.kind == CurveKind::Line);
                    let normals = (straight && ai.kind == SurfaceKind::Plane)
                        .then_some((ai.normal, bi.normal));
                    seams.push(Seam {
                        points: pts,
                        normals,
                    });
                }
            }
        }
    }
    Ok(())
}

/// The selected edges, with edges tangent to them when asked.
fn seams_on_edges(state: &State, edges: &[Ref], tangent: bool) -> Result<Vec<Seam>, String> {
    let whole = &state.body.as_ref().ok_or("there's no body")?.shape;
    let mut picked: Vec<u32> = Vec::new();
    for r in edges {
        match state.index_of(r)? {
            (Kind::Edge, e) => picked.push(e),
            _ => return Err("select edges for a bead".into()),
        }
    }
    if tangent {
        let edge_faces = whole.edge_faces();
        let mut grew = true;
        while grew {
            grew = false;
            for e in 0..whole.count(Kind::Edge) {
                if picked.contains(&e) {
                    continue;
                }
                let Ok(i) = whole.edge_info(e) else { continue };
                let joins = picked.iter().any(|p| {
                    let Ok(j) = whole.edge_info(*p) else {
                        return false;
                    };
                    // Sharing an end and a face, running on smoothly.
                    let shared = [
                        (i.start, j.end),
                        (i.start, j.start),
                        (i.end, j.start),
                        (i.end, j.end),
                    ]
                    .iter()
                    .any(|(a, b)| (*a - *b).length() < 1e-6);
                    let face = edge_faces[e as usize]
                        .iter()
                        .any(|f| edge_faces[*p as usize].contains(f));
                    shared && face && smooth(whole, e, *p)
                });
                if joins {
                    picked.push(e);
                    grew = true;
                }
            }
        }
    }
    picked
        .iter()
        .map(|e| {
            Ok(Seam {
                points: polyline(whole, *e)?,
                normals: None,
            })
        })
        .collect()
}

/// Whether two edges meeting at an end continue each other's direction.
fn smooth(shape: &Shape, a: u32, b: u32) -> bool {
    let (Ok(pa), Ok(pb)) = (polyline(shape, a), polyline(shape, b)) else {
        return false;
    };
    let ends = |p: &[DVec3]| {
        let n = p.len();
        [(p[0], p[1] - p[0]), (p[n - 1], p[n - 2] - p[n - 1])]
    };
    for (qa, da) in ends(&pa) {
        for (qb, db) in ends(&pb) {
            if (qa - qb).length() < 1e-6 {
                // Leaving the shared point in opposite directions.
                return da.normalize().dot(db.normalize()) < -0.999;
            }
        }
    }
    false
}

/// The parts of a polyline between distances `a` and `b` along it.
fn piece(points: &[DVec3], a: f64, b: f64) -> Vec<DVec3> {
    let mut out = Vec::new();
    let mut s = 0.0;
    for w in points.windows(2) {
        let len = (w[1] - w[0]).length();
        let (s0, s1) = (s, s + len);
        let (lo, hi) = (a.max(s0), b.min(s1));
        if hi > lo && len > 0.0 {
            let at = |t: f64| w[0] + (w[1] - w[0]) * ((t - s0) / len);
            if out.is_empty() {
                out.push(at(lo));
            }
            out.push(at(hi));
        }
        s = s1;
    }
    out
}

fn length(points: &[DVec3]) -> f64 {
    points.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

/// The welded ranges along a seam of length `total`, for one side.
fn ranges(p: &WeldBead, total: f64, second_side: bool) -> Vec<(f64, f64)> {
    let (start, end) = match p.from_to {
        Some([a, b]) => (a.clamp(0.0, total), b.clamp(0.0, total)),
        None => (0.0, total),
    };
    let Some(i) = p.intermittent else {
        return vec![(start, end)];
    };
    let pitch = i.length + i.gap;
    let shift = if second_side && i.staggered {
        pitch / 2.0
    } else {
        0.0
    };
    let mut out = Vec::new();
    let mut at = start + shift;
    while at < end - 1e-9 {
        out.push((at, (at + i.length).min(end)));
        at += pitch;
    }
    out
}

/// A triangular fillet bead along a straight seam between two faces.
fn fillet_body(seam: &Seam, size: f64, a: f64, b: f64) -> Result<Shape, String> {
    let (n1, n2) = seam
        .normals
        .ok_or("modeled beads need straight seams between flat faces")?;
    let pts = piece(&seam.points, a, b);
    let (p0, p1) = (pts[0], *pts.last().unwrap());
    let k = n1.dot(n2);
    // Legs along each face, away from the other.
    let l1 = (n2 - n1 * k).normalize_or_zero();
    let l2 = (n1 - n2 * k).normalize_or_zero();
    if l1 == DVec3::ZERO || l2 == DVec3::ZERO {
        return Err("the faces of a modeled bead are parallel".into());
    }
    let tri = [p0, p0 + l1 * size, p0 + l2 * size];
    let normal = (p1 - p0).normalize();
    let plane = of_geom::Plane::new(p0, l1, normal);
    let local: Vec<_> = tri.iter().map(|q| plane.to_local(*q)).collect();
    let face = of_geom::Profile {
        plane,
        faces: vec![of_geom::ProfileFace {
            outer: (0..3)
                .map(|i| of_geom::Curve2::Line {
                    a: local[i],
                    b: local[(i + 1) % 3],
                })
                .collect(),
            holes: Vec::new(),
        }],
    }
    .to_shape()
    .map_err(|e| e.0)?;
    Ok(face.prism(p1 - p0).map_err(|e| e.0)?.shape)
}

pub(crate) fn weld_bead(
    fid: FeatureId,
    p: &WeldBead,
    prev: &State,
    state: &mut State,
) -> Result<(), String> {
    if p.kind == BeadKind::Fillet && p.sides == BeadSides::Both {
        return Err("model the other side's fillet bead as a bead of its own".into());
    }
    let seams = match &p.path {
        BeadPath::Edges { edges } => {
            if p.kind == BeadKind::Fillet {
                return Err(
                    "modeled fillet beads follow the seams between from and to faces".into(),
                );
            }
            seams_on_edges(prev, edges, p.tangent)?
        }
        BeadPath::Faces { from, to } => seams_between(prev, from, to)?,
        BeadPath::Smart { faces } => smart_seams(prev, faces)?,
    };
    let sides: &[bool] = match p.sides {
        BeadSides::One => &[false],
        BeadSides::Both => &[false, true],
    };
    let mut pieces = Vec::new();
    let mut total = 0.0;
    let mut bodies = Vec::new();
    for seam in &seams {
        let len = length(&seam.points);
        for &second in sides {
            for (a, b) in ranges(p, len, second) {
                let pts = piece(&seam.points, a, b);
                if pts.len() < 2 {
                    continue;
                }
                total += length(&pts);
                pieces.push(pts.iter().map(|q| q.to_array()).collect());
                if p.kind == BeadKind::Fillet {
                    let shape = fillet_body(seam, p.size, a, b)?;
                    let k = bodies.len();
                    let id = BodyId(format!("b{fid}:bead{k}"));
                    let names = Naming::by_index(&shape, &format!("{fid}.bead{k}"));
                    state.bodies.push(SolidBody {
                        id: id.clone(),
                        geometry: Body { shape, names },
                    });
                    bodies.push(id);
                }
            }
        }
    }
    state.welds.push(WeldRecord {
        feature: fid,
        kind: p.kind,
        size: p.size,
        sides: p.sides,
        pieces,
        length: total,
        symbol: p.symbol.clone(),
        material: p.material.clone(),
        process: p.process.clone(),
        mass_per_length: p.mass_per_length,
        bodies,
    });
    state.aggregate_bodies()
}
