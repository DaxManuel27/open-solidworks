//! Forms pressed into sheet metal (forming tools and gussets), and tab and
//! slot joints.
//!
//! A form is a tool solid placed with its stopping face on the sheet. The
//! sheet keeps its thickness: the tool's footprint is cut away and a cup one
//! thickness thick is added round the tool, open where the tool's stopping
//! face and removed faces are. What a form added and removed is recorded, so
//! the flat pattern can undo it and mark where it goes.

use std::f64::consts::PI;

use of_geom::{
    BoolOp, Curve2 as GCurve, DAffine3, DMat3, DVec2, DVec3, Kind, Plane, Profile, ProfileFace,
    Shape, SurfaceKind,
};

use super::{RResult, geo, record_body, record_for_face, sketch_of};
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::resolve::{Resolved, resolve};
use crate::sheet_metal::{
    FormRecord, FormShape, FormTool, SheetMetalGusset, TabAndSlot, TabCorner,
};
use crate::{FeatureId, FeatureKind, Ref};

/// A tool in its own frame: the stopping face on z = 0 facing +z, the tool
/// below it; and the faces to leave open.
struct Tool {
    solid: Shape,
    open: Vec<u32>,
    /// The stopping face's outline, for marking the flat.
    outline: Vec<DVec3>,
}

fn region(plane: Plane, outer: Vec<GCurve>) -> RResult<Shape> {
    Profile {
        plane,
        faces: vec![ProfileFace {
            outer,
            holes: Vec::new(),
        }],
    }
    .to_shape()
    .map_err(geo)
}

fn rect(w: f64, l: f64) -> Vec<GCurve> {
    let c = [
        (-l / 2.0, -w / 2.0),
        (l / 2.0, -w / 2.0),
        (l / 2.0, w / 2.0),
        (-l / 2.0, w / 2.0),
    ]
    .map(|(x, y)| DVec2::new(x, y));
    (0..4)
        .map(|i| GCurve::Line {
            a: c[i],
            b: c[(i + 1) % 4],
        })
        .collect()
}

fn obround(l: f64, w: f64) -> Vec<GCurve> {
    let r = w / 2.0;
    let h = (l / 2.0 - r).max(1e-6);
    vec![
        GCurve::Line {
            a: DVec2::new(-h, -r),
            b: DVec2::new(h, -r),
        },
        GCurve::Arc {
            center: DVec2::new(h, 0.0),
            start: DVec2::new(h, -r),
            end: DVec2::new(h, r),
        },
        GCurve::Line {
            a: DVec2::new(h, r),
            b: DVec2::new(-h, r),
        },
        GCurve::Arc {
            center: DVec2::new(-h, 0.0),
            start: DVec2::new(-h, r),
            end: DVec2::new(-h, -r),
        },
    ]
}

/// A prism of `outer` from z = 0 down to z = -height, with the face on
/// z = 0 found again.
fn prism_down(outer: Vec<GCurve>, height: f64) -> RResult<(Shape, Vec<DVec3>)> {
    if height.is_nan() || height <= 1e-6 {
        return Err("a form needs a height".into());
    }
    let plane = Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z);
    let face = region(plane, outer)?;
    let outline: Vec<DVec3> = (0..face.count(Kind::Vertex))
        .filter_map(|v| face.vertex_point(v).ok())
        .collect();
    Ok((
        face.prism(DVec3::NEG_Z * height).map_err(geo)?.shape,
        outline,
    ))
}

fn face_where(solid: &Shape, test: impl Fn(&of_geom::FaceInfo) -> bool) -> Vec<u32> {
    (0..solid.count(Kind::Face))
        .filter(|f| solid.face_info(*f).is_ok_and(|i| test(&i)))
        .collect()
}

fn top(i: &of_geom::FaceInfo) -> bool {
    i.kind == SurfaceKind::Plane && i.normal.z > 1.0 - 1e-9 && i.point.z.abs() < 1e-9
}

