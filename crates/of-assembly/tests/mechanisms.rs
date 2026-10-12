//! Advanced and mechanical mates, dragging and collision detection.

mod common;

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use common::*;
use of_assembly::*;
use of_model::Ref;

fn ground() -> CompRef {
    CompRef {
        component: vec![],
        entity: Ref::FrontPlane,
    }
}

fn pose_of(a: &Assembly, key: &str) -> Pose {
    a.doc.component(key).unwrap().transform.into()
}

/// Turn a component about Z by `angle` from where it is, keeping position.
fn turn(a: &mut Assembly, key: &str, angle: f64) {
    let p = pose_of(a, key);
    let target = Pose::new(p.translation, DQuat::from_rotation_z(angle) * p.rotation);
    a.solve(&[(vec![key.into()], DragTarget::Pose(target))], false);
}

/// Spin angle of a component about Z (its X axis against the world's).
fn spin(a: &Assembly, key: &str) -> f64 {
    let x = pose_of(a, key).vector(DVec3::X);
    x.y.atan2(x.x)
}

/// A plate with two holes and two shafts standing in them.
fn two_shafts(dir: &Scratch, gap: f64) -> (Assembly, String, String, String) {
    let plate = plate_doc(
        200.0,
        60.0,
        10.0,
        &[(30.0, 30.0, 5.0), (30.0 + gap, 30.0, 5.0)],
    );
    let shaft = shaft_doc(5.0, 40.0);
    let pf = save(&plate, &dir.path("plate.ofp"));
    let sf = save(&shaft, &dir.path("shaft.ofp"));
    let mut a = Assembly::default();
    let p = a.insert(&pf, None, None).unwrap();
    let s1 = a
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(25.0, 25.0, 12.0))),
            None,
        )
        .unwrap();
    let s2 = a
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(25.0 + gap, 33.0, 15.0))),
            None,
        )
        .unwrap();
    let hole = |x: f64| {
        face(&plate, move |f| {
            f.kind == of_model::geom::SurfaceKind::Cylinder && (f.axis_origin.x - x).abs() < 1e-6
        })
    };
    for (s, x) in [(&s1, 30.0), (&s2, 30.0 + gap)] {
        a.add_mate(
            None,
            MateKind::Concentric {
                a: r(&p, hole(x)),
                b: r(s, cylinder_face(&shaft, 5.0)),
                alignment: Alignment::Closest,
                lock_rotation: false,
            },
        )
        .unwrap();
        a.add_mate(
            None,
            MateKind::Coincident {
                a: r(&p, plane_face(&plate, DVec3::Z, 10.0)),
                b: r(s, plane_face(&shaft, -DVec3::Z, 0.0)),
                alignment: Alignment::Closest,
            },
        )
        .unwrap();
    }
    (a, p, s1, s2)
}

#[test]
fn gear_mate_drives_the_second_shaft_at_the_ratio() {
    let dir = Scratch::new("gear");
    let (mut a, _, s1, s2) = two_shafts(&dir, 60.0);
    let shaft = shaft_doc(5.0, 40.0);
    a.add_mate(
        None,
        MateKind::Gear {
            a: r(&s1, cylinder_face(&shaft, 5.0)),
            b: r(&s2, cylinder_face(&shaft, 5.0)),
            ratio_a: 20.0,
            ratio_b: 40.0,
            reverse: false,
        },
    )
    .unwrap();
    assert_eq!(a.diagnostics.free, 1, "{:?}", a.diagnostics);
    let (b0, c0) = (spin(&a, &s1), spin(&a, &s2));
    // Over a full turn of the input in eighths, the output turns half a turn back.
    for _ in 0..8 {
        turn(&mut a, &s1, TAU / 8.0);
    }
    // Spins are measured about each cylinder's own axis direction.
    let m = &a.doc.mates.last().unwrap().captured.motions;
    assert!((m[0].abs() - TAU).abs() < 1e-3, "{m:?}");
    assert!((20.0 * m[0] + 40.0 * m[1]).abs() < 1e-3, "{m:?}");
    assert!((spin(&a, &s1) - b0).abs() < 1e-3);
    let c = spin(&a, &s2) - c0;
    assert!((c.abs() - PI).abs() < 1e-3, "{c}");
}

