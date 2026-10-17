//! Building solid features.

use std::f64::consts::{PI, TAU};

use of_geom::{
    BoolOp, Curve2 as GCurve, DAffine3, DVec2, DVec3, HistoryEntry, Kind, Plane, Profile,
    ProfileFace, Relation, Shape,
};
use of_sketch::{Curve2 as SCurve, Geometry, Sketch};

use crate::build::{Body, State, Tool};
use crate::features::holes;
use crate::naming::{Inputs, Naming};
use crate::resolve::{RResult, Resolved, resolve};
mod face_fillet;
mod rib;
pub(crate) mod sheet;
mod thread;
mod wrap;

use crate::{
    Chamfer, ChamferMode, CircularPattern, EndCondition, Extrude, FeatureId, FeatureKind, Fillet,
    Hole, HoleEnd, HoleKind, HoleStandard, Import, ImportFormat, LinearPattern, MirrorDef, Ref,
    Revolve, RevolveAngle, ShellDef, Thin, ThinKind, unit,
};

pub(crate) fn build(
    fid: FeatureId,
    kind: &FeatureKind,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let result = match kind {
        FeatureKind::Dome(p) => dome(fid, p, prev, state),
        FeatureKind::Rib(p) => rib::rib(fid, p, prev, state),
        FeatureKind::Thread(p) => thread::thread(fid, p, prev, state),
        FeatureKind::Wrap(p) => wrap::wrap(fid, p, prev, state),
        FeatureKind::SplitLine(p) => split_line(fid, p, prev, state),
        FeatureKind::Draft(p) => draft(fid, p, prev, state),
        FeatureKind::VariableFillet(p) => variable_fillet(fid, p, prev, state),
        FeatureKind::FaceFillet(p) => face_fillet::face_fillet(fid, p, prev, state),
        FeatureKind::Sweep(p) => sweep(fid, p, prev, state),
        FeatureKind::Loft(p) => loft(fid, p, prev, state),
        FeatureKind::Boundary(p) => boundary(fid, p, prev, state),
        FeatureKind::Extrude(e) => extrude(fid, e, prev, state),
        FeatureKind::Revolve(r) => revolve(fid, r, prev, state),
        FeatureKind::Fillet(f) => fillet(fid, f, prev, state),
        FeatureKind::Chamfer(c) => chamfer(fid, c, prev, state),
        FeatureKind::Shell(s) => shell(fid, s, prev, state),
        FeatureKind::Hole(h) => hole(fid, h, prev, state),
        FeatureKind::LinearPattern(p) => linear_pattern(fid, p, prev, state),
        FeatureKind::CircularPattern(p) => circular_pattern(fid, p, prev, state),
        FeatureKind::Mirror(m) => mirror(fid, m, prev, state),
        FeatureKind::Import(i) => import(fid, i, state),
        FeatureKind::BaseFlange(p) => sheet::base_flange(fid, p, prev, state),
        FeatureKind::EdgeFlange(p) => sheet::edge_flange(fid, p, prev, state),
        FeatureKind::Hem(p) => sheet::hem(fid, p, prev, state),
        FeatureKind::MiterFlange(p) => sheet::miter_flange(fid, p, prev, state),
        FeatureKind::SweptFlange(p) => sheet::swept_flange(fid, p, prev, state),
        FeatureKind::LoftedBend(p) => sheet::lofted_bend(fid, p, prev, state),
        _ => Ok(()),
    };
    if result.is_ok() && state.tools.contains_key(&fid) {
        state.tool_order.push(fid);
    }
    result
}

