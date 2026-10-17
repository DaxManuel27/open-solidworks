//! Mesh bodies: importing meshes and point clouds, converting to and from
//! B-rep, decimating, segmenting, preparing scans, fitting surfaces and
//! solids to meshes, and mesh sections.

use std::collections::BTreeMap;
use std::sync::Arc;

use of_geom::{Analytic, BoolOp, Curve3, DVec3, Kind, Plane, Shape};
use of_mesh::TriMesh;
use of_mesh::fit::{self, Primitive};

use super::geo;
use super::surface::add_surfaces;
use crate::build::{Body, State};
use crate::meshes::{
    ConvertToMesh, DecimateMesh, DecimateTarget, FitKind, MeshBody, MeshImport, MeshImportMode,
    MeshPrep, MeshSections, MeshToBrep, MeshToSolid, PrepStep, SegmentMesh, SurfaceFromMesh,
};
use crate::naming::{Inputs, Naming};
use crate::resolve::{RResult, resolve};
use crate::{BodyId, FeatureId, Import, ImportFormat, SolidBody};

fn mesh_err(e: of_mesh::MeshError) -> String {
    e.to_string()
}

/// Read an imported mesh or point cloud.
fn read(i: &Import) -> RResult<TriMesh> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&i.data)
        .map_err(|_| "the imported data is corrupt".to_string())?;
    let format = match i.format {
        ImportFormat::Stl => of_mesh::io::Format::Stl,
        ImportFormat::Obj => of_mesh::io::Format::Obj,
        ImportFormat::Off => of_mesh::io::Format::Off,
        ImportFormat::Ply => of_mesh::io::Format::Ply,
        ImportFormat::ThreeMf => of_mesh::io::Format::ThreeMf,
        ImportFormat::Amf => of_mesh::io::Format::Amf,
        ImportFormat::Points => {
            let points = of_mesh::io::read_points(&bytes).map_err(mesh_err)?;
            return of_mesh::reconstruct::reconstruct(&points, 0.0).map_err(mesh_err);
        }
        ImportFormat::Step | ImportFormat::Iges => unreachable!("B-rep imports read elsewhere"),
    };
    of_mesh::io::read(format, &bytes).map_err(mesh_err)
}

/// A B-rep of a mesh's triangles, flat runs joined into single faces when
/// `group`. Faces are named `{fid}.t{triangle}`.
fn brep(fid: FeatureId, mesh: &TriMesh, group: bool, limit: u32) -> RResult<Body> {
    if mesh.triangles.len() > limit as usize {
        return Err(format!(
            "the mesh has {} triangles, more than the limit of {limit}; keep it as a graphics body or raise the limit",
            mesh.triangles.len()
        ));
    }
    let shell = Shape::mesh_brep(&mesh.positions, &mesh.triangles).map_err(geo)?;
    let faces = (0..shell.count(Kind::Face))
        .map(|t| vec![format!("{fid}.t{t}")])
        .collect();
    let mut body = Body {
        names: Naming::from_faces(&shell, faces),
        shape: shell,
    };
    if group {
        let r = body.shape.unify().map_err(geo)?;
        let names = Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&body.names), None],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        body = Body {
            shape: r.shape,
            names,
        };
    }
    Ok(body)
}

fn place_mesh(
    fid: FeatureId,
    state: &mut State,
    mesh: TriMesh,
    mode: MeshImportMode,
    group: bool,
    limit: u32,
) -> RResult<()> {
    match mode {
        MeshImportMode::Graphics => {
            state.meshes.push(MeshBody {
                id: BodyId(format!("b{fid}:m")),
                mesh: Arc::new(mesh),
            });
            Ok(())
        }
        MeshImportMode::Surface => add_surfaces(fid, state, brep(fid, &mesh, group, limit)?),
        MeshImportMode::Solid => {
            if !mesh.is_closed() {
                return Err("the mesh has holes, so it can't be a solid; fill them or import it as a surface".into());
            }
            let body = brep(fid, &mesh, group, limit)?;
            let solid = body.shape.solidify().map_err(geo)?;
            let names = Naming::from_history(
                &solid.shape,
                &solid.history,
                Inputs {
                    named: [Some(&body.names), None],
                },
                &mut |_| None,
                &fid.to_string(),
            );
            super::apply_tool(fid, state, solid.shape, names, BoolOp::Fuse, false)
        }
    }
}

