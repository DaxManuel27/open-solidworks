//! A bounding volume hierarchy over a mesh's triangles: ray casts and
//! closest points.

use glam::DVec3;

use crate::{Bounds, TriMesh};

#[derive(Debug, Clone)]
struct Node {
    bounds: Bounds,
    /// Children for an inner node, or the range in `order` for a leaf.
    left: u32,
    right: u32,
    leaf: bool,
}

/// A ray hit: the triangle, distance along the ray and barycentrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub triangle: u32,
    pub distance: f64,
    pub point: DVec3,
}

/// A closest point on the mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Closest {
    pub triangle: u32,
    pub point: DVec3,
    pub distance: f64,
}

pub struct Bvh<'a> {
    mesh: &'a TriMesh,
    nodes: Vec<Node>,
    order: Vec<u32>,
}

const LEAF: usize = 4;

impl<'a> Bvh<'a> {
    pub fn new(mesh: &'a TriMesh) -> Bvh<'a> {
        let mut order: Vec<u32> = (0..mesh.triangles.len() as u32).collect();
        let centers: Vec<DVec3> = (0..mesh.triangles.len())
            .map(|t| {
                let [a, b, c] = mesh.corners(t);
                (a + b + c) / 3.0
            })
            .collect();
        let mut nodes = Vec::new();
        if !order.is_empty() {
            let n = order.len();
            build(mesh, &centers, &mut order, 0, n, &mut nodes);
        }
        Bvh { mesh, nodes, order }
    }

    /// The first triangle a ray from `origin` along `dir` hits past `min`.
    pub fn ray(&self, origin: DVec3, dir: DVec3, min: f64) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = DVec3::ONE / dir;
        let mut best: Option<Hit> = None;
        let mut stack = vec![0u32];
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i as usize];
            let limit = best.map_or(f64::INFINITY, |h| h.distance);
            if !slab(&node.bounds, origin, inv, limit) {
                continue;
            }
            if node.leaf {
                for &t in &self.order[node.left as usize..node.right as usize] {
                    let [a, b, c] = self.mesh.corners(t as usize);
                    if let Some(d) = intersect(origin, dir, a, b, c)
                        && d > min
                        && d < limit.min(best.map_or(f64::INFINITY, |h| h.distance))
                    {
                        best = Some(Hit {
                            triangle: t,
                            distance: d,
                            point: origin + dir * d,
                        });
                    }
                }
            } else {
                stack.push(node.left);
                stack.push(node.right);
            }
        }
        best
    }

    /// How many times a ray from `origin` along `dir` crosses the mesh, with
    /// crossings at shared edges counted once.
    pub fn crossings(&self, origin: DVec3, dir: DVec3) -> usize {
        if self.nodes.is_empty() {
            return 0;
        }
        let inv = DVec3::ONE / dir;
        let mut ts = Vec::new();
        let mut stack = vec![0u32];
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i as usize];
            if !slab(&node.bounds, origin, inv, f64::INFINITY) {
                continue;
            }
            if node.leaf {
                for &t in &self.order[node.left as usize..node.right as usize] {
                    let [a, b, c] = self.mesh.corners(t as usize);
                    if let Some(d) = intersect(origin, dir, a, b, c)
                        && d > 0.0
                    {
                        ts.push(d);
                    }
                }
            } else {
                stack.push(node.left);
                stack.push(node.right);
            }
        }
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-9 * (1.0 + b.abs()));
        ts.len()
    }

    /// Whether `p` is inside a closed mesh (by crossing parity).
    pub fn contains(&self, p: DVec3) -> bool {
        // An oblique direction is unlikely to graze edges.
        self.crossings(p, DVec3::new(0.5773, 0.5774, 0.5772)) % 2 == 1
    }

    /// The point on the mesh nearest `p`.
    pub fn closest(&self, p: DVec3) -> Option<Closest> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut best: Option<Closest> = None;
        let mut stack = vec![0u32];
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i as usize];
            let d = box_distance(&node.bounds, p);
            if best.is_some_and(|b| d > b.distance) {
                continue;
            }
            if node.leaf {
                for &t in &self.order[node.left as usize..node.right as usize] {
                    let [a, b, c] = self.mesh.corners(t as usize);
                    let q = closest_on_triangle(p, a, b, c);
                    let d = (q - p).length();
                    if best.is_none_or(|b| d < b.distance) {
                        best = Some(Closest {
                            triangle: t,
                            point: q,
                            distance: d,
                        });
                    }
                }
            } else {
                // Visit the nearer child last so it pops first.
                let (l, r) = (node.left, node.right);
                let dl = box_distance(&self.nodes[l as usize].bounds, p);
                let dr = box_distance(&self.nodes[r as usize].bounds, p);
                if dl < dr {
                    stack.push(r);
                    stack.push(l);
                } else {
                    stack.push(l);
                    stack.push(r);
                }
            }
        }
        best
    }
}

