use of_model::{
    geom::{DVec2, DVec3, Kind, Shape},
    sketch::{
        Sketch, SolveStatus,
        spatial::{Axis3, Geometry3, Relation3, Sketch3D},
    },
    *,
};
use std::f64::consts::PI;

fn circle(doc: &mut Document, center: DVec2, radius: f64) -> FeatureId {
    let mut sketch = Sketch::new();
    sketch.add_circle_at(center, radius);
    doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }))
}

fn sweep(doc: &mut Document, profile: FeatureId, path: Ref) -> FeatureId {
    doc.add(FeatureKind::Sweep(Sweep {
        profile: Some(profile.into()),
        inner_profiles: vec![],
        circle_diameter: None,
        twist: 0.0,
        guide: None,
        guide_scale: false,
        guide2: None,
        thin: None,
        path,
        reverse_path: false,
        orientation: SweepOrientation::MinimumTwist,
        cut: false,
        merge: false,
    }))
}

fn volume(model: &Model) -> f64 {
    let shape = &model.state().body.as_ref().unwrap().shape;
    assert!(shape.is_valid());
    shape.mass_props().unwrap().volume
}

fn valid(model: &Model) {
    assert!(
        model.statuses().iter().all(|s| s.error.is_none()),
        "{:?}",
        model.statuses()
    );
}

fn dimensioned_fixture() -> (Model, FeatureId, FeatureId, u32, u32) {
    let mut doc = Document::new();
    let base = circle(&mut doc, DVec2::X * 30., 2.);
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: base,
        cut: false,
        start_offset: 0.,
        end1: EndCondition::Blind { depth: 5. },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: false,
    }));
    let profile = circle(&mut doc, DVec2::ZERO, 1.);
    let mut path = Sketch3D::default();
    let a = path.add_point(DVec3::ZERO);
    let b = path.add_point(DVec3::Z * 10.);
    let line = path.add(Geometry3::Line { a, b });
    path.constrain(Relation3::Fixed {
        point: a,
        at: DVec3::ZERO,
    });
    path.constrain(Relation3::Axis {
        line,
        axis: Axis3::Z,
    });
    let dimension = path.constrain(Relation3::Distance { a, b, value: 10. });
    let source = doc.add(FeatureKind::Sketch3d(path));
    let sweep = sweep(&mut doc, profile, Ref::Feature { id: source });
    (Model::new(doc), source, sweep, dimension, b)
}

#[test]
fn spatial_dimension_drives_sweep_preview_rebuild_reload_and_export() {
    let (mut model, source, sweep, dimension, _) = dimensioned_fixture();
    valid(&model);
    assert_eq!(model.state().bodies.len(), 2);
    assert!((volume(&model) - 30. * PI).abs() < 1e-5);
    assert_eq!(
        model.state().sketches3d[&source].1.status,
        SolveStatus::Solved
    );
    assert_eq!(model.state().sketches3d[&source].1.dof, 0);
    model
        .doc
        .variables
        .insert("pathLength".into(), "25mm".into());
    model.doc.equations.insert(
        format!("f{source}.dimension.{dimension}"),
        "pathLength".into(),
    );
    model.invalidate(source);
    model.rebuild();
    valid(&model);
    let expected = 45. * PI;
    assert!((volume(&model) - expected).abs() < 1e-5);
    let params = model.doc.feature(sweep).unwrap().kind.clone();
    let preview = model.preview(&params, Some(sweep)).unwrap();
    assert!((preview.body.unwrap().shape.mass_props().unwrap().volume - expected).abs() < 1e-5);
    let names = model.state().body.as_ref().unwrap().names.clone();
    model.rebuild_all();
    assert!((volume(&model) - expected).abs() < 1e-5);
    assert_eq!(model.state().body.as_ref().unwrap().names, names);
    let reopened = Model::new(file::from_str(&file::to_string(&model.doc)).unwrap());
    valid(&reopened);
    assert!((volume(&reopened) - expected).abs() < 1e-5);
    let stem = std::env::temp_dir().join(format!("openforge-spatial-path-{}", std::process::id()));
    let step = stem.with_extension("step");
    let stl = stem.with_extension("stl");
    let shape = &reopened.state().body.as_ref().unwrap().shape;
    shape.write_step(step.to_str().unwrap()).unwrap();
    let imported = Shape::read_step(step.to_str().unwrap()).unwrap();
    assert!(imported.is_valid());
    assert_eq!(imported.count(Kind::Solid), 2);
    assert!((imported.mass_props().unwrap().volume - expected).abs() < 1e-5);
    shape
        .write_stl(stl.to_str().unwrap(), 0.02, 0.1, false)
        .unwrap();
    assert!(std::fs::metadata(&stl).unwrap().len() > 84);
    std::fs::remove_file(step).unwrap();
    std::fs::remove_file(stl).unwrap();
}

