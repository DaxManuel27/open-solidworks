//! Sketched bends and jogs: the sheet is cut at a sketched line, the far side
//! is moved to where the bends put it, and the bend region is added between.
//!
//! The far side is cut off where it starts in the flat (one bend allowance,
//! or the jog's flat length, past the near side), so flattening gives back
//! the original sheet.

use std::f64::consts::PI;

use of_geom::{BoolOp, DAffine3, DMat3, DVec2, DVec3, Kind, Plane, Shape};

use super::{
    FlangeFrame, RResult, Seg, flange_prism, geo, has_name, record_body, round_corners, thickened,
};
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::resolve::resolve;
use crate::sheet_metal::{
    BendOverride, BendPosition, BendRecord, Jog, JogDimension, SketchedBend, bend_allowance,
};
use crate::{FeatureId, Ref};

/// A sheet metal body cut by a sketched line.
struct Cut {
    frame: FlangeFrame,
    body: usize,
    record: usize,
    /// Extent of the sheet along the line.
    z: (f64, f64),
}

impl FlangeFrame {
    /// The 3D motion that applies a 2D affine map to (x, y) and keeps z.
    fn affine(&self, a: [f64; 6]) -> DAffine3 {
        let basis = DMat3::from_cols(self.x, self.y, self.z);
        let to_local =
            DAffine3::from_mat3(basis.transpose()) * DAffine3::from_translation(-self.origin);
        let map = DAffine3::from_mat3_translation(
            DMat3::from_cols(
                DVec3::new(a[0], a[3], 0.0),
                DVec3::new(a[1], a[4], 0.0),
                DVec3::Z,
            ),
            DVec3::new(a[2], a[5], 0.0),
        );
        DAffine3::from_translation(self.origin) * DAffine3::from_mat3(basis) * map * to_local
    }
}

/// The part of `body` on the side of `plane` its normal points to.
fn side(fid: FeatureId, body: &Body, plane: &Plane) -> RResult<Option<Body>> {
    let half = Shape::half_space(
        &Shape::plane_face(plane).map_err(geo)?,
        plane.origin + plane.normal,
    )
    .map_err(geo)?;
    let r = body.shape.boolean(BoolOp::Common, &half).map_err(geo)?;
    if r.shape.count(Kind::Solid) == 0 {
        return Ok(None);
    }
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(&body.names), None],
        },
        &mut |_| Some(format!("{fid}.split")),
        &fid.to_string(),
    );
    Ok(Some(Body {
        shape: r.shape,
        names,
    }))
}

fn volume(b: &Option<Body>) -> f64 {
    b.as_ref()
        .and_then(|b| b.shape.mass_props().ok())
        .map_or(0.0, |m| m.volume.abs())
}

/// Set up the cut along the line of `sketch`, which must lie on a flat face
/// of a sheet metal body.
fn cut(prev: &State, sketch: FeatureId, fixed: Option<&Ref>, reverse: bool) -> RResult<Cut> {
    let (plane, sk) = super::sketch_of(prev, sketch)?;
    let contours = sk.contours().map_err(geo)?;
    let line = match (contours.loops.as_slice(), contours.open.as_slice()) {
        ([], [chain]) if chain.curves.len() == 1 => match &chain.curves[0] {
            of_sketch::Curve2::Line { a, b } => (plane.to_world(*a), plane.to_world(*b)),
            _ => return Err("the bend sketch must be a single straight line".into()),
        },
        _ => return Err("the bend sketch must be a single straight line".into()),
    };
    // The sheet metal body with a flat face on the sketch plane.
    let mut found = None;
    'bodies: for (r, record) in prev.sheet_metal.iter().enumerate() {
        let Some(b) = record_body(prev, record) else {
            continue;
        };
        let shape = &prev.bodies[b].geometry.shape;
        for f in 0..shape.count(Kind::Face) {
            let info = shape.face_info(f).map_err(geo)?;
            if info.kind == of_geom::SurfaceKind::Plane
                && info.normal.cross(plane.normal).length() < 1e-9
                && plane.distance(info.point).abs() < 1e-6
            {
                found = Some((r, b, info.normal));
                break 'bodies;
            }
        }
    }
    let (record, body, n) = found.ok_or("sketch the line on a flat face of a sheet metal body")?;
    let t = prev.sheet_metal[record].params.thickness;
    let geometry = &prev.bodies[body].geometry;
    let e = (line.1 - line.0).normalize();
    let mut w = e.cross(n).normalize();
    // The side that stays: the one with the fixed face, else the larger one.
    let towards = match fixed {
        Some(f) => {
            let c = match resolve(prev, f)? {
                crate::resolve::Resolved::Face { info, .. } => info.centroid,
                _ => return Err("select a face on the side that stays".into()),
            };
            (c - line.0).dot(w)
        }
        None => {
            let ahead = side(0, geometry, &Plane::new(line.0, e, w))?;
            let behind = side(0, geometry, &Plane::new(line.0, e, -w))?;
            volume(&ahead) - volume(&behind)
        }
    };
    if towards > 0.0 {
        w = -w;
    }
    let (y, origin) = if reverse {
        (-n, line.0 - n * t)
    } else {
        (n, line.0)
    };
    let mut frame = FlangeFrame {
        origin,
        x: w,
        y,
        z: e,
        length: (line.1 - line.0).length(),
        thickness: t,
        record,
        ends: [line.0, line.1],
    };
    // Where the sheet runs along the line.
    let section = geometry
        .shape
        .section(&Shape::plane_face(&Plane::new(origin, e, w)).map_err(geo)?)
        .map_err(geo)?;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..section.count(Kind::Edge) {
        let info = section.edge_info(i).map_err(geo)?;
        for p in [info.start, info.end] {
            let (_, yy, zz) = frame.local(p);
            if yy > 1e-6 || yy < -t - 1e-6 {
                return Err("the line must cross a flat part of the sheet".into());
            }
            lo = lo.min(zz);
            hi = hi.max(zz);
        }
    }
    if !(hi - lo > 1e-6) {
        return Err("the line does not cross the sheet".into());
    }
    frame.length = hi;
    Ok(Cut {
        frame,
        body,
        record,
        z: (lo, hi),
    })
}

