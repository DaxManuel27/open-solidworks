use of_geom::{Curve2, Curve3, DVec2, DVec3, EndTangent, Plane, Shape};
use std::f64::consts::PI;

fn circle(z: f64, radius: f64) -> Shape {
    let plane = Plane::new(DVec3::Z * z, DVec3::X, DVec3::Z);
    Shape::wire(
        &plane,
        &[Curve2::Circle {
            center: DVec2::ZERO,
            radius,
        }],
    )
    .unwrap()
}

fn square(plane: &Plane, half: f64) -> Shape {
    let p = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(x, y)| DVec2::new(x * half, y * half));
    let curves: Vec<Curve2> = (0..4)
        .map(|i| Curve2::Line {
            a: p[i],
            b: p[(i + 1) % 4],
        })
        .collect();
    Shape::wire(plane, &curves).unwrap()
}

fn volume(s: &Shape) -> f64 {
    s.mass_props().unwrap().volume
}

/// Volume of a solid of revolution with radius `r(z)` over `[0, h]`.
fn revolved(r: impl Fn(f64) -> f64, h: f64) -> f64 {
    let n = 4000;
    let dz = h / n as f64;
    (0..n)
        .map(|i| r((i as f64 + 0.5) * dz).powi(2) * dz)
        .sum::<f64>()
        * PI
}

#[test]
fn normal_end_tangents_make_a_hermite_profile() {
    let sections = [circle(0.0, 5.0), circle(10.0, 3.0)];
    // Tangent length 1 along the axis at both ends: the height grows linearly
    // and the radius follows the cubic Hermite blend with zero end slopes.
    let r = Shape::loft_tangent(
        &sections,
        false,
        EndTangent::Normal(1.0),
        EndTangent::Normal(1.0),
    )
    .unwrap();
    assert!(r.shape.is_valid());
    let hermite = |z: f64| {
        let s = z / 10.0;
        5.0 - 2.0 * (3.0 * s * s - 2.0 * s.powi(3))
    };
    let expected = revolved(hermite, 10.0);
    assert!(
        (volume(&r.shape) / expected - 1.0).abs() < 1e-5,
        "{} vs {expected}",
        volume(&r.shape)
    );
    // Caps and one side face per section edge.
    assert_eq!(r.shape.count(of_geom::Kind::Face), 3);
    let cone = PI * 10.0 / 3.0 * (25.0 + 15.0 + 9.0);
    assert!((volume(&r.shape) - cone).abs() > 1.0);

    // Only the start constrained: zero slope at the base and a quadratic radius.
    let start =
        Shape::loft_tangent(&sections, false, EndTangent::Normal(1.0), EndTangent::Free).unwrap();
    let quadratic = revolved(|z| 5.0 - 2.0 * (z / 10.0).powi(2), 10.0);
    let v = volume(&start.shape);
    assert!((v / quadratic - 1.0).abs() < 1e-5, "{v} vs {quadratic}");

    // A longer tangent bulges further: compare against the radius at mid-height.
    let long = Shape::loft_tangent(
        &sections,
        false,
        EndTangent::Normal(1.5),
        EndTangent::Normal(1.5),
    )
    .unwrap();
    assert!(long.shape.is_valid());
    assert!((volume(&long.shape) - expected).abs() > 1.0);

    // A direction tangent tilts the side away from the axis.
    let tilted = Shape::loft_tangent(
        &sections,
        false,
        EndTangent::Direction(DVec3::new(1.0, 0.0, 1.0), 1.0),
        EndTangent::Free,
    )
    .unwrap();
    assert!(tilted.shape.is_valid());

    // Bad inputs.
    assert!(
        Shape::loft_tangent(&sections, false, EndTangent::Normal(0.0), EndTangent::Free).is_err()
    );
    assert!(
        Shape::loft_tangent(
            &sections,
            false,
            EndTangent::Direction(DVec3::ZERO, 1.0),
            EndTangent::Free
        )
        .is_err()
    );
}

#[test]
fn centerline_loft_blends_sections_along_the_curve() {
    // Straight centerline running past both sections: a frustum between them.
    let sections = [circle(0.0, 5.0), circle(10.0, 3.0)];
    let axis = Shape::wire3(&[Curve3::Line {
        a: DVec3::Z * -2.0,
        b: DVec3::Z * 12.0,
    }])
    .unwrap();
    let r = Shape::loft_centerline(&sections, &axis).unwrap();
    assert!(r.shape.is_valid());
    let cone = PI * 10.0 / 3.0 * (25.0 + 15.0 + 9.0);
    assert!(
        (volume(&r.shape) / cone - 1.0).abs() < 1e-5,
        "{}",
        volume(&r.shape)
    );

    // Equal squares at the ends of a quarter arc: area times arc length.
    let arc = Shape::wire3(&[Curve3::Bezier {
        points: [
            DVec3::new(10.0, 0.0, 0.0),
            DVec3::new(10.0, 0.0, 5.0),
            DVec3::new(5.0, 0.0, 10.0),
            DVec3::new(0.0, 0.0, 10.0),
        ],
    }])
    .unwrap();
    let a = square(
        &Plane::new(DVec3::new(10.0, 0.0, 0.0), DVec3::X, DVec3::Z),
        1.0,
    );
    let b = square(
        &Plane::new(DVec3::new(0.0, 0.0, 10.0), DVec3::Z, DVec3::X),
        1.0,
    );
    let r = Shape::loft_centerline(&[a.clone(), b.clone()], &arc).unwrap();
    assert!(r.shape.is_valid());
    let length = arc.edge_info(0).unwrap().length;
    assert!(
        (volume(&r.shape) / (4.0 * length) - 1.0).abs() < 1e-3,
        "{} vs {}",
        volume(&r.shape),
        4.0 * length
    );

    // A centerline that misses a section's plane is rejected.
    let short = Shape::wire3(&[Curve3::Line {
        a: DVec3::Z * 1.0,
        b: DVec3::Z * 12.0,
    }])
    .unwrap();
    assert!(Shape::loft_centerline(&sections, &short).is_err());
    // Sections out of order along it are rejected.
    let reversed = [circle(10.0, 3.0), circle(0.0, 5.0)];
    assert!(Shape::loft_centerline(&reversed, &axis).is_err());
}
