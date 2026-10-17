//! Flattening a curved surface mesh into the plane: a least-squares conformal
//! layout, then as-rigid-as-possible refinement so lengths change as little
//! as they can. Reports how much each triangle stretched.

use glam::{DVec2, DVec3};

use crate::solve::{Sparse, cg, least_squares};
use crate::{Result, TriMesh, invalid};

/// A flattened mesh: a 2D position per vertex and the stretch of each
/// triangle.
#[derive(Debug, Clone)]
pub struct Flat {
    pub uv: Vec<DVec2>,
    /// Flat area over 3D area, per triangle: above 1 stretched, below 1
    /// compressed.
    pub area_ratio: Vec<f64>,
    /// The largest change of length in any direction, per triangle, as a
    /// fraction (0 is exact).
    pub strain: Vec<f64>,
}

/// Each triangle's corners in its own plane: the first at the origin and the
/// second along +x.
fn local(mesh: &TriMesh, t: usize) -> [DVec2; 3] {
    let [a, b, c] = mesh.corners(t);
    let x = (b - a).normalize_or_zero();
    let n = (b - a).cross(c - a);
    let y = n.cross(x).normalize_or_zero();
    [
        DVec2::ZERO,
        DVec2::new((b - a).dot(x), 0.0),
        DVec2::new((c - a).dot(x), (c - a).dot(y)),
    ]
}

/// Flatten `mesh` (one connected piece, open, without overlaps once flat).
/// `fixed` stays at the origin; `iterations` rounds of refinement are run.
pub fn flatten(mesh: &TriMesh, fixed: u32, iterations: usize) -> Result<Flat> {
    let n = mesh.positions.len();
    if mesh.triangles.is_empty() || n < 3 {
        return Err(invalid("there's nothing to flatten"));
    }
    if fixed as usize >= n {
        return Err(invalid("the fixed vertex isn't on the surface"));
    }
    if mesh.components().len() != 1 {
        return Err(invalid("the faces must form one connected surface"));
    }
    if mesh.is_closed() {
        return Err(invalid(
            "a closed surface can't be flattened; add relief cuts",
        ));
    }
    let rest: Vec<[DVec2; 3]> = (0..mesh.triangles.len()).map(|t| local(mesh, t)).collect();
    // Second pin: the vertex furthest from the fixed one.
    let p0 = mesh.positions[fixed as usize];
    let (far, dist) = mesh
        .positions
        .iter()
        .enumerate()
        .map(|(i, p)| (i, (*p - p0).length()))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    if dist < 1e-12 {
        return Err(invalid("the surface has no extent"));
    }
    let mut uv = lscm(mesh, &rest, fixed as usize, far, dist)?;
    arap(mesh, &rest, fixed as usize, &mut uv, iterations);
    // Measure.
    let mut area_ratio = Vec::with_capacity(rest.len());
    let mut strain = Vec::with_capacity(rest.len());
    for (t, x) in rest.iter().enumerate() {
        let tri = mesh.triangles[t];
        let u = tri.map(|i| uv[i as usize]);
        let a3 = 0.5 * (x[1] - x[0]).perp_dot(x[2] - x[0]);
        let a2 = 0.5 * (u[1] - u[0]).perp_dot(u[2] - u[0]);
        area_ratio.push(if a3.abs() > 1e-300 { a2 / a3 } else { 1.0 });
        // The map's Jacobian from rest to flat, and its singular values.
        let e = nalgebra::Matrix2::new(
            x[1].x - x[0].x,
            x[2].x - x[0].x,
            x[1].y - x[0].y,
            x[2].y - x[0].y,
        );
        let f = nalgebra::Matrix2::new(
            u[1].x - u[0].x,
            u[2].x - u[0].x,
            u[1].y - u[0].y,
            u[2].y - u[0].y,
        );
        let s = match e.try_inverse() {
            Some(inv) => (f * inv).singular_values(),
            None => nalgebra::Vector2::new(1.0, 1.0),
        };
        strain.push((s[0] - 1.0).abs().max((s[1] - 1.0).abs()));
    }
    Ok(Flat {
        uv,
        area_ratio,
        strain,
    })
}

