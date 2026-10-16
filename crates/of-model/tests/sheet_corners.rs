//! Corner treatments, notches, tabs and slots, gussets and forming tools.

mod sheet_common;

use std::f64::consts::FRAC_PI_2;

use of_model::sheet_metal::*;
use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::{Sketch, tools},
    *,
};
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

/// A plate with flanges on two adjacent edges meeting at (60, 40).
fn corner_tray() -> Model {
    let t = 1.0;
    let mut m = plate(60.0, 40.0, t);
    let edges = vec![
        edge_at(&m, DVec3::new(30.0, 40.0, t)),
        edge_at(&m, DVec3::new(60.0, 20.0, t)),
    ];
    m.doc.add(FeatureKind::EdgeFlange(flange(edges, 15.0)));
    m.rebuild();
    ok(&m);
    m
}

fn add(m: &mut Model, kind: FeatureKind) {
    m.doc.add(kind);
    m.rebuild();
    ok(m);
}

#[test]
fn corner_reliefs_of_every_kind_cut_and_flatten() {
    for kind in [
        CornerReliefKind::Rectangular,
        CornerReliefKind::Circular,
        CornerReliefKind::Tear,
        CornerReliefKind::Obround,
        CornerReliefKind::ConstantWidth,
        CornerReliefKind::FullRound,
        CornerReliefKind::Suitcase,
    ] {
        let mut m = corner_tray();
        let before = volume(&m);
        add(
            &mut m,
            FeatureKind::CornerRelief(CornerRelief {
                bends: vec![],
                kind,
                ratio: 0.5,
                centered: true,
            }),
        );
        assert!(volume(&m) < before, "{kind:?} cut nothing");
        flatten(&mut m);
        ok(&m);
    }
}

#[test]
fn bend_notches_at_both_ends() {
    let mut m = corner_tray();
    let before = volume(&m);
    add(
        &mut m,
        FeatureKind::BendNotch(BendNotch {
            bends: vec![],
            shape: NotchShape::Rectangular,
            width: 1.0,
            depth: 1.5,
            both_ends: true,
        }),
    );
    assert!(volume(&m) < before);
    flatten(&mut m);
    ok(&m);
}

#[test]
fn break_corners_formed_and_in_the_flat() {
    let mut m = plate(60.0, 40.0, 1.0);
    let before = volume(&m);
    add(
        &mut m,
        FeatureKind::BreakCorner(BreakCorner {
            edges: vec![],
            corner: CornerBreak::Fillet { radius: 5.0 },
            internal_only: false,
        }),
    );
    let lost = 4.0 * (25.0 - std::f64::consts::PI * 25.0 / 4.0);
    close(volume(&m), before - lost, 1e-6);

    let mut m = corner_tray();
    let id = m.doc.add(FeatureKind::FlatPattern(FlatPattern {
        fixed_face: None,
        break_corners: Some(CornerBreak::Chamfer { distance: 2.0 }),
        ..Default::default()
    }));
    m.rebuild();
    ok(&m);
    let chamfered = volume(&m);
    m.doc.feature_mut(id).unwrap().kind = FeatureKind::FlatPattern(FlatPattern::default());
    m.invalidate(id);
    m.rebuild();
    assert!(chamfered < volume(&m));
}

/// Whether the body has a flat face with outward normal `n` on the plane
/// through `p`, beside the corner of `corner_tray` and up the flanges.
fn corner_face(m: &Model, n: DVec3, p: DVec3) -> bool {
    let shape = &m.state().body.as_ref().unwrap().shape;
    (0..shape.count(Kind::Face)).any(|f| {
        let i = shape.face_info(f).unwrap();
        i.kind == of_model::geom::SurfaceKind::Plane
            && i.normal.dot(n) > 1.0 - 1e-9
            && (i.point - p).dot(n).abs() < 1e-6
            && (i.centroid.x - 59.5).abs() < 1.5
            && (i.centroid.y - 39.5).abs() < 1.5
            && i.centroid.z > 2.0
    })
}

