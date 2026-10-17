//! Flat pattern DXF: a sheet metal tray written at true size and read back.

mod common;

use std::f64::consts::{FRAC_PI_2, PI};

use common::{entities, get, num, pairs};
use of_drawing::dxf::DxfVersion;
use of_drawing::flat;
use of_drawing::standard::Standard;
use of_model::flat::sheet_properties;
use of_model::geom::{CurveKind, DVec2, DVec3, Kind};
use of_model::sheet_metal::*;
use of_model::sketch::{Sketch, tools};
use of_model::*;

/// A 60 × 40 × 1 tray with 15 mm flanges on two adjacent edges and a Ø8 hole.
fn tray() -> Model {
    let t = 1.0;
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(60.0, 40.0));
    sk.add_circle_at(DVec2::new(20.0, 20.0), 4.0);
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: SheetMetalParams {
            thickness: t,
            radius: 1.0,
            allowance: Allowance::KFactor { k: 0.5 },
            ..Default::default()
        },
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    let edge = |m: &Model, p: DVec3| {
        let shape = &m.state().body.as_ref().unwrap().shape;
        let e = (0..shape.count(Kind::Edge))
            .find(|e| {
                let i = shape.edge_info(*e).unwrap();
                i.kind == CurveKind::Line && (i.mid - p).length() < 1e-6
            })
            .expect("edge");
        m.state().ref_for(Kind::Edge, e).unwrap()
    };
    let edges = vec![
        edge(&m, DVec3::new(30.0, 40.0, t)),
        edge(&m, DVec3::new(60.0, 20.0, t)),
    ];
    m.doc.add(FeatureKind::EdgeFlange(EdgeFlange {
        edges,
        angle: FRAC_PI_2,
        length: FlangeLength::Blind { length: 15.0 },
        measure: LengthMeasure::OuterVirtualSharp,
        position: FlangePosition::MaterialInside,
        reverse: false,
        gap: 0.2,
        start_offset: 0.0,
        end_offset: 0.0,
        position_offset: 0.0,
        bend: BendOverride::default(),
        angle_from: None,
        profiles: Vec::new(),
    }));
    m.rebuild();
    for s in m.statuses() {
        assert!(s.error.is_none(), "{:?}", s.error);
    }
    m
}

#[test]
fn flat_dxf_has_cut_and_bend_layers_at_true_size() {
    let m = tray();
    let (outline, props) = sheet_properties(m.state(), 0).unwrap();
    for version in [DxfVersion::R12, DxfVersion::R2018] {
        let text = flat::dxf(
            std::slice::from_ref(&outline),
            true,
            &Standard::iso(),
            version,
        )
        .unwrap();
        let p = pairs(&text);
        let on = |layer: &str, kind: &str| {
            entities(&p, kind)
                .into_iter()
                .filter(|e| get(e, 8) == Some(layer))
                .collect::<Vec<_>>()
        };
        // Both flanges bend the same way, so all bend lines share a layer.
        let up = on(flat::BEND_UP, "LINE");
        let down = on(flat::BEND_DOWN, "LINE");
        assert_eq!(up.len() + down.len(), 2, "{version:?}");
        assert!(up.is_empty() || down.is_empty());
        // The hole is a full circle on CUT, at true size.
        let circles: Vec<_> = on(flat::CUT, "CIRCLE")
            .into_iter()
            .chain(
                on(flat::CUT, "ARC")
                    .into_iter()
                    .filter(|e| (num(e, 51) - num(e, 50)).rem_euclid(360.0) < 1e-6),
            )
            .collect();
        assert_eq!(circles.len(), 1, "{version:?}");
        assert!((num(circles[0], 40) - 4.0).abs() < 1e-6);
        // Model space extents of the cut geometry match the flat size.
        let cut_lines = on(flat::CUT, "LINE");
        let xs: Vec<f64> = cut_lines
            .iter()
            .flat_map(|e| [num(e, 10), num(e, 11)])
            .collect();
        let ys: Vec<f64> = cut_lines
            .iter()
            .flat_map(|e| [num(e, 20), num(e, 21)])
            .collect();
        let span = |v: &[f64]| {
            v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min)
        };
        let (a, b) = (span(&xs), span(&ys));
        assert!((a.max(b) - props.flat_length).abs() < 1e-4, "{a} {b}");
        assert!((a.min(b) - props.flat_width).abs() < 1e-4, "{a} {b}");
        // Bend notes name the direction, angle and radius (R12 escapes °).
        let note = match version {
            DxfVersion::R12 => "90\\U+00B0 R1",
            DxfVersion::R2018 => "90° R1",
        };
        assert!(text.contains(note), "{version:?}");
    }
    // The Ø8 hole is a cut-out. The two bend regions also touch at one point
    // in the flat beside the corner relief, closing off a sliver.
    let holes: Vec<f64> = outline
        .loops
        .iter()
        .filter(|l| !l.outer)
        .map(|l| l.length)
        .collect();
    assert!(
        holes.iter().any(|l| (l - PI * 8.0).abs() < 1e-6),
        "{holes:?}"
    );
    assert!(
        holes
            .iter()
            .all(|l| (l - PI * 8.0).abs() < 1e-6 || *l < 2.0),
        "{holes:?}"
    );
}