#[test]
fn conflicting_or_suppressed_spatial_path_preserves_prior_body_and_recovers() {
    let (mut model, source, consumer, _, b) = dimensioned_fixture();
    let original = model.doc.clone();
    if let FeatureKind::Sketch3d(path) = &mut model.doc.feature_mut(source).unwrap().kind {
        path.constrain(Relation3::Fixed {
            point: b,
            at: DVec3::Z * 20.,
        });
    }
    model.invalidate(source);
    model.rebuild();
    assert!(model.status(source).unwrap().error.is_some());
    assert!(
        model
            .status(consumer)
            .unwrap()
            .error
            .unwrap()
            .contains("conflicting constraints")
    );
    assert!((volume(&model) - 20. * PI).abs() < 1e-5);
    assert_eq!(
        model.state().sketches3d[&source].0.point(b).unwrap(),
        DVec3::Z * 10.
    );
    model.doc = original.clone();
    model.rebuild_all();
    valid(&model);
    assert!((volume(&model) - 30. * PI).abs() < 1e-5);
    model.doc.feature_mut(source).unwrap().suppressed = true;
    model.invalidate(source);
    model.rebuild();
    assert!(model.status(consumer).unwrap().error.is_some());
    assert!((volume(&model) - 20. * PI).abs() < 1e-5);
    model.doc = original;
    model.doc.rollback = Some(model.doc.index_of(source).unwrap());
    model.rebuild_all();
    assert!(model.status(source).unwrap().rolled_back);
    assert!(model.status(consumer).unwrap().rolled_back);
    assert!((volume(&model) - 20. * PI).abs() < 1e-5);
    model.doc.rollback = None;
    model.rebuild_all();
    valid(&model);
    assert!((volume(&model) - 30. * PI).abs() < 1e-5);
}

#[test]
fn spatial_lines_and_cubic_segments_ignore_construction_and_reject_branches() {
    let mut doc = Document::new();
    let profile = circle(&mut doc, DVec2::ZERO, 0.5);
    let mut path = Sketch3D::default();
    let points: Vec<_> = [
        DVec3::ZERO,
        DVec3::Z * 10.,
        DVec3::Z * 15.,
        DVec3::new(5., 5., 20.),
        DVec3::new(10., 10., 20.),
    ]
    .into_iter()
    .map(|p| path.add_point(p))
    .collect();
    let line = path.add(Geometry3::Line {
        a: points[0],
        b: points[1],
    });
    let cubic = path.add(Geometry3::Bezier {
        points: [points[1], points[2], points[3], points[4]],
    });
    let construction = path.add(Geometry3::Line {
        a: points[1],
        b: points[3],
    });
    path.entities
        .iter_mut()
        .find(|e| e.id == construction)
        .unwrap()
        .construction = true;
    let source = doc.add(FeatureKind::Sketch3d(path));
    let curve = doc.add(FeatureKind::CompositeCurve(CompositeCurve {
        segments: vec![CurveSelection {
            reference: Ref::Feature { id: source },
            reverse: false,
        }],
    }));
    let consumer = sweep(&mut doc, profile, Ref::Feature { id: curve });
    let mut model = Model::new(doc);
    valid(&model);
    let path_shape = &model.state().curves[&curve].shape;
    assert_eq!(path_shape.count(Kind::Edge), 2);
    let length: f64 = (0..2)
        .map(|e| path_shape.edge_info(e).unwrap().length)
        .sum();
    assert!((volume(&model) - 0.25 * PI * length).abs() < 1e-3);
    for entity in [line, cubic] {
        assert!(
            model.state().curves[&curve]
                .segments
                .contains_key(&format!("sketch3d:{source}/entity:{entity}"))
        );
    }
    let selected = model.doc.add(FeatureKind::CompositeCurve(CompositeCurve {
        segments: vec![CurveSelection {
            reference: Ref::SketchEntity {
                sketch: source,
                entity: cubic,
            },
            reverse: true,
        }],
    }));
    model.rebuild();
    valid(&model);
    assert_eq!(model.state().curves[&selected].shape.count(Kind::Edge), 1);
    if let FeatureKind::Sketch3d(path) = &mut model.doc.feature_mut(source).unwrap().kind {
        path.entities
            .iter_mut()
            .find(|e| e.id == construction)
            .unwrap()
            .construction = false;
    }
    model.invalidate(source);
    model.rebuild();
    assert!(
        model
            .status(curve)
            .unwrap()
            .error
            .unwrap()
            .contains("branches")
    );
    assert!(model.status(consumer).unwrap().error.is_some());
}

