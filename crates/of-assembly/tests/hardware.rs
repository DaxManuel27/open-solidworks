//! Generated hardware builds, has the standard's dimensions and mates.

mod common;

use std::f64::consts::PI;

use common::*;
use of_assembly::hardware::{self, HardwareKind as K, HardwareSpec};
use of_assembly::*;
use of_model::Model;

fn build(spec: &HardwareSpec) -> (Model, of_model::geom::BBox, f64) {
    let (doc, _) = hardware::generate(spec).unwrap();
    let mut m = Model::new(doc);
    m.rebuild();
    for s in m.statuses() {
        assert!(s.error.is_none(), "{spec:?}: {:?}", s.error);
    }
    let shape = &m.state().body.as_ref().unwrap().shape;
    let b = shape.bbox().unwrap();
    let v = shape.mass_props().unwrap().volume;
    (m, b, v)
}

#[test]
fn every_catalog_item_builds() {
    for entry in hardware::catalog() {
        for size in &entry.sizes {
            let length = entry.lengths.iter().copied().find(|l| *l >= 20.0);
            let spec = HardwareSpec {
                kind: entry.kind,
                size: size.to_string(),
                length,
            };
            let (_, b, v) = build(&spec);
            assert!(v > 0.0, "{spec:?}");
            assert!(
                b.max.z >= -1e-9 && b.min.z <= 1e-9,
                "{spec:?} bearing face at z = 0"
            );
        }
    }
}

#[test]
fn socket_head_cap_screw_dimensions() {
    let spec = HardwareSpec {
        kind: K::SocketHeadCapScrew,
        size: "M6".into(),
        length: Some(20.0),
    };
    let (m, b, v) = build(&spec);
    assert!((b.max.z - 6.0).abs() < 1e-6 && (b.min.z + 20.0).abs() < 1e-6);
    assert!((b.max.x - 5.0).abs() < 1e-6, "head Ø10");
    // Hexagon across flats s has area √3/2 · s².
    let socket = 3f64.sqrt() / 2.0 * 25.0 * 3.0;
    let expected = PI * 25.0 * 6.0 + PI * 9.0 * 20.0 - socket;
    assert!((v - expected).abs() < 1e-3, "{v} vs {expected}");
    assert_eq!(m.state().cosmetic_threads.len(), 1);
    let props = &m.doc.properties;
    assert_eq!(props["PartNo"].value, "ISO 4762 M6x20");
    assert!(
        hardware::generate(&HardwareSpec {
            length: None,
            ..spec.clone()
        })
        .is_err()
    );
    assert!(
        hardware::generate(&HardwareSpec {
            size: "M7".into(),
            ..spec
        })
        .is_err()
    );
}

#[test]
fn library_parts_are_reused_and_mate_into_holes() {
    let dir = Scratch::new("hardware");
    let lib = dir.path("Hardware");
    let spec = HardwareSpec {
        kind: K::SocketHeadCapScrew,
        size: "M5".into(),
        length: Some(16.0),
    };
    let f = hardware::library_part(&lib, &spec).unwrap();
    let stamp = std::fs::metadata(&f).unwrap().modified().unwrap();
    assert_eq!(hardware::library_part(&lib, &spec).unwrap(), f);
    assert_eq!(std::fs::metadata(&f).unwrap().modified().unwrap(), stamp);

    let plate = plate_doc(60.0, 40.0, 10.0, &[(20.0, 20.0, 2.75)]);
    let pf = save(&plate, &dir.path("plate.ofp"));
    let screw = of_model::file::load(&f).unwrap();
    let mut a = Assembly::default();
    let p = a.insert(&pf, None, None).unwrap();
    let s = a
        .insert(
            &f,
            Some(Pose::from_translation(DVec3::new(5.0, 5.0, 30.0))),
            None,
        )
        .unwrap();
    let shank = {
        let mut m = Model::new(screw.clone());
        m.rebuild();
        let st = m.state();
        let shape = &st.body.as_ref().unwrap().shape;
        (0..shape.count(of_model::geom::Kind::Face))
            .find_map(|i| {
                let fi = shape.face_info(i).unwrap();
                (fi.kind == of_model::geom::SurfaceKind::Cylinder && (fi.radius - 2.5).abs() < 1e-9)
                    .then(|| st.ref_for(of_model::geom::Kind::Face, i).unwrap())
            })
            .unwrap()
    };
    a.add_mate(
        None,
        MateKind::Concentric {
            a: r(&p, cylinder_face(&plate, 2.75)),
            b: r(&s, shank),
            alignment: Alignment::Closest,
            lock_rotation: false,
        },
    )
    .unwrap();
    a.add_mate(
        None,
        MateKind::Coincident {
            a: r(&p, plane_face(&plate, DVec3::Z, 10.0)),
            b: r(&s, plane_face(&screw, -DVec3::Z, 0.0)),
            alignment: Alignment::Closest,
        },
    )
    .unwrap();
    let t: Pose = a.doc.component(&s).unwrap().transform.into();
    assert!((t.point(DVec3::ZERO) - DVec3::new(20.0, 20.0, 10.0)).length() < 1e-6);
    let rows = bom::bom(&mut a, bom::BomType::TopLevel);
    assert!(rows.iter().any(|r| r.part_number == "ISO 4762 M5x16"));
}
