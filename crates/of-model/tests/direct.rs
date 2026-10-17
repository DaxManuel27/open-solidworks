//! Direct editing: move face (offset, translate, rotate), replace face and
//! delete hole, as history features that keep downstream references.

use of_model::direct::*;
use of_model::surfaces::*;
use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::{Sketch, tools},
    *,
};

fn rectangle(doc: &mut Document, a: DVec2, b: DVec2) -> FeatureId {
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, a, b);
    doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }))
}

fn extrude(doc: &mut Document, sketch: FeatureId, depth: f64) -> FeatureId {
    doc.add(FeatureKind::Extrude(Extrude {
        sketch,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }))
}

/// A 20 × 20 × 10 block.
fn block() -> (Model, FeatureId) {
    let mut doc = Document::new();
    let s = rectangle(&mut doc, DVec2::ZERO, DVec2::new(20.0, 20.0));
    let e = extrude(&mut doc, s, 10.0);
    (Model::new(doc), e)
}

fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
}

fn volume(m: &Model) -> f64 {
    m.state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

fn face_at(m: &Model, p: DVec3) -> Ref {
    let all = m.state().everything().unwrap();
    let i = (0..all.shape.count(Kind::Face))
        .min_by(|a, b| {
            let da = (all.shape.face_info(*a).unwrap().centroid - p).length();
            let db = (all.shape.face_info(*b).unwrap().centroid - p).length();
            da.total_cmp(&db)
        })
        .unwrap();
    m.state().ref_for(Kind::Face, i).unwrap()
}

fn edge_at(m: &Model, p: DVec3) -> Ref {
    let all = m.state().everything().unwrap();
    let i = (0..all.shape.count(Kind::Edge))
        .min_by(|a, b| {
            let da = (all.shape.edge_info(*a).unwrap().mid - p).length();
            let db = (all.shape.edge_info(*b).unwrap().mid - p).length();
            da.total_cmp(&db)
        })
        .unwrap();
    m.state().ref_for(Kind::Edge, i).unwrap()
}

#[test]
fn offset_face_keeps_its_name_through_upstream_edits() {
    let (mut m, extrude_id) = block();
    let top = face_at(&m, DVec3::new(10.0, 10.0, 10.0));
    let mv = m.doc.add(FeatureKind::MoveFace(MoveFace {
        faces: vec![top.clone()],
        motion: FaceMotion::Offset { distance: 5.0 },
    }));
    m.rebuild();
    ok(&m);
    close(volume(&m), 20.0 * 20.0 * 15.0, 1e-6);
    // The moved face still answers to its old name.
    let Resolved::Face { info, .. } = m.resolve(&top).unwrap() else {
        panic!("not a face")
    };
    close(info.centroid.z, 15.0, 1e-6);
    assert_eq!(m.state().body.as_ref().unwrap().shape.count(Kind::Face), 6);
    // A fillet on the moved face's edge after the edit.
    let edge = edge_at(&m, DVec3::new(10.0, 0.0, 15.0));
    let fillet = m.doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![edge],
        faces: vec![],
        radius: 2.0,
    }));
    m.rebuild();
    ok(&m);
    // Make the block deeper: the move and the fillet re-apply.
    if let FeatureKind::Extrude(e) = &mut m.doc.feature_mut(extrude_id).unwrap().kind {
        e.end1 = EndCondition::Blind { depth: 12.0 };
    }
    m.invalidate(extrude_id);
    m.rebuild();
    ok(&m);
    let _ = (mv, fillet);
    let full = 20.0 * 20.0 * 17.0;
    let fillet_loss = 20.0 * 4.0 * (1.0 - std::f64::consts::PI / 4.0);
    close(volume(&m), full - fillet_loss, 1e-3);
}

#[test]
fn translate_face_up_to_a_plane() {
    let (mut m, _) = block();
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 25.0,
        flip: false,
    }));
    m.rebuild();
    let top = face_at(&m, DVec3::new(10.0, 10.0, 10.0));
    m.doc.add(FeatureKind::MoveFace(MoveFace {
        faces: vec![top],
        motion: FaceMotion::Translate {
            direction: Ref::FrontPlane,
            distance: 0.0,
            reverse: false,
            up_to: Some(Ref::Feature { id: plane }),
        },
    }));
    m.rebuild();
    ok(&m);
    close(volume(&m), 20.0 * 20.0 * 25.0, 1e-6);
}

