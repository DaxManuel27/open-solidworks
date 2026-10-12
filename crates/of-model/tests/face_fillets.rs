use of_model::{
    geom::{DVec2, DVec3, Kind, SurfaceKind},
    sketch::Sketch,
    *,
};
use std::f64::consts::PI;

fn polygon_part(points: &[(f64, f64)], depth: f64) -> Model {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    let ids: Vec<_> = points
        .iter()
        .map(|(x, y)| sk.add_point(DVec2::new(*x, *y)))
        .collect();
    for i in 0..ids.len() {
        sk.add_line(ids[i], ids[(i + 1) % ids.len()]).unwrap();
    }
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let m = Model::new(doc);
    valid(&m);
    m
}

/// The planar face facing `n` that contains `at`.
fn face(m: &Model, n: DVec3, at: DVec3) -> Ref {
    let body = m.state().body.as_ref().unwrap();
    let i = (0..body.shape.count(Kind::Face))
        .find(|&i| {
            let f = body.shape.face_info(i).unwrap();
            f.kind == SurfaceKind::Plane
                && f.normal.dot(n) > 0.999
                && (at - f.point).dot(f.normal).abs() < 1e-9
        })
        .unwrap_or_else(|| panic!("no face facing {n} through {at}"));
    m.state().ref_for(Kind::Face, i).unwrap()
}

fn valid(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
    assert!(m.state().body.as_ref().unwrap().shape.is_valid());
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

fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6 * b.abs().max(1.0), "{a} != {b}");
}

fn fillet(m: &mut Model, faces1: Ref, faces2: Ref, radius: f64) -> FeatureId {
    let id = m.doc.add(FeatureKind::FaceFillet(FaceFillet {
        faces1: vec![faces1],
        faces2: vec![faces2],
        radius,
    }));
    m.rebuild_all();
    id
}

const BOX: [(f64, f64); 4] = [(0.0, 0.0), (40.0, 0.0), (40.0, 20.0), (0.0, 20.0)];

#[test]
fn convex_and_concave_corners_match_rolling_ball_volumes() {
    // Convex: the top and +X side of a 40 x 20 x 10 block along a 20 mm edge.
    let mut m = polygon_part(&BOX, 10.0);
    let top = face(&m, DVec3::Z, DVec3::new(0.0, 0.0, 10.0));
    let side = face(&m, DVec3::X, DVec3::new(40.0, 0.0, 0.0));
    let f = fillet(&mut m, top, side, 2.0);
    valid(&m);
    assert_close(volume(&m), 8000.0 - 20.0 * (4.0 - PI));
    // The blend is a 2 mm cylinder tangent to both planes.
    let body = m.state().body.as_ref().unwrap();
    let blend = (0..body.shape.count(Kind::Face))
        .map(|i| body.shape.face_info(i).unwrap())
        .find(|f| f.kind == SurfaceKind::Cylinder)
        .unwrap();
    assert!((blend.radius - 2.0).abs() < 1e-9);
    assert!((blend.axis_origin.x - 38.0).abs() < 1e-9 && (blend.axis_origin.z - 8.0).abs() < 1e-9);
    let names = &body.names;
    assert!(names.face(&format!("{f}.blend")).is_some());

    // Concave: the inside corner of an L section, 2 mm thick, 10 deep.
    let l = [
        (0.0, 0.0),
        (20.0, 0.0),
        (20.0, 2.0),
        (2.0, 2.0),
        (2.0, 20.0),
        (0.0, 20.0),
    ];
    let mut m = polygon_part(&l, 10.0);
    let floor = face(&m, DVec3::Y, DVec3::new(10.0, 2.0, 0.0));
    let wall = face(&m, DVec3::X, DVec3::new(2.0, 10.0, 0.0));
    fillet(&mut m, floor, wall, 1.0);
    valid(&m);
    assert_close(volume(&m), 760.0 + 10.0 * (1.0 - PI / 4.0));
}

#[test]
fn non_adjacent_faces_blend_across_a_chamfer() {
    // A 3 mm face fillet between the top and side across a 1 mm chamfer leaves
    // the same part as a 3 mm edge fillet on the unchamfered block.
    let mut m = polygon_part(&BOX, 10.0);
    let top = face(&m, DVec3::Z, DVec3::new(0.0, 0.0, 10.0));
    let side = face(&m, DVec3::X, DVec3::new(40.0, 0.0, 0.0));
    let body = m.state().body.as_ref().unwrap();
    let edge = (0..body.shape.count(Kind::Edge))
        .find(|&e| {
            let mid = body.shape.edge_info(e).unwrap().mid;
            (mid.x - 40.0).abs() < 1e-9 && (mid.z - 10.0).abs() < 1e-9
        })
        .unwrap();
    let edge = m.state().ref_for(Kind::Edge, edge).unwrap();
    m.doc.add(FeatureKind::Chamfer(Chamfer {
        edges: vec![edge],
        faces: vec![],
        mode: ChamferMode::Equal { distance: 1.0 },
        flip: false,
    }));
    m.rebuild();
    valid(&m);
    fillet(&mut m, top, side, 3.0);
    valid(&m);
    assert_close(volume(&m), 8000.0 - 20.0 * (9.0 - 9.0 * PI / 4.0));
}

#[test]
fn face_fillet_rejects_invalid_input() {
    let mut m = polygon_part(&BOX, 10.0);
    let top = face(&m, DVec3::Z, DVec3::new(0.0, 0.0, 10.0));
    let bottom = face(&m, -DVec3::Z, DVec3::ZERO);
    let side = face(&m, DVec3::X, DVec3::new(40.0, 0.0, 0.0));
    let f = fillet(&mut m, top.clone(), bottom, 1.0);
    assert!(m.status(f).unwrap().error.unwrap().contains("parallel"));
    m.doc.delete(&[f]);
    let f = fillet(&mut m, top.clone(), side.clone(), 30.0);
    assert!(m.status(f).unwrap().error.unwrap().contains("does not fit"));
    m.doc.delete(&[f]);
    let f = fillet(&mut m, top, side, 0.0);
    assert!(m.status(f).unwrap().error.unwrap().contains("radius"));
}
