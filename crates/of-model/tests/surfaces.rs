//! Surface bodies and the features that make, join, thicken and cut with
//! them, and Delete Face.

use std::f64::consts::PI;

use of_model::surfaces::*;
use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::{Sketch, tools},
    *,
};

fn sketch_on(doc: &mut Document, plane: Ref, sk: Sketch) -> FeatureId {
    doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch: sk }))
}

/// Connected lines through `points`.
fn polyline(doc: &mut Document, plane: Ref, points: &[DVec2]) -> FeatureId {
    let mut sk = Sketch::new();
    let ids: Vec<_> = points.iter().map(|p| sk.add_point(*p)).collect();
    for w in ids.windows(2) {
        sk.add_line(w[0], w[1]).unwrap();
    }
    sketch_on(doc, plane, sk)
}

fn rectangle(doc: &mut Document, plane: Ref, a: DVec2, b: DVec2) -> FeatureId {
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, a, b);
    sketch_on(doc, plane, sk)
}

fn blind(depth: f64) -> EndCondition {
    EndCondition::Blind { depth }
}

/// A `w × h × d` box on the front plane at the origin.
fn block(doc: &mut Document, w: f64, h: f64, d: f64) -> FeatureId {
    let sk = rectangle(doc, Ref::FrontPlane, DVec2::ZERO, DVec2::new(w, h));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: sk,
        cut: false,
        start_offset: 0.0,
        end1: blind(d),
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }))
}

fn surface_extrude(doc: &mut Document, sketch: FeatureId, end1: EndCondition) -> FeatureId {
    doc.add(FeatureKind::ExtrudedSurface(ExtrudedSurface {
        sketch,
        end1,
        end2: None,
        reverse: false,
        cap_ends: false,
    }))
}

fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
}

fn error(m: &Model, id: FeatureId) -> String {
    m.status(id)
        .and_then(|s| s.error)
        .unwrap_or_else(|| panic!("feature {id} should fail"))
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

fn surface_area(m: &Model, i: usize) -> f64 {
    m.state().surfaces[i].geometry.shape.area()
}

fn solid_volume(m: &Model) -> f64 {
    m.state()
        .body
        .as_ref()
        .map_or(0.0, |b| b.shape.mass_props().unwrap().volume)
}

/// The face of everything whose centroid is nearest `p`, as a reference.
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

fn edge_at(m: &Model, p: DVec3) -> Ref {
    let all = m.state().everything().unwrap();
    let i = (0..all.shape.count(Kind::Edge))
        .min_by(|a, b| {
            let da = (all.shape.edge_info(*a).unwrap().mid - p).length();
            let db = (all.shape.edge_info(*b).unwrap().mid - p).length();
            da.total_cmp(&db)
        })
        .unwrap();
    m.state().ref_for(Kind::Edge, i).unwrap()
}

#[test]
fn an_open_sketch_extrudes_into_a_surface_body() {
    let mut doc = Document::new();
    let l = polyline(
        &mut doc,
        Ref::FrontPlane,
        &[
            DVec2::new(0.0, 0.0),
            DVec2::new(30.0, 0.0),
            DVec2::new(30.0, 20.0),
        ],
    );
    surface_extrude(&mut doc, l, blind(10.0));
    let m = Model::new(doc);
    ok(&m);
    let state = m.state();
    assert!(state.bodies.is_empty(), "no solids");
    assert!(state.body.is_none());
    assert_eq!(state.surfaces.len(), 1);
    assert_eq!(state.surfaces[0].geometry.shape.count(Kind::Face), 2);
    close(surface_area(&m, 0), 50.0 * 10.0, 1e-6);
    // Its faces have stable names and can be referenced.
    let face = face_at(&m, DVec3::new(15.0, 0.0, 5.0));
    assert!(matches!(m.resolve(&face).unwrap(), Resolved::Face { .. }));
}

#[test]
fn surfaces_stay_apart_from_solids() {
    let mut doc = Document::new();
    block(&mut doc, 10.0, 10.0, 10.0);
    let l = polyline(
        &mut doc,
        Ref::FrontPlane,
        &[DVec2::new(5.0, -5.0), DVec2::new(5.0, 15.0)],
    );
    surface_extrude(&mut doc, l, EndCondition::MidPlane { depth: 30.0 });
    // A later boss still only sees the solid.
    let s = rectangle(
        &mut doc,
        Ref::FrontPlane,
        DVec2::new(0.0, 0.0),
        DVec2::new(2.0, 2.0),
    );
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: blind(15.0),
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let m = Model::new(doc);
    ok(&m);
    assert_eq!(m.state().bodies.len(), 1);
    assert_eq!(m.state().surfaces.len(), 1);
    close(solid_volume(&m), 1000.0 + 4.0 * 5.0, 1e-6);
    // The surface isn't split by the solid features.
    assert_eq!(m.state().surfaces[0].geometry.shape.count(Kind::Face), 1);
}

#[test]
fn solid_features_refuse_surface_geometry() {
    let mut doc = Document::new();
    let l = polyline(
        &mut doc,
        Ref::FrontPlane,
        &[
            DVec2::new(0.0, 0.0),
            DVec2::new(30.0, 0.0),
            DVec2::new(30.0, 20.0),
        ],
    );
    surface_extrude(&mut doc, l, blind(10.0));
    let mut m = Model::new(doc);
    let edge = edge_at(&m, DVec3::new(30.0, 0.0, 5.0));
    let f = m.doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![edge],
        faces: vec![],
        radius: 2.0,
    }));
    m.rebuild();
    let e = error(&m, f);
    assert!(e.contains("solid body"), "{e}");
    // With a solid in the part too, picking surface geometry is refused.
    m.doc.delete(&[f]);
    let r = rectangle(
        &mut m.doc,
        Ref::FrontPlane,
        DVec2::new(50.0, 0.0),
        DVec2::new(60.0, 10.0),
    );
    m.doc.add(FeatureKind::Extrude(Extrude {
        sketch: r,
        cut: false,
        start_offset: 0.0,
        end1: blind(10.0),
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    m.rebuild();
    let edge = edge_at(&m, DVec3::new(30.0, 0.0, 5.0));
    let f = m.doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![edge],
        faces: vec![],
        radius: 2.0,
    }));
    m.rebuild();
    let e = error(&m, f);
    assert!(e.contains("surface body"), "{e}");
}

