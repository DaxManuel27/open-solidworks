use of_model::{
    geom::{DVec2, Kind, SurfaceKind},
    sketch::Sketch,
    *,
};
use std::f64::consts::{PI, TAU};

const DEPTH: f64 = 0.541_265_877_365_274; // 5H/8 per unit pitch

fn groove(r0: f64, a: f64, b: f64, h: f64, length: f64, pitch: f64) -> f64 {
    let area = (a + b) / 2.0 * h;
    let centroid = r0 + h * (a + 2.0 * b) / (3.0 * (a + b));
    area * TAU * centroid * length / pitch
}

/// A cylinder of `radius`, 20 mm long, optionally bored to `bore`.
fn part(radius: f64, bore: Option<f64>) -> Model {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::ZERO, radius);
    if let Some(b) = bore {
        sk.add_circle_at(DVec2::ZERO, b);
    }
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: 20.0 },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    Model::new(doc)
}

fn cylinder_face(m: &Model, radius: f64) -> Ref {
    let body = m.state().body.as_ref().unwrap();
    let i = (0..body.shape.count(Kind::Face))
        .find(|&i| {
            let f = body.shape.face_info(i).unwrap();
            f.kind == SurfaceKind::Cylinder && (f.radius - radius).abs() < 1e-9
        })
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

fn thread(face: Ref, diameter: f64, pitch: f64, length: f64, internal: bool) -> Thread {
    Thread {
        face,
        diameter,
        pitch,
        length,
        start_offset: 0.0,
        start_angle: 0.0,
        reverse: false,
        left_handed: false,
        internal,
    }
}

#[test]
fn external_thread_rebuilds_with_pitch_and_survives_reload() {
    let mut m = part(5.0, None);
    let full = PI * 25.0 * 20.0;
    let face = cylinder_face(&m, 5.0);
    let mut spec = thread(face, 10.0, 1.5, 15.0, false);
    spec.start_offset = 2.0;
    let t = m.doc.add(FeatureKind::Thread(spec));
    m.rebuild();
    valid(&m);
    let removed = |p: f64| groove(5.0 - DEPTH * p, p / 4.0, 7.0 * p / 8.0, DEPTH * p, 15.0, p);
    assert!(((full - volume(&m)) / removed(1.5) - 1.0).abs() < 1e-4);

    for edit in [
        |t: &mut Thread| t.pitch = 1.0,
        |t: &mut Thread| t.left_handed = true,
        |t: &mut Thread| t.reverse = true,
    ] {
        if let FeatureKind::Thread(spec) = &mut m.doc.feature_mut(t).unwrap().kind {
            edit(spec);
        }
        m.invalidate(t);
        m.rebuild();
        valid(&m);
        assert!(((full - volume(&m)) / removed(1.0) - 1.0).abs() < 1e-4);
    }

    let reopened = Model::new(file::from_str(&file::to_string(&m.doc)).unwrap());
    valid(&reopened);
    assert!((volume(&reopened) - volume(&m)).abs() < 1e-6);
}

#[test]
fn internal_thread_cuts_the_bore() {
    let p = 1.5;
    let minor = 5.0 - DEPTH * p;
    let mut m = part(10.0, Some(minor));
    let before = volume(&m);
    let face = cylinder_face(&m, minor);
    m.doc.add(FeatureKind::Thread(thread(
        face,
        2.0 * minor,
        p,
        20.0,
        true,
    )));
    m.rebuild();
    valid(&m);
    let expected = groove(minor, 0.75 * p, p / 8.0, DEPTH * p, 20.0, p);
    assert!(((before - volume(&m)) / expected - 1.0).abs() < 1e-4);
}

#[test]
fn thread_rejects_mismatched_input() {
    let mut m = part(5.0, None);
    let face = cylinder_face(&m, 5.0);
    // Wrong diameter, internal on a boss, and longer than the face.
    for spec in [
        thread(face.clone(), 12.0, 1.5, 10.0, false),
        thread(face.clone(), 10.0, 1.5, 10.0, true),
        thread(face.clone(), 10.0, 1.5, 25.0, false),
    ] {
        let id = m.doc.add(FeatureKind::Thread(spec));
        m.rebuild_all();
        assert!(m.status(id).unwrap().error.is_some());
        m.doc.delete(&[id]);
        m.rebuild_all();
    }
}
