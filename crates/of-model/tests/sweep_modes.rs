use of_model::{
    geom::DVec2,
    sketch::{Sketch, tools},
    *,
};
use std::f64::consts::{FRAC_PI_2, PI};

/// A straight path on the top plane, 20 mm along +Z from the origin.
fn straight_path(doc: &mut Document) -> FeatureId {
    let mut sketch = Sketch::new();
    sketch.add_line_at(DVec2::ZERO, DVec2::new(0.0, -20.0));
    doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::TopPlane,
        sketch,
    }))
}

fn front_sketch(doc: &mut Document, sketch: Sketch) -> FeatureId {
    doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }))
}

fn sweep(profile: Option<ProfileSelection>, path: FeatureId) -> Sweep {
    Sweep {
        profile,
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
    }
}

fn volume(model: &Model) -> f64 {
    for s in model.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
    let shape = &model.state().body.as_ref().unwrap().shape;
    assert!(shape.is_valid());
    shape.mass_props().unwrap().volume
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a / b - 1.0).abs() < rel, "{a} != {b}");
}

#[test]
fn circular_section_needs_no_profile_sketch() {
    let mut doc = Document::new();
    let path = straight_path(&mut doc);
    let mut spec = sweep(None, path);
    spec.circle_diameter = Some(4.0);
    let id = doc.add(FeatureKind::Sweep(spec));
    let mut m = Model::new(doc);
    close(volume(&m), PI * 4.0 * 20.0, 1e-9);
    // The diameter is an equation parameter.
    m.doc
        .equations
        .insert(format!("f{id}.circle_diameter"), "6mm".into());
    m.rebuild_all();
    close(volume(&m), PI * 9.0 * 20.0, 1e-9);

    // A profile and a circle together are ambiguous.
    let profile = {
        let mut sk = Sketch::new();
        sk.add_circle_at(DVec2::ZERO, 1.0);
        front_sketch(&mut m.doc, sk)
    };
    if let FeatureKind::Sweep(s) = &mut m.doc.feature_mut(id).unwrap().kind {
        s.profile = Some(profile.into());
    }
    m.rebuild_all();
    assert!(m.status(id).unwrap().error.unwrap().contains("not both"));
}

#[test]
fn thin_sweeps_thicken_open_and_closed_profiles() {
    // A 1 mm mid-plane wall around a 10 mm square runs from 9 to 11 mm across.
    for (closed, area) in [(false, 10.0), (true, 11.0 * 11.0 - 9.0 * 9.0)] {
        let mut doc = Document::new();
        let path = straight_path(&mut doc);
        let mut sk = Sketch::new();
        if closed {
            tools::corner_rectangle(&mut sk, DVec2::splat(-5.0), DVec2::splat(5.0));
        } else {
            sk.add_line_at(DVec2::new(-5.0, 0.0), DVec2::new(5.0, 0.0));
        }
        let profile = front_sketch(&mut doc, sk);
        let mut spec = sweep(Some(profile.into()), path);
        spec.thin = Some(Thin {
            kind: ThinKind::MidPlane,
            thickness: 1.0,
            thickness2: 0.0,
            reverse: false,
        });
        doc.add(FeatureKind::Sweep(spec));
        let m = Model::new(doc);
        close(volume(&m), area * 20.0, 1e-9);
    }
}

#[test]
fn twist_turns_the_section_and_keeps_its_area() {
    let mut doc = Document::new();
    let path = straight_path(&mut doc);
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::splat(-2.0), DVec2::splat(2.0));
    let profile = front_sketch(&mut doc, sk);
    let mut spec = sweep(Some(profile.into()), path);
    spec.twist = FRAC_PI_2;
    let id = doc.add(FeatureKind::Sweep(spec));
    let mut m = Model::new(doc);
    close(volume(&m), 320.0, 1e-5);
    let top = |m: &Model| {
        let shape = &m.state().body.as_ref().unwrap().shape;
        (0..shape.count(of_model::geom::Kind::Vertex))
            .map(|i| shape.vertex_point(i).unwrap())
            .filter(|p| (p.z - 20.0).abs() < 1e-6)
            .map(|p| p.x)
            .fold(f64::MIN, f64::max)
    };
    // A quarter turn maps the square onto itself; an eighth turn puts a corner on +X.
    assert!((top(&m) - 2.0).abs() < 1e-5);
    m.doc
        .equations
        .insert(format!("f{id}.twist"), "45deg".into());
    m.rebuild_all();
    close(volume(&m), 320.0, 1e-5);
    assert!((top(&m) - 2.0 * 2f64.sqrt()).abs() < 1e-5);
}

