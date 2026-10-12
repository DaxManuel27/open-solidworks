use of_model::{
    geom::{DVec2, DVec3, Kind, SurfaceKind},
    sketch::{Sketch, tools},
    *,
};
use std::f64::consts::PI;

const R: f64 = 10.0;
const H: f64 = 40.0;

/// A radius-10, 40-high cylinder standing on the front plane.
fn cylinder() -> Model {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::ZERO, R);
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: H },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    Model::new(doc)
}

fn side(m: &Model) -> Ref {
    let body = m.state().body.as_ref().unwrap();
    let i = (0..body.shape.count(Kind::Face))
        .find(|&i| body.shape.face_info(i).unwrap().kind == SurfaceKind::Cylinder)
        .unwrap();
    m.state().ref_for(Kind::Face, i).unwrap()
}

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
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume
}

fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6 * b.abs().max(1.0), "{a} != {b}");
}

/// A sketch on a plane tangent to the cylinder at +X with a `w` wide (around the
/// axis) by `h` high rectangle centred on the tangent line, starting at height `z`.
fn wrap_sketch(m: &mut Model, w: f64, h: f64, z: f64, plane: Option<Ref>) -> FeatureId {
    let plane = plane.unwrap_or_else(|| {
        let id = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
            reference: Ref::RightPlane,
            distance: R,
            flip: false,
        }));
        Ref::Feature { id }
    });
    let id = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: plane.clone(),
        sketch: Sketch::new(),
    }));
    m.rebuild();
    let frame = m.resolve(&plane).unwrap().plane().unwrap();
    let a = frame.to_local(DVec3::new(R, -w / 2.0, z));
    let b = frame.to_local(DVec3::new(R, w / 2.0, z + h));
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, a.min(b), a.max(b));
    if let FeatureKind::Sketch(s) = &mut m.doc.feature_mut(id).unwrap().kind {
        s.sketch = sk;
    }
    m.invalidate(id);
    id
}

fn add_wrap(m: &mut Model, sketch: FeatureId, mode: WrapMode, depth: f64) -> FeatureId {
    let face = side(m);
    let id = m.doc.add(FeatureKind::Wrap(Wrap {
        sketch,
        face,
        mode,
        depth,
    }));
    m.rebuild();
    id
}

fn set_wrap(m: &mut Model, id: FeatureId, edit: impl FnOnce(&mut Wrap)) {
    if let FeatureKind::Wrap(w) = &mut m.doc.feature_mut(id).unwrap().kind {
        edit(w);
    }
    m.invalidate(id);
    m.rebuild();
}

/// Volume of an annular sector `w` of arc length wide and `h` high between radii.
fn sector(w: f64, h: f64, r0: f64, r1: f64) -> f64 {
    0.5 * (w / R) * (r1 * r1 - r0 * r0) * h
}

#[test]
fn emboss_deboss_and_scribe_across_the_seam() {
    let full = PI * R * R * H;
    let mut m = cylinder();
    let s = wrap_sketch(&mut m, 5.0, 4.0, 10.0, None);
    let w = add_wrap(&mut m, s, WrapMode::Emboss, 1.0);
    valid(&m);
    assert_close(volume(&m), full + sector(5.0, 4.0, R, R + 1.0));

    set_wrap(&mut m, w, |p| p.mode = WrapMode::Deboss);
    valid(&m);
    assert_close(volume(&m), full - sector(5.0, 4.0, R - 1.0, R));

    // Upstream depth edits rebuild.
    set_wrap(&mut m, w, |p| p.depth = 2.0);
    valid(&m);
    assert_close(volume(&m), full - sector(5.0, 4.0, R - 2.0, R));

    let faces = m.state().body.as_ref().unwrap().shape.count(Kind::Face);
    set_wrap(&mut m, w, |p| p.mode = WrapMode::Scribe);
    valid(&m);
    assert_close(volume(&m), full);
    let scribed = m.state().body.as_ref().unwrap().shape.count(Kind::Face);
    assert!(
        scribed > 3,
        "scribing splits the side face: {scribed} faces"
    );
    assert!(faces > 3);

    let text = file::to_string(&m.doc);
    let reopened = Model::new(file::from_str(&text).unwrap());
    valid(&reopened);
    assert_close(volume(&reopened), full);
}

