//! The flat pattern in 2D and the sheet metal cut list properties.

mod sheet_common;

use std::f64::consts::{FRAC_PI_2, PI};

use of_model::flat::{FlatCurve, sheet_properties};
use of_model::sheet_metal::*;
use of_model::{
    geom::{DVec2, DVec3},
    sketch::{Sketch, tools},
    *,
};
use sheet_common::*;

fn l_bracket(t: f64, r: f64, k: f64) -> Model {
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
    let m = Model::new(doc);
    ok(&m);
    m
}

#[test]
fn l_bracket_flat_size_and_bend_line() {
    let (t, r, k) = (2.0, 2.0, 0.44);
    let mut m = l_bracket(t, r, k);
    let length = (30.0 - r - t) + (50.0 - r - t) + FRAC_PI_2 * (r + k * t);
    let check = |m: &Model| {
        let (flat, p) = sheet_properties(m.state(), 0).unwrap();
        close(p.flat_length, length, 1e-6);
        close(p.flat_width, 20.0, 1e-6);
        close(p.thickness, t, 1e-12);
        close(p.flat_area, length * 20.0, 1e-6);
        close(p.outer_cut, 2.0 * (length + 20.0), 1e-6);
        assert_eq!((p.bends, p.cutouts, p.inner_cut), (1, 0, 0.0));
        assert_eq!(p.body, Some(m.state().bodies[0].id.clone()));
        assert_eq!(flat.loops.len(), 1);
        assert!(flat.loops[0].outer);
        assert_eq!(flat.bend_lines.len(), 1);
        let l = &flat.bend_lines[0];
        close((DVec2::from(l.a) - DVec2::from(l.b)).length(), 20.0, 1e-6);
        close(l.angle, FRAC_PI_2, 1e-9);
        // The bend line lies across the middle of the bend region.
        let (lo, hi) = flat.extents().unwrap();
        let along = if hi[0] - lo[0] > hi[1] - lo[1] { 0 } else { 1 };
        let at = l.a[along] - lo[along];
        let near = (30.0 - r - t) + FRAC_PI_2 * (r + k * t) / 2.0;
        let far = (50.0 - r - t) + FRAC_PI_2 * (r + k * t) / 2.0;
        assert!(
            (at - near).abs() < 1e-6 || (at - far).abs() < 1e-6,
            "bend line at {at}"
        );
    };
    check(&m);
    // The same while the Flat-Pattern feature has flattened the body.
    flatten(&mut m);
    ok(&m);
    check(&m);
}

#[test]
fn plate_with_holes_counts_cutouts_and_rotated_box() {
    let t = 1.5;
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(80.0, 30.0));
    sk.add_circle_at(DVec2::new(15.0, 15.0), 4.0);
    sk.add_circle_at(DVec2::new(65.0, 15.0), 5.0);
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, 1.0, 0.5),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let m = Model::new(doc);
    ok(&m);
    let (flat, p) = sheet_properties(m.state(), 0).unwrap();
    assert_eq!(p.cutouts, 2);
    close(p.inner_cut, 2.0 * PI * 9.0, 1e-6);
    close(p.outer_cut, 220.0, 1e-6);
    close(p.flat_length, 80.0, 1e-6);
    close(p.flat_width, 30.0, 1e-6);
    close(p.flat_area, 80.0 * 30.0 - PI * (16.0 + 25.0), 1e-4);
    let circles = flat
        .loops
        .iter()
        .filter(|l| !l.outer)
        .flat_map(|l| &l.curves)
        .filter(|c| matches!(c, FlatCurve::Arc { start, end, .. } if (end - start - 2.0 * PI).abs() < 1e-9))
        .count();
    assert_eq!(circles, 2);
}

#[test]
fn tray_flat_box_and_bend_directions() {
    let t = 1.0;
    let mut doc = Document::new();
    let s = rectangle(&mut doc, 60.0, 40.0);
    doc.add(FeatureKind::BaseFlange(BaseFlange {
        sketch: s,
        sheet: params(t, 1.0, 0.5),
        side: ThicknessSide::Normal,
        end1: None,
        end2: None,
        reverse: false,
    }));
    let mut m = Model::new(doc);
    let edges = vec![
        edge_at(&m, DVec3::new(30.0, 40.0, t)),
        edge_at(&m, DVec3::new(60.0, 20.0, t)),
    ];
    m.doc.add(FeatureKind::EdgeFlange(flange(edges, 15.0)));
    m.rebuild();
    ok(&m);
    let (flat, p) = sheet_properties(m.state(), 0).unwrap();
    assert_eq!(p.bends, 2);
    assert_eq!(flat.bend_lines.len(), 2);
    // Both flanges bend the same way from the base.
    assert_eq!(flat.bend_lines[0].up, flat.bend_lines[1].up);
    // Flange 15 to the outer virtual sharp: 15 - (r + t) + allowance.
    let leg = 15.0 - 2.0 + FRAC_PI_2 * (1.0 + 0.5);
    close(p.flat_length, 60.0 - 2.0 + leg, 1e-3);
    close(p.flat_width, 40.0 - 2.0 + leg, 1e-3);
}

#[test]
fn custom_properties_link_to_the_flat_size() {
    let m = l_bracket(2.0, 2.0, 0.44);
    let length = (30.0 - 4.0) + (50.0 - 4.0) + FRAC_PI_2 * (2.0 + 0.44 * 2.0);
    let text = of_model::properties::evaluate(
        &m.doc,
        m.state(),
        "BLANK {flat_length} x {flat_width} x {thickness}, {bends} bend",
    );
    let rounded = (length * 1e3).round() / 1e3;
    assert_eq!(text, format!("BLANK {rounded} x 20 x 2, 1 bend"));
    // Parts without sheet metal leave the link as written.
    let plain = Model::new(Document::new());
    assert_eq!(
        of_model::properties::evaluate(&plain.doc, plain.state(), "{flat_length}"),
        "{flat_length}"
    );
}