#[test]
fn screw_mate_advances_per_turn() {
    let dir = Scratch::new("screw");
    let block = save(
        &plate_doc(40.0, 40.0, 20.0, &[(20.0, 20.0, 5.0)]),
        &dir.path("nut.ofp"),
    );
    let shaft = shaft_doc(5.0, 80.0);
    let sf = save(&shaft, &dir.path("screw.ofp"));
    let nut = plate_doc(40.0, 40.0, 20.0, &[(20.0, 20.0, 5.0)]);
    let mut a = Assembly::default();
    let n = a.insert(&block, None, None).unwrap();
    let s = a
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(20.0, 20.0, -20.0))),
            None,
        )
        .unwrap();
    a.add_mate(
        None,
        MateKind::Screw {
            a: r(&n, cylinder_face(&nut, 5.0)),
            b: r(&s, cylinder_face(&shaft, 5.0)),
            distance_per_revolution: 2.0,
            reverse: false,
        },
    )
    .unwrap();
    assert_eq!(a.diagnostics.dof[&s], 1);
    let z0 = pose_of(&a, &s).translation.z;
    for _ in 0..12 {
        turn(&mut a, &s, TAU / 4.0);
    }
    let dz = pose_of(&a, &s).translation.z - z0;
    assert!((dz.abs() - 6.0).abs() < 1e-3, "three turns move 6 mm: {dz}");
}

#[test]
fn rack_and_pinion() {
    let dir = Scratch::new("rack");
    let (mut a, _, s1, _) = two_shafts(&dir, 60.0);
    let bar = block_doc(100.0, 10.0, 10.0);
    let bf = save(&bar, &dir.path("rack.ofp"));
    let k = a
        .insert(
            &bf,
            Some(Pose::from_translation(DVec3::new(0.0, 50.0, 30.0))),
            None,
        )
        .unwrap();
    // The rack slides along X on the plate's top face.
    for (n, f) in [(DVec3::Z, 0.0), (DVec3::Y, 0.0)] {
        a.add_mate(
            None,
            MateKind::Parallel {
                a: CompRef {
                    component: vec![],
                    entity: if n == DVec3::Z {
                        Ref::FrontPlane
                    } else {
                        Ref::TopPlane
                    },
                },
                b: r(&k, plane_face(&bar, -n, f)),
                alignment: Alignment::Closest,
            },
        )
        .unwrap();
    }
    let edge = edge_along_x(&bar);
    let shaft = shaft_doc(5.0, 40.0);
    a.add_mate(
        None,
        MateKind::RackPinion {
            rack: r(&k, edge),
            pinion: r(&s1, cylinder_face(&shaft, 5.0)),
            distance_per_revolution: 31.4,
            reverse: false,
        },
    )
    .unwrap();
    let x0 = pose_of(&a, &k).translation.x;
    for _ in 0..4 {
        turn(&mut a, &s1, TAU / 8.0);
    }
    let dx = pose_of(&a, &k).translation.x - x0;
    assert!(
        (dx.abs() - 15.7).abs() < 1e-2,
        "half a turn moves the rack 15.7: {dx}"
    );
}

fn edge_along_x(doc: &of_model::Document) -> Ref {
    let mut m = of_model::Model::new(doc.clone());
    m.rebuild();
    let st = m.state();
    let shape = &st.body.as_ref().unwrap().shape;
    for i in 0..shape.count(of_model::geom::Kind::Edge) {
        let e = shape.edge_info(i).unwrap();
        if e.kind == of_model::geom::CurveKind::Line
            && (e.end - e.start).normalize().x.abs() > 0.999
        {
            return st.ref_for(of_model::geom::Kind::Edge, i).unwrap();
        }
    }
    panic!("no X edge");
}

