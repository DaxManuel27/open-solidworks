//! Trim/extend: members cut back or lengthened to faces, planes or other
//! bodies, square, mitered or butted.

use of_geom::{BoolOp, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Shape, SurfaceKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::members::{CornerTreatment, Side, corner_cuts, half_space};
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::{BodyId, FeatureId, Ref, resolve};

/// What members are trimmed to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrimBoundary {
    /// Planar faces or planes: members end flush with them.
    Faces { faces: Vec<Ref> },
    /// Other bodies.
    Bodies {
        bodies: Vec<BodyId>,
        /// Cope to the bodies' shape instead of a plane at their face.
        #[serde(default)]
        coped: bool,
    },
}

/// How trimmed members end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrimKind {
    /// Cut (or extended) to the boundary.
    #[default]
    EndTrim,
    /// One member and one boundary member mitered to each other.
    EndMiter,
    /// The member runs through; the boundary member stops at its face.
    EndButt1,
    /// The boundary member runs through; the member stops at its face.
    EndButt2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TrimExtend {
    /// Member bodies to trim or extend.
    #[schemars(length(min = 1))]
    pub bodies: Vec<BodyId>,
    pub boundary: TrimBoundary,
    #[serde(default)]
    pub kind: TrimKind,
    /// Lengthen members that stop short of the boundary.
    #[serde(default = "yes")]
    pub allow_extension: bool,
    /// Weld gap left at the boundary, mm.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub gap: f64,
}

fn yes() -> bool {
    true
}

/// A member being trimmed: its body index and its record's axis.
struct Member {
    body: usize,
    record: usize,
    a: DVec3,
    d: DVec3,
    len: f64,
}

fn member(state: &State, id: &BodyId) -> Result<Member, String> {
    let body = state
        .bodies
        .iter()
        .position(|b| b.id == *id)
        .ok_or_else(|| format!("no body {}", id.0))?;
    let records: Vec<usize> = (0..state.members.len())
        .filter(|r| state.members[*r].body == *id)
        .collect();
    let [record] = records[..] else {
        return Err(if records.is_empty() {
            format!("body {} is not a structural member", id.0)
        } else {
            "a merged group of members can't be trimmed; trim its members before merging".into()
        });
    };
    let [a, b] = state.members[record].axis.map(DVec3::from);
    let len = (b - a).length();
    Ok(Member {
        body,
        record,
        a,
        d: (b - a) / len,
        len,
    })
}

/// Where along the member the body ends: the extreme of its vertices.
fn extent(body: &Body, m: &Member) -> (f64, f64) {
    let s: Vec<f64> = (0..body.shape.count(Kind::Vertex))
        .filter_map(|v| body.shape.vertex_point(v).ok())
        .map(|q| (q - m.a).dot(m.d))
        .collect();
    (
        s.iter().copied().fold(f64::MAX, f64::min),
        s.iter().copied().fold(f64::MIN, f64::max),
    )
}

/// Lengthen a member's `end` (0 start, 1 end) so it reaches `to` along its
/// axis, by sweeping its end face.
fn extend(fid: FeatureId, body: Body, m: &Member, end: usize, to: f64) -> Result<Body, String> {
    let (lo, hi) = extent(&body, m);
    let (reach, dir) = if end == 1 {
        (to - hi, m.d)
    } else {
        (lo - to, -m.d)
    };
    if reach <= 1e-9 {
        return Ok(body);
    }
    // The end face: planar, facing along the axis, farthest out.
    let shape = &body.shape;
    let face = (0..shape.count(Kind::Face))
        .filter_map(|f| shape.face_info(f).ok().map(|i| (f, i)))
        .filter(|(_, i)| i.kind == SurfaceKind::Plane && i.normal.dot(dir) > 0.5)
        .max_by(|x, y| x.1.centroid.dot(dir).total_cmp(&y.1.centroid.dot(dir)))
        .ok_or("the member has no end face to extend")?
        .0;
    let end_face = shape.sub_shape(Kind::Face, face).map_err(|e| e.0)?;
    let grown = end_face.prism(dir * (reach + 1.0)).map_err(|e| e.0)?.shape;
    let r = body.shape.boolean(BoolOp::Fuse, &grown).map_err(|e| e.0)?;
    Ok(Body {
        names: Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&body.names), None],
            },
            &mut |_| Some(format!("{fid}.m{}.extended", m.record)),
            &fid.to_string(),
        ),
        shape: r.shape,
    })
}

