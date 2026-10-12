use std::f64::consts::{PI, TAU};

use of_geom::{DVec2, DVec3, Kind};
use of_model::sketch::{ConstraintKind as K, Sketch, tools};
use of_model::*;

fn reference_revolve(angle: RevolveAngle, reverse: bool) -> (Model, FeatureId) {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(10.0, 0.0), DVec2::new(20.0, 5.0));
    let (axis, _, _) = sk.add_line_at(DVec2::ZERO, DVec2::Y);
    sk.entities.get_mut(&axis).unwrap().construction = true;
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let r = doc.add(FeatureKind::Revolve(Revolve {
        end2: None,
        sketch: s,
        axis: Ref::SketchEntity {
            sketch: s,
            entity: axis,
        },
        angle,
        reverse,
        cut: false,
        thin: None,
        merge: true,
    }));
    (Model::new(doc), r)
}

#[test]
fn revolve_to_surface_and_offset() {
    for reverse in [false, true] {
        let (m, _) = reference_revolve(
            RevolveAngle::UpToSurface {
                target: Ref::RightPlane,
            },
            reverse,
        );
        no_errors(&m);
        assert_close(volume(&m), PI * 300.0 * 5.0 / 4.0, 1e-4);
        let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
        if reverse {
            assert!(b.min.z > -1e-5);
        } else {
            assert!(b.max.z < 1e-5);
        }
    }
    for reverse_offset in [false, true] {
        let (m, _) = reference_revolve(
            RevolveAngle::OffsetFromSurface {
                target: Ref::RightPlane,
                offset: 2.0,
                reverse_offset,
            },
            false,
        );
        no_errors(&m);
        let x = if reverse_offset { -2.0 } else { 2.0 };
        let area = |r: f64| (r * r * (x / r).acos() - x * (r * r - x * x).sqrt()) / 2.0;
        assert_close(volume(&m), (area(20.0) - area(10.0)) * 5.0, 1e-3);
    }
}

#[test]
fn revolve_to_point_rebuilds_and_rejects_axis_target() {
    let (mut m, r) = reference_revolve(RevolveAngle::Blind { angle: PI }, false);
    // Put the reference point before the revolve in the tree.
    let p = m.doc.add(FeatureKind::Point(PointDef::Coordinates {
        x: 0.0,
        y: 0.0,
        z: -30.0,
    }));
    m.doc.reorder(p, 1).unwrap();
    if let FeatureKind::Revolve(rev) = &mut m.doc.feature_mut(r).unwrap().kind {
        rev.angle = RevolveAngle::UpToVertex {
            target: Ref::Feature { id: p },
        };
    }
    assert!(m.doc.feature(r).unwrap().kind.depends_on().contains(&p));
    m.rebuild_all();
    no_errors(&m);
    assert_close(volume(&m), PI * 300.0 * 5.0 / 4.0, 1e-4);
    m.doc.feature_mut(p).unwrap().kind = FeatureKind::Point(PointDef::Coordinates {
        x: 0.0,
        y: 0.0,
        z: 30.0,
    });
    m.rebuild_all();
    no_errors(&m);
    assert_close(volume(&m), 3.0 * PI * 300.0 * 5.0 / 4.0, 1e-4);
    m.doc.feature_mut(p).unwrap().kind = FeatureKind::Point(PointDef::Coordinates {
        x: 0.0,
        y: 2.0,
        z: 0.0,
    });
    m.rebuild_all();
    assert!(m.statuses().iter().any(|s| s.id == r && s.error.is_some()));
}

