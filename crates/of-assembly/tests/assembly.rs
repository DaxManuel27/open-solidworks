//! Assembly documents, components, mates and solving on real part files.

mod common;

use common::*;
use of_assembly::*;
use of_model::Ref;

/// A 100×60×10 plate with a Ø10 hole at (30, 30), and a Ø10 × 40 pin.
fn plate_and_pin(dir: &Scratch) -> (std::path::PathBuf, std::path::PathBuf) {
    let plate = save(
        &plate_doc(100.0, 60.0, 10.0, &[(30.0, 30.0, 5.0)]),
        &dir.path("plate.ofp"),
    );
    let pin = save(&shaft_doc(5.0, 40.0), &dir.path("pin.ofp"));
    (plate, pin)
}

#[test]
fn insert_mate_solve_save_and_reopen() {
    let dir = Scratch::new("basic");
    let (plate, pin) = plate_and_pin(&dir);
    let plate_doc = of_model::file::load(&plate).unwrap();
    let pin_doc = of_model::file::load(&pin).unwrap();

    let mut a = Assembly::default();
    let p = a.insert(&plate, None, None).unwrap();
    let start = Pose::new(DVec3::new(70.0, -20.0, 35.0), DQuat::from_rotation_x(0.4));
    let s = a.insert(&pin, Some(start), None).unwrap();
    assert!(
        a.doc.component(&p).unwrap().fixed,
        "first component is fixed"
    );
    assert!(!a.doc.component(&s).unwrap().fixed);

    // Pin in the hole, its bottom face flush with the plate's top.
    a.add_mate(
        None,
        MateKind::Concentric {
            a: r(&p, cylinder_face(&plate_doc, 5.0)),
            b: r(&s, cylinder_face(&pin_doc, 5.0)),
            alignment: Alignment::Closest,
            lock_rotation: false,
        },
    )
    .unwrap();
    let d = a.diagnostics.clone();
    assert!(d.solved, "{d:?}");
    assert_eq!(d.dof[&s], 2, "slides and turns");
    a.add_mate(
        None,
        MateKind::Coincident {
            a: r(&p, plane_face(&plate_doc, DVec3::Z, 10.0)),
            b: r(&s, plane_face(&pin_doc, -DVec3::Z, 0.0)),
            alignment: Alignment::Closest,
        },
    )
    .unwrap();
    assert_eq!(a.diagnostics.dof[&s], 1, "only rotation left");
    let pose: Pose = a.doc.component(&s).unwrap().transform.into();
    let base = pose.point(DVec3::ZERO);
    assert_close(base.x, 30.0, 1e-6);
    assert_close(base.y, 30.0, 1e-6);
    assert_close(base.z, 10.0, 1e-6);
    assert!(pose.vector(DVec3::Z).dot(DVec3::Z) > 1.0 - 1e-9);
    assert_eq!(a.doc.mates[0].name, "Concentric1");
    assert_eq!(a.doc.mates[1].name, "Coincident1");

    // Saving makes component paths relative; reopening restores poses.
    let file = dir.path("sub/pinned.ofa");
    std::fs::create_dir_all(dir.path("sub")).unwrap();
    a.save(&file).unwrap();
    assert_eq!(a.doc.components[0].path, "../plate.ofp");
    let mut b = Assembly::open(&file).unwrap();
    assert_eq!(b.doc.mates, a.doc.mates);
    assert_eq!(b.doc.components.len(), 2);
    b.solve(&[], true);
    assert!(b.diagnostics.solved);
    let again: Pose = b.doc.component(&s).unwrap().transform.into();
    assert!(again.translation.distance(pose.translation) < 1e-6);
    assert_eq!(b.instances().len(), 2);
}

#[test]
fn a_conflicting_mate_is_refused_and_names_the_conflict() {
    let dir = Scratch::new("conflict");
    let (plate, pin) = plate_and_pin(&dir);
    let plate_doc = of_model::file::load(&plate).unwrap();
    let pin_doc = of_model::file::load(&pin).unwrap();
    let mut a = Assembly::default();
    let p = a.insert(&plate, None, None).unwrap();
    let s = a.insert(&pin, None, None).unwrap();
    a.add_mate(
        Some("Seat".into()),
        MateKind::Coincident {
            a: r(&p, plane_face(&plate_doc, DVec3::Z, 10.0)),
            b: r(&s, plane_face(&pin_doc, -DVec3::Z, 0.0)),
            alignment: Alignment::Closest,
        },
    )
    .unwrap();
    let before = a.doc.clone();
    let err = a
        .add_mate(
            None,
            MateKind::Distance {
                a: r(&p, plane_face(&plate_doc, DVec3::Z, 10.0)),
                b: r(&s, plane_face(&pin_doc, -DVec3::Z, 0.0)),
                distance: 5.0,
                flip: false,
                limit: None,
            },
        )
        .unwrap_err();
    assert!(err.contains("Seat"), "{err}");
    assert_eq!(a.doc, before);
}

