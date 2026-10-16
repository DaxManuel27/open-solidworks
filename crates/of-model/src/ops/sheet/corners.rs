//! Corner treatments: reliefs and bend notches (drawn in the flat and folded
//! onto the sheet), broken corners, closed corners and welded corners.

use of_geom::{
    BoolOp, Curve2 as GCurve, CurveKind, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Shape,
    SurfaceKind,
};

use super::unfold::{Layout, layout};
use super::{RResult, base_name, cut_flat_regions, geo, record_body, record_for_face};
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::resolve::{Resolved, resolve};
use crate::sheet_metal::{
    BendLine, BendNotch, BreakCorner, ClosedCorner, ClosedCornerKind, CornerBreak, CornerRelief,
    CornerReliefKind, CornerWeld, NotchShape, SheetMetalBody, WeldedCorner, bend_allowance,
};
use crate::{FeatureId, Ref};

/// A planar region in `plane` from a closed polygon given in its 2D coordinates.
fn polygon(plane: &Plane, pts: &[DVec2]) -> RResult<Shape> {
    let outer = (0..pts.len())
        .map(|i| GCurve::Line {
            a: pts[i],
            b: pts[(i + 1) % pts.len()],
        })
        .collect();
    face(plane, outer)
}

fn face(plane: &Plane, outer: Vec<GCurve>) -> RResult<Shape> {
    Profile {
        plane: *plane,
        faces: vec![ProfileFace {
            outer,
            holes: Vec::new(),
        }],
    }
    .to_shape()
    .map_err(geo)
}

fn circle(plane: &Plane, c: DVec2, r: f64) -> RResult<Shape> {
    face(
        plane,
        vec![GCurve::Circle {
            center: c,
            radius: r,
        }],
    )
}

/// A slot from `a` to `b` of width `w`, with round ends when `round`.
fn slot(plane: &Plane, a: DVec2, b: DVec2, w: f64, round: bool) -> RResult<Shape> {
    let d = (b - a).normalize();
    let n = DVec2::new(-d.y, d.x) * (w / 2.0);
    if !round {
        return polygon(plane, &[a - n, b - n, b + n, a + n]);
    }
    face(
        plane,
        vec![
            GCurve::Line { a: a - n, b: b - n },
            GCurve::Arc {
                center: b,
                start: b - n,
                end: b + n,
            },
            GCurve::Line { a: b + n, b: a + n },
            GCurve::Arc {
                center: a,
                start: a + n,
                end: a - n,
            },
        ],
    )
}

/// The flat width (bend allowance) of a bend line.
fn width(line: &BendLine, record: &SheetMetalBody) -> RResult<f64> {
    let allowance = record
        .bends
        .iter()
        .find(|b| Some(b.feature) == line.feature && Some(b.index) == line.index)
        .map(|b| b.allowance.clone())
        .unwrap_or(record.params.allowance.clone());
    if line.radius <= 0.0 {
        return Ok(0.0);
    }
    bend_allowance(&allowance, line.angle, line.radius, record.params.thickness)
}

/// Names of the bend faces selected by `refs`, or None for every bend.
fn selected(prev: &State, refs: &[Ref]) -> RResult<Option<Vec<String>>> {
    if refs.is_empty() {
        return Ok(None);
    }
    let whole = prev.body.as_ref().ok_or("there's no body")?;
    refs.iter()
        .map(|r| match prev.index_of(r)? {
            (Kind::Face, i) => Ok(whole
                .names
                .face_name(i)
                .map(|n| base_name(n).to_string())
                .unwrap_or_default()),
            _ => Err("select bend faces".to_string()),
        })
        .collect::<RResult<Vec<_>>>()
        .map(Some)
}

fn wanted(line: &BendLine, record: &SheetMetalBody, names: &Option<Vec<String>>) -> bool {
    let Some(names) = names else { return true };
    record
        .bends
        .iter()
        .find(|b| Some(b.feature) == line.feature && Some(b.index) == line.index)
        .is_some_and(|b| {
            b.faces
                .iter()
                .any(|f| names.iter().any(|n| n == base_name(f)))
        })
}