#[test]
fn flat_pattern_view_with_bend_lines_notes_and_tables() {
    use common::{Scratch, extent, save, texts};
    use of_drawing::annotations::{Annotation, AnnotationKind};
    use of_drawing::document::*;
    use of_drawing::drawing::Drawing;
    use of_drawing::graphics::layers;
    use of_drawing::tables::{Table, TableKind};

    let dir = Scratch::new("flatview");
    let m = tray();
    let path = save(&m.doc, &dir.path("tray.ofp"));
    let (_, props) = sheet_properties(m.state(), 0).unwrap();
    let view = View::new(
        2,
        "Flat".into(),
        ViewKind::Model {
            model: ModelSource {
                path: path.to_string_lossy().into_owned(),
                hash: None,
                configuration: None,
                flat: Some(FlatView::default()),
                bodies: None,
            },
            orientation: Orientation::Front,
            exploded: false,
        },
        [150.0, 150.0],
    );
    let table = |id: u32, at: [f64; 2], kind: TableKind| Annotation {
        id,
        attach: None,
        layer: None,
        kind: AnnotationKind::Table(Box::new(Table {
            at,
            anchor: Default::default(),
            kind,
            text_height: None,
            max_rows: None,
            continued: Vec::new(),
        })),
    };
    let mut doc = DrawingDocument {
        sheets: vec![Sheet {
            id: 1,
            name: "Sheet1".into(),
            size: SheetSize::A3,
            portrait: false,
            scale: Scale::ONE,
            projection: Projection::Third,
            format: SheetFormat::default(),
            properties_view: None,
            views: vec![view],
            annotations: vec![
                table(
                    3,
                    [20.0, 280.0],
                    TableKind::Bend {
                        view: 2,
                        show_tags: true,
                    },
                ),
                table(
                    4,
                    [250.0, 280.0],
                    TableKind::Punch {
                        view: 2,
                        origin: None,
                    },
                ),
            ],
        }],
        ..Default::default()
    };
    doc.next_id = 100;
    doc.validate().unwrap();
    let mut d = Drawing::new(doc);
    let r = d.render(1).unwrap();
    for v in &r.views {
        assert_eq!(v.error, None, "view {}", v.id);
    }
    let items = &r.graphics.items;
    // The flat outline at 1:1, square to the paper.
    let outline: Vec<_> = items
        .iter()
        .filter(|i| i.layer == layers::VISIBLE)
        .collect();
    let [lo, hi] = extent(&outline);
    let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
    assert!((w.max(h) - props.flat_length).abs() < 1e-3, "{w} × {h}");
    assert!((w.min(h) - props.flat_width).abs() < 1e-3, "{w} × {h}");
    let bends = items
        .iter()
        .filter(|i| i.layer == flat::BEND_UP || i.layer == flat::BEND_DOWN)
        .count();
    assert_eq!(bends, 2);
    let words = texts(items).join(" ");
    assert!(words.contains("90° R1"), "{words}");
    assert!(
        words.contains("DIRECTION") && words.contains("K 0.5"),
        "{words}"
    );
    assert!(words.contains("PUNCH ID"), "{words}");
}
