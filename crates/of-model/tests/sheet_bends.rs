//! Hems, miter flanges, sketched bends, jogs and cross breaks.

mod sheet_common;

use std::f64::consts::{FRAC_PI_2, PI};

use of_model::sheet_metal::*;
use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::Sketch,
    *,
};
use sheet_common::*;

fn plate(w: f64, h: f64, t: f64, r: f64, k: f64) -> Model {
    let mut doc = Document::new();
    let s = rectangle(&mut doc, w, h);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, k),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    m
}

fn hem(m: &mut Model, kind: HemKind, length: f64) {
    let t = m.state().sheet_metal[0].params.thickness;
    let e = edge_at(m, DVec3::new(30.0, 40.0, t));
    m.doc.add(FeatureKind::Hem(Hem {
        edges: vec![e],
        kind,
        length,
        position: HemPosition::MaterialInside,
        reverse: false,
        gap: 0.1,
        allowance: None,
    }));
    m.rebuild();
    ok(m);
}

#[test]
fn open_hem_flattens_to_its_length() {
    let (t, k, gap, length) = (1.0, 0.5, 2.0, 8.0);
    let mut m = plate(60.0, 40.0, t, 1.0, k);
    hem(&mut m, HemKind::Open { gap }, length);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.y, 40.0, 1e-6);
    close(b.max.z, gap + 2.0 * t, 1e-6);
    let formed = volume(&m);
    flatten(&mut m);
    ok(&m);
    // Plate up to the fold, the fold at the neutral radius, then the leg.
    let r = gap / 2.0;
    let flat = (40.0 - (r + t)) + PI * (r + k * t) + length;
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.y - b.min.y, flat, 1e-6);
    close(volume(&m), formed, 1e-6 * formed);
}

#[test]
fn every_hem_kind_builds_and_flattens() {
    for kind in [
        HemKind::Closed,
        HemKind::Double { gap: 0.5 },
        HemKind::TearDrop {
            angle: 220f64.to_radians(),
            radius: 2.0,
        },
        HemKind::Rolled {
            angle: 270f64.to_radians(),
            radius: 3.0,
        },
    ] {
        let mut m = plate(60.0, 40.0, 1.0, 1.0, 0.44);
        hem(&mut m, kind.clone(), 6.0);
        flatten(&mut m);
        ok(&m);
        let [a, _, _] = sizes(&m);
        close(a, 1.0, 1e-6);
    }
}

/// A sketch of one line on the plate's top face at height `z`.
fn line_on_top(m: &mut Model, a: DVec2, b: DVec2) -> FeatureId {
    let top = {
        let shape = &m.state().body.as_ref().unwrap().shape;
        let i = (0..shape.count(Kind::Face))
            .find(|i| shape.face_info(*i).unwrap().normal.z > 0.99)
            .unwrap();
        m.state().ref_for(Kind::Face, i).unwrap()
    };
    let frame = m.resolve(&top).unwrap().plane().unwrap();
    let mut sk = Sketch::new();
    let to = |p: DVec2| frame.to_local(DVec3::new(p.x, p.y, frame.origin.z));
    sk.add_line_at(to(a), to(b));
    let id = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: top,
        sketch: sk,
    }));
    m.rebuild();
    id
}

#[test]
fn sketched_bend_keeps_the_flat() {
    let (t, r, k) = (2.0, 2.0, 0.44);
    let mut m = plate(80.0, 40.0, t, r, k);
    let before = volume(&m);
    let s = line_on_top(&mut m, DVec2::new(50.0, -5.0), DVec2::new(50.0, 45.0));
    m.doc.add(FeatureKind::SketchedBend(SketchedBend {
        sketch: s,
        fixed: None,
        angle: FRAC_PI_2,
        position: BendPosition::Centerline,
        reverse: false,
        bend: BendOverride::default(),
    }));
    m.rebuild();
    ok(&m);
    // The smaller side (x > 50) stands up.
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    assert!(b.max.z > 20.0, "{b:?}");
    assert!(b.max.x < 60.0, "{b:?}");
    flatten(&mut m);
    ok(&m);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.x - b.min.x, 80.0, 1e-6);
    close(b.max.y - b.min.y, 40.0, 1e-6);
    close(volume(&m), before, 1e-6 * before);
}

