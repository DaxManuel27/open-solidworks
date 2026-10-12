//! Ribs: thin webs grown from open sketch chains until they meet material.
//!
//! Each chain is extended along its end tangents past the part, thickened into a
//! long slab and cut by the existing material. The slab pieces that touch the
//! sketched chain and stay inside the part's extent are the rib; a piece that
//! escapes the part means the rib never meets material in that direction.

use of_geom::{
    BoolOp, Curve2 as GCurve, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Relation, Shape,
};
use of_sketch::{Chain, Curve2 as SCurve, Sketch};

use super::{
    RResult, apply_feature_tool, body_of, copy_chain, geo, keep_solids, sketch_of, sweep_names,
    thin_faces, through_all_length, to_geom, translate,
};
use crate::build::State;
use crate::naming::{Inputs, Naming};
use crate::{FeatureId, Rib, RibDirection, RibSide, Thin, ThinKind};

pub(super) fn rib(fid: FeatureId, p: &Rib, prev: &State, state: &mut State) -> RResult<()> {
    if !(p.thickness.is_finite() && p.thickness > 1e-9) {
        return Err("the rib thickness must be more than zero".into());
    }
    let body = body_of(prev).map_err(|_| "a rib needs existing material to meet".to_string())?;
    let (plane, sk) = sketch_of(prev, p.sketch)?;
    let contours = sk.contours().map_err(geo)?;
    if !contours.loops.is_empty() {
        return Err("a rib sketch must contain only open lines and arcs, not closed loops".into());
    }
    if contours.open.is_empty() {
        return Err("the rib sketch has no open chains".into());
    }
    let bbox = body.shape.bbox().ok_or("the part has no extent")?;
    let long = through_all_length(prev, plane);
    let mut tool: Option<(Shape, Naming)> = None;
    for chain in &contours.open {
        let (shape, names) = one_rib(fid, p, plane, sk, chain, long, bbox.center(), prev)?;
        tool = Some(match tool {
            None => (shape, names),
            Some((all, all_names)) => {
                let r = all.boolean(BoolOp::Fuse, &shape).map_err(geo)?;
                let names = Naming::from_history(
                    &r.shape,
                    &r.history,
                    Inputs {
                        named: [Some(&all_names), Some(&names)],
                    },
                    &mut |_| None,
                    &fid.to_string(),
                );
                (r.shape, names)
            }
        });
    }
    let (tool, names) = tool.expect("at least one chain");
    apply_feature_tool(fid, state, tool, names, BoolOp::Fuse, true)
}

/// A chain's curves in travel order, each flagged when it runs backwards.
fn ordered(chain: &Chain) -> RResult<Vec<(SCurve, bool)>> {
    let near = |a: DVec2, b: DVec2| (a - b).length() < 1e-6;
    let curves = &chain.curves;
    if curves.iter().any(SCurve::is_closed) {
        return Err("a rib sketch must contain only open lines and arcs, not closed loops".into());
    }
    let first_reversed = curves.len() > 1
        && (near(curves[0].start(), curves[1].start()) || near(curves[0].start(), curves[1].end()));
    let mut out = vec![(curves[0].clone(), first_reversed)];
    for c in &curves[1..] {
        let (last, rev) = out.last().unwrap();
        let exit = if *rev { last.start() } else { last.end() };
        out.push((c.clone(), !near(c.start(), exit)));
    }
    Ok(out)
}

fn entry(c: &(SCurve, bool)) -> DVec2 {
    if c.1 { c.0.end() } else { c.0.start() }
}

fn exit(c: &(SCurve, bool)) -> DVec2 {
    if c.1 { c.0.start() } else { c.0.end() }
}

/// Unit travel direction at the start (`at_end == false`) or end of an oriented curve.
fn travel(c: &(SCurve, bool), at_end: bool) -> DVec2 {
    let t = if at_end != c.1 { 1.0 } else { 0.0 };
    let d = c.0.tangent_at(t).normalize_or_zero();
    if c.1 { -d } else { d }
}

