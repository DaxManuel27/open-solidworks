use of_geom::{Curve2, Curve3, DVec2, DVec3, Kind, Plane, Shape};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

fn square(half: f64) -> Shape {
    let p = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(x, y)| DVec2::new(x * half, y * half));
    let curves: Vec<Curve2> = (0..4)
        .map(|i| Curve2::Line {
            a: p[i],
            b: p[(i + 1) % 4],
        })
        .collect();
    Shape::wire(&Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z), &curves).unwrap()
}

fn line(a: DVec3, b: DVec3) -> Shape {
    Shape::wire3(&[Curve3::Line { a, b }]).unwrap()
}

/// Largest x among the vertices at height `z`.
fn max_x_at(s: &Shape, z: f64) -> f64 {
    (0..s.count(Kind::Vertex))
        .map(|i| s.vertex_point(i).unwrap())
        .filter(|p| (p.z - z).abs() < 1e-6)
        .map(|p| p.x)
        .fold(f64::MIN, f64::max)
}

#[test]
fn twisted_sweep_keeps_area_times_length_and_turns_the_end() {
    let path = line(DVec3::ZERO, DVec3::Z * 20.0);
    for (twist, corner) in [(FRAC_PI_4, 2.0 * 2f64.sqrt()), (FRAC_PI_2, 2.0), (PI, 2.0)] {
        let r = square(2.0).sweep_guided(&path, None, twist).unwrap();
        assert!(r.shape.is_valid());
        let v = r.shape.mass_props().unwrap().volume;
        assert!((v / 320.0 - 1.0).abs() < 1e-5, "{twist}: {v}");
        assert!((max_x_at(&r.shape, 0.0) - 2.0).abs() < 1e-6);
        assert!((max_x_at(&r.shape, 20.0) - corner).abs() < 1e-5, "{twist}");
    }
    // No twist matches an ordinary sweep.
    let r = square(2.0).sweep_guided(&path, None, 0.0).unwrap();
    assert!((max_x_at(&r.shape, 20.0) - 2.0).abs() < 1e-6);
}

#[test]
fn guide_curve_sets_the_section_rotation() {
    let path = line(DVec3::ZERO, DVec3::Z * 20.0);
    // An eighth-turn helix beside the path turns the section by 45° over 20 mm.
    let frame = Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z);
    let guide = Shape::helix(&frame, 8.0, 160.0, 0.125, 0.0, false).unwrap();
    let r = square(2.0).sweep_guided(&path, Some(&guide), 0.0).unwrap();
    assert!(r.shape.is_valid());
    let v = r.shape.mass_props().unwrap().volume;
    assert!((v / 320.0 - 1.0).abs() < 1e-5, "{v}");
    let corner = max_x_at(&r.shape, 20.0);
    assert!((corner - 2.0 * 2f64.sqrt()).abs() < 1e-5, "{corner}");
    // A twist and a guide together are rejected.
    assert!(square(2.0).sweep_guided(&path, Some(&guide), 1.0).is_err());
}

#[test]
fn path_start_reports_point_and_tangent() {
    let path = line(DVec3::new(1.0, 2.0, 3.0), DVec3::new(1.0, 2.0, 13.0));
    let start = path.path_start().unwrap();
    assert!((start.origin - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-12);
    assert!((start.normal - DVec3::Z).length() < 1e-12);
    assert!(start.x_dir.dot(start.normal).abs() < 1e-12);
}

fn circle_profile(radius: f64) -> Shape {
    Shape::wire(
        &Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z),
        &[Curve2::Circle {
            center: DVec2::ZERO,
            radius,
        }],
    )
    .unwrap()
}

