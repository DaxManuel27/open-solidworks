//! Shaping surfaces: filled, ruled and radiate surfaces, trim and untrim,
//! extend, mid-surface and freeform.

use of_model::surfaces::*;
use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::{Sketch, tools},
    *,
};

fn sketch(doc: &mut Document, plane: Ref, sk: Sketch) -> FeatureId {
    doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch: sk }))
}

fn rectangle(doc: &mut Document, a: DVec2, b: DVec2) -> FeatureId {
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, a, b);
    sketch(doc, Ref::FrontPlane, sk)
}

fn planar(doc: &mut Document, a: DVec2, b: DVec2) -> FeatureId {
    let s = rectangle(doc, a, b);
    doc.add(FeatureKind::PlanarSurface(PlanarSurface {
        sketch: Some(s),
        edges: vec![],
    }))
}

fn block(doc: &mut Document, w: f64, h: f64, d: f64) {
    let s = rectangle(doc, DVec2::ZERO, DVec2::new(w, h));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: d },
        reverse: false,
        end2: None,
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

fn area(m: &Model, i: usize) -> f64 {
    m.state().surfaces[i].geometry.shape.area()
}

fn total_area(m: &Model) -> f64 {
    m.state()
        .surfaces
        .iter()
        .map(|s| s.geometry.shape.area())
        .sum()
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

fn with(m: &Model, kind: FeatureKind) -> Model {
    let mut doc = m.doc.clone();
    doc.add(kind);
    let m = Model::new(doc);
    ok(&m);
    m
}

#[test]
fn filled_surface_closes_an_open_box_into_a_solid() {
    let mut doc = Document::new();
    block(&mut doc, 10.0, 10.0, 10.0);
    let m = Model::new(doc);
    let top = face_at(&m, DVec3::new(5.0, 5.0, 10.0));
    let open = with(
        &m,
        FeatureKind::DeleteFace(DeleteFace {
            faces: vec![top],
            mode: DeleteFaceMode::Delete,
        }),
    );
    let rim: Vec<FillBoundary> = [
        DVec3::new(5.0, 0.0, 10.0),
        DVec3::new(10.0, 5.0, 10.0),
        DVec3::new(5.0, 10.0, 10.0),
        DVec3::new(0.0, 5.0, 10.0),
    ]
    .iter()
    .map(|p| FillBoundary {
        edge: edge_at(&open, *p),
        continuity: PatchContinuity::Contact,
        alternate_face: false,
    })
    .collect();
    let patched = with(
        &open,
        FeatureKind::FilledSurface(FilledSurface {
            boundary: rim.clone(),
            constraints: vec![],
            merge: true,
            create_solid: true,
        }),
    );
    assert!(patched.state().surfaces.is_empty());
    let v = patched
        .state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume;
    close(v, 1000.0, 1e-3);
    // Unmerged, the patch is a body of its own.
    let alone = with(
        &open,
        FeatureKind::FilledSurface(FilledSurface {
            boundary: rim,
            constraints: vec![],
            merge: false,
            create_solid: false,
        }),
    );
    assert_eq!(alone.state().surfaces.len(), 2);
    close(area(&alone, 1), 100.0, 1e-3);
}

#[test]
fn a_tangent_fill_follows_a_sphere() {
    // A spherical zone, radius 10, from the equator to 60° up: its top is a
    // hole of radius 5 at height 10 sin 60°.
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    let c = sk.add_point(DVec2::ZERO);
    let a = sk.add_point(DVec2::new(10.0, 0.0));
    let b = sk.add_point(DVec2::new(5.0, 75f64.sqrt()));
    sk.add_arc(c, a, b).unwrap();
    let profile = sketch(&mut doc, Ref::FrontPlane, sk);
    let mut axis = Sketch::new();
    let p = axis.add_point(DVec2::ZERO);
    let q = axis.add_point(DVec2::new(0.0, 1.0));
    let line = axis.add_line(p, q).unwrap();
    let axis_sketch = sketch(&mut doc, Ref::FrontPlane, axis);
    doc.add(FeatureKind::RevolvedSurface(RevolvedSurface {
        sketch: profile,
        axis: Ref::SketchEntity {
            sketch: axis_sketch,
            entity: line,
        },
        angle: RevolveAngle::Blind {
            angle: std::f64::consts::TAU,
        },
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    let top = 75f64.sqrt();
    let rim = |c: PatchContinuity| FillBoundary {
        edge: edge_at(&m, DVec3::new(-5.0, top, 0.0)),
        continuity: c,
        alternate_face: false,
    };
    let fill = |c| {
        with(
            &m,
            FeatureKind::FilledSurface(FilledSurface {
                boundary: vec![rim(c)],
                constraints: vec![],
                merge: false,
                create_solid: false,
            }),
        )
    };
    let flat = fill(PatchContinuity::Contact);
    close(area(&flat, 1), std::f64::consts::PI * 25.0, 1e-2);
    let round = fill(PatchContinuity::Tangent);
    let cap = std::f64::consts::TAU * 10.0 * (10.0 - top);
    close(area(&round, 1), cap, cap * 0.02);
}

#[test]
fn ruled_and_radiate_strips() {
    let mut doc = Document::new();
    planar(&mut doc, DVec2::ZERO, DVec2::new(20.0, 10.0));
    let m = Model::new(doc);
    let edge = edge_at(&m, DVec3::new(10.0, 0.0, 0.0));
    for direction in [
        RuledDirection::TangentToSurface,
        RuledDirection::NormalToSurface,
    ] {
        let r = with(
            &m,
            FeatureKind::RuledSurface(RuledSurface {
                edges: vec![edge.clone()],
                direction: direction.clone(),
                distance: 5.0,
                alternate_side: false,
                knit: true,
            }),
        );
        close(area(&r, 1), 100.0, 1e-6);
        let c = r.state().surfaces[1]
            .geometry
            .shape
            .face_info(0)
            .unwrap()
            .centroid;
        match direction {
            // In the plane, away from the face.
            RuledDirection::TangentToSurface => {
                close(c.z, 0.0, 1e-9);
                assert!(c.y < 0.0, "{c:?}");
            }
            _ => close(c.z.abs(), 2.5, 1e-9),
        }
    }
    // Radiate from a block's top edge, parallel to the front plane.
    let mut doc = Document::new();
    block(&mut doc, 20.0, 10.0, 5.0);
    let m = Model::new(doc);
    let edge = edge_at(&m, DVec3::new(10.0, 0.0, 5.0));
    let r = with(
        &m,
        FeatureKind::RadiateSurface(RadiateSurface {
            edges: vec![edge],
            plane: Ref::FrontPlane,
            distance: 4.0,
            flip: false,
        }),
    );
    close(area(&r, 0), 80.0, 1e-6);
    let c = r.state().surfaces[0]
        .geometry
        .shape
        .face_info(0)
        .unwrap()
        .centroid;
    close(c.z, 5.0, 1e-9);
}

#[test]
fn trim_keeps_or_removes_picked_pieces() {
    let mut doc = Document::new();
    planar(&mut doc, DVec2::ZERO, DVec2::new(20.0, 10.0));
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::RightPlane,
        distance: 8.0,
        flip: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    let body = m.state().surfaces[0].id.clone();
    let trim = |remove: bool, split_all: bool| {
        with(
            &m,
            FeatureKind::TrimSurface(TrimSurface {
                tool: Some(Ref::Feature { id: plane }),
                bodies: vec![body.clone()],
                pieces: if split_all {
                    vec![]
                } else {
                    vec![[2.0, 5.0, 0.0]]
                },
                remove,
                split_all,
            }),
        )
    };
    close(total_area(&trim(false, false)), 80.0, 1e-6);
    close(total_area(&trim(true, false)), 120.0, 1e-6);
    let split = trim(false, true);
    assert_eq!(split.state().surfaces.len(), 2);
    close(total_area(&split), 200.0, 1e-6);
}

#[test]
fn mutual_trim_cuts_two_surfaces_against_each_other() {
    let mut doc = Document::new();
    // A flat 20 × 20 sheet in the front plane and an upright 20 × 20 sheet
    // crossing it along y = 10.
    planar(&mut doc, DVec2::ZERO, DVec2::new(20.0, 20.0));
    let upright = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::TopPlane,
        distance: 10.0,
        flip: false,
    }));
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(0.0, -10.0), DVec2::new(20.0, 10.0));
    let s = sketch(&mut doc, Ref::Feature { id: upright }, sk);
    doc.add(FeatureKind::PlanarSurface(PlanarSurface {
        sketch: Some(s),
        edges: vec![],
    }));
    let m = Model::new(doc);
    ok(&m);
    let a = m.state().surfaces[0].id.clone();
    let b = m.state().surfaces[1].id.clone();
    let upright_z = m.state().surfaces[1].geometry.shape.bbox().unwrap();
    // Keep the flat sheet beyond the cut and the upright half on one side.
    let side = if upright_z.max.z > 5.0 { 5.0 } else { -5.0 };
    let t = with(
        &m,
        FeatureKind::TrimSurface(TrimSurface {
            tool: None,
            bodies: vec![a, b],
            pieces: vec![[10.0, 15.0, 0.0], [10.0, 10.0, side]],
            remove: false,
            split_all: false,
        }),
    );
    close(total_area(&t), 400.0, 1e-6);
    // The two kept pieces are knitted into one body.
    assert_eq!(t.state().surfaces.len(), 1);
}

