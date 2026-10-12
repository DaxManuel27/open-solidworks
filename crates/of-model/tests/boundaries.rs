use of_model::{
    geom::{DVec2, DVec3},
    sketch::{Sketch, tools},
    *,
};

fn valid(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
    assert!(m.state().body.as_ref().unwrap().shape.is_valid());
}

fn volume(m: &Model) -> f64 {
    m.state()
        .body
        .as_ref()
        .map(|b| b.shape.mass_props().unwrap().volume)
        .unwrap_or(0.0)
}

fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6 * b.abs().max(1.0), "{a} != {b}");
}

fn square_sketch(m: &mut Model, plane: Ref, half: f64) -> FeatureId {
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::splat(-half), DVec2::splat(half));
    m.doc
        .add(FeatureKind::Sketch(SketchFeature { plane, sketch: sk }))
}

fn set_square(m: &mut Model, id: FeatureId, half: f64) {
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::splat(-half), DVec2::splat(half));
    if let FeatureKind::Sketch(s) = &mut m.doc.feature_mut(id).unwrap().kind {
        s.sketch = sk;
    }
    m.invalidate(id);
}

/// Two lines on the right plane through the squares' side midpoints at y = ±half.
fn guides(m: &mut Model, id: Option<FeatureId>, bottom: f64, top: f64) -> (FeatureId, [Ref; 2]) {
    let id = id.unwrap_or_else(|| {
        m.doc.add(FeatureKind::Sketch(SketchFeature {
            plane: Ref::RightPlane,
            sketch: Sketch::new(),
        }))
    });
    m.rebuild();
    let frame = m.resolve(&Ref::RightPlane).unwrap().plane().unwrap();
    let mut sk = Sketch::new();
    let mut lines = Vec::new();
    for sign in [-1.0, 1.0] {
        let a = frame.to_local(DVec3::new(0.0, sign * bottom, 0.0));
        let b = frame.to_local(DVec3::new(0.0, sign * top, 10.0));
        lines.push(sk.add_line_at(a, b).0);
    }
    if let FeatureKind::Sketch(s) = &mut m.doc.feature_mut(id).unwrap().kind {
        s.sketch = sk;
    }
    m.invalidate(id);
    let r = |entity| Ref::SketchEntity { sketch: id, entity };
    (id, [r(lines[0]), r(lines[1])])
}

#[test]
fn boundary_boss_follows_both_curve_families() {
    let mut m = Model::new(Document::new());
    let bottom = square_sketch(&mut m, Ref::FrontPlane, 5.0);
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.0,
        flip: false,
    }));
    let top = square_sketch(&mut m, Ref::Feature { id: plane }, 5.0);
    let (guide, curves) = guides(&mut m, None, 5.0, 5.0);
    let b = m.doc.add(FeatureKind::Boundary(Boundary {
        sections: vec![bottom.into(), top.into()],
        curves: curves.to_vec(),
        cut: false,
        merge: true,
    }));
    m.rebuild();
    valid(&m);
    assert_close(volume(&m), 1000.0);
    let names = &m.state().body.as_ref().unwrap().names;
    for name in [
        format!("{b}.start"),
        format!("{b}.end"),
        format!("{b}.b0.1"),
    ] {
        assert!(names.face(&name).is_some(), "{name}");
    }

    // Enlarging the top section and moving the crossing curves to meet it
    // turns the prism into a frustum of a pyramid.
    set_square(&mut m, top, 10.0);
    guides(&mut m, Some(guide), 5.0, 10.0);
    m.rebuild();
    valid(&m);
    assert_close(volume(&m), 10.0 / 3.0 * (100.0 + 400.0 + 200.0));

    let reopened = Model::new(file::from_str(&file::to_string(&m.doc)).unwrap());
    valid(&reopened);
    assert_close(volume(&reopened), volume(&m));

    // A top section the curves no longer reach fails the feature.
    set_square(&mut m, top, 7.0);
    m.rebuild();
    let error = m.status(b).unwrap().error.unwrap();
    assert!(error.contains("does not touch"), "{error}");
}

#[test]
fn boundary_cut_removes_the_enclosed_region() {
    let mut m = Model::new(Document::new());
    // A 30 x 30 x 10 block, then a 10 x 10 boundary cut through it.
    let block = square_sketch(&mut m, Ref::FrontPlane, 15.0);
    m.doc.add(FeatureKind::Extrude(Extrude {
        sketch: block,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: 10.0 },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let bottom = square_sketch(&mut m, Ref::FrontPlane, 5.0);
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.0,
        flip: false,
    }));
    let top = square_sketch(&mut m, Ref::Feature { id: plane }, 5.0);
    let (_, curves) = guides(&mut m, None, 5.0, 5.0);
    m.doc.add(FeatureKind::Boundary(Boundary {
        sections: vec![bottom.into(), top.into()],
        curves: curves.to_vec(),
        cut: true,
        merge: true,
    }));
    m.rebuild();
    valid(&m);
    assert_close(volume(&m), 9000.0 - 1000.0);
}