fn library(shape: &FormShape, t: f64) -> RResult<Tool> {
    let positive = |v: &[f64]| v.iter().all(|x| x.is_finite() && *x > 1e-6);
    let tool = match *shape {
        FormShape::Emboss { diameter, height } | FormShape::Dimple { diameter, height } => {
            if !positive(&[diameter, height]) {
                return Err("form sizes must be more than zero".into());
            }
            let (solid, outline) = prism_down(
                vec![GCurve::Circle {
                    center: DVec2::ZERO,
                    radius: diameter / 2.0,
                }],
                height,
            )?;
            let mut open = face_where(&solid, top);
            if matches!(shape, FormShape::Dimple { .. }) {
                // The bottom opens into a hole with a drawn collar.
                open.extend(face_where(&solid, |i| {
                    i.kind == SurfaceKind::Plane && i.normal.z < -1.0 + 1e-9
                }));
            }
            Tool {
                solid,
                open,
                outline,
            }
        }
        FormShape::Rib {
            length,
            width,
            height,
        } => {
            if !positive(&[length, width, height]) || length <= width {
                return Err("a rib is longer than it is wide, all sizes more than zero".into());
            }
            let (solid, outline) = prism_down(obround(length, width), height)?;
            let open = face_where(&solid, top);
            Tool {
                solid,
                open,
                outline,
            }
        }
        FormShape::Louver {
            length,
            width,
            height,
        }
        | FormShape::Lance {
            length,
            width,
            height,
        } => {
            if !positive(&[length, width, height]) {
                return Err("form sizes must be more than zero".into());
            }
            let (solid, outline) = prism_down(rect(width, length), height)?;
            let mut open = face_where(&solid, top);
            // A louver opens along one long side; a lance along both.
            let lance = matches!(shape, FormShape::Lance { .. });
            open.extend(face_where(&solid, |i| {
                i.kind == SurfaceKind::Plane
                    && (i.normal.y > 1.0 - 1e-9 || (lance && i.normal.y < -1.0 + 1e-9))
            }));
            Tool {
                solid,
                open,
                outline,
            }
        }
        FormShape::File {
            ref path,
            configuration,
        } => return file_tool(path, configuration),
    };
    if t <= 0.0 {
        return Err("the sheet has no thickness".into());
    }
    Ok(tool)
}

/// A tool from a part file with a Forming Tool feature.
fn file_tool(path: &str, configuration: Option<u32>) -> RResult<Tool> {
    let mut doc = crate::file::load(std::path::Path::new(path))
        .map_err(|e| format!("can't open forming tool {path}: {e}"))?;
    if let Some(c) = configuration {
        if !doc.configurations.items.iter().any(|i| i.id == c) {
            return Err(format!("forming tool {path} has no configuration {c}"));
        }
        doc.configurations.active = c;
    }
    let def = doc
        .features
        .iter()
        .find_map(|f| match &f.kind {
            FeatureKind::FormingTool(d) => Some(d.clone()),
            _ => None,
        })
        .ok_or_else(|| format!("{path} has no Forming Tool feature"))?;
    let model = crate::Model::new(doc);
    if let Some(e) = model.statuses().into_iter().find_map(|s| s.error) {
        return Err(format!("forming tool {path} doesn't build: {e}"));
    }
    let state = model.state();
    let body = state
        .body
        .as_ref()
        .ok_or("the forming tool part has no body")?;
    let stop = match resolve(state, &def.stopping_face)? {
        Resolved::Face { index, info } if info.kind == SurfaceKind::Plane => (index, info),
        _ => return Err("the tool's stopping face must be flat".into()),
    };
    let origin = match &def.insertion_point {
        Some(r) => resolve(state, r)?.point()?,
        None => stop.1.centroid,
    };
    // Into the tool frame: the stopping face on z = 0, its normal +z.
    let frame = Plane::new(origin, stop.1.x_dir, stop.1.normal);
    let to_tool = frame.to_affine().inverse();
    let moved = body.shape.transform(&to_tool).map_err(geo)?.shape;
    let mut open = face_where(&moved, top);
    for r in &def.remove_faces {
        if let Resolved::Face { info, .. } = resolve(state, r)? {
            let c = to_tool.transform_point3(info.centroid);
            open.extend(face_where(&moved, |i| (i.centroid - c).length() < 1e-6));
        }
    }
    let stopping = moved
        .sub_shape(
            Kind::Face,
            *face_where(&moved, top).first().ok_or("no stopping face")?,
        )
        .map_err(geo)?;
    let outline = (0..stopping.count(Kind::Vertex))
        .filter_map(|v| stopping.vertex_point(v).ok())
        .collect();
    let _ = stop.0;
    Ok(Tool {
        solid: moved,
        open,
        outline,
    })
}