#[test]
fn extrude_offset_from_cylinder_stops_at_near_side() {
    for (offset, reverse_offset, radius) in
        [(0.0, false, 20.0), (2.0, false, 22.0), (2.0, true, 18.0)]
    {
        let (mut m, _) = reference_revolve(RevolveAngle::Blind { angle: TAU }, false);
        let original = volume(&m);
        let body = m.state().body.as_ref().unwrap();
        let face = (0..body.shape.count(Kind::Face))
            .find(|&i| {
                let info = body.shape.face_info(i).unwrap();
                info.kind == of_geom::SurfaceKind::Cylinder && (info.radius - 20.0).abs() < 1e-6
            })
            .unwrap();
        let target = m.state().ref_for(Kind::Face, face).unwrap();
        let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
            reference: Ref::FrontPlane,
            distance: -30.0,
            flip: false,
        }));
        let sketch = rect_sketch(
            &mut m.doc,
            Ref::Feature { id: plane },
            DVec2::new(-2.0, 1.0),
            DVec2::new(2.0, 4.0),
        );
        let mut e = blind(sketch, 1.0);
        e.merge = false;
        e.end1 = EndCondition::OffsetFromSurface {
            target,
            offset,
            reverse_offset,
        };
        m.doc.add(FeatureKind::Extrude(e));
        m.rebuild();
        no_errors(&m);
        // Integrate sqrt(radius² - x²) between x=-2 and x=2.
        let under_curve =
            2.0 * (radius * radius - 4.0_f64).sqrt() + radius * radius * (2.0 / radius).asin();
        assert_close(volume(&m) - original, 3.0 * (120.0 - under_curve), 1e-3);
        assert_eq!(m.state().body.as_ref().unwrap().shape.count(Kind::Solid), 2);
    }
}

#[test]
fn revolve_to_curved_surface_and_offset() {
    for offset in [0.0, 2.0] {
        let (mut m, r) = reference_revolve(RevolveAngle::Blind { angle: PI }, false);
        let mut circle = Sketch::new();
        circle.add_circle_at(DVec2::new(0.0, 30.0), 25.0);
        let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
            plane: Ref::TopPlane,
            sketch: circle,
        }));
        m.doc.reorder(s, 0).unwrap();
        let e = m.doc.add(FeatureKind::Extrude(blind(s, 5.0)));
        m.doc.reorder(e, 1).unwrap();
        m.doc.rollback = Some(2);
        m.rebuild_all();
        no_errors(&m);
        let original = volume(&m);
        let shape = &m.state().body.as_ref().unwrap().shape;
        let i = (0..shape.count(Kind::Face))
            .find(|&i| shape.face_info(i).unwrap().kind == of_geom::SurfaceKind::Cylinder)
            .unwrap();
        let target = m.state().ref_for(Kind::Face, i).unwrap();
        if let FeatureKind::Revolve(rev) = &mut m.doc.feature_mut(r).unwrap().kind {
            rev.angle = RevolveAngle::OffsetFromSurface {
                target,
                offset,
                reverse_offset: false,
            };
        }
        m.doc.rollback = None;
        m.rebuild_all();
        no_errors(&m);
        // Independent Simpson integration in polar coordinates. At each
        // radius, the terminating circle gives sin(theta) by the cosine rule.
        let target_radius = 25.0 + offset;
        let f = |radius: f64| {
            radius
                * ((radius * radius + 900.0 - target_radius * target_radius) / (60.0 * radius))
                    .asin()
        };
        let mut integral = f(10.0) + f(20.0);
        for k in 1..1000 {
            integral += if k % 2 == 0 { 2.0 } else { 4.0 } * f(10.0 + k as f64 * 0.01);
        }
        assert_close(volume(&m) - original, integral * 0.01 / 3.0 * 5.0, 1e-3);
    }
}

#[test]
fn revolve_surface_rejects_nonterminating_target_and_round_trips() {
    let (m, r) = reference_revolve(
        RevolveAngle::UpToSurface {
            target: Ref::TopPlane,
        },
        false,
    );
    assert!(m.statuses().iter().any(|s| s.id == r && s.error.is_some()));
    let (m, _) = reference_revolve(
        RevolveAngle::OffsetFromSurface {
            target: Ref::RightPlane,
            offset: 2.0,
            reverse_offset: true,
        },
        false,
    );
    let json = serde_json::to_string(&m.doc).unwrap();
    let restored = Model::new(serde_json::from_str(&json).unwrap());
    no_errors(&restored);
    assert_close(volume(&m), volume(&restored), 1e-6);
}

