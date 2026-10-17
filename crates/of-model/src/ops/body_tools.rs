//! Delete/keep body, intersect, scale, flex, deform, indent, insert part,
//! defeature and slicing.

use std::collections::BTreeMap;

use of_geom::{BoolOp, DAffine3, DMat3, DVec3, Kind, Shape, SurfaceKind};

use super::geo;
use crate::body_tools::{
    Defeature, Deform, DeformMode, DeleteBody, Flex, FlexMode, Indent, InsertPart, Intersect,
    IntersectMode, Scale, ScaleAbout, Slicing,
};
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::resolve::{RResult, Resolved, resolve};
use crate::{BodyId, FeatureId, SolidBody};

fn renamed(fid: FeatureId, body: &Body, r: &of_geom::OpResult) -> Body {
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(&body.names), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    Body {
        shape: r.shape.clone(),
        names,
    }
}

/// Indices into `bodies` and `surfaces` of the chosen IDs (all solids when
/// empty, and all surfaces too when `surfaces_too`).
fn chosen(state: &State, ids: &[BodyId], surfaces_too: bool) -> RResult<(Vec<usize>, Vec<usize>)> {
    if ids.is_empty() {
        let surfaces = if surfaces_too {
            (0..state.surfaces.len()).collect()
        } else {
            Vec::new()
        };
        return Ok(((0..state.bodies.len()).collect(), surfaces));
    }
    let mut solids = Vec::new();
    let mut surfaces = Vec::new();
    for id in ids {
        match state.any_body(id)? {
            (_, false) => solids.push(state.bodies.iter().position(|b| &b.id == id).unwrap()),
            (_, true) => surfaces.push(state.surfaces.iter().position(|b| &b.id == id).unwrap()),
        }
    }
    Ok((solids, surfaces))
}

pub(crate) fn delete_body(
    _fid: FeatureId,
    p: &DeleteBody,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    for id in &p.bodies {
        prev.any_body(id)?;
    }
    let gone = |id: &BodyId| p.bodies.contains(id) != p.keep;
    state.bodies.retain(|b| !gone(&b.id));
    state.surfaces.retain(|b| !gone(&b.id));
    state.aggregate_bodies()
}

pub(crate) fn intersect(
    fid: FeatureId,
    p: &Intersect,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let mut faces: Vec<Shape> = Vec::new();
    let mut solids: Vec<(BodyId, Shape)> = Vec::new();
    let mut used_surfaces = Vec::new();
    for r in &p.tools {
        match resolve(prev, r)? {
            Resolved::Body { id, .. } => {
                let (b, surface) = prev.any_body(&id)?;
                faces.push(b.geometry.shape.clone());
                if surface {
                    if p.cap_planar {
                        faces.push(b.geometry.shape.cap_loops().map_err(geo)?);
                    }
                    used_surfaces.push(id);
                } else {
                    solids.push((id, b.geometry.shape.clone()));
                }
            }
            Resolved::Face { index, .. } => faces.push(prev.face_shape(index)?),
            Resolved::Plane(plane) => faces.push(super::big_face(prev, &plane)?),
            _ => return Err("intersect with bodies, faces or planes".into()),
        }
    }
    let cells = Shape::compound(&faces).cells().map_err(geo)?;
    if cells.is_empty() {
        return Err("the tools enclose no region".into());
    }
    let mut kept = Vec::new();
    for cell in cells {
        let inside = cell.inner_point().map_err(geo)?;
        let within = solids
            .iter()
            .filter(|(_, s)| s.classify(inside).is_ok_and(|c| c == 1))
            .count();
        let wanted = match p.mode {
            IntersectMode::Intersection => within > 0,
            IntersectMode::Internal => within == 0,
            IntersectMode::Both => true,
        };
        let excluded = p
            .exclude
            .iter()
            .any(|q| cell.classify(DVec3::from_array(*q)).is_ok_and(|c| c == 1));
        if wanted && !excluded {
            kept.push(cell);
        }
    }
    if kept.is_empty() {
        return Err("no region is left".into());
    }
    let named_cell = |i: usize, s: Shape| {
        let faces = (0..s.count(Kind::Face))
            .map(|f| vec![format!("{fid}.r{i}.f{f}")])
            .collect();
        Body {
            names: Naming::from_faces(&s, faces),
            shape: s,
        }
    };
    let mut regions: Vec<Body> = kept
        .into_iter()
        .enumerate()
        .map(|(i, s)| named_cell(i, s))
        .collect();
    if p.merge && regions.len() > 1 {
        let mut merged = regions.remove(0);
        for r in regions.drain(..) {
            let op = merged.shape.boolean(BoolOp::Fuse, &r.shape).map_err(geo)?;
            let names = Naming::from_history(
                &op.shape,
                &op.history,
                Inputs {
                    named: [Some(&merged.names), Some(&r.names)],
                },
                &mut |_| None,
                &fid.to_string(),
            );
            merged = Body {
                shape: op.shape,
                names,
            };
        }
        regions = crate::bodies::partition(&merged)?;
    }
    let consumed: Vec<BodyId> = solids.iter().map(|(id, _)| id.clone()).collect();
    state.bodies.retain(|b| !consumed.contains(&b.id));
    if p.consume_surfaces {
        state.surfaces.retain(|b| !used_surfaces.contains(&b.id));
    }
    for (i, geometry) in regions.into_iter().enumerate() {
        state.bodies.push(SolidBody {
            id: BodyId(format!("b{fid}:r{i}")),
            geometry,
        });
    }
    state.aggregate_bodies()
}

