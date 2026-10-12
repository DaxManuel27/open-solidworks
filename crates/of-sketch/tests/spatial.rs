use glam::DVec3;
use of_sketch::SolveStatus;
use of_sketch::spatial::{Axis3, Geometry3, Relation3 as R, Sketch3D};

#[test]
fn stationary_spline_parameter_is_not_a_geometric_degree_of_freedom() {
    let mut sk = Sketch3D::default();
    let points = [-1.0, 1.0, -1.0, 1.0].map(|x| sk.add_point(DVec3::X * x));
    let curve = sk.add(Geometry3::Bezier { points });
    for point in points {
        sk.constrain(R::Fixed {
            point,
            at: sk.point(point).unwrap(),
        });
    }
    let point = sk.add_point(DVec3::ZERO);
    sk.constrain(R::Fixed {
        point,
        at: DVec3::ZERO,
    });
    sk.constrain(R::PointOnCurve { point, curve });
    let report = sk.solve(None).unwrap();
    assert_eq!(report.status, SolveStatus::Solved);
    assert_eq!(report.dof, 0, "{report:?}");
}

#[test]
fn tangent_relation_rejects_backtracking_cusps_without_moving_points() {
    let mut sk = Sketch3D::default();
    let a = sk.add_point(DVec3::ZERO);
    let b = sk.add_point(DVec3::X);
    let c = sk.add_point(DVec3::ZERO);
    let first = sk.add(Geometry3::Line { a, b });
    let second = sk.add(Geometry3::Line { a: b, b: c });
    for point in [a, b, c] {
        sk.constrain(R::Fixed {
            point,
            at: sk.point(point).unwrap(),
        });
    }
    let tangent = sk.constrain(R::Tangent {
        a: first,
        b: second,
        end_a: true,
        end_b: false,
    });
    let before = sk.clone();
    let report = sk.solve(None).unwrap();
    assert_eq!(report.status, SolveStatus::Failed);
    assert!(report.conflicting.contains(&tangent));
    assert_eq!(sk, before);
}

#[test]
fn spatial_dimensions_conflicts_and_drag_are_transactional() {
    for size in [0.001, 1.0, 1000.0, 100_000.0] {
        let mut sk = Sketch3D::default();
        let a = sk.add_point(DVec3::ZERO);
        let b = sk.add_point(DVec3::new(size * 0.9, size * 0.03, size * 0.04));
        let line = sk.add(Geometry3::Line { a, b });
        sk.constrain(R::Fixed {
            point: a,
            at: DVec3::ZERO,
        });
        sk.constrain(R::Axis {
            line,
            axis: Axis3::X,
        });
        let dim = sk.constrain(R::Distance { a, b, value: size });
        let r = sk.solve(None).unwrap();
        assert_eq!(r.status, SolveStatus::Solved, "size={size}: {r:?}");
        assert_eq!(r.dof, 0, "{r:?}");
        assert!(sk.point(b).unwrap().distance(DVec3::X * size) < size * 1e-6 + 1e-8);
        sk.constraints.retain(|c| c.id != dim);
        assert_eq!(
            sk.solve(Some((b, DVec3::new(size * 2.0, size, size))))
                .unwrap()
                .status,
            SolveStatus::Solved
        );
        assert!(
            (sk.point(b).unwrap().x - size * 2.0).abs() < size * 1e-4 + 1e-8,
            "size={size}, point={:?}",
            sk.point(b).unwrap()
        );
        sk.constrain(R::Distance { a, b, value: size });
        sk.solve(None).unwrap();
        let bad = sk.constrain(R::Distance {
            a,
            b,
            value: size * 2.0,
        });
        let before = sk.clone();
        let report = sk.solve(None).unwrap();
        assert_eq!(report.status, SolveStatus::Failed);
        assert_eq!(sk, before);
        assert!(report.conflicting.contains(&bad));
    }
}

