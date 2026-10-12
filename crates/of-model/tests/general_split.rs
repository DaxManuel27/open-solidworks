use of_model::{geom::DVec3, *};

fn source() -> Model {
    Model::new(file::from_str(include_str!("fixtures/v1-multisolid.ofp")).unwrap())
}
fn check(model: &Model) {
    assert!(
        model.statuses().iter().all(|s| s.error.is_none()),
        "{:?}",
        model.statuses()
    );
    assert!(model.state().body.as_ref().unwrap().shape.is_valid());
}
fn volume(model: &Model, id: &BodyId) -> f64 {
    model
        .state()
        .solid(id)
        .unwrap()
        .geometry
        .shape
        .mass_props()
        .unwrap()
        .volume
}
fn plane(model: &mut Model, offset: f64) -> u32 {
    model.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::RightPlane,
        distance: offset,
        flip: false,
    }))
}
fn parameters(body: BodyId, tools: Vec<Ref>) -> SplitBody {
    SplitBody {
        bodies: vec![body],
        tools,
        plane: Ref::FrontPlane,
        keep: KeepSide::Both,
        retained_pieces: None,
    }
}
fn pieces(model: &Model, feature: u32) -> Vec<BodyId> {
    model
        .state()
        .bodies
        .iter()
        .filter(|b| b.id.0.starts_with(&format!("b{feature}:piece:")))
        .map(|b| b.id.clone())
        .collect()
}
fn set_retained(model: &mut Model, feature: u32, retained: Vec<BodyId>) {
    if let FeatureKind::SplitBody(params) = &mut model.doc.feature_mut(feature).unwrap().kind {
        params.retained_pieces = Some(retained);
    }
    model.invalidate(feature);
    model.rebuild();
}

#[test]
fn multiple_planes_retain_arbitrary_pieces_and_survive_tool_edits_reload_copy_combine() {
    let mut model = source();
    let source_id = model.state().bodies[0].id.clone();
    let other = model.state().bodies[1].id.clone();
    let a = plane(&mut model, 3.0);
    let b = plane(&mut model, 7.0);
    let split = model.doc.add(FeatureKind::SplitBody(parameters(
        source_id,
        vec![Ref::Feature { id: a }, Ref::Feature { id: b }],
    )));
    model.rebuild();
    check(&model);
    let original = pieces(&model, split);
    assert_eq!(original.len(), 3);
    assert!((original.iter().map(|id| volume(&model, id)).sum::<f64>() - 1000.0).abs() < 1e-6);
    if let FeatureKind::SplitBody(params) = &mut model.doc.feature_mut(split).unwrap().kind {
        params.tools.reverse();
    }
    model.invalidate(split);
    model.rebuild();
    check(&model);
    let mut reordered = pieces(&model, split);
    let mut expected = original.clone();
    reordered.sort();
    expected.sort();
    assert_eq!(reordered, expected, "tool enumeration cannot rename pieces");
    let retained: Vec<_> = original
        .iter()
        .filter(|id| (volume(&model, id) - 300.0).abs() < 1e-6)
        .cloned()
        .collect();
    assert_eq!(retained.len(), 2);
    set_retained(&mut model, split, retained.clone());
    check(&model);
    assert_eq!(pieces(&model, split).len(), 2);
    if let FeatureKind::Plane(PlaneDef::Offset { distance, .. }) =
        &mut model.doc.feature_mut(a).unwrap().kind
    {
        *distance = 4.0;
    }
    model.invalidate(a);
    model.rebuild();
    check(&model);
    assert_eq!(pieces(&model, split), retained);
    assert!((retained.iter().map(|id| volume(&model, id)).sum::<f64>() - 700.0).abs() < 1e-6);
    assert!((volume(&model, &other) - 1000.0).abs() < 1e-6);
    let saved = file::to_string(&model.doc);
    let mut model = Model::new(file::from_str(&saved).unwrap());
    check(&model);
    assert_eq!(pieces(&model, split), retained);
    let copy = model.doc.add(FeatureKind::MoveBody(MoveBody {
        bodies: retained.clone(),
        translation: DVec3::new(0.0, 30.0, 0.0),
        rotation: DVec3::ZERO,
        origin: DVec3::ZERO,
        copy: true,
    }));
    model.rebuild();
    check(&model);
    let copies: Vec<_> = model
        .state()
        .bodies
        .iter()
        .filter(|b| b.id.creator() == Some(copy))
        .map(|b| b.id.clone())
        .collect();
    assert_eq!(copies.len(), 2);
    // The retained pieces don't touch, so adding their copies is rejected rather
    // than producing a "combined" result that is still two bodies.
    let combine = model.doc.add(FeatureKind::Combine(Combine {
        bodies: copies,
        operation: geom::BoolOp::Fuse,
    }));
    model.rebuild();
    let error = model.status(combine).unwrap().error.unwrap();
    assert!(error.contains("don't touch"), "{error}");
    assert!(retained.iter().all(|id| model.state().solid(id).is_ok()));
}