fn named(fid: FeatureId, before: &Naming, r: of_geom::OpResult, tag: &str) -> Body {
    Body {
        names: Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(before), None],
            },
            &mut |_| Some(format!("{fid}.{tag}")),
            &fid.to_string(),
        ),
        shape: r.shape,
    }
}

/// Press a placed tool into a body: cut its footprint and add its cup.
fn press(
    fid: FeatureId,
    body: &Body,
    tool: &Shape,
    cup: &Shape,
    k: usize,
) -> RResult<(Body, Shape, Shape)> {
    let removed = body.shape.boolean(BoolOp::Common, tool).map_err(geo)?.shape;
    let cut = body.shape.boolean(BoolOp::Cut, tool).map_err(geo)?;
    let cut = named(fid, &body.names, cut, &format!("form{k}"));
    let added = cup.boolean(BoolOp::Cut, &body.shape).map_err(geo)?.shape;
    let fused = cut.shape.boolean(BoolOp::Fuse, cup).map_err(geo)?;
    let result = named(fid, &cut.names, fused, &format!("form{k}"));
    if result.shape.count(Kind::Solid) != 1 {
        return Err("the form breaks the sheet apart; is it too close to an edge?".into());
    }
    Ok((result, added, removed))
}

pub(crate) fn form_tool(
    fid: FeatureId,
    p: &FormTool,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (Kind::Face, global) = prev.index_of(&p.face)? else {
        return Err("select a flat face of the sheet".into());
    };
    let record = record_for_face(prev, global)?;
    let t = prev.sheet_metal[record].params.thickness;
    let info = prev
        .body
        .as_ref()
        .unwrap()
        .shape
        .face_info(global)
        .map_err(geo)?;
    if info.kind != SurfaceKind::Plane {
        return Err("forms go on flat faces".into());
    }
    let points: Vec<DVec3> = match p.sketch {
        Some(s) => {
            let (plane, sk) = sketch_of(prev, s)?;
            let pts: Vec<DVec3> = sk
                .entities
                .iter()
                .filter(|(id, e)| {
                    **id != of_sketch::ORIGIN
                        && !e.construction
                        && matches!(e.geo, of_sketch::Geometry::Point { .. })
                        && !sk.entities.values().any(|o| o.point_refs().contains(id))
                })
                .map(|(id, _)| plane.to_world(sk.point(*id).unwrap()))
                .collect();
            if pts.is_empty() {
                return Err("the placement sketch has no points".into());
            }
            pts
        }
        None => vec![info.centroid],
    };
    let tool = library(&p.shape, t)?;
    if let FormShape::File { path, .. } = &p.shape {
        let path = std::path::PathBuf::from(path);
        if let Some(hash) = crate::build::file_hash(&path) {
            state.files.push((fid, path, hash));
        }
    }
    // The tool pushes the sheet away from the face, or toward it when flipped.
    let (normal, offset) = if p.flip {
        (-info.normal, -t)
    } else {
        (info.normal, 0.0)
    };
    let cup = tool
        .solid
        .shell(&tool.open, -t)
        .map_err(|e| format!("the form is too small for this thickness: {}", e.0))?
        .shape;
    let b = record_body(prev, &prev.sheet_metal[record]).unwrap();
    let mut body = prev.bodies[b].geometry.clone();
    for (k, point) in points.iter().enumerate() {
        let q =
            *point - info.normal * (info.normal.dot(*point - info.point)) + info.normal * offset;
        let frame = Plane::new(q, info.x_dir, normal);
        let turn = DAffine3::from_mat3(DMat3::from_rotation_z(p.rotation));
        let place = frame.to_affine() * turn;
        let placed_tool = tool.solid.transform(&place).map_err(geo)?.shape;
        let placed_cup = cup.transform(&place).map_err(geo)?.shape;
        let (next, added, removed) = press(fid, &body, &placed_tool, &placed_cup, k)?;
        body = next;
        state.sheet_metal[record].forms.push(FormRecord {
            feature: fid,
            added: Some(added),
            removed: Some(removed),
            center: q.to_array(),
            outline: tool
                .outline
                .iter()
                .map(|v| place.transform_point3(*v).to_array())
                .collect(),
            punch_id: p.punch_id.clone(),
            angle: p.rotation,
        });
    }
    state.bodies[b].geometry = body;
    state.aggregate_bodies()
}

