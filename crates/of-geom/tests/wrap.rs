use of_geom::{
    BoolOp, Curve2, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Shape, SurfaceKind,
};

fn cylinder(radius: f64, height: f64) -> Shape {
    Profile {
        plane: Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z),
        faces: vec![ProfileFace {
            outer: vec![Curve2::Circle {
                center: DVec2::ZERO,
                radius,
            }],
            holes: vec![],
        }],
    }
    .to_shape()
    .unwrap()
    .prism(DVec3::Z * height)
    .unwrap()
    .shape
}

fn side(body: &Shape) -> u32 {
    (0..body.count(Kind::Face))
        .find(|&i| body.face_info(i).unwrap().kind == SurfaceKind::Cylinder)
        .unwrap()
}

fn rect(x0: f64, y0: f64, w: f64, h: f64) -> Vec<Curve2> {
    let p = [
        DVec2::new(x0, y0),
        DVec2::new(x0 + w, y0),
        DVec2::new(x0 + w, y0 + h),
        DVec2::new(x0, y0 + h),
    ];
    (0..4)
        .map(|i| Curve2::Line {
            a: p[i],
            b: p[(i + 1) % 4],
        })
        .collect()
}

fn volume(s: &Shape) -> f64 {
    s.mass_props().unwrap().volume
}

/// A plane tangent to the cylinder at angle `theta`, whose local x runs around the
/// axis and local y runs along it.
fn tangent_plane(radius: f64, theta: f64) -> Plane {
    let n = DVec3::new(theta.cos(), theta.sin(), 0.0);
    Plane::new(n * radius, DVec3::Z.cross(n), n)
}

#[test]
fn emboss_and_deboss_rectangles_match_annular_sector_volumes() {
    let (r, w, h, d) = (10.0, 5.0, 4.0, 1.0);
    let body = cylinder(r, 40.0);
    let face = side(&body);
    // theta 0 straddles the cylinder's parametric seam; pi/2 does not.
    for theta in [0.0, std::f64::consts::FRAC_PI_2, 2.5] {
        let profile = Profile {
            plane: tangent_plane(r, theta),
            faces: vec![ProfileFace {
                outer: rect(-w / 2.0, 10.0, w, h),
                holes: vec![],
            }],
        };
        let faces = profile.wrap_onto(&body, face).unwrap();
        let sector = |r0: f64, r1: f64| 0.5 * (w / r) * (r1 * r1 - r0 * r0) * h;
        let emboss = faces.wrap_solid(d, true).unwrap();
        assert!(emboss.is_valid());
        assert!((volume(&emboss) - sector(r, r + d)).abs() < 1e-6, "{theta}");
        let deboss = faces.wrap_solid(d, false).unwrap();
        assert!(
            (volume(&deboss) - sector(r - d, r)).abs() < 1e-6,
            "{theta}: {} != {}",
            volume(&deboss),
            sector(r - d, r)
        );
        let cut = body.boolean(BoolOp::Cut, &deboss).unwrap().shape;
        assert!(cut.is_valid());
        let full = std::f64::consts::PI * r * r * 40.0;
        assert!((volume(&cut) - (full - sector(r - d, r))).abs() < 1e-5);
        // The wrapped region sits where the plane touches the cylinder.
        let b = emboss.bbox().unwrap();
        let mid = (b.min + b.max) / 2.0;
        let toward = DVec3::new(theta.cos(), theta.sin(), 0.0);
        assert!(DVec3::new(mid.x, mid.y, 0.0).normalize().dot(toward) > 0.99);
        assert!((b.min.z - 10.0).abs() < 1e-6 && (b.max.z - 14.0).abs() < 1e-6);
    }
}

#[test]
fn circles_and_holes_wrap_exactly() {
    let (r, d) = (10.0, 0.5);
    let body = cylinder(r, 40.0);
    let profile = Profile {
        plane: tangent_plane(r, 1.0),
        faces: vec![ProfileFace {
            outer: rect(-4.0, 5.0, 8.0, 8.0),
            holes: vec![vec![Curve2::Circle {
                center: DVec2::new(0.0, 9.0),
                radius: 2.0,
            }]],
        }],
    };
    let faces = profile.wrap_onto(&body, side(&body)).unwrap();
    let solid = faces.wrap_solid(d, true).unwrap();
    // A wrapped area A thickened outward by d holds A * (r + d/2) / r * d.
    let area = 64.0 - std::f64::consts::PI * 4.0;
    let expected = area * (r + d / 2.0) / r * d;
    assert!(
        (volume(&solid) - expected).abs() < 1e-6,
        "{}",
        volume(&solid)
    );
}

