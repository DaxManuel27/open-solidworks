//! Weldments: profiles, the Weldment feature, structural members, trims,
//! end caps, gussets, weld beads and cut lists.

use of_model::geom::DVec2;
use of_model::sketch::{Sketch, tools};
use of_model::weldment::*;
use of_model::*;

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

/// Volumes against the outline's area, which samples arcs every 0.5°.
fn close_rel(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-5 * b.abs(), "{a} != {b}");
}

#[test]
fn custom_profiles_come_from_a_part_sketch() {
    let dir = std::env::temp_dir().join(format!("of-weld-profile-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tee.ofp");
    // A 30 × 20 rectangle with a 10 × 6 slot cut from it, off center.
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(100.0, 50.0), DVec2::new(130.0, 70.0));
    tools::corner_rectangle(&mut sk, DVec2::new(110.0, 57.0), DVec2::new(120.0, 63.0));
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    of_model::file::save(&doc, &path).unwrap();
    let section = section(&ProfileSource::Custom {
        path: path.to_string_lossy().into_owned(),
        sketch: s,
    })
    .unwrap();
    close(section.size.x, 30.0, 1e-9);
    close(section.size.y, 20.0, 1e-9);
    let (area, centroid, _, _) = profiles::properties(&section);
    close(area, 600.0 - 60.0, 1e-6);
    // Centered on its bounding box.
    close(centroid.x, 0.0, 1e-6);
    close(centroid.y, 0.0, 1e-6);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_part_has_one_weldment_feature() {
    let mut doc = Document::new();
    doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    let second = doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    let m = Model::new(doc);
    let status = m.statuses().into_iter().find(|s| s.id == second).unwrap();
    assert!(status.error.unwrap().contains("one Weldment"));
    assert!(m.state().weldment.is_some());
}

const SHS: &str = "SHS 40x40x3";

fn shs() -> ProfileSource {
    ProfileSource::Standard {
        family: Family::Rhs,
        size: SHS.into(),
    }
}

fn shs_area() -> f64 {
    profiles::properties(&profiles::section(Family::Rhs, SHS).unwrap()).0
}

/// A weldment part with a w × h rectangle sketch on the front plane; returns
/// the sketch and its four lines (bottom, right, top, left).
fn rectangle_frame(w: f64, h: f64) -> (Document, FeatureId, Vec<Ref>) {
    let mut doc = Document::new();
    doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    let mut sk = Sketch::new();
    let created = tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(w, h));
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let lines = created.entities[4..8]
        .iter()
        .map(|e| Ref::SketchEntity {
            sketch: s,
            entity: *e,
        })
        .collect();
    (doc, s, lines)
}

fn member(segments: Vec<Ref>, corner: CornerTreatment) -> StructuralMember {
    StructuralMember {
        profile: shs(),
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
    }
}

fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
}

fn volume(m: &Model) -> f64 {
    m.state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume
}

#[test]
fn mitered_rectangle_of_square_tube() {
    let (mut doc, _, lines) = rectangle_frame(1200.0, 600.0);
    let f = doc.add(FeatureKind::StructuralMember(member(
        lines,
        CornerTreatment::EndMiter,
    )));
    let m = Model::new(doc);
    ok(&m);
    assert_eq!(m.state().bodies.len(), 4);
    // Miters through the centerline keep area × centerline length.
    close_rel(volume(&m), shs_area() * 2.0 * (1200.0 + 600.0));
    let records = &m.state().members;
    assert_eq!(records.len(), 4);
    for r in records {
        assert_eq!(r.feature, f);
        let centerline = if r.segment % 2 == 0 { 1200.0 } else { 600.0 };
        close(r.length, centerline + 40.0, 1e-6);
        for a in r.angles {
            close(a, std::f64::consts::FRAC_PI_4, 1e-9);
        }
    }
    // Merged into one body per group when asked.
    let (mut doc, _, lines) = rectangle_frame(1200.0, 600.0);
    let mut p = member(lines, CornerTreatment::EndMiter);
    p.merge_miter_trimmed = true;
    doc.add(FeatureKind::StructuralMember(p));
    let merged = Model::new(doc);
    ok(&merged);
    assert_eq!(merged.state().bodies.len(), 1);
    close(volume(&merged), volume(&m), 1e-3);
}