pub(crate) fn scale(fid: FeatureId, p: &Scale, prev: &State, state: &mut State) -> RResult<()> {
    let factors = match p.factors {
        Some(f) => DVec3::from_array(f),
        None => DVec3::splat(p.factor),
    };
    if !(factors.min_element() > 0.0 && factors.is_finite()) {
        return Err("scale factors must be more than zero".into());
    }
    let (solids, surfaces) = chosen(prev, &p.bodies, true)?;
    if solids.is_empty() && surfaces.is_empty() {
        return Err("there's nothing to scale".into());
    }
    // The frame the factors act in.
    let frame = match &p.about {
        ScaleAbout::Origin => DAffine3::IDENTITY,
        ScaleAbout::Centroid => {
            let all = Shape::compound(
                solids
                    .iter()
                    .map(|i| &prev.bodies[*i].geometry.shape)
                    .chain(surfaces.iter().map(|i| &prev.surfaces[*i].geometry.shape)),
            );
            let c = if solids.is_empty() {
                all.bbox().map_or(DVec3::ZERO, |b| b.center())
            } else {
                all.mass_props().map_err(geo)?.center
            };
            DAffine3::from_translation(c)
        }
        ScaleAbout::CoordinateSystem { coordinate_system } => {
            match resolve(prev, coordinate_system)? {
                Resolved::CoordSys { origin, x, y, z } => {
                    DAffine3::from_mat3_translation(DMat3::from_cols(x, y, z), origin)
                }
                _ => return Err("select a coordinate system".into()),
            }
        }
    };
    let rigid = frame.matrix3.determinant() > 0.0
        && frame != DAffine3::IDENTITY
        && !matches!(p.about, ScaleAbout::Centroid);
    let apply = |body: &Body| -> RResult<Body> {
        if rigid {
            let local = body.shape.transform(&frame.inverse()).map_err(geo)?;
            let scaled = local.shape.scale(DVec3::ZERO, factors).map_err(geo)?;
            let back = scaled.shape.transform(&frame).map_err(geo)?;
            let history = of_geom::compose(
                &of_geom::compose(&local.history, &scaled.history),
                &back.history,
            );
            Ok(renamed(
                fid,
                body,
                &of_geom::OpResult {
                    shape: back.shape,
                    history,
                },
            ))
        } else {
            let r = body.shape.scale(frame.translation, factors).map_err(geo)?;
            Ok(renamed(fid, body, &r))
        }
    };
    for i in solids {
        state.bodies[i].geometry = apply(&prev.bodies[i].geometry)?;
    }
    for i in surfaces {
        state.surfaces[i].geometry = apply(&prev.surfaces[i].geometry)?;
    }
    state.aggregate_bodies()
}

