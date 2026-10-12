use std::f64::consts::PI;

use of_geom::{Curve2, Curve3, DVec2, DVec3, Plane, Shape};

fn circle(radius: f64) -> Shape {
    Shape::wire(
        &Plane::front(),
        &[Curve2::Circle {
            center: DVec2::ZERO,
            radius,
        }],
    )
    .unwrap()
}

fn polyline(points: &[DVec3]) -> Shape {
    let lines: Vec<_> = points
        .windows(2)
        .map(|w| Curve3::Line { a: w[0], b: w[1] })
        .collect();
    Shape::wire3(&lines).unwrap()
}

#[test]
fn sharp_corner_sweeps_the_whole_path_with_a_mitre() {
    // A mitred elbow keeps each leg's volume up to the corner point.
    let path = polyline(&[
        DVec3::ZERO,
        DVec3::new(0.0, 0.0, 20.0),
        DVec3::new(15.0, 0.0, 20.0),
    ]);
    for orientation in 0..2 {
        let result = circle(1.0)
            .sweep(&path, &Plane::front(), orientation)
            .unwrap();
        assert!(result.shape.is_valid());
        let volume = result.shape.mass_props().unwrap().volume;
        assert!(
            (volume - PI * 35.0).abs() < 1e-3,
            "orientation {orientation}: volume {volume}"
        );
    }
    // A fixed profile orientation lies flat along the second leg.
    let error = circle(1.0)
        .sweep(&path, &Plane::front(), 2)
        .err()
        .expect("a degenerate fixed-orientation sweep must fail");
    assert!(error.0.contains("intersects itself"), "{}", error.0);
}

#[test]
fn several_sharp_corners_in_three_dimensions() {
    let path = polyline(&[
        DVec3::ZERO,
        DVec3::new(0.0, 0.0, 20.0),
        DVec3::new(15.0, 0.0, 20.0),
        DVec3::new(15.0, 10.0, 20.0),
    ]);
    let result = circle(1.0).sweep(&path, &Plane::front(), 0).unwrap();
    let volume = result.shape.mass_props().unwrap().volume;
    assert!((volume - PI * 45.0).abs() < 1e-3, "volume {volume}");
}

#[test]
fn self_intersecting_sweep_is_rejected() {
    // A radius-3 profile cannot turn around a radius-1 bend.
    let path = Shape::wire(
        &Plane::top(),
        &[
            Curve2::Line {
                a: DVec2::ZERO,
                b: DVec2::new(0.0, -10.0),
            },
            Curve2::Arc {
                center: DVec2::new(1.0, -10.0),
                start: DVec2::new(0.0, -10.0),
                end: DVec2::new(1.0, -11.0),
            },
        ],
    )
    .unwrap();
    let error = circle(3.0)
        .sweep(&path, &Plane::front(), 0)
        .err()
        .expect("self-intersecting sweep must fail");
    assert!(error.0.contains("intersects itself"), "{}", error.0);
}

#[test]
fn profile_just_inside_the_bend_radius_sweeps() {
    let path = Shape::wire(
        &Plane::top(),
        &[
            Curve2::Line {
                a: DVec2::ZERO,
                b: DVec2::new(0.0, -10.0),
            },
            Curve2::Arc {
                center: DVec2::new(5.0, -10.0),
                start: DVec2::new(0.0, -10.0),
                end: DVec2::new(5.0, -15.0),
            },
        ],
    )
    .unwrap();
    let result = circle(4.5).sweep(&path, &Plane::front(), 0).unwrap();
    let volume = result.shape.mass_props().unwrap().volume;
    let expected = PI * 4.5 * 4.5 * (10.0 + PI / 2.0 * 5.0);
    assert!(
        (volume - expected).abs() < expected * 1e-4,
        "volume {volume}"
    );
}
