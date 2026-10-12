use of_model::{
    configurations::{Configuration, MaterialOverride},
    *,
};

fn document() -> Document {
    file::from_str(include_str!("fixtures/v1-block.ofp")).unwrap()
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
fn formulas_rebuild_without_overwriting_authored_parameters() {
    let mut doc = document();
    doc.variables.insert("height".into(), "2cm".into());
    doc.equations
        .insert("f2.end1.depth".into(), "height / 2".into());
    let mut m = Model::new(doc);
    assert!((volume(&m) - 2000.0).abs() < 1e-6);
    if let FeatureKind::Extrude(e) = &m.doc.features[1].kind {
        assert_eq!(e.end1, EndCondition::Blind { depth: 5.0 });
    }
    m.doc.variables.insert("height".into(), "1in".into());
    m.rebuild_all();
    assert!((volume(&m) - 2540.0).abs() < 1e-6);
    let reload = Model::new(file::from_str(&file::to_string(&m.doc)).unwrap());
    assert!((volume(&reload) - volume(&m)).abs() < 1e-6);
    for expression in ["height + 1deg", "missing", "height / 0"] {
        m.doc
            .equations
            .insert("f2.end1.depth".into(), expression.into());
        m.rebuild_all();
        assert!(m.status(2).unwrap().error.is_some());
        assert!(m.state().body.is_none());
    }
}

#[test]
fn parameter_cycles_report_the_path() {
    let mut doc = document();
    doc.variables
        .insert("height".into(), "f2.end1.depth".into());
    doc.equations
        .insert("f2.end1.depth".into(), "height".into());
    let m = Model::new(doc);
    let error = m.status(2).unwrap().error.unwrap();
    assert!(
        error.contains("cycle") && error.contains("height") && error.contains("f2.end1.depth"),
        "{error}"
    );
}

#[test]
fn configurations_isolate_values_suppression_material_and_roundtrip() {
    let mut doc = document();
    doc.variables.insert("height".into(), "5mm".into());
    doc.equations
        .insert("f2.end1.depth".into(), "height".into());
    let mut tall = Configuration::new(2, "Tall".into());
    tall.variables.insert("height".into(), "12mm".into());
    tall.material = MaterialOverride::Set(materials::Material {
        name: "Test material".into(),
        density: 2000.0,
    });
    let mut hidden = Configuration::new(3, "Suppressed".into());
    hidden.suppressed.insert(2, true);
    doc.configurations.items.extend([tall, hidden]);
    doc.configurations.next_id = 4;
    let mut m = Model::new(doc);
    let base_names = m.state().body.as_ref().unwrap().names.clone();
    assert!((volume(&m) - 1000.0).abs() < 1e-6);
    m.doc.configurations.active = 2;
    m.rebuild_all();
    assert!((volume(&m) - 2400.0).abs() < 1e-6);
    let mass = measure::mass_properties(&m.doc, m.state()).unwrap();
    assert_eq!(mass.material.as_deref(), Some("Test material"));
    assert!((mass.mass - 0.0048).abs() < 1e-9);
    m.doc.configurations.active = 3;
    m.rebuild_all();
    assert!(m.state().body.is_none());
    assert!(m.status(2).unwrap().suppressed);
    m.doc.configurations.active = 1;
    m.rebuild_all();
    assert!((volume(&m) - 1000.0).abs() < 1e-6);
    assert_eq!(m.state().body.as_ref().unwrap().names, base_names);
    assert!(!m.doc.features[1].suppressed);
    assert!(m.doc.material.is_none());
    let loaded = file::from_str(&file::to_string(&m.doc)).unwrap();
    assert_eq!(m.doc.configurations, loaded.configurations);
}

#[test]
fn numeric_override_replaces_base_equation_and_delete_cleans_targets() {
    let mut doc = document();
    doc.equations.insert("f2.end1.depth".into(), "8mm".into());
    doc.configurations.items[0]
        .parameters
        .insert("f2.end1.depth".into(), 3.0);
    let mut m = Model::new(doc);
    assert!((volume(&m) - 600.0).abs() < 1e-6);
    m.doc.delete(&[2]);
    assert!(m.doc.equations.is_empty());
    assert!(m.doc.configurations.items[0].parameters.is_empty());
}

#[test]
fn editing_preview_rebuilds_earlier_formula_consumers_without_mutating_document() {
    let mut doc = file::from_str(include_str!("fixtures/v1-multisolid.ofp")).unwrap();
    doc.equations
        .insert("f2.end1.depth".into(), "f4.end1.depth".into());
    let mut m = Model::new(doc);
    let original = file::to_string(&m.doc);
    let mut edited = m.doc.feature(4).unwrap().kind.clone();
    if let FeatureKind::Extrude(e) = &mut edited {
        e.end1 = EndCondition::Blind { depth: 10.0 };
    }
    let preview = m.preview(&edited, Some(4)).unwrap();
    let preview_volume = preview
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume;
    assert!((preview_volume - 4000.0).abs() < 1e-6);
    assert_eq!(original, file::to_string(&m.doc));
    assert!((volume(&m) - 2000.0).abs() < 1e-6);
    m.doc.feature_mut(4).unwrap().kind = edited;
    m.invalidate(4);
    m.rebuild();
    assert!((volume(&m) - preview_volume).abs() < 1e-6);
}

#[test]
fn scalar_registration_covers_stations_and_vectors_without_registering_references() {
    let mut doc = Document::new();
    let id = doc.add(
        serde_json::from_value(serde_json::json!({
            "type":"variable_fillet", "edge":{"type":"edge","name":"1.e0"},
            "stations":[{"position":0.0,"radius":2.0},{"position":1.0,"radius":3.0}]
        }))
        .unwrap(),
    );
    let parameters = equations::parameters(doc.feature(id).unwrap());
    assert_eq!(parameters.len(), 4);
    for p in &parameters {
        assert_eq!(p.value.length, i16::from(p.path.ends_with("radius")));
    }
    let radius = parameters
        .iter()
        .find(|p| p.path == "stations.1.radius")
        .unwrap();
    equations::set_parameter(
        doc.feature_mut(id).unwrap(),
        radius,
        equations::Quantity {
            value: 4.0,
            ..radius.value
        },
    )
    .unwrap();
    assert_eq!(
        equations::parameters(doc.feature(id).unwrap())
            .iter()
            .find(|p| p.path == "stations.1.radius")
            .unwrap()
            .value
            .value,
        4.0
    );
    let projected = doc.add(
        serde_json::from_value(serde_json::json!({
            "type":"projected_curve", "source":{"type":"feature","id":1},
            "target":{"type":"face","name":"2.end"}, "direction":[0.0,0.0,-1.0]
        }))
        .unwrap(),
    );
    let parameters = equations::parameters(doc.feature(projected).unwrap());
    assert_eq!(parameters.len(), 3);
    assert!(
        parameters
            .iter()
            .all(|p| p.path.starts_with("direction.") && p.value.length == 0 && p.value.angle == 0)
    );
}

#[test]
fn integer_parameters_reject_fractional_dimensioned_and_nonpositive_formulas() {
    let mut doc = Document::new();
    let id = doc.add(
        serde_json::from_value(serde_json::json!({
            "type":"linear_pattern", "features":[],
            "direction1":{"reference":{"type":"right_plane"},"spacing":10.0,"count":3}
        }))
        .unwrap(),
    );
    let parameter = equations::parameters(doc.feature(id).unwrap())
        .into_iter()
        .find(|p| p.integer)
        .unwrap();
    for expression in ["1.5", "2mm", "0", "-1", "4294967296"] {
        doc.equations
            .insert(parameter.key.clone(), expression.into());
        assert!(
            equations::effective(&doc).values[&parameter.key].is_err(),
            "{expression}"
        );
    }
    doc.equations.insert(parameter.key.clone(), "2 + 3".into());
    assert_eq!(
        equations::effective(&doc).values[&parameter.key]
            .as_ref()
            .unwrap()
            .value,
        5.0
    );
}

#[test]
fn deleted_equation_target_cannot_drive_another_parameter() {
    let mut doc = document();
    doc.equations.insert("f99.radius".into(), "12mm".into());
    doc.equations
        .insert("f2.end1.depth".into(), "f99.radius".into());
    let evaluation = equations::effective(&doc);
    assert!(evaluation.errors[&2].contains("no longer exists"));
    assert!(evaluation.values["f2.end1.depth"].is_err());
}

fn assert_same_rebuild(a: &Model, b: &Model) {
    assert_eq!(a.statuses(), b.statuses());
    assert_eq!(a.state().bodies.len(), b.state().bodies.len());
    for (a, b) in a.state().bodies.iter().zip(&b.state().bodies) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.geometry.names, b.geometry.names);
        assert!(
            (a.geometry.shape.mass_props().unwrap().volume
                - b.geometry.shape.mass_props().unwrap().volume)
                .abs()
                < 1e-7
        );
        assert_eq!(
            a.geometry.shape.count(geom::Kind::Face),
            b.geometry.shape.count(geom::Kind::Face)
        );
        assert_eq!(
            a.geometry.shape.count(geom::Kind::Edge),
            b.geometry.shape.count(geom::Kind::Edge)
        );
    }
}