/// Faces a trim makes are named per member, so bodies never share names.
fn cut(fid: FeatureId, m: &Member, body: Body, point: DVec3, keep: DVec3) -> Result<Body, String> {
    let tool = half_space(point, keep)?;
    let r = body.shape.boolean(BoolOp::Common, &tool).map_err(|e| e.0)?;
    if r.shape.count(Kind::Solid) == 0 {
        return Err("trimming leaves nothing of a member".into());
    }
    Ok(Body {
        names: Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&body.names), None],
            },
            &mut |_| Some(format!("{fid}.m{}.trim", m.record)),
            &fid.to_string(),
        ),
        shape: r.shape,
    })
}

/// A plane the member's axis crosses: where (along the axis) and which end
/// it trims.
fn crossing(m: &Member, point: DVec3, normal: DVec3) -> Option<(f64, usize)> {
    let along = normal.dot(m.d);
    if along.abs() < 1e-9 {
        return None;
    }
    let s = normal.dot(point - m.a) / along;
    Some((s, if s > m.len / 2.0 { 1 } else { 0 }))
}

/// Cut or extend `end` of a member at a plane through `point`, keeping the
/// side toward its other end, `gap` back from the plane.
#[allow(clippy::too_many_arguments)]
fn trim_at(
    fid: FeatureId,
    body: Body,
    m: &Member,
    end: usize,
    s: f64,
    point: DVec3,
    normal: DVec3,
    gap: f64,
    allow_extension: bool,
) -> Result<Option<(Body, DVec3)>, String> {
    let other = if end == 1 { m.a } else { m.a + m.d * m.len };
    let keep = if normal.dot(other - point) >= 0.0 {
        normal
    } else {
        -normal
    };
    let beyond = if end == 1 { s > m.len } else { s < 0.0 };
    let body = if beyond {
        if !allow_extension {
            return Ok(None);
        }
        let to = if end == 1 { s + 1.0 } else { s - 1.0 };
        extend(fid, body, m, end, to)?
    } else {
        body
    };
    Ok(Some((cut(fid, m, body, point + keep * gap, keep)?, keep)))
}

/// Where the member's axis enters and leaves a body, along the axis.
fn hits(m: &Member, other: &Shape) -> Result<Option<(f64, f64)>, String> {
    let reach = other.bbox().map_or(1e3, |b| b.diagonal()) + m.len;
    let plane = Plane::new(m.a - m.d * reach, m.d.any_orthonormal_vector(), m.d);
    let w = 1e-3;
    let probe = Profile {
        plane,
        faces: vec![ProfileFace {
            outer: (0..4)
                .map(|i| {
                    let c = [
                        DVec2::new(-w, -w),
                        DVec2::new(w, -w),
                        DVec2::new(w, w),
                        DVec2::new(-w, w),
                    ];
                    of_geom::Curve2::Line {
                        a: c[i],
                        b: c[(i + 1) % 4],
                    }
                })
                .collect(),
            holes: Vec::new(),
        }],
    }
    .to_shape()
    .map_err(|e| e.0)?
    .prism(m.d * (2.0 * reach + m.len))
    .map_err(|e| e.0)?
    .shape;
    let inside = probe.boolean(BoolOp::Common, other).map_err(|e| e.0)?.shape;
    if inside.count(Kind::Solid) == 0 {
        return Ok(None);
    }
    let s: Vec<f64> = (0..inside.count(Kind::Vertex))
        .filter_map(|v| inside.vertex_point(v).ok())
        .map(|q| (q - m.a).dot(m.d))
        .collect();
    Ok(Some((
        s.iter().copied().fold(f64::MAX, f64::min),
        s.iter().copied().fold(f64::MIN, f64::max),
    )))
}

/// The planar face of `other` through `point`, for a simple cut.
fn face_through(other: &Shape, point: DVec3) -> Option<DVec3> {
    (0..other.count(Kind::Face))
        .filter_map(|f| other.face_info(f).ok())
        .filter(|i| i.kind == SurfaceKind::Plane && (point - i.point).dot(i.normal).abs() < 1e-3)
        .map(|i| i.normal)
        .next()
}

