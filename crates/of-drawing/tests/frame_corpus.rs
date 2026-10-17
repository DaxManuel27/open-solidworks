//! Phase 5 weldment corpus: a machine frame with mitered and butted
//! rectangles, legs trimmed between them, cross members, an outrigger with
//! an end cap, a gusset, cosmetic and fillet beads and a welded-on sheet
//! metal bracket; its cut list, and a drawing with a member view, cut list
//! and weld tables and balloons.

mod common;

use common::{Scratch, texts};
use of_drawing::annotations::{Annotation, AnnotationKind};
use of_drawing::document::*;
use of_drawing::drawing::Drawing;
use of_drawing::tables::{Table, TableKind};
use of_model::geom::{DVec2, DVec3, Kind, SurfaceKind};
use of_model::sketch::spatial::{Geometry3, Sketch3D};
use of_model::sketch::{Sketch, tools};
use of_model::weldment::*;
use of_model::*;

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b}");
}

fn tube(size: &str) -> ProfileSource {
    ProfileSource::Standard {
        family: Family::Rhs,
        size: size.into(),
    }
}

fn members(profile: ProfileSource, segments: Vec<Ref>, corner: CornerTreatment) -> FeatureKind {
    FeatureKind::StructuralMember(StructuralMember {
        profile,
        groups: vec![MemberGroup {
            segments,
            corner,
            gap: 0.0,
        }],
        pierce: PiercePoint::Center,
        rotation: 0.0,
        mirror: ProfileMirror::None,
        offset: [0.0, 0.0],
        merge_miter_trimmed: false,
    })
}

/// Lines of a sketch on `plane`.
fn lines(doc: &mut Document, plane: Ref, segs: &[([f64; 2], [f64; 2])]) -> Vec<Ref> {
    let mut sk = Sketch::new();
    let ids: Vec<_> = segs
        .iter()
        .map(|(a, b)| sk.add_line_at(DVec2::from(*a), DVec2::from(*b)).0)
        .collect();
    let s = doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch: sk }));
    ids.into_iter()
        .map(|entity| Ref::SketchEntity { sketch: s, entity })
        .collect()
}

/// A 1200 × 600 rectangle on `plane`: bottom, right, top, left.
fn rectangle(doc: &mut Document, plane: Ref) -> Vec<Ref> {
    let mut sk = Sketch::new();
    let c = tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(1200.0, 600.0));
    let s = doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch: sk }));
    c.entities[4..8]
        .iter()
        .map(|e| Ref::SketchEntity {
            sketch: s,
            entity: *e,
        })
        .collect()
}

fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
}

/// The planar face with normal `n` whose centroid passes `f`.
fn face(m: &Model, n: DVec3, f: impl Fn(DVec3) -> bool) -> Ref {
    let shape = &m.state().body.as_ref().unwrap().shape;
    let i = (0..shape.count(Kind::Face))
        .find(|i| {
            let info = shape.face_info(*i).unwrap();
            info.kind == SurfaceKind::Plane && info.normal.dot(n) > 1.0 - 1e-9 && f(info.centroid)
        })
        .expect("face");
    m.state().ref_for(Kind::Face, i).unwrap()
}