#[test]
fn butted_corners_run_one_member_through() {
    let (mut doc, _, lines) = rectangle_frame(1200.0, 600.0);
    doc.add(FeatureKind::StructuralMember(member(
        lines,
        CornerTreatment::EndButt1,
    )));
    let m = Model::new(doc);
    ok(&m);
    // Earlier segments run through: bottom at both ends, right past top,
    // top past left; left stops at both.
    let length = |i: usize| {
        m.state()
            .members
            .iter()
            .find(|r| r.segment == i)
            .unwrap()
            .length
    };
    close(length(0), 1240.0, 1e-6);
    close(length(1), 600.0, 1e-6);
    close(length(2), 1200.0, 1e-6);
    close(length(3), 560.0, 1e-6);
    for r in &m.state().members {
        for a in r.angles {
            close(a, 0.0, 1e-9);
        }
    }
    close_rel(volume(&m), shs_area() * 3600.0);
}

#[test]
fn members_keep_their_bodies_when_the_sketch_changes() {
    let (mut doc, s, lines) = rectangle_frame(1200.0, 600.0);
    doc.add(FeatureKind::StructuralMember(member(
        lines,
        CornerTreatment::EndMiter,
    )));
    let mut m = Model::new(doc);
    ok(&m);
    let ids: Vec<BodyId> = m.state().bodies.iter().map(|b| b.id.clone()).collect();
    let before = volume(&m);
    // Widen the rectangle: move the right side's points.
    if let FeatureKind::Sketch(sf) = &mut m.doc.feature_mut(s).unwrap().kind {
        let right: Vec<_> = sf
            .sketch
            .entities
            .iter()
            .filter_map(|(id, e)| match e.geo {
                of_model::sketch::Geometry::Point { p } if (p.x - 1200.0).abs() < 1e-9 => Some(*id),
                _ => None,
            })
            .collect();
        for id in right {
            if let Some(e) = sf.sketch.entities.get_mut(&id)
                && let of_model::sketch::Geometry::Point { p } = &mut e.geo
            {
                p.x = 1500.0;
            }
        }
    }
    m.invalidate(s);
    m.rebuild();
    ok(&m);
    let after: Vec<BodyId> = m.state().bodies.iter().map(|b| b.id.clone()).collect();
    assert_eq!(ids, after);
    close_rel(volume(&m) - before, shs_area() * 600.0);
}

#[test]
fn members_follow_3d_sketch_lines_with_any_pierce_point() {
    let mut doc = Document::new();
    doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    let mut sk = of_model::sketch::spatial::Sketch3D::default();
    let a = sk.add_point(of_model::geom::DVec3::ZERO);
    let b = sk.add_point(of_model::geom::DVec3::new(0.0, 0.0, 800.0));
    let line = sk.add(of_model::sketch::spatial::Geometry3::Line { a, b });
    let s = doc.add(FeatureKind::Sketch3d(sk));
    let seg = Ref::SketchEntity {
        sketch: s,
        entity: line,
    };
    let mut p = member(vec![seg], CornerTreatment::EndMiter);
    p.pierce = PiercePoint::BottomLeft;
    doc.add(FeatureKind::StructuralMember(p));
    let m = Model::new(doc);
    ok(&m);
    close_rel(volume(&m), shs_area() * 800.0);
    // The tube sits beside the line: its bottom left corner on the axis.
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    close(b.max.z - b.min.z, 800.0, 1e-6);
    assert!(b.min.x.abs() < 1e-6 || b.max.x.abs() < 1e-6, "{b:?}");
}

/// A sketch of lines on the front plane; returns it and its line refs.
fn lines(doc: &mut Document, segs: &[([f64; 2], [f64; 2])]) -> Vec<Ref> {
    let mut sk = Sketch::new();
    let ids: Vec<_> = segs
        .iter()
        .map(|(a, b)| sk.add_line_at(DVec2::from(*a), DVec2::from(*b)).0)
        .collect();
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    ids.into_iter()
        .map(|entity| Ref::SketchEntity { sketch: s, entity })
        .collect()
}

