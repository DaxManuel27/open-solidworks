use of_model::{
    geom::{DVec2, Kind},
    sketch::Sketch,
    *,
};

fn square(doc: &mut Document, plane: Ref) -> (FeatureId, Vec<u32>) {
    let mut sketch = Sketch::new();
    let points = [
        DVec2::ZERO,
        DVec2::new(10., 0.),
        DVec2::splat(10.),
        DVec2::new(0., 10.),
    ];
    for i in 0..4 {
        sketch.add_line_at(points[i], points[(i + 1) % 4]);
    }
    let entities = sketch.contours().unwrap().loops[0].entities.clone();
    sketch.add_circle_at(DVec2::splat(30.), 2.);
    (
        doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch })),
        entities,
    )
}

fn selection(sketch: FeatureId, entities: &[u32], start: usize) -> ProfileSelection {
    ProfileSelection::Contour(ContourSelection {
        sketch,
        entities: entities.to_vec(),
        start: Some(entities[start]),
        reverse: false,
        seam: None,
    })
}

fn volume(model: &Model) -> f64 {
    assert!(
        model.statuses().iter().all(|s| s.error.is_none()),
        "{:?}",
        model.statuses()
    );
    let shape = &model.state().body.as_ref().unwrap().shape;
    assert!(shape.is_valid());
    shape.mass_props().unwrap().volume
}