pub(crate) fn gusset(
    fid: FeatureId,
    p: &SheetMetalGusset,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (Kind::Face, global) = prev.index_of(&p.bend)? else {
        return Err("select a face of the bend".into());
    };
    let record = record_for_face(prev, global)?;
    let rec = &prev.sheet_metal[record];
    let b = record_body(prev, rec).unwrap();
    let body = prev.bodies[b].geometry.clone();
    let name = prev
        .body
        .as_ref()
        .unwrap()
        .names
        .face_name(global)
        .unwrap_or_default();
    let bend = rec
        .bends
        .iter()
        .find(|r| {
            r.faces
                .iter()
                .any(|f| super::base_name(f) == super::base_name(name))
        })
        .ok_or("select a face of a bend")?;
    // The inside face of the bend: the one with the inside radius.
    let inside = bend
        .faces
        .iter()
        .filter_map(|f| body.names.face(f))
        .find(|f| {
            body.shape
                .face_frame(*f)
                .is_ok_and(|fr| matches!(fr.surface, of_geom::FaceSurface::Cylinder { radius } if (radius - bend.radius).abs() < 1e-6))
        })
        .ok_or("can't find the inside of the bend")?;
    let fr = body.shape.face_frame(inside).map_err(geo)?;
    let r = bend.radius;
    let v = fr.v.0 + p.position * (fr.v.1 - fr.v.0);
    let axis = fr.z.normalize();
    let center = fr.origin + axis * v;
    let (u0, u1) = fr.u;
    let tangent = |u: f64| (fr.y * u.cos() - fr.x * u.sin()).normalize();
    let at = |u: f64| fr.point(u, v);
    // The inside virtual sharp: where the two walls' inside lines meet.
    let half = (u1 - u0) / 2.0;
    let mid = (u0 + u1) / 2.0;
    let radial = (fr.x * mid.cos() + fr.y * mid.sin()).normalize();
    let sharp = center + radial * (r / half.cos());
    let plane = Plane::new(center - axis * (p.width / 2.0), radial, axis);
    if half >= PI / 2.0 - 1e-9 {
        return Err("a gusset needs a bend of less than 180°".into());
    }
    // Legs along the walls from the bend's ends, away from it.
    let (w0, w1) = (-tangent(u0), tangent(u1));
    let q0 = at(u0) + w0 * p.depth;
    let q1 = match (p.depth2, p.angle) {
        (Some(d2), _) => at(u1) + w1 * d2,
        (None, Some(alpha)) => {
            // From q0, back toward the sharp turned by `alpha`, to wall 1.
            let toward = (sharp - q0).normalize();
            let mut normal = w0.cross(w1).normalize_or_zero();
            // Turn toward the second wall.
            if normal.cross(toward).dot(w1) < 0.0 {
                normal = -normal;
            }
            if normal == DVec3::ZERO || !(alpha > 1e-6 && alpha < PI - 2.0 * half - 1e-6) {
                return Err("the gusset angle doesn't meet the other wall".into());
            }
            // Rodrigues' rotation of `toward` about `normal` by `alpha`.
            let (sa, ca) = alpha.sin_cos();
            let dir =
                toward * ca + normal.cross(toward) * sa + normal * normal.dot(toward) * (1.0 - ca);
            // Intersect q0 + s dir with the line at(u1) + k w1.
            let (o, k) = (at(u1), w1);
            let m = dir.cross(k);
            let denom = m.length_squared();
            if denom < 1e-12 {
                return Err("the gusset angle runs parallel to the other wall".into());
            }
            let along = (o - q0).cross(k).dot(m) / denom;
            let q = q0 + dir * along;
            if (q - o).dot(k) <= 1e-6 {
                return Err("the gusset angle meets the other wall inside the bend".into());
            }
            q
        }
        (None, None) => at(u1) + w1 * p.depth,
    };
    let tri = [q0, sharp, q1].map(|q| plane.to_local(q));
    let face = region(
        plane,
        (0..3)
            .map(|i| GCurve::Line {
                a: tri[i],
                b: tri[(i + 1) % 3],
            })
            .collect(),
    )?;
    let mut rib = face.prism(axis * p.width).map_err(geo)?.shape;
    if p.rounded_back {
        // The two long edges of the back (q0 to q1), rounded nearly to a
        // half round.
        let back = (q1 - q0).normalize();
        let edges: Vec<u32> = (0..rib.count(Kind::Edge))
            .filter(|e| {
                rib.edge_info(*e).is_ok_and(|i| {
                    i.kind == of_geom::CurveKind::Line
                        && (i.end - i.start).normalize().dot(back).abs() > 1.0 - 1e-9
                })
            })
            .collect();
        let radius = (p.width / 2.0) * 0.98;
        rib = rib
            .fillet(&edges, radius)
            .map_err(|e| format!("can't round the gusset's back: {}", e.0))?
            .shape;
    }
    let added = rib.boolean(BoolOp::Cut, &body.shape).map_err(geo)?.shape;
    let fused = body.shape.boolean(BoolOp::Fuse, &rib).map_err(geo)?;
    let result = named(fid, &body.names, fused, "gusset");
    state.bodies[b].geometry = result;
    state.sheet_metal[record].forms.push(FormRecord {
        feature: fid,
        added: Some(added),
        removed: None,
        center: center.to_array(),
        outline: vec![q0.to_array(), q1.to_array()],
        punch_id: p.punch_id.clone(),
        angle: 0.0,
    });
    state.aggregate_bodies()
}

