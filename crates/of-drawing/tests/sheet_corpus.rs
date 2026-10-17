//! The Phase 5 sheet metal corpus: an electrical enclosure, a round-to-square
//! duct transition and a converted housing, each built in code and checked
//! numerically, with flat DXF read back and flat pattern drawings.
//! `OPENFORGE_CORPUS_DIR` keeps the parts, DXF files and PDFs.

mod common;

use std::f64::consts::{FRAC_PI_2, PI};
use std::path::PathBuf;

use common::{Scratch, entities, get, pairs};
use of_drawing::annotations::{Annotation, AnnotationKind};
use of_drawing::document::*;
use of_drawing::drawing::Drawing;
use of_drawing::dxf::DxfVersion;
use of_drawing::flat;
use of_drawing::standard::Standard;
use of_drawing::tables::{Table, TableKind};
use of_model::flat::sheet_properties;
use of_model::geom::{CurveKind, DVec2, DVec3, Kind, SurfaceKind};
use of_model::sheet_metal::*;
use of_model::sketch::{Sketch, tools};
use of_model::*;

const T: f64 = 1.5;
const R: f64 = 1.5;
const K: f64 = 0.44;

fn sheet(t: f64, r: f64, k: f64) -> SheetMetalParams {
    SheetMetalParams {
        thickness: t,
        radius: r,
        allowance: Allowance::KFactor { k },
        ..Default::default()
    }
}

fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
    assert!(m.state().body.as_ref().unwrap().shape.is_valid());
}

fn add(m: &mut Model, kind: FeatureKind) -> FeatureId {
    let id = m.doc.add(kind);
    m.rebuild();
    ok(m);
    id
}

/// Straight edges whose both ends satisfy `f`.
fn edges_where(m: &Model, f: impl Fn(DVec3, DVec3) -> bool) -> Vec<Ref> {
    let shape = &m.state().body.as_ref().unwrap().shape;
    (0..shape.count(Kind::Edge))
        .filter(|e| {
            let i = shape.edge_info(*e).unwrap();
            i.kind == CurveKind::Line && f(i.start, i.end)
        })
        .map(|e| m.state().ref_for(Kind::Edge, e).unwrap())
        .collect()
}

fn face_where(m: &Model, f: impl Fn(&of_model::geom::FaceInfo) -> bool) -> Ref {
    let shape = &m.state().body.as_ref().unwrap().shape;
    let i = (0..shape.count(Kind::Face))
        .filter(|i| f(&shape.face_info(*i).unwrap()))
        .max_by(|a, b| {
            shape
                .face_info(*a)
                .unwrap()
                .area
                .total_cmp(&shape.face_info(*b).unwrap().area)
        })
        .expect("face");
    m.state().ref_for(Kind::Face, i).unwrap()
}

fn keep(dir: &Scratch, name: &str) -> PathBuf {
    match std::env::var_os("OPENFORGE_CORPUS_DIR") {
        Some(d) => {
            std::fs::create_dir_all(&d).unwrap();
            PathBuf::from(d).join(name)
        }
        None => dir.path(name),
    }
}