#[test]
fn split_count_changes_keep_unaffected_ids_and_never_rebind_removed_pieces() {
    let mut model = source();
    let source = model.state().bodies[0].id.clone();
    let a = plane(&mut model, 3.0);
    let b = plane(&mut model, 7.0);
    let split = model.doc.add(FeatureKind::SplitBody(parameters(
        source,
        vec![Ref::Feature { id: a }, Ref::Feature { id: b }],
    )));
    model.rebuild();
    check(&model);
    let old = pieces(&model, split);
    let left = old
        .iter()
        .find(|id| {
            model
                .state()
                .solid(id)
                .unwrap()
                .geometry
                .shape
                .mass_props()
                .unwrap()
                .center
                .x
                < 3.0
        })
        .unwrap()
        .clone();
    let right = old
        .iter()
        .find(|id| {
            model
                .state()
                .solid(id)
                .unwrap()
                .geometry
                .shape
                .mass_props()
                .unwrap()
                .center
                .x
                > 7.0
        })
        .unwrap()
        .clone();
    if let FeatureKind::Plane(PlaneDef::Offset { distance, .. }) =
        &mut model.doc.feature_mut(b).unwrap().kind
    {
        *distance = 12.0;
    }
    model.rebuild_all();
    check(&model);
    assert_eq!(pieces(&model, split).len(), 2);
    assert!(model.state().solid(&left).is_ok());
    assert!(model.state().solid(&right).is_err());
    set_retained(&mut model, split, vec![left]);
    check(&model);
    set_retained(&mut model, split, vec![right]);
    assert!(
        model
            .status(split)
            .unwrap()
            .error
            .unwrap()
            .contains("unresolved")
    );
    assert_eq!(
        model.state().bodies.len(),
        2,
        "failed split preserves preceding state"
    );
}

#[test]
fn solid_tools_and_bounded_faces_use_history_and_leave_tools_intact() {
    let mut model = source();
    if let FeatureKind::Sketch(sketch) = &mut model.doc.features[2].kind {
        for entity in sketch.sketch.entities.values_mut() {
            if let sketch::Geometry::Point { p } = &mut entity.geo
                && p.x == 40.0
            {
                p.x = 32.0;
            }
        }
    }
    model.rebuild_all();
    check(&model);
    let target = model.state().bodies[0].id.clone();
    let tool = model.state().bodies[1].id.clone();
    // A two-millimetre slab crosses the source at x=4..6, making three pieces.
    let movement = model.doc.add(FeatureKind::MoveBody(MoveBody {
        bodies: vec![tool.clone()],
        translation: DVec3::new(-26.0, 0.0, 0.0),
        rotation: DVec3::ZERO,
        origin: DVec3::ZERO,
        copy: false,
    }));
    model.rebuild();
    check(&model);
    let split = model.doc.add(FeatureKind::SplitBody(parameters(
        target,
        vec![Ref::Body { id: tool.clone() }],
    )));
    model.rebuild();
    check(&model);
    assert_eq!(pieces(&model, split).len(), 3);
    assert!((volume(&model, &tool) - 200.0).abs() < 1e-6);
    let retained: Vec<_> = pieces(&model, split)
        .into_iter()
        .filter(|id| (volume(&model, id) - 400.0).abs() < 1e-6)
        .collect();
    assert_eq!(retained.len(), 2);
    set_retained(&mut model, split, retained.clone());
    if let FeatureKind::MoveBody(params) = &mut model.doc.feature_mut(movement).unwrap().kind {
        params.translation.x = -25.0;
    }
    model.invalidate(movement);
    model.rebuild();
    check(&model);
    assert_eq!(pieces(&model, split), retained);
    assert!((retained.iter().map(|id| volume(&model, id)).sum::<f64>() - 800.0).abs() < 1e-6);
    let face = model
        .state()
        .solid(&tool)
        .unwrap()
        .geometry
        .names
        .faces
        .iter()
        .enumerate()
        .find(|(i, _)| {
            (model
                .state()
                .solid(&tool)
                .unwrap()
                .geometry
                .shape
                .face_info(*i as u32)
                .unwrap()
                .centroid
                .x
                - 5.0)
                .abs()
                < 1e-6
        })
        .unwrap()
        .1[0]
        .clone();
    if let FeatureKind::SplitBody(params) = &mut model.doc.feature_mut(split).unwrap().kind {
        params.tools = vec![Ref::BodyTopology {
            body: tool,
            kind: geom::Kind::Face,
            name: face,
        }];
        params.retained_pieces = None;
    }
    model.invalidate(split);
    model.rebuild();
    check(&model);
    assert_eq!(pieces(&model, split).len(), 2);
}

