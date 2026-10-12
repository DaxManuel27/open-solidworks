use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::Sketch,
    *,
};

fn model() -> Model {
    Model::new(file::from_str(include_str!("fixtures/v1-block.ofp")).unwrap())
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
fn neutral_plane_draft_has_expected_taper_and_preserves_identity() {
    let mut m = model();
    let body = m.state().body.as_ref().unwrap();
    let side = (0..body.shape.count(Kind::Face))
        .find(|i| body.shape.face_info(*i).unwrap().normal.x > 0.99)
        .unwrap();
    let face = m.state().qualified_ref_for(Kind::Face, side).unwrap();
    let body_id = m.state().bodies[0].id.clone();
    let angle = 3_f64.to_radians();
    let id = m.doc.add(FeatureKind::Draft(Draft {
        faces: vec![face],
        neutral_plane: Ref::FrontPlane,
        direction: None,
        angle,
        parting_line: vec![],
        other_side: false,
        step: false,
    }));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_none(), "{:?}", m.status(id));
    assert!((volume(&m) - (1000.0 - 250.0 * angle.tan())).abs() < 1e-5);
    assert_eq!(m.state().bodies[0].id, body_id);
    assert!(m.state().body.as_ref().unwrap().shape.is_valid());
}

#[test]
fn variable_fillet_rejects_bad_stations_and_retains_previous_solid() {
    let mut m = model();
    let body = m.state().body.as_ref().unwrap();
    let edge = (0..body.shape.count(Kind::Edge))
        .find(|i| {
            let e = body.shape.edge_info(*i).unwrap();
            (e.end - e.start).normalize().dot(DVec3::Z).abs() > 0.99
        })
        .unwrap();
    let edge = m.state().qualified_ref_for(Kind::Edge, edge).unwrap();
    let id = m.doc.add(FeatureKind::VariableFillet(VariableFillet {
        edge,
        stations: vec![
            RadiusStation {
                position: 0.0,
                radius: 0.5,
            },
            RadiusStation {
                position: 1.0,
                radius: 1.0,
            },
        ],
        reverse: false,
        straight: false,
    }));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_none(), "{:?}", m.status(id));
    assert!(volume(&m) < 1000.0 && volume(&m) > 900.0);
    if let FeatureKind::VariableFillet(p) = &mut m.doc.feature_mut(id).unwrap().kind {
        p.stations[1].position = 0.0;
    }
    m.invalidate(id);
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_some());
    assert!((volume(&m) - 1000.0).abs() < 1e-6);
}

#[test]
fn projected_split_line_preserves_volume_and_rejects_ambiguous_old_face() {
    let mut m = model();
    let body = m.state().body.as_ref().unwrap();
    let face = (0..body.shape.count(Kind::Face))
        .find(|i| body.shape.face_info(*i).unwrap().normal.z > 0.99)
        .unwrap();
    let target = m.state().qualified_ref_for(Kind::Face, face).unwrap();
    let original_faces = body.shape.count(Kind::Face);
    let mut sketch = Sketch::new();
    sketch.add_circle_at(DVec2::new(5.0, 10.0), 2.0);
    let source = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    let p = ProjectedCurve {
        source: Ref::Feature { id: source },
        target: target.clone(),
        direction: DVec3::Z,
        mode: ProjectionMode::OntoFace,
    };
    let projected = m.doc.add(FeatureKind::ProjectedCurve(p.clone()));
    let split = m.doc.add(FeatureKind::SplitLine(p));
    m.rebuild();
    assert!(
        m.status(projected).unwrap().error.is_none(),
        "{:?}",
        m.status(projected)
    );
    assert!(
        m.status(split).unwrap().error.is_none(),
        "{:?}",
        m.status(split)
    );
    assert!((volume(&m) - 1000.0).abs() < 1e-6);
    assert_eq!(
        m.state().body.as_ref().unwrap().shape.count(Kind::Face),
        original_faces + 1
    );
    assert!(m.resolve(&target).is_err());
}