#[test]
fn revolve_surface_fillet_survives_offset_edit() {
    let (mut m, r) = reference_revolve(
        RevolveAngle::OffsetFromSurface {
            target: Ref::RightPlane,
            offset: 2.0,
            reverse_offset: false,
        },
        false,
    );
    no_errors(&m);
    let end = face_with_normal(&m, DVec3::NEG_X);
    let edges = edges_of(&m, DVec3::NEG_X);
    m.doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![edges[0].clone()],
        faces: vec![],
        radius: 0.2,
    }));
    m.rebuild();
    no_errors(&m);
    if let FeatureKind::Revolve(rev) = &mut m.doc.feature_mut(r).unwrap().kind {
        rev.angle = RevolveAngle::OffsetFromSurface {
            target: Ref::RightPlane,
            offset: 3.0,
            reverse_offset: false,
        };
    }
    m.invalidate(r);
    m.rebuild();
    no_errors(&m);
    let plane = resolve(m.state(), &end).unwrap().plane().unwrap();
    assert_close(plane.origin.x, 3.0, 1e-6);
}

#[test]
fn reference_termination_works_for_thin_revolves_and_cuts() {
    let (mut m, r) = reference_revolve(
        RevolveAngle::UpToSurface {
            target: Ref::RightPlane,
        },
        false,
    );
    if let FeatureKind::Revolve(rev) = &mut m.doc.feature_mut(r).unwrap().kind {
        rev.thin = Some(Thin {
            kind: ThinKind::MidPlane,
            thickness: 1.0,
            thickness2: 0.0,
            reverse: false,
        });
    }
    m.invalidate(r);
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), PI / 4.0 * (330.0 * 6.0 - 270.0 * 4.0), 1e-3);

    let (mut m, r) = reference_revolve(RevolveAngle::Blind { angle: TAU }, false);
    let original = volume(&m);
    let FeatureKind::Revolve(mut cut) = m.doc.feature(r).unwrap().kind.clone() else {
        unreachable!()
    };
    cut.cut = true;
    cut.angle = RevolveAngle::UpToSurface {
        target: Ref::RightPlane,
    };
    m.doc.add(FeatureKind::Revolve(cut));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), original * 0.75, 1e-4);
}

#[test]
fn revolve_surface_must_terminate_every_contour() {
    let mut doc = Document::new();
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::RightPlane,
        distance: 15.0,
        flip: false,
    }));
    let mut sk = Sketch::new();
    for x in [10.0, 18.0] {
        tools::corner_rectangle(&mut sk, DVec2::new(x, 0.0), DVec2::new(x + 2.0, 5.0));
    }
    let (axis, _, _) = sk.add_line_at(DVec2::ZERO, DVec2::Y);
    sk.entities.get_mut(&axis).unwrap().construction = true;
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let r = doc.add(FeatureKind::Revolve(Revolve {
        end2: None,
        sketch: s,
        axis: Ref::SketchEntity {
            sketch: s,
            entity: axis,
        },
        angle: RevolveAngle::UpToSurface {
            target: Ref::Feature { id: plane },
        },
        reverse: false,
        cut: false,
        thin: None,
        merge: true,
    }));
    let m = Model::new(doc);
    assert!(
        m.statuses().iter().any(|s| {
            s.id == r
                && s.error
                    .as_deref()
                    .is_some_and(|e| e.contains("every sketch contour"))
        }),
        "{:?}",
        m.statuses()
    );
}

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
        end2: None,
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
        cosmetic_thread: false,
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
        keep_original: true,
    }));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), 2.0 * (8000.0 - 2.0 * one), 0.1);

    // Replacing the body with its mirror image keeps the volume and moves
    // the material to the other side.
    let before = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    let id = m.doc.add(FeatureKind::Mirror(MirrorDef {
        features: vec![],
        plane: Ref::FrontPlane,
        keep_original: false,
    }));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), 2.0 * (8000.0 - 2.0 * one), 0.1);
    let after = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    assert!((after.max.z + before.min.z).abs() < 1e-6);
    assert!((after.min.z + before.max.z).abs() < 1e-6);
    m.doc.delete(&[id]);
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
    let r = measure::measure(state, std::slice::from_ref(&top)).unwrap();
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

