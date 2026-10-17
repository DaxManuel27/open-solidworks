//! OFF (object file format).

use glam::DVec3;

use super::{fan, io};
use crate::{Result, TriMesh};

pub fn read(bytes: &[u8]) -> Result<TriMesh> {
    let text = String::from_utf8_lossy(bytes);
    let mut tokens = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .flat_map(|l| l.split_whitespace())
        .peekable();
    let head = tokens.next().ok_or_else(|| io("the OFF file is empty"))?;
    let colored = head.starts_with("C");
    let counts: Vec<usize> = if head.ends_with("OFF") {
        (0..3).filter_map(|_| tokens.next()?.parse().ok()).collect()
    } else {
        std::iter::once(head)
            .chain((0..2).filter_map(|_| tokens.next()))
            .filter_map(|s| s.parse().ok())
            .collect()
    };
    if counts.len() != 3 {
        return Err(io("the OFF header needs vertex, face and edge counts"));
    }
    let mut num = || -> Result<f64> {
        tokens
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io("the OFF file ends early"))
    };
    let mut positions = Vec::with_capacity(counts[0]);
    for _ in 0..counts[0] {
        positions.push(DVec3::new(num()?, num()?, num()?));
        if colored {
            for _ in 0..3 {
                num()?;
            }
        }
    }
    let mut triangles = Vec::new();
    for _ in 0..counts[1] {
        let k = num()? as usize;
        let poly = (0..k)
            .map(|_| num().map(|v| v as u32))
            .collect::<Result<Vec<_>>>()?;
        fan(&poly, &mut triangles);
    }
    Ok(TriMesh::new(positions, triangles))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_square() {
        let m = super::read(b"OFF\n4 1 0\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n").unwrap();
        assert_eq!(m.triangles.len(), 2);
    }
}
