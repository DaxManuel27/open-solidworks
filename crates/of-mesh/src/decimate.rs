//! Decimation by quadric error edge collapse (Garland and Heckbert), with
//! boundary preservation and an optional region.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

use glam::DVec3;
use nalgebra::{Matrix3, Matrix4, Vector3, Vector4};

use crate::TriMesh;

/// When to stop collapsing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// Keep this fraction of the triangles (0..1).
    Fraction(f64),
    /// Stop before moving the surface further than this distance.
    Tolerance(f64),
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub target: Target,
    /// Keep open boundaries where they are.
    pub preserve_boundary: bool,
}

#[derive(Copy, Clone, PartialEq)]
struct Candidate {
    cost: f64,
    a: u32,
    b: u32,
    version: (u32, u32),
    to: DVec3,
}

impl Eq for Candidate {}
impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.total_cmp(&self.cost)
    }
}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn plane_quadric(a: DVec3, b: DVec3, c: DVec3) -> Matrix4<f64> {
    let n = (b - a).cross(c - a);
    let area = n.length() * 0.5;
    let n = n.normalize_or_zero();
    let p = Vector4::new(n.x, n.y, n.z, -n.dot(a));
    p * p.transpose() * area.max(1e-12)
}

fn quadric_cost(q: &Matrix4<f64>, p: DVec3) -> f64 {
    let v = Vector4::new(p.x, p.y, p.z, 1.0);
    (v.transpose() * q * v)[0].max(0.0)
}