/// The frame and its outrigger member feature.
fn machine_frame() -> (Model, FeatureId) {
    let mut doc = Document::new();
    doc.material = Some(of_model::materials::Material {
        name: "Steel".into(),
        density: 7850.0,
    });
    doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    // Base: mitered SHS 40 rectangle at z = 0, two RHS cross members.
    let base = rectangle(&mut doc, Ref::FrontPlane);
    let fb = doc.add(members(
        tube("SHS 40x40x3"),
        base,
        CornerTreatment::EndMiter,
    ));
    let cross = lines(
        &mut doc,
        Ref::FrontPlane,
        &[
            ([0.0, 200.0], [1200.0, 200.0]),
            ([0.0, 400.0], [1200.0, 400.0]),
        ],
    );
    let fc = doc.add(members(
        tube("RHS 60x40x3"),
        cross,
        CornerTreatment::EndMiter,
    ));
    // Top: butted rectangle at z = 500.
    let top = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 500.0,
        flip: false,
    }));
    let top_lines = rectangle(&mut doc, Ref::Feature { id: top });
    doc.add(members(
        tube("SHS 40x40x3"),
        top_lines,
        CornerTreatment::EndButt1,
    ));
    // Legs up the corners, centre line to centre line.
    let mut sk = Sketch3D::default();
    let legs: Vec<_> = [[0.0, 0.0], [1200.0, 0.0], [1200.0, 600.0], [0.0, 600.0]]
        .iter()
        .map(|[x, y]| {
            let a = sk.add_point(DVec3::new(*x, *y, 0.0));
            let b = sk.add_point(DVec3::new(*x, *y, 500.0));
            sk.add(Geometry3::Line { a, b })
        })
        .collect();
    let s3 = doc.add(FeatureKind::Sketch3d(sk));
    let fl = doc.add(members(
        tube("SHS 40x40x3"),
        legs.iter()
            .map(|entity| Ref::SketchEntity {
                sketch: s3,
                entity: *entity,
            })
            .collect(),
        CornerTreatment::EndMiter,
    ));
    // An outrigger off the right side.
    let out = lines(
        &mut doc,
        Ref::FrontPlane,
        &[([1200.0, 300.0], [1400.0, 300.0])],
    );
    let fo = doc.add(members(tube("SHS 20x20x2"), out, CornerTreatment::EndMiter));
    let id = |f: FeatureId, k: &str| BodyId(format!("b{f}:{k}"));
    doc.add(FeatureKind::TrimExtend(TrimExtend {
        bodies: vec![id(fc, "g0.0"), id(fc, "g0.1")],
        boundary: TrimBoundary::Bodies {
            bodies: vec![id(fb, "g0.3"), id(fb, "g0.1")],
            coped: false,
        },
        kind: TrimKind::EndTrim,
        allow_extension: true,
        gap: 0.0,
    }));
    let mut m = Model::new(doc);
    ok(&m);
    // Legs trimmed between the rectangles' faces, the outrigger to the
    // base's outer side.
    let base_top = face(&m, DVec3::Z, |c| {
        (c.z - 20.0).abs() < 1e-6 && c.y.abs() < 20.0
    });
    let top_under = face(&m, -DVec3::Z, |c| {
        (c.z - 480.0).abs() < 1e-6 && c.y.abs() < 20.0
    });
    m.doc.add(FeatureKind::TrimExtend(TrimExtend {
        bodies: (0..4).map(|i| id(fl, &format!("g0.{i}"))).collect(),
        boundary: TrimBoundary::Faces {
            faces: vec![base_top, top_under],
        },
        kind: TrimKind::EndTrim,
        allow_extension: true,
        gap: 0.0,
    }));
    let outer = face(&m, DVec3::X, |c| {
        (c.x - 1220.0).abs() < 1e-6 && c.z.abs() < 1e-6
    });
    m.doc.add(FeatureKind::TrimExtend(TrimExtend {
        bodies: vec![id(fo, "g0.0")],
        boundary: TrimBoundary::Faces {
            faces: vec![outer.clone()],
        },
        kind: TrimKind::EndTrim,
        allow_extension: true,
        gap: 0.0,
    }));
    m.rebuild();
    ok(&m);
    // Cap on the outrigger's end, a gusset under it.
    let end = face(&m, DVec3::X, |c| (c.x - 1400.0).abs() < 1e-6);
    m.doc.add(FeatureKind::EndCap(EndCap {
        faces: vec![end],
        direction: CapDirection::Outward,
        thickness: 5.0,
        offset: CapOffset::Distance { distance: 0.0 },
        corners: None,
    }));
    let outer = face(&m, DVec3::X, |c| {
        (c.x - 1220.0).abs() < 1e-6 && c.z.abs() < 1e-6
    });
    let near = |c: DVec3| c.x > 1300.0 && c.x < 1400.0;
    let side = face(&m, DVec3::Y, |c| near(c) && (c.y - 310.0).abs() < 1e-6);
    m.doc.add(FeatureKind::WeldGusset(WeldGusset {
        faces: [outer.clone(), side],
        shape: GussetShape::Triangle {
            d1: 100.0,
            d2: 100.0,
        },
        chamfer: 10.0,
        thickness: 5.0,
        side: GussetSide::Both,
        location: GussetLocation::Middle,
        offset: 0.0,
    }));
    m.rebuild();
    ok(&m);
    // Beads: three sides cosmetic, a modeled fillet on top.
    let bead = |path, kind, size| WeldBead {
        path,
        kind,
        size,
        sides: BeadSides::One,
        tangent: false,
        intermittent: None,
        from_to: None,
        symbol: Some(format!("{size} FILLET")),
        material: Some("ER70S-6".into()),
        process: Some("MAG".into()),
        mass_per_length: Some(0.1),
    };
    let sides = [DVec3::Y, -DVec3::Y, -DVec3::Z]
        .map(|n| face(&m, n, |c| near(c) && (c.y - 300.0).abs() < 11.0))
        .to_vec();
    let top_face = face(&m, DVec3::Z, |c| near(c) && (c.z - 10.0).abs() < 1e-6);
    m.doc.add(FeatureKind::WeldBead(bead(
        BeadPath::Faces {
            from: sides,
            to: vec![outer.clone()],
        },
        BeadKind::Cosmetic,
        4.0,
    )));
    m.doc.add(FeatureKind::WeldBead(bead(
        BeadPath::Faces {
            from: vec![top_face],
            to: vec![outer],
        },
        BeadKind::Fillet,
        3.0,
    )));
    // The bracket: a 100 × 40 × 3 plate on the top rectangle's front member.
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(500.0, -20.0), DVec2::new(600.0, 20.0));
    let on_top = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 520.0,
        flip: false,
    }));
    let s = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::Feature { id: on_top },
        sketch: sk,
    }));
    m.doc
        .add(FeatureKind::BaseFlange(of_model::sheet_metal::BaseFlange {
            sketch: s,
            sheet: of_model::sheet_metal::SheetMetalParams {
                thickness: 3.0,
                radius: 3.0,
                ..Default::default()
            },
            side: of_model::sheet_metal::ThicknessSide::Normal,
            end1: None,
            end2: None,
            reverse: false,
        }));
    m.rebuild();
    ok(&m);
    (m, fo)
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

