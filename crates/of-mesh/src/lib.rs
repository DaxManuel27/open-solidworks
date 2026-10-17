//! Triangle meshes for OpenForge: mesh bodies, scan data and mesh-based
//! analysis. Pure Rust, with no dependency on the geometry kernel.
//!
//! A [`TriMesh`] is indexed triangles with optional per-vertex colours and a
//! group (face) per triangle. Readers and writers live in [`io`], spatial
//! queries in [`bvh`], and the algorithms in their own modules.

pub mod bvh;
pub mod decimate;
pub mod fit;
pub mod flatten;
pub mod io;
pub mod prep;
pub mod reconstruct;
pub mod segment;
pub mod solve;

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// A fast hash for integer keys (FxHash's multiply-rotate); not for keys an
/// attacker chooses.
#[derive(Default)]
struct FastHasher(u64);

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u64(*b as u64);
        }
    }
    fn write_u32(&mut self, v: u32) {
        self.write_u64(v as u64);
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

type Fast = BuildHasherDefault<FastHasher>;

pub use glam::{DVec2, DVec3};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum MeshError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, MeshError>;

pub(crate) fn invalid(msg: impl Into<String>) -> MeshError {
    MeshError::Invalid(msg.into())
}

/// Indexed triangles.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TriMesh {
    pub positions: Vec<DVec3>,
    pub triangles: Vec<[u32; 3]>,
    /// Per-vertex colour, when the file had one.
    pub colors: Option<Vec<[u8; 3]>>,
    /// The group (face) each triangle belongs to; empty means one group.
    pub groups: Vec<u32>,
}

/// Axis-aligned bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: DVec3,
    pub max: DVec3,
}

impl Bounds {
    pub fn empty() -> Bounds {
        Bounds {
            min: DVec3::splat(f64::INFINITY),
            max: DVec3::splat(f64::NEG_INFINITY),
        }
    }

    pub fn add(&mut self, p: DVec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn diagonal(&self) -> f64 {
        (self.max - self.min).length()
    }

    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }
}

impl TriMesh {
    pub fn new(positions: Vec<DVec3>, triangles: Vec<[u32; 3]>) -> TriMesh {
        TriMesh {
            positions,
            triangles,
            colors: None,
            groups: Vec::new(),
        }
    }

    /// Check every index is in range and every triangle has three vertices.
    pub fn validate(&self) -> Result<()> {
        let n = self.positions.len() as u32;
        if self.triangles.iter().flatten().any(|i| *i >= n) {
            return Err(invalid("a triangle refers to a vertex that doesn't exist"));
        }
        if !self.groups.is_empty() && self.groups.len() != self.triangles.len() {
            return Err(invalid("there must be one group per triangle"));
        }
        if self
            .colors
            .as_ref()
            .is_some_and(|c| c.len() != self.positions.len())
        {
            return Err(invalid("there must be one colour per vertex"));
        }
        if self.positions.iter().any(|p| !p.is_finite()) {
            return Err(invalid("a vertex has a non-finite coordinate"));
        }
        Ok(())
    }

    pub fn group(&self, t: usize) -> u32 {
        self.groups.get(t).copied().unwrap_or(0)
    }

    pub fn corners(&self, t: usize) -> [DVec3; 3] {
        let [a, b, c] = self.triangles[t];
        [
            self.positions[a as usize],
            self.positions[b as usize],
            self.positions[c as usize],
        ]
    }

    /// Unit normal of a triangle (zero for a degenerate one).
    pub fn normal(&self, t: usize) -> DVec3 {
        let [a, b, c] = self.corners(t);
        (b - a).cross(c - a).normalize_or_zero()
    }

    pub fn triangle_area(&self, t: usize) -> f64 {
        let [a, b, c] = self.corners(t);
        0.5 * (b - a).cross(c - a).length()
    }

    pub fn area(&self) -> f64 {
        (0..self.triangles.len())
            .map(|t| self.triangle_area(t))
            .sum()
    }