#[allow(clippy::too_many_arguments)]
fn one_rib(
    fid: FeatureId,
    p: &Rib,
    plane: &Plane,
    sk: &Sketch,
    chain: &Chain,
    long: f64,
    center: DVec3,
    prev: &State,
) -> RResult<(Shape, Naming)> {
    let path = ordered(chain)?;
    let (e0, e1) = (entry(&path[0]), exit(path.last().unwrap()));
    let out0 = -travel(&path[0], false);
    let out1 = travel(path.last().unwrap(), true);
    if out0 == DVec2::ZERO || out1 == DVec2::ZERO {
        return Err("the rib sketch has a zero-length entity".into());
    }
    let (far0, far1) = (e0 + out0 * long, e1 + out1 * long);
    let chain_curves: Vec<GCurve> = chain.curves.iter().map(to_geom).collect();
    let n = plane.normal;

    let (face, slab_dir, start_shift) = match p.direction {
        RibDirection::ParallelToSketch => {
            let chord = e1 - e0;
            if chord.length() < 1e-9 {
                return Err("a parallel rib needs a chain whose ends are apart".into());
            }
            let u = chord.normalize();
            // Every point of the chain, including its extensions, must advance
            // along the chord or the swept region folds over itself.
            let mut last = f64::NEG_INFINITY;
            let mut points = vec![far0];
            for c in &path {
                let mut s = c.0.sample(1e-3);
                if c.1 {
                    s.reverse();
                }
                points.extend(s);
            }
            points.push(far1);
            for q in points {
                let x = (q - e0).dot(u);
                if x < last - 1e-9 * long {
                    return Err("the rib sketch folds back along its length; use the normal-to-sketch direction or simplify the sketch".into());
                }
                last = last.max(x);
            }
            if out0.dot(u) > -1e-3 || out1.dot(u) < 1e-3 {
                return Err(
                    "the rib sketch's end tangents must point away from each other to extend it"
                        .into(),
                );
            }
            let mut d = DVec2::new(-u.y, u.x);
            if (plane.to_local(center) - e0).dot(d) < 0.0 {
                d = -d;
            }
            if p.flip {
                d = -d;
            }
            let shift = d * long;
            let mut ext = vec![GCurve::Line { a: far0, b: e0 }];
            ext.extend(chain_curves.iter().cloned());
            ext.push(GCurve::Line { a: e1, b: far1 });
            let mut outer = ext.clone();
            outer.push(GCurve::Line {
                a: far1,
                b: far1 + shift,
            });
            outer.extend(ext.iter().rev().map(|c| shifted(c, shift)));
            outer.push(GCurve::Line {
                a: far0 + shift,
                b: far0,
            });
            let face = Profile {
                plane: *plane,
                faces: vec![ProfileFace {
                    outer,
                    holes: vec![],
                }],
            }
            .to_shape()
            .map_err(geo)?;
            let t = p.thickness;
            match p.side {
                RibSide::Both => (face, n * t, -n * (t / 2.0)),
                RibSide::First => (face, n * t, DVec3::ZERO),
                RibSide::Second => (face, -n * t, DVec3::ZERO),
            }
        }
        RibDirection::NormalToSketch => {
            // Thicken the extended chain within the plane, then grow it along the normal.
            let mut tmp = Sketch::new();
            let mut ids = copy_chain(sk, &chain.entities, &mut tmp);
            let point_at = |tmp: &Sketch, at: DVec2| {
                tmp.entities
                    .keys()
                    .copied()
                    .find(|id| tmp.point(*id).is_ok_and(|q| (q - at).length() < 1e-6))
            };
            let (p0, p1) = (point_at(&tmp, e0), point_at(&tmp, e1));
            let (Some(p0), Some(p1)) = (p0, p1) else {
                return Err("the rib sketch chain is not connected".into());
            };
            let f0 = tmp.add_point(far0);
            let f1 = tmp.add_point(far1);
            ids.push(tmp.add_line(f0, p0).map_err(geo)?);
            ids.push(tmp.add_line(p1, f1).map_err(geo)?);
            let extended = tmp.contours().map_err(geo)?;
            if extended.open.len() != 1 || !extended.loops.is_empty() {
                return Err("the extended rib sketch intersects itself".into());
            }
            let thin = Thin {
                kind: if p.side == RibSide::Both {
                    ThinKind::MidPlane
                } else {
                    ThinKind::OneDirection
                },
                thickness: p.thickness,
                thickness2: 0.0,
                reverse: p.side == RibSide::Second,
            };
            let faces = thin_faces(&tmp, &extended, &thin)?;
            let face = Profile {
                plane: *plane,
                faces,
            }
            .to_shape()
            .map_err(geo)?;
            let mut dir = if (center - plane.origin).dot(n) >= 0.0 {
                n
            } else {
                -n
            };
            if p.flip {
                dir = -dir;
            }
            (face, dir * long, DVec3::ZERO)
        }
    };

    // Name the slab's side faces after the sketch curves they grow from.
    let candidates: Vec<(of_sketch::Id, &SCurve)> =
        chain.entities.iter().copied().zip(&chain.curves).collect();
    let edge_names: Vec<String> = (0..face.count(Kind::Edge))
        .map(|i| {
            let Ok(info) = face.edge_info(i) else {
                return "x".to_string();
            };
            let mid = plane.to_local(info.mid);
            let best = candidates
                .iter()
                .map(|(id, c)| (id, (c.closest_point(mid) - mid).length()))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match best {
                Some((id, d)) if d < 1e-6 => format!("s{id}"),
                Some((id, d)) if d < p.thickness * 1.01 + 1e-6 => format!("t{id}"),
                _ => "x".to_string(),
            }
        })
        .collect();
    let face = if start_shift == DVec3::ZERO {
        face
    } else {
        translate(&face, start_shift)?
    };
    let r = face.prism(slab_dir).map_err(geo)?;
    let mut generate = sweep_names(fid, "", &edge_names);
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [None, None],
        },
        &mut generate,
        &fid.to_string(),
    );
    let (slab, names) = match p.draft.filter(|angle| *angle != 0.0) {
        None => (r.shape, names),
        Some(angle) => {
            // The thickness faces: the slab's caps when thickness is along the plane
            // normal, or the faces grown from the thickened chain otherwise.
            let parallel = p.direction == RibDirection::ParallelToSketch;
            let faces: Vec<u32> = r
                .history
                .iter()
                .filter(|h| h.to_kind == Kind::Face)
                .filter(|h| {
                    if parallel {
                        matches!(h.rel, Relation::StartCap | Relation::EndCap)
                    } else {
                        h.rel == Relation::Generated
                            && h.from_kind == Kind::Edge
                            && edge_names
                                .get(h.from as usize)
                                .is_some_and(|n| n.starts_with('t') || n.starts_with('s'))
                    }
                })
                .map(|h| h.to)
                .collect();
            let (start, pull) = if parallel {
                // In-plane growth, perpendicular to the chord toward the material.
                let u = (e1 - e0).normalize();
                let mut d = DVec2::new(-u.y, u.x);
                if (plane.to_local(center) - e0).dot(d) < 0.0 {
                    d = -d;
                }
                if p.flip {
                    d = -d;
                }
                (
                    plane.to_world(e0),
                    (plane.to_world(d) - plane.to_world(DVec2::ZERO)).normalize(),
                )
            } else {
                (plane.origin, slab_dir.normalize())
            };
            let neutral = Plane::new(start, pull.any_orthonormal_vector(), pull);
            draft_slab(fid, &r.shape, &names, &faces, &neutral, pull, angle)?
        }
    };
    let body = body_of(prev)?;
    let wire = Shape::wire(plane, &chain_curves).map_err(geo)?;
    let mut allowed = body.shape.bbox().ok_or("the part has no extent")?;
    if let Some(w) = wire.bbox() {
        allowed.min = allowed.min.min(w.min);
        allowed.max = allowed.max.max(w.max);
    }
    let margin = allowed.diagonal() * 1e-3 + 1e-6;
    let inside = |s: &Shape| {
        s.bbox().is_some_and(|b| {
            b.min.cmpge(allowed.min - margin).all() && b.max.cmple(allowed.max + margin).all()
        })
    };
    let cut = slab.boolean(BoolOp::Cut, &body.shape).map_err(geo)?;
    keep_solids(
        fid,
        "",
        cut,
        &names,
        &body.names,
        |s| inside(s) && s.distance(&wire).is_ok_and(|d| d.value < 1e-6),
        "the rib does not meet the surrounding material; flip its direction or move the sketch",
    )
}

