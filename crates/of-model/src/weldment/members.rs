//! Structural members: a profile swept along sketch segments, one body per
//! segment, with corners cut by planes so they never depend on how two
//! profiles happen to intersect.

use of_geom::{BoolOp, Curve2, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Shape};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{ProfileSource, Section, profiles};
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::{BodyId, FeatureId, Ref, SolidBody};

/// The point of the profile that lies on the sketch segment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PiercePoint {
    #[default]
    Center,
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
    /// The section's centroid.
    Centroid,
}

/// Mirror the profile across its own axes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMirror {
    #[default]
    None,
    /// Left for right.
    Horizontal,
    /// Top for bottom.
    Vertical,
}

/// How the members of a group meet at their shared corners.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CornerTreatment {
    /// Both cut on the plane bisecting the corner.
    #[default]
    EndMiter,
    /// The earlier segment runs through; the later one stops at its face.
    EndButt1,
    /// The later segment runs through; the earlier one stops at its face.
    EndButt2,
}

/// Segments that share a profile placement and corner treatment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MemberGroup {
    /// Straight lines of 2D or 3D sketches.
    #[schemars(length(min = 1))]
    pub segments: Vec<Ref>,
    #[serde(default)]
    pub corner: CornerTreatment,
    /// Gap left between members at their corners, mm.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub gap: f64,
}

/// A profile swept along groups of sketch segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StructuralMember {
    pub profile: ProfileSource,
    #[schemars(length(min = 1))]
    pub groups: Vec<MemberGroup>,
    #[serde(default)]
    pub pierce: PiercePoint,
    /// Turn the profile about its pierce point, radians.
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub mirror: ProfileMirror,
    /// Move the profile in its own plane (x across, y up), mm.
    #[serde(default)]
    pub offset: [f64; 2],
    /// Join each mitered group's members into one body.
    #[serde(default, skip_serializing_if = "crate::sheet_metal::is_false")]
    pub merge_miter_trimmed: bool,
}

/// What the cut list needs to know about a member body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberRecord {
    pub feature: FeatureId,
    pub body: BodyId,
    pub group: usize,
    pub segment: usize,
    pub profile: ProfileSource,
    /// Overall length along the axis, mm.
    pub length: f64,
    /// Each end's cut, radians off square.
    pub angles: [f64; 2],
}

/// A segment in model space and the direction profiles stand up from.
struct Segment {
    a: DVec3,
    b: DVec3,
    up: DVec3,
}

/// The ends of a sketch line, 2D or 3D, and the direction profiles stand
/// up from (a 2D sketch's normal; Z, or Y along Z, for 3D sketches).
fn segment(state: &State, r: &Ref) -> Result<Segment, String> {
    let Ref::SketchEntity { sketch, entity } = r else {
        return Err("select straight sketch lines for members".into());
    };
    if let Some((sk, _)) = state.sketches3d.get(sketch) {
        let of_sketch::spatial::Geometry3::Line { a, b } = sk.entity(*entity)?.geometry else {
            return Err("members follow straight 3D sketch lines".into());
        };
        let (a, b) = (sk.point(a)?, sk.point(b)?);
        let d = (b - a).normalize_or_zero();
        let up = if d.z.abs() > 0.99 { DVec3::Y } else { DVec3::Z };
        return Ok(Segment { a, b, up });
    }
    let (plane, sk) = state
        .sketches
        .get(sketch)
        .ok_or_else(|| format!("sketch {sketch} is missing"))?;
    let e = sk.entity(*entity).map_err(|e| e.to_string())?;
    let of_sketch::Geometry::Line { a, b } = &e.geo else {
        return Err("members follow straight sketch lines".into());
    };
    let (a, b) = (
        plane.to_world(sk.point(*a).map_err(|e| e.to_string())?),
        plane.to_world(sk.point(*b).map_err(|e| e.to_string())?),
    );
    Ok(Segment {
        a,
        b,
        up: plane.normal,
    })
}

/// A segment's profile frame: x across, y up, z along it.
fn frame(s: &Segment) -> Result<(DVec3, DVec3, DVec3), String> {
    let z = (s.b - s.a).normalize_or_zero();
    if z == DVec3::ZERO {
        return Err("a member segment has no length".into());
    }
    let mut y = s.up - z * s.up.dot(z);
    if y.length() < 1e-9 {
        y = z.any_orthonormal_vector();
    }
    let y = y.normalize();
    Ok((y.cross(z), y, z))
}

