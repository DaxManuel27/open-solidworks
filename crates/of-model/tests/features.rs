use std::f64::consts::{PI, TAU};

use of_geom::{DVec2, DVec3, Kind};
use of_model::sketch::{ConstraintKind as K, Sketch, tools};
use of_model::*;

fn volume(m: &Model) -> f64 {
    m.state()
        .body
        .as_ref()
        .map(|b| b.shape.mass_props().unwrap().volume)
        .unwrap_or(0.0)
}

fn assert_close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

fn rect_sketch(doc: &mut Document, plane: Ref, p0: DVec2, p1: DVec2) -> FeatureId {
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, p0, p1);
    doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch: sk }))
}

fn blind(sketch: FeatureId, depth: f64) -> Extrude {
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

fn no_errors(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
}

/// A 40 x 20 x 10 block on the front plane.
fn block() -> (Model, FeatureId, FeatureId) {
    let mut doc = Document::new();
    let s = rect_sketch(
        &mut doc,
        Ref::FrontPlane,
        DVec2::ZERO,
        DVec2::new(40.0, 20.0),
    );
    let e = doc.add(FeatureKind::Extrude(blind(s, 10.0)));
    let m = Model::new(doc);
    no_errors(&m);
    (m, s, e)
}

/// The reference for the face whose outward normal is `n`.
fn face_with_normal(m: &Model, n: DVec3) -> Ref {
    let body = m.state().body.as_ref().unwrap();
    for i in 0..body.shape.count(Kind::Face) {
        let f = body.shape.face_info(i).unwrap();
        if f.normal.dot(n) > 0.999 {
            return m.state().ref_for(Kind::Face, i).unwrap();
        }
    }
    panic!("no face facing {n}");
}

/// Edges of the face facing `n`.
fn edges_of(m: &Model, n: DVec3) -> Vec<Ref> {
    let state = m.state();
    let body = state.body.as_ref().unwrap();
    let Ref::Face { name } = face_with_normal(m, n) else {
        unreachable!()
    };
    let f = body.names.face(&name).unwrap();
    body.shape.face_edges()[f as usize]
        .iter()
        .map(|e| state.ref_for(Kind::Edge, *e).unwrap())
        .collect()
}

#[test]
fn extrude_variants() {
    let (m, _, _) = block();
    assert_close(volume(&m), 8000.0, 1e-6);
    let names = &m.state().body.as_ref().unwrap().names;
    assert!(
        names.faces.iter().any(|f| f[0].ends_with(".end")),
        "{:?}",
        names.faces
    );

    let (mut m, _, e) = block();
    let f = m.doc.feature_mut(e).unwrap();
    let FeatureKind::Extrude(x) = &mut f.kind else {
        unreachable!()
    };
    x.end1 = EndCondition::MidPlane { depth: 6.0 };
    m.invalidate(e);
    m.rebuild();
    no_errors(&m);
    let bb = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    assert_close(bb.min.z, -3.0, 1e-6);
    assert_close(volume(&m), 4800.0, 1e-6);

    // Two directions, reversed first.
    let FeatureKind::Extrude(x) = &mut m.doc.feature_mut(e).unwrap().kind else {
        unreachable!()
    };
    x.end1 = EndCondition::Blind { depth: 5.0 };
    x.end2 = Some(EndCondition::Blind { depth: 2.0 });
    x.reverse = true;
    m.invalidate(e);
    m.rebuild();
    no_errors(&m);
    let bb = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    assert_close(bb.min.z, -5.0, 1e-6);
    assert_close(bb.max.z, 2.0, 1e-6);
}

#[test]
fn fillet_survives_upstream_edit() {
    let (mut m, _, e) = block();
    let top_edges = edges_of(&m, DVec3::Z);
    assert_eq!(top_edges.len(), 4);
    let fid = m.doc.add(FeatureKind::Fillet(Fillet {
        edges: top_edges.clone(),
        faces: vec![],
        radius: 2.0,
    }));
    m.rebuild();
    no_errors(&m);
    let v1 = volume(&m);
    let lost_per_len = 4.0 - PI; // 2x2 square minus a quarter circle, per mm of edge
    assert_close(
        v1,
        8000.0 - lost_per_len * (2.0 * (40.0 + 20.0) - 0.0) + 0.0,
        40.0,
    );

    // Make the block taller: the fillet's edges must still be found.
    let FeatureKind::Extrude(x) = &mut m.doc.feature_mut(e).unwrap().kind else {
        unreachable!()
    };
    x.end1 = EndCondition::Blind { depth: 25.0 };
    m.invalidate(e);
    m.rebuild();
    no_errors(&m);
    let v2 = volume(&m);
    assert_close(v2 - v1, 40.0 * 20.0 * 15.0, 1e-3);
    assert!(
        m.state()
            .body
            .as_ref()
            .unwrap()
            .names
            .faces
            .iter()
            .any(|f| f[0].starts_with(&format!("{fid}.f(")))
    );
}

#[test]
fn cut_on_face_follows_face_and_patterns() {
    let (mut m, _, e) = block();
    let top = face_with_normal(&m, DVec3::Z);
    let mut sk = Sketch::new();
    let c = sk.add_circle_at(DVec2::new(5.0, 5.0), 2.0);
    let _ = c;
    let s2 = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: top.clone(),
        sketch: sk,
    }));
    let mut cut = blind(s2, 0.0);
    cut.cut = true;
    cut.end1 = EndCondition::ThroughAll;
    cut.reverse = true;
    let c1 = m.doc.add(FeatureKind::Extrude(cut));
    m.rebuild();
    no_errors(&m);
    let hole = PI * 4.0 * 10.0;
    assert_close(volume(&m), 8000.0 - hole, 1e-3);

    // Linear pattern along X: 4 holes 10 mm apart.
    let lp = m.doc.add(FeatureKind::LinearPattern(LinearPattern {
        features: vec![c1],
        direction1: PatternDirection {
            reference: Ref::RightPlane,
            spacing: 10.0,
            count: 4,
            reverse: false,
        },
        direction2: None,
    }));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), 8000.0 - 4.0 * hole, 1e-3);

    // Taller block: the sketch on the top face moves up, the cut still goes through.
    let FeatureKind::Extrude(x) = &mut m.doc.feature_mut(e).unwrap().kind else {
        unreachable!()
    };
    x.end1 = EndCondition::Blind { depth: 20.0 };
    m.invalidate(e);
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), 16000.0 - 8.0 * hole, 1e-2);

    // Suppressing the pattern brings three holes back.
    m.doc.feature_mut(lp).unwrap().suppressed = true;
    m.invalidate(lp);
    m.rebuild();
    assert_close(volume(&m), 16000.0 - 2.0 * hole, 1e-2);

    // Deleting the cut deletes the pattern that depends on it.
    let gone = m.doc.delete(&[c1]);
    assert!(gone.contains(&lp));
    m.rebuild_all();
    assert_close(volume(&m), 16000.0, 1e-3);
}

