//! Patterns, mirrors, flexible subassemblies, explode, BOM, mass,
//! interference and exchange.

mod common;

use std::f64::consts::TAU;

use common::*;
use of_assembly::analysis::{self, InterferenceOptions};
use of_assembly::bom::{self, BomType};
use of_assembly::*;
use of_model::Ref;
use of_model::properties::Property;

fn pose_of(a: &Assembly, key: &str) -> Pose {
    a.doc.component(key).unwrap().transform.into()
}

fn assembly_ref(entity: Ref) -> CompRef {
    CompRef {
        component: vec![],
        entity,
    }
}

/// A plate with a pin in its hole at (30, 30).
fn pinned(
    dir: &Scratch,
) -> (
    Assembly,
    String,
    String,
    of_model::Document,
    of_model::Document,
) {
    let plate = plate_doc(200.0, 60.0, 10.0, &[(30.0, 30.0, 5.0)]);
    let pin = shaft_doc(5.0, 40.0);
    let pf = save(&plate, &dir.path("plate.ofp"));
    let sf = save(&pin, &dir.path("pin.ofp"));
    let mut a = Assembly::default();
    let p = a.insert(&pf, None, None).unwrap();
    let s = a
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(20.0, 20.0, 20.0))),
            None,
        )
        .unwrap();
    a.add_mate(
        None,
        MateKind::Concentric {
            a: r(&p, cylinder_face(&plate, 5.0)),
            b: r(&s, cylinder_face(&pin, 5.0)),
            alignment: Alignment::Closest,
            lock_rotation: false,
        },
    )
    .unwrap();
    a.add_mate(
        None,
        MateKind::Coincident {
            a: r(&p, plane_face(&plate, DVec3::Z, 10.0)),
            b: r(&s, plane_face(&pin, -DVec3::Z, 0.0)),
            alignment: Alignment::Closest,
        },
    )
    .unwrap();
    (a, p, s, plate, pin)
}

#[test]
fn linear_circular_and_pattern_driven_component_patterns() {
    let dir = Scratch::new("patterns");
    let (mut a, _, s, _, _) = pinned(&dir);
    let id = a.doc.take_id();
    a.doc.patterns.push(Pattern {
        id,
        name: "LocalLPattern1".into(),
        kind: PatternKind::Linear {
            direction1: PatternDirection {
                reference: assembly_ref(Ref::RightPlane),
                spacing: 40.0,
                count: 4,
                reverse: false,
            },
            direction2: None,
        },
        seeds: vec![s.clone()],
        skip: vec![2],
        suppressed: false,
    });
    let inst = a.instances();
    let copies: Vec<_> = inst.iter().filter(|i| i.derived == Some(id)).collect();
    assert_eq!(
        copies.len(),
        2,
        "four instances minus the seed and one skipped"
    );
    let xs: Vec<f64> = copies.iter().map(|i| i.pose.point(DVec3::ZERO).x).collect();
    assert!(xs.contains(&70.0) && xs.contains(&150.0), "{xs:?}");
    // Moving the seed moves the instances.
    a.doc.component_mut(&s).unwrap().transform =
        Pose::from_translation(DVec3::new(31.0, 30.0, 10.0)).into();
    let moved = a.instances();
    assert!(
        moved
            .iter()
            .any(|i| (i.pose.translation.x - 71.0).abs() < 1e-9)
    );

    // Circular: 6 about the assembly's Z axis through the origin.
    a.doc.patterns.clear();
    a.doc.reference.add(of_model::FeatureKind::Axis(
        of_model::AxisDef::PointAndFace {
            point: Ref::Origin,
            face: Ref::FrontPlane,
        },
    ));
    let axis = a.doc.reference.features.last().unwrap().id;
    let id = a.doc.take_id();
    a.doc.patterns.push(Pattern {
        id,
        name: "LocalCirPattern1".into(),
        kind: PatternKind::Circular {
            axis: assembly_ref(Ref::Feature { id: axis }),
            angle: TAU,
            count: 6,
            equal_spacing: true,
            reverse: false,
        },
        seeds: vec![s.clone()],
        skip: vec![],
        suppressed: false,
    });
    let inst = a.instances();
    let ring: Vec<_> = inst.iter().filter(|i| i.derived == Some(id)).collect();
    assert_eq!(ring.len(), 5);
    let r0 = pose_of(&a, &s).translation.truncate().length();
    for i in &ring {
        let r = i.pose.translation.truncate().length();
        assert!((r - r0).abs() < 1e-6);
    }

    // Pattern driven: pins follow a linear hole pattern in a plate.
    let dir = Scratch::new("driven");
    let (plate, pattern) = patterned_plate_doc(20.0, 30.0, 5.0, 50.0, 3);
    let pin = shaft_doc(5.0, 40.0);
    let pf = save(&plate, &dir.path("holes.ofp"));
    let sf = save(&pin, &dir.path("pin.ofp"));
    let mut b = Assembly::default();
    let p = b.insert(&pf, None, None).unwrap();
    let s = b
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(20.0, 30.0, 10.0))),
            None,
        )
        .unwrap();
    let id = b.doc.take_id();
    b.doc.patterns.push(Pattern {
        id,
        name: "DerivedLPattern1".into(),
        kind: PatternKind::PatternDriven {
            feature: CompRef {
                component: vec![p.clone()],
                entity: Ref::Feature { id: pattern },
            },
        },
        seeds: vec![s.clone()],
        skip: vec![],
        suppressed: false,
    });
    let inst = b.instances();
    let mut xs: Vec<f64> = inst
        .iter()
        .filter(|i| i.derived == Some(id))
        .map(|i| i.pose.translation.x)
        .collect();
    xs.sort_by(f64::total_cmp);
    assert_eq!(xs, vec![70.0, 120.0]);
}