#[test]
fn closed_corner_and_weld() {
    // The flange on y = 40 spans y 39..40; the one on x = 60 spans x 59..60.
    let (x, y) = (DVec3::X, DVec3::Y);
    for (kind, ratio) in [
        (ClosedCornerKind::Butt, 1.0),
        (ClosedCornerKind::Overlap, 1.0),
        (ClosedCornerKind::Underlap, 0.5),
    ] {
        let mut m = corner_tray();
        let before = volume(&m);
        // The mitered end of the flange on y = 40, facing the other flange.
        let shape = &m.state().body.as_ref().unwrap().shape;
        let f = (0..shape.count(Kind::Face))
            .filter(|f| {
                let i = shape.face_info(*f).unwrap();
                i.kind == of_model::geom::SurfaceKind::Plane
                    && i.centroid.y > 37.0
                    && i.centroid.x > 55.0
                    && i.centroid.z > 2.0
                    && i.normal.x > 0.3
            })
            .max_by(|a, b| {
                shape
                    .face_info(*a)
                    .unwrap()
                    .area
                    .total_cmp(&shape.face_info(*b).unwrap().area)
            })
            .expect("flange end face");
        let face = m.state().ref_for(Kind::Face, f).unwrap();
        add(
            &mut m,
            FeatureKind::ClosedCorner(ClosedCorner {
                faces: vec![face],
                kind,
                gap: 0.2,
                overlap_ratio: ratio,
            }),
        );
        // A full cover adds the corner column's material; a part cover's two
        // gap lines can cut more than the miter it replaces.
        if ratio == 1.0 {
            assert!(volume(&m) > before, "{kind:?}");
        }
        let at = |x: f64, y: f64| DVec3::new(x, y, 8.0);
        match kind {
            ClosedCornerKind::Butt => {
                assert!(
                    corner_face(&m, x, at(58.8, 0.0)),
                    "end short of the inside face"
                );
                assert!(
                    corner_face(&m, y, at(0.0, 40.0)),
                    "other flange squared across"
                );
            }
            ClosedCornerKind::Overlap => {
                assert!(
                    corner_face(&m, x, at(60.0, 0.0)),
                    "end flush with the outside"
                );
                assert!(
                    corner_face(&m, y, at(0.0, 38.8)),
                    "other flange pulled back"
                );
            }
            ClosedCornerKind::Underlap => {
                assert!(
                    corner_face(&m, y, at(0.0, 39.5)),
                    "other flange covers half"
                );
                assert!(
                    corner_face(&m, x, at(60.0, 0.0)),
                    "the rest closes the corner"
                );
            }
        }
        // A vertical edge of the seam, at the inside of the joint.
        let shape = &m.state().body.as_ref().unwrap().shape;
        let seam = (0..shape.count(Kind::Edge))
            .filter(|e| {
                let i = shape.edge_info(*e).unwrap();
                i.kind == of_model::geom::CurveKind::Line
                    && (i.end - i.start).normalize().z.abs() > 1.0 - 1e-9
                    && i.mid.z > 2.0
            })
            .min_by(|a, b| {
                let d = |e: u32| {
                    let m = shape.edge_info(e).unwrap().mid;
                    (m.x - 59.0).hypot(m.y - 39.0)
                };
                d(*a).total_cmp(&d(*b))
            })
            .expect("seam edge");
        let seam = m.state().ref_for(Kind::Edge, seam).unwrap();
        add(
            &mut m,
            FeatureKind::WeldedCorner(WeldedCorner {
                edge: seam,
                size: 1.0,
                symbol: None,
            }),
        );
        assert_eq!(m.state().sheet_metal[0].welds.len(), 1);
        flatten(&mut m);
        ok(&m);
    }
}