fn geo(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn dome(fid: FeatureId, p: &crate::Dome, prev: &State, state: &mut State) -> RResult<()> {
    let body = body_of(prev)?;
    let (kind, face) = prev.index_of(&p.face)?;
    if kind != Kind::Face {
        return Err("select a planar face for the dome".into());
    }
    let owners = prev
        .bodies
        .iter()
        .map(|solid| {
            solid
                .geometry
                .shape
                .indices_in(&body.shape, Kind::Face)
                .map(|indices| usize::from(indices.contains(&face)))
                .map_err(geo)
        })
        .collect::<RResult<Vec<_>>>()?
        .into_iter()
        .sum::<usize>();
    if owners != 1 {
        return Err(
            "dome face is shared by multiple bodies; select its owning body in Feature scope"
                .into(),
        );
    }
    let result = if p.elliptical {
        body.shape.dome_elliptical(face, p.height, p.inward)
    } else {
        body.shape.dome(face, p.height, p.inward)
    }
    .map_err(geo)?;
    let names = dressup_names(fid, "dome", body, &result);
    remember_dressup(fid, body, &result.shape, &names, state)?;
    state.body = Some(Body {
        shape: result.shape,
        names,
    });
    Ok(())
}

fn split_line(
    fid: FeatureId,
    p: &crate::ProjectedCurve,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let body = body_of(prev)?;
    let (kind, face) = prev.index_of(&p.target)?;
    if kind != Kind::Face {
        return Err("select a face to split".into());
    }
    let result = match p.mode {
        crate::ProjectionMode::OntoFace | crate::ProjectionMode::SketchOnSketch => {
            let wire = crate::curves::projected(p, prev)?;
            body.shape.split_face(face, &wire).map_err(geo)?
        }
        crate::ProjectionMode::Intersection => {
            let target = body.shape.sub_shape(Kind::Face, face).map_err(geo)?;
            let tool = match resolve(prev, &p.source)? {
                Resolved::Plane(plane) => big_face(prev, &plane)?,
                Resolved::Face { index, .. } => {
                    body.shape.sub_shape(Kind::Face, index).map_err(geo)?
                }
                Resolved::Body { id, .. } => prev.solid(&id)?.geometry.shape.clone(),
                _ => return Err("split with a plane, face or body".into()),
            };
            let edges = target
                .section(&tool)
                .map_err(|_| "the splitting geometry does not cross the face".to_string())?;
            body.shape.split_tools(&edges).map_err(geo)?
        }
        crate::ProjectionMode::Silhouette => {
            let edges = silhouette(&body.shape, face, p.direction)?;
            body.shape.split_tools(&edges).map_err(geo)?
        }
    };
    if result.shape.count(Kind::Face) <= body.shape.count(Kind::Face) {
        return Err("the split line does not divide the face".into());
    }
    let volume = |s: &Shape| s.mass_props().map(|m| m.volume).map_err(geo);
    let before = volume(&body.shape)?;
    if (volume(&result.shape)? - before).abs() > 1e-7 * before.abs().max(1.0) {
        return Err("splitting changed the part's volume".into());
    }
    let names = dressup_names(fid, "split", body, &result);
    state.body = Some(Body {
        shape: result.shape,
        names,
    });
    Ok(())
}

/// Silhouette edges of a cylindrical or spherical face seen along `direction`:
/// where the surface normal is perpendicular to the direction.
fn silhouette(shape: &Shape, face: u32, direction: DVec3) -> RResult<Shape> {
    if !direction.is_finite() || direction.length() < 1e-12 {
        return Err("a silhouette needs a pull direction".into());
    }
    let d = direction.normalize();
    let info = shape.face_info(face).map_err(geo)?;
    let reach = shape.bbox().map_or(1.0, |b| b.diagonal()) * 2.0 + 1.0;
    match info.kind {
        of_geom::SurfaceKind::Cylinder => {
            let axis = info.axis_dir.normalize();
            let side = axis.cross(d);
            if side.length() < 1e-9 {
                return Err(
                    "the pull direction runs along the cylinder; it has no silhouette".into(),
                );
            }
            let side = side.normalize() * info.radius;
            let lines = [side, -side].map(|offset| of_geom::Curve3::Line {
                a: info.axis_origin + offset - axis * reach,
                b: info.axis_origin + offset + axis * reach,
            });
            let wires = lines
                .iter()
                .map(|l| Shape::wire3(std::slice::from_ref(l)))
                .collect::<Result<Vec<_>, _>>()
                .map_err(geo)?;
            Ok(Shape::compound(&wires))
        }
        of_geom::SurfaceKind::Sphere => Shape::wire(
            &Plane::new(info.axis_origin, d.any_orthonormal_vector(), d),
            &[GCurve::Circle {
                center: DVec2::ZERO,
                radius: info.radius,
            }],
        )
        .map_err(geo),
        _ => Err("silhouette split lines support cylindrical and spherical faces".into()),
    }
}

fn variable_fillet(
    fid: FeatureId,
    p: &crate::VariableFillet,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let body = body_of(prev)?;
    let (kind, edge) = prev.index_of(&p.edge)?;
    if kind != Kind::Edge {
        return Err("select an edge to fillet".into());
    }
    let stations: Vec<_> = if p.reverse {
        p.stations
            .iter()
            .rev()
            .map(|s| (1.0 - s.position, s.radius))
            .collect()
    } else {
        p.stations.iter().map(|s| (s.position, s.radius)).collect()
    };
    let result = body
        .shape
        .variable_fillet(edge, &stations, p.straight)
        .map_err(geo)?;
    let names = dressup_names(fid, "vf", body, &result);
    remember_dressup(fid, body, &result.shape, &names, state)?;
    state.body = Some(Body {
        shape: result.shape,
        names,
    });
    Ok(())
}

fn draft(fid: FeatureId, p: &crate::Draft, prev: &State, state: &mut State) -> RResult<()> {
    let body = body_of(prev)?;
    let plane = resolve(prev, &p.neutral_plane)?.plane()?;
    let direction = match &p.direction {
        Some(r) => resolve(prev, r)?.direction()?,
        None => plane.normal,
    };
    if !p.parting_line.is_empty() {
        if !p.faces.is_empty() {
            return Err(
                "a parting-line draft chooses its faces from the edges; clear the face list".into(),
            );
        }
        let hinged = parting_faces(prev, &p.parting_line, direction, p.other_side)?;
        let result = if p.step {
            step_draft(body, &hinged, direction, p.angle, p.other_side)?
        } else {
            let hinged = hinged
                .into_iter()
                .map(|f| f.hinge.map(|h| (f.face, h)))
                .collect::<Option<Vec<_>>>()
                .ok_or(
                    "a face meets the parting line along edges that do not share a plane; use a step draft",
                )?;
            body.shape
                .draft_hinged(&hinged, direction, p.angle)
                .map_err(geo)?
        };
        let names = dressup_names(fid, "draft", body, &result);
        remember_dressup(fid, body, &result.shape, &names, state)?;
        state.body = Some(Body {
            shape: result.shape,
            names,
        });
        return Ok(());
    }
    if p.faces.is_empty() {
        return Err("select faces to draft, or parting-line edges".into());
    }
    if p.step {
        return Err("a step draft needs parting-line edges".into());
    }
    let faces = p
        .faces
        .iter()
        .map(|r| {
            let (kind, index) = prev.index_of(r)?;
            if kind != Kind::Face {
                return Err("select faces to draft".into());
            }
            Ok(index)
        })
        .collect::<RResult<Vec<_>>>()?;
    let result = body
        .shape
        .draft(&faces, &plane, direction, p.angle)
        .map_err(geo)?;
    let names = dressup_names(fid, "draft", body, &result);
    remember_dressup(fid, body, &result.shape, &names, state)?;
    state.body = Some(Body {
        shape: result.shape,
        names,
    });
    Ok(())
}

/// Step draft: each planar face tilts about a line normal to the pull direction
/// through its parting line's nearest point, so the tilt is a true draft even
/// where the parting line climbs. The material between the face and its tilted
/// plane is removed or added inside the column standing on the face, whose walls
/// along the parting line become the perpendicular step faces.
fn step_draft(
    body: &Body,
    faces: &[PartingFace],
    pull: DVec3,
    angle: f64,
    other_side: bool,
) -> RResult<of_geom::OpResult> {
    let pull = pull.normalize();
    let side = if other_side { -1.0 } else { 1.0 };
    let scale = body.shape.bbox().map_or(1.0, |b| b.diagonal().max(1.0));
    let remove = side * angle > 0.0;
    let mut tools = Vec::new();
    for PartingFace { face, points, .. } in faces {
        let info = body.shape.face_info(*face).map_err(geo)?;
        if info.kind != of_geom::SurfaceKind::Plane {
            return Err("a step draft needs planar faces next to the parting line".into());
        }
        let n = info.normal.normalize();
        let up = pull - n * pull.dot(n);
        if up.length() < 1e-6 {
            return Err("a face next to the parting line is normal to the pull direction".into());
        }
        let up = up.normalize();
        let hinge = points
            .iter()
            .copied()
            .min_by(|a, b| (side * a.dot(pull)).total_cmp(&(side * b.dot(pull))))
            .ok_or("a parting-line edge has no points")?;
        // Tilt so that the face moves into the material going along the pull.
        let tilted = n * angle.cos() + up * angle.sin();
        let plane = Plane::new(hinge, n.cross(tilted), tilted);
        let outside = hinge + tilted * scale;
        let half = Shape::half_space(
            &Shape::plane_face(&plane).map_err(geo)?,
            if remove {
                outside
            } else {
                hinge - tilted * scale
            },
        )
        .map_err(geo)?;
        let column = body
            .shape
            .sub_shape(Kind::Face, *face)
            .map_err(geo)?
            .prism(if remove { -n * scale } else { n * scale })
            .map_err(geo)?;
        let tool = column
            .shape
            .boolean(of_geom::BoolOp::Common, &half)
            .map_err(geo)?;
        tools.push(tool.shape);
    }
    let tools = Shape::compound(&tools);
    let op = if remove {
        of_geom::BoolOp::Cut
    } else {
        of_geom::BoolOp::Fuse
    };
    let result = body.shape.boolean(op, &tools).map_err(geo)?;
    if result.shape.count(Kind::Solid) != body.shape.count(Kind::Solid) {
        return Err("the step draft splits or joins bodies; reduce the angle".into());
    }
    Ok(result)
}

/// A face next to a parting line: its hinge plane through its parting edges
/// (missing when they do not share a plane) and points along those edges.
struct PartingFace {
    face: u32,
    hinge: Option<Plane>,
    points: Vec<DVec3>,
}

/// For a parting-line draft: each face next to a parting edge on the chosen side
/// of the pull direction, with a hinge plane through its edge and points sampled
/// along all its parting edges. The hinge is missing when the face's parting
/// edges do not share a plane.
fn parting_faces(
    prev: &State,
    edges: &[Ref],
    pull: DVec3,
    other_side: bool,
) -> RResult<Vec<PartingFace>> {
    let body = body_of(prev)?;
    let pull = pull.normalize();
    let adjacent = body.shape.edge_faces();
    let scale = body.shape.bbox().map_or(1.0, |b| b.diagonal().max(1.0));
    let tol = 1e-7 * scale;
    let mut out: Vec<PartingFace> = Vec::new();
    for r in edges {
        let (kind, edge) = prev.index_of(r)?;
        if kind != Kind::Edge {
            return Err("parting-line references must be edges".into());
        }
        let info = body.shape.edge_info(edge).map_err(geo)?;
        let hinge = match info.kind {
            of_geom::CurveKind::Line => {
                let along = (info.end - info.start).normalize();
                let normal = pull - along * pull.dot(along);
                if normal.length() < 1e-6 {
                    return Err("a parting-line edge runs along the pull direction".into());
                }
                Plane::new(info.start, along, normal.normalize())
            }
            of_geom::CurveKind::Circle => {
                let x = (info.start - info.center).normalize_or_zero();
                let x = if x.length() > 0.5 {
                    x
                } else {
                    info.axis.any_orthonormal_vector()
                };
                Plane::new(info.center, x, info.axis.normalize())
            }
            _ => return Err("parting-line edges must be straight or circular".into()),
        };
        let side = |face: u32| -> RResult<f64> {
            Ok((body.shape.face_info(face).map_err(geo)?.centroid - info.mid).dot(pull))
        };
        let mut chosen = None;
        for &face in &adjacent[edge as usize] {
            let offset = side(face)?;
            if (!other_side && offset > tol) || (other_side && offset < -tol) {
                chosen = Some(face);
            }
        }
        let face = chosen.ok_or(
            "no face lies on that side of a parting-line edge; flip the side or the pull direction",
        )?;
        let samples = body
            .shape
            .edge_samples(edge, 1e-3 * scale, 0.05)
            .map_err(geo)?;
        match out.iter_mut().find(|f| f.face == face) {
            Some(PartingFace {
                hinge: existing,
                points,
                ..
            }) => {
                // A face meeting the parting line along several edges hinges about
                // one plane, unless the draft steps.
                let on = |e: &Plane, p: DVec3| (p - e.origin).dot(e.normal).abs() < tol;
                if existing
                    .is_some_and(|e| !(on(&e, info.start) && on(&e, info.end) && on(&e, info.mid)))
                {
                    *existing = None;
                }
                points.extend(samples);
            }
            None => out.push(PartingFace {
                face,
                hinge: Some(hinge),
                points: samples,
            }),
        }
    }
    Ok(out)
}

fn section_names(prev: &State, id: FeatureId, shape: &Shape) -> RResult<Vec<String>> {
    let (plane, sketch) = sketch_of(prev, id)?;
    let curves = sketch
        .curve_ids()
        .filter_map(|id| sketch.curve(id).ok().map(|c| (id, c)))
        .collect::<Vec<_>>();
    // An entity split by an interior seam gives two edges: the one after the
    // seam comes first in the section, the one before it last.
    let mut seen = std::collections::HashSet::new();
    let names = (0..shape.count(Kind::Edge))
        .map(|edge| {
            let mid = plane.to_local(shape.edge_info(edge).map_err(geo)?.mid);
            let (entity, _) = curves
                .iter()
                .min_by(|a, b| {
                    (a.1.closest_point(mid) - mid)
                        .length()
                        .total_cmp(&(b.1.closest_point(mid) - mid).length())
                })
                .ok_or("profile has no curves")?;
            Ok(format!("s{id}.e{entity}"))
        })
        .collect::<RResult<Vec<String>>>()?;
    Ok(names
        .into_iter()
        .map(|n| {
            if seen.insert(n.clone()) {
                n
            } else {
                format!("{n}.before")
            }
        })
        .collect())
}

/// How a sweep orients its section along the path.
pub(crate) enum SweepFrame<'a> {
    Mode(u8),
    Guided {
        guide: Option<&'a Shape>,
        twist: f64,
    },
    /// Guides that scale (one) or stretch (two) the section.
    Scaled {
        guide: &'a Shape,
        second: Option<&'a Shape>,
        scale: bool,
    },
}

/// Sweep one closed section wire, naming its side faces after `edges` (one per
/// section edge) and its caps with `cap_prefix`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sweep_section(
    fid: FeatureId,
    section: &Shape,
    plane: &Plane,
    edges: &[String],
    path: &Shape,
    frame: &SweepFrame,
    cap_prefix: &str,
) -> RResult<Body> {
    let result = match frame {
        SweepFrame::Mode(orientation) => section.sweep(path, plane, *orientation),
        SweepFrame::Guided { guide, twist } => section.sweep_guided(path, *guide, *twist),
        SweepFrame::Scaled {
            guide,
            second,
            scale,
        } => section.sweep_scaled(path, guide, *second, *scale),
    }
    .map_err(geo)?;
    let names = Naming::from_history(
        &result.shape,
        &result.history,
        Inputs {
            named: [None, None],
        },
        &mut |h| {
            if h.rel == Relation::StartCap {
                return Some(format!("{fid}.{cap_prefix}start"));
            }
            if h.rel == Relation::EndCap {
                return Some(format!("{fid}.{cap_prefix}end"));
            }
            (h.input == 0 && h.from_kind == Kind::Edge)
                .then(|| edges.get(h.from as usize).map(|e| format!("{fid}.{e}")))
                .flatten()
        },
        &fid.to_string(),
    );
    Ok(Body {
        shape: result.shape,
        names,
    })
}

fn sweep_profile(
    fid: FeatureId,
    profile: &crate::ProfileSelection,
    prev: &State,
    path: &Shape,
    frame: &SweepFrame,
    cap_prefix: &str,
) -> RResult<Body> {
    let (plane, section) = crate::curves::section(prev, profile)?;
    let edges = section_names(prev, profile.sketch(), &section)?;
    sweep_section(fid, &section, &plane, &edges, path, frame, cap_prefix)
}

