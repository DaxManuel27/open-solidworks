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