/// An orthonormal frame on an axis: (origin, x, y, z).
fn axis_frame(
    prev: &State,
    origin: &crate::Ref,
    axis: &crate::Ref,
    toward: Option<&crate::Ref>,
) -> RResult<(DVec3, DVec3, DVec3, DVec3)> {
    let o = resolve(prev, origin)?.point()?;
    let z = resolve(prev, axis)?.direction()?.normalize();
    let y = match toward {
        Some(r) => {
            let d = resolve(prev, r)?.direction()?;
            let d = d - z * d.dot(z);
            if d.length() < 1e-9 {
                return Err("the bend direction runs along the axis".into());
            }
            d.normalize()
        }
        None => z.any_orthonormal_vector(),
    };
    Ok((o, y.cross(z), y, z))
}

pub(crate) fn flex(fid: FeatureId, p: &Flex, prev: &State, state: &mut State) -> RResult<()> {
    let (o, x, y, z) = axis_frame(prev, &p.origin, &p.axis, p.toward.as_ref())?;
    let (solids, _) = chosen(prev, &p.bodies, false)?;
    if solids.is_empty() {
        return Err("there's no solid to flex".into());
    }
    let (t1, t2) = match p.trim {
        Some([a, b]) => (a.min(b), a.max(b)),
        None => {
            let all = Shape::compound(solids.iter().map(|i| &prev.bodies[*i].geometry.shape));
            let b = all.bbox().ok_or("the bodies are empty")?;
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for k in 0..8 {
                let c = DVec3::new(
                    if k & 1 == 0 { b.min.x } else { b.max.x },
                    if k & 2 == 0 { b.min.y } else { b.max.y },
                    if k & 4 == 0 { b.min.z } else { b.max.z },
                );
                let t = (c - o).dot(z);
                lo = lo.min(t);
                hi = hi.max(t);
            }
            (lo, hi)
        }
    };
    let len = t2 - t1;
    if len < 1e-9 {
        return Err("the trim planes must be apart".into());
    }
    let mode = p.mode.clone();
    let map = move |q: DVec3| -> DVec3 {
        let d = q - o;
        let (lx, ly, t) = (d.dot(x), d.dot(y), d.dot(z));
        let s = ((t - t1) / len).clamp(0.0, 1.0);
        let (nx, ny, nt) = match mode {
            FlexMode::Twist { angle } => {
                let a = angle * s;
                (lx * a.cos() - ly * a.sin(), lx * a.sin() + ly * a.cos(), t)
            }
            FlexMode::Taper { factor } => {
                let k = 1.0 + (factor - 1.0) * s;
                (lx * k, ly * k, t)
            }
            FlexMode::Stretch { distance } => {
                let nt = if t <= t1 {
                    t
                } else if t >= t2 {
                    t + distance
                } else {
                    t1 + (t - t1) * (1.0 + distance / len)
                };
                (lx, ly, nt)
            }
            FlexMode::Bend { angle } => {
                if angle.abs() < 1e-12 || t <= t1 {
                    (lx, ly, t)
                } else {
                    let r = len / angle;
                    let (phi, along) = if t >= t2 {
                        (angle, t - t2)
                    } else {
                        ((t - t1) / r, 0.0)
                    };
                    let ny = r - (r - ly) * phi.cos() + along * phi.sin();
                    let nt = t1 + (r - ly) * phi.sin() + along * phi.cos();
                    (lx, ny, nt)
                }
            }
        };
        o + x * nx + y * ny + z * nt
    };
    if let FlexMode::Taper { factor } = p.mode
        && factor <= 0.0
    {
        return Err("the taper factor must be more than zero".into());
    }
    for i in solids {
        let body = &prev.bodies[i].geometry;
        let r = body
            .shape
            .deform(p.accuracy.clamp(1, 16), map.clone())
            .map_err(geo)?;
        state.bodies[i].geometry = renamed(fid, body, &r);
    }
    state.aggregate_bodies()
}

/// Smooth falloff: 1 at 0, 0 from 1 on.
fn falloff(s: f64) -> f64 {
    if s >= 1.0 { 0.0 } else { (1.0 - s * s).powi(2) }
}