/// Subtract a swept hole from the outer sweep after checking it stays inside.
fn subtract_swept_hole(fid: FeatureId, body: Body, tool: Body) -> RResult<Body> {
    // Curvature can fold an inner section outside the swept outer profile even
    // when the sketch is valid, so check full containment after sweeping.
    let excess = tool.shape.boolean(BoolOp::Cut, &body.shape).map_err(geo)?;
    let tolerance = (tool.shape.mass_props().map_err(geo)?.volume.abs() * 1e-8).max(1e-8);
    if excess.shape.count(Kind::Solid) > 0
        && excess.shape.mass_props().map_err(geo)?.volume.abs() > tolerance
    {
        return Err(
            "a swept profile hole leaves the outer sweep; reduce curvature or hole size".into(),
        );
    }
    let result = body.shape.boolean(BoolOp::Cut, &tool.shape).map_err(geo)?;
    let names = Naming::from_history(
        &result.shape,
        &result.history,
        Inputs {
            named: [Some(&body.names), Some(&tool.names)],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    Ok(Body {
        shape: result.shape,
        names,
    })
}

fn sweep(fid: FeatureId, p: &crate::Sweep, prev: &State, state: &mut State) -> RResult<()> {
    crate::curves::validate_inner_profiles(prev, p)?;
    let path = crate::curves::path(prev, &p.path)?;
    let path = if p.reverse_path {
        path.reversed()
    } else {
        path
    };
    let guide = p
        .guide
        .as_ref()
        .map(|g| crate::curves::path(prev, g))
        .transpose()?;
    if !p.twist.is_finite() {
        return Err("the twist angle must be finite".into());
    }
    let second = p
        .guide2
        .as_ref()
        .map(|g| crate::curves::path(prev, g))
        .transpose()?;
    if (p.guide_scale || second.is_some()) && guide.is_none() {
        return Err("select the first guide curve".into());
    }
    if (p.guide_scale || second.is_some()) && p.twist != 0.0 {
        return Err("a sweep twists or follows scaling guides, not both".into());
    }
    let frame = if let (Some(guide), true) = (&guide, p.guide_scale || second.is_some()) {
        SweepFrame::Scaled {
            guide,
            second: second.as_ref(),
            scale: p.guide_scale,
        }
    } else if guide.is_some() || p.twist != 0.0 {
        SweepFrame::Guided {
            guide: guide.as_ref(),
            twist: p.twist,
        }
    } else {
        SweepFrame::Mode(match p.orientation {
            crate::SweepOrientation::MinimumTwist => 0,
            crate::SweepOrientation::Frenet => 1,
            crate::SweepOrientation::Fixed => 2,
        })
    };
    let mut body = match (&p.profile, p.circle_diameter, &p.thin) {
        (Some(_), Some(_), _) => {
            return Err("sweep a sketch profile or a circle, not both".into());
        }
        (None, None, _) => return Err("select a profile or set a circle diameter".into()),
        (None, Some(_), Some(_)) => return Err("a thin sweep needs a sketch profile".into()),
        (None, Some(d), None) => {
            if !(d.is_finite() && d > 1e-9) {
                return Err("the circle diameter must be more than zero".into());
            }
            let start = path.path_start().map_err(geo)?;
            let circle = Shape::wire(
                &start,
                &[GCurve::Circle {
                    center: DVec2::ZERO,
                    radius: d / 2.0,
                }],
            )
            .map_err(geo)?;
            sweep_section(
                fid,
                &circle,
                &start,
                &["circle".to_string()],
                &path,
                &frame,
                "",
            )?
        }
        (Some(profile), None, Some(thin)) => {
            let crate::ProfileSelection::Sketch(sketch) = profile else {
                return Err("a thin sweep uses the whole profile sketch".into());
            };
            if !p.inner_profiles.is_empty() {
                return Err("a thin sweep cannot also have profile holes".into());
            }
            let (plane, sk) = sketch_of(prev, *sketch)?;
            let contours = sk.contours().map_err(geo)?;
            if contours.loops.len() + contours.open.len() != 1 {
                return Err("a thin sweep needs a sketch with one connected chain".into());
            }
            let faces = thin_faces(sk, &contours, thin)?;
            let face = &faces[0];
            let outer = Shape::wire(plane, &face.outer).map_err(geo)?;
            let mut body = sweep_section(
                fid,
                &outer,
                plane,
                &vec!["thin".to_string(); outer.count(Kind::Edge) as usize],
                &path,
                &frame,
                "",
            )?;
            for hole in &face.holes {
                let wire = Shape::wire(plane, hole).map_err(geo)?;
                let tool = sweep_section(
                    fid,
                    &wire,
                    plane,
                    &vec!["thin.in".to_string(); wire.count(Kind::Edge) as usize],
                    &path,
                    &frame,
                    "hole.",
                )?;
                body = subtract_swept_hole(fid, body, tool)?;
            }
            body
        }
        (Some(profile), None, None) => sweep_profile(fid, profile, prev, &path, &frame, "")?,
    };
    for inner in &p.inner_profiles {
        let mut entities = inner.entities.clone();
        entities.sort_unstable();
        let cap_prefix = format!(
            "hole:{}.",
            entities
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join("-")
        );
        let tool = sweep_profile(
            fid,
            &crate::ProfileSelection::Contour(inner.clone()),
            prev,
            &path,
            &frame,
            &cap_prefix,
        )?;
        body = subtract_swept_hole(fid, body, tool)?;
    }
    apply_feature_tool(
        fid,
        state,
        body.shape,
        body.names,
        if p.cut { BoolOp::Cut } else { BoolOp::Fuse },
        p.merge,
    )
}

fn loft(fid: FeatureId, p: &crate::Loft, prev: &State, state: &mut State) -> RResult<()> {
    if p.profiles.len() < 2 {
        return Err("select at least two ordered profiles".into());
    }
    if p.closed && !p.guides.is_empty() {
        return Err("a closed loft can't also use guide curves".into());
    }
    let ends = !p.start.is_free() || !p.end.is_free();
    if ends && (p.closed || !p.guides.is_empty() || p.centerline.is_some()) {
        return Err(
            "start/end conditions can't be combined with guides, a centerline or a closed loft"
                .into(),
        );
    }
    if p.centerline.is_some() && (p.closed || !p.guides.is_empty()) {
        return Err("a centerline loft can't also use guide curves or be closed".into());
    }
    if ends && p.ruled {
        return Err("a ruled loft has no start/end conditions".into());
    }
    let mut unique = std::collections::HashSet::new();
    if p.profiles
        .iter()
        .any(|profile| !unique.insert(profile.sketch()))
    {
        return Err("each loft profile must be different".into());
    }
    let mut profiles = Vec::new();
    let mut edges = Vec::new();
    for id in &p.profiles {
        let (_, section) = crate::curves::section(prev, id)?;
        let section = if p.reversed.contains(&id.sketch()) {
            section.reversed()
        } else {
            section
        };
        edges.extend(section_names(prev, id.sketch(), &section)?);
        profiles.push(section);
    }
    if !p.guides.is_empty() {
        if p.guides.len() < 2 {
            return Err("loft guide curves need at least two curves; add another guide".into());
        }
        let guides = p
            .guides
            .iter()
            .map(|r| crate::curves::path(prev, r))
            .collect::<RResult<Vec<_>>>()?;
        let result = Shape::boundary(&profiles, &guides).map_err(geo)?;
        let gaps = guides.len() as u32;
        let names = Naming::from_history(
            &result.shape,
            &result.history,
            Inputs {
                named: [None, None],
            },
            &mut |h| match h.rel {
                Relation::StartCap => Some(format!("{fid}.start")),
                Relation::EndCap => Some(format!("{fid}.end")),
                Relation::Generated => Some(format!("{fid}.g{}.{}", h.from / gaps, h.from % gaps)),
                _ => None,
            },
            &fid.to_string(),
        );
        return apply_feature_tool(
            fid,
            state,
            result.shape,
            names,
            if p.cut { BoolOp::Cut } else { BoolOp::Fuse },
            p.merge,
        );
    }
    let explicit = p
        .profiles
        .iter()
        .any(|p| matches!(p, crate::ProfileSelection::Contour(_)));
    let named = |result: of_geom::OpResult| {
        let names = Naming::from_history(
            &result.shape,
            &result.history,
            Inputs {
                named: [None, None],
            },
            &mut |h| {
                if h.rel == Relation::StartCap {
                    return Some(format!("{fid}.start"));
                }
                if h.rel == Relation::EndCap {
                    return Some(format!("{fid}.end"));
                }
                (h.from_kind == Kind::Edge)
                    .then(|| edges.get(h.from as usize).map(|e| format!("{fid}.{e}")))
                    .flatten()
            },
            &fid.to_string(),
        );
        (result.shape, names)
    };
    if let Some(centerline) = &p.centerline {
        let centerline = crate::curves::path(prev, centerline)?;
        let (shape, names) = named(Shape::loft_centerline(&profiles, &centerline).map_err(geo)?);
        return apply_feature_tool(
            fid,
            state,
            shape,
            names,
            if p.cut { BoolOp::Cut } else { BoolOp::Fuse },
            p.merge,
        );
    }
    if ends {
        let condition = |end: &crate::LoftEnd| -> RResult<of_geom::EndTangent> {
            Ok(match end {
                crate::LoftEnd::None => of_geom::EndTangent::Free,
                crate::LoftEnd::Normal { tangent_length } => {
                    of_geom::EndTangent::Normal(*tangent_length)
                }
                crate::LoftEnd::Direction {
                    direction,
                    flip,
                    tangent_length,
                } => {
                    let d = resolve(prev, direction)?.direction()?;
                    of_geom::EndTangent::Direction(if *flip { -d } else { d }, *tangent_length)
                }
            })
        };
        let result = Shape::loft_tangent(
            &profiles,
            explicit,
            condition(&p.start)?,
            condition(&p.end)?,
        )
        .map_err(geo)?;
        let (shape, names) = named(result);
        return apply_feature_tool(
            fid,
            state,
            shape,
            names,
            if p.cut { BoolOp::Cut } else { BoolOp::Fuse },
            p.merge,
        );
    }
    if p.closed {
        if p.profiles.len() < 3 {
            return Err("a closed loft needs at least three sections".into());
        }
        // The kernel closes a loft whose last section is the first one again.
        profiles.push(profiles[0].clone());
    }
    if explicit
        && profiles
            .iter()
            .any(|p| p.count(Kind::Edge) != profiles[0].count(Kind::Edge))
    {
        return Err(
            "explicit loft correspondence requires the same number of edges in each section".into(),
        );
    }
    let (shape, names) = named(Shape::loft_ordered(&profiles, p.ruled, explicit).map_err(geo)?);
    apply_feature_tool(
        fid,
        state,
        shape,
        names,
        if p.cut { BoolOp::Cut } else { BoolOp::Fuse },
        p.merge,
    )
}

fn boundary(fid: FeatureId, p: &crate::Boundary, prev: &State, state: &mut State) -> RResult<()> {
    if p.sections.len() < 2 {
        return Err("select at least two direction 1 sections".into());
    }
    if p.curves.len() < 2 {
        return Err("select at least two direction 2 curves".into());
    }
    let mut sections = Vec::new();
    for selection in &p.sections {
        sections.push(crate::curves::section(prev, selection)?.1);
    }
    let curves = p
        .curves
        .iter()
        .map(|r| crate::curves::path(prev, r))
        .collect::<RResult<Vec<_>>>()?;
    let result = Shape::boundary(&sections, &curves).map_err(geo)?;
    let gaps = p.curves.len() as u32;
    let names = Naming::from_history(
        &result.shape,
        &result.history,
        Inputs {
            named: [None, None],
        },
        &mut |h| match h.rel {
            Relation::StartCap => Some(format!("{fid}.start")),
            Relation::EndCap => Some(format!("{fid}.end")),
            Relation::Generated => Some(format!("{fid}.b{}.{}", h.from / gaps, h.from % gaps)),
            _ => None,
        },
        &fid.to_string(),
    );
    apply_feature_tool(
        fid,
        state,
        result.shape,
        names,
        if p.cut { BoolOp::Cut } else { BoolOp::Fuse },
        p.merge,
    )
}

pub(crate) fn to_geom(c: &SCurve) -> GCurve {
    match c {
        SCurve::Line { a, b } => GCurve::Line { a: *a, b: *b },
        SCurve::Bezier { points } => GCurve::Bezier { points: *points },
        SCurve::Arc { center, start, end } => GCurve::Arc {
            center: *center,
            start: *start,
            end: *end,
        },
        SCurve::Circle { center, radius } => GCurve::Circle {
            center: *center,
            radius: *radius,
        },
        SCurve::Spline { points, closed } => GCurve::Spline {
            points: points.clone(),
            closed: *closed,
        },
    }
}

/// A sketch profile as kernel faces, with a name for each face edge.
pub(crate) struct ProfileShape {
    pub(crate) shape: Shape,
    pub(crate) plane: Plane,
    /// Per kernel edge of `shape`: `s{entity}` for the sketch curve it came from.
    pub(crate) edge_names: Vec<String>,
}

fn sketch_of(prev: &State, id: FeatureId) -> RResult<&(Plane, Sketch)> {
    prev.sketches.get(&id).ok_or_else(|| {
        "the sketch is missing, failed to build or comes later in the feature tree".to_string()
    })
}

/// A sketch's closed regions as profile faces, outer loops with their holes.
pub(crate) fn closed_regions(contours: &of_sketch::Contours) -> RResult<Vec<ProfileFace>> {
    if contours.regions.is_empty() {
        return Err("the sketch has no closed contours".into());
    }
    Ok(contours
        .regions
        .iter()
        .map(|r| ProfileFace {
            outer: contours.loops[r.outer].curves.iter().map(to_geom).collect(),
            holes: r
                .holes
                .iter()
                .map(|h| contours.loops[*h].curves.iter().map(to_geom).collect())
                .collect(),
        })
        .collect())
}

fn sketch_profile(prev: &State, sketch: FeatureId, thin: Option<&Thin>) -> RResult<ProfileShape> {
    let (plane, sk) = sketch_of(prev, sketch)?;
    let contours = sk.contours().map_err(geo)?;
    let faces: Vec<ProfileFace> = match thin {
        None => closed_regions(&contours)?,
        Some(t) => thin_faces(sk, &contours, t)?,
    };
    let profile = Profile {
        plane: *plane,
        faces,
    };
    let shape = profile.to_shape().map_err(geo)?;
    // Which sketch curve each kernel edge came from, by nearest midpoint.
    let candidates: Vec<(of_sketch::Id, SCurve)> = contours
        .loops
        .iter()
        .chain(&contours.open)
        .flat_map(|c| c.entities.iter().copied().zip(c.curves.iter().cloned()))
        .collect();
    let edge_names = (0..shape.count(Kind::Edge))
        .map(|i| {
            let Ok(info) = shape.edge_info(i) else {
                return "e".to_string();
            };
            let mid = plane.to_local(info.mid);
            let best = candidates.iter().min_by(|a, b| {
                let da = (a.1.closest_point(mid) - mid).length();
                let db = (b.1.closest_point(mid) - mid).length();
                da.total_cmp(&db)
            });
            match best {
                Some((id, c)) if thin.is_none() && (c.closest_point(mid) - mid).length() < 1e-4 => {
                    format!("s{id}")
                }
                Some((id, _)) => format!("t{id}"),
                None => "e".to_string(),
            }
        })
        .collect();
    Ok(ProfileShape {
        shape,
        plane: *plane,
        edge_names,
    })
}

/// Faces for a thin feature: each contour thickened to the given sides.
fn thin_faces(sk: &Sketch, contours: &of_sketch::Contours, t: &Thin) -> RResult<Vec<ProfileFace>> {
    if t.thickness <= 0.0 {
        return Err("the wall thickness must be more than zero".into());
    }
    let (mut left, mut right) = match t.kind {
        ThinKind::OneDirection => (t.thickness, 0.0),
        ThinKind::MidPlane => (t.thickness / 2.0, t.thickness / 2.0),
        ThinKind::TwoDirection => (t.thickness, t.thickness2.max(0.0)),
    };
    if t.reverse {
        std::mem::swap(&mut left, &mut right);
    }
    let mut faces = Vec::new();
    let chains: Vec<(&of_sketch::Chain, bool)> = contours
        .loops
        .iter()
        .map(|c| (c, true))
        .chain(contours.open.iter().map(|c| (c, false)))
        .collect();
    if chains.is_empty() {
        return Err("the sketch has no contours".into());
    }
    for (chain, closed) in chains {
        let side = |d: f64| -> RResult<Vec<GCurve>> {
            if d == 0.0 {
                return Ok(chain.curves.iter().map(to_geom).collect());
            }
            // Offset a copy of just this chain.
            let mut tmp = Sketch::new();
            let ids = copy_chain(sk, &chain.entities, &mut tmp);
            let made = of_sketch::tools::offset(&mut tmp, &ids, d).map_err(geo)?;
            made.entities
                .iter()
                .map(|e| tmp.curve(*e).map(|c| to_geom(&c)).map_err(geo))
                .collect()
        };
        let (a, b) = (side(left)?, side(-right)?);
        if closed {
            let area = |c: &[GCurve]| -> f64 {
                let pts: Vec<DVec2> = c.iter().flat_map(sample).collect();
                of_sketch::geom2::signed_area(&pts).abs()
            };
            let (outer, inner) = if area(&a) >= area(&b) { (a, b) } else { (b, a) };
            faces.push(ProfileFace {
                outer,
                holes: vec![inner],
            });
        } else {
            let (a0, a1) = chain_ends(&a);
            let (b0, b1) = chain_ends(&b);
            let mut outer = a;
            outer.push(GCurve::Line { a: a1, b: b1 });
            outer.extend(b.into_iter().rev());
            outer.push(GCurve::Line { a: b0, b: a0 });
            faces.push(ProfileFace {
                outer,
                holes: vec![],
            });
        }
    }
    Ok(faces)
}

fn sample(c: &GCurve) -> Vec<DVec2> {
    let s = match c {
        GCurve::Line { a, b } => SCurve::Line { a: *a, b: *b },
        GCurve::Arc { center, start, end } => SCurve::Arc {
            center: *center,
            start: *start,
            end: *end,
        },
        GCurve::Circle { center, radius } => SCurve::Circle {
            center: *center,
            radius: *radius,
        },
        GCurve::Bezier { points } => SCurve::Bezier { points: *points },
        GCurve::Spline { points, closed } => SCurve::Spline {
            points: points.clone(),
            closed: *closed,
        },
    };
    s.sample(0.01)
}

/// Start and end of an ordered chain of curves.
fn chain_ends(c: &[GCurve]) -> (DVec2, DVec2) {
    if c.len() == 1 {
        return (c[0].start(), c[0].end());
    }
    let near =
        |p: DVec2, q: &GCurve| (q.start() - p).length() < 1e-6 || (q.end() - p).length() < 1e-6;
    let start = if near(c[0].end(), &c[1]) {
        c[0].start()
    } else {
        c[0].end()
    };
    let n = c.len();
    let end = if near(c[n - 1].start(), &c[n - 2]) {
        c[n - 1].end()
    } else {
        c[n - 1].start()
    };
    (start, end)
}

/// Copy entities (and their points) into another sketch; returns new ids.
fn copy_chain(from: &Sketch, ids: &[of_sketch::Id], to: &mut Sketch) -> Vec<of_sketch::Id> {
    let mut map = std::collections::HashMap::new();
    let mut pt = |to: &mut Sketch, p: of_sketch::Id| -> of_sketch::Id {
        *map.entry(p)
            .or_insert_with(|| to.add_point(from.point(p).unwrap()))
    };
    ids.iter()
        .map(|id| {
            let geo = match from.entities[id].geo.clone() {
                Geometry::Line { a, b } => Geometry::Line {
                    a: pt(to, a),
                    b: pt(to, b),
                },
                Geometry::Arc { center, start, end } => Geometry::Arc {
                    center: pt(to, center),
                    start: pt(to, start),
                    end: pt(to, end),
                },
                Geometry::Circle { center, radius } => Geometry::Circle {
                    center: pt(to, center),
                    radius,
                },
                Geometry::Bezier { points } => Geometry::Bezier {
                    points: points.map(|p| pt(to, p)),
                },
                Geometry::Spline { points, closed } => Geometry::Spline {
                    points: points.iter().map(|p| pt(to, *p)).collect(),
                    closed,
                },
                g => g,
            };
            to.add_entity(of_sketch::Entity::new(geo)).unwrap()
        })
        .collect()
}

/// Name faces swept from a profile: sides after their sketch curves, plus caps.
pub(crate) fn sweep_names<'a>(
    fid: FeatureId,
    prefix: &str,
    edge_names: &'a [String],
) -> impl FnMut(&HistoryEntry) -> Option<String> + 'a {
    let prefix = prefix.to_string();
    move |h: &HistoryEntry| match (h.rel, h.from_kind) {
        (Relation::Generated, Kind::Edge) => Some(format!(
            "{fid}.{prefix}{}",
            edge_names
                .get(h.from as usize)
                .map(|s| s.as_str())
                .unwrap_or("e")
        )),
        (Relation::StartCap, _) => Some(format!("{fid}.{prefix}start")),
        (Relation::EndCap, _) => Some(format!("{fid}.{prefix}end")),
        _ => None,
    }
}