#[test]
fn loft_guides_shape_the_loft() {
    let mut m = Model::new(Document::new());
    let bottom = square_sketch(&mut m, Ref::FrontPlane, 5.0);
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.0,
        flip: false,
    }));
    let top = square_sketch(&mut m, Ref::Feature { id: plane }, 10.0);
    let (_, curves) = guides(&mut m, None, 5.0, 10.0);
    let loft = m.doc.add(FeatureKind::Loft(Loft {
        profiles: vec![bottom.into(), top.into()],
        reversed: vec![],
        ruled: false,
        guides: curves.to_vec(),
        closed: false,
        centerline: None,
        start: LoftEnd::None,
        end: LoftEnd::None,
        cut: false,
        merge: true,
    }));
    m.rebuild();
    valid(&m);
    // Straight guides through the side midpoints give the frustum exactly.
    assert_close(volume(&m), 10.0 / 3.0 * (100.0 + 400.0 + 200.0));
    // One guide is not enough to constrain the sections.
    if let FeatureKind::Loft(l) = &mut m.doc.feature_mut(loft).unwrap().kind {
        l.guides.truncate(1);
    }
    m.rebuild_all();
    assert!(
        m.status(loft)
            .unwrap()
            .error
            .unwrap()
            .contains("at least two")
    );
}

#[test]
fn closed_loft_joins_the_last_section_to_the_first() {
    let mut m = Model::new(Document::new());
    let mut sections = Vec::new();
    // Four radius 2 circles 20 mm from the Y axis, a quarter turn apart.
    for (plane, center) in [
        (Ref::FrontPlane, DVec3::new(20.0, 0.0, 0.0)),
        (Ref::RightPlane, DVec3::new(0.0, 0.0, 20.0)),
        (Ref::FrontPlane, DVec3::new(-20.0, 0.0, 0.0)),
        (Ref::RightPlane, DVec3::new(0.0, 0.0, -20.0)),
    ] {
        let id = m.doc.add(FeatureKind::Sketch(SketchFeature {
            plane: plane.clone(),
            sketch: Sketch::new(),
        }));
        m.rebuild();
        let frame = m.resolve(&plane).unwrap().plane().unwrap();
        let c = frame.to_local(center);
        let mut sk = Sketch::new();
        sk.add_circle_at(c, 2.0);
        if let FeatureKind::Sketch(s) = &mut m.doc.feature_mut(id).unwrap().kind {
            s.sketch = sk;
        }
        m.invalidate(id);
        sections.push(ProfileSelection::from(id));
    }
    // Travelling around the ring, the second and third sections' plane normals
    // point backwards, so they are reversed.
    let reversed = vec![sections[1].sketch(), sections[2].sketch()];
    let loft = m.doc.add(FeatureKind::Loft(Loft {
        profiles: sections.clone(),
        reversed,
        ruled: false,
        guides: vec![],
        closed: true,
        centerline: None,
        start: LoftEnd::None,
        end: LoftEnd::None,
        cut: false,
        merge: true,
    }));
    m.rebuild();
    valid(&m);
    // A smooth loop through only four circles approximates a torus; its spine
    // bulges outward between sections, so allow 10%.
    let ring = 2.0 * std::f64::consts::PI * 20.0 * std::f64::consts::PI * 4.0;
    let v = volume(&m);
    assert!((v / ring - 1.0).abs() < 0.1, "{v} vs {ring}");
    // A ring has no end caps.
    let names = &m.state().body.as_ref().unwrap().names;
    assert!(names.face(&format!("{loft}.start")).is_none());
    // Two sections can't close.
    if let FeatureKind::Loft(l) = &mut m.doc.feature_mut(loft).unwrap().kind {
        l.profiles.truncate(2);
    }
    m.rebuild_all();
    assert!(m.status(loft).unwrap().error.unwrap().contains("three"));
}

fn loft_of(profiles: Vec<ProfileSelection>) -> Loft {
    Loft {
        profiles,
        reversed: vec![],
        ruled: false,
        guides: vec![],
        closed: false,
        centerline: None,
        start: LoftEnd::None,
        end: LoftEnd::None,
        cut: false,
        merge: true,
    }
}