/// Run `regions` for every sheet metal body and cut what they draw in its flat.
fn cut_in_flats(
    fid: FeatureId,
    prev: &State,
    state: &mut State,
    mut regions: impl FnMut(&SheetMetalBody, &Layout) -> RResult<Vec<Shape>>,
) -> RResult<()> {
    let mut hit = false;
    for record in &prev.sheet_metal {
        let Some(b) = record_body(prev, record) else {
            continue;
        };
        let body = &prev.bodies[b].geometry;
        let flat = layout(body, record, None)?;
        let shapes = regions(record, &flat)?;
        if shapes.is_empty() {
            continue;
        }
        state.bodies[b].geometry =
            cut_flat_regions(fid, body, &flat, record.params.thickness, &shapes)?;
        hit = true;
    }
    if !hit {
        return Err("no corners or bends to treat".into());
    }
    state.aggregate_bodies()
}

pub(crate) fn corner_relief(
    fid: FeatureId,
    p: &CornerRelief,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let names = selected(prev, &p.bends)?;
    cut_in_flats(fid, prev, state, |record, flat| {
        let t = record.params.thickness;
        let lines: Vec<&BendLine> = flat
            .bend_lines
            .iter()
            .filter(|l| l.radius > 0.0 && wanted(l, record, &names))
            .collect();
        let local = |p: [f64; 3]| flat.plane.to_local(DVec3::from(p));
        let mut out = Vec::new();
        for (i, l1) in lines.iter().enumerate() {
            for l2 in &lines[i + 1..] {
                let (a1, b1, a2, b2) = (local(l1.a), local(l1.b), local(l2.a), local(l2.b));
                let (d1, d2) = ((b1 - a1).normalize(), (b2 - a2).normalize());
                let cross = d1.perp_dot(d2);
                if cross.abs() < 1e-6 {
                    continue;
                }
                let s = (a2 - a1).perp_dot(d2) / cross;
                let q = a1 + d1 * s;
                let (w1, w2) = (width(l1, record)?, width(l2, record)?);
                let reach = w1.max(w2) + 4.0 * t;
                let end_near = |a: DVec2, b: DVec2| (a - q).length().min((b - q).length());
                if end_near(a1, b1) > reach || end_near(a2, b2) > reach {
                    continue;
                }
                // Each line runs from the corner into its bend.
                let away = |a: DVec2, b: DVec2, d: DVec2| {
                    if (a - q).length() < (b - q).length() {
                        d
                    } else {
                        -d
                    }
                };
                let (u1, u2) = (away(a1, b1, d1), away(a2, b2, d2));
                let o = p.ratio * t;
                let (h1, h2) = (w1 / 2.0 + o, w2 / 2.0 + o);
                // Outward from the material, along the bisector.
                let out_dir = -(u1 + u2).normalize_or_zero();
                let center = if p.centered {
                    q
                } else {
                    q + out_dir * (h1.max(h2) - o)
                };
                let (n1, n2) = (DVec2::new(-u1.y, u1.x), DVec2::new(-u2.y, u2.x));
                let corner = |s1: f64, s2: f64| {
                    // (x - c)·n1 = s1 h2 and (x - c)·n2 = s2 h1: across each strip.
                    let det = n1.perp_dot(n2);
                    let (r1, r2) = (s1 * h1, s2 * h2);
                    center + DVec2::new(r1 * n2.y - r2 * n1.y, r2 * n1.x - r1 * n2.x) / det
                };
                let reach_out = h1.max(h2) * 3.0 + t;
                let shape = match p.kind {
                    CornerReliefKind::Rectangular => polygon(
                        &flat.plane,
                        &[
                            corner(1.0, 1.0),
                            corner(1.0, -1.0),
                            corner(-1.0, -1.0),
                            corner(-1.0, 1.0),
                        ],
                    )?,
                    CornerReliefKind::Suitcase => slot(
                        &flat.plane,
                        center,
                        center + out_dir * reach_out,
                        2.0 * h1.max(h2),
                        false,
                    )?,
                    CornerReliefKind::Circular => circle(&flat.plane, center, h1.max(h2))?,
                    CornerReliefKind::FullRound => {
                        circle(&flat.plane, center, (w1 / 2.0).hypot(w2 / 2.0) + o)?
                    }
                    CornerReliefKind::Tear => slot(
                        &flat.plane,
                        center - out_dir * h1.max(h2),
                        center + out_dir * reach_out,
                        (t / 10.0).max(0.02),
                        false,
                    )?,
                    CornerReliefKind::Obround | CornerReliefKind::ConstantWidth => slot(
                        &flat.plane,
                        center,
                        center + out_dir * reach_out,
                        2.0 * h1.max(h2),
                        p.kind == CornerReliefKind::Obround,
                    )?,
                };
                out.push(shape);
            }
        }
        Ok(out)
    })
}