#[test]
fn revolve_shell_chamfer() {
    let mut doc = Document::new();
    // Cup profile: rectangle 10..20 x 0..30 revolved around the Y axis -> tube.
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(0.0, 0.0), DVec2::new(20.0, 30.0));
    let (axis, _, _) = sk.add_line_at(DVec2::new(0.0, -5.0), DVec2::new(0.0, 40.0));
    sk.entities.get_mut(&axis).unwrap().construction = true;
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Revolve(Revolve {
        sketch: s,
        axis: Ref::SketchEntity {
            sketch: s,
            entity: axis,
        },
        angle: RevolveAngle::Blind { angle: TAU },
        reverse: false,
        cut: false,
        thin: None,
        merge: true,
    }));
    let mut m = Model::new(doc);
    no_errors(&m);
    let cyl = PI * 400.0 * 30.0;
    assert_close(volume(&m), cyl, 1e-3);

    // Shell 2 mm, removing the top face (+Y).
    let top = face_with_normal(&m, DVec3::Y);
    m.doc.add(FeatureKind::Shell(ShellDef {
        faces: vec![top],
        thickness: 2.0,
        outward: false,
    }));
    m.rebuild();
    no_errors(&m);
    let inner = PI * 18.0 * 18.0 * 28.0;
    assert_close(volume(&m), cyl - inner, 1.0);

    // Chamfer the outer bottom edge 1 mm.
    let bottom_edges = edges_of(&m, DVec3::NEG_Y);
    m.doc.add(FeatureKind::Chamfer(Chamfer {
        edges: bottom_edges,
        faces: vec![],
        mode: ChamferMode::Equal { distance: 1.0 },
        flip: false,
    }));
    m.rebuild();
    no_errors(&m);
    // A 1x1 triangle swept around the 20 mm radius (approximately).
    assert_close(
        volume(&m),
        cyl - inner - 0.5 * TAU * (20.0 - 1.0 / 3.0),
        1.0,
    );
}

