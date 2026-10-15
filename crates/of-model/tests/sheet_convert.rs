//! Sheet metal from solids, rips, unfold and fold, and normal cuts.

mod sheet_common;

use of_model::sheet_metal::*;
use of_model::{
    geom::{DVec2, DVec3, Kind, SurfaceKind},
    sketch::{Sketch, tools},
    *,
};
use sheet_common::*;

/// A 60 × 40 × 30 box on the front plane.
fn box_part() -> Model {
    let mut doc = Document::new();
    let s = rectangle(&mut doc, 60.0, 40.0);
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: 30.0 },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let m = Model::new(doc);
    ok(&m);
    m
}

fn face_facing(m: &Model, n: DVec3) -> Ref {
    let shape = &m.state().body.as_ref().unwrap().shape;
    let i = (0..shape.count(Kind::Face))
        .find(|i| {
            let f = shape.face_info(*i).unwrap();
            f.kind == SurfaceKind::Plane && f.normal.dot(n) > 0.999
        })
        .unwrap();
    m.state().ref_for(Kind::Face, i).unwrap()
}

#[test]
fn convert_a_box_into_an_open_tray() {
    let t = 1.5;
    let mut m = box_part();
    let bottom = face_facing(&m, -DVec3::Z);
    let edges = vec![
        edge_at(&m, DVec3::new(30.0, 0.0, 0.0)),
        edge_at(&m, DVec3::new(60.0, 20.0, 0.0)),
        edge_at(&m, DVec3::new(30.0, 40.0, 0.0)),
        edge_at(&m, DVec3::new(0.0, 20.0, 0.0)),
    ];
    m.doc
        .add(FeatureKind::ConvertToSheetMetal(ConvertToSheetMetal {
            fixed_face: bottom,
            sheet: params(t, 1.0, 0.5),
            bend_edges: edges,
            rip_gap: 0.2,
            keep_body: false,
        }));
    m.rebuild();
    ok(&m);
    assert_eq!(m.state().sheet_metal.len(), 1);
    assert_eq!(m.state().sheet_metal[0].bends.len(), 4);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.x - b.min.x, 60.0, 1e-6);
    close(b.max.z - b.min.z, 30.0, 1e-6);
    flatten(&mut m);
    ok(&m);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.z - b.min.z, t, 1e-6);
    // Across the long side: base, two bends and two walls.
    let ba = std::f64::consts::FRAC_PI_2 * (1.0 + 0.5 * t);
    let across = 60.0 - 2.0 * (1.0 + t) + 2.0 * ba + 2.0 * (30.0 - (1.0 + t));
    close(b.max.x - b.min.x, across, 1e-6);
}

#[test]
fn insert_bends_into_a_ripped_shell() {
    let t = 2.0;
    let mut m = box_part();
    let top = face_facing(&m, DVec3::Z);
    m.doc.add(FeatureKind::Shell(ShellDef {
        faces: vec![top],
        thickness: t,
        outward: false,
    }));
    m.rebuild();
    ok(&m);
    let bottom = face_facing(&m, -DVec3::Z);
    let rips = [(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (0.0, 40.0)]
        .iter()
        .map(|(x, y)| RipEdge {
            edge: edge_at(&m, DVec3::new(*x, *y, 15.0)),
            direction: RipDirection::Both,
        })
        .collect();
    m.doc.add(FeatureKind::InsertBends(InsertBends {
        fixed_face: bottom,
        sheet: params(t, 2.0, 0.44),
        rips,
        rip_gap: 0.2,
    }));
    m.rebuild();
    ok(&m);
    assert_eq!(m.state().sheet_metal[0].params.thickness, t);
    assert_eq!(m.state().sheet_metal[0].bends.len(), 4);
    flatten(&mut m);
    ok(&m);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.z - b.min.z, t, 1e-6);
}