pub(crate) fn bend_notch(
    fid: FeatureId,
    p: &BendNotch,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let names = selected(prev, &p.bends)?;
    cut_in_flats(fid, prev, state, |record, flat| {
        let eps = 1e-3 * record.params.thickness.max(1.0);
        let mut out = Vec::new();
        for l in flat.bend_lines.iter().filter(|l| wanted(l, record, &names)) {
            let (a, b) = (
                flat.plane.to_local(DVec3::from(l.a)),
                flat.plane.to_local(DVec3::from(l.b)),
            );
            let ends: &[(DVec2, DVec2)] = if p.both_ends {
                &[(a, b), (b, a)]
            } else {
                &[(a, b)]
            };
            for (at, other) in ends {
                let d = (*other - *at).normalize();
                let n = DVec2::new(-d.y, d.x) * (p.width / 2.0);
                let base = *at - d * eps;
                let pts = match p.shape {
                    NotchShape::V => vec![base - n, *at + d * p.depth, base + n],
                    NotchShape::Rectangular => vec![
                        base - n,
                        *at + d * p.depth - n,
                        *at + d * p.depth + n,
                        base + n,
                    ],
                };
                out.push(polygon(&flat.plane, &pts)?);
            }
        }
        Ok(out)
    })
}

/// Edges through the sheet's thickness at the corners of its outline:
/// straight, one thickness long, between two faces that are not the sheet's
/// sides. Convex ones are outside corners.
pub(super) fn outline_corners(body: &Body, t: f64) -> RResult<Vec<(u32, bool)>> {
    let shape = &body.shape;
    let edge_faces = shape.edge_faces();
    let infos: Vec<_> = (0..shape.count(Kind::Face))
        .map(|f| shape.face_info(f).map_err(geo))
        .collect::<RResult<_>>()?;
    let mut out = Vec::new();
    for e in 0..shape.count(Kind::Edge) {
        let info = shape.edge_info(e).map_err(geo)?;
        if info.kind != CurveKind::Line || (info.length - t).abs() > 1e-6 * t.max(1.0) {
            continue;
        }
        let [a, b] = edge_faces[e as usize].as_slice() else {
            continue;
        };
        let dir = (info.end - info.start) / info.length;
        let across = |f: &of_geom::FaceInfo| match f.kind {
            SurfaceKind::Plane => f.normal.dot(dir).abs() < 1e-9,
            SurfaceKind::Cylinder => f.axis_dir.cross(dir).length() < 1e-9,
            _ => false,
        };
        let (fa, fb) = (&infos[*a as usize], &infos[*b as usize]);
        if !across(fa) || !across(fb) {
            continue;
        }
        // Tangent faces meet smoothly: not a corner.
        let na = if fa.kind == SurfaceKind::Plane {
            fa.normal
        } else {
            continue;
        };
        let nb = if fb.kind == SurfaceKind::Plane {
            fb.normal
        } else {
            continue;
        };
        if na.dot(nb) > 1.0 - 1e-9 {
            continue;
        }
        let convex = (fb.centroid - info.mid).dot(na) < 0.0;
        out.push((e, convex));
    }
    Ok(out)
}