/// The thickness of a body at a flat face, from its sheet metal record or by
/// measuring.
fn thickness(prev: &State, b: usize, face: u32) -> RResult<f64> {
    if let Some(r) = prev
        .sheet_metal
        .iter()
        .find(|r| record_body(prev, r) == Some(b))
    {
        return Ok(r.params.thickness);
    }
    let body = &prev.bodies[b].geometry.shape;
    let f = body.face_info(face).map_err(geo)?;
    (0..body.count(Kind::Face))
        .filter_map(|g| body.face_info(g).ok())
        .filter(|g| g.kind == SurfaceKind::Plane && g.normal.dot(f.normal) < -1.0 + 1e-9)
        .map(|g| (f.point - g.point).dot(f.normal))
        .filter(|d| *d > 1e-6)
        .min_by(f64::total_cmp)
        .ok_or_else(|| "can't tell the slot body's thickness".into())
}

fn body_of_index(prev: &State, kind: Kind, global: u32) -> RResult<(usize, u32)> {
    let whole = &prev.body.as_ref().ok_or("there's no body")?.shape;
    for (b, body) in prev.bodies.iter().enumerate() {
        let ix = body.geometry.shape.indices_in(whole, kind).map_err(geo)?;
        if let Some(l) = ix.iter().position(|g| *g == global) {
            return Ok((b, l as u32));
        }
    }
    Err("the selection is not on a body".into())
}

