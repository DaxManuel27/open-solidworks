//! Surface Flatten: developable faces unroll exactly, doubly curved ones
//! stretch a little, and tear edges let a box open into a cross.

use std::f64::consts::PI;

use of_model::surfaces::*;
use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::{Sketch, tools},
    *,
};

fn sketch(doc: &mut Document, plane: Ref, sk: Sketch) -> FeatureId {
    doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch: sk }))
}

fn y_axis(doc: &mut Document) -> Ref {
    let mut axis = Sketch::new();
    let p = axis.add_point(DVec2::ZERO);
    let q = axis.add_point(DVec2::new(0.0, 1.0));
    let line = axis.add_line(p, q).unwrap();
    let id = sketch(doc, Ref::FrontPlane, axis);
    Ref::SketchEntity {
        sketch: id,
        entity: line,
    }
}

fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
}

fn faces_of(m: &Model, body: usize) -> Vec<Ref> {
    let all = m.state().everything().unwrap();
    let ix = m.state().surfaces[body]
        .geometry
        .shape
        .indices_in(&all.shape, Kind::Face)
        .unwrap();
    ix.iter()
        .map(|i| m.state().ref_for(Kind::Face, *i).unwrap())
        .collect()
}

fn flatten(m: &Model, tears: Vec<Ref>) -> (Model, f64) {
    let mut doc = m.doc.clone();
    let id = doc.add(FeatureKind::SurfaceFlatten(SurfaceFlatten {
        faces: faces_of(m, 0),
        fixed: None,
        accuracy: 0.5,
        tears,
    }));
    let f = Model::new(doc);
    ok(&f);
    let worst = f.state().flattenings[&id]
        .strain
        .iter()
        .copied()
        .fold(0.0, f64::max);
    (f, worst)
}

fn flat_area(m: &Model) -> f64 {
    let flat = m.state().surfaces.last().unwrap();
    let info = flat.geometry.shape.face_info(0).unwrap();
    assert_eq!(info.kind, of_model::geom::SurfaceKind::Plane);
    flat.geometry.shape.area()
}

#[test]
fn a_cylindrical_panel_unrolls_to_its_true_size() {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    let a = sk.add_point(DVec2::new(10.0, 0.0));
    let b = sk.add_point(DVec2::new(10.0, 20.0));
    sk.add_line(a, b).unwrap();
    let s = sketch(&mut doc, Ref::FrontPlane, sk);
    let axis = y_axis(&mut doc);
    doc.add(FeatureKind::RevolvedSurface(RevolvedSurface {
        sketch: s,
        axis,
        angle: RevolveAngle::Blind { angle: PI / 2.0 },
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    let (f, worst) = flatten(&m, vec![]);
    assert!(worst < 2e-3, "strain {worst}");
    let true_area = 10.0 * PI / 2.0 * 20.0;
    let a = flat_area(&f);
    assert!(
        (a - true_area).abs() < true_area * 2e-3,
        "{a} vs {true_area}"
    );
}

#[test]
fn a_spherical_cap_flattens_with_a_little_stretch() {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    let c = sk.add_point(DVec2::ZERO);
    let a = sk.add_point(DVec2::new(20.0 * 0.5, 20.0 * 0.75f64.sqrt()));
    let b = sk.add_point(DVec2::new(0.0, 20.0));
    sk.add_arc(c, a, b).unwrap();
    let s = sketch(&mut doc, Ref::FrontPlane, sk);
    let axis = y_axis(&mut doc);
    doc.add(FeatureKind::RevolvedSurface(RevolvedSurface {
        sketch: s,
        axis,
        angle: RevolveAngle::Blind { angle: 2.0 * PI },
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    let (f, worst) = flatten(&m, vec![]);
    assert!(worst > 1e-3 && worst < 0.1, "strain {worst}");
    let cap = 2.0 * PI * 20.0 * (20.0 - 20.0 * 0.75f64.sqrt());
    let a = flat_area(&f);
    assert!((a - cap).abs() < cap * 0.05, "{a} vs {cap}");
}

#[test]
fn an_open_box_opens_into_a_cross_along_its_tears() {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(10.0, 10.0));
    let s = sketch(&mut doc, Ref::FrontPlane, sk);
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: 10.0 },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let m = Model::new(doc);
    let all = m.state().everything().unwrap();
    let top = (0..all.shape.count(Kind::Face))
        .find(|i| all.shape.face_info(*i).unwrap().centroid.z > 9.99)
        .unwrap();
    let top = m.state().ref_for(Kind::Face, top).unwrap();
    let mut doc = m.doc.clone();
    doc.add(FeatureKind::DeleteFace(DeleteFace {
        faces: vec![top],
        mode: DeleteFaceMode::Delete,
    }));
    let open = Model::new(doc);
    ok(&open);
    // The four upright edges.
    let all = open.state().everything().unwrap();
    let tears: Vec<Ref> = (0..all.shape.count(Kind::Edge))
        .filter(|i| {
            let e = all.shape.edge_info(*i).unwrap();
            (e.end - e.start).normalize().z.abs() > 0.99
        })
        .map(|i| open.state().ref_for(Kind::Edge, i).unwrap())
        .collect();
    assert_eq!(tears.len(), 4);
    let (f, worst) = flatten(&open, tears);
    assert!(worst < 1e-3, "strain {worst}");
    let a = flat_area(&f);
    assert!((a - 500.0).abs() < 0.5, "{a}");
    let _ = DVec3::ZERO;
}
