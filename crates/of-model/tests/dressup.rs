use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::Sketch,
    *,
};

fn model() -> Model {
    Model::new(file::from_str(include_str!("fixtures/v1-block.ofp")).unwrap())
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

#[test]
fn neutral_plane_draft_has_expected_taper_and_preserves_identity() {
    let mut m = model();
    let body = m.state().body.as_ref().unwrap();
    let side = (0..body.shape.count(Kind::Face))
        .find(|i| body.shape.face_info(*i).unwrap().normal.x > 0.99)
        .unwrap();
    let face = m.state().qualified_ref_for(Kind::Face, side).unwrap();
    let body_id = m.state().bodies[0].id.clone();
    let angle = 3_f64.to_radians();
    let id = m.doc.add(FeatureKind::Draft(Draft {
        faces: vec![face],
        neutral_plane: Ref::FrontPlane,
        direction: None,
        angle,
    }));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_none(), "{:?}", m.status(id));
    assert!((volume(&m) - (1000.0 - 250.0 * angle.tan())).abs() < 1e-5);
    assert_eq!(m.state().bodies[0].id, body_id);
    assert!(m.state().body.as_ref().unwrap().shape.is_valid());
}

#[test]
fn variable_fillet_rejects_bad_stations_and_retains_previous_solid() {
    let mut m = model();
    let body = m.state().body.as_ref().unwrap();
    let edge = (0..body.shape.count(Kind::Edge))
        .find(|i| {
            let e = body.shape.edge_info(*i).unwrap();
            (e.end - e.start).normalize().dot(DVec3::Z).abs() > 0.99
        })
        .unwrap();
    let edge = m.state().qualified_ref_for(Kind::Edge, edge).unwrap();
    let id = m.doc.add(FeatureKind::VariableFillet(VariableFillet {
        edge,
        stations: vec![
            RadiusStation {
                position: 0.0,
                radius: 0.5,
            },
            RadiusStation {
                position: 1.0,
                radius: 1.0,
            },
        ],
        reverse: false,
    }));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_none(), "{:?}", m.status(id));
    assert!(volume(&m) < 1000.0 && volume(&m) > 900.0);
    if let FeatureKind::VariableFillet(p) = &mut m.doc.feature_mut(id).unwrap().kind {
        p.stations[1].position = 0.0;
    }
    m.invalidate(id);
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_some());
    assert!((volume(&m) - 1000.0).abs() < 1e-6);
}

#[test]
fn projected_split_line_preserves_volume_and_rejects_ambiguous_old_face() {
    let mut m = model();
    let body = m.state().body.as_ref().unwrap();
    let face = (0..body.shape.count(Kind::Face))
        .find(|i| body.shape.face_info(*i).unwrap().normal.z > 0.99)
        .unwrap();
    let target = m.state().qualified_ref_for(Kind::Face, face).unwrap();
    let original_faces = body.shape.count(Kind::Face);
    let mut sketch = Sketch::new();
    sketch.add_circle_at(DVec2::new(5.0, 10.0), 2.0);
    let source = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    let p = ProjectedCurve {
        source: Ref::Feature { id: source },
        target: target.clone(),
        direction: DVec3::Z,
    };
    let projected = m.doc.add(FeatureKind::ProjectedCurve(p.clone()));
    let split = m.doc.add(FeatureKind::SplitLine(p));
    m.rebuild();
    assert!(
        m.status(projected).unwrap().error.is_none(),
        "{:?}",
        m.status(projected)
    );
    assert!(
        m.status(split).unwrap().error.is_none(),
        "{:?}",
        m.status(split)
    );
    assert!((volume(&m) - 1000.0).abs() < 1e-6);
    assert_eq!(
        m.state().body.as_ref().unwrap().shape.count(Kind::Face),
        original_faces + 1
    );
    assert!(m.resolve(&target).is_err());
}
