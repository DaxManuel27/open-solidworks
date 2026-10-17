//! Body tools and shape features: delete/keep, intersect, scale, flex,
//! deform, indent, insert part, defeature, slicing and move/copy extras.

use of_model::body_tools::*;
use of_model::surfaces::*;
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

fn extrude(doc: &mut Document, sketch: FeatureId, depth: f64, merge: bool) -> FeatureId {
    doc.add(FeatureKind::Extrude(Extrude {
        sketch,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge,
    }))
}

fn block(doc: &mut Document, a: DVec2, b: DVec2, d: f64) {
    let s = rectangle(doc, Ref::FrontPlane, a, b);
    extrude(doc, s, d, false);
}

fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

fn volume(m: &Model) -> f64 {
    m.state()
        .body
        .as_ref()
        .map_or(0.0, |b| b.shape.mass_props().unwrap().volume)
}

fn with(m: &Model, kind: FeatureKind) -> Model {
    let mut doc = m.doc.clone();
    doc.add(kind);
    let m = Model::new(doc);
    ok(&m);
    m
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

/// A 10 × 10 × 40 bar standing on the front plane.
fn bar() -> Model {
    let mut doc = Document::new();
    block(&mut doc, DVec2::ZERO, DVec2::new(10.0, 10.0), 40.0);
    Model::new(doc)
}

#[test]
fn delete_and_keep_bodies() {
    let mut doc = Document::new();
    block(&mut doc, DVec2::ZERO, DVec2::new(10.0, 10.0), 10.0);
    block(&mut doc, DVec2::new(20.0, 0.0), DVec2::new(25.0, 5.0), 10.0);
    let m = Model::new(doc);
    assert_eq!(m.state().bodies.len(), 2);
    let first = m.state().bodies[0].id.clone();
    let gone = with(
        &m,
        FeatureKind::DeleteBody(DeleteBody {
            bodies: vec![first.clone()],
            keep: false,
        }),
    );
    close(volume(&gone), 250.0, 1e-6);
    let kept = with(
        &m,
        FeatureKind::DeleteBody(DeleteBody {
            bodies: vec![first],
            keep: true,
        }),
    );
    close(volume(&kept), 1000.0, 1e-6);
}

#[test]
fn intersect_splits_by_a_plane_and_fills_a_closed_surface() {
    let mut doc = Document::new();
    block(&mut doc, DVec2::ZERO, DVec2::new(20.0, 20.0), 20.0);
    let mid = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.0,
        flip: false,
    }));
    let m = Model::new(doc);
    let body = m.state().bodies[0].id.clone();
    let tools = vec![Ref::Body { id: body }, Ref::Feature { id: mid }];
    let split = with(
        &m,
        FeatureKind::Intersect(Intersect {
            tools: tools.clone(),
            mode: IntersectMode::Intersection,
            exclude: vec![],
            merge: false,
            consume_surfaces: true,
            cap_planar: false,
        }),
    );
    assert_eq!(split.state().bodies.len(), 2);
    close(volume(&split), 8000.0, 1e-6);
    let one = with(
        &m,
        FeatureKind::Intersect(Intersect {
            tools,
            mode: IntersectMode::Intersection,
            exclude: vec![[10.0, 10.0, 15.0]],
            merge: true,
            consume_surfaces: true,
            cap_planar: false,
        }),
    );
    assert_eq!(one.state().bodies.len(), 1);
    close(volume(&one), 4000.0, 1e-6);

    // A closed surface box encloses an internal region.
    let mut doc = Document::new();
    let r = rectangle(
        &mut doc,
        Ref::FrontPlane,
        DVec2::ZERO,
        DVec2::new(10.0, 10.0),
    );
    doc.add(FeatureKind::ExtrudedSurface(ExtrudedSurface {
        sketch: r,
        end1: EndCondition::Blind { depth: 10.0 },
        end2: None,
        reverse: false,
        cap_ends: true,
    }));
    let m = Model::new(doc);
    let surface = m.state().surfaces[0].id.clone();
    let filled = with(
        &m,
        FeatureKind::Intersect(Intersect {
            tools: vec![Ref::Body { id: surface }],
            mode: IntersectMode::Internal,
            exclude: vec![],
            merge: true,
            consume_surfaces: true,
            cap_planar: false,
        }),
    );
    assert!(filled.state().surfaces.is_empty());
    close(volume(&filled), 1000.0, 1e-6);
}