/// A length that reaches past everything in the model from this plane.
pub(crate) fn through_all_length(state: &State, plane: &Plane) -> f64 {
    match state.body.as_ref().and_then(|b| b.shape.bbox()) {
        Some(b) => b.diagonal() * 2.0 + (b.center() - plane.origin).length() * 2.0 + 10.0,
        None => 1000.0,
    }
}

/// A large square face on a plane, centered near the model.
fn big_face(state: &State, plane: &Plane) -> RResult<Shape> {
    let size = through_all_length(state, plane);
    let center = match state.body.as_ref().and_then(|b| b.shape.bbox()) {
        Some(b) => plane.to_local(b.center()),
        None => DVec2::ZERO,
    };
    let c = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(x, y)| center + DVec2::new(x, y) * size);
    let outer = (0..4)
        .map(|i| GCurve::Line {
            a: c[i],
            b: c[(i + 1) % 4],
        })
        .collect();
    Profile {
        plane: *plane,
        faces: vec![ProfileFace {
            outer,
            holes: vec![],
        }],
    }
    .to_shape()
    .map_err(geo)
}

fn translate(s: &Shape, v: DVec3) -> RResult<Shape> {
    Ok(s.transform(&DAffine3::from_translation(v))
        .map_err(geo)?
        .shape)
}