    /// The enclosed volume, signed by orientation; meaningful when closed.
    pub fn volume(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| self.positions[i as usize]);
                a.dot(b.cross(c)) / 6.0
            })
            .sum()
    }

    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds::empty();
        for p in &self.positions {
            b.add(*p);
        }
        b
    }

    /// Area-weighted vertex normals.
    pub fn vertex_normals(&self) -> Vec<DVec3> {
        let mut n = vec![DVec3::ZERO; self.positions.len()];
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.positions[i as usize]);
            let w = (b - a).cross(c - a);
            for i in t {
                n[*i as usize] += w;
            }
        }
        n.into_iter().map(|v| v.normalize_or_zero()).collect()
    }

    /// Merge vertices closer than `tolerance`, dropping triangles that
    /// collapse. Returns the old-to-new vertex map.
    pub fn weld(&mut self, tolerance: f64) -> Vec<u32> {
        let tags = vec![0; self.positions.len()];
        self.weld_tagged(tolerance, &tags)
    }

    /// [`TriMesh::weld`], merging only vertices with the same tag: tagging
    /// the two sides of a cut differently keeps them apart.
    pub fn weld_tagged(&mut self, tolerance: f64, tags: &[u32]) -> Vec<u32> {
        let mut map = vec![0u32; self.positions.len()];
        let mut positions: Vec<DVec3> = Vec::new();
        let mut colors = self.colors.as_ref().map(|_| Vec::new());
        if tolerance <= 0.0 {
            // Exact: one lookup per vertex, as file readers need for
            // millions of corners.
            let mut seen: HashMap<([u64; 3], u32), u32, Fast> = HashMap::default();
            for (i, p) in self.positions.iter().enumerate() {
                // +0.0 and -0.0 are the same point.
                let key = ([p.x + 0.0, p.y + 0.0, p.z + 0.0].map(f64::to_bits), tags[i]);
                map[i] = *seen.entry(key).or_insert_with(|| {
                    positions.push(*p);
                    if let (Some(out), Some(src)) = (&mut colors, &self.colors) {
                        out.push(src[i]);
                    }
                    positions.len() as u32 - 1
                });
            }
        } else {
            self.weld_near(tolerance, tags, &mut map, &mut positions, &mut colors);
        }
        let mut triangles = Vec::new();
        let mut groups = Vec::new();
        for (t, tri) in self.triangles.iter().enumerate() {
            let m = tri.map(|i| map[i as usize]);
            if m[0] != m[1] && m[1] != m[2] && m[0] != m[2] {
                triangles.push(m);
                if !self.groups.is_empty() {
                    groups.push(self.groups[t]);
                }
            }
        }
        self.positions = positions;
        self.triangles = triangles;
        self.groups = groups;
        self.colors = colors;
        map
    }

    /// The tolerant half of [`TriMesh::weld_tagged`]: a grid of cells
    /// `tolerance` wide, searching the neighbouring cells.
    fn weld_near(
        &self,
        tolerance: f64,
        tags: &[u32],
        map: &mut [u32],
        positions: &mut Vec<DVec3>,
        colors: &mut Option<Vec<[u8; 3]>>,
    ) {
        let cell = tolerance.max(1e-12);
        let key = |p: DVec3| {
            (
                (p.x / cell).round() as i64,
                (p.y / cell).round() as i64,
                (p.z / cell).round() as i64,
            )
        };
        let mut grid: HashMap<(i64, i64, i64), Vec<u32>, Fast> = HashMap::default();
        let mut new_tags: Vec<u32> = Vec::new();
        for (i, p) in self.positions.iter().enumerate() {
            let (kx, ky, kz) = key(*p);
            let mut found = None;
            'search: for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(list) = grid.get(&(kx + dx, ky + dy, kz + dz)) {
                            for &j in list {
                                if new_tags[j as usize] == tags[i]
                                    && (positions[j as usize] - *p).length() <= tolerance
                                {
                                    found = Some(j);
                                    break 'search;
                                }
                            }
                        }
                    }
                }
            }
            let j = found.unwrap_or_else(|| {
                positions.push(*p);
                new_tags.push(tags[i]);
                if let (Some(out), Some(src)) = (colors.as_mut(), &self.colors) {
                    out.push(src[i]);
                }
                let j = positions.len() as u32 - 1;
                grid.entry((kx, ky, kz)).or_default().push(j);
                j
            });
            map[i] = j;
        }
    }

    /// For each undirected edge (smaller vertex first), the triangles using it.
    pub fn edge_triangles(&self) -> HashMap<(u32, u32), Vec<u32>> {
        let mut m: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
        for (t, tri) in self.triangles.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                m.entry((a.min(b), a.max(b))).or_default().push(t as u32);
            }
        }
        m
    }

    /// Triangles sharing an edge with each triangle.
    pub fn triangle_neighbours(&self) -> Vec<Vec<u32>> {
        let mut n = vec![Vec::new(); self.triangles.len()];
        for list in self.edge_triangles().values() {
            for &a in list {
                for &b in list {
                    if a != b {
                        n[a as usize].push(b);
                    }
                }
            }
        }
        n
    }

    /// Closed loops of boundary edges (edges used by one triangle), each as
    /// vertex indices in order, running the same way as their triangles.
    pub fn boundary_loops(&self) -> Vec<Vec<u32>> {
        let mut next: HashMap<u32, Vec<u32>> = HashMap::new();
        let edges = self.edge_triangles();
        for tri in &self.triangles {
            for k in 0..3 {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                if edges[&(a.min(b), a.max(b))].len() == 1 {
                    next.entry(a).or_default().push(b);
                }
            }
        }
        let mut loops = Vec::new();
        let mut starts: Vec<u32> = next.keys().copied().collect();
        starts.sort_unstable();
        for start in starts {
            while let Some(first) = next.get_mut(&start).and_then(|v| v.pop()) {
                let mut lp = vec![start];
                let mut at = first;
                while at != start {
                    lp.push(at);
                    match next.get_mut(&at).and_then(|v| v.pop()) {
                        Some(n) => at = n,
                        None => break,
                    }
                }
                if at == start && lp.len() >= 3 {
                    loops.push(lp);
                }
            }
        }
        loops
    }

    /// Whether every edge is used by exactly two triangles.
    pub fn is_closed(&self) -> bool {
        self.edge_triangles().values().all(|t| t.len() == 2)
    }

    /// Closed, and every edge runs once each way: the triangles all face
    /// the same side.
    pub fn is_oriented(&self) -> bool {
        let mut directed: HashMap<(u32, u32), u32, Fast> = HashMap::default();
        for t in &self.triangles {
            for k in 0..3 {
                *directed.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
            }
        }
        directed
            .iter()
            .all(|(&(a, b), &n)| n == 1 && directed.get(&(b, a)) == Some(&1))
    }

    /// Connected sets of triangles (by shared edges).
    pub fn components(&self) -> Vec<Vec<u32>> {
        let neighbours = self.triangle_neighbours();
        let mut seen = vec![false; self.triangles.len()];
        let mut out = Vec::new();
        for s in 0..self.triangles.len() {
            if seen[s] {
                continue;
            }
            let mut stack = vec![s as u32];
            seen[s] = true;
            let mut comp = Vec::new();
            while let Some(t) = stack.pop() {
                comp.push(t);
                for &n in &neighbours[t as usize] {
                    if !seen[n as usize] {
                        seen[n as usize] = true;
                        stack.push(n);
                    }
                }
            }
            out.push(comp);
        }
        out
    }

    /// A mesh of a subset of triangles, with only the vertices they use.
    pub fn subset(&self, triangles: &[u32]) -> TriMesh {
        let mut map = HashMap::new();
        let mut positions = Vec::new();
        let mut colors = self.colors.as_ref().map(|_| Vec::new());
        let mut tris = Vec::new();
        let mut groups = Vec::new();
        for &t in triangles {
            let tri = self.triangles[t as usize].map(|v| {
                *map.entry(v).or_insert_with(|| {
                    positions.push(self.positions[v as usize]);
                    if let (Some(out), Some(src)) = (&mut colors, &self.colors) {
                        out.push(src[v as usize]);
                    }
                    positions.len() as u32 - 1
                })
            });
            tris.push(tri);
            if !self.groups.is_empty() {
                groups.push(self.groups[t as usize]);
            }
        }
        TriMesh {
            positions,
            triangles: tris,
            colors,
            groups,
        }
    }

    /// Append another mesh.
    pub fn append(&mut self, other: &TriMesh) {
        let base = self.positions.len() as u32;
        let group_base = self.groups.iter().copied().max().map_or(0, |g| g + 1);
        if self.groups.is_empty() && !other.groups.is_empty() {
            self.groups = vec![0; self.triangles.len()];
        }
        self.positions.extend(&other.positions);
        self.triangles
            .extend(other.triangles.iter().map(|t| t.map(|i| i + base)));
        if !self.groups.is_empty() || !other.groups.is_empty() {
            self.groups
                .extend((0..other.triangles.len()).map(|t| other.group(t) + group_base));
        }
        self.colors = None;
    }

    /// Flip every triangle's orientation.
    pub fn flip(&mut self) {
        for t in &mut self.triangles {
            t.swap(1, 2);
        }
    }

    /// Scale about the origin.
    pub fn scale(&mut self, s: f64) {
        for p in &mut self.positions {
            *p *= s;
        }
    }
}