#[test]
fn mirror_components_copies_and_opposite_hands() {
    let dir = Scratch::new("mirror");
    // An L bracket is not symmetric: 30 × 10 × 10 with a 10 × 20 × 10 upright.
    let mut bracket = block_doc(30.0, 10.0, 10.0);
    {
        let mut sk = of_model::sketch::Sketch::new();
        of_model::sketch::tools::corner_rectangle(
            &mut sk,
            of_model::geom::DVec2::new(0.0, 0.0),
            of_model::geom::DVec2::new(10.0, 10.0),
        );
        let s = bracket.add(of_model::FeatureKind::Sketch(of_model::SketchFeature {
            plane: Ref::FrontPlane,
            sketch: sk,
        }));
        bracket.add(of_model::FeatureKind::Extrude(of_model::Extrude {
            sketch: s,
            cut: false,
            start_offset: 0.0,
            end1: of_model::EndCondition::Blind { depth: 30.0 },
            reverse: false,
            end2: None,
            thin: None,
            flip_side_to_cut: false,
            merge: true,
        }));
    }
    let bf = save(&bracket, &dir.path("bracket.ofp"));
    let mut a = Assembly::default();
    a.save(&dir.path("mirror.ofa")).unwrap();
    let k = a
        .insert(
            &bf,
            Some(Pose::from_translation(DVec3::new(20.0, 5.0, 0.0))),
            None,
        )
        .unwrap();
    let id = a.doc.take_id();
    a.doc.mirrors.push(MirrorComponents {
        id,
        name: "MirrorComponent1".into(),
        plane: assembly_ref(Ref::RightPlane),
        components: vec![MirrorItem {
            component: k.clone(),
            opposite_hand: Some("bracket-left.ofp".into()),
        }],
        position: MirrorPosition::BoundingBox,
        suppressed: false,
    });
    let inst = a.instances();
    let left = inst.iter().find(|i| i.derived == Some(id)).unwrap();
    assert!(dir.path("bracket-left.ofp").exists());
    let shape = analysis::world_shape(left).unwrap();
    let orig = analysis::world_shape(inst.iter().find(|i| i.key == [k.clone()]).unwrap()).unwrap();
    let (bl, bo) = (shape.bbox().unwrap(), orig.bbox().unwrap());
    assert!((bl.min.x + bo.max.x).abs() < 1e-6 && (bl.max.x + bo.min.x).abs() < 1e-6);
    assert!((bl.min.y - bo.min.y).abs() < 1e-6 && (bl.max.z - bo.max.z).abs() < 1e-6);
    // The mirrored solid is the exact reflection: same volume, and the
    // reflected original fills it.
    let v = |s: &of_model::geom::Shape| s.mass_props().unwrap().volume;
    let reflected = orig.mirror(DVec3::ZERO, DVec3::X).unwrap().shape;
    assert!((shape.common_volume(&reflected).unwrap() - v(&orig)).abs() < 1e-3);

    // A reorienting copy of a symmetric part mirrors its position.
    let pin = save(&shaft_doc(5.0, 40.0), &dir.path("pin.ofp"));
    let p = a
        .insert(
            &pin,
            Some(Pose::from_translation(DVec3::new(50.0, 0.0, 0.0))),
            None,
        )
        .unwrap();
    a.doc.mirrors[0].components.push(MirrorItem {
        component: p.clone(),
        opposite_hand: None,
    });
    let inst = a.instances();
    let copy = inst
        .iter()
        .find(|i| i.key == [format!("{p}@m{id}")])
        .unwrap();
    assert!((copy.pose.translation.x + 50.0).abs() < 1e-6);
}

