//! Reference stability across imported, patterned and generated topology:
//! references follow their target through upstream edits and save/reopen, and
//! report a missing reference rather than binding to unrelated topology.

use std::f64::consts::{PI, TAU};

use of_geom::{CurveKind, DVec2, DVec3, Kind, SurfaceKind};
use of_model::sketch::{Sketch, tools};
use of_model::*;

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

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol * b.abs().max(1.0), "{a} != {b}");
}

fn no_errors(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
}

fn reopened(m: &Model) -> Model {
    Model::new(file::from_str(&file::to_string(&m.doc)).unwrap())
}

fn extrude(sketch: FeatureId, depth: f64) -> Extrude {
    Extrude {
        sketch,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }
}

fn sketch(doc: &mut Document, plane: Ref, sketch: Sketch) -> FeatureId {
    doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch }))
}

/// A `w` x 20 x `h` block's STEP file, base64-encoded.
fn block_step(w: f64, h: f64) -> String {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(w, 20.0));
    let s = sketch(&mut doc, Ref::FrontPlane, sk);
    doc.add(FeatureKind::Extrude(extrude(s, h)));
    let m = Model::new(doc);
    let path = std::env::temp_dir().join(format!(
        "of-model-corpus-{w}-{h}-{}.step",
        std::process::id()
    ));
    m.state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .write_step(path.to_str().unwrap())
        .unwrap();
    let data = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// The index of the face of the current body matching `pred`.
fn face_where(m: &Model, pred: impl Fn(&of_geom::FaceInfo) -> bool) -> u32 {
    let body = &m.state().body.as_ref().unwrap().shape;
    (0..body.count(Kind::Face))
        .find(|&i| pred(&body.face_info(i).unwrap()))
        .expect("no matching face")
}

#[test]
fn imported_references_survive_reload_and_reject_a_different_file() {
    let original = block_step(40.0, 10.0);
    let mut doc = Document::new();
    let import = doc.add(FeatureKind::Import(Import {
        format: ImportFormat::Step,
        source: "block.step".into(),
        data: original.clone(),
        mesh: None,
    }));
    let mut m = Model::new(doc);
    no_errors(&m);
    // A fillet on a vertical edge and a boss on the top face of the import.
    let body = &m.state().body.as_ref().unwrap().shape;
    let edge = (0..body.count(Kind::Edge))
        .find(|&i| {
            let e = body.edge_info(i).unwrap();
            (e.end - e.start).normalize().z.abs() > 0.99
        })
        .unwrap();
    let edge = m.state().qualified_ref_for(Kind::Edge, edge).unwrap();
    let top = face_where(&m, |f| f.normal.z > 0.99);
    let top_ref = m.state().ref_for(Kind::Face, top).unwrap();
    let fillet = m.doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![edge],
        faces: vec![],
        radius: 2.0,
    }));
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::ZERO, 3.0);
    let on_top = sketch(&mut m.doc, top_ref.clone(), sk);
    let boss = m.doc.add(FeatureKind::Extrude(extrude(on_top, 5.0)));
    m.rebuild();
    no_errors(&m);
    let expected = 8000.0 - (4.0 - PI) * 10.0 + PI * 9.0 * 5.0;
    close(volume(&m), expected, 1e-6);
    // The references name the file's content.
    let Ref::Face { name } = &top_ref else {
        panic!("{top_ref:?}")
    };
    assert!(name.starts_with(&format!("{import}.i")), "{name}");
    let r = reopened(&m);
    no_errors(&r);
    close(volume(&r), expected, 1e-9);

    // Older files named imported faces by position only; those names still work.
    let legacy = m.state().body.as_ref().unwrap().names.faces[top as usize]
        .iter()
        .find(|n| !n.contains(".i"))
        .unwrap()
        .clone();
    assert!(m.resolve(&Ref::Face { name: legacy }).is_ok());

    // A different file: its faces are not the ones the features were built on.
    let set_data = |m: &mut Model, data: String| {
        if let FeatureKind::Import(i) = &mut m.doc.feature_mut(import).unwrap().kind {
            i.data = data;
        }
        m.invalidate(import);
        m.rebuild();
    };
    set_data(&mut m, block_step(40.0, 15.0));
    assert!(m.status(import).unwrap().error.is_none());
    for id in [fillet, on_top, boss] {
        assert!(
            m.status(id).unwrap().error.is_some(),
            "feature {id} rebound"
        );
    }
    close(volume(&m), 12000.0, 1e-6);
    // The same file again restores every reference.
    set_data(&mut m, original);
    no_errors(&m);
    close(volume(&m), expected, 1e-6);
}

