//! Flattening a sheet metal body.
//!
//! One side of the sheet is a set of planar walls and cylindrical bends joined
//! tangentially; its thickness faces meet it at sharp edges. Starting from the
//! fixed face, the walk crosses tangent edges only, so it visits exactly that
//! side. Each wall gets the rigid motion that lays it in the fixed face's
//! plane; each bend is rolled out at its neutral radius, so its flat width is
//! its bend allowance. Every visited face is mapped through its (u, v)
//! parameters, the images are joined, and the region is extruded back by the
//! thickness.

use std::collections::VecDeque;
use std::f64::consts::TAU;

use of_geom::{CurveKind, DAffine3, DVec3, FaceFrame, FaceSurface, FlatSource, Kind, Plane, UvMap};

use super::{RResult, base_name, geo};
use crate::build::Body;
use crate::naming::Naming;
use crate::sheet_metal::{BendLine, BendRecord, FormMark, SheetMetalBody, bend_allowance};

/// A flattened body.
pub(crate) struct Flat {
    pub shape: of_geom::Shape,
    pub names: Naming,
    /// The fixed face's plane, normal pointing out of the material.
    pub plane: Plane,
    pub bend_lines: Vec<BendLine>,
    pub marks: Vec<FormMark>,
}

/// How a bend face is laid flat.
#[derive(Clone, Copy)]
struct Roll {
    /// The rigid motion of the wall it was entered from.
    base: DAffine3,
    /// The angle parameter of the entry tangent line.
    entry: f64,
    /// +1 if the bend runs toward larger angles from the entry.
    sense: f64,
    /// Flat length per radian.
    rho: f64,
    /// Total bend angle, radians.
    angle: f64,
    radius: f64,
    up: bool,
    record: Option<(crate::FeatureId, u32)>,
}

#[derive(Clone, Copy)]
enum Place {
    Wall(DAffine3),
    Bend(Roll),
}

/// Shift an angle by whole turns to lie nearest `around`.
fn near(angle: f64, around: f64) -> f64 {
    angle + ((around - angle) / TAU).round() * TAU
}

fn rotation_about(origin: DVec3, axis: DVec3, angle: f64) -> DAffine3 {
    DAffine3::from_translation(origin)
        * DAffine3::from_axis_angle(axis.normalize(), angle)
        * DAffine3::from_translation(-origin)
}

/// Which way angles grow on a cylinder frame.
fn spin(f: &FaceFrame) -> DVec3 {
    f.x.cross(f.y)
}

fn tangent(f: &FaceFrame, u: f64) -> DVec3 {
    f.y * u.cos() - f.x * u.sin()
}

fn record_of<'a>(records: &'a [BendRecord], names: &[String]) -> Option<&'a BendRecord> {
    records.iter().find(|r| {
        names
            .iter()
            .any(|n| r.faces.iter().any(|f| base_name(n) == base_name(f)))
    })
}

/// The affine map from a plane's (u, v) to its image in `flat`'s 2D frame.
fn plane_map(f: &FaceFrame, m: &DAffine3, flat: &Plane) -> UvMap {
    let p0 = m.transform_point3(f.origin) - flat.origin;
    let (xu, xv) = (m.transform_vector3(f.x), m.transform_vector3(f.y));
    let (fx, fy) = (flat.x_dir, flat.y_dir());
    UvMap([
        xu.dot(fx),
        xv.dot(fx),
        p0.dot(fx),
        xu.dot(fy),
        xv.dot(fy),
        p0.dot(fy),
    ])
}

/// Model point of a bend's (u, v) once rolled flat.
fn rolled(f: &FaceFrame, roll: &Roll, u: f64, v: f64) -> DVec3 {
    let at_entry = f.point(roll.entry, v);
    let dir = tangent(f, roll.entry).normalize();
    roll.base.transform_point3(at_entry)
        + roll.base.transform_vector3(dir) * ((u - roll.entry) * roll.rho)
}

