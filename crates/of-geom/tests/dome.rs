use of_geom::{Curve2, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Shape, SurfaceKind};
use std::f64::consts::PI;

fn cylinder(radius: f64, depth: f64, plane: Plane) -> Shape {
    let profile = Profile {
        plane,
        faces: vec![ProfileFace {
            outer: vec![Curve2::Circle {
                center: DVec2::ZERO,
                radius,
            }],
            holes: vec![],
        }],
    };
    profile
        .to_shape()
        .unwrap()
        .prism(plane.normal * depth)
        .unwrap()
        .shape
}

#[test]
fn circular_caps_have_exact_height_volume_attachment_and_face_history() {
    for scale in [0.1, 1.0, 10.0] {
        for normal in [DVec3::Z, DVec3::new(1.0, 2.0, 3.0).normalize()] {
            let radius = 10.0 * scale;
            let depth = 8.0 * scale;
            let plane = Plane::new(DVec3::new(3.0, 4.0, -2.0), DVec3::X, normal);
            let body = cylinder(radius, depth, plane);
            let top = (0..body.count(Kind::Face))
                .find(|&i| {
                    let f = body.face_info(i).unwrap();
                    f.kind == SurfaceKind::Plane && f.normal.dot(normal) > 0.99
                })
                .unwrap();
            for inward in [false, true] {
                let height = 3.0 * scale;
                let result = body.dome(top, height, inward).unwrap();
                assert!(result.shape.is_valid());
                assert_eq!(result.shape.count(Kind::Solid), 1);
                let expected = PI * radius * radius * depth
                    + if inward { -1.0 } else { 1.0 }
                        * PI
                        * height
                        * (3.0 * radius * radius + height * height)
                        / 6.0;
                assert!(
                    (result.shape.mass_props().unwrap().volume - expected).abs() < expected * 1e-7
                );
                let spherical: Vec<_> = (0..result.shape.count(Kind::Face))
                    .filter_map(|i| {
                        let f = result.shape.face_info(i).unwrap();
                        (f.kind == SurfaceKind::Sphere).then_some((i, f))
                    })
                    .collect();
                assert_eq!(spherical.len(), 1);
                let (cap, info) = &spherical[0];
                let direction = if inward { -normal } else { normal };
                let tip = plane.origin + normal * depth + direction * height;
                let probe = Shape::wire3(&[of_geom::Curve3::Line {
                    a: tip,
                    b: tip + direction * scale,
                }])
                .unwrap();
                let cap_face = result.shape.sub_shape(Kind::Face, *cap).unwrap();
                assert!(cap_face.distance(&probe).unwrap().value < scale * 1e-7);
                let spherical_radius = (radius * radius + height * height) / (2.0 * height);
                assert!((info.radius - spherical_radius).abs() < scale * 1e-7);
                assert!(result.history.iter().any(|h| h.from_kind == Kind::Face
                    && h.from == top
                    && h.to_kind == Kind::Face
                    && h.to == *cap
                    && h.rel == of_geom::Relation::Generated));
                // The replacement cap keeps the original circular attachment edge.
                assert!((0..result.shape.count(Kind::Edge)).any(|i| {
                    let e = result.shape.edge_info(i).unwrap();
                    e.kind == of_geom::CurveKind::Circle
                        && (e.radius - radius).abs() < scale * 1e-7
                        && (e.center - (plane.origin + normal * depth)).length() < scale * 1e-7
                }));
            }
        }
    }
}

#[test]
fn dome_rejects_non_disks_invalid_height_and_insufficient_support() {
    // A face with a hole has no dome.
    let ring = Profile {
        plane: Plane::front(),
        faces: vec![ProfileFace {
            outer: vec![Curve2::Circle {
                center: DVec2::ZERO,
                radius: 10.0,
            }],
            holes: vec![vec![Curve2::Circle {
                center: DVec2::ZERO,
                radius: 4.0,
            }]],
        }],
    }
    .to_shape()
    .unwrap()
    .prism(DVec3::Z * 2.0)
    .unwrap()
    .shape;
    let top = (0..ring.count(Kind::Face))
        .find(|&i| ring.face_info(i).unwrap().normal.z > 0.99)
        .unwrap();
    assert!(
        ring.dome(top, 2.0, false)
            .err()
            .unwrap()
            .0
            .contains("holes")
    );
    let body = cylinder(10.0, 2.0, Plane::front());
    let top = (0..body.count(Kind::Face))
        .find(|&i| body.face_info(i).unwrap().normal.z > 0.99)
        .unwrap();
    for h in [0.0, -1.0, f64::NAN, 11.0] {
        assert!(body.dome(top, h, false).is_err());
    }
    assert!(body.dome(top, 3.0, true).is_err());
}

#[test]
fn elliptical_domes_have_ellipsoid_volume_at_any_height() {
    let (radius, depth) = (10.0, 8.0);
    let plane = Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z);
    let body = cylinder(radius, depth, plane);
    let top = (0..body.count(Kind::Face))
        .find(|&i| {
            let f = body.face_info(i).unwrap();
            f.kind == SurfaceKind::Plane && f.normal.z > 0.99
        })
        .unwrap();
    let base = body.mass_props().unwrap().volume;
    for height in [4.0, 10.0, 15.0] {
        let r = body.dome_elliptical(top, height, false).unwrap();
        assert!(r.shape.is_valid());
        let added = r.shape.mass_props().unwrap().volume - base;
        let expected = 2.0 / 3.0 * PI * radius * radius * height;
        assert!((added / expected - 1.0).abs() < 1e-6, "{height}: {added}");
        // The cap leaves the rim along the axis: it never bulges past the cylinder.
        let b = r.shape.bbox().unwrap();
        assert!(b.max.x < radius + 1e-5 && (b.max.z - depth - height).abs() < 1e-5);
    }
    let r = body.dome_elliptical(top, 4.0, true).unwrap();
    let removed = base - r.shape.mass_props().unwrap().volume;
    assert!((removed / (2.0 / 3.0 * PI * 400.0) - 1.0).abs() < 1e-6);
    // Deeper than the material fails.
    assert!(body.dome_elliptical(top, 9.0, true).is_err());
}

