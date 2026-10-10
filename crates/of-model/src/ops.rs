//! Building solid features.

use std::f64::consts::{PI, TAU};

use of_geom::{
    BoolOp, Curve2 as GCurve, DAffine3, DVec2, DVec3, HistoryEntry, Kind, Plane, Profile,
    ProfileFace, Relation, Shape,
};
use of_sketch::{Curve2 as SCurve, Geometry, Sketch};

use crate::build::{Body, State, Tool};
use crate::features::holes;
use crate::naming::{Inputs, Naming};
use crate::resolve::{RResult, Resolved, resolve};
use crate::{
    Chamfer, ChamferMode, CircularPattern, EndCondition, Extrude, FeatureId, FeatureKind, Fillet,
    Hole, HoleEnd, HoleKind, HoleStandard, Import, ImportFormat, LinearPattern, MirrorDef, Ref,
    Revolve, RevolveAngle, ShellDef, Thin, ThinKind, unit,
};

pub(crate) fn build(
    fid: FeatureId,
    kind: &FeatureKind,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let result = match kind {
        FeatureKind::SplitLine(p) => split_line(fid, p, prev, state),
        FeatureKind::Draft(p) => draft(fid, p, prev, state),
        FeatureKind::VariableFillet(p) => variable_fillet(fid, p, prev, state),
        FeatureKind::Sweep(p) => sweep(fid, p, prev, state),
        FeatureKind::Loft(p) => loft(fid, p, prev, state),
        FeatureKind::Extrude(e) => extrude(fid, e, prev, state),
        FeatureKind::Revolve(r) => revolve(fid, r, prev, state),
        FeatureKind::Fillet(f) => fillet(fid, f, prev, state),
        FeatureKind::Chamfer(c) => chamfer(fid, c, prev, state),
        FeatureKind::Shell(s) => shell(fid, s, prev, state),
        FeatureKind::Hole(h) => hole(fid, h, prev, state),
        FeatureKind::LinearPattern(p) => linear_pattern(fid, p, prev, state),
        FeatureKind::CircularPattern(p) => circular_pattern(fid, p, prev, state),
        FeatureKind::Mirror(m) => mirror(fid, m, prev, state),
        FeatureKind::Import(i) => import(fid, i, state),
        _ => Ok(()),
    };
    if result.is_ok() && state.tools.contains_key(&fid) {
        state.tool_order.push(fid);
    }
    result
}