#[test]
fn slot_mate_keeps_a_pin_in_the_slot_and_limits_travel() {
    let dir = Scratch::new("slot");
    let plate = slot_plate_doc(100.0, 40.0, 10.0, (20.0, 20.0), (60.0, 20.0), 10.0);
    let pf = save(&plate, &dir.path("slotted.ofp"));
    let pin = shaft_doc(5.0, 30.0);
    let sf = save(&pin, &dir.path("pin.ofp"));
    let mut a = Assembly::default();
    let p = a.insert(&pf, None, None).unwrap();
    let s = a
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(30.0, 22.0, 10.0))),
            None,
        )
        .unwrap();
    let end = face(&plate, |f| {
        f.kind == of_model::geom::SurfaceKind::Cylinder && (f.axis_origin.x - 20.0).abs() < 1e-6
    });
    a.add_mate(
        None,
        MateKind::Slot {
            slot: r(&p, end.clone()),
            pin: r(&s, cylinder_face(&pin, 5.0)),
            constraint: TravelConstraint::Free,
        },
    )
    .unwrap();
    let t = pose_of(&a, &s).translation;
    assert!((t.y - 20.0).abs() < 1e-6, "on the center line: {t}");
    // Dragging past the end stops at the end of the slot.
    let target = Pose::from_translation(DVec3::new(90.0, 20.0, t.z));
    a.solve(&[(vec![s.clone()], DragTarget::Pose(target))], false);
    assert!((pose_of(&a, &s).translation.x - 60.0).abs() < 1e-4);
    // Centered.
    let id = a.doc.mates[0].id;
    a.update_mate(
        id,
        MateKind::Slot {
            slot: r(&p, end),
            pin: r(&s, cylinder_face(&pin, 5.0)),
            constraint: TravelConstraint::Center,
        },
    )
    .unwrap();
    assert!((pose_of(&a, &s).translation.x - 40.0).abs() < 1e-6);
}