#[test]
fn untrim_restores_a_trimmed_loft() {
    let mut doc = Document::new();
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 10.0,
        flip: false,
    }));
    let line = |doc: &mut Document, plane: Ref, p: DVec2, q: DVec2| {
        let mut sk = Sketch::new();
        let a = sk.add_point(p);
        let b = sk.add_point(q);
        let l = sk.add_line(a, b).unwrap();
        let id = sketch(doc, plane, sk);
        Ref::SketchContour {
            sketch: id,
            entities: vec![l],
        }
    };
    let one = line(
        &mut doc,
        Ref::FrontPlane,
        DVec2::ZERO,
        DVec2::new(20.0, 0.0),
    );
    // The second line is turned, so the loft is twisted, not flat.
    let two = line(
        &mut doc,
        Ref::Feature { id: plane },
        DVec2::new(0.0, 5.0),
        DVec2::new(20.0, -5.0),
    );
    doc.add(FeatureKind::LoftedSurface(LoftedSurface {
        profiles: vec![one, two],
        ruled: false,
    }));
    let cut = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::RightPlane,
        distance: 5.0,
        flip: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    let whole = total_area(&m);
    let body = m.state().surfaces[0].id.clone();
    let trimmed = with(
        &m,
        FeatureKind::TrimSurface(TrimSurface {
            tool: Some(Ref::Feature { id: cut }),
            bodies: vec![body],
            pieces: vec![[15.0, 0.0, 5.0]],
            remove: false,
            split_all: false,
        }),
    );
    assert!(total_area(&trimmed) < whole * 0.9);
    let face = face_at(&trimmed, DVec3::new(12.5, 0.0, 5.0));
    let restored = with(
        &trimmed,
        FeatureKind::UntrimSurface(UntrimSurface {
            faces: vec![face],
            mode: UntrimMode::Natural,
        }),
    );
    close(total_area(&restored), whole, 1e-4);
}

