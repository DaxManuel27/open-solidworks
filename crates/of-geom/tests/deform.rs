//! Scaling and control-point deformation.

use of_geom::{DVec3, Kind, Shape};

fn volume(s: &Shape) -> f64 {
    s.mass_props().unwrap().volume
}

#[test]
fn scale_uniform_and_by_axis() {
    let b = Shape::make_box(10.0, 10.0, 10.0).unwrap();
    let u = b.scale(DVec3::ZERO, DVec3::splat(2.0)).unwrap();
    assert!((volume(&u.shape) - 8000.0).abs() < 1e-6);
    let x = b
        .scale(DVec3::splat(5.0), DVec3::new(2.0, 1.0, 1.0))
        .unwrap();
    assert!((volume(&x.shape) - 2000.0).abs() < 1e-6);
    let bb = x.shape.bbox().unwrap();
    assert!((bb.min.x + 5.0).abs() < 1e-6 && (bb.max.x - 15.0).abs() < 1e-6);
    assert_eq!(
        x.history
            .iter()
            .filter(|h| h.from_kind == Kind::Face)
            .count(),
        6
    );
}

#[test]
fn deforming_by_identity_keeps_the_shape_and_twist_keeps_volume() {
    let b = Shape::make_box(10.0, 10.0, 40.0).unwrap();
    let same = b.deform(2, |p| p).unwrap();
    assert!((volume(&same.shape) - 4000.0).abs() < 1e-6);
    assert_eq!(same.shape.count(Kind::Face), 6);
    // Twist 30° about the box's long axis over its height.
    let c = DVec3::new(5.0, 5.0, 0.0);
    let twisted = b
        .deform(8, |p| {
            let a = 30f64.to_radians() * p.z / 40.0;
            let d = p - c;
            c + DVec3::new(
                d.x * a.cos() - d.y * a.sin(),
                d.x * a.sin() + d.y * a.cos(),
                d.z,
            )
        })
        .unwrap();
    let v = volume(&twisted.shape);
    assert!((v - 4000.0).abs() < 4000.0 * 0.01, "{v}");
    let bb = twisted.shape.bbox().unwrap();
    assert!(bb.max.x > 10.5, "the top corners turned: {bb:?}");
}

#[test]
fn a_displayed_shape_scales_and_deforms() {
    // Display meshes stay on the shape; they mustn't make the result invalid.
    let b = Shape::make_box(40.0, 20.0, 10.0).unwrap();
    b.mesh(0.1, 0.5).unwrap();
    let x = b.scale(DVec3::ZERO, DVec3::new(1.0, 2.0, 3.0)).unwrap();
    assert!((volume(&x.shape) - 48000.0).abs() < 1e-6);
    let bent = b
        .deform(4, |p| p + DVec3::new(0.0, 0.0, p.x * 0.01))
        .unwrap();
    assert!((volume(&bent.shape) - 8000.0).abs() < 1.0);
}