#[test]
fn incremental_edits_match_full_rebuild_for_geometry_equations_and_variants() {
    for formula in [false, true] {
        let mut doc = file::from_str(include_str!("fixtures/v1-multisolid.ofp")).unwrap();
        if formula {
            // A later feature drives an earlier one; invalidation must not retain it.
            doc.equations
                .insert("f2.end1.depth".into(), "f4.end1.depth / 2".into());
        }
        let mut model = Model::new(doc);
        for depth in [9.0, 3.0, 11.0] {
            if let FeatureKind::Extrude(e) = &mut model.doc.feature_mut(4).unwrap().kind {
                e.end1 = EndCondition::Blind { depth };
            }
            model.invalidate(4);
            model.rebuild();
            assert_same_rebuild(&model, &Model::new(model.doc.clone()));
        }
        let mut variant = Configuration::new(2, "Variant".into());
        variant.parameters.insert("f4.end1.depth".into(), 7.0);
        model.doc.configurations.items.push(variant);
        model.doc.configurations.next_id = 3;
        for active in [2, 1, 2, 1] {
            model.doc.configurations.active = active;
            model.invalidate(4);
            model.rebuild();
            assert_same_rebuild(&model, &Model::new(model.doc.clone()));
        }
        model.doc.configurations.items[1]
            .equations
            .insert("f4.end1.depth".into(), Some("missing".into()));
        model.doc.configurations.active = 2;
        model.invalidate(4);
        model.rebuild();
        assert!(model.status(4).unwrap().error.is_some());
        assert_same_rebuild(&model, &Model::new(model.doc.clone()));
        model.doc.configurations.active = 1;
        model.invalidate(4);
        model.rebuild();
        assert_same_rebuild(&model, &Model::new(model.doc.clone()));
    }
}

