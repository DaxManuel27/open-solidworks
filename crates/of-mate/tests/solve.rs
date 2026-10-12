//! Analytic checks for the mate solver.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use of_mate::*;

const EPS: f64 = 1e-6;

fn pose(t: [f64; 3], axis: DVec3, angle: f64) -> Pose {
    Pose::new(DVec3::from(t), DQuat::from_axis_angle(axis, angle))
}

/// Body 0 fixed at the origin, body 1 floating at `start`.
fn two(start: Pose, mates: Vec<Mate>) -> System {
    System {
        poses: vec![Pose::IDENTITY, start],
        fixed: vec![true, false],
        mates,
    }
}

fn plane(body: usize, origin: [f64; 3], normal: [f64; 3]) -> Plane {
    Plane::new(Some(body), DVec3::from(origin), DVec3::from(normal))
}

/// Coincident planar faces, anti-aligned (face to face).
fn coincident(id: u32, a: Plane, b: Plane) -> Mate {
    Mate {
        id,
        terms: vec![
            Term::Parallel {
                a: a.direction(),
                b: b.direction(),
                sign: Some(-1.0),
            },
            Term::Value {
                measure: Measure::PlaneDistance {
                    point: b.point(),
                    plane: a,
                },
                target: Target::Equal(0.0),
            },
        ],
    }
}

fn distance(id: u32, a: Plane, b: Plane, target: Target) -> Mate {
    Mate {
        id,
        terms: vec![
            Term::Parallel {
                a: a.direction(),
                b: b.direction(),
                sign: Some(-1.0),
            },
            Term::Value {
                measure: Measure::PlaneDistance {
                    point: b.point(),
                    plane: a,
                },
                target,
            },
        ],
    }
}

fn concentric(id: u32, a: Line, b: Line) -> Mate {
    Mate {
        id,
        terms: vec![
            Term::Parallel {
                a: a.direction(),
                b: b.direction(),
                sign: None,
            },
            Term::PointOnLine {
                point: b.point(),
                line: a,
            },
        ],
    }
}