#[test]
fn wrap_rejects_bad_targets() {
    let r = 10.0;
    let body = cylinder(r, 40.0);
    let face = side(&body);
    let square = |plane| Profile {
        plane,
        faces: vec![ProfileFace {
            outer: rect(-2.0, 10.0, 4.0, 4.0),
            holes: vec![],
        }],
    };
    // Plane not parallel to the axis.
    let tilted = Plane::new(
        DVec3::X * r,
        DVec3::Y,
        DVec3::new(1.0, 0.0, 0.3).normalize(),
    );
    assert!(square(tilted).wrap_onto(&body, face).is_err());
    // Beyond the cylinder's end.
    let high = Profile {
        plane: tangent_plane(r, 0.0),
        faces: vec![ProfileFace {
            outer: rect(-2.0, 38.0, 4.0, 4.0),
            holes: vec![],
        }],
    };
    assert!(high.wrap_onto(&body, face).is_err());
    // Wider than the circumference.
    let wide = Profile {
        plane: tangent_plane(r, 0.0),
        faces: vec![ProfileFace {
            outer: rect(-40.0, 10.0, 80.0, 4.0),
            holes: vec![],
        }],
    };
    assert!(wide.wrap_onto(&body, face).is_err());
    // A planar face.
    let top = (0..body.count(Kind::Face))
        .find(|&i| body.face_info(i).unwrap().kind == SurfaceKind::Plane)
        .unwrap();
    assert!(square(tangent_plane(r, 0.0)).wrap_onto(&body, top).is_err());
    // Deboss as deep as the radius.
    let faces = square(tangent_plane(r, 0.0))
        .wrap_onto(&body, face)
        .unwrap();
    assert!(faces.wrap_solid(r, false).is_err());
}

/// A cone narrowing from radius 10 at z = 0 to 5 at z = 20 (apex at z = 40).
fn cone() -> Shape {
    let curves = vec![
        Curve2::Line {
            a: DVec2::new(0.0, 0.0),
            b: DVec2::new(10.0, 0.0),
        },
        Curve2::Line {
            a: DVec2::new(10.0, 0.0),
            b: DVec2::new(5.0, 20.0),
        },
        Curve2::Line {
            a: DVec2::new(5.0, 20.0),
            b: DVec2::new(0.0, 20.0),
        },
        Curve2::Line {
            a: DVec2::new(0.0, 20.0),
            b: DVec2::new(0.0, 0.0),
        },
    ];
    Profile {
        plane: Plane::new(DVec3::ZERO, DVec3::X, -DVec3::Y),
        faces: vec![ProfileFace {
            outer: curves,
            holes: vec![],
        }],
    }
    .to_shape()
    .unwrap()
    .revolve(DVec3::ZERO, DVec3::Z, std::f64::consts::TAU)
    .unwrap()
    .shape
}

#[test]
fn cone_wrap_unrolls_about_the_apex() {
    let body = cone();
    let face = (0..body.count(Kind::Face))
        .find(|&i| body.face_info(i).unwrap().kind == SurfaceKind::Cone)
        .unwrap();
    let apex = DVec3::Z * 40.0;
    let sin = 10.0 / 1700f64.sqrt();
    let cos = 40.0 / 1700f64.sqrt();
    for theta in [0.0, std::f64::consts::FRAC_PI_2, 2.5] {
        let er = DVec3::new(f64::cos(theta), f64::sin(theta), 0.0);
        let et = DVec3::Z.cross(er);
        // The cone's line at theta, from the apex down to the base.
        let line = er * sin - DVec3::Z * cos;
        let plane = Plane::new(apex, et, et.cross(line));
        assert!((plane.normal.cross(et) - line).length() < 1e-12);
        // An annular sector about the apex in the development: radii 25..35,
        // angles ±0.2 from the line.
        let (r0, r1, half) = (25.0, 35.0, 0.2);
        let at = |r: f64, psi: f64| DVec2::new(r * psi.sin(), r * psi.cos());
        let outer = vec![
            Curve2::Arc {
                center: DVec2::ZERO,
                start: at(r1, half),
                end: at(r1, -half),
            },
            Curve2::Line {
                a: at(r1, -half),
                b: at(r0, -half),
            },
            Curve2::Arc {
                center: DVec2::ZERO,
                start: at(r0, half),
                end: at(r0, -half),
            },
            Curve2::Line {
                a: at(r0, half),
                b: at(r1, half),
            },
        ];
        let profile = Profile {
            plane,
            faces: vec![ProfileFace {
                outer,
                holes: vec![],
            }],
        };
        let faces = profile.wrap_onto(&body, face).unwrap();
        // Area is kept by the development; the offset adds d^2/2 times the
        // integral of the circumferential curvature cos(a) / (rho sin(a)).
        let area = 0.5 * (r1 * r1 - r0 * r0) * 2.0 * half;
        let curvature = cos / sin * (r1 - r0) * 2.0 * half;
        let d = 1.0;
        let emboss = faces.wrap_solid(d, true).unwrap();
        assert!(emboss.is_valid());
        let expected = d * area + 0.5 * d * d * curvature;
        assert!(
            (volume(&emboss) / expected - 1.0).abs() < 1e-6,
            "{theta}: {} vs {expected}",
            volume(&emboss)
        );
        let deboss = faces.wrap_solid(d, false).unwrap();
        let expected = d * area - 0.5 * d * d * curvature;
        assert!(
            (volume(&deboss) / expected - 1.0).abs() < 1e-6,
            "{theta}: {} vs {expected}",
            volume(&deboss)
        );
        // The wrapped outline lies on the cone: corners at the right slant distance.
        for i in 0..faces.count(Kind::Vertex) {
            let p = faces.vertex_point(i).unwrap();
            let rho = (p - apex).length();
            assert!((rho - r0).abs() < 1e-6 || (rho - r1).abs() < 1e-6, "{rho}");
        }
    }
    // A plane parallel to the axis is not tangent to the cone.
    let profile = Profile {
        plane: tangent_plane(10.0, 0.0),
        faces: vec![ProfileFace {
            outer: rect(-1.0, 5.0, 2.0, 2.0),
            holes: vec![],
        }],
    };
    assert!(profile.wrap_onto(&body, face).is_err());
}