/// Sweep a profile along `dir` to an end condition.
fn extrude_side(
    fid: FeatureId,
    prof: &ProfileShape,
    dir: DVec3,
    cond: &EndCondition,
    prefix: &str,
    prev: &State,
) -> RResult<(Shape, Naming)> {
    let plane = prof.plane;
    let named = |r: of_geom::OpResult| {
        let mut generate = sweep_names(fid, prefix, &prof.edge_names);
        let names = Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [None, None],
            },
            &mut generate,
            &fid.to_string(),
        );
        (r.shape, names)
    };
    let blind = |depth: f64, from: &Shape| -> RResult<(Shape, Naming)> {
        if depth <= 1e-9 {
            return Err("the depth must be more than zero".into());
        }
        Ok(named(from.prism(dir * depth).map_err(geo)?))
    };
    let long = through_all_length(prev, &plane);
    match cond {
        EndCondition::Blind { depth } => blind(*depth, &prof.shape),
        EndCondition::MidPlane { depth } => {
            let start = translate(&prof.shape, -dir * (depth / 2.0))?;
            blind(*depth, &start)
        }
        EndCondition::ThroughAll => blind(long, &prof.shape),
        EndCondition::UpToVertex { target } => {
            let p = resolve(prev, target)?.point()?;
            let d = (p - plane.origin).dot(dir);
            if d <= 1e-9 {
                return Err("the vertex is behind the sketch plane".into());
            }
            blind(d, &prof.shape)
        }
        EndCondition::UpToSurface { target } | EndCondition::OffsetFromSurface { target, .. } => {
            let offset = match cond {
                EndCondition::OffsetFromSurface {
                    offset,
                    reverse_offset,
                    ..
                } => {
                    if *reverse_offset {
                        -offset
                    } else {
                        *offset
                    }
                }
                _ => 0.0,
            };
            let res = resolve(prev, target)?;
            match res.plane() {
                Ok(tp) => {
                    // The target plane, moved back toward the sketch by the offset.
                    let toward = if tp.normal.dot(dir) > 0.0 {
                        -tp.normal
                    } else {
                        tp.normal
                    };
                    let tp = Plane::new(tp.origin + toward * offset, tp.x_dir, tp.normal);
                    if tp.normal.cross(dir).length() < 1e-9 {
                        let d = (tp.origin - plane.origin).dot(dir);
                        return blind(d, &prof.shape);
                    }
                    let (s, n) = blind(long, &prof.shape)?;
                    let face = big_face(prev, &tp)?;
                    clip_by_face(fid, prefix, &s, &n, &face, &prof.shape)
                }
                Err(_) => {
                    let Resolved::Face { index, .. } = res else {
                        return Err("select a face, plane or surface to extrude up to".into());
                    };
                    let mut face = prev
                        .body
                        .as_ref()
                        .unwrap()
                        .shape
                        .sub_shape(Kind::Face, index)
                        .map_err(geo)?;
                    if offset != 0.0 {
                        face = face
                            .offset_face(offset, prof.shape.face_info(0).map_err(geo)?.centroid)
                            .map_err(geo)?;
                    }
                    let (s, n) = blind(long, &prof.shape)?;
                    clip_by_face(fid, prefix, &s, &n, &face, &prof.shape)
                }
            }
        }
        EndCondition::UpToNext => {
            let body = prev
                .body
                .as_ref()
                .ok_or("there's no body to extrude up to")?;
            let (s, n) = blind(long, &prof.shape)?;
            // Pieces of the sweep that are outside the body (for a boss) or
            // inside it (for a cut) and touch the sketch.
            let outside = s.boolean(BoolOp::Cut, &body.shape).map_err(geo)?;
            keep_touching(fid, prefix, outside, &n, &body.names, &prof.shape)
        }
    }
}

/// Keep only the solids of an operation's result that touch `start`.
fn keep_touching(
    fid: FeatureId,
    prefix: &str,
    r: of_geom::OpResult,
    names: &Naming,
    body_names: &Naming,
    start: &Shape,
) -> RResult<(Shape, Naming)> {
    keep_solids(
        fid,
        prefix,
        r,
        names,
        body_names,
        |s| s.distance(start).map(|d| d.value < 1e-6).unwrap_or(false),
        "nothing to extrude up to",
    )
}

/// Keep the solids of an operation's result accepted by `keep`, with names carried
/// through the boolean that removes the rest.
fn keep_solids(
    fid: FeatureId,
    prefix: &str,
    r: of_geom::OpResult,
    names: &Naming,
    body_names: &Naming,
    keep: impl Fn(&Shape) -> bool,
    empty: &str,
) -> RResult<(Shape, Naming)> {
    let mut generate = |_: &HistoryEntry| Some(format!("{fid}.{prefix}end"));
    let all = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(names), Some(body_names)],
        },
        &mut generate,
        &fid.to_string(),
    );
    let solids = r.shape.solids();
    let (kept, drop): (Vec<&Shape>, Vec<&Shape>) = solids.iter().partition(|s| keep(s));
    if kept.is_empty() {
        return Err(empty.into());
    }
    if drop.is_empty() {
        return Ok((r.shape, all));
    }
    let drop = Shape::compound(drop);
    let r2 = r.shape.boolean(BoolOp::Cut, &drop).map_err(geo)?;
    let n2 = Naming::from_history(
        &r2.shape,
        &r2.history,
        Inputs {
            named: [Some(&all), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    Ok((r2.shape, n2))
}

/// Cut a long sweep off at a face, keeping the side with the sketch.
fn clip_by_face(
    fid: FeatureId,
    prefix: &str,
    sweep: &Shape,
    names: &Naming,
    face: &Shape,
    start: &Shape,
) -> RResult<(Shape, Naming)> {
    let inside = start.face_info(0).map_err(geo)?.centroid;
    let half = Shape::half_space(face, inside).map_err(geo)?;
    let r = sweep.boolean(BoolOp::Common, &half).map_err(geo)?;
    if r.shape.count(Kind::Solid) == 0 {
        return Err("the extrude doesn't reach the surface".into());
    }
    // A curved surface can intersect the sweep again on its far side.
    // Keep only material connected to the starting profile.
    keep_touching(fid, prefix, r, names, &Naming::default(), start)
}

/// Combine a tool with the body and remember it for patterns.
/// The rebuild error for a feature cut that misses every body.
pub const CUT_REMOVES_NOTHING: &str = "the cut does not remove material";

/// [`apply_tool`] for a feature's own cut, which must remove material. Pattern and
/// mirror copies use `apply_tool` directly so one copy off the part doesn't fail them.
fn apply_feature_tool(
    fid: FeatureId,
    state: &mut State,
    tool: Shape,
    names: Naming,
    op: BoolOp,
    merge: bool,
) -> RResult<()> {
    let volume = |state: &State| -> RResult<f64> {
        Ok(state
            .body
            .as_ref()
            .map(|b| b.shape.mass_props().map(|m| m.volume.abs()))
            .transpose()
            .map_err(geo)?
            .unwrap_or(0.0))
    };
    let before = match (op, &state.body) {
        (BoolOp::Cut, Some(_)) => Some(volume(state)?),
        _ => None,
    };
    apply_tool(fid, state, tool, names, op, merge)?;
    if let Some(before) = before
        && before - volume(state)? <= (before * 1e-9).max(1e-9)
    {
        return Err(CUT_REMOVES_NOTHING.into());
    }
    Ok(())
}

fn apply_tool(
    fid: FeatureId,
    state: &mut State,
    tool: Shape,
    names: Naming,
    op: BoolOp,
    merge: bool,
) -> RResult<()> {
    let body = match (&state.body, op) {
        (None, BoolOp::Cut | BoolOp::Common) => return Err("there's no material to cut".into()),
        (None, BoolOp::Fuse) => Body {
            shape: tool.clone(),
            names: names.clone(),
        },
        (Some(b), BoolOp::Fuse) if !merge => {
            let shape = Shape::compound([&b.shape, &tool]);
            let mut faces = b.names.faces.clone();
            faces.extend(names.faces.iter().cloned());
            let names = Naming::from_faces(&shape, faces);
            Body { shape, names }
        }
        (Some(b), op) => {
            let r = b.shape.boolean(op, &tool).map_err(geo)?;
            if r.shape.count(Kind::Solid) == 0 {
                return Err(if op == BoolOp::Cut {
                    "the cut removes the whole body"
                } else {
                    "the result is empty"
                }
                .into());
            }
            let names = Naming::from_history(
                &r.shape,
                &r.history,
                Inputs {
                    named: [Some(&b.names), Some(&names)],
                },
                &mut |_| None,
                &fid.to_string(),
            );
            Body {
                shape: r.shape,
                names,
            }
        }
    };
    state.body = Some(body);
    state.tools.entry(fid).or_default().push(Tool {
        shape: tool,
        names,
        op,
    });
    Ok(())
}

fn extrude(fid: FeatureId, e: &Extrude, prev: &State, state: &mut State) -> RResult<()> {
    let mut prof = sketch_profile(prev, e.sketch, e.thin.as_ref())?;
    if e.start_offset != 0.0 {
        prof.shape = translate(&prof.shape, prof.plane.normal * e.start_offset)?;
        prof.plane = prof.plane.offset(e.start_offset);
    }
    let dir = if e.reverse {
        -prof.plane.normal
    } else {
        prof.plane.normal
    };
    let (mut tool, mut names) = extrude_side(fid, &prof, dir, &e.end1, "", prev)?;
    if let Some(end2) = &e.end2 {
        if matches!(e.end1, EndCondition::MidPlane { .. }) {
            return Err("mid plane can't be combined with a second direction".into());
        }
        let (t2, n2) = extrude_side(fid, &prof, -dir, end2, "d2.", prev)?;
        let r = tool.boolean(BoolOp::Fuse, &t2).map_err(geo)?;
        names = Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&names), Some(&n2)],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        tool = r.shape;
    }
    if e.cut && e.flip_side_to_cut {
        let slab_face = big_face(prev, &prof.plane)?;
        let slab_prof = ProfileShape {
            shape: slab_face,
            plane: prof.plane,
            edge_names: vec!["slab".into(); 4],
        };
        let (mut slab, mut slab_names) = extrude_side(fid, &slab_prof, dir, &e.end1, "", prev)?;
        if let Some(end2) = &e.end2 {
            let (t2, n2) = extrude_side(fid, &slab_prof, -dir, end2, "d2.", prev)?;
            let r = slab.boolean(BoolOp::Fuse, &t2).map_err(geo)?;
            slab_names = Naming::from_history(
                &r.shape,
                &r.history,
                Inputs {
                    named: [Some(&slab_names), Some(&n2)],
                },
                &mut |_| None,
                &fid.to_string(),
            );
            slab = r.shape;
        }
        let r = slab.boolean(BoolOp::Cut, &tool).map_err(geo)?;
        names = Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&slab_names), Some(&names)],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        tool = r.shape;
    }
    let op = if e.cut { BoolOp::Cut } else { BoolOp::Fuse };
    apply_feature_tool(fid, state, tool, names, op, e.merge)
}

