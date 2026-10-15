//! Dimensions, annotations, symbols and tables on real parts and
//! assemblies: model items, driving dimensions, smart dimensions, fits, hole
//! callouts, GD&T and other symbols, BOMs with balloons, hole, revision and
//! general tolerance tables.

mod common;

use common::*;
use of_assembly::{Assembly, CompRef, DVec3, Pose};
use of_drawing::annot::{BalloonLayout, ModelItems};
use of_drawing::annotations::*;
use of_drawing::dims::{DimKind, Dimension, FitDisplay, Orient, Pick, Tolerance, Which};
use of_drawing::document::*;
use of_drawing::drawing::{Drawing, SheetRender};
use of_drawing::graphics::{Owner, Prim};
use of_drawing::symbols::{Characteristic, DatumRef, GtolRow};
use of_drawing::tables::{RevisionRow, Table, TableKind, Tags, ToleranceClass};
use of_model::geom::{CurveKind, DVec2, Kind};
use of_model::sketch::{ConstraintKind, Sketch, tools};
use of_model::*;

const SHEET: u32 = 1;
const FRONT: u32 = 2;

/// A 100 × 60 × 10 bracket from a dimensioned sketch (100, 60 and a Ø12
/// hole at (75, 30)), with two ISO M6 counterbored holes from the top.
fn bracket(dir: &Scratch) -> std::path::PathBuf {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    let rect = tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(100.0, 60.0));
    let lines: Vec<_> = rect.entities[4..8].to_vec();
    sk.add_constraint(ConstraintKind::Length {
        line: lines[0],
        value: 100.0,
    })
    .unwrap();
    sk.add_constraint(ConstraintKind::Length {
        line: lines[1],
        value: 60.0,
    })
    .unwrap();
    let c = sk.add_circle_at(DVec2::new(75.0, 30.0), 6.0);
    sk.add_constraint(ConstraintKind::Diameter {
        curve: c,
        value: 12.0,
    })
    .unwrap();
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: 10.0 },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let mut m = Model::new(doc);
    m.rebuild();
    // Counterbores on the top face (z = 10), drilled down.
    let top = face_facing(&m, DVec3::Z);
    let mut holes = Sketch::new();
    holes.add_point(DVec2::new(20.0, 30.0));
    holes.add_point(DVec2::new(40.0, 30.0));
    let hs = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: top,
        sketch: holes,
    }));
    m.doc.add(FeatureKind::Hole(Hole {
        sketch: hs,
        kind: HoleKind::Counterbore,
        standard: HoleStandard::Iso,
        size: "M6".into(),
        fit: HoleFit::Normal,
        end: HoleEnd::ThroughAll,
        reverse: false,
        diameter_override: None,
        cbore_diameter_override: None,
        cbore_depth_override: None,
        csink_diameter_override: None,
        csink_angle_override: None,
        cosmetic_thread: false,
    }));
    m.doc.properties.insert(
        "PartNo".into(),
        of_model::properties::Property::text("BRK-1"),
    );
    m.rebuild();
    assert!(m.statuses().iter().all(|s| s.error.is_none()));
    save(&m.doc, &dir.path("bracket.ofp"))
}

fn face_facing(m: &Model, n: DVec3) -> Ref {
    let body = m.state().body.as_ref().unwrap();
    for i in 0..body.shape.count(Kind::Face) {
        let f = body.shape.face_info(i).unwrap();
        if f.normal.dot(n) > 0.999 {
            return m.state().ref_for(Kind::Face, i).unwrap();
        }
    }
    panic!("no face facing {n}");
}

/// The rebuilt part in a file.
fn model(path: &std::path::Path) -> Model {
    let mut m = Model::new(of_model::file::load(path).unwrap());
    m.rebuild();
    m
}

/// The first edge of a part matching `f`.
fn edge(path: &std::path::Path, f: impl Fn(&of_model::geom::EdgeInfo) -> bool) -> CompRef {
    let m = model(path);
    let state = m.state();
    let body = state.body.as_ref().unwrap();
    for i in 0..body.shape.count(Kind::Edge) {
        let e = body.shape.edge_info(i).unwrap();
        if f(&e) {
            return CompRef {
                component: Vec::new(),
                entity: state.ref_for(Kind::Edge, i).unwrap(),
            };
        }
    }
    panic!("no such edge");
}

