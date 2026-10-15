//! Swept flanges: an open profile, thickened with bends at its corners,
//! swept along a path.

use of_geom::{BoolOp, FaceSurface, Kind};

use super::{
    RResult, apply_tool, check_params, geo, named_region, oriented_chain, round_corners, sketch_of,
    thickened,
};
use crate::FeatureId;
use crate::build::State;
use crate::sheet_metal::{BendRecord, SheetMetalBody, SweptFlange, ThicknessSide};

pub(in crate::ops) fn swept_flange(
    fid: FeatureId,
    p: &SweptFlange,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (plane, sk) = sketch_of(prev, p.profile)?;
    let contours = sk.contours().map_err(geo)?;
    let chain = match (contours.loops.as_slice(), contours.open.as_slice()) {
        ([], [chain]) => chain,
        _ => return Err("a swept flange profile must be one open chain of lines and arcs".into()),
    };
    let path = crate::curves::path(prev, &p.path)?;
    // Join an existing sheet metal body the profile touches, else start one.
    let section_probe = {
        let segs = oriented_chain(&chain.curves)?;
        plane.to_world(segs[0].start())
    };
    let joins = prev.sheet_metal.iter().position(|r| {
        super::record_body(prev, r).is_some_and(|b| {
            let shape = &prev.bodies[b].geometry.shape;
            of_geom::Shape::make_box(1e-3, 1e-3, 1e-3)
                .and_then(|s| {
                    s.transform(&of_geom::DAffine3::from_translation(section_probe - 5e-4))
                })
                .and_then(|probe| shape.distance(&probe.shape))
                .is_ok_and(|d| d.value < 1e-3)
        })
    });
    let params = match (joins, &p.sheet) {
        (Some(r), _) => prev.sheet_metal[r].params.clone(),
        (None, Some(sheet)) => check_params(sheet, prev.material.as_deref())?,
        (None, None) => {
            return Err("a swept flange that starts a body needs sheet metal values".into());
        }
    };
    let t = params.thickness;
    let r = p.bend.radius.unwrap_or(params.radius);
    let (lo, hi) = match p.side {
        ThicknessSide::Normal => (0.0, t),
        ThicknessSide::Reverse => (-t, 0.0),
        ThicknessSide::MidPlane => (-t / 2.0, t / 2.0),
    };
    let segs = oriented_chain(&chain.curves)?;
    let names: Vec<String> = chain.entities.iter().map(|e| format!("s{e}")).collect();
    let elements = round_corners(&segs, &names, r, lo, hi)?;
    let region = named_region(*plane, &thickened(&elements, lo, hi))?;
    let body = crate::ops::sweep_section(
        fid,
        &region.shape,
        plane,
        &region.edge_names,
        &path,
        &crate::ops::SweepFrame::Mode(0),
        "",
    )?;
    if !body.shape.is_valid() || body.shape.count(Kind::Solid) == 0 {
        return Err("the profile can't be swept along this path".into());
    }
    apply_tool(
        fid,
        state,
        body.shape,
        body.names,
        BoolOp::Fuse,
        joins.is_some(),
    )?;
    let allowance = p.bend.allowance.clone().unwrap_or(params.allowance.clone());
    let bends: Vec<BendRecord> = elements
        .iter()
        .filter_map(|e| e.bend.map(|b| (e, b)))
        .enumerate()
        .map(|(i, (e, (radius, angle)))| BendRecord {
            feature: fid,
            index: i as u32,
            faces: [format!("{fid}.{}.a", e.name), format!("{fid}.{}.b", e.name)],
            radius,
            angle,
            allowance: allowance.clone(),
        })
        .collect();
    match joins {
        Some(r) => state.sheet_metal[r].bends.extend(bends),
        None => {
            // The fixed face: the first flat face swept from a straight part.
            let line = elements
                .iter()
                .find(|e| e.bend.is_none())
                .ok_or("the profile needs a straight part")?;
            let side = if p.side == ThicknessSide::Reverse {
                "a"
            } else {
                "b"
            };
            let base = format!("{fid}.{}.{side}", line.name);
            let whole = state.body.as_ref().unwrap();
            let fixed = (0..whole.shape.count(Kind::Face))
                .find_map(|i| {
                    let n = whole.names.face_name(i)?;
                    (super::base_name(n) == base
                        && whole
                            .shape
                            .face_frame(i)
                            .is_ok_and(|f| f.surface == FaceSurface::Plane))
                    .then(|| n.to_string())
                })
                .ok_or("the swept flange has no flat face to keep fixed")?;
            state.sheet_metal.push(SheetMetalBody {
                feature: fid,
                fixed_face: fixed,
                params,
                bends,
                bend_lines: Vec::new(),
                cross_breaks: Vec::new(),
                facets: Vec::new(),
            });
        }
    }
    Ok(())
}