fn rotation_about(origin: DVec3, dir: DVec3, angle: f64) -> DAffine3 {
    DAffine3::from_translation(origin)
        * DAffine3::from_axis_angle(unit(dir), angle)
        * DAffine3::from_translation(-origin)
}

fn revolve(fid: FeatureId, r: &Revolve, prev: &State, state: &mut State) -> RResult<()> {
    let (mut shape, mut names) = revolve_side(fid, r, prev, "")?;
    if let Some(end2) = &r.end2 {
        let single = |a: &RevolveAngle| {
            !matches!(
                a,
                RevolveAngle::MidPlane { .. } | RevolveAngle::TwoDirection { .. }
            )
        };
        if !single(&r.angle) || !single(end2) {
            return Err(
                "independent directions cannot use mid-plane or two-direction angles".into(),
            );
        }
        let mut opposite = r.clone();
        opposite.reverse = !r.reverse;
        opposite.angle = end2.clone();
        opposite.end2 = None;
        let (other, other_names) = revolve_side(fid, &opposite, prev, "d2.")?;
        let combined = shape.boolean(BoolOp::Fuse, &other).map_err(geo)?;
        names = Naming::from_history(
            &combined.shape,
            &combined.history,
            Inputs {
                named: [Some(&names), Some(&other_names)],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        shape = combined.shape;
    }
    let op = if r.cut { BoolOp::Cut } else { BoolOp::Fuse };
    apply_feature_tool(fid, state, shape, names, op, r.merge)
}

fn revolve_side(
    fid: FeatureId,
    r: &Revolve,
    prev: &State,
    prefix: &str,
) -> RResult<(Shape, Naming)> {
    let prof = sketch_profile(prev, r.sketch, r.thin.as_ref())?;
    let (origin, mut dir) = resolve(prev, &r.axis)?.axis()?;
    if r.reverse {
        dir = -dir;
    }
    let (start, total) = match &r.angle {
        RevolveAngle::Blind { angle } => (0.0, *angle),
        RevolveAngle::MidPlane { angle } => (-angle / 2.0, *angle),
        RevolveAngle::TwoDirection { angle1, angle2 } => (-angle2, angle1 + angle2),
        RevolveAngle::UpToVertex { target } => {
            let radial = |p: DVec3| {
                let v = p - origin;
                v - dir * v.dot(dir)
            };
            let from = radial(prof.shape.face_info(0).map_err(geo)?.centroid);
            let to = radial(resolve(prev, target)?.point()?);
            if from.length() < 1e-9 || to.length() < 1e-9 {
                return Err("the revolve target must be away from its axis".into());
            }
            let angle = dir.dot(from.cross(to)).atan2(from.dot(to)).rem_euclid(TAU);
            (0.0, angle)
        }
        RevolveAngle::UpToSurface { .. } | RevolveAngle::OffsetFromSurface { .. } => (0.0, TAU),
    };
    if !total.is_finite() || total <= 1e-9 || total > TAU + 1e-9 {
        return Err("the revolve angle must be between 0° and 360°".into());
    }
    // The axis must not pass through the profile's interior.
    let local_dir = DVec2::new(dir.dot(prof.plane.x_dir), dir.dot(prof.plane.y_dir()));
    if dir.dot(prof.plane.normal).abs() > 1e-6
        || local_dir.length() < 1e-9
        || prof.plane.distance(origin).abs() > 1e-6
    {
        return Err("the axis must lie in the sketch plane".into());
    }
    let face = if start != 0.0 {
        prof.shape
            .transform(&rotation_about(origin, dir, start))
            .map_err(geo)?
            .shape
    } else {
        prof.shape.clone()
    };
    let until = match &r.angle {
        RevolveAngle::UpToSurface { target } | RevolveAngle::OffsetFromSurface { target, .. } => {
            let resolved = resolve(prev, target)?;
            let mut target_face = match resolved.plane() {
                Ok(p) => big_face(prev, &p)?,
                Err(_) => match resolved {
                    Resolved::Face { index, .. } => body_of(prev)?
                        .shape
                        .sub_shape(Kind::Face, index)
                        .map_err(geo)?,
                    _ => return Err("select a face or plane to revolve up to".into()),
                },
            };
            if let RevolveAngle::OffsetFromSurface {
                offset,
                reverse_offset,
                ..
            } = &r.angle
            {
                if !offset.is_finite() || *offset < 0.0 {
                    return Err("the surface offset must be finite and nonnegative".into());
                }
                if *offset > 0.0 {
                    target_face = target_face
                        .offset_face(
                            if *reverse_offset { -*offset } else { *offset },
                            prof.shape.face_info(0).map_err(geo)?.centroid,
                        )
                        .map_err(geo)?;
                }
            }
            Some(target_face)
        }
        _ => None,
    };
    let res = if let Some(until) = &until {
        face.revolve_until(until, origin, dir)
    } else {
        face.revolve(origin, dir, total.min(TAU))
    }
    .map_err(|e| {
        if e.0.contains("invalid") {
            "the profile crosses the axis of revolution".to_string()
        } else {
            e.0
        }
    })?;
    let mut sides = sweep_names(fid, prefix, &prof.edge_names);
    let mut generate = |h: &HistoryEntry| {
        if h.input == 1 && h.to_kind == Kind::Face {
            Some(format!("{fid}.{prefix}end"))
        } else {
            sides(h)
        }
    };
    let names = Naming::from_history(
        &res.shape,
        &res.history,
        Inputs {
            named: [None, None],
        },
        &mut generate,
        &fid.to_string(),
    );
    Ok((res.shape, names))
}

fn body_of(prev: &State) -> RResult<&Body> {
    prev.body
        .as_ref()
        .ok_or_else(|| "there's no body yet".to_string())
}

/// Edge indices from edge references plus all edges of face references.
fn edge_set(prev: &State, edges: &[Ref], faces: &[Ref]) -> RResult<Vec<u32>> {
    let body = body_of(prev)?;
    let mut out = Vec::new();
    for r in edges {
        match prev.index_of(r)? {
            (Kind::Edge, i) => out.push(i),
            _ => return Err("select edges".into()),
        }
    }
    if !faces.is_empty() {
        let fe = body.shape.face_edges();
        for r in faces {
            match prev.index_of(r)? {
                (Kind::Face, i) => out.extend(&fe[i as usize]),
                _ => return Err("select faces".into()),
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err("select at least one edge or face".into());
    }
    Ok(out)
}

pub(crate) fn dressup_names(
    fid: FeatureId,
    tag: &str,
    body: &Body,
    r: &of_geom::OpResult,
) -> Naming {
    let mut generate = |h: &HistoryEntry| match h.from_kind {
        Kind::Edge => body
            .names
            .edges
            .get(h.from as usize)
            .map(|e| format!("{fid}.{tag}({e})")),
        Kind::Vertex => body
            .names
            .vertices
            .get(h.from as usize)
            .map(|v| format!("{fid}.{tag}v({v})")),
        Kind::Face => body
            .names
            .face_name(h.from)
            .map(|f| format!("{fid}.{tag}o({f})")),
        Kind::Solid => None,
    };
    Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(&body.names), None],
        },
        &mut generate,
        &fid.to_string(),
    )
}

/// Store exact added/removed volumes so a dress-up can be repeated like a cut
/// or boss, including concave fillets and outward shells which add material.
fn remember_dressup(
    fid: FeatureId,
    before: &Body,
    after: &Shape,
    names: &Naming,
    state: &mut State,
) -> RResult<()> {
    let mut tools = Vec::new();
    for (a, an, b, bn, op) in [
        (&before.shape, &before.names, after, names, BoolOp::Cut),
        (after, names, &before.shape, &before.names, BoolOp::Fuse),
    ] {
        let delta = a.boolean(BoolOp::Cut, b).map_err(geo)?;
        if delta.shape.count(Kind::Solid) == 0 {
            continue;
        }
        let names = Naming::from_history(
            &delta.shape,
            &delta.history,
            Inputs {
                named: [Some(an), Some(bn)],
            },
            &mut |_| None,
            &format!("{fid}.delta"),
        );
        tools.push(Tool {
            shape: delta.shape,
            names,
            op,
        });
    }
    state.tools.insert(fid, tools);
    Ok(())
}

fn fillet(fid: FeatureId, f: &Fillet, prev: &State, state: &mut State) -> RResult<()> {
    if f.radius <= 0.0 {
        return Err("the radius must be more than zero".into());
    }
    let body = body_of(prev)?;
    let edges = edge_set(prev, &f.edges, &f.faces)?;
    let r = body.shape.fillet(&edges, f.radius).map_err(geo)?;
    let names = dressup_names(fid, "f", body, &r);
    remember_dressup(fid, body, &r.shape, &names, state)?;
    state.body = Some(Body {
        shape: r.shape,
        names,
    });
    Ok(())
}

fn chamfer(fid: FeatureId, c: &Chamfer, prev: &State, state: &mut State) -> RResult<()> {
    let (mut d1, mut d2) = match c.mode {
        ChamferMode::Equal { distance } => (distance, distance),
        ChamferMode::DistanceDistance { d1, d2 } => (d1, d2),
        ChamferMode::AngleDistance { distance, angle } => {
            if !(0.0..PI / 2.0).contains(&angle) || angle == 0.0 {
                return Err("the chamfer angle must be between 0° and 90°".into());
            }
            (distance, distance * angle.tan())
        }
    };
    if c.flip {
        std::mem::swap(&mut d1, &mut d2);
    }
    if d1 <= 0.0 || d2 <= 0.0 {
        return Err("chamfer distances must be more than zero".into());
    }
    let body = body_of(prev)?;
    let edges = edge_set(prev, &c.edges, &c.faces)?;
    let r = body.shape.chamfer(&edges, d1, d2).map_err(geo)?;
    let names = dressup_names(fid, "c", body, &r);
    remember_dressup(fid, body, &r.shape, &names, state)?;
    state.body = Some(Body {
        shape: r.shape,
        names,
    });
    Ok(())
}

fn shell(fid: FeatureId, s: &ShellDef, prev: &State, state: &mut State) -> RResult<()> {
    if s.thickness <= 0.0 {
        return Err("the thickness must be more than zero".into());
    }
    let body = body_of(prev)?;
    let mut faces = Vec::new();
    for r in &s.faces {
        match prev.index_of(r)? {
            (Kind::Face, i) => faces.push(i),
            _ => return Err("select faces to remove".into()),
        }
    }
    let t = if s.outward { -s.thickness } else { s.thickness };
    let r = body.shape.shell(&faces, t).map_err(geo)?;
    // A wall as thick as half the part leaves no cavity: the kernel returns the
    // solid unchanged rather than failing.
    let volume = |shape: &Shape| shape.mass_props().map(|m| m.volume).map_err(geo);
    let before = volume(&body.shape)?;
    if (volume(&r.shape)? - before).abs() <= 1e-9 * before.abs().max(1.0) {
        return Err("the shell is too thick for the part; reduce the thickness".into());
    }
    let names = dressup_names(fid, "s", body, &r);
    remember_dressup(fid, body, &r.shape, &names, state)?;
    state.body = Some(Body {
        shape: r.shape,
        names,
    });
    Ok(())
}

/// Hole cross-section dimensions after applying the standard and overrides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoleDims {
    pub diameter: f64,
    pub cbore: Option<(f64, f64)>,
    pub csink: Option<(f64, f64)>,
}

