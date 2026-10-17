//! Mould tools: parting line, shut-off and parting surfaces, tooling split
//! and side cores.

use std::collections::{BTreeMap, BTreeSet};

use of_geom::{BoolOp, Continuity, DVec3, FillEdge, Kind, Plane, Profile, Ruled, Shape};

use super::geo;
use crate::build::{Body, State};
use crate::mold::{
    MoldCore, PartingLine, PartingRecord, PartingSurface, PartingSurfaceMode, ShutOffPatch,
    ShutOffSurfaces, ToolingSplit,
};
use crate::naming::{Inputs, Naming};
use crate::resolve::{RResult, resolve};
use crate::{BodyId, FeatureId, SolidBody};

/// Area-weighted mean of each face's normal along `pull`.
fn face_facing(shape: &Shape, pull: DVec3) -> RResult<Vec<f64>> {
    let mesh = crate::meshes::tessellate(shape, crate::meshes::deflection(shape, 0.002))?;
    let n = shape.count(Kind::Face) as usize;
    let mut sum = vec![0.0; n];
    let mut area = vec![0.0; n];
    for t in 0..mesh.triangles.len() {
        let a = mesh.triangle_area(t);
        let f = mesh.groups[t] as usize;
        sum[f] += mesh.normal(t).dot(pull) * a;
        area[f] += a;
    }
    Ok((0..n)
        .map(|f| if area[f] > 0.0 { sum[f] / area[f] } else { 0.0 })
        .collect())
}

/// Connected chains of edges (sharing vertices), each as edge indices.
fn edge_loops(shape: &Shape, edges: &[u32]) -> RResult<Vec<Vec<u32>>> {
    let ends: Vec<(DVec3, DVec3)> = edges
        .iter()
        .map(|e| shape.edge_info(*e).map(|i| (i.start, i.end)).map_err(geo))
        .collect::<RResult<_>>()?;
    let size = shape.bbox().map_or(1.0, |b| b.diagonal());
    let tol = size * 1e-7;
    let mut group = (0..edges.len()).collect::<Vec<usize>>();
    fn find(g: &mut [usize], mut i: usize) -> usize {
        while g[i] != i {
            g[i] = g[g[i]];
            i = g[i];
        }
        i
    }
    for i in 0..edges.len() {
        for j in i + 1..edges.len() {
            let (a, b) = (ends[i], ends[j]);
            let touch = [a.0, a.1]
                .iter()
                .any(|p| (*p - b.0).length() < tol || (*p - b.1).length() < tol);
            if touch {
                let (ri, rj) = (find(&mut group, i), find(&mut group, j));
                group[ri] = rj;
            }
        }
    }
    let mut out: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
    for (i, e) in edges.iter().enumerate() {
        let r = find(&mut group, i);
        out.entry(r).or_default().push(*e);
    }
    let mut loops: Vec<Vec<u32>> = out.into_values().collect();
    let extent = |l: &Vec<u32>| {
        Shape::compound(
            l.iter()
                .filter_map(|e| shape.sub_shape(Kind::Edge, *e).ok())
                .collect::<Vec<_>>()
                .iter(),
        )
        .bbox()
        .map_or(0.0, |b| b.diagonal())
    };
    loops.sort_by(|a, b| extent(b).total_cmp(&extent(a)));
    Ok(loops)
}

