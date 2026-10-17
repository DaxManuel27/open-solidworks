//! Sheets rendered from real part and assembly files: standard views,
//! alignment, edge references, title block fields, caching, assembly and
//! exploded views, and PDF/DXF output of the result.

mod common;

use common::*;
use of_assembly::{Assembly, DVec3, ExplodeMotion, ExplodeStep, Pose};
use of_drawing::annotations::{Annotation, AnnotationKind};
use of_drawing::document::*;
use of_drawing::drawing::Drawing;
use of_drawing::graphics::{Owner, Prim, layers};
use of_drawing::{dxf, pdf};
use of_model::properties::Property;

const SHEET: u32 = 1;
const FRONT: u32 = 2;
const RIGHT: u32 = 3;
const TOP: u32 = 4;

/// A 100×60×10 plate (x, y, z) with a Ø10 hole at (30, 30).
fn plate(dir: &Scratch) -> std::path::PathBuf {
    let mut doc = plate_doc(100.0, 60.0, 10.0, &[(30.0, 30.0, 5.0)]);
    doc.properties
        .insert("PartNo".into(), Property::text("PLATE-1"));
    doc.properties
        .insert("Description".into(), Property::text("Base plate"));
    doc.material = Some(of_model::materials::Material {
        name: "Steel".into(),
        density: 7850.0,
    });
    save(&doc, &dir.path("plate.ofp"))
}

fn model_view(id: u32, path: &std::path::Path, orientation: Orientation, at: [f64; 2]) -> View {
    View::new(
        id,
        format!("View{id}"),
        ViewKind::Model {
            model: ModelSource {
                path: path.to_string_lossy().into_owned(),
                hash: None,
                configuration: None,
                flat: None,
                bodies: None,
            },
            orientation,
            exploded: false,
        },
        at,
    )
}

fn projected(id: u32, parent: u32, placement: Placement, at: [f64; 2]) -> View {
    View::new(
        id,
        format!("View{id}"),
        ViewKind::Projected { parent, placement },
        at,
    )
}

fn sheet(projection: Projection, views: Vec<View>) -> Sheet {
    Sheet {
        id: SHEET,
        name: "Sheet1".into(),
        size: SheetSize::A3,
        portrait: false,
        scale: Scale::ONE,
        projection,
        format: SheetFormat::default(),
        properties_view: None,
        views,
        annotations: Vec::new(),
    }
}

fn drawing(sheets: Vec<Sheet>) -> Drawing {
    let mut doc = DrawingDocument {
        sheets,
        ..Default::default()
    };
    doc.next_id = 100;
    doc.validate().unwrap();
    Drawing::new(doc)
}

/// Front at (150, 180), a view placed right at x = 300 and one above at
/// y = 260, both deliberately off-line so alignment has to fix them.
fn three_view(path: &std::path::Path, projection: Projection) -> Sheet {
    let mut top = projected(TOP, FRONT, Placement::Above, [140.0, 260.0]);
    top.display = Some(DisplayStyle::HiddenVisible);
    sheet(
        projection,
        vec![
            model_view(FRONT, path, Orientation::Front, [150.0, 180.0]),
            projected(RIGHT, FRONT, Placement::Right, [300.0, 170.0]),
            top,
        ],
    )
}

fn toward(r: &of_drawing::drawing::SheetRender, view: u32) -> DVec3 {
    DVec3::from(r.views.iter().find(|v| v.id == view).unwrap().toward)
}