#[test]
fn rotate_face_about_an_edge() {
    let (mut m, _) = block();
    let top = face_at(&m, DVec3::new(10.0, 10.0, 10.0));
    let hinge = edge_at(&m, DVec3::new(10.0, 0.0, 10.0));
    m.doc.add(FeatureKind::MoveFace(MoveFace {
        faces: vec![top],
        motion: FaceMotion::Rotate {
            axis: hinge,
            angle: 3f64.to_radians(),
        },
    }));
    m.rebuild();
    ok(&m);
    // A wedge 20 wide, rising 20·tan 3° over 20.
    let wedge = 0.5 * 20.0 * 20.0 * 3f64.to_radians().tan() * 20.0;
    close((volume(&m) - 4000.0).abs(), wedge, 1e-6);
    assert_eq!(m.state().body.as_ref().unwrap().shape.count(Kind::Face), 6);
}

#[test]
fn replace_face_with_a_surface() {
    let (mut m, _) = block();
    let top = face_at(&m, DVec3::new(10.0, 10.0, 10.0));
    m.doc.add(FeatureKind::OffsetSurface(OffsetSurface {
        faces: vec![top.clone()],
        distance: 4.0,
        flip: false,
    }));
    m.rebuild();
    let surface = m.state().surfaces[0].id.clone();
    m.doc.add(FeatureKind::ReplaceFace(ReplaceFace {
        faces: vec![top],
        replacement: Ref::Body { id: surface },
    }));
    m.rebuild();
    ok(&m);
    close(volume(&m), 20.0 * 20.0 * 14.0, 1e-6);
}

#[test]
fn move_face_refuses_surface_faces() {
    let mut doc = Document::new();
    let s = rectangle(&mut doc, DVec2::ZERO, DVec2::new(20.0, 20.0));
    doc.add(FeatureKind::PlanarSurface(PlanarSurface {
        sketch: Some(s),
        edges: vec![],
    }));
    let mut m = Model::new(doc);
    let face = face_at(&m, DVec3::new(10.0, 10.0, 0.0));
    let mv = m.doc.add(FeatureKind::MoveFace(MoveFace {
        faces: vec![face],
        motion: FaceMotion::Offset { distance: 2.0 },
    }));
    m.rebuild();
    let e = m.status(mv).unwrap().error.unwrap();
    assert!(e.contains("surface body"), "{e}");
}

#[test]
fn delete_hole_closes_a_planar_surface() {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(20.0, 20.0));
    let c = sk.add_point(DVec2::new(10.0, 10.0));
    sk.add_circle(c, 3.0).unwrap();
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::PlanarSurface(PlanarSurface {
        sketch: Some(s),
        edges: vec![],
    }));
    let mut m = Model::new(doc);
    ok(&m);
    let area = |m: &Model| m.state().surfaces[0].geometry.shape.area();
    close(area(&m), 400.0 - std::f64::consts::PI * 9.0, 1e-6);
    let face = face_at(&m, DVec3::new(10.0, 10.0, 0.0));
    m.doc
        .add(FeatureKind::DeleteHole(DeleteHole { faces: vec![face] }));
    m.rebuild();
    ok(&m);
    close(area(&m), 400.0, 1e-6);
}

#[test]
fn delete_face_patch_heals_a_through_hole() {
    let (mut m, _) = block();
    let mut sk = Sketch::new();
    let c = sk.add_point(DVec2::new(10.0, 10.0));
    sk.add_circle(c, 3.0).unwrap();
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    m.doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: true,
        start_offset: 0.0,
        end1: EndCondition::ThroughAll,
        reverse: false,
        end2: Some(EndCondition::ThroughAll),
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    m.rebuild();
    ok(&m);
    assert!(volume(&m) < 4000.0);
    let wall = face_at(&m, DVec3::new(10.0, 10.0, 5.0));
    m.doc.add(FeatureKind::DeleteFace(DeleteFace {
        faces: vec![wall],
        mode: DeleteFaceMode::Patch,
    }));
    m.rebuild();
    ok(&m);
    close(volume(&m), 4000.0, 1e-6);
}