fn pick(view: u32, entity: CompRef) -> Pick {
    Pick {
        target: ViewRef { view, entity },
        point: Which::Auto,
    }
}

fn model_view(id: u32, path: &std::path::Path, o: Orientation, at: [f64; 2]) -> View {
    View::new(
        id,
        format!("View{id}"),
        ViewKind::Model {
            model: ModelSource {
                path: path.to_string_lossy().into_owned(),
                hash: None,
                configuration: None,
            },
            orientation: o,
            exploded: false,
        },
        at,
    )
}

fn drawing(standard: of_drawing::standard::Standard, views: Vec<View>) -> Drawing {
    let doc = DrawingDocument {
        standard,
        sheets: vec![Sheet {
            id: SHEET,
            name: "Sheet1".into(),
            size: SheetSize::A3,
            portrait: false,
            scale: Scale::ONE,
            projection: Projection::Third,
            format: SheetFormat {
                template: "iso".into(),
                visible: true,
            },
            properties_view: None,
            views,
            annotations: Vec::new(),
        }],
        properties: Default::default(),
        next_id: 100,
    };
    doc.validate().unwrap();
    Drawing::new(doc)
}

fn render(d: &mut Drawing) -> SheetRender {
    let r = d.render(SHEET).unwrap();
    for v in &r.views {
        assert_eq!(v.error, None, "view {}", v.id);
    }
    assert!(r.dangling.is_empty(), "{:?}", r.dangling);
    if let Ok(dir) = std::env::var("OPENFORGE_OUTPUT_DIR") {
        let bytes = of_drawing::pdf::write(
            std::slice::from_ref(&r.graphics),
            &d.doc.standard,
            &Default::default(),
        )
        .unwrap();
        let name = std::thread::current()
            .name()
            .unwrap_or("sheet")
            .replace("::", "-");
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.pdf")),
            bytes,
        )
        .unwrap();
        let sheet = &d.doc.sheets[0];
        let dxf = of_drawing::dxf::write_version(
            &[of_drawing::dxf::DxfSheet {
                graphics: &r.graphics,
                name: sheet.name.clone(),
                scale: sheet.scale.factor(),
                view_annotations: sheet
                    .annotations
                    .iter()
                    .filter(|a| a.view.is_some())
                    .map(|a| a.id)
                    .collect(),
            }],
            &d.doc.standard,
            of_drawing::dxf::DxfVersion::R2018,
        )
        .unwrap();
        std::fs::write(std::path::Path::new(&dir).join(format!("{name}.dxf")), dxf).unwrap();
    }
    r
}

/// Add an annotation and return its ID.
fn add(d: &mut Drawing, view: Option<u32>, kind: AnnotationKind) -> u32 {
    let id = d.doc.alloc_id();
    d.doc.sheets[0].annotations.push(Annotation {
        id,
        view,
        layer: None,
        kind,
    });
    d.doc.validate().unwrap();
    id
}

