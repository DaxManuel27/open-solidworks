use of_geom::{BoolOp, Curve2, Curve3, DVec2, DVec3, Kind, Plane, Relation, Shape};
use std::f64::consts::PI;

fn square(z: f64, half: f64) -> Shape {
    let p = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(x, y)| DVec2::new(x * half, y * half));
    let curves: Vec<Curve2> = (0..4)
        .map(|i| Curve2::Line {
            a: p[i],
            b: p[(i + 1) % 4],
        })
        .collect();
    Shape::wire(&Plane::new(DVec3::Z * z, DVec3::X, DVec3::Z), &curves).unwrap()
}

fn circle(z: f64, radius: f64) -> Shape {
    Shape::wire(
        &Plane::new(DVec3::Z * z, DVec3::X, DVec3::Z),
        &[Curve2::Circle {
            center: DVec2::ZERO,
            radius,
        }],
    )
    .unwrap()
}

fn line(a: DVec3, b: DVec3) -> Shape {
    Shape::wire3(&[Curve3::Line { a, b }]).unwrap()
}

/// A cubic from `a` to `b` whose middle bulges by `bulge` along `out`.
fn bulge(a: DVec3, b: DVec3, out: DVec3, bulge: f64) -> Shape {
    let lift = out * (bulge * 4.0 / 3.0);
    Shape::wire3(&[Curve3::Bezier {
        points: [
            a,
            a + (b - a) / 3.0 + lift,
            a + (b - a) * 2.0 / 3.0 + lift,
            b,
        ],
    }])
    .unwrap()
}

fn volume(s: &Shape) -> f64 {
    s.mass_props().unwrap().volume
}

fn corners(z: f64, half: f64) -> [DVec3; 4] {
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(x, y)| DVec3::new(x * half, y * half, z))
}

#[test]
fn straight_guides_reproduce_prisms_and_frustums() {
    // Guides at the corners.
    let guides: Vec<Shape> = corners(0.0, 5.0)
        .iter()
        .zip(corners(10.0, 5.0))
        .map(|(a, b)| line(*a, b))
        .collect();
    let r = Shape::boundary(&[square(0.0, 5.0), square(10.0, 5.0)], &guides).unwrap();
    assert!(r.shape.is_valid());
    assert!((volume(&r.shape) - 1000.0).abs() < 1e-6);
    let caps: Vec<_> = r
        .history
        .iter()
        .filter(|h| matches!(h.rel, Relation::StartCap | Relation::EndCap))
        .collect();
    assert_eq!(caps.len(), 2);
    let patches = r
        .history
        .iter()
        .filter(|h| h.rel == Relation::Generated)
        .count();
    assert_eq!(patches, 4);

    // Guides through the middles of the sides split the sections between corners.
    let mids = [(0.0, -5.0), (5.0, 0.0), (0.0, 5.0), (-5.0, 0.0)];
    let guides: Vec<Shape> = mids
        .iter()
        .map(|(x, y)| line(DVec3::new(*x, *y, 0.0), DVec3::new(*x, *y, 10.0)))
        .collect();
    let r = Shape::boundary(&[square(0.0, 5.0), square(10.0, 5.0)], &guides).unwrap();
    assert!((volume(&r.shape) - 1000.0).abs() < 1e-6);

    // A larger top section gives a frustum of a pyramid.
    let guides: Vec<Shape> = corners(0.0, 5.0)
        .iter()
        .zip(corners(10.0, 10.0))
        .map(|(a, b)| line(*a, b))
        .collect();
    let r = Shape::boundary(&[square(0.0, 5.0), square(10.0, 10.0)], &guides).unwrap();
    let expected = 10.0 / 3.0 * (100.0 + 400.0 + 200.0);
    assert!(
        (volume(&r.shape) - expected).abs() < 1e-6,
        "{}",
        volume(&r.shape)
    );

    // Circles with two straight guides make an exact cylinder.
    let guides = [
        line(DVec3::new(5.0, 0.0, 0.0), DVec3::new(5.0, 0.0, 10.0)),
        line(DVec3::new(-5.0, 0.0, 0.0), DVec3::new(-5.0, 0.0, 10.0)),
    ];
    let r = Shape::boundary(&[circle(0.0, 5.0), circle(10.0, 5.0)], &guides).unwrap();
    assert!(
        (volume(&r.shape) / (PI * 250.0) - 1.0).abs() < 1e-7,
        "{}",
        volume(&r.shape)
    );
}

