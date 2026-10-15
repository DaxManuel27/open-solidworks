//! Sheet metal: bodies made from flanges, and flat patterns checked against
//! the bend allowance formula.

use std::f64::consts::FRAC_PI_2;

mod sheet_common;

use of_model::sheet_metal::*;
use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::{Sketch, tools},
    *,
};
use sheet_common::*;

#[test]
fn open_profile_l_bracket_flattens_to_the_allowance() {
    let (t, r, k) = (2.0, 2.0, 0.44);
    let mut doc = Document::new();
    let s = polyline(
        &mut doc,
        &[DVec2::new(0.0, 30.0), DVec2::ZERO, DVec2::new(50.0, 0.0)],
    );
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, k),
        side: ThicknessSide::Normal,
        end1: Some(EndCondition::Blind { depth: 20.0 }),
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    // The sketch is the outside of the bend: legs 30 and 50 outside, 20 deep.
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.x - b.min.x, 50.0, 1e-6);
    close(b.max.y - b.min.y, 30.0, 1e-6);
    close(b.max.z - b.min.z, 20.0, 1e-6);
    assert_eq!(m.state().sheet_metal.len(), 1);
    assert_eq!(m.state().sheet_metal[0].bends.len(), 1);
    let formed = volume(&m);

    flatten(&mut m);
    ok(&m);
    let setback = r + t;
    let ba = FRAC_PI_2 * (r + k * t);
    let length = (30.0 - setback) + (50.0 - setback) + ba;
    let [a, b, c] = sizes(&m);
    close(a, t, 1e-6);
    close(b, 20.0, 1e-6);
    close(c, length, 1e-6);
    close(volume(&m), length * 20.0 * t, 1e-6);
    // The neutral fibre keeps its length, so the volume is unchanged for k = 0.5
    // only; here it differs by the stretch of the bend.
    assert!((volume(&m) - formed).abs() < 20.0);
    let sm = &m.state().sheet_metal[0];
    assert_eq!(sm.bend_lines.len(), 1);
}

#[test]
fn edge_flange_on_a_tab() {
    let (t, r, k) = (1.5, 1.5, 0.44);
    let mut doc = Document::new();
    let s = rectangle(&mut doc, 60.0, 40.0);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, k),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    let edge = edge_at(&m, DVec3::new(30.0, 40.0, t));
    m.doc.add(FeatureKind::EdgeFlange(EdgeFlange {
        edges: vec![edge],
        angle: FRAC_PI_2,
        length: FlangeLength::Blind { length: 20.0 },
        measure: LengthMeasure::OuterVirtualSharp,
        position: FlangePosition::MaterialInside,
        reverse: false,
        gap: 0.1,
        start_offset: 0.0,
        end_offset: 0.0,
        position_offset: 0.0,
        bend: BendOverride::default(),
    }));
    m.rebuild();
    ok(&m);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.y, 40.0, 1e-6);
    close(b.max.z, 20.0, 1e-6);
    close(b.min.z, 0.0, 1e-6);
    assert_eq!(m.state().sheet_metal[0].bends.len(), 1);

    flatten(&mut m);
    ok(&m);
    let ba = FRAC_PI_2 * (r + k * t);
    let length = (40.0 - (r + t)) + ba + (20.0 - (r + t));
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.min.z, 0.0, 1e-6);
    close(b.max.z, t, 1e-6);
    close(b.max.x - b.min.x, 60.0, 1e-6);
    close(b.max.y - b.min.y, length, 1e-6);
}

#[test]
fn neutral_k_factor_keeps_volume() {
    let (t, r, k) = (2.0, 3.0, 0.5);
    let mut doc = Document::new();
    let s = polyline(
        &mut doc,
        &[
            DVec2::new(0.0, 30.0),
            DVec2::ZERO,
            DVec2::new(50.0, 0.0),
            DVec2::new(50.0, 20.0),
        ],
    );
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, k),
        side: ThicknessSide::Reverse,
        end1: Some(EndCondition::Blind { depth: 30.0 }),
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    assert_eq!(m.state().sheet_metal[0].bends.len(), 2);
    let formed = volume(&m);
    let flat = flatten(&mut m);
    ok(&m);
    // With k = 0.5 the neutral surface is the middle of the sheet, so the flat
    // keeps the formed volume.
    close(volume(&m), formed, 1e-6 * formed);
    assert_eq!(m.state().sheet_metal[0].bend_lines.len(), 2);
    m.doc.feature_mut(flat).unwrap().suppressed = true;
    m.rebuild();
    close(volume(&m), formed, 1e-9 * formed);
}