fn geo(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn split_line(
    fid: FeatureId,
    p: &crate::ProjectedCurve,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let body = body_of(prev)?;
    let (kind, face) = prev.index_of(&p.target)?;
    if kind != Kind::Face {
        return Err("select a face to split".into());
    }
    let wire = crate::curves::projected(p, prev)?;
    let result = body.shape.split_face(face, &wire).map_err(geo)?;
    if result.shape.count(Kind::Face) <= body.shape.count(Kind::Face) {
        return Err("the projected curve does not divide the face".into());
    }
    let names = dressup_names(fid, "split", body, &result);
    state.body = Some(Body {
        shape: result.shape,
        names,
    });
    Ok(())
}

fn variable_fillet(
    fid: FeatureId,
    p: &crate::VariableFillet,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let body = body_of(prev)?;
    let (kind, edge) = prev.index_of(&p.edge)?;
    if kind != Kind::Edge {
        return Err("select an edge to fillet".into());
    }
    let stations: Vec<_> = if p.reverse {
        p.stations
            .iter()
            .rev()
            .map(|s| (1.0 - s.position, s.radius))
            .collect()
    } else {
        p.stations.iter().map(|s| (s.position, s.radius)).collect()
    };
    let result = body.shape.variable_fillet(edge, &stations).map_err(geo)?;
    let names = dressup_names(fid, "vf", body, &result);
    remember_dressup(fid, body, &result.shape, &names, state)?;
    state.body = Some(Body {
        shape: result.shape,
        names,
    });
    Ok(())
}

fn draft(fid: FeatureId, p: &crate::Draft, prev: &State, state: &mut State) -> RResult<()> {
    let body = body_of(prev)?;
    let plane = resolve(prev, &p.neutral_plane)?.plane()?;
    let direction = match &p.direction {
        Some(r) => resolve(prev, r)?.direction()?,
        None => plane.normal,
    };
    let faces = p
        .faces
        .iter()
        .map(|r| {
            let (kind, index) = prev.index_of(r)?;
            if kind != Kind::Face {
                return Err("select faces to draft".into());
            }
            Ok(index)
        })
        .collect::<RResult<Vec<_>>>()?;
    let result = body
        .shape
        .draft(&faces, &plane, direction, p.angle)
        .map_err(geo)?;
    let names = dressup_names(fid, "draft", body, &result);
    remember_dressup(fid, body, &result.shape, &names, state)?;
    state.body = Some(Body {
        shape: result.shape,
        names,
    });
    Ok(())
}

fn section_names(prev: &State, id: FeatureId, shape: &Shape) -> RResult<Vec<String>> {
    let (plane, sketch) = sketch_of(prev, id)?;
    let curves = sketch
        .curve_ids()
        .filter_map(|id| sketch.curve(id).ok().map(|c| (id, c)))
        .collect::<Vec<_>>();
    (0..shape.count(Kind::Edge))
        .map(|edge| {
            let mid = plane.to_local(shape.edge_info(edge).map_err(geo)?.mid);
            let (entity, _) = curves
                .iter()
                .min_by(|a, b| {
                    (a.1.closest_point(mid) - mid)
                        .length()
                        .total_cmp(&(b.1.closest_point(mid) - mid).length())
                })
                .ok_or("profile has no curves")?;
            Ok(format!("s{id}.e{entity}"))
        })
        .collect()
}

fn sweep(fid: FeatureId, p: &crate::Sweep, prev: &State, state: &mut State) -> RResult<()> {
    let (plane, section) = crate::curves::section(prev, p.profile)?;
    let path = crate::curves::path(prev, &p.path)?;
    let path = if p.reverse_path {
        path.reversed()
    } else {
        path
    };
    let orientation = match p.orientation {
        crate::SweepOrientation::MinimumTwist => 0,
        crate::SweepOrientation::Frenet => 1,
        crate::SweepOrientation::Fixed => 2,
    };
    let result = section.sweep(&path, &plane, orientation).map_err(geo)?;
    let edges = section_names(prev, p.profile, &section)?;
    let names = Naming::from_history(
        &result.shape,
        &result.history,
        Inputs {
            named: [None, None],
        },
        &mut |h| {
            if h.rel == Relation::StartCap {
                return Some(format!("{fid}.start"));
            }
            if h.rel == Relation::EndCap {
                return Some(format!("{fid}.end"));
            }
            (h.input == 0 && h.from_kind == Kind::Edge)
                .then(|| edges.get(h.from as usize).map(|e| format!("{fid}.{e}")))
                .flatten()
        },
        &fid.to_string(),
    );
    apply_tool(
        fid,
        state,
        result.shape,
        names,
        if p.cut { BoolOp::Cut } else { BoolOp::Fuse },
        p.merge,
    )
}

fn loft(fid: FeatureId, p: &crate::Loft, prev: &State, state: &mut State) -> RResult<()> {
    if p.profiles.len() < 2 {
        return Err("select at least two ordered profiles".into());
    }
    let mut unique = std::collections::HashSet::new();
    if p.profiles.iter().any(|id| !unique.insert(id)) {
        return Err("each loft profile must be different".into());
    }
    let mut profiles = Vec::new();
    let mut edges = Vec::new();
    for id in &p.profiles {
        let (_, section) = crate::curves::section(prev, *id)?;
        let section = if p.reversed.contains(id) {
            section.reversed()
        } else {
            section
        };
        edges.extend(section_names(prev, *id, &section)?);
        profiles.push(section);
    }
    let result = Shape::loft(&profiles, p.ruled).map_err(geo)?;
    let names = Naming::from_history(
        &result.shape,
        &result.history,
        Inputs {
            named: [None, None],
        },
        &mut |h| {
            if h.rel == Relation::StartCap {
                return Some(format!("{fid}.start"));
            }
            if h.rel == Relation::EndCap {
                return Some(format!("{fid}.end"));
            }
            (h.from_kind == Kind::Edge)
                .then(|| edges.get(h.from as usize).map(|e| format!("{fid}.{e}")))
                .flatten()
        },
        &fid.to_string(),
    );
    apply_tool(
        fid,
        state,
        result.shape,
        names,
        if p.cut { BoolOp::Cut } else { BoolOp::Fuse },
        p.merge,
    )
}

pub(crate) fn to_geom(c: &SCurve) -> GCurve {
    match c {
        SCurve::Line { a, b } => GCurve::Line { a: *a, b: *b },
        SCurve::Bezier { points } => GCurve::Bezier { points: *points },
        SCurve::Arc { center, start, end } => GCurve::Arc {
            center: *center,
            start: *start,
            end: *end,
        },
        SCurve::Circle { center, radius } => GCurve::Circle {
            center: *center,
            radius: *radius,
        },
        SCurve::Spline { points, closed } => GCurve::Spline {
            points: points.clone(),
            closed: *closed,
        },
    }
}

/// A sketch profile as kernel faces, with a name for each face edge.
struct ProfileShape {
    shape: Shape,
    plane: Plane,
    /// Per kernel edge of `shape`: `s{entity}` for the sketch curve it came from.
    edge_names: Vec<String>,
}

fn sketch_of(prev: &State, id: FeatureId) -> RResult<&(Plane, Sketch)> {
    prev.sketches
        .get(&id)
        .ok_or_else(|| "the sketch is missing or failed to build".to_string())
}

fn sketch_profile(prev: &State, sketch: FeatureId, thin: Option<&Thin>) -> RResult<ProfileShape> {
    let (plane, sk) = sketch_of(prev, sketch)?;
    let contours = sk.contours().map_err(geo)?;
    let faces: Vec<ProfileFace> = match thin {
        None => {
            if contours.regions.is_empty() {
                return Err("the sketch has no closed contours".into());
            }
            contours
                .regions
                .iter()
                .map(|r| ProfileFace {
                    outer: contours.loops[r.outer].curves.iter().map(to_geom).collect(),
                    holes: r
                        .holes
                        .iter()
                        .map(|h| contours.loops[*h].curves.iter().map(to_geom).collect())
                        .collect(),
                })
                .collect()
        }
        Some(t) => thin_faces(sk, &contours, t)?,
    };
    let profile = Profile {
        plane: *plane,
        faces,
    };
    let shape = profile.to_shape().map_err(geo)?;
    // Which sketch curve each kernel edge came from, by nearest midpoint.
    let candidates: Vec<(of_sketch::Id, SCurve)> = contours
        .loops
        .iter()
        .chain(&contours.open)
        .flat_map(|c| c.entities.iter().copied().zip(c.curves.iter().cloned()))
        .collect();
    let edge_names = (0..shape.count(Kind::Edge))
        .map(|i| {
            let Ok(info) = shape.edge_info(i) else {
                return "e".to_string();
            };
            let mid = plane.to_local(info.mid);
            let best = candidates.iter().min_by(|a, b| {
                let da = (a.1.closest_point(mid) - mid).length();
                let db = (b.1.closest_point(mid) - mid).length();
                da.total_cmp(&db)
            });
            match best {
                Some((id, c)) if thin.is_none() && (c.closest_point(mid) - mid).length() < 1e-4 => {
                    format!("s{id}")
                }
                Some((id, _)) => format!("t{id}"),
                None => "e".to_string(),
            }
        })
        .collect();
    Ok(ProfileShape {
        shape,
        plane: *plane,
        edge_names,
    })
}

/// Faces for a thin feature: each contour thickened to the given sides.
fn thin_faces(sk: &Sketch, contours: &of_sketch::Contours, t: &Thin) -> RResult<Vec<ProfileFace>> {
    if t.thickness <= 0.0 {
        return Err("the wall thickness must be more than zero".into());
    }
    let (mut left, mut right) = match t.kind {
        ThinKind::OneDirection => (t.thickness, 0.0),
        ThinKind::MidPlane => (t.thickness / 2.0, t.thickness / 2.0),
        ThinKind::TwoDirection => (t.thickness, t.thickness2.max(0.0)),
    };
    if t.reverse {
        std::mem::swap(&mut left, &mut right);
    }
    let mut faces = Vec::new();
    let chains: Vec<(&of_sketch::Chain, bool)> = contours
        .loops
        .iter()
        .map(|c| (c, true))
        .chain(contours.open.iter().map(|c| (c, false)))
        .collect();
    if chains.is_empty() {
        return Err("the sketch has no contours".into());
    }
    for (chain, closed) in chains {
        let side = |d: f64| -> RResult<Vec<GCurve>> {
            if d == 0.0 {
                return Ok(chain.curves.iter().map(to_geom).collect());
            }
            // Offset a copy of just this chain.
            let mut tmp = Sketch::new();
            let ids = copy_chain(sk, &chain.entities, &mut tmp);
            let made = of_sketch::tools::offset(&mut tmp, &ids, d).map_err(geo)?;
            made.entities
                .iter()
                .map(|e| tmp.curve(*e).map(|c| to_geom(&c)).map_err(geo))
                .collect()
        };
        let (a, b) = (side(left)?, side(-right)?);
        if closed {
            let area = |c: &[GCurve]| -> f64 {
                let pts: Vec<DVec2> = c.iter().flat_map(sample).collect();
                of_sketch::geom2::signed_area(&pts).abs()
            };
            let (outer, inner) = if area(&a) >= area(&b) { (a, b) } else { (b, a) };
            faces.push(ProfileFace {
                outer,
                holes: vec![inner],
            });
        } else {
            let (a0, a1) = chain_ends(&a);
            let (b0, b1) = chain_ends(&b);
            let mut outer = a;
            outer.push(GCurve::Line { a: a1, b: b1 });
            outer.extend(b.into_iter().rev());
            outer.push(GCurve::Line { a: b0, b: a0 });
            faces.push(ProfileFace {
                outer,
                holes: vec![],
            });
        }
    }
    Ok(faces)
}

fn sample(c: &GCurve) -> Vec<DVec2> {
    let s = match c {
        GCurve::Line { a, b } => SCurve::Line { a: *a, b: *b },
        GCurve::Arc { center, start, end } => SCurve::Arc {
            center: *center,
            start: *start,
            end: *end,
        },
        GCurve::Circle { center, radius } => SCurve::Circle {
            center: *center,
            radius: *radius,
        },
        GCurve::Bezier { points } => SCurve::Bezier { points: *points },
        GCurve::Spline { points, closed } => SCurve::Spline {
            points: points.clone(),
            closed: *closed,
        },
    };
    s.sample(0.01)
}

/// Start and end of an ordered chain of curves.
fn chain_ends(c: &[GCurve]) -> (DVec2, DVec2) {
    if c.len() == 1 {
        return (c[0].start(), c[0].end());
    }
    let near =
        |p: DVec2, q: &GCurve| (q.start() - p).length() < 1e-6 || (q.end() - p).length() < 1e-6;
    let start = if near(c[0].end(), &c[1]) {
        c[0].start()
    } else {
        c[0].end()
    };
    let n = c.len();
    let end = if near(c[n - 1].start(), &c[n - 2]) {
        c[n - 1].end()
    } else {
        c[n - 1].start()
    };
    (start, end)
}

/// Copy entities (and their points) into another sketch; returns new ids.
fn copy_chain(from: &Sketch, ids: &[of_sketch::Id], to: &mut Sketch) -> Vec<of_sketch::Id> {
    let mut map = std::collections::HashMap::new();
    let mut pt = |to: &mut Sketch, p: of_sketch::Id| -> of_sketch::Id {
        *map.entry(p)
            .or_insert_with(|| to.add_point(from.point(p).unwrap()))
    };
    ids.iter()
        .map(|id| {
            let geo = match from.entities[id].geo.clone() {
                Geometry::Line { a, b } => Geometry::Line {
                    a: pt(to, a),
                    b: pt(to, b),
                },
                Geometry::Arc { center, start, end } => Geometry::Arc {
                    center: pt(to, center),
                    start: pt(to, start),
                    end: pt(to, end),
                },
                Geometry::Circle { center, radius } => Geometry::Circle {
                    center: pt(to, center),
                    radius,
                },
                Geometry::Bezier { points } => Geometry::Bezier {
                    points: points.map(|p| pt(to, p)),
                },
                Geometry::Spline { points, closed } => Geometry::Spline {
                    points: points.iter().map(|p| pt(to, *p)).collect(),
                    closed,
                },
                g => g,
            };
            to.add_entity(of_sketch::Entity::new(geo)).unwrap()
        })
        .collect()
}

/// Name faces swept from a profile: sides after their sketch curves, plus caps.
fn sweep_names<'a>(
    fid: FeatureId,
    prefix: &str,
    edge_names: &'a [String],
) -> impl FnMut(&HistoryEntry) -> Option<String> + 'a {
    let prefix = prefix.to_string();
    move |h: &HistoryEntry| match (h.rel, h.from_kind) {
        (Relation::Generated, Kind::Edge) => Some(format!(
            "{fid}.{prefix}{}",
            edge_names
                .get(h.from as usize)
                .map(|s| s.as_str())
                .unwrap_or("e")
        )),
        (Relation::StartCap, _) => Some(format!("{fid}.{prefix}start")),
        (Relation::EndCap, _) => Some(format!("{fid}.{prefix}end")),
        _ => None,
    }
}

