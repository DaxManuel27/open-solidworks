//! Unfold and Fold, and normal cuts.
//!
//! Unfold flattens a body and remembers how. Fold finds what the features in
//! between cut away from the flat, takes the cut-outs' outlines on the flat's
//! front face, maps them back onto the formed faces through the inverse of
//! each face's flat map, and cuts through the sheet there, square to it. A
//! normal cut does the same with a sketch projected onto the formed sheet.

use of_geom::{BoolOp, FaceSurface, Kind, Plane, Shape, UvMap};

use super::unfold::{Layout, layout};
use super::{RResult, base_name, flatten, geo, record_body, sketch_of};
use crate::FeatureId;
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::sheet_metal::{Fold, NormalCut, Unfold};

/// A body flattened by Unfold.
#[derive(Debug, Clone)]
pub(crate) struct Unfolded {
    pub record: usize,
    pub formed: Body,
    pub flat: Shape,
    pub faces: Vec<(u32, UvMap)>,
    pub plane: Plane,
}

pub(crate) fn unfold(fid: FeatureId, p: &Unfold, prev: &State, state: &mut State) -> RResult<()> {
    if prev.unfolded.is_some() {
        return Err("fold the last unfolded body before unfolding again".into());
    }
    let (record, b, fixed) = match &p.fixed_face {
        Some(r) => {
            let (Kind::Face, global) = prev.index_of(r)? else {
                return Err("select a face to stay fixed".into());
            };
            let whole = &prev.body.as_ref().ok_or("there's no body")?.shape;
            let found = prev.sheet_metal.iter().enumerate().find_map(|(i, rec)| {
                let b = record_body(prev, rec)?;
                let ix = prev.bodies[b]
                    .geometry
                    .shape
                    .indices_in(whole, Kind::Face)
                    .ok()?;
                let local = ix.iter().position(|g| *g == global)?;
                Some((i, b, Some(local as u32)))
            });
            found.ok_or("select a face of a sheet metal body")?
        }
        None => {
            let rec = prev
                .sheet_metal
                .iter()
                .position(|r| record_body(prev, r).is_some())
                .ok_or("the part has no sheet metal body")?;
            (
                rec,
                record_body(prev, &prev.sheet_metal[rec]).unwrap(),
                None,
            )
        }
    };
    let formed = prev.bodies[b].geometry.clone();
    let record_data = &prev.sheet_metal[record];
    let Layout { plane, faces, .. } = layout(&formed, record_data, fixed)?;
    let flat = flatten(&formed, record_data, fixed)?;
    let _ = fid;
    state.bodies[b].geometry = Body {
        shape: flat.shape.clone(),
        names: flat.names,
    };
    state.unfolded = Some(Unfolded {
        record,
        formed,
        flat: flat.shape,
        faces,
        plane,
    });
    state.aggregate_bodies()
}

/// Solids that cut through the sheet under each mapped face, from the face
/// on the fixed side into the material.
fn through_tools(formed: &Shape, pieces: Vec<(Shape, u32)>, t: f64) -> RResult<Vec<Shape>> {
    let mut tools = Vec::new();
    for (piece, owner) in pieces {
        let frame = formed.face_frame(owner).map_err(geo)?;
        let tool = match frame.surface {
            FaceSurface::Plane => {
                let n = frame.normal(0.0, 0.0);
                // From just outside the face, through the sheet and past it.
                let start = piece
                    .transform(&of_geom::DAffine3::from_translation(n * (0.01 * t)))
                    .map_err(geo)?
                    .shape;
                start.prism(-n * (1.02 * t)).map_err(geo)?.shape
            }
            FaceSurface::Cylinder { .. } => {
                let mid_u = (frame.u.0 + frame.u.1) / 2.0;
                let p = frame.point(mid_u, (frame.v.0 + frame.v.1) / 2.0);
                let radial = (p - (frame.origin + frame.z * frame.height_of(p))).normalize();
                let concave = frame.normal(mid_u, 0.0).dot(radial) < 0.0;
                piece.wrap_solid(t * 1.001, concave).map_err(geo)?
            }
            _ => return Err("a cut crosses a face that can't be folded".into()),
        };
        tools.push(tool);
    }
    Ok(tools)
}

/// Cut each tool from the body in turn: touching tools in one compound
/// confuse the boolean.
fn cut_with(fid: FeatureId, body: &Body, tools: &[Shape]) -> RResult<Body> {
    let mut body = body.clone();
    for tool in tools {
        body = cut_one(fid, &body, tool)?;
    }
    Ok(body)
}