/// Decimate `mesh`. When `region` is given, only edges with both ends in it
/// collapse.
pub fn decimate(mesh: &TriMesh, options: Options, region: Option<&HashSet<u32>>) -> TriMesh {
    let mut pos = mesh.positions.clone();
    let mut tris: Vec<[u32; 3]> = mesh.triangles.clone();
    let mut alive = vec![true; tris.len()];
    let n = pos.len();
    let mut vtris: Vec<Vec<u32>> = vec![Vec::new(); n];
    for (t, tri) in tris.iter().enumerate() {
        for v in tri {
            vtris[*v as usize].push(t as u32);
        }
    }
    let mut q = vec![Matrix4::zeros(); n];
    for tri in &tris {
        let k = plane_quadric(
            pos[tri[0] as usize],
            pos[tri[1] as usize],
            pos[tri[2] as usize],
        );
        for v in tri {
            q[*v as usize] += k;
        }
    }
    // Boundary edges get a steep plane square to the surface.
    let edges = mesh.edge_triangles();
    let mut boundary = vec![false; n];
    for (&(a, b), list) in &edges {
        if list.len() == 1 {
            boundary[a as usize] = true;
            boundary[b as usize] = true;
            if options.preserve_boundary {
                let t = list[0] as usize;
                let normal = mesh.normal(t);
                let (pa, pb) = (pos[a as usize], pos[b as usize]);
                let side = (pb - pa).cross(normal).normalize_or_zero();
                let p = Vector4::new(side.x, side.y, side.z, -side.dot(pa));
                let k = p * p.transpose() * 1e6 * (pb - pa).length_squared().max(1e-12);
                q[a as usize] += k;
                q[b as usize] += k;
            }
        }
    }
    let mut version = vec![0u32; n];
    let mut removed = vec![false; n];
    let candidate = |a: u32, b: u32, q: &[Matrix4<f64>], pos: &[DVec3], version: &[u32]| {
        let qq = q[a as usize] + q[b as usize];
        let m = Matrix3::new(
            qq[(0, 0)],
            qq[(0, 1)],
            qq[(0, 2)],
            qq[(1, 0)],
            qq[(1, 1)],
            qq[(1, 2)],
            qq[(2, 0)],
            qq[(2, 1)],
            qq[(2, 2)],
        );
        let rhs = -Vector3::new(qq[(0, 3)], qq[(1, 3)], qq[(2, 3)]);
        let (pa, pb) = (pos[a as usize], pos[b as usize]);
        let mut best = [pa, pb, (pa + pb) * 0.5]
            .into_iter()
            .map(|p| (quadric_cost(&qq, p), p))
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .unwrap();
        if m.determinant().abs() > 1e-12
            && let Some(x) = m.try_inverse().map(|inv| inv * rhs)
        {
            let p = DVec3::new(x[0], x[1], x[2]);
            // Stay near the edge, or the optimum can fly off on flat areas.
            let reach = (pb - pa).length() * 2.0;
            if p.is_finite() && (p - (pa + pb) * 0.5).length() <= reach {
                let c = quadric_cost(&qq, p);
                if c < best.0 {
                    best = (c, p);
                }
            }
        }
        Candidate {
            cost: best.0,
            a,
            b,
            version: (version[a as usize], version[b as usize]),
            to: best.1,
        }
    };
    let allowed = |a: u32, b: u32| region.is_none_or(|r| r.contains(&a) && r.contains(&b));
    let mut heap = BinaryHeap::new();
    for &(a, b) in edges.keys() {
        if allowed(a, b) {
            heap.push(candidate(a, b, &q, &pos, &version));
        }
    }
    let mut live = tris.len();
    let target_count = match options.target {
        Target::Fraction(f) => ((tris.len() as f64) * f.clamp(0.0, 1.0)).ceil() as usize,
        Target::Tolerance(_) => 0,
    };
    let max_cost = match options.target {
        Target::Tolerance(d) => d * d,
        Target::Fraction(_) => f64::INFINITY,
    };
    while let Some(c) = heap.pop() {
        if live <= target_count.max(4) {
            break;
        }
        if c.cost > max_cost {
            break;
        }
        let (a, b) = (c.a, c.b);
        if removed[a as usize]
            || removed[b as usize]
            || version[a as usize] != c.version.0
            || version[b as usize] != c.version.1
        {
            continue;
        }
        if options.preserve_boundary && (boundary[a as usize] != boundary[b as usize]) {
            continue;
        }
        // Link condition: the edge's two ends share only the edge's triangles' third vertices.
        let ring = |v: u32, tris: &[[u32; 3]], alive: &[bool], vtris: &[Vec<u32>]| {
            let mut s = HashSet::new();
            for &t in &vtris[v as usize] {
                if alive[t as usize] {
                    for w in tris[t as usize] {
                        if w != v {
                            s.insert(w);
                        }
                    }
                }
            }
            s
        };
        let ra = ring(a, &tris, &alive, &vtris);
        let rb = ring(b, &tris, &alive, &vtris);
        let shared: Vec<u32> = ra.intersection(&rb).copied().collect();
        let edge_tris: Vec<u32> = vtris[a as usize]
            .iter()
            .copied()
            .filter(|t| alive[*t as usize] && tris[*t as usize].contains(&b))
            .collect();
        if shared.len() != edge_tris.len() {
            continue;
        }
        // No triangle may flip.
        let flips = [a, b].iter().any(|&v| {
            vtris[v as usize].iter().any(|&t| {
                if !alive[t as usize] || edge_tris.contains(&t) {
                    return false;
                }
                let tri = tris[t as usize];
                let before = tri.map(|i| pos[i as usize]);
                let after = tri.map(|i| {
                    if i == a || i == b {
                        c.to
                    } else {
                        pos[i as usize]
                    }
                });
                let n0 = (before[1] - before[0]).cross(before[2] - before[0]);
                let n1 = (after[1] - after[0]).cross(after[2] - after[0]);
                n0.dot(n1) <= 0.0
            })
        });
        if flips {
            continue;
        }
        // Collapse b into a.
        for t in &edge_tris {
            alive[*t as usize] = false;
            live -= 1;
        }
        let moved: Vec<u32> = vtris[b as usize].clone();
        for t in moved {
            if alive[t as usize] {
                for v in &mut tris[t as usize] {
                    if *v == b {
                        *v = a;
                    }
                }
                vtris[a as usize].push(t);
            }
        }
        removed[b as usize] = true;
        pos[a as usize] = c.to;
        let qb = q[b as usize];
        q[a as usize] += qb;
        boundary[a as usize] |= boundary[b as usize];
        version[a as usize] += 1;
        vtris[a as usize].retain(|t| alive[*t as usize]);
        let neighbours: Vec<u32> = ring(a, &tris, &alive, &vtris).into_iter().collect();
        for w in neighbours {
            if allowed(a, w) {
                heap.push(candidate(a.min(w), a.max(w), &q, &pos, &version));
            }
        }
    }
    let kept: Vec<u32> = (0..tris.len() as u32)
        .filter(|t| alive[*t as usize])
        .collect();
    let mut out = TriMesh {
        positions: pos,
        triangles: tris,
        colors: None,
        groups: mesh.groups.clone(),
    };
    out = out.subset(&kept);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flatten::cylinder_patch;

    #[test]
    fn a_flat_grid_collapses_and_keeps_its_outline() {
        let mut m = cylinder_patch(1e6, 1e-5, 10.0, 20, 20);
        // Make it truly flat in z = 0.
        for p in &mut m.positions {
            *p = DVec3::new(p.y, p.z, 0.0);
        }
        let before = m.area();
        let d = decimate(
            &m,
            Options {
                target: Target::Fraction(0.25),
                preserve_boundary: true,
            },
            None,
        );
        assert!(
            d.triangles.len() <= m.triangles.len() / 4 + 2,
            "{}",
            d.triangles.len()
        );
        assert!(
            (d.area() - before).abs() < 1e-6 * before,
            "{} {}",
            d.area(),
            before
        );
        assert!(d.positions.iter().all(|p| p.z.abs() < 1e-9));
    }

    #[test]
    fn tolerance_stops_on_a_curved_patch() {
        let m = cylinder_patch(10.0, 3.0, 10.0, 60, 10);
        let d = decimate(
            &m,
            Options {
                target: Target::Tolerance(0.01),
                preserve_boundary: true,
            },
            None,
        );
        assert!(d.triangles.len() < m.triangles.len());
        assert!(d.triangles.len() > 20);
    }
}