/// Of the automatically chosen `corners`, those whose sides along the outline
/// are long enough to take a break of `size` at every chosen corner they touch.
pub(super) fn fitting_corners(
    body: &Body,
    t: f64,
    corners: &[u32],
    size: f64,
) -> RResult<Vec<u32>> {
    let shape = &body.shape;
    let edge_faces = shape.edge_faces();
    let mut keep: Vec<u32> = corners.to_vec();
    loop {
        let mut uses = std::collections::HashMap::<u32, usize>::new();
        for e in &keep {
            for f in &edge_faces[*e as usize] {
                *uses.entry(*f).or_default() += 1;
            }
        }
        let mut fits = Vec::with_capacity(keep.len());
        for e in &keep {
            let mut ok = true;
            for f in &edge_faces[*e as usize] {
                // Side faces run through the thickness: length is area / t.
                let side = shape.face_info(*f).map_err(geo)?.area / t;
                ok &= side + 1e-9 >= size * uses[f] as f64;
            }
            if ok {
                fits.push(*e);
            }
        }
        if fits.len() == keep.len() {
            return Ok(keep);
        }
        keep = fits;
    }
}

pub(super) fn break_size(corner: CornerBreak) -> f64 {
    match corner {
        CornerBreak::Chamfer { distance } => distance,
        CornerBreak::Fillet { radius } => radius,
    }
}

/// Chamfer or round edges of a body.
pub(crate) fn break_edges(
    fid: FeatureId,
    body: &Body,
    edges: &[u32],
    corner: CornerBreak,
) -> RResult<Body> {
    if edges.is_empty() {
        return Ok(body.clone());
    }
    let r = match corner {
        CornerBreak::Chamfer { distance } => body.shape.chamfer(edges, distance, distance),
        CornerBreak::Fillet { radius } => body.shape.fillet(edges, radius),
    }
    .map_err(|e| format!("can't break the corners: {}", e.0))?;
    let names = super::super::dressup_names(fid, "k", body, &r);
    Ok(Body {
        shape: r.shape,
        names,
    })
}

pub(crate) fn break_corner(
    fid: FeatureId,
    p: &BreakCorner,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let whole = prev.body.as_ref().ok_or("there's no body")?;
    let mut done = false;
    for record in &prev.sheet_metal {
        let Some(b) = record_body(prev, record) else {
            continue;
        };
        let body = &prev.bodies[b].geometry;
        let local = body
            .shape
            .indices_in(&whole.shape, Kind::Edge)
            .map_err(geo)?;
        let edges: Vec<u32> = if p.edges.is_empty() {
            let t = record.params.thickness;
            let corners: Vec<u32> = outline_corners(body, t)?
                .into_iter()
                .filter(|(_, convex)| *convex != p.internal_only)
                .map(|(e, _)| e)
                .collect();
            fitting_corners(body, t, &corners, break_size(p.corner))?
        } else {
            let mut out = Vec::new();
            for r in &p.edges {
                let (Kind::Edge, g) = prev.index_of(r)? else {
                    return Err("select edges through the sheet".into());
                };
                if let Some(l) = local.iter().position(|x| *x == g) {
                    out.push(l as u32);
                }
            }
            out
        };
        if edges.is_empty() {
            continue;
        }
        state.bodies[b].geometry = break_edges(fid, body, &edges, p.corner)?;
        done = true;
    }
    if !done {
        return Err("no corners to break".into());
    }
    state.aggregate_bodies()
}

