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

#[test]
fn swept_flange_rolls_out_along_its_path() {
    let (t, h) = (1.0, 15.0);
    let mut doc = Document::new();
    // Path: 40 along x, a quarter turn of radius 20 to the left, 40 along y.
    let mut path = Sketch::new();
    let p0 = path.add_point(DVec2::ZERO);
    let p1 = path.add_point(DVec2::new(40.0, 0.0));
    let c = path.add_point(DVec2::new(40.0, 20.0));
    let p2 = path.add_point(DVec2::new(60.0, 20.0));
    let p3 = path.add_point(DVec2::new(60.0, 60.0));
    path.add_line(p0, p1).unwrap();
    path.add_arc(c, p1, p2).unwrap();
    path.add_line(p2, p3).unwrap();
    let path = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: path,
    }));
    // Profile: straight up h from the path start, on the right plane.
    let mut profile = Sketch::new();
    profile.add_line_at(DVec2::ZERO, DVec2::new(-h, 0.0));
    let profile = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::RightPlane,
        sketch: profile,
    }));
    doc.add(FeatureKind::SweptFlange(SweptFlange {
        profile,
        path: Ref::Feature { id: path },
        sheet: Some(params(t, 1.0, 0.5)),
        side: ThicknessSide::Normal,
        bend: BendOverride::default(),
    }));
    let mut m = Model::new(doc);
    ok(&m);
    let formed = volume(&m);
    flatten(&mut m);
    ok(&m);
    let radius = 20.0;
    let length = 40.0 + FRAC_PI_2 * (radius + 0.5 * t) + 40.0;
    let [a, b, c] = sizes(&m);
    close(a, t, 1e-6);
    close(b, h, 1e-6);
    close(c, length, 1e-6);
    close(volume(&m), formed, 1e-6 * formed);
}

/// An open square of side `s` with its seam in the middle of one side, on a
/// plane `z` up.
fn square_profile(doc: &mut Document, s: f64) -> FeatureId {
    let h = s / 2.0;
    polyline(
        doc,
        &[
            DVec2::new(1.0, -h),
            DVec2::new(h, -h),
            DVec2::new(h, h),
            DVec2::new(-h, h),
            DVec2::new(-h, -h),
            DVec2::new(-1.0, -h),
        ],
    )
}

fn round_profile(doc: &mut Document, r: f64, z: f64) -> FeatureId {
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: z,
        flip: false,
    }));
    let mut sk = Sketch::new();
    let gap = 1.0 / r;
    let c = sk.add_point(DVec2::ZERO);
    let start = -FRAC_PI_2 + gap;
    let end = -FRAC_PI_2 - gap;
    let a = sk.add_point(DVec2::from_angle(start) * r);
    let b = sk.add_point(DVec2::from_angle(end) * r);
    sk.add_arc(c, a, b).unwrap();
    doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::Feature { id: plane },
        sketch: sk,
    }))
}

#[test]
fn lofted_bend_square_to_round_unfolds() {
    for method in [LoftMethod::Bent, LoftMethod::Formed] {
        let mut doc = Document::new();
        let square = square_profile(&mut doc, 40.0);
        let round = round_profile(&mut doc, 15.0, 30.0);
        doc.add(FeatureKind::LoftedBend(LoftedBend {
            profile1: square,
            profile2: round,
            method,
            facets: 4,
            deviation: 0.1,
            sheet: params(1.0, 1.0, 0.5),
            reverse: false,
        }));
        let mut m = Model::new(doc);
        ok(&m);
        let formed = volume(&m);
        flatten(&mut m);
        ok(&m);
        close(thickness_of(&m), 1.0, 1e-6);
        // Facets unfold without stretching: the flat holds about the same
        // material (the offset side of a facet strip is only nearly parallel).
        assert!(
            (volume(&m) - formed).abs() < 0.03 * formed,
            "{} vs {formed}",
            volume(&m)
        );
        assert!(!m.state().sheet_metal[0].bend_lines.is_empty());
    }
}

/// The body's extent along the normal of its largest face.
fn thickness_of(m: &Model) -> f64 {
    let shape = &m.state().body.as_ref().unwrap().shape;
    let big = (0..shape.count(Kind::Face))
        .max_by(|a, b| {
            let fa = shape.face_info(*a).unwrap().area;
            let fb = shape.face_info(*b).unwrap().area;
            fa.total_cmp(&fb)
        })
        .unwrap();
    let n = shape.face_info(big).unwrap().normal;
    let d: Vec<f64> = (0..shape.count(Kind::Vertex))
        .map(|v| shape.vertex_point(v).unwrap().dot(n))
        .collect();
    d.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - d.iter().copied().fold(f64::INFINITY, f64::min)
}