/// Where the first bend starts, from the line, in the flat.
fn start(position: BendPosition, theta: f64, r: f64, t: f64, ba: f64) -> f64 {
    let (s, c) = (theta.sin(), theta.cos());
    match position {
        BendPosition::Centerline => -ba / 2.0,
        BendPosition::MaterialInside => (r * c - r - t) / s,
        BendPosition::MaterialOutside => -(r * (1.0 - c) - t * c) / s,
        BendPosition::BendOutside => 0.0,
    }
}

/// Replace the sheet between `xa` and `xb` (flat coordinates) with the bend
/// chain `segs`, moving the far side by `motion`.
#[allow(clippy::too_many_arguments)]
fn rebuild(
    fid: FeatureId,
    prev: &State,
    state: &mut State,
    c: &Cut,
    xa: f64,
    xb: f64,
    segs: Vec<(Seg, String)>,
    motion: [f64; 6],
    radius: f64,
    allowance: crate::sheet_metal::Allowance,
) -> RResult<()> {
    let f = &c.frame;
    let t = f.thickness;
    let body = &prev.bodies[c.body].geometry;
    let plane_at = |x: f64, keep_below: bool| {
        Plane::new(
            f.world(x, 0.0, 0.0),
            f.z,
            if keep_below { -f.x } else { f.x },
        )
    };
    let near =
        side(fid, body, &plane_at(xa, true))?.ok_or("the bend leaves nothing on the fixed side")?;
    let far = side(fid, body, &plane_at(xb, false))?
        .ok_or("the bend leaves nothing on the moving side")?;
    let r = far.shape.transform(&f.affine(motion)).map_err(geo)?;
    let far = Body {
        names: Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&far.names), None],
            },
            &mut |_| None,
            &fid.to_string(),
        ),
        shape: r.shape,
    };
    let (segs, names): (Vec<Seg>, Vec<String>) = segs.into_iter().unzip();
    let elements = round_corners(&segs, &names, radius, -t, 0.0)?;
    let (shape, names) = flange_prism(fid, "", f, &thickened(&elements, -t, 0.0), c.z.0, c.z.1)?;
    let mut joined = Body { shape, names };
    for part in [near, far] {
        let r = joined
            .shape
            .boolean(BoolOp::Fuse, &part.shape)
            .map_err(geo)?;
        joined = Body {
            names: Naming::from_history(
                &r.shape,
                &r.history,
                Inputs {
                    named: [Some(&joined.names), Some(&part.names)],
                },
                &mut |_| None,
                &fid.to_string(),
            ),
            shape: r.shape,
        };
    }
    if joined.shape.count(Kind::Solid) != 1 {
        return Err("the bend splits the sheet apart".into());
    }
    state.bodies[c.body].geometry = joined;
    for (i, e) in elements.iter().filter(|e| e.bend.is_some()).enumerate() {
        let (radius, angle) = e.bend.unwrap();
        state.sheet_metal[c.record].bends.push(BendRecord {
            feature: fid,
            index: i as u32,
            faces: [format!("{fid}.{}.a", e.name), format!("{fid}.{}.b", e.name)],
            radius,
            angle,
            allowance: allowance.clone(),
        });
    }
    // The fixed face must still be on the body.
    let record = &state.sheet_metal[c.record];
    let geometry = &state.bodies[c.body].geometry;
    if !geometry
        .names
        .faces
        .iter()
        .any(|n| has_name(n, &record.fixed_face))
    {
        return Err("the bend removes the sheet's fixed face".into());
    }
    state.aggregate_bodies()
}

