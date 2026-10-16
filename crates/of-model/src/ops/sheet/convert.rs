//! Sheet metal from solids: rips, insert bends and convert to sheet metal.
//!
//! A thin-walled solid's walls are found as pairs of opposite flat faces one
//! thickness apart. Where two walls meet at a sharp corner, the outside edge
//! is rounded to the bend radius plus the thickness and the inside edge to
//! the bend radius, which makes a bend; each pair of rounds is recorded.

use of_geom::{
    BoolOp, CurveKind, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Shape, SurfaceKind,
};

use super::{RResult, check_params, geo, record_body};
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::resolve::{Resolved, resolve};
use crate::sheet_metal::{
    BendRecord, ConvertToSheetMetal, InsertBends, Rip, RipDirection, RipEdge, SheetMetalBody,
    SheetMetalParams,
};
use crate::{FeatureId, Ref};

/// The body of the aggregate that holds face or edge `index`.
fn body_holding(state: &State, kind: Kind, index: u32) -> RResult<usize> {
    let whole = &state.body.as_ref().ok_or("there's no body yet")?.shape;
    state
        .bodies
        .iter()
        .position(|b| {
            b.geometry
                .shape
                .indices_in(whole, kind)
                .is_ok_and(|ix| ix.contains(&index))
        })
        .ok_or_else(|| "the selection is not on a body".into())
}

/// A straight edge, as its two ends.
#[derive(Clone, Copy)]
struct Line {
    a: DVec3,
    b: DVec3,
}

/// Find the edge of `body` that lies along `line`.
fn edge_along(body: &Shape, line: Line) -> Option<u32> {
    let dir = (line.b - line.a).normalize();
    (0..body.count(Kind::Edge)).find(|e| {
        body.edge_info(*e).is_ok_and(|i| {
            i.kind == CurveKind::Line
                && (i.end - i.start).normalize().cross(dir).length() < 1e-9
                && (i.mid - line.a).cross(dir).length() < 1e-6
                && (i.mid - (line.a + line.b) / 2.0).length() < (line.b - line.a).length() / 2.0
        })
    })
}

fn rename(fid: FeatureId, before: &Naming, r: of_geom::OpResult, tag: &str) -> Body {
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(before), None],
        },
        &mut |_| Some(format!("{fid}.{tag}")),
        &fid.to_string(),
    );
    Body {
        shape: r.shape,
        names,
    }
}

/// Cut a gap along each edge where two walls meet.
fn rip_edges(
    fid: FeatureId,
    body: Body,
    edges: &[(Line, RipDirection)],
    t: f64,
    gap: f64,
) -> RResult<Body> {
    let mut body = body;
    for (k, (line, direction)) in edges.iter().enumerate() {
        let e = edge_along(&body.shape, *line).ok_or("a rip edge no longer exists")?;
        let faces = body.shape.edge_faces()[e as usize].clone();
        if faces.len() != 2 {
            return Err("a rip edge must be where two walls meet".into());
        }
        let (n1, n2) = (
            body.shape.face_info(faces[0]).map_err(geo)?.normal,
            body.shape.face_info(faces[1]).map_err(geo)?.normal,
        );
        let into = -(n1 + n2).normalize_or_zero();
        if into == DVec3::ZERO {
            return Err("a rip edge must be a corner".into());
        }
        let dir = (line.b - line.a).normalize();
        let across = into.cross(dir).normalize();
        // Toward the first face is the side its outward normal leans to.
        let toward_first = if across.dot(n1) > 0.0 { 1.0 } else { -1.0 };
        let shift = match direction {
            RipDirection::Both => 0.0,
            RipDirection::First => toward_first * gap / 2.0,
            RipDirection::Second => -toward_first * gap / 2.0,
        };
        let interior = std::f64::consts::PI - n1.dot(n2).clamp(-1.0, 1.0).acos();
        let depth = t / (interior / 2.0).sin() + gap + t;
        let eps = 1e-3 * t.max(1.0);
        let plane = Plane::new(line.a - dir * eps, into, dir);
        let corners = [
            DVec2::new(-gap, shift - gap / 2.0),
            DVec2::new(depth, shift - gap / 2.0),
            DVec2::new(depth, shift + gap / 2.0),
            DVec2::new(-gap, shift + gap / 2.0),
        ];
        // Plane y is normal × x = dir × into; flip to match `across`.
        let flip = if plane.y_dir().dot(across) > 0.0 {
            1.0
        } else {
            -1.0
        };
        let outer = (0..4)
            .map(|i| of_geom::Curve2::Line {
                a: DVec2::new(corners[i].x, corners[i].y * flip),
                b: DVec2::new(corners[(i + 1) % 4].x, corners[(i + 1) % 4].y * flip),
            })
            .collect();
        let slot = Profile {
            plane,
            faces: vec![ProfileFace {
                outer,
                holes: Vec::new(),
            }],
        }
        .to_shape()
        .map_err(geo)?
        .prism(dir * ((line.b - line.a).length() + 2.0 * eps))
        .map_err(geo)?;
        let r = body.shape.boolean(BoolOp::Cut, &slot.shape).map_err(geo)?;
        body = rename(fid, &body.names, r, &format!("rip{k}"));
    }
    Ok(body)
}