/// A length that reaches past everything in the model from this plane.
fn through_all_length(state: &State, plane: &Plane) -> f64 {
    match state.body.as_ref().and_then(|b| b.shape.bbox()) {
        Some(b) => b.diagonal() * 2.0 + (b.center() - plane.origin).length() * 2.0 + 10.0,
        None => 1000.0,
    }
}

/// A large square face on a plane, centered near the model.
fn big_face(state: &State, plane: &Plane) -> RResult<Shape> {
    let size = through_all_length(state, plane);
    let center = match state.body.as_ref().and_then(|b| b.shape.bbox()) {
        Some(b) => plane.to_local(b.center()),
        None => DVec2::ZERO,
    };
    let c = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(x, y)| center + DVec2::new(x, y) * size);
    let outer = (0..4)
        .map(|i| GCurve::Line {
            a: c[i],
            b: c[(i + 1) % 4],
        })
        .collect();
    Profile {
        plane: *plane,
        faces: vec![ProfileFace {
            outer,
            holes: vec![],
        }],
    }
    .to_shape()
    .map_err(geo)
}

fn translate(s: &Shape, v: DVec3) -> RResult<Shape> {
    Ok(s.transform(&DAffine3::from_translation(v))
        .map_err(geo)?
        .shape)
}

/// Sweep a profile along `dir` to an end condition.
fn extrude_side(
    fid: FeatureId,
    prof: &ProfileShape,
    dir: DVec3,
    cond: &EndCondition,
    prefix: &str,
    prev: &State,
) -> RResult<(Shape, Naming)> {
    let plane = prof.plane;
    let named = |r: of_geom::OpResult| {
        let mut generate = sweep_names(fid, prefix, &prof.edge_names);
        let names = Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [None, None],
            },
            &mut generate,
            &fid.to_string(),
        );
        (r.shape, names)
    };
    let blind = |depth: f64, from: &Shape| -> RResult<(Shape, Naming)> {
        if depth <= 1e-9 {
            return Err("the depth must be more than zero".into());
        }
        Ok(named(from.prism(dir * depth).map_err(geo)?))
    };
    let long = through_all_length(prev, &plane);
    match cond {
        EndCondition::Blind { depth } => blind(*depth, &prof.shape),
        EndCondition::MidPlane { depth } => {
            let start = translate(&prof.shape, -dir * (depth / 2.0))?;
            blind(*depth, &start)
        }
        EndCondition::ThroughAll => blind(long, &prof.shape),
        EndCondition::UpToVertex { target } => {
            let p = resolve(prev, target)?.point()?;
            let d = (p - plane.origin).dot(dir);
            if d <= 1e-9 {
                return Err("the vertex is behind the sketch plane".into());
            }
            blind(d, &prof.shape)
        }
        EndCondition::UpToSurface { target } | EndCondition::OffsetFromSurface { target, .. } => {
            let offset = match cond {
                EndCondition::OffsetFromSurface {
                    offset,
                    reverse_offset,
                    ..
                } => {
                    if *reverse_offset {
                        -offset
                    } else {
                        *offset
                    }
                }
                _ => 0.0,
            };
            let res = resolve(prev, target)?;
            match res.plane() {
                Ok(tp) => {
                    // The target plane, moved back toward the sketch by the offset.
                    let toward = if tp.normal.dot(dir) > 0.0 {
                        -tp.normal
                    } else {
                        tp.normal
                    };
                    let tp = Plane::new(tp.origin + toward * offset, tp.x_dir, tp.normal);
                    if tp.normal.cross(dir).length() < 1e-9 {
                        let d = (tp.origin - plane.origin).dot(dir);
                        return blind(d, &prof.shape);
                    }
                    let (s, n) = blind(long, &prof.shape)?;
                    let face = big_face(prev, &tp)?;
                    clip_by_face(fid, prefix, &s, &n, &face, &prof.shape)
                }
                Err(_) => {
                    let Resolved::Face { index, .. } = res else {
                        return Err("select a face, plane or surface to extrude up to".into());
                    };
                    let mut face = prev
                        .body
                        .as_ref()
                        .unwrap()
                        .shape
                        .sub_shape(Kind::Face, index)
                        .map_err(geo)?;
                    if offset != 0.0 {
                        face = face
                            .offset_face(offset, prof.shape.face_info(0).map_err(geo)?.centroid)
                            .map_err(geo)?;
                    }
                    let (s, n) = blind(long, &prof.shape)?;
                    clip_by_face(fid, prefix, &s, &n, &face, &prof.shape)
                }
            }
        }
        EndCondition::UpToNext => {
            let body = prev
                .body
                .as_ref()
                .ok_or("there's no body to extrude up to")?;
            let (s, n) = blind(long, &prof.shape)?;
            // Pieces of the sweep that are outside the body (for a boss) or
            // inside it (for a cut) and touch the sketch.
            let outside = s.boolean(BoolOp::Cut, &body.shape).map_err(geo)?;
            keep_touching(fid, prefix, outside, &n, &body.names, &prof.shape)
        }
    }
}