fn lscm(
    mesh: &TriMesh,
    rest: &[[DVec2; 3]],
    pin: usize,
    far: usize,
    dist: f64,
) -> Result<Vec<DVec2>> {
    let n = mesh.positions.len();
    // Unknowns: u then v of every vertex except the two pins.
    let pins = [(pin, DVec2::ZERO), (far, DVec2::new(dist, 0.0))];
    let mut column = vec![usize::MAX; n];
    let mut free = 0;
    for (i, c) in column.iter_mut().enumerate() {
        if i != pin && i != far {
            *c = free;
            free += 1;
        }
    }
    let rows = 2 * mesh.triangles.len();
    let mut a = Sparse::new(rows, 2 * free);
    let mut b = vec![0.0; rows];
    for (t, x) in rest.iter().enumerate() {
        let d = (x[1] - x[0]).perp_dot(x[2] - x[0]);
        if d.abs() < 1e-300 {
            continue;
        }
        let s = 1.0 / d.abs().sqrt();
        let tri = mesh.triangles[t];
        for j in 0..3 {
            let w = (x[(j + 2) % 3] - x[(j + 1) % 3]) * s;
            let (wr, wi) = (w.x, w.y);
            let v = tri[j] as usize;
            let (re, im) = (2 * t, 2 * t + 1);
            // Re(W U) = wr u - wi v; Im(W U) = wi u + wr v.
            if let Some((_, at)) = pins.iter().find(|(p, _)| *p == v) {
                b[re] -= wr * at.x - wi * at.y;
                b[im] -= wi * at.x + wr * at.y;
            } else {
                let cu = column[v];
                let cv = free + column[v];
                a.add(re, cu, wr);
                a.add(re, cv, -wi);
                a.add(im, cu, wi);
                a.add(im, cv, wr);
            }
        }
    }
    let a = a.to_csr();
    let mut x = vec![0.0; 2 * free];
    least_squares(&a, &b, &mut x, 1e-10, 20 * free.max(50));
    let mut uv = vec![DVec2::ZERO; n];
    for (i, p) in uv.iter_mut().enumerate() {
        *p = match pins.iter().find(|(v, _)| *v == i) {
            Some((_, at)) => *at,
            None => DVec2::new(x[column[i]], x[free + column[i]]),
        };
    }
    if uv.iter().any(|p| !p.is_finite()) {
        return Err(invalid("the surface can't be laid flat"));
    }
    Ok(uv)
}

fn cot(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    // The cotangent of the angle at `a` in triangle abc.
    let (u, v) = (b - a, c - a);
    let cross = u.perp_dot(v).abs();
    if cross < 1e-300 {
        0.0
    } else {
        u.dot(v) / cross
    }
}

fn arap(mesh: &TriMesh, rest: &[[DVec2; 3]], pin: usize, uv: &mut [DVec2], iterations: usize) {
    let n = mesh.positions.len();
    // Cotangent weights per triangle half-edge (opposite corner k).
    let weights: Vec<[f64; 3]> = rest
        .iter()
        .map(|x| {
            let mut w = [0.0; 3];
            for (k, wk) in w.iter_mut().enumerate() {
                *wk = (0.5 * cot(x[k], x[(k + 1) % 3], x[(k + 2) % 3])).max(1e-6);
            }
            w
        })
        .collect();
    let mut column = vec![usize::MAX; n];
    let mut free = 0;
    for (i, c) in column.iter_mut().enumerate() {
        if i != pin {
            *c = free;
            free += 1;
        }
    }
    let mut lap = Sparse::new(free, free);
    for (t, tri) in mesh.triangles.iter().enumerate() {
        for k in 0..3 {
            let (i, j) = (tri[(k + 1) % 3] as usize, tri[(k + 2) % 3] as usize);
            let w = weights[t][k];
            for (p, q) in [(i, j), (j, i)] {
                if p != pin {
                    lap.add(column[p], column[p], w);
                    if q != pin {
                        lap.add(column[p], column[q], -w);
                    }
                }
            }
        }
    }
    let lap = lap.to_csr();
    for _ in 0..iterations {
        // Local step: the rotation that best maps each rest triangle.
        let rotations: Vec<nalgebra::Matrix2<f64>> = mesh
            .triangles
            .iter()
            .enumerate()
            .map(|(t, tri)| {
                let x = &rest[t];
                let mut s = nalgebra::Matrix2::zeros();
                for (k, w) in weights[t].iter().enumerate() {
                    let (i, j) = ((k + 1) % 3, (k + 2) % 3);
                    let du = uv[tri[i] as usize] - uv[tri[j] as usize];
                    let dx = x[i] - x[j];
                    s += *w
                        * nalgebra::Matrix2::new(
                            du.x * dx.x,
                            du.x * dx.y,
                            du.y * dx.x,
                            du.y * dx.y,
                        );
                }
                let angle = (s[(1, 0)] - s[(0, 1)]).atan2(s[(0, 0)] + s[(1, 1)]);
                nalgebra::Matrix2::new(angle.cos(), -angle.sin(), angle.sin(), angle.cos())
            })
            .collect();
        // Global step: positions that best fit the rotated triangles.
        let mut bx = vec![0.0; free];
        let mut by = vec![0.0; free];
        for (t, tri) in mesh.triangles.iter().enumerate() {
            let x = &rest[t];
            for (k, &w) in weights[t].iter().enumerate() {
                let (a, b) = ((k + 1) % 3, (k + 2) % 3);
                let (i, j) = (tri[a] as usize, tri[b] as usize);
                let d = rotations[t] * nalgebra::Vector2::new(x[a].x - x[b].x, x[a].y - x[b].y);
                for (p, q, sign) in [(i, j, 1.0), (j, i, -1.0)] {
                    if p == pin {
                        continue;
                    }
                    bx[column[p]] += sign * w * d.x;
                    by[column[p]] += sign * w * d.y;
                    if q == pin {
                        bx[column[p]] += w * uv[pin].x;
                        by[column[p]] += w * uv[pin].y;
                    }
                }
            }
        }
        let mut x: Vec<f64> = (0..n).filter(|i| *i != pin).map(|i| uv[i].x).collect();
        let mut y: Vec<f64> = (0..n).filter(|i| *i != pin).map(|i| uv[i].y).collect();
        cg(&lap, &bx, &mut x, 1e-10, 4 * free.max(50));
        cg(&lap, &by, &mut y, 1e-10, 4 * free.max(50));
        for i in 0..n {
            if i != pin {
                uv[i] = DVec2::new(x[column[i]], y[column[i]]);
            }
        }
    }
}

