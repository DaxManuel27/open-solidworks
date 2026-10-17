//! Shaping surfaces: filled, ruled and radiate surfaces, trim and untrim,
//! extend, mid-surface and freeform.

use of_geom::{Continuity, DVec3, FillEdge, Kind, Ruled, Shape};

use super::geo;
use super::surface::add_surfaces;
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::resolve::{RResult, Resolved, resolve};
use crate::surfaces::{
    ExtendEnd, ExtendSurface, FilledSurface, Freeform, MidSurface, PatchContinuity, RadiateSurface,
    RuledDirection, RuledSurface, TrimSurface, UntrimMode, UntrimSurface,
};
use crate::{BodyId, FeatureId, Ref, SolidBody};

/// Where a face or edge of [`State::everything`] lives.
#[derive(Debug, Clone, Copy)]
struct Owner {
    surface: bool,
    body: usize,
    local: u32,
}

fn body_at<'a>(state: &'a State, o: &Owner) -> &'a SolidBody {
    if o.surface {
        &state.surfaces[o.body]
    } else {
        &state.bodies[o.body]
    }
}

fn owner(state: &State, kind: Kind, global: u32) -> RResult<Owner> {
    let whole = &state.everything().ok_or("there's no body")?.shape;
    for (surface, list) in [(false, &state.bodies), (true, &state.surfaces)] {
        for (body, b) in list.iter().enumerate() {
            let ix = b.geometry.shape.indices_in(whole, kind).map_err(geo)?;
            if let Some(local) = ix.iter().position(|g| *g == global) {
                return Ok(Owner {
                    surface,
                    body,
                    local: local as u32,
                });
            }
        }
    }
    Err("that belongs to no body".into())
}

fn owner_of(state: &State, r: &Ref, kind: Kind) -> RResult<Owner> {
    let (k, global) = state.any_index_of(r)?;
    if k != kind {
        return Err(match kind {
            Kind::Edge => "select edges",
            _ => "select faces",
        }
        .into());
    }
    owner(state, kind, global)
}

/// The faces beside an edge of a shape.
fn faces_beside(shape: &Shape, edge: u32) -> Vec<u32> {
    shape
        .edge_faces()
        .get(edge as usize)
        .cloned()
        .unwrap_or_default()
}

fn named(shape: Shape, prefix: &str) -> Body {
    let faces = (0..shape.count(Kind::Face))
        .map(|i| vec![format!("{prefix}{i}")])
        .collect();
    let names = Naming::from_faces(&shape, faces);
    Body { shape, names }
}