#[test]
fn independent_revolve_directions_round_trip_and_rebuild() {
    let (mut m, r) = reference_revolve(
        RevolveAngle::UpToSurface {
            target: Ref::RightPlane,
        },
        false,
    );
    let p = m.doc.add(FeatureKind::Point(PointDef::Coordinates {
        x: 20.0,
        y: 0.0,
        z: 20.0,
    }));
    m.doc.reorder(p, 1).unwrap();
    if let FeatureKind::Revolve(rev) = &mut m.doc.feature_mut(r).unwrap().kind {
        rev.end2 = Some(RevolveAngle::UpToVertex {
            target: Ref::Feature { id: p },
        });
    }
    assert!(m.doc.feature(r).unwrap().kind.depends_on().contains(&p));
    m.rebuild_all();
    no_errors(&m);
    assert_close(volume(&m), PI * 300.0 * 5.0 * 3.0 / 8.0, 1e-4);
    let mut m = Model::new(file::from_str(&file::to_string(&m.doc)).unwrap());
    no_errors(&m);
    m.doc.feature_mut(p).unwrap().kind = FeatureKind::Point(PointDef::Coordinates {
        x: 0.0,
        y: 0.0,
        z: 20.0,
    });
    m.invalidate(p);
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), PI * 300.0 * 5.0 / 2.0, 1e-4);
}

#[test]
fn dressups_repeat_with_their_base_features() {
    for mode in 0..3 {
        let (mut m, _, base) = block();
        let top = face_with_normal(&m, DVec3::Z);
        let edges = edges_of(&m, DVec3::Z);
        let dressup = m.doc.add(match mode {
            0 => FeatureKind::Fillet(Fillet {
                edges,
                faces: vec![],
                radius: 1.0,
            }),
            1 => FeatureKind::Chamfer(Chamfer {
                edges,
                faces: vec![],
                mode: ChamferMode::Equal { distance: 1.0 },
                flip: false,
            }),
            _ => FeatureKind::Shell(ShellDef {
                faces: vec![top],
                thickness: 1.0,
                outward: false,
            }),
        });
        m.rebuild();
        no_errors(&m);
        let original = volume(&m);
        m.doc.add(FeatureKind::LinearPattern(LinearPattern {
            features: vec![dressup, base], // Selection order must not change feature semantics.
            direction1: PatternDirection {
                reference: Ref::RightPlane,
                spacing: 50.0,
                count: 3,
                reverse: false,
            },
            direction2: None,
        }));
        m.rebuild();
        no_errors(&m);
        assert_close(volume(&m), 3.0 * original, 0.01);
        m.doc.add(FeatureKind::Mirror(MirrorDef {
            features: vec![dressup, base],
            plane: Ref::RightPlane,
            keep_original: true,
        }));
        m.rebuild();
        no_errors(&m);
        assert_close(volume(&m), 4.0 * original, 0.01);
        let loaded = Model::new(file::from_str(&file::to_string(&m.doc)).unwrap());
        no_errors(&loaded);
        assert_close(volume(&loaded), volume(&m), 1e-6);
    }
}

