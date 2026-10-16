//! Options added to delivered sheet metal features: edge flange angles from
//! faces and wall outlines, flat pattern options, hem miters, 3-bend corner
//! reliefs and the remaining corner, tab and gusset settings.

mod sheet_common;

use of_model::sheet_metal::*;
use of_model::{geom::DVec3, *};
use sheet_common::*;

fn plate(w: f64, h: f64, t: f64) -> Model {
    let mut doc = Document::new();
    let s = rectangle(&mut doc, w, h);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, 1.0, 0.5),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    m
}

fn with(m: &Model, kind: FeatureKind) -> Model {
    let mut m = Model::new(m.doc.clone());
    m.doc.add(kind);
    m.rebuild();
    m
}

fn errors(m: &Model) -> Vec<String> {
    m.statuses().into_iter().filter_map(|s| s.error).collect()
}

#[test]
fn flange_angle_from_a_face() {
    let t = 1.0;
    let m = plate(60.0, 40.0, t);
    let edge = edge_at(&m, DVec3::new(30.0, 40.0, t));
    let plain = with(
        &m,
        FeatureKind::EdgeFlange(flange(vec![edge.clone()], 15.0)),
    );
    ok(&plain);
    // Square to the front plane (the sheet's own plane) is a 90° flange.
    let mut f = flange(vec![edge.clone()], 15.0);
    f.angle = 0.3; // ignored
    f.angle_from = Some(AngleFrom {
        face: Ref::FrontPlane,
        parallel: false,
    });
    let square = with(&m, FeatureKind::EdgeFlange(f.clone()));
    ok(&square);
    close(volume(&square), volume(&plain), 1e-6);
    // Parallel to the sheet's plane would be flat; square to the edge fails.
    f.angle_from = Some(AngleFrom {
        face: Ref::FrontPlane,
        parallel: true,
    });
    assert!(!errors(&with(&m, FeatureKind::EdgeFlange(f.clone()))).is_empty());
    f.angle_from = Some(AngleFrom {
        face: Ref::RightPlane,
        parallel: true,
    });
    let e = errors(&with(&m, FeatureKind::EdgeFlange(f)));
    assert!(
        e.iter().any(|e| e.contains("square to the flange's edge")),
        "{e:?}"
    );
}

#[test]
fn flange_wall_outline() {
    let t = 1.0;
    let m = plate(60.0, 40.0, t);
    let edge = edge_at(&m, DVec3::new(30.0, 40.0, t));
    // A rectangle 40 wide and 20 high above the bend...
    let mut rect = flange(vec![edge.clone()], 20.0);
    rect.measure = LengthMeasure::TangentToBend;
    rect.start_offset = 10.0;
    rect.end_offset = 10.0;
    let rect = with(&m, FeatureKind::EdgeFlange(rect));
    ok(&rect);
    // ...and a trapezoid on the same base, 20 narrower at the top.
    let mut trap = flange(vec![edge], 99.0);
    trap.profiles = vec![FlangeProfile {
        edge: 0,
        points: vec![[10.0, 0.0], [50.0, 0.0], [40.0, 20.0], [20.0, 20.0]],
    }];
    let trap = with(&m, FeatureKind::EdgeFlange(trap));
    ok(&trap);
    close(volume(&rect) - volume(&trap), 20.0 * 20.0 / 2.0 * t, 1e-6);
    // The flat still unfolds.
    let mut trap = trap;
    flatten(&mut trap);
    ok(&trap);
}

fn l_bracket(t: f64, r: f64) -> Model {
    let mut doc = Document::new();
    let s = polyline(
        &mut doc,
        &[
            of_model::geom::DVec2::new(0.0, 30.0),
            of_model::geom::DVec2::ZERO,
            of_model::geom::DVec2::new(50.0, 0.0),
        ],
    );
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, 0.44),
        side: ThicknessSide::Normal,
        end1: Some(EndCondition::Blind { depth: 20.0 }),
        end2: None,
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    m
}

fn flat_with(m: &Model, p: FlatPattern) -> Model {
    let m = with(m, FeatureKind::FlatPattern(p));
    ok(&m);
    m
}

fn edge_kinds(m: &Model, kind: of_model::geom::CurveKind) -> usize {
    let shape = &m.state().body.as_ref().unwrap().shape;
    (0..shape.count(of_model::geom::Kind::Edge))
        .filter(|e| shape.edge_info(*e).unwrap().kind == kind)
        .count()
}