/// The wall thickness at a flat face: the distance to the nearest opposite
/// parallel face behind it.
fn thickness_at(shape: &Shape, face: u32) -> RResult<f64> {
    let f = shape.face_info(face).map_err(geo)?;
    if f.kind != SurfaceKind::Plane {
        return Err("the fixed face must be flat".into());
    }
    (0..shape.count(Kind::Face))
        .filter_map(|g| shape.face_info(g).ok())
        .filter(|g| g.kind == SurfaceKind::Plane && g.normal.dot(f.normal) < -1.0 + 1e-9)
        .map(|g| (f.point - g.point).dot(f.normal))
        .filter(|d| *d > 1e-6)
        .min_by(f64::total_cmp)
        .ok_or_else(|| "no face lies behind the fixed face; is the part thin-walled?".into())
}

/// Faces of the walls: flat faces with an opposite face one thickness behind.
fn wall_faces(shape: &Shape, t: f64) -> Vec<bool> {
    let infos: Vec<_> = (0..shape.count(Kind::Face))
        .map(|f| shape.face_info(f).ok())
        .collect();
    infos
        .iter()
        .map(|f| {
            let Some(f) = f.filter(|f| f.kind == SurfaceKind::Plane) else {
                return false;
            };
            infos.iter().flatten().any(|g| {
                g.kind == SurfaceKind::Plane
                    && g.normal.dot(f.normal) < -1.0 + 1e-9
                    && ((f.point - g.point).dot(f.normal) - t).abs() < 1e-6 * t.max(1.0)
            })
        })
        .collect()
}

fn lines_of(lines: &[Option<Line>], e: u32) -> Option<Line> {
    lines.get(e as usize).copied().flatten()
}

/// Where two bends would meet at a corner (two outside corner edges on one
/// face whose ends nearly touch), cut a square relief through the corner so
/// that each bend ends on a flat face.
fn corner_reliefs(
    fid: FeatureId,
    body: Body,
    outside: &[u32],
    t: f64,
    params: &SheetMetalParams,
) -> RResult<Body> {
    let shape = &body.shape;
    let edge_faces = shape.edge_faces();
    let size = params.radius + t + params.relief.width(t);
    let reach = 3.0 * t + size;
    let mut boxes = Vec::new();
    for (i, a) in outside.iter().enumerate() {
        for b in &outside[i + 1..] {
            let common: Vec<u32> = edge_faces[*a as usize]
                .iter()
                .filter(|f| edge_faces[*b as usize].contains(f))
                .copied()
                .collect();
            let [face] = common.as_slice() else { continue };
            let (ea, eb) = (
                shape.edge_info(*a).map_err(geo)?,
                shape.edge_info(*b).map_err(geo)?,
            );
            let (da, db) = (
                (ea.end - ea.start).normalize(),
                (eb.end - eb.start).normalize(),
            );
            if da.cross(db).length() < 1e-6 {
                continue;
            }
            // Where the two edge lines cross.
            let n = da.cross(db).normalize();
            let w = (eb.start - ea.start).dot(db.cross(n)) / da.dot(db.cross(n));
            let corner = ea.start + da * w;
            let near =
                |e: &of_geom::EdgeInfo| (e.start - corner).length().min((e.end - corner).length());
            if near(&ea) > reach || near(&eb) > reach {
                continue;
            }
            // Directions from the corner along each edge.
            let along = |e: &of_geom::EdgeInfo, d: DVec3| {
                if (e.start - corner).length() < (e.end - corner).length() {
                    d
                } else {
                    -d
                }
            };
            let (ua, ub) = (along(&ea, da), along(&eb, db));
            let outward = shape.face_info(*face).map_err(geo)?.normal;
            let eps = 1e-3 * t.max(1.0);
            let start = corner + outward * eps - (ua + ub) * eps;
            let plane = Plane::new(start, ua, outward);
            let pts = [
                DVec3::ZERO,
                ua * (size + eps),
                (ua + ub) * (size + eps),
                ub * (size + eps),
            ]
            .map(|p| plane.to_local(start + p));
            let outer = (0..4)
                .map(|k| of_geom::Curve2::Line {
                    a: pts[k],
                    b: pts[(k + 1) % 4],
                })
                .collect();
            let prism = Profile {
                plane,
                faces: vec![ProfileFace {
                    outer,
                    holes: Vec::new(),
                }],
            }
            .to_shape()
            .map_err(geo)?
            .prism(-outward * (size + eps))
            .map_err(geo)?;
            boxes.push(prism.shape);
        }
    }
    let mut body = body;
    for (k, tool) in boxes.iter().enumerate() {
        let r = body.shape.boolean(BoolOp::Cut, tool).map_err(geo)?;
        body = rename(fid, &body.names, r, &format!("relief{k}"));
    }
    Ok(body)
}

