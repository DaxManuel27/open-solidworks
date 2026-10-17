//! Replacing a face of a solid with another surface: move, rotate, offset.

use of_geom::{DAffine3, DVec3, Kind, Shape, SurfaceKind};

fn top(s: &Shape) -> u32 {
    (0..s.count(Kind::Face))
        .find(|i| {
            let f = s.face_info(*i).unwrap();
            f.kind == SurfaceKind::Plane && f.normal.z > 0.99 && f.centroid.z > 9.99
        })
        .unwrap()
}

fn volume(s: &Shape) -> f64 {
    s.mass_props().unwrap().volume
}

fn moved(s: &Shape, face: u32, t: &DAffine3) -> Shape {
    let f = s.sub_shape(Kind::Face, face).unwrap();
    f.transform(t).unwrap().shape.extend_face(100.0).unwrap()
}

#[test]
fn moving_a_face_out_grows_the_neighbours() {
    let b = Shape::make_box(10.0, 10.0, 10.0).unwrap();
    let t = top(&b);
    let up = moved(&b, t, &DAffine3::from_translation(DVec3::Z * 5.0));
    let r = b.replace_face(t, &up).unwrap();
    assert!((volume(&r.result.shape) - 1500.0).abs() < 1e-6);
    // Side faces grew in place rather than gaining strips.
    assert_eq!(r.result.shape.count(Kind::Face), 6);
    assert_eq!(r.new_faces.len(), 1);
    let f = r.result.shape.face_info(r.new_faces[0]).unwrap();
    assert!((f.centroid.z - 15.0).abs() < 1e-6);
    // Every side face is traced from the input.
    let traced = r
        .result
        .history
        .iter()
        .filter(|h| h.from_kind == Kind::Face)
        .count();
    assert!(traced >= 5, "{traced}");
}

#[test]
fn moving_a_face_in_shrinks_the_neighbours() {
    let b = Shape::make_box(10.0, 10.0, 10.0).unwrap();
    let t = top(&b);
    let down = moved(&b, t, &DAffine3::from_translation(DVec3::Z * -3.0));
    let r = b.replace_face(t, &down).unwrap();
    assert!((volume(&r.result.shape) - 700.0).abs() < 1e-6);
    assert_eq!(r.result.shape.count(Kind::Face), 6);
}

#[test]
fn rotating_a_face_tilts_it() {
    let b = Shape::make_box(10.0, 10.0, 10.0).unwrap();
    let t = top(&b);
    let pivot = DVec3::new(5.0, 5.0, 10.0);
    let rot = DAffine3::from_translation(pivot)
        * DAffine3::from_axis_angle(DVec3::X, 5f64.to_radians())
        * DAffine3::from_translation(-pivot);
    let tilted = moved(&b, t, &rot);
    let r = b.replace_face(t, &tilted).unwrap();
    // Tilting about the middle adds as much as it removes.
    assert!((volume(&r.result.shape) - 1000.0).abs() < 1e-6);
    assert_eq!(r.result.shape.count(Kind::Face), 6);
    let f = r.result.shape.face_info(r.new_faces[0]).unwrap();
    assert!((f.normal.angle_between(DVec3::Z) - 5f64.to_radians()).abs() < 1e-9);
}

#[test]
fn offsetting_a_hole_wall_widens_the_hole() {
    let b = Shape::make_box(20.0, 20.0, 10.0).unwrap();
    let hole = {
        // A 6 mm hole through the middle.
        let plane = of_geom::Plane::new(DVec3::new(0.0, 0.0, -1.0), DVec3::X, DVec3::Z);
        let disk = of_geom::Profile {
            plane,
            faces: vec![of_geom::ProfileFace {
                outer: vec![of_geom::Curve2::Circle {
                    center: of_geom::DVec2::new(10.0, 10.0),
                    radius: 3.0,
                }],
                holes: vec![],
            }],
        }
        .to_shape()
        .unwrap();
        disk.prism(DVec3::Z * 12.0).unwrap().shape
    };
    let drilled = b.boolean(of_geom::BoolOp::Cut, &hole).unwrap().shape;
    let wall = (0..drilled.count(Kind::Face))
        .find(|i| drilled.face_info(*i).unwrap().kind == SurfaceKind::Cylinder)
        .unwrap();
    let face = drilled.sub_shape(Kind::Face, wall).unwrap();
    // The wall's outward normal points into the hole: offsetting along it
    // adds material and narrows the hole; against it widens the hole.
    for (offset, radius) in [(1.0, 2.0), (-1.0, 4.0)] {
        let surface = face
            .offset_shape(offset)
            .unwrap()
            .shape
            .extend_face(50.0)
            .unwrap();
        let r = drilled.replace_face(wall, &surface).unwrap();
        let expected = 4000.0 - std::f64::consts::PI * radius * radius * 10.0;
        let v = volume(&r.result.shape);
        assert!((v - expected).abs() < 1e-3, "{v} != {expected}");
        let f = r.result.shape.face_info(r.new_faces[0]).unwrap();
        assert!((f.radius - radius).abs() < 1e-6);
    }
}
