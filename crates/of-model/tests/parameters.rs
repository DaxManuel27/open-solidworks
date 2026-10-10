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