fn bend_map(f: &FaceFrame, roll: &Roll, flat: &Plane) -> UvMap {
    let p0 = rolled(f, roll, 0.0, 0.0) - flat.origin;
    let du = rolled(f, roll, 1.0, 0.0) - rolled(f, roll, 0.0, 0.0);
    let dv = rolled(f, roll, 0.0, 1.0) - rolled(f, roll, 0.0, 0.0);
    let (fx, fy) = (flat.x_dir, flat.y_dir());
    UvMap([
        du.dot(fx),
        dv.dot(fx),
        p0.dot(fx),
        du.dot(fy),
        dv.dot(fy),
        p0.dot(fy),
    ])
}

/// The rigid motion of the wall leaving a bend at angle `exit`.
fn leaving(f: &FaceFrame, roll: &Roll, exit: f64) -> DAffine3 {
    let turn = exit - roll.entry;
    let dir = roll
        .base
        .transform_vector3(tangent(f, roll.entry).normalize());
    DAffine3::from_translation(dir * (turn * roll.rho))
        * roll.base
        * rotation_about(f.origin, spin(f), -turn)
}

/// How one side of a sheet metal body lays flat.
pub(crate) struct Layout {
    /// The fixed face's plane, normal out of the material.
    pub plane: Plane,
    /// Each face on the fixed face's side, with its map into `plane`.
    pub faces: Vec<(u32, UvMap)>,
    /// Per face of the body: whether it is on that side.
    pub side: Vec<bool>,
    pub bend_lines: Vec<BendLine>,
    pub fixed: u32,
}

/// The body with its forms pressed out again: what they removed put back
/// and what they added taken away.
pub(crate) fn unformed(body: &Body, record: &SheetMetalBody) -> RResult<Body> {
    let mut body = body.clone();
    for form in &record.forms {
        for (shape, op) in [
            (&form.removed, of_geom::BoolOp::Fuse),
            (&form.added, of_geom::BoolOp::Cut),
        ] {
            let Some(shape) = shape else { continue };
            if shape.count(Kind::Solid) == 0 {
                continue;
            }
            let r = body.shape.boolean(op, shape).map_err(geo)?;
            body = Body {
                names: Naming::from_history(
                    &r.shape,
                    &r.history,
                    crate::naming::Inputs {
                        named: [Some(&body.names), None],
                    },
                    &mut |_| Some(format!("{}.unformed", form.feature)),
                    "unformed",
                ),
                shape: r.shape,
            };
        }
    }
    Ok(body)
}

/// Where a 3D point on the sheet lands in the flat: through the flat face on
/// the fixed side nearest to it.
fn flat_point(body: &Body, layout: &Layout, t: f64, p: DVec3) -> Option<DVec3> {
    let mut best: Option<(f64, DVec3)> = None;
    for (f, map) in &layout.faces {
        let fr = body.shape.face_frame(*f).ok()?;
        if fr.surface != FaceSurface::Plane {
            continue;
        }
        let n = fr.normal(0.0, 0.0);
        let depth = (fr.origin - p).dot(n);
        if !(-1e-6..=t + 1e-6).contains(&depth) {
            continue;
        }
        let c = body.shape.face_info(*f).ok()?.centroid;
        let d = (c - p).length();
        if best.is_some_and(|(bd, _)| bd <= d) {
            continue;
        }
        let q = p + n * depth - fr.origin;
        let uv = of_geom::DVec2::new(q.dot(fr.x), q.dot(fr.y));
        let xy = map.apply(uv);
        best = Some((d, layout.plane.to_world(xy)));
    }
    best.map(|b| b.1)
}

