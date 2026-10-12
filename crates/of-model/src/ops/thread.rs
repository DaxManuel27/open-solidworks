//! Modeled threads: an ISO-profile helical groove cut into a cylindrical face.

use of_geom::{BoolOp, Plane, Shape};

use super::{RResult, apply_feature_tool, geo};
use crate::build::State;
use crate::naming::Naming;
use crate::{FeatureId, Thread};

pub(super) fn thread(fid: FeatureId, p: &Thread, prev: &State, state: &mut State) -> RResult<()> {
    // The cosmetic resolver validates the cylinder and fixes the start, axis and phase.
    let at = crate::cosmetic_thread::annotation(&p.cosmetic(), prev)?;
    let phase = (at.points[0] - at.start).normalize();
    let frame = Plane::new(at.start, phase, at.axis);
    let tool = Shape::thread_tool(
        &frame,
        p.diameter / 2.0,
        p.pitch,
        p.length,
        p.left_handed,
        p.internal,
    )
    .map_err(geo)?;
    let names = Naming::by_index(&tool, &format!("{fid}.thread"));
    apply_feature_tool(fid, state, tool, names, BoolOp::Cut, true)
}