pub(crate) fn import_mesh(fid: FeatureId, i: &Import, state: &mut State) -> RResult<()> {
    let options = i.mesh.clone().unwrap_or_default();
    let mut mesh = read(i)?;
    if !(options.scale.is_finite() && options.scale > 0.0) {
        return Err("the import scale must be more than zero".into());
    }
    if options.scale != 1.0 {
        mesh.scale(options.scale);
    }
    let MeshImport {
        mode,
        max_facets,
        group_facets,
        ..
    } = options;
    place_mesh(fid, state, mesh, mode, group_facets, max_facets)
}

fn mesh_index(state: &State, id: &BodyId) -> RResult<usize> {
    state
        .meshes
        .iter()
        .position(|m| &m.id == id)
        .ok_or_else(|| format!("body {} isn't a mesh body", id.0))
}

pub(crate) fn convert_to_mesh(
    fid: FeatureId,
    p: &ConvertToMesh,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (body, surface) = prev.any_body(&p.body)?;
    let mut mesh = crate::meshes::tessellate(&body.geometry.shape, p.chord.max(1e-5))?;
    let size = mesh.bounds().diagonal();
    mesh.weld(size * 1e-9);
    if !p.keep {
        if surface {
            state.surfaces.retain(|b| b.id != p.body);
        } else {
            state.bodies.retain(|b| b.id != p.body);
            state.aggregate_bodies()?;
        }
    }
    state.meshes.push(MeshBody {
        id: BodyId(format!("b{fid}:m")),
        mesh: Arc::new(mesh),
    });
    let _ = p.angle;
    Ok(())
}

pub(crate) fn decimate_mesh(
    _fid: FeatureId,
    p: &DecimateMesh,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let i = mesh_index(prev, &p.body)?;
    let mesh = &prev.meshes[i].mesh;
    let region = (!p.facets.is_empty()).then(|| {
        p.facets
            .iter()
            .filter_map(|t| mesh.triangles.get(*t as usize))
            .flatten()
            .copied()
            .collect::<std::collections::HashSet<u32>>()
    });
    let target = match p.target {
        DecimateTarget::Fraction { fraction } => of_mesh::decimate::Target::Fraction(fraction),
        DecimateTarget::Tolerance { tolerance } => of_mesh::decimate::Target::Tolerance(tolerance),
    };
    let out = of_mesh::decimate::decimate(
        mesh,
        of_mesh::decimate::Options {
            target,
            preserve_boundary: p.preserve_boundary,
        },
        region.as_ref(),
    );
    state.meshes[i].mesh = Arc::new(out);
    Ok(())
}

pub(crate) fn segment_mesh(
    _fid: FeatureId,
    p: &SegmentMesh,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let i = mesh_index(prev, &p.body)?;
    let mut mesh = (*prev.meshes[i].mesh).clone();
    mesh.groups = of_mesh::segment::segment(&mesh, p.angle, p.min_area);
    state.meshes[i].mesh = Arc::new(mesh);
    Ok(())
}