/// Keep only the solids of an operation's result that touch `start`.
fn keep_touching(
    fid: FeatureId,
    prefix: &str,
    r: of_geom::OpResult,
    names: &Naming,
    body_names: &Naming,
    start: &Shape,
) -> RResult<(Shape, Naming)> {
    let mut generate = |_: &HistoryEntry| Some(format!("{fid}.{prefix}end"));
    let all = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(names), Some(body_names)],
        },
        &mut generate,
        &fid.to_string(),
    );
    let solids = r.shape.solids();
    let (keep, drop): (Vec<&Shape>, Vec<&Shape>) = solids
        .iter()
        .partition(|s| s.distance(start).map(|d| d.value < 1e-6).unwrap_or(false));
    if keep.is_empty() {
        return Err("nothing to extrude up to".into());
    }
    if drop.is_empty() {
        return Ok((r.shape, all));
    }
    let drop = Shape::compound(drop);
    let r2 = r.shape.boolean(BoolOp::Cut, &drop).map_err(geo)?;
    let n2 = Naming::from_history(
        &r2.shape,
        &r2.history,
        Inputs {
            named: [Some(&all), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    Ok((r2.shape, n2))
}

/// Cut a long sweep off at a face, keeping the side with the sketch.
fn clip_by_face(
    fid: FeatureId,
    prefix: &str,
    sweep: &Shape,
    names: &Naming,
    face: &Shape,
    start: &Shape,
) -> RResult<(Shape, Naming)> {
    let inside = start.face_info(0).map_err(geo)?.centroid;
    let half = Shape::half_space(face, inside).map_err(geo)?;
    let r = sweep.boolean(BoolOp::Common, &half).map_err(geo)?;
    if r.shape.count(Kind::Solid) == 0 {
        return Err("the extrude doesn't reach the surface".into());
    }
    // A curved surface can intersect the sweep again on its far side.
    // Keep only material connected to the starting profile.
    keep_touching(fid, prefix, r, names, &Naming::default(), start)
}

/// Combine a tool with the body and remember it for patterns.
fn apply_tool(
    fid: FeatureId,
    state: &mut State,
    tool: Shape,
    names: Naming,
    op: BoolOp,
    merge: bool,
) -> RResult<()> {
    let body = match (&state.body, op) {
        (None, BoolOp::Cut | BoolOp::Common) => return Err("there's no material to cut".into()),
        (None, BoolOp::Fuse) => Body {
            shape: tool.clone(),
            names: names.clone(),
        },
        (Some(b), BoolOp::Fuse) if !merge => {
            let shape = Shape::compound([&b.shape, &tool]);
            let mut faces = b.names.faces.clone();
            faces.extend(names.faces.iter().cloned());
            let names = Naming::from_faces(&shape, faces);
            Body { shape, names }
        }
        (Some(b), op) => {
            let r = b.shape.boolean(op, &tool).map_err(geo)?;
            if r.shape.count(Kind::Solid) == 0 {
                return Err(if op == BoolOp::Cut {
                    "the cut removes the whole body"
                } else {
                    "the result is empty"
                }
                .into());
            }
            let names = Naming::from_history(
                &r.shape,
                &r.history,
                Inputs {
                    named: [Some(&b.names), Some(&names)],
                },
                &mut |_| None,
                &fid.to_string(),
            );
            Body {
                shape: r.shape,
                names,
            }
        }
    };
    state.body = Some(body);
    state.tools.entry(fid).or_default().push(Tool {
        shape: tool,
        names,
        op,
    });
    Ok(())
}

fn extrude(fid: FeatureId, e: &Extrude, prev: &State, state: &mut State) -> RResult<()> {
    let mut prof = sketch_profile(prev, e.sketch, e.thin.as_ref())?;
    if e.start_offset != 0.0 {
        prof.shape = translate(&prof.shape, prof.plane.normal * e.start_offset)?;
        prof.plane = prof.plane.offset(e.start_offset);
    }
    let dir = if e.reverse {
        -prof.plane.normal
    } else {
        prof.plane.normal
    };
    let (mut tool, mut names) = extrude_side(fid, &prof, dir, &e.end1, "", prev)?;
    if let Some(end2) = &e.end2 {
        if matches!(e.end1, EndCondition::MidPlane { .. }) {
            return Err("mid plane can't be combined with a second direction".into());
        }
        let (t2, n2) = extrude_side(fid, &prof, -dir, end2, "d2.", prev)?;
        let r = tool.boolean(BoolOp::Fuse, &t2).map_err(geo)?;
        names = Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&names), Some(&n2)],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        tool = r.shape;
    }
    if e.cut && e.flip_side_to_cut {
        let slab_face = big_face(prev, &prof.plane)?;
        let slab_prof = ProfileShape {
            shape: slab_face,
            plane: prof.plane,
            edge_names: vec!["slab".into(); 4],
        };
        let (mut slab, mut slab_names) = extrude_side(fid, &slab_prof, dir, &e.end1, "", prev)?;
        if let Some(end2) = &e.end2 {
            let (t2, n2) = extrude_side(fid, &slab_prof, -dir, end2, "d2.", prev)?;
            let r = slab.boolean(BoolOp::Fuse, &t2).map_err(geo)?;
            slab_names = Naming::from_history(
                &r.shape,
                &r.history,
                Inputs {
                    named: [Some(&slab_names), Some(&n2)],
                },
                &mut |_| None,
                &fid.to_string(),
            );
            slab = r.shape;
        }
        let r = slab.boolean(BoolOp::Cut, &tool).map_err(geo)?;
        names = Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&slab_names), Some(&names)],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        tool = r.shape;
    }
    let op = if e.cut { BoolOp::Cut } else { BoolOp::Fuse };
    apply_tool(fid, state, tool, names, op, e.merge)
}

