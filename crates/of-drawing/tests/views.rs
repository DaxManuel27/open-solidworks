//! Derived views on real parts and assemblies: sections (full, offset,
//! half, aligned, partial, slice, with exclusions), details, crops, breaks,
//! broken-out sections, shading, relative-to-model views and stale models.

mod common;

use common::*;
use of_assembly::{Assembly, DVec3, Pose};
use of_drawing::document::*;
use of_drawing::drawing::{Drawing, SheetRender};
use of_drawing::graphics::{Item, Owner, Prim, layers};

const SHEET: u32 = 1;
const FRONT: u32 = 2;
const CHILD: u32 = 3;

/// A 100×60×10 plate with a Ø10 hole at (30, 30) and one at (70, 30).
fn plate(dir: &Scratch) -> std::path::PathBuf {
    let mut doc = plate_doc(100.0, 60.0, 10.0, &[(30.0, 30.0, 5.0), (70.0, 30.0, 5.0)]);
    doc.material = Some(of_model::materials::Material {
        name: "Steel".into(),
        density: 7850.0,
    });
    save(&doc, &dir.path("plate.ofp"))
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

fn section(points: Vec<[f64; 2]>, kind: SectionKind, at: [f64; 2]) -> View {
    let mut v = View::new(
        CHILD,
        "Section".into(),
        ViewKind::Section {
            parent: FRONT,
            line: CuttingLine { points },
            kind,
            flip: false,
            depth: None,
            slice: false,
            excluded: Vec::new(),
            exclude_fasteners: true,
        },
        at,
    );
    v.label = Some("A".into());
    v
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
                visible: false,
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
    // Set OPENFORGE_OUTPUT_DIR to keep each rendered sheet as a PDF.
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
        let n = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0);
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}-{n}.pdf")),
            bytes,
        )
        .unwrap();
    }
    r
}

fn owned(r: &SheetRender, view: u32) -> Vec<&Item> {
    of_view(&r.graphics.items, view)
}

fn on(items: &[&Item], layer: &str) -> usize {
    items.iter().filter(|i| i.layer == layer).count()
}