#[test]
fn third_angle_three_view_of_a_plate() {
    let dir = Scratch::new("third");
    let path = plate(&dir);
    let mut d = drawing(vec![three_view(&path, Projection::Third)]);
    let r = d.render(SHEET).unwrap();
    for v in &r.views {
        assert_eq!(v.error, None, "view {}", v.id);
    }
    let items = &r.graphics.items;

    // Front: the plate's outline 100 × 60 from the origin.
    let front = extent(&of_view(items, FRONT));
    assert_close(front[0][0], 150.0, 1e-6);
    assert_close(front[0][1], 180.0, 1e-6);
    assert_close(front[1][0], 250.0, 1e-6);
    assert_close(front[1][1], 240.0, 1e-6);
    // The hole is drawn as arcs of radius 5 around (180, 210) that close
    // the circle, each tied to a model edge.
    let hole: Vec<_> = of_view(items, FRONT)
        .into_iter()
        .filter_map(|i| match (&i.prim, &i.owner) {
            (
                Prim::Arc {
                    center,
                    radius,
                    start,
                    end,
                },
                Owner::View { entity, .. },
            ) => Some((*center, *radius, end - start, entity.clone())),
            _ => None,
        })
        .collect();
    assert!(!hole.is_empty());
    let sweep: f64 = hole.iter().map(|h| h.2).sum();
    assert_close(sweep, std::f64::consts::TAU, 1e-6);
    for (c, rad, _, entity) in &hole {
        assert_close(c[0], 180.0, 1e-6);
        assert_close(c[1], 210.0, 1e-6);
        assert_close(*rad, 5.0, 1e-6);
        let e = entity.as_ref().expect("the hole edge has a reference");
        assert!(e.component.is_empty(), "a part view has no component");
    }
    // Every visible model line names its edge.
    for i in of_view(items, FRONT) {
        if let Owner::View { entity, .. } = &i.owner {
            assert!(entity.is_some(), "{:?}", i.prim);
        }
        assert_eq!(i.layer, layers::VISIBLE);
    }

    // Third angle: the right side on the right, the top above.
    assert!(toward(&r, RIGHT).abs_diff_eq(DVec3::X, 1e-9));
    assert!(toward(&r, TOP).abs_diff_eq(DVec3::Y, 1e-9));
    // Aligned with the front: same height range on the right (the view's
    // y = 170 is ignored), same width range above (x = 140 is ignored).
    // The right view's paper x is -z: the plate's 10 mm thickness lies
    // left of the view origin.
    let right = extent(&of_view(items, RIGHT));
    assert_close(right[0][1], 180.0, 1e-6);
    assert_close(right[1][1], 240.0, 1e-6);
    assert_close(right[0][0], 290.0, 1e-6);
    assert_close(right[1][0], 300.0, 1e-6);
    let top = extent(&of_view(items, TOP));
    assert_close(top[0][0], 150.0, 1e-6);
    assert_close(top[1][0], 250.0, 1e-6);
    assert_close(top[0][1], 250.0, 1e-6);
    assert_close(top[1][1], 260.0, 1e-6);

    // Hidden lines are shown only where asked: the hole's two walls in the
    // top view, at x = 175 and 185.
    let hidden: Vec<_> = items.iter().filter(|i| i.layer == layers::HIDDEN).collect();
    assert!(hidden.iter().all(|i| matches!(
        &i.owner,
        Owner::View { view, .. } if *view == TOP
    )));
    let mut xs: Vec<f64> = hidden
        .iter()
        .filter_map(|i| match i.prim {
            Prim::Line { a, b } => {
                assert_close(a[0], b[0], 1e-6);
                Some(a[0])
            }
            _ => None,
        })
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    assert_eq!(xs.len(), 2, "{xs:?}");
    assert_close(xs[0], 175.0, 1e-6);
    assert_close(xs[1], 185.0, 1e-6);

    // View boxes reported to the desktop match what was drawn.
    let info = r.views.iter().find(|v| v.id == FRONT).unwrap();
    let b = info.bbox.unwrap();
    assert!(b[0][0] <= 150.0 + 1e-6 && b[1][0] >= 250.0 - 1e-6);
}

#[test]
fn first_angle_places_the_opposite_sides() {
    let dir = Scratch::new("first");
    let path = plate(&dir);
    let mut d = drawing(vec![three_view(&path, Projection::First)]);
    let r = d.render(SHEET).unwrap();
    for v in &r.views {
        assert_eq!(v.error, None, "view {}", v.id);
    }
    // First angle: the view on the right shows the left side, the view
    // above shows the bottom.
    assert!(toward(&r, RIGHT).abs_diff_eq(DVec3::NEG_X, 1e-9));
    assert!(toward(&r, TOP).abs_diff_eq(DVec3::NEG_Y, 1e-9));
    let items = &r.graphics.items;
    let right = extent(&of_view(items, RIGHT));
    assert_close(right[0][1], 180.0, 1e-6);
    assert_close(right[1][1], 240.0, 1e-6);
    // Seen from -X the plate's thickness lies right of the origin.
    assert_close(right[0][0], 300.0, 1e-6);
    assert_close(right[1][0], 310.0, 1e-6);
    let top = extent(&of_view(items, TOP));
    assert_close(top[0][0], 150.0, 1e-6);
    assert_close(top[1][0], 250.0, 1e-6);
    // The projection symbol in the title block follows the sheet.
    let format_arcs = items
        .iter()
        .filter(|i| i.owner == Owner::Format && matches!(i.prim, Prim::Arc { .. }))
        .count();
    assert_eq!(format_arcs, 2, "the symbol's two end-view circles");
}

