//! Preparing scan meshes: orientation, removing noise and stray pieces,
//! smoothing, filling holes and smoothing boundaries.

use glam::{DMat3, DVec3};

use crate::{TriMesh, fit};

/// The rotation that lines the mesh's principal directions up with x, y and
/// z (largest spread along x), about its centroid, and the centroid.
pub fn principal_frame(mesh: &TriMesh) -> (DMat3, DVec3) {
    let c = mesh.positions.iter().copied().sum::<DVec3>() / mesh.positions.len().max(1) as f64;
    let mut m = nalgebra::Matrix3::zeros();
    for p in &mesh.positions {
        let d = *p - c;
        let d = nalgebra::Vector3::new(d.x, d.y, d.z);
        m += d * d.transpose();
    }
    let e = nalgebra::SymmetricEigen::new(m);
    let mut order: Vec<usize> = (0..3).collect();
    order.sort_by(|a, b| e.eigenvalues[*b].total_cmp(&e.eigenvalues[*a]));
    let axis = |i: usize| {
        let v = e.eigenvectors.column(order[i]);
        DVec3::new(v[0], v[1], v[2]).normalize()
    };
    let x = axis(0);
    let y = axis(1);
    let z = x.cross(y);
    // Rows are the new axes: p' = R (p - c).
    (DMat3::from_cols(x, y, z).transpose(), c)
}

/// Remove connected pieces with less than `fraction` of the largest
/// piece's area (stray scan data).
pub fn remove_small_pieces(mesh: &TriMesh, fraction: f64) -> TriMesh {
    let comps = mesh.components();
    let areas: Vec<f64> = comps
        .iter()
        .map(|c| c.iter().map(|t| mesh.triangle_area(*t as usize)).sum())
        .collect();
    let largest = areas.iter().copied().fold(0.0, f64::max);
    let keep: Vec<u32> = comps
        .iter()
        .zip(&areas)
        .filter(|(_, a)| **a >= largest * fraction)
        .flat_map(|(c, _)| c.iter().copied())
        .collect();
    mesh.subset(&keep)
}

/// Remove spikes: vertices further than `sigma` standard deviations from the
/// plane fitted to their neighbours are pulled onto it.
pub fn remove_noise(mesh: &mut TriMesh, sigma: f64) {
    let rings = vertex_rings(mesh);
    let mut moved = mesh.positions.clone();
    let mut dev = vec![0.0; mesh.positions.len()];
    let mut plane_of = vec![None; mesh.positions.len()];
    for (v, ring) in rings.iter().enumerate() {
        if ring.len() < 3 {
            continue;
        }
        let pts: Vec<DVec3> = ring.iter().map(|i| mesh.positions[*i as usize]).collect();
        if let Ok(f) = fit::plane(&pts)
            && let fit::Primitive::Plane { origin, normal } = f.primitive
        {
            dev[v] = (mesh.positions[v] - origin).dot(normal);
            plane_of[v] = Some((origin, normal));
        }
    }
    let rms = (dev.iter().map(|d| d * d).sum::<f64>() / dev.len().max(1) as f64).sqrt();
    for v in 0..mesh.positions.len() {
        if let Some((_, normal)) = plane_of[v]
            && dev[v].abs() > sigma * rms
        {
            moved[v] = mesh.positions[v] - normal * dev[v];
        }
    }
    mesh.positions = moved;
}

/// Each vertex's neighbours.
pub fn vertex_rings(mesh: &TriMesh) -> Vec<Vec<u32>> {
    let mut rings = vec![Vec::new(); mesh.positions.len()];
    for t in &mesh.triangles {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            if !rings[a as usize].contains(&b) {
                rings[a as usize].push(b);
            }
            if !rings[b as usize].contains(&a) {
                rings[b as usize].push(a);
            }
        }
    }
    rings
}

fn boundary_vertices(mesh: &TriMesh) -> Vec<bool> {
    let mut b = vec![false; mesh.positions.len()];
    for ((a, c), list) in mesh.edge_triangles() {
        if list.len() == 1 {
            b[a as usize] = true;
            b[c as usize] = true;
        }
    }
    b
}