pub(crate) fn flatten(body: &Body, record: &SheetMetalBody, fixed: Option<u32>) -> RResult<Flat> {
    let fixed_name = fixed.and_then(|f| body.names.face_name(f).map(str::to_string));
    let restored;
    let (body, fixed) = if record.forms.is_empty() {
        (body, fixed)
    } else {
        restored = unformed(body, record)?;
        let fixed = fixed_name.as_deref().and_then(|n| restored.names.face(n));
        (&restored, fixed)
    };
    let laid = layout(body, record, fixed)?;
    let marks = record
        .forms
        .iter()
        .map(|f| FormMark {
            feature: f.feature,
            center: flat_point(body, &laid, record.params.thickness, DVec3::from(f.center))
                .map(|p| p.to_array())
                .unwrap_or(f.center),
            outline: f
                .outline
                .iter()
                .filter_map(|q| flat_point(body, &laid, record.params.thickness, DVec3::from(*q)))
                .map(|p| p.to_array())
                .collect(),
            punch_id: f.punch_id.clone(),
            angle: f.angle,
        })
        .collect();
    let Layout {
        plane: flat,
        faces,
        side,
        bend_lines: lines,
        fixed,
    } = laid;
    let shape = &body.shape;
    let t = record.params.thickness;
    let edge_faces = shape.edge_faces();
    let (solid, sources) = shape
        .flatten(&faces, &flat, -flat.normal * t)
        .map_err(|e| format!("can't flatten: {}", e.0))?;
    let fixed_name = body.names.face_name(fixed).unwrap_or("flat").to_string();
    let names: Vec<Vec<String>> = sources
        .iter()
        .map(|s| {
            vec![match *s {
                FlatSource::Plane => fixed_name.clone(),
                FlatSource::Back => format!("{}.flat.back", record.feature),
                FlatSource::Edge(e) => edge_faces
                    .get(e as usize)
                    .and_then(|fs| fs.iter().find(|f| !side[**f as usize]))
                    .and_then(|f| body.names.face_name(*f))
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{}.flat.e{e}", record.feature)),
                FlatSource::Unknown => format!("{}.flat.u", record.feature),
            }]
        })
        .collect();
    let names = Naming::from_faces(&solid, names);
    Ok(Flat {
        shape: solid,
        names,
        plane: flat,
        bend_lines: lines,
        marks,
    })
}