/// Round every sharp corner between walls into a bend.
fn insert(
    fid: FeatureId,
    body: Body,
    t: f64,
    params: &SheetMetalParams,
) -> RResult<(Body, Vec<BendRecord>)> {
    let r = params.radius;
    let walls = wall_faces(&body.shape, t);
    let prev_lines: Vec<Option<Line>> = (0..body.shape.count(Kind::Edge))
        .map(|e| {
            body.shape
                .edge_info(e)
                .ok()
                .filter(|i| i.kind == CurveKind::Line)
                .map(|i| Line {
                    a: i.start,
                    b: i.end,
                })
        })
        .collect();
    let edge_faces = body.shape.edge_faces();
    let mut outside = Vec::new();
    let mut inside = Vec::new();
    for (e, faces) in edge_faces.iter().enumerate() {
        let [a, b] = faces.as_slice() else { continue };
        if !walls[*a as usize] || !walls[*b as usize] {
            continue;
        }
        let info = body.shape.edge_info(e as u32).map_err(geo)?;
        let (fa, fb) = (
            body.shape.face_info(*a).map_err(geo)?,
            body.shape.face_info(*b).map_err(geo)?,
        );
        if info.kind != CurveKind::Line || fa.normal.cross(fb.normal).length() < 1e-9 {
            continue;
        }
        // Convex when the second face falls away behind the first.
        if (fb.centroid - info.mid).dot(fa.normal) < 0.0 {
            outside.push(e as u32);
        } else {
            inside.push(e as u32);
        }
    }
    if outside.is_empty() {
        return Err("the part has no sharp corners between walls to bend".into());
    }
    let mut body = corner_reliefs(fid, body, &outside, t, params)?;
    let outside: Vec<Line> = outside
        .iter()
        .filter_map(|e| lines_of(&prev_lines, *e))
        .collect();
    let inside: Vec<Line> = inside
        .iter()
        .filter_map(|e| lines_of(&prev_lines, *e))
        .collect();
    for (edges, radius, tag) in [(&outside, r + t, "bo"), (&inside, r, "bi")] {
        if edges.is_empty() {
            continue;
        }
        // Edges are renumbered by each step: find them again by line.
        let current: Vec<u32> = edges
            .iter()
            .map(|l| edge_along(&body.shape, *l).ok_or("a corner edge was lost"))
            .collect::<Result<_, _>>()?;
        let res = body
            .shape
            .fillet(&current, radius)
            .map_err(|e| format!("can't round the corners into bends: {}", e.0))?;
        let names = super::super::dressup_names(fid, tag, &body, &res);
        body = Body {
            shape: res.shape,
            names,
        };
    }
    // Pair each inside round with the outside round on the same axis.
    let mut cylinders = Vec::new();
    for f in 0..body.shape.count(Kind::Face) {
        let info = body.shape.face_info(f).map_err(geo)?;
        let name = body.names.face_name(f).unwrap_or_default().to_string();
        if info.kind == SurfaceKind::Cylinder && name.starts_with(&format!("{fid}.b")) {
            cylinders.push((info, name));
        }
    }
    let mut bends = Vec::new();
    for (inner, iname) in cylinders
        .iter()
        .filter(|(i, _)| (i.radius - r).abs() < 1e-6)
    {
        let Some((outer, oname)) = cylinders.iter().find(|(o, _)| {
            (o.radius - r - t).abs() < 1e-6
                && o.axis_dir.cross(inner.axis_dir).length() < 1e-9
                && (o.axis_origin - inner.axis_origin)
                    .cross(inner.axis_dir)
                    .length()
                    < 1e-6
        }) else {
            continue;
        };
        let frame = body
            .shape
            .face_frame(body.names.face(iname).unwrap())
            .map_err(geo)?;
        let _ = outer;
        bends.push(BendRecord {
            feature: fid,
            index: bends.len() as u32,
            faces: [iname.clone(), oname.clone()],
            radius: r,
            angle: frame.u.1 - frame.u.0,
            allowance: params.allowance.clone(),
        });
    }
    Ok((body, bends))
}