fn rotation_about(origin: DVec3, dir: DVec3, angle: f64) -> DAffine3 {
    DAffine3::from_translation(origin)
        * DAffine3::from_axis_angle(unit(dir), angle)
        * DAffine3::from_translation(-origin)
}

fn revolve(fid: FeatureId, r: &Revolve, prev: &State, state: &mut State) -> RResult<()> {
    let (mut shape, mut names) = revolve_side(fid, r, prev, "")?;
    if let Some(end2) = &r.end2 {
        let single = |a: &RevolveAngle| {
            !matches!(
                a,
                RevolveAngle::MidPlane { .. } | RevolveAngle::TwoDirection { .. }
            )
        };
        if !single(&r.angle) || !single(end2) {
            return Err(
                "independent directions cannot use mid-plane or two-direction angles".into(),
            );
        }
        let mut opposite = r.clone();
        opposite.reverse = !r.reverse;
        opposite.angle = end2.clone();
        opposite.end2 = None;
        let (other, other_names) = revolve_side(fid, &opposite, prev, "d2.")?;
        let combined = shape.boolean(BoolOp::Fuse, &other).map_err(geo)?;
        names = Naming::from_history(
            &combined.shape,
            &combined.history,
            Inputs {
                named: [Some(&names), Some(&other_names)],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        shape = combined.shape;
    }
    let op = if r.cut { BoolOp::Cut } else { BoolOp::Fuse };
    apply_tool(fid, state, shape, names, op, r.merge)
}

fn revolve_side(
    fid: FeatureId,
    r: &Revolve,
    prev: &State,
    prefix: &str,
) -> RResult<(Shape, Naming)> {
    let prof = sketch_profile(prev, r.sketch, r.thin.as_ref())?;
    let (origin, mut dir) = resolve(prev, &r.axis)?.axis()?;
    if r.reverse {
        dir = -dir;
    }
    let (start, total) = match &r.angle {
        RevolveAngle::Blind { angle } => (0.0, *angle),
        RevolveAngle::MidPlane { angle } => (-angle / 2.0, *angle),
        RevolveAngle::TwoDirection { angle1, angle2 } => (-angle2, angle1 + angle2),
        RevolveAngle::UpToVertex { target } => {
            let radial = |p: DVec3| {
                let v = p - origin;
                v - dir * v.dot(dir)
            };
            let from = radial(prof.shape.face_info(0).map_err(geo)?.centroid);
            let to = radial(resolve(prev, target)?.point()?);
            if from.length() < 1e-9 || to.length() < 1e-9 {
                return Err("the revolve target must be away from its axis".into());
            }
            let angle = dir.dot(from.cross(to)).atan2(from.dot(to)).rem_euclid(TAU);
            (0.0, angle)
        }
        RevolveAngle::UpToSurface { .. } | RevolveAngle::OffsetFromSurface { .. } => (0.0, TAU),
    };
    if !total.is_finite() || total <= 1e-9 || total > TAU + 1e-9 {
        return Err("the revolve angle must be between 0° and 360°".into());
    }
    // The axis must not pass through the profile's interior.
    let local_dir = DVec2::new(dir.dot(prof.plane.x_dir), dir.dot(prof.plane.y_dir()));
    if dir.dot(prof.plane.normal).abs() > 1e-6
        || local_dir.length() < 1e-9
        || prof.plane.distance(origin).abs() > 1e-6
    {
        return Err("the axis must lie in the sketch plane".into());
    }
    let face = if start != 0.0 {
        prof.shape
            .transform(&rotation_about(origin, dir, start))
            .map_err(geo)?
            .shape
    } else {
        prof.shape.clone()
    };
    let until = match &r.angle {
        RevolveAngle::UpToSurface { target } | RevolveAngle::OffsetFromSurface { target, .. } => {
            let resolved = resolve(prev, target)?;
            let mut target_face = match resolved.plane() {
                Ok(p) => big_face(prev, &p)?,
                Err(_) => match resolved {
                    Resolved::Face { index, .. } => body_of(prev)?
                        .shape
                        .sub_shape(Kind::Face, index)
                        .map_err(geo)?,
                    _ => return Err("select a face or plane to revolve up to".into()),
                },
            };
            if let RevolveAngle::OffsetFromSurface {
                offset,
                reverse_offset,
                ..
            } = &r.angle
            {
                if !offset.is_finite() || *offset < 0.0 {
                    return Err("the surface offset must be finite and nonnegative".into());
                }
                if *offset > 0.0 {
                    target_face = target_face
                        .offset_face(
                            if *reverse_offset { -*offset } else { *offset },
                            prof.shape.face_info(0).map_err(geo)?.centroid,
                        )
                        .map_err(geo)?;
                }
            }
            Some(target_face)
        }
        _ => None,
    };
    let res = if let Some(until) = &until {
        face.revolve_until(until, origin, dir)
    } else {
        face.revolve(origin, dir, total.min(TAU))
    }
    .map_err(|e| {
        if e.0.contains("invalid") {
            "the profile crosses the axis of revolution".to_string()
        } else {
            e.0
        }
    })?;
    let mut sides = sweep_names(fid, prefix, &prof.edge_names);
    let mut generate = |h: &HistoryEntry| {
        if h.input == 1 && h.to_kind == Kind::Face {
            Some(format!("{fid}.{prefix}end"))
        } else {
            sides(h)
        }
    };
    let names = Naming::from_history(
        &res.shape,
        &res.history,
        Inputs {
            named: [None, None],
        },
        &mut generate,
        &fid.to_string(),
    );
    Ok((res.shape, names))
}

fn body_of(prev: &State) -> RResult<&Body> {
    prev.body
        .as_ref()
        .ok_or_else(|| "there's no body yet".to_string())
}

/// Edge indices from edge references plus all edges of face references.
fn edge_set(prev: &State, edges: &[Ref], faces: &[Ref]) -> RResult<Vec<u32>> {
    let body = body_of(prev)?;
    let mut out = Vec::new();
    for r in edges {
        match prev.index_of(r)? {
            (Kind::Edge, i) => out.push(i),
            _ => return Err("select edges".into()),
        }
    }
    if !faces.is_empty() {
        let fe = body.shape.face_edges();
        for r in faces {
            match prev.index_of(r)? {
                (Kind::Face, i) => out.extend(&fe[i as usize]),
                _ => return Err("select faces".into()),
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err("select at least one edge or face".into());
    }
    Ok(out)
}

fn dressup_names(fid: FeatureId, tag: &str, body: &Body, r: &of_geom::OpResult) -> Naming {
    let mut generate = |h: &HistoryEntry| match h.from_kind {
        Kind::Edge => body
            .names
            .edges
            .get(h.from as usize)
            .map(|e| format!("{fid}.{tag}({e})")),
        Kind::Vertex => body
            .names
            .vertices
            .get(h.from as usize)
            .map(|v| format!("{fid}.{tag}v({v})")),
        Kind::Face => body
            .names
            .face_name(h.from)
            .map(|f| format!("{fid}.{tag}o({f})")),
        Kind::Solid => None,
    };
    Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(&body.names), None],
        },
        &mut generate,
        &fid.to_string(),
    )
}

/// Store exact added/removed volumes so a dress-up can be repeated like a cut
/// or boss, including concave fillets and outward shells which add material.
fn remember_dressup(
    fid: FeatureId,
    before: &Body,
    after: &Shape,
    names: &Naming,
    state: &mut State,
) -> RResult<()> {
    let mut tools = Vec::new();
    for (a, an, b, bn, op) in [
        (&before.shape, &before.names, after, names, BoolOp::Cut),
        (after, names, &before.shape, &before.names, BoolOp::Fuse),
    ] {
        let delta = a.boolean(BoolOp::Cut, b).map_err(geo)?;
        if delta.shape.count(Kind::Solid) == 0 {
            continue;
        }
        let names = Naming::from_history(
            &delta.shape,
            &delta.history,
            Inputs {
                named: [Some(an), Some(bn)],
            },
            &mut |_| None,
            &format!("{fid}.delta"),
        );
        tools.push(Tool {
            shape: delta.shape,
            names,
            op,
        });
    }
    state.tools.insert(fid, tools);
    Ok(())
}

fn fillet(fid: FeatureId, f: &Fillet, prev: &State, state: &mut State) -> RResult<()> {
    if f.radius <= 0.0 {
        return Err("the radius must be more than zero".into());
    }
    let body = body_of(prev)?;
    let edges = edge_set(prev, &f.edges, &f.faces)?;
    let r = body.shape.fillet(&edges, f.radius).map_err(geo)?;
    let names = dressup_names(fid, "f", body, &r);
    remember_dressup(fid, body, &r.shape, &names, state)?;
    state.body = Some(Body {
        shape: r.shape,
        names,
    });
    Ok(())
}

fn chamfer(fid: FeatureId, c: &Chamfer, prev: &State, state: &mut State) -> RResult<()> {
    let (mut d1, mut d2) = match c.mode {
        ChamferMode::Equal { distance } => (distance, distance),
        ChamferMode::DistanceDistance { d1, d2 } => (d1, d2),
        ChamferMode::AngleDistance { distance, angle } => {
            if !(0.0..PI / 2.0).contains(&angle) || angle == 0.0 {
                return Err("the chamfer angle must be between 0° and 90°".into());
            }
            (distance, distance * angle.tan())
        }
    };
    if c.flip {
        std::mem::swap(&mut d1, &mut d2);
    }
    if d1 <= 0.0 || d2 <= 0.0 {
        return Err("chamfer distances must be more than zero".into());
    }
    let body = body_of(prev)?;
    let edges = edge_set(prev, &c.edges, &c.faces)?;
    let r = body.shape.chamfer(&edges, d1, d2).map_err(geo)?;
    let names = dressup_names(fid, "c", body, &r);
    remember_dressup(fid, body, &r.shape, &names, state)?;
    state.body = Some(Body {
        shape: r.shape,
        names,
    });
    Ok(())
}

fn shell(fid: FeatureId, s: &ShellDef, prev: &State, state: &mut State) -> RResult<()> {
    if s.thickness <= 0.0 {
        return Err("the thickness must be more than zero".into());
    }
    let body = body_of(prev)?;
    let mut faces = Vec::new();
    for r in &s.faces {
        match prev.index_of(r)? {
            (Kind::Face, i) => faces.push(i),
            _ => return Err("select faces to remove".into()),
        }
    }
    let t = if s.outward { -s.thickness } else { s.thickness };
    let r = body.shape.shell(&faces, t).map_err(geo)?;
    let names = dressup_names(fid, "s", body, &r);
    remember_dressup(fid, body, &r.shape, &names, state)?;
    state.body = Some(Body {
        shape: r.shape,
        names,
    });
    Ok(())
}

/// Hole cross-section dimensions after applying the standard and overrides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoleDims {
    pub diameter: f64,
    pub cbore: Option<(f64, f64)>,
    pub csink: Option<(f64, f64)>,
}