#[test]
fn title_block_fields_come_from_the_model_and_drawing() {
    let dir = Scratch::new("fields");
    let path = plate(&dir);
    let mut d = drawing(vec![three_view(&path, Projection::First)]);
    d.doc
        .properties
        .insert("DrawnBy".into(), "A. Drafter".into());
    d.doc.properties.insert("Revision".into(), "B".into());
    std::fs::create_dir_all(dir.path("drawings")).unwrap();
    d.save(&dir.path("drawings/plate-drawing.ofd")).unwrap();
    // Model paths become relative to the drawing.
    let ViewKind::Model { model, .. } = &d.doc.sheets[0].views[0].kind else {
        panic!()
    };
    assert_eq!(model.path, "../plate.ofp");

    let mut d = Drawing::open(&dir.path("drawings/plate-drawing.ofd")).unwrap();
    let r = d.render(SHEET).unwrap();
    let format: Vec<_> = r
        .graphics
        .items
        .iter()
        .filter(|i| i.owner == Owner::Format)
        .cloned()
        .collect();
    let t = texts(&format);
    for want in ["PLATE-1", "Base plate", "A. Drafter", "B", "Steel", "1:1"] {
        assert!(t.iter().any(|s| s.contains(want)), "{want} in {t:?}");
    }
    // 100 × 60 × 10 less a Ø10 hole, at 7850 kg/m³.
    let mass = (60_000.0 - std::f64::consts::PI * 25.0 * 10.0) * 7850.0 * 1e-9;
    let mass = format!("{mass:.3} kg");
    assert!(t.iter().any(|s| s.contains(&mass)), "{mass} in {t:?}");
    assert!(t.iter().any(|s| s.contains("1 / 1")), "{t:?}");
}

#[test]
fn projections_are_cached_until_the_model_changes() {
    let dir = Scratch::new("cache");
    let path = plate(&dir);
    let mut d = drawing(vec![three_view(&path, Projection::Third)]);
    let first = d.render(SHEET).unwrap();
    assert_eq!(d.hlr_runs, 3);
    let again = d.render(SHEET).unwrap();
    assert_eq!(d.hlr_runs, 3, "nothing changed");
    assert_eq!(first, again);

    // Moving a view reuses its projection.
    d.doc.sheets[0].views[0].origin = [160.0, 170.0];
    d.render(SHEET).unwrap();
    assert_eq!(d.hlr_runs, 3);

    // Editing the part redraws every view: the hole moves to x = 70.
    let mut doc = of_model::file::load(&path).unwrap();
    let edited = plate_doc(100.0, 60.0, 10.0, &[(70.0, 30.0, 5.0)]);
    doc.features = edited.features;
    save(&doc, &path);
    let r = d.render(SHEET).unwrap();
    assert_eq!(d.hlr_runs, 6);
    let centers: Vec<_> = of_view(&r.graphics.items, FRONT)
        .into_iter()
        .filter_map(|i| match i.prim {
            Prim::Arc { center, .. } => Some(center),
            _ => None,
        })
        .collect();
    assert!(!centers.is_empty());
    for c in centers {
        assert_close(c[0], 160.0 + 70.0, 1e-6);
    }
}

#[test]
fn hidden_edges_are_left_out() {
    let dir = Scratch::new("hide");
    let path = plate(&dir);
    let mut d = drawing(vec![sheet(
        Projection::First,
        vec![model_view(FRONT, &path, Orientation::Front, [150.0, 180.0])],
    )]);
    let r = d.render(SHEET).unwrap();
    let hole: Vec<_> = r
        .graphics
        .items
        .iter()
        .filter_map(|i| match (&i.prim, &i.owner) {
            (Prim::Arc { .. }, Owner::View { entity, .. }) => entity.clone(),
            _ => None,
        })
        .collect();
    assert!(!hole.is_empty());
    let before = of_view(&r.graphics.items, FRONT).len();
    d.doc.sheets[0].views[0].hidden_edges = hole.clone();
    let r = d.render(SHEET).unwrap();
    let after = of_view(&r.graphics.items, FRONT);
    assert_eq!(after.len(), before - hole.len());
    assert!(!after.iter().any(|i| matches!(i.prim, Prim::Arc { .. })));
}