#[test]
fn extend_past_an_open_edge() {
    let mut doc = Document::new();
    planar(&mut doc, DVec2::ZERO, DVec2::new(20.0, 10.0));
    let m = Model::new(doc);
    let edge = edge_at(&m, DVec3::new(20.0, 5.0, 0.0));
    for linear in [false, true] {
        let e = with(
            &m,
            FeatureKind::ExtendSurface(ExtendSurface {
                edges: vec![edge.clone()],
                end: ExtendEnd::Distance { distance: 5.0 },
                linear,
            }),
        );
        assert_eq!(e.state().surfaces.len(), 1);
        close(area(&e, 0), 250.0, 1e-4);
        let bbox = e.state().surfaces[0].geometry.shape.bbox().unwrap();
        close(bbox.max.x, 25.0, 1e-4);
    }
}

#[test]
fn mid_surface_of_a_plate() {
    let mut doc = Document::new();
    block(&mut doc, 20.0, 20.0, 2.0);
    let m = Model::new(doc);
    let body = m.state().bodies[0].id.clone();
    let mid = with(
        &m,
        FeatureKind::MidSurface(MidSurface {
            pairs: vec![],
            body: Some(body),
            threshold: 5.0,
            position: 0.5,
            knit: true,
        }),
    );
    let face = mid.state().surfaces[0].geometry.shape.face_info(0).unwrap();
    close(face.area, 400.0, 1e-6);
    close(face.centroid.z, 1.0, 1e-6);
}

#[test]
fn freeform_lifts_the_middle_of_a_face() {
    let mut doc = Document::new();
    planar(&mut doc, DVec2::ZERO, DVec2::new(20.0, 20.0));
    let m = Model::new(doc);
    let face = face_at(&m, DVec3::new(10.0, 10.0, 0.0));
    let f = with(
        &m,
        FeatureKind::Freeform(Freeform {
            face,
            grid: 8,
            moves: vec![FreeformMove {
                u: 0.5,
                v: 0.5,
                offset: [0.0, 0.0, 3.0],
            }],
            continuity: PatchContinuity::Contact,
        }),
    );
    let shape = &f.state().surfaces[0].geometry.shape;
    assert!(shape.area() > 400.0 + 1e-3, "{}", shape.area());
    let bbox = shape.bbox().unwrap();
    assert!(bbox.max.z > 0.5, "{bbox:?}");
    // The edges stay put.
    close(bbox.min.x, 0.0, 1e-6);
    close(bbox.max.x, 20.0, 1e-6);
}
