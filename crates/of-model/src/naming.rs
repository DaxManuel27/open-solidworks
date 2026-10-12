//! Topological naming: stable names for faces, edges and vertices.
//!
//! Faces are named by the feature and sketch entity that created them, for
//! example `7.s12` (the side face Extrude 7 swept from sketch curve 12),
//! `7.end` (its end cap) or `9.f(E(7.end|7.s12))` (the face Fillet 9 made
//! from that edge). Kernel history carries names through later operations: a
//! face that is trimmed or split keeps its name, and split pieces get `~1`,
//! `~2` suffixes in a stable order.
//!
//! Edges and vertices are named by the faces that meet there, e.g.
//! `E(7.end|7.s12)`, so they follow their faces. Resolving a name falls back
//! gracefully when splits add or remove suffixes.

use std::collections::BTreeMap;

use of_geom::{DVec3, HistoryEntry, Kind, Relation, Shape};

/// Names for every face, edge and vertex of one shape, by kernel index.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Naming {
    /// Each face's names; the first is its primary name, the rest aliases
    /// from faces that were merged into it.
    pub faces: Vec<Vec<String>>,
    pub edges: Vec<String>,
    pub vertices: Vec<String>,
    edge_faces: Vec<Vec<u32>>,
    vertex_faces: Vec<Vec<u32>>,
}

/// Where a face's name comes from in an operation.
pub(crate) struct Inputs<'a> {
    /// Naming of input 0 and input 1 (a boolean's tool), if they're named.
    pub named: [Option<&'a Naming>; 2],
}

fn key(p: DVec3) -> (i64, i64, i64) {
    let r = |v: f64| (v * 1e4).round() as i64;
    (r(p.x), r(p.y), r(p.z))
}

impl Naming {
    /// Name the result of an operation from its history. Same/modified faces
    /// inherit names from named inputs; everything else asks `generate`.
    /// Faces still unnamed get `{fallback}.u`, numbered.
    pub(crate) fn from_history(
        shape: &Shape,
        history: &[HistoryEntry],
        inputs: Inputs,
        generate: &mut dyn FnMut(&HistoryEntry) -> Option<String>,
        fallback: &str,
    ) -> Naming {
        let nfaces = shape.count(Kind::Face) as usize;
        let mut faces: Vec<Vec<String>> = vec![Vec::new(); nfaces];
        // Inherited names first (input 0, then 1), then generated ones.
        let mut ordered: Vec<&HistoryEntry> =
            history.iter().filter(|h| h.to_kind == Kind::Face).collect();
        ordered.sort_by_key(|h| {
            let inherited = matches!(h.rel, Relation::Same | Relation::Modified)
                && h.from_kind == Kind::Face
                && inputs.named[h.input as usize].is_some();
            (!inherited, h.input, h.from)
        });
        for h in ordered {
            let to = h.to as usize;
            if to >= nfaces {
                continue;
            }
            let names: Vec<String> = match (h.rel, h.from_kind, inputs.named[h.input as usize]) {
                (Relation::Same | Relation::Modified, Kind::Face, Some(n)) => {
                    n.faces.get(h.from as usize).cloned().unwrap_or_default()
                }
                _ => generate(h).into_iter().collect(),
            };
            for n in names {
                if !faces[to].contains(&n) {
                    faces[to].push(n);
                }
            }
        }
        let mut k = 0;
        for f in faces.iter_mut().filter(|f| f.is_empty()) {
            k += 1;
            f.push(format!("{fallback}.u{k}"));
        }
        Naming::finish(shape, faces)
    }

    /// Name every face of a shape by index (used for imported bodies).
    pub(crate) fn by_index(shape: &Shape, prefix: &str) -> Naming {
        let faces = (0..shape.count(Kind::Face))
            .map(|i| vec![format!("{prefix}.f{i}")])
            .collect();
        Naming::finish(shape, faces)
    }

    /// Give a shape that is a moved copy of a named shape the same names,
    /// prefixed (used for patterns and mirrors).
    pub(crate) fn prefixed(&self, shape: &Shape, history: &[HistoryEntry], prefix: &str) -> Naming {
        let mut faces = vec![Vec::new(); shape.count(Kind::Face) as usize];
        for h in history
            .iter()
            .filter(|h| h.from_kind == Kind::Face && h.to_kind == Kind::Face)
        {
            if let (Some(src), Some(dst)) = (
                self.faces.get(h.from as usize),
                faces.get_mut(h.to as usize),
            ) {
                *dst = src.iter().map(|n| format!("{prefix}{n}")).collect();
            }
        }
        for (i, f) in faces.iter_mut().enumerate() {
            if f.is_empty() {
                f.push(format!("{prefix}u{i}"));
            }
        }
        Naming::finish(shape, faces)
    }

    /// Name a shape from given face names (primary first), making them unique
    /// and deriving edge and vertex names.
    pub(crate) fn from_faces(shape: &Shape, faces: Vec<Vec<String>>) -> Naming {
        Naming::finish(shape, faces)
    }

    /// Make primary face names unique and derive edge and vertex names.
    fn finish(shape: &Shape, mut faces: Vec<Vec<String>>) -> Naming {
        // Split faces share a primary name: number them by position.
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, f) in faces.iter().enumerate() {
            groups.entry(f[0].clone()).or_default().push(i);
        }
        for (name, idx) in groups {
            if idx.len() < 2 {
                continue;
            }
            let mut keyed: Vec<((i64, i64, i64), usize)> = idx
                .iter()
                .map(|&i| {
                    let c = shape
                        .face_info(i as u32)
                        .map(|f| f.centroid)
                        .unwrap_or_default();
                    (key(c), i)
                })
                .collect();
            keyed.sort();
            for (k, (_, i)) in keyed.into_iter().enumerate() {
                faces[i][0] = format!("{name}~{}", k + 1);
                // The unsuffixed name stays as an alias of the first piece.
                if k == 0 {
                    faces[i].push(name.clone());
                }
            }
        }

