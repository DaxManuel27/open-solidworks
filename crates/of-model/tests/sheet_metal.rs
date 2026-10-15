//! Sheet metal: bodies made from flanges, and flat patterns checked against
//! the bend allowance formula.

use std::f64::consts::FRAC_PI_2;

use of_model::sheet_metal::*;
use of_model::{
    geom::{CurveKind, DVec2, DVec3, Kind},
    sketch::{Sketch, tools},
    *,
};

fn params(t: f64, r: f64, k: f64) -> SheetMetalParams {
    SheetMetalParams {
        thickness: t,
        radius: r,
        allowance: Allowance::KFactor { k },
        ..Default::default()
    }
}

/// A sketch of connected lines through `points` on the front plane.
fn polyline(doc: &mut Document, points: &[DVec2]) -> FeatureId {
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

fn rectangle(doc: &mut Document, w: f64, h: f64) -> FeatureId {
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(w, h));
    doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }))
}

fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
    assert!(m.state().body.as_ref().unwrap().shape.is_valid());
}

fn volume(m: &Model) -> f64 {
    let b = m.state().body.as_ref().unwrap();
    b.shape.mass_props().unwrap().volume
}

/// Bounding box sizes, smallest first.
fn sizes(m: &Model) -> [f64; 3] {
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    let d = b.max - b.min;
    let mut v = [d.x, d.y, d.z];
    v.sort_by(f64::total_cmp);
    v
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

/// The edge whose midpoint is nearest `p`.
fn edge_at(m: &Model, p: DVec3) -> Ref {
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

fn flatten(m: &mut Model) -> FeatureId {
    let id = m.doc.add(FeatureKind::FlatPattern(FlatPattern::default()));
    m.rebuild();
    id
}

#[test]
fn open_profile_l_bracket_flattens_to_the_allowance() {
    let (t, r, k) = (2.0, 2.0, 0.44);
    let mut doc = Document::new();
    let s = polyline(
        &mut doc,
        &[DVec2::new(0.0, 30.0), DVec2::ZERO, DVec2::new(50.0, 0.0)],
    );
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, k),
        side: ThicknessSide::Normal,
        end1: Some(EndCondition::Blind { depth: 20.0 }),
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    // The sketch is the outside of the bend: legs 30 and 50 outside, 20 deep.
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.x - b.min.x, 50.0, 1e-6);
    close(b.max.y - b.min.y, 30.0, 1e-6);
    close(b.max.z - b.min.z, 20.0, 1e-6);
    assert_eq!(m.state().sheet_metal.len(), 1);
    assert_eq!(m.state().sheet_metal[0].bends.len(), 1);
    let formed = volume(&m);

    flatten(&mut m);
    ok(&m);
    let setback = r + t;
    let ba = FRAC_PI_2 * (r + k * t);
    let length = (30.0 - setback) + (50.0 - setback) + ba;
    let [a, b, c] = sizes(&m);
    close(a, t, 1e-6);
    close(b, 20.0, 1e-6);
    close(c, length, 1e-6);
    close(volume(&m), length * 20.0 * t, 1e-6);
    // The neutral fibre keeps its length, so the volume is unchanged for k = 0.5
    // only; here it differs by the stretch of the bend.
    assert!((volume(&m) - formed).abs() < 20.0);
    let sm = &m.state().sheet_metal[0];
    assert_eq!(sm.bend_lines.len(), 1);
}

#[test]
fn edge_flange_on_a_tab() {
    let (t, r, k) = (1.5, 1.5, 0.44);
    let mut doc = Document::new();
    let s = rectangle(&mut doc, 60.0, 40.0);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, k),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    let edge = edge_at(&m, DVec3::new(30.0, 40.0, t));
    m.doc.add(FeatureKind::EdgeFlange(EdgeFlange {
        edges: vec![edge],
        angle: FRAC_PI_2,
        length: FlangeLength::Blind { length: 20.0 },
        measure: LengthMeasure::OuterVirtualSharp,
        position: FlangePosition::MaterialInside,
        reverse: false,
        gap: 0.1,
        start_offset: 0.0,
        end_offset: 0.0,
        position_offset: 0.0,
        bend: BendOverride::default(),
    }));
    m.rebuild();
    ok(&m);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.y, 40.0, 1e-6);
    close(b.max.z, 20.0, 1e-6);
    close(b.min.z, 0.0, 1e-6);
    assert_eq!(m.state().sheet_metal[0].bends.len(), 1);

    flatten(&mut m);
    ok(&m);
    let ba = FRAC_PI_2 * (r + k * t);
    let length = (40.0 - (r + t)) + ba + (20.0 - (r + t));
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.min.z, 0.0, 1e-6);
    close(b.max.z, t, 1e-6);
    close(b.max.x - b.min.x, 60.0, 1e-6);
    close(b.max.y - b.min.y, length, 1e-6);
}

#[test]
fn neutral_k_factor_keeps_volume() {
    let (t, r, k) = (2.0, 3.0, 0.5);
    let mut doc = Document::new();
    let s = polyline(
        &mut doc,
        &[
            DVec2::new(0.0, 30.0),
            DVec2::ZERO,
            DVec2::new(50.0, 0.0),
            DVec2::new(50.0, 20.0),
        ],
    );
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, k),
        side: ThicknessSide::Reverse,
        end1: Some(EndCondition::Blind { depth: 30.0 }),
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    assert_eq!(m.state().sheet_metal[0].bends.len(), 2);
    let formed = volume(&m);
    let flat = flatten(&mut m);
    ok(&m);
    // With k = 0.5 the neutral surface is the middle of the sheet, so the flat
    // keeps the formed volume.
    close(volume(&m), formed, 1e-6 * formed);
    assert_eq!(m.state().sheet_metal[0].bend_lines.len(), 2);
    m.doc.feature_mut(flat).unwrap().suppressed = true;
    m.rebuild();
    close(volume(&m), formed, 1e-9 * formed);
}

#[test]
fn hole_across_a_tangent_line_unfolds() {
    let (t, r, k) = (2.0, 2.0, 0.44);
    let mut doc = Document::new();
    let s = polyline(
        &mut doc,
        &[DVec2::new(0.0, 30.0), DVec2::ZERO, DVec2::new(50.0, 0.0)],
    );
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, k),
        side: ThicknessSide::Normal,
        end1: Some(EndCondition::Blind { depth: 30.0 }),
        end2: None,
        reverse: false,
    }));
    // A Ø4 hole through the upright leg at y = 5, reaching into the bend
    // (which runs from y = 4 down).
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::new(-15.0, 5.0), 2.0);
    let hole = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::RightPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: hole,
        cut: true,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: 5.0 },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    flatten(&mut m);
    ok(&m);
    let [a, b, c] = sizes(&m);
    let length = (30.0 - (r + t)) + (50.0 - (r + t)) + FRAC_PI_2 * (r + k * t);
    close(a, t, 1e-6);
    close(b, 30.0, 1e-6);
    close(c, length, 1e-6);
    let full = length * 30.0 * t;
    let v = volume(&m);
    assert!(v < full - 10.0 && v > full - 40.0, "{v} vs {full}");
    // The hole's wall keeps the name the cut gave it.
    let names = &m.state().body.as_ref().unwrap().names;
    assert!(
        names.faces.iter().flatten().any(|n| n.starts_with("4.s")),
        "{:?}",
        names.faces
    );
}