#[test]
fn both_curve_families_shape_the_surface() {
    let straight: Vec<Shape> = corners(0.0, 5.0)
        .iter()
        .zip(corners(10.0, 5.0))
        .map(|(a, b)| line(*a, b))
        .collect();
    // Direction 2 curves bulging outward add material between the sections.
    let bulged: Vec<Shape> = corners(0.0, 5.0)
        .iter()
        .zip(corners(10.0, 5.0))
        .map(|(a, b)| {
            let out = DVec3::new(a.x, a.y, 0.0).normalize();
            bulge(*a, b, out, 2.0)
        })
        .collect();
    let sections = [square(0.0, 5.0), square(10.0, 5.0)];
    let base = volume(&Shape::boundary(&sections, &straight).unwrap().shape);
    let wide = Shape::boundary(&sections, &bulged).unwrap();
    assert!(wide.shape.is_valid());
    assert!(volume(&wide.shape) > base + 50.0, "{}", volume(&wide.shape));
    // The bulge reaches the curves' apex at mid-height.
    let b = wide.shape.bbox().unwrap();
    assert!(
        b.max.x > 5.0 + 2.0 * std::f64::consts::FRAC_1_SQRT_2 - 1e-6,
        "{}",
        b.max.x
    );

    // A middle section changes the surface too.
    let three = [square(0.0, 5.0), square(5.0, 3.0), square(10.0, 5.0)];
    let pinch: Vec<Shape> = (0..4)
        .map(|i| {
            let (a, m, c) = (
                corners(0.0, 5.0)[i],
                corners(5.0, 3.0)[i],
                corners(10.0, 5.0)[i],
            );
            Shape::wire3(&[Curve3::Line { a, b: m }, Curve3::Line { a: m, b: c }]).unwrap()
        })
        .collect();
    let r = Shape::boundary(&three, &pinch).unwrap();
    let frustum = |a1: f64, a2: f64| 5.0 / 3.0 * (a1 + a2 + (a1 * a2).sqrt());
    assert!((volume(&r.shape) - 2.0 * frustum(100.0, 36.0)).abs() < 1e-6);

    // The solid accepts downstream booleans.
    let cut = r
        .shape
        .boolean(BoolOp::Cut, &Shape::make_box(2.0, 2.0, 20.0).unwrap())
        .unwrap();
    assert!(cut.shape.is_valid() && cut.shape.count(Kind::Solid) == 1);
}

#[test]
fn boundary_rejects_broken_networks() {
    let sections = [square(0.0, 5.0), square(10.0, 5.0)];
    let guides: Vec<Shape> = corners(0.0, 5.0)
        .iter()
        .zip(corners(10.0, 5.0))
        .map(|(a, b)| line(*a, b))
        .collect();
    // One direction 2 curve.
    assert!(Shape::boundary(&sections, &guides[..1]).is_err());
    // A curve that misses the top section.
    let mut short = guides.clone();
    short[0] = line(corners(0.0, 5.0)[0], DVec3::new(-5.0, -5.0, 8.0));
    let Err(e) = Shape::boundary(&sections, &short) else {
        panic!("a curve that misses a section must fail");
    };
    let e = e.to_string();
    assert!(e.contains("does not touch"), "{e}");
    // Crossed curves twist the network.
    let crossed = vec![
        line(corners(0.0, 5.0)[0], corners(10.0, 5.0)[1]),
        line(corners(0.0, 5.0)[1], corners(10.0, 5.0)[0]),
        guides[2].clone(),
        guides[3].clone(),
    ];
    assert!(Shape::boundary(&sections, &crossed).is_err());
    // A single section.
    assert!(Shape::boundary(&sections[..1], &guides).is_err());
}