pub(crate) fn trim_extend(
    fid: FeatureId,
    p: &TrimExtend,
    prev: &State,
    state: &mut State,
) -> Result<(), String> {
    match (&p.boundary, p.kind) {
        (_, TrimKind::EndMiter | TrimKind::EndButt1 | TrimKind::EndButt2) => {
            return pair(fid, p, prev, state);
        }
        (TrimBoundary::Faces { faces }, TrimKind::EndTrim) => {
            let planes: Vec<Plane> = faces
                .iter()
                .map(|r| resolve(prev, r)?.plane())
                .collect::<Result<_, _>>()?;
            for id in &p.bodies {
                let m = member(prev, id)?;
                let mut body = prev.bodies[m.body].geometry.clone();
                let mut cuts = Vec::new();
                // Each plane trims the end it crosses nearest.
                for pl in &planes {
                    let Some((s, end)) = crossing(&m, pl.origin, pl.normal) else {
                        return Err("the boundary runs parallel to a member".into());
                    };
                    if let Some((b, keep)) = trim_at(
                        fid,
                        body.clone(),
                        &m,
                        end,
                        s,
                        pl.origin,
                        pl.normal,
                        p.gap,
                        p.allow_extension,
                    )? {
                        body = b;
                        cuts.push((end, keep));
                    }
                }
                finish(state, &m, body, &cuts);
            }
        }
        (TrimBoundary::Bodies { bodies, coped }, TrimKind::EndTrim) => {
            for id in &p.bodies {
                if bodies.contains(id) {
                    return Err("a member can't be its own boundary".into());
                }
                let m = member(prev, id)?;
                let mut body = prev.bodies[m.body].geometry.clone();
                let mut cuts = Vec::new();
                let mut reached = false;
                // Each boundary body trims the end it is nearest.
                for other in bodies {
                    let boundary = prev
                        .solid(other)
                        .map_err(|e| e.to_string())?
                        .geometry
                        .shape
                        .clone();
                    let Some((s_in, s_out)) = hits(&m, &boundary)? else {
                        continue;
                    };
                    reached = true;
                    // Entered toward the end, or left toward the start.
                    let end = if (s_in + s_out) / 2.0 > m.len / 2.0 {
                        1
                    } else {
                        0
                    };
                    let s = if end == 1 { s_in } else { s_out };
                    let beyond = (end == 1 && s > m.len) || (end == 0 && s < 0.0);
                    if beyond && !p.allow_extension {
                        continue;
                    }
                    let point = m.a + m.d * s;
                    if *coped {
                        let far = if end == 1 { s_out + 1.0 } else { s_in - 1.0 };
                        let grown = extend(fid, body, &m, end, far)?;
                        body = cope(fid, grown, &boundary, &m, end)?;
                    } else {
                        let normal = face_through(&boundary, point).ok_or(
                            "the boundary isn't flat where the member meets it; use a coped trim",
                        )?;
                        if let Some((b, keep)) = trim_at(
                            fid,
                            body.clone(),
                            &m,
                            end,
                            s,
                            point,
                            normal,
                            p.gap,
                            p.allow_extension,
                        )? {
                            body = b;
                            cuts.push((end, keep));
                        }
                    }
                }
                if !reached {
                    return Err("a member's axis doesn't reach the boundary bodies".into());
                }
                finish(state, &m, body, &cuts);
            }
        }
    }
    state.aggregate_bodies()
}

/// Take the boundary's shape out of a member, keeping the piece that holds
/// its other end.
fn cope(
    fid: FeatureId,
    body: Body,
    boundary: &Shape,
    m: &Member,
    end: usize,
) -> Result<Body, String> {
    let r = body.shape.boolean(BoolOp::Cut, boundary).map_err(|e| e.0)?;
    let named = Body {
        names: Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&body.names), None],
            },
            &mut |_| Some(format!("{fid}.m{}.cope", m.record)),
            &fid.to_string(),
        ),
        shape: r.shape,
    };
    let pieces = named.shape.solids();
    if pieces.len() <= 1 {
        return Ok(named);
    }
    // The piece reaching farthest toward the member's other end.
    let toward = if end == 1 { -m.d } else { m.d };
    let keep = pieces
        .iter()
        .max_by(|x, y| {
            let far = |s: &Shape| {
                (0..s.count(Kind::Vertex))
                    .filter_map(|v| s.vertex_point(v).ok())
                    .map(|q| q.dot(toward))
                    .fold(f64::MIN, f64::max)
            };
            far(x).total_cmp(&far(y))
        })
        .unwrap();
    let tool = Shape::compound(pieces.iter().filter(|s| !std::ptr::eq(*s, keep)));
    let r = named.shape.boolean(BoolOp::Cut, &tool).map_err(|e| e.0)?;
    Ok(Body {
        names: Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&named.names), None],
            },
            &mut |_| Some(format!("{fid}.m{}.cope", m.record)),
            &fid.to_string(),
        ),
        shape: r.shape,
    })
}