pub(crate) fn tab_and_slot(
    fid: FeatureId,
    p: &TabAndSlot,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let Resolved::Edge { index, info } = resolve(prev, &p.edge)? else {
        return Err("select an edge of the tab body".into());
    };
    if info.kind != of_geom::CurveKind::Line {
        return Err("tabs go along a straight edge".into());
    }
    let (Kind::Face, slot_face) = prev.index_of(&p.face)? else {
        return Err("select the face the tabs go through".into());
    };
    let whole = prev.body.as_ref().unwrap();
    let (tb, _) = body_of_index(prev, Kind::Edge, index)?;
    let (sb, slot_local) = body_of_index(prev, Kind::Face, slot_face)?;
    if tb == sb {
        return Err("the tabs and slots must be on different bodies".into());
    }
    let sf = whole.shape.face_info(slot_face).map_err(geo)?;
    // The tab body's side face and its thickness at the edge.
    let faces = whole.shape.edge_faces()[index as usize].clone();
    let side = faces
        .iter()
        .filter_map(|f| whole.shape.face_info(*f).ok())
        .find(|f| f.kind == SurfaceKind::Plane && f.normal.dot(sf.normal).abs() < 1e-6)
        .ok_or("the tab edge must be on a face square to the slot face")?;
    let (_, side_local) = body_of_index(
        prev,
        Kind::Face,
        faces
            .iter()
            .copied()
            .find(|f| {
                whole
                    .shape
                    .face_info(*f)
                    .is_ok_and(|i| (i.normal - side.normal).length() < 1e-9)
            })
            .unwrap(),
    )?;
    let tt = thickness(prev, tb, side_local)?;
    let st = thickness(prev, sb, slot_local)?;
    let height = p.height.unwrap_or(st);
    let e = (info.end - info.start).normalize();
    // The span along the edge the tabs share.
    let along = |r: &Option<Ref>, default: f64| -> RResult<f64> {
        match r {
            Some(r) => Ok((resolve(prev, r)?.point()? - info.start)
                .dot(e)
                .clamp(0.0, info.length)),
            None => Ok(default),
        }
    };
    // With one reference, run to the end of the edge farther from it.
    let far = |s: f64| {
        if s < info.length / 2.0 {
            info.length
        } else {
            0.0
        }
    };
    let (s0, s1) = match (&p.start, &p.end) {
        (Some(_), None) => {
            let s = along(&p.start, 0.0)?;
            (s, far(s))
        }
        (None, Some(_)) => {
            let s = along(&p.end, 0.0)?;
            (far(s), s)
        }
        _ => (along(&p.start, 0.0)?, along(&p.end, info.length)?),
    };
    let (s0, s1) = (s0.min(s1), s0.max(s1));
    let len = s1 - s0;
    if len <= p.length {
        return Err("the tabs don't fit between their start and end".into());
    }
    let centers: Vec<f64> = match p.spacing {
        Some(s) if s > 0.0 => {
            let n = ((len - p.length) / s).floor() as i64 + 1;
            let first = (len - (n - 1) as f64 * s) / 2.0;
            (0..n.max(1)).map(|i| s0 + first + i as f64 * s).collect()
        }
        _ => (0..p.count)
            .map(|i| s0 + len * (i as f64 + 0.5) / p.count as f64)
            .collect(),
    };
    let (cl_len, cl_wid) = (
        p.length_clearance.unwrap_or(p.clearance),
        p.width_clearance.unwrap_or(p.clearance),
    );
    let reach = if p.through_all {
        whole.shape.bbox().map_or(1e3, |b| b.diagonal() * 2.0)
    } else {
        st + 2e-3 + height
    };
    // Into the slot body, from the face the tab body rests on.
    let into = -sf.normal;
    let on_face = info.start - sf.normal * sf.normal.dot(info.start - sf.point);
    let tab_plane = Plane::new(on_face, e, side.normal);
    let mut tabs = Vec::new();
    let mut slots = Vec::new();
    for c in centers {
        let a = c - p.length / 2.0;
        let b = c + p.length / 2.0;
        if a < s0 - 1e-9 || b > s1 + 1e-9 {
            return Err("the tabs don't fit along the edge".into());
        }
        // Tab: in the tab body's wall, from the slot face through it; only
        // its far end's corners are finished.
        let pts = [(a, 0.0), (b, 0.0), (b, height), (a, height)]
            .map(|(x, h)| tab_plane.to_local(on_face + e * x + into * h));
        let face = region(
            tab_plane,
            finished(&pts, &[false, false, true, true], p.corners)?,
        )?;
        tabs.push(face.prism(-side.normal * tt).map_err(geo)?.shape);
        // Slot: the tab's footprint grown by the clearances, through the slot body.
        let slot_plane = Plane::new(on_face + sf.normal * 1e-3, e, sf.normal);
        let corner = |x: f64, w: f64| slot_plane.to_local(on_face + e * x - side.normal * w);
        let pts = [
            corner(a - cl_len, -cl_wid),
            corner(b + cl_len, -cl_wid),
            corner(b + cl_len, tt + cl_wid),
            corner(a - cl_len, tt + cl_wid),
        ];
        let face = region(slot_plane, finished(&pts, &[true; 4], p.corners)?)?;
        slots.push(face.prism(into * reach).map_err(geo)?.shape);
    }
    let mut tab_body = prev.bodies[tb].geometry.clone();
    for (k, tab) in tabs.iter().enumerate() {
        let r = tab_body.shape.boolean(BoolOp::Fuse, tab).map_err(geo)?;
        tab_body = named(fid, &tab_body.names, r, &format!("tab{k}"));
    }
    let mut slot_body = prev.bodies[sb].geometry.clone();
    for (k, slot) in slots.iter().enumerate() {
        let r = slot_body.shape.boolean(BoolOp::Cut, slot).map_err(geo)?;
        slot_body = named(fid, &slot_body.names, r, &format!("slot{k}"));
    }
    state.bodies[tb].geometry = tab_body;
    state.bodies[sb].geometry = slot_body;
    state.aggregate_bodies()
}