pub(crate) fn layout(body: &Body, record: &SheetMetalBody, fixed: Option<u32>) -> RResult<Layout> {
    let shape = &body.shape;
    let t = record.params.thickness;
    let nfaces = shape.count(Kind::Face);
    let frames: Vec<FaceFrame> = (0..nfaces)
        .map(|i| shape.face_frame(i))
        .collect::<Result<_, _>>()
        .map_err(geo)?;
    let fixed = match fixed {
        Some(f) => f,
        None => body
            .names
            .face(&record.fixed_face)
            .ok_or("the sheet metal body's fixed face no longer exists")?,
    };
    let ff = &frames[fixed as usize];
    if ff.surface != FaceSurface::Plane {
        return Err("the fixed face must be flat".into());
    }
    let outward = ff.normal(0.0, 0.0);
    let flat = Plane::new(ff.origin, ff.x, outward);
    let edge_faces = shape.edge_faces();
    let face_edges = shape.face_edges();
    let edge = |e: u32| shape.edge_info(e).map_err(geo);
    let normal_at = |f: u32, p: DVec3| {
        let fr = &frames[f as usize];
        match fr.surface {
            FaceSurface::Plane => fr.normal(0.0, 0.0),
            _ => fr.normal(fr.angle_of(p), 0.0),
        }
    };

    let facets: std::collections::HashSet<&str> =
        record.facets.iter().map(|n| base_name(n)).collect();
    let is_facet = |f: u32| {
        body.names.faces[f as usize]
            .iter()
            .any(|n| facets.contains(base_name(n)))
    };
    let mut hinges: Vec<BendLine> = Vec::new();
    let mut place: Vec<Option<Place>> = vec![None; nfaces as usize];
    place[fixed as usize] = Some(Place::Wall(DAffine3::IDENTITY));
    let mut queue = VecDeque::from([fixed]);
    while let Some(f) = queue.pop_front() {
        let here = place[f as usize].unwrap();
        for &e in &face_edges[f as usize] {
            let info = edge(e)?;
            for &g in &edge_faces[e as usize] {
                if g == f || place[g as usize].is_some() {
                    continue;
                }
                // Only tangent neighbours are on this side of the sheet,
                // except across the edges between facets.
                let gf = &frames[g as usize];
                if normal_at(f, info.mid).dot(normal_at(g, info.mid)) < 1.0 - 1e-6 {
                    let (Place::Wall(m), true) = (here, is_facet(f) && is_facet(g)) else {
                        continue;
                    };
                    if gf.surface != FaceSurface::Plane || info.kind != CurveKind::Line {
                        continue;
                    }
                    let (nf, ng) = (normal_at(f, info.mid), normal_at(g, info.mid));
                    let axis = (info.end - info.start).normalize();
                    let angle = ng.cross(nf).dot(axis).atan2(ng.dot(nf));
                    let turn = rotation_about(info.start, axis, angle);
                    place[g as usize] = Some(Place::Wall(m * turn));
                    let centroid = shape.face_info(g).map_err(geo)?.centroid;
                    hinges.push(BendLine {
                        a: m.transform_point3(info.start).to_array(),
                        b: m.transform_point3(info.end).to_array(),
                        up: (centroid - info.start).dot(nf) > 0.0,
                        angle: angle.abs(),
                        radius: 0.0,
                        feature: Some(record.feature),
                        index: None,
                    });
                    queue.push_back(g);
                    continue;
                }
                let next = match (here, gf.surface) {
                    (Place::Wall(m), FaceSurface::Plane) => Place::Wall(m),
                    (Place::Wall(m), FaceSurface::Cylinder { radius }) => {
                        if info.kind != CurveKind::Line {
                            return Err("a bend meets a wall along a curved edge".into());
                        }
                        Place::Bend(enter(
                            gf,
                            radius,
                            m,
                            info.mid,
                            &body.names.faces[g as usize],
                            record,
                            t,
                        )?)
                    }
                    (Place::Bend(roll), FaceSurface::Plane) => {
                        let ff = &frames[f as usize];
                        let exit = near(ff.angle_of(info.mid), roll.entry);
                        Place::Wall(leaving(ff, &roll, exit))
                    }
                    (Place::Bend(roll), FaceSurface::Cylinder { radius }) => {
                        let ff = &frames[f as usize];
                        let same_axis = gf.z.cross(ff.z).length() < 1e-9
                            && (gf.origin - ff.origin).cross(ff.z).length() < 1e-6
                            && matches!(ff.surface, FaceSurface::Cylinder { radius: r } if (r - radius).abs() < 1e-6);
                        if same_axis {
                            // Another piece of the same bend: same roll, in its frame.
                            let entry = near(
                                gf.angle_of(ff.point(roll.entry, 0.0)),
                                (gf.u.0 + gf.u.1) / 2.0,
                            );
                            let sense = if spin(gf).dot(spin(ff)) > 0.0 {
                                roll.sense
                            } else {
                                -roll.sense
                            };
                            Place::Bend(Roll {
                                entry,
                                sense,
                                ..roll
                            })
                        } else {
                            if info.kind != CurveKind::Line {
                                return Err("two bends meet along a curved edge".into());
                            }
                            let exit = near(ff.angle_of(info.mid), roll.entry);
                            let m = leaving(ff, &roll, exit);
                            Place::Bend(enter(
                                gf,
                                radius,
                                m,
                                info.mid,
                                &body.names.faces[g as usize],
                                record,
                                t,
                            )?)
                        }
                    }
                    (_, _) => {
                        let name = body.names.face_name(g).unwrap_or("?");
                        return Err(format!(
                            "face {name} can't be flattened: only flat walls and cylindrical bends unfold"
                        ));
                    }
                };
                place[g as usize] = Some(next);
                queue.push_back(g);
            }
        }
    }

    let mut faces = Vec::new();
    let mut lines: Vec<BendLine> = Vec::new();
    for (i, p) in place.iter().enumerate() {
        let Some(p) = p else { continue };
        let fr = &frames[i];
        match p {
            Place::Wall(m) => faces.push((i as u32, plane_map(fr, m, &flat))),
            Place::Bend(roll) => {
                faces.push((i as u32, bend_map(fr, roll, &flat)));
                let mid = roll.entry + roll.sense * roll.angle / 2.0;
                let (a, b) = (rolled(fr, roll, mid, fr.v.0), rolled(fr, roll, mid, fr.v.1));
                // Pieces of one bend share a line: keep the longest extent.
                let dir = (b - a).normalize_or_zero();
                if let Some(line) = lines.iter_mut().find(|l| {
                    let la = DVec3::from(l.a);
                    let ld = (DVec3::from(l.b) - la).normalize_or_zero();
                    ld.cross(dir).length() < 1e-9 && (a - la).cross(ld).length() < 1e-6
                }) {
                    let la = DVec3::from(line.a);
                    let ld = (DVec3::from(line.b) - la).normalize_or_zero();
                    let ts = [
                        0.0,
                        (DVec3::from(line.b) - la).dot(ld),
                        (a - la).dot(ld),
                        (b - la).dot(ld),
                    ];
                    let lo = ts.iter().copied().fold(f64::INFINITY, f64::min);
                    let hi = ts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    line.a = (la + ld * lo).to_array();
                    line.b = (la + ld * hi).to_array();
                } else {
                    lines.push(BendLine {
                        a: a.to_array(),
                        b: b.to_array(),
                        up: roll.up,
                        angle: roll.angle,
                        radius: roll.radius,
                        feature: roll.record.map(|r| r.0),
                        index: roll.record.map(|r| r.1),
                    });
                }
            }
        }
    }
    lines.extend(hinges.into_iter().filter(|h| h.angle > 1e-6));
    let side: Vec<bool> = place.iter().map(Option::is_some).collect();
    Ok(Layout {
        plane: flat,
        faces,
        side,
        bend_lines: lines,
        fixed,
    })
}