/// Points evenly spaced along a curve or edge reference.
fn curve_points(prev: &State, r: &crate::Ref, n: usize) -> RResult<Vec<DVec3>> {
    let shape = match prev.any_index_of(r) {
        Ok((Kind::Edge, i)) => prev
            .everything()
            .ok_or("there's no body")?
            .shape
            .sub_shape(Kind::Edge, i)
            .map_err(geo)?,
        _ => crate::curves::path(prev, r)?,
    };
    let mut pts: Vec<DVec3> = Vec::new();
    for e in 0..shape.count(Kind::Edge) {
        let s = shape.edge_samples(e, 1e-3, 0.05).map_err(geo)?;
        if let Some(last) = pts.last()
            && s.first().is_some_and(|f| (*f - *last).length() < 1e-9)
        {
            pts.extend(&s[1..]);
        } else {
            pts.extend(s);
        }
    }
    if pts.len() < 2 {
        return Err("the curve is too short".into());
    }
    // Resample by length.
    let mut acc = vec![0.0];
    for w in pts.windows(2) {
        acc.push(acc.last().unwrap() + (w[1] - w[0]).length());
    }
    let total = *acc.last().unwrap();
    Ok((0..n)
        .map(|k| {
            let at = total * k as f64 / (n - 1) as f64;
            let j = acc.partition_point(|a| *a < at).clamp(1, pts.len() - 1);
            let span = acc[j] - acc[j - 1];
            let u = if span > 0.0 {
                (at - acc[j - 1]) / span
            } else {
                0.0
            };
            pts[j - 1] + (pts[j] - pts[j - 1]) * u
        })
        .collect())
}

pub(crate) fn deform(fid: FeatureId, p: &Deform, prev: &State, state: &mut State) -> RResult<()> {
    let (solids, _) = chosen(prev, &p.bodies, false)?;
    let solids: Vec<usize> = match &p.mode {
        DeformMode::SurfacePush { tool, .. } => solids
            .into_iter()
            .filter(|i| &prev.bodies[*i].id != tool)
            .collect(),
        _ => solids,
    };
    if solids.is_empty() {
        return Err("there's no solid to deform".into());
    }
    let refine = p.accuracy.clamp(1, 16);
    let field: Box<dyn Fn(DVec3) -> DVec3> = match &p.mode {
        DeformMode::Point {
            point,
            direction,
            distance,
            radius,
        } => {
            let p0 = resolve(prev, point)?.point()?;
            let d = resolve(prev, direction)?.direction()?.normalize() * *distance;
            let r = *radius;
            Box::new(move |q| q + d * falloff((q - p0).length() / r))
        }
        DeformMode::CurveToCurve { from, to, radius } => {
            if from.len() != to.len() {
                return Err("pair each curve to move with one to move it to".into());
            }
            let mut samples = Vec::new();
            for (a, b) in from.iter().zip(to) {
                let pa = curve_points(prev, a, 40)?;
                let pb = curve_points(prev, b, 40)?;
                samples.extend(pa.into_iter().zip(pb).map(|(s, t)| (s, t - s)));
            }
            let r = *radius;
            Box::new(move |q| {
                let mut sum = DVec3::ZERO;
                let mut weight = 0.0;
                let mut reach: f64 = 0.0;
                for (s, d) in &samples {
                    let dist = (q - *s).length();
                    if dist < 1e-12 {
                        return q + *d;
                    }
                    let w = falloff(dist / r) / (dist * dist);
                    sum += *d * w;
                    weight += w;
                    reach = reach.max(falloff(dist / r));
                }
                if weight > 0.0 {
                    q + sum / weight * reach
                } else {
                    q
                }
            })
        }
        DeformMode::SurfacePush {
            tool,
            direction,
            distance,
        } => {
            let shape = prev.solid(tool)?.geometry.shape.clone();
            let d = resolve(prev, direction)?.direction()?.normalize() * *distance;
            let moved = shape
                .transform(&DAffine3::from_translation(d))
                .map_err(geo)?
                .shape;
            let mesh = crate::meshes::tessellate(&moved, crate::meshes::deflection(&moved, 0.002))?;
            let mesh = std::sync::Arc::new(mesh);
            Box::new(move |q| {
                if moved.classify(q).is_ok_and(|c| c == 1) {
                    let bvh = of_mesh::bvh::Bvh::new(&mesh);
                    bvh.closest(q).map_or(q, |c| c.point)
                } else {
                    q
                }
            })
        }
    };
    for i in solids {
        let body = &prev.bodies[i].geometry;
        let r = body.shape.deform(refine, &field).map_err(geo)?;
        state.bodies[i].geometry = renamed(fid, body, &r);
    }
    state.aggregate_bodies()
}