#[test]
fn explicit_profile_seams_control_loft_correspondence_and_survive_reload() {
    let mut doc = Document::new();
    let (bottom, a) = square(&mut doc, Ref::FrontPlane);
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.,
        flip: false,
    }));
    let (top, b) = square(&mut doc, Ref::Feature { id: plane });
    let loft = doc.add(FeatureKind::Loft(Loft {
        profiles: vec![selection(bottom, &a, 0), selection(top, &b, 0)],
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
    let mut model = Model::new(doc);
    assert!((volume(&model) - 1000.).abs() < 1e-5);
    if let FeatureKind::Loft(p) = &mut model.doc.feature_mut(loft).unwrap().kind {
        p.profiles[1] = selection(top, &b, 1);
    }
    model.invalidate(loft);
    model.rebuild();
    let twisted = volume(&model);
    assert!((twisted - 2000. / 3.).abs() < 1e-4, "{twisted}");
    let params = model.doc.feature(loft).unwrap().kind.clone();
    let preview = model.preview(&params, Some(loft)).unwrap();
    assert!((preview.body.unwrap().shape.mass_props().unwrap().volume - twisted).abs() < 1e-6);
    let reopened = Model::new(file::from_str(&file::to_string(&model.doc)).unwrap());
    assert!((volume(&reopened) - twisted).abs() < 1e-6);
    model.rebuild_all();
    assert!((volume(&model) - twisted).abs() < 1e-6);
    assert!(model.doc.delete(&[top]).contains(&loft));
}

#[test]
fn invalid_profile_membership_start_and_closure_do_not_rebind() {
    let mut doc = Document::new();
    let (bottom, a) = square(&mut doc, Ref::FrontPlane);
    let (top, b) = square(&mut doc, Ref::TopPlane);
    let loft = doc.add(FeatureKind::Loft(Loft {
        profiles: vec![selection(bottom, &a, 0), selection(top, &b, 0)],
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
    if let FeatureKind::Loft(p) = &mut doc.feature_mut(loft).unwrap().kind {
        let ProfileSelection::Contour(c) = &mut p.profiles[0] else {
            unreachable!()
        };
        c.start = Some(u32::MAX);
    }
    let model = Model::new(doc.clone());
    assert!(
        model
            .status(loft)
            .unwrap()
            .error
            .unwrap()
            .contains("start/seam entity")
    );
    if let FeatureKind::Loft(p) = &mut doc.feature_mut(loft).unwrap().kind {
        let ProfileSelection::Contour(c) = &mut p.profiles[0] else {
            unreachable!()
        };
        c.start = None;
        c.entities.pop();
    }
    assert!(
        Model::new(doc)
            .status(loft)
            .unwrap()
            .error
            .unwrap()
            .contains("selected contour changed")
    );

    let mut doc = Document::new();
    let mut sketch = Sketch::new();
    let (entity, _, _) = sketch.add_line_at(DVec2::ZERO, DVec2::X);
    let source = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    let loft = doc.add(FeatureKind::Loft(Loft {
        profiles: vec![selection(source, &[entity], 0), 999.into()],
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
    assert!(
        Model::new(doc)
            .status(loft)
            .unwrap()
            .error
            .unwrap()
            .contains("complete closed profile")
    );
}

#[test]
fn generated_segment_identity_tracks_source_entity_not_edge_position() {
    let mut doc = Document::new();
    let mut sketch = Sketch::new();
    let (first, _, _) = sketch.add_line_at(DVec2::ZERO, DVec2::X * 10.);
    let (second, _, _) = sketch.add_line_at(DVec2::X * 10., DVec2::X * 20.);
    let source = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    let curve = doc.add(FeatureKind::CompositeCurve(CompositeCurve {
        segments: vec![CurveSelection {
            reference: Ref::Feature { id: source },
            reverse: false,
        }],
    }));
    let mut model = Model::new(doc);
    let name = format!("sketch:{source}/entity:{second}");
    assert!(model.state().curves[&curve].segments.contains_key(&name));
    let consumer = model.doc.add(FeatureKind::CompositeCurve(CompositeCurve {
        segments: vec![CurveSelection {
            reference: Ref::CurveSegment {
                curve,
                segment: name.clone(),
            },
            reverse: false,
        }],
    }));
    model.rebuild();
    assert!(model.status(consumer).unwrap().error.is_none());
    if let FeatureKind::Sketch(s) = &mut model.doc.feature_mut(source).unwrap().kind {
        s.sketch.delete(&[first]);
    }
    model.invalidate(source);
    model.rebuild();
    assert!(model.status(consumer).unwrap().error.is_none());
    assert_eq!(model.state().curves[&consumer].shape.count(Kind::Edge), 1);
    assert!(
        (model.state().curves[&consumer]
            .shape
            .edge_info(0)
            .unwrap()
            .length
            - 10.)
            .abs()
            < 1e-7
    );
    let reopened = Model::new(file::from_str(&file::to_string(&model.doc)).unwrap());
    assert!(reopened.status(consumer).unwrap().error.is_none());
    assert!(reopened.state().curves[&curve].segments.contains_key(&name));
    if let FeatureKind::Sketch(s) = &mut model.doc.feature_mut(source).unwrap().kind {
        s.sketch.delete(&[second]);
        s.sketch.add_line_at(DVec2::X * 10., DVec2::X * 20.);
    }
    model.rebuild_all();
    assert!(
        model
            .status(consumer)
            .unwrap()
            .error
            .unwrap()
            .contains("missing or ambiguous")
    );
    assert!(model.doc.delete(&[curve]).contains(&consumer));
}

#[test]
fn ordered_open_paths_reject_interior_starts_and_honor_reverse() {
    let mut doc = Document::new();
    let mut sketch = Sketch::new();
    let (first, _, _) = sketch.add_line_at(DVec2::ZERO, DVec2::X * 10.);
    let (second, _, _) = sketch.add_line_at(DVec2::X * 10., DVec2::X * 20.);
    let source = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    let curve = doc.add(FeatureKind::CompositeCurve(CompositeCurve {
        segments: vec![CurveSelection {
            reference: Ref::OrderedSketchContour(ContourSelection {
                sketch: source,
                entities: vec![first, second],
                start: Some(second),
                reverse: false,
                seam: None,
            }),
            reverse: false,
        }],
    }));
    let mut model = Model::new(doc);
    assert!(
        model
            .status(curve)
            .unwrap()
            .error
            .unwrap()
            .contains("endpoint")
    );
    if let FeatureKind::CompositeCurve(p) = &mut model.doc.feature_mut(curve).unwrap().kind {
        let Ref::OrderedSketchContour(c) = &mut p.segments[0].reference else {
            unreachable!()
        };
        c.reverse = true;
    }
    model.rebuild_all();
    assert!(model.status(curve).unwrap().error.is_none());
}

#[test]
fn partial_projection_preserves_whole_curve_and_names_only_reached_sources() {
    let mut model = Model::new(file::from_str(include_str!("fixtures/v1-block.ofp")).unwrap());
    let body = &model.state().body.as_ref().unwrap().shape;
    let top = (0..body.count(Kind::Face))
        .find(|i| body.face_info(*i).unwrap().normal.z > 0.99)
        .unwrap();
    let target = model.state().qualified_ref_for(Kind::Face, top).unwrap();
    let mut sketch = Sketch::new();
    let (missed, _, _) = sketch.add_line_at(DVec2::new(-10., 10.), DVec2::new(-5., 10.));
    let (reached, _, _) = sketch.add_line_at(DVec2::new(-5., 10.), DVec2::new(5., 10.));
    let source = model.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    let projected = model.doc.add(FeatureKind::ProjectedCurve(ProjectedCurve {
        source: Ref::Feature { id: source },
        target,
        direction: geom::DVec3::Z,
        mode: ProjectionMode::OntoFace,
    }));
    model.rebuild();
    assert!(
        model.status(projected).unwrap().error.is_none(),
        "{:?}",
        model.status(projected)
    );
    let generated = &model.state().curves[&projected];
    assert_eq!(generated.shape.count(Kind::Edge), 1);
    assert_eq!(generated.segments.len(), 1);
    let segment = generated.segments.keys().next().unwrap();
    assert!(segment.contains(&format!("entity:{reached}@")));
    assert!(!segment.contains(&format!("entity:{missed}@")));
    let consumer = model.doc.add(FeatureKind::CompositeCurve(CompositeCurve {
        segments: vec![CurveSelection {
            reference: Ref::CurveSegment {
                curve: projected,
                segment: segment.clone(),
            },
            reverse: false,
        }],
    }));
    model.rebuild();
    assert!(model.status(consumer).unwrap().error.is_none());
}

fn circle_contour(doc: &mut Document, plane: Ref, seam: Option<f64>) -> ProfileSelection {
    let mut sketch = Sketch::new();
    let entity = sketch.add_circle_at(DVec2::ZERO, 5.0);
    let id = doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch }));
    ProfileSelection::Contour(ContourSelection {
        sketch: id,
        entities: vec![entity],
        start: None,
        reverse: false,
        seam: seam.map(OrderedFloat),
    })
}

#[test]
fn interior_seams_turn_circles_and_split_edges() {
    // Ruled between equal circles whose seams are a quarter turn apart: each
    // section at height t is a circle of radius 5 |(1 - t) + t i|.
    let mut doc = Document::new();
    let bottom = circle_contour(&mut doc, Ref::FrontPlane, None);
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.,
        flip: false,
    }));
    let top = circle_contour(&mut doc, Ref::Feature { id: plane }, Some(0.25));
    doc.add(FeatureKind::Loft(Loft {
        profiles: vec![bottom, top],
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
    let mut model = Model::new(doc);
    let expected = 250.0 * std::f64::consts::PI * 2.0 / 3.0;
    assert!(
        (volume(&model) - expected).abs() < 1e-5,
        "{}",
        volume(&model)
    );
    // The seam survives save and reopen.
    let reopened = Model::new(file::from_str(&file::to_string(&model.doc)).unwrap());
    assert!((volume(&reopened) - expected).abs() < 1e-6);

    // Seams half way along the first edge of both squares: the same untwisted
    // prism, with the split edge's two side faces named apart.
    let mut doc = Document::new();
    let (bottom, a) = square(&mut doc, Ref::FrontPlane);
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.,
        flip: false,
    }));
    let (top, b) = square(&mut doc, Ref::Feature { id: plane });
    let seamed = |sketch, entities: &[u32]| {
        let ProfileSelection::Contour(mut c) = selection(sketch, entities, 0) else {
            unreachable!()
        };
        c.seam = Some(OrderedFloat(0.5));
        ProfileSelection::Contour(c)
    };
    let id = doc.add(FeatureKind::Loft(Loft {
        profiles: vec![seamed(bottom, &a), seamed(top, &b)],
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
    model = Model::new(doc);
    assert!((volume(&model) - 1000.).abs() < 1e-5);
    let names = &model.state().body.as_ref().unwrap().names;
    let first = format!("{id}.s{bottom}.e{}", a[0]);
    assert!(names.face(&first).is_some(), "{first}");
    assert!(names.face(&format!("{first}.before")).is_some());
    // One seamed square against a plain one has mismatched edge counts.
    if let FeatureKind::Loft(l) = &mut model.doc.feature_mut(id).unwrap().kind {
        l.profiles[1] = selection(top, &b, 0);
    }
    model.invalidate(id);
    model.rebuild();
    assert!(
        model
            .status(id)
            .unwrap()
            .error
            .unwrap()
            .contains("same number of edges")
    );
    // Seams must lie inside the entity.
    if let FeatureKind::Loft(l) = &mut model.doc.feature_mut(id).unwrap().kind {
        l.profiles[1] = seamed(top, &b);
        if let ProfileSelection::Contour(c) = &mut l.profiles[1] {
            c.seam = Some(OrderedFloat(1.0));
        }
    }
    model.invalidate(id);
    model.rebuild();
    assert!(
        model
            .status(id)
            .unwrap()
            .error
            .unwrap()
            .contains("between 0 and 1")
    );
}

#[test]
fn split_projection_names_pieces_in_order_along_the_source() {
    let mut model = Model::new(file::from_str(include_str!("fixtures/v1-block.ofp")).unwrap());
    let b = model.state().body.as_ref().unwrap().shape.bbox().unwrap();
    let (cx, cy) = ((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0);
    // A hole through the middle of the top face.
    let mut hole = Sketch::new();
    let hole_entity = hole.add_circle_at(DVec2::new(cx, cy), 1.0);
    let hole_sketch = model.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: hole,
    }));
    model.doc.add(FeatureKind::Extrude(Extrude {
        sketch: hole_sketch,
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
    let body = &model.state().body.as_ref().unwrap().shape;
    let top = (0..body.count(Kind::Face))
        .find(|i| body.face_info(*i).unwrap().normal.z > 0.99)
        .unwrap();
    let target = model.state().qualified_ref_for(Kind::Face, top).unwrap();
    // A line across the top, over the hole: it lands as two pieces.
    let mut sketch = Sketch::new();
    let (line, _, _) =
        sketch.add_line_at(DVec2::new(b.min.x - 1.0, cy), DVec2::new(b.max.x + 1.0, cy));
    let source = model.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    let projected = model.doc.add(FeatureKind::ProjectedCurve(ProjectedCurve {
        source: Ref::Feature { id: source },
        target,
        direction: geom::DVec3::Z,
        mode: ProjectionMode::OntoFace,
    }));
    model.rebuild();
    assert!(
        model.status(projected).unwrap().error.is_none(),
        "{:?}",
        model.status(projected)
    );
    let generated = &model.state().curves[&projected];
    assert_eq!(generated.shape.count(Kind::Edge), 2);
    let names: Vec<_> = generated.segments.keys().cloned().collect();
    assert_eq!(names.len(), 2, "{names:?}");
    let first = names.iter().find(|n| n.ends_with("#1")).unwrap().clone();
    let second = names.iter().find(|n| n.ends_with("#2")).unwrap().clone();
    assert!(first.contains(&format!("entity:{line}@")));
    // #1 starts at the line's start side of the face, #2 ends at the far side.
    let mid = |name: &str| generated.segments[name].edge_info(0).unwrap().mid.x;
    assert!(mid(&first) < cx && mid(&second) > cx);
    let consumer = model.doc.add(FeatureKind::CompositeCurve(CompositeCurve {
        segments: vec![CurveSelection {
            reference: Ref::CurveSegment {
                curve: projected,
                segment: second.clone(),
            },
            reverse: false,
        }],
    }));
    model.rebuild();
    assert!(model.status(consumer).unwrap().error.is_none());
    let far = |m: &Model| {
        m.state().curves[&consumer]
            .shape
            .edge_info(0)
            .unwrap()
            .mid
            .x
    };
    let before = far(&model);
    // Moving the hole keeps #2 the piece past it.
    let mut hole = Sketch::new();
    hole.add_circle_at(DVec2::new(cx - 2.0, cy), 1.0);
    if let FeatureKind::Sketch(s) = &mut model.doc.feature_mut(hole_sketch).unwrap().kind {
        s.sketch = hole;
    }
    let _ = hole_entity;
    model.invalidate(hole_sketch);
    model.rebuild();
    assert!(
        model.status(consumer).unwrap().error.is_none(),
        "{:?}",
        model.status(consumer)
    );
    assert!(far(&model) < before && far(&model) > cx - 2.0);
    // Reversing the source line reverses the order.
    let mut sketch = Sketch::new();
    sketch.add_line_at(DVec2::new(b.max.x + 1.0, cy), DVec2::new(b.min.x - 1.0, cy));
    if let FeatureKind::Sketch(s) = &mut model.doc.feature_mut(source).unwrap().kind {
        s.sketch = sketch;
    }
    model.invalidate(source);
    model.rebuild();
    assert!(far(&model) < cx - 2.0, "{}", far(&model));
}