fn plate_with_flange() -> Model {
    let (t, r) = (1.0, 1.0);
    let mut doc = Document::new();
    let s = rectangle(&mut doc, 60.0, 40.0);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, 0.5),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    let e = edge_at(&m, DVec3::new(30.0, 40.0, t));
    m.doc.add(FeatureKind::EdgeFlange(flange(vec![e], 20.0)));
    m.rebuild();
    ok(&m);
    m
}

#[test]
fn cut_across_a_bend_while_unfolded() {
    let mut m = plate_with_flange();
    let formed = volume(&m);
    m.doc.add(FeatureKind::Unfold(Unfold::default()));
    m.rebuild();
    ok(&m);
    let flat_volume = volume(&m);
    // A slot across the bend region of the flat (the bend lies near y = 38..40).
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(25.0, 30.0), DVec2::new(35.0, 48.0));
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
    let removed_flat = flat_volume - volume(&m);
    close(removed_flat, 10.0 * 18.0, 1e-6);
    m.doc.add(FeatureKind::Fold(Fold {}));
    m.rebuild();
    ok(&m);
    // Folded with the slot: the flange stands again, minus the slot.
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.z, 20.0, 1e-4);
    let removed = formed - volume(&m);
    assert!(
        (removed - removed_flat).abs() < 0.05 * removed_flat,
        "{removed} vs {removed_flat}"
    );
    // And it still flattens, with the slot where it was cut.
    flatten(&mut m);
    ok(&m);
    close(volume(&m), flat_volume - removed_flat, 1e-3);
}

#[test]
fn normal_cut_through_a_bend() {
    let mut m = plate_with_flange();
    let formed = volume(&m);
    // A circle seen from above, over the bend.
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::new(30.0, 39.0), 3.0);
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 50.0,
        flip: false,
    }));
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::Feature { id: plane },
        sketch: sk,
    }));
    m.doc.add(FeatureKind::NormalCut(NormalCut { sketch: s }));
    m.rebuild();
    ok(&m);
    assert!(volume(&m) < formed - 1.0);
    flatten(&mut m);
    ok(&m);
}

#[test]
fn edge_flange_merges_two_bodies() {
    let t = 1.0;
    let mut doc = Document::new();
    let s = rectangle(&mut doc, 60.0, 40.0);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, 1.0, 0.5),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    // A second plate standing at y = 40 .. 41, from z = 10 up.
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(0.0, -30.0), DVec2::new(60.0, -10.0));
    let wall_plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::TopPlane,
        distance: 40.0,
        flip: false,
    }));
    let s2 = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::Feature { id: wall_plane },
        sketch: sk,
    }));
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s2,
        sheet: params(t, 1.0, 0.5),
        side: ThicknessSide::Reverse,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    assert_eq!(m.state().bodies.len(), 2);
    let e = edge_at(&m, DVec3::new(30.0, 40.0, t));
    let shape = &m.state().body.as_ref().unwrap().shape;
    let target = (0..shape.count(Kind::Edge))
        .find(|i| {
            let info = shape.edge_info(*i).unwrap();
            (info.mid - DVec3::new(30.0, 40.0, 10.0)).length() < 1e-6
                || (info.mid - DVec3::new(30.0, 39.0, 10.0)).length() < 1e-6
                || (info.mid - DVec3::new(30.0, 41.0, 10.0)).length() < 1e-6
        })
        .map(|i| m.state().ref_for(Kind::Edge, i).unwrap())
        .expect("the wall's lower edge");
    let mut f = flange(vec![e], 1.0);
    f.length = FlangeLength::UpToEdgeAndMerge { target };
    f.measure = LengthMeasure::OuterVirtualSharp;
    m.doc.add(FeatureKind::EdgeFlange(f));
    m.rebuild();
    ok(&m);
    assert_eq!(m.state().bodies.len(), 1);
    assert_eq!(m.state().sheet_metal.len(), 1);
    flatten(&mut m);
    ok(&m);
}
