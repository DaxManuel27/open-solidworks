//! Surface Flatten: faces tessellated, cut open along tear edges, laid flat
//! with as little stretch as possible, and outlined as a planar surface.

use std::sync::Arc;

use of_geom::{Curve2, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Shape};

use super::geo;
use super::surface::add_surfaces;
use crate::FeatureId;
use crate::build::{Body, State};
use crate::meshes::{Flattening, deflection, tessellate};
use crate::naming::Naming;
use crate::resolve::{RResult, resolve};
use crate::surfaces::SurfaceFlatten;

pub(crate) fn surface_flatten(
    fid: FeatureId,
    p: &SurfaceFlatten,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let whole = prev.everything().ok_or("there's no body")?;
    let faces = p
        .faces
        .iter()
        .map(|r| match prev.any_index_of(r)? {
            (Kind::Face, i) => whole.shape.sub_shape(Kind::Face, i).map_err(geo),
            _ => Err("select faces".into()),
        })
        .collect::<RResult<Vec<_>>>()?;
    let region = Shape::compound(&faces);
    let size = region.bbox().map_or(1.0, |b| b.diagonal());
    let accuracy = p.accuracy.clamp(0.0, 1.0);
    let mut mesh = tessellate(&region, deflection(&region, 0.01 - 0.008 * accuracy))?;
    // Tear edges, sampled: vertices on them are kept apart per face.
    let mut tear_points: Vec<Vec<DVec3>> = Vec::new();
    for r in &p.tears {
        let (Kind::Edge, i) = prev.any_index_of(r)? else {
            return Err("tears are edges".into());
        };
        tear_points.push(whole.shape.edge_samples(i, size * 1e-3, 0.1).map_err(geo)?);
    }
    let tol = size * 1e-5;
    // The surface's open boundary: tears open where their ends reach it.
    let rim = region.free_edges().map_err(geo)?;
    let rim_points: Vec<DVec3> = (0..rim.count(Kind::Edge))
        .filter_map(|i| rim.edge_samples(i, size * 1e-3, 0.1).ok())
        .flatten()
        .collect();
    let on_rim = |q: DVec3| rim_points.iter().any(|r| (*r - q).length() < size * 2e-3);
    let near_tear = |q: DVec3| {
        tear_points.iter().any(|pts| {
            let (first, last) = (pts[0], *pts.last().unwrap());
            // An end of the tear inside the surface stays joined.
            let at_end = (q - first).length() < tol || (q - last).length() < tol;
            if at_end && !on_rim(q) {
                return false;
            }
            pts.windows(2).any(|w| {
                let d = w[1] - w[0];
                let t = ((q - w[0]).dot(d) / d.length_squared().max(1e-300)).clamp(0.0, 1.0);
                (w[0] + d * t - q).length() < size * 2e-4
            })
        })
    };
    let mut tags = vec![0u32; mesh.positions.len()];
    for (t, tri) in mesh.triangles.iter().enumerate() {
        for v in tri {
            if near_tear(mesh.positions[*v as usize]) {
                tags[*v as usize] = mesh.groups[t] + 1;
            }
        }
    }
    mesh.weld_tagged(size * 1e-6, &tags);
    let normals = mesh.vertex_normals();
    let fixed = match &p.fixed {
        Some(r) => resolve(prev, r)?.point()?,
        None => region.bbox().map_or(DVec3::ZERO, |b| b.center()),
    };
    let start = (0..mesh.positions.len())
        .min_by(|a, b| {
            (mesh.positions[*a] - fixed)
                .length()
                .total_cmp(&(mesh.positions[*b] - fixed).length())
        })
        .ok_or("there's nothing to flatten")? as u32;
    let iterations = 10 + (40.0 * accuracy) as usize;
    let flat = of_mesh::flatten::flatten(&mesh, start, iterations).map_err(|e| e.to_string())?;
    // The flat lies in the plane tangent to the faces at the fixed point,
    // its x along the first refined direction.
    let origin = mesh.positions[start as usize];
    let normal = normals[start as usize].normalize_or(DVec3::Z);
    let far = (0..mesh.positions.len())
        .max_by(|a, b| flat.uv[*a].length().total_cmp(&flat.uv[*b].length()))
        .unwrap();
    let uv_far = flat.uv[far];
    let toward = mesh.positions[far] - origin;
    let x3 = (toward - normal * toward.dot(normal)).normalize_or(normal.any_orthonormal_vector());
    // Turn the flat so `far` lies along x3, like it does in the model.
    let turn = -uv_far.y.atan2(uv_far.x);
    let (c, s) = (turn.cos(), turn.sin());
    let uv: Vec<DVec2> = flat
        .uv
        .iter()
        .map(|q| DVec2::new(q.x * c - q.y * s, q.x * s + q.y * c))
        .collect();
    let plane = Plane::new(origin, x3, normal);
    // Outline: the flat mesh's boundary loops, outer first.
    let mut loops: Vec<Vec<DVec2>> = mesh
        .boundary_loops()
        .into_iter()
        .map(|l| l.iter().map(|v| uv[*v as usize]).collect())
        .collect();
    if loops.is_empty() {
        return Err("the faces have no open boundary to flatten from".into());
    }
    let area = |l: &Vec<DVec2>| {
        (0..l.len())
            .map(|i| l[i].perp_dot(l[(i + 1) % l.len()]))
            .sum::<f64>()
            / 2.0
    };
    loops.sort_by(|a, b| area(b).abs().total_cmp(&area(a).abs()));
    let lines = |l: &Vec<DVec2>| -> Vec<Curve2> {
        (0..l.len())
            .map(|i| Curve2::Line {
                a: l[i],
                b: l[(i + 1) % l.len()],
            })
            .filter(|c| matches!(c, Curve2::Line { a, b } if (*b - *a).length() > 1e-9))
            .collect()
    };
    let face = Profile {
        plane,
        faces: vec![ProfileFace {
            outer: lines(&loops[0]),
            holes: loops[1..].iter().map(lines).collect(),
        }],
    }
    .to_shape()
    .map_err(|e| format!("the flat outline overlaps itself: {e}"))?;
    let names = Naming::from_faces(
        &face,
        vec![vec![format!("{fid}.flat")]; face.count(Kind::Face) as usize],
    );
    add_surfaces(fid, state, Body { shape: face, names })?;
    let mut placed = mesh.clone();
    for (i, q) in uv.iter().enumerate() {
        placed.positions[i] = plane.to_world(*q);
    }
    state.flattenings.insert(
        fid,
        Arc::new(Flattening {
            mesh: placed,
            strain: flat.strain,
        }),
    );
    Ok(())
}
