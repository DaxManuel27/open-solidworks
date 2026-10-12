use of_model::{FeatureKind, Model, file, geom::Kind};

#[test]
fn phase_one_files_migrate_without_geometry_or_identity_changes() {
    for (text, count) in [
        (include_str!("fixtures/v1-block.ofp"), 1),
        (include_str!("fixtures/v1-multisolid.ofp"), 2),
    ] {
        let doc = file::from_str(text).unwrap();
        let ids: Vec<_> = doc.features.iter().map(|f| f.id).collect();
        let mut model = Model::new(doc);
        assert!(model.statuses().iter().all(|s| s.error.is_none()));
        let body = model.state().body.as_ref().unwrap();
        assert_eq!(body.shape.count(Kind::Solid), count);
        assert!((body.shape.mass_props().unwrap().volume - 1000.0 * count as f64).abs() < 1e-6);
        let names = body.names.clone();
        let serialized = file::to_string(&model.doc);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&serialized).unwrap()["version"],
            file::VERSION
        );
        let loaded = Model::new(file::from_str(&serialized).unwrap());
        assert_eq!(
            ids,
            loaded.doc.features.iter().map(|f| f.id).collect::<Vec<_>>()
        );
        assert_eq!(names, loaded.state().body.as_ref().unwrap().names);
        let extrude = model
            .doc
            .features
            .iter_mut()
            .find(|f| matches!(f.kind, FeatureKind::Extrude(_)))
            .unwrap();
        let id = extrude.id;
        if let FeatureKind::Extrude(e) = &mut extrude.kind {
            e.end1 = of_model::EndCondition::Blind { depth: 8.0 };
        }
        model.invalidate(id);
        model.rebuild();
        assert!(model.statuses().iter().all(|s| s.error.is_none()));
        assert_eq!(
            model
                .state()
                .body
                .as_ref()
                .unwrap()
                .shape
                .count(Kind::Solid),
            count
        );
    }
}

#[test]
fn header_errors_precede_unknown_feature_errors() {
    let future =
        r#"{"format":"openforge-part","version":99,"document":{"features":[{"type":"future"}]}}"#;
    assert!(
        file::from_str(future)
            .unwrap_err()
            .contains("newer OpenForge")
    );
    assert!(
        file::from_str(&future.replace("openforge-part", "other"))
            .unwrap_err()
            .contains("not an OpenForge")
    );
    assert!(
        file::from_str(&future.replace("99", "0"))
            .unwrap_err()
            .contains("version 0")
    );
}

#[test]
fn version_two_parts_keep_authored_parameters_and_body_identity() {
    let mut source: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/v1-block.ofp")).unwrap();
    source["version"] = 2.into();
    let mut old = file::from_str(&source.to_string()).unwrap();
    old.variables.insert("depth".into(), "12mm".into());
    old.equations.insert("f2.end1.depth".into(), "depth".into());
    source["document"] = serde_json::to_value(&old).unwrap();
    let model = Model::new(file::from_str(&source.to_string()).unwrap());
    let reopened = Model::new(file::from_str(&file::to_string(&model.doc)).unwrap());
    assert!(model.statuses().iter().all(|s| s.error.is_none()));
    assert_eq!(model.doc, reopened.doc);
    assert_eq!(model.state().bodies[0].id, reopened.state().bodies[0].id);
    assert!(
        (reopened
            .state()
            .body
            .as_ref()
            .unwrap()
            .shape
            .mass_props()
            .unwrap()
            .volume
            - 2400.0)
            .abs()
            < 1e-6
    );
}

#[test]
fn corrupt_persistent_identity_is_rejected() {
    let original: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/v1-block.ofp")).unwrap();
    for field in ["duplicate", "next_id", "exhausted_id", "rollback"] {
        let mut value = original.clone();
        match field {
            "duplicate" => value["document"]["features"][1]["id"] = 1.into(),
            "next_id" => value["document"]["next_id"] = 1.into(),
            "exhausted_id" => value["document"]["next_id"] = u32::MAX.into(),
            _ => value["document"]["rollback"] = 99.into(),
        }
        assert!(
            file::from_str(&value.to_string())
                .unwrap_err()
                .contains("invalid document")
        );
    }
}