#[test]
fn capped_extrusion_knits_into_a_solid() {
    let mut doc = Document::new();
    let r = rectangle(
        &mut doc,
        Ref::FrontPlane,
        DVec2::ZERO,
        DVec2::new(20.0, 10.0),
    );
    doc.add(FeatureKind::ExtrudedSurface(ExtrudedSurface {
        sketch: r,
        end1: blind(5.0),
        end2: None,
        reverse: false,
        cap_ends: true,
    }));
    let m = Model::new(doc);
    ok(&m);
    let surface = m.state().surfaces[0].id.clone();
    let shape = &m.state().surfaces[0].geometry.shape;
    assert_eq!(shape.count(Kind::Face), 6);
    assert_eq!(shape.free_edges().unwrap().count(Kind::Edge), 0);
    let mut doc = m.doc.clone();
    doc.add(FeatureKind::Knit(Knit {
        bodies: vec![surface],
        create_solid: true,
        tolerance: 0.0025,
    }));
    let m = Model::new(doc);
    ok(&m);
    assert!(m.state().surfaces.is_empty());
    assert_eq!(m.state().bodies.len(), 1);
    close(solid_volume(&m), 1000.0, 1e-6);
}

#[test]
fn knitting_open_surfaces_without_a_volume_fails_clearly() {
    let mut doc = Document::new();
    let l = polyline(
        &mut doc,
        Ref::FrontPlane,
        &[DVec2::new(0.0, 0.0), DVec2::new(30.0, 0.0)],
    );
    surface_extrude(&mut doc, l, blind(10.0));
    let mut m = Model::new(doc);
    let surface = m.state().surfaces[0].id.clone();
    let k = m.doc.add(FeatureKind::Knit(Knit {
        bodies: vec![surface],
        create_solid: true,
        tolerance: 0.0025,
    }));
    m.rebuild();
    let e = error(&m, k);
    assert!(e.contains("open edge"), "{e}");
}