/// The section moved so its pierce point is at the origin, then mirrored,
/// turned and offset.
fn placed(section: &Section, p: &StructuralMember) -> Section {
    let (w, h) = (section.size.x / 2.0, section.size.y / 2.0);
    let pierce = match p.pierce {
        PiercePoint::Center => DVec2::ZERO,
        PiercePoint::TopLeft => DVec2::new(-w, h),
        PiercePoint::Top => DVec2::new(0.0, h),
        PiercePoint::TopRight => DVec2::new(w, h),
        PiercePoint::Left => DVec2::new(-w, 0.0),
        PiercePoint::Right => DVec2::new(w, 0.0),
        PiercePoint::BottomLeft => DVec2::new(-w, -h),
        PiercePoint::Bottom => DVec2::new(0.0, -h),
        PiercePoint::BottomRight => DVec2::new(w, -h),
        PiercePoint::Centroid => profiles::properties(section).1,
    };
    let (sx, sy) = match p.mirror {
        ProfileMirror::None => (1.0, 1.0),
        ProfileMirror::Horizontal => (-1.0, 1.0),
        ProfileMirror::Vertical => (1.0, -1.0),
    };
    let rot = DVec2::from_angle(p.rotation);
    let map = |q: DVec2| {
        let m = DVec2::new((q.x - pierce.x) * sx, (q.y - pierce.y) * sy);
        rot.rotate(m) + DVec2::from(p.offset)
    };
    // A mirror reverses arcs' direction: swap their ends.
    let flip = sx * sy < 0.0;
    let curve = |c: &Curve2| match c {
        Curve2::Line { a, b } => Curve2::Line {
            a: map(*a),
            b: map(*b),
        },
        Curve2::Arc { center, start, end } => {
            let (s, e) = if flip { (end, start) } else { (start, end) };
            Curve2::Arc {
                center: map(*center),
                start: map(*s),
                end: map(*e),
            }
        }
        Curve2::Circle { center, radius } => Curve2::Circle {
            center: map(*center),
            radius: *radius,
        },
        other => other.clone(),
    };
    Section {
        outer: section.outer.iter().map(curve).collect(),
        holes: section
            .holes
            .iter()
            .map(|h| h.iter().map(curve).collect())
            .collect(),
        size: section.size,
        wall: section.wall,
    }
}

/// How far a section reaches from its axis along a model direction.
fn reach(points: &[DVec2], x: DVec3, y: DVec3, dir: DVec3) -> f64 {
    points
        .iter()
        .map(|q| (x * q.x + y * q.y).dot(dir))
        .fold(f64::MIN, f64::max)
}

fn half_space(point: DVec3, keep: DVec3) -> Result<Shape, String> {
    let plane = Plane::new(point, keep.any_orthonormal_vector(), keep);
    Shape::half_space(&Shape::plane_face(&plane).map_err(|e| e.0)?, point + keep).map_err(|e| e.0)
}

/// A segment's place in its feature, end angles and length.
type Part = (usize, usize, [f64; 2], f64);

/// One member body before its corners are cut.
struct Raw {
    group: usize,
    index: usize,
    seg: Segment,
    axes: (DVec3, DVec3, DVec3),
    points: Vec<DVec2>,
    body: Body,
    /// Planes cutting each end: point and the normal kept.
    ends: [Option<(DVec3, DVec3)>; 2],
}