/// Fuse or cut `tool` into `body`, naming new faces with `new_name`.
fn apply(
    fid: FeatureId,
    body: Body,
    op: BoolOp,
    tool: &Shape,
    tool_names: Option<&Naming>,
    new_name: &str,
) -> RResult<Body> {
    let r = body.shape.boolean(op, tool).map_err(geo)?;
    Ok(Body {
        names: Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&body.names), tool_names],
            },
            &mut |_| Some(new_name.to_string()),
            &fid.to_string(),
        ),
        shape: r.shape,
    })
}

pub(crate) fn closed_corner(
    fid: FeatureId,
    p: &ClosedCorner,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    for (k, r) in p.faces.iter().enumerate() {
        let (Kind::Face, global) = prev.index_of(r)? else {
            return Err("select end faces of flanges".into());
        };
        let record = record_for_face(prev, global)?;
        let b = record_body(state, &state.sheet_metal[record]).ok_or("the body is gone")?;
        let t = state.sheet_metal[record].params.thickness;
        let whole = prev.body.as_ref().unwrap();
        let name = whole
            .names
            .face_name(global)
            .ok_or("the face has no name")?
            .to_string();
        let body = state.bodies[b].geometry.clone();
        let f = body.names.face(&name).ok_or("the face is gone")?;
        let info = body.shape.face_info(f).map_err(geo)?;
        if info.kind != SurfaceKind::Plane {
            return Err("select flat end faces of flanges".into());
        }
        // The flange's wall: the largest flat face beside the end face.
        let ef = body.shape.edge_faces();
        let wall = body.shape.face_edges()[f as usize]
            .iter()
            .flat_map(|e| ef[*e as usize].iter().copied())
            .filter(|g| *g != f)
            .filter_map(|g| body.shape.face_info(g).ok().map(|i| (g, i)))
            .filter(|(_, i)| i.kind == SurfaceKind::Plane)
            .max_by(|a, b| a.1.area.total_cmp(&b.1.area))
            .ok_or("the end face is not beside a flange wall")?;
        let (wall_face, wall) = wall;
        let n = info.normal;
        let dir = (n - wall.normal * n.dot(wall.normal)).normalize_or_zero();
        if dir == DVec3::ZERO {
            return Err("the end face is parallel to its flange".into());
        }
        // Coordinates at the end face: u toward the other flange, c along the
        // flange's height, and w through this flange's thickness.
        let across = dir.cross(wall.normal).normalize();
        let verts = |face: u32| -> RResult<Vec<DVec3>> {
            let s = body.shape.sub_shape(Kind::Face, face).map_err(geo)?;
            Ok((0..s.count(Kind::Vertex))
                .filter_map(|v| s.vertex_point(v).ok())
                .collect())
        };
        let range = |pts: &[DVec3], axis: DVec3| {
            pts.iter()
                .map(|v| (*v - info.centroid).dot(axis))
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| {
                    (lo.min(x), hi.max(x))
                })
        };
        // The end face may run on down through the bend: only the wall's
        // height and thickness are closed.
        let (u_lo, _) = range(&verts(f)?, dir);
        let (c_lo, c_hi) = range(&verts(wall_face)?, across);
        let w_hi = (wall.point - info.centroid).dot(wall.normal);
        let w_lo = w_hi - t;
        // The other flange's inside and outside faces: planes square to u,
        // ahead of this end face's back and level with its height.
        let other = |sign: f64, beyond: f64| {
            (0..body.shape.count(Kind::Face))
                .filter_map(|g| body.shape.face_info(g).ok())
                .filter(|g| g.kind == SurfaceKind::Plane && g.normal.dot(dir) * sign > 1.0 - 1e-9)
                .filter(|g| {
                    let c = (g.centroid - info.centroid).dot(across);
                    c > c_lo - 1e-6 && c < c_hi + 1e-6
                })
                .map(|g| ((g.point - info.centroid).dot(dir), g.centroid))
                .filter(|(u, _)| *u > beyond)
                .min_by(|a, b| a.0.total_cmp(&b.0))
        };
        let (xi, inside) = other(-1.0, u_lo + 1e-6).ok_or("no flange ahead of the end face")?;
        let xo = other(1.0, xi + 1e-6).map_or(xi + t, |(u, _)| u);
        // v runs from the other flange toward this one; this flange spans vi..vo.
        let toward = (inside - info.centroid).dot(wall.normal);
        let (vdir, vi, vo) = if toward < 0.0 {
            (wall.normal, w_lo, w_hi)
        } else {
            (-wall.normal, -w_hi, -w_lo)
        };
        let at = |u: f64, v: f64| info.centroid + dir * u + vdir * v + across * c_lo;
        let plane = Plane::new(at(0.0, 0.0), dir, across);
        let height = across * (c_hi - c_lo);
        let block = |u0: f64, u1: f64, v0: f64, v1: f64| -> RResult<Shape> {
            let pts = [at(u0, v0), at(u1, v0), at(u1, v1), at(u0, v1)].map(|q| plane.to_local(q));
            Ok(polygon(&plane, &pts)?.prism(height).map_err(geo)?.shape)
        };
        let g = p.gap;
        let reach = (xo - xi).max(t);
        // Square off both flanges round the corner column (u xi..xo, v vi..vo).
        let clear = block(xi - g, xo + reach, vi - g, vo + reach)?;
        let mut body = apply(
            fid,
            body,
            BoolOp::Cut,
            &clear,
            None,
            &format!("{fid}.c{k}.gap"),
        )?;
        // Then fill: this flange's part and the other flange's part.
        let span = p.overlap_ratio.clamp(0.0, 1.0) * (xo - xi);
        let back = 2.0 * reach;
        let mut fills = Vec::new();
        match p.kind {
            ClosedCornerKind::Butt => {
                fills.push(block(u_lo.min(xi - g) - back, xi - g, vi, vo)?);
                fills.push(block(xi, xo, vi - g - back, vo)?);
            }
            ClosedCornerKind::Overlap => {
                fills.push(block(u_lo.min(xi - g) - back, xi + span, vi, vo)?);
                if xi + span + g < xo {
                    fills.push(block(xi + span + g, xo, vi - g - back, vo)?);
                }
                fills.push(block(xi, (xi + span + g).min(xo), vi - g - back, vi - g)?);
            }
            ClosedCornerKind::Underlap => {
                fills.push(block(xi, xo, vi - g - back, vi + span)?);
                if vi + span + g < vo {
                    fills.push(block(u_lo.min(xi - g) - back, xo, vi + span + g, vo)?);
                }
                fills.push(block(
                    u_lo.min(xi - g) - back,
                    xi - g,
                    vi,
                    (vi + span + g).min(vo),
                )?);
            }
        }
        // Fills reach back into each flange's own wall so they join it.
        let fill = Shape::compound(&fills);
        let fill_names = Naming::by_index(&fill, &format!("{fid}.c{k}"));
        body = apply(
            fid,
            body,
            BoolOp::Fuse,
            &fill,
            Some(&fill_names),
            &format!("{fid}.c{k}"),
        )?;
        state.bodies[b].geometry = body;
    }
    state.aggregate_bodies()
}

pub(crate) fn welded_corner(
    fid: FeatureId,
    p: &WeldedCorner,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let Resolved::Edge { index, info } = resolve(prev, &p.edge)? else {
        return Err("select an edge of the corner's seam".into());
    };
    let faces = prev.body.as_ref().unwrap().shape.edge_faces()[index as usize].clone();
    let record = record_for_face(prev, faces[0])?;
    state.sheet_metal[record].welds.push(CornerWeld {
        feature: fid,
        a: info.start.to_array(),
        b: info.end.to_array(),
        size: p.size,
        symbol: p.symbol.clone(),
    });
    Ok(())
}
