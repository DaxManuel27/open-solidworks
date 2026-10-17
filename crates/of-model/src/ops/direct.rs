//! Move, replace and offset face, and delete hole.

use of_geom::{DAffine3, DVec3, Kind, Shape};

use super::geo;
use crate::build::{Body, State};
use crate::direct::{DeleteHole, FaceMotion, MoveFace, ReplaceFace};
use crate::naming::{Inputs, Naming};
use crate::resolve::{RResult, Resolved, resolve};
use crate::{FeatureId, Ref};

/// The solid body a face belongs to, and the face's index in it.
fn solid_face(state: &State, r: &Ref) -> RResult<(usize, u32)> {
    let (Kind::Face, global) = state.any_index_of(r)? else {
        return Err("select faces".into());
    };
    if state.surface_of_face(global).is_some() {
        return Err("move and replace face work on solids; this face is on a surface body".into());
    }
    let whole = &state.body.as_ref().ok_or("there's no solid")?.shape;
    for (i, b) in state.bodies.iter().enumerate() {
        let ix = b
            .geometry
            .shape
            .indices_in(whole, Kind::Face)
            .map_err(geo)?;
        if let Some(local) = ix.iter().position(|g| *g == global) {
            return Ok((i, local as u32));
        }
    }
    Err("the face belongs to no solid".into())
}

/// Replace one face of a body with `surface`, keeping names: the faces on
/// the new surface take the old face's names.
fn replace(fid: FeatureId, body: &Body, face: u32, surface: &Shape) -> RResult<Body> {
    let r = body.shape.replace_face(face, surface).map_err(geo)?;
    let mut names = Naming::from_history(
        &r.result.shape,
        &r.result.history,
        Inputs {
            named: [Some(&body.names), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    let old = body.names.faces[face as usize].clone();
    for (n, f) in r.new_faces.iter().enumerate() {
        names.faces[*f as usize] = if n == 0 {
            old.clone()
        } else {
            old.iter().map(|o| format!("{o}~{n}")).collect()
        };
    }
    let names = Naming::from_faces(&r.result.shape, names.faces);
    Ok(Body {
        shape: r.result.shape,
        names,
    })
}

/// How far past the model a moved surface must reach.
fn reach(state: &State) -> f64 {
    state
        .everything()
        .and_then(|b| b.shape.bbox())
        .map_or(1000.0, |b| b.diagonal() * 2.0 + 10.0)
}

fn rotation_about(origin: DVec3, dir: DVec3, angle: f64) -> DAffine3 {
    DAffine3::from_translation(origin)
        * DAffine3::from_axis_angle(dir.normalize(), angle)
        * DAffine3::from_translation(-origin)
}

/// Apply `edit` to each face in turn, re-finding each by name in the body
/// as it is after the previous edits.
fn each_face(
    prev: &State,
    state: &mut State,
    faces: &[Ref],
    mut edit: impl FnMut(&State, &Body, u32) -> RResult<Body>,
) -> RResult<()> {
    if faces.is_empty() {
        return Err("select faces".into());
    }
    let mut work = prev.clone();
    for r in faces {
        let (b, face) = solid_face(&work, r)?;
        let geometry = edit(&work, &work.bodies[b].geometry, face)?;
        work.bodies[b].geometry = geometry;
        work.aggregate_bodies()?;
        work.aggregate_all()?;
    }
    state.bodies = work.bodies;
    state.aggregate_bodies()
}

pub(crate) fn move_face(
    fid: FeatureId,
    p: &MoveFace,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let long = reach(prev);
    each_face(prev, state, &p.faces, |work, body, face| {
        let shape = body.shape.sub_shape(Kind::Face, face).map_err(geo)?;
        let moved = match &p.motion {
            FaceMotion::Offset { distance } => {
                if !distance.is_finite() || distance.abs() < 1e-9 {
                    return Err("the offset must not be zero".into());
                }
                shape.offset_shape(*distance).map_err(geo)?.shape
            }
            FaceMotion::Translate {
                direction,
                distance,
                reverse,
                up_to,
            } => {
                let mut dir = resolve(work, direction)?.direction()?.normalize();
                if *reverse {
                    dir = -dir;
                }
                let info = shape.face_info(0).map_err(geo)?;
                let d = match up_to {
                    None => *distance,
                    Some(target) => match resolve(work, target)? {
                        r @ (Resolved::Plane(_) | Resolved::Face { .. }) if r.plane().is_ok() => {
                            let plane = r.plane()?;
                            let along = dir.dot(plane.normal);
                            if along.abs() < 1e-9 {
                                return Err("the direction runs parallel to the target face".into());
                            }
                            (plane.origin - info.centroid).dot(plane.normal) / along
                        }
                        r => (r.point()? - info.centroid).dot(dir),
                    },
                };
                if !d.is_finite() || d.abs() < 1e-9 {
                    return Err("the faces don't move".into());
                }
                shape
                    .transform(&DAffine3::from_translation(dir * d))
                    .map_err(geo)?
                    .shape
            }
            FaceMotion::Rotate { axis, angle } => {
                if !angle.is_finite() || angle.abs() < 1e-12 {
                    return Err("the angle must not be zero".into());
                }
                let (origin, dir) = resolve(work, axis)?.axis()?;
                shape
                    .transform(&rotation_about(origin, dir, *angle))
                    .map_err(geo)?
                    .shape
            }
        };
        let surface = moved.extend_face(long).map_err(geo)?;
        replace(fid, body, face, &surface)
    })
}

pub(crate) fn replace_face(
    fid: FeatureId,
    p: &ReplaceFace,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let surface = match resolve(prev, &p.replacement)? {
        Resolved::Body { id, .. } => prev.surface(&id)?.geometry.shape.clone(),
        Resolved::Face { index, .. } => {
            if prev.surface_of_face(index).is_none() {
                return Err("the replacement must be a surface".into());
            }
            prev.face_shape(index)?
        }
        _ => return Err("select a surface body to replace the faces with".into()),
    };
    // A single face can be extended to reach past the old face's neighbours.
    let surface = if surface.count(Kind::Face) == 1 {
        surface
            .sub_shape(Kind::Face, 0)
            .and_then(|f| f.extend_face(reach(prev)))
            .map_err(geo)?
    } else {
        surface
    };
    each_face(prev, state, &p.faces, |_, body, face| {
        replace(fid, body, face, &surface)
    })
}

pub(crate) fn delete_hole(
    fid: FeatureId,
    p: &DeleteHole,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let mut by_body: Vec<(usize, Vec<u32>)> = Vec::new();
    let whole = &prev.everything().ok_or("there's no body")?.shape;
    for r in &p.faces {
        let (Kind::Face, global) = prev.any_index_of(r)? else {
            return Err("select faces".into());
        };
        let s = prev
            .surface_of_face(global)
            .ok_or("delete hole works on surface faces; use delete face on solids")?;
        let ix = prev.surfaces[s]
            .geometry
            .shape
            .indices_in(whole, Kind::Face)
            .map_err(geo)?;
        let local = ix.iter().position(|g| *g == global).unwrap() as u32;
        match by_body.iter_mut().find(|(b, _)| *b == s) {
            Some((_, faces)) => faces.push(local),
            None => by_body.push((s, vec![local])),
        }
    }
    for (s, faces) in by_body {
        let geometry = &prev.surfaces[s].geometry;
        let r = geometry.shape.remove_holes(&faces).map_err(geo)?;
        let names = Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&geometry.names), None],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        state.surfaces[s].geometry = Body {
            shape: r.shape,
            names,
        };
    }
    Ok(())
}
