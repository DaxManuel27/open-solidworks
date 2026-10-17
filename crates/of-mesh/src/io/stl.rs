//! STL, binary and ASCII.

use glam::DVec3;

use super::io;
use crate::{Result, TriMesh};

pub fn read(bytes: &[u8]) -> Result<TriMesh> {
    // Binary files can start with "solid" too; trust the size when it fits.
    if bytes.len() >= 84 {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        if 84 + n * 50 == bytes.len() {
            return binary(bytes, n);
        }
    }
    if bytes.trim_ascii_start().starts_with(b"solid") {
        return ascii(bytes);
    }
    Err(io("not an STL file"))
}

fn binary(bytes: &[u8], n: usize) -> Result<TriMesh> {
    let mut positions = Vec::with_capacity(n * 3);
    let mut triangles = Vec::with_capacity(n);
    let f = |at: usize| {
        f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as f64
    };
    for t in 0..n {
        let base = 84 + t * 50 + 12;
        for k in 0..3 {
            let at = base + k * 12;
            positions.push(DVec3::new(f(at), f(at + 4), f(at + 8)));
        }
        let i = (t * 3) as u32;
        triangles.push([i, i + 1, i + 2]);
    }
    let mut m = TriMesh::new(positions, triangles);
    m.weld(0.0);
    Ok(m)
}

fn ascii(bytes: &[u8]) -> Result<TriMesh> {
    let text = std::str::from_utf8(bytes).map_err(|_| io("the STL file isn't text"))?;
    let mut positions = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() == Some("vertex") {
            let v: Vec<f64> = parts.filter_map(|s| s.parse().ok()).collect();
            if v.len() != 3 {
                return Err(io("a vertex line needs three numbers"));
            }
            positions.push(DVec3::new(v[0], v[1], v[2]));
        }
    }
    if positions.len() % 3 != 0 {
        return Err(io("the STL file has an incomplete facet"));
    }
    let triangles = (0..positions.len() as u32 / 3)
        .map(|t| [3 * t, 3 * t + 1, 3 * t + 2])
        .collect();
    let mut m = TriMesh::new(positions, triangles);
    m.weld(0.0);
    Ok(m)
}

/// Binary STL.
pub fn write_stl(mesh: &TriMesh) -> Vec<u8> {
    let mut out = vec![0u8; 80];
    out[..18].copy_from_slice(b"OpenForge mesh STL");
    out.extend((mesh.triangles.len() as u32).to_le_bytes());
    for t in 0..mesh.triangles.len() {
        let n = mesh.normal(t);
        for v in std::iter::once(n).chain(mesh.corners(t)) {
            for c in [v.x, v.y, v.z] {
                out.extend((c as f32).to_le_bytes());
            }
        }
        out.extend([0u8, 0]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_round_trip_and_ascii() {
        let c = crate::tests::cube();
        let bytes = write_stl(&c);
        let back = read(&bytes).unwrap();
        assert_eq!(back.positions.len(), 8);
        assert_eq!(back.triangles.len(), 12);
        assert!((back.volume() - 1.0).abs() < 1e-6);
        let ascii = "solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid t\n";
        let m = read(ascii.as_bytes()).unwrap();
        assert_eq!(m.triangles.len(), 1);
        assert!((m.area() - 0.5).abs() < 1e-12);
    }
}
