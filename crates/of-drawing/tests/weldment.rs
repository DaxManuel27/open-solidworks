//! Weldment drawings: views of chosen bodies, cut list and weld tables,
//! balloons numbered by cut list item and weld symbols read from beads.

mod common;

use common::{Scratch, texts};
use of_drawing::annotations::{Annotation, AnnotationKind, BalloonText};
use of_drawing::document::*;
use of_drawing::drawing::Drawing;
use of_drawing::tables::{Table, TableKind};
use of_model::geom::{CurveKind, DVec2, Kind};
use of_model::sketch::{Sketch, tools};
use of_model::weldment::*;
use of_model::*;

/// A mitered 1200 × 600 SHS frame, two RHS cross members trimmed to it and
/// a cosmetic bead along one edge of the first cross member.
fn frame() -> (Document, FeatureId, FeatureId, FeatureId) {
    let mut doc = Document::new();
    doc.material = Some(of_model::materials::Material {
        name: "Steel".into(),
        density: 7850.0,
    });
    doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    let mut sk = Sketch::new();
    let c = tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(1200.0, 600.0));
    let a = sk
        .add_line_at(DVec2::new(0.0, 200.0), DVec2::new(1200.0, 200.0))
        .0;
    let b = sk
        .add_line_at(DVec2::new(0.0, 400.0), DVec2::new(1200.0, 400.0))
        .0;
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let seg = |entity| Ref::SketchEntity { sketch: s, entity };
    let member = |size: &str, family, groups: Vec<MemberGroup>| {
        FeatureKind::StructuralMember(StructuralMember {
            profile: ProfileSource::Standard {
                family,
                size: size.into(),
            },
            groups,
            pierce: PiercePoint::Center,
            rotation: 0.0,
            mirror: ProfileMirror::None,
            offset: [0.0, 0.0],
            merge_miter_trimmed: false,
            align: None,
            allow_protrusion: true,
            connected: Default::default(),
        })
    };
    let f1 = doc.add(member(
        "SHS 40x40x3",
        Family::Rhs,
        vec![MemberGroup {
            segments: c.entities[4..8].iter().map(|e| seg(*e)).collect(),
            corner: CornerTreatment::EndMiter,
            gap: 0.0,
        }],
    ));
    let f2 = doc.add(member(
        "RHS 60x40x3",
        Family::Rhs,
        [a, b]
            .iter()
            .map(|e| MemberGroup {
                segments: vec![seg(*e)],
                corner: CornerTreatment::EndMiter,
                gap: 0.0,
            })
            .collect(),
    ));
    let id = |f: FeatureId, k: &str| BodyId(format!("b{f}:{k}"));
    doc.add(FeatureKind::TrimExtend(TrimExtend {
        bodies: vec![id(f2, "g0.0"), id(f2, "g1.0")],
        boundary: TrimBoundary::Bodies {
            bodies: vec![id(f1, "g0.3"), id(f1, "g0.1")],
            coped: false,
        },
        kind: TrimKind::EndTrim,
        allow_extension: true,
        gap: 0.0,
    }));
    let m = Model::new(doc.clone());
    // A long straight edge of the first cross member, along x at y = 230.
    let shape = &m.state().body.as_ref().unwrap().shape;
    let e = (0..shape.count(Kind::Edge))
        .find(|e| {
            let i = shape.edge_info(*e).unwrap();
            i.kind == CurveKind::Line
                && (i.mid.y - 230.0).abs() < 1e-6
                && (i.end - i.start).length() > 1000.0
        })
        .expect("cross member edge");
    let edge = m.state().ref_for(Kind::Edge, e).unwrap();
    let bead = doc.add(FeatureKind::WeldBead(WeldBead {
        path: BeadPath::Edges { edges: vec![edge] },
        kind: BeadKind::Cosmetic,
        size: 4.0,
        sides: BeadSides::One,
        tangent: false,
        intermittent: None,
        from_to: None,
        symbol: Some("4 FILLET".into()),
        material: Some("ER70S-6".into()),
        process: Some("MAG".into()),
        mass_per_length: Some(0.1),
    }));
    let m = Model::new(doc.clone());
    for st in m.statuses() {
        assert!(st.error.is_none(), "{:?}", st.error);
    }
    (doc, f1, f2, bead)
}

fn view(id: u32, path: &str, bodies: Option<Vec<BodyId>>, at: [f64; 2]) -> View {
    View::new(
        id,
        format!("View{id}"),
        ViewKind::Model {
            model: ModelSource {
                path: path.into(),
                hash: None,
                configuration: None,
                flat: None,
                bodies,
            },
            orientation: Orientation::Front,
            exploded: false,
        },
        at,
    )
}