pub(crate) fn parting_line(
    fid: FeatureId,
    p: &PartingLine,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let index = match &p.body {
        Some(id) => prev
            .bodies
            .iter()
            .position(|b| &b.id == id)
            .ok_or_else(|| prev.solid(id).err().unwrap_or_default())?,
        None => {
            if prev.bodies.len() != 1 {
                return Err("choose the solid to mould".into());
            }
            0
        }
    };
    let mut pull = resolve(prev, &p.pull)?.direction()?.normalize();
    if p.reverse {
        pull = -pull;
    }
    let mut body = prev.bodies[index].geometry.clone();
    let tilt = p.draft_angle.max(0.0).sin();
    // Split faces that face both ways at their silhouettes.
    if p.split_faces {
        let facing = crate::analysis::draft(
            &State {
                body: Some(body.clone()),
                ..Default::default()
            },
            pull,
            0.0,
        )?;
        let straddle: Vec<u32> = facing
            .iter()
            .filter(|f| f.min < -1e-6 && f.max > 1e-6)
            .map(|f| f.face)
            .collect();
        let mut cuts = Vec::new();
        for f in straddle {
            if let Ok(w) = super::silhouette(&body.shape, f, pull) {
                cuts.push(w);
            }
        }
        if !cuts.is_empty() {
            let r = body
                .shape
                .split_tools(&Shape::compound(&cuts))
                .map_err(geo)?;
            let names = Naming::from_history(
                &r.shape,
                &r.history,
                Inputs {
                    named: [Some(&body.names), None],
                },
                &mut |_| Some(format!("{fid}.part")),
                &fid.to_string(),
            );
            body = Body {
                shape: r.shape,
                names,
            };
        }
    }
    let facing = face_facing(&body.shape, pull)?;
    let flat: Vec<u32> = (0..facing.len() as u32)
        .filter(|f| facing[*f as usize].abs() < tilt.max(1e-6) && facing[*f as usize].abs() < 1e-3)
        .collect();
    if !flat.is_empty() && p.draft_angle > 0.0 {
        return Err(format!(
            "{} face{} need draft before the part can be split",
            flat.len(),
            if flat.len() == 1 { "" } else { "s" }
        ));
    }
    // Upright faces go with the core when no draft is asked for.
    let cavity = |f: u32| facing[f as usize] > 1e-3;
    let edge_faces = body.shape.edge_faces();
    let split: Vec<u32> = edge_faces
        .iter()
        .enumerate()
        .filter(|(_, fs)| fs.len() == 2 && cavity(fs[0]) != cavity(fs[1]))
        .map(|(e, _)| e as u32)
        .collect();
    if split.is_empty() {
        return Err("no edges divide the faces facing each way; check the pull direction".into());
    }
    let loops = edge_loops(&body.shape, &split)?;
    let name = |e: u32| body.names.edges[e as usize].clone();
    let face_names = |want: bool| -> Vec<String> {
        (0..facing.len() as u32)
            .filter(|f| cavity(*f) == want)
            .filter_map(|f| body.names.face_name(f).map(str::to_owned))
            .collect()
    };
    let record = PartingRecord {
        feature: fid,
        body: prev.bodies[index].id.clone(),
        pull,
        edges: loops[0].iter().map(|e| name(*e)).collect(),
        openings: loops[1..]
            .iter()
            .map(|l| l.iter().map(|e| name(*e)).collect())
            .collect(),
        cavity_faces: face_names(true),
        core_faces: face_names(false),
    };
    state.bodies[index].geometry = body;
    state.mold.parting_lines.push(record);
    state.aggregate_bodies()
}

/// Edge shapes of the part by stable name, with the cavity-side face beside
/// each.
fn named_edges(
    state: &State,
    record: &PartingRecord,
    names: &[String],
) -> RResult<Vec<(Shape, Option<Shape>)>> {
    let body = &state.solid(&record.body)?.geometry;
    let cavity: BTreeSet<&String> = record.cavity_faces.iter().collect();
    let edge_faces = body.shape.edge_faces();
    names
        .iter()
        .map(|n| {
            let e = body
                .names
                .edge(n)
                .ok_or_else(|| format!("parting edge {n} no longer exists"))?;
            let edge = body.shape.sub_shape(Kind::Edge, e).map_err(geo)?;
            let face = edge_faces[e as usize]
                .iter()
                .find(|f| {
                    body.names
                        .face_name(**f)
                        .is_some_and(|n| cavity.contains(&n.to_string()))
                })
                .map(|f| body.shape.sub_shape(Kind::Face, *f))
                .transpose()
                .map_err(geo)?;
            Ok((edge, face))
        })
        .collect()
}

fn surface_body(fid: FeatureId, shape: Shape, tag: &str, n: usize) -> SolidBody {
    let faces = (0..shape.count(Kind::Face))
        .map(|i| vec![format!("{fid}.{tag}{i}")])
        .collect();
    SolidBody {
        id: BodyId(format!("b{fid}:{tag}{n}")),
        geometry: Body {
            names: Naming::from_faces(&shape, faces),
            shape,
        },
    }
}

