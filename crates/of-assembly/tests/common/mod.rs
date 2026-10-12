//! Part files for assembly tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use of_assembly::{CompRef, DVec3};
use of_model::geom::{DVec2, Kind, SurfaceKind};
use of_model::sketch::{Sketch, tools};
use of_model::*;

/// A scratch directory removed when dropped.
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!(
            "of-assembly-{name}-{}-{}",
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

/// A block with a through hole of radius `r` at (x, y).
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

/// The stable reference of the face of a part matching a predicate.
pub fn face(doc: &Document, pred: impl Fn(&of_model::geom::FaceInfo) -> bool) -> Ref {
    let mut m = Model::new(doc.clone());
    m.rebuild();
    let state = m.state();
    let shape = &state.body.as_ref().expect("part has a body").shape;
    for i in 0..shape.count(Kind::Face) {
        let info = shape.face_info(i).unwrap();
        if pred(&info) {
            return state.ref_for(Kind::Face, i).unwrap();
        }
    }
    panic!("no matching face");
}

/// A planar face whose outward normal is `n` and that passes through
/// coordinate `at` along it.
pub fn plane_face(doc: &Document, n: DVec3, at: f64) -> Ref {
    face(doc, |f| {
        f.kind == SurfaceKind::Plane
            && f.normal.normalize().dot(n) > 0.999
            && (f.point.dot(n) - at).abs() < 1e-6
    })
}

pub fn cylinder_face(doc: &Document, radius: f64) -> Ref {
    face(doc, |f| {
        f.kind == SurfaceKind::Cylinder && (f.radius - radius).abs() < 1e-6
    })
}

pub fn r(component: &str, entity: Ref) -> CompRef {
    CompRef {
        component: vec![component.to_string()],
        entity,
    }
}

pub fn assert_close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b}");
}

/// A block with a through slot from `c0` to `c1` (centers) of `width`.
pub fn slot_plate_doc(
    w: f64,
    d: f64,
    h: f64,
    c0: (f64, f64),
    c1: (f64, f64),
    width: f64,
) -> Document {
    let mut doc = block_doc(w, d, h);
    let mut sk = Sketch::new();
    tools::straight_slot(
        &mut sk,
        DVec2::new(c0.0, c0.1),
        DVec2::new(c1.0, c1.1),
        width,
    )
    .unwrap();
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(extrude(s, 0.0, true));
    doc
}

/// A plate with a hole at (x, y) repeated `count` times along X by `spacing`
/// with a linear pattern feature. Returns the document and the pattern ID.
pub fn patterned_plate_doc(
    x: f64,
    y: f64,
    r: f64,
    spacing: f64,
    count: u32,
) -> (Document, FeatureId) {
    let mut doc = plate_doc(200.0, 60.0, 10.0, &[(x, y, r)]);
    let cut = doc.features.last().unwrap().id;
    let id = doc.add(FeatureKind::LinearPattern(LinearPattern {
        features: vec![cut],
        direction1: PatternDirection {
            reference: Ref::RightPlane,
            spacing,
            count,
            reverse: false,
        },
        direction2: None,
    }));
    (doc, id)
}