pub fn hole_dims(h: &Hole) -> RResult<HoleDims> {
    let size =
        holes::find(h.standard, &h.size).ok_or_else(|| format!("unknown hole size {}", h.size))?;
    let diameter = h.diameter_override.unwrap_or(match h.kind {
        HoleKind::Tapped => size.tap_drill,
        _ => holes::clearance(size, h.fit),
    });
    let cbore = (h.kind == HoleKind::Counterbore).then(|| {
        (
            h.cbore_diameter_override.unwrap_or(size.cbore.0),
            h.cbore_depth_override.unwrap_or(size.cbore.1),
        )
    });
    let default_angle = match h.standard {
        HoleStandard::Iso => 90f64.to_radians(),
        HoleStandard::Ansi => 82f64.to_radians(),
    };
    let csink = (h.kind == HoleKind::Countersink).then(|| {
        (
            h.csink_diameter_override.unwrap_or(size.csink),
            h.csink_angle_override.unwrap_or(default_angle),
        )
    });
    if diameter <= 0.0
        || cbore.is_some_and(|(d, _)| d <= diameter)
        || csink.is_some_and(|(d, _)| d <= diameter)
    {
        return Err("the hole's diameters don't fit together".into());
    }
    Ok(HoleDims {
        diameter,
        cbore,
        csink,
    })
}

fn hole(fid: FeatureId, h: &Hole, prev: &State, state: &mut State) -> RResult<()> {
    let (plane, sk) = sketch_of(prev, h.sketch)?;
    let points: Vec<DVec2> = sk
        .entities
        .iter()
        .filter(|(id, e)| {
            **id != of_sketch::ORIGIN
                && !e.construction
                && matches!(e.geo, Geometry::Point { .. })
                && !sk.entities.values().any(|o| o.point_refs().contains(id))
        })
        .map(|(id, _)| sk.point(*id).unwrap())
        .collect();
    if points.is_empty() {
        return Err("add sketch points where the holes go".into());
    }
    let dims = hole_dims(h)?;
    let depth = match h.end {
        HoleEnd::Blind { depth } if depth > 0.0 => depth,
        HoleEnd::Blind { .. } => return Err("the hole depth must be more than zero".into()),
        HoleEnd::ThroughAll => through_all_length(prev, plane),
    };
    // Drill into the face: against the sketch normal.
    let out = if h.reverse {
        -plane.normal
    } else {
        plane.normal
    };
    let r = dims.diameter / 2.0;
    let mut pts: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    if let Some((cd, cdepth)) = dims.cbore {
        pts.extend([(cd / 2.0, 0.0), (cd / 2.0, -cdepth), (r, -cdepth)]);
    } else if let Some((sd, angle)) = dims.csink {
        let sr = sd / 2.0;
        pts.extend([(sr, 0.0), (r, -(sr - r) / (angle / 2.0).tan())]);
    } else {
        pts.push((r, 0.0));
    }
    pts.push((r, -depth));
    if matches!(h.end, HoleEnd::Blind { .. }) {
        // 118° drill point.
        pts.push((0.0, -depth - r / 59f64.to_radians().tan()));
    } else {
        pts.push((0.0, -depth));
    }
    let n = pts.len();
    let curves: Vec<GCurve> = (0..n)
        .map(|i| GCurve::Line {
            a: DVec2::new(pts[i].0, pts[i].1),
            b: DVec2::new(pts[(i + 1) % n].0, pts[(i + 1) % n].1),
        })
        .collect();
    let mut tools = Vec::new();
    let mut all_faces: Vec<Vec<String>> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let origin = plane.to_world(*p);
        let section_plane = Plane::new(origin, plane.x_dir, plane.x_dir.cross(out));
        let face = Profile {
            plane: section_plane,
            faces: vec![ProfileFace {
                outer: curves.clone(),
                holes: vec![],
            }],
        }
        .to_shape()
        .map_err(geo)?;
        let edge_names: Vec<String> = (0..face.count(Kind::Edge))
            .map(|e| {
                let mid = face
                    .edge_info(e)
                    .map(|x| section_plane.to_local(x.mid))
                    .unwrap_or_default();
                let k = curves
                    .iter()
                    .position(|c| match c {
                        GCurve::Line { a, b } => ((*a + *b) * 0.5 - mid).length() < 1e-6,
                        _ => false,
                    })
                    .unwrap_or(99);
                format!("p{i}.h{k}")
            })
            .collect();
        let res = face.revolve(origin, out, TAU).map_err(geo)?;
        let mut generate = sweep_names(fid, "", &edge_names);
        let names = Naming::from_history(
            &res.shape,
            &res.history,
            Inputs {
                named: [None, None],
            },
            &mut generate,
            &format!("{fid}.p{i}"),
        );
        all_faces.extend(names.faces.iter().cloned());
        tools.push(res.shape);
    }
    let tool = Shape::compound(tools.iter());
    let names = Naming::from_faces(&tool, all_faces);
    if h.cosmetic_thread {
        if h.kind != HoleKind::Tapped {
            return Err("cosmetic threads apply to tapped holes".into());
        }
        let size = holes::find(h.standard, &h.size).ok_or("unknown hole size")?;
        let pitch = holes::pitch(h.standard, size)
            .ok_or_else(|| format!("no coarse pitch is listed for {}", h.size))?;
        let body = body_of(prev)?;
        let threads = points
            .iter()
            .map(|p| {
                let start = plane.to_world(*p);
                let axis = -out;
                // The thread runs through the material the drill passes.
                let probe = Shape::wire3(&[of_geom::Curve3::Line {
                    a: start,
                    b: start + axis * depth,
                }])
                .map_err(geo)?;
                let inside = probe.boolean(BoolOp::Common, &body.shape).map_err(geo)?;
                let length = (0..inside.shape.count(Kind::Edge))
                    .filter_map(|e| inside.shape.edge_info(e).ok())
                    .filter(|e| {
                        (e.start - start).length() < 1e-6 || (e.end - start).length() < 1e-6
                    })
                    .map(|e| e.length)
                    .next()
                    .unwrap_or(0.0);
                if length <= 1e-9 {
                    return Err("a tapped hole starts outside the material".to_string());
                }
                Ok(crate::cosmetic_thread::HoleThread {
                    start,
                    axis,
                    diameter: size.nominal,
                    pitch,
                    length,
                    size: size.name.to_string(),
                })
            })
            .collect::<RResult<Vec<_>>>()?;
        state.hole_threads.insert(fid, threads);
    }
    apply_feature_tool(fid, state, tool, names, BoolOp::Cut, true)
}

