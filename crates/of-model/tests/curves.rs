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
        profile: Some(profile.into()),
        inner_profiles: vec![],
        circle_diameter: None,
        twist: 0.0,
        guide: None,
        guide_scale: false,
        guide2: None,
        thin: None,
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
        profiles: vec![bottom.into(), top.into()],
        reversed: vec![],
        ruled: true,
        guides: vec![],
        closed: false,
        centerline: None,
        start: LoftEnd::None,
        end: LoftEnd::None,
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
            defined_by: HelixInput::PitchAndTurns,
            height: 0.0,
            variable_pitch: vec![],
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

fn helix_end(spec: Helix) -> (DVec3, f64) {
    let mut doc = Document::new();
    let helix = doc.add(FeatureKind::Helix(spec));
    let m = Model::new(doc);
    assert!(
        m.status(helix).unwrap().error.is_none(),
        "{:?}",
        m.status(helix)
    );
    let info = m.state().curves[&helix].shape.edge_info(0).unwrap();
    (info.end, info.length)
}

fn base_helix() -> Helix {
    Helix {
        plane: Ref::FrontPlane,
        radius: 10.0,
        pitch: 3.0,
        turns: 2.0,
        start_angle: 0.0,
        left_handed: false,
        defined_by: HelixInput::PitchAndTurns,
        height: 0.0,
        variable_pitch: vec![],
    }
}

#[test]
fn helix_height_combinations_and_variable_pitch() {
    // Height and turns: 2.25 turns over 10 mm ends a quarter turn past whole.
    let mut spec = base_helix();
    spec.defined_by = HelixInput::HeightAndTurns;
    spec.height = 10.0;
    spec.turns = 2.25;
    let (end, _) = helix_end(spec);
    assert!((end - DVec3::new(0.0, 10.0, 10.0)).length() < 1e-6, "{end}");

    // Height and pitch: 12 mm at 4 mm per turn is three whole turns.
    let mut spec = base_helix();
    spec.defined_by = HelixInput::HeightAndPitch;
    spec.height = 12.0;
    spec.pitch = 4.0;
    let (end, length) = helix_end(spec);
    assert!((end - DVec3::new(10.0, 0.0, 12.0)).length() < 1e-6, "{end}");
    assert!((length - 3.0 * (2.0 * PI * 10.0).hypot(4.0)).abs() < 1e-4);

    // Pitch growing linearly from 2 to 6 over 2 turns rises (2 + 6) / 2 * 2 = 8 mm,
    // then a constant 6 mm pitch for 1 more turn reaches 14 mm.
    let mut spec = base_helix();
    spec.variable_pitch = vec![
        HelixPitch {
            turn: 0.0,
            pitch: 2.0,
        },
        HelixPitch {
            turn: 2.0,
            pitch: 6.0,
        },
        HelixPitch {
            turn: 3.0,
            pitch: 6.0,
        },
    ];
    let (end, _) = helix_end(spec.clone());
    assert!((end - DVec3::new(10.0, 0.0, 14.0)).length() < 1e-6, "{end}");

    // Rows out of order are rejected.
    spec.variable_pitch.swap(1, 2);
    let mut doc = Document::new();
    let helix = doc.add(FeatureKind::Helix(spec));
    assert!(Model::new(doc).status(helix).unwrap().error.is_some());
}