#[test]
fn notes_lead_to_model_points_and_fill_fields() {
    let dir = Scratch::new("note");
    let path = plate(&dir);
    let mut s = sheet(
        Projection::First,
        vec![model_view(FRONT, &path, Orientation::Front, [150.0, 180.0])],
    );
    s.annotations.push(Annotation {
        id: 10,
        attach: None,
        layer: None,
        kind: AnnotationKind::Note {
            at: [100.0, 250.0],
            text: "⌀10 THRU\n$PRP:\"PartNo\"".into(),
            height: None,
            leader: Some(Anchor::Model {
                target: ViewRef {
                    view: FRONT,
                    entity: of_drawing::model::part_ref(of_model::Ref::FrontPlane),
                },
                at: [30.0, 35.0],
            }),
            jog: false,
            border: Default::default(),
            angle: 0.0,
        },
    });
    let mut d = drawing(vec![s]);
    let r = d.render(SHEET).unwrap();
    let note: Vec<_> = r
        .graphics
        .items
        .iter()
        .filter(|i| i.owner == Owner::Annotation { id: 10 })
        .cloned()
        .collect();
    assert!(texts(&note).iter().any(|t| t.contains("PLATE-1")));
    // The arrow tip sits on the hole's top edge at (180, 215).
    let tip = note
        .iter()
        .find_map(|i| match &i.prim {
            Prim::Fill { loops } => Some(loops[0][0]),
            _ => None,
        })
        .expect("an arrowhead");
    assert_close(tip[0], 180.0, 1e-9);
    assert_close(tip[1], 215.0, 1e-9);
}

/// The plate with a Ø10 × 40 pin standing in its hole.
fn pinned(dir: &Scratch) -> (std::path::PathBuf, String, String) {
    let plate = plate(dir);
    let pin = save(&shaft_doc(5.0, 40.0), &dir.path("pin.ofp"));
    let mut a = Assembly::default();
    let p = a.insert(&plate, None, None).unwrap();
    let s = a
        .insert(
            &pin,
            Some(Pose::from_translation(DVec3::new(30.0, 30.0, 10.0))),
            None,
        )
        .unwrap();
    let id = a.doc.take_id();
    let c = a.doc.active_configuration;
    let config = a.doc.configurations.iter_mut().find(|x| x.id == c).unwrap();
    config.explode.push(ExplodeStep {
        id,
        name: "Explode Step1".into(),
        components: vec![vec![s.clone()]],
        motion: ExplodeMotion::Translate {
            direction: [0.0, 0.0, 1.0],
            distance: 50.0,
            auto_space: false,
        },
    });
    let file = dir.path("pinned.ofa");
    a.save(&file).unwrap();
    (file, p, s)
}

#[test]
fn assembly_views_name_components_and_explode() {
    let dir = Scratch::new("asm");
    let (file, p, s) = pinned(&dir);
    let mut top = model_view(TOP, &file, Orientation::Top, [150.0, 150.0]);
    let mut exploded = model_view(5, &file, Orientation::Top, [300.0, 150.0]);
    if let ViewKind::Model { exploded: e, .. } = &mut exploded.kind {
        *e = true;
    }
    top.name = "Top".into();
    let mut d = drawing(vec![sheet(Projection::Third, vec![top, exploded])]);
    let r = d.render(SHEET).unwrap();
    for v in &r.views {
        assert_eq!(v.error, None, "view {}", v.id);
    }
    let items = &r.graphics.items;
    let component = |view: u32, key: &str| -> Vec<&of_drawing::graphics::Item> {
        of_view(items, view)
            .into_iter()
            .filter(
                |i| matches!(&i.owner, Owner::View { entity: Some(e), .. } if e.component == [key]),
            )
            .collect()
    };
    // Seen from above (paper y = -z): the plate spans z 0..10, the pin
    // stands on it from z = 10 to 50.
    let plate_box = extent(&component(TOP, &p));
    let pin_box = extent(&component(TOP, &s));
    assert_close(plate_box[0][0], 150.0, 1e-6);
    assert_close(plate_box[1][0], 250.0, 1e-6);
    assert_close(plate_box[1][1], 150.0, 1e-6);
    assert_close(pin_box[0][0], 175.0, 1e-6);
    assert_close(pin_box[1][0], 185.0, 1e-6);
    assert_close(pin_box[0][1], 100.0, 1e-6);
    assert_close(pin_box[1][1], 140.0, 1e-6);
    // Every model edge belongs to one of the two components. The pin's
    // silhouettes (x = 175 and 185) are outlines of its cylinder, not model
    // edges, so they may carry no reference.
    let mut outlines = Vec::new();
    for i in of_view(items, TOP) {
        match &i.owner {
            Owner::View {
                entity: Some(e), ..
            } => {
                assert!(e.component == [p.clone()] || e.component == [s.clone()])
            }
            _ => outlines.push(i),
        }
    }
    let xs: Vec<f64> = outlines
        .iter()
        .map(|i| match i.prim {
            Prim::Line { a, b } => {
                assert_close(a[0], b[0], 1e-6);
                a[0]
            }
            ref other => panic!("{other:?} has no edge"),
        })
        .collect();
    // (A silhouette lying on the cylinder's seam is traced to the seam.)
    assert!(!xs.is_empty());
    for x in xs {
        assert!((x - 175.0).abs() < 1e-6 || (x - 185.0).abs() < 1e-6, "{x}");
    }

    // Exploded: the pin is lifted 50 mm and a phantom line shows its travel.
    let lifted = extent(&component(5, &s));
    assert_close(lifted[0][1], 50.0, 1e-6);
    assert_close(lifted[1][1], 90.0, 1e-6);
    let travel: Vec<_> = of_view(items, 5)
        .into_iter()
        .filter(|i| i.layer == "EXPLODE")
        .collect();
    assert_eq!(travel.len(), 1);
    let Prim::Line { a, b } = travel[0].prim else {
        panic!()
    };
    assert_close((a[1] - b[1]).abs(), 50.0, 1e-6);

    // Hiding the pin leaves only the plate.
    d.doc.sheets[0].views[0].hidden_components = vec![vec![s.clone()]];
    let r = d.render(SHEET).unwrap();
    assert!(component_free(&r.graphics.items, TOP, &s));
}

