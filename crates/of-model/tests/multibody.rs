use of_model::{
    geom::{BoolOp, DVec3, Kind},
    *,
};

fn model() -> Model {
    Model::new(file::from_str(include_str!("fixtures/v1-multisolid.ofp")).unwrap())
}

fn check(m: &Model, count: usize, volume: f64) {
    assert!(
        m.statuses().iter().all(|s| s.error.is_none()),
        "{:?}",
        m.statuses()
    );
    assert_eq!(m.state().bodies.len(), count);
    let b = m.state().body.as_ref().unwrap();
    assert!(b.shape.is_valid());
    assert!((b.shape.mass_props().unwrap().volume - volume).abs() < 1e-5);
}

#[test]
fn body_identity_survives_edit_move_copy_and_reload() {
    let mut m = model();
    check(&m, 2, 2000.0);
    let ids: Vec<_> = m.state().bodies.iter().map(|b| b.id.clone()).collect();
    let face = m.state().qualified_ref_for(Kind::Face, 0).unwrap();
    let id = m.doc.add(FeatureKind::MoveBody(MoveBody {
        bodies: vec![ids[0].clone()],
        translation: DVec3::new(5.0, 0.0, 0.0),
        rotation: DVec3::ZERO,
        origin: DVec3::ZERO,
        copy: false,
    }));
    m.rebuild();
    check(&m, 2, 2000.0);
    assert_eq!(
        ids,
        m.state()
            .bodies
            .iter()
            .map(|b| b.id.clone())
            .collect::<Vec<_>>()
    );
    assert!(m.resolve(&face).is_ok());
    let reloaded = Model::new(file::from_str(&file::to_string(&m.doc)).unwrap());
    assert_eq!(
        ids,
        reloaded
            .state()
            .bodies
            .iter()
            .map(|b| b.id.clone())
            .collect::<Vec<_>>()
    );
    if let FeatureKind::MoveBody(p) = &mut m.doc.feature_mut(id).unwrap().kind {
        p.copy = true;
    }
    m.invalidate(id);
    m.rebuild();
    check(&m, 3, 3000.0);
    assert_ne!(m.state().bodies[2].id, ids[0]);
    let copy_id = m.state().bodies[2].id.clone();
    if let FeatureKind::Extrude(p) = &mut m.doc.features[1].kind {
        p.end1 = EndCondition::Blind { depth: 8.0 };
    }
    m.rebuild_all();
    check(&m, 3, 4200.0);
    assert_eq!(copy_id, m.state().bodies[2].id);
}

#[test]
fn combine_order_and_failure_preserve_other_bodies() {
    let mut m = model();
    let ids: Vec<_> = m.state().bodies.iter().map(|b| b.id.clone()).collect();
    let move_id = m.doc.add(FeatureKind::MoveBody(MoveBody {
        bodies: vec![ids[1].clone()],
        translation: DVec3::new(-25.0, 0.0, 0.0),
        rotation: DVec3::ZERO,
        origin: DVec3::ZERO,
        copy: false,
    }));
    m.rebuild();
    let combine = m.doc.add(FeatureKind::Combine(Combine {
        bodies: ids.clone(),
        operation: BoolOp::Cut,
    }));
    m.rebuild();
    check(&m, 1, 500.0);
    assert!(m.state().solid(&ids[0]).is_err());
    m.doc.feature_mut(move_id).unwrap().suppressed = true;
    if let FeatureKind::Combine(p) = &mut m.doc.feature_mut(combine).unwrap().kind {
        p.operation = BoolOp::Common;
    }
    m.rebuild_all();
    assert!(m.status(combine).unwrap().error.is_some());
    assert_eq!(m.state().bodies.len(), 2);
    assert_eq!(m.state().bodies[0].id, ids[0]);
}

#[test]
fn plane_split_conserves_volume_and_piece_identity() {
    let mut m = model();
    let original = m.state().bodies[0].id.clone();
    let other = m.state().bodies[1].id.clone();
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::RightPlane,
        distance: 4.0,
        flip: false,
    }));
    let split = m.doc.add(FeatureKind::SplitBody(SplitBody {
        bodies: vec![original],
        plane: Ref::Feature { id: plane },
        keep: KeepSide::Both,
        tools: vec![],
        retained_pieces: None,
    }));
    m.rebuild();
    check(&m, 3, 2000.0);
    let ids: Vec<_> = m.state().bodies.iter().map(|b| b.id.clone()).collect();
    assert!(ids.contains(&other));
    if let FeatureKind::Plane(PlaneDef::Offset { distance, .. }) =
        &mut m.doc.feature_mut(plane).unwrap().kind
    {
        *distance = 6.0;
    }
    m.invalidate(plane);
    m.rebuild();
    check(&m, 3, 2000.0);
    assert_eq!(
        ids,
        m.state()
            .bodies
            .iter()
            .map(|b| b.id.clone())
            .collect::<Vec<_>>()
    );
    m.doc.feature_mut(split).unwrap().suppressed = true;
    m.invalidate(split);
    m.rebuild();
    check(&m, 2, 2000.0);
}

#[test]
fn scope_preserves_unselected_bodies_and_rejects_missing_targets() {
    let mut m = model();
    let first = m.state().bodies[0].id.clone();
    let second = m.state().bodies[1].clone();
    // Shell only the first body, leaving the second body's identity and volume.
    let shell = m.doc.add(FeatureKind::Shell(ShellDef {
        faces: vec![],
        thickness: 1.0,
        outward: false,
    }));
    m.doc.feature_mut(shell).unwrap().scope = Some(vec![first.clone()]);
    m.rebuild();
    assert!(m.status(shell).unwrap().error.is_none());
    let unchanged = m.state().solid(&second.id).unwrap();
    assert_eq!(unchanged.geometry.names, second.geometry.names);
    assert!((unchanged.geometry.shape.mass_props().unwrap().volume - 1000.0).abs() < 1e-6);
    assert!(
        m.state()
            .solid(&first)
            .unwrap()
            .geometry
            .shape
            .mass_props()
            .unwrap()
            .volume
            < 1000.0
    );
    m.doc.feature_mut(shell).unwrap().scope = Some(vec![BodyId("b999:missing".into())]);
    m.invalidate(shell);
    m.rebuild();
    assert!(
        m.status(shell)
            .unwrap()
            .error
            .unwrap()
            .contains("no longer exists")
    );
    assert!(
        (m.state()
            .body
            .as_ref()
            .unwrap()
            .shape
            .mass_props()
            .unwrap()
            .volume
            - 2000.0)
            .abs()
            < 1e-6
    );
}
