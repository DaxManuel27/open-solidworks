//! Wrap: closed sketch regions mapped onto a curved face by true arc length,
//! then embossed, debossed or scribed into the face.

use of_geom::{BoolOp, Kind, Profile};

use super::{RResult, apply_feature_tool, body_of, closed_regions, dressup_names, geo, sketch_of};
use crate::build::{Body, State};
use crate::naming::Naming;
use crate::{FeatureId, Wrap, WrapMode};

pub(super) fn wrap(fid: FeatureId, p: &Wrap, prev: &State, state: &mut State) -> RResult<()> {
    let body = body_of(prev)?;
    let (kind, face) = prev.index_of(&p.face)?;
    if kind != Kind::Face {
        return Err("select a curved face to wrap onto".into());
    }
    let (plane, sk) = sketch_of(prev, p.sketch)?;
    let contours = sk.contours().map_err(geo)?;
    let profile = Profile {
        plane: *plane,
        faces: closed_regions(&contours)?,
    };
    let wrapped = profile.wrap_onto(&body.shape, face).map_err(geo)?;
    let (outward, op) = match p.mode {
        WrapMode::Emboss => (true, BoolOp::Fuse),
        WrapMode::Deboss => (false, BoolOp::Cut),
        WrapMode::Scribe => return scribe(fid, &wrapped, prev, state),
    };
    if !(p.depth.is_finite() && p.depth > 1e-9) {
        return Err("the wrap depth must be more than zero".into());
    }
    let tool = wrapped.wrap_solid(p.depth, outward).map_err(geo)?;
    let names = Naming::by_index(&tool, &format!("{fid}.wrap"));
    apply_feature_tool(fid, state, tool, names, op, true)
}

/// Split the target face along the wrapped outline; material is unchanged.
fn scribe(
    fid: FeatureId,
    wrapped: &of_geom::Shape,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let body = body_of(prev)?;
    let r = body.shape.split_tools(wrapped).map_err(geo)?;
    if r.shape.count(Kind::Face) <= body.shape.count(Kind::Face) {
        return Err("the wrapped outline does not divide the face".into());
    }
    let volume = |s: &of_geom::Shape| s.mass_props().map(|m| m.volume).map_err(geo);
    let (before, after) = (volume(&body.shape)?, volume(&r.shape)?);
    if (after - before).abs() > 1e-7 * before.abs().max(1.0) {
        return Err("scribing changed the part's volume".into());
    }
    let names = dressup_names(fid, "wrap", body, &r);
    state.body = Some(Body {
        shape: r.shape,
        names,
    });
    Ok(())
}