pub(crate) fn indent(fid: FeatureId, p: &Indent, prev: &State, state: &mut State) -> RResult<()> {
    if p.target == p.tool {
        return Err("the tool must be another body".into());
    }
    let target = prev.solid(&p.target)?.geometry.clone();
    let tool = prev.solid(&p.tool)?.geometry.shape.clone();
    let inner = tool.offset_shape(p.clearance).map_err(geo)?.shape;
    let mut result = target.clone();
    if !p.cut {
        if p.thickness <= 0.0 {
            return Err("an indent needs a wall thickness".into());
        }
        let outer = tool
            .offset_shape(p.clearance + p.thickness)
            .map_err(geo)?
            .shape;
        let wall = outer.boolean(BoolOp::Cut, &inner).map_err(geo)?.shape;
        let outside = wall.boolean(BoolOp::Cut, &target.shape).map_err(geo)?.shape;
        // Keep the wall that bulges out beyond the target, away from the tool.
        let tool_c = tool.mass_props().map_err(geo)?.center;
        let target_c = target.shape.mass_props().map_err(geo)?.center;
        let reach = (target_c - tool_c).length();
        let keep: Vec<Shape> = outside
            .solids()
            .into_iter()
            .filter(|s| {
                let touches = s.distance(&target.shape).is_ok_and(|d| d.value < 1e-6);
                let beyond = s
                    .mass_props()
                    .is_ok_and(|m| (m.center - tool_c).length() > reach);
                touches && beyond
            })
            .collect();
        if !keep.is_empty() {
            let r = result
                .shape
                .boolean(BoolOp::Fuse, &Shape::compound(&keep))
                .map_err(geo)?;
            result = renamed(fid, &result, &r);
        }
    }
    let r = result.shape.boolean(BoolOp::Cut, &inner).map_err(geo)?;
    if r.shape.count(Kind::Solid) == 0 {
        return Err("the tool removes the whole target".into());
    }
    let cut_names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(&result.names), None],
        },
        &mut |_| Some(format!("{fid}.indent")),
        &fid.to_string(),
    );
    let i = state.bodies.iter().position(|b| b.id == p.target).unwrap();
    state.bodies[i].geometry = Body {
        shape: r.shape,
        names: cut_names,
    };
    state.aggregate_bodies()
}

pub(crate) fn insert_part(
    fid: FeatureId,
    p: &InsertPart,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let path = std::path::Path::new(&p.path);
    let mut doc = crate::file::load(path).map_err(|e| format!("can't open {}: {e}", p.path))?;
    if let Some(c) = p.configuration {
        if !doc.configurations.items.iter().any(|i| i.id == c) {
            return Err(format!("{} has no configuration {c}", p.path));
        }
        doc.configurations.active = c;
    }
    let model = crate::Model::new(doc);
    if let Some(e) = model.statuses().into_iter().find_map(|s| s.error) {
        return Err(format!("{} doesn't build: {e}", p.path));
    }
    if let Some(hash) = crate::build::file_hash(path) {
        state.files.push((fid, path.to_path_buf(), hash));
    }
    let source = model.state();
    let r = DMat3::from_rotation_z(p.rotation[2])
        * DMat3::from_rotation_y(p.rotation[1])
        * DMat3::from_rotation_x(p.rotation[0]);
    let place = DAffine3::from_mat3_translation(r, DVec3::from_array(p.translation));
    let mirror = match &p.mirror {
        Some(m) => Some(resolve(prev, m)?.plane()?),
        None => None,
    };
    let bring = |b: &SolidBody| -> RResult<SolidBody> {
        let mut shape = b.geometry.shape.clone();
        if let Some(m) = &mirror {
            shape = shape.mirror(m.origin, m.normal).map_err(geo)?.shape;
        }
        shape = shape.transform(&place).map_err(geo)?.shape;
        // Same topology order throughout: carry the names across, prefixed.
        let faces = b
            .geometry
            .names
            .faces
            .iter()
            .map(|n| n.iter().map(|s| format!("{fid}.{s}")).collect())
            .collect();
        let names = Naming::from_faces(&shape, faces);
        Ok(SolidBody {
            id: BodyId(format!("b{fid}:{}", b.id.0)),
            geometry: Body { shape, names },
        })
    };
    let mut count = 0;
    if p.transfer.solids {
        for b in &source.bodies {
            state.bodies.push(bring(b)?);
            count += 1;
        }
    }
    if p.transfer.surfaces {
        for b in &source.surfaces {
            state.surfaces.push(bring(b)?);
            count += 1;
        }
    }
    if count == 0 {
        return Err(format!("{} has no bodies to insert", p.path));
    }
    state.aggregate_bodies()
}