pub(crate) fn shut_off(
    fid: FeatureId,
    p: &ShutOffSurfaces,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let record = prev.mold.record(p.parting_line)?.clone();
    if record.openings.is_empty() {
        return Err("the part has no through openings to close".into());
    }
    let mut made = 0;
    for (i, opening) in record.openings.iter().enumerate() {
        let patch = p.patches.get(i).copied().unwrap_or(p.patch);
        if patch == ShutOffPatch::NoFill {
            continue;
        }
        let edges = named_edges(prev, &record, opening)?;
        let boundary: Vec<FillEdge> = edges
            .iter()
            .map(|(e, f)| FillEdge {
                edge: e,
                support: f.as_ref(),
                continuity: if patch == ShutOffPatch::Tangent && f.is_some() {
                    Continuity::Tangent
                } else {
                    Continuity::Contact
                },
            })
            .collect();
        let (face, _) = Shape::fill(&boundary, &[]).map_err(geo)?;
        let body = surface_body(fid, face, "shutoff", i);
        state.mold.shut_off.push(body.id.clone());
        state.surfaces.push(body);
        made += 1;
    }
    if made == 0 {
        return Err("every opening is left open".into());
    }
    Ok(())
}

pub(crate) fn parting_surface(
    fid: FeatureId,
    p: &PartingSurface,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let record = prev.mold.record(p.parting_line)?.clone();
    let body = &prev.solid(&record.body)?.geometry;
    let cavity: BTreeSet<&String> = record.cavity_faces.iter().collect();
    let edge_faces = body.shape.edge_faces();
    let mut strips = Vec::new();
    for n in &record.edges {
        let e = body
            .names
            .edge(n)
            .ok_or_else(|| format!("parting edge {n} no longer exists"))?;
        // The core-side face: strips leave the part away from it.
        let face = edge_faces[e as usize].iter().copied().find(|f| {
            body.names
                .face_name(*f)
                .is_none_or(|n| !cavity.contains(&n.to_string()))
        });
        let ruled = match p.mode {
            PartingSurfaceMode::PerpendicularToPull => Ruled::Perpendicular {
                vector: record.pull,
            },
            PartingSurfaceMode::TangentToSurface => Ruled::Tangent,
            PartingSurfaceMode::NormalToSurface => Ruled::Normal,
        };
        strips.push(
            body.shape
                .ruled_strip(e, face, ruled, p.distance, p.reverse)
                .map_err(geo)?,
        );
    }
    let corners = corner_patches(&body.shape, &record, &strips)?;
    strips.extend(corners);
    let knitted = Shape::compound(&strips)
        .sew(1e-4)
        .map_err(geo)?
        .result
        .shape;
    let body = surface_body(fid, knitted, "parting", 0);
    state.mold.parting_surfaces.push(body.id.clone());
    state.surfaces.push(body);
    Ok(())
}

/// Patches filling the gaps between neighbouring strips at the parting
/// line's corners: each a four-sided patch from the corner out along both
/// strips' sides.
fn corner_patches(part: &Shape, record: &PartingRecord, strips: &[Shape]) -> RResult<Vec<Shape>> {
    let size = part.bbox().map_or(1.0, |b| b.diagonal());
    let tol = size * 1e-6;
    let _ = record;
    // Each strip's side edges: edges with exactly one end on the part.
    let mut sides: Vec<(DVec3, DVec3, Shape)> = Vec::new();
    for strip in strips {
        for e in 0..strip.count(Kind::Edge) {
            let info = strip.edge_info(e).map_err(geo)?;
            let on = |p: DVec3| {
                Shape::vertex_at(p)
                    .and_then(|v| v.distance(part))
                    .is_ok_and(|d| d.value < tol)
            };
            let (a, b) = (on(info.start), on(info.end));
            if a != b {
                let (base, far) = if a {
                    (info.start, info.end)
                } else {
                    (info.end, info.start)
                };
                sides.push((base, far, strip.sub_shape(Kind::Edge, e).map_err(geo)?));
            }
        }
    }
    let mut out = Vec::new();
    let mut used = vec![false; sides.len()];
    for i in 0..sides.len() {
        if used[i] {
            continue;
        }
        let Some(j) =
            (i + 1..sides.len()).find(|j| !used[*j] && (sides[*j].0 - sides[i].0).length() < tol)
        else {
            continue;
        };
        used[i] = true;
        used[j] = true;
        let (v, f1, f2) = (sides[i].0, sides[i].1, sides[j].1);
        if (f1 - f2).length() < tol {
            continue;
        }
        let corner = f1 + (f2 - v);
        let line = |a: DVec3, b: DVec3| -> RResult<Shape> {
            let w = Shape::wire3(&[of_geom::Curve3::Line { a, b }]).map_err(geo)?;
            w.sub_shape(Kind::Edge, 0).map_err(geo)
        };
        let (l1, l2) = (line(f1, corner)?, line(corner, f2)?);
        let boundary = [&sides[i].2, &l1, &l2, &sides[j].2].map(|edge| FillEdge {
            edge,
            support: None,
            continuity: Continuity::Contact,
        });
        out.push(Shape::fill(&boundary, &[]).map_err(geo)?.0);
    }
    Ok(out)
}