/// A developable test surface: a cylinder patch `radius`, `angle` around and
/// `height` tall, as a grid of triangles.
pub fn cylinder_patch(radius: f64, angle: f64, height: f64, nu: usize, nv: usize) -> TriMesh {
    let mut p = Vec::new();
    for j in 0..=nv {
        for i in 0..=nu {
            let a = angle * i as f64 / nu as f64;
            p.push(DVec3::new(
                radius * a.cos(),
                radius * a.sin(),
                height * j as f64 / nv as f64,
            ));
        }
    }
    let mut t = Vec::new();
    let w = nu as u32 + 1;
    for j in 0..nv as u32 {
        for i in 0..nu as u32 {
            let a = j * w + i;
            t.push([a, a + 1, a + w + 1]);
            t.push([a, a + w + 1, a + w]);
        }
    }
    TriMesh::new(p, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cylinder_patch_unrolls_without_stretch() {
        let m = cylinder_patch(10.0, 1.5, 8.0, 24, 6);
        let f = flatten(&m, 0, 20).unwrap();
        let worst = f.strain.iter().copied().fold(0.0, f64::max);
        assert!(worst < 1e-3, "strain {worst}");
        let flat_area: f64 = (0..m.triangles.len())
            .map(|t| {
                let [a, b, c] = m.triangles[t].map(|i| f.uv[i as usize]);
                0.5 * (b - a).perp_dot(c - a)
            })
            .sum();
        assert!(
            (flat_area - 10.0 * 1.5 * 8.0 * 0.9988).abs() < 0.5,
            "{flat_area}"
        );
    }

    #[test]
    fn a_spherical_cap_stretches_a_little() {
        // A shallow dome: not developable, so the flat must stretch.
        let mut p = Vec::new();
        let n = 12;
        for j in 0..=n {
            for i in 0..=n {
                let x = -5.0 + 10.0 * i as f64 / n as f64;
                let y = -5.0 + 10.0 * j as f64 / n as f64;
                let z = (400.0 - x * x - y * y).sqrt() - 20.0;
                p.push(DVec3::new(x, y, z));
            }
        }
        let w = n as u32 + 1;
        let mut t = Vec::new();
        for j in 0..n as u32 {
            for i in 0..n as u32 {
                let a = j * w + i;
                t.push([a, a + 1, a + w + 1]);
                t.push([a, a + w + 1, a + w]);
            }
        }
        let m = TriMesh::new(p, t);
        let f = flatten(&m, 0, 30).unwrap();
        let worst = f.strain.iter().copied().fold(0.0, f64::max);
        assert!(worst > 1e-4 && worst < 0.05, "strain {worst}");
        assert!(f.area_ratio.iter().all(|r| *r > 0.9 && *r < 1.1));
    }
}