pub fn hole_dims(h: &Hole) -> RResult<HoleDims> {
    let size =
        holes::find(h.standard, &h.size).ok_or_else(|| format!("unknown hole size {}", h.size))?;
    let diameter = h.diameter_override.unwrap_or(match h.kind {
        HoleKind::Tapped => size.tap_drill,
        _ => holes::clearance(size, h.fit),
    });
    let cbore = (h.kind == HoleKind::Counterbore).then(|| {
        (
            h.cbore_diameter_override.unwrap_or(size.cbore.0),
            h.cbore_depth_override.unwrap_or(size.cbore.1),
        )
    });
    let default_angle = match h.standard {
        HoleStandard::Iso => 90f64.to_radians(),
        HoleStandard::Ansi => 82f64.to_radians(),
    };
    let csink = (h.kind == HoleKind::Countersink).then(|| {
        (
            h.csink_diameter_override.unwrap_or(size.csink),
            h.csink_angle_override.unwrap_or(default_angle),
        )
    });
    if diameter <= 0.0
        || cbore.is_some_and(|(d, _)| d <= diameter)
        || csink.is_some_and(|(d, _)| d <= diameter)
    {
        return Err("the hole's diameters don't fit together".into());
    }
    Ok(HoleDims {
        diameter,
        cbore,
        csink,
    })
}

