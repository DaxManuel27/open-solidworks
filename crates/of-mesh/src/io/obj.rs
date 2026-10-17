//! Wavefront OBJ: vertices (with optional colours), polygonal faces and
//! groups.

use glam::DVec3;

use super::{fan, io};
use crate::{Result, TriMesh};

pub fn read(bytes: &[u8]) -> Result<TriMesh> {
    let text = String::from_utf8_lossy(bytes);
    let mut positions = Vec::new();
    let mut colors: Vec<[u8; 3]> = Vec::new();
    let mut triangles = Vec::new();
    let mut groups = Vec::new();
    let mut group = 0u32;
    let mut named = std::collections::HashMap::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let v: Vec<f64> = parts.filter_map(|s| s.parse().ok()).collect();
                if v.len() < 3 {
                    return Err(io("a vertex needs three coordinates"));
                }
                positions.push(DVec3::new(v[0], v[1], v[2]));
                if v.len() >= 6 {
                    let c = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
                    colors.push([c(v[3]), c(v[4]), c(v[5])]);
                }
            }
            Some("f") => {
                let n = positions.len() as i64;
                let poly = parts
                    .map(|p| {
                        let i: i64 = p
                            .split('/')
                            .next()
                            .and_then(|s| s.parse().ok())
                            .ok_or_else(|| io("a face has a bad vertex index"))?;
                        let i = if i < 0 { n + i } else { i - 1 };
                        if i < 0 || i >= n {
                            return Err(io("a face refers to a missing vertex"));
                        }
                        Ok(i as u32)
                    })
                    .collect::<Result<Vec<_>>>()?;
                let before = triangles.len();
                fan(&poly, &mut triangles);
                groups.extend(std::iter::repeat_n(group, triangles.len() - before));
            }
            Some("g") | Some("o") => {
                let name = parts.collect::<Vec<_>>().join(" ");
                let next = named.len() as u32;
                group = *named.entry(name).or_insert(next);
            }
            _ => {}
        }
    }
    let mut m = TriMesh::new(positions, triangles);
    if !colors.is_empty() && colors.len() == m.positions.len() {
        m.colors = Some(colors);
    }
    if named.len() > 1 {
        m.groups = groups;
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    #[test]
    fn quads_and_negative_indices() {
        let obj = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\nf -4/1 -2 -1\n";
        let m = super::read(obj.as_bytes()).unwrap();
        assert_eq!(m.triangles.len(), 3);
        assert!((m.area() - 1.5).abs() < 1e-12);
    }
}