/// Taubin smoothing, which smooths without shrinking. Boundary vertices stay
/// put unless `boundary` is set.
pub fn smooth(mesh: &mut TriMesh, iterations: usize, boundary: bool) {
    let rings = vertex_rings(mesh);
    let fixed = boundary_vertices(mesh);
    for _ in 0..iterations {
        for factor in [0.5, -0.53] {
            let next: Vec<DVec3> = (0..mesh.positions.len())
                .map(|v| {
                    if (fixed[v] && !boundary) || rings[v].is_empty() {
                        return mesh.positions[v];
                    }
                    let avg = rings[v]
                        .iter()
                        .map(|i| mesh.positions[*i as usize])
                        .sum::<DVec3>()
                        / rings[v].len() as f64;
                    mesh.positions[v] + (avg - mesh.positions[v]) * factor
                })
                .collect();
            mesh.positions = next;
        }
    }
}

/// Smooth only the open boundaries, along themselves.
pub fn smooth_boundaries(mesh: &mut TriMesh, iterations: usize) {
    for lp in mesh.boundary_loops() {
        let n = lp.len();
        for _ in 0..iterations {
            let next: Vec<DVec3> = (0..n)
                .map(|i| {
                    let (a, b, c) = (lp[(i + n - 1) % n], lp[i], lp[(i + 1) % n]);
                    let mid = (mesh.positions[a as usize] + mesh.positions[c as usize]) * 0.5;
                    mesh.positions[b as usize] * 0.5 + mid * 0.5
                })
                .collect();
            for (i, p) in next.into_iter().enumerate() {
                mesh.positions[lp[i] as usize] = p;
            }
        }
    }
}

/// Fill holes with at most `max_edges` edges around them: a new vertex at
/// each hole's centre, fanned to its rim. Returns the number filled.
pub fn fill_holes(mesh: &mut TriMesh, max_edges: usize) -> usize {
    let loops = mesh.boundary_loops();
    // The longest loop is the outer boundary of an open scan: leave it.
    let outer = loops
        .iter()
        .enumerate()
        .max_by(|a, b| {
            let len = |l: &Vec<u32>| {
                (0..l.len())
                    .map(|i| {
                        (mesh.positions[l[i] as usize]
                            - mesh.positions[l[(i + 1) % l.len()] as usize])
                            .length()
                    })
                    .sum::<f64>()
            };
            len(a.1).total_cmp(&len(b.1))
        })
        .map(|(i, _)| i);
    let closed_scan = loops.len() == 1 && !mesh.positions.is_empty();
    let mut filled = 0;
    for (i, lp) in loops.iter().enumerate() {
        if lp.len() > max_edges || (Some(i) == outer && !closed_scan && loops.len() > 1) {
            continue;
        }
        let center = lp
            .iter()
            .map(|v| mesh.positions[*v as usize])
            .sum::<DVec3>()
            / lp.len() as f64;
        mesh.positions.push(center);
        if let Some(c) = &mut mesh.colors {
            c.push(c[lp[0] as usize]);
        }
        let c = mesh.positions.len() as u32 - 1;
        let group = mesh.groups.first().copied();
        for k in 0..lp.len() {
            // Boundary loops run with their triangles; the fill runs back.
            mesh.triangles.push([lp[(k + 1) % lp.len()], lp[k], c]);
            if let Some(g) = group {
                mesh.groups.push(g);
            }
        }
        filled += 1;
    }
    filled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_holed_cube_is_filled() {
        let mut c = crate::tests::cube();
        c.triangles.truncate(10);
        assert!(!c.is_closed());
        assert_eq!(fill_holes(&mut c, 8), 1);
        assert!(c.is_closed());
        assert!((c.volume() - 1.0).abs() < 1e-12, "{}", c.volume());
    }

    #[test]
    fn small_pieces_go() {
        let mut m = crate::tests::cube();
        let mut tiny = crate::tests::cube();
        tiny.scale(0.01);
        m.append(&tiny);
        let kept = remove_small_pieces(&m, 0.1);
        assert_eq!(kept.triangles.len(), 12);
    }

    #[test]
    fn smoothing_keeps_a_flat_sheet_flat() {
        let mut m = crate::flatten::cylinder_patch(1e7, 1e-6, 10.0, 10, 10);
        for p in &mut m.positions {
            *p = DVec3::new(p.y, p.z, 0.0);
        }
        // A spike in the middle.
        m.positions[60].z = 3.0;
        remove_noise(&mut m, 2.0);
        assert!(m.positions[60].z.abs() < 1e-6);
        smooth(&mut m, 5, false);
        assert!(m.positions.iter().all(|p| p.z.abs() < 1e-6));
    }
}