/// A closed polygon's outline with the corners marked in `which` filleted or
/// chamfered.
fn finished(pts: &[DVec2], which: &[bool], corner: TabCorner) -> RResult<Vec<GCurve>> {
    let n = pts.len();
    // Arcs run counter-clockwise: walk the polygon that way.
    let area: f64 = (0..n).map(|i| pts[i].perp_dot(pts[(i + 1) % n])).sum();
    let (pts, which): (Vec<DVec2>, Vec<bool>) = if area < 0.0 {
        (
            pts.iter().rev().copied().collect(),
            which.iter().rev().copied().collect(),
        )
    } else {
        (pts.to_vec(), which.to_vec())
    };
    let size = match corner {
        TabCorner::Sharp => 0.0,
        TabCorner::Fillet { radius } => radius,
        TabCorner::Chamfer { distance } => distance,
    };
    // Each corner's points where its two sides now end.
    let mut ends: Vec<(DVec2, DVec2)> = Vec::with_capacity(n);
    for i in 0..n {
        let p = pts[i];
        if !which[i] || size <= 0.0 {
            ends.push((p, p));
            continue;
        }
        let (prev, next) = (pts[(i + n - 1) % n], pts[(i + 1) % n]);
        let shortest = (prev - p).length().min((next - p).length());
        if size * 2.0 > shortest + 1e-9 {
            return Err("the tab corner size is more than half a side".into());
        }
        ends.push((
            p + (prev - p).normalize() * size,
            p + (next - p).normalize() * size,
        ));
    }
    let mut out = Vec::new();
    for i in 0..n {
        let (t_in, t_out) = ends[i];
        if t_in != t_out {
            match corner {
                TabCorner::Fillet { .. } => {
                    let p = pts[i];
                    let center = t_in + t_out - p;
                    out.push(GCurve::Arc {
                        center,
                        start: t_in,
                        end: t_out,
                    });
                }
                _ => out.push(GCurve::Line { a: t_in, b: t_out }),
            }
        }
        let next = ends[(i + 1) % n].0;
        out.push(GCurve::Line { a: t_out, b: next });
    }
    Ok(out)
}