#[test]
fn a_planar_surface_thickens_into_a_solid() {
    let mut doc = Document::new();
    let r = rectangle(
        &mut doc,
        Ref::FrontPlane,
        DVec2::ZERO,
        DVec2::new(20.0, 10.0),
    );
    doc.add(FeatureKind::PlanarSurface(PlanarSurface {
        sketch: Some(r),
        edges: vec![],
    }));
    let m = Model::new(doc);
    ok(&m);
    close(surface_area(&m, 0), 200.0, 1e-9);
    for (side, z) in [
        (ThickenSide::Side1, 1.5_f64),
        (ThickenSide::Side2, -1.5),
        (ThickenSide::Both, 0.0),
    ] {
        let mut doc = m.doc.clone();
        doc.add(FeatureKind::Thicken(Thicken {
            body: m.state().surfaces[0].id.clone(),
            thickness: 3.0,
            side,
            fill_volume: false,
            cut: false,
            merge: true,
        }));
        let t = Model::new(doc);
        ok(&t);
        assert!(t.state().surfaces.is_empty(), "the surface is consumed");
        close(solid_volume(&t), 600.0, 1e-6);
        let c = t
            .state()
            .body
            .as_ref()
            .unwrap()
            .shape
            .mass_props()
            .unwrap()
            .center;
        close(c.z.abs(), z.abs(), 1e-6);
    }
}

#[test]
fn thicken_cut_removes_material() {
    let mut doc = Document::new();
    block(&mut doc, 20.0, 10.0, 10.0);
    let r = rectangle(
        &mut doc,
        Ref::FrontPlane,
        DVec2::new(5.0, 2.0),
        DVec2::new(15.0, 8.0),
    );
    doc.add(FeatureKind::PlanarSurface(PlanarSurface {
        sketch: Some(r),
        edges: vec![],
    }));
    let m = Model::new(doc);
    let mut doc = m.doc.clone();
    doc.add(FeatureKind::Thicken(Thicken {
        body: m.state().surfaces[0].id.clone(),
        thickness: 3.0,
        side: ThickenSide::Both,
        fill_volume: false,
        cut: true,
        merge: true,
    }));
    let m = Model::new(doc);
    ok(&m);
    // Half of the 10 × 6 × 3 slab lies inside the block.
    close(solid_volume(&m), 2000.0 - 10.0 * 6.0 * 1.5, 1e-6);
}

#[test]
fn cut_with_surface_keeps_one_side() {
    let mut volumes = Vec::new();
    for flip in [false, true] {
        let mut doc = Document::new();
        block(&mut doc, 20.0, 10.0, 10.0);
        let l = polyline(
            &mut doc,
            Ref::FrontPlane,
            &[DVec2::new(8.0, -5.0), DVec2::new(8.0, 15.0)],
        );
        let s = surface_extrude(&mut doc, l, EndCondition::MidPlane { depth: 40.0 });
        let mut m = Model::new(doc);
        let body = m.state().surfaces[0].id.clone();
        let _ = s;
        m.doc.add(FeatureKind::CutWithSurface(CutWithSurface {
            surface: Ref::Body { id: body },
            flip,
        }));
        m.rebuild();
        ok(&m);
        let c = m.state().body.as_ref().unwrap().shape.mass_props().unwrap();
        volumes.push((c.volume, c.center.x));
    }
    let mut sizes: Vec<f64> = volumes.iter().map(|v| v.0).collect();
    sizes.sort_by(f64::total_cmp);
    close(sizes[0], 800.0, 1e-6);
    close(sizes[1], 1200.0, 1e-6);
    assert!(
        (volumes[0].1 - 8.0) * (volumes[1].1 - 8.0) < 0.0,
        "opposite sides"
    );
}

#[test]
fn delete_face_leaves_a_surface_then_fill_closes_it() {
    let mut doc = Document::new();
    block(&mut doc, 10.0, 10.0, 10.0);
    let m = Model::new(doc);
    let top = face_at(&m, DVec3::new(5.0, 5.0, 10.0));
    let mut doc = m.doc.clone();
    doc.add(FeatureKind::DeleteFace(DeleteFace {
        faces: vec![top.clone()],
        mode: DeleteFaceMode::Delete,
    }));
    let deleted = Model::new(doc);
    ok(&deleted);
    assert!(deleted.state().bodies.is_empty());
    assert_eq!(deleted.state().surfaces.len(), 1);
    assert_eq!(
        deleted.state().surfaces[0].geometry.shape.count(Kind::Face),
        5
    );
    let mut doc = m.doc.clone();
    doc.add(FeatureKind::DeleteFace(DeleteFace {
        faces: vec![top],
        mode: DeleteFaceMode::Fill,
    }));
    let filled = Model::new(doc);
    ok(&filled);
    assert_eq!(filled.state().bodies.len(), 1);
    close(solid_volume(&filled), 1000.0, 1e-3);
}