#[test]
fn machine_frame_corpus() {
    let dir = Scratch::new("frame-corpus");
    let (m, fo) = machine_frame();
    let state = m.state();
    // 15 members, a cap, a gusset, a fillet bead and the bracket.
    assert_eq!(state.members.len(), 15);
    assert_eq!(state.bodies.len(), 19);
    let list = of_model::weldment::cutlist::cut_list(&m.doc, state).unwrap();
    let rows: Vec<(&str, &str, &str, &str, u32)> = list
        .items
        .iter()
        .map(|i| {
            let p = |k: &str| i.properties.get(k).map(String::as_str).unwrap_or("");
            (
                p("DESCRIPTION"),
                p("LENGTH"),
                p("ANGLE1"),
                p("MASS"),
                i.quantity,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("SHS 40x40x3", "1240", "45", "3.964", 2),
            ("SHS 40x40x3", "640", "45", "1.982", 2),
            ("RHS 60x40x3", "1160", "0", "4.925", 2),
            ("SHS 40x40x3", "1240", "0", "4.096", 1),
            ("SHS 40x40x3", "600", "0", "1.982", 1),
            ("SHS 40x40x3", "1200", "0", "3.964", 1),
            ("SHS 40x40x3", "560", "0", "1.85", 1),
            ("SHS 40x40x3", "460", "0", "1.52", 4),
            ("SHS 20x20x2", "180", "0", "0.189", 1),
            ("Sheet 3 mm", "", "", "0.094", 1),
            ("Body", "", "", "0.015", 1),
            ("Body", "", "", "0.194", 1),
        ]
    );
    let blank = &list.items[9].properties;
    assert_eq!(blank["BLANK LENGTH"], "100");
    assert_eq!(blank["BLANK WIDTH"], "40");
    assert_eq!(blank["CUTTING LENGTH"], "280");
    // Welds: three 12 mm cosmetic seams and a 12 mm modeled fillet (its
    // mass from its volume, the cosmetic one's from 0.1 kg/m).
    assert_eq!(list.welds.beads.len(), 2);
    close(list.welds.total_length, 48.0, 1e-6);
    close(
        list.welds.total_mass,
        0.036 * 0.1 + 0.5 * 3.0 * 3.0 * 12.0 * 7850e-9,
        1e-9,
    );

    // The drawing: the whole frame, the outrigger alone, both tables.
    let path = dir.path("frame.ofp");
    of_model::file::save(&m.doc, &path).unwrap();
    let path = path.to_string_lossy().into_owned();
    let outrigger = BodyId(format!("b{fo}:g0.0"));
    let mut doc = DrawingDocument {
        sheets: vec![Sheet {
            id: 1,
            name: "Sheet1".into(),
            size: SheetSize::A2,
            portrait: false,
            scale: Scale::from_factor(0.2),
            projection: Projection::Third,
            format: SheetFormat::default(),
            properties_view: None,
            views: vec![
                view(2, &path, None, [80.0, 250.0]),
                view(3, &path, Some(vec![outrigger]), [80.0, 120.0]),
            ],
            annotations: vec![
                table(
                    4,
                    [360.0, 400.0],
                    TableKind::CutList {
                        view: 2,
                        columns: Vec::new(),
                    },
                ),
                table(5, [360.0, 150.0], TableKind::Weld { view: 2 }),
            ],
        }],
        ..Default::default()
    };
    doc.next_id = 100;
    let mut d = Drawing::new(doc);
    let r = d.render(1).unwrap();
    for v in &r.views {
        assert_eq!(v.error, None, "view {}", v.id);
    }
    let xs: Vec<f64> = r
        .graphics
        .items
        .iter()
        .filter(|i| matches!(i.owner, of_drawing::graphics::Owner::View { view: 3, .. }))
        .flat_map(|i| match &i.prim {
            of_drawing::graphics::Prim::Line { a, b } => vec![a[0], b[0]],
            _ => vec![],
        })
        .collect();
    let span =
        xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
    close(span, 180.0 * 0.2, 1e-6);
    let words = texts(&r.graphics.items).join(" ");
    for w in [
        "SHS 20x20x2",
        "RHS 60x40x3",
        "Sheet 3 mm",
        "460",
        "4 FILLET",
        "3 FILLET",
        "MAG",
    ] {
        assert!(words.contains(w), "{w} missing");
    }
    let sheet = d.doc.sheets[0].clone();
    let AnnotationKind::Table(t) = &sheet.annotations[0].kind else {
        unreachable!()
    };
    let csv = d.table_cells(&sheet, t).unwrap().csv();
    assert!(
        csv.starts_with(
            "ITEM NO.,QTY.,DESCRIPTION,LENGTH,ANGLE1,ANGLE2\n1,2,SHS 40x40x3,1240,45,45\n"
        ),
        "{csv}"
    );
    assert_eq!(csv.lines().count(), 13);
    // A balloon per cut list item the view shows: looking down, the top
    // rectangle hides the base rectangle (items 1 and 2) and the legs (8).
    let balloons = d.auto_balloons(1, 2, Default::default(), true).unwrap();
    let mut items: Vec<u32> = balloons
        .iter()
        .map(|b| {
            let AnnotationKind::Balloon {
                content: of_drawing::annotations::BalloonText::CutListItem { body },
                ..
            } = &b.kind
            else {
                panic!("not a cut list balloon")
            };
            list.items
                .iter()
                .find(|i| i.bodies.contains(body))
                .and_then(|i| i.number)
                .unwrap()
        })
        .collect();
    items.sort();
    assert_eq!(items, [3, 4, 5, 6, 7, 9, 10, 11, 12]);
}