/// A 200 × 120 × 40 enclosure of 1.5 mm steel: base, four flanges with
/// closed corners on one end and corner reliefs, hems on the long flanges,
/// a louver array in the floor and a hole cut across a bend.
fn enclosure() -> Model {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(200.0, 120.0));
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: sheet(T, R, K),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    // Four flanges up from the top edges of the floor.
    let top = |a: DVec3, b: DVec3| (a.z - T).abs() < 1e-9 && (b.z - T).abs() < 1e-9;
    let edges = edges_where(&m, top);
    assert_eq!(edges.len(), 4);
    add(
        &mut m,
        FeatureKind::EdgeFlange(EdgeFlange {
            edges,
            angle: FRAC_PI_2,
            length: FlangeLength::Blind { length: 40.0 },
            measure: LengthMeasure::OuterVirtualSharp,
            position: FlangePosition::MaterialInside,
            reverse: false,
            gap: 0.5,
            start_offset: 0.0,
            end_offset: 0.0,
            position_offset: 0.0,
            bend: BendOverride::default(),
            angle_from: None,
            profiles: Vec::new(),
        }),
    );
    // Hems folded inward along the top of the two long flanges.
    let zmax = m.state().body.as_ref().unwrap().shape.bbox().unwrap().max.z;
    let hem_edges = edges_where(&m, |a, b| {
        (a.z - zmax).abs() < 1e-6
            && (b.z - zmax).abs() < 1e-6
            && (a.y - b.y).abs() < 1e-9
            && ((a.y - T).abs() < 1e-6 || (a.y - (120.0 - T)).abs() < 1e-6)
    });
    assert_eq!(hem_edges.len(), 2);
    add(
        &mut m,
        FeatureKind::Hem(Hem {
            edges: hem_edges,
            kind: HemKind::Open { gap: 1.0 },
            length: 8.0,
            position: HemPosition::MaterialInside,
            reverse: false,
            gap: 0.5,
            allowance: None,
        }),
    );
    // Close the corner at x = 200, y = 120 with an overlap.
    let end = face_where(&m, |i| {
        i.kind == SurfaceKind::Plane
            && i.centroid.x > 195.0
            && i.centroid.y > 115.0
            && i.centroid.z > 5.0
            && i.normal.x > 0.3
    });
    add(
        &mut m,
        FeatureKind::ClosedCorner(ClosedCorner {
            faces: vec![end],
            kind: ClosedCornerKind::Overlap,
            gap: 0.2,
            overlap_ratio: 1.0,
            coplanar: false,
        }),
    );
    // Circular reliefs at the corners.
    add(
        &mut m,
        FeatureKind::CornerRelief(CornerRelief {
            bends: Vec::new(),
            kind: CornerReliefKind::Circular,
            ratio: 0.5,
            centered: true,
            slit: false,
        }),
    );
    // Three louvers in the floor.
    let mut pts = Sketch::new();
    for x in [60.0, 100.0, 140.0] {
        pts.add_point(DVec2::new(x, 60.0));
    }
    let placement = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: pts,
    }));
    let floor = face_where(&m, |i| {
        i.kind == SurfaceKind::Plane && i.normal.z > 0.999 && (i.point.z - T).abs() < 1e-9
    });
    add(
        &mut m,
        FeatureKind::FormTool(FormTool {
            shape: FormShape::Louver {
                length: 24.0,
                width: 5.0,
                height: 3.0,
            },
            face: floor,
            sketch: Some(placement),
            rotation: FRAC_PI_2,
            flip: false,
            punch_id: Some("LV-24".into()),
        }),
    );
    // A Ø6 hole straight across the front bend (y = 0 side), seen from the front.
    let mut hole = Sketch::new();
    hole.add_circle_at(DVec2::new(100.0, -3.0), 3.0);
    let plane = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::TopPlane,
        distance: -10.0,
        flip: false,
    }));
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::Feature { id: plane },
        sketch: hole,
    }));
    add(&mut m, FeatureKind::NormalCut(NormalCut { sketch: s }));
    m
}