fn along(profile: ProfileSource, segments: Vec<Ref>) -> FeatureKind {
    let mut p = member(segments, CornerTreatment::EndMiter);
    p.profile = profile;
    FeatureKind::StructuralMember(p)
}

fn rhs() -> ProfileSource {
    ProfileSource::Standard {
        family: Family::Rhs,
        size: "RHS 60x40x3".into(),
    }
}

fn record(m: &Model, body: &BodyId) -> MemberRecord {
    m.state()
        .members
        .iter()
        .find(|r| r.body == *body)
        .unwrap()
        .clone()
}

fn body_volume(m: &Model, id: &BodyId) -> f64 {
    m.state()
        .solid(id)
        .unwrap()
        .geometry
        .shape
        .mass_props()
        .unwrap()
        .volume
}

#[test]
fn cross_member_trimmed_to_the_frame() {
    let (mut doc, _, frame) = rectangle_frame(1200.0, 600.0);
    let f1 = doc.add(FeatureKind::StructuralMember(member(
        frame,
        CornerTreatment::EndMiter,
    )));
    let cross = lines(&mut doc, &[([0.0, 300.0], [1200.0, 300.0])]);
    let f2 = doc.add(along(rhs(), cross));
    let id = |f: FeatureId, k: &str| BodyId(format!("b{f}:{k}"));
    doc.add(FeatureKind::TrimExtend(TrimExtend {
        bodies: vec![id(f2, "g0.0")],
        boundary: TrimBoundary::Bodies {
            bodies: vec![id(f1, "g0.3"), id(f1, "g0.1")],
            coped: false,
        },
        kind: TrimKind::EndTrim,
        allow_extension: true,
        gap: 0.0,
    }));
    let m = Model::new(doc);
    ok(&m);
    // Between the side members' inner faces at x = 20 and 1180.
    let r = record(&m, &id(f2, "g0.0"));
    close(r.length, 1160.0, 1e-6);
    assert_eq!(r.angles, [0.0, 0.0]);
    let area = profiles::properties(&profiles::section(Family::Rhs, "RHS 60x40x3").unwrap()).0;
    close_rel(body_volume(&m, &id(f2, "g0.0")), area * 1160.0);
}

#[test]
fn members_extend_to_a_plane_with_a_gap() {
    for (allow, length) in [(true, 498.0), (false, 300.0)] {
        let mut doc = Document::new();
        doc.add(FeatureKind::Weldment(WeldmentDef::default()));
        let segs = lines(&mut doc, &[([0.0, 0.0], [0.0, 300.0])]);
        let f = doc.add(along(shs(), segs));
        let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
            reference: Ref::TopPlane,
            distance: 500.0,
            flip: false,
        }));
        doc.add(FeatureKind::TrimExtend(TrimExtend {
            bodies: vec![BodyId(format!("b{f}:g0.0"))],
            boundary: TrimBoundary::Faces {
                faces: vec![Ref::Feature { id: plane }],
            },
            kind: TrimKind::EndTrim,
            allow_extension: allow,
            gap: 2.0,
        }));
        let m = Model::new(doc);
        ok(&m);
        let r = record(&m, &BodyId(format!("b{f}:g0.0")));
        close(r.length, length, 1e-6);
        close_rel(volume(&m), shs_area() * length);
    }
}

#[test]
fn round_tube_coped_onto_a_post() {
    let chs = |size: &str| ProfileSource::Standard {
        family: Family::Chs,
        size: size.into(),
    };
    let mut doc = Document::new();
    doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    let post = lines(&mut doc, &[([0.0, 0.0], [0.0, 400.0])]);
    let fp = doc.add(along(chs("CHS 88.9x5"), post));
    let branch = lines(&mut doc, &[([300.0, 200.0], [0.0, 200.0])]);
    let fb = doc.add(along(chs("CHS 48.3x3"), branch));
    let branch_id = BodyId(format!("b{fb}:g0.0"));
    let before = {
        let m = Model::new(doc.clone());
        ok(&m);
        body_volume(&m, &branch_id)
    };
    doc.add(FeatureKind::TrimExtend(TrimExtend {
        bodies: vec![branch_id.clone()],
        boundary: TrimBoundary::Bodies {
            bodies: vec![BodyId(format!("b{fp}:g0.0"))],
            coped: true,
        },
        kind: TrimKind::EndTrim,
        allow_extension: true,
        gap: 0.0,
    }));
    let m = Model::new(doc);
    ok(&m);
    let after = body_volume(&m, &branch_id);
    // The branch now stops at the post's surface: between its outside
    // (44.45 from the axis) and its axis.
    let area = profiles::properties(&profiles::section(Family::Chs, "CHS 48.3x3").unwrap()).0;
    assert!(after < before - area * 40.0 && after > area * (300.0 - 44.45 - 1.0));
    assert_eq!(m.state().bodies.len(), 2);
}