#[test]
fn spline_profile_matches_sketch_area_and_trimmed_curves_extrude() {
    let mut sk = Sketch::new();
    let points =
        [DVec2::ZERO, DVec2::new(10.0, 5.0), DVec2::new(20.0, 0.0)].map(|p| sk.add_point(p));
    let spline = sk.add_spline(points.to_vec(), false).unwrap();
    let samples = sk.curve(spline).unwrap().sample(1e-5);
    let area = samples
        .windows(2)
        .map(|p| (p[1].x - p[0].x) * (p[0].y + p[1].y) * 0.5)
        .sum::<f64>();
    sk.add_line(points[2], points[0]).unwrap();
    let mut doc = Document::new();
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk.clone(),
    }));
    doc.add(FeatureKind::Extrude(blind(s, 3.0)));
    let m = Model::new(doc);
    no_errors(&m);
    assert_close(volume(&m), area * 3.0, 0.001);
    // Retain the left side of the spline and close it with a vertical line.
    let (boundary, _, _) = sk.add_line_at(DVec2::new(10.0, -5.0), DVec2::new(10.0, 10.0));
    sk.entities.get_mut(&boundary).unwrap().construction = true;
    tools::trim(&mut sk, spline, DVec2::new(17.0, 2.0)).unwrap();
    let bottom = sk
        .curve_ids()
        .find(|id| {
            matches!(sk.entities[id].geo, of_model::sketch::Geometry::Line { .. })
                && *id != boundary
        })
        .unwrap();
    tools::trim(&mut sk, bottom, DVec2::new(17.0, 0.0)).unwrap();
    sk.add_line_at(DVec2::new(10.0, 0.0), DVec2::new(10.0, 5.0));
    let mut doc = Document::new();
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(blind(s, 3.0)));
    let m = Model::new(doc);
    no_errors(&m);
    assert_close(volume(&m), area * 1.5, 0.001);
}

#[test]
fn extrude_to_freeform_surface_and_normal_offset() {
    use of_model::sketch::{Curve2, Entity, Geometry};
    let control = [
        DVec2::new(0.0, -5.0),
        DVec2::new(6.0, -5.0),
        DVec2::new(14.0, -9.0),
        DVec2::new(20.0, -9.0),
    ];
    let curve = Curve2::Bezier { points: control };
    for offset in [0.0, 1.0] {
        let mut sk = Sketch::new();
        let points = control.map(|p| sk.add_point(p));
        sk.add_entity(Entity::new(Geometry::Bezier { points }))
            .unwrap();
        let a = sk.add_point(DVec2::new(20.0, -15.0));
        let b = sk.add_point(DVec2::new(0.0, -15.0));
        sk.add_line(points[3], a).unwrap();
        sk.add_line(a, b).unwrap();
        sk.add_line(b, points[0]).unwrap();
        let mut doc = Document::new();
        let s = doc.add(FeatureKind::Sketch(SketchFeature {
            plane: Ref::TopPlane,
            sketch: sk,
        }));
        doc.add(FeatureKind::Extrude(blind(s, 10.0)));
        let mut m = Model::new(doc);
        no_errors(&m);
        let original = volume(&m);
        let shape = &m.state().body.as_ref().unwrap().shape;
        let face = (0..shape.count(Kind::Face))
            .find(|&i| shape.face_info(i).unwrap().kind == of_geom::SurfaceKind::Other)
            .unwrap();
        let target = m.state().ref_for(Kind::Face, face).unwrap();
        let s = rect_sketch(
            &mut m.doc,
            Ref::FrontPlane,
            DVec2::new(2.0, 2.0),
            DVec2::new(18.0, 8.0),
        );
        let mut e = blind(s, 1.0);
        e.merge = false;
        e.end1 = EndCondition::OffsetFromSurface {
            target,
            offset,
            reverse_offset: false,
        };
        m.doc.add(FeatureKind::Extrude(e));
        m.rebuild();
        no_errors(&m);
        // Independently integrate the normal-offset boundary, clipped to the profile width.
        let samples: Vec<_> = (0..=10000)
            .map(|i| {
                let t = i as f64 / 10000.0;
                curve.point_at(t) + curve.tangent_at(t).normalize().perp() * offset
            })
            .collect();
        let area: f64 = samples
            .windows(2)
            .map(|p| {
                let lo = p[0].x.max(2.0);
                let hi = p[1].x.min(18.0);
                if hi <= lo {
                    return 0.0;
                }
                let y = |x| p[0].y + (p[1].y - p[0].y) * (x - p[0].x) / (p[1].x - p[0].x);
                -(hi - lo) * (y(lo) + y(hi)) * 0.5
            })
            .sum();
        assert_close(volume(&m) - original, area * 6.0, 0.01);
    }
}