fn bend_values(
    prev: &State,
    record: usize,
    bend: &BendOverride,
    theta: f64,
) -> RResult<(f64, crate::sheet_metal::Allowance, f64)> {
    let params = &prev.sheet_metal[record].params;
    let r = bend.radius.unwrap_or(params.radius);
    let allowance = bend.allowance.clone().unwrap_or(params.allowance.clone());
    let ba = bend_allowance(&allowance, theta, r, params.thickness)?;
    Ok((r, allowance, ba))
}

fn arc(from: DVec2, heading: f64, r: f64, angle: f64) -> Seg {
    let normal = DVec2::from_angle(heading + angle.signum() * PI / 2.0);
    let c = from + normal * r;
    Seg::Arc {
        c,
        r,
        a0: (from - c).to_angle(),
        sweep: angle,
    }
}

pub(crate) fn sketched_bend(
    fid: FeatureId,
    p: &SketchedBend,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let theta = p.angle;
    if !(theta > 1e-6 && theta < PI - 1e-6) {
        return Err("the bend angle must be between 0° and 180°".into());
    }
    let c = cut(prev, p.sketch, p.fixed.as_ref(), p.reverse)?;
    let t = c.frame.thickness;
    let (r, allowance, ba) = bend_values(prev, c.record, &p.bend, theta)?;
    let xa = start(p.position, theta, r, t, ba);
    let xb = xa + ba;
    let bend = arc(DVec2::new(xa, 0.0), 0.0, r, theta);
    let end = bend.end();
    let (s, co) = (theta.sin(), theta.cos());
    let motion = [co, -s, end.x - co * xb, s, co, end.y - s * xb];
    rebuild(
        fid,
        prev,
        state,
        &c,
        xa,
        xb,
        vec![(bend, "b0".into())],
        motion,
        r,
        allowance,
    )
}

pub(crate) fn jog(fid: FeatureId, p: &Jog, prev: &State, state: &mut State) -> RResult<()> {
    let theta = p.angle;
    if !(theta > 1e-6 && theta < PI - 1e-6) {
        return Err("the jog angle must be between 0° and 180°".into());
    }
    let c = cut(prev, p.sketch, p.fixed.as_ref(), p.reverse)?;
    let t = c.frame.thickness;
    let (r, allowance, ba) = bend_values(prev, c.record, &p.bend, theta)?;
    // The rise of the reference (upper) face from one wall to the other.
    let rise = match p.dimension {
        JogDimension::Outside => p.offset,
        JogDimension::Inside => p.offset + t,
        JogDimension::Overall => p.offset - t,
    };
    let (s, co) = (theta.sin(), theta.cos());
    let leg = (rise - (2.0 * r + t) * (1.0 - co)) / s;
    if leg < -1e-9 {
        return Err("the offset is too small for two bends of this radius".into());
    }
    let leg = leg.max(0.0);
    let xa = start(p.position, theta, r, t, ba);
    let first = arc(DVec2::new(xa, 0.0), 0.0, r, theta);
    let mut segs = vec![(first, "b0".to_string())];
    let mut at = first.end();
    if leg > 1e-9 {
        let l = Seg::Line {
            a: at,
            b: at + DVec2::from_angle(theta) * leg,
        };
        at = l.end();
        segs.push((l, "l0".into()));
    }
    let second = arc(at, theta, r + t, -theta);
    let end = second.end();
    segs.push((second, "b1".into()));
    let flat = 2.0 * ba + leg;
    let projected = end.x - xa;
    // The far side starts after the jog's flat length, or where the jog ends
    // in plan when the projected length is kept.
    let xb = if p.fix_projected_length {
        xa + projected
    } else {
        xa + flat
    };
    let motion = [1.0, 0.0, end.x - xb, 0.0, 1.0, end.y];
    rebuild(fid, prev, state, &c, xa, xb, segs, motion, r, allowance)
}