fn rip_lines(prev: &State, rips: &[RipEdge]) -> RResult<Vec<(Line, RipDirection)>> {
    rips.iter()
        .map(|r| match resolve(prev, &r.edge)? {
            Resolved::Edge { info, .. } if info.kind == CurveKind::Line => Ok((
                Line {
                    a: info.start,
                    b: info.end,
                },
                r.direction,
            )),
            _ => Err("rip straight edges".to_string()),
        })
        .collect()
}

fn finish(
    fid: FeatureId,
    state: &mut State,
    b: usize,
    body: Body,
    fixed: String,
    params: SheetMetalParams,
    bends: Vec<BendRecord>,
) -> RResult<()> {
    state.bodies[b].geometry = body;
    state.sheet_metal.push(SheetMetalBody {
        feature: fid,
        fixed_face: fixed,
        params,
        bends,
        bend_lines: Vec::new(),
        cross_breaks: Vec::new(),
        facets: Vec::new(),
        forms: Vec::new(),
        welds: Vec::new(),
        form_marks: Vec::new(),
        flat_plane: None,
    });
    state.aggregate_bodies()
}

fn fixed_face(prev: &State, r: &Ref) -> RResult<(usize, u32, String)> {
    let (Kind::Face, index) = prev.index_of(r)? else {
        return Err("select a flat face to stay fixed".into());
    };
    let b = body_holding(prev, Kind::Face, index)?;
    if prev
        .sheet_metal
        .iter()
        .any(|rec| record_body(prev, rec) == Some(b))
    {
        return Err("the body is already sheet metal".into());
    }
    let whole = prev.body.as_ref().unwrap();
    let name = whole
        .names
        .face_name(index)
        .ok_or("the face has no name")?
        .to_string();
    let local = prev.bodies[b]
        .geometry
        .names
        .face(&name)
        .ok_or("the fixed face is not on its body")?;
    Ok((b, local, name))
}

pub(crate) fn insert_bends(
    fid: FeatureId,
    p: &InsertBends,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (b, local, name) = fixed_face(prev, &p.fixed_face)?;
    let body = prev.bodies[b].geometry.clone();
    let t = thickness_at(&body.shape, local)?;
    let mut params = check_params(
        &SheetMetalParams {
            thickness: t,
            ..p.sheet.clone()
        },
        prev.material.as_deref(),
    )?;
    params.thickness = t;
    let body = rip_edges(fid, body, &rip_lines(prev, &p.rips)?, t, p.rip_gap)?;
    let (body, bends) = insert(fid, body, t, &params)?;
    finish(fid, state, b, body, name, params, bends)
}