#[test]
fn two_members_mitered_by_trim() {
    let mut doc = Document::new();
    doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    let a = lines(&mut doc, &[([0.0, 0.0], [1000.0, 0.0])]);
    let fa = doc.add(along(shs(), a));
    let b = lines(&mut doc, &[([0.0, 0.0], [0.0, 500.0])]);
    let fb = doc.add(along(shs(), b));
    let (ia, ib) = (BodyId(format!("b{fa}:g0.0")), BodyId(format!("b{fb}:g0.0")));
    doc.add(FeatureKind::TrimExtend(TrimExtend {
        bodies: vec![ia.clone()],
        boundary: TrimBoundary::Bodies {
            bodies: vec![ib.clone()],
            coped: false,
        },
        kind: TrimKind::EndMiter,
        allow_extension: true,
        gap: 0.0,
    }));
    let m = Model::new(doc);
    ok(&m);
    close(record(&m, &ia).length, 1020.0, 1e-6);
    close(record(&m, &ib).length, 520.0, 1e-6);
    close(record(&m, &ia).angles[0], std::f64::consts::FRAC_PI_4, 1e-9);
    close_rel(volume(&m), shs_area() * 1500.0);
}

/// The part's planar face with outward normal `n` whose centroid satisfies `f`.
fn face(m: &Model, n: of_model::geom::DVec3, f: impl Fn(of_model::geom::DVec3) -> bool) -> Ref {
    let shape = &m.state().body.as_ref().unwrap().shape;
    let i = (0..shape.count(of_model::geom::Kind::Face))
        .find(|i| {
            let info = shape.face_info(*i).unwrap();
            info.kind == of_model::geom::SurfaceKind::Plane
                && info.normal.dot(n) > 1.0 - 1e-9
                && f(info.centroid)
        })
        .expect("face");
    m.state().ref_for(of_model::geom::Kind::Face, i).unwrap()
}

#[test]
fn end_caps_outward_and_inward() {
    use of_model::geom::DVec3;
    let hea = ProfileSource::Standard {
        family: Family::Hea,
        size: "HEA 100".into(),
    };
    for (profile, direction, offset, corners, area) in [
        // A full 100 × 96 plate on the beam's end, then with chamfered corners.
        (
            hea.clone(),
            CapDirection::Outward,
            CapOffset::Distance { distance: 0.0 },
            None,
            9600.0,
        ),
        (
            hea,
            CapDirection::Outward,
            CapOffset::Distance { distance: 0.0 },
            Some(of_model::sheet_metal::CornerBreak::Chamfer { distance: 5.0 }),
            9600.0 - 4.0 * 12.5,
        ),
        // A plug in the square tube's 34 × 34 opening with its R3 corners.
        (
            shs(),
            CapDirection::Inward,
            CapOffset::Distance { distance: 0.0 },
            None,
            34.0 * 34.0 - (4.0 - std::f64::consts::PI) * 9.0,
        ),
    ] {
        let mut doc = Document::new();
        doc.add(FeatureKind::Weldment(WeldmentDef::default()));
        let segs = lines(&mut doc, &[([0.0, 0.0], [0.0, 300.0])]);
        doc.add(along(profile, segs));
        let mut m = Model::new(doc);
        ok(&m);
        let before = volume(&m);
        let end = face(&m, DVec3::Y, |c| (c.y - 300.0).abs() < 1e-6);
        m.doc.add(FeatureKind::EndCap(EndCap {
            faces: vec![end],
            direction,
            thickness: 5.0,
            offset,
            corners,
        }));
        m.rebuild();
        ok(&m);
        assert_eq!(m.state().bodies.len(), 2);
        // Arcs are sampled every 0.5° in the cap outline.
        let added = volume(&m) - before;
        assert!(
            (added - 5.0 * area).abs() < 2e-3 * 5.0 * area,
            "{added} vs {}",
            5.0 * area
        );
    }
}