#[test]
fn flexible_subassemblies_move_with_parent_mates() {
    let dir = Scratch::new("flex");
    let plate = plate_doc(100.0, 60.0, 10.0, &[(30.0, 30.0, 5.0)]);
    let pin = shaft_doc(5.0, 40.0);
    let pf = save(&plate, &dir.path("plate.ofp"));
    let sf = save(&pin, &dir.path("pin.ofp"));
    // Subassembly: a fixed plate and a pin free to slide in its hole.
    let mut sub = Assembly::default();
    let sp = sub.insert(&pf, None, None).unwrap();
    let ss = sub
        .insert(
            &sf,
            Some(Pose::from_translation(DVec3::new(30.0, 30.0, 20.0))),
            None,
        )
        .unwrap();
    sub.add_mate(
        None,
        MateKind::Concentric {
            a: r(&sp, cylinder_face(&plate, 5.0)),
            b: r(&ss, cylinder_face(&pin, 5.0)),
            alignment: Alignment::Closest,
            lock_rotation: false,
        },
    )
    .unwrap();
    let sub_file = dir.path("slide.ofa");
    sub.save(&sub_file).unwrap();

    let mut a = Assembly::default();
    let k = a.insert(&sub_file, None, None).unwrap();
    // Push the pin's bottom onto the assembly front plane (z = 0).
    let pin_bottom = CompRef {
        component: vec![k.clone(), ss.clone()],
        entity: plane_face(&pin, -DVec3::Z, 0.0),
    };
    let mate = MateKind::Coincident {
        a: assembly_ref(Ref::FrontPlane),
        b: pin_bottom,
        alignment: Alignment::Closest,
    };
    // Rigid: the pin can't move inside the subassembly, so this conflicts.
    assert!(a.add_mate(None, mate.clone()).is_err());
    a.doc.component_mut(&k).unwrap().flexible = true;
    a.add_mate(None, mate).unwrap();
    let inst = a.instances();
    let child = inst
        .iter()
        .find(|i| i.key == [k.clone(), ss.clone()])
        .unwrap();
    assert!(child.pose.translation.z.abs() < 1e-6, "{:?}", child.pose);
    assert!((child.pose.translation.x - 30.0).abs() < 1e-6);
    let stored: Pose = (*a
        .doc
        .component(&k)
        .unwrap()
        .flexible_poses
        .get(&ss)
        .unwrap())
    .into();
    assert!(stored.translation.z.abs() < 1e-6);
    // The subassembly file itself is untouched.
    let again = Assembly::open(&sub_file).unwrap();
    let t: Pose = again.doc.component(&ss).unwrap().transform.into();
    assert!((t.translation.z - 20.0).abs() < 1e-9);
}