/// A 60 x 20 x 10 block with a radius 2, 5 high boss at (5, 10) on top, and a
/// linear pattern of the boss along X.
fn patterned(spacing: f64, count: u32) -> (Model, FeatureId) {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(60.0, 20.0));
    let s = sketch(&mut doc, Ref::FrontPlane, sk);
    doc.add(FeatureKind::Extrude(extrude(s, 10.0)));
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.0,
        flip: false,
    }));
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::new(5.0, 10.0), 2.0);
    let s = sketch(&mut doc, Ref::Feature { id: plane }, sk);
    let boss = doc.add(FeatureKind::Extrude(extrude(s, 5.0)));
    let pattern = doc.add(FeatureKind::LinearPattern(LinearPattern {
        features: vec![boss],
        direction1: PatternDirection {
            reference: Ref::RightPlane,
            spacing,
            count,
            reverse: false,
        },
        direction2: None,
    }));
    let m = Model::new(doc);
    no_errors(&m);
    (m, pattern)
}

#[test]
fn patterned_references_follow_their_instance() {
    let (mut m, pattern) = patterned(15.0, 3);
    // Fillet the top rim of the second boss.
    let body = &m.state().body.as_ref().unwrap().shape;
    let rim = (0..body.count(Kind::Edge))
        .find(|&i| {
            let e = body.edge_info(i).unwrap();
            e.kind == CurveKind::Circle
                && (e.center.z - 15.0).abs() < 1e-6
                && (e.center.x - 20.0).abs() < 1e-6
        })
        .unwrap();
    let rim = m.state().qualified_ref_for(Kind::Edge, rim).unwrap();
    let fillet = m.doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![rim],
        faces: vec![],
        radius: 0.5,
    }));
    m.rebuild();
    no_errors(&m);
    let plain = 12000.0 + 3.0 * PI * 4.0 * 5.0;
    let filleted = volume(&m);
    assert!(filleted < plain && filleted > plain - 1.0);
    // The rounded face sits on the second boss, wherever the pattern puts it.
    let torus_x = |m: &Model| {
        let body = &m.state().body.as_ref().unwrap().shape;
        (0..body.count(Kind::Face))
            .map(|i| body.face_info(i).unwrap())
            .find(|f| f.kind == SurfaceKind::Torus)
            .unwrap()
            .centroid
            .x
    };
    close(torus_x(&m), 20.0, 1e-6);
    let set = |m: &mut Model, spacing: f64, count: u32| {
        if let FeatureKind::LinearPattern(p) = &mut m.doc.feature_mut(pattern).unwrap().kind {
            p.direction1.spacing = spacing;
            p.direction1.count = count;
        }
        m.invalidate(pattern);
        m.rebuild();
    };
    set(&mut m, 12.0, 3);
    no_errors(&m);
    close(torus_x(&m), 17.0, 1e-6);
    close(volume(&m), filleted, 1e-9);
    // Dropping the third instance keeps the second.
    set(&mut m, 12.0, 2);
    no_errors(&m);
    close(torus_x(&m), 17.0, 1e-6);
    let r = reopened(&m);
    no_errors(&r);
    close(volume(&r), volume(&m), 1e-9);
    // Without a second instance the fillet reports its missing edge and the
    // part keeps its previous valid geometry.
    set(&mut m, 12.0, 1);
    assert!(m.status(fillet).unwrap().error.is_some());
    close(volume(&m), 12000.0 + PI * 4.0 * 5.0, 1e-6);
    set(&mut m, 15.0, 3);
    no_errors(&m);
    close(volume(&m), filleted, 1e-9);
}