pub(crate) fn defeature(
    fid: FeatureId,
    p: &Defeature,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (solids, _) = chosen(prev, &p.bodies, false)?;
    let whole = prev.body.as_ref().ok_or("there's no solid")?;
    let mut per_body: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
    for r in &p.faces {
        let (Kind::Face, g) = prev.index_of(r)? else {
            return Err("select faces".into());
        };
        let b = prev
            .bodies
            .iter()
            .position(|b| {
                b.geometry
                    .shape
                    .indices_in(&whole.shape, Kind::Face)
                    .is_ok_and(|ix| ix.contains(&g))
            })
            .ok_or("the face belongs to no solid")?;
        let local = prev.bodies[b]
            .geometry
            .shape
            .indices_in(&whole.shape, Kind::Face)
            .map_err(geo)?
            .iter()
            .position(|x| *x == g)
            .unwrap() as u32;
        per_body.entry(b).or_default().push(local);
    }
    for &b in &solids {
        let shape = &prev.bodies[b].geometry.shape;
        for f in 0..shape.count(Kind::Face) {
            let info = shape.face_info(f).map_err(geo)?;
            if info.kind != SurfaceKind::Cylinder && info.kind != SurfaceKind::Torus {
                continue;
            }
            // A hole's wall faces its axis; a fillet's doesn't, or is a torus.
            let radial = info.point
                - (info.axis_origin
                    + info.axis_dir * (info.point - info.axis_origin).dot(info.axis_dir));
            let hole = info.kind == SurfaceKind::Cylinder && info.normal.dot(radial) < 0.0;
            let small = if hole {
                p.holes_below.is_some_and(|d| 2.0 * info.radius < d)
            } else {
                p.fillets_below.is_some_and(|r| info.radius < r)
            };
            if small {
                per_body.entry(b).or_default().push(f);
            }
        }
    }
    if per_body.values().all(|v| v.is_empty()) {
        return Err("there's nothing to remove".into());
    }
    for (b, mut faces) in per_body {
        faces.sort_unstable();
        faces.dedup();
        let body = &prev.bodies[b].geometry;
        let r = body.shape.defeature(&faces).map_err(geo)?;
        state.bodies[b].geometry = renamed(fid, body, &r);
    }
    state.aggregate_bodies()
}

pub(crate) fn slicing(fid: FeatureId, p: &Slicing, prev: &State, state: &mut State) -> RResult<()> {
    let plane = resolve(prev, &p.plane)?.plane()?;
    let (solids, surfaces) = chosen(prev, &p.bodies, true)?;
    let target = Shape::compound(
        solids
            .iter()
            .map(|i| &prev.bodies[*i].geometry.shape)
            .chain(surfaces.iter().map(|i| &prev.surfaces[*i].geometry.shape)),
    );
    if target.count(Kind::Face) == 0 {
        return Err("there's nothing to slice".into());
    }
    let mut segments = BTreeMap::new();
    let mut all = Vec::new();
    for i in 0..p.count {
        let at = plane.offset(p.offset + p.spacing * i as f64);
        let face = super::big_face(prev, &at)?;
        let section = target.section(&face).map_err(geo)?;
        if section.count(Kind::Edge) == 0 {
            continue;
        }
        segments.insert(format!("slice{}", i + 1), section.clone());
        all.push(section);
    }
    if all.is_empty() {
        return Err("no slice plane crosses the bodies".into());
    }
    state.curves.insert(
        fid,
        crate::curves::CurveBody {
            shape: Shape::compound(&all),
            segments,
        },
    );
    Ok(())
}