fn hole(fid: FeatureId, h: &Hole, prev: &State, state: &mut State) -> RResult<()> {
    let (plane, sk) = sketch_of(prev, h.sketch)?;
    let points: Vec<DVec2> = sk
        .entities
        .iter()
        .filter(|(id, e)| {
            **id != of_sketch::ORIGIN
                && !e.construction
                && matches!(e.geo, Geometry::Point { .. })
                && !sk.entities.values().any(|o| o.point_refs().contains(id))
        })
        .map(|(id, _)| sk.point(*id).unwrap())
        .collect();
    if points.is_empty() {
        return Err("add sketch points where the holes go".into());
    }
    let dims = hole_dims(h)?;
    let depth = match h.end {
        HoleEnd::Blind { depth } if depth > 0.0 => depth,
        HoleEnd::Blind { .. } => return Err("the hole depth must be more than zero".into()),
        HoleEnd::ThroughAll => through_all_length(prev, plane),
    };
    // Drill into the face: against the sketch normal.
    let out = if h.reverse {
        -plane.normal
    } else {
        plane.normal
    };
    let r = dims.diameter / 2.0;
    let mut pts: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    if let Some((cd, cdepth)) = dims.cbore {
        pts.extend([(cd / 2.0, 0.0), (cd / 2.0, -cdepth), (r, -cdepth)]);
    } else if let Some((sd, angle)) = dims.csink {
        let sr = sd / 2.0;
        pts.extend([(sr, 0.0), (r, -(sr - r) / (angle / 2.0).tan())]);
    } else {
        pts.push((r, 0.0));
    }
    pts.push((r, -depth));
    if matches!(h.end, HoleEnd::Blind { .. }) {
        // 118° drill point.
        pts.push((0.0, -depth - r / 59f64.to_radians().tan()));
    } else {
        pts.push((0.0, -depth));
    }
    let n = pts.len();
    let curves: Vec<GCurve> = (0..n)
        .map(|i| GCurve::Line {
            a: DVec2::new(pts[i].0, pts[i].1),
            b: DVec2::new(pts[(i + 1) % n].0, pts[(i + 1) % n].1),
        })
        .collect();
    let mut tools = Vec::new();
    let mut all_faces: Vec<Vec<String>> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let origin = plane.to_world(*p);
        let section_plane = Plane::new(origin, plane.x_dir, plane.x_dir.cross(out));
        let face = Profile {
            plane: section_plane,
            faces: vec![ProfileFace {
                outer: curves.clone(),
                holes: vec![],
            }],
        }
        .to_shape()
        .map_err(geo)?;
        let edge_names: Vec<String> = (0..face.count(Kind::Edge))
            .map(|e| {
                let mid = face
                    .edge_info(e)
                    .map(|x| section_plane.to_local(x.mid))
                    .unwrap_or_default();
                let k = curves
                    .iter()
                    .position(|c| match c {
                        GCurve::Line { a, b } => ((*a + *b) * 0.5 - mid).length() < 1e-6,
                        _ => false,
                    })
                    .unwrap_or(99);
                format!("p{i}.h{k}")
            })
            .collect();
        let res = face.revolve(origin, out, TAU).map_err(geo)?;
        let mut generate = sweep_names(fid, "", &edge_names);
        let names = Naming::from_history(
            &res.shape,
            &res.history,
            Inputs {
                named: [None, None],
            },
            &mut generate,
            &format!("{fid}.p{i}"),
        );
        all_faces.extend(names.faces.iter().cloned());
        tools.push(res.shape);
    }
    let tool = Shape::compound(tools.iter());
    let names = Naming::from_faces(&tool, all_faces);
    apply_tool(fid, state, tool, names, BoolOp::Cut, true)
}

/// Replay selections in feature-tree order, regardless of click order.
fn ordered_seeds<'a>(prev: &'a State, selected: &[FeatureId]) -> RResult<Vec<&'a FeatureId>> {
    if selected.iter().any(|id| !prev.tools.contains_key(id)) {
        return Err("select solid features that rebuilt successfully".into());
    }
    Ok(prev
        .tool_order
        .iter()
        .filter(|id| selected.contains(id))
        .collect())
}

/// Repeat the tools of `features` at each transform.
fn repeat(
    fid: FeatureId,
    features: &[FeatureId],
    transforms: &[DAffine3],
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    if features.is_empty() {
        return Err("select features to pattern".into());
    }
    let mut seeds: Vec<Tool> = Vec::new();
    for f in ordered_seeds(prev, features)? {
        let tools = prev.tools.get(f).ok_or_else(|| {
            "select a solid feature that rebuilt successfully to pattern".to_string()
        })?;
        seeds.extend(tools.iter().cloned());
    }
    for (k, t) in transforms.iter().enumerate() {
        for (j, seed) in seeds.iter().enumerate() {
            let moved = seed.shape.transform(t).map_err(geo)?;
            let names =
                seed.names
                    .prefixed(&moved.shape, &moved.history, &format!("{fid}.i{k}.{j}."));
            apply_tool(fid, state, moved.shape, names, seed.op, true)?;
        }
    }
    Ok(())
}