#[test]
fn generated_curve_sweeps_keep_their_names_under_patterns() {
    let mut doc = Document::new();
    let helix = doc.add(FeatureKind::Helix(Helix {
        plane: Ref::FrontPlane,
        radius: 10.0,
        pitch: 6.0,
        turns: 2.0,
        start_angle: 0.0,
        left_handed: false,
        defined_by: HelixInput::PitchAndTurns,
        height: 0.0,
        variable_pitch: vec![],
    }));
    let spring = doc.add(FeatureKind::Sweep(Sweep {
        profile: None,
        inner_profiles: vec![],
        circle_diameter: Some(2.0),
        twist: 0.0,
        guide: None,
        guide_scale: false,
        guide2: None,
        thin: None,
        path: Ref::Feature { id: helix },
        reverse_path: false,
        orientation: SweepOrientation::MinimumTwist,
        cut: false,
        merge: true,
    }));
    let axis = doc.add(FeatureKind::Axis(AxisDef::TwoPlanes {
        a: Ref::RightPlane,
        b: Ref::TopPlane,
    }));
    doc.add(FeatureKind::CircularPattern(CircularPattern {
        features: vec![spring],
        axis: Ref::Feature { id: axis },
        count: 2,
        angle: TAU,
        equal_spacing: true,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    no_errors(&m);
    // Dome the end cap of the copy, half a turn around from the original's end.
    let cap = |m: &Model, z: f64| {
        face_where(m, |f| {
            f.kind == SurfaceKind::Plane && (f.centroid - DVec3::new(-10.0, 0.0, z)).length() < 1e-3
        })
    };
    let end = cap(&m, 12.0);
    let face = m.state().qualified_ref_for(Kind::Face, end).unwrap();
    let dome = m.doc.add(FeatureKind::Dome(Dome {
        face,
        height: 0.5,
        inward: false,
        elliptical: false,
    }));
    m.rebuild();
    no_errors(&m);
    let cap_volume = PI * 0.5 * (3.0 + 0.25) / 6.0;
    let springs = volume(&m) - cap_volume;
    // A steeper helix moves both copies' ends; the dome follows the copy's cap.
    if let FeatureKind::Helix(h) = &mut m.doc.feature_mut(helix).unwrap().kind {
        h.pitch = 7.0;
    }
    m.invalidate(helix);
    m.rebuild();
    no_errors(&m);
    let longer = |pitch: f64| 2.0 * 2.0 * (TAU * TAU * 100.0 + pitch * pitch).sqrt();
    close(
        volume(&m),
        springs * longer(7.0) / longer(6.0) + cap_volume,
        1e-5,
    );
    // The dome now caps the copy's moved end, and no longer the old position.
    let names = &m.state().body.as_ref().unwrap().names;
    let body = &m.state().body.as_ref().unwrap().shape;
    let domed: Vec<_> = (0..body.count(Kind::Face))
        .filter(|&i| {
            names.faces[i as usize]
                .iter()
                .any(|n| n.starts_with(&format!("{dome}.")))
        })
        .map(|i| body.face_info(i).unwrap().centroid)
        .collect();
    assert_eq!(domed.len(), 1);
    assert!(
        (domed[0] - DVec3::new(-10.0, 0.0, 14.0)).length() < 1.0,
        "{domed:?}"
    );
    let r = reopened(&m);
    no_errors(&r);
    close(volume(&r), volume(&m), 1e-9);
}
