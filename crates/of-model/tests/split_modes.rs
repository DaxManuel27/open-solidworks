use of_model::{
    geom::{DVec2, DVec3, Kind, SurfaceKind},
    sketch::{Sketch, tools},
    *,
};

fn extrude(doc: &mut Document, sketch: Sketch, depth: f64) {
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
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
}

fn face(m: &Model, kind: SurfaceKind, normal: Option<DVec3>) -> Ref {
    let body = m.state().body.as_ref().unwrap();
    let i = (0..body.shape.count(Kind::Face))
        .find(|&i| {
            let f = body.shape.face_info(i).unwrap();
            f.kind == kind && normal.is_none_or(|n| f.normal.dot(n) > 0.999)
        })
        .unwrap();
    m.state().ref_for(Kind::Face, i).unwrap()
}

fn faces_and_volume(m: &Model) -> (u32, f64) {
    let shape = &m.state().body.as_ref().unwrap().shape;
    (shape.count(Kind::Face), shape.mass_props().unwrap().volume)
}

fn split(m: &mut Model, p: ProjectedCurve) -> FeatureId {
    let id = m.doc.add(FeatureKind::SplitLine(p));
    m.rebuild();
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
    id
}

#[test]
fn intersection_split_line_divides_a_face_without_changing_material() {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(40.0, 20.0));
    extrude(&mut doc, sk, 10.0);
    let mid = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::RightPlane,
        distance: 20.0,
        flip: false,
    }));
    let mut m = Model::new(doc);
    let (faces, volume) = faces_and_volume(&m);
    let top = face(&m, SurfaceKind::Plane, Some(DVec3::Z));
    split(
        &mut m,
        ProjectedCurve {
            source: Ref::Feature { id: mid },
            target: top,
            direction: DVec3::Z,
            mode: ProjectionMode::Intersection,
        },
    );
    let (after, v) = faces_and_volume(&m);
    assert_eq!(after, faces + 1);
    assert!((v - volume).abs() < 1e-9 * volume);
    // Both halves of the top are 20 x 20.
    let body = m.state().body.as_ref().unwrap();
    let tops: Vec<f64> = (0..body.shape.count(Kind::Face))
        .map(|i| body.shape.face_info(i).unwrap())
        .filter(|f| f.normal.z > 0.999)
        .map(|f| f.area)
        .collect();
    assert_eq!(tops.len(), 2);
    assert!(tops.iter().all(|a| (a - 400.0).abs() < 1e-6), "{tops:?}");
}

#[test]
fn silhouette_split_line_divides_a_cylinder_along_its_outline() {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::ZERO, 5.0);
    extrude(&mut doc, sk, 20.0);
    let mut m = Model::new(doc);
    let (faces, volume) = faces_and_volume(&m);
    let side = face(&m, SurfaceKind::Cylinder, None);
    split(
        &mut m,
        ProjectedCurve {
            source: Ref::Origin,
            target: side,
            direction: DVec3::X,
            mode: ProjectionMode::Silhouette,
        },
    );
    let (after, v) = faces_and_volume(&m);
    assert!(after > faces, "{after} faces");
    assert!((v - volume).abs() < 1e-9 * volume);
    // No piece of the side crosses the outline: the faces facing +X and -X each
    // make up exactly half of it (the cylinder's own seam may divide one half).
    let body = m.state().body.as_ref().unwrap();
    let side: Vec<_> = (0..body.shape.count(Kind::Face))
        .map(|i| body.shape.face_info(i).unwrap())
        .filter(|f| f.kind == SurfaceKind::Cylinder)
        .collect();
    let half = std::f64::consts::PI * 5.0 * 20.0;
    for sign in [1.0, -1.0] {
        let area: f64 = side
            .iter()
            .filter(|f| f.centroid.x * sign > 0.0)
            .map(|f| f.area)
            .sum();
        assert!((area - half).abs() < 1e-6, "{sign}: {area}");
    }
}

#[test]
fn sketch_on_sketch_meets_where_both_extrusions_cross() {
    let mut doc = Document::new();
    // On the front plane, the diagonal x = y; on the top plane, z = x / 2.
    let mut a = Sketch::new();
    a.add_line_at(DVec2::ZERO, DVec2::new(10.0, 10.0));
    let a = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: a,
    }));
    let mut b = Sketch::new();
    b.add_line_at(DVec2::ZERO, DVec2::new(10.0, -5.0));
    let b = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::TopPlane,
        sketch: b,
    }));
    let curve = doc.add(FeatureKind::ProjectedCurve(ProjectedCurve {
        source: Ref::Feature { id: b },
        target: Ref::Feature { id: a },
        direction: DVec3::Z,
        mode: ProjectionMode::SketchOnSketch,
    }));
    let m = Model::new(doc);
    assert!(
        m.status(curve).unwrap().error.is_none(),
        "{:?}",
        m.status(curve)
    );
    let shape = &m.state().curves[&curve].shape;
    let length: f64 = (0..shape.count(Kind::Edge))
        .map(|e| shape.edge_info(e).unwrap().length)
        .sum();
    assert!((length - 15.0).abs() < 1e-6, "{length}");
    let ends: Vec<DVec3> = (0..shape.count(Kind::Vertex))
        .map(|v| shape.vertex_point(v).unwrap())
        .collect();
    assert!(
        ends.iter()
            .any(|p| (*p - DVec3::new(10.0, 10.0, 5.0)).length() < 1e-6)
    );
}