#[test]
fn face_to_face_leaves_three_dof_and_lands_on_the_plane() {
    // A 10 mm cube's bottom face on the ground's top face, starting tilted.
    let start = pose([3.0, -2.0, 7.0], DVec3::new(1.0, 1.0, 0.0).normalize(), 0.4);
    let sys = two(
        start,
        vec![coincident(
            1,
            plane(0, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            plane(1, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        )],
    );
    let (poses, r) = solve(&sys, &[], true);
    assert!(r.solved, "{r:?}");
    assert_eq!(r.dof, vec![0, 3]);
    let p = poses[1];
    assert!(p.point(DVec3::ZERO).z.abs() < EPS);
    assert!((p.vector(DVec3::Z) - DVec3::Z).length() < EPS);
}

#[test]
fn three_planes_fully_define_a_block() {
    let start = pose([2.0, 3.0, 4.0], DVec3::Z, 0.3);
    let mates = vec![
        coincident(
            1,
            plane(0, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            plane(1, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ),
        coincident(
            2,
            plane(0, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
            plane(1, [0.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
        ),
        coincident(
            3,
            plane(0, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            plane(1, [0.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
        ),
    ];
    let (poses, r) = solve(&two(start, mates), &[], true);
    assert!(r.solved, "{r:?}");
    assert_eq!(r.dof, vec![0, 0]);
    assert!(poses[1].translation.length() < EPS, "{:?}", poses[1]);
    assert!(poses[1].rotation.angle_between(DQuat::IDENTITY) < EPS);
}

#[test]
fn concentric_shaft_slides_and_turns_until_rotation_is_locked() {
    let bore = Line::new(Some(0), DVec3::ZERO, DVec3::Z);
    let shaft = Line::new(Some(1), DVec3::ZERO, DVec3::Z);
    let start = pose([5.0, 1.0, 3.0], DVec3::X, 0.2);
    let (poses, r) = solve(&two(start, vec![concentric(1, bore, shaft)]), &[], true);
    assert!(r.solved);
    assert_eq!(r.dof[1], 2);
    let p = poses[1];
    assert!(p.translation.truncate().length() < EPS);
    assert!(p.vector(DVec3::Z).cross(DVec3::Z).length() < EPS);

    // Lock rotation: a vector on the shaft keeps its angle to the housing.
    let mut lock = concentric(1, bore, shaft);
    lock.terms.push(Term::Value {
        measure: Measure::SignedAngle {
            a: Dir::new(Some(0), DVec3::X),
            b: Dir::new(Some(1), DVec3::X),
            axis: Dir::new(Some(0), DVec3::Z),
        },
        target: Target::Equal(0.0),
    });
    let (_, r) = solve(&two(start, vec![lock]), &[], true);
    assert!(r.solved);
    assert_eq!(r.dof[1], 1);
}

#[test]
fn conflicting_distances_are_named_and_poses_kept() {
    let a = plane(0, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let b = plane(1, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
    let start = pose([0.0, 0.0, 15.0], DVec3::Z, 0.0);
    let base = coincident(
        1,
        plane(0, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        plane(1, [0.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
    );
    let mates = vec![
        base,
        distance(2, a, b, Target::Equal(10.0)),
        distance(3, a, b, Target::Equal(20.0)),
    ];
    let (poses, r) = solve(&two(start, mates), &[], true);
    assert!(!r.solved);
    assert_eq!(r.conflicting, vec![2, 3]);
    assert_eq!(poses[1], start);
}

#[test]
fn a_repeated_mate_is_redundant() {
    let a = plane(0, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let b = plane(1, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
    let (_, r) = solve(
        &two(
            Pose::IDENTITY,
            vec![coincident(1, a, b), coincident(2, a, b)],
        ),
        &[],
        true,
    );
    assert!(r.solved);
    assert_eq!(r.redundant, vec![2]);
}

#[test]
fn distance_limit_holds_while_dragging() {
    let a = plane(0, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let b = plane(1, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
    let side = coincident(
        1,
        plane(0, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        plane(1, [0.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
    );
    let front = coincident(
        2,
        plane(0, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        plane(1, [0.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
    );
    let limit = distance(
        3,
        a,
        b,
        Target::Range {
            min: 5.0,
            max: 25.0,
        },
    );
    let sys = two(
        pose([0.0, 0.0, 10.0], DVec3::Z, 0.0),
        vec![side, front, limit],
    );
    let (_, r) = solve(&sys, &[], true);
    assert_eq!(r.dof[1], 1, "free inside the range");
    let drag = Drag::Point {
        body: 1,
        local: DVec3::ZERO,
        target: DVec3::new(0.0, 0.0, 60.0),
    };
    let (poses, r) = solve(&sys, &[drag], false);
    assert!(r.solved);
    assert!(
        (poses[1].translation.z - 25.0).abs() < 1e-5,
        "{:?}",
        poses[1]
    );
    let drag = Drag::Point {
        body: 1,
        local: DVec3::ZERO,
        target: DVec3::new(0.0, 0.0, 17.0),
    };
    let (poses, _) = solve(&sys, &[drag], false);
    assert!(
        (poses[1].translation.z - 17.0).abs() < 1e-3,
        "{:?}",
        poses[1]
    );
}

/// Two shafts on parallel fixed axes 30 mm apart; gear ratio 2.
fn gear_system(start_b: f64) -> System {
    let axis_a = Line::new(Some(1), DVec3::ZERO, DVec3::Z);
    let axis_b = Line::new(Some(2), DVec3::ZERO, DVec3::Z);
    let ground_a = Line::new(None, DVec3::ZERO, DVec3::Z);
    let ground_b = Line::new(None, DVec3::new(30.0, 0.0, 0.0), DVec3::Z);
    let face = |body| {
        coincident(
            0,
            Plane::new(None, DVec3::ZERO, DVec3::Z),
            Plane::new(Some(body), DVec3::ZERO, -DVec3::Z),
        )
    };
    let rotation = |body| Motion::Rotation {
        vector: Dir::new(Some(body), DVec3::X),
        reference: Dir::new(None, DVec3::X),
        axis: Dir::new(None, DVec3::Z),
    };
    let mut mates = vec![
        concentric(1, ground_a, axis_a),
        concentric(2, ground_b, axis_b),
        Mate { id: 3, ..face(1) },
        Mate { id: 4, ..face(2) },
        Mate {
            id: 5,
            terms: vec![Term::Coupler {
                // θa + 2 θb = phase: B turns half as fast, the other way.
                motions: vec![(rotation(1), 1.0), (rotation(2), 2.0)],
                start: vec![0.0, start_b],
                phase: 2.0 * start_b,
            }],
        },
    ];
    mates.iter_mut().for_each(|m| {
        if m.id == 0 {
            m.id = 9;
        }
    });
    System {
        poses: vec![
            Pose::IDENTITY,
            Pose::IDENTITY,
            pose([30.0, 0.0, 0.0], DVec3::Z, start_b),
        ],
        fixed: vec![true, false, false],
        mates,
    }
}

#[test]
fn gear_coupler_turns_the_driven_shaft_at_the_ratio_across_turns() {
    let mut sys = gear_system(0.0);
    let (_, r) = solve(&sys, &[], true);
    assert!(r.solved, "{r:?}");
    assert_eq!(r.dof, vec![0, 1, 1]);
    assert_eq!(r.free, 1, "one shared rotation");

    // Turn shaft A by 3/4 turn in steps of 1/8 so angles unwrap continuously.
    let mut angle_a = 0.0;
    for _ in 0..6 {
        angle_a += TAU / 8.0;
        let target = pose([0.0, 0.0, 0.0], DVec3::Z, angle_a);
        let (poses, r) = solve(
            &sys,
            &[Drag::Pose {
                body: 1,
                pose: target,
            }],
            false,
        );
        assert!(r.solved);
        sys.poses = poses;
        if let Some(Term::Coupler { start, .. }) = sys.mates[4].terms.first_mut() {
            *start = r.couplers[&5].clone();
        }
    }
    let values = &solve(&sys, &[], false).1.couplers[&5];
    assert!((values[0] - 0.75 * TAU).abs() < 1e-4, "{values:?}");
    assert!((values[1] + 0.375 * TAU).abs() < 1e-4, "{values:?}");
}

#[test]
fn lock_keeps_a_relative_pose() {
    let relative = pose([10.0, 0.0, 0.0], DVec3::Z, FRAC_PI_2);
    let sys = System {
        poses: vec![
            pose([1.0, 2.0, 3.0], DVec3::X, 0.5),
            pose([0.0, 5.0, 0.0], DVec3::Y, 1.0),
        ],
        fixed: vec![true, false],
        mates: vec![Mate {
            id: 1,
            terms: vec![Term::Lock {
                a: Some(0),
                b: Some(1),
                relative,
            }],
        }],
    };
    let (poses, r) = solve(&sys, &[], true);
    assert!(r.solved);
    assert_eq!(r.dof[1], 0);
    let want = sys.poses[0].then(&relative);
    assert!((poses[1].translation - want.translation).length() < EPS);
    assert!(poses[1].rotation.angle_between(want.rotation) < EPS);
}

#[test]
fn path_mate_places_a_point_by_arc_length() {
    // A square path 40 mm per side in the assembly; the point is 50 mm along.
    let path = Curve {
        body: None,
        points: vec![
            DVec3::ZERO,
            DVec3::new(40.0, 0.0, 0.0),
            DVec3::new(40.0, 40.0, 0.0),
            DVec3::new(0.0, 40.0, 0.0),
        ],
        closed: true,
    };
    let sys = System {
        poses: vec![pose([5.0, 5.0, 5.0], DVec3::Z, 0.0)],
        fixed: vec![false],
        mates: vec![Mate {
            id: 1,
            terms: vec![Term::PointAtParameter {
                point: Pt::new(Some(0), DVec3::ZERO),
                curve: path.clone(),
                at: 50.0,
            }],
        }],
    };
    let (poses, r) = solve(&sys, &[], true);
    assert!(r.solved);
    assert!((poses[0].translation - DVec3::new(40.0, 10.0, 0.0)).length() < EPS);
    assert_eq!(r.dof[0], 3, "rotation is still free");

    let along = Measure::CurveParameter {
        point: Pt::new(Some(0), DVec3::ZERO),
        curve: path,
    };
    assert!((measure_value(&poses, &along) - 50.0).abs() < 1e-6);
}

#[test]
fn angle_mate_and_wrap() {
    assert!((wrap(3.0 * PI) - PI).abs() < 1e-12);
    assert!((wrap(-0.5) + 0.5).abs() < 1e-12);
    let sys = two(
        pose([0.0, 0.0, 0.0], DVec3::Z, 0.1),
        vec![
            concentric(
                1,
                Line::new(Some(0), DVec3::ZERO, DVec3::Z),
                Line::new(Some(1), DVec3::ZERO, DVec3::Z),
            ),
            Mate {
                id: 2,
                terms: vec![Term::Value {
                    measure: Measure::Angle {
                        a: Dir::new(Some(0), DVec3::X),
                        b: Dir::new(Some(1), DVec3::X),
                    },
                    target: Target::Equal(PI / 3.0),
                }],
            },
        ],
    );
    let (poses, r) = solve(&sys, &[], true);
    assert!(r.solved);
    let x = poses[1].vector(DVec3::X);
    assert!((x.angle_between(DVec3::X) - PI / 3.0).abs() < 1e-6);
}

#[test]
fn independent_groups_fail_separately() {
    // Body 1 conflicts; body 2 solves.
    let a = plane(0, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let b1 = plane(1, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
    let b2 = plane(2, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
    let sys = System {
        poses: vec![
            Pose::IDENTITY,
            Pose::from_translation(DVec3::Z * 3.0),
            Pose::from_translation(DVec3::Z * 3.0),
        ],
        fixed: vec![true, false, false],
        mates: vec![
            distance(1, a, b1, Target::Equal(1.0)),
            distance(2, a, b1, Target::Equal(2.0)),
            coincident(3, a, b2),
        ],
    };
    let (poses, r) = solve(&sys, &[], true);
    assert!(!r.solved);
    assert_eq!(r.conflicting, vec![1, 2]);
    assert!(poses[2].translation.z.abs() < EPS);
    assert_eq!(poses[1], sys.poses[1]);
}

#[test]
fn chain_of_two_hundred_bodies_solves_quickly() {
    // Each body sits on the previous one: a stack of 10 mm blocks.
    let n = 200;
    let mut poses = vec![Pose::IDENTITY];
    let mut fixed = vec![true];
    let mut mates = Vec::new();
    for i in 1..=n {
        poses.push(pose([0.3, -0.2, 10.0 * i as f64 + 0.5], DVec3::Z, 0.01));
        fixed.push(false);
        let id = (i * 3) as u32;
        mates.push(coincident(
            id,
            plane(i - 1, [0.0, 0.0, 10.0], [0.0, 0.0, 1.0]),
            plane(i, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ));
        mates.push(coincident(
            id + 1,
            plane(i - 1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
            plane(i, [0.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
        ));
        mates.push(coincident(
            id + 2,
            plane(i - 1, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            plane(i, [0.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
        ));
    }
    let sys = System {
        poses,
        fixed,
        mates,
    };
    let t = std::time::Instant::now();
    let (poses, r) = solve(&sys, &[], false);
    let elapsed = t.elapsed();
    assert!(r.solved);
    let error = (poses[n].translation.z - 10.0 * n as f64).abs();
    assert!(error < 1e-4, "{error}");
    eprintln!("200 bodies / 600 mates: {elapsed:?}");
    let t = std::time::Instant::now();
    let (_, r) = solve(&System { poses, ..sys }, &[], true);
    eprintln!("analysis: {:?}", t.elapsed());
    assert!(r.dof.iter().all(|d| *d == 0));
    assert_eq!(r.free, 0);
}

#[test]
fn symmetric_points_and_width_sums() {
    // Body 1's origin mirrors body 0's point (5, 2, 0) in the YZ plane.
    let mirror = Plane::new(None, DVec3::ZERO, DVec3::X);
    let sys = System {
        poses: vec![Pose::IDENTITY, pose([1.0, 1.0, 1.0], DVec3::Z, 0.0)],
        fixed: vec![true, false],
        mates: vec![Mate {
            id: 1,
            terms: vec![Term::MirrorPoints {
                a: Pt::new(Some(0), DVec3::new(5.0, 2.0, 0.0)),
                b: Pt::new(Some(1), DVec3::ZERO),
                plane: mirror,
            }],
        }],
    };
    let (poses, r) = solve(&sys, &[], true);
    assert!(r.solved);
    assert!((poses[1].translation - DVec3::new(-5.0, 2.0, 0.0)).length() < EPS);

    // A 4 mm tab (faces at local x = 0 and 4) centred in a 10 mm gap.
    let w1 = Plane::new(Some(0), DVec3::ZERO, DVec3::X);
    let tab = |x: f64| Pt::new(Some(1), DVec3::new(x, 0.0, 0.0));
    let center = Measure::Sum(vec![
        (
            Measure::PlaneDistance {
                point: tab(0.0),
                plane: w1,
            },
            1.0,
        ),
        (
            Measure::PlaneDistance {
                point: tab(4.0),
                plane: w1,
            },
            1.0,
        ),
    ]);
    let sys = System {
        poses: vec![Pose::IDENTITY, pose([1.0, 0.0, 0.0], DVec3::Z, 0.0)],
        fixed: vec![true, false],
        mates: vec![
            Mate {
                id: 1,
                terms: vec![Term::Parallel {
                    a: Dir::new(Some(0), DVec3::X),
                    b: Dir::new(Some(1), DVec3::X),
                    sign: Some(1.0),
                }],
            },
            Mate {
                id: 2,
                terms: vec![Term::Value {
                    measure: center,
                    target: Target::Equal(10.0),
                }],
            },
        ],
    };
    let (poses, r) = solve(&sys, &[], true);
    assert!(r.solved);
    assert!((poses[1].translation.x - 3.0).abs() < EPS, "{:?}", poses[1]);
}

#[test]
fn path_tangent_turns_a_follower_along_the_path() {
    let path = Curve {
        body: None,
        points: vec![DVec3::ZERO, DVec3::new(0.0, 50.0, 0.0)],
        closed: false,
    };
    let sys = System {
        poses: vec![pose([0.0, 10.0, 0.0], DVec3::Z, 0.3)],
        fixed: vec![false],
        mates: vec![Mate {
            id: 1,
            terms: vec![
                Term::Value {
                    measure: Measure::CurveDistance {
                        point: Pt::new(Some(0), DVec3::ZERO),
                        curve: path.clone(),
                        normal: None,
                    },
                    target: Target::Equal(0.0),
                },
                Term::CurveTangent {
                    dir: Dir::new(Some(0), DVec3::X),
                    point: Pt::new(Some(0), DVec3::ZERO),
                    curve: path,
                },
            ],
        }],
    };
    let (poses, r) = solve(&sys, &[], false);
    assert!(r.solved);
    assert!(poses[0].vector(DVec3::X).cross(DVec3::Y).length() < 1e-6);
}