fn ball(radius: f64) -> Shape {
    let half = vec![
        Curve2::Arc {
            center: DVec2::ZERO,
            start: DVec2::new(0.0, -radius),
            end: DVec2::new(0.0, radius),
        },
        Curve2::Line {
            a: DVec2::new(0.0, radius),
            b: DVec2::new(0.0, -radius),
        },
    ];
    Profile {
        plane: Plane::new(DVec3::ZERO, DVec3::X, -DVec3::Y),
        faces: vec![ProfileFace {
            outer: half,
            holes: vec![],
        }],
    }
    .to_shape()
    .unwrap()
    .revolve(DVec3::ZERO, DVec3::Z, std::f64::consts::TAU)
    .unwrap()
    .shape
}

#[test]
fn freeform_wrap_follows_geodesics_and_limits_distortion() {
    let r = 25.0;
    let body = ball(r);
    let face = (0..body.count(Kind::Face))
        .max_by(|a, b| {
            body.face_info(*a)
                .unwrap()
                .area
                .total_cmp(&body.face_info(*b).unwrap().area)
        })
        .unwrap();
    let disk = |rho: f64, theta: f64| {
        let n = DVec3::new(theta.cos(), theta.sin(), 0.0);
        Profile {
            plane: Plane::new(n * r, DVec3::Z.cross(n), n),
            faces: vec![ProfileFace {
                outer: vec![Curve2::Circle {
                    center: DVec2::ZERO,
                    radius: rho,
                }],
                holes: vec![],
            }],
        }
    };
    // A geodesic disk of radius rho is the cap of angular radius rho / r.
    let (rho, d) = (5.0, 1.0);
    let cap = 1.0 - (rho / r).cos();
    for theta in [0.0, 2.0] {
        let faces = disk(rho, theta).wrap_onto(&body, face).unwrap();
        let emboss = faces.wrap_solid(d, true).unwrap();
        assert!(emboss.is_valid());
        let expected = 2.0 * std::f64::consts::PI / 3.0 * ((r + d).powi(3) - r.powi(3)) * cap;
        assert!(
            (volume(&emboss) / expected - 1.0).abs() < 1e-6,
            "{theta}: {} vs {expected}",
            volume(&emboss)
        );
        let deboss = faces.wrap_solid(d, false).unwrap();
        let expected = 2.0 * std::f64::consts::PI / 3.0 * (r.powi(3) - (r - d).powi(3)) * cap;
        assert!(
            (volume(&deboss) / expected - 1.0).abs() < 1e-6,
            "{}",
            volume(&deboss)
        );
        // Emboss adds outside the ball, deboss stays inside it.
        let added = body.boolean(BoolOp::Fuse, &emboss).unwrap().shape;
        assert!((volume(&added) - volume(&body) - volume(&emboss)).abs() < 1e-4);
        let removed = body.boolean(BoolOp::Cut, &deboss).unwrap().shape;
        assert!((volume(&body) - volume(&removed) - volume(&deboss)).abs() < 1e-4);
    }
    // A disk twice as large would stretch lengths by about 2.6%.
    let err = disk(10.0, 0.0).wrap_onto(&body, face).unwrap_err();
    assert!(err.to_string().contains("stretch"), "{err}");
    // The sketch plane must touch the face under its origin.
    let tilted = Profile {
        plane: Plane::new(DVec3::X * r, DVec3::Y, DVec3::new(1.0, 0.0, 0.3)),
        faces: disk(rho, 0.0).faces,
    };
    assert!(tilted.wrap_onto(&body, face).is_err());
}