#[test]
fn width_mate_centers_a_tab() {
    let dir = Scratch::new("width");
    // A U channel: a block with a 20 mm gap cut from x = 40 to 60.
    let mut channel = block_doc(100.0, 30.0, 30.0);
    let mut sk = of_model::sketch::Sketch::new();
    of_model::sketch::tools::corner_rectangle(
        &mut sk,
        of_model::geom::DVec2::new(40.0, -1.0),
        of_model::geom::DVec2::new(60.0, 31.0),
    );
    let s = channel.add(of_model::FeatureKind::Sketch(of_model::SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    channel.add(of_model::FeatureKind::Extrude(of_model::Extrude {
        sketch: s,
        cut: true,
        start_offset: 0.0,
        end1: of_model::EndCondition::ThroughAll,
        reverse: false,
        end2: Some(of_model::EndCondition::ThroughAll),
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let cf = save(&channel, &dir.path("channel.ofp"));
    let tab = block_doc(8.0, 30.0, 30.0);
    let tf = save(&tab, &dir.path("tab.ofp"));
    let mut a = Assembly::default();
    let c = a.insert(&cf, None, None).unwrap();
    let t = a
        .insert(
            &tf,
            Some(Pose::from_translation(DVec3::new(42.0, 0.0, 0.0))),
            None,
        )
        .unwrap();
    a.add_mate(
        None,
        MateKind::Width {
            width: [
                r(&c, plane_face(&channel, DVec3::X, 40.0)),
                r(&c, plane_face(&channel, -DVec3::X, -60.0)),
            ],
            tab: [
                r(&t, plane_face(&tab, -DVec3::X, 0.0)),
                r(&t, plane_face(&tab, DVec3::X, 8.0)),
            ],
            constraint: WidthConstraint::Centered,
        },
    )
    .unwrap();
    assert!(
        (pose_of(&a, &t).translation.x - 46.0).abs() < 1e-6,
        "{:?}",
        pose_of(&a, &t)
    );
}

#[test]
fn distance_limit_and_collision_stop_a_drag() {
    let dir = Scratch::new("collide");
    let base = block_doc(100.0, 40.0, 10.0);
    let wall = block_doc(10.0, 40.0, 40.0);
    let slider = block_doc(20.0, 40.0, 10.0);
    let bf = save(&base, &dir.path("base.ofp"));
    let wf = save(&wall, &dir.path("wall.ofp"));
    let sf = save(&slider, &dir.path("slider.ofp"));
    let mut a = Assembly::default();
    let b = a.insert(&bf, None, None).unwrap();
    let w = a
        .insert(
            &wf,
            Some(Pose::from_translation(DVec3::new(90.0, 0.0, 10.0))),
            None,
        )
        .unwrap();
    a.doc.component_mut(&w).unwrap().fixed = true;
    let s = a
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(10.0, 0.0, 10.0))),
            None,
        )
        .unwrap();
    // The slider sits on the base and slides along X only.
    a.add_mate(
        None,
        MateKind::Coincident {
            a: r(&b, plane_face(&base, DVec3::Z, 10.0)),
            b: r(&s, plane_face(&slider, -DVec3::Z, 0.0)),
            alignment: Alignment::Closest,
        },
    )
    .unwrap();
    a.add_mate(
        None,
        MateKind::Coincident {
            a: r(&b, plane_face(&base, -DVec3::Y, 0.0)),
            b: r(&s, plane_face(&slider, -DVec3::Y, 0.0)),
            alignment: Alignment::Closest,
        },
    )
    .unwrap();
    assert_eq!(a.diagnostics.dof[&s], 1);

    // Without collision detection the slider passes into the wall.
    let mut free = Assembly::new(a.doc.clone());
    free.drag(
        std::slice::from_ref(&s),
        DragTarget::Pose(Pose::from_translation(DVec3::new(80.0, 0.0, 10.0))),
        None,
    )
    .unwrap();
    assert!((pose_of(&free, &s).translation.x - 80.0).abs() < 1e-3);

    // With it, the slider stops against the wall face at x = 90.
    let out = a
        .drag(
            std::slice::from_ref(&s),
            DragTarget::Pose(Pose::from_translation(DVec3::new(80.0, 0.0, 10.0))),
            Some(analysis::CollisionOptions::default()),
        )
        .unwrap();
    let x = pose_of(&a, &s).translation.x;
    assert!(out.collision.is_some());
    assert!(x <= 70.0 + 1e-9 && x > 69.5, "stopped at the wall: {x}");
    assert!(out.faces.iter().all(|(_, f)| !f.is_empty()));

    // A distance limit keeps it at most 30 mm from the base's left end.
    let mut limited = Assembly::new(a.doc.clone());
    limited
        .add_mate(
            None,
            MateKind::Distance {
                a: r(&b, plane_face(&base, -DVec3::X, 0.0)),
                b: r(&s, plane_face(&slider, -DVec3::X, 0.0)),
                distance: 0.0,
                flip: false,
                limit: Some(Limit {
                    min: 0.0,
                    max: 30.0,
                }),
            },
        )
        .unwrap();
    limited.solve(
        &[(
            vec![s.clone()],
            DragTarget::Pose(Pose::from_translation(DVec3::new(60.0, 0.0, 10.0))),
        )],
        false,
    );
    assert!((pose_of(&limited, &s).translation.x - 30.0).abs() < 1e-4);
    // Fixed components can't be dragged.
    assert!(
        a.drag(&[b], DragTarget::Pose(Pose::IDENTITY), None)
            .is_err()
    );
}

#[test]
fn hinge_stops_at_its_angle_limit() {
    let dir = Scratch::new("hinge");
    let plate = plate_doc(100.0, 60.0, 10.0, &[(30.0, 30.0, 5.0)]);
    let lever = plate_doc(60.0, 10.0, 4.0, &[(5.0, 5.0, 5.0)]);
    let pf = save(&plate, &dir.path("plate.ofp"));
    let lf = save(&lever, &dir.path("lever.ofp"));
    let mut a = Assembly::default();
    let p = a.insert(&pf, None, None).unwrap();
    let l = a
        .insert(
            &lf,
            Some(Pose::from_translation(DVec3::new(25.0, 25.0, 12.0))),
            None,
        )
        .unwrap();
    a.add_mate(
        None,
        MateKind::Hinge {
            axis: [
                r(&p, cylinder_face(&plate, 5.0)),
                r(&l, cylinder_face(&lever, 5.0)),
            ],
            faces: [
                r(&p, plane_face(&plate, DVec3::Z, 10.0)),
                r(&l, plane_face(&lever, -DVec3::Z, 0.0)),
            ],
            limit: Some(HingeLimit {
                faces: [
                    r(&p, plane_face(&plate, -DVec3::Y, 0.0)),
                    r(&l, plane_face(&lever, -DVec3::Y, 0.0)),
                ],
                min: 0.0,
                max: FRAC_PI_2,
            }),
        },
    )
    .unwrap();
    assert_eq!(a.diagnostics.dof[&l], 1);
    for _ in 0..6 {
        turn(&mut a, &l, PI / 8.0);
    }
    let x = pose_of(&a, &l).vector(DVec3::X);
    let angle = x.y.atan2(x.x);
    assert!(
        (angle - FRAC_PI_2).abs() < 1e-3,
        "stopped at 90°: {}",
        angle.to_degrees()
    );
    let c = pose_of(&a, &l).point(DVec3::new(5.0, 5.0, 0.0));
    assert!((c - DVec3::new(30.0, 30.0, 10.0)).length() < 1e-6);
}

#[test]
fn universal_joint_couples_shafts_at_an_angle() {
    let dir = Scratch::new("ujoint");
    let bearing = plate_doc(40.0, 40.0, 10.0, &[(20.0, 20.0, 3.0)]);
    let shaft = shaft_doc(3.0, 50.0);
    let bf = save(&bearing, &dir.path("bearing.ofp"));
    let sf = save(&shaft, &dir.path("shaft.ofp"));
    let mut a = Assembly::default();
    let b1 = a
        .insert(
            &bf,
            Some(Pose::from_translation(DVec3::new(-20.0, -20.0, -30.0))),
            None,
        )
        .unwrap();
    let joint = DVec3::new(0.0, 0.0, 30.0);
    let tilt = DQuat::from_rotation_x(PI / 6.0);
    let b2_pose = Pose::new(
        joint + tilt * DVec3::Z * 20.0 - tilt * DVec3::new(20.0, 20.0, 0.0),
        tilt,
    );
    let b2 = a.insert(&bf, Some(b2_pose), None).unwrap();
    a.doc.component_mut(&b2).unwrap().fixed = true;
    let s1 = a
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(0.0, 0.0, -20.0))),
            None,
        )
        .unwrap();
    let s2 = a.insert(&sf, Some(Pose::new(joint, tilt)), None).unwrap();
    for (bearing_key, s) in [(&b1, &s1), (&b2, &s2)] {
        a.add_mate(
            None,
            MateKind::Concentric {
                a: r(bearing_key, cylinder_face(&bearing, 3.0)),
                b: r(s, cylinder_face(&shaft, 3.0)),
                alignment: Alignment::Closest,
                lock_rotation: false,
            },
        )
        .unwrap();
    }
    a.add_mate(
        None,
        MateKind::Coincident {
            a: r(&b1, plane_face(&bearing, DVec3::Z, 10.0)),
            b: r(&s1, plane_face(&shaft, -DVec3::Z, 0.0)),
            alignment: Alignment::Closest,
        },
    )
    .unwrap();
    a.add_mate(
        None,
        MateKind::UniversalJoint {
            a: r(&s1, cylinder_face(&shaft, 3.0)),
            b: r(&s2, cylinder_face(&shaft, 3.0)),
            joint_point: None,
        },
    )
    .unwrap();
    assert!(a.diagnostics.solved, "{:?}", a.diagnostics);
    assert_eq!(a.diagnostics.free, 1, "{:?}", a.diagnostics);
    let spin2 = |a: &Assembly| {
        let p = pose_of(a, &s2);
        let (x, axis) = (p.vector(DVec3::X), p.vector(DVec3::Z));
        let r0 = tilt * DVec3::X;
        let r1 = axis.cross(r0);
        x.dot(r1).atan2(x.dot(r0))
    };
    let start = spin2(&a);
    // A quarter turn in, a quarter turn out; at 45° the output lags or leads.
    turn(&mut a, &s1, PI / 8.0);
    turn(&mut a, &s1, PI / 8.0);
    let half = spin2(&a) - start;
    turn(&mut a, &s1, PI / 8.0);
    turn(&mut a, &s1, PI / 8.0);
    let quarter = spin2(&a) - start;
    assert!(
        (quarter.abs() - FRAC_PI_2).abs() < 1e-3,
        "{}",
        quarter.to_degrees()
    );
    let expected = (PI / 4.0).tan() * (PI / 6.0).cos();
    let alternative = (PI / 4.0).tan() / (PI / 6.0).cos();
    let t = half.abs().tan();
    assert!(
        (t - expected).abs() < 1e-3 || (t - alternative).abs() < 1e-3,
        "{}",
        half.to_degrees()
    );
}

