use of_model::{
    geom::{DVec2, DVec3, Kind, SurfaceKind},
    sketch::Sketch,
    *,
};
use std::f64::consts::PI;

fn cylinder(radius: f64, depth: f64) -> Model {
    let mut doc = Document::new();
    let mut sketch = Sketch::new();
    sketch.add_circle_at(DVec2::ZERO, radius);
    let sketch = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    Model::new(doc)
}
fn valid(model: &Model) {
    assert!(
        model.statuses().iter().all(|s| s.error.is_none()),
        "{:?}",
        model.statuses()
    );
    assert!(model.state().body.as_ref().unwrap().shape.is_valid());
}
fn volume(model: &Model) -> f64 {
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
fn cap_volume(radius: f64, height: f64) -> f64 {
    PI * height * (3.0 * radius * radius + height * height) / 6.0
}
fn top(model: &Model) -> Ref {
    let body = model.state().body.as_ref().unwrap();
    let face = (0..body.shape.count(Kind::Face))
        .find(|&i| {
            let f = body.shape.face_info(i).unwrap();
            f.kind == SurfaceKind::Plane && f.normal.z > 0.99
        })
        .unwrap();
    model.state().qualified_ref_for(Kind::Face, face).unwrap()
}

#[test]
fn dome_preview_commit_naming_upstream_edit_and_reload() {
    let mut model = cylinder(10.0, 8.0);
    let body_id = model.state().bodies[0].id.clone();
    let face = top(&model);
    let feature = FeatureKind::Dome(Dome {
        face,
        height: 3.0,
        inward: false,
        elliptical: false,
    });
    let preview = model.preview(&feature, None).unwrap();
    let id = model.doc.add(feature);
    model.rebuild();
    valid(&model);
    assert_eq!(
        preview.body.as_ref().unwrap().names,
        model.state().body.as_ref().unwrap().names
    );
    assert_eq!(model.state().bodies[0].id, body_id);
    assert!((volume(&model) - (800.0 * PI + cap_volume(10.0, 3.0))).abs() < 1e-5);
    let names = model.state().body.as_ref().unwrap().names.clone();
    let cap = model
        .state()
        .body
        .as_ref()
        .unwrap()
        .names
        .faces
        .iter()
        .flatten()
        .find(|n| n.starts_with(&format!("{id}.dome")))
        .unwrap()
        .clone();
    if let FeatureKind::Extrude(extrude) = &mut model.doc.features[1].kind {
        extrude.end1 = EndCondition::Blind { depth: 9.0 };
    }
    model.invalidate(2);
    model.rebuild();
    valid(&model);
    assert!(model.resolve(&Ref::Face { name: cap }).is_ok());
    assert_eq!(names, model.state().body.as_ref().unwrap().names);
    assert!((volume(&model) - (900.0 * PI + cap_volume(10.0, 3.0))).abs() < 1e-5);
    if let FeatureKind::Sketch(sketch) = &mut model.doc.features[0].kind {
        for entity in sketch.sketch.entities.values_mut() {
            if let sketch::Geometry::Circle { radius, .. } = &mut entity.geo {
                *radius = 12.0;
            }
        }
    }
    model.invalidate(1);
    model.rebuild();
    valid(&model);
    assert_eq!(names, model.state().body.as_ref().unwrap().names);
    assert!((volume(&model) - (1296.0 * PI + cap_volume(12.0, 3.0))).abs() < 1e-5);
    let saved = file::to_string(&model.doc);
    let reloaded = Model::new(file::from_str(&saved).unwrap());
    valid(&reloaded);
    assert!((volume(&reloaded) - volume(&model)).abs() < 1e-6);
    assert_eq!(reloaded.state().body.as_ref().unwrap().names, names);
    model.doc.feature_mut(id).unwrap().suppressed = true;
    model.invalidate(id);
    model.rebuild();
    valid(&model);
    assert!((volume(&model) - 1296.0 * PI).abs() < 1e-5);
}

#[test]
fn inward_dome_height_equations_configurations_and_failure_recovery() {
    let mut model = cylinder(10.0, 8.0);
    let face = top(&model);
    let dome = model.doc.add(FeatureKind::Dome(Dome {
        face,
        height: 3.0,
        inward: true,
        elliptical: false,
    }));
    model
        .doc
        .variables
        .insert("cap_height".into(), "2mm".into());
    model
        .doc
        .equations
        .insert(format!("f{dome}.height"), "cap_height".into());
    model.rebuild();
    valid(&model);
    assert!((volume(&model) - (800.0 * PI - cap_volume(10.0, 2.0))).abs() < 1e-5);
    let mut config = configurations::Configuration::new(2, "Deep".into());
    config.variables.insert("cap_height".into(), "4mm".into());
    model.doc.configurations.items.push(config);
    model.doc.configurations.next_id = 3;
    model.doc.configurations.active = 2;
    model.rebuild_all();
    valid(&model);
    assert!((volume(&model) - (800.0 * PI - cap_volume(10.0, 4.0))).abs() < 1e-5);
    model.doc.configurations.active = 1;
    model.rebuild_all();
    valid(&model);
    assert!((volume(&model) - (800.0 * PI - cap_volume(10.0, 2.0))).abs() < 1e-5);
    model
        .doc
        .variables
        .insert("cap_height".into(), "9mm".into());
    model.rebuild_all();
    assert!(model.status(dome).unwrap().error.is_some());
    assert!((volume(&model) - 800.0 * PI).abs() < 1e-5);
}

#[test]
fn dome_downstream_fillet_cut_and_exports_are_valid() {
    let mut model = cylinder(10.0, 8.0);
    let face = top(&model);
    model.doc.add(FeatureKind::Dome(Dome {
        face,
        height: 3.0,
        inward: false,
        elliptical: false,
    }));
    model.rebuild();
    valid(&model);
    let body = model.state().body.as_ref().unwrap();
    let rim = (0..body.shape.count(Kind::Edge))
        .find(|&i| {
            let edge = body.shape.edge_info(i).unwrap();
            edge.kind == geom::CurveKind::Circle
                && (edge.center.z - 8.0).abs() < 1e-6
                && (edge.radius - 10.0).abs() < 1e-6
        })
        .unwrap();
    let edge = model.state().qualified_ref_for(Kind::Edge, rim).unwrap();
    model.doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![edge],
        faces: vec![],
        radius: 0.25,
    }));
    model.rebuild();
    valid(&model);
    let mut sketch = Sketch::new();
    sketch.add_circle_at(DVec2::ZERO, 1.0);
    let sketch = model.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    model.doc.add(FeatureKind::Extrude(Extrude {
        sketch,
        cut: true,
        start_offset: 0.0,
        end1: EndCondition::ThroughAll,
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    model.rebuild();
    valid(&model);
    assert!(volume(&model) < 800.0 * PI + cap_volume(10.0, 3.0));
    let shape = &model.state().body.as_ref().unwrap().shape;
    let path = std::env::temp_dir().join(format!("openforge-dome-{}.step", std::process::id()));
    shape.write_step(path.to_str().unwrap()).unwrap();
    let imported = geom::Shape::read_step(path.to_str().unwrap()).unwrap();
    assert!(imported.is_valid());
    assert!((imported.mass_props().unwrap().volume - volume(&model)).abs() < 1e-4);
    let stl = path.with_extension("stl");
    shape
        .write_stl(stl.to_str().unwrap(), 0.05, 0.3, false)
        .unwrap();
    assert!(std::fs::metadata(&stl).unwrap().len() > 100);
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(stl);
}

#[test]
fn dome_scope_preserves_other_bodies_and_reports_wrong_scope() {
    let mut model = cylinder(10.0, 8.0);
    let first = model.state().bodies[0].id.clone();
    let face = top(&model);
    model.doc.add(FeatureKind::MoveBody(MoveBody {
        bodies: vec![first.clone()],
        translation: DVec3::new(30.0, 0.0, 0.0),
        rotation: DVec3::ZERO,
        origin: DVec3::ZERO,
        copy: true,
        ..Default::default()
    }));
    model.rebuild();
    valid(&model);
    let second = model.state().bodies[1].id.clone();
    let dome = model.doc.add(FeatureKind::Dome(Dome {
        face,
        height: 3.0,
        inward: false,
        elliptical: false,
    }));
    model.doc.feature_mut(dome).unwrap().scope = Some(vec![first]);
    model.rebuild();
    valid(&model);
    assert!(
        (model
            .state()
            .solid(&second)
            .unwrap()
            .geometry
            .shape
            .mass_props()
            .unwrap()
            .volume
            - 800.0 * PI)
            .abs()
            < 1e-5
    );
    model.doc.feature_mut(dome).unwrap().scope = Some(vec![second]);
    model.invalidate(dome);
    model.rebuild();
    assert!(
        model
            .status(dome)
            .unwrap()
            .error
            .unwrap()
            .contains("no longer exists")
    );
    assert!((volume(&model) - 1600.0 * PI).abs() < 1e-5);
}

#[test]
fn dome_delta_patterns_attach_to_copied_supporting_bodies() {
    let mut model = cylinder(10.0, 8.0);
    let body = model.state().bodies[0].id.clone();
    let face = top(&model);
    model.doc.add(FeatureKind::MoveBody(MoveBody {
        bodies: vec![body],
        translation: DVec3::new(30.0, 0.0, 0.0),
        rotation: DVec3::ZERO,
        origin: DVec3::ZERO,
        copy: true,
        ..Default::default()
    }));
    model.rebuild();
    let dome = model.doc.add(FeatureKind::Dome(Dome {
        face,
        height: 3.0,
        inward: false,
        elliptical: false,
    }));
    model.doc.add(FeatureKind::LinearPattern(LinearPattern {
        features: vec![dome],
        direction1: PatternDirection {
            reference: Ref::RightPlane,
            spacing: 30.0,
            count: 2,
            reverse: false,
        },
        direction2: None,
    }));
    model.rebuild();
    valid(&model);
    assert_eq!(model.state().bodies.len(), 2);
    assert!((volume(&model) - (1600.0 * PI + 2.0 * cap_volume(10.0, 3.0))).abs() < 1e-5);
}

#[test]
fn shared_split_face_requires_explicit_owner_scope_for_direction() {
    let mut model = cylinder(10.0, 8.0);
    let original = model.state().bodies[0].id.clone();
    let plane = model.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 4.0,
        flip: false,
    }));
    model.doc.add(FeatureKind::SplitBody(SplitBody {
        bodies: vec![original],
        plane: Ref::Feature { id: plane },
        keep: KeepSide::Both,
        tools: vec![],
        retained_pieces: None,
    }));
    model.rebuild();
    valid(&model);
    let upper = model
        .state()
        .bodies
        .iter()
        .find(|b| b.geometry.shape.mass_props().unwrap().center.z > 4.0)
        .unwrap();
    let upper_id = upper.id.clone();
    let face = (0..upper.geometry.shape.count(Kind::Face))
        .find(|&i| {
            let f = upper.geometry.shape.face_info(i).unwrap();
            f.kind == SurfaceKind::Plane && (f.centroid.z - 4.0).abs() < 1e-6
        })
        .unwrap();
    let reference = Ref::BodyTopology {
        body: upper_id.clone(),
        kind: Kind::Face,
        name: upper.geometry.names.face_name(face).unwrap().into(),
    };
    let dome = model.doc.add(FeatureKind::Dome(Dome {
        face: reference,
        height: 1.0,
        inward: true,
        elliptical: false,
    }));
    model.rebuild();
    assert!(
        model
            .status(dome)
            .unwrap()
            .error
            .unwrap()
            .contains("Feature scope")
    );
    model.doc.feature_mut(dome).unwrap().scope = Some(vec![upper_id.clone()]);
    model.invalidate(dome);
    model.rebuild();
    valid(&model);
    assert!(
        (model
            .state()
            .solid(&upper_id)
            .unwrap()
            .geometry
            .shape
            .mass_props()
            .unwrap()
            .volume
            - (400.0 * PI - cap_volume(10.0, 1.0)))
        .abs()
            < 1e-5
    );
}