#[test]
fn wrap_rejects_invalid_input() {
    let mut m = cylinder();
    let s = wrap_sketch(&mut m, 5.0, 4.0, 10.0, None);
    let w = add_wrap(&mut m, s, WrapMode::Deboss, R);
    assert!(m.status(w).unwrap().error.unwrap().contains("radius"));
    set_wrap(&mut m, w, |p| p.depth = 0.0);
    assert!(m.status(w).unwrap().error.unwrap().contains("depth"));

    // A sketch past the end of the face.
    let high = wrap_sketch(&mut m, 5.0, 4.0, 38.0, None);
    let w2 = add_wrap(&mut m, high, WrapMode::Emboss, 1.0);
    assert!(m.status(w2).unwrap().error.unwrap().contains("past"));

    // A sketch plane across the axis.
    let front = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 20.0,
        flip: false,
    }));
    let across = wrap_sketch(&mut m, 2.0, 2.0, 20.0, Some(Ref::Feature { id: front }));
    let w3 = add_wrap(&mut m, across, WrapMode::Emboss, 1.0);
    assert!(m.status(w3).unwrap().error.unwrap().contains("parallel"));
}

#[test]
fn wraps_onto_a_domed_cap_along_geodesics() {
    let mut m = cylinder();
    let body = m.state().body.as_ref().unwrap();
    let top = (0..body.shape.count(Kind::Face))
        .find(|&i| body.shape.face_info(i).unwrap().normal.z > 0.99)
        .unwrap();
    let face = m.state().ref_for(Kind::Face, top).unwrap();
    let h = 5.0;
    m.doc.add(FeatureKind::Dome(Dome {
        face,
        height: h,
        inward: false,
        elliptical: false,
    }));
    m.rebuild();
    valid(&m);
    // A sketch on the plane touching the dome's apex.
    let apex = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: H + h,
        flip: false,
    }));
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::ZERO, 2.0);
    let sketch = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::Feature { id: apex },
        sketch: sk,
    }));
    m.rebuild();
    let before = volume(&m);
    let body = m.state().body.as_ref().unwrap();
    let faces = body.shape.count(Kind::Face);
    let cap = (0..body.shape.count(Kind::Face))
        .find(|&i| body.shape.face_info(i).unwrap().kind == SurfaceKind::Sphere)
        .unwrap();
    let face = m.state().ref_for(Kind::Face, cap).unwrap();
    let id = m.doc.add(FeatureKind::Wrap(Wrap {
        sketch,
        face,
        mode: WrapMode::Emboss,
        depth: 0.5,
    }));
    m.rebuild();
    valid(&m);
    // The sphere through the rim and apex, and the cap of geodesic radius 2 on it.
    let rs = (R * R + h * h) / (2.0 * h);
    let shell = 2.0 * PI / 3.0 * ((rs + 0.5).powi(3) - rs.powi(3)) * (1.0 - (2.0 / rs).cos());
    assert_close(volume(&m) - before, shell);

    // Scribing splits the cap without changing the volume.
    if let FeatureKind::Wrap(w) = &mut m.doc.feature_mut(id).unwrap().kind {
        w.mode = WrapMode::Scribe;
    }
    m.invalidate(id);
    m.rebuild();
    valid(&m);
    assert_close(volume(&m), before);
    let scribed = m.state().body.as_ref().unwrap().shape.count(Kind::Face);
    assert_eq!(scribed, faces + 1);

    // A larger sketch stretches too much and reports how much.
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::ZERO, 6.0);
    if let FeatureKind::Sketch(s) = &mut m.doc.feature_mut(sketch).unwrap().kind {
        s.sketch = sk;
    }
    m.invalidate(sketch);
    m.rebuild();
    let error = m.status(id).unwrap().error.unwrap();
    assert!(error.contains("stretch") && error.contains('%'), "{error}");
}