/// Text of one annotation: its runs joined, whitespace collapsed. Drafting
/// symbols are strokes, not text, so compare with [`plain`].
fn text_of(r: &SheetRender, id: u32) -> String {
    let runs: Vec<&str> = r
        .graphics
        .items
        .iter()
        .filter(|i| i.owner == Owner::Annotation { id })
        .filter_map(|i| match &i.prim {
            Prim::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    runs.join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Expected text without the stroked drafting symbols.
fn plain(s: &str) -> String {
    s.chars()
        .filter(|c| !of_drawing::text::is_symbol(*c))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn dims_of(d: &Drawing) -> Vec<(u32, Option<u32>, String)> {
    d.doc.sheets[0]
        .annotations
        .iter()
        .filter_map(|a| match &a.kind {
            AnnotationKind::Dimension(dim) => match &dim.kind {
                DimKind::Model { view, key } => Some((a.id, Some(*view), key.clone())),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn model_items_import_once_into_the_truest_view_and_drive_the_part() {
    let dir = Scratch::new("items");
    let path = bracket(&dir);
    let mut d = drawing(of_drawing::standard::Standard::iso(), Vec::new());
    let source = ModelSource {
        path: path.to_string_lossy().into_owned(),
        hash: None,
        configuration: None,
    };
    let views = d.add_standard_views(SHEET, source, false, None).unwrap();
    let ids: Vec<u32> = d.doc.sheets[0].views.iter().map(|v| v.id).collect();
    let new = d.model_items(SHEET, &ids, &ModelItems::default()).unwrap();
    d.doc.next_id = new.iter().map(|a| a.id + 1).max().unwrap();
    d.doc.sheets[0].annotations.extend(new);
    let dims = dims_of(&d);
    let keys: Vec<&str> = dims.iter().map(|x| x.2.as_str()).collect();
    // Three sketch dimensions and the extrusion depth; no Hole Wizard
    // overrides are set, so the hole adds a callout, not dimensions.
    assert_eq!(dims.len(), 4, "{keys:?}");
    assert!(keys.contains(&"f2.end1.depth"), "{keys:?}");
    // Sketch dimensions lie in the front plane: the front view shows them
    // true; the depth goes to the top or side view.
    for (_, view, key) in &dims {
        if key.starts_with("f1.") {
            assert_eq!(*view, Some(views.front), "{key}");
        } else {
            assert_ne!(*view, Some(views.front), "{key}");
        }
    }
    let r = render(&mut d);
    let all: Vec<String> = dims.iter().map(|x| text_of(&r, x.0)).collect();
    for want in ["100", "60", "⌀12", "10"] {
        assert!(all.iter().any(|t| *t == plain(want)), "{want} in {all:?}");
    }
    // The counterbore callout reads the Hole Wizard data.
    let callouts: Vec<String> = d.doc.sheets[0]
        .annotations
        .iter()
        .filter(|a| matches!(a.kind, AnnotationKind::HoleCallout { .. }))
        .map(|a| text_of(&r, a.id))
        .collect();
    assert_eq!(callouts, vec![plain("2X ⌀6.6 THRU ⌴⌀11 ↧6.4")]);

    // Importing again adds no duplicates.
    let again = d.model_items(SHEET, &ids, &ModelItems::default()).unwrap();
    assert!(
        again
            .iter()
            .all(|a| !matches!(a.kind, AnnotationKind::Dimension(_))),
        "{again:?}"
    );

    // Changing the 100 on the drawing rebuilds and saves the part.
    let (len_id, _, _) = dims
        .iter()
        .find(|x| text_of(&r, x.0) == "100")
        .unwrap()
        .clone();
    let saved = d.set_model_dimension(len_id, 120.0).unwrap();
    assert_eq!(saved, path);
    let m = model(&path);
    let bb = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    assert_close(bb.max.x - bb.min.x, 120.0, 1e-6);
    let r = render(&mut d);
    assert_eq!(text_of(&r, len_id), "120");
    // A value the part cannot rebuild with is refused and changes nothing.
    let bore = dims.iter().find(|x| text_of(&r, x.0) == "12").unwrap().0;
    let before = std::fs::read_to_string(&path).unwrap();
    let e = d.set_model_dimension(bore, 80.0).unwrap_err();
    assert!(e.contains("would not rebuild"), "{e}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
}

#[test]
fn hidden_model_dimensions_are_not_imported() {
    let dir = Scratch::new("hidden");
    let path = bracket(&dir);
    let mut doc = of_model::file::load(&path).unwrap();
    doc.dimension_display.insert(
        "f2.end1.depth".into(),
        DimensionDisplay {
            show: false,
            name: None,
        },
    );
    let key = of_model::equations::parameters(doc.feature(1).unwrap())
        .into_iter()
        .find(|p| (p.value.value - 12.0).abs() < 1e-9)
        .unwrap()
        .key;
    doc.dimension_display.insert(
        key.clone(),
        DimensionDisplay {
            show: true,
            name: Some("BORE".into()),
        },
    );
    of_model::file::save(&doc, &path).unwrap();
    let mut d = drawing(
        of_drawing::standard::Standard::iso(),
        vec![model_view(FRONT, &path, Orientation::Front, [150.0, 150.0])],
    );
    let new = d
        .model_items(SHEET, &[FRONT], &ModelItems::default())
        .unwrap();
    d.doc.sheets[0].annotations.extend(new);
    d.doc.next_id = 1000;
    let keys: Vec<String> = dims_of(&d).into_iter().map(|x| x.2).collect();
    assert!(!keys.contains(&"f2.end1.depth".to_string()));
    let named = d.doc.sheets[0]
        .annotations
        .iter()
        .find_map(|a| match &a.kind {
            AnnotationKind::Dimension(dim) if dim.driving() == Some(key.as_str()) => {
                Some(dim.below.clone())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(named, "BORE");
}

#[test]
fn smart_dimensions_measure_view_geometry_with_tolerances() {
    let dir = Scratch::new("smart");
    let path = bracket(&dir);
    let mut d = drawing(
        of_drawing::standard::Standard::iso(),
        vec![model_view(FRONT, &path, Orientation::Front, [150.0, 150.0])],
    );
    // The bottom edge (y = 0 on the front face) and the Ø12 hole's circle.
    let bottom = edge(&path, |e| {
        e.kind == CurveKind::Line
            && e.start.y.abs() < 1e-6
            && e.end.y.abs() < 1e-6
            && e.start.z.abs() < 1e-6
            && e.end.z.abs() < 1e-6
    });
    let left = edge(&path, |e| {
        e.kind == CurveKind::Line
            && e.start.x.abs() < 1e-6
            && e.end.x.abs() < 1e-6
            && e.start.z.abs() < 1e-6
            && e.end.z.abs() < 1e-6
    });
    let right = edge(&path, |e| {
        e.kind == CurveKind::Line
            && (e.start.x - 100.0).abs() < 1e-6
            && (e.end.x - 100.0).abs() < 1e-6
            && e.start.z.abs() < 1e-6
            && e.end.z.abs() < 1e-6
    });
    let bore = edge(&path, |e| {
        e.kind == CurveKind::Circle && (e.radius - 6.0).abs() < 1e-6 && e.center.z.abs() < 1e-6
    });
    let length = add(
        &mut d,
        Some(FRONT),
        AnnotationKind::Dimension(Box::new(Dimension::new(
            DimKind::Linear {
                a: pick(FRONT, bottom.clone()),
                b: None,
                orientation: Orient::Auto,
            },
            [50.0, -15.0],
        ))),
    );
    let between = add(
        &mut d,
        Some(FRONT),
        AnnotationKind::Dimension(Box::new(Dimension::new(
            DimKind::Linear {
                a: pick(FRONT, left.clone()),
                b: Some(pick(FRONT, right)),
                orientation: Orient::Auto,
            },
            [50.0, 75.0],
        ))),
    );
    let mut fit = Dimension::new(
        DimKind::Diameter {
            target: pick(FRONT, bore),
        },
        [100.0, 50.0],
    );
    fit.tolerance = Tolerance::Fit {
        hole: Some("H7".into()),
        shaft: Some("g6".into()),
        display: FitDisplay::Classes,
    };
    let fit = add(
        &mut d,
        Some(FRONT),
        AnnotationKind::Dimension(Box::new(fit)),
    );
    let mut angle = Dimension::new(
        DimKind::Angular {
            a: pick(FRONT, bottom),
            b: pick(FRONT, left),
        },
        [10.0, 10.0],
    );
    angle.tolerance = Tolerance::Symmetric { value: 0.5 };
    let angle = add(
        &mut d,
        Some(FRONT),
        AnnotationKind::Dimension(Box::new(angle)),
    );
    let r = render(&mut d);
    assert_eq!(text_of(&r, length), "100");
    assert_eq!(text_of(&r, between), "100");
    assert_eq!(text_of(&r, fit), plain("⌀12 H7/g6"));
    assert_eq!(text_of(&r, angle), "90°±0.5");
    // Moving the view moves its dimensions with it.
    let before: Vec<_> = r
        .graphics
        .items
        .iter()
        .filter(|i| i.owner == Owner::Annotation { id: length })
        .cloned()
        .collect();
    d.doc.sheets[0].views[0].origin = [160.0, 150.0];
    let r = render(&mut d);
    let after: Vec<_> = r
        .graphics
        .items
        .iter()
        .filter(|i| i.owner == Owner::Annotation { id: length })
        .collect();
    let x = |i: &of_drawing::graphics::Item| match &i.prim {
        Prim::Line { a, .. } => Some(a[0]),
        _ => None,
    };
    let (b0, a0) = (
        before.iter().find_map(x).unwrap(),
        after.iter().find_map(|i| x(i)).unwrap(),
    );
    assert_close(a0 - b0, 10.0, 1e-9);
}

#[test]
fn symbols_and_free_annotations_draw() {
    let dir = Scratch::new("symbols");
    let path = bracket(&dir);
    let mut d = drawing(
        of_drawing::standard::Standard::ansi(),
        vec![model_view(FRONT, &path, Orientation::Front, [150.0, 150.0])],
    );
    let bottom = edge(&path, |e| {
        e.kind == CurveKind::Line
            && e.start.y.abs() < 1e-6
            && e.end.y.abs() < 1e-6
            && e.start.z.abs() < 1e-6
    });
    let on_bottom = Anchor::Model {
        target: ViewRef {
            view: FRONT,
            entity: bottom,
        },
        at: [50.0, 0.0],
    };
    let datum = add(
        &mut d,
        Some(FRONT),
        AnnotationKind::DatumFeature {
            at: [50.0, -20.0],
            label: "A".into(),
            target: on_bottom.clone(),
        },
    );
    let gtol = add(
        &mut d,
        None,
        AnnotationKind::Gtol {
            at: [250.0, 220.0],
            leader: Some(on_bottom.clone()),
            rows: vec![GtolRow {
                characteristic: Characteristic::Position,
                diameter: true,
                tolerance: "0.1".into(),
                modifiers: Vec::new(),
                datums: vec![
                    DatumRef {
                        label: "A".into(),
                        modifier: None,
                    },
                    DatumRef {
                        label: "B".into(),
                        modifier: None,
                    },
                ],
            }],
            above: "2X".into(),
        },
    );
    let finish = add(
        &mut d,
        None,
        AnnotationKind::SurfaceFinish {
            at: [60.0, 100.0],
            leader: None,
            process: FinishProcess::Removal,
            roughness: "Ra 1.6".into(),
            method: String::new(),
            lay: String::new(),
            all_around: false,
            angle: 0.0,
        },
    );
    let cloud = add(
        &mut d,
        None,
        AnnotationKind::RevisionCloud {
            shape: CloudShape::Rectangle {
                a: [240.0, 200.0],
                b: [300.0, 240.0],
            },
            arc: 6.0,
            revision: Some("B".into()),
        },
    );
    let hatch = add(
        &mut d,
        None,
        AnnotationKind::AreaHatch {
            boundary: vec![[20.0, 20.0], [60.0, 20.0], [60.0, 40.0], [20.0, 40.0]],
            pattern: of_drawing::hatch::Pattern::default(),
            scale: 1.0,
            angle: 0.0,
        },
    );
    let balloon = add(
        &mut d,
        None,
        AnnotationKind::Balloon {
            at: [100.0, 60.0],
            leader: on_bottom,
            content: BalloonText::Text { text: "7".into() },
            lower: None,
            shape: BalloonShape::Hexagon,
            size: None,
            stacked: Vec::new(),
            table: None,
        },
    );
    let r = render(&mut d);
    assert_eq!(text_of(&r, datum), "A");
    let g = text_of(&r, gtol);
    assert!(
        g.contains("2X") && g.contains("0.1") && g.contains('A') && g.contains('B'),
        "{g}"
    );
    assert_eq!(text_of(&r, finish), "Ra 1.6");
    assert_eq!(text_of(&r, cloud), "B");
    assert_eq!(text_of(&r, balloon), "7");
    assert!(
        r.graphics
            .items
            .iter()
            .filter(|i| i.owner == Owner::Annotation { id: hatch })
            .count()
            > 3
    );
}

/// The bracket with a Ø12 × 40 pin in its bore, twice.
fn assembly(dir: &Scratch) -> std::path::PathBuf {
    let plate = bracket(dir);
    let mut pin_doc = shaft_doc(6.0, 40.0);
    pin_doc.properties.insert(
        "PartNo".into(),
        of_model::properties::Property::text("PIN-12"),
    );
    let pin = save(&pin_doc, &dir.path("pin.ofp"));
    let mut a = Assembly::default();
    a.insert(&plate, None, None).unwrap();
    // One pin standing on the bore, one hanging under it.
    for z in [10.0, -40.0] {
        a.insert(
            &pin,
            Some(Pose::from_translation(DVec3::new(75.0, 30.0, z))),
            None,
        )
        .unwrap();
    }
    let file = dir.path("pinned.ofa");
    a.save(&file).unwrap();
    file
}

#[test]
fn bom_balloons_follow_item_numbers() {
    let dir = Scratch::new("bom");
    let file = assembly(&dir);
    let mut d = drawing(
        of_drawing::standard::Standard::iso(),
        vec![model_view(
            FRONT,
            &file,
            Orientation::Isometric,
            [180.0, 160.0],
        )],
    );
    let bom = add(
        &mut d,
        None,
        AnnotationKind::Table(Box::new(Table {
            at: [20.0, 280.0],
            anchor: Default::default(),
            kind: TableKind::Bom {
                view: FRONT,
                bom: Default::default(),
                numbering: Default::default(),
                columns: vec![
                    of_drawing::tables::Column::Item,
                    of_drawing::tables::Column::PartNumber,
                    of_drawing::tables::Column::Quantity,
                ],
                start: 1,
                increment: 1,
            },
            text_height: None,
            max_rows: None,
            continued: Vec::new(),
        })),
    );
    let balloons = d
        .auto_balloons(SHEET, FRONT, BalloonLayout::Square, true)
        .unwrap();
    // Two distinct parts: one balloon each.
    assert_eq!(balloons.len(), 2, "{balloons:?}");
    d.doc.next_id = balloons.iter().map(|a| a.id + 1).max().unwrap();
    let ids: Vec<u32> = balloons.iter().map(|a| a.id).collect();
    d.doc.sheets[0].annotations.extend(balloons);
    let r = render(&mut d);
    let table = text_of(&r, bom);
    assert!(
        table.contains("BRK-1") && table.contains("PIN-12"),
        "{table}"
    );
    let mut items: Vec<String> = ids.iter().map(|id| text_of(&r, *id)).collect();
    items.sort();
    assert_eq!(items, vec!["1", "2"]);
    // The pin row counts both pins.
    let sheet = d.doc.sheets[0].clone();
    let AnnotationKind::Table(t) = &sheet.annotations[0].kind else {
        unreachable!()
    };
    let csv = d.table_cells(&sheet, t).unwrap().csv();
    assert!(
        csv.lines()
            .any(|l| l.contains("PIN-12") && l.ends_with(",2")),
        "{csv}"
    );
}

#[test]
fn hole_revision_and_tolerance_tables() {
    let dir = Scratch::new("tables");
    let path = bracket(&dir);
    let mut d = drawing(
        of_drawing::standard::Standard::iso(),
        vec![model_view(FRONT, &path, Orientation::Front, [150.0, 150.0])],
    );
    // The holes' exits on the back face (z = 0), seen end-on from the front.
    let m = model(&path);
    let state = m.state();
    let body = state.body.as_ref().unwrap();
    let mut holes = Vec::new();
    for i in 0..body.shape.count(Kind::Edge) {
        let e = body.shape.edge_info(i).unwrap();
        if e.kind == CurveKind::Circle
            && (e.radius - 3.3).abs() < 1e-6
            && (e.center.z - 0.0).abs() < 1e-6
        {
            holes.push(ViewRef {
                view: FRONT,
                entity: CompRef {
                    component: Vec::new(),
                    entity: state.ref_for(Kind::Edge, i).unwrap(),
                },
            });
        }
    }
    assert_eq!(holes.len(), 2);
    let corner = edge(&path, |e| {
        e.kind == CurveKind::Line
            && e.start.x.abs() < 1e-6
            && e.end.x.abs() < 1e-6
            && e.start.y.abs() < 1e-6
            && e.end.y.abs() < 1e-6
    });
    let table = |kind, at: [f64; 2], anchor| {
        AnnotationKind::Table(Box::new(Table {
            at,
            anchor,
            kind,
            text_height: None,
            max_rows: None,
            continued: Vec::new(),
        }))
    };
    let hole = add(
        &mut d,
        None,
        table(
            TableKind::Hole {
                origin: Pick {
                    target: ViewRef {
                        view: FRONT,
                        entity: corner,
                    },
                    point: Which::Start,
                },
                holes,
                tags: Tags::Alphabetic,
                combine: false,
                show_tags: true,
            },
            [20.0, 280.0],
            of_drawing::tables::Corner::TopLeft,
        ),
    );
    let rev = add(
        &mut d,
        None,
        table(
            TableKind::Revision {
                rows: vec![
                    RevisionRow {
                        revision: String::new(),
                        description: "Released".into(),
                        date: "2026-10-01".into(),
                        approved: "AK".into(),
                        zone: String::new(),
                    },
                    RevisionRow {
                        revision: String::new(),
                        description: "Bore H7".into(),
                        date: "2026-10-14".into(),
                        approved: "AK".into(),
                        zone: "B3".into(),
                    },
                ],
                numeric: false,
            },
            [410.0, 287.0],
            of_drawing::tables::Corner::TopRight,
        ),
    );
    let tol = add(
        &mut d,
        None,
        table(
            TableKind::GeneralTolerance {
                class: ToleranceClass::M,
            },
            [20.0, 200.0],
            of_drawing::tables::Corner::TopLeft,
        ),
    );
    let r = render(&mut d);
    let h = text_of(&r, hole);
    // Measured from the plate's corner: (20, 30) and (40, 30).
    let size = plain("⌀6.6 THRU ⌴⌀11 ↧6.4");
    assert_eq!(
        h,
        // The tags are also drawn beside the holes.
        format!("TAG X LOC Y LOC SIZE A1 20 30 {size} A2 40 30 {size} A1 A2"),
    );
    assert!(text_of(&r, rev).contains("Bore H7"));
    let t = text_of(&r, tol);
    assert!(t.contains("ISO 2768-m"), "{t}");
    assert!(t.contains("±0.1"), "{t}");
    // The latest revision fills the title block's revision cell.
    let format_text: Vec<&str> = r
        .graphics
        .items
        .iter()
        .filter(|i| i.owner == Owner::Format)
        .filter_map(|i| match &i.prim {
            Prim::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(format_text.contains(&"B"), "{format_text:?}");
}

/// A 40 × 40 × 10 block with one tapped ISO M6 hole through it at (20, 20).
fn tapped(dir: &Scratch) -> std::path::PathBuf {
    let mut m = Model::new(block_doc(40.0, 40.0, 10.0));
    m.rebuild();
    let top = face_facing(&m, DVec3::Z);
    let mut sk = Sketch::new();
    sk.add_point(DVec2::new(20.0, 20.0));
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: top,
        sketch: sk,
    }));
    m.doc.add(FeatureKind::Hole(Hole {
        sketch: s,
        kind: HoleKind::Tapped,
        standard: HoleStandard::Iso,
        size: "M6".into(),
        fit: HoleFit::Normal,
        end: HoleEnd::ThroughAll,
        reverse: false,
        diameter_override: None,
        cbore_diameter_override: None,
        cbore_depth_override: None,
        csink_diameter_override: None,
        csink_angle_override: None,
        cosmetic_thread: true,
    }));
    m.rebuild();
    assert!(m.statuses().iter().all(|s| s.error.is_none()));
    save(&m.doc, &dir.path("tapped.ofp"))
}

#[test]
fn cosmetic_threads_follow_the_standard() {
    let dir = Scratch::new("threads");
    let path = tapped(&dir);
    let threads = |std: of_drawing::standard::Standard, o: Orientation| {
        let mut d = drawing(std, vec![model_view(FRONT, &path, o, [150.0, 150.0])]);
        let r = render(&mut d);
        r.graphics
            .items
            .into_iter()
            .filter(|i| i.layer == "THREADS")
            .collect::<Vec<_>>()
    };
    // End view, ISO: a thin three-quarter circle at the major diameter
    // around the tap drill hole.
    let iso = threads(of_drawing::standard::Standard::iso(), Orientation::Front);
    assert_eq!(iso.len(), 1, "{iso:?}");
    let Prim::Arc {
        center,
        radius,
        start,
        end,
    } = iso[0].prim
    else {
        panic!("{:?}", iso[0].prim)
    };
    assert_close(radius, 3.0, 1e-9);
    assert_close(center[0], 170.0, 1e-6);
    assert_close(center[1], 170.0, 1e-6);
    assert_close((end - start).to_degrees(), 250.0, 1e-9);
    assert_eq!(
        iso[0].pen.style,
        of_drawing::graphics::LineStyle::Continuous
    );
    // ANSI: a full hidden circle.
    let ansi = threads(of_drawing::standard::Standard::ansi(), Orientation::Front);
    assert_eq!(ansi.len(), 1);
    assert_eq!(ansi[0].pen.style, of_drawing::graphics::LineStyle::Hidden);
    // Side view: hidden lines at the major diameter over the 10 mm depth,
    // and the thread end.
    let side = threads(of_drawing::standard::Standard::iso(), Orientation::Top);
    assert_eq!(side.len(), 3, "{side:?}");
    for i in &side {
        assert_eq!(i.pen.style, of_drawing::graphics::LineStyle::Hidden);
        let Prim::Line { a, b } = i.prim else {
            panic!()
        };
        let len = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        assert!(
            (len - 10.0).abs() < 1e-6 || (len - 6.0).abs() < 1e-6,
            "{len}"
        );
    }
}