#[test]
fn enclosure_flat_dxf_and_drawing() {
    let dir = Scratch::new("corpus-enclosure");
    let m = enclosure();
    let (outline, props) = sheet_properties(m.state(), 0).unwrap();

    // Across the side flanges: floor, two bends, two walls.
    let ba = FRAC_PI_2 * (R + K * T);
    let leg = 40.0 - (R + T) + ba;
    let (lo, hi) = outline.extents().unwrap();
    let span = [hi[0] - lo[0], hi[1] - lo[1]];
    let across = 200.0 - 2.0 * (R + T) + 2.0 * leg;
    assert!(
        span.iter().any(|s| (s - across).abs() < 1e-3),
        "{span:?} vs {across}"
    );
    // The long flanges carry their hems: longer than walls alone.
    let along = 120.0 - 2.0 * (R + T) + 2.0 * leg;
    assert!(span.iter().any(|s| *s > along + 2.0 * 8.0), "{span:?}");
    // Four flange bends and two hems' bends; three louvers; one hole.
    assert!(props.bends >= 6, "{props:?}");
    assert_eq!(props.forms, 3);
    assert!(props.cutouts >= 1);
    assert!(
        outline
            .marks
            .iter()
            .all(|k| k.punch_id.as_deref() == Some("LV-24"))
    );

    // Flat DXF read back: layers and extents at true size.
    for version in [DxfVersion::R12, DxfVersion::R2018] {
        let text = flat::dxf(
            std::slice::from_ref(&outline),
            true,
            &Standard::iso(),
            version,
        )
        .unwrap();
        let p = pairs(&text);
        let on = |layer: &str| {
            ["LINE", "ARC", "CIRCLE", "POLYLINE", "LWPOLYLINE"]
                .iter()
                .flat_map(|k| entities(&p, k))
                .filter(|e| get(e, 8) == Some(layer))
                .count()
        };
        assert!(on(flat::CUT) > 8, "{version:?}");
        assert_eq!(
            entities(&p, "LINE")
                .iter()
                .filter(|e| matches!(get(e, 8), Some(flat::BEND_UP | flat::BEND_DOWN)))
                .count(),
            props.bends.max(outline.bend_lines.len()),
            "{version:?}"
        );
        assert!(on(flat::FORM) >= 3, "{version:?}");
        std::fs::write(
            keep(&dir, &format!("enclosure-{version:?}.dxf").to_lowercase()),
            &text,
        )
        .unwrap();
    }

    // A drawing: flat pattern view, bend table and punch table.
    let path = keep(&dir, "enclosure.ofp");
    of_model::file::save(&m.doc, &path).unwrap();
    let view = View::new(
        2,
        "Flat".into(),
        ViewKind::Model {
            model: ModelSource {
                path: path.to_string_lossy().into_owned(),
                hash: None,
                configuration: None,
                flat: Some(FlatView::default()),
            },
            orientation: Orientation::Front,
            exploded: false,
        },
        [100.0, 120.0],
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
            scale: Scale::from_factor(0.5),
            projection: Projection::Third,
            format: SheetFormat::default(),
            properties_view: None,
            views: vec![view],
            annotations: vec![
                table(
                    3,
                    [260.0, 280.0],
                    TableKind::Bend {
                        view: 2,
                        show_tags: true,
                    },
                ),
                table(
                    4,
                    [260.0, 160.0],
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
    let sheet = d.doc.sheets[0].clone();
    let bend_rows = d.table_cells(&sheet, table_of(&sheet, 3)).unwrap().rows;
    let punch_rows = d.table_cells(&sheet, table_of(&sheet, 4)).unwrap().rows;
    assert_eq!(bend_rows.len(), outline.bend_lines.len());
    assert_eq!(punch_rows.len(), 3);
    assert!(punch_rows.iter().all(|r| r[4] == "LV-24" && r[5] == "3"));
    let r = d.render(1).unwrap();
    for v in &r.views {
        assert_eq!(v.error, None);
    }
    let pdf = of_drawing::pdf::write(&[r.graphics], &d.doc.standard, &Default::default()).unwrap();
    std::fs::write(keep(&dir, "enclosure.pdf"), pdf).unwrap();
}

fn table_of(sheet: &Sheet, id: u32) -> &Table {
    sheet
        .annotations
        .iter()
        .find_map(|a| match &a.kind {
            AnnotationKind::Table(t) if a.id == id => Some(t.as_ref()),
            _ => None,
        })
        .unwrap()
}

/// A square 60 × 60 duct end rising 50 mm to a Ø40 round end, 1 mm sheet.
fn duct(method: LoftMethod) -> Model {
    let mut doc = Document::new();
    // Both ends are open chains with a 2 mm seam gap at the bottom (y = -30).
    let mut sq = Sketch::new();
    let ids: Vec<_> = [
        (1.0, -30.0),
        (30.0, -30.0),
        (30.0, 30.0),
        (-30.0, 30.0),
        (-30.0, -30.0),
        (-1.0, -30.0),
    ]
    .iter()
    .map(|(x, y)| sq.add_point(DVec2::new(*x, *y)))
    .collect();
    for w in ids.windows(2) {
        sq.add_line(w[0], w[1]).unwrap();
    }
    let square = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sq,
    }));
    let top = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 50.0,
        flip: false,
    }));
    let mut rd = Sketch::new();
    let gap = 1.0 / 20.0;
    let c = rd.add_point(DVec2::ZERO);
    let a = rd.add_point(DVec2::from_angle(-FRAC_PI_2 + gap) * 20.0);
    let b = rd.add_point(DVec2::from_angle(-FRAC_PI_2 - gap) * 20.0);
    rd.add_arc(c, a, b).unwrap();
    let round = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::Feature { id: top },
        sketch: rd,
    }));
    doc.add(FeatureKind::LoftedBend(LoftedBend {
        profile1: square,
        profile2: round,
        method,
        facets: 12,
        deviation: 0.1,
        sheet: sheet(1.0, 1.0, 0.5),
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    m
}

/// Area of the ruled surface between the duct's two open profiles, matched by
/// arc length fraction from their seam and finely triangulated: the
/// profile side's development, computed independently of the model.
fn duct_profile_area() -> f64 {
    let square = [
        DVec3::new(1.0, -30.0, 0.0),
        DVec3::new(30.0, -30.0, 0.0),
        DVec3::new(30.0, 30.0, 0.0),
        DVec3::new(-30.0, 30.0, 0.0),
        DVec3::new(-30.0, -30.0, 0.0),
        DVec3::new(-1.0, -30.0, 0.0),
    ];
    let lens: Vec<f64> = square.windows(2).map(|w| (w[1] - w[0]).length()).collect();
    let total: f64 = lens.iter().sum();
    let lower = |f: f64| {
        let mut d = f * total;
        for (k, l) in lens.iter().enumerate() {
            if d <= *l || k == lens.len() - 1 {
                return square[k] + (square[k + 1] - square[k]) * (d / l).min(1.0);
            }
            d -= l;
        }
        unreachable!()
    };
    let gap = 1.0 / 20.0;
    let upper = |f: f64| {
        let a = -FRAC_PI_2 + gap + f * (2.0 * PI - 2.0 * gap);
        DVec3::new(20.0 * a.cos(), 20.0 * a.sin(), 50.0)
    };
    let n = 20_000;
    let tri = |a: DVec3, b: DVec3, c: DVec3| (b - a).cross(c - a).length() / 2.0;
    (0..n)
        .map(|i| {
            let (f0, f1) = (i as f64 / n as f64, (i + 1) as f64 / n as f64);
            let (a0, a1, b0, b1) = (lower(f0), lower(f1), upper(f0), upper(f1));
            tri(a0, a1, b1) + tri(a0, b1, b0)
        })
        .sum()
}

#[test]
fn duct_transition_develops_its_profile_side() {
    let dir = Scratch::new("corpus-duct");
    let expected = duct_profile_area();
    // Formed facets follow the surface within 0.1 mm; bent ones are 12 per
    // curved stretch, so their flat strays further from the smooth surface.
    for (method, tol) in [(LoftMethod::Formed, 2e-3), (LoftMethod::Bent, 2e-2)] {
        let m = duct(method);
        let (outline, props) = sheet_properties(m.state(), 0).unwrap();
        assert!(
            (props.flat_area - expected).abs() < tol * expected,
            "{method:?}: flat {} vs developed {expected}",
            props.flat_area
        );
        close(props.thickness, 1.0);
        assert!(!outline.bend_lines.is_empty());
        let text = flat::dxf(&[outline], false, &Standard::iso(), DxfVersion::R2018).unwrap();
        std::fs::write(
            keep(&dir, &format!("duct-{method:?}.dxf").to_lowercase()),
            text,
        )
        .unwrap();
    }
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

/// A 100 × 60 × 40 box shelled 2 mm open at the top, made sheet metal with
/// its vertical corners ripped.
#[test]
fn converted_housing_flattens_to_its_walls() {
    let t = 2.0;
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(100.0, 60.0));
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: 40.0 },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let mut m = Model::new(doc);
    let top = face_where(&m, |i| i.normal.z > 0.999);
    add(
        &mut m,
        FeatureKind::Shell(ShellDef {
            faces: vec![top],
            thickness: t,
            outward: false,
        }),
    );
    let bottom = face_where(&m, |i| i.normal.z < -0.999);
    let rips = edges_where(&m, |a, b| {
        (a.x - b.x).abs() < 1e-9
            && (a.y - b.y).abs() < 1e-9
            && [0.0, 100.0].iter().any(|x| (a.x - x).abs() < 1e-9)
            && [0.0, 60.0].iter().any(|y| (a.y - y).abs() < 1e-9)
    })
    .into_iter()
    .map(|edge| RipEdge {
        edge,
        direction: RipDirection::Both,
    })
    .collect::<Vec<_>>();
    assert_eq!(rips.len(), 4);
    add(
        &mut m,
        FeatureKind::InsertBends(InsertBends {
            fixed_face: bottom,
            sheet: sheet(t, 2.0, 0.44),
            rips,
            rip_gap: 0.2,
        }),
    );
    assert_eq!(m.state().sheet_metal[0].bends.len(), 4);
    let (outline, props) = sheet_properties(m.state(), 0).unwrap();
    assert_eq!(props.bends, 4);
    // Across the long side: floor less two bends' setbacks, two bends and
    // two walls.
    let ba = FRAC_PI_2 * (2.0 + 0.44 * t);
    let across = 100.0 - 2.0 * (2.0 + t) + 2.0 * ba + 2.0 * (40.0 - (2.0 + t));
    let (lo, hi) = outline.extents().unwrap();
    let span = [hi[0] - lo[0], hi[1] - lo[1]];
    assert!(
        span.iter().any(|s| (s - across).abs() < 1e-3),
        "{span:?} vs {across}"
    );
    let dir = Scratch::new("corpus-housing");
    of_model::file::save(&m.doc, &keep(&dir, "housing.ofp")).unwrap();
}