#[test]
fn delete_face_patch_removes_a_fillet() {
    let mut doc = Document::new();
    block(&mut doc, 20.0, 20.0, 20.0);
    let mut m = Model::new(doc);
    let edge = edge_at(&m, DVec3::new(10.0, 20.0, 20.0));
    m.doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![edge],
        faces: vec![],
        radius: 3.0,
    }));
    m.rebuild();
    ok(&m);
    let rounded = solid_volume(&m);
    assert!(rounded < 8000.0);
    let r = 3.0 * (1.0 - std::f64::consts::FRAC_1_SQRT_2);
    let fillet = face_at(&m, DVec3::new(10.0, 20.0 - r, 20.0 - r));
    m.doc.add(FeatureKind::DeleteFace(DeleteFace {
        faces: vec![fillet],
        mode: DeleteFaceMode::Patch,
    }));
    m.rebuild();
    ok(&m);
    close(solid_volume(&m), 8000.0, 1e-6);
}

#[test]
fn offset_surface_copies_a_face_at_a_distance() {
    let mut doc = Document::new();
    block(&mut doc, 10.0, 10.0, 10.0);
    let m = Model::new(doc);
    let top = face_at(&m, DVec3::new(5.0, 5.0, 10.0));
    let mut doc = m.doc.clone();
    doc.add(FeatureKind::OffsetSurface(OffsetSurface {
        faces: vec![top],
        distance: 5.0,
        flip: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    let face = m.state().surfaces[0].geometry.shape.face_info(0).unwrap();
    close(face.centroid.z, 15.0, 1e-6);
    close(face.area, 100.0, 1e-6);
    assert_eq!(m.state().bodies.len(), 1, "the solid is untouched");
}

#[test]
fn revolved_swept_lofted_and_boundary_surfaces() {
    // Revolve a line at radius 5 into a cylinder 20 long.
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    let a = sk.add_point(DVec2::new(5.0, 0.0));
    let b = sk.add_point(DVec2::new(5.0, 20.0));
    sk.add_line(a, b).unwrap();
    let p = sketch_on(&mut doc, Ref::FrontPlane, sk);
    let mut axis = Sketch::new();
    let a = axis.add_point(DVec2::ZERO);
    let b = axis.add_point(DVec2::new(0.0, 1.0));
    let line = axis.add_line(a, b).unwrap();
    let axis_sk = sketch_on(&mut doc, Ref::FrontPlane, axis);
    doc.add(FeatureKind::RevolvedSurface(RevolvedSurface {
        sketch: p,
        axis: Ref::SketchEntity {
            sketch: axis_sk,
            entity: line,
        },
        angle: RevolveAngle::Blind { angle: 2.0 * PI },
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    close(surface_area(&m, 0), 2.0 * PI * 5.0 * 20.0, 1e-6);

    // Sweep a 10 mm line along a 30 mm line square to it, and loft two
    // parallel lines 10 apart.
    let mut doc = Document::new();
    let profile = polyline(
        &mut doc,
        Ref::FrontPlane,
        &[DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)],
    );
    let path = polyline(
        &mut doc,
        Ref::RightPlane,
        &[DVec2::new(0.0, 0.0), DVec2::new(0.0, 30.0)],
    );
    let contour = |doc: &Document, id: FeatureId| {
        let FeatureKind::Sketch(s) = &doc.feature(id).unwrap().kind else {
            unreachable!()
        };
        Ref::SketchContour {
            sketch: id,
            entities: s
                .sketch
                .entities
                .iter()
                .filter(|(_, e)| !matches!(e.geo, of_model::sketch::Geometry::Point { .. }))
                .map(|(id, _)| *id)
                .collect(),
        }
    };
    let (pr, pa) = (contour(&doc, profile), contour(&doc, path));
    doc.add(FeatureKind::SweptSurface(SweptSurface {
        profile: pr,
        path: pa,
        orientation: SweepOrientation::MinimumTwist,
    }));
    let m = Model::new(doc);
    ok(&m);
    close(surface_area(&m, 0), 300.0, 1e-6);

    let mut doc = Document::new();
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.0,
        flip: false,
    }));
    let one = polyline(
        &mut doc,
        Ref::FrontPlane,
        &[DVec2::new(0.0, 0.0), DVec2::new(20.0, 0.0)],
    );
    let two = polyline(
        &mut doc,
        Ref::Feature { id: plane },
        &[DVec2::new(0.0, 0.0), DVec2::new(20.0, 0.0)],
    );
    let (c1, c2) = (contour(&doc, one), contour(&doc, two));
    doc.add(FeatureKind::LoftedSurface(LoftedSurface {
        profiles: vec![c1.clone(), c2.clone()],
        ruled: false,
    }));
    doc.add(FeatureKind::BoundarySurface(BoundarySurface {
        curves1: vec![c1, c2],
        curves2: vec![],
    }));
    let m = Model::new(doc);
    ok(&m);
    close(surface_area(&m, 0), 200.0, 1e-6);
    close(surface_area(&m, 1), 200.0, 1e-6);
}

#[test]
fn boundary_surface_through_a_curve_network() {
    // Four lines bounding a 20 × 10 square, as two directions.
    let mut doc = Document::new();
    let mut lines = Vec::new();
    for (a, b) in [
        (DVec2::new(0.0, 0.0), DVec2::new(20.0, 0.0)),
        (DVec2::new(0.0, 10.0), DVec2::new(20.0, 10.0)),
        (DVec2::new(0.0, 0.0), DVec2::new(0.0, 10.0)),
        (DVec2::new(20.0, 0.0), DVec2::new(20.0, 10.0)),
    ] {
        let mut sk = Sketch::new();
        let p = sk.add_point(a);
        let q = sk.add_point(b);
        let l = sk.add_line(p, q).unwrap();
        let id = sketch_on(&mut doc, Ref::FrontPlane, sk);
        lines.push(Ref::SketchContour {
            sketch: id,
            entities: vec![l],
        });
    }
    doc.add(FeatureKind::BoundarySurface(BoundarySurface {
        curves1: lines[..2].to_vec(),
        curves2: lines[2..].to_vec(),
    }));
    let m = Model::new(doc);
    ok(&m);
    close(surface_area(&m, 0), 200.0, 1e-6);
}

#[test]
fn imported_open_shells_become_surface_bodies() {
    use base64::Engine;
    use of_model::geom::Shape;
    let solid = Shape::make_box(10.0, 10.0, 10.0).unwrap();
    let top = (0..6)
        .find(|i| solid.face_info(*i).unwrap().normal.z > 0.5)
        .unwrap();
    let open = solid.remove_faces(&[top]).unwrap().shape;
    let other = Shape::make_box(5.0, 5.0, 5.0)
        .unwrap()
        .transform(&of_model::geom::DAffine3::from_translation(DVec3::new(
            20.0, 0.0, 0.0,
        )))
        .unwrap()
        .shape;
    let dir = std::env::temp_dir().join(format!("of-surface-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mixed.step");
    Shape::compound([&open, &other])
        .write_step(path.to_str().unwrap())
        .unwrap();
    let data = base64::engine::general_purpose::STANDARD.encode(std::fs::read(&path).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    let mut doc = Document::new();
    doc.add(FeatureKind::Import(Import {
        format: ImportFormat::Step,
        source: "mixed.step".into(),
        data,
        mesh: None,
    }));
    let m = Model::new(doc);
    ok(&m);
    assert_eq!(m.state().bodies.len(), 1);
    close(solid_volume(&m), 125.0, 1e-6);
    assert_eq!(m.state().surfaces.len(), 1);
    assert_eq!(m.state().surfaces[0].geometry.shape.count(Kind::Face), 5);
    close(surface_area(&m, 0), 500.0, 1e-6);
}

#[test]
fn surface_parts_round_trip_through_the_file() {
    let mut doc = Document::new();
    let r = rectangle(
        &mut doc,
        Ref::FrontPlane,
        DVec2::ZERO,
        DVec2::new(20.0, 10.0),
    );
    doc.add(FeatureKind::ExtrudedSurface(ExtrudedSurface {
        sketch: r,
        end1: blind(5.0),
        end2: None,
        reverse: false,
        cap_ends: true,
    }));
    let m = Model::new(doc);
    let text = file::to_string(&m.doc);
    assert!(text.contains("\"version\": 7"), "{}", &text[..80]);
    let loaded = Model::new(file::from_str(&text).unwrap());
    ok(&loaded);
    assert_eq!(loaded.doc, m.doc);
    let (a, b) = (&m.state().surfaces[0], &loaded.state().surfaces[0]);
    assert_eq!(a.id, b.id);
    assert_eq!(a.geometry.names, b.geometry.names);
}