#[test]
fn jog_offsets_the_far_side() {
    let (t, r, k) = (1.5, 1.5, 0.44);
    for fix in [true, false] {
        let mut m = plate(100.0, 40.0, t, r, k);
        let s = line_on_top(&mut m, DVec2::new(60.0, -5.0), DVec2::new(60.0, 45.0));
        m.doc.add(FeatureKind::Jog(Jog {
            sketch: s,
            fixed: None,
            offset: 10.0,
            dimension: JogDimension::Outside,
            angle: FRAC_PI_2,
            position: BendPosition::BendOutside,
            fix_projected_length: fix,
            reverse: false,
            bend: BendOverride::default(),
        }));
        m.rebuild();
        ok(&m);
        assert_eq!(m.state().sheet_metal[0].bends.len(), 2);
        let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
        close(b.max.z, t + 10.0, 1e-6);
        let projected = 2.0 * r + t;
        let flat = 2.0 * FRAC_PI_2 * (r + k * t) + (10.0 - projected);
        let expected = if fix {
            100.0
        } else {
            100.0 - (flat - projected)
        };
        close(b.max.x, expected, 1e-6);
        flatten(&mut m);
        ok(&m);
        let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
        if !fix {
            close(b.max.x - b.min.x, 100.0, 1e-6);
        }
    }
}

#[test]
fn miter_flange_runs_round_a_corner() {
    let (t, r) = (1.0, 1.0);
    let mut m = plate(60.0, 40.0, t, r, 0.5);
    // Profile on a plane square to the edge y = 40 at x = 0: up 10, out 5.
    let e1 = edge_at(&m, DVec3::new(30.0, 40.0, t));
    let e2 = edge_at(&m, DVec3::new(60.0, 20.0, t));
    let mut sk = Sketch::new();
    // The right plane's sketch coordinates are (-z, y).
    let p = [
        DVec2::new(-t, 40.0),
        DVec2::new(-(t + 10.0), 40.0),
        DVec2::new(-(t + 10.0), 45.0),
    ];
    let ids: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    sk.add_line(ids[0], ids[1]).unwrap();
    sk.add_line(ids[1], ids[2]).unwrap();
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::RightPlane,
        sketch: sk,
    }));
    m.doc.add(FeatureKind::MiterFlange(MiterFlange {
        sketch: s,
        edges: vec![e1, e2],
        position: FlangePosition::MaterialInside,
        gap: 0.2,
        start_offset: 0.0,
        end_offset: 0.0,
        bend: BendOverride::default(),
    }));
    m.rebuild();
    ok(&m);
    // Two bends per edge: sheet to riser, riser to lip.
    assert_eq!(m.state().sheet_metal[0].bends.len(), 4);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.z, t + 10.0, 1e-6);
    flatten(&mut m);
    ok(&m);
}

#[test]
fn cross_break_is_recorded() {
    let mut m = plate(60.0, 40.0, 1.0, 1.0, 0.5);
    let top = {
        let shape = &m.state().body.as_ref().unwrap().shape;
        let i = (0..shape.count(Kind::Face))
            .find(|i| shape.face_info(*i).unwrap().normal.z > 0.99)
            .unwrap();
        m.state().ref_for(Kind::Face, i).unwrap()
    };
    let v = volume(&m);
    m.doc.add(FeatureKind::CrossBreak(CrossBreak {
        face: top,
        angle: 3f64.to_radians(),
        radius: 1.0,
        reverse: false,
    }));
    m.rebuild();
    ok(&m);
    assert_eq!(m.state().sheet_metal[0].cross_breaks.len(), 1);
    close(volume(&m), v, 1e-9);
}