#[test]
fn scaling_guide_keeps_the_section_on_the_guide() {
    let path = line(DVec3::ZERO, DVec3::Z * 20.0);
    // A straight guide from the circle's rim outward: a frustum.
    let guide = line(DVec3::new(2.0, 0.0, 0.0), DVec3::new(4.0, 0.0, 20.0));
    let r = circle_profile(2.0)
        .sweep_scaled(&path, &guide, None, true)
        .unwrap();
    assert!(r.shape.is_valid());
    let frustum = PI * 20.0 / 3.0 * (4.0 + 8.0 + 16.0);
    let v = r.shape.mass_props().unwrap().volume;
    assert!((v / frustum - 1.0).abs() < 1e-6, "{v} vs {frustum}");
    // Without scaling the guide only turns the section.
    let r = circle_profile(2.0)
        .sweep_scaled(&path, &guide, None, false)
        .unwrap();
    let v = r.shape.mass_props().unwrap().volume;
    assert!((v / (PI * 4.0 * 20.0) - 1.0).abs() < 1e-6, "{v}");

    // A curved guide: a square whose half-width follows the guide's distance
    // from the path. The guide crosses each plane once.
    let points = [
        DVec3::new(2.0, 0.0, 0.0),
        DVec3::new(4.0, 0.0, 6.0),
        DVec3::new(1.0, 0.0, 14.0),
        DVec3::new(3.0, 0.0, 20.0),
    ];
    let guide = Shape::wire3(&[Curve3::Bezier { points }]).unwrap();
    let r = square(2.0).sweep_scaled(&path, &guide, None, true).unwrap();
    assert!(r.shape.is_valid());
    let bezier = |t: f64| {
        let s = 1.0 - t;
        points[0] * s.powi(3)
            + points[1] * 3.0 * s * s * t
            + points[2] * 3.0 * s * t * t
            + points[3] * t.powi(3)
    };
    let n = 20000;
    let expected: f64 = (0..n)
        .map(|i| {
            let (t0, t1) = (i as f64 / n as f64, (i + 1) as f64 / n as f64);
            let p = bezier(0.5 * (t0 + t1));
            4.0 * p.x * p.x * (bezier(t1).z - bezier(t0).z)
        })
        .sum();
    let v = r.shape.mass_props().unwrap().volume;
    assert!((v / expected - 1.0).abs() < 1e-5, "{v} vs {expected}");
    // Side faces come from the profile's four edges, plus the two caps.
    assert_eq!(r.shape.count(Kind::Face), 6);
}

#[test]
fn two_guides_stretch_the_section() {
    let path = line(DVec3::ZERO, DVec3::Z * 20.0);
    let widening = line(DVec3::new(2.0, 0.0, 0.0), DVec3::new(4.0, 0.0, 20.0));
    let constant = line(DVec3::new(0.0, 2.0, 0.0), DVec3::new(0.0, 2.0, 20.0));
    let r = square(2.0)
        .sweep_scaled(&path, &widening, Some(&constant), false)
        .unwrap();
    assert!(r.shape.is_valid());
    // Half-widths 2 + z/10 along x and 2 along y.
    let v = r.shape.mass_props().unwrap().volume;
    assert!((v / 480.0 - 1.0).abs() < 1e-6, "{v}");
    assert!((max_x_at(&r.shape, 20.0) - 4.0).abs() < 1e-6);
    // A circle stretched by the second guide becomes an ellipse.
    let r = circle_profile(2.0)
        .sweep_scaled(&path, &widening, Some(&constant), false)
        .unwrap();
    let v = r.shape.mass_props().unwrap().volume;
    assert!((v / (PI * 2.0 * 60.0) - 1.0).abs() < 1e-6, "{v}");

    // Guides in line with the path can't fix the stretch.
    let opposite = line(DVec3::new(-2.0, 0.0, 0.0), DVec3::new(-2.0, 0.0, 20.0));
    assert!(
        square(2.0)
            .sweep_scaled(&path, &widening, Some(&opposite), false)
            .is_err()
    );
    // A guide that ends before the path does is rejected.
    let short = line(DVec3::new(2.0, 0.0, 0.0), DVec3::new(2.0, 0.0, 10.0));
    assert!(square(2.0).sweep_scaled(&path, &short, None, true).is_err());
    // So is a path with a sharp corner.
    let corner = Shape::wire3(&[
        Curve3::Line {
            a: DVec3::ZERO,
            b: DVec3::Z * 10.0,
        },
        Curve3::Line {
            a: DVec3::Z * 10.0,
            b: DVec3::new(10.0, 0.0, 10.0),
        },
    ])
    .unwrap();
    assert!(
        square(2.0)
            .sweep_scaled(&corner, &widening, None, true)
            .is_err()
    );
}
