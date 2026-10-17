//! Replacing a face of a solid with another surface, the operation behind
//! move, offset and replace face.
//!
//! The face's neighbours are extended, and the closed regions between the old
//! face, the new surface and the extended neighbours are found. Regions outside
//! the solid are added and regions inside are removed, so the neighbours grow
//! or shrink to meet the new surface.

use crate::{BoolOp, HistoryEntry, Kind, OpResult, Relation, Result, Shape};

/// Chain two histories: `a` maps the input to an intermediate shape, `b` maps
/// that shape to the result. Only input 0's entries of `b` are followed.
pub fn compose(a: &[HistoryEntry], b: &[HistoryEntry]) -> Vec<HistoryEntry> {
    let mut out = Vec::new();
    for x in a {
        for y in b {
            if y.input == 0 && y.from_kind == x.to_kind && y.from == x.to {
                let rel = match (x.rel, y.rel) {
                    (Relation::Same, r) => r,
                    (r, _) => r,
                };
                out.push(HistoryEntry {
                    input: x.input,
                    from_kind: x.from_kind,
                    from: x.from,
                    rel,
                    to_kind: y.to_kind,
                    to: y.to,
                });
            }
        }
    }
    out
}

/// History that maps every face, edge and vertex of `s` to itself.
fn identity(s: &Shape) -> Vec<HistoryEntry> {
    [Kind::Face, Kind::Edge, Kind::Vertex]
        .iter()
        .flat_map(|&k| {
            (0..s.count(k)).map(move |i| HistoryEntry {
                input: 0,
                from_kind: k,
                from: i,
                rel: Relation::Same,
                to_kind: k,
                to: i,
            })
        })
        .collect()
}

/// The result of replacing a face: the new solid, its history from the input,
/// and the indices of the faces that lie on the new surface.
pub struct Replaced {
    pub result: OpResult,
    pub new_faces: Vec<u32>,
}

impl Shape {
    /// Replace face `face` of this solid with `surface` (a face, or a shell
    /// that covers the face's boundary). Neighbouring faces are extended or
    /// trimmed to meet it.
    pub fn replace_face(&self, face: u32, surface: &Shape) -> Result<Replaced> {
        let old = self.sub_shape(Kind::Face, face)?;
        let size = self
            .bbox()
            .map(|b| b.diagonal())
            .unwrap_or(100.0)
            .max(surface.bbox().map(|b| b.diagonal()).unwrap_or(0.0));
        let reach = size * 2.0;
        // Faces that share an edge with the face, extended.
        let edges = self.face_edges();
        let edge_faces = self.edge_faces();
        let mut neighbours: Vec<u32> = edges[face as usize]
            .iter()
            .flat_map(|e| edge_faces[*e as usize].iter().copied())
            .filter(|f| *f != face)
            .collect();
        neighbours.sort_unstable();
        neighbours.dedup();
        if neighbours.is_empty() {
            return Err(crate::GeomError(
                "the face has no neighbours to extend".into(),
            ));
        }
        let mut faces = vec![old.clone(), surface.clone()];
        for n in &neighbours {
            faces.push(self.sub_shape(Kind::Face, *n)?.extend_face(reach)?);
        }
        let cells = Shape::compound(&faces).cells()?;
        let tol = 1e-6 * size.max(1.0);
        let mut add = Vec::new();
        let mut cut = Vec::new();
        for cell in cells {
            if cell.faces_on(&old, tol)?.is_empty() || cell.faces_on(surface, tol)?.is_empty() {
                continue;
            }
            let p = cell.inner_point()?;
            match self.classify(p)? {
                1 => cut.push(cell),
                -1 => add.push(cell),
                _ => {}
            }
        }
        if add.is_empty() && cut.is_empty() {
            return Err(crate::GeomError(
                "the neighbouring faces don't meet the new surface".into(),
            ));
        }
        let mut shape = self.clone();
        let mut history = identity(self);
        for (op, tools) in [(BoolOp::Fuse, add), (BoolOp::Cut, cut)] {
            if tools.is_empty() {
                continue;
            }
            let r = shape.boolean(op, &Shape::compound(&tools))?;
            if r.shape.count(Kind::Solid) == 0 {
                return Err(crate::GeomError(
                    "moving the face removes the whole body".into(),
                ));
            }
            history = compose(&history, &r.history);
            shape = r.shape;
        }
        if shape.count(Kind::Solid) != self.count(Kind::Solid) {
            return Err(crate::GeomError(
                "moving the face splits the body or merges it with another".into(),
            ));
        }
        let new_faces = shape.faces_on(surface, tol)?;
        Ok(Replaced {
            result: OpResult { shape, history },
            new_faces,
        })
    }
}