#[test]
fn hole_across_a_tangent_line_unfolds() {
    let (t, r, k) = (2.0, 2.0, 0.44);
    let mut doc = Document::new();
    let s = polyline(
        &mut doc,
        &[DVec2::new(0.0, 30.0), DVec2::ZERO, DVec2::new(50.0, 0.0)],
    );
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, k),
        side: ThicknessSide::Normal,
        end1: Some(EndCondition::Blind { depth: 30.0 }),
        end2: None,
        reverse: false,
    }));
    // A Ø4 hole through the upright leg at y = 5, reaching into the bend
    // (which runs from y = 4 down).
    let mut sk = Sketch::new();
    sk.add_circle_at(DVec2::new(-15.0, 5.0), 2.0);
    let hole = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::RightPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: hole,
        cut: true,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: 5.0 },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    flatten(&mut m);
    ok(&m);
    let [a, b, c] = sizes(&m);
    let length = (30.0 - (r + t)) + (50.0 - (r + t)) + FRAC_PI_2 * (r + k * t);
    close(a, t, 1e-6);
    close(b, 30.0, 1e-6);
    close(c, length, 1e-6);
    let full = length * 30.0 * t;
    let v = volume(&m);
    assert!(v < full - 10.0 && v > full - 40.0, "{v} vs {full}");
    // The hole's wall keeps the name the cut gave it.
    let names = &m.state().body.as_ref().unwrap().names;
    assert!(
        names.faces.iter().flatten().any(|n| n.starts_with("4.s")),
        "{:?}",
        names.faces
    );
}

fn tray(sheet: SheetMetalParams) -> Model {
    let mut doc = Document::new();
    let s = rectangle(&mut doc, 100.0, 60.0);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet,
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    let t = m.state().sheet_metal[0].params.thickness;
    let edges = vec![
        edge_at(&m, DVec3::new(50.0, 0.0, t)),
        edge_at(&m, DVec3::new(100.0, 30.0, t)),
        edge_at(&m, DVec3::new(50.0, 60.0, t)),
        edge_at(&m, DVec3::new(0.0, 30.0, t)),
    ];
    m.doc.add(FeatureKind::EdgeFlange(flange(edges, 25.0)));
    m.rebuild();
    ok(&m);
    m
}

#[test]
fn tray_with_four_flanges_flattens_and_reforms() {
    let steel16 = SheetMetalParams {
        gauge: Some(GaugeChoice {
            table: "steel".into(),
            gauge: "16".into(),
            radius: None,
        }),
        ..Default::default()
    };
    let mut m = tray(steel16);
    let p = m.state().sheet_metal[0].params.clone();
    let (t, r) = (p.thickness, p.radius);
    assert_eq!(m.state().sheet_metal[0].bends.len(), 4);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.x - b.min.x, 100.0, 1e-6);
    close(b.max.y - b.min.y, 60.0, 1e-6);
    close(b.max.z - b.min.z, 25.0, 1e-6);
    let formed = volume(&m);

    let flat = flatten(&mut m);
    ok(&m);
    let ba = FRAC_PI_2 * (r + 0.44 * t);
    let across = |side: f64| side - 2.0 * (r + t) + 2.0 * ba + 2.0 * (25.0 - (r + t));
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.x - b.min.x, across(100.0), 1e-6);
    close(b.max.y - b.min.y, across(60.0), 1e-6);
    close(b.max.z - b.min.z, t, 1e-6);
    assert_eq!(m.state().sheet_metal[0].bend_lines.len(), 4);
    // The fixed face is the underside; seen from it the flanges bend away.
    assert!(m.state().sheet_metal[0].bend_lines.iter().all(|l| !l.up));

    // Back to formed, then a thinner gauge: the flat follows.
    m.doc.feature_mut(flat).unwrap().suppressed = true;
    m.rebuild();
    close(volume(&m), formed, 1e-9 * formed);
    if let FeatureKind::BaseFlange(bf) = &mut m.doc.features[1].kind {
        bf.sheet.gauge.as_mut().unwrap().gauge = "20".into();
    }
    m.doc.feature_mut(flat).unwrap().suppressed = false;
    m.invalidate_from(0);
    m.rebuild();
    ok(&m);
    let p = m.state().sheet_metal[0].params.clone();
    assert!(p.thickness < t);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.z - b.min.z, p.thickness, 1e-6);
}