pub(crate) fn mesh_prep(
    _fid: FeatureId,
    p: &MeshPrep,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let i = mesh_index(prev, &p.body)?;
    let mut mesh = (*prev.meshes[i].mesh).clone();
    for step in &p.steps {
        match *step {
            PrepStep::Orient => {
                let (r, c) = of_mesh::prep::principal_frame(&mesh);
                for q in &mut mesh.positions {
                    *q = r * (*q - c);
                }
            }
            PrepStep::RemovePieces { fraction } => {
                mesh = of_mesh::prep::remove_small_pieces(&mesh, fraction.clamp(0.0, 1.0));
            }
            PrepStep::RemoveNoise { sigma } => {
                of_mesh::prep::remove_noise(&mut mesh, sigma.max(0.5))
            }
            PrepStep::Simplify { fraction } => {
                mesh = of_mesh::decimate::decimate(
                    &mesh,
                    of_mesh::decimate::Options {
                        target: of_mesh::decimate::Target::Fraction(fraction),
                        preserve_boundary: true,
                    },
                    None,
                );
            }
            PrepStep::Smooth {
                iterations,
                boundary,
            } => of_mesh::prep::smooth(&mut mesh, iterations as usize, boundary),
            PrepStep::FillHoles { max_edges } => {
                of_mesh::prep::fill_holes(&mut mesh, max_edges as usize);
            }
            PrepStep::SmoothBoundaries { iterations } => {
                of_mesh::prep::smooth_boundaries(&mut mesh, iterations as usize)
            }
        }
    }
    if mesh.triangles.is_empty() {
        return Err("the preparation removed every triangle".into());
    }
    state.meshes[i].mesh = Arc::new(mesh);
    Ok(())
}

pub(crate) fn mesh_to_brep(
    fid: FeatureId,
    p: &MeshToBrep,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let i = mesh_index(prev, &p.body)?;
    let mesh = (*prev.meshes[i].mesh).clone();
    state.meshes.remove(i);
    let mode = if p.solid {
        MeshImportMode::Solid
    } else {
        MeshImportMode::Surface
    };
    place_mesh(fid, state, mesh, mode, p.group_facets, p.max_facets)
}

/// Points (triangle corners and centres) and normals of a mesh region.
fn region(
    mesh: &TriMesh,
    facets: &[u32],
    group: Option<u32>,
) -> RResult<(TriMesh, Vec<DVec3>, Vec<DVec3>)> {
    let tris: Vec<u32> = if !facets.is_empty() {
        facets.to_vec()
    } else if let Some(g) = group {
        (0..mesh.triangles.len() as u32)
            .filter(|t| mesh.group(*t as usize) == g)
            .collect()
    } else {
        (0..mesh.triangles.len() as u32).collect()
    };
    if tris.iter().any(|t| *t as usize >= mesh.triangles.len()) {
        return Err("a picked triangle isn't in the mesh".into());
    }
    if tris.is_empty() {
        return Err("the region has no triangles".into());
    }
    let mut points = Vec::new();
    let mut normals = Vec::new();
    for t in &tris {
        let [a, b, c] = mesh.corners(*t as usize);
        let n = mesh.normal(*t as usize);
        for q in [(a + b + c) / 3.0, a, b, c] {
            points.push(q);
            normals.push(n);
        }
    }
    Ok((mesh.subset(&tris), points, normals))
}

/// The angular span of points about an axis frame: a range covering every
/// point, as (start, end) radians, the full turn when they go all the way
/// round.
fn angle_span(angles: &mut [f64]) -> (f64, f64) {
    use std::f64::consts::TAU;
    for a in angles.iter_mut() {
        *a = a.rem_euclid(TAU);
    }
    angles.sort_by(f64::total_cmp);
    let n = angles.len();
    let (mut gap, mut at) = (TAU - angles[n - 1] + angles[0], 0);
    for i in 1..n {
        let g = angles[i] - angles[i - 1];
        if g > gap {
            gap = g;
            at = i;
        }
    }
    if gap < 0.2 {
        return (0.0, TAU);
    }
    let start = angles[at];
    let end = angles[(at + n - 1) % n];
    let end = if end < start { end + TAU } else { end };
    (start, end)
}