#[test]
fn spatial_relations_follow_model_geometry() {
    let mut doc = Document::new();
    let mut rect = Sketch::new();
    of_model::sketch::tools::corner_rectangle(&mut rect, DVec2::ZERO, DVec2::new(40.0, 20.0));
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: rect,
    }));
    let block = doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.,
        end1: EndCondition::Blind { depth: 10. },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let mut m = Model::new(doc);
    let body = m.state().body.as_ref().unwrap();
    let corner = (0..body.shape.count(Kind::Vertex))
        .find(|&v| {
            (body.shape.vertex_point(v).unwrap() - DVec3::new(40., 20., 10.)).length() < 1e-9
        })
        .unwrap();
    let corner = m.state().ref_for(Kind::Vertex, corner).unwrap();
    let top = (0..body.shape.count(Kind::Face))
        .find(|&f| body.shape.face_info(f).unwrap().normal.z > 0.999)
        .unwrap();
    let top = m.state().ref_for(Kind::Face, top).unwrap();

    let mut spatial = Sketch3D::default();
    let a = spatial.add_point(DVec3::new(40., 20., 10.));
    let b = spatial.add_point(DVec3::new(0., 0., 10.));
    spatial.add(Geometry3::Line { a, b });
    let fixed = spatial.constrain(Relation3::Fixed {
        point: a,
        at: DVec3::new(40., 20., 10.),
    });
    let on_top = spatial.constrain(Relation3::PointOnPlane {
        point: b,
        origin: DVec3::new(0., 0., 10.),
        normal: DVec3::Z,
    });
    for c in &mut spatial.constraints {
        if c.id == fixed {
            c.external = Some(serde_json::to_string(&corner).unwrap());
        }
        if c.id == on_top {
            c.external = Some(serde_json::to_string(&top).unwrap());
        }
    }
    let id = m.doc.add(FeatureKind::Sketch3d(spatial));
    m.rebuild();
    valid(&m);

    // Deepen the block: the fixed point follows the corner, the other point the top.
    if let FeatureKind::Extrude(e) = &mut m.doc.feature_mut(block).unwrap().kind {
        e.end1 = EndCondition::Blind { depth: 15. };
    }
    m.invalidate(block);
    m.rebuild();
    valid(&m);
    let (solved, _) = &m.state().sketches3d[&id];
    assert!((solved.point(a).unwrap() - DVec3::new(40., 20., 15.)).length() < 1e-9);
    assert!((solved.point(b).unwrap().z - 15.).abs() < 1e-9);

    // A reference to geometry that disappears fails the sketch.
    if let FeatureKind::Sketch3d(s) = &mut m.doc.feature_mut(id).unwrap().kind {
        s.constraints[0].external = Some(r#"{"type":"vertex","name":"nope"}"#.into());
    }
    m.invalidate(id);
    m.rebuild();
    assert!(
        m.status(id)
            .unwrap()
            .error
            .unwrap()
            .contains("no longer exists")
    );
}