#[test]
fn gusset_in_the_corner_of_a_post_and_beam() {
    use of_model::geom::DVec3;
    for (chamfer, area) in [(0.0, 5000.0), (10.0, 5000.0 - 50.0)] {
        let mut doc = Document::new();
        doc.add(FeatureKind::Weldment(WeldmentDef::default()));
        let post = lines(&mut doc, &[([0.0, 0.0], [0.0, 500.0])]);
        doc.add(along(shs(), post));
        let beam = lines(&mut doc, &[([0.0, 500.0], [500.0, 500.0])]);
        doc.add(along(shs(), beam));
        let mut m = Model::new(doc);
        ok(&m);
        let before = volume(&m);
        // The post's right side and the beam's underside.
        let side = face(&m, DVec3::X, |c| (c.x - 20.0).abs() < 1e-6 && c.y < 400.0);
        let under = face(&m, -DVec3::Y, |c| (c.y - 480.0).abs() < 1e-6 && c.x > 100.0);
        m.doc.add(FeatureKind::WeldGusset(WeldGusset {
            faces: [side, under],
            shape: GussetShape::Triangle {
                d1: 100.0,
                d2: 100.0,
            },
            chamfer,
            thickness: 10.0,
            side: GussetSide::Both,
            location: GussetLocation::Middle,
            offset: 0.0,
        }));
        m.rebuild();
        ok(&m);
        assert_eq!(m.state().bodies.len(), 3);
        close(volume(&m) - before, area * 10.0, 1e-6);
    }
}

/// A 60 mm post up the y axis (flat sides 44 wide) and a 40 mm beam
/// butting into its right side at y = 250.
fn post_and_beam() -> Model {
    let mut doc = Document::new();
    doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    let post = lines(&mut doc, &[([0.0, 0.0], [0.0, 500.0])]);
    doc.add(along(
        ProfileSource::Standard {
            family: Family::Rhs,
            size: "SHS 60x60x4".into(),
        },
        post,
    ));
    let beam = lines(&mut doc, &[([30.0, 250.0], [300.0, 250.0])]);
    doc.add(along(shs(), beam));
    let m = Model::new(doc);
    ok(&m);
    m
}

fn bead(path: BeadPath) -> WeldBead {
    WeldBead {
        path,
        kind: BeadKind::Cosmetic,
        size: 5.0,
        sides: BeadSides::One,
        tangent: false,
        intermittent: None,
        from_to: None,
        symbol: Some("5 ⊿".into()),
        material: Some("ER70S-6".into()),
        process: Some("MIG".into()),
        mass_per_length: Some(0.15),
    }
}

/// The beam's four flat sides and the post's right face.
fn seam_faces(m: &Model) -> (Vec<Ref>, Ref) {
    use of_model::geom::DVec3;
    let sides = [DVec3::Y, -DVec3::Y, DVec3::Z, -DVec3::Z]
        .iter()
        .map(|n| face(m, *n, |c| c.x > 100.0 && (c.y - 250.0).abs() < 30.0))
        .collect();
    let post = face(m, DVec3::X, |c| (c.x - 30.0).abs() < 1e-6);
    (sides, post)
}

