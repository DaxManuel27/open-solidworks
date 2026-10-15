//! Hems: edges folded back on themselves, built as a chain of folds and legs
//! from the edge and extruded along it.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use of_geom::DVec2;

use super::{
    FlangeFrame, RResult, Seg, apply_flanges, bend_cuts, corner_trims, flange_frame, flange_prism,
    round_corners, thickened,
};
use crate::FeatureId;
use crate::build::State;
use crate::sheet_metal::{BendRecord, Hem, HemKind, HemPosition};

/// The smallest inside radius a closed hem is modelled with, as a fraction
/// of the thickness. A fold of zero radius has no solid.
const CLOSED: f64 = 0.01;

/// A counter-clockwise or clockwise fold of `angle` starting at `from`
/// heading at angle `heading`, with the reference (inner, Y = 0) face's
/// radius `r`.
fn fold(from: DVec2, heading: f64, r: f64, angle: f64) -> Seg {
    let side = angle.signum();
    let normal = DVec2::from_angle(heading + side * FRAC_PI_2);
    let c = from + normal * r;
    Seg::Arc {
        c,
        r,
        a0: (from - c).to_angle(),
        sweep: angle,
    }
}

/// The hem's chain from `x` along the reference face (material below), and
/// the names of its pieces.
fn chain(kind: &HemKind, length: f64, x: f64, t: f64) -> RResult<Vec<(Seg, String)>> {
    let start = DVec2::new(x, 0.0);
    let mut out = Vec::new();
    let leg = |from: DVec2, heading: f64, length: f64| Seg::Line {
        a: from,
        b: from + DVec2::from_angle(heading) * length,
    };
    match *kind {
        HemKind::Closed | HemKind::Open { .. } => {
            let r = match *kind {
                HemKind::Open { gap } => gap / 2.0,
                _ => (CLOSED * t).max(0.005),
            };
            let f = fold(start, 0.0, r, PI);
            out.push((f, "h0".into()));
            if length > 1e-9 {
                out.push((leg(f.end(), PI, length), "l0".into()));
            }
        }
        HemKind::Double { gap } => {
            if length <= 1e-9 {
                return Err("a double hem needs a length".into());
            }
            let r = (gap / 2.0).max((CLOSED * t).max(0.005));
            let f = fold(start, 0.0, r, PI);
            let l = leg(f.end(), PI, length);
            // The second fold turns the other way, so the reference face is
            // its outside: its radius is the inside radius plus the thickness.
            let g = fold(l.end(), PI, r + t, -PI);
            let l2 = leg(g.end(), 0.0, length);
            out.extend([
                (f, "h0".to_string()),
                (l, "l0".to_string()),
                (g, "h1".to_string()),
                (l2, "l1".to_string()),
            ]);
        }
        HemKind::TearDrop { angle, radius } => {
            if !(angle > PI + 1e-6 && angle < 1.5 * PI - 1e-6) {
                return Err("a tear drop hem's angle must be between 180° and 270°".into());
            }
            let f = fold(start, 0.0, radius, angle);
            out.push((f, "h0".into()));
            // Down to the sheet: the leg ends when its reference face reaches it.
            let end = f.end();
            let reach = end.y / -angle.sin();
            if reach > 1e-9 {
                out.push((leg(end, angle, reach), "l0".into()));
            }
        }
        HemKind::Rolled { angle, radius } => {
            if !(angle > 1e-6 && angle < TAU - 1e-6) {
                return Err("a rolled hem's angle must be between 0° and 360°".into());
            }
            out.push((fold(start, 0.0, radius, angle), "h0".into()));
        }
    }
    Ok(out)
}

/// How far the outside of the first fold reaches past its start.
fn reach(kind: &HemKind, t: f64) -> f64 {
    match *kind {
        HemKind::Closed => (CLOSED * t).max(0.005) + t,
        HemKind::Open { gap } => gap / 2.0 + t,
        HemKind::Double { gap } => (gap / 2.0).max((CLOSED * t).max(0.005)) + t,
        HemKind::TearDrop { radius, .. } | HemKind::Rolled { radius, .. } => radius + t,
    }
}

pub(in crate::ops) fn hem(fid: FeatureId, p: &Hem, prev: &State, state: &mut State) -> RResult<()> {
    let frames: Vec<FlangeFrame> = p
        .edges
        .iter()
        .map(|r| flange_frame(prev, r, p.reverse))
        .collect::<RResult<_>>()?;
    let mut cuts = Vec::new();
    let mut tools = Vec::new();
    let mut bends = Vec::new();
    for (k, f) in frames.iter().enumerate() {
        let record = &prev.sheet_metal[f.record];
        let t = f.thickness;
        let x = match p.position {
            HemPosition::MaterialInside => -reach(&p.kind, t),
            HemPosition::BendOutside => 0.0,
        };
        let mut pieces = chain(&p.kind, p.length, x, t)?;
        if x > 1e-9 {
            pieces.insert(
                0,
                (
                    Seg::Line {
                        a: DVec2::ZERO,
                        b: DVec2::new(x, 0.0),
                    },
                    "ext".into(),
                ),
            );
        }
        let (segs, names): (Vec<Seg>, Vec<String>) = pieces.into_iter().unzip();
        let elements = round_corners(&segs, &names, record.params.radius, -t, 0.0)?;
        let prefix = format!("e{k}.");
        let tool = flange_prism(
            fid,
            &prefix,
            f,
            &thickened(&elements, -t, 0.0),
            0.0,
            f.length,
        )?;
        tools.push(corner_trims(fid, tool, k, &frames, p.gap)?);
        cuts.extend(bend_cuts(
            fid,
            &prefix,
            f,
            x,
            0.0,
            f.length,
            record.params.relief,
        )?);
        let allowance = p
            .allowance
            .clone()
            .unwrap_or(record.params.allowance.clone());
        for (i, e) in elements.iter().filter(|e| e.bend.is_some()).enumerate() {
            let (radius, angle) = e.bend.unwrap();
            bends.push((
                f.record,
                BendRecord {
                    feature: fid,
                    index: (k * 4 + i) as u32,
                    faces: [
                        format!("{fid}.{prefix}{}.a", e.name),
                        format!("{fid}.{prefix}{}.b", e.name),
                    ],
                    radius,
                    angle,
                    allowance: allowance.clone(),
                },
            ));
        }
    }
    apply_flanges(fid, state, cuts, tools, bends)
}