#[test]
fn tabs_go_through_slots() {
    let t = 2.0;
    let mut m = plate(80.0, 40.0, t);
    // An upright plate standing on the base's top face at y = 20..22.
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(10.0, -40.0), DVec2::new(70.0, -t));
    let wall_plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::TopPlane,
        distance: 20.0,
        flip: false,
    }));
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::Feature { id: wall_plane },
        sketch: sk,
    }));
    m.doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, 1.0, 0.5),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    m.rebuild();
    ok(&m);
    assert_eq!(m.state().bodies.len(), 2);
    let edge = edge_at(&m, DVec3::new(40.0, 20.0, t));
    let top = {
        let shape = &m.state().body.as_ref().unwrap().shape;
        let f = (0..shape.count(Kind::Face))
            .find(|f| {
                let i = shape.face_info(*f).unwrap();
                i.normal.z > 0.999 && (i.point.z - t).abs() < 1e-9
            })
            .unwrap();
        m.state().ref_for(Kind::Face, f).unwrap()
    };
    let before = volume(&m);
    add(
        &mut m,
        FeatureKind::TabAndSlot(TabAndSlot {
            edge,
            face: top,
            count: 3,
            spacing: None,
            length: 8.0,
            height: None,
            clearance: 0.1,
        }),
    );
    // Three tabs of 8 × 2 × 2 added; three slots of 8.2 × 2.2 × 2 cut.
    close(
        volume(&m),
        before + 3.0 * 32.0 - 3.0 * 8.2 * 2.2 * 2.0,
        1e-6,
    );
    assert_eq!(m.state().bodies.len(), 2);
}

/// A 90° L bracket from an open profile.
fn bracket() -> Model {
    let mut doc = Document::new();
    let s = polyline(
        &mut doc,
        &[DVec2::new(0.0, 30.0), DVec2::ZERO, DVec2::new(50.0, 0.0)],
    );
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(2.0, 2.0, 0.44),
        side: ThicknessSide::Normal,
        end1: Some(EndCondition::Blind { depth: 40.0 }),
        end2: None,
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    m
}

#[test]
fn gusset_stiffens_the_bend_but_not_the_flat() {
    let mut plain = bracket();
    flatten(&mut plain);
    let flat = volume(&plain);
    let mut m = bracket();
    let shape = &m.state().body.as_ref().unwrap().shape;
    let bend = (0..shape.count(Kind::Face))
        .find(|f| shape.face_info(*f).unwrap().kind == of_model::geom::SurfaceKind::Cylinder)
        .unwrap();
    let bend = m.state().ref_for(Kind::Face, bend).unwrap();
    let before = volume(&m);
    add(
        &mut m,
        FeatureKind::SheetMetalGusset(SheetMetalGusset {
            bend,
            position: 0.5,
            depth: 8.0,
            width: 3.0,
            punch_id: None,
        }),
    );
    assert!(volume(&m) > before);
    flatten(&mut m);
    ok(&m);
    close(volume(&m), flat, 1e-6 * flat);
    assert_eq!(m.state().sheet_metal[0].form_marks.len(), 1);
}

#[test]
fn forming_tools_press_and_flatten_back() {
    for shape in [
        FormShape::Emboss {
            diameter: 12.0,
            height: 3.0,
        },
        FormShape::Dimple {
            diameter: 10.0,
            height: 3.0,
        },
        FormShape::Rib {
            length: 30.0,
            width: 6.0,
            height: 2.0,
        },
        FormShape::Louver {
            length: 25.0,
            width: 6.0,
            height: 3.0,
        },
        FormShape::Lance {
            length: 20.0,
            width: 5.0,
            height: 3.0,
        },
    ] {
        let mut m = plate(80.0, 50.0, 1.0);
        let flat_volume = volume(&m);
        let top = {
            let s = &m.state().body.as_ref().unwrap().shape;
            let f = (0..s.count(Kind::Face))
                .find(|f| s.face_info(*f).unwrap().normal.z > 0.999)
                .unwrap();
            m.state().ref_for(Kind::Face, f).unwrap()
        };
        // Two placements on the top face.
        let mut sk = Sketch::new();
        sk.add_point(DVec2::new(25.0, 25.0));
        sk.add_point(DVec2::new(55.0, 25.0));
        let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
            plane: top.clone(),
            sketch: sk,
        }));
        add(
            &mut m,
            FeatureKind::FormTool(FormTool {
                shape: shape.clone(),
                face: top,
                sketch: Some(s),
                rotation: FRAC_PI_2,
                flip: false,
                punch_id: Some("P1".into()),
            }),
        );
        assert_eq!(m.state().sheet_metal[0].forms.len(), 2, "{shape:?}");
        let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
        assert!(b.min.z < -1.0, "{shape:?} pressed nothing: {b:?}");
        flatten(&mut m);
        ok(&m);
        close(volume(&m), flat_volume, 1e-6 * flat_volume);
        let marks = &m.state().sheet_metal[0].form_marks;
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].punch_id.as_deref(), Some("P1"));
    }
}
