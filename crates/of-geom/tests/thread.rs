use of_geom::{BoolOp, Curve2, DVec2, DVec3, Plane, Profile, ProfileFace, Shape};
use std::f64::consts::TAU;

fn disk(radius: f64) -> Vec<Curve2> {
    vec![Curve2::Circle {
        center: DVec2::ZERO,
        radius,
    }]
}

fn prism(outer: Vec<Curve2>, holes: Vec<Vec<Curve2>>, height: f64) -> Shape {
    Profile {
        plane: Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z),
        faces: vec![ProfileFace { outer, holes }],
    }
    .to_shape()
    .unwrap()
    .prism(DVec3::Z * height)
    .unwrap()
    .shape
}

fn volume(s: &Shape) -> f64 {
    s.mass_props().unwrap().volume
}

/// Removed volume of a helical groove whose axial section, inside the material,
/// is a trapezoid of widths `a` at radius `r0` and `b` at `r0 + h`.
fn groove(r0: f64, a: f64, b: f64, h: f64, length: f64, pitch: f64) -> f64 {
    let area = (a + b) / 2.0 * h;
    let centroid = r0 + h * (a + 2.0 * b) / (3.0 * (a + b));
    area * TAU * centroid * length / pitch
}

const DEPTH: f64 = 0.541_265_877_365_274; // 5H/8 per unit pitch

#[test]
fn external_threads_remove_the_iso_groove() {
    let (r, p, l) = (5.0, 1.5, 15.0);
    let body = prism(disk(r), vec![], 20.0);
    for left in [false, true] {
        let frame = Plane::new(DVec3::Z * 2.0, DVec3::X, DVec3::Z);
        let tool = Shape::thread_tool(&frame, r, p, l, left, false).unwrap();
        let cut = body.boolean(BoolOp::Cut, &tool).unwrap().shape;
        assert!(cut.is_valid());
        let removed = volume(&body) - volume(&cut);
        let expected = groove(r - DEPTH * p, p / 4.0, 7.0 * p / 8.0, DEPTH * p, l, p);
        // Matching this volume also pins the groove to its depth and length.
        assert!(
            (removed / expected - 1.0).abs() < 1e-4,
            "{removed} vs {expected}"
        );
    }
}

#[test]
fn internal_threads_cut_into_the_hole_wall() {
    // An M10 x 1.5 nut bored to the basic minor diameter.
    let p = 1.5;
    let minor = 5.0 - DEPTH * p;
    let body = prism(disk(10.0), vec![disk(minor)], 12.0);
    let frame = Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z);
    let tool = Shape::thread_tool(&frame, minor, p, 12.0, false, true).unwrap();
    let cut = body.boolean(BoolOp::Cut, &tool).unwrap().shape;
    assert!(cut.is_valid());
    let removed = volume(&body) - volume(&cut);
    let expected = groove(minor, 0.75 * p, p / 8.0, DEPTH * p, 12.0, p);
    assert!(
        (removed / expected - 1.0).abs() < 1e-4,
        "{removed} vs {expected}"
    );
}

#[test]
fn thread_rejects_bad_input() {
    let frame = Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z);
    assert!(Shape::thread_tool(&frame, 1.0, 5.0, 10.0, false, false).is_err());
    assert!(Shape::thread_tool(&frame, 5.0, 0.0, 10.0, false, false).is_err());
    assert!(Shape::thread_tool(&frame, 5.0, 0.1, 100.0, false, false).is_err());
}