#[test]
fn hole_wizard_and_mirror() {
    let (mut m, _, _) = block();
    let top = face_with_normal(&m, DVec3::Z);
    let mut sk = Sketch::new();
    sk.add_point(DVec2::new(10.0, 10.0));
    sk.add_point(DVec2::new(30.0, 10.0));
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: top,
        sketch: sk,
    }));
    let h = Hole {
        sketch: s,
        kind: HoleKind::Counterbore,
        standard: HoleStandard::Iso,
        size: "M6".into(),
        fit: HoleFit::Normal,
        end: HoleEnd::ThroughAll,
        reverse: false,
        diameter_override: None,
        cbore_diameter_override: None,
        cbore_depth_override: None,
        csink_diameter_override: None,
        csink_angle_override: None,
    };
    let dims = hole_dims(&h).unwrap();
    assert_eq!(dims.diameter, 6.6);
    m.doc.add(FeatureKind::Hole(h));
    m.rebuild();
    no_errors(&m);
    let one = PI * 3.3 * 3.3 * (10.0 - 6.4) + PI * 5.5 * 5.5 * 6.4;
    assert_close(volume(&m), 8000.0 - 2.0 * one, 0.05);

    // Mirror the whole body about the right plane: twice the volume.
    m.doc.add(FeatureKind::Mirror(MirrorDef {
        features: vec![],
        plane: Ref::RightPlane,
    }));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), 2.0 * (8000.0 - 2.0 * one), 0.1);
}

#[test]
fn up_to_conditions_planes_and_thin() {
    let (mut m, _, _) = block();
    // A plane 30 mm in front; extrude a small square up to it from the top face.
    let p = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 30.0,
        flip: false,
    }));
    let top = face_with_normal(&m, DVec3::Z);
    let s = rect_sketch(
        &mut m.doc,
        top.clone(),
        DVec2::new(0.0, 0.0),
        DVec2::new(5.0, 5.0),
    );
    let mut e = blind(s, 0.0);
    e.end1 = EndCondition::UpToSurface {
        target: Ref::Feature { id: p },
    };
    m.doc.add(FeatureKind::Extrude(e));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), 8000.0 + 25.0 * 20.0, 1e-3);
    let bb = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    assert_close(bb.max.z, 30.0, 1e-6);

    // Thin open L-shaped wall, 1 mm, extruded 5 mm from the back of the block.
    let mut sk = Sketch::new();
    let (_, _, b) = sk.add_line_at(DVec2::new(50.0, 0.0), DVec2::new(60.0, 0.0));
    let c = sk.add_point(DVec2::new(60.0, 10.0));
    sk.add_line(b, c).unwrap();
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let mut e = blind(s, 5.0);
    e.thin = Some(Thin {
        kind: ThinKind::MidPlane,
        thickness: 1.0,
        thickness2: 0.0,
        reverse: false,
    });
    e.merge = false;
    m.doc.add(FeatureKind::Extrude(e));
    m.rebuild();
    no_errors(&m);
    // Mid-plane wall: 20 mm of centreline minus overlap at the outer corner.
    assert_close(volume(&m) - (8000.0 + 500.0), 20.0 * 1.0 * 5.0, 0.6);
}

#[test]
fn up_to_next_and_vertex() {
    let (mut m, _, _) = block();
    // A sketch on a plane 50 mm in front, extruded back toward the block up to next.
    let p = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 50.0,
        flip: false,
    }));
    let s = rect_sketch(
        &mut m.doc,
        Ref::Feature { id: p },
        DVec2::new(1.0, 1.0),
        DVec2::new(4.0, 4.0),
    );
    let mut e = blind(s, 0.0);
    e.end1 = EndCondition::UpToNext;
    e.reverse = true;
    m.doc.add(FeatureKind::Extrude(e));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), 8000.0 + 9.0 * 40.0, 1e-3);

    let (mut m, _, _) = block();
    let top = face_with_normal(&m, DVec3::Z);
    let corner = {
        let state = m.state();
        let body = state.body.as_ref().unwrap();
        let i = (0..body.shape.count(Kind::Vertex))
            .find(|i| {
                (body.shape.vertex_point(*i).unwrap() - DVec3::new(40.0, 20.0, 0.0)).length() < 1e-9
            })
            .unwrap();
        state.ref_for(Kind::Vertex, i).unwrap()
    };
    let s = rect_sketch(&mut m.doc, top, DVec2::new(0.0, 0.0), DVec2::new(2.0, 2.0));
    let mut e = blind(s, 0.0);
    e.cut = true;
    e.reverse = true;
    e.end1 = EndCondition::UpToVertex { target: corner };
    m.doc.add(FeatureKind::Extrude(e));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), 8000.0 - 40.0, 1e-3);
}

#[test]
fn circular_pattern_and_reference_axis() {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::ZERO, 30.0);
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(blind(s, 5.0)));
    let axis = doc.add(FeatureKind::Axis(AxisDef::TwoPlanes {
        a: Ref::TopPlane,
        b: Ref::RightPlane,
    }));
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::new(20.0, 0.0), 3.0);
    let s2 = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let mut cut = blind(s2, 0.0);
    cut.cut = true;
    cut.end1 = EndCondition::ThroughAll;
    cut.end2 = Some(EndCondition::ThroughAll);
    let c = doc.add(FeatureKind::Extrude(cut));
    doc.add(FeatureKind::CircularPattern(CircularPattern {
        features: vec![c],
        axis: Ref::Feature { id: axis },
        count: 6,
        angle: TAU,
        equal_spacing: true,
        reverse: false,
    }));
    let m = Model::new(doc);
    no_errors(&m);
    assert_close(volume(&m), PI * 900.0 * 5.0 - 6.0 * PI * 9.0 * 5.0, 1e-3);
}