#[test]
fn explode_bom_mass_interference_and_clearance() {
    let dir = Scratch::new("tools");
    let (mut a, p, s, plate, _) = pinned(&dir);

    // Exploded view: lift the pin 50 mm; the solved pose is untouched.
    let instances = a.instances();
    let step = ExplodeStep {
        id: 99,
        name: "Explode Step1".into(),
        components: vec![vec![s.clone()]],
        motion: ExplodeMotion::Translate {
            direction: [0.0, 0.0, 1.0],
            distance: 50.0,
            auto_space: false,
        },
    };
    let ex = explode::explode(&instances, &[step], None);
    let lifted = ex.poses[&vec![s.clone()]].translation.z - pose_of(&a, &s).translation.z;
    assert!((lifted - 50.0).abs() < 1e-9);
    assert_eq!(ex.lines.len(), 1);
    assert!((ex.lines[0][1].z - ex.lines[0][0].z - 50.0).abs() < 1e-9);

    // BOM: properties from the part, quantities per file.
    let mut pin_doc = shaft_doc(5.0, 40.0);
    pin_doc
        .properties
        .insert("PartNo".into(), Property::text("PIN-10x40"));
    pin_doc.properties.insert(
        "Description".into(),
        Property::text("Dowel {material} {mass} kg"),
    );
    pin_doc.material = Some(of_model::materials::Material {
        name: "Steel".into(),
        density: 7850.0,
    });
    save(&pin_doc, &dir.path("pin.ofp"));
    a.refresh();
    let pin2 = a
        .insert(
            &dir.path("pin.ofp"),
            Some(Pose::from_translation(DVec3::new(150.0, 30.0, 10.0))),
            None,
        )
        .unwrap();
    let rows = bom::bom(&mut a, BomType::TopLevel);
    assert_eq!(rows.len(), 2);
    let pins = rows.iter().find(|r| r.part_number == "PIN-10x40").unwrap();
    assert_eq!(pins.quantity, 2);
    let mass = 7850.0 * 1e-9 * std::f64::consts::PI * 25.0 * 40.0;
    assert!((pins.mass.unwrap() - mass).abs() < 1e-9);
    assert!(
        pins.description.starts_with("Dowel Steel 0.0247"),
        "{}",
        pins.description
    );
    let csv = bom::csv(&rows);
    assert!(
        csv.lines()
            .next()
            .unwrap()
            .starts_with("Item,Qty,Part Number")
    );
    assert!(csv.contains("PIN-10x40"));
    a.doc.component_mut(&pin2).unwrap().exclude_from_bom = true;
    let rows = bom::bom(&mut a, BomType::PartsOnly);
    assert_eq!(
        rows.iter()
            .find(|r| r.part_number == "PIN-10x40")
            .unwrap()
            .quantity,
        1
    );

    // Mass: plate (default density) plus two steel pins; center weighted.
    let m = analysis::mass_properties(&mut a).unwrap();
    assert_eq!(m.components, 3);
    let plate_mass = 1000.0 * 1e-9 * (200.0 * 60.0 * 10.0 - std::f64::consts::PI * 25.0 * 10.0);
    assert!(
        (m.mass - plate_mass - 2.0 * mass).abs() < 1e-9,
        "{}",
        m.mass
    );

    // Interference: a pin pushed halfway into the plate overlaps it.
    a.doc.component_mut(&pin2).unwrap().transform =
        Pose::from_translation(DVec3::new(150.0, 30.0, 5.0)).into();
    let found = analysis::interference(&mut a, &InterferenceOptions::default());
    assert_eq!(found.len(), 1, "the seated pin only touches the plate");
    let overlap = std::f64::consts::PI * 25.0 * 5.0;
    assert!(
        (found[0].volume - overlap).abs() < 1e-3,
        "{}",
        found[0].volume
    );
    // Coincident contact shows up only when asked.
    let touching = analysis::interference(
        &mut a,
        &InterferenceOptions {
            treat_coincidence: true,
            ..Default::default()
        },
    );
    assert_eq!(touching.len(), 2);
    a.doc
        .ignored_interferences
        .push(analysis::pair_key(&found[0].a, &found[0].b));
    assert!(analysis::interference(&mut a, &InterferenceOptions::default())[0].ignored);

    // Clearance: the seated pin's top is 40 mm above the plate; the pins
    // are 120 mm apart.
    let gaps = analysis::clearance(&mut a, &[vec![s.clone()], vec![pin2.clone()]], 200.0);
    assert!((gaps[0].distance - 110.0).abs() < 1e-6, "{:?}", gaps[0]);
    let _ = (p, plate);
}

#[test]
fn step_export_keeps_structure_and_import_rebuilds_it() {
    let dir = Scratch::new("step");
    let (mut a, _, s, _, _) = pinned(&dir);
    let id = a.doc.take_id();
    a.doc.patterns.push(Pattern {
        id,
        name: "LocalLPattern1".into(),
        kind: PatternKind::Linear {
            direction1: PatternDirection {
                reference: assembly_ref(Ref::RightPlane),
                spacing: 40.0,
                count: 3,
                reverse: false,
            },
            direction2: None,
        },
        seeds: vec![s.clone()],
        skip: vec![],
        suppressed: false,
    });
    let step = dir.path("pinned.step");
    exchange::export_step(&mut a, &step).unwrap();
    let text = std::fs::read_to_string(&step).unwrap();
    assert!(text.contains("NEXT_ASSEMBLY_USAGE_OCCURRENCE"));

    let nodes = of_model::geom::read_step_assembly(&step.to_string_lossy()).unwrap();
    let leaves = nodes.iter().filter(|n| n.shape.is_some()).count();
    assert_eq!(leaves, 4, "plate and three pins");

    let out = dir.path("imported");
    let mut b = exchange::import_step(&step, &out).unwrap();
    assert_eq!(b.doc.components.len(), 4);
    let files: std::collections::BTreeSet<String> =
        b.doc.components.iter().map(|c| c.path.clone()).collect();
    assert_eq!(files.len(), 2, "pins share one part file: {files:?}");
    let v = |a: &mut Assembly| analysis::mass_properties(a).unwrap().volume;
    assert!((v(&mut a) - v(&mut b)).abs() < 1e-3);
    let ca = analysis::mass_properties(&mut a).unwrap().center;
    let cb = analysis::mass_properties(&mut b).unwrap().center;
    for k in 0..3 {
        assert!((ca[k] - cb[k]).abs() < 1e-6);
    }
}