#[test]
fn invalid_surface_offset_leaves_the_existing_body_intact() {
    let (mut m, _) = reference_revolve(RevolveAngle::Blind { angle: TAU }, false);
    let original = volume(&m);
    let shape = &m.state().body.as_ref().unwrap().shape;
    let face = (0..shape.count(Kind::Face))
        .find(|&i| {
            let info = shape.face_info(i).unwrap();
            info.kind == of_geom::SurfaceKind::Cylinder && (info.radius - 20.0).abs() < 1e-6
        })
        .unwrap();
    let target = m.state().ref_for(Kind::Face, face).unwrap();
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: -30.0,
        flip: false,
    }));
    let s = rect_sketch(
        &mut m.doc,
        Ref::Feature { id: plane },
        DVec2::new(-2.0, 1.0),
        DVec2::new(2.0, 4.0),
    );
    let mut e = blind(s, 1.0);
    e.end1 = EndCondition::OffsetFromSurface {
        target,
        offset: 20.0,
        reverse_offset: true,
    };
    let id = m.doc.add(FeatureKind::Extrude(e));
    m.rebuild();
    assert!(m.status(id).unwrap().error.is_some());
    assert_close(volume(&m), original, 1e-6);
}

#[test]
fn corrupt_sketch_file_is_rejected_before_rebuild() {
    let (mut m, s, _) = block();
    let FeatureKind::Sketch(sketch) = &mut m.doc.feature_mut(s).unwrap().kind else {
        unreachable!()
    };
    let line = sketch.sketch.curve_ids().next().unwrap();
    sketch.sketch.entities.get_mut(&line).unwrap().geo =
        of_model::sketch::Geometry::Line { a: 9999, b: 9998 };
    assert!(
        file::from_str(&file::to_string(&m.doc))
            .unwrap_err()
            .contains("invalid sketch")
    );
}

fn error_of(m: &Model, id: FeatureId) -> Option<String> {
    m.statuses().into_iter().find(|s| s.id == id)?.error
}

#[test]
fn cut_that_removes_no_material_fails() {
    // Misses the block entirely, then only touches its end face.
    for (p0, p1) in [
        (DVec2::new(100.0, 100.0), DVec2::new(110.0, 110.0)),
        (DVec2::new(40.0, 0.0), DVec2::new(50.0, 20.0)),
    ] {
        let (mut m, _, _) = block();
        let s = rect_sketch(&mut m.doc, Ref::FrontPlane, p0, p1);
        let mut cut = blind(s, 0.0);
        cut.cut = true;
        cut.end1 = EndCondition::ThroughAll;
        let id = m.doc.add(FeatureKind::Extrude(cut));
        m.rebuild();
        assert_eq!(
            error_of(&m, id).as_deref(),
            Some("the cut does not remove material")
        );
        assert_close(volume(&m), 8000.0, 1e-6);
    }
}

#[test]
fn patterned_cut_copies_off_the_part_are_skipped() {
    let (mut m, _, _) = block();
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::new(5.0, 5.0), 2.0);
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let mut cut = blind(s, 0.0);
    cut.cut = true;
    cut.end1 = EndCondition::ThroughAll;
    let c = m.doc.add(FeatureKind::Extrude(cut));
    // Six copies 10 mm apart: the last two fall past the 40 mm block.
    m.doc.add(FeatureKind::LinearPattern(LinearPattern {
        features: vec![c],
        direction1: PatternDirection {
            reference: Ref::RightPlane,
            spacing: 10.0,
            count: 6,
            reverse: false,
        },
        direction2: None,
    }));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), 8000.0 - 4.0 * PI * 4.0 * 10.0, 1e-3);
}