#[test]
fn scale_uniformly_and_per_axis() {
    let m = bar();
    let s = with(
        &m,
        FeatureKind::Scale(Scale {
            bodies: vec![],
            factor: 1.006,
            factors: None,
            about: ScaleAbout::Centroid,
        }),
    );
    close(volume(&s), 4000.0 * 1.006f64.powi(3), 1e-6);
    // The centroid stays put.
    let c = s
        .state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .center;
    assert!((c - DVec3::new(5.0, 5.0, 20.0)).length() < 1e-9);
    let s = with(
        &m,
        FeatureKind::Scale(Scale {
            bodies: vec![],
            factor: 1.0,
            factors: Some([2.0, 1.0, 0.5]),
            about: ScaleAbout::Origin,
        }),
    );
    close(volume(&s), 4000.0, 1e-6);
    let b = s.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.x, 20.0, 1e-6);
    close(b.max.z, 20.0, 1e-6);
}

fn flexed(mode: FlexMode, accuracy: u32) -> Model {
    let m = bar();
    let mut doc = m.doc.clone();
    let mut sk = Sketch::new();
    let a = sk.add_point(DVec2::new(5.0, 5.0));
    let _ = a;
    doc.add(FeatureKind::Point(PointDef::Coordinates {
        x: 5.0,
        y: 5.0,
        z: 0.0,
    }));
    let point = doc.features.last().unwrap().id;
    doc.add(FeatureKind::Flex(Flex {
        bodies: vec![],
        mode,
        origin: Ref::Feature { id: point },
        axis: Ref::FrontPlane,
        toward: Some(Ref::RightPlane),
        trim: None,
        accuracy,
    }));
    let m = Model::new(doc);
    ok(&m);
    m
}

#[test]
fn flex_twists_tapers_stretches_and_bends() {
    let twisted = flexed(
        FlexMode::Twist {
            angle: 30f64.to_radians(),
        },
        8,
    );
    close(volume(&twisted), 4000.0, 40.0);
    let stretched = flexed(FlexMode::Stretch { distance: 10.0 }, 2);
    close(volume(&stretched), 5000.0, 1e-3);
    let tapered = flexed(FlexMode::Taper { factor: 0.5 }, 4);
    close(volume(&tapered), 4000.0 * (1.0 - 0.125) / 1.5, 10.0);
    let bent = flexed(
        FlexMode::Bend {
            angle: std::f64::consts::FRAC_PI_2,
        },
        8,
    );
    close(volume(&bent), 4000.0, 40.0);
    // Bent a quarter turn toward +x: the top now points along x.
    let b = bent.state().body.as_ref().unwrap().shape.bbox().unwrap();
    assert!(b.max.x > 25.0 && b.max.z < 32.0, "{b:?}");
}

#[test]
fn deform_pulls_a_point() {
    let m = bar();
    let mut doc = m.doc.clone();
    doc.add(FeatureKind::Point(PointDef::Coordinates {
        x: 5.0,
        y: 5.0,
        z: 40.0,
    }));
    let point = doc.features.last().unwrap().id;
    doc.add(FeatureKind::Deform(Deform {
        bodies: vec![],
        mode: DeformMode::Point {
            point: Ref::Feature { id: point },
            direction: Ref::FrontPlane,
            distance: 5.0,
            radius: 20.0,
        },
        accuracy: 6,
    }));
    let d = Model::new(doc);
    ok(&d);
    assert!(volume(&d) > 4100.0, "{}", volume(&d));
    let b = d.state().body.as_ref().unwrap().shape.bbox().unwrap();
    assert!(b.max.z > 43.0, "{b:?}");
}

#[test]
fn indent_cut_takes_the_tool_out() {
    let mut doc = Document::new();
    block(&mut doc, DVec2::ZERO, DVec2::new(20.0, 20.0), 4.0);
    block(&mut doc, DVec2::new(5.0, 5.0), DVec2::new(15.0, 15.0), 10.0);
    let m = Model::new(doc);
    let target = m.state().bodies[0].id.clone();
    let tool = m.state().bodies[1].id.clone();
    let cut = with(
        &m,
        FeatureKind::Indent(Indent {
            target: target.clone(),
            tool,
            thickness: 0.0,
            clearance: 0.5,
            cut: true,
        }),
    );
    let t = cut.state().solid(&target).unwrap();
    let v = t.geometry.shape.mass_props().unwrap().volume;
    // The tool offset by the clearance has rounded upright edges.
    let pocket = 10.0 * 10.0 + 4.0 * 10.0 * 0.5 + std::f64::consts::PI * 0.25;
    close(v, 1600.0 - pocket * 4.0, 1e-3);
}