/// Distance from the shape to a point.
fn reach(shape: &Shape, point: DVec3) -> f64 {
    let size = 1e-6;
    let probe = Shape::make_box(size, size, size)
        .unwrap()
        .transform(&of_geom::DAffine3::from_translation(
            point - DVec3::splat(size / 2.0),
        ))
        .unwrap()
        .shape;
    shape.distance(&probe).unwrap().value
}

fn top_of(body: &Shape) -> u32 {
    (0..body.count(Kind::Face))
        .find(|&i| {
            let f = body.face_info(i).unwrap();
            f.kind == SurfaceKind::Plane && f.normal.z > 0.99
        })
        .unwrap()
}

#[test]
fn noncircular_domes_fill_the_face_up_to_the_apex() {
    let body = Shape::make_box(10.0, 10.0, 10.0).unwrap();
    let top = top_of(&body);
    let base = body.mass_props().unwrap().volume;
    let mut previous = 0.0;
    for h in [1.0, 2.0, 4.0] {
        let r = body.dome(top, h, false).unwrap();
        assert!(r.shape.is_valid());
        assert_eq!(r.shape.count(Kind::Solid), 1);
        // The apex reaches the requested height and the dome stays over the face.
        let apex = reach(&r.shape, DVec3::new(5.0, 5.0, 10.0 + h + 0.01));
        assert!((apex - 0.01).abs() < 1e-4, "{h}: {apex}");
        // Next to a side, the dome comes no further out than the side.
        let side = reach(&r.shape, DVec3::new(10.01, 5.0, 10.0 + h / 4.0));
        assert!(side > 0.01 - 1e-6, "{h}: {side}");
        let added = r.shape.mass_props().unwrap().volume - base;
        // More than the pyramid to the apex, less than the prism.
        assert!(added > 100.0 * h / 3.0 && added < 100.0 * h, "{h}: {added}");
        assert!(added > previous);
        previous = added;
        // Inward removes the mirror image.
        let r = body.dome(top, h, true).unwrap();
        let removed = base - r.shape.mass_props().unwrap().volume;
        assert!((removed / added - 1.0).abs() < 1e-4, "{removed} vs {added}");
        // The dome replaces the top face: four sides, the bottom and the dome.
        assert_eq!(r.shape.count(Kind::Face), 6);
    }
    // A square has corners, so the dome can't be tangent to all of its sides.
    assert!(
        body.dome_elliptical(top, 2.0, false)
            .err()
            .unwrap()
            .0
            .contains("corners")
    );
}

#[test]
fn tangent_dome_on_a_smooth_noncircular_face_stays_over_it() {
    // A 20 x 8 slot with round ends.
    let slot = Profile {
        plane: Plane::front(),
        faces: vec![ProfileFace {
            outer: vec![
                Curve2::Line {
                    a: DVec2::new(-6.0, -4.0),
                    b: DVec2::new(6.0, -4.0),
                },
                Curve2::Arc {
                    center: DVec2::new(6.0, 0.0),
                    start: DVec2::new(6.0, -4.0),
                    end: DVec2::new(6.0, 4.0),
                },
                Curve2::Line {
                    a: DVec2::new(6.0, 4.0),
                    b: DVec2::new(-6.0, 4.0),
                },
                Curve2::Arc {
                    center: DVec2::new(-6.0, 0.0),
                    start: DVec2::new(-6.0, 4.0),
                    end: DVec2::new(-6.0, -4.0),
                },
            ],
            holes: vec![],
        }],
    }
    .to_shape()
    .unwrap()
    .prism(DVec3::Z * 5.0)
    .unwrap()
    .shape;
    let top = top_of(&slot);
    let base = slot.mass_props().unwrap().volume;
    let r = slot.dome_elliptical(top, 3.0, false).unwrap();
    assert!(r.shape.is_valid());
    assert!((reach(&r.shape, DVec3::new(0.0, 0.0, 8.01)) - 0.01).abs() < 1e-4);
    // Tangent to the vertical sides, the dome rises nearly straight up from
    // them and never bulges past them.
    let beside = |r: &Shape| reach(r, DVec3::new(0.0, 4.01, 5.1));
    let rising = beside(&r.shape);
    assert!(rising > 0.01 - 1e-6 && rising < 0.03, "{rising}");
    assert!(reach(&r.shape, DVec3::new(10.01, 0.0, 5.1)) > 0.01 - 1e-6);
    let tangent = r.shape.mass_props().unwrap().volume - base;
    // A positional dome of the same height is flatter at the rim.
    let r = slot.dome(top, 3.0, false).unwrap();
    let positional = r.shape.mass_props().unwrap().volume - base;
    assert!(tangent > positional, "{tangent} vs {positional}");
    assert!(beside(&r.shape) > 2.0 * rising);
}