fn component_free(items: &[of_drawing::graphics::Item], view: u32, key: &str) -> bool {
    !of_view(items, view)
        .into_iter()
        .any(|i| matches!(&i.owner, Owner::View { entity: Some(e), .. } if e.component == [key]))
}

#[test]
fn a_rendered_sheet_exports_to_pdf_and_dxf() {
    let dir = Scratch::new("export");
    let path = plate(&dir);
    let mut d = drawing(vec![three_view(&path, Projection::Third)]);
    let r = d.render(SHEET).unwrap();
    let std = d.doc.standard;
    let bytes = pdf::write(
        std::slice::from_ref(&r.graphics),
        &std,
        &pdf::PdfOptions::default(),
    )
    .unwrap();
    if let Ok(out) = std::env::var("OPENFORGE_OUTPUT_DIR") {
        std::fs::write(std::path::Path::new(&out).join("plate.pdf"), &bytes).unwrap();
    }
    let s = String::from_utf8_lossy(&bytes);
    assert!(s.starts_with("%PDF-"));
    for l in ["FORMAT", "VISIBLE", "HIDDEN"] {
        assert!(s.contains(&format!("({l})")), "layer {l}");
    }
    let text = dxf::write(std::slice::from_ref(&r.graphics), &std).unwrap();
    if let Ok(out) = std::env::var("OPENFORGE_OUTPUT_DIR") {
        std::fs::write(std::path::Path::new(&out).join("plate.dxf"), &text).unwrap();
    }
    assert!(text.contains("PLATE-1"));
    // The hole as a full circle on the visible layer.
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let circles = lines
        .windows(2)
        .filter(|w| w[0] == "0" && w[1] == "CIRCLE")
        .count();
    assert!(circles >= 1, "{circles}");
}

#[test]
fn auxiliary_views_fold_about_a_parent_edge() {
    let dir = Scratch::new("aux");
    let path = plate(&dir);
    let mut d = drawing(vec![sheet(
        Projection::First,
        vec![model_view(FRONT, &path, Orientation::Front, [150.0, 180.0])],
    )]);
    let r = d.render(SHEET).unwrap();
    // The plate's bottom edge, a horizontal line at y = 180.
    let edge = of_view(&r.graphics.items, FRONT)
        .into_iter()
        .find_map(|i| match (&i.prim, &i.owner) {
            (Prim::Line { a, b }, Owner::View { entity, .. })
                if (a[1] - 180.0).abs() < 1e-6 && (b[1] - 180.0).abs() < 1e-6 =>
            {
                entity.clone()
            }
            _ => None,
        })
        .expect("the bottom edge");
    // Placed below the front view and folded about that edge, it looks the
    // way a projected view below does: in first angle, down from the top.
    d.doc.sheets[0].views.push(View::new(
        RIGHT,
        "Auxiliary".into(),
        ViewKind::Auxiliary {
            parent: FRONT,
            edge,
            flip: false,
        },
        [170.0, 120.0],
    ));
    let r = d.render(SHEET).unwrap();
    let aux = r.views.iter().find(|v| v.id == RIGHT).unwrap();
    assert_eq!(aux.error, None);
    assert!(toward(&r, RIGHT).abs_diff_eq(DVec3::Y, 1e-9));
    // Aligned: it keeps the parent's x range.
    let b = extent(&of_view(&r.graphics.items, RIGHT));
    assert_close(b[0][0], 150.0, 1e-6);
    assert_close(b[1][0], 250.0, 1e-6);
}