#[test]
fn cosmetic_beads_on_the_seams_between_faces() {
    let mut m = post_and_beam();
    let before = volume(&m);
    let (sides, post) = seam_faces(&m);
    let path = BeadPath::Faces {
        from: sides,
        to: vec![post],
    };
    // Whole seams: four flats of 40 less the R6 corners.
    let mut p = bead(path.clone());
    m.doc.add(FeatureKind::WeldBead(p.clone()));
    m.rebuild();
    ok(&m);
    let w = &m.state().welds[0];
    assert_eq!(w.pieces.len(), 4);
    close(w.length, 4.0 * 28.0, 1e-6);
    close(volume(&m), before, 1e-9);
    // Both sides, intermittent 10 on 10, staggered.
    let mut m2 = post_and_beam();
    p.sides = BeadSides::Both;
    p.intermittent = Some(Intermittent {
        length: 10.0,
        gap: 10.0,
        staggered: true,
    });
    m2.doc.add(FeatureKind::WeldBead(p));
    m2.rebuild();
    ok(&m2);
    let w = &m2.state().welds[0];
    // Side one: 0–10, 20–28 per seam; side two: 10–20 per seam.
    close(w.length, 4.0 * (10.0 + 8.0 + 10.0), 1e-6);
    assert_eq!(w.pieces.len(), 4 * 3);
}

#[test]
fn modeled_fillet_bead_adds_its_triangle() {
    use of_model::geom::DVec3;
    let mut m = post_and_beam();
    let before = volume(&m);
    let top = face(&m, DVec3::Y, |c| c.x > 100.0 && (c.y - 270.0).abs() < 1e-6);
    let (_, post) = seam_faces(&m);
    let mut p = bead(BeadPath::Faces {
        from: vec![top],
        to: vec![post],
    });
    p.kind = BeadKind::Fillet;
    m.doc.add(FeatureKind::WeldBead(p));
    m.rebuild();
    ok(&m);
    assert_eq!(m.state().bodies.len(), 3);
    close(volume(&m) - before, 0.5 * 5.0 * 5.0 * 28.0, 1e-6);
    assert_eq!(m.state().welds[0].bodies.len(), 1);
}

/// The frame: a mitered 1200 × 600 SHS rectangle, two RHS cross members
/// trimmed to its sides, and a 100 × 60 × 3 sheet metal bracket on top.
fn frame_with_bracket() -> (Model, FeatureId, FeatureId) {
    let (mut doc, _, frame) = rectangle_frame(1200.0, 600.0);
    doc.material = Some(of_model::materials::Material {
        name: "Steel".into(),
        density: 7850.0,
    });
    let f1 = doc.add(FeatureKind::StructuralMember(member(
        frame,
        CornerTreatment::EndMiter,
    )));
    let cross = lines(
        &mut doc,
        &[
            ([0.0, 200.0], [1200.0, 200.0]),
            ([0.0, 400.0], [1200.0, 400.0]),
        ],
    );
    let f2 = doc.add(along(rhs(), cross));
    let id = |f: FeatureId, k: &str| BodyId(format!("b{f}:{k}"));
    doc.add(FeatureKind::TrimExtend(TrimExtend {
        bodies: vec![id(f2, "g0.0"), id(f2, "g0.1")],
        boundary: TrimBoundary::Bodies {
            bodies: vec![id(f1, "g0.3"), id(f1, "g0.1")],
            coped: false,
        },
        kind: TrimKind::EndTrim,
        allow_extension: true,
        gap: 0.0,
    }));
    // The bracket: a plate standing clear of the frame.
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(500.0, 250.0), DVec2::new(600.0, 310.0));
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference: Ref::FrontPlane,
        distance: 20.0,
        flip: false,
    }));
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::Feature { id: plane },
        sketch: sk,
    }));
    doc.add(FeatureKind::BaseFlange(of_model::sheet_metal::BaseFlange {
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
    let m = Model::new(doc);
    ok(&m);
    (m, f1, f2)
}

fn prop<'a>(item: &'a cutlist::CutListItem, k: &str) -> &'a str {
    item.properties.get(k).map(String::as_str).unwrap_or("")
}