/// Start rolling a bend face entered from a wall with motion `base` along the
/// tangent line through `on_line`.
fn enter(
    f: &FaceFrame,
    radius: f64,
    base: DAffine3,
    on_line: DVec3,
    names: &[String],
    record: &SheetMetalBody,
    t: f64,
) -> RResult<Roll> {
    let mid = (f.u.0 + f.u.1) / 2.0;
    let entry_raw = near(f.angle_of(on_line), mid);
    let (entry, sense) = if (entry_raw - f.u.0).abs() <= (entry_raw - f.u.1).abs() {
        (entry_raw, 1.0)
    } else {
        (entry_raw, -1.0)
    };
    // Concave when the face looks at the axis: then this is the bend's inside.
    let p = f.point(mid, (f.v.0 + f.v.1) / 2.0);
    let radial = (p - (f.origin + f.z * f.height_of(p))).normalize();
    let concave = f.normal(mid, 0.0).dot(radial) < 0.0;
    let rec = record_of(&record.bends, names);
    let inside = match rec {
        Some(r) => r.radius,
        None if concave => radius,
        None => radius - t,
    };
    let angle = match rec {
        Some(r) => r.angle,
        None => f.u.1 - f.u.0,
    };
    if !(angle > 1e-9 && angle < TAU) || inside <= 0.0 {
        return Err("a bend has no angle or radius".into());
    }
    let allowance = rec
        .map(|r| r.allowance.clone())
        .unwrap_or(record.params.allowance.clone());
    let ba = bend_allowance(&allowance, angle, inside, t)?;
    Ok(Roll {
        base,
        entry,
        sense,
        rho: ba / angle,
        angle,
        radius: inside,
        up: concave,
        record: rec.map(|r| (r.feature, r.index)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_shifts_by_turns() {
        assert!((near(-3.0, 3.0) - (-3.0 + TAU)).abs() < 1e-12);
        assert!((near(1.0, 1.2) - 1.0).abs() < 1e-12);
    }
}
