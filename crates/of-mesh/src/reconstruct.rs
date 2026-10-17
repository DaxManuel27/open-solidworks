//! Turning a point cloud into a mesh: normals from each point's neighbours,
//! oriented consistently, a signed distance from the nearest oriented point
//! (Hoppe et al.), and its zero level traced by marching tetrahedra over the
//! cells near the points.

use std::collections::{HashMap, HashSet, VecDeque};

use glam::DVec3;

use crate::{Result, TriMesh, fit, invalid};

/// A hash grid of points for neighbour queries.
pub struct Grid<'a> {
    points: &'a [DVec3],
    cell: f64,
    cells: HashMap<(i64, i64, i64), Vec<u32>>,
}

impl<'a> Grid<'a> {
    pub fn new(points: &'a [DVec3], cell: f64) -> Grid<'a> {
        let mut cells: HashMap<_, Vec<u32>> = HashMap::new();
        for (i, p) in points.iter().enumerate() {
            cells.entry(key(*p, cell)).or_default().push(i as u32);
        }
        Grid {
            points,
            cell,
            cells,
        }
    }

    /// The `k` nearest points to `p`, nearest first.
    pub fn nearest(&self, p: DVec3, k: usize) -> Vec<u32> {
        let (cx, cy, cz) = key(p, self.cell);
        let mut found: Vec<(f64, u32)> = Vec::new();
        for r in 0..64i64 {
            found.clear();
            for dx in -r..=r {
                for dy in -r..=r {
                    for dz in -r..=r {
                        if let Some(list) = self.cells.get(&(cx + dx, cy + dy, cz + dz)) {
                            for &i in list {
                                found.push(((self.points[i as usize] - p).length_squared(), i));
                            }
                        }
                    }
                }
            }
            // Enough points, and none outside the searched cube could be nearer.
            if found.len() >= k {
                found.sort_by(|a, b| a.0.total_cmp(&b.0));
                if found[k - 1].0.sqrt() <= r as f64 * self.cell {
                    break;
                }
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.into_iter().take(k).map(|(_, i)| i).collect()
    }
}

/// A grid cell or corner.
type Cell = (i64, i64, i64);

fn key(p: DVec3, cell: f64) -> Cell {
    (
        (p.x / cell).floor() as i64,
        (p.y / cell).floor() as i64,
        (p.z / cell).floor() as i64,
    )
}

/// Average distance from each point to its nearest neighbour.
pub fn spacing(points: &[DVec3]) -> f64 {
    let b = points.iter().fold(crate::Bounds::empty(), |mut b, p| {
        b.add(*p);
        b
    });
    let guess = (b.diagonal() / (points.len() as f64).cbrt()).max(1e-9);
    let grid = Grid::new(points, guess);
    let step = (points.len() / 500).max(1);
    let samples: Vec<f64> = (0..points.len())
        .step_by(step)
        .filter_map(|i| {
            let n = grid.nearest(points[i], 2);
            n.get(1).map(|j| (points[*j as usize] - points[i]).length())
        })
        .collect();
    samples.iter().sum::<f64>() / samples.len().max(1) as f64
}

/// Point normals from `k` neighbours, flipped to agree across neighbours,
/// with the outermost point facing away from the centroid.
pub fn normals(points: &[DVec3], k: usize) -> Vec<DVec3> {
    let h = spacing(points).max(1e-9);
    let grid = Grid::new(points, h * 2.0);
    let neighbours: Vec<Vec<u32>> = points.iter().map(|p| grid.nearest(*p, k)).collect();
    let mut n: Vec<DVec3> = neighbours
        .iter()
        .map(|nb| {
            let pts: Vec<DVec3> = nb.iter().map(|i| points[*i as usize]).collect();
            match fit::plane(&pts).map(|f| f.primitive) {
                Ok(fit::Primitive::Plane { normal, .. }) => normal,
                _ => DVec3::Z,
            }
        })
        .collect();
    // Propagate orientation outward from the point furthest from the centroid.
    let c = points.iter().copied().sum::<DVec3>() / points.len() as f64;
    let mut seen = vec![false; points.len()];
    while let Some(seed) = (0..points.len()).filter(|i| !seen[*i]).max_by(|a, b| {
        (points[*a] - c)
            .length()
            .total_cmp(&(points[*b] - c).length())
    }) {
        if n[seed].dot(points[seed] - c) < 0.0 {
            n[seed] = -n[seed];
        }
        seen[seed] = true;
        let mut queue = VecDeque::from([seed]);
        while let Some(i) = queue.pop_front() {
            for &j in &neighbours[i] {
                let j = j as usize;
                if !seen[j] {
                    if n[j].dot(n[i]) < 0.0 {
                        n[j] = -n[j];
                    }
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
    }
    n
}

/// Reconstruct a surface from points. `cell` is the grid size (0 picks one
/// from the point spacing). Triangles further than two cells from any point
/// are dropped, so open scans stay open.
pub fn reconstruct(points: &[DVec3], cell: f64) -> Result<TriMesh> {
    if points.len() < 10 {
        return Err(invalid("there are too few points to make a surface"));
    }
    let h = if cell > 0.0 {
        cell
    } else {
        spacing(points) * 1.5
    };
    if !(h.is_finite() && h > 0.0) {
        return Err(invalid("the points have no spread"));
    }
    let normals = normals(points, 12);
    let grid = Grid::new(points, h);
    let sdf = |x: DVec3| -> (f64, f64) {
        let i = grid.nearest(x, 1)[0] as usize;
        let d = x - points[i];
        (normals[i].dot(d), d.length())
    };
    // Cells within reach of a point.
    let mut cells = HashSet::new();
    for p in points {
        let (cx, cy, cz) = key(*p, h);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    cells.insert((cx + dx, cy + dy, cz + dz));
                }
            }
        }
    }
    let mut corner_cache: HashMap<(i64, i64, i64), (f64, f64)> = HashMap::new();
    let mut value = |c: (i64, i64, i64)| {
        *corner_cache
            .entry(c)
            .or_insert_with(|| sdf(DVec3::new(c.0 as f64, c.1 as f64, c.2 as f64) * h))
    };
    // A cube's six tetrahedra around its main diagonal (corners 0 and 6).
    const CORNERS: [(i64, i64, i64); 8] = [
        (0, 0, 0),
        (1, 0, 0),
        (1, 1, 0),
        (0, 1, 0),
        (0, 0, 1),
        (1, 0, 1),
        (1, 1, 1),
        (0, 1, 1),
    ];
    const TETS: [[usize; 4]; 6] = [
        [0, 5, 1, 6],
        [0, 1, 2, 6],
        [0, 2, 3, 6],
        [0, 3, 7, 6],
        [0, 7, 4, 6],
        [0, 4, 5, 6],
    ];
    let mut mesh = TriMesh::default();
    let mut vertex_of: HashMap<(Cell, Cell), u32> = HashMap::new();
    let mut cell_list: Vec<_> = cells.into_iter().collect();
    cell_list.sort_unstable();
    for c in cell_list {
        let corner = |k: usize| (c.0 + CORNERS[k].0, c.1 + CORNERS[k].1, c.2 + CORNERS[k].2);
        for tet in TETS {
            let v: Vec<(i64, i64, i64)> = tet.iter().map(|k| corner(*k)).collect();
            let f: Vec<(f64, f64)> = v.iter().map(|k| value(*k)).collect();
            // Skip where the nearest point is far: the distance is meaningless.
            if f.iter().any(|(_, d)| *d > 2.0 * h) {
                continue;
            }
            let inside: Vec<usize> = (0..4).filter(|i| f[*i].0 < 0.0).collect();
            if inside.is_empty() || inside.len() == 4 {
                continue;
            }
            let mut edge_point = |a: usize, b: usize, mesh: &mut TriMesh| -> u32 {
                let (ka, kb) = if v[a] < v[b] {
                    (v[a], v[b])
                } else {
                    (v[b], v[a])
                };
                *vertex_of.entry((ka, kb)).or_insert_with(|| {
                    let (fa, fb) = (f[a].0, f[b].0);
                    let t = fa / (fa - fb);
                    let pa = DVec3::new(v[a].0 as f64, v[a].1 as f64, v[a].2 as f64) * h;
                    let pb = DVec3::new(v[b].0 as f64, v[b].1 as f64, v[b].2 as f64) * h;
                    mesh.positions.push(pa + (pb - pa) * t);
                    mesh.positions.len() as u32 - 1
                })
            };
            let outside: Vec<usize> = (0..4).filter(|i| f[*i].0 >= 0.0).collect();
            let tet_pos: Vec<DVec3> = v
                .iter()
                .map(|k| DVec3::new(k.0 as f64, k.1 as f64, k.2 as f64))
                .collect();
            let push = |mesh: &mut TriMesh, mut t: [u32; 3], toward: DVec3| {
                // Face the triangle outward: toward the positive side.
                let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
                if (b - a).cross(c - a).dot(toward) < 0.0 {
                    t.swap(1, 2);
                }
                mesh.triangles.push(t);
            };
            let centre =
                |idx: &[usize]| idx.iter().map(|i| tet_pos[*i]).sum::<DVec3>() / idx.len() as f64;
            let toward = centre(&outside) - centre(&inside);
            match inside.len() {
                1 | 3 => {
                    let (lone, others) = if inside.len() == 1 {
                        (inside[0], outside.clone())
                    } else {
                        (outside[0], inside.clone())
                    };
                    let t = [
                        edge_point(lone, others[0], &mut mesh),
                        edge_point(lone, others[1], &mut mesh),
                        edge_point(lone, others[2], &mut mesh),
                    ];
                    push(&mut mesh, t, toward);
                }
                _ => {
                    let (a, b) = (inside[0], inside[1]);
                    let (c, d) = (outside[0], outside[1]);
                    let p = [
                        edge_point(a, c, &mut mesh),
                        edge_point(a, d, &mut mesh),
                        edge_point(b, d, &mut mesh),
                        edge_point(b, c, &mut mesh),
                    ];
                    push(&mut mesh, [p[0], p[1], p[2]], toward);
                    push(&mut mesh, [p[0], p[2], p[3]], toward);
                }
            }
        }
    }
    if mesh.triangles.is_empty() {
        return Err(invalid("no surface could be traced through the points"));
    }
    // Drop slivers made of one point, and triangles far from the data.
    let keep: Vec<u32> = (0..mesh.triangles.len() as u32)
        .filter(|t| {
            let [a, b, c] = mesh.corners(*t as usize);
            let centre = (a + b + c) / 3.0;
            let i = grid.nearest(centre, 1)[0] as usize;
            (points[i] - centre).length() <= 1.5 * h && mesh.triangle_area(*t as usize) > 1e-14
        })
        .collect();
    let mut out = mesh.subset(&keep);
    out.weld(h * 1e-6);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sampled_sphere_comes_back() {
        // Points on a sphere of radius 10, roughly evenly spread.
        let n = 3000;
        let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
        let points: Vec<DVec3> = (0..n)
            .map(|i| {
                let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
                let r = (1.0 - y * y).sqrt();
                let a = golden * i as f64;
                DVec3::new(a.cos() * r, y, a.sin() * r) * 10.0
            })
            .collect();
        let m = reconstruct(&points, 0.0).unwrap();
        assert!(m.triangles.len() > 1000);
        let worst = m
            .positions
            .iter()
            .map(|p| (p.length() - 10.0).abs())
            .fold(0.0, f64::max);
        assert!(worst < 0.5, "{worst}");
        let area = m.area();
        let true_area = 4.0 * std::f64::consts::PI * 100.0;
        assert!((area - true_area).abs() < 0.1 * true_area, "{area}");
        // Outward-facing.
        assert!(m.volume() > 0.0);
    }
}