#[test]
fn rollback_file_round_trip_and_mass() {
    let (mut m, _, e) = block();
    let edges = edges_of(&m, DVec3::Z);
    m.doc.add(FeatureKind::Chamfer(Chamfer {
        edges,
        faces: vec![],
        mode: ChamferMode::Equal { distance: 1.0 },
        flip: false,
    }));
    m.rebuild();
    let v_full = volume(&m);
    m.doc.rollback = Some(m.doc.index_of(e).unwrap() + 1);
    m.rebuild();
    assert_close(volume(&m), 8000.0, 1e-6);
    assert!(m.statuses().last().unwrap().rolled_back);
    m.doc.rollback = None;
    m.rebuild();
    assert_close(volume(&m), v_full, 1e-6);

    m.doc.material = materials::by_name("Aluminum 6061");
    let text = file::to_string(&m.doc);
    let doc2 = file::from_str(&text).unwrap();
    assert_eq!(doc2, m.doc);
    let m2 = Model::new(doc2);
    assert_close(volume(&m2), v_full, 1e-6);
    let mp = measure::mass_properties(&m2.doc, m2.state()).unwrap();
    assert_close(mp.mass, v_full * 2700.0 * 1e-9, 1e-9);
    assert_close(mp.center.x, 20.0, 1e-6);
}

#[test]
fn measure_edges_and_faces() {
    let (m, _, _) = block();
    let state = m.state();
    let top = face_with_normal(&m, DVec3::Z);
    let bottom = face_with_normal(&m, DVec3::NEG_Z);
    let r = measure::measure(state, &[top.clone()]).unwrap();
    assert_close(r.values[0].value, 800.0, 1e-6);
    let r = measure::measure(state, &[top, bottom]).unwrap();
    assert_close(r.values[0].value, 10.0, 1e-9);
    assert_close(
        r.values.iter().find(|v| v.label == "Angle").unwrap().value,
        0.0,
        1e-9,
    );
}

#[test]
fn step_import_and_convert_entities() {
    let (m, _, _) = block();
    let path = std::env::temp_dir().join("of-model-import-test.step");
    m.state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .write_step(path.to_str().unwrap())
        .unwrap();
    let data = std::fs::read(&path).unwrap();
    use base64::Engine;
    let mut doc = Document::new();
    doc.add(FeatureKind::Import(Import {
        format: ImportFormat::Step,
        source: "block.step".into(),
        data: base64::engine::general_purpose::STANDARD.encode(data),
    }));
    let mut m2 = Model::new(doc);
    no_errors(&m2);
    assert_close(volume(&m2), 8000.0, 1e-6);

    // Convert the top face's edges into a sketch on that face and extrude it.
    let top = face_with_normal(&m2, DVec3::Z);
    let edges = edges_of(&m2, DVec3::Z);
    let plane = m2.resolve(&top).unwrap().plane().unwrap();
    let mut sk = Sketch::new();
    convert_edges(m2.state(), &plane, &mut sk, &edges).unwrap();
    assert_eq!(sk.contours().unwrap().regions.len(), 1);
    let s = m2.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: top,
        sketch: sk,
    }));
    m2.doc.add(FeatureKind::Extrude(blind(s, 5.0)));
    m2.rebuild();
    no_errors(&m2);
    assert_close(volume(&m2), 12000.0, 1e-3);
}

#[test]
fn sketch_relations_drive_geometry() {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    let r = tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(10.0, 10.0));
    sk.add_constraint(K::Coincident {
        a: r.entities[0],
        b: of_model::sketch::ORIGIN,
    })
    .unwrap();
    let w = sk
        .add_constraint(K::Length {
            line: r.entities[4],
            value: 30.0,
        })
        .unwrap();
    sk.add_constraint(K::Length {
        line: r.entities[5],
        value: 15.0,
    })
    .unwrap();
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(blind(s, 2.0)));
    let mut m = Model::new(doc);
    no_errors(&m);
    assert_eq!(m.status(s).unwrap().sketch.unwrap().dof, 0);
    assert_close(volume(&m), 900.0, 1e-6);
    // Change the dimension and rebuild.
    let FeatureKind::Sketch(sf) = &mut m.doc.feature_mut(s).unwrap().kind else {
        unreachable!()
    };
    sf.sketch.set_value(w, 50.0).unwrap();
    m.invalidate(s);
    m.rebuild();
    assert_close(volume(&m), 1500.0, 1e-6);
}
