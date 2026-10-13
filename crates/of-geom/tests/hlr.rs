use of_geom::{
    BoolOp, Curve2, Curve2d, DVec2, DVec3, EdgeClass, HlrQuality, Kind, Plane, Profile,
    ProfileFace, Shape,
};

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

fn disk(center: DVec2, radius: f64) -> Vec<Curve2> {
    vec![Curve2::Circle { center, radius }]
}

/// A 40 x 20 x 10 plate (z up from 0) with a 6 mm hole through it at (10, 10).
fn plate() -> Shape {
    let block = Shape::make_box(40.0, 20.0, 10.0).unwrap();
    let pin = prism(disk(DVec2::new(10.0, 10.0), 3.0), vec![], 10.0);
    block.boolean(BoolOp::Cut, &pin).unwrap().shape
}

fn front() -> Plane {
    Plane::new(DVec3::ZERO, DVec3::X, DVec3::Z)
}

/// Looking down -Y at the plate's long side: x right, z up.
fn side() -> Plane {
    Plane::new(DVec3::ZERO, DVec3::X, DVec3::NEG_Y)
}

#[test]
fn front_view_of_a_plate_shows_its_outline_and_hole() {
    let plate = plate();
    let edges = plate
        .hidden_lines(&front(), HlrQuality::Exact, true, 0.01)
        .unwrap();
    let visible: Vec<_> = edges.iter().filter(|e| e.visible).collect();
    let lines = visible
        .iter()
        .filter(|e| matches!(e.curve, Curve2d::Line(..)))
        .count();
    assert_eq!(lines, 4, "{visible:#?}");
    let arcs: Vec<_> = visible
        .iter()
        .filter_map(|e| match e.curve {
            Curve2d::Arc {
                center,
                radius,
                start,
                end,
            } => Some((center, radius, end - start)),
            _ => None,
        })
        .collect();
    let sweep: f64 = arcs.iter().map(|a| a.2).sum();
    assert!((sweep - std::f64::consts::TAU).abs() < 1e-6, "{arcs:?}");
    for (c, r, _) in &arcs {
        assert!(c.distance(DVec2::new(10.0, 10.0)) < 1e-6 && (r - 3.0).abs() < 1e-6);
    }
    // Every visible model edge is traced, and to an edge on the top (z = 10)
    // face, which is the one nearest the viewer.
    for e in &visible {
        let src = e.source.expect("traced");
        let info = plate.edge_info(src).unwrap();
        assert!((info.mid.z - 10.0).abs() < 1e-6, "{e:?} -> {info:?}");
    }
}

#[test]
fn side_view_shows_hidden_hole_and_classes() {
    let plate = plate();
    let edges = plate
        .hidden_lines(&side(), HlrQuality::Exact, true, 0.01)
        .unwrap();
    // The hole is hidden behind the long face: two vertical silhouette lines
    // at x = 7 and x = 13.
    let hidden: Vec<_> = edges.iter().filter(|e| !e.visible).collect();
    let mut xs: Vec<f64> = hidden
        .iter()
        .filter_map(|e| match e.curve {
            Curve2d::Line(a, b) if (a.x - b.x).abs() < 1e-9 => Some(a.x),
            _ => None,
        })
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    assert_eq!(xs.len(), 2, "{hidden:#?}");
    assert!((xs[0] - 7.0).abs() < 1e-6 && (xs[1] - 13.0).abs() < 1e-6);
    assert!(
        hidden
            .iter()
            .all(|e| e.class == EdgeClass::Outline || e.class == EdgeClass::Sharp)
    );
    // Paper y is model z: the visible outline spans 0..10.
    let ys: Vec<f64> = edges
        .iter()
        .filter(|e| e.visible)
        .flat_map(|e| e.curve.points(0.1))
        .map(|p| p.y)
        .collect();
    let (lo, hi) = ys
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), y| (l.min(*y), h.max(*y)));
    assert!(lo.abs() < 1e-6 && (hi - 10.0).abs() < 1e-6, "{lo} {hi}");
}

#[test]
fn hidden_edges_are_left_out_unless_asked_for() {
    let edges = plate()
        .hidden_lines(&side(), HlrQuality::Exact, false, 0.01)
        .unwrap();
    assert!(edges.iter().all(|e| e.visible));
}

#[test]
fn preview_quality_matches_exact_extents() {
    let plate = plate();
    let iso = Plane::new(
        DVec3::ZERO,
        DVec3::new(1.0, -1.0, 0.0),
        DVec3::new(1.0, 1.0, 1.0),
    );
    let extent = |q| {
        let pts: Vec<DVec2> = plate
            .hidden_lines(&iso, q, false, 0.01)
            .unwrap()
            .iter()
            .flat_map(|e| e.curve.points(0.05))
            .collect();
        pts.iter().fold(
            (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)),
            |(a, b), p| (a.min(*p), b.max(*p)),
        )
    };
    let (a0, a1) = extent(HlrQuality::Exact);
    let (b0, b1) = extent(HlrQuality::Preview);
    assert!(a0.distance(b0) < 0.05 && a1.distance(b1) < 0.05, "{a0} {a1} {b0} {b1}");
    assert!(plate.count(Kind::Edge) > 12);
}

#[test]
fn cylinder_seams_facing_the_viewer_are_not_drawn() {
    // The disk's seam is at angle 0 (x = 5); seen along -X it would be a line
    // down the middle of the cylinder.
    let rod = prism(disk(DVec2::ZERO, 5.0), vec![], 30.0);
    let view = Plane::new(DVec3::ZERO, DVec3::Y, DVec3::X);
    let edges = rod
        .hidden_lines(&view, HlrQuality::Exact, true, 0.01)
        .unwrap();
    for e in &edges {
        for p in e.curve.points(0.1) {
            assert!(
                p.x.abs() > 5.0 - 1e-6 || p.y.abs() < 1e-6 || (p.y - 30.0).abs() < 1e-6,
                "{e:?}"
            );
        }
    }
    let outlines = edges
        .iter()
        .filter(|e| e.visible && e.class == EdgeClass::Outline)
        .count();
    assert_eq!(outlines, 2, "{edges:#?}");
    // The end circles are seen edge-on, as straight lines.
    assert!(
        edges
            .iter()
            .filter(|e| e.class == EdgeClass::Sharp)
            .all(|e| matches!(e.curve, Curve2d::Line(..)))
    );
}