fn table(id: u32, at: [f64; 2], kind: TableKind) -> Annotation {
    Annotation {
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
    }
}

#[test]
fn frame_drawing_with_member_view_tables_and_balloons() {
    let dir = Scratch::new("weldment-drawing");
    let (doc, _, f2, bead) = frame();
    let path = dir.path("frame.ofp");
    of_model::file::save(&doc, &path).unwrap();
    let path = path.to_string_lossy().into_owned();
    let cross = BodyId(format!("b{f2}:g0.0"));
    let mut d = Drawing::new({
        let mut doc = DrawingDocument {
            sheets: vec![Sheet {
                id: 1,
                name: "Sheet1".into(),
                size: SheetSize::A2,
                portrait: false,
                scale: Scale::from_factor(0.25),
                projection: Projection::Third,
                format: SheetFormat::default(),
                properties_view: None,
                views: vec![
                    view(2, &path, None, [60.0, 220.0]),
                    view(3, &path, Some(vec![cross.clone()]), [60.0, 120.0]),
                ],
                annotations: vec![
                    table(
                        4,
                        [400.0, 400.0],
                        TableKind::CutList {
                            view: 2,
                            columns: Vec::new(),
                        },
                    ),
                    table(5, [400.0, 250.0], TableKind::Weld { view: 2 }),
                ],
            }],
            ..Default::default()
        };
        doc.next_id = 100;
        doc
    });
    let r = d.render(1).unwrap();
    for v in &r.views {
        assert_eq!(v.error, None, "view {}", v.id);
    }
    // The member view shows one body: 1160 long at a quarter scale.
    let member: Vec<_> = r
        .graphics
        .items
        .iter()
        .filter(|i| matches!(i.owner, of_drawing::graphics::Owner::View { view: 3, .. }))
        .collect();
    let xs: Vec<f64> = member
        .iter()
        .flat_map(|i| match &i.prim {
            of_drawing::graphics::Prim::Line { a, b } => vec![a[0], b[0]],
            _ => vec![],
        })
        .collect();
    let span =
        xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
    assert!((span - 1160.0 * 0.25).abs() < 1e-6, "{span}");
    let words = texts(&r.graphics.items).join(" ");
    for w in [
        "ITEM NO.",
        "SHS 40x40x3",
        "RHS 60x40x3",
        "1240",
        "1160",
        "45",
        "4 FILLET",
        "MAG",
    ] {
        assert!(words.contains(w), "{w} missing: {words}");
    }

    // Auto balloons: one per cut list item, numbered as the table.
    let sheet = d.doc.sheets[0].clone();
    let list = d.view_cut_list(&sheet, 2).unwrap();
    let balloons = d.auto_balloons(1, 2, Default::default(), true).unwrap();
    assert_eq!(balloons.len(), list.items.len());
    let mut doc = d.doc.clone();
    doc.sheets[0].annotations.extend(balloons.clone());
    // A weld symbol on the bead's edge, its size and process from the bead.
    let edge_item = r
        .graphics
        .items
        .iter()
        .find_map(|i| match &i.owner {
            of_drawing::graphics::Owner::View {
                view: 2,
                entity: Some(e),
            } => Some(e.clone()),
            _ => None,
        })
        .unwrap();
    doc.sheets[0].annotations.push(Annotation {
        id: 900,
        attach: Some(2),
        layer: None,
        kind: AnnotationKind::Weld {
            at: [80.0, 40.0],
            leader: Anchor::Model {
                target: ViewRef {
                    view: 2,
                    entity: edge_item,
                },
                at: [0.0, 0.0],
            },
            arrow: None,
            other: None,
            all_around: false,
            field: false,
            tail: String::new(),
            bead: Some(bead),
        },
    });
    doc.next_id = 1000;
    let mut d = Drawing::new(doc);
    let r = d.render(1).unwrap();
    let numbers: Vec<String> = balloons
        .iter()
        .map(|b| {
            let AnnotationKind::Balloon { content, .. } = &b.kind else {
                unreachable!()
            };
            assert!(matches!(content, BalloonText::CutListItem { .. }));
            texts(
                &r.graphics
                    .items
                    .iter()
                    .filter(|i| i.owner == of_drawing::graphics::Owner::Annotation { id: b.id })
                    .cloned()
                    .collect::<Vec<_>>(),
            )
            .join("")
        })
        .collect();
    let mut sorted = numbers.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        (1..=list.items.len())
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
    );
    let weld: String = texts(
        &r.graphics
            .items
            .iter()
            .filter(|i| i.owner == of_drawing::graphics::Owner::Annotation { id: 900 })
            .cloned()
            .collect::<Vec<_>>(),
    )
    .join(" ");
    assert!(weld.contains('4') && weld.contains("MAG"), "{weld}");
}
