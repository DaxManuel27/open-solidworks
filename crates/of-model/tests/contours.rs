use of_model::{geom::DVec2, sketch::Sketch, *};

fn fixture() -> (Model, FeatureId, FeatureId, u32, u32) {
    let mut doc = Document::new();
    let mut circle = Sketch::new();
    circle.add_circle_at(DVec2::ZERO, 2.0);
    let profile = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: circle,
    }));
    let mut sketch = Sketch::new();
    let (unrelated, _, _) = sketch.add_line_at(DVec2::new(20.0, 0.0), DVec2::new(20.0, -5.0));
    let (selected, _, _) = sketch.add_line_at(DVec2::ZERO, DVec2::new(0.0, -10.0));
    let path = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::TopPlane,
        sketch,
    }));
    let sweep = doc.add(FeatureKind::Sweep(Sweep {
        profile: Some(profile.into()),
        inner_profiles: vec![],
        circle_diameter: None,
        twist: 0.0,
        guide: None,
        guide_scale: false,
        guide2: None,
        thin: None,
        path: Ref::SketchContour {
            sketch: path,
            entities: vec![selected],
        },
        reverse_path: false,
        orientation: SweepOrientation::MinimumTwist,
        cut: false,
        merge: true,
    }));
    (Model::new(doc), path, sweep, unrelated, selected)
}

fn assert_swept(model: &Model) {
    assert!(
        model.statuses().iter().all(|s| s.error.is_none()),
        "{:?}",
        model.statuses()
    );
    let shape = &model.state().body.as_ref().unwrap().shape;
    assert!(shape.is_valid());
    assert!((shape.mass_props().unwrap().volume - 40.0 * std::f64::consts::PI).abs() < 1e-5);
}

#[test]
fn contour_selection_survives_other_contour_removal_and_reload() {
    let (mut model, path, sweep, unrelated, _) = fixture();
    assert_swept(&model);
    if let FeatureKind::Sketch(s) = &mut model.doc.feature_mut(path).unwrap().kind {
        s.sketch.delete(&[unrelated]);
    }
    model.invalidate(path);
    model.rebuild();
    assert_swept(&model);
    let params = model.doc.feature(sweep).unwrap().kind.clone();
    assert!(params.depends_on().contains(&path));
    let preview = model.preview(&params, Some(sweep)).unwrap();
    assert!(
        (preview.body.unwrap().shape.mass_props().unwrap().volume
            - model
                .state()
                .body
                .as_ref()
                .unwrap()
                .shape
                .mass_props()
                .unwrap()
                .volume)
            .abs()
            < 1e-7
    );
    assert_swept(&Model::new(
        file::from_str(&file::to_string(&model.doc)).unwrap(),
    ));
    let deleted = model.doc.delete(&[path]);
    assert!(
        deleted.contains(&sweep),
        "contour source must remain a dependency"
    );
}

#[test]
fn replaced_or_extended_contour_never_rebinds_by_position() {
    for replace in [true, false] {
        let (mut model, path, sweep, _, selected) = fixture();
        if let FeatureKind::Sketch(s) = &mut model.doc.feature_mut(path).unwrap().kind {
            if replace {
                s.sketch.delete(&[selected]);
                s.sketch.add_line_at(DVec2::ZERO, DVec2::new(0.0, -10.0));
            } else {
                s.sketch
                    .add_line_at(DVec2::new(0.0, -10.0), DVec2::new(0.0, -20.0));
            }
        }
        model.rebuild_all();
        assert!(
            model
                .status(sweep)
                .unwrap()
                .error
                .unwrap()
                .contains("selected contour changed")
        );
        assert!(model.state().body.is_none());
    }
}

#[test]
fn duplicate_empty_and_partial_memberships_are_rejected() {
    for entities in [vec![], vec![999], vec![7, 7]] {
        let (mut model, _, sweep, _, _) = fixture();
        if let FeatureKind::Sweep(p) = &mut model.doc.feature_mut(sweep).unwrap().kind {
            let Ref::SketchContour { entities: ids, .. } = &mut p.path else {
                unreachable!()
            };
            *ids = entities;
        }
        model.rebuild_all();
        assert!(model.status(sweep).unwrap().error.is_some());
    }
}