#[test]
fn guide_curve_beside_the_path_keeps_a_straight_sweep_untwisted() {
    let mut doc = Document::new();
    let path = straight_path(&mut doc);
    let mut guide = Sketch::new();
    guide.add_line_at(DVec2::new(8.0, 0.0), DVec2::new(8.0, -20.0));
    let guide = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::TopPlane,
        sketch: guide,
    }));
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::splat(-2.0), DVec2::splat(2.0));
    let profile = front_sketch(&mut doc, sk);
    let mut spec = sweep(Some(profile.into()), path);
    spec.guide = Some(Ref::Feature { id: guide });
    let id = doc.add(FeatureKind::Sweep(spec));
    let mut m = Model::new(doc);
    close(volume(&m), 320.0, 1e-9);
    // A twist together with a guide is rejected.
    if let FeatureKind::Sweep(s) = &mut m.doc.feature_mut(id).unwrap().kind {
        s.twist = 1.0;
    }
    m.rebuild_all();
    assert!(m.status(id).unwrap().error.is_some());
}

#[test]
fn guides_scale_and_stretch_the_section() {
    let mut doc = Document::new();
    let path = straight_path(&mut doc);
    // On the top plane local y runs along world -z: this guide leaves the
    // radius 2 circle's rim and reaches radius 4 at the path end.
    let mut guide = Sketch::new();
    let (line, _, _) = guide.add_line_at(DVec2::new(2.0, 0.0), DVec2::new(4.0, -20.0));
    let guide = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::TopPlane,
        sketch: guide,
    }));
    // A second guide along world y at a fixed distance. On the right plane
    // local x runs along world -z and local y along world y.
    let mut second = Sketch::new();
    second.add_line_at(DVec2::new(0.0, 2.0), DVec2::new(-20.0, 2.0));
    let second = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::RightPlane,
        sketch: second,
    }));
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::ZERO, 2.0);
    let profile = front_sketch(&mut doc, sk);
    let mut spec = sweep(Some(profile.into()), path);
    spec.guide = Some(Ref::SketchEntity {
        sketch: guide,
        entity: line,
    });
    spec.guide_scale = true;
    let id = doc.add(FeatureKind::Sweep(spec));
    let mut m = Model::new(doc);
    close(volume(&m), PI * 20.0 / 3.0 * (4.0 + 8.0 + 16.0), 1e-6);
    let names = &m.state().body.as_ref().unwrap().names;
    assert!(names.face(&format!("{id}.start")).is_some());
    assert!(names.face(&format!("{id}.end")).is_some());
    assert!(
        m.doc
            .feature(id)
            .unwrap()
            .kind
            .depends_on()
            .contains(&guide)
    );

    // The second guide stretches the section along x only: an elliptic frustum.
    if let FeatureKind::Sweep(s) = &mut m.doc.feature_mut(id).unwrap().kind {
        s.guide_scale = false;
        s.guide2 = Some(Ref::Feature { id: second });
    }
    m.rebuild_all();
    close(volume(&m), PI * 2.0 * 60.0, 1e-6);

    // Scaling guides don't combine with a twist.
    if let FeatureKind::Sweep(s) = &mut m.doc.feature_mut(id).unwrap().kind {
        s.twist = 0.5;
    }
    m.rebuild_all();
    assert!(m.status(id).unwrap().error.unwrap().contains("twist"));
    // A second guide needs the first.
    if let FeatureKind::Sweep(s) = &mut m.doc.feature_mut(id).unwrap().kind {
        s.twist = 0.0;
        s.guide = None;
    }
    m.rebuild_all();
    assert!(m.status(id).unwrap().error.unwrap().contains("first guide"));
}