/// A face of `primitive` covering `points`, `extend` mm past them.
fn face_for(primitive: Primitive, points: &[DVec3], extend: f64) -> RResult<Shape> {
    use std::f64::consts::{FRAC_PI_2, TAU};
    let frame_of = |origin: DVec3, axis: DVec3| {
        let x = axis.any_orthonormal_vector();
        (Plane::new(origin, x, axis), x, axis.cross(x))
    };
    let local = |frame: &Plane, x: DVec3, y: DVec3, p: DVec3| {
        let d = p - frame.origin;
        (d.dot(x), d.dot(y), d.dot(frame.normal))
    };
    let span = |v: &[f64], e: f64| {
        let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (lo - e, hi + e)
    };
    let around = |angles: &mut Vec<f64>, e: f64| {
        let (a, b) = angle_span(angles);
        if b - a >= TAU - 1e-9 {
            (0.0, TAU)
        } else {
            ((a - e).max(b - TAU), (b + e).min(a + TAU))
        }
    };
    match primitive {
        Primitive::Plane { origin, normal } => {
            let (frame, x, y) = frame_of(origin, normal);
            let (us, vs): (Vec<f64>, Vec<f64>) = points
                .iter()
                .map(|p| {
                    let (u, v, _) = local(&frame, x, y, *p);
                    (u, v)
                })
                .unzip();
            let (u0, u1) = span(&us, extend);
            let (v0, v1) = span(&vs, extend);
            Shape::analytic_face(Analytic::Plane, &frame, [u0, u1, v0, v1]).map_err(geo)
        }
        Primitive::Cylinder {
            origin,
            axis,
            radius,
        } => {
            let (frame, x, y) = frame_of(origin, axis);
            let mut angles: Vec<f64> = Vec::new();
            let mut hs: Vec<f64> = Vec::new();
            for p in points {
                let (u, v, h) = local(&frame, x, y, *p);
                angles.push(v.atan2(u));
                hs.push(h);
            }
            let (a0, a1) = around(&mut angles, extend / radius.max(1e-9));
            let (h0, h1) = span(&hs, extend);
            Shape::analytic_face(Analytic::Cylinder { radius }, &frame, [a0, a1, h0, h1])
                .map_err(geo)
        }
        Primitive::Cone {
            apex,
            axis,
            half_angle,
        } => {
            // A frame at the lowest point's height, where the radius is known.
            let ts: Vec<f64> = points.iter().map(|p| (*p - apex).dot(axis)).collect();
            let t0 = ts.iter().copied().fold(f64::INFINITY, f64::min).max(1e-6);
            let (frame, x, y) = frame_of(apex + axis * t0, axis);
            let radius = t0 * half_angle.tan();
            let mut angles: Vec<f64> = Vec::new();
            let mut vs: Vec<f64> = Vec::new();
            for (p, t) in points.iter().zip(&ts) {
                let (u, v, _) = local(&frame, x, y, *p);
                angles.push(v.atan2(u));
                vs.push((t - t0) / half_angle.cos());
            }
            let (a0, a1) = around(&mut angles, extend / radius.max(1e-6));
            let (v0, v1) = span(&vs, extend);
            Shape::analytic_face(
                Analytic::Cone { radius, half_angle },
                &frame,
                [a0, a1, v0.max(-t0 / half_angle.cos() + 1e-6), v1],
            )
            .map_err(geo)
        }
        Primitive::Sphere { center, radius } => {
            let (frame, x, y) = frame_of(center, DVec3::Z);
            let mut angles: Vec<f64> = Vec::new();
            let mut lats: Vec<f64> = Vec::new();
            for p in points {
                let (u, v, h) = local(&frame, x, y, *p);
                angles.push(v.atan2(u));
                lats.push((h / radius).clamp(-1.0, 1.0).asin());
            }
            let e = extend / radius.max(1e-9);
            let (a0, a1) = around(&mut angles, e);
            let (l0, l1) = span(&lats, e);
            Shape::analytic_face(
                Analytic::Sphere { radius },
                &frame,
                [a0, a1, l0.max(-FRAC_PI_2), l1.min(FRAC_PI_2)],
            )
            .map_err(geo)
        }
        Primitive::Torus {
            center,
            axis,
            major,
            minor,
        } => {
            let (frame, x, y) = frame_of(center, axis);
            let mut us: Vec<f64> = Vec::new();
            let mut vs: Vec<f64> = Vec::new();
            for p in points {
                let (a, b, h) = local(&frame, x, y, *p);
                us.push(b.atan2(a));
                vs.push(h.atan2((a * a + b * b).sqrt() - major));
            }
            let (u0, u1) = around(&mut us, extend / major.max(1e-9));
            let (v0, v1) = around(&mut vs, extend / minor.max(1e-9));
            Shape::analytic_face(Analytic::Torus { major, minor }, &frame, [u0, u1, v0, v1])
                .map_err(geo)
        }
    }
}

