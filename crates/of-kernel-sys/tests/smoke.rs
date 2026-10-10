use of_kernel_sys::ffi;

fn frame() -> ffi::Frame {
    ffi::Frame {
        origin: ffi::Vec3::default(),
        x_dir: ffi::Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        normal: ffi::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
    }
}

fn square_with_hole() -> cxx::UniquePtr<ffi::Shape> {
    let line = |a: [f64; 4]| ffi::Curve {
        kind: 0,
        data: a.to_vec(),
    };
    let curves = vec![
        line([0.0, 0.0, 10.0, 0.0]),
        line([10.0, 0.0, 10.0, 10.0]),
        line([10.0, 10.0, 0.0, 10.0]),
        line([0.0, 10.0, 0.0, 0.0]),
        ffi::Curve {
            kind: 2,
            data: vec![5.0, 5.0, 2.0],
        },
    ];
    ffi::make_faces(&frame(), &curves, &[4, 1], &[2]).unwrap()
}

#[test]
fn extrude_profile_with_hole() {
    let face = square_with_hole();
    let mut hist = Vec::new();
    let solid = ffi::prism(
        &face,
        &ffi::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 5.0,
        },
        &mut hist,
    )
    .unwrap();
    assert!(ffi::is_valid(&solid));
    // 4 sides + hole + 2 caps.
    assert_eq!(ffi::count(&solid, 0).unwrap(), 7);
    let props = ffi::mass_props(&solid).unwrap();
    let expected = (100.0 - std::f64::consts::PI * 4.0) * 5.0;
    assert!((props.volume - expected).abs() < 1e-6, "{}", props.volume);
    // Every profile edge generated one side face; plus both caps.
    assert_eq!(
        hist.iter()
            .filter(|h| h.from_kind == 1 && h.rel == 2 && h.to_kind == 0)
            .count(),
        5
    );
    assert_eq!(hist.iter().filter(|h| h.rel == 3 || h.rel == 4).count(), 2);
}

#[test]
fn fillet_cut_and_mesh() {
    let block = ffi::make_box(20.0, 20.0, 20.0).unwrap();
    let mut hist = Vec::new();
    let filleted = ffi::fillet(&block, &[0, 1], 2.0, &mut hist).unwrap();
    assert_eq!(ffi::count(&filleted, 0).unwrap(), 8);
    assert!(hist.iter().any(|h| h.from_kind == 1 && h.rel == 2));

    let tool = ffi::make_box(5.0, 5.0, 40.0).unwrap();
    let mut hist = Vec::new();
    let cut = ffi::boolean(1, &filleted, &tool, &mut hist).unwrap();
    assert!(ffi::is_valid(&cut));
    assert!(hist.iter().any(|h| h.input == 1));

    let mesh = ffi::tessellate(&cut, 0.1, 0.5).unwrap();
    let faces = ffi::count(&cut, 0).unwrap() as usize;
    assert_eq!(mesh.face_tri_start.len(), faces + 1);
    assert!(!mesh.indices.is_empty());
    assert_eq!(mesh.positions.len(), mesh.normals.len());
}

#[test]
fn kernel_errors_become_rust_errors() {
    let block = ffi::make_box(10.0, 10.0, 10.0).unwrap();
    let mut hist = Vec::new();
    assert!(ffi::fillet(&block, &[0], 50.0, &mut hist).is_err());
    assert!(ffi::sub_shape(&block, 0, 99).is_err());
}

#[test]
fn step_round_trip() {
    let block = ffi::make_box(10.0, 20.0, 30.0).unwrap();
    let path = std::env::temp_dir().join("ofk-roundtrip.step");
    let path = path.to_str().unwrap();
    ffi::write_step(&block, path).unwrap();
    let back = ffi::read_step(path).unwrap();
    let v = ffi::mass_props(&back).unwrap().volume;
    assert!((v - 6000.0).abs() < 1e-6);
}