        let edge_faces = shape.edge_faces();
        let vertex_faces = shape.vertex_faces();
        let primary = |ids: &[u32]| -> String {
            let mut n: Vec<&str> = ids.iter().map(|&f| faces[f as usize][0].as_str()).collect();
            n.sort_unstable();
            n.dedup();
            n.join("|")
        };
        let mut edges: Vec<String> = edge_faces
            .iter()
            .map(|f| format!("E({})", primary(f)))
            .collect();
        number_duplicates(&mut edges, |i| {
            shape
                .edge_info(i as u32)
                .map(|e| key(e.mid))
                .unwrap_or_default()
        });
        let mut vertices: Vec<String> = vertex_faces
            .iter()
            .map(|f| format!("V({})", primary(f)))
            .collect();
        number_duplicates(&mut vertices, |i| {
            shape.vertex_point(i as u32).map(key).unwrap_or_default()
        });
        Naming {
            faces,
            edges,
            vertices,
            edge_faces,
            vertex_faces,
        }
    }

    pub fn face_name(&self, index: u32) -> Option<&str> {
        self.faces.get(index as usize).map(|f| f[0].as_str())
    }

    /// Find a face by name, tolerating split suffixes appearing or going away.
    pub fn face(&self, name: &str) -> Option<u32> {
        if let Some(i) = self.faces.iter().position(|f| f.iter().any(|n| n == name)) {
            return Some(i as u32);
        }
        let base = strip_suffix(name);
        let ordinal = suffix_of(name);
        // The face was split since: pick the matching piece, else the first.
        let pieces: Vec<usize> = (0..self.faces.len())
            .filter(|&i| self.faces[i].iter().any(|n| strip_suffix(n) == base))
            .collect();
        if pieces.is_empty() {
            return None;
        }
        let pick = ordinal
            .and_then(|k| pieces.get(k - 1))
            .unwrap_or(&pieces[0]);
        Some(*pick as u32)
    }

    /// Find an edge by name, falling back to the edge between the named faces.
    pub fn edge(&self, name: &str) -> Option<u32> {
        if let Some(i) = self.edges.iter().position(|e| e == name) {
            return Some(i as u32);
        }
        let faces = self.resolve_faces(name, "E(")?;
        let candidates: Vec<usize> = (0..self.edge_faces.len())
            .filter(|&i| {
                let mut f = self.edge_faces[i].clone();
                f.sort_unstable();
                f.dedup();
                f == faces
            })
            .collect();
        let ordinal = suffix_of(name);
        candidates
            .get(ordinal.unwrap_or(1).saturating_sub(1))
            .or(candidates.first())
            .map(|&i| i as u32)
    }

    pub fn vertex(&self, name: &str) -> Option<u32> {
        if let Some(i) = self.vertices.iter().position(|e| e == name) {
            return Some(i as u32);
        }
        let faces = self.resolve_faces(name, "V(")?;
        (0..self.vertex_faces.len())
            .find(|&i| {
                let mut f = self.vertex_faces[i].clone();
                f.sort_unstable();
                f.dedup();
                f == faces
            })
            .map(|i| i as u32)
    }

    /// Face indices named inside `E(a|b)` or `V(a|b|c)`.
    fn resolve_faces(&self, name: &str, prefix: &str) -> Option<Vec<u32>> {
        let inner = name.strip_prefix(prefix)?;
        let inner = &inner[..inner.rfind(')')?];
        let mut out = Vec::new();
        for part in split_top_level(inner) {
            out.push(self.face(part)?);
        }
        out.sort_unstable();
        out.dedup();
        Some(out)
    }
}

/// A short stable hash (FNV-1a, 32 bits as hex) identifying content.
pub(crate) fn content_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", (h ^ (h >> 32)) as u32)
}

/// Append `~k` to names that occur more than once, ordered by `order`.
fn number_duplicates(names: &mut [String], order: impl Fn(usize) -> (i64, i64, i64)) {
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, n) in names.iter().enumerate() {
        groups.entry(n.clone()).or_default().push(i);
    }
    for (name, mut idx) in groups {
        if idx.len() < 2 {
            continue;
        }
        idx.sort_by_key(|&i| order(i));
        for (k, i) in idx.into_iter().enumerate() {
            names[i] = format!("{name}~{}", k + 1);
        }
    }
}

/// Split on `|` outside parentheses.
fn split_top_level(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '|' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// `name~3` → `name`.
fn strip_suffix(name: &str) -> &str {
    match name.rfind('~') {
        Some(i) if name[i + 1..].chars().all(|c| c.is_ascii_digit()) && i + 1 < name.len() => {
            &name[..i]
        }
        _ => name,
    }
}

fn suffix_of(name: &str) -> Option<usize> {
    let i = name.rfind('~')?;
    name[i + 1..].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffix_helpers() {
        assert_eq!(strip_suffix("7.s3~2"), "7.s3");
        assert_eq!(strip_suffix("7.s3"), "7.s3");
        assert_eq!(suffix_of("E(a|b)~2"), Some(2));
        assert_eq!(
            split_top_level("9.f(E(a|b))|7.end"),
            vec!["9.f(E(a|b))", "7.end"]
        );
    }
}