fn block_profile(prev: &State, sketch: FeatureId) -> RResult<(Plane, Shape)> {
    let (plane, sk) = super::sketch_of(prev, sketch)?;
    let face = Profile {
        plane: *plane,
        faces: super::closed_regions(&sk.contours().map_err(geo)?)?,
    }
    .to_shape()
    .map_err(geo)?;
    Ok((*plane, face))
}

pub(crate) fn tooling_split(
    fid: FeatureId,
    p: &ToolingSplit,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let record = prev.mold.record(p.parting_line)?.clone();
    let part = prev.solid(&record.body)?.geometry.clone();
    let (plane, outline) = block_profile(prev, p.sketch)?;
    let pull = record.pull;
    if plane.normal.cross(pull).length() > 1e-6 {
        return Err("sketch the block on a plane square to the pull direction".into());
    }
    let along = if plane.normal.dot(pull) > 0.0 {
        plane.normal
    } else {
        -plane.normal
    };
    let start = outline
        .transform(&of_geom::DAffine3::from_translation(-along * p.depth2))
        .map_err(geo)?
        .shape;
    let block = start
        .prism(along * (p.depth1 + p.depth2))
        .map_err(geo)?
        .shape;
    let mould = block.boolean(BoolOp::Cut, &part.shape).map_err(geo)?.shape;
    let tools: Vec<Shape> = prev
        .mold
        .parting_surfaces
        .iter()
        .chain(&prev.mold.shut_off)
        .filter_map(|id| prev.surface(id).ok().map(|b| b.geometry.shape.clone()))
        .collect();
    if tools.is_empty() {
        return Err("make a parting surface first".into());
    }
    let pieces = mould
        .split_tools(&Shape::compound(&tools))
        .map_err(geo)?
        .shape
        .solids();
    if pieces.len() < 2 {
        return Err(
            "the parting surfaces don't divide the block; extend them past its sides".into(),
        );
    }
    let named = |names: &[String]| -> Shape {
        Shape::compound(
            names
                .iter()
                .filter_map(|n| part.names.face(n))
                .filter_map(|f| part.shape.sub_shape(Kind::Face, f).ok())
                .collect::<Vec<_>>()
                .iter(),
        )
    };
    let cavity_faces = named(&record.cavity_faces);
    let core_faces = named(&record.core_faces);
    let mut cavity = Vec::new();
    let mut core = Vec::new();
    for piece in pieces {
        let near = |faces: &Shape| {
            faces.count(Kind::Face) > 0 && piece.distance(faces).is_ok_and(|d| d.value < 1e-6)
        };
        let (c, k) = (near(&cavity_faces), near(&core_faces));
        let above = piece
            .mass_props()
            .map(|m| (m.center - plane.origin).dot(pull) > 0.0)
            .unwrap_or(false);
        if (c && !k) || (c == k && above) {
            cavity.push(piece);
        } else {
            core.push(piece);
        }
    }
    let fuse = |list: Vec<Shape>| -> RResult<Shape> {
        let mut it = list.into_iter();
        let mut acc = it.next().ok_or("a mould half is empty")?;
        for s in it {
            acc = acc.boolean(BoolOp::Fuse, &s).map_err(geo)?.shape;
        }
        Ok(acc)
    };
    let mut cavity = fuse(cavity)?;
    let mut core = fuse(core)?;
    if let Some(lip) = &p.interlock {
        // A tapered lip on the core around the block's edge, above the
        // parting surface, fitted into the cavity.
        let parting: Vec<Shape> = prev
            .mold
            .parting_surfaces
            .iter()
            .filter_map(|id| prev.surface(id).ok().map(|b| b.geometry.shape.clone()))
            .collect();
        let height = Shape::compound(&parting)
            .bbox()
            .map(|b| (b.center() - plane.origin).dot(along))
            .ok_or("the parting surface is empty")?;
        let level = outline
            .transform(&of_geom::DAffine3::from_translation(along * height))
            .map_err(geo)?
            .shape;
        let ring_outer = level.prism(along * lip.height).map_err(geo)?.shape;
        let inner_low = level.outline_offset(-lip.width).map_err(geo)?;
        let shrink = lip.height * lip.angle.tan();
        let inner_high = level
            .transform(&of_geom::DAffine3::from_translation(along * lip.height))
            .map_err(geo)?
            .shape
            .outline_offset(-(lip.width + shrink))
            .map_err(geo)?;
        // The faces carry their outline wires, which the loft runs through.
        let taper = Shape::loft(&[inner_low, inner_high], true)
            .map_err(geo)?
            .shape;
        let ring = ring_outer.boolean(BoolOp::Cut, &taper).map_err(geo)?.shape;
        core = core.boolean(BoolOp::Fuse, &ring).map_err(geo)?.shape;
        cavity = cavity.boolean(BoolOp::Cut, &ring).map_err(geo)?.shape;
    }
    for (tag, shape) in [("cavity", cavity), ("core", core)] {
        let faces = (0..shape.count(Kind::Face))
            .map(|i| vec![format!("{fid}.{tag}{i}")])
            .collect();
        let id = BodyId(format!("b{fid}:{tag}"));
        state.bodies.push(SolidBody {
            id: id.clone(),
            geometry: Body {
                names: Naming::from_faces(&shape, faces),
                shape,
            },
        });
        if tag == "core" {
            state.mold.cores.push(id);
        } else {
            state.mold.cavities.push(id);
        }
    }
    state.aggregate_bodies()
}