/// Area of the union of the triangles projected on a plane with unit
/// normal `normal`, estimated on a grid of `resolution` cells across.
/// Returns the area and the largest error the grid can make.
pub fn projected_area(mesh: &TriMesh, normal: DVec3, resolution: usize) -> (f64, f64) {
    let n = normal.normalize();
    let x = n.any_orthonormal_vector();
    let y = n.cross(x);
    let flat: Vec<DVec2> = mesh
        .positions
        .iter()
        .map(|p| DVec2::new(p.dot(x), p.dot(y)))
        .collect();
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for p in &flat {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    if mesh.triangles.is_empty() || !(hi.x > lo.x && hi.y > lo.y) {
        return (0.0, 0.0);
    }
    let size = (hi - lo).max_element();
    let res = resolution.max(16);
    let cell = size / res as f64;
    let w = ((hi.x - lo.x) / cell).ceil() as usize + 1;
    let h = ((hi.y - lo.y) / cell).ceil() as usize + 1;
    let mut covered = vec![false; w * h];
    for tri in &mesh.triangles {
        let [a, b, c] = tri.map(|i| flat[i as usize]);
        let tmin = a.min(b).min(c);
        let tmax = a.max(b).max(c);
        let i0 = ((tmin.x - lo.x) / cell).floor().max(0.0) as usize;
        let i1 = (((tmax.x - lo.x) / cell).ceil() as usize).min(w - 1);
        let j0 = ((tmin.y - lo.y) / cell).floor().max(0.0) as usize;
        let j1 = (((tmax.y - lo.y) / cell).ceil() as usize).min(h - 1);
        let cross = |o: DVec2, p: DVec2, q: DVec2| (p - o).perp_dot(q - o);
        let area = cross(a, b, c);
        if area.abs() < 1e-18 {
            continue;
        }
        for j in j0..=j1 {
            for i in i0..=i1 {
                let p = lo + DVec2::new((i as f64 + 0.5) * cell, (j as f64 + 0.5) * cell);
                let (u, v, s) = (cross(b, c, p), cross(c, a, p), cross(a, b, p));
                let inside = if area > 0.0 {
                    u >= 0.0 && v >= 0.0 && s >= 0.0
                } else {
                    u <= 0.0 && v <= 0.0 && s <= 0.0
                };
                if inside {
                    covered[j * w + i] = true;
                }
            }
        }
    }
    let count = covered.iter().filter(|c| **c).count();
    let area = count as f64 * cell * cell;
    // Cells on the outline may be wrong either way.
    let boundary = covered
        .iter()
        .enumerate()
        .filter(|(k, c)| {
            **c && {
                let (i, j) = (k % w, k / w);
                i == 0
                    || j == 0
                    || i + 1 == w
                    || j + 1 == h
                    || !covered[k - 1]
                    || !covered[k + 1]
                    || !covered[k - w]
                    || !covered[k + w]
            }
        })
        .count();
    (area, boundary as f64 * cell * cell)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_weld_joins_equal_corners_and_keeps_tags_apart() {
        // Two triangles sharing an edge, one corner written as -0.0.
        let p = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(-0.0, 1.0, 0.0),
        ];
        let mut m = TriMesh::new(p.clone(), vec![[0, 1, 2], [3, 4, 5]]);
        m.weld(0.0);
        assert_eq!(m.positions.len(), 4);
        let mut apart = TriMesh::new(p, vec![[0, 1, 2], [3, 4, 5]]);
        apart.weld_tagged(0.0, &[0, 0, 0, 1, 1, 1]);
        assert_eq!(apart.positions.len(), 6);
    }

    /// A unit cube, outward triangles.
    pub(crate) fn cube() -> TriMesh {
        let p = (0..8)
            .map(|i| DVec3::new((i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64))
            .collect();
        let quads = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        let t = quads
            .iter()
            .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
            .collect();
        TriMesh::new(p, t)
    }

    #[test]
    fn cube_measures() {
        let c = cube();
        c.validate().unwrap();
        assert!((c.area() - 6.0).abs() < 1e-12);
        assert!((c.volume() - 1.0).abs() < 1e-12);
        assert!(c.is_closed());
        assert!(c.boundary_loops().is_empty());
        assert_eq!(c.components().len(), 1);
        let (a, err) = projected_area(&c, DVec3::Z, 400);
        assert!((a - 1.0).abs() <= err + 1e-9, "{a} ± {err}");
    }

    #[test]
    fn weld_joins_split_vertices_and_finds_the_boundary() {
        // Two triangles of a square, with the shared edge duplicated.
        let mut m = TriMesh::new(
            vec![
                DVec3::ZERO,
                DVec3::X,
                DVec3::new(1.0, 1.0, 0.0),
                DVec3::ZERO,
                DVec3::new(1.0, 1.0, 0.0),
                DVec3::Y,
            ],
            vec![[0, 1, 2], [3, 4, 5]],
        );
        m.weld(1e-9);
        assert_eq!(m.positions.len(), 4);
        let loops = m.boundary_loops();
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].len(), 4);
    }
}