/// The +X side face and its edge at height `z`.
fn side_edge(m: &Model, z: f64) -> Ref {
    let body = m.state().body.as_ref().unwrap();
    let x = body.shape.bbox().unwrap().max.x;
    let edge = (0..body.shape.count(Kind::Edge))
        .find(|i| {
            let e = body.shape.edge_info(*i).unwrap();
            (e.mid.x - x).abs() < 1e-6 && (e.mid.z - z).abs() < 1e-6
        })
        .unwrap();
    m.state().ref_for(Kind::Edge, edge).unwrap()
}

#[test]
fn parting_line_draft_hinges_each_face_on_its_edge() {
    let angle = 3_f64.to_radians();
    let height = model()
        .state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .bbox()
        .unwrap()
        .max
        .z
        .round();
    // Hinged on the bottom edge it matches a neutral-plane draft at the bottom.
    let mut m = model();
    let bottom = side_edge(&m, 0.0);
    let id = m.doc.add(FeatureKind::Draft(Draft {
        faces: vec![],
        neutral_plane: Ref::FrontPlane,
        direction: None,
        angle,
        parting_line: vec![bottom],
        other_side: false,
        step: false,
    }));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_none(), "{:?}", m.status(id));
    assert!((volume(&m) - (1000.0 - 250.0 * angle.tan())).abs() < 1e-5);

    // Hinged on the top edge, drafting the face below it, it matches a neutral
    // plane through the top.
    let mut m = model();
    let top = side_edge(&m, height);
    let id = m.doc.add(FeatureKind::Draft(Draft {
        faces: vec![],
        neutral_plane: Ref::FrontPlane,
        direction: None,
        angle,
        parting_line: vec![top],
        other_side: true,
        step: false,
    }));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_none(), "{:?}", m.status(id));
    let parting = volume(&m);
    let mut n = model();
    let body = n.state().body.as_ref().unwrap();
    let side = (0..body.shape.count(Kind::Face))
        .find(|i| body.shape.face_info(*i).unwrap().normal.x > 0.99)
        .unwrap();
    let face = n.state().ref_for(Kind::Face, side).unwrap();
    let plane = n.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: height,
        flip: false,
    }));
    n.doc.add(FeatureKind::Draft(Draft {
        faces: vec![face],
        neutral_plane: Ref::Feature { id: plane },
        direction: Some(Ref::FrontPlane),
        angle,
        parting_line: vec![],
        other_side: false,
        step: false,
    }));
    n.rebuild();
    assert!(
        (parting - volume(&n)).abs() < 1e-6,
        "{parting} vs {}",
        volume(&n)
    );
    assert!((parting - 1000.0).abs() > 1.0);

    // No face lies on the pull side of the top edge.
    if let FeatureKind::Draft(d) = &mut m.doc.feature_mut(id).unwrap().kind {
        d.other_side = false;
    }
    m.invalidate(id);
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_some());
}

#[test]
fn straight_variable_fillet_matches_a_linear_radius_law() {
    // A linear radius r(s) on a straight right-angle edge removes
    // (1 - pi/4) r^2 per unit length: (1 - pi/4) L (r0^2 + r0 r1 + r1^2) / 3.
    let mut m = model();
    let body = m.state().body.as_ref().unwrap();
    let (edge, length) = (0..body.shape.count(Kind::Edge))
        .map(|i| (i, body.shape.edge_info(i).unwrap()))
        .find(|(_, e)| (e.end - e.start).normalize().dot(DVec3::Z).abs() > 0.99)
        .map(|(i, e)| (i, e.length))
        .unwrap();
    let edge = m.state().qualified_ref_for(Kind::Edge, edge).unwrap();
    let (r0, r1) = (0.5, 1.5);
    let id = m.doc.add(FeatureKind::VariableFillet(VariableFillet {
        edge,
        stations: vec![
            RadiusStation {
                position: 0.0,
                radius: r0,
            },
            RadiusStation {
                position: 1.0,
                radius: r1,
            },
        ],
        reverse: false,
        straight: true,
    }));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_none(), "{:?}", m.status(id));
    let quarter = 1.0 - std::f64::consts::FRAC_PI_4;
    let expected = 1000.0 - quarter * length * (r0 * r0 + r0 * r1 + r1 * r1) / 3.0;
    assert!(
        (volume(&m) - expected).abs() < 1e-3,
        "{} vs {expected}",
        volume(&m)
    );
    // Reversing measures from the other end: same volume, mirrored radii.
    if let FeatureKind::VariableFillet(p) = &mut m.doc.feature_mut(id).unwrap().kind {
        p.reverse = true;
    }
    m.invalidate(id);
    m.rebuild();
    assert!((volume(&m) - expected).abs() < 1e-3);
}