/// A member and one boundary member mitered or butted at their axes' meeting.
fn pair(fid: FeatureId, p: &TrimExtend, prev: &State, state: &mut State) -> Result<(), String> {
    let TrimBoundary::Bodies { bodies, .. } = &p.boundary else {
        return Err("miters and butts trim against one member body".into());
    };
    let ([one], [two]) = (&p.bodies[..], &bodies[..]) else {
        return Err("miters and butts trim one member against one member".into());
    };
    let (m1, m2) = (member(prev, one)?, member(prev, two)?);
    // The closest points of the two axes.
    let w = m1.a - m2.a;
    let (b, d, e) = (m1.d.dot(m2.d), m1.d.dot(w), m2.d.dot(w));
    let denom = 1.0 - b * b;
    if denom < 1e-9 {
        return Err("the members run parallel".into());
    }
    let s1 = (b * e - d) / denom;
    let s2 = (e - b * d) / denom;
    let corner = (m1.a + m1.d * s1 + m2.a + m2.d * s2) / 2.0;
    let away = |m: &Member, s: f64| if s > m.len / 2.0 { -m.d } else { m.d };
    let side = |m: &Member, away: DVec3| {
        let r = &prev.members[m.record];
        (
            r.outline
                .iter()
                .map(|q| DVec2::from(*q))
                .collect::<Vec<_>>(),
            DVec3::from(r.frame[0]),
            DVec3::from(r.frame[1]),
            away,
        )
    };
    let (p1, x1, y1, u) = side(&m1, away(&m1, s1));
    let (p2, x2, y2, v) = side(&m2, away(&m2, s2));
    let kind = match p.kind {
        TrimKind::EndMiter => CornerTreatment::EndMiter,
        TrimKind::EndButt1 => CornerTreatment::EndButt1,
        _ => CornerTreatment::EndButt2,
    };
    let (c1, c2) = corner_cuts(
        kind,
        corner,
        p.gap,
        true,
        Side {
            points: &p1,
            x: x1,
            y: y1,
            away: u,
        },
        Side {
            points: &p2,
            x: x2,
            y: y2,
            away: v,
        },
    )?;
    for (m, s, (point, normal), pts) in [(&m1, s1, c1, &p1), (&m2, s2, c2, &p2)] {
        let end = if s > m.len / 2.0 { 1 } else { 0 };
        let at = crossing(m, point, normal).map_or(s, |c| c.0);
        let body = prev.bodies[m.body].geometry.clone();
        // Reach past the cutting plane by the section's size, then cut.
        let size = pts.iter().map(|q| q.length()).fold(0.0, f64::max) * 2.0 + 1.0;
        let body = if p.allow_extension {
            extend(
                fid,
                body,
                m,
                end,
                if end == 1 { at + size } else { at - size },
            )?
        } else {
            body
        };
        finish(
            state,
            m,
            cut(fid, m, body, point, normal)?,
            &[(end, normal)],
        );
    }
    state.aggregate_bodies()
}

/// Store a trimmed member and its new length and, for planar cuts, the
/// angles of the ends they cut.
fn finish(state: &mut State, m: &Member, body: Body, cuts: &[(usize, DVec3)]) {
    let (lo, hi) = extent(&body, m);
    let record = &mut state.members[m.record];
    record.length = hi - lo;
    for (end, keep) in cuts {
        record.angles[*end] = keep.dot(m.d).abs().clamp(0.0, 1.0).acos();
    }
    state.bodies[m.body].geometry = body;
}