fn linear_pattern(
    fid: FeatureId,
    p: &LinearPattern,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let dir_of = |d: &crate::PatternDirection| -> RResult<DVec3> {
        let v = resolve(prev, &d.reference)?.direction()?;
        if d.count < 1 {
            return Err("the instance count must be at least 1".into());
        }
        Ok(if d.reverse { -v } else { v })
    };
    let d1 = dir_of(&p.direction1)?;
    let (d2, s2, n2) = match &p.direction2 {
        Some(d) => (dir_of(d)?, d.spacing, d.count),
        None => (DVec3::ZERO, 0.0, 1),
    };
    let mut ts = Vec::new();
    for i in 0..p.direction1.count {
        for j in 0..n2 {
            if i == 0 && j == 0 {
                continue;
            }
            let v = d1 * (p.direction1.spacing * i as f64) + d2 * (s2 * j as f64);
            ts.push(DAffine3::from_translation(v));
        }
    }
    if ts.is_empty() {
        return Err("the pattern needs more than one instance".into());
    }
    repeat(fid, &p.features, &ts, prev, state)
}

fn circular_pattern(
    fid: FeatureId,
    p: &CircularPattern,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (origin, mut dir) = resolve(prev, &p.axis)?.axis()?;
    if p.reverse {
        dir = -dir;
    }
    if p.count < 2 {
        return Err("the pattern needs at least 2 instances".into());
    }
    let step = if p.equal_spacing {
        if (p.angle - TAU).abs() < 1e-9 {
            p.angle / p.count as f64
        } else {
            p.angle / (p.count - 1) as f64
        }
    } else {
        p.angle
    };
    let ts: Vec<DAffine3> = (1..p.count)
        .map(|k| rotation_about(origin, dir, step * k as f64))
        .collect();
    repeat(fid, &p.features, &ts, prev, state)
}

fn mirror(fid: FeatureId, m: &MirrorDef, prev: &State, state: &mut State) -> RResult<()> {
    let plane = resolve(prev, &m.plane)?.plane()?;
    if m.features.is_empty() {
        let body = body_of(prev)?;
        let r = body.shape.mirror(plane.origin, plane.normal).map_err(geo)?;
        let names = body
            .names
            .prefixed(&r.shape, &r.history, &format!("{fid}.m."));
        return apply_tool(fid, state, r.shape, names, BoolOp::Fuse, true);
    }
    for f in ordered_seeds(prev, &m.features)? {
        let tools = prev
            .tools
            .get(f)
            .ok_or_else(|| {
                "select a solid feature that rebuilt successfully to mirror".to_string()
            })?
            .clone();
        for (j, seed) in tools.iter().enumerate() {
            let r = seed.shape.mirror(plane.origin, plane.normal).map_err(geo)?;
            let names = seed
                .names
                .prefixed(&r.shape, &r.history, &format!("{fid}.m{f}.{j}."));
            apply_tool(fid, state, r.shape, names, seed.op, true)?;
        }
    }
    Ok(())
}

fn import(fid: FeatureId, i: &Import, state: &mut State) -> RResult<()> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&i.data)
        .map_err(|_| "the imported data is corrupt".to_string())?;
    let ext = match i.format {
        ImportFormat::Step => "step",
        ImportFormat::Iges => "igs",
    };
    let path = std::env::temp_dir().join(format!(
        "openforge-import-{}-{fid}.{ext}",
        std::process::id()
    ));
    std::fs::write(&path, bytes).map_err(geo)?;
    let p = path.to_string_lossy().to_string();
    let shape = match i.format {
        ImportFormat::Step => Shape::read_step(&p),
        ImportFormat::Iges => Shape::read_iges(&p),
    };
    let _ = std::fs::remove_file(&path);
    let shape = shape.map_err(geo)?;
    let names = Naming::by_index(&shape, &fid.to_string());
    apply_tool(fid, state, shape, names, BoolOp::Fuse, false)
}

/// Sketch geometry for model edges projected onto a sketch plane, for
/// Convert Entities. Returns the new entity ids.
pub fn convert_edges(
    state: &State,
    plane: &Plane,
    sketch: &mut Sketch,
    edges: &[Ref],
) -> RResult<Vec<of_sketch::Id>> {
    let body = body_of(state)?;
    let mut out = Vec::new();
    for r in edges {
        let Ok(Resolved::Edge { index, info }) = resolve(state, r) else {
            return Err("select model edges to convert".into());
        };
        let name = match r {
            Ref::Edge { name } => name.clone(),
            Ref::BodyTopology {
                kind: Kind::Edge, ..
            } => serde_json::to_string(r).map_err(geo)?,
            _ => return Err("select an edge to convert".into()),
        };
        let local = |p: DVec3| plane.to_local(p);
        let flat = info.axis.cross(plane.normal).length() < 1e-6;
        let g = match info.kind {
            of_geom::CurveKind::Line => {
                let a = sketch.add_point(local(info.start));
                let b = sketch.add_point(local(info.end));
                Geometry::Line { a, b }
            }
            of_geom::CurveKind::Circle if flat && info.closed => Geometry::Circle {
                center: sketch.add_point(local(info.center)),
                radius: info.radius,
            },
            of_geom::CurveKind::Circle if flat => {
                let (s, e) = if info.axis.dot(plane.normal) >= 0.0 {
                    (info.start, info.end)
                } else {
                    (info.end, info.start)
                };
                let c = sketch.add_point(local(info.center));
                let s = sketch.add_point(local(s));
                let e = sketch.add_point(local(e));
                Geometry::Arc {
                    center: c,
                    start: s,
                    end: e,
                }
            }
            _ => {
                let pts = body.shape.edge_samples(index, 0.05, 0.2).map_err(geo)?;
                let step = (pts.len() / 12).max(1);
                let mut ids: Vec<of_sketch::Id> = pts
                    .iter()
                    .step_by(step)
                    .map(|p| sketch.add_point(local(*p)))
                    .collect();
                if ids.len() < 2 {
                    ids.push(sketch.add_point(local(*pts.last().unwrap())));
                }
                Geometry::Spline {
                    points: ids,
                    closed: info.closed,
                }
            }
        };
        // Splines are converted once; lines and circles follow the edge on rebuild.
        let external = (!matches!(g, Geometry::Spline { .. })).then(|| name.clone());
        let id = sketch
            .add_entity(of_sketch::Entity {
                geo: g,
                construction: false,
                external,
            })
            .map_err(geo)?;
        out.push(id);
    }
    Ok(out)
}