fn cut_one(fid: FeatureId, body: &Body, tool: &Shape) -> RResult<Body> {
    let r = body.shape.boolean(BoolOp::Cut, tool).map_err(geo)?;
    let mut body = Body {
        names: Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&body.names), None],
            },
            &mut |_| Some(format!("{fid}.cut")),
            &fid.to_string(),
        ),
        shape: r.shape,
    };
    let solids = body.shape.solids();
    if solids.len() > 1 {
        // Neighbouring tool pieces can leave slivers between them: drop
        // solids far smaller than the sheet.
        let volumes: Vec<f64> = solids
            .iter()
            .map(|s| s.mass_props().map(|m| m.volume.abs()).unwrap_or(0.0))
            .collect();
        let largest = volumes.iter().copied().fold(0.0, f64::max);
        let slivers: Vec<&Shape> = solids
            .iter()
            .zip(&volumes)
            .filter(|(_, v)| **v < largest * 1e-6)
            .map(|(s, _)| s)
            .collect();
        if slivers.len() + 1 != solids.len() {
            return Err("the cut splits the sheet apart".into());
        }
        let r = body
            .shape
            .boolean(BoolOp::Cut, &Shape::compound(slivers))
            .map_err(geo)?;
        body = Body {
            names: Naming::from_history(
                &r.shape,
                &r.history,
                Inputs {
                    named: [Some(&body.names), None],
                },
                &mut |_| None,
                &fid.to_string(),
            ),
            shape: r.shape,
        };
    }
    Ok(body)
}

pub(crate) fn fold(fid: FeatureId, _p: &Fold, prev: &State, state: &mut State) -> RResult<()> {
    let u = prev
        .unfolded
        .as_ref()
        .ok_or("there's no unfolded body to fold; add an Unfold first")?;
    let record = &prev.sheet_metal[u.record];
    let b = record_body(prev, record).ok_or("the unfolded body no longer exists")?;
    let current = &prev.bodies[b].geometry.shape;
    let t = record.params.thickness;
    let added = current.boolean(BoolOp::Cut, &u.flat).map_err(geo)?;
    if added.shape.count(Kind::Solid) > 0
        && added.shape.mass_props().map_err(geo)?.volume.abs() > 1e-6
    {
        return Err("only cuts made while the body is unfolded can be folded back".into());
    }
    let removed = u.flat.boolean(BoolOp::Cut, current).map_err(geo)?.shape;
    // The cut-outs' outlines on the flat's front face.
    let mut outline = Vec::new();
    for f in 0..removed.count(Kind::Face) {
        let info = removed.face_info(f).map_err(geo)?;
        if info.kind == of_geom::SurfaceKind::Plane
            && info.normal.cross(u.plane.normal).length() < 1e-9
            && u.plane.distance(info.point).abs() < 1e-6
        {
            outline.push(removed.sub_shape(Kind::Face, f).map_err(geo)?);
        }
    }
    let mut formed = u.formed.clone();
    if !outline.is_empty() {
        let region = Shape::compound(outline.iter());
        let pieces = formed
            .shape
            .fold(&u.faces, &u.plane, &region)
            .map_err(|e| format!("can't fold the cuts back: {}", e.0))?;
        let tools = through_tools(&formed.shape, pieces, t)?;
        formed = cut_with(fid, &formed, &tools)?;
    }
    state.bodies[b].geometry = formed;
    state.unfolded = None;
    state.aggregate_bodies()
}

pub(crate) fn normal_cut(
    fid: FeatureId,
    p: &NormalCut,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (plane, sk) = sketch_of(prev, p.sketch)?;
    let contours = sk.contours().map_err(geo)?;
    let regions = super::super::closed_regions(&contours)?;
    let face = of_geom::Profile {
        plane: *plane,
        faces: regions,
    }
    .to_shape()
    .map_err(geo)?;
    let long = super::super::through_all_length(prev, plane);
    let start = face
        .transform(&of_geom::DAffine3::from_translation(-plane.normal * long))
        .map_err(geo)?
        .shape;
    let prism = start.prism(plane.normal * (2.0 * long)).map_err(geo)?.shape;
    let mut hit = false;
    for (i, record) in prev.sheet_metal.iter().enumerate() {
        let Some(b) = record_body(prev, record) else {
            continue;
        };
        let body = &prev.bodies[b].geometry;
        let Layout { faces, .. } = layout(body, record, None)?;
        let mut pieces = Vec::new();
        for (f, _) in &faces {
            let face = body.shape.sub_shape(Kind::Face, *f).map_err(geo)?;
            let common = prism.boolean(BoolOp::Common, &face).map_err(geo)?.shape;
            for k in 0..common.count(Kind::Face) {
                pieces.push((common.sub_shape(Kind::Face, k).map_err(geo)?, *f));
            }
        }
        let tools = through_tools(&body.shape, pieces, record.params.thickness)?;
        if !tools.is_empty() {
            state.bodies[b].geometry = cut_with(fid, body, &tools)?;
            hit = true;
        }
        let _ = (i, base_name);
    }
    if !hit {
        return Err(super::super::CUT_REMOVES_NOTHING.into());
    }
    state.aggregate_bodies()
}

/// Cut regions drawn in the flat (planar faces in the layout's plane) out of
/// the formed body, square through the sheet.
pub(crate) fn cut_flat_regions(
    fid: FeatureId,
    body: &Body,
    layout: &Layout,
    t: f64,
    regions: &[Shape],
) -> RResult<Body> {
    if regions.is_empty() {
        return Ok(body.clone());
    }
    let region = Shape::compound(regions.iter());
    let pieces = body
        .shape
        .fold(&layout.faces, &layout.plane, &region)
        .map_err(|e| format!("can't map the cut onto the sheet: {}", e.0))?;
    let tools = through_tools(&body.shape, pieces, t)?;
    if tools.is_empty() {
        return Err(super::super::CUT_REMOVES_NOTHING.into());
    }
    cut_with(fid, body, &tools)
}