#[test]
fn flat_pattern_splits_bend_regions() {
    let m = l_bracket(2.0, 2.0);
    let merged = flat_with(&m, FlatPattern::default());
    let split = flat_with(
        &m,
        FlatPattern {
            split_bend_regions: true,
            ..Default::default()
        },
    );
    close(volume(&split), volume(&merged), 1e-6);
    let faces = |m: &Model| {
        m.state()
            .body
            .as_ref()
            .unwrap()
            .shape
            .count(of_model::geom::Kind::Face)
    };
    // Front and back each split in three; still one solid.
    assert_eq!(faces(&split), faces(&merged) + 4);
    let solids = split
        .state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .count(of_model::geom::Kind::Solid);
    assert_eq!(solids, 1);
}

#[test]
fn flat_pattern_simplifies_bends_and_excludes_faces() {
    let mut m = l_bracket(2.0, 2.0);
    // A circle cut down through the bend (x 0..4) and the long leg; top
    // plane sketch X/Y are model X/-Z.
    let mut sk = of_model::sketch::Sketch::new();
    sk.add_circle_at(of_model::geom::DVec2::new(3.0, -10.0), 2.0);
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::TopPlane,
        distance: -10.0,
        flip: false,
    }));
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::Feature { id: plane },
        sketch: sk,
    }));
    m.doc.add(FeatureKind::NormalCut(NormalCut { sketch: s }));
    m.rebuild();
    ok(&m);
    let plain = flat_with(&m, FlatPattern::default());
    let simple = flat_with(
        &m,
        FlatPattern {
            simplify_bends: true,
            ..Default::default()
        },
    );
    use of_model::geom::CurveKind;
    assert!(edge_kinds(&plain, CurveKind::Other) > 0);
    assert_eq!(edge_kinds(&simple, CurveKind::Other), 0);

    // Leaving out the 30 mm leg's wall removes its 26 × 20 region.
    let m = l_bracket(2.0, 2.0);
    let shape = &m.state().body.as_ref().unwrap().shape;
    let wall = (0..shape.count(of_model::geom::Kind::Face))
        .find(|f| {
            let i = shape.face_info(*f).unwrap();
            i.kind == of_model::geom::SurfaceKind::Plane
                && i.normal.x.abs() > 1.0 - 1e-9
                && i.centroid.y > 10.0
                && (i.area - 26.0 * 20.0).abs() < 1e-6
        })
        .expect("leg wall");
    let wall = m.state().ref_for(of_model::geom::Kind::Face, wall).unwrap();
    let plain = flat_with(&m, FlatPattern::default());
    let cut = flat_with(
        &m,
        FlatPattern {
            exclude: vec![wall],
            ..Default::default()
        },
    );
    close(volume(&plain) - volume(&cut), 26.0 * 20.0 * 2.0, 1e-3);
}

#[test]
fn flat_pattern_corner_treatment_rounds_inside_corners() {
    // An L-shaped plate: one inside corner at (30, 20).
    let mut doc = Document::new();
    let mut sk = of_model::sketch::Sketch::new();
    let pts: Vec<_> = [
        (0.0, 0.0),
        (60.0, 0.0),
        (60.0, 20.0),
        (30.0, 20.0),
        (30.0, 40.0),
        (0.0, 40.0),
    ]
    .iter()
    .map(|(x, y)| sk.add_point(of_model::geom::DVec2::new(*x, *y)))
    .collect();
    for i in 0..pts.len() {
        sk.add_line(pts[i], pts[(i + 1) % pts.len()]).unwrap();
    }
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let t = 2.0;
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, 1.0, 0.5),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    let plain = flat_with(&m, FlatPattern::default());
    let treated = flat_with(
        &m,
        FlatPattern {
            corner_treatment: true,
            ..Default::default()
        },
    );
    // A radius t/2 = 1 in the inside corner adds (1 - π/4) r² of area.
    let added = (1.0 - std::f64::consts::PI / 4.0) * t;
    close(volume(&treated) - volume(&plain), added, 1e-6);
}

#[test]
fn hems_meeting_at_a_corner_are_mitered() {
    let t = 1.0;
    let m = plate(60.0, 40.0, t);
    let top = edge_at(&m, DVec3::new(30.0, 40.0, t));
    let side = edge_at(&m, DVec3::new(60.0, 20.0, t));
    let hem = |edges: Vec<Ref>| {
        FeatureKind::Hem(Hem {
            edges,
            kind: HemKind::Open { gap: 1.0 },
            length: 8.0,
            position: HemPosition::MaterialInside,
            reverse: false,
            gap: 0.3,
            allowance: None,
        })
    };
    let base = volume(&m);
    let a = with(&m, hem(vec![top.clone()]));
    let b = with(&m, hem(vec![side.clone()]));
    let both = with(&m, hem(vec![top, side]));
    ok(&both);
    // Each hem loses a mitered corner where they meet.
    let separate = (volume(&a) - base) + (volume(&b) - base);
    assert!(volume(&both) - base < separate - 0.1);
    let mut both = both;
    flatten(&mut both);
    ok(&both);
}