pub(crate) fn rip(fid: FeatureId, p: &Rip, prev: &State, state: &mut State) -> RResult<()> {
    let (Kind::Edge, first) = prev.index_of(&p.edges[0].edge)? else {
        return Err("rip edges".into());
    };
    let b = body_holding(prev, Kind::Edge, first)?;
    let body = prev.bodies[b].geometry.clone();
    let t = prev
        .sheet_metal
        .iter()
        .find(|r| record_body(prev, r) == Some(b))
        .map(|r| r.params.thickness)
        .unwrap_or_else(|| {
            (0..body.shape.count(Kind::Face))
                .filter_map(|f| thickness_at(&body.shape, f).ok())
                .fold(f64::INFINITY, f64::min)
        });
    if !t.is_finite() {
        return Err("can't tell the wall thickness".into());
    }
    let body = rip_edges(fid, body, &rip_lines(prev, &p.edges)?, t, p.gap)?;
    state.bodies[b].geometry = body;
    state.aggregate_bodies()
}

pub(crate) fn convert(
    fid: FeatureId,
    p: &ConvertToSheetMetal,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let params = check_params(&p.sheet, prev.material.as_deref())?;
    let t = params.thickness;
    let (b, local, name) = fixed_face(prev, &p.fixed_face)?;
    let solid = prev.bodies[b].geometry.clone();
    let whole = prev.body.as_ref().unwrap();
    // The bend edges, as lines on this body.
    let mut bend_lines = Vec::new();
    for r in &p.bend_edges {
        let Resolved::Edge { info, .. } = resolve(prev, r)? else {
            return Err("select edges to bend".into());
        };
        if info.kind != CurveKind::Line {
            return Err("bend edges must be straight".into());
        }
        bend_lines.push(Line {
            a: info.start,
            b: info.end,
        });
    }
    let _ = whole;
    let shape = &solid.shape;
    let bend_edges: Vec<u32> = bend_lines
        .iter()
        .map(|l| edge_along(shape, *l).ok_or("a bend edge is not on the fixed face's body"))
        .collect::<Result<_, _>>()?;
    // Faces joined to the fixed face through bend edges become walls.
    let edge_faces = shape.edge_faces();
    let mut walls = vec![false; shape.count(Kind::Face) as usize];
    walls[local as usize] = true;
    let mut queue = vec![local];
    while let Some(f) = queue.pop() {
        for e in &bend_edges {
            let faces = &edge_faces[*e as usize];
            if faces.contains(&f) {
                for g in faces {
                    if !walls[*g as usize] {
                        walls[*g as usize] = true;
                        queue.push(*g);
                    }
                }
            }
        }
    }
    for (f, wall) in walls.iter().enumerate() {
        if *wall && shape.face_info(f as u32).map_err(geo)?.kind != SurfaceKind::Plane {
            return Err("only flat faces can become sheet metal walls".into());
        }
    }
    // Edges between walls that are not bends are ripped.
    let mut rips = Vec::new();
    for (e, faces) in edge_faces.iter().enumerate() {
        if faces.len() == 2
            && walls[faces[0] as usize]
            && walls[faces[1] as usize]
            && !bend_edges.contains(&(e as u32))
        {
            let i = shape.edge_info(e as u32).map_err(geo)?;
            if i.kind != CurveKind::Line {
                return Err("walls meet along a curved edge that is not a bend".into());
            }
            rips.push((
                Line {
                    a: i.start,
                    b: i.end,
                },
                RipDirection::Both,
            ));
        }
    }
    let removed: Vec<u32> = (0..walls.len() as u32)
        .filter(|f| !walls[*f as usize])
        .collect();
    let shelled = shape
        .shell(&removed, t)
        .map_err(|e| format!("can't make walls of that thickness: {}", e.0))?;
    let names = super::super::dressup_names(fid, "s", &solid, &shelled);
    let body = Body {
        shape: shelled.shape,
        names,
    };
    let body = rip_edges(fid, body, &rips, t, p.rip_gap)?;
    let (body, bends) = insert(fid, body, t, &params)?;
    finish(fid, state, b, body, name, params, bends)?;
    if p.keep_body {
        // The kept solid's faces must not share names with the sheet's.
        let faces = solid
            .names
            .faces
            .iter()
            .map(|f| f.iter().map(|n| format!("{fid}.kept.{n}")).collect())
            .collect();
        let names = Naming::from_faces(&solid.shape, faces);
        state.bodies.push(crate::SolidBody {
            id: crate::BodyId(format!("b{fid}:kept")),
            geometry: Body {
                shape: solid.shape,
                names,
            },
        });
        state.aggregate_bodies()?;
    }
    Ok(())
}