pub(crate) fn structural_member(
    fid: FeatureId,
    p: &StructuralMember,
    prev: &State,
    state: &mut State,
) -> Result<(), String> {
    if prev.weldment.is_none() {
        return Err("add a Weldment feature before structural members".into());
    }
    let section = placed(&super::section(&p.profile)?, p);
    let points = profiles::outline_points(&section);
    let reach_out = section.size.length() * 2.0 + 1.0;
    let mut raws: Vec<Raw> = Vec::new();
    for (g, group) in p.groups.iter().enumerate() {
        for (i, r) in group.segments.iter().enumerate() {
            let seg = segment(prev, r)?;
            let (x, y, z) = frame(&seg)?;
            let len = (seg.b - seg.a).length();
            let plane = Plane::new(seg.a - z * reach_out, x, z);
            let face = Profile {
                plane,
                faces: vec![ProfileFace {
                    outer: section.outer.clone(),
                    holes: section.holes.clone(),
                }],
            }
            .to_shape()
            .map_err(|e| format!("the profile isn't a closed region: {}", e.0))?;
            let edge_names: Vec<String> = (0..face.count(Kind::Edge))
                .map(|k| format!("c{k}"))
                .collect();
            let r = face.prism(z * (len + 2.0 * reach_out)).map_err(|e| e.0)?;
            let prefix = format!("g{g}.{i}.");
            let mut generate = crate::ops::sweep_names(fid, &prefix, &edge_names);
            let names = Naming::from_history(
                &r.shape,
                &r.history,
                Inputs {
                    named: [None, None],
                },
                &mut generate,
                &fid.to_string(),
            );
            raws.push(Raw {
                group: g,
                index: i,
                seg,
                axes: (x, y, z),
                points: points.clone(),
                body: Body {
                    shape: r.shape,
                    names,
                },
                ends: [None, None],
            });
        }
    }
    // Corners: segments of one group sharing an end.
    for g in 0..p.groups.len() {
        let group = &p.groups[g];
        let members: Vec<usize> = (0..raws.len()).filter(|k| raws[*k].group == g).collect();
        for (n, &i) in members.iter().enumerate() {
            for &j in &members[n + 1..] {
                let ends = |k: usize| [raws[k].seg.a, raws[k].seg.b];
                let Some((ei, ej)) = (0..2)
                    .flat_map(|ei| (0..2).map(move |ej| (ei, ej)))
                    .find(|(ei, ej)| (ends(i)[*ei] - ends(j)[*ej]).length() < 1e-6)
                else {
                    continue;
                };
                let corner = ends(i)[ei];
                // Each member's direction away from the corner.
                let u = if ei == 0 {
                    raws[i].axes.2
                } else {
                    -raws[i].axes.2
                };
                let v = if ej == 0 {
                    raws[j].axes.2
                } else {
                    -raws[j].axes.2
                };
                if u.dot(v) > 1.0 - 1e-9 {
                    return Err("two members of a group fold back onto each other".into());
                }
                let (cut_i, cut_j) = match group.corner {
                    CornerTreatment::EndMiter => {
                        let n = (u - v).normalize();
                        (
                            (corner + n * (group.gap / 2.0), n),
                            (corner - n * (group.gap / 2.0), -n),
                        )
                    }
                    CornerTreatment::EndButt1 | CornerTreatment::EndButt2 => {
                        // `through` runs past the corner to cover the other.
                        let (through, stops, ut, us) = if group.corner == CornerTreatment::EndButt1
                        {
                            (i, j, u, v)
                        } else {
                            (j, i, v, u)
                        };
                        let side = (us - ut * ut.dot(us)).normalize_or_zero();
                        if side == DVec3::ZERO {
                            return Err("butted members can't run straight on".into());
                        }
                        let (tx, ty, _) = raws[through].axes;
                        let (sx, sy, _) = raws[stops].axes;
                        let face_at = reach(&raws[through].points, tx, ty, side);
                        let cover = reach(&raws[stops].points, sx, sy, -ut);
                        let stop = (corner + side * (face_at + group.gap), side);
                        let run = (corner - ut * cover, ut);
                        if through == i {
                            (run, stop)
                        } else {
                            (stop, run)
                        }
                    }
                };
                raws[i].ends[ei] = Some(cut_i);
                raws[j].ends[ej] = Some(cut_j);
            }
        }
    }
    // Free ends are cut square at the segment's ends.
    let mut bodies: Vec<(usize, usize, Body, [f64; 2], f64)> = Vec::new();
    for raw in &mut raws {
        let z = raw.axes.2;
        let defaults = [(raw.seg.a, z), (raw.seg.b, -z)];
        let mut body = raw.body.clone();
        let mut angles = [0.0; 2];
        for e in 0..2 {
            let (point, keep) = raw.ends[e].unwrap_or(defaults[e]);
            let along = if e == 0 { z } else { -z };
            angles[e] = keep.dot(along).clamp(-1.0, 1.0).acos();
            let tool = half_space(point, keep)?;
            let r = body.shape.boolean(BoolOp::Common, &tool).map_err(|e| e.0)?;
            if r.shape.count(Kind::Solid) == 0 {
                return Err("a member's corner cuts leave nothing of it".into());
            }
            let name = format!("{fid}.g{}.{}.{}", raw.group, raw.index, ["start", "end"][e]);
            body = Body {
                names: Naming::from_history(
                    &r.shape,
                    &r.history,
                    Inputs {
                        named: [Some(&body.names), None],
                    },
                    &mut |_| Some(name.clone()),
                    &fid.to_string(),
                ),
                shape: r.shape,
            };
        }
        let length = {
            let shape = &body.shape;
            let along: Vec<f64> = (0..shape.count(Kind::Vertex))
                .filter_map(|v| shape.vertex_point(v).ok())
                .map(|q| q.dot(z))
                .collect();
            along.iter().copied().fold(f64::MIN, f64::max)
                - along.iter().copied().fold(f64::MAX, f64::min)
        };
        bodies.push((raw.group, raw.index, body, angles, length));
    }
    // Mitered groups may become one body each.
    let mut merged: Vec<(String, Body, Vec<Part>)> = Vec::new();
    for (g, i, body, angles, length) in bodies {
        let join = p.merge_miter_trimmed && p.groups[g].corner == CornerTreatment::EndMiter;
        let key = if join {
            format!("g{g}")
        } else {
            format!("g{g}.{i}")
        };
        match merged.iter_mut().find(|m| m.0 == key) {
            Some(m) => {
                let r =
                    m.1.shape
                        .boolean(BoolOp::Fuse, &body.shape)
                        .map_err(|e| e.0)?;
                m.1 = Body {
                    names: Naming::from_history(
                        &r.shape,
                        &r.history,
                        Inputs {
                            named: [Some(&m.1.names), Some(&body.names)],
                        },
                        &mut |_| None,
                        &fid.to_string(),
                    ),
                    shape: r.shape,
                };
                m.2.push((g, i, angles, length));
            }
            None => merged.push((key, body, vec![(g, i, angles, length)])),
        }
    }
    for (key, body, parts) in merged {
        let id = BodyId(format!("b{fid}:{key}"));
        for (g, i, angles, length) in parts {
            state.members.push(MemberRecord {
                feature: fid,
                body: id.clone(),
                group: g,
                segment: i,
                profile: p.profile.clone(),
                length,
                angles,
            });
        }
        state.bodies.push(SolidBody { id, geometry: body });
    }
    state.aggregate_bodies()
}