#[test]
fn symmetric_profile_center_lock_and_linear_coupler() {
    let dir = Scratch::new("advanced");
    let plate = plate_doc(100.0, 60.0, 10.0, &[]);
    let block = block_doc(20.0, 20.0, 10.0);
    let pf = save(&plate, &dir.path("plate.ofp"));
    let bf = save(&block, &dir.path("block.ofp"));
    let mut a = Assembly::default();
    let p = a.insert(&pf, None, None).unwrap();
    let b1 = a
        .insert(
            &bf,
            Some(Pose::from_translation(DVec3::new(5.0, 3.0, 14.0))),
            None,
        )
        .unwrap();
    a.add_mate(
        None,
        MateKind::ProfileCenter {
            a: r(&p, plane_face(&plate, DVec3::Z, 10.0)),
            b: r(&b1, plane_face(&block, -DVec3::Z, 0.0)),
            offset: 0.0,
            flip: false,
            lock_rotation: false,
        },
    )
    .unwrap();
    let t = pose_of(&a, &b1).point(DVec3::new(10.0, 10.0, 0.0));
    assert!((t - DVec3::new(50.0, 30.0, 10.0)).length() < 1e-6, "{t}");
    assert_eq!(a.diagnostics.dof[&b1], 1, "spins about the shared normal");

    // A second block mirrors the first about the plate's YZ plane at x = 0.
    let b2 = a
        .insert(
            &bf,
            Some(Pose::from_translation(DVec3::new(-70.0, 20.0, 10.0))),
            None,
        )
        .unwrap();
    a.add_mate(
        None,
        MateKind::Symmetric {
            a: r(&b1, plane_face(&block, DVec3::X, 20.0)),
            b: r(&b2, plane_face(&block, -DVec3::X, 0.0)),
            plane: CompRef {
                component: vec![],
                entity: Ref::RightPlane,
            },
        },
    )
    .unwrap();
    // B1's right face, reflected in x = 0, lies on B2's left face.
    let mirror = |v: DVec3| DVec3::new(-v.x, v.y, v.z);
    let (p1, n1) = (
        pose_of(&a, &b1).point(DVec3::new(20.0, 10.0, 5.0)),
        pose_of(&a, &b1).vector(DVec3::X),
    );
    let (p2, n2) = (
        pose_of(&a, &b2).point(DVec3::ZERO),
        pose_of(&a, &b2).vector(-DVec3::X),
    );
    assert!((mirror(p1) - p2).dot(n2).abs() < 1e-6);
    assert!((mirror(n1) - n2).length() < 1e-6);

    // Lock a third block to the second, then move the second: it follows.
    let b3 = a
        .insert(
            &bf,
            Some(Pose::from_translation(DVec3::new(-70.0, 50.0, 10.0))),
            None,
        )
        .unwrap();
    a.add_mate(
        None,
        MateKind::Lock {
            a: vec![b2.clone()],
            b: vec![b3.clone()],
        },
    )
    .unwrap();
    let rel = pose_of(&a, &b2).inverse().then(&pose_of(&a, &b3));
    a.solve(
        &[(
            vec![b2.clone()],
            DragTarget::Pose(Pose::from_translation(DVec3::new(-80.0, 10.0, 30.0))),
        )],
        false,
    );
    let rel2 = pose_of(&a, &b2).inverse().then(&pose_of(&a, &b3));
    assert!(rel.translation.distance(rel2.translation) < 1e-6);
}

