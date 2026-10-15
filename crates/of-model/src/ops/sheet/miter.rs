//! Miter flanges: one open profile swept along a chain of edges, with a bend
//! from the sheet into the profile and at each sharp corner of it, mitered
//! where the edges meet.

use of_geom::{DVec2, DVec3};

use super::{
    FlangeFrame, RResult, Seg, apply_flanges, bend_cuts, corner_trims, flange_frame, flange_prism,
    geo, oriented_chain, round_corners, sketch_of, thickened,
};
use crate::FeatureId;
use crate::build::State;
use crate::sheet_metal::{BendRecord, FlangePosition, MiterFlange};

/// A segment mapped by a rigid 2D motion `f` that reflects when `mirror`.
fn mapped(seg: Seg, f: &impl Fn(DVec2) -> DVec2, mirror: bool) -> Seg {
    match seg {
        Seg::Line { a, b } => Seg::Line { a: f(a), b: f(b) },
        Seg::Arc { c, r, a0, sweep } => {
            let center = f(c);
            let start = f(c + DVec2::from_angle(a0) * r);
            Seg::Arc {
                c: center,
                r,
                a0: (start - center).to_angle(),
                sweep: if mirror { -sweep } else { sweep },
            }
        }
    }
}

pub(in crate::ops) fn miter_flange(
    fid: FeatureId,
    p: &MiterFlange,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (plane, sk) = sketch_of(prev, p.sketch)?;
    let contours = sk.contours().map_err(geo)?;
    let chain = match (contours.loops.as_slice(), contours.open.as_slice()) {
        ([], [chain]) => chain,
        _ => return Err("a miter flange profile must be one open chain of lines and arcs".into()),
    };
    let mut frames: Vec<FlangeFrame> = p
        .edges
        .iter()
        .map(|r| flange_frame(prev, r, false))
        .collect::<RResult<_>>()?;
    let f0 = &frames[0];
    if plane.normal.cross(f0.z).length() > 1e-6 {
        return Err("sketch the profile on a plane square to the first edge".into());
    }
    let segs = oriented_chain(&chain.curves)?;
    // Sketch coordinates to the first edge's (x, y).
    let world = |q: DVec2| plane.to_world(q);
    let first = world(segs[0].start());
    let (x0, y0, _) = f0.local(first);
    let t = f0.thickness;
    // The profile may start on either face at the edge; it rises the way it
    // goes, so the frames turn over when it goes the other way.
    let rises: f64 = segs.iter().map(|s| f0.local(world(s.end())).1 - y0).sum();
    let flip = rises < 0.0;
    if flip {
        frames = p
            .edges
            .iter()
            .map(|r| flange_frame(prev, r, true))
            .collect::<RResult<_>>()?;
    }
    let f0 = &frames[0];
    let (x0b, y0b, _) = f0.local(first);
    if x0b.abs() > 1e-4 || (y0b.abs() > 1e-4 && (y0b + t).abs() > 1e-4) {
        let _ = (x0, y0);
        return Err("start the profile on the first edge".into());
    }
    let into_local = |q: DVec2| {
        let (x, y, _) = f0.local(world(q));
        DVec2::new(x - x0b, y - y0b)
    };
    let mirror = {
        let o = into_local(DVec2::ZERO);
        (into_local(DVec2::X) - o).perp_dot(into_local(DVec2::Y) - o) < 0.0
    };
    let profile: Vec<Seg> = segs
        .iter()
        .map(|s| mapped(*s, &into_local, mirror))
        .collect();
    let record = &prev.sheet_metal[f0.record];
    let r = p.bend.radius.unwrap_or(record.params.radius);
    let allowance = p
        .bend
        .allowance
        .clone()
        .unwrap_or(record.params.allowance.clone());
    // The bend from the sheet into the profile's first segment.
    let dir = (profile[0].end() - profile[0].start()).normalize();
    let turn = dir.y.atan2(dir.x);
    if !(turn > 1e-6 && turn < std::f64::consts::PI - 1e-6) {
        return Err("the profile must leave the sheet upward".into());
    }
    let trim = r * (turn / 2.0).tan();
    let shift = match p.position {
        FlangePosition::MaterialInside | FlangePosition::BendFromVirtualSharp => -t / turn.sin(),
        FlangePosition::MaterialOutside => 0.0,
        FlangePosition::BendOutside | FlangePosition::TangentToBend => trim,
    };
    let x_end = shift - trim;
    let mut chain: Vec<Seg> = vec![Seg::Line {
        a: DVec2::new(x_end - t, 0.0),
        b: DVec2::new(shift, 0.0),
    }];
    chain.extend(
        profile
            .iter()
            .map(|s| mapped(*s, &|q| q + DVec2::new(shift, 0.0), false)),
    );
    let mut names = vec!["w".to_string()];
    names.extend(chain_names(&chain_entities(
        chain.len() - 1,
        &contours.open[0].entities,
    )));
    let elements = round_corners(&chain, &names, r, -t, 0.0)?;
    let region = thickened(&elements, -t, 0.0);
    let reach = chain
        .iter()
        .flat_map(|s| {
            [
                s.start().x.abs(),
                s.end().x.abs(),
                s.start().y.abs(),
                s.end().y.abs(),
            ]
        })
        .fold(0.0, f64::max)
        + r
        + 2.0 * t;
    let mut cuts = Vec::new();
    let mut tools = Vec::new();
    let mut bends = Vec::new();
    let n = frames.len();
    for (k, f) in frames.iter().enumerate() {
        let shares = |end: DVec3| {
            frames
                .iter()
                .enumerate()
                .any(|(j, g)| j != k && g.ends.iter().any(|o| (*o - end).length() < 1e-6))
        };
        let z0 = if shares(f.ends[0]) {
            -reach
        } else if k == 0 {
            p.start_offset
        } else {
            0.0
        };
        let z1 = if shares(f.ends[1]) {
            f.length + reach
        } else if k == n - 1 {
            f.length - p.end_offset
        } else {
            f.length
        };
        let prefix = format!("e{k}.");
        let tool = flange_prism(fid, &prefix, f, &region, z0, z1)?;
        tools.push(corner_trims(fid, tool, k, &frames, p.gap)?);
        cuts.extend(bend_cuts(
            fid,
            &prefix,
            f,
            x_end,
            z0.max(0.0),
            z1.min(f.length),
            record.params.relief,
        )?);
        for (i, e) in elements.iter().filter(|e| e.bend.is_some()).enumerate() {
            let (radius, angle) = e.bend.unwrap();
            bends.push((
                f.record,
                BendRecord {
                    feature: fid,
                    index: (k * 64 + i) as u32,
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

fn chain_entities(n: usize, entities: &[of_sketch::Id]) -> Vec<of_sketch::Id> {
    entities.iter().copied().take(n).collect()
}

fn chain_names(entities: &[of_sketch::Id]) -> Vec<String> {
    entities.iter().map(|e| format!("s{e}")).collect()
}