#[test]
fn variable_fillet_follows_a_tangent_chain() {
    // A slot (two lines joined by half circles) extruded: its top boundary is one
    // closed tangent chain, so one selected edge fillets the whole rim.
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    let a = sk.add_point(DVec2::new(0.0, -5.0));
    let b = sk.add_point(DVec2::new(20.0, -5.0));
    let c = sk.add_point(DVec2::new(20.0, 5.0));
    let d = sk.add_point(DVec2::new(0.0, 5.0));
    let right = sk.add_point(DVec2::new(20.0, 0.0));
    let left = sk.add_point(DVec2::new(0.0, 0.0));
    sk.add_line(a, b).unwrap();
    sk.add_arc(right, b, c).unwrap();
    sk.add_line(c, d).unwrap();
    sk.add_arc(left, d, a).unwrap();
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: 10.0 },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let mut m = Model::new(doc);
    let full = volume(&m);
    let body = m.state().body.as_ref().unwrap();
    let edge = (0..body.shape.count(Kind::Edge))
        .find(|i| {
            let e = body.shape.edge_info(*i).unwrap();
            (e.mid.z - 10.0).abs() < 1e-9 && e.kind == of_model::geom::CurveKind::Line
        })
        .unwrap();
    let edge = m.state().qualified_ref_for(Kind::Edge, edge).unwrap();
    let id = m.doc.add(FeatureKind::VariableFillet(VariableFillet {
        edge,
        stations: vec![
            RadiusStation {
                position: 0.0,
                radius: 1.0,
            },
            RadiusStation {
                position: 1.0,
                radius: 1.0,
            },
        ],
        reverse: false,
        straight: true,
    }));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_none(), "{:?}", m.status(id));
    // The 1 mm round removes (1 - pi/4) mm^2 along both 20 mm lines and, by
    // Pappus, about the same per unit length around the two half circles.
    let quarter = 1.0 - std::f64::consts::FRAC_PI_4;
    let removed = full - volume(&m);
    assert!(
        removed > 40.0 * quarter + 1.0,
        "the round continues around the arcs: {removed}"
    );
    assert!(m.state().body.as_ref().unwrap().shape.is_valid());
}

/// Split the +X side face along a line climbing from 0.2 to 0.6 of the height.
fn inclined_parting_line(m: &mut Model) -> (Ref, f64, f64, f64) {
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    let (y0, y1, h) = (b.min.y, b.max.y, b.max.z);
    let x = b.max.x;
    let body = m.state().body.as_ref().unwrap();
    let face = (0..body.shape.count(Kind::Face))
        .find(|i| body.shape.face_info(*i).unwrap().normal.x > 0.99)
        .unwrap();
    let target = m.state().qualified_ref_for(Kind::Face, face).unwrap();
    let z = |y: f64| 0.2 * h + 0.4 * h * (y - y0) / (y1 - y0);
    // On the right plane local x runs along world -z and local y along world y.
    let mut sketch = Sketch::new();
    let (a, c) = (y0 - 1.0, y1 + 1.0);
    sketch.add_line_at(DVec2::new(-z(a), a), DVec2::new(-z(c), c));
    let source = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::RightPlane,
        sketch,
    }));
    let split = m.doc.add(FeatureKind::SplitLine(ProjectedCurve {
        source: Ref::Feature { id: source },
        target,
        direction: DVec3::X,
        mode: ProjectionMode::OntoFace,
    }));
    m.rebuild();
    assert!(
        m.status(split).unwrap().error.is_none(),
        "{:?}",
        m.status(split)
    );
    let body = m.state().body.as_ref().unwrap();
    let edge = (0..body.shape.count(Kind::Edge))
        .find(|i| {
            let e = body.shape.edge_info(*i).unwrap();
            (e.mid.x - x).abs() < 1e-6
                && (e.end.z - e.start.z).abs() > 0.1 * h
                && (e.mid.z - z(e.mid.y)).abs() < 1e-6
        })
        .unwrap();
    (
        m.state().ref_for(Kind::Edge, edge).unwrap(),
        y1 - y0,
        h,
        0.4 * h / (y1 - y0),
    )
}