/// Fit one region: an analytic face, or a freeform one.
fn fit_face(
    region_mesh: &TriMesh,
    points: &[DVec3],
    normals: &[DVec3],
    kind: FitKind,
    extend: f64,
) -> RResult<Shape> {
    let primitive = match kind {
        FitKind::Freeform => {
            let grid = fit::grid(region_mesh, 12).map_err(mesh_err)?;
            return Shape::bspline_face(&grid).map_err(geo);
        }
        FitKind::Auto => fit::best(points, normals),
        FitKind::Plane => fit::plane(points),
        FitKind::Cylinder => fit::cylinder(points, normals),
        FitKind::Cone => fit::cone(points, normals),
        FitKind::Sphere => fit::sphere(points),
        FitKind::Torus => fit::torus(points),
    }
    .map_err(mesh_err)?;
    let size = region_mesh.bounds().diagonal().max(1e-9);
    // A poor automatic fit becomes freeform.
    if kind == FitKind::Auto && primitive.rms > size * 0.01 {
        let grid = fit::grid(region_mesh, 12).map_err(mesh_err)?;
        return Shape::bspline_face(&grid).map_err(geo);
    }
    face_for(primitive.primitive, points, extend)
}

pub(crate) fn surface_from_mesh(
    fid: FeatureId,
    p: &SurfaceFromMesh,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let mesh = &prev.mesh(&p.body)?.mesh;
    let (sub, points, normals) = region(mesh, &p.facets, p.group)?;
    let face = fit_face(&sub, &points, &normals, p.fit, p.extend.max(0.0))?;
    let names = Naming::from_faces(&face, vec![vec![format!("{fid}.fit")]]);
    add_surfaces(fid, state, Body { shape: face, names })
}

pub(crate) fn mesh_to_solid(
    fid: FeatureId,
    p: &MeshToSolid,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let mesh = &prev.mesh(&p.body)?.mesh;
    if !mesh.is_closed() {
        return Err("the mesh must be closed; fill its holes first".into());
    }
    let groups = of_mesh::segment::segment(mesh, p.angle, p.min_area);
    let count = groups.iter().copied().max().map_or(0, |g| g + 1);
    let size = mesh.bounds().diagonal();
    let mut faces = Vec::new();
    for g in 0..count {
        let tris: Vec<u32> = (0..mesh.triangles.len() as u32)
            .filter(|t| groups[*t as usize] == g)
            .collect();
        let (sub, points, normals) = region(mesh, &tris, None)?;
        // Reach well past each region so neighbours cross.
        faces.push(fit_face(
            &sub,
            &points,
            &normals,
            FitKind::Auto,
            size * 0.25,
        )?);
    }
    let cells = Shape::compound(&faces).cells().map_err(geo)?;
    let bvh = of_mesh::bvh::Bvh::new(mesh);
    let inside: Vec<Shape> = cells
        .into_iter()
        .filter(|c| c.inner_point().is_ok_and(|q| bvh.contains(q)))
        .collect();
    if inside.is_empty() {
        return Err("the fitted surfaces don't enclose the mesh's volume".into());
    }
    let mut solid = inside[0].clone();
    for c in &inside[1..] {
        solid = solid.boolean(BoolOp::Fuse, c).map_err(geo)?.shape;
    }
    let faces = (0..solid.count(Kind::Face))
        .map(|f| vec![format!("{fid}.f{f}")])
        .collect();
    let names = Naming::from_faces(&solid, faces);
    state.bodies.push(SolidBody {
        id: BodyId(format!("b{fid}:solid")),
        geometry: Body {
            shape: solid,
            names,
        },
    });
    state.aggregate_bodies()
}