#[test]
fn linear_coupler_moves_the_second_slider_twice_as_far() {
    let dir = Scratch::new("coupler");
    let base = block_doc(200.0, 40.0, 10.0);
    let slider = block_doc(20.0, 40.0, 10.0);
    let bf = save(&base, &dir.path("base.ofp"));
    let sf = save(&slider, &dir.path("slider.ofp"));
    let mut a = Assembly::default();
    let b = a.insert(&bf, None, None).unwrap();
    let s1 = a
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(10.0, 0.0, 12.0))),
            None,
        )
        .unwrap();
    let s2 = a
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(100.0, 0.0, 13.0))),
            None,
        )
        .unwrap();
    for s in [&s1, &s2] {
        for (n, at) in [(DVec3::Z, 10.0), (-DVec3::Y, 0.0)] {
            a.add_mate(
                None,
                MateKind::Coincident {
                    a: r(&b, plane_face(&base, n, at)),
                    b: r(
                        s,
                        plane_face(&slider, if n == DVec3::Z { -DVec3::Z } else { n }, 0.0),
                    ),
                    alignment: Alignment::Closest,
                },
            )
            .unwrap();
        }
    }
    let x_edge = r(&s1, plane_face(&slider, DVec3::X, 20.0));
    a.add_mate(
        None,
        MateKind::LinearCoupler {
            a: x_edge,
            b: r(&s2, plane_face(&slider, DVec3::X, 20.0)),
            ratio_a: 1.0,
            ratio_b: 2.0,
            reverse: false,
        },
    )
    .unwrap();
    let (x1, x2) = (
        pose_of(&a, &s1).translation.x,
        pose_of(&a, &s2).translation.x,
    );
    a.solve(
        &[(
            vec![s1.clone()],
            DragTarget::Pose(Pose::from_translation(DVec3::new(x1 + 10.0, 0.0, 10.0))),
        )],
        false,
    );
    let d1 = pose_of(&a, &s1).translation.x - x1;
    let d2 = pose_of(&a, &s2).translation.x - x2;
    assert!(d1.abs() > 1.0, "{d1}");
    assert!((d2 - 2.0 * d1).abs() < 1e-4, "{d1} {d2}");
}