fn shifted(c: &GCurve, v: DVec2) -> GCurve {
    match c {
        GCurve::Line { a, b } => GCurve::Line { a: a + v, b: b + v },
        GCurve::Arc { center, start, end } => GCurve::Arc {
            center: center + v,
            start: start + v,
            end: end + v,
        },
        GCurve::Circle { center, radius } => GCurve::Circle {
            center: center + v,
            radius: *radius,
        },
        GCurve::Bezier { points } => GCurve::Bezier {
            points: points.map(|q| q + v),
        },
        GCurve::Spline { points, closed } => GCurve::Spline {
            points: points.iter().map(|q| q + v).collect(),
            closed: *closed,
        },
    }
}

/// Draft the slab's thickness faces so it widens along `pull`, keeping whichever
/// angle sign makes it thicker away from the neutral plane.
fn draft_slab(
    fid: FeatureId,
    slab: &Shape,
    names: &Naming,
    faces: &[u32],
    neutral: &Plane,
    pull: DVec3,
    angle: f64,
) -> RResult<(Shape, Naming)> {
    if !(angle.is_finite() && angle > 0.0 && angle < std::f64::consts::FRAC_PI_4) {
        return Err("the rib draft angle must be between 0 and 45 degrees".into());
    }
    if faces.is_empty() {
        return Err("the rib has no faces to draft".into());
    }
    let before = slab.mass_props().map_err(geo)?.volume;
    let r = [angle, -angle]
        .into_iter()
        .filter_map(|signed| slab.draft(faces, neutral, pull, signed).ok())
        .find(|r| r.shape.mass_props().is_ok_and(|m| m.volume > before))
        .ok_or("the rib faces cannot be drafted; reduce the draft angle")?;
    let drafted = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(names), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    Ok((r.shape, drafted))
}
