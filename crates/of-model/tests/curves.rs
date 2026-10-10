use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::Sketch,
    *,
};
use std::f64::consts::PI;

fn circle(doc: &mut Document, plane: Ref, radius: f64) -> FeatureId {
    let mut sketch = Sketch::new();
    sketch.add_circle_at(DVec2::ZERO, radius);
    doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch }))
}
fn valid(m: &Model) {
    assert!(
        m.statuses().iter().all(|s| s.error.is_none()),
        "{:?}",
        m.statuses()
    );
    assert!(m.state().body.as_ref().unwrap().shape.is_valid());
}

#[test]
fn sweep_rebuilds_and_preserves_downstream_cap_reference() {
    let mut doc = Document::new();
    let profile = circle(&mut doc, Ref::FrontPlane, 2.0);
    let mut path = Sketch::new();
    path.add_line_at(DVec2::ZERO, DVec2::new(0.0, -10.0));
    let path = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::TopPlane,
        sketch: path,
    }));
    let sweep = doc.add(FeatureKind::Sweep(Sweep {
        profile,
        path: Ref::Feature { id: path },
        reverse_path: false,
        orientation: SweepOrientation::MinimumTwist,
        cut: false,
        merge: true,
    }));
    let mut m = Model::new(doc);
    valid(&m);
    let body = m.state().body.as_ref().unwrap();
    assert!((body.shape.mass_props().unwrap().volume - PI * 4.0 * 10.0).abs() < 1e-5);
    assert!(
        body.names.face(&format!("{sweep}.end")).is_some(),
        "{:?}",
        body.names.faces
    );
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::Face {
            name: format!("{sweep}.end"),
        },
        distance: 2.0,
        flip: false,
    }));
    m.doc.variables.insert("radius".into(), "3mm".into());
    // Replacing a profile with the same entity layout retains its side/cap roles.
    if let FeatureKind::Sketch(s) = &mut m.doc.feature_mut(profile).unwrap().kind {
        for entity in s.sketch.entities.values_mut() {
            if let sketch::Geometry::Circle { radius, .. } = &mut entity.geo {
                *radius = 3.0;
            }
        }
    }
    m.rebuild_all();
    valid(&m);
    assert!(m.status(plane).unwrap().error.is_none());
    assert!(
        (m.state()
            .body
            .as_ref()
            .unwrap()
            .shape
            .mass_props()
            .unwrap()
            .volume
            - PI * 9.0 * 10.0)
            .abs()
            < 1e-5
    );
    valid(&Model::new(
        file::from_str(&file::to_string(&m.doc)).unwrap(),
    ));
    let pattern = m.doc.add(FeatureKind::LinearPattern(LinearPattern {
        features: vec![sweep],
        direction1: PatternDirection {
            reference: Ref::RightPlane,
            spacing: 20.0,
            count: 2,
            reverse: false,
        },
        direction2: None,
    }));
    m.rebuild();
    valid(&m);
    assert!(m.status(pattern).unwrap().error.is_none());
    assert_eq!(m.state().bodies.len(), 2);
    assert!(
        (m.state()
            .body
            .as_ref()
            .unwrap()
            .shape
            .mass_props()
            .unwrap()
            .volume
            - PI * 9.0 * 20.0)
            .abs()
            < 1e-5
    );
}

#[test]
fn loft_between_sections_has_expected_frustum_volume() {
    let mut doc = Document::new();
    let bottom = circle(&mut doc, Ref::FrontPlane, 2.0);
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 12.0,
        flip: false,
    }));
    let top = circle(&mut doc, Ref::Feature { id: plane }, 4.0);
    doc.add(FeatureKind::Loft(Loft {
        profiles: vec![bottom, top],
        reversed: vec![],
        ruled: true,
        cut: false,
        merge: true,
    }));
    let m = Model::new(doc);
    valid(&m);
    assert!(
        (m.state()
            .body
            .as_ref()
            .unwrap()
            .shape
            .mass_props()
            .unwrap()
            .volume
            - PI * 12.0 * (4.0 + 8.0 + 16.0) / 3.0)
            .abs()
            < 1e-4
    );
}

#[test]
fn helix_pitch_and_handedness_and_disconnected_composite() {
    for left_handed in [false, true] {
        let mut doc = Document::new();
        let helix = doc.add(FeatureKind::Helix(Helix {
            plane: Ref::FrontPlane,
            radius: 10.0,
            pitch: 3.0,
            turns: 2.25,
            start_angle: 0.0,
            left_handed,
        }));
        let m = Model::new(doc);
        assert!(m.status(helix).unwrap().error.is_none());
        let shape = &m.state().curves[&helix].shape;
        assert_eq!(shape.count(Kind::Edge), 1);
        let info = shape.edge_info(0).unwrap();
        assert!((info.start - DVec3::new(10.0, 0.0, 0.0)).length() < 1e-6);
        assert!(
            (info.end - DVec3::new(0.0, if left_handed { -10.0 } else { 10.0 }, 6.75)).length()
                < 1e-6
        );
        let expected = (2.0 * PI * 10.0).hypot(3.0) * 2.25;
        assert!((info.length - expected).abs() < 1e-4);
    }
    let mut doc = Document::new();
    let mut ids = Vec::new();
    for x in [0.0, 10.0] {
        let mut sketch = Sketch::new();
        sketch.add_line_at(DVec2::new(x, 0.0), DVec2::new(x + 1.0, 0.0));
        ids.push(doc.add(FeatureKind::Sketch(SketchFeature {
            plane: Ref::FrontPlane,
            sketch,
        })));
    }
    let composite = doc.add(FeatureKind::CompositeCurve(CompositeCurve {
        segments: ids
            .iter()
            .map(|id| CurveSelection {
                reference: Ref::Feature { id: *id },
                reverse: false,
            })
            .collect(),
    }));
    let m = Model::new(doc);
    assert!(m.status(composite).unwrap().error.is_some());
}
