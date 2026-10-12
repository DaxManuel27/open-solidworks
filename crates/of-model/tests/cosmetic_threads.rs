use of_model::{
    configurations::Configuration,
    cosmetic_thread::annotation,
    geom::{DVec2, DVec3, Kind, SurfaceKind},
    sketch::Sketch,
    *,
};
use std::f64::consts::PI;

fn cylinder(internal: bool) -> (Model, FeatureId, CosmeticThread) {
    let mut doc = Document::new();
    let mut sketch = Sketch::new();
    sketch.add_circle_at(DVec2::ZERO, 5.0);
    if internal {
        sketch.add_circle_at(DVec2::ZERO, 2.0);
    }
    let sketch = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    let extrusion = doc.add(
        serde_json::from_value(serde_json::json!({
            "type":"extrude","sketch":sketch,"end1":{"type":"blind","depth":20.0}
        }))
        .unwrap(),
    );
    let model = Model::new(doc);
    let body = model.state().body.as_ref().unwrap();
    let radius = if internal { 2.0 } else { 5.0 };
    let index = (0..body.shape.count(Kind::Face))
        .find(|i| {
            let f = body.shape.face_info(*i).unwrap();
            f.kind == SurfaceKind::Cylinder && (f.radius - radius).abs() < 1e-8
        })
        .unwrap();
    let p = CosmeticThread {
        face: model.state().ref_for(Kind::Face, index).unwrap(),
        diameter: 2.0 * radius,
        pitch: 2.0,
        length: 12.0,
        start_offset: 1.0,
        start_angle: 0.0,
        reverse: false,
        left_handed: false,
        internal,
    };
    (model, extrusion, p)
}
fn mass(model: &Model) -> f64 {
    model
        .state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume
}
fn valid(model: &Model) {
    assert!(
        model.statuses().iter().all(|s| s.error.is_none()),
        "{:?}",
        model.statuses()
    );
}

#[test]
fn cosmetic_threads_follow_cylinders_with_measured_pitch_and_leave_mass_and_topology_unchanged() {
    for internal in [false, true] {
        let (mut model, extrusion, p) = cylinder(internal);
        let original_volume = mass(&model);
        let names = model.state().body.as_ref().unwrap().names.clone();
        let preview = model
            .preview(&FeatureKind::CosmeticThread(p.clone()), None)
            .unwrap();
        assert_eq!(preview.cosmetic_threads.values().next(), Some(&p));
        let id = model.doc.add(FeatureKind::CosmeticThread(p.clone()));
        model.rebuild();
        valid(&model);
        assert_eq!(model.doc.feature(id).unwrap().depends_on(), vec![extrusion]);
        assert!((mass(&model) - original_volume).abs() < 1e-8);
        assert_eq!(model.state().body.as_ref().unwrap().names, names);
        let geometry = annotation(&p, model.state()).unwrap();
        let expected_samples_per_turn = 48;
        let advance = geometry.points[expected_samples_per_turn] - geometry.points[0];
        assert!((advance.dot(geometry.axis) - p.pitch).abs() < 1e-8);
        assert!(
            ((geometry.points.last().unwrap() - geometry.points[0]).dot(geometry.axis) - p.length)
                .abs()
                < 1e-8
        );
        for point in &geometry.points {
            let d = *point - geometry.start;
            assert!(
                ((d - geometry.axis * d.dot(geometry.axis)).length() - p.diameter / 2.0).abs()
                    < 1e-8
            );
        }
        let left = annotation(
            &CosmeticThread {
                left_handed: true,
                ..p.clone()
            },
            model.state(),
        )
        .unwrap();
        let side = |points: &[DVec3]| {
            (points[0] - geometry.start)
                .cross(points[1] - geometry.start)
                .dot(geometry.axis)
        };
        assert!(side(&geometry.points) > 0.0 && side(&left.points) < 0.0);
        let reversed = annotation(
            &CosmeticThread {
                reverse: true,
                ..p.clone()
            },
            model.state(),
        )
        .unwrap();
        assert!(reversed.axis.dot(geometry.axis) < -0.999999);
        let reloaded = Model::new(file::from_str(&file::to_string(&model.doc)).unwrap());
        valid(&reloaded);
        assert_eq!(reloaded.state().cosmetic_threads[&id], p);
        assert_eq!(
            annotation(&p, reloaded.state()).unwrap().points,
            geometry.points
        );
    }
}