#[test]
fn short_flange_gets_reliefs() {
    let (t, r) = (2.0, 2.0);
    let mut doc = Document::new();
    let s = rectangle(&mut doc, 80.0, 50.0);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, r, 0.5),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    let e = edge_at(&m, DVec3::new(40.0, 50.0, t));
    let mut f = flange(vec![e], 15.0);
    f.start_offset = 20.0;
    f.end_offset = 20.0;
    m.doc.add(FeatureKind::EdgeFlange(f));
    m.rebuild();
    ok(&m);
    let solid = volume(&m);
    // Two reliefs of width t/2, reaching t/2 past the bend, through the sheet.
    let w = 0.5 * t;
    let plate = 80.0 * 50.0 * t;
    let strip = 40.0 * (r + t) * t;
    let reliefs = 2.0 * w * (r + t + w) * t;
    let bend = FRAC_PI_2 * ((r + t).powi(2) - r * r) / 2.0 * 40.0;
    let wall = (15.0 - (r + t)) * t * 40.0;
    close(solid, plate - strip - reliefs + bend + wall, 1e-6 * solid);
    flatten(&mut m);
    ok(&m);
}

#[test]
fn flange_positions_place_the_bend() {
    let (t, r) = (1.0, 2.0);
    let outer = |position: FlangePosition| {
        let mut doc = Document::new();
        let s = rectangle(&mut doc, 40.0, 30.0);
        doc.add(FeatureKind::BaseFlange(BaseFlange {
            sketch: s,
            sheet: params(t, r, 0.5),
            side: ThicknessSide::Normal,
            end1: None,
            end2: None,
            reverse: false,
        }));
        let mut m = Model::new(doc);
        let e = edge_at(&m, DVec3::new(20.0, 30.0, t));
        let mut f = flange(vec![e], 10.0);
        f.position = position;
        m.doc.add(FeatureKind::EdgeFlange(f));
        m.rebuild();
        ok(&m);
        m.state().body.as_ref().unwrap().shape.bbox().unwrap().max.y
    };
    close(outer(FlangePosition::MaterialInside), 30.0, 1e-6);
    close(outer(FlangePosition::MaterialOutside), 30.0 + t, 1e-6);
    close(outer(FlangePosition::BendOutside), 30.0 + r + t, 1e-6);
    close(outer(FlangePosition::BendFromVirtualSharp), 30.0, 1e-6);
}

#[test]
fn tab_merges_into_the_sheet() {
    let t = 2.0;
    let mut doc = Document::new();
    let s = rectangle(&mut doc, 40.0, 30.0);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, 1.0, 0.5),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    // A tab sketched on the top face, overhanging the right side by 10.
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(30.0, 10.0), DVec2::new(50.0, 20.0));
    let mut m = Model::new(doc);
    let top = {
        let shape = &m.state().body.as_ref().unwrap().shape;
        let i = (0..shape.count(Kind::Face))
            .find(|i| shape.face_info(*i).unwrap().normal.z > 0.99)
            .unwrap();
        m.state().ref_for(Kind::Face, i).unwrap()
    };
    let tab = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: top,
        sketch: sk,
    }));
    m.doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: tab,
        sheet: params(t, 1.0, 0.5),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    m.rebuild();
    ok(&m);
    assert_eq!(m.state().bodies.len(), 1);
    assert_eq!(m.state().sheet_metal.len(), 1);
    close(volume(&m), (40.0 * 30.0 + 10.0 * 10.0) * t, 1e-6);
    flatten(&mut m);
    ok(&m);
    close(volume(&m), (40.0 * 30.0 + 10.0 * 10.0) * t, 1e-6);
}

#[test]
fn flange_up_to_vertex_and_material_values() {
    let mut doc = Document::new();
    doc.material = materials::by_name("Stainless Steel 304");
    let s = rectangle(&mut doc, 40.0, 30.0);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: SheetMetalParams {
            thickness: 2.0,
            use_material: true,
            ..Default::default()
        },
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    let p = m.state().sheet_metal[0].params.clone();
    close(p.radius, 3.0, 1e-12);
    assert_eq!(p.allowance, Allowance::KFactor { k: 0.45 });
    let e = edge_at(&m, DVec3::new(20.0, 30.0, 2.0));
    m.doc.add(FeatureKind::Point(PointDef::Coordinates {
        x: 0.0,
        y: 0.0,
        z: 17.0,
    }));
    m.rebuild();
    let point = m.doc.features.last().unwrap().id;
    let mut f = flange(vec![e], 1.0);
    f.length = FlangeLength::UpToVertex {
        target: Ref::Feature { id: point },
    };
    m.doc.add(FeatureKind::EdgeFlange(f));
    m.rebuild();
    ok(&m);
    close(
        m.state().body.as_ref().unwrap().shape.bbox().unwrap().max.z,
        17.0,
        1e-6,
    );
}