pub(crate) fn mold_core(
    fid: FeatureId,
    p: &MoldCore,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let target = prev.solid(&p.body)?.geometry.clone();
    let (plane, outline) = block_profile(prev, p.sketch)?;
    let dir = if p.reverse {
        -plane.normal
    } else {
        plane.normal
    };
    let start = outline
        .transform(&of_geom::DAffine3::from_translation(-dir * p.depth2))
        .map_err(geo)?
        .shape;
    let mut tool = start.prism(dir * (p.depth1 + p.depth2)).map_err(geo)?.shape;
    if p.draft.abs() > 1e-12 {
        // Taper the core's sides so it can pull out.
        let sides: Vec<u32> = (0..tool.count(Kind::Face))
            .filter(|f| {
                tool.face_info(*f)
                    .is_ok_and(|i| i.normal.dot(dir).abs() < 1e-9)
            })
            .collect();
        let neutral = Plane::new(start.face_info(0).map_err(geo)?.point, plane.x_dir, dir);
        tool = tool
            .draft(&sides, &neutral, dir, p.draft)
            .map_err(geo)?
            .shape;
    }
    let piece = target
        .shape
        .boolean(BoolOp::Common, &tool)
        .map_err(geo)?
        .shape;
    if piece.count(Kind::Solid) == 0 {
        return Err("the core's outline doesn't cross the block".into());
    }
    let rest = target.shape.boolean(BoolOp::Cut, &tool).map_err(geo)?;
    let names = Naming::from_history(
        &rest.shape,
        &rest.history,
        Inputs {
            named: [Some(&target.names), None],
        },
        &mut |_| Some(format!("{fid}.pocket")),
        &fid.to_string(),
    );
    let i = state.bodies.iter().position(|b| b.id == p.body).unwrap();
    state.bodies[i].geometry = Body {
        shape: rest.shape,
        names,
    };
    let faces = (0..piece.count(Kind::Face))
        .map(|f| vec![format!("{fid}.core{f}")])
        .collect();
    let id = BodyId(format!("b{fid}:core"));
    state.bodies.push(SolidBody {
        id: id.clone(),
        geometry: Body {
            names: Naming::from_faces(&piece, faces),
            shape: piece,
        },
    });
    state.mold.cores.push(id);
    state.aggregate_bodies()
}