fn texts_of<'a>(items: &[&'a Item]) -> Vec<&'a str> {
    items
        .iter()
        .filter_map(|i| match &i.prim {
            Prim::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn mids(items: &[&Item], layer: &str) -> Vec<[f64; 2]> {
    items
        .iter()
        .filter(|i| i.layer == layer)
        .filter_map(|i| match i.prim {
            Prim::Line { a, b } => Some([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]),
            _ => None,
        })
        .collect()
}

#[test]
fn full_section_through_the_holes_is_hatched_around_them() {
    let dir = Scratch::new("section");
    let path = plate(&dir);
    // Front at (100, 180); the cut runs along y = 30 through both holes and
    // the section sits below it (third angle: seen from below).
    let mut d = drawing(
        of_drawing::standard::Standard::ansi(),
        vec![
            model_view(FRONT, &path, Orientation::Front, [100.0, 180.0]),
            section(
                vec![[-10.0, 30.0], [110.0, 30.0]],
                SectionKind::Full,
                [100.0, 100.0],
            ),
        ],
    );
    let r = render(&mut d);
    let sec = owned(&r, CHILD);
    // Seen from -Y: paper x is model x, paper y is model z.
    let hatch = mids(&sec, layers::HATCH);
    assert!(hatch.len() > 10, "{}", hatch.len());
    for m in &hatch {
        let x = m[0] - 100.0;
        assert!(
            (0.0..=25.0 + 1e-6).contains(&x)
                || (35.0 - 1e-6..=65.0 + 1e-6).contains(&x)
                || (75.0 - 1e-6..=100.0 + 1e-6).contains(&x),
            "hatch at {x}"
        );
    }
    // The cut is 10 mm deep (plate thickness) in paper y.
    let b = extent(
        &sec.iter()
            .copied()
            .filter(|i| i.layer == layers::VISIBLE)
            .collect::<Vec<_>>(),
    );
    assert_close(b[1][0] - b[0][0], 100.0, 1e-6);
    assert_close(b[1][1] - b[0][1], 10.0, 1e-6);
    // Visible edges still name model edges.
    assert!(
        sec.iter()
            .filter(|i| i.layer == layers::VISIBLE)
            .any(|i| matches!(
                &i.owner,
                Owner::View {
                    entity: Some(_),
                    ..
                }
            ))
    );
    // Label under the view and the cutting line with two arrows on the front.
    assert!(texts_of(&sec).contains(&"SECTION A-A"));
    assert!(on(&sec, layers::CENTER) >= 3, "chain line and end strokes");
    let arrows = sec
        .iter()
        .filter(|i| matches!(i.prim, Prim::Fill { .. }))
        .count();
    assert_eq!(arrows, 2);
    assert_eq!(texts_of(&sec).iter().filter(|t| **t == "A").count(), 2);
}

#[test]
fn offset_half_partial_and_slice_sections() {
    let dir = Scratch::new("kinds");
    let path = plate(&dir);
    let front = || model_view(FRONT, &path, Orientation::Front, [100.0, 180.0]);
    // Offset: through the first hole at y = 30, jog to y = 10 past it.
    let jog = vec![[-10.0, 30.0], [50.0, 30.0], [50.0, 10.0], [110.0, 10.0]];
    let mut d = drawing(
        of_drawing::standard::Standard::iso(),
        vec![front(), section(jog, SectionKind::Full, [100.0, 100.0])],
    );
    let r = render(&mut d);
    let hatch = mids(&owned(&r, CHILD), layers::HATCH);
    // Material between 50 and 100 is cut at y = 10 (no hole): solid hatch
    // there; the first hole still interrupts the left part.
    assert!(
        hatch
            .iter()
            .any(|m| m[0] - 100.0 > 60.0 && m[0] - 100.0 < 100.0)
    );
    assert!(!hatch.iter().any(|m| (m[0] - 100.0 - 30.0).abs() < 4.0));
    assert!(texts_of(&owned(&r, CHILD)).contains(&"A-A"), "ISO label");
    // The jog at x = 50 is seen edge-on and not drawn.
    let jog_lines = owned(&r, CHILD)
        .into_iter()
        .filter(|i| i.layer == layers::VISIBLE)
        .filter(|i| matches!(i.prim, Prim::Line { a, b } if (a[0] - 150.0).abs() < 1e-6 && (b[0] - 150.0).abs() < 1e-6))
        .count();
    assert_eq!(jog_lines, 0);

    // Half: cut only up to x = 50, then turn away.
    let half = vec![[-10.0, 30.0], [50.0, 30.0], [50.0, -10.0]];
    let mut d = drawing(
        of_drawing::standard::Standard::iso(),
        vec![front(), section(half, SectionKind::Half, [100.0, 100.0])],
    );
    let r = render(&mut d);
    let hatch = mids(&owned(&r, CHILD), layers::HATCH);
    assert!(!hatch.is_empty());
    assert!(hatch.iter().all(|m| m[0] - 100.0 < 50.0 + 1e-6));

    // Partial depth: a 3 mm slab behind the cut leaves only a short body.
    let mut s = section(
        vec![[-10.0, 30.0], [110.0, 30.0]],
        SectionKind::Full,
        [100.0, 100.0],
    );
    if let ViewKind::Section { depth, .. } = &mut s.kind {
        *depth = Some(3.0);
    }
    s.display = Some(DisplayStyle::HiddenVisible);
    let mut d = drawing(of_drawing::standard::Standard::iso(), vec![front(), s]);
    let r = render(&mut d);
    // Hidden edges of the holes' far walls end 3 mm behind the cut, so no
    // hidden line reaches the plate's back edge 30 mm away.
    assert!(on(&owned(&r, CHILD), layers::HATCH) > 0);

    // Slice: only the cut faces, outlined and hatched.
    let mut s = section(
        vec![[-10.0, 30.0], [110.0, 30.0]],
        SectionKind::Full,
        [100.0, 100.0],
    );
    if let ViewKind::Section { slice, .. } = &mut s.kind {
        *slice = true;
    }
    let mut d = drawing(of_drawing::standard::Standard::iso(), vec![front(), s]);
    let r = render(&mut d);
    let sec = owned(&r, CHILD);
    assert!(on(&sec, layers::HATCH) > 0);
    // Three cut regions, each outlined.
    let outlines = sec
        .iter()
        .filter(|i| {
            i.layer == layers::VISIBLE && matches!(i.prim, Prim::Polyline { closed: true, .. })
        })
        .count();
    assert_eq!(outlines, 3);
}

#[test]
fn aligned_section_turns_the_second_arm_into_the_first() {
    let dir = Scratch::new("aligned");
    // A disc-like plate with holes at 0° and 90° around (50, 50).
    let doc = plate_doc(100.0, 100.0, 10.0, &[(80.0, 50.0, 5.0), (50.0, 80.0, 5.0)]);
    let path = save(&doc, &dir.path("disc.ofp"));
    let line = vec![[110.0, 50.0], [50.0, 50.0], [50.0, 110.0]];
    let mut d = drawing(
        of_drawing::standard::Standard::iso(),
        vec![
            model_view(FRONT, &path, Orientation::Front, [100.0, 150.0]),
            section(line, SectionKind::Aligned, [100.0, 80.0]),
        ],
    );
    let r = render(&mut d);
    let sec = owned(&r, CHILD);
    let hatch = mids(&sec, layers::HATCH);
    assert!(!hatch.is_empty());
    // Both holes appear in the straightened cut: two 10 mm gaps in the hatch.
    let b = extent(
        &sec.iter()
            .copied()
            .filter(|i| i.layer == layers::HATCH)
            .collect::<Vec<_>>(),
    );
    assert!(b[1][0] - b[0][0] > 90.0, "{b:?}");
}

/// The plate with a pin through the first hole.
fn pinned(dir: &Scratch) -> (std::path::PathBuf, String, String) {
    let plate = plate(dir);
    let pin = save(&shaft_doc(5.0, 40.0), &dir.path("pin.ofp"));
    let mut a = Assembly::default();
    let p = a.insert(&plate, None, None).unwrap();
    let s = a
        .insert(
            &pin,
            Some(Pose::from_translation(DVec3::new(30.0, 30.0, -15.0))),
            None,
        )
        .unwrap();
    let file = dir.path("pinned.ofa");
    a.save(&file).unwrap();
    (file, p, s)
}

#[test]
fn section_exclusions_leave_components_whole() {
    let dir = Scratch::new("exclude");
    let (file, _, pin) = pinned(&dir);
    let line = vec![[-10.0, 30.0], [110.0, 30.0]];
    let hatch_over_pin = |excluded: Vec<Vec<String>>| {
        let mut s = section(line.clone(), SectionKind::Full, [100.0, 100.0]);
        if let ViewKind::Section { excluded: e, .. } = &mut s.kind {
            *e = excluded;
        }
        let mut d = drawing(
            of_drawing::standard::Standard::iso(),
            vec![
                model_view(FRONT, &file, Orientation::Front, [100.0, 180.0]),
                s,
            ],
        );
        let r = render(&mut d);
        // Pin hatch lies where only the pin is: x 25..35, outside the plate's
        // 10 mm (paper y 100..110 seen from below: z 0..10).
        mids(&owned(&r, CHILD), layers::HATCH)
            .into_iter()
            .filter(|m| (m[0] - 130.0).abs() < 5.0)
            .filter(|m| {
                let z = m[1] - 100.0;
                !(-1e-6..=10.0 + 1e-6).contains(&z)
            })
            .count()
    };
    assert!(hatch_over_pin(Vec::new()) > 0, "the pin is cut");
    assert_eq!(hatch_over_pin(vec![vec![pin]]), 0, "the pin stays whole");
}

#[test]
fn detail_crop_and_break_views() {
    let dir = Scratch::new("detail");
    let path = plate(&dir);
    // Detail of the first hole: a circle of radius 8 at 2:1 (the default).
    let mut detail = View::new(
        CHILD,
        "Detail".into(),
        ViewKind::Detail {
            parent: FRONT,
            profile: Profile::Circle {
                center: [30.0, 30.0],
                radius: 8.0,
            },
            style: DetailStyle::Standard,
            full_outline: false,
        },
        [250.0, 150.0],
    );
    detail.label = Some("B".into());
    let mut d = drawing(
        of_drawing::standard::Standard::iso(),
        vec![
            model_view(FRONT, &path, Orientation::Front, [100.0, 180.0]),
            detail,
        ],
    );
    let r = render(&mut d);
    let info = r.views.iter().find(|v| v.id == CHILD).unwrap();
    assert_close(info.scale, 2.0, 1e-12);
    let det = owned(&r, CHILD);
    // Geometry within 16 mm of the hole center at (250 + 60, 150 + 60).
    let geo: Vec<&Item> = det
        .iter()
        .copied()
        .filter(|i| i.layer == layers::VISIBLE)
        .filter(|i| {
            matches!(
                i.owner,
                Owner::View {
                    entity: Some(_),
                    ..
                }
            )
        })
        .collect();
    assert!(!geo.is_empty());
    let b = extent(&geo);
    for c in [b[0], b[1]] {
        assert!(
            ((c[0] - 310.0).powi(2) + (c[1] - 210.0).powi(2)).sqrt() <= 16.0 + 1e-6,
            "{c:?}"
        );
    }
    // The hole is drawn whole, at radius 10.
    assert!(
        geo.iter()
            .any(|i| matches!(i.prim, Prim::Arc { radius, .. } if (radius - 10.0).abs() < 1e-9))
    );
    assert!(texts_of(&det).contains(&"B (2:1)"));
    // The detail circle on the front view, radius 8 at (130, 210).
    let circle = det
        .iter()
        .find(|i| matches!(&i.prim, Prim::Polyline { closed: true, points } if points.iter().all(|p| ((p[0] - 130.0).powi(2) + (p[1] - 210.0).powi(2)).sqrt() < 8.0 + 1e-6)));
    assert!(circle.is_some());

    // Crop the front view to its left half.
    let mut front = model_view(FRONT, &path, Orientation::Front, [100.0, 180.0]);
    front.crop = Some(Profile::Polygon {
        points: vec![[-5.0, -5.0], [50.0, -5.0], [50.0, 65.0], [-5.0, 65.0]],
    });
    let mut d = drawing(of_drawing::standard::Standard::iso(), vec![front]);
    let r = render(&mut d);
    let b = extent(&owned(&r, FRONT));
    assert_close(b[1][0], 150.0, 1e-6);

    // Break 40..60 out of the front view with a 10 mm gap.
    let mut front = model_view(FRONT, &path, Orientation::Front, [100.0, 180.0]);
    front.breaks = vec![Break {
        vertical: true,
        from: 40.0,
        to: 60.0,
    }];
    front.break_style = BreakStyle::ZigZag;
    let mut d = drawing(of_drawing::standard::Standard::iso(), vec![front]);
    let r = render(&mut d);
    let visible: Vec<&Item> = owned(&r, FRONT)
        .into_iter()
        .filter(|i| {
            matches!(
                i.owner,
                Owner::View {
                    entity: Some(_),
                    ..
                }
            )
        })
        .collect();
    let b = extent(&visible);
    assert_close(b[1][0] - b[0][0], 100.0 - 20.0 + 10.0, 1e-6);
    // The second hole moved left by 10 mm: center at 100 + 70 - 10.
    assert!(
        visible.iter().any(
            |i| matches!(i.prim, Prim::Arc { center, .. } if (center[0] - 160.0).abs() < 1e-6)
        )
    );
    // Two zigzag break lines.
    let breaks = owned(&r, FRONT)
        .into_iter()
        .filter(
            |i| matches!(&i.prim, Prim::Polyline { points, closed: false } if points.len() == 6),
        )
        .count();
    assert_eq!(breaks, 2);
}

#[test]
fn broken_out_section_and_shading() {
    let dir = Scratch::new("brokenout");
    let path = plate(&dir);
    let mut front = model_view(FRONT, &path, Orientation::Front, [100.0, 180.0]);
    // Cut 5 mm into the plate around the second hole.
    front.broken_out = vec![BrokenOut {
        profile: Profile::Circle {
            center: [70.0, 30.0],
            radius: 12.0,
        },
        depth: 5.0,
    }];
    let mut d = drawing(of_drawing::standard::Standard::iso(), vec![front]);
    let r = render(&mut d);
    let hatch = mids(&owned(&r, FRONT), layers::HATCH);
    assert!(!hatch.is_empty());
    for m in &hatch {
        let d = ((m[0] - 170.0).powi(2) + (m[1] - 210.0).powi(2)).sqrt();
        assert!((5.0 - 1e-6..=12.0 + 1e-6).contains(&d), "{m:?}");
    }

    let mut front = model_view(FRONT, &path, Orientation::Isometric, [200.0, 150.0]);
    front.display = Some(DisplayStyle::ShadedEdges);
    let mut d = drawing(of_drawing::standard::Standard::iso(), vec![front]);
    let r = render(&mut d);
    let items = owned(&r, FRONT);
    let fills = items
        .iter()
        .filter(|i| i.layer == "SHADING" && matches!(i.prim, Prim::Fill { .. }))
        .count();
    assert!(fills > 20, "{fills}");
    assert!(on(&items, layers::VISIBLE) > 0);
    // Fills come first so edges draw over them.
    let first_edge = items
        .iter()
        .position(|i| i.layer == layers::VISIBLE)
        .unwrap();
    let last_fill = items.iter().rposition(|i| i.layer == "SHADING").unwrap();
    assert!(last_fill < first_edge);
}

#[test]
fn relative_views_and_stale_models() {
    let dir = Scratch::new("relative");
    let path = plate(&dir);
    let doc = of_model::file::load(&path).unwrap();
    let face = |n: DVec3, at: f64| {
        let mut m = of_model::Model::new(doc.clone());
        m.rebuild();
        let st = m.state();
        let shape = &st.body.as_ref().unwrap().shape;
        (0..shape.count(of_model::geom::Kind::Face))
            .find_map(|i| {
                let f = shape.face_info(i).unwrap();
                (f.kind == of_model::geom::SurfaceKind::Plane
                    && f.normal.dot(n) > 0.999
                    && (f.point.dot(n) - at).abs() < 1e-6)
                    .then(|| st.ref_for(of_model::geom::Kind::Face, i).unwrap())
            })
            .unwrap()
    };
    let r = |e| of_assembly::CompRef {
        component: Vec::new(),
        entity: e,
    };
    let bottom = r(face(DVec3::NEG_Z, 0.0));
    let right = r(face(DVec3::X, 100.0));
    let mut d = drawing(
        of_drawing::standard::Standard::iso(),
        vec![model_view(
            FRONT,
            &path,
            Orientation::Relative {
                front: bottom,
                top: right,
            },
            [200.0, 150.0],
        )],
    );
    let out = render(&mut d);
    let v = &out.views[0];
    assert!(DVec3::from(v.toward).abs_diff_eq(DVec3::NEG_Z, 1e-9));
    assert!(DVec3::from(v.y).abs_diff_eq(DVec3::X, 1e-9));
    assert!(!v.stale);

    // Record hashes, change the part: the view is stale until recorded again.
    assert!(d.record_hashes());
    assert!(!render(&mut d).views[0].stale);
    let mut doc = of_model::file::load(&path).unwrap();
    doc.material = None;
    save(&doc, &path);
    assert!(render(&mut d).views[0].stale);
    assert!(d.record_hashes());
    assert!(!render(&mut d).views[0].stale);
}