#[test]
fn inserted_and_mirrored_parts_follow_their_file() {
    let dir = std::env::temp_dir().join(format!("of-insert-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("source.ofp");
    let source = bar();
    file::save(&source.doc, &path).unwrap();
    let mut doc = Document::new();
    doc.add(FeatureKind::InsertPart(InsertPart {
        path: path.display().to_string(),
        configuration: None,
        transfer: InsertTransfer::default(),
        mirror: Some(Ref::RightPlane),
        translation: [0.0, 0.0, 5.0],
        rotation: [0.0, 0.0, 0.0],
    }));
    let mut m = Model::new(doc);
    ok(&m);
    close(volume(&m), 4000.0, 1e-6);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.x, 0.0, 1e-6);
    close(b.min.x, -10.0, 1e-6);
    close(b.min.z, 5.0, 1e-6);
    // Edit the source: the insert picks it up.
    let mut changed = source.doc.clone();
    if let Some(f) = changed
        .features
        .iter_mut()
        .find(|f| matches!(f.kind, FeatureKind::Extrude(_)))
        && let FeatureKind::Extrude(e) = &mut f.kind
    {
        e.end1 = EndCondition::Blind { depth: 20.0 };
    }
    file::save(&changed, &path).unwrap();
    assert!(m.refresh_files());
    close(volume(&m), 2000.0, 1e-6);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn defeature_removes_small_holes_and_fillets() {
    let mut doc = Document::new();
    block(&mut doc, DVec2::ZERO, DVec2::new(20.0, 20.0), 10.0);
    let mut m = Model::new(doc);
    let edge = edge_at(&m, DVec3::new(10.0, 0.0, 10.0));
    m.doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![edge],
        faces: vec![],
        radius: 2.0,
    }));
    let mut sk = Sketch::new();
    let c = sk.add_point(DVec2::new(10.0, 10.0));
    sk.add_circle(c, 1.5).unwrap();
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    m.doc.add(FeatureKind::Extrude(Extrude {
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
    m.rebuild();
    ok(&m);
    assert!(volume(&m) < 4000.0);
    let d = with(
        &m,
        FeatureKind::Defeature(Defeature {
            bodies: vec![],
            faces: vec![],
            holes_below: Some(5.0),
            fillets_below: Some(5.0),
        }),
    );
    close(volume(&d), 4000.0, 1e-6);
}

#[test]
fn slicing_makes_section_curves() {
    let m = bar();
    let s = with(
        &m,
        FeatureKind::Slicing(Slicing {
            plane: Ref::FrontPlane,
            count: 4,
            spacing: 10.0,
            offset: 5.0,
            bodies: vec![],
        }),
    );
    let id = s.doc.features.last().unwrap().id;
    let curves = &s.state().curves[&id];
    assert_eq!(curves.segments.len(), 4);
    assert!(curves.segments.values().all(|c| c.count(Kind::Edge) == 4));
}

#[test]
fn copies_along_an_edge_and_mates() {
    let m = bar();
    let id = m.state().bodies[0].id.clone();
    let edge = edge_at(&m, DVec3::new(5.0, 0.0, 0.0));
    let copies = with(
        &m,
        FeatureKind::MoveBody(MoveBody {
            bodies: vec![id.clone()],
            translation: DVec3::ZERO,
            rotation: DVec3::ZERO,
            origin: DVec3::ZERO,
            copy: true,
            copies: 3,
            along: Some(bodies::Along {
                reference: edge,
                distance: 15.0,
            }),
            to_point: None,
            about: None,
            mates: vec![],
        }),
    );
    assert_eq!(copies.state().bodies.len(), 4);
    let b = copies.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.x.abs().max(b.min.x.abs()), 55.0, 1e-6);

    // Mate a second block's bottom onto the bar's top.
    let mut doc = m.doc.clone();
    block(&mut doc, DVec2::new(30.0, 0.0), DVec2::new(36.0, 6.0), 3.0);
    let two = Model::new(doc);
    let small = two.state().bodies[1].id.clone();
    let bottom = face_at(&two, DVec3::new(33.0, 3.0, 0.0));
    let top = face_at(&two, DVec3::new(5.0, 5.0, 40.0));
    let mated = with(
        &two,
        FeatureKind::MoveBody(MoveBody {
            bodies: vec![small.clone()],
            translation: DVec3::ZERO,
            rotation: DVec3::ZERO,
            origin: DVec3::ZERO,
            copy: false,
            copies: 1,
            along: None,
            to_point: None,
            about: None,
            mates: vec![bodies::BodyMate {
                kind: bodies::BodyMateKind::Coincident,
                moving: bottom,
                fixed: top,
            }],
        }),
    );
    let b = mated
        .state()
        .solid(&small)
        .unwrap()
        .geometry
        .shape
        .bbox()
        .unwrap();
    close(b.min.z, 40.0, 1e-6);
    close(b.max.z, 43.0, 1e-6);
}
