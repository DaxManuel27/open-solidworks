//! Helpers shared by the sheet metal tests.
#![allow(dead_code)]

use std::f64::consts::FRAC_PI_2;

use of_model::sheet_metal::*;
use of_model::{
    geom::{CurveKind, DVec2, DVec3, Kind},
    sketch::{Sketch, tools},
    *,
};

pub fn params(t: f64, r: f64, k: f64) -> SheetMetalParams {
    SheetMetalParams {
        thickness: t,
        radius: r,
        allowance: Allowance::KFactor { k },
        ..Default::default()
    }
}

/// A sketch of connected lines through `points` on the front plane.
pub fn polyline(doc: &mut Document, points: &[DVec2]) -> FeatureId {
    let mut sk = Sketch::new();
    let ids: Vec<_> = points.iter().map(|p| sk.add_point(*p)).collect();
    for w in ids.windows(2) {
        sk.add_line(w[0], w[1]).unwrap();
    }
    doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }))
}

pub fn rectangle(doc: &mut Document, w: f64, h: f64) -> FeatureId {
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(w, h));
    doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }))
}

pub fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
    assert!(m.state().body.as_ref().unwrap().shape.is_valid());
}

pub fn volume(m: &Model) -> f64 {
    let b = m.state().body.as_ref().unwrap();
    b.shape.mass_props().unwrap().volume
}

/// Bounding box sizes, smallest first.
pub fn sizes(m: &Model) -> [f64; 3] {
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    let d = b.max - b.min;
    let mut v = [d.x, d.y, d.z];
    v.sort_by(f64::total_cmp);
    v
}

pub fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

/// The edge whose midpoint is nearest `p`.
pub fn edge_at(m: &Model, p: DVec3) -> Ref {
    let shape = &m.state().body.as_ref().unwrap().shape;
    let i = (0..shape.count(Kind::Edge))
        .min_by(|a, b| {
            let da = (shape.edge_info(*a).unwrap().mid - p).length();
            let db = (shape.edge_info(*b).unwrap().mid - p).length();
            da.total_cmp(&db)
        })
        .unwrap();
    let info = shape.edge_info(i).unwrap();
    assert_eq!(info.kind, CurveKind::Line);
    assert!((info.mid - p).length() < 1e-6, "no edge at {p}");
    m.state().ref_for(Kind::Edge, i).unwrap()
}

pub fn flatten(m: &mut Model) -> FeatureId {
    let id = m.doc.add(FeatureKind::FlatPattern(FlatPattern::default()));
    m.rebuild();
    id
}

pub fn flange(edges: Vec<Ref>, length: f64) -> EdgeFlange {
    EdgeFlange {
        edges,
        angle: FRAC_PI_2,
        length: FlangeLength::Blind { length },
        measure: LengthMeasure::OuterVirtualSharp,
        position: FlangePosition::MaterialInside,
        reverse: false,
        gap: 0.2,
        start_offset: 0.0,
        end_offset: 0.0,
        position_offset: 0.0,
        bend: BendOverride::default(),
    }
}