#[test]
fn path_and_cam_mates() {
    let dir = Scratch::new("path");
    let plate = plate_doc(100.0, 60.0, 10.0, &[]);
    let block = block_doc(10.0, 10.0, 10.0);
    let pf = save(&plate, &dir.path("plate.ofp"));
    let bf = save(&block, &dir.path("block.ofp"));
    let mut a = Assembly::default();
    let p = a.insert(&pf, None, None).unwrap();
    let k = a
        .insert(
            &bf,
            Some(Pose::from_translation(DVec3::new(10.0, 5.0, 15.0))),
            None,
        )
        .unwrap();
    // The block's origin corner follows the plate's top front edge, 25 mm in.
    let edge = {
        let mut m = of_model::Model::new(plate.clone());
        m.rebuild();
        let st = m.state();
        let shape = &st.body.as_ref().unwrap().shape;
        (0..shape.count(of_model::geom::Kind::Edge))
            .find_map(|i| {
                let e = shape.edge_info(i).unwrap();
                let mid = (e.start + e.end) * 0.5;
                (e.kind == of_model::geom::CurveKind::Line
                    && mid.y.abs() < 1e-9
                    && (mid.z - 10.0).abs() < 1e-9)
                    .then(|| st.ref_for(of_model::geom::Kind::Edge, i).unwrap())
            })
            .unwrap()
    };
    let corner = {
        let mut m = of_model::Model::new(block.clone());
        m.rebuild();
        let st = m.state();
        let shape = &st.body.as_ref().unwrap().shape;
        (0..shape.count(of_model::geom::Kind::Vertex))
            .find_map(|i| {
                (shape.vertex_point(i).unwrap().length() < 1e-9)
                    .then(|| st.ref_for(of_model::geom::Kind::Vertex, i).unwrap())
            })
            .unwrap()
    };
    a.add_mate(
        None,
        MateKind::Path {
            point: r(&k, corner),
            path: vec![r(&p, edge)],
            constraint: TravelConstraint::Distance {
                distance: 25.0,
                flip: false,
            },
            follow: None,
            roll: None,
        },
    )
    .unwrap();
    let c = pose_of(&a, &k).point(DVec3::ZERO);
    let along = (c.x - 25.0).abs().min((c.x - 75.0).abs());
    assert!(
        along < 1e-6 && c.y.abs() < 1e-6 && (c.z - 10.0).abs() < 1e-6,
        "{c}"
    );

    // Cam: a Ø10 follower stays tangent to a 40 mm square cam's profile.
    let dir = Scratch::new("cam");
    let cam = block_doc(40.0, 40.0, 10.0);
    let follower = shaft_doc(5.0, 10.0);
    let cf = save(&cam, &dir.path("cam.ofp"));
    let ff = save(&follower, &dir.path("follower.ofp"));
    let mut a = Assembly::default();
    let c = a.insert(&cf, None, None).unwrap();
    let f = a
        .insert(
            &ff,
            Some(Pose::from_translation(DVec3::new(52.0, 20.0, 0.0))),
            None,
        )
        .unwrap();
    a.add_mate(
        None,
        MateKind::Coincident {
            a: ground(),
            b: r(&f, plane_face(&follower, -DVec3::Z, 0.0)),
            alignment: Alignment::Closest,
        },
    )
    .unwrap();
    a.add_mate(
        None,
        MateKind::Cam {
            cam: r(&c, plane_face(&cam, DVec3::Z, 10.0)),
            follower: r(&f, cylinder_face(&follower, 5.0)),
        },
    )
    .unwrap();
    let t = pose_of(&a, &f).translation;
    assert!((t.x - 45.0).abs() < 1e-5, "{t}");
}