#[test]
fn annotation_length_configuration_suppression_rollback_and_upstream_edits_are_isolated() {
    let (mut model, extrusion, p) = cylinder(false);
    let id = model.doc.add(FeatureKind::CosmeticThread(p.clone()));
    let key = format!("f{id}.length");
    model.doc.equations.insert(key.clone(), "8mm".into());
    let mut variant = Configuration::new(2, "Long annotation".into());
    variant.parameters.insert(key, 16.0);
    model.doc.configurations.items.push(variant);
    model.doc.configurations.next_id = 3;
    for (active, length) in [(1, 8.0), (2, 16.0), (1, 8.0)] {
        model.doc.configurations.active = active;
        model.rebuild_all();
        valid(&model);
        assert_eq!(model.state().cosmetic_threads[&id].length, length);
        assert!((mass(&model) - 500.0 * PI).abs() < 1e-7);
        assert_eq!(
            model.doc.feature(id).unwrap().kind,
            FeatureKind::CosmeticThread(p.clone())
        );
    }
    model.doc.feature_mut(id).unwrap().suppressed = true;
    model.rebuild_all();
    assert!(model.state().cosmetic_threads.is_empty());
    model.doc.feature_mut(id).unwrap().suppressed = false;
    model.doc.rollback = Some(model.doc.index_of(id).unwrap());
    model.rebuild_all();
    assert!(model.state().cosmetic_threads.is_empty());
    model.doc.rollback = None;
    if let FeatureKind::Extrude(extrude) = &mut model.doc.feature_mut(extrusion).unwrap().kind {
        extrude.end1 = EndCondition::Blind { depth: 5.0 };
    }
    model.rebuild_all();
    assert!(model.status(id).unwrap().error.unwrap().contains("exceeds"));
    assert!((mass(&model) - 125.0 * PI).abs() < 1e-7);
}

#[test]
fn downstream_body_moves_transform_the_annotation_and_bad_parameters_preserve_the_body() {
    let (mut model, _, p) = cylinder(false);
    let id = model.doc.add(FeatureKind::CosmeticThread(p.clone()));
    model.rebuild();
    let before = annotation(&p, model.state()).unwrap();
    let moved = model.doc.add(FeatureKind::MoveBody(MoveBody {
        bodies: vec![model.state().bodies[0].id.clone()],
        translation: DVec3::new(3.0, 4.0, 7.0),
        rotation: DVec3::new(PI, 0.0, 0.0),
        origin: DVec3::ZERO,
        copy: false,
    }));
    model.rebuild();
    valid(&model);
    let after = annotation(&p, model.state()).unwrap();
    let transformed = |p: DVec3| DVec3::new(p.x + 3.0, -p.y + 4.0, -p.z + 7.0);
    assert!(after.start.distance(transformed(before.start)) < 1e-8);
    for (a, b) in before.points.iter().zip(&after.points) {
        assert!(transformed(*a).distance(*b) < 1e-8);
    }
    model.doc.delete(&[moved]);
    model.rebuild_all();
    for bad in [
        CosmeticThread {
            diameter: 12.0,
            ..p.clone()
        },
        CosmeticThread {
            length: 30.0,
            ..p.clone()
        },
        CosmeticThread {
            pitch: 0.0,
            ..p.clone()
        },
        CosmeticThread {
            pitch: 0.00001,
            ..p.clone()
        },
        CosmeticThread {
            internal: true,
            ..p.clone()
        },
        CosmeticThread {
            face: Ref::FrontPlane,
            ..p.clone()
        },
    ] {
        model.doc.feature_mut(id).unwrap().kind = FeatureKind::CosmeticThread(bad);
        model.invalidate(id);
        model.rebuild();
        assert!(model.status(id).unwrap().error.is_some());
        assert!(model.state().cosmetic_threads.is_empty());
        assert!((mass(&model) - 500.0 * PI).abs() < 1e-7);
    }
}

#[test]
fn downstream_trimming_reports_unresolved_annotation_instead_of_drawing_a_full_thread() {
    let (mut model, _, p) = cylinder(false);
    let id = model.doc.add(FeatureKind::CosmeticThread(p.clone()));
    model.rebuild();
    let split = model.doc.add(
        serde_json::from_value(serde_json::json!({
            "type":"split_body", "bodies":[model.state().bodies[0].id],
            "plane":{"type":"right_plane"}, "keep":"positive"
        }))
        .unwrap(),
    );
    model.rebuild();
    assert!(
        model.status(split).unwrap().error.is_none(),
        "{:?}",
        model.status(split)
    );
    assert!(model.status(id).unwrap().error.is_some());
    assert!(annotation(&p, model.state()).is_err());
    assert!((mass(&model) - 250.0 * PI).abs() < 1e-7);
    model.doc.delete(&[split]);
    model.rebuild_all();
    valid(&model);
    assert!(annotation(&p, model.state()).is_ok());
}