#[test]
fn step_draft_tilts_about_a_level_hinge_with_perpendicular_steps() {
    let angle = 4_f64.to_radians();
    let draft = |edge: Ref, other_side: bool, angle: f64| {
        FeatureKind::Draft(Draft {
            faces: vec![],
            neutral_plane: Ref::FrontPlane,
            direction: None,
            angle,
            parting_line: vec![edge],
            other_side,
            step: true,
        })
    };
    // A level parting line: the step draft matches the parting-line draft.
    let mut m = model();
    let bottom = side_edge(&m, 0.0);
    let id = m.doc.add(draft(bottom, false, angle));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_none(), "{:?}", m.status(id));
    assert!(
        (volume(&m) - (1000.0 - 250.0 * angle.tan())).abs() < 1e-5,
        "{}",
        volume(&m)
    );

    // An inclined parting line: the face above tilts about the line's lowest
    // point, removing a wedge that grows with height above the hinge.
    let mut m = model();
    let (edge, w, h, slope) = inclined_parting_line(&mut m);
    let a = 0.2 * h;
    let removed = angle.tan() * (w * (h - a).powi(2) / 2.0 - slope.powi(2) * w.powi(3) / 6.0);
    let id = m.doc.add(draft(edge.clone(), false, angle));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_none(), "{:?}", m.status(id));
    let body = m.state().body.as_ref().unwrap();
    assert!(body.shape.is_valid());
    assert_eq!(body.shape.count(Kind::Solid), 1);
    assert!(
        (volume(&m) - (1000.0 - removed)).abs() < 1e-5,
        "{} vs {}",
        volume(&m),
        1000.0 - removed
    );
    // A step face perpendicular to the original side face stands on the parting line.
    let steps = (0..body.shape.count(Kind::Face))
        .filter(|i| {
            let f = body.shape.face_info(*i).unwrap();
            f.normal.x.abs() < 1e-9 && f.normal.z > 0.5 && f.normal.y < -0.05
        })
        .count();
    assert_eq!(steps, 1);

    // The face below hinges on the line's highest point; the opposite angle
    // adds material there instead.
    let mut n = model();
    let (edge, w, h, slope) = inclined_parting_line(&mut n);
    let top = 0.6 * h;
    let below = |y: f64| 0.2 * h + slope * y;
    // Wedge thickness (top - z) tan(angle) for z below the parting line.
    let steps = 2000;
    let added: f64 = (0..steps)
        .map(|i| {
            let y = (i as f64 + 0.5) * w / steps as f64;
            let (z0, z1) = (0.0, below(y));
            ((top - z0).powi(2) - (top - z1).powi(2)) / 2.0 * w / steps as f64
        })
        .sum::<f64>()
        * angle.tan();
    let id = n.doc.add(draft(edge, true, angle));
    n.rebuild();
    assert!(n.status(id).unwrap().error.is_none(), "{:?}", n.status(id));
    assert!(
        (volume(&n) - (1000.0 + added)).abs() < 1e-3,
        "{} vs {}",
        volume(&n),
        1000.0 + added
    );

    // Step drafts need parting-line edges.
    if let FeatureKind::Draft(d) = &mut n.doc.feature_mut(id).unwrap().kind {
        d.parting_line.clear();
    }
    n.invalidate(id);
    n.rebuild();
    assert!(n.status(id).unwrap().error.is_some());
}
