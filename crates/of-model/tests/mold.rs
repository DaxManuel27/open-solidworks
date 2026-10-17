//! Mould tools on a drafted ABS block with a through-hole: draft and
//! undercut analysis, shrinkage, parting line, shut-off and parting
//! surfaces, tooling split with an interlock, and a side core.

use of_model::analysis::{DraftClass, UndercutClass};
use of_model::body_tools::*;
use of_model::mold::*;
use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::{Sketch, tools},
    *,
};

fn rectangle(doc: &mut Document, plane: Ref, a: DVec2, b: DVec2) -> FeatureId {
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, a, b);
    doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch: sk }))
}

fn extrude(
    doc: &mut Document,
    sketch: FeatureId,
    end1: EndCondition,
    end2: Option<EndCondition>,
    cut: bool,
) {
    doc.add(FeatureKind::Extrude(Extrude {
        sketch,
        cut,
        start_offset: 0.0,
        end1,
        reverse: false,
        end2,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
}

fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

fn face_at(m: &Model, p: DVec3) -> Ref {
    let all = m.state().everything().unwrap();
    let i = (0..all.shape.count(Kind::Face))
        .min_by(|a, b| {
            let da = (all.shape.face_info(*a).unwrap().centroid - p).length();
            let db = (all.shape.face_info(*b).unwrap().centroid - p).length();
            da.total_cmp(&db)
        })
        .unwrap();
    m.state().ref_for(Kind::Face, i).unwrap()
}

fn volume_of(m: &Model, id: &BodyId) -> f64 {
    m.state()
        .solid(id)
        .unwrap()
        .geometry
        .shape
        .mass_props()
        .unwrap()
        .volume
}

/// A 40 × 30 × 20 ABS block, walls drafted 3° from the bottom, with a 6 mm
/// hole straight through.
fn part() -> Model {
    let mut doc = Document::new();
    doc.material = materials::by_name("ABS");
    let s = rectangle(
        &mut doc,
        Ref::FrontPlane,
        DVec2::new(-20.0, -15.0),
        DVec2::new(20.0, 15.0),
    );
    extrude(
        &mut doc,
        s,
        EndCondition::Blind { depth: 20.0 },
        None,
        false,
    );
    let m = Model::new(doc);
    let walls = [
        DVec3::new(20.0, 0.0, 10.0),
        DVec3::new(-20.0, 0.0, 10.0),
        DVec3::new(0.0, 15.0, 10.0),
        DVec3::new(0.0, -15.0, 10.0),
    ]
    .map(|p| face_at(&m, p))
    .to_vec();
    let mut doc = m.doc.clone();
    doc.add(FeatureKind::Draft(Draft {
        faces: walls,
        neutral_plane: Ref::FrontPlane,
        direction: None,
        angle: 3f64.to_radians(),
        parting_line: vec![],
        other_side: false,
        step: false,
    }));
    let mut sk = Sketch::new();
    let c = sk.add_point(DVec2::ZERO);
    sk.add_circle(c, 3.0).unwrap();
    let hole = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    extrude(
        &mut doc,
        hole,
        EndCondition::ThroughAll,
        Some(EndCondition::ThroughAll),
        true,
    );
    let m = Model::new(doc);
    ok(&m);
    m
}

fn add(m: &Model, kind: FeatureKind) -> (Model, FeatureId) {
    let mut doc = m.doc.clone();
    let id = doc.add(kind);
    let m = Model::new(doc);
    ok(&m);
    (m, id)
}

#[test]
fn draft_analysis_classifies_the_faces() {
    let m = part();
    let faces = analysis::draft(m.state(), DVec3::Z, 2f64.to_radians()).unwrap();
    let all = m.state().everything().unwrap();
    let class_at = |p: DVec3| {
        let r = face_at(&m, p);
        let (_, i) = m.state().index_of(&r).unwrap();
        faces.iter().find(|f| f.face == i).unwrap().class
    };
    assert_eq!(class_at(DVec3::new(10.0, 0.0, 20.0)), DraftClass::Positive);
    assert_eq!(class_at(DVec3::new(10.0, 0.0, 0.0)), DraftClass::Negative);
    // The drafted walls lean in toward the top.
    assert_eq!(class_at(DVec3::new(20.0, 0.0, 10.0)), DraftClass::Positive);
    // The hole's wall is upright: it needs draft.
    assert_eq!(
        class_at(DVec3::new(3.0, 0.0, 10.0)),
        DraftClass::RequiresDraft
    );
    assert_eq!(faces.len() as u32, all.shape.count(Kind::Face));
}

#[test]
fn undercut_analysis_finds_a_side_pocket() {
    // A C shape opening along +x: a base, a lid, and a wall at the back.
    let mut doc = Document::new();
    for (a, b, from, depth) in [
        (DVec2::ZERO, DVec2::new(30.0, 30.0), 0.0, 10.0),
        (DVec2::ZERO, DVec2::new(30.0, 30.0), 20.0, 10.0),
        (DVec2::ZERO, DVec2::new(10.0, 30.0), 10.0, 10.0),
    ] {
        let s = rectangle(&mut doc, Ref::FrontPlane, a, b);
        doc.add(FeatureKind::Extrude(Extrude {
            sketch: s,
            cut: false,
            start_offset: from,
            end1: EndCondition::Blind { depth },
            reverse: false,
            end2: None,
            thin: None,
            flip_side_to_cut: false,
            merge: true,
        }));
    }
    let m = Model::new(doc);
    ok(&m);
    let result = analysis::undercut(m.state(), DVec3::Z).unwrap();
    let class_at = |p: DVec3| {
        let r = face_at(&m, p);
        let (_, i) = m.state().index_of(&r).unwrap();
        result.iter().find(|f| f.face == i).unwrap().class
    };
    // The pocket's floor faces up under its ceiling, and the ceiling down.
    assert_eq!(
        class_at(DVec3::new(20.0, 15.0, 10.0)),
        UndercutClass::Direction1
    );
    assert_eq!(
        class_at(DVec3::new(20.0, 15.0, 20.0)),
        UndercutClass::Direction2
    );
    assert_eq!(class_at(DVec3::new(15.0, 15.0, 30.0)), UndercutClass::None);
}

#[test]
fn shrinkage_scales_by_the_material() {
    let m = part();
    let before = m
        .state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume;
    let (s, _) = add(
        &m,
        FeatureKind::Scale(Scale {
            bodies: vec![],
            factor: 1.0,
            factors: None,
            about: ScaleAbout::Centroid,
            shrinkage: true,
        }),
    );
    let after = s
        .state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume;
    close(after / before, 1.005f64.powi(3), 1e-9);
}

#[test]
fn the_part_splits_into_core_and_cavity() {
    let m = part();
    let part_volume = m
        .state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume;
    let (m, line) = add(
        &m,
        FeatureKind::PartingLine(PartingLine {
            body: None,
            pull: Ref::FrontPlane,
            reverse: false,
            draft_angle: 0.0,
            split_faces: true,
        }),
    );
    let record = m.state().mold.record(line).unwrap();
    // The bottom outline, and the hole's top rim as an opening.
    assert_eq!(record.edges.len(), 4);
    assert_eq!(record.openings.len(), 1);
    let (m, _) = add(
        &m,
        FeatureKind::ShutOffSurfaces(ShutOffSurfaces {
            parting_line: line,
            patch: ShutOffPatch::Contact,
            patches: vec![],
        }),
    );
    assert_eq!(m.state().mold.shut_off.len(), 1);
    let (m, _) = add(
        &m,
        FeatureKind::PartingSurface(PartingSurface {
            parting_line: line,
            mode: PartingSurfaceMode::PerpendicularToPull,
            distance: 30.0,
            reverse: false,
        }),
    );
    let mut doc = m.doc.clone();
    let block = rectangle(
        &mut doc,
        Ref::FrontPlane,
        DVec2::new(-35.0, -30.0),
        DVec2::new(35.0, 30.0),
    );
    let split = |interlock: Option<Interlock>| {
        let mut doc = doc.clone();
        doc.add(FeatureKind::ToolingSplit(ToolingSplit {
            parting_line: line,
            sketch: block,
            depth1: 30.0,
            depth2: 10.0,
            interlock,
        }));
        let m = Model::new(doc);
        ok(&m);
        m
    };
    let plain = split(None);
    let core = plain.state().mold.cores[0].clone();
    let cavity = plain.state().mold.cavities[0].clone();
    let (vc, vk) = (volume_of(&plain, &cavity), volume_of(&plain, &core));
    let block_volume = 70.0 * 60.0 * 40.0;
    close(vc + vk, block_volume - part_volume, 1e-2);
    // The core is the 10 mm below the parting plane plus the hole's pin.
    close(
        vk,
        70.0 * 60.0 * 10.0 + std::f64::consts::PI * 9.0 * 20.0,
        1e-2,
    );
    let locked = split(Some(Interlock {
        angle: 5f64.to_radians(),
        height: 3.0,
        width: 5.0,
    }));
    let (lc, lk) = (volume_of(&locked, &cavity), volume_of(&locked, &core));
    close(lc + lk, vc + vk, 1e-2);
    assert!(lk > vk + 100.0, "the core gains the lip: {lk} vs {vk}");

    // A side core out of the cavity block.
    let mut doc = locked.doc.clone();
    let side = rectangle(
        &mut doc,
        Ref::RightPlane,
        DVec2::new(-5.0, 22.0),
        DVec2::new(5.0, 28.0),
    );
    doc.add(FeatureKind::MoldCore(MoldCore {
        body: cavity.clone(),
        sketch: side,
        reverse: false,
        depth1: 40.0,
        depth2: 0.0,
        draft: 0.0,
    }));
    let with_core = Model::new(doc);
    ok(&with_core);
    assert_eq!(with_core.state().mold.cores.len(), 2);
    assert!(volume_of(&with_core, &cavity) < lc);
}
