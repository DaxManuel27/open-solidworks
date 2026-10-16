//! Part and assembly files for drawing tests (builders shared with the
//! `of-assembly` tests).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use of_drawing::graphics::{Item, Owner, P2, Prim};
use of_model::geom::DVec2;
use of_model::sketch::{Sketch, tools};
use of_model::*;

/// A scratch directory removed when dropped.
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!(
            "of-drawing-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn extrude(sketch: FeatureId, depth: f64, cut: bool) -> FeatureKind {
    FeatureKind::Extrude(Extrude {
        sketch,
        cut,
        start_offset: 0.0,
        end1: if cut {
            EndCondition::ThroughAll
        } else {
            EndCondition::Blind { depth }
        },
        reverse: false,
        end2: if cut {
            Some(EndCondition::ThroughAll)
        } else {
            None
        },
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    })
}

/// A block from (0, 0, 0) to (w, d, h).
pub fn block_doc(w: f64, d: f64, h: f64) -> Document {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(w, d));
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(extrude(s, h, false));
    doc
}

/// A block with through holes of radius `r` at (x, y).
pub fn plate_doc(w: f64, d: f64, h: f64, holes: &[(f64, f64, f64)]) -> Document {
    let mut doc = block_doc(w, d, h);
    let mut sk = Sketch::new();
    for (x, y, r) in holes {
        sk.add_circle_at(DVec2::new(*x, *y), *r);
    }
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(extrude(s, 0.0, true));
    doc
}

/// A cylinder of radius `r` along +Z from 0 to `len`.
pub fn shaft_doc(r: f64, len: f64) -> Document {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::ZERO, r);
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(extrude(s, len, false));
    doc
}

pub fn save(doc: &Document, path: &Path) -> PathBuf {
    file::save(doc, path).unwrap();
    path.to_path_buf()
}

pub fn assert_close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b}");
}

/// Items drawn for a view.
pub fn of_view(items: &[Item], view: u32) -> Vec<&Item> {
    items
        .iter()
        .filter(|i| matches!(&i.owner, Owner::View { view: v, .. } if *v == view))
        .collect()
}

/// Box around the end points, arc extremes and polyline points of items.
pub fn extent(items: &[&Item]) -> [P2; 2] {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    let mut add = |p: P2| {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    };
    for i in items {
        match &i.prim {
            Prim::Line { a, b } => {
                add(*a);
                add(*b);
            }
            Prim::Arc {
                center,
                radius,
                start,
                end,
            } => {
                for p in of_drawing::graphics::arc_points(*center, *radius, *start, *end, 0.05) {
                    add(p);
                }
            }
            Prim::Polyline { points, .. } => points.iter().for_each(|p| add(*p)),
            Prim::Fill { loops } => loops.iter().flatten().for_each(|p| add(*p)),
            Prim::Text { at, .. } => add(*at),
        }
    }
    [lo, hi]
}

/// Every text run on a sheet.
pub fn texts(items: &[Item]) -> Vec<&str> {
    items
        .iter()
        .filter_map(|i| match &i.prim {
            Prim::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// Group-code pairs of a DXF file.
pub fn pairs(text: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].trim().parse().unwrap(), c[1].to_string()))
        .collect()
}

/// Entities of one kind: their pairs up to the next entity.
pub fn entities<'a>(p: &'a [(i32, String)], kind: &str) -> Vec<&'a [(i32, String)]> {
    let mut out = Vec::new();
    for (i, (c, v)) in p.iter().enumerate() {
        if *c == 0 && v == kind {
            let end = p[i + 1..]
                .iter()
                .position(|(c, _)| *c == 0)
                .map_or(p.len(), |k| i + 1 + k);
            out.push(&p[i + 1..end]);
        }
    }
    out
}

pub fn get(e: &[(i32, String)], code: i32) -> Option<&str> {
    e.iter().find(|(c, _)| *c == code).map(|(_, v)| v.as_str())
}

pub fn num(e: &[(i32, String)], code: i32) -> f64 {
    get(e, code).unwrap().parse().unwrap()
}
