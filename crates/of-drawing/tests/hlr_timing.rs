//! Hidden-line timings on a gearbox-sized housing and an assembly built
//! around it (P4.0). Ignored by default; run with
//!
//! ```sh
//! cargo test -p of-drawing --test hlr_timing -- --ignored --nocapture
//! cargo test -p of-drawing --release --test hlr_timing -- --ignored --nocapture
//! ```

mod common;

use std::time::Instant;

use common::*;
use of_assembly::{Assembly, DQuat, DVec3, Pose};
use of_drawing::document::Orientation;
use of_drawing::model::Sources;
use of_drawing::views;
use of_model::geom::{DVec2, HlrQuality, Kind, SurfaceKind};
use of_model::sketch::Sketch;
use of_model::*;

/// The planar face of a part whose outward normal is `n` at coordinate `at`.
fn plane_face(doc: &Document, n: DVec3, at: f64) -> Ref {
    let mut m = Model::new(doc.clone());
    m.rebuild();
    let state = m.state();
    let shape = &state.body.as_ref().expect("a body").shape;
    for i in 0..shape.count(Kind::Face) {
        let f = shape.face_info(i).unwrap();
        if f.kind == SurfaceKind::Plane
            && f.normal.normalize().dot(n) > 0.999
            && (f.point.dot(n) - at).abs() < 1e-6
        {
            return state.ref_for(Kind::Face, i).unwrap();
        }
    }
    panic!("no face");
}

/// Bores along Z for the shafts of a two-stage gearbox.
const BORES: [(f64, f64, f64); 3] = [(50.0, 60.0, 16.0), (120.0, 60.0, 20.0), (190.0, 60.0, 16.0)];

/// A 240 × 120 × 90 housing, open at the top, with filleted bottom edges, three
/// shaft bores and 16 bolt holes through the end walls.
fn housing() -> Document {
    let mut doc = block_doc(240.0, 120.0, 90.0);
    // Open at +Y so bores along Z cross the end walls.
    let top = plane_face(&doc, DVec3::Y, 120.0);
    doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![],
        faces: vec![plane_face(&doc, DVec3::NEG_Y, 0.0)],
        radius: 8.0,
    }));
    doc.add(FeatureKind::Shell(ShellDef {
        faces: vec![top],
        thickness: 8.0,
        outward: false,
    }));
    let mut sk = Sketch::new();
    for (x, y, r) in BORES {
        sk.add_circle_at(DVec2::new(x, y), r);
    }
    for i in 0..8 {
        let x = 20.0 + i as f64 * 200.0 / 7.0;
        sk.add_circle_at(DVec2::new(x, 15.0), 3.0);
        sk.add_circle_at(DVec2::new(x, 105.0), 3.0);
    }
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: true,
        start_offset: 0.0,
        end1: EndCondition::ThroughAll,
        reverse: false,
        end2: Some(EndCondition::ThroughAll),
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    doc
}

fn time<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let v = f();
    (v, t.elapsed().as_secs_f64() * 1000.0)
}

fn measure(name: &str, sources: &mut Sources, path: &std::path::Path) {
    let scene = sources.scene(path, None, false, false, &[]).unwrap();
    let edges = scene.shape.count(Kind::Edge);
    let faces = scene.shape.count(Kind::Face);
    println!(
        "\n{name}: {} pieces, {faces} faces, {edges} edges",
        scene.pieces.len()
    );
    println!("| view | exact ms | preview (0.1 mm) ms | exact curves | preview curves |");
    println!("| --- | ---: | ---: | ---: | ---: |");
    for (label, o) in [
        ("front", Orientation::Front),
        ("top", Orientation::Top),
        ("right", Orientation::Right),
        ("isometric", Orientation::Isometric),
    ] {
        let plane = views::orientation(&o).plane();
        let (exact, te) = time(|| {
            scene
                .shape
                .hidden_lines(&plane, HlrQuality::Exact, true, 0.01)
                .unwrap()
        });
        let (preview, tp) = time(|| {
            scene
                .shape
                .hidden_lines(&plane, HlrQuality::Preview, true, 0.1)
                .unwrap()
        });
        assert!(!exact.is_empty() && !preview.is_empty());
        println!(
            "| {label} | {te:.0} | {tp:.0} | {} | {} |",
            exact.len(),
            preview.len()
        );
    }
}

#[test]
#[ignore = "timing run"]
fn hlr_timings() {
    let dir = Scratch::new("hlr-timing");
    let housing = save(&housing(), &dir.path("housing.ofp"));
    let mut sources = Sources::default();
    measure("Housing", &mut sources, &housing);

    // Housing with a shaft in each bore, a gear blank on each, and a bolt in
    // every floor hole.
    let shaft = save(&shaft_doc(19.5, 160.0), &dir.path("shaft.ofp"));
    let small = save(&shaft_doc(15.5, 160.0), &dir.path("small-shaft.ofp"));
    let gear = save(&shaft_doc(30.0, 20.0), &dir.path("gear.ofp"));
    let bolt = save(&shaft_doc(2.9, 25.0), &dir.path("bolt.ofp"));
    let mut a = Assembly::default();
    a.insert(&housing, None, None).unwrap();
    for (i, (x, y, r)) in BORES.iter().enumerate() {
        let file = if *r > 18.0 { &shaft } else { &small };
        let at = |z| Some(Pose::new(DVec3::new(*x, *y, z), DQuat::IDENTITY));
        a.insert(file, at(-35.0), None).unwrap();
        a.insert(&gear, at(20.0 + i as f64 * 25.0), None).unwrap();
    }
    for i in 0..8 {
        let x = 20.0 + i as f64 * 200.0 / 7.0;
        for y in [15.0, 105.0] {
            let at = Some(Pose::new(DVec3::new(x, y, -10.0), DQuat::IDENTITY));
            a.insert(&bolt, at, None).unwrap();
        }
    }
    let file = dir.path("gearbox.ofa");
    a.save(&file).unwrap();
    measure("Gearbox", &mut sources, &file);
}