#[test]
fn spatial_distance_formulas_and_configuration_overrides_rebuild_without_base_leakage() {
    use of_sketch::spatial::{Axis3, Geometry3, Relation3, Sketch3D};
    let mut sketch = Sketch3D::default();
    let a = sketch.add_point(geom::DVec3::ZERO);
    let b = sketch.add_point(geom::DVec3::new(0.0, 0.0, 10.0));
    let line = sketch.add(Geometry3::Line { a, b });
    sketch.constrain(Relation3::Fixed {
        point: a,
        at: geom::DVec3::ZERO,
    });
    sketch.constrain(Relation3::Axis {
        line,
        axis: Axis3::Z,
    });
    let dimension = sketch.constrain(Relation3::Distance { a, b, value: 10.0 });
    let mut doc = Document::new();
    let feature = doc.add(FeatureKind::Sketch3d(sketch));
    let key = format!("f{feature}.dimension.{dimension}");
    assert_eq!(
        equations::parameters(doc.feature(feature).unwrap()).len(),
        1
    );
    doc.equations.insert(key.clone(), "2cm".into());
    let mut variant = Configuration::new(2, "Long spatial path".into());
    variant.parameters.insert(key.clone(), 30.0);
    doc.configurations.items.push(variant);
    doc.configurations.next_id = 3;
    let mut model = Model::new(doc);
    for (active, expected) in [(1, 20.0), (2, 30.0), (1, 20.0)] {
        model.doc.configurations.active = active;
        model.invalidate(feature);
        model.rebuild();
        assert!(
            model.status(feature).unwrap().error.is_none(),
            "{:?}",
            model.status(feature)
        );
        let solved = &model.state().sketches3d[&feature].0;
        assert!(
            (solved.point(a).unwrap().distance(solved.point(b).unwrap()) - expected).abs() < 1e-6
        );
        assert_eq!(
            equations::parameters(model.doc.feature(feature).unwrap())[0]
                .value
                .value,
            10.0
        );
    }
    if let FeatureKind::Sketch3d(sketch) = &mut model.doc.feature_mut(feature).unwrap().kind {
        sketch
            .constraints
            .retain(|constraint| constraint.id != dimension);
    }
    let evaluation = equations::effective(&model.doc);
    assert!(
        evaluation.values[&key]
            .as_ref()
            .unwrap_err()
            .contains("no longer exists")
    );
}

#[test]
fn removing_the_last_equation_invalidates_previously_driven_earlier_geometry() {
    let mut doc = file::from_str(include_str!("fixtures/v1-multisolid.ofp")).unwrap();
    doc.equations
        .insert("f2.end1.depth".into(), "2 * f4.end1.depth".into());
    let mut model = Model::new(doc);
    model.doc.equations.clear();
    // The last edited feature used to drive feature 2. Dropping the formula
    // must discard feature 2's evaluated cache even though no equations remain.
    model.invalidate(4);
    model.rebuild();
    assert_same_rebuild(&model, &Model::new(model.doc.clone()));
}