/// Replay selections in feature-tree order, regardless of click order.
fn ordered_seeds<'a>(prev: &'a State, selected: &[FeatureId]) -> RResult<Vec<&'a FeatureId>> {
    if selected.iter().any(|id| !prev.tools.contains_key(id)) {
        return Err("select solid features that rebuilt successfully".into());
    }
    Ok(prev
        .tool_order
        .iter()
        .filter(|id| selected.contains(id))
        .collect())
}

/// Repeat the tools of `features` at each transform.
fn repeat(
    fid: FeatureId,
    features: &[FeatureId],
    transforms: &[DAffine3],
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    if features.is_empty() {
        return Err("select features to pattern".into());
    }
    let mut seeds: Vec<Tool> = Vec::new();
    for f in ordered_seeds(prev, features)? {
        let tools = prev.tools.get(f).ok_or_else(|| {
            "select a solid feature that rebuilt successfully to pattern".to_string()
        })?;
        seeds.extend(tools.iter().cloned());
    }
    for (k, t) in transforms.iter().enumerate() {
        for (j, seed) in seeds.iter().enumerate() {
            let moved = seed.shape.transform(t).map_err(geo)?;
            let names =
                seed.names
                    .prefixed(&moved.shape, &moved.history, &format!("{fid}.i{k}.{j}."));
            apply_tool(fid, state, moved.shape, names, seed.op, true)?;
        }
    }
    Ok(())
}

fn linear_pattern(
    fid: FeatureId,
    p: &LinearPattern,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let dir_of = |d: &crate::PatternDirection| -> RResult<DVec3> {
        let v = resolve(prev, &d.reference)?.direction()?;
        if d.count < 1 {
            return Err("the instance count must be at least 1".into());
        }
        Ok(if d.reverse { -v } else { v })
    };
    let d1 = dir_of(&p.direction1)?;
    let (d2, s2, n2) = match &p.direction2 {
        Some(d) => (dir_of(d)?, d.spacing, d.count),
        None => (DVec3::ZERO, 0.0, 1),
    };
    let mut ts = Vec::new();
    for i in 0..p.direction1.count {
        for j in 0..n2 {
            if i == 0 && j == 0 {
                continue;
            }
            let v = d1 * (p.direction1.spacing * i as f64) + d2 * (s2 * j as f64);
            ts.push(DAffine3::from_translation(v));
        }
    }
    if ts.is_empty() {
        return Err("the pattern needs more than one instance".into());
    }
    repeat(fid, &p.features, &ts, prev, state)
}

fn circular_pattern(
    fid: FeatureId,
    p: &CircularPattern,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (origin, mut dir) = resolve(prev, &p.axis)?.axis()?;
    if p.reverse {
        dir = -dir;
    }
    if p.count < 2 {
        return Err("the pattern needs at least 2 instances".into());
    }
    let step = if p.equal_spacing {
        if (p.angle - TAU).abs() < 1e-9 {
            p.angle / p.count as f64
        } else {
            p.angle / (p.count - 1) as f64
        }
    } else {
        p.angle
    };
    let ts: Vec<DAffine3> = (1..p.count)
        .map(|k| rotation_about(origin, dir, step * k as f64))
        .collect();
    repeat(fid, &p.features, &ts, prev, state)
}

fn mirror(fid: FeatureId, m: &MirrorDef, prev: &State, state: &mut State) -> RResult<()> {
    let plane = resolve(prev, &m.plane)?.plane()?;
    if !m.keep_original && !m.features.is_empty() {
        return Err("only a whole-body mirror can replace the original".into());
    }
    if m.features.is_empty() {
        let body = body_of(prev)?;
        let r = body.shape.mirror(plane.origin, plane.normal).map_err(geo)?;
        let names = body
            .names
            .prefixed(&r.shape, &r.history, &format!("{fid}.m."));
        if !m.keep_original {
            state.body = Some(Body {
                shape: r.shape,
                names,
            });
            return Ok(());
        }
        return apply_tool(fid, state, r.shape, names, BoolOp::Fuse, true);
    }
    for f in ordered_seeds(prev, &m.features)? {
        let tools = prev
            .tools
            .get(f)
            .ok_or_else(|| {
                "select a solid feature that rebuilt successfully to mirror".to_string()
            })?
            .clone();
        for (j, seed) in tools.iter().enumerate() {
            let r = seed.shape.mirror(plane.origin, plane.normal).map_err(geo)?;
            let names = seed
                .names
                .prefixed(&r.shape, &r.history, &format!("{fid}.m{f}.{j}."));
            apply_tool(fid, state, r.shape, names, seed.op, true)?;
        }
    }
    Ok(())
}

fn import(fid: FeatureId, i: &Import, state: &mut State) -> RResult<()> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&i.data)
        .map_err(|_| "the imported data is corrupt".to_string())?;
    let ext = match i.format {
        ImportFormat::Step => "step",
        ImportFormat::Iges => "igs",
    };
    let path = std::env::temp_dir().join(format!(
        "openforge-import-{}-{fid}.{ext}",
        std::process::id()
    ));
    std::fs::write(&path, bytes).map_err(geo)?;
    let p = path.to_string_lossy().to_string();
    let shape = match i.format {
        ImportFormat::Step => Shape::read_step(&p),
        ImportFormat::Iges => Shape::read_iges(&p),
    };
    let _ = std::fs::remove_file(&path);
    let shape = shape.map_err(geo)?;
    // Faces are named by their position in the file, qualified by the file's
    // content: replacing the data with a different file reports missing
    // references instead of silently binding them to unrelated faces. The bare
    // position names of older files stay as aliases.
    let content = crate::naming::content_hash(i.data.as_bytes());
    let faces = (0..shape.count(Kind::Face))
        .map(|f| vec![format!("{fid}.i{content}.f{f}"), format!("{fid}.f{f}")])
        .collect();
    let names = Naming::from_faces(&shape, faces);
    apply_tool(fid, state, shape, names, BoolOp::Fuse, false)
}

/// Replaces each face reference by references to all of its boundary edges, outer
/// and inner loops, keeping the selection order and dropping repeated edges.
fn expand_face_edges(state: &State, refs: &[Ref]) -> RResult<Vec<Ref>> {
    let mut out: Vec<Ref> = Vec::new();
    let push = |r: Ref, out: &mut Vec<Ref>| {
        if !out.contains(&r) {
            out.push(r);
        }
    };
    for r in refs {
        match r {
            Ref::Face { name } => {
                let body = body_of(state)?;
                let face = body
                    .names
                    .face(name)
                    .ok_or_else(|| format!("face {name} no longer exists"))?;
                for &e in &body.shape.face_edges()[face as usize] {
                    let name = body
                        .names
                        .edges
                        .get(e as usize)
                        .ok_or("unnamed face edge")?;
                    push(Ref::Edge { name: name.clone() }, &mut out);
                }
            }
            Ref::BodyTopology {
                body: id,
                kind: Kind::Face,
                name,
            } => {
                let selected = &state.solid(id)?.geometry;
                let face = selected
                    .names
                    .faces
                    .iter()
                    .position(|n| n.first() == Some(name))
                    .ok_or_else(|| format!("body topology {name} no longer exists"))?;
                for &e in &selected.shape.face_edges()[face] {
                    let name = selected
                        .names
                        .edges
                        .get(e as usize)
                        .ok_or("unnamed face edge")?;
                    push(
                        Ref::BodyTopology {
                            body: id.clone(),
                            kind: Kind::Edge,
                            name: name.clone(),
                        },
                        &mut out,
                    );
                }
            }
            other => push(other.clone(), &mut out),
        }
    }
    Ok(out)
}

/// Sketch geometry for model edges projected onto a sketch plane, for
/// Convert Entities. Face references convert every boundary edge. Returns the new entity ids.
pub fn convert_edges(
    state: &State,
    plane: &Plane,
    sketch: &mut Sketch,
    edges: &[Ref],
) -> RResult<Vec<of_sketch::Id>> {
    let body = body_of(state)?;
    let edges = expand_face_edges(state, edges)?;
    let mut out = Vec::new();
    for r in &edges {
        let Ok(Resolved::Edge { index, info }) = resolve(state, r) else {
            return Err("select model edges to convert".into());
        };
        let name = match r {
            Ref::Edge { name } => name.clone(),
            Ref::BodyTopology {
                kind: Kind::Edge, ..
            } => serde_json::to_string(r).map_err(geo)?,
            _ => return Err("select an edge to convert".into()),
        };
        let local = |p: DVec3| plane.to_local(p);
        let flat = info.axis.cross(plane.normal).length() < 1e-6;
        let g = match info.kind {
            of_geom::CurveKind::Line => {
                let a = sketch.add_point(local(info.start));
                let b = sketch.add_point(local(info.end));
                Geometry::Line { a, b }
            }
            of_geom::CurveKind::Circle if flat && info.closed => Geometry::Circle {
                center: sketch.add_point(local(info.center)),
                radius: info.radius,
            },
            of_geom::CurveKind::Circle if flat => {
                let (s, e) = if info.axis.dot(plane.normal) >= 0.0 {
                    (info.start, info.end)
                } else {
                    (info.end, info.start)
                };
                let c = sketch.add_point(local(info.center));
                let s = sketch.add_point(local(s));
                let e = sketch.add_point(local(e));
                Geometry::Arc {
                    center: c,
                    start: s,
                    end: e,
                }
            }
            _ => {
                let pts = body.shape.edge_samples(index, 0.05, 0.2).map_err(geo)?;
                let step = (pts.len() / 12).max(1);
                let mut ids: Vec<of_sketch::Id> = pts
                    .iter()
                    .step_by(step)
                    .map(|p| sketch.add_point(local(*p)))
                    .collect();
                if ids.len() < 2 {
                    ids.push(sketch.add_point(local(*pts.last().unwrap())));
                }
                Geometry::Spline {
                    points: ids,
                    closed: info.closed,
                }
            }
        };
        // Splines are converted once; lines and circles follow the edge on rebuild.
        let external = (!matches!(g, Geometry::Spline { .. })).then(|| name.clone());
        let id = sketch
            .add_entity(of_sketch::Entity {
                geo: g,
                construction: false,
                external,
            })
            .map_err(geo)?;
        out.push(id);
    }
    Ok(out)
}