/// Sew `parts` (each named) and name the result from them.
fn sew_named(fid: FeatureId, parts: &[&Body], tolerance: f64) -> RResult<(Body, u32)> {
    let shape = Shape::compound(parts.iter().map(|b| &b.shape));
    let mut faces = Vec::new();
    for b in parts {
        faces.extend(b.names.faces.iter().cloned());
    }
    let loose = Naming::from_faces(&shape, faces);
    let sewn = shape.sew(tolerance).map_err(geo)?;
    let names = Naming::from_history(
        &sewn.result.shape,
        &sewn.result.history,
        Inputs {
            named: [Some(&loose), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    Ok((
        Body {
            shape: sewn.result.shape,
            names,
        },
        sewn.free_edges,
    ))
}

/// Replace surface bodies `old` with the shells of `body`; the first shell
/// keeps the first old body's ID.
fn replace_surfaces(fid: FeatureId, state: &mut State, old: &[BodyId], body: Body) -> RResult<()> {
    state.surfaces.retain(|b| !old.contains(&b.id));
    let parts = super::surface::shells_of_body(&body)?;
    for (i, geometry) in parts.into_iter().enumerate() {
        let id = match (i, old.first()) {
            (0, Some(id)) => id.clone(),
            _ => BodyId(format!("b{fid}:s{i}")),
        };
        state.surfaces.push(SolidBody { id, geometry });
    }
    Ok(())
}

fn continuity(c: PatchContinuity) -> Continuity {
    match c {
        PatchContinuity::Contact => Continuity::Contact,
        PatchContinuity::Tangent => Continuity::Tangent,
        PatchContinuity::Curvature => Continuity::Curvature,
    }
}

pub(crate) fn filled_surface(
    fid: FeatureId,
    p: &FilledSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let whole = prev.everything();
    let mut edges: Vec<(Shape, Option<Shape>, Continuity)> = Vec::new();
    let mut touched: Vec<BodyId> = Vec::new();
    for b in &p.boundary {
        match prev.any_index_of(&b.edge) {
            Ok((Kind::Edge, global)) => {
                let whole = whole.ok_or("there's no body")?;
                let edge = whole.shape.sub_shape(Kind::Edge, global).map_err(geo)?;
                let o = owner(prev, Kind::Edge, global)?;
                let body = body_at(prev, &o);
                if o.surface && !touched.contains(&body.id) {
                    touched.push(body.id.clone());
                }
                let beside = faces_beside(&body.geometry.shape, o.local);
                let pick = if b.alternate_face {
                    beside.get(1)
                } else {
                    beside.first()
                };
                let support = match (b.continuity, pick) {
                    (PatchContinuity::Contact, _) => None,
                    (_, Some(f)) => {
                        Some(body.geometry.shape.sub_shape(Kind::Face, *f).map_err(geo)?)
                    }
                    (_, None) => {
                        return Err("a tangent or curvature edge needs a face beside it".into());
                    }
                };
                edges.push((edge, support, continuity(b.continuity)));
            }
            _ => {
                if b.continuity != PatchContinuity::Contact {
                    return Err("curves have no face to be tangent to; use contact".into());
                }
                let wire = crate::curves::path(prev, &b.edge)?;
                for i in 0..wire.count(Kind::Edge) {
                    edges.push((
                        wire.sub_shape(Kind::Edge, i).map_err(geo)?,
                        None,
                        Continuity::Contact,
                    ));
                }
            }
        }
    }
    let constraints = p
        .constraints
        .iter()
        .map(|r| crate::curves::path(prev, r))
        .collect::<RResult<Vec<_>>>()?;
    let boundary: Vec<FillEdge> = edges
        .iter()
        .map(|(edge, support, continuity)| FillEdge {
            edge,
            support: support.as_ref(),
            continuity: *continuity,
        })
        .collect();
    let (patch, _) =
        Shape::fill(&boundary, &constraints.iter().collect::<Vec<_>>()).map_err(geo)?;
    let patch = named(patch, &format!("{fid}.patch"));
    if !p.merge || touched.is_empty() {
        return add_surfaces(fid, state, patch);
    }
    let bodies: Vec<&Body> = touched
        .iter()
        .map(|id| prev.surface(id).map(|b| &b.geometry))
        .collect::<RResult<_>>()?;
    let mut parts = bodies;
    parts.push(&patch);
    let (sewn, free) = sew_named(fid, &parts, 1e-4)?;
    if p.create_solid {
        if free > 0 {
            return Err(format!("the patched surface still has {free} open edges"));
        }
        let solid = sewn.shape.solidify().map_err(geo)?;
        let names = Naming::from_history(
            &solid.shape,
            &solid.history,
            Inputs {
                named: [Some(&sewn.names), None],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        state.surfaces.retain(|b| !touched.contains(&b.id));
        return super::apply_tool(fid, state, solid.shape, names, of_geom::BoolOp::Fuse, false);
    }
    replace_surfaces(fid, state, &touched, sewn)
}

fn strips(
    fid: FeatureId,
    prev: &State,
    edges: &[Ref],
    ruled: Ruled,
    distance: f64,
    alternate: bool,
    knit: bool,
) -> RResult<Body> {
    if !(distance.is_finite() && distance > 0.0) {
        return Err("the distance must be more than zero".into());
    }
    let mut out = Vec::new();
    for (i, r) in edges.iter().enumerate() {
        let o = owner_of(prev, r, Kind::Edge)?;
        let shape = &body_at(prev, &o).geometry.shape;
        let face = faces_beside(shape, o.local).first().copied();
        let strip = shape
            .ruled_strip(o.local, face, ruled, distance, alternate)
            .map_err(geo)?;
        out.push(named(strip, &format!("{fid}.r{i}.")));
    }
    if knit && out.len() > 1 {
        let parts: Vec<&Body> = out.iter().collect();
        Ok(sew_named(fid, &parts, 1e-4)?.0)
    } else {
        let parts: Vec<&Body> = out.iter().collect();
        let shape = Shape::compound(parts.iter().map(|b| &b.shape));
        let faces = parts
            .iter()
            .flat_map(|b| b.names.faces.iter().cloned())
            .collect();
        let names = Naming::from_faces(&shape, faces);
        Ok(Body { shape, names })
    }
}

pub(crate) fn ruled_surface(
    fid: FeatureId,
    p: &RuledSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let dir = |r: &Ref| -> RResult<DVec3> { Ok(resolve(prev, r)?.direction()?.normalize()) };
    let ruled = match &p.direction {
        RuledDirection::TangentToSurface => Ruled::Tangent,
        RuledDirection::NormalToSurface => Ruled::Normal,
        RuledDirection::TaperedToVector { direction, angle } => Ruled::Tapered {
            vector: dir(direction)?,
            angle: *angle,
        },
        RuledDirection::PerpendicularToVector { direction } => Ruled::Perpendicular {
            vector: dir(direction)?,
        },
        RuledDirection::Sweep { direction } => Ruled::Swept {
            vector: dir(direction)?,
        },
    };
    let body = strips(
        fid,
        prev,
        &p.edges,
        ruled,
        p.distance,
        p.alternate_side,
        p.knit,
    )?;
    add_surfaces(fid, state, body)
}

pub(crate) fn radiate_surface(
    fid: FeatureId,
    p: &RadiateSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let normal = resolve(prev, &p.plane)?.plane()?.normal;
    let body = strips(
        fid,
        prev,
        &p.edges,
        Ruled::Perpendicular { vector: normal },
        p.distance,
        p.flip,
        true,
    )?;
    add_surfaces(fid, state, body)
}

/// Groups of faces of `shape` connected across edges that don't lie on `cut`.
fn regions(shape: &Shape, cut: &Shape, tolerance: f64) -> RResult<Vec<Vec<u32>>> {
    let n = shape.count(Kind::Face) as usize;
    let edge_faces = shape.edge_faces();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut i = i;
        while p[i] != r {
            let next = p[i];
            p[i] = r;
            i = next;
        }
        r
    }
    for (e, faces) in edge_faces.iter().enumerate() {
        if faces.len() < 2 {
            continue;
        }
        let edge = shape.sub_shape(Kind::Edge, e as u32).map_err(geo)?;
        let mid = edge.edge_info(0).map_err(geo)?.mid;
        let on_cut = of_geom::Shape::vertex_at(mid)
            .and_then(|v| v.distance(cut))
            .is_ok_and(|d| d.value < tolerance);
        if on_cut {
            continue;
        }
        let a = find(&mut parent, faces[0] as usize);
        for f in &faces[1..] {
            let b = find(&mut parent, *f as usize);
            parent[b] = a;
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<u32>> = Default::default();
    for f in 0..n {
        let r = find(&mut parent, f);
        groups.entry(r).or_default().push(f as u32);
    }
    Ok(groups.into_values().collect())
}

pub(crate) fn trim_surface(
    fid: FeatureId,
    p: &TrimSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let targets: Vec<SolidBody> = p
        .bodies
        .iter()
        .map(|id| prev.surface(id).cloned())
        .collect::<RResult<_>>()?;
    let target = Shape::compound(targets.iter().map(|b| &b.geometry.shape));
    let mut faces = Vec::new();
    for b in &targets {
        faces.extend(b.geometry.names.faces.iter().cloned());
    }
    let target_names = Naming::from_faces(&target, faces);
    // Split the bodies, and find the cut lines between the pieces.
    let (split, names, cut) = match &p.tool {
        None => {
            if targets.len() < 2 {
                return Err("a mutual trim needs two or more surface bodies".into());
            }
            // Each body is split by all the others.
            let mut shapes = Vec::new();
            let mut faces = Vec::new();
            let mut cuts = Vec::new();
            for (i, body) in targets.iter().enumerate() {
                let others = Shape::compound(
                    targets
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, b)| &b.geometry.shape),
                );
                let r = body.geometry.shape.split_tools(&others).map_err(geo)?;
                let n = Naming::from_history(
                    &r.shape,
                    &r.history,
                    Inputs {
                        named: [Some(&body.geometry.names), None],
                    },
                    &mut |_| Some(format!("{fid}.trim")),
                    &fid.to_string(),
                );
                cuts.push(body.geometry.shape.section(&others).map_err(geo)?);
                faces.extend(n.faces.iter().cloned());
                shapes.push(r.shape);
            }
            let shape = Shape::compound(&shapes);
            let names = Naming::from_faces(&shape, faces);
            (
                of_geom::OpResult {
                    shape,
                    history: Vec::new(),
                },
                names,
                Shape::compound(&cuts),
            )
        }
        Some(r) => {
            let tool = match resolve(prev, r)? {
                Resolved::Plane(plane) => super::big_face(prev, &plane)?,
                Resolved::Face { index, .. } => prev.face_shape(index)?,
                Resolved::Body { id, .. } => prev.any_body(&id)?.0.geometry.shape.clone(),
                _ => return Err("trim with a plane, face or surface".into()),
            };
            let split = target.split_tools(&tool).map_err(geo)?;
            let names = Naming::from_history(
                &split.shape,
                &split.history,
                Inputs {
                    named: [Some(&target_names), None],
                },
                &mut |_| Some(format!("{fid}.trim")),
                &fid.to_string(),
            );
            let cut = target.section(&tool).map_err(geo)?;
            (split, names, cut)
        }
    };
    let size = target.bbox().map_or(1.0, |b| b.diagonal());
    let groups = regions(&split.shape, &cut, 1e-6 * size.max(1.0))?;
    if groups.len() < 2 {
        return Err("the tool doesn't cut the surfaces".into());
    }
    let chosen: Vec<usize> = if p.split_all {
        (0..groups.len()).collect()
    } else {
        if p.pieces.is_empty() {
            return Err("pick the pieces to keep".into());
        }
        let mut picked = Vec::new();
        for point in &p.pieces {
            let at = Shape::vertex_at(DVec3::from_array(*point)).map_err(geo)?;
            let best = groups
                .iter()
                .enumerate()
                .map(|(g, faces)| {
                    let d = faces
                        .iter()
                        .filter_map(|f| split.shape.sub_shape(Kind::Face, *f).ok())
                        .filter_map(|f| f.distance(&at).ok())
                        .map(|d| d.value)
                        .fold(f64::INFINITY, f64::min);
                    (g, d)
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(g, _)| g)
                .unwrap();
            picked.push(best);
        }
        if p.remove {
            (0..groups.len()).filter(|g| !picked.contains(g)).collect()
        } else {
            picked
        }
    };
    if chosen.is_empty() {
        return Err("that removes every piece".into());
    }
    let drop: Vec<u32> = (0..split.shape.count(Kind::Face))
        .filter(|f| !chosen.iter().any(|g| groups[*g].contains(f)))
        .collect();
    let kept = if drop.is_empty() {
        of_geom::OpResult {
            history: (0..split.shape.count(Kind::Face))
                .map(|i| of_geom::HistoryEntry {
                    input: 0,
                    from_kind: Kind::Face,
                    from: i,
                    rel: of_geom::Relation::Same,
                    to_kind: Kind::Face,
                    to: i,
                })
                .collect(),
            shape: split.shape.clone(),
        }
    } else {
        split.shape.remove_faces(&drop).map_err(geo)?
    };
    let names = Naming::from_history(
        &kept.shape,
        &kept.history,
        Inputs {
            named: [Some(&names), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    let body = Body {
        shape: kept.shape,
        names,
    };
    if p.split_all {
        // Each region its own body.
        state.surfaces.retain(|b| !p.bodies.contains(&b.id));
        let edges_cut = regions(&body.shape, &cut, 1e-6 * size.max(1.0))?;
        for (i, faces) in edges_cut.iter().enumerate() {
            let drop: Vec<u32> = (0..body.shape.count(Kind::Face))
                .filter(|f| !faces.contains(f))
                .collect();
            let part = if drop.is_empty() {
                body.clone()
            } else {
                let r = body.shape.remove_faces(&drop).map_err(geo)?;
                let names = Naming::from_history(
                    &r.shape,
                    &r.history,
                    Inputs {
                        named: [Some(&body.names), None],
                    },
                    &mut |_| None,
                    &fid.to_string(),
                );
                Body {
                    shape: r.shape,
                    names,
                }
            };
            state.surfaces.push(SolidBody {
                id: BodyId(format!("b{fid}:p{i}")),
                geometry: part,
            });
        }
        return Ok(());
    }
    // Kept pieces of different bodies meet along the cut: join them.
    let body = if p.tool.is_none() {
        sew_named(fid, &[&body], 1e-5)?.0
    } else {
        body
    };
    replace_surfaces(fid, state, &p.bodies, body)
}

pub(crate) fn untrim_surface(
    fid: FeatureId,
    p: &UntrimSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let mut work = prev.clone();
    for r in &p.faces {
        let o = owner_of(&work, r, Kind::Face)?;
        if !o.surface {
            return Err("untrim works on surface bodies".into());
        }
        let body = work.surfaces[o.body].clone();
        let geometry = match p.mode {
            UntrimMode::FillHoles => {
                let r = body.geometry.shape.remove_holes(&[o.local]).map_err(geo)?;
                let names = Naming::from_history(
                    &r.shape,
                    &r.history,
                    Inputs {
                        named: [Some(&body.geometry.names), None],
                    },
                    &mut |_| None,
                    &fid.to_string(),
                );
                Body {
                    shape: r.shape,
                    names,
                }
            }
            UntrimMode::Natural => {
                let face = body.geometry.shape.untrim_face(o.local).map_err(geo)?;
                let old_names = body.geometry.names.faces[o.local as usize].clone();
                let face_names = Naming::from_faces(&face, vec![old_names]);
                let rest = if body.geometry.shape.count(Kind::Face) > 1 {
                    let r = body.geometry.shape.remove_faces(&[o.local]).map_err(geo)?;
                    let names = Naming::from_history(
                        &r.shape,
                        &r.history,
                        Inputs {
                            named: [Some(&body.geometry.names), None],
                        },
                        &mut |_| None,
                        &fid.to_string(),
                    );
                    Some(Body {
                        shape: r.shape,
                        names,
                    })
                } else {
                    None
                };
                let untrimmed = Body {
                    shape: face,
                    names: face_names,
                };
                match rest {
                    Some(rest) => sew_named(fid, &[&rest, &untrimmed], 1e-6)?.0,
                    None => untrimmed,
                }
            }
        };
        work.surfaces[o.body].geometry = geometry;
        work.aggregate_all()?;
    }
    state.surfaces = work.surfaces;
    Ok(())
}

pub(crate) fn extend_surface(
    fid: FeatureId,
    p: &ExtendSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let mut by_body: Vec<(usize, Vec<Body>)> = Vec::new();
    let reach = prev
        .everything()
        .and_then(|b| b.shape.bbox())
        .map_or(1000.0, |b| b.diagonal() * 2.0 + 10.0);
    for (i, r) in p.edges.iter().enumerate() {
        let o = owner_of(prev, r, Kind::Edge)?;
        if !o.surface {
            return Err("extend works on the open edges of surface bodies".into());
        }
        let geometry = &prev.surfaces[o.body].geometry;
        let shape = &geometry.shape;
        let beside = faces_beside(shape, o.local);
        let [face] = beside[..] else {
            return Err("select open edges of a surface".into());
        };
        let edge = shape.sub_shape(Kind::Edge, o.local).map_err(geo)?;
        let (distance, target) = match &p.end {
            ExtendEnd::Distance { distance } => (*distance, None),
            ExtendEnd::UpToPoint { point } => {
                let at = Shape::vertex_at(resolve(prev, point)?.point()?).map_err(geo)?;
                (edge.distance(&at).map_err(geo)?.value, None)
            }
            ExtendEnd::UpToSurface { target } => {
                let tool = match resolve(prev, target)? {
                    Resolved::Plane(plane) => super::big_face(prev, &plane)?,
                    Resolved::Face { index, .. } => prev.face_shape(index)?,
                    Resolved::Body { id, .. } => prev.any_body(&id)?.0.geometry.shape.clone(),
                    _ => return Err("extend up to a plane, face or surface".into()),
                };
                (reach, Some(tool))
            }
        };
        if !(distance.is_finite() && distance > 1e-9) {
            return Err("the extension must be longer than zero".into());
        }
        let mut strip = if p.linear {
            shape
                .ruled_strip(o.local, Some(face), Ruled::Tangent, distance, false)
                .map_err(geo)?
        } else {
            shape.extend_strip(o.local, face, distance).map_err(geo)?
        };
        if let Some(tool) = target {
            // Keep the part of the strip between the edge and the target.
            let split = strip.split_tools(&tool).map_err(geo)?;
            let near: Vec<u32> = (0..split.shape.count(Kind::Face))
                .filter(|f| {
                    split
                        .shape
                        .sub_shape(Kind::Face, *f)
                        .and_then(|face| face.distance(&edge))
                        .is_ok_and(|d| d.value < 1e-5)
                })
                .collect();
            if near.len() as u32 == split.shape.count(Kind::Face) {
                return Err("the extension doesn't reach the target".into());
            }
            let drop: Vec<u32> = (0..split.shape.count(Kind::Face))
                .filter(|f| !near.contains(f))
                .collect();
            strip = split.shape.remove_faces(&drop).map_err(geo)?.shape;
        }
        let base = geometry.names.face_name(face).unwrap_or("f").to_string();
        let strip = named(strip, &format!("{base}.ext{fid}.{i}."));
        match by_body.iter_mut().find(|(b, _)| *b == o.body) {
            Some((_, list)) => list.push(strip),
            None => by_body.push((o.body, vec![strip])),
        }
    }
    for (b, list) in by_body {
        let body = &prev.surfaces[b];
        let mut parts: Vec<&Body> = vec![&body.geometry];
        parts.extend(list.iter());
        let (sewn, _) = sew_named(fid, &parts, 1e-4)?;
        let id = body.id.clone();
        let at = state.surfaces.iter().position(|s| s.id == id).unwrap();
        state.surfaces[at].geometry = sewn;
    }
    Ok(())
}

pub(crate) fn mid_surface(
    fid: FeatureId,
    p: &MidSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let whole = prev.everything().ok_or("there's no body")?;
    let index = |r: &Ref| -> RResult<u32> {
        match prev.any_index_of(r)? {
            (Kind::Face, i) => Ok(i),
            _ => Err("select faces".into()),
        }
    };
    let mut pairs: Vec<(u32, u32)> = p
        .pairs
        .iter()
        .map(|pair| Ok((index(&pair.a)?, index(&pair.b)?)))
        .collect::<RResult<_>>()?;
    if pairs.is_empty() {
        let id = p
            .body
            .as_ref()
            .ok_or("select face pairs, or a body to find them in")?;
        let body = prev.solid(id)?;
        let ix = body
            .geometry
            .shape
            .indices_in(&whole.shape, Kind::Face)
            .map_err(geo)?;
        pairs = find_pairs(&whole.shape, &ix, p.threshold)?;
        if pairs.is_empty() {
            return Err("no faces face each other closer than the threshold".into());
        }
    }
    // Each mid face is face `a` moved toward `b`.
    let mut offsets = Vec::new();
    for (a, b) in &pairs {
        let fa = whole.shape.face_info(*a).map_err(geo)?;
        let fb = whole.shape.face_info(*b).map_err(geo)?;
        let gap = match (fa.kind, fb.kind) {
            (of_geom::SurfaceKind::Plane, of_geom::SurfaceKind::Plane) => {
                if fa.normal.dot(fb.normal) > -0.999 {
                    return Err("paired flat faces must face each other".into());
                }
                (fb.point - fa.point).dot(fa.normal).abs()
            }
            (of_geom::SurfaceKind::Cylinder, of_geom::SurfaceKind::Cylinder) => {
                let axis = fa.axis_dir.normalize();
                let off = fb.axis_origin - fa.axis_origin;
                if axis.cross(fb.axis_dir.normalize()).length() > 1e-6
                    || (off - axis * off.dot(axis)).length() > 1e-6
                {
                    return Err("paired cylindrical faces must share an axis".into());
                }
                (fa.radius - fb.radius).abs()
            }
            _ => {
                return Err(
                    "mid-surfaces are made between flat or coaxial cylindrical faces".into(),
                );
            }
        };
        offsets.push(gap * p.position);
    }
    let same = offsets.iter().all(|o| (o - offsets[0]).abs() < 1e-6);
    let source = Shape::compound(
        pairs
            .iter()
            .map(|(a, _)| whole.shape.sub_shape(Kind::Face, *a))
            .collect::<Result<Vec<_>, _>>()
            .map_err(geo)?
            .iter(),
    );
    let mid = if same {
        // One offset of the connected faces keeps the corners joined.
        let sewn = source.sew(1e-6).map_err(geo)?;
        sewn.result
            .shape
            .offset_shape(-offsets[0])
            .map_err(geo)?
            .shape
    } else {
        let faces = pairs
            .iter()
            .zip(&offsets)
            .map(|((a, _), d)| {
                whole
                    .shape
                    .sub_shape(Kind::Face, *a)
                    .and_then(|f| f.offset_shape(-d))
                    .map(|r| r.shape)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(geo)?;
        let c = Shape::compound(&faces);
        if p.knit {
            c.sew(offsets.iter().copied().fold(0.0, f64::max).max(1e-4))
                .map_err(geo)?
                .result
                .shape
        } else {
            c
        }
    };
    add_surfaces(fid, state, named(mid, &format!("{fid}.mid")))
}

/// Faces of a body facing each other closer than `threshold`: parallel flat
/// faces with opposite normals, or coaxial cylinders. The larger face of each
/// pair comes first.
fn find_pairs(whole: &Shape, faces: &[u32], threshold: f64) -> RResult<Vec<(u32, u32)>> {
    let info: Vec<_> = faces
        .iter()
        .map(|f| whole.face_info(*f).map_err(geo))
        .collect::<RResult<_>>()?;
    let mut used = vec![false; faces.len()];
    let mut out = Vec::new();
    for i in 0..faces.len() {
        if used[i] {
            continue;
        }
        let a = &info[i];
        let mut best: Option<(usize, f64)> = None;
        for j in 0..faces.len() {
            if i == j || used[j] {
                continue;
            }
            let b = &info[j];
            let gap = match (a.kind, b.kind) {
                (of_geom::SurfaceKind::Plane, of_geom::SurfaceKind::Plane)
                    if a.normal.dot(b.normal) < -0.999 =>
                {
                    let gap = (b.point - a.point).dot(a.normal);
                    // b lies behind a, inside the material.
                    if gap >= -1e-9 {
                        continue;
                    }
                    // Their outlines overlap seen along the normal.
                    let along = a.normal * (b.centroid - a.centroid).dot(a.normal);
                    let shifted = b.centroid - along;
                    if (shifted - a.centroid).length() > (a.area.sqrt() + b.area.sqrt()) {
                        continue;
                    }
                    -gap
                }
                (of_geom::SurfaceKind::Cylinder, of_geom::SurfaceKind::Cylinder) => {
                    let axis = a.axis_dir.normalize();
                    let off = b.axis_origin - a.axis_origin;
                    if axis.cross(b.axis_dir.normalize()).length() > 1e-6
                        || (off - axis * off.dot(axis)).length() > 1e-6
                    {
                        continue;
                    }
                    (a.radius - b.radius).abs()
                }
                _ => continue,
            };
            if gap > 1e-9 && gap <= threshold && best.is_none_or(|(_, g)| gap < g) {
                best = Some((j, gap));
            }
        }
        if let Some((j, _)) = best {
            used[i] = true;
            used[j] = true;
            let (a, b) = if info[i].area >= info[j].area {
                (i, j)
            } else {
                (j, i)
            };
            out.push((faces[a], faces[b]));
        }
    }
    Ok(out)
}

pub(crate) fn freeform(
    fid: FeatureId,
    p: &Freeform,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let o = owner_of(prev, &p.face, Kind::Face)?;
    let body = body_at(prev, &o).clone();
    let fixed = match p.continuity {
        PatchContinuity::Contact => 1,
        PatchContinuity::Tangent => 2,
        PatchContinuity::Curvature => 3,
    };
    let moves: Vec<(f64, f64, DVec3)> = p
        .moves
        .iter()
        .map(|m| (m.u, m.v, DVec3::from_array(m.offset)))
        .collect();
    let r = body
        .geometry
        .shape
        .freeform_face(o.local, p.grid.clamp(4, 40), &moves, fixed)
        .map_err(geo)?;
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(&body.geometry.names), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    let geometry = Body {
        shape: r.shape,
        names,
    };
    if o.surface {
        state.surfaces[o.body].geometry = geometry;
    } else {
        state.bodies[o.body].geometry = geometry;
        state.aggregate_bodies()?;
    }
    Ok(())
}
