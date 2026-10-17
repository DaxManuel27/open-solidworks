//! Surface features: building surface bodies, knitting them into solids,
//! thickening them, cutting solids with them, and deleting faces.

use of_geom::{BoolOp, Continuity, DVec3, FillEdge, HistoryEntry, Kind, Relation, Shape};

use super::{ProfileShape, closed_regions, extrude_side, geo, sketch_of, to_geom, translate};
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::resolve::{RResult, Resolved, resolve};
use crate::surfaces::{
    BoundarySurface, CutWithSurface, DeleteFace, DeleteFaceMode, ExtrudedSurface, Knit,
    LoftedSurface, OffsetSurface, PlanarSurface, RevolvedSurface, SweptSurface, Thicken,
    ThickenSide,
};
use crate::{BodyId, EndCondition, FeatureId, RevolveAngle, SolidBody};

/// Every contour of a sketch, open and closed, as wires, with each kernel
/// edge's sketch name.
fn sketch_wires(prev: &State, sketch: FeatureId) -> RResult<ProfileShape> {
    let (plane, sk) = sketch_of(prev, sketch)?;
    let contours = sk.contours().map_err(geo)?;
    let chains: Vec<_> = contours.loops.iter().chain(&contours.open).collect();
    if chains.is_empty() {
        return Err("the sketch has no curves".into());
    }
    let wires = chains
        .iter()
        .map(|c| Shape::wire(plane, &c.curves.iter().map(to_geom).collect::<Vec<_>>()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(geo)?;
    let shape = Shape::compound(&wires);
    let candidates: Vec<(of_sketch::Id, of_sketch::Curve2)> = chains
        .iter()
        .flat_map(|c| c.entities.iter().copied().zip(c.curves.iter().cloned()))
        .collect();
    let edge_names = (0..shape.count(Kind::Edge))
        .map(|i| {
            let Ok(info) = shape.edge_info(i) else {
                return "e".to_string();
            };
            let mid = plane.to_local(info.mid);
            candidates
                .iter()
                .min_by(|a, b| {
                    let da = (a.1.closest_point(mid) - mid).length();
                    let db = (b.1.closest_point(mid) - mid).length();
                    da.total_cmp(&db)
                })
                .map_or("e".to_string(), |(id, _)| format!("s{id}"))
        })
        .collect();
    Ok(ProfileShape {
        shape,
        plane: *plane,
        edge_names,
    })
}

/// Name faces from a sweep-like history: generated from profile edge `i` is
/// `{fid}.{names[i]}`, and the caps are `{fid}.start` and `{fid}.end`.
fn swept_names(
    fid: FeatureId,
    shape: &Shape,
    history: &[HistoryEntry],
    edges: &[String],
) -> Naming {
    Naming::from_history(
        shape,
        history,
        Inputs {
            named: [None, None],
        },
        &mut |h: &HistoryEntry| match (h.rel, h.from_kind) {
            (Relation::Generated, Kind::Edge) => Some(format!(
                "{fid}.{}",
                edges.get(h.from as usize).map_or("e", |s| s.as_str())
            )),
            (Relation::Generated, Kind::Face) => Some(format!("{fid}.p{}", h.from)),
            (Relation::StartCap, _) => Some(format!("{fid}.start")),
            (Relation::EndCap, _) => Some(format!("{fid}.end")),
            _ => None,
        },
        &fid.to_string(),
    )
}

/// Split a named shape into its shells, each with its faces' names.
fn shells(body: &Body) -> RResult<Vec<Body>> {
    body.shape
        .shells()
        .into_iter()
        .map(|shape| {
            let indices = shape.indices_in(&body.shape, Kind::Face).map_err(geo)?;
            let faces = indices
                .iter()
                .map(|i| body.names.faces[*i as usize].clone())
                .collect();
            let names = Naming::from_faces(&shape, faces);
            Ok(Body { shape, names })
        })
        .collect()
}

/// Add each shell of `body` as a surface body made by `fid`.
pub(crate) fn add_surfaces(fid: FeatureId, state: &mut State, body: Body) -> RResult<()> {
    let parts = shells(&body)?;
    if parts.is_empty() {
        return Err("the feature made no surface".into());
    }
    for (i, geometry) in parts.into_iter().enumerate() {
        state.surfaces.push(SolidBody {
            id: BodyId(format!("b{fid}:s{i}")),
            geometry,
        });
    }
    Ok(())
}

fn surface_side(
    fid: FeatureId,
    prof: &ProfileShape,
    dir: DVec3,
    cond: &EndCondition,
    prefix: &str,
    prev: &State,
) -> RResult<(Shape, crate::naming::Naming, Option<f64>)> {
    match cond {
        EndCondition::Blind { depth } | EndCondition::MidPlane { depth } => {
            let (s, n) = extrude_side(fid, prof, dir, cond, prefix, prev)?;
            Ok((s, n, Some(*depth)))
        }
        EndCondition::UpToVertex { target } => {
            let p = resolve(prev, target)?.point()?;
            let d = (p - prof.plane.origin).dot(dir);
            let (s, n) = extrude_side(fid, prof, dir, cond, prefix, prev)?;
            Ok((s, n, Some(d)))
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
            if let Ok(tp) = res.plane()
                && tp.normal.cross(dir).length() < 1e-9
            {
                let toward = if tp.normal.dot(dir) > 0.0 {
                    -tp.normal
                } else {
                    tp.normal
                };
                let d = (tp.origin + toward * offset - prof.plane.origin).dot(dir);
                let blind = EndCondition::Blind { depth: d };
                let (s, n) = extrude_side(fid, prof, dir, &blind, prefix, prev)?;
                return Ok((s, n, Some(d)));
            }
            let face = match res {
                Resolved::Face { index, .. } => prev.face_shape(index)?,
                Resolved::Plane(p) => Shape::plane_face(&p).map_err(geo)?,
                _ => return Err("select a face, plane or surface to extrude up to".into()),
            };
            let face = if offset != 0.0 {
                face.offset_face(offset, prof.plane.origin).map_err(geo)?
            } else {
                face
            };
            let long = super::through_all_length(prev, &prof.plane);
            let blind = EndCondition::Blind { depth: long };
            let (s, n) = extrude_side(fid, prof, dir, &blind, prefix, prev)?;
            // Keep the faces between the sketch and the target.
            let split = s.split_tools(&face).map_err(geo)?;
            let names = Naming::from_history(
                &split.shape,
                &split.history,
                Inputs {
                    named: [Some(&n), None],
                },
                &mut |_| None,
                &fid.to_string(),
            );
            let keep: Vec<u32> = (0..split.shape.count(Kind::Face))
                .filter(|f| {
                    split
                        .shape
                        .sub_shape(Kind::Face, *f)
                        .and_then(|face| face.distance(&prof.shape))
                        .is_ok_and(|d| d.value < 1e-6)
                })
                .collect();
            if keep.len() == split.shape.count(Kind::Face) as usize {
                return Err("the surface doesn't reach the target".into());
            }
            let drop: Vec<u32> = (0..split.shape.count(Kind::Face))
                .filter(|f| !keep.contains(f))
                .collect();
            let r = split.shape.remove_faces(&drop).map_err(geo)?;
            let names = Naming::from_history(
                &r.shape,
                &r.history,
                Inputs {
                    named: [Some(&names), None],
                },
                &mut |_| None,
                &fid.to_string(),
            );
            Ok((r.shape, names, None))
        }
        EndCondition::ThroughAll | EndCondition::UpToNext => {
            Err("extruded surfaces end blind, mid plane, at a vertex or at a surface".into())
        }
    }
}

pub(crate) fn extruded_surface(
    fid: FeatureId,
    p: &ExtrudedSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let prof = sketch_wires(prev, p.sketch)?;
    let dir = if p.reverse {
        -prof.plane.normal
    } else {
        prof.plane.normal
    };
    let (mut shape, mut names, depth1) = surface_side(fid, &prof, dir, &p.end1, "", prev)?;
    let mut depth2 = Some(0.0);
    if let Some(end2) = &p.end2 {
        if matches!(p.end1, EndCondition::MidPlane { .. }) {
            return Err("mid plane can't be combined with a second direction".into());
        }
        let (s2, n2, d2) = surface_side(fid, &prof, -dir, end2, "d2.", prev)?;
        depth2 = d2;
        let both = Shape::compound([&shape, &s2]).sew(1e-6).map_err(geo)?;
        let mut faces = names.faces.clone();
        faces.extend(n2.faces.iter().cloned());
        let joined = Naming::from_faces(&Shape::compound([&shape, &s2]), faces);
        names = Naming::from_history(
            &both.result.shape,
            &both.result.history,
            Inputs {
                named: [Some(&joined), None],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        shape = both.result.shape;
    }
    if p.cap_ends {
        let (Some(d1), Some(d2)) = (depth1, depth2) else {
            return Err("ends can only be capped when both ends are flat".into());
        };
        let (plane, sk) = sketch_of(prev, p.sketch)?;
        let regions = closed_regions(&sk.contours().map_err(geo)?)
            .map_err(|_| "capping the ends needs closed contours".to_string())?;
        let face = of_geom::Profile {
            plane: *plane,
            faces: regions,
        }
        .to_shape()
        .map_err(geo)?;
        let (start, end) = match p.end1 {
            EndCondition::MidPlane { depth } => (-dir * (depth / 2.0), dir * (depth / 2.0)),
            _ => (-dir * d2, dir * d1),
        };
        let start = translate(&face, start)?;
        let end = translate(&face, end)?;
        let mut faces = names.faces.clone();
        faces.extend((0..start.count(Kind::Face)).map(|_| vec![format!("{fid}.start")]));
        faces.extend((0..end.count(Kind::Face)).map(|_| vec![format!("{fid}.end")]));
        let loose = Shape::compound([&shape, &start, &end]);
        let loose_names = Naming::from_faces(&loose, faces);
        let sewn = loose.sew(1e-6).map_err(geo)?;
        names = Naming::from_history(
            &sewn.result.shape,
            &sewn.result.history,
            Inputs {
                named: [Some(&loose_names), None],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        shape = sewn.result.shape;
    }
    add_surfaces(fid, state, Body { shape, names })
}

pub(crate) fn revolved_surface(
    fid: FeatureId,
    p: &RevolvedSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let prof = sketch_wires(prev, p.sketch)?;
    let (origin, mut dir) = resolve(prev, &p.axis)?.axis()?;
    if p.reverse {
        dir = -dir;
    }
    let (start, angle) = match p.angle {
        RevolveAngle::Blind { angle } => (0.0, angle),
        RevolveAngle::MidPlane { angle } => (-angle / 2.0, angle),
        RevolveAngle::TwoDirection { angle1, angle2 } => (-angle2, angle1 + angle2),
        _ => return Err("revolved surfaces turn a set angle, mid plane or two ways".into()),
    };
    if !(angle > 1e-9 && angle <= std::f64::consts::TAU + 1e-9) {
        return Err("the angle must be more than zero and at most a full turn".into());
    }
    let from = if start != 0.0 {
        prof.shape
            .transform(&super::rotation_about(origin, dir, start))
            .map_err(geo)?
            .shape
    } else {
        prof.shape.clone()
    };
    let r = from.revolve(origin, dir, angle).map_err(geo)?;
    let names = swept_names(fid, &r.shape, &r.history, &prof.edge_names);
    add_surfaces(
        fid,
        state,
        Body {
            shape: r.shape,
            names,
        },
    )
}

/// The wire of a profile or path reference, with one name per kernel edge.
fn path_wire(prev: &State, r: &crate::Ref, what: &str) -> RResult<(Shape, Vec<String>)> {
    let wire = crate::curves::path(prev, r).map_err(|e| format!("the {what}: {e}"))?;
    let names = (0..wire.count(Kind::Edge))
        .map(|i| format!("e{i}"))
        .collect();
    Ok((wire, names))
}

pub(crate) fn swept_surface(
    fid: FeatureId,
    p: &SweptSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (profile, edges) = path_wire(prev, &p.profile, "profile")?;
    let (path, _) = path_wire(prev, &p.path, "path")?;
    let start = path.path_start().map_err(geo)?;
    let mode = match p.orientation {
        crate::SweepOrientation::MinimumTwist => 0,
        crate::SweepOrientation::Frenet => 1,
        crate::SweepOrientation::Fixed => 2,
    };
    let r = Shape::sweep_surface(&profile, &path, &start, mode).map_err(geo)?;
    let names = swept_names(fid, &r.shape, &r.history, &edges);
    add_surfaces(
        fid,
        state,
        Body {
            shape: r.shape,
            names,
        },
    )
}

pub(crate) fn lofted_surface(
    fid: FeatureId,
    p: &LoftedSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    if p.profiles.len() < 2 {
        return Err("select at least two profiles".into());
    }
    let wires = p
        .profiles
        .iter()
        .map(|r| crate::curves::path(prev, r))
        .collect::<RResult<Vec<_>>>()?;
    let r = Shape::loft_surface(&Shape::compound(&wires), p.ruled, false).map_err(geo)?;
    let edges: Vec<String> = (0..r.shape.count(Kind::Edge))
        .map(|i| format!("e{i}"))
        .collect();
    let names = swept_names(fid, &r.shape, &r.history, &edges);
    add_surfaces(
        fid,
        state,
        Body {
            shape: r.shape,
            names,
        },
    )
}

pub(crate) fn boundary_surface(
    fid: FeatureId,
    p: &BoundarySurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let wires = |refs: &[crate::Ref]| {
        refs.iter()
            .map(|r| crate::curves::path(prev, r))
            .collect::<RResult<Vec<_>>>()
    };
    let one = wires(&p.curves1)?;
    if one.len() < 2 {
        return Err("select at least two curves in direction 1".into());
    }
    let r = if p.curves2.is_empty() {
        Shape::loft_surface(&Shape::compound(&one), false, false).map_err(geo)?
    } else {
        let two = wires(&p.curves2)?;
        Shape::boundary_surface(&Shape::compound(&one), &Shape::compound(&two)).map_err(geo)?
    };
    let names = swept_names(fid, &r.shape, &r.history, &[]);
    add_surfaces(
        fid,
        state,
        Body {
            shape: r.shape,
            names,
        },
    )
}

pub(crate) fn planar_surface(
    fid: FeatureId,
    p: &PlanarSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let shape = match (p.sketch, p.edges.is_empty()) {
        (Some(sketch), true) => {
            let (plane, sk) = sketch_of(prev, sketch)?;
            of_geom::Profile {
                plane: *plane,
                faces: closed_regions(&sk.contours().map_err(geo)?)?,
            }
            .to_shape()
            .map_err(geo)?
        }
        (None, false) => {
            let edges = p
                .edges
                .iter()
                .map(|r| match resolve(prev, r)? {
                    Resolved::Edge { index, .. } => prev
                        .everything()
                        .ok_or("there's no body")?
                        .shape
                        .sub_shape(Kind::Edge, index)
                        .map_err(geo),
                    _ => Err("select edges".to_string()),
                })
                .collect::<RResult<Vec<_>>>()?;
            Shape::planar_face(&Shape::compound(&edges)).map_err(geo)?
        }
        _ => return Err("select a sketch or a loop of edges, not both".into()),
    };
    let faces = (0..shape.count(Kind::Face))
        .map(|i| vec![format!("{fid}.f{i}")])
        .collect();
    let names = Naming::from_faces(&shape, faces);
    add_surfaces(fid, state, Body { shape, names })
}

/// Faces of any body by reference, with the names they have there.
fn picked_faces(prev: &State, refs: &[crate::Ref]) -> RResult<Body> {
    let whole = prev.everything().ok_or("there's no body")?;
    let mut faces = Vec::new();
    let mut names = Vec::new();
    for r in refs {
        let (Kind::Face, index) = prev.any_index_of(r)? else {
            return Err("select faces".into());
        };
        faces.push(whole.shape.sub_shape(Kind::Face, index).map_err(geo)?);
        names.push(whole.names.faces[index as usize].clone());
    }
    let shape = Shape::compound(&faces);
    let names = Naming::from_faces(&shape, names);
    Ok(Body { shape, names })
}

pub(crate) fn offset_surface(
    fid: FeatureId,
    p: &OffsetSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let picked = picked_faces(prev, &p.faces)?;
    // Sew the faces so neighbours offset together.
    let sewn = picked.shape.sew(1e-6).map_err(geo)?;
    let sewn_names = Naming::from_history(
        &sewn.result.shape,
        &sewn.result.history,
        Inputs {
            named: [Some(&picked.names), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    let distance = if p.flip { -p.distance } else { p.distance };
    let r = sewn.result.shape.offset_shape(distance).map_err(geo)?;
    // Offset faces are new faces: name them after their source.
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [None, None],
        },
        &mut |h: &HistoryEntry| {
            (h.from_kind == Kind::Face).then(|| {
                let source = sewn_names
                    .face_name(h.from)
                    .unwrap_or("f")
                    .replace(['.', '(', ')', '|'], "_");
                format!("{fid}.offset({source})")
            })
        },
        &fid.to_string(),
    );
    add_surfaces(
        fid,
        state,
        Body {
            shape: r.shape,
            names,
        },
    )
}

/// Surface bodies by ID, each once.
fn surfaces_of(prev: &State, ids: &[BodyId]) -> RResult<Vec<SolidBody>> {
    let mut seen = std::collections::HashSet::new();
    ids.iter()
        .map(|id| {
            if !seen.insert(id) {
                return Err("a body was selected more than once".to_string());
            }
            prev.surface(id).cloned()
        })
        .collect()
}

pub(crate) fn knit(fid: FeatureId, p: &Knit, prev: &State, state: &mut State) -> RResult<()> {
    let inputs = surfaces_of(prev, &p.bodies)?;
    if inputs.is_empty() {
        return Err("select the surfaces to knit".into());
    }
    let shape = Shape::compound(inputs.iter().map(|b| &b.geometry.shape));
    let mut faces = Vec::new();
    for b in &inputs {
        faces.extend(b.geometry.names.faces.iter().cloned());
    }
    let input_names = Naming::from_faces(&shape, faces);
    let sewn = shape.sew(p.tolerance).map_err(geo)?;
    let names = Naming::from_history(
        &sewn.result.shape,
        &sewn.result.history,
        Inputs {
            named: [Some(&input_names), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    state.surfaces.retain(|b| !p.bodies.contains(&b.id));
    if !p.create_solid {
        let parts = shells(&Body {
            shape: sewn.result.shape,
            names,
        })?;
        for (i, geometry) in parts.into_iter().enumerate() {
            let id = if i == 0 {
                p.bodies[0].clone()
            } else {
                BodyId(format!("b{fid}:s{i}"))
            };
            state.surfaces.push(SolidBody { id, geometry });
        }
        return Ok(());
    }
    if sewn.free_edges > 0 {
        return Err(format!(
            "the surfaces leave {} open edge{}; they must enclose a volume to make a solid",
            sewn.free_edges,
            if sewn.free_edges == 1 { "" } else { "s" }
        ));
    }
    let solid = sewn.result.shape.solidify().map_err(geo)?;
    let names = Naming::from_history(
        &solid.shape,
        &solid.history,
        Inputs {
            named: [Some(&names), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    add_solid(
        fid,
        state,
        Body {
            shape: solid.shape,
            names,
        },
        BoolOp::Fuse,
        false,
    )
}

/// Add a new solid (or cut it from the solids) through the legacy aggregate,
/// so solid body identities follow the usual rules.
fn add_solid(
    fid: FeatureId,
    state: &mut State,
    body: Body,
    op: BoolOp,
    merge: bool,
) -> RResult<()> {
    super::apply_tool(fid, state, body.shape, body.names, op, merge)
}

pub(crate) fn thicken(fid: FeatureId, p: &Thicken, prev: &State, state: &mut State) -> RResult<()> {
    let input = prev.surface(&p.body)?.clone();
    let shape = &input.geometry.shape;
    let r = if p.fill_volume {
        let sewn = shape.sew(1e-6).map_err(geo)?;
        if sewn.free_edges > 0 {
            return Err("the surface body isn't closed, so it encloses no volume".into());
        }
        let s = sewn.result.shape.solidify().map_err(geo)?;
        let mut history = sewn.result.history.clone();
        // Compose the two histories: input → sewn → solid.
        history = compose(&history, &s.history);
        of_geom::OpResult {
            shape: s.shape,
            history,
        }
    } else {
        if !(p.thickness.is_finite() && p.thickness > 1e-9) {
            return Err("the thickness must be more than zero".into());
        }
        match p.side {
            ThickenSide::Side1 => shape.thicken(p.thickness).map_err(geo)?,
            ThickenSide::Side2 => shape.thicken(-p.thickness).map_err(geo)?,
            ThickenSide::Both => {
                let mid = shape.offset_shape(-p.thickness / 2.0).map_err(geo)?;
                let t = mid.shape.thicken(p.thickness).map_err(geo)?;
                of_geom::OpResult {
                    history: compose(&mid.history, &t.history),
                    shape: t.shape,
                }
            }
        }
    };
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(&input.geometry.names), None],
        },
        &mut |h: &HistoryEntry| {
            (h.rel == Relation::Generated && h.from_kind == Kind::Edge)
                .then(|| format!("{fid}.side{}", h.from))
        },
        &fid.to_string(),
    );
    // Faces the input had are the thickened body's side 1; the copy is new.
    let names = name_unnamed(&r.shape, names, &format!("{fid}.face"));
    state.surfaces.retain(|b| b.id != p.body);
    let op = if p.cut { BoolOp::Cut } else { BoolOp::Fuse };
    let before = state
        .body
        .as_ref()
        .map(|b| b.shape.mass_props().map(|m| m.volume));
    add_solid(
        fid,
        state,
        Body {
            shape: r.shape,
            names,
        },
        op,
        p.merge,
    )?;
    if p.cut
        && let Some(Ok(before)) = before
    {
        let after = state
            .body
            .as_ref()
            .and_then(|b| b.shape.mass_props().ok())
            .map_or(0.0, |m| m.volume);
        if before - after <= before.abs() * 1e-9 {
            return Err(super::CUT_REMOVES_NOTHING.into());
        }
    }
    Ok(())
}

/// Chain two histories: `a` maps inputs to an intermediate shape, `b` maps
/// that shape to the result.
fn compose(a: &[HistoryEntry], b: &[HistoryEntry]) -> Vec<HistoryEntry> {
    let mut out = Vec::new();
    for x in a {
        for y in b {
            if y.from_kind == x.to_kind && y.from == x.to {
                let rel = match (x.rel, y.rel) {
                    (Relation::Same, r) | (r, Relation::Same) => r,
                    (r, _) => r,
                };
                out.push(HistoryEntry {
                    input: x.input,
                    from_kind: x.from_kind,
                    from: x.from,
                    rel,
                    to_kind: y.to_kind,
                    to: y.to,
                });
            }
        }
    }
    out
}

/// Give faces without a name one from `prefix` and their index.
fn name_unnamed(shape: &Shape, names: Naming, prefix: &str) -> Naming {
    let faces = names
        .faces
        .iter()
        .enumerate()
        .map(|(i, n)| {
            if n.is_empty() {
                vec![format!("{prefix}{i}")]
            } else {
                n.clone()
            }
        })
        .collect();
    Naming::from_faces(shape, faces)
}

pub(crate) fn cut_with_surface(
    fid: FeatureId,
    p: &CutWithSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let body = prev.body.as_ref().ok_or("there's no solid to cut")?;
    let tool = match resolve(prev, &p.surface)? {
        Resolved::Body { id, .. } => prev.surface(&id)?.geometry.shape.clone(),
        Resolved::Face { index, .. } => prev.face_shape(index)?,
        Resolved::Plane(plane) => super::big_face(prev, &plane)?,
        _ => return Err("select a surface body, a face or a plane".into()),
    };
    let r = body.shape.cut_with_surface(&tool, p.flip).map_err(geo)?;
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(&body.names), None],
        },
        &mut |_| Some(format!("{fid}.cut")),
        &fid.to_string(),
    );
    state.body = Some(Body {
        shape: r.shape,
        names,
    });
    Ok(())
}

/// Which body all the faces belong to: (index, is_surface).
fn owner_of(prev: &State, faces: &[u32]) -> RResult<(usize, bool)> {
    let whole = &prev.everything().ok_or("there's no body")?.shape;
    let mut owner = None;
    for (surface, list) in [(false, &prev.bodies), (true, &prev.surfaces)] {
        for (i, b) in list.iter().enumerate() {
            let ix = b
                .geometry
                .shape
                .indices_in(whole, Kind::Face)
                .map_err(geo)?;
            if faces.iter().any(|f| ix.contains(f)) {
                if owner.is_some() || !faces.iter().all(|f| ix.contains(f)) {
                    return Err("select faces of one body".into());
                }
                owner = Some((i, surface));
            }
        }
    }
    owner.ok_or_else(|| "the faces belong to no body".into())
}

pub(crate) fn delete_face(
    fid: FeatureId,
    p: &DeleteFace,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let whole = prev.everything().ok_or("there's no body")?;
    let global = p
        .faces
        .iter()
        .map(|r| match prev.any_index_of(r)? {
            (Kind::Face, i) => Ok(i),
            _ => Err("select faces".to_string()),
        })
        .collect::<RResult<Vec<_>>>()?;
    let (owner, is_surface) = owner_of(prev, &global)?;
    let source = if is_surface {
        &prev.surfaces[owner]
    } else {
        &prev.bodies[owner]
    };
    let ix = source
        .geometry
        .shape
        .indices_in(&whole.shape, Kind::Face)
        .map_err(geo)?;
    let local: Vec<u32> = global
        .iter()
        .map(|g| ix.iter().position(|i| i == g).unwrap() as u32)
        .collect();
    let geometry = &source.geometry;
    let rename = |r: &of_geom::OpResult, tag: &str| {
        Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&geometry.names), None],
            },
            &mut |_| Some(format!("{fid}.{tag}")),
            &fid.to_string(),
        )
    };
    let result = match p.mode {
        DeleteFaceMode::Delete => {
            let r = geometry.shape.remove_faces(&local).map_err(geo)?;
            let names = rename(&r, "face");
            (
                Body {
                    shape: r.shape,
                    names,
                },
                true,
            )
        }
        DeleteFaceMode::Patch => {
            if is_surface {
                return Err("patching works on solids; use fill on a surface".into());
            }
            let r = geometry.shape.defeature(&local).map_err(geo)?;
            let names = rename(&r, "patch");
            (
                Body {
                    shape: r.shape,
                    names,
                },
                false,
            )
        }
        DeleteFaceMode::Fill | DeleteFaceMode::FillTangent => {
            let tangent = p.mode == DeleteFaceMode::FillTangent;
            let filled = fill_holes(fid, geometry, &local, tangent)?;
            let closed = !is_surface;
            if closed {
                let solid = filled.shape.solidify().map_err(geo)?;
                let names = Naming::from_history(
                    &solid.shape,
                    &solid.history,
                    Inputs {
                        named: [Some(&filled.names), None],
                    },
                    &mut |_| None,
                    &fid.to_string(),
                );
                (
                    Body {
                        shape: solid.shape,
                        names,
                    },
                    false,
                )
            } else {
                (filled, true)
            }
        }
    };
    let (body, surface) = result;
    let id = source.id.clone();
    if is_surface {
        state.surfaces.retain(|b| b.id != id);
    } else {
        state.bodies.retain(|b| b.id != id);
        state.aggregate_bodies()?;
    }
    if surface {
        let parts = shells(&body)?;
        for (i, geometry) in parts.into_iter().enumerate() {
            let id = if i == 0 {
                id.clone()
            } else {
                BodyId(format!("b{fid}:s{i}"))
            };
            state.surfaces.push(SolidBody { id, geometry });
        }
    } else {
        state.bodies.push(SolidBody { id, geometry: body });
        state.aggregate_bodies()?;
    }
    Ok(())
}

/// Remove faces and patch each hole they leave, sewn into one shell.
fn fill_holes(fid: FeatureId, body: &Body, faces: &[u32], tangent: bool) -> RResult<Body> {
    let open = body.shape.remove_faces(faces).map_err(geo)?;
    let open_names = Naming::from_history(
        &open.shape,
        &open.history,
        Inputs {
            named: [Some(&body.names), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    let before = body.shape.free_edges().map_err(geo)?;
    let rim = open.shape.free_edges().map_err(geo)?;
    // Only the new gap's edges: the body may already have had open edges.
    let edges: Vec<Shape> = (0..rim.count(Kind::Edge))
        .filter_map(|i| rim.sub_shape(Kind::Edge, i).ok())
        .filter(|e| {
            before.count(Kind::Edge) == 0 || e.distance(&before).is_ok_and(|d| d.value > 1e-7)
        })
        .collect();
    if edges.is_empty() {
        return Err("deleting the faces leaves no gap to fill".into());
    }
    let edge_faces = open.shape.edge_faces();
    let supports: Vec<Option<Shape>> = edges
        .iter()
        .map(|e| {
            let index = e.indices_in(&open.shape, Kind::Edge).ok()?[0];
            let face = *edge_faces.get(index as usize)?.first()?;
            open.shape.sub_shape(Kind::Face, face).ok()
        })
        .collect();
    let boundary: Vec<FillEdge> = edges
        .iter()
        .zip(&supports)
        .map(|(edge, support)| FillEdge {
            edge,
            support: support.as_ref(),
            continuity: if tangent {
                Continuity::Tangent
            } else {
                Continuity::Contact
            },
        })
        .collect();
    let (patch, _) = Shape::fill(&boundary, &[]).map_err(geo)?;
    let loose = Shape::compound([&open.shape, &patch]);
    let mut faces_names = open_names.faces.clone();
    faces_names.extend((0..patch.count(Kind::Face)).map(|i| vec![format!("{fid}.fill{i}")]));
    let loose_names = Naming::from_faces(&loose, faces_names);
    let sewn = loose.sew(1e-5).map_err(geo)?;
    let names = Naming::from_history(
        &sewn.result.shape,
        &sewn.result.history,
        Inputs {
            named: [Some(&loose_names), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    Ok(Body {
        shape: sewn.result.shape,
        names,
    })
}