#[test]
fn convert_entities_accepts_faces_with_all_loops() {
    let (mut m, _, _) = block();
    let top = face_with_normal(&m, DVec3::Z);
    let plane = m.resolve(&top).unwrap().plane().unwrap();
    let mut sk = Sketch::new();
    // A face and one of its own edges convert each edge once.
    let mut refs = vec![top.clone()];
    refs.push(edges_of(&m, DVec3::Z)[0].clone());
    let created = convert_edges(m.state(), &plane, &mut sk, &refs).unwrap();
    assert_eq!(created.len(), 4);
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: top,
        sketch: sk,
    }));
    m.doc.add(FeatureKind::Extrude(blind(s, 5.0)));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), 12000.0, 1e-3);

    // A face with a hole converts the outer and inner loops.
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(40.0, 20.0));
    sk.add_circle_at(DVec2::new(20.0, 10.0), 5.0);
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(blind(s, 10.0)));
    let mut m = Model::new(doc);
    no_errors(&m);
    let holed = 8000.0 - PI * 250.0;
    assert_close(volume(&m), holed, 1e-3);
    let top = face_with_normal(&m, DVec3::Z);
    let plane = m.resolve(&top).unwrap().plane().unwrap();
    let mut sk = Sketch::new();
    let created = convert_edges(m.state(), &plane, &mut sk, std::slice::from_ref(&top)).unwrap();
    assert_eq!(created.len(), 5);
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: top,
        sketch: sk,
    }));
    m.doc.add(FeatureKind::Extrude(blind(s, 5.0)));
    m.rebuild();
    no_errors(&m);
    assert_close(volume(&m), holed * 1.5, 1e-3);
}

#[test]
fn tapped_holes_carry_cosmetic_thread_metadata() {
    let (mut m, _, _) = block();
    let top = face_with_normal(&m, DVec3::Z);
    let mut sk = Sketch::new();
    sk.add_point(DVec2::new(10.0, 10.0));
    sk.add_point(DVec2::new(30.0, 10.0));
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: top,
        sketch: sk,
    }));
    let mut h = Hole {
        sketch: s,
        kind: HoleKind::Tapped,
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
        cosmetic_thread: false,
    };
    let plain = m.doc.add(FeatureKind::Hole(h.clone()));
    m.rebuild();
    no_errors(&m);
    let mass = volume(&m);
    m.doc.delete(&[plain]);
    h.cosmetic_thread = true;
    let id = m.doc.add(FeatureKind::Hole(h.clone()));
    m.rebuild_all();
    no_errors(&m);
    // The annotation leaves the solid unchanged.
    assert_close(volume(&m), mass, 1e-9);
    let threads = &m.state().hole_threads[&id];
    assert_eq!(threads.len(), 2);
    for t in threads {
        assert_eq!(t.size, "M6");
        assert_close(t.diameter, 6.0, 1e-12);
        assert_close(t.pitch, 1.0, 1e-12);
        // Through the 10 mm block, measured along the drill.
        assert_close(t.length, 10.0, 1e-6);
        assert_close(t.axis.z, -1.0, 1e-12);
    }
    // ANSI sizes take the pitch from threads per inch.
    let size = holes::find(HoleStandard::Ansi, "1/4-20").unwrap();
    assert_close(holes::pitch(HoleStandard::Ansi, size).unwrap(), 1.27, 1e-12);
    // Only tapped holes carry threads.
    if let FeatureKind::Hole(hole) = &mut m.doc.feature_mut(id).unwrap().kind {
        hole.kind = HoleKind::Simple;
    }
    m.rebuild_all();
    assert!(m.status(id).unwrap().error.unwrap().contains("tapped"));
}

#[test]
fn closed_spline_extrusion_has_mass_properties() {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    let points = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]
        .map(|(x, y)| sk.add_point(DVec2::new(x, y)));
    sk.add_spline(points.to_vec(), true).unwrap();
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(blind(s, 1.0)));
    let m = Model::new(doc);
    no_errors(&m);
    // The periodic spline face once made volume integration fail outright.
    let v = volume(&m);
    assert!(v > 50.0 && v < 150.0, "{v}");
}

#[test]
fn shell_as_thick_as_half_the_part_fails() {
    let (mut m, _, _) = block();
    let id = m.doc.add(FeatureKind::Shell(ShellDef {
        faces: vec![],
        thickness: 5.0,
        outward: false,
    }));
    m.rebuild();
    let error = m.status(id).unwrap().error.unwrap();
    assert!(error.contains("too thick"), "{error}");
    assert_close(volume(&m), 8000.0, 1e-9);
}