/// Where a mesh crosses a plane, as chains of points.
fn section(mesh: &TriMesh, plane: &Plane) -> Vec<Vec<DVec3>> {
    let side = |p: DVec3| plane.distance(p);
    let mut segments: Vec<(DVec3, DVec3)> = Vec::new();
    for t in 0..mesh.triangles.len() {
        let c = mesh.corners(t);
        let d = c.map(side);
        let mut pts = Vec::new();
        for k in 0..3 {
            let (a, b) = (k, (k + 1) % 3);
            if (d[a] < 0.0) != (d[b] < 0.0) {
                let s = d[a] / (d[a] - d[b]);
                pts.push(c[a] + (c[b] - c[a]) * s);
            }
        }
        if pts.len() == 2 && (pts[0] - pts[1]).length() > 1e-12 {
            segments.push((pts[0], pts[1]));
        }
    }
    // Chain segments end to end.
    let tol = mesh.bounds().diagonal() * 1e-9;
    let key = |p: DVec3| {
        (
            (p.x / tol.max(1e-12)).round() as i64,
            (p.y / tol.max(1e-12)).round() as i64,
            (p.z / tol.max(1e-12)).round() as i64,
        )
    };
    let mut at: BTreeMap<(i64, i64, i64), Vec<usize>> = BTreeMap::new();
    for (i, (a, b)) in segments.iter().enumerate() {
        at.entry(key(*a)).or_default().push(i);
        at.entry(key(*b)).or_default().push(i);
    }
    let mut used = vec![false; segments.len()];
    let mut chains = Vec::new();
    for s in 0..segments.len() {
        if used[s] {
            continue;
        }
        used[s] = true;
        let mut chain = vec![segments[s].0, segments[s].1];
        loop {
            let end = *chain.last().unwrap();
            let next = at
                .get(&key(end))
                .and_then(|l| l.iter().copied().find(|i| !used[*i]));
            let Some(n) = next else { break };
            used[n] = true;
            let (a, b) = segments[n];
            chain.push(if key(a) == key(end) { b } else { a });
        }
        chains.push(chain);
    }
    chains
}

pub(crate) fn mesh_sections(
    fid: FeatureId,
    p: &MeshSections,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let mesh = &prev.mesh(&p.body)?.mesh;
    let plane = resolve(prev, &p.plane)?.plane()?;
    let mut segments = BTreeMap::new();
    let mut all = Vec::new();
    for i in 0..p.count {
        let at = plane.offset(p.offset + p.spacing * i as f64);
        for (k, chain) in section(mesh, &at).into_iter().enumerate() {
            let lines: Vec<Curve3> = chain
                .windows(2)
                .filter(|w| (w[1] - w[0]).length() > 1e-9)
                .map(|w| Curve3::Line { a: w[0], b: w[1] })
                .collect();
            if lines.is_empty() {
                continue;
            }
            let wire = Shape::wire3(&lines).map_err(geo)?;
            segments.insert(format!("slice{}#{}", i + 1, k + 1), wire.clone());
            all.push(wire);
        }
    }
    if all.is_empty() {
        return Err("no section plane crosses the mesh".into());
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