#[test]
fn plane_parallel_perpendicular_and_coincidence() {
    let mut sk = Sketch3D::default();
    let a = sk.add_point(DVec3::ZERO);
    let b = sk.add_point(DVec3::X * 5.0);
    let c = sk.add_point(DVec3::new(0.0, 2.0, 0.0));
    let d = sk.add_point(DVec3::new(4.0, 2.4, 0.1));
    let ab = sk.add(Geometry3::Line { a, b });
    let cd = sk.add(Geometry3::Line { a: c, b: d });
    sk.constrain(R::Fixed {
        point: a,
        at: DVec3::ZERO,
    });
    sk.constrain(R::Fixed {
        point: b,
        at: DVec3::X * 5.0,
    });
    sk.constrain(R::Parallel { a: ab, b: cd });
    sk.constrain(R::PointOnPlane {
        point: c,
        origin: DVec3::ZERO,
        normal: DVec3::Z,
    });
    assert_eq!(sk.solve(None).unwrap().status, SolveStatus::Solved);
    assert!(
        (sk.point(d).unwrap() - sk.point(c).unwrap())
            .normalize()
            .cross(DVec3::X)
            .length()
            < 1e-7
    );
    sk.constraints
        .retain(|c| !matches!(c.relation, R::Parallel { .. }));
    sk.entities.iter_mut().find(|e| e.id == d).unwrap().geometry = Geometry3::Point {
        at: DVec3::new(1.0, 5.0, 0.0),
    };
    sk.constrain(R::Perpendicular { a: ab, b: cd });
    assert_eq!(sk.solve(None).unwrap().status, SolveStatus::Solved);
    assert!(
        (sk.point(d).unwrap() - sk.point(c).unwrap())
            .normalize()
            .dot(DVec3::X)
            .abs()
            < 1e-7
    );
    let e = sk.add_point(DVec3::new(0.1, 0.2, 0.3));
    sk.constrain(R::Coincident { a: e, b: a });
    assert_eq!(sk.solve(None).unwrap().status, SolveStatus::Solved);
    assert!(sk.point(e).unwrap().length() < 1e-7);
}

#[test]
fn cubic_tangent_point_on_curve_and_validation() {
    let mut sk = Sketch3D::default();
    let points = [
        DVec3::ZERO,
        DVec3::X,
        DVec3::new(2.0, 1.0, 1.0),
        DVec3::new(3.0, 1.0, 1.0),
    ]
    .map(|p| sk.add_point(p));
    let curve = sk.add(Geometry3::Bezier { points });
    for id in points {
        sk.constrain(R::Fixed {
            point: id,
            at: sk.point(id).unwrap(),
        });
    }
    let q = sk.add_point(DVec3::new(1.4, 0.7, 0.3));
    sk.constrain(R::PointOnCurve { point: q, curve });
    let report = sk.solve(None).unwrap();
    assert_eq!(report.status, SolveStatus::Solved, "{report:?}");
    assert_eq!(report.dof, 1, "{report:?}");
    let end = sk.add_point(DVec3::new(4.0, 1.1, 1.2));
    let line = sk.add(Geometry3::Line {
        a: points[3],
        b: end,
    });
    sk.constrain(R::Tangent {
        a: curve,
        b: line,
        end_a: true,
        end_b: false,
    });
    assert_eq!(sk.solve(None).unwrap().status, SolveStatus::Solved);
    assert!(
        (sk.point(end).unwrap() - sk.point(points[3]).unwrap())
            .normalize()
            .cross(DVec3::X)
            .length()
            < 1e-7
    );
    let roundtrip: Sketch3D = serde_json::from_str(&serde_json::to_string(&sk).unwrap()).unwrap();
    assert_eq!(roundtrip.constraints, sk.constraints);
    assert_eq!(roundtrip.next_id, sk.next_id);
    for e in &sk.entities {
        if let Geometry3::Point { at } = e.geometry {
            assert!(roundtrip.point(e.id).unwrap().distance(at) < 1e-12);
        }
    }
    let before = sk.clone();
    assert!(sk.solve(Some((q, DVec3::splat(f64::NAN)))).is_err());
    assert_eq!(sk, before);
    sk.add(Geometry3::Line { a: q, b: q });
    assert!(sk.validate().unwrap_err().contains("zero-length"));
}