#[test]
fn cut_list_groups_members_and_sheet_metal() {
    let (mut m, f1, f2) = frame_with_bracket();
    let list = cutlist::cut_list(&m.doc, m.state()).unwrap();
    let summary: Vec<(String, String, u32, String, String)> = list
        .items
        .iter()
        .map(|i| {
            (
                prop(i, "DESCRIPTION").into(),
                prop(i, "LENGTH").into(),
                i.quantity,
                prop(i, "ANGLE1").into(),
                prop(i, "ANGLE2").into(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                "SHS 40x40x3".into(),
                "1240".into(),
                2,
                "45".into(),
                "45".into()
            ),
            (
                "SHS 40x40x3".into(),
                "640".into(),
                2,
                "45".into(),
                "45".into()
            ),
            (
                "RHS 60x40x3".into(),
                "1160".into(),
                2,
                "0".into(),
                "0".into()
            ),
            ("Sheet 3 mm".into(), "".into(), 1, "".into(), "".into()),
        ]
    );
    let sheet = &list.items[3];
    assert_eq!(sheet.kind, cutlist::ItemKind::SheetMetal);
    assert_eq!(prop(sheet, "BLANK LENGTH"), "100");
    assert_eq!(prop(sheet, "BLANK WIDTH"), "60");
    assert_eq!(
        list.items.iter().map(|i| i.number).collect::<Vec<_>>(),
        vec![Some(1), Some(2), Some(3), Some(4)]
    );
    // Each SHS 1240 weighs its section × 1200 at 7850 kg/m³.
    let mass: f64 = prop(&list.items[0], "MASS").parse().unwrap();
    close(mass, shs_area() * 1200.0 * 7850e-9, 1e-3);

    // Exclude a cross member, group two sides by hand, override a
    // description, put the other cross member in a sub-weldment.
    let id = |f: FeatureId, k: &str| BodyId(format!("b{f}:{k}"));
    m.doc.cut_list.excluded = vec![id(f2, "g0.0")];
    m.doc.cut_list.groups = vec![vec![id(f1, "g0.0"), id(f1, "g0.1")]];
    m.doc.cut_list.overrides.insert(
        id(f1, "g0.0"),
        [("Description".into(), "Base frame side".into())].into(),
    );
    m.doc.cut_list.sub_weldments = vec![cutlist::SubWeldment {
        name: "Stiffeners".into(),
        bodies: vec![id(f2, "g0.1")],
    }];
    let list = cutlist::cut_list(&m.doc, m.state()).unwrap();
    type Row = (Option<u32>, String, u32, Option<String>, bool);
    let rows: Vec<Row> = list
        .items
        .iter()
        .map(|i| {
            (
                i.number,
                prop(i, "DESCRIPTION").into(),
                i.quantity,
                i.folder.clone(),
                i.excluded,
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (Some(1), "Base frame side".into(), 2, None, false),
            (Some(2), "SHS 40x40x3".into(), 1, None, false),
            (Some(3), "SHS 40x40x3".into(), 1, None, false),
            (None, "RHS 60x40x3".into(), 1, None, true),
            (Some(4), "Sheet 3 mm".into(), 1, None, false),
            (
                Some(5),
                "RHS 60x40x3".into(),
                1,
                Some("Stiffeners".into()),
                false
            ),
        ]
    );
}

#[test]
fn weld_folder_totals_beads() {
    use of_model::geom::DVec3;
    let mut m = post_and_beam();
    let (sides, post) = seam_faces(&m);
    m.doc.add(FeatureKind::WeldBead(bead(BeadPath::Faces {
        from: sides,
        to: vec![post.clone()],
    })));
    m.rebuild();
    let top = face(&m, DVec3::Y, |c| c.x > 100.0 && (c.y - 270.0).abs() < 1e-6);
    let mut fillet = bead(BeadPath::Faces {
        from: vec![top],
        to: vec![post],
    });
    fillet.kind = BeadKind::Fillet;
    m.doc.add(FeatureKind::WeldBead(fillet));
    m.rebuild();
    ok(&m);
    let list = cutlist::cut_list(&m.doc, m.state()).unwrap();
    // The modeled bead belongs to the weld folder, not the cut list.
    assert_eq!(list.items.iter().map(|i| i.quantity).sum::<u32>(), 2);
    close(list.welds.total_length, 4.0 * 28.0 + 28.0, 1e-6);
    // 0.15 kg/m over 112 mm, plus 350 mm³ at the default 1000 kg/m³.
    close(list.welds.total_mass, 0.15 * 0.112 + 350.0 * 1e-6, 1e-9);
    assert_eq!(list.welds.beads[0].symbol.as_deref(), Some("5 ⊿"));
}