/// Squares of half-width 5 at z = 0 and 3 at z = 10.
fn tapered(m: &mut Model) -> (FeatureId, FeatureId) {
    let bottom = square_sketch(m, Ref::FrontPlane, 5.0);
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.0,
        flip: false,
    }));
    (bottom, square_sketch(m, Ref::Feature { id: plane }, 3.0))
}

/// Volume of square sections of half-width `r(z)` over `[0, 10]`.
fn squares(r: impl Fn(f64) -> f64) -> f64 {
    let n = 4000;
    let dz = 10.0 / n as f64;
    (0..n)
        .map(|i| 4.0 * r((i as f64 + 0.5) * dz).powi(2) * dz)
        .sum()
}

#[test]
fn loft_end_conditions_leave_the_sections_along_their_normals() {
    let mut m = Model::new(Document::new());
    let (bottom, top) = tapered(&mut m);
    let mut loft = loft_of(vec![bottom.into(), top.into()]);
    loft.start = LoftEnd::Normal {
        tangent_length: 1.0,
    };
    loft.end = LoftEnd::Normal {
        tangent_length: 1.0,
    };
    let id = m.doc.add(FeatureKind::Loft(loft));
    m.rebuild();
    valid(&m);
    // Height grows linearly and the half-width follows the Hermite blend with
    // zero end slopes.
    let hermite = |z: f64| {
        let s = z / 10.0;
        5.0 - 2.0 * (3.0 * s * s - 2.0 * s.powi(3))
    };
    let expected = squares(hermite);
    assert!((volume(&m) / expected - 1.0).abs() < 1e-5, "{}", volume(&m));
    let names = &m.state().body.as_ref().unwrap().names;
    assert!(names.face(&format!("{id}.start")).is_some());
    assert!(names.face(&format!("{id}.end")).is_some());

    // The tangent length is a formula target.
    let parameters = of_model::equations::parameters(m.doc.feature(id).unwrap());
    assert!(
        parameters
            .iter()
            .any(|p| p.key == format!("f{id}.start.tangent_length"))
    );

    // Start only: a quadratic half-width.
    if let FeatureKind::Loft(l) = &mut m.doc.feature_mut(id).unwrap().kind {
        l.end = LoftEnd::None;
    }
    m.invalidate(id);
    m.rebuild();
    valid(&m);
    let quadratic = squares(|z| 5.0 - 2.0 * (z / 10.0).powi(2));
    assert!(
        (volume(&m) / quadratic - 1.0).abs() < 1e-5,
        "{}",
        volume(&m)
    );

    // A direction condition along the front plane normal matches the normal one.
    if let FeatureKind::Loft(l) = &mut m.doc.feature_mut(id).unwrap().kind {
        l.start = LoftEnd::Direction {
            direction: Ref::FrontPlane,
            flip: false,
            tangent_length: 1.0,
        };
    }
    m.invalidate(id);
    m.rebuild();
    valid(&m);
    assert!(
        (volume(&m) / quadratic - 1.0).abs() < 1e-5,
        "{}",
        volume(&m)
    );

    // End conditions don't combine with ruled lofts or guides.
    if let FeatureKind::Loft(l) = &mut m.doc.feature_mut(id).unwrap().kind {
        l.ruled = true;
    }
    m.invalidate(id);
    m.rebuild();
    assert!(m.status(id).unwrap().error.unwrap().contains("ruled"));
}

#[test]
fn centerline_loft_follows_its_curve() {
    let mut m = Model::new(Document::new());
    let (bottom, top) = tapered(&mut m);
    // A line along Z past both sections. On the right plane local x runs along
    // world -z.
    let mut sk = Sketch::new();
    let (line, _, _) = sk.add_line_at(DVec2::new(2.0, 0.0), DVec2::new(-12.0, 0.0));
    let axis = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::RightPlane,
        sketch: sk,
    }));
    let mut loft = loft_of(vec![bottom.into(), top.into()]);
    loft.centerline = Some(Ref::SketchEntity {
        sketch: axis,
        entity: line,
    });
    let id = m.doc.add(FeatureKind::Loft(loft));
    m.rebuild();
    valid(&m);
    assert_close(volume(&m), 10.0 / 3.0 * (100.0 + 36.0 + 60.0));
    assert!(m.doc.feature(id).unwrap().kind.depends_on().contains(&axis));
    let names = &m.state().body.as_ref().unwrap().names;
    assert!(names.face(&format!("{id}.start")).is_some());

    // A centerline that stops short of the top section is rejected.
    let mut sk = Sketch::new();
    sk.add_line_at(DVec2::new(2.0, 0.0), DVec2::new(-8.0, 0.0));
    if let FeatureKind::Sketch(s) = &mut m.doc.feature_mut(axis).unwrap().kind {
        s.sketch = sk;
    }
    m.invalidate(axis);
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_some());
}