#[test]
fn missing_files_are_reported_and_relinked() {
    let dir = Scratch::new("missing");
    let (plate, pin) = plate_and_pin(&dir);
    let mut a = Assembly::default();
    a.insert(&plate, None, None).unwrap();
    let s = a.insert(&pin, None, None).unwrap();
    let file = dir.path("asm.ofa");
    a.save(&file).unwrap();
    std::fs::rename(&pin, dir.path("moved.ofp")).unwrap();

    let mut b = Assembly::open(&file).unwrap();
    b.solve(&[], true);
    assert!(b.diagnostics.component_errors[&s].contains("missing"));
    let missing = b
        .instances()
        .iter()
        .filter(|i| matches!(i.loaded, Loaded::Missing(_)))
        .count();
    assert_eq!(missing, 1);

    b.doc.component_mut(&s).unwrap().path = "moved.ofp".into();
    b.solve(&[], true);
    assert!(b.diagnostics.component_errors.is_empty());
}

#[test]
fn part_changes_on_disk_are_picked_up() {
    let dir = Scratch::new("reload");
    let (plate, pin) = plate_and_pin(&dir);
    let mut a = Assembly::default();
    a.insert(&plate, None, None).unwrap();
    let s = a.insert(&pin, None, None).unwrap();
    assert!(a.refresh().is_empty());
    save(&shaft_doc(5.0, 80.0), &pin);
    assert_eq!(a.refresh(), vec![s.clone()]);
    let pin_inst = a
        .instances()
        .into_iter()
        .find(|i| i.key == [s.clone()])
        .unwrap();
    let bb = pin_inst.part().unwrap().shape().unwrap().bbox().unwrap();
    assert_close(bb.max.z, 80.0, 1e-6);
}

#[test]
fn subassemblies_insert_rigid_and_mate_to_their_children() {
    let dir = Scratch::new("sub");
    let (plate, pin) = plate_and_pin(&dir);
    let plate_doc = of_model::file::load(&plate).unwrap();
    let pin_doc = of_model::file::load(&pin).unwrap();

    // Subassembly: a pin standing on the origin.
    let mut sub = Assembly::default();
    let sp = sub.insert(&pin, None, None).unwrap();
    let sub_file = dir.path("pin-sub.ofa");
    sub.save(&sub_file).unwrap();

    let mut a = Assembly::default();
    let p = a.insert(&plate, None, None).unwrap();
    let k = a
        .insert(
            &sub_file,
            Some(Pose::from_translation(DVec3::new(0.0, 0.0, 50.0))),
            None,
        )
        .unwrap();
    let pin_in_sub = CompRef {
        component: vec![k.clone(), sp.clone()],
        entity: cylinder_face(&pin_doc, 5.0),
    };
    a.add_mate(
        None,
        MateKind::Concentric {
            a: r(&p, cylinder_face(&plate_doc, 5.0)),
            b: pin_in_sub,
            alignment: Alignment::Closest,
            lock_rotation: false,
        },
    )
    .unwrap();
    let inst = a.instances();
    let child = inst
        .iter()
        .find(|i| i.key == [k.clone(), sp.clone()])
        .unwrap();
    assert_close(child.pose.translation.x, 30.0, 1e-6);
    assert_close(child.pose.translation.y, 30.0, 1e-6);
    assert_eq!(inst.len(), 3);

    // Inserting an assembly into itself is refused.
    let file = dir.path("top.ofa");
    a.save(&file).unwrap();
    let mut sub2 = Assembly::open(&sub_file).unwrap();
    assert!(
        sub2.insert(&file, None, None)
            .unwrap_err()
            .contains("itself")
    );
}

#[test]
fn reference_planes_of_the_assembly_can_be_mated_to() {
    let dir = Scratch::new("ref");
    let (_, pin) = plate_and_pin(&dir);
    let pin_doc = of_model::file::load(&pin).unwrap();
    let mut a = Assembly::default();
    let s = a
        .insert(
            &pin,
            Some(Pose::from_translation(DVec3::new(5.0, 5.0, 5.0))),
            None,
        )
        .unwrap();
    a.doc.component_mut(&s).unwrap().fixed = false;
    a.add_mate(
        None,
        MateKind::Coincident {
            a: CompRef {
                component: vec![],
                entity: Ref::FrontPlane,
            },
            b: r(&s, plane_face(&pin_doc, -DVec3::Z, 0.0)),
            alignment: Alignment::Closest,
        },
    )
    .unwrap();
    let pose: Pose = a.doc.component(&s).unwrap().transform.into();
    assert_close(pose.point(DVec3::ZERO).z, 0.0, 1e-6);
    assert_eq!(a.diagnostics.dof[&s], 3);
}