#[test]
fn invalid_selections_and_nonintersecting_tools_report_actionable_errors() {
    let mut model = source();
    let source = model.state().bodies[0].id.clone();
    let outside = plane(&mut model, 20.0);
    let split = model.doc.add(FeatureKind::SplitBody(parameters(
        source.clone(),
        vec![Ref::Feature { id: outside }],
    )));
    model.rebuild();
    assert!(
        model
            .status(split)
            .unwrap()
            .error
            .unwrap()
            .contains("do not divide")
    );
    if let FeatureKind::SplitBody(params) = &mut model.doc.feature_mut(split).unwrap().kind {
        params.tools = vec![Ref::Body { id: source }];
    }
    model.invalidate(split);
    model.rebuild();
    assert!(
        model
            .status(split)
            .unwrap()
            .error
            .unwrap()
            .contains("cannot also")
    );
    if let FeatureKind::SplitBody(params) = &mut model.doc.feature_mut(split).unwrap().kind {
        params.tools = vec![Ref::Feature { id: outside }];
        params.keep = KeepSide::Positive;
    }
    model.invalidate(split);
    model.rebuild();
    assert!(
        model
            .status(split)
            .unwrap()
            .error
            .unwrap()
            .contains("legacy plane")
    );
}

#[test]
fn geometrically_distinct_pieces_with_identical_provenance_fail_as_ambiguous() {
    let mut doc = Document::new();
    let mut sketch = sketch::Sketch::new();
    sketch.add_circle_at(geom::DVec2::new(10.0, 0.0), 2.0);
    let (axis, _, _) = sketch.add_line_at(geom::DVec2::ZERO, geom::DVec2::Y);
    sketch.entities.get_mut(&axis).unwrap().construction = true;
    let profile = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    doc.add(FeatureKind::Revolve(Revolve {
        sketch: profile,
        axis: Ref::SketchEntity {
            sketch: profile,
            entity: axis,
        },
        angle: RevolveAngle::Blind {
            angle: std::f64::consts::TAU,
        },
        end2: None,
        reverse: false,
        cut: false,
        thin: None,
        merge: true,
    }));
    let mut model = Model::new(doc);
    check(&model);
    let source = model.state().bodies[0].id.clone();
    let a = plane(&mut model, -3.0);
    let b = plane(&mut model, 3.0);
    let split = model.doc.add(FeatureKind::SplitBody(parameters(
        source.clone(),
        vec![Ref::Feature { id: a }, Ref::Feature { id: b }],
    )));
    model.rebuild();
    assert!(
        model
            .status(split)
            .unwrap()
            .error
            .unwrap()
            .contains("ambiguous")
    );
    assert_eq!(model.state().bodies.len(), 1);
    assert!(model.state().solid(&source).is_ok());
}

#[test]
fn legacy_side_selection_keeps_positive_and_negative_meaning() {
    for (side, expected) in [(KeepSide::Positive, 700.0), (KeepSide::Negative, 300.0)] {
        let mut model = source();
        let source = model.state().bodies[0].id.clone();
        let plane = plane(&mut model, 3.0);
        let split = model.doc.add(FeatureKind::SplitBody(SplitBody {
            bodies: vec![source],
            plane: Ref::Feature { id: plane },
            keep: side,
            tools: vec![],
            retained_pieces: None,
        }));
        model.rebuild();
        check(&model);
        let piece = model
            .state()
            .bodies
            .iter()
            .find(|b| b.id.creator() == Some(split))
            .unwrap();
        assert!((volume(&model, &piece.id) - expected).abs() < 1e-6);
        assert!(piece.id.0.ends_with(if side == KeepSide::Positive {
            "positive"
        } else {
            "negative"
        }));
    }
}
