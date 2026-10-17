//! Segmenting a mesh into regions: triangles joined across edges whose
//! fold angle is gentle, with tiny regions merged into a neighbour.

use crate::TriMesh;

/// Group triangles into regions. Triangles sharing an edge join when their
/// normals differ by less than `angle` (radians); regions smaller than
/// `min_area` join their largest neighbour. Returns one region number per
/// triangle, numbered from 0.
pub fn segment(mesh: &TriMesh, angle: f64, min_area: f64) -> Vec<u32> {
    let n = mesh.triangles.len();
    let normals: Vec<_> = (0..n).map(|t| mesh.normal(t)).collect();
    let cos = angle.cos();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let edges = mesh.edge_triangles();
    for list in edges.values() {
        if list.len() != 2 {
            continue;
        }
        let (a, b) = (list[0] as usize, list[1] as usize);
        if normals[a].dot(normals[b]) >= cos {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            if ra != rb {
                parent[rb] = ra;
            }
        }
    }
    // Merge small regions into their largest neighbour, repeatedly.
    loop {
        let roots: Vec<usize> = (0..n).map(|t| find(&mut parent, t)).collect();
        let mut area = std::collections::HashMap::<usize, f64>::new();
        for (t, root) in roots.iter().enumerate() {
            *area.entry(*root).or_default() += mesh.triangle_area(t);
        }
        let mut merged = false;
        let mut small: Vec<usize> = area
            .iter()
            .filter(|(_, a)| **a < min_area)
            .map(|(r, _)| *r)
            .collect();
        small.sort_unstable();
        for r in small {
            let mut best: Option<(usize, f64)> = None;
            for list in edges.values() {
                if list.len() != 2 {
                    continue;
                }
                let (a, b) = (list[0] as usize, list[1] as usize);
                let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                let other = if ra == r && rb != r {
                    rb
                } else if rb == r && ra != r {
                    ra
                } else {
                    continue;
                };
                let size = area.get(&other).copied().unwrap_or(0.0);
                if best.is_none_or(|(_, s)| size > s) {
                    best = Some((other, size));
                }
            }
            if let Some((other, _)) = best {
                let (ra, rb) = (find(&mut parent, r), find(&mut parent, other));
                if ra != rb {
                    parent[ra] = rb;
                    merged = true;
                }
            }
        }
        if !merged {
            break;
        }
    }
    let mut ids = std::collections::HashMap::new();
    (0..n)
        .map(|t| {
            let r = find(&mut parent, t);
            let next = ids.len() as u32;
            *ids.entry(r).or_insert(next)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_cube_has_six_regions() {
        let c = crate::tests::cube();
        let r = super::segment(&c, 0.3, 0.0);
        let count = r.iter().copied().max().unwrap() + 1;
        assert_eq!(count, 6);
    }

    #[test]
    fn a_smooth_patch_is_one_region() {
        let m = crate::flatten::cylinder_patch(10.0, 2.0, 5.0, 40, 4);
        let r = super::segment(&m, 0.2, 0.0);
        assert!(r.iter().all(|g| *g == 0));
    }
}