fn build(
    mesh: &TriMesh,
    centers: &[DVec3],
    order: &mut [u32],
    start: usize,
    end: usize,
    nodes: &mut Vec<Node>,
) -> u32 {
    let mut bounds = Bounds::empty();
    for &t in &order[start..end] {
        for p in mesh.corners(t as usize) {
            bounds.add(p);
        }
    }
    let index = nodes.len() as u32;
    nodes.push(Node {
        bounds,
        left: start as u32,
        right: end as u32,
        leaf: true,
    });
    if end - start <= LEAF {
        return index;
    }
    let mut cb = Bounds::empty();
    for &t in &order[start..end] {
        cb.add(centers[t as usize]);
    }
    let ext = cb.max - cb.min;
    let axis = if ext.x >= ext.y && ext.x >= ext.z {
        0
    } else if ext.y >= ext.z {
        1
    } else {
        2
    };
    let mid = (start + end) / 2;
    order[start..end].select_nth_unstable_by(mid - start, |a, b| {
        centers[*a as usize][axis].total_cmp(&centers[*b as usize][axis])
    });
    let left = build(mesh, centers, order, start, mid, nodes);
    let right = build(mesh, centers, order, mid, end, nodes);
    nodes[index as usize] = Node {
        bounds,
        left,
        right,
        leaf: false,
    };
    index
}

fn slab(b: &Bounds, o: DVec3, inv: DVec3, limit: f64) -> bool {
    let mut t0: f64 = 0.0;
    let mut t1 = limit;
    for k in 0..3 {
        let (a, c) = ((b.min[k] - o[k]) * inv[k], (b.max[k] - o[k]) * inv[k]);
        let (lo, hi) = if a <= c { (a, c) } else { (c, a) };
        if lo.is_nan() || hi.is_nan() {
            continue;
        }
        t0 = t0.max(lo);
        t1 = t1.min(hi);
        if t0 > t1 + 1e-12 {
            return false;
        }
    }
    true
}

fn box_distance(b: &Bounds, p: DVec3) -> f64 {
    let d = (b.min - p).max(p - b.max).max(DVec3::ZERO);
    d.length()
}

/// Möller–Trumbore; the distance along `dir` (not normalised).
pub fn intersect(o: DVec3, dir: DVec3, a: DVec3, b: DVec3, c: DVec3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-14 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(-1e-12..=1.0 + 1e-12).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < -1e-12 || u + v > 1.0 + 1e-12 {
        return None;
    }
    Some(e2.dot(q) * inv)
}

/// The point of triangle `abc` nearest `p` (Ericson's method).
pub fn closest_on_triangle(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> DVec3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rays_and_closest_points_on_a_cube() {
        let c = crate::tests::cube();
        let bvh = Bvh::new(&c);
        let hit = bvh.ray(DVec3::new(0.5, 0.5, -5.0), DVec3::Z, 0.0).unwrap();
        assert!((hit.distance - 5.0).abs() < 1e-12);
        assert!(bvh.ray(DVec3::new(3.0, 0.5, -5.0), DVec3::Z, 0.0).is_none());
        assert!(bvh.contains(DVec3::splat(0.5)));
        assert!(!bvh.contains(DVec3::splat(1.5)));
        let q = bvh.closest(DVec3::new(0.5, 0.5, 3.0)).unwrap();
        assert!((q.distance - 2.0).abs() < 1e-12);
        assert!((q.point.z - 1.0).abs() < 1e-12);
    }
}
