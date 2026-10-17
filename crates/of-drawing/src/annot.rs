//! Drawing annotations: every annotation kind turned into display
//! primitives from the model geometry it names, plus the operations that
//! make annotations from the model (model items, hole callouts, auto
//! balloons) and drive the model from them (editing imported dimensions).

use std::collections::BTreeMap;

use of_assembly::CompRef;
use of_model::geom::{DVec2, DVec3, Plane};
use of_model::{FeatureKind, HoleEnd, HoleKind};

use crate::annotations::{Annotation, AnnotationKind, BalloonText, MarkPattern};
use crate::dims::{
    self, DimKind, Dimension, Measure, Orient, Painter, Pick, p, point_of, value_text,
};
use crate::document::*;
use crate::drawing::{Drawing, Resolved};
use crate::graphics::{Graphics, HAlign, LineStyle, Owner, P2, Pen, Prim, layers};
use crate::locate::{Geo, Located};
use crate::standard::StandardBase;
use crate::symbols::Ink;
use crate::tables::{self, TableKind};
use crate::text;

fn d2(p: P2) -> DVec2 {
    DVec2::from(p)
}

/// A model dimension's geometry, worked out from the part.
fn model_measure(part: &of_assembly::cache::Part, key: &str) -> Result<Measure, String> {
    let doc = part.doc();
    let state = part.state();
    let rest = key
        .strip_prefix('f')
        .ok_or_else(|| format!("{key} is not a model dimension"))?;
    let (fid, path) = rest
        .split_once('.')
        .ok_or_else(|| format!("{key} is not a model dimension"))?;
    let fid: u32 = fid
        .parse()
        .map_err(|_| format!("{key} is not a model dimension"))?;
    let feature = doc
        .feature(fid)
        .ok_or_else(|| format!("feature {fid} no longer exists"))?;
    if let Some(cid) = path.strip_prefix("dimension.") {
        let cid: u32 = cid
            .parse()
            .map_err(|_| format!("{key} is not a sketch dimension"))?;
        let (plane, sk) = state
            .sketches
            .get(&fid)
            .ok_or_else(|| format!("sketch {fid} has no geometry"))?;
        let c = sk
            .constraints
            .get(&cid)
            .ok_or_else(|| format!("dimension {key} no longer exists"))?;
        return sketch_measure(plane, sk, &c.kind);
    }
    match (&feature.kind, path) {
        (FeatureKind::Extrude(e), "end1.depth" | "end2.depth") => {
            let (plane, sk) = state
                .sketches
                .get(&e.sketch)
                .ok_or("the extrusion's sketch has no geometry")?;
            let depth = match (path, &e.end1, &e.end2) {
                ("end1.depth", of_model::EndCondition::Blind { depth }, _) => *depth,
                ("end2.depth", _, Some(of_model::EndCondition::Blind { depth })) => *depth,
                _ => return Err(format!("{key} is not a blind depth")),
            };
            let mut n = plane.normal;
            if e.reverse {
                n = -n;
            }
            if path == "end2.depth" {
                n = -n;
            }
            // A profile corner: the first point of the sketch.
            let pt = sk
                .entities
                .iter()
                .find_map(|(id, ent)| {
                    matches!(ent.geo, of_model::sketch::Geometry::Point { .. })
                        .then(|| sk.point(*id).ok())
                        .flatten()
                        .filter(|_| *id != of_model::sketch::ORIGIN)
                })
                .unwrap_or(DVec2::ZERO);
            let a = plane.to_world(pt) + plane.normal * e.start_offset;
            Ok(Measure::Linear {
                a,
                b: a + n * depth,
                dir: n,
            })
        }
        _ => Err(format!("{key} cannot be shown in a drawing yet")),
    }
}

/// Normal of the sketch a sketch dimension belongs to.
fn sketch_normal(part: &of_assembly::cache::Part, key: &str) -> Option<DVec3> {
    let (fid, path) = key.strip_prefix('f')?.split_once('.')?;
    path.strip_prefix("dimension.")?;
    let fid: u32 = fid.parse().ok()?;
    part.state()
        .sketches
        .get(&fid)
        .map(|(plane, _)| plane.normal)
}

fn sketch_measure(
    plane: &Plane,
    sk: &of_model::sketch::Sketch,
    kind: &of_model::sketch::ConstraintKind,
) -> Result<Measure, String> {
    use of_model::sketch::{ConstraintKind as C, Geometry as G};
    let w = |v: DVec2| plane.to_world(v);
    let geo = |id| {
        sk.entity(id)
            .map(|e| e.geo.clone())
            .map_err(|e| e.to_string())
    };
    let locus = |id| -> Result<DVec2, String> {
        match geo(id)? {
            G::Circle { center, .. } | G::Arc { center, .. } => {
                sk.point(center).map_err(|e| e.to_string())
            }
            _ => sk.point(id).map_err(|e| e.to_string()),
        }
    };
    let line = |id| -> Result<(DVec2, DVec2), String> {
        match geo(id)? {
            G::Line { a, b } => Ok((
                sk.point(a).map_err(|e| e.to_string())?,
                sk.point(b).map_err(|e| e.to_string())?,
            )),
            _ => Err("not a line".into()),
        }
    };
    let radius = |id| -> Result<(DVec2, f64), String> {
        match geo(id)? {
            G::Circle { center, radius } => {
                Ok((sk.point(center).map_err(|e| e.to_string())?, radius))
            }
            G::Arc { center, start, .. } => {
                let c = sk.point(center).map_err(|e| e.to_string())?;
                let s = sk.point(start).map_err(|e| e.to_string())?;
                Ok((c, c.distance(s)))
            }
            _ => Err("not a circle or arc".into()),
        }
    };
    Ok(match kind {
        C::Distance { a, b, .. } => {
            match (line(*a), line(*b)) {
                (Ok((a0, a1)), Ok((b0, _b1))) => {
                    // Parallel lines: perpendicular distance.
                    let d = (a1 - a0).normalize_or(DVec2::X);
                    let n = d.perp();
                    let pa = (a0 + a1) / 2.0;
                    let pb = pa + n * (b0 - pa).dot(n);
                    Measure::Linear {
                        a: w(pa),
                        b: w(pb),
                        dir: plane.x_dir * n.x + plane.y_dir() * n.y,
                    }
                }
                (Ok((l0, l1)), Err(_)) | (Err(_), Ok((l0, l1))) => {
                    let q = if line(*a).is_ok() {
                        locus(*b)?
                    } else {
                        locus(*a)?
                    };
                    let d = (l1 - l0).normalize_or(DVec2::X);
                    let foot = l0 + d * (q - l0).dot(d);
                    let n = (q - foot).normalize_or(d.perp());
                    Measure::Linear {
                        a: w(foot),
                        b: w(q),
                        dir: plane.x_dir * n.x + plane.y_dir() * n.y,
                    }
                }
                _ => {
                    let (pa, pb) = (locus(*a)?, locus(*b)?);
                    let n = (pb - pa).normalize_or(DVec2::X);
                    Measure::Linear {
                        a: w(pa),
                        b: w(pb),
                        dir: plane.x_dir * n.x + plane.y_dir() * n.y,
                    }
                }
            }
        }
        C::HorizontalDistance { a, b, .. } => Measure::Linear {
            a: w(locus(*a)?),
            b: w(locus(*b)?),
            dir: plane.x_dir,
        },
        C::VerticalDistance { a, b, .. } => Measure::Linear {
            a: w(locus(*a)?),
            b: w(locus(*b)?),
            dir: plane.y_dir(),
        },
        C::Length { line: l, .. } => {
            let (a, b) = line(*l)?;
            let n = (b - a).normalize_or(DVec2::X);
            Measure::Linear {
                a: w(a),
                b: w(b),
                dir: plane.x_dir * n.x + plane.y_dir() * n.y,
            }
        }
        C::Radius { curve, .. } | C::Diameter { curve, .. } => {
            let (c, r) = radius(*curve)?;
            Measure::Radial {
                center: w(c),
                radius: r,
                axis: plane.normal,
                diameter: matches!(kind, C::Diameter { .. }),
            }
        }
        C::Angle { a, b, .. } => {
            let (a0, a1) = line(*a)?;
            let (b0, b1) = line(*b)?;
            let (da, db) = (a1 - a0, b1 - b0);
            let den = da.perp_dot(db);
            let vertex = if den.abs() > 1e-12 {
                a0 + da * ((b0 - a0).perp_dot(db) / den)
            } else {
                a0
            };
            let to3 =
                |d: DVec2| (plane.x_dir * d.x + plane.y_dir() * d.y).normalize_or(plane.x_dir);
            Measure::Angle {
                vertex: w(vertex),
                d1: to3(da),
                d2: to3(db),
            }
        }
        _ => return Err("this relation has no value".into()),
    })
}

/// The model point a pick names: its `point`, or with `near` set and
/// `Auto`, the end, middle or center nearest that view-plane point.
fn pick_point(l: &Located, p: &Pick) -> DVec3 {
    let Some(near) = p.near.filter(|_| p.point == dims::Which::Auto) else {
        return point_of(&l.geo, p.point);
    };
    let near = DVec2::from(near);
    let candidates: Vec<DVec3> = match l.geo {
        Geo::Point(q) => vec![q],
        Geo::Line(a, b) => vec![a, b, (a + b) / 2.0],
        Geo::Circle { center, ends, .. } => {
            let mut v = vec![center];
            if let Some((a, b)) = ends {
                v.extend([a, b]);
            }
            v
        }
        Geo::Plane { point, .. } => vec![point],
        Geo::Cylinder { a, b, .. } => vec![a, b, (a + b) / 2.0],
    };
    candidates
        .into_iter()
        .min_by(|x, y| {
            l.local(*x)
                .distance(near)
                .total_cmp(&l.local(*y).distance(near))
        })
        .unwrap_or_else(|| l.geo.anchor())
}

/// The `Located` view geometry needed only for its view mapping.
fn view_only(view: Resolved) -> Located {
    Located {
        view,
        geo: Geo::Point(DVec3::ZERO),
    }
}

impl Drawing {
    /// Sheet position of an annotation coordinate.
    fn ann_pos(&mut self, sheet: &Sheet, a: &Annotation, at: P2) -> Result<P2, String> {
        match a.attach {
            None => Ok(at),
            Some(v) => {
                let r = self.resolve(sheet, v)?;
                Ok([r.origin.x + at[0], r.origin.y + at[1]])
            }
        }
    }

    /// Draw one annotation. An error means its model anchor is gone; what
    /// can still be drawn is.
    pub(crate) fn draw_annotation(
        &mut self,
        sheet: &Sheet,
        a: &Annotation,
        g: &mut Graphics,
    ) -> Result<(), String> {
        let std = self.doc.standard;
        let owner = Owner::Annotation { id: a.id };
        let default_layer = match &a.kind {
            AnnotationKind::Dimension(_) => layers::DIMENSIONS,
            AnnotationKind::CenterMark { .. } | AnnotationKind::Centerline { .. } => layers::CENTER,
            AnnotationKind::Table(_) => layers::TABLES,
            AnnotationKind::AreaHatch { .. } => layers::HATCH,
            _ => layers::NOTES,
        };
        let layer = a.layer.clone().unwrap_or_else(|| default_layer.to_string());
        match &a.kind {
            AnnotationKind::Note {
                at,
                text: t,
                height,
                leader,
                jog,
                border,
                angle,
            } => {
                let at = self.ann_pos(sheet, a, *at)?;
                let fields = self.fields(sheet);
                let value = crate::formats::fill(t, &fields);
                let h = height.unwrap_or(std.text_height());
                let mut ink = Ink {
                    g,
                    std,
                    owner: owner.clone(),
                    layer,
                };
                for p in text::layout(&value, at, h, *angle, HAlign::Left) {
                    ink.push(p);
                }
                let (w, bh) = text::block_size(&value, h);
                if *angle == 0.0 {
                    ink.border(*border, [at[0], at[1] + h - bh], [at[0] + w, at[1] + h]);
                }
                if let Some(l) = leader {
                    let target = self
                        .anchor_point(sheet, l)
                        .ok_or("the leader's view is gone")?;
                    let mut ink = Ink {
                        g,
                        std,
                        owner,
                        layer: layers::NOTES.into(),
                    };
                    let left = target[0] < at[0] + w / 2.0;
                    let start = if left {
                        [at[0] - 1.0, at[1] + h / 2.0]
                    } else {
                        [at[0] + w + 1.0, at[1] + h / 2.0]
                    };
                    if *jog {
                        let shoulder = [start[0] + if left { -3.0 } else { 3.0 }, start[1]];
                        ink.line(start, shoulder);
                        ink.leader(shoulder, target, false);
                    } else {
                        ink.leader(start, target, false);
                    }
                }
                Ok(())
            }
            AnnotationKind::Dimension(d) => self.draw_dimension(sheet, a, d, g, owner, layer),
            AnnotationKind::Balloon {
                at,
                leader,
                content,
                lower,
                shape,
                size,
                stacked,
                table,
            } => {
                let at = self.ann_pos(sheet, a, *at)?;
                let items = self.bom_items(sheet, *table);
                // Weldment balloons number the leader's part's cut list.
                let weldment = std::iter::once(content)
                    .chain(lower)
                    .chain(stacked)
                    .any(|c| {
                        matches!(
                            c,
                            BalloonText::CutListItem { .. } | BalloonText::CutListQuantity { .. }
                        )
                    });
                let cut = match (weldment, leader) {
                    (true, crate::document::Anchor::Model { target, .. }) => {
                        self.view_cut_list(sheet, target.view).ok()
                    }
                    _ => None,
                };
                let resolve = |c: &BalloonText| balloon_text(c, &items, cut.as_ref());
                let size = size.unwrap_or(std.text_height() * 3.0);
                let upper = resolve(content);
                let low = match (shape, lower) {
                    (_, Some(l)) => Some(resolve(l)),
                    (crate::annotations::BalloonShape::SplitCircle, None) => match content {
                        BalloonText::Item { component } => Some(resolve(&BalloonText::Quantity {
                            component: component.clone(),
                        })),
                        BalloonText::CutListItem { body } => {
                            Some(resolve(&BalloonText::CutListQuantity {
                                body: body.clone(),
                            }))
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let target = self.anchor_point(sheet, leader);
                let mut ink = Ink {
                    g,
                    std,
                    owner,
                    layer,
                };
                let r = ink.balloon(*shape, at, size, &upper, low.as_deref());
                for (i, s) in stacked.iter().enumerate() {
                    let c = [at[0] + size * (i + 1) as f64, at[1]];
                    ink.balloon(*shape, c, size, &resolve(s), None);
                }
                let target = target.ok_or("the balloon's view is gone")?;
                let d = (d2(target) - d2(at)).normalize_or(DVec2::X);
                ink.leader(p(d2(at) + d * r), target, false);
                Ok(())
            }
            AnnotationKind::HoleCallout { at, target, text } => {
                let at = self.ann_pos(sheet, a, *at)?;
                let loc = self.locate(sheet, target)?;
                let t = match text {
                    Some(t) => t.clone(),
                    None => self.hole_text(sheet, target, &loc)?,
                };
                let to = loc.sheet(
                    loc.geo.nearest(
                        loc.geo.anchor()
                            + loc
                                .view
                                .frame
                                .unproject(d2(at) - d2(loc.sheet(loc.geo.anchor()))),
                    ),
                );
                let h = std.text_height();
                let mut ink = Ink {
                    g,
                    std,
                    owner,
                    layer,
                };
                let w = t
                    .lines()
                    .map(|l| text::line_width(l, h))
                    .fold(0.0, f64::max);
                let left = to[0] > at[0] + w / 2.0;
                let start = if left {
                    [at[0] + w + 1.0, at[1] + h / 2.0]
                } else {
                    [at[0] - 1.0, at[1] + h / 2.0]
                };
                ink.leader(start, to, false);
                for prim in text::layout(&t, at, h, 0.0, HAlign::Left) {
                    ink.push(prim);
                }
                Ok(())
            }
            AnnotationKind::CenterMark {
                targets,
                pattern,
                size,
                extended,
            } => {
                let mut centers = Vec::new();
                let mut first_err = None;
                for t in targets {
                    match self.locate(sheet, t) {
                        Ok(l) => {
                            if let Geo::Circle { center, radius, .. } = l.geo {
                                centers.push((d2(l.sheet(center)), radius * l.view.scale));
                            }
                        }
                        Err(e) => first_err = Some(e),
                    }
                }
                let s = size.unwrap_or(2.5);
                let pen = Pen::new(LineStyle::Center, std.thin_line());
                let thin = Pen::new(LineStyle::Continuous, std.thin_line());
                for (c, r) in &centers {
                    let reach = if *extended { r + 2.0 } else { s };
                    for d in [DVec2::X, DVec2::Y] {
                        if *extended {
                            g.line(p(*c - d * reach), p(*c + d * reach), pen, &layer, &owner);
                        } else {
                            g.line(p(*c - d * s), p(*c + d * s), thin, &layer, &owner);
                        }
                    }
                }
                match pattern {
                    MarkPattern::Single => {}
                    MarkPattern::Linear if centers.len() >= 2 => {
                        let (a, b) = (centers[0].0, centers[centers.len() - 1].0);
                        let d = (b - a).normalize_or(DVec2::X);
                        g.line(
                            p(a - d * (centers[0].1 + 2.0)),
                            p(b + d * (centers[centers.len() - 1].1 + 2.0)),
                            pen,
                            &layer,
                            &owner,
                        );
                    }
                    MarkPattern::Circular if centers.len() >= 3 => {
                        if let Some(o) = circumcenter(centers[0].0, centers[1].0, centers[2].0) {
                            let r = o.distance(centers[0].0);
                            g.push(
                                Prim::Arc {
                                    center: p(o),
                                    radius: r,
                                    start: 0.0,
                                    end: std::f64::consts::TAU,
                                },
                                pen,
                                &layer,
                                &owner,
                            );
                            for (c, cr) in &centers {
                                let d = (*c - o).normalize_or(DVec2::X);
                                g.line(
                                    p(*c - d * (cr + 2.0)),
                                    p(*c + d * (cr + 2.0)),
                                    pen,
                                    &layer,
                                    &owner,
                                );
                            }
                        }
                    }
                    _ => {}
                }
                first_err.map_or(Ok(()), Err)
            }
            AnnotationKind::Centerline {
                a: ra,
                b: rb,
                extend,
            } => {
                let la = self.locate(sheet, ra)?;
                let (s, e) = match (&la.geo, rb) {
                    (Geo::Cylinder { a, b, .. }, None) => (d2(la.sheet(*a)), d2(la.sheet(*b))),
                    (Geo::Line(a0, a1), Some(rb)) => {
                        let lb = self.locate(sheet, rb)?;
                        let Geo::Line(b0, b1) = lb.geo else {
                            return Err("a centerline needs two straight edges".into());
                        };
                        let (a0, a1) = (d2(la.sheet(*a0)), d2(la.sheet(*a1)));
                        let (mut b0, mut b1) = (d2(lb.sheet(b0)), d2(lb.sheet(b1)));
                        if (a1 - a0).dot(b1 - b0) < 0.0 {
                            std::mem::swap(&mut b0, &mut b1);
                        }
                        ((a0 + b0) / 2.0, (a1 + b1) / 2.0)
                    }
                    _ => {
                        return Err(
                            "a centerline needs two straight edges or a cylindrical face".into(),
                        );
                    }
                };
                let d = (e - s).normalize_or(DVec2::X);
                let pen = Pen::new(LineStyle::Center, std.thin_line());
                g.line(p(s - d * *extend), p(e + d * *extend), pen, &layer, &owner);
                Ok(())
            }
            AnnotationKind::SurfaceFinish {
                at,
                leader,
                process,
                roughness,
                method,
                lay,
                all_around,
                angle,
            } => {
                let at = self.ann_pos(sheet, a, *at)?;
                let target = match leader {
                    Some(l) => Some(
                        self.anchor_point(sheet, l)
                            .ok_or("the leader's view is gone")?,
                    ),
                    None => None,
                };
                let mut ink = Ink {
                    g,
                    std,
                    owner,
                    layer,
                };
                ink.surface_finish(
                    at,
                    &crate::symbols::Finish {
                        process: *process,
                        roughness,
                        method,
                        lay,
                        all_around: *all_around,
                    },
                    *angle,
                );
                if let Some(t) = target {
                    ink.leader(at, t, false);
                }
                Ok(())
            }
            AnnotationKind::DatumFeature { at, label, target } => {
                let at = self.ann_pos(sheet, a, *at)?;
                let on = self
                    .anchor_point(sheet, target)
                    .ok_or("the datum's view is gone")?;
                Ink {
                    g,
                    std,
                    owner,
                    layer,
                }
                .datum_feature(label, at, on);
                Ok(())
            }
            AnnotationKind::DatumTarget {
                at,
                label,
                size,
                target,
                kind,
            } => {
                let at = self.ann_pos(sheet, a, *at)?;
                let on = self
                    .anchor_point(sheet, target)
                    .ok_or("the target's view is gone")?;
                Ink {
                    g,
                    std,
                    owner,
                    layer,
                }
                .datum_target(label, size, at, on, *kind);
                Ok(())
            }
            AnnotationKind::Gtol {
                at,
                leader,
                rows,
                above,
            } => {
                let at = self.ann_pos(sheet, a, *at)?;
                let target = match leader {
                    Some(l) => Some(
                        self.anchor_point(sheet, l)
                            .ok_or("the leader's view is gone")?,
                    ),
                    None => None,
                };
                let mut ink = Ink {
                    g,
                    std,
                    owner,
                    layer,
                };
                let b = ink.gtol(rows, at, above);
                if let Some(t) = target {
                    let mid_y = b[1][1] - (std.text_height() * 2.0) / 2.0;
                    let from = if t[0] < b[0][0] {
                        [b[0][0], mid_y]
                    } else {
                        [b[1][0], mid_y]
                    };
                    ink.leader(from, t, false);
                }
                Ok(())
            }
            AnnotationKind::Weld {
                at,
                leader,
                arrow,
                other,
                all_around,
                field,
                tail,
                bead,
            } => {
                let at = self.ann_pos(sheet, a, *at)?;
                let on = self
                    .anchor_point(sheet, leader)
                    .ok_or("the weld's view is gone")?;
                // A linked bead fills the arrow side and tail from the model.
                let (arrow, tail) = match (bead, leader) {
                    (Some(bead), crate::document::Anchor::Model { target, .. }) => {
                        let part = self.view_part(sheet, target.view)?;
                        let w = part
                            .state()
                            .welds
                            .iter()
                            .find(|w| w.feature == *bead)
                            .ok_or("the weld symbol's bead is gone")?;
                        let side = crate::symbols::WeldSide {
                            symbol: arrow
                                .as_ref()
                                .map_or(crate::symbols::WeldSymbol::Fillet, |s| s.symbol),
                            size: std.number(w.size, std.precision()),
                            length: arrow.as_ref().map(|s| s.length.clone()).unwrap_or_default(),
                            contour: arrow.as_ref().map(|s| s.contour).unwrap_or_default(),
                        };
                        let tail = w.process.clone().unwrap_or_else(|| tail.clone());
                        (Some(side), tail)
                    }
                    _ => (arrow.clone(), tail.clone()),
                };
                let (arrow, tail) = (&arrow, &tail);
                Ink {
                    g,
                    std,
                    owner,
                    layer,
                }
                .weld(
                    at,
                    on,
                    arrow.as_ref(),
                    other.as_ref(),
                    *all_around,
                    *field,
                    tail,
                );
                Ok(())
            }
            AnnotationKind::AreaHatch {
                boundary,
                pattern,
                scale,
                angle,
            } => {
                let pts: Vec<P2> = boundary
                    .iter()
                    .map(|q| self.ann_pos(sheet, a, *q))
                    .collect::<Result<_, _>>()?;
                let thin = Pen::new(LineStyle::Continuous, std.thin_line());
                if *pattern == crate::hatch::Pattern::Solid {
                    let mut pen = thin;
                    pen.color = [80, 80, 80];
                    g.push(Prim::Fill { loops: vec![pts] }, pen, &layer, &owner);
                } else {
                    for [x, y] in crate::hatch::lines(&[pts], *pattern, *scale, *angle) {
                        g.line(x, y, thin, &layer, &owner);
                    }
                }
                Ok(())
            }
            AnnotationKind::RevisionCloud {
                shape,
                arc,
                revision,
            } => {
                let shape = match shape {
                    crate::annotations::CloudShape::Rectangle { a: c0, b: c1 } => {
                        crate::annotations::CloudShape::Rectangle {
                            a: self.ann_pos(sheet, a, *c0)?,
                            b: self.ann_pos(sheet, a, *c1)?,
                        }
                    }
                    crate::annotations::CloudShape::Ellipse { center, rx, ry } => {
                        crate::annotations::CloudShape::Ellipse {
                            center: self.ann_pos(sheet, a, *center)?,
                            rx: *rx,
                            ry: *ry,
                        }
                    }
                    crate::annotations::CloudShape::Polygon { points } => {
                        crate::annotations::CloudShape::Polygon {
                            points: points
                                .iter()
                                .map(|q| self.ann_pos(sheet, a, *q))
                                .collect::<Result<_, _>>()?,
                        }
                    }
                    crate::annotations::CloudShape::Freehand { points } => {
                        crate::annotations::CloudShape::Freehand {
                            points: points
                                .iter()
                                .map(|q| self.ann_pos(sheet, a, *q))
                                .collect::<Result<_, _>>()?,
                        }
                    }
                };
                let mut ink = Ink {
                    g,
                    std,
                    owner,
                    layer,
                };
                let corner = ink.cloud(&shape, *arc);
                if let Some(r) = revision {
                    ink.revision([corner[0] + 4.0, corner[1] + 4.0], r);
                }
                Ok(())
            }
            AnnotationKind::RevisionSymbol {
                at,
                revision,
                leader,
            } => {
                let at = self.ann_pos(sheet, a, *at)?;
                let target = match leader {
                    Some(l) => Some(
                        self.anchor_point(sheet, l)
                            .ok_or("the leader's view is gone")?,
                    ),
                    None => None,
                };
                let mut ink = Ink {
                    g,
                    std,
                    owner,
                    layer,
                };
                ink.revision(at, revision);
                if let Some(t) = target {
                    ink.leader(at, t, false);
                }
                Ok(())
            }
            AnnotationKind::Table(t) => {
                // Pieces on this sheet are drawn from the table's own sheet.
                let pieces = self.table_pieces(sheet, a, t)?;
                let h = t.text_height.unwrap_or(std.text_height() * 0.75);
                for (range, at, anchor, cells) in pieces {
                    tables::draw(g, &cells, range, at, anchor, h, &std, &owner);
                }
                if let TableKind::Bend {
                    view,
                    show_tags: true,
                } = &t.kind
                    && let Ok((r, flat)) = self.flat_items(sheet, *view)
                {
                    let h = std.text_height() * 0.8;
                    let thin = Pen::new(LineStyle::Continuous, std.thin_line());
                    for (i, b) in flat.bends.iter().enumerate() {
                        // A fifth along the line, clear of the bend note at its middle.
                        let at = r.to_sheet(b.a + (b.b - b.a) * 0.2);
                        for prim in text::layout(
                            &(i + 1).to_string(),
                            [at[0] + 1.0, at[1] + 1.0],
                            h,
                            0.0,
                            HAlign::Left,
                        ) {
                            g.push(prim, thin, layers::TABLES, &owner);
                        }
                    }
                }
                if let TableKind::Hole {
                    holes,
                    tags,
                    combine: _,
                    show_tags: true,
                    ..
                } = &t.kind
                {
                    let tags_of = self.hole_tags(sheet, holes, *tags);
                    let h = std.text_height() * 0.8;
                    let thin = Pen::new(LineStyle::Continuous, std.thin_line());
                    for (r, tag) in holes.iter().zip(tags_of) {
                        if let Ok(l) = self.locate(sheet, r) {
                            let c = l.sheet(l.geo.anchor());
                            let rr = match l.geo {
                                Geo::Circle { radius, .. } => radius * l.view.scale,
                                _ => 0.0,
                            };
                            // Clear of a counterbore or countersink around the edge.
                            let off = rr * 1.2 + 1.5;
                            for prim in
                                text::layout(&tag, [c[0] + off, c[1] + off], h, 0.0, HAlign::Left)
                            {
                                g.push(prim, thin, layers::TABLES, &owner);
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }

    fn draw_dimension(
        &mut self,
        sheet: &Sheet,
        a: &Annotation,
        d: &Dimension,
        g: &mut Graphics,
        owner: Owner,
        layer: String,
    ) -> Result<(), String> {
        let std = self.doc.standard;
        let at = d2(self.ann_pos(sheet, a, d.at)?);
        let mut pt = Painter {
            g,
            std,
            owner,
            layer,
        };
        let with_prefix = |sym: &str| {
            let mut c = d.clone();
            if c.prefix.is_empty() {
                c.prefix = sym.to_string();
            }
            c
        };
        match &d.kind {
            DimKind::Linear {
                a: pa,
                b: pb,
                orientation,
            } => {
                let la = self.locate(sheet, &pa.target)?;
                let (sa, sb, pa3, pb3, fixed) = match pb {
                    None => match la.geo {
                        Geo::Line(x, y) => (d2(la.sheet(x)), d2(la.sheet(y)), x, y, None),
                        Geo::Circle { .. } | Geo::Cylinder { .. }
                            if matches!(la.geo, Geo::Cylinder { .. })
                                || matches!(la.geo, Geo::Circle { axis, .. } if axis_side_on(&la, axis)) =>
                        {
                            let (center, radius, axis) = match la.geo {
                                Geo::Cylinder { a, b, radius } => {
                                    ((a + b) / 2.0, radius, (b - a).normalize_or(DVec3::Z))
                                }
                                Geo::Circle {
                                    center,
                                    radius,
                                    axis,
                                    ..
                                } => (center, radius, axis),
                                _ => unreachable!(),
                            };
                            let across = la
                                .view
                                .frame
                                .toward
                                .cross(axis)
                                .normalize_or(la.view.frame.x);
                            let (x, y) = (center - across * radius, center + across * radius);
                            let dd = with_prefix("⌀");
                            let t = value_text(&dd, radius * 2.0, &std, false, radius * 2.0);
                            let u = d2(la.sheet(y)) - d2(la.sheet(x));
                            dims::draw_linear(
                                &mut pt,
                                d2(la.sheet(x)),
                                d2(la.sheet(y)),
                                u,
                                at,
                                &t,
                                d.arrows,
                            );
                            return Ok(());
                        }
                        _ => return Err("pick a straight edge, or two items".into()),
                    },
                    Some(pb) => {
                        let lb = self.locate(sheet, &pb.target)?;
                        match (la.geo, lb.geo, pa.point, pb.point) {
                            (
                                Geo::Line(a0, a1),
                                Geo::Line(b0, b1),
                                dims::Which::Auto,
                                dims::Which::Auto,
                            ) if parallel(&la, a1 - a0, b1 - b0) => {
                                let dir = (a1 - a0).normalize_or(DVec3::X);
                                let mid = (a0 + a1) / 2.0;
                                let off = (b0 - mid) - dir * (b0 - mid).dot(dir);
                                let foot = mid + off;
                                let n = off.normalize_or(la.view.frame.y);
                                (d2(la.sheet(mid)), d2(lb.sheet(foot)), mid, foot, Some(n))
                            }
                            (
                                Geo::Plane {
                                    point: p0,
                                    normal: n0,
                                },
                                Geo::Plane {
                                    point: p1,
                                    normal: n1,
                                },
                                _,
                                _,
                            ) if n0.cross(n1).length() < 1e-6 => {
                                let foot = p0 + n0 * (p1 - p0).dot(n0);
                                (d2(la.sheet(p0)), d2(lb.sheet(foot)), p0, foot, Some(n0))
                            }
                            _ => {
                                let x = pick_point(&la, pa);
                                let y = pick_point(&lb, pb);
                                (d2(la.sheet(x)), d2(lb.sheet(y)), x, y, None)
                            }
                        }
                    }
                };
                let frame = la.view.frame;
                let (u, dir) = match (fixed, orientation) {
                    (Some(n), _) => {
                        let u = d2(p(frame.project(n))).normalize_or(DVec2::Y);
                        (u, n)
                    }
                    (None, o) => {
                        let o = match o {
                            Orient::Auto => auto_orient(sa, sb, at),
                            o => *o,
                        };
                        match o {
                            Orient::Horizontal => (DVec2::X, frame.x),
                            Orient::Vertical => (DVec2::Y, frame.y),
                            _ => {
                                let u = (sb - sa).normalize_or(DVec2::X);
                                (u, frame.unproject(u).normalize())
                            }
                        }
                    }
                };
                let value = (pb3 - pa3).dot(dir).abs();
                let t = value_text(d, value, &std, false, value);
                dims::draw_linear(&mut pt, sa, sb, u, at, &t, d.arrows);
                Ok(())
            }
            DimKind::Radius { target } | DimKind::Diameter { target } => {
                let diameter = matches!(d.kind, DimKind::Diameter { .. });
                let l = self.locate(sheet, &target.target)?;
                let (center, radius, axis) = match l.geo {
                    Geo::Circle {
                        center,
                        radius,
                        axis,
                        ..
                    } => (center, radius, axis),
                    Geo::Cylinder { a, b, radius } => {
                        ((a + b) / 2.0, radius, (b - a).normalize_or(DVec3::Z))
                    }
                    _ => return Err("pick a circle, arc or cylindrical face".into()),
                };
                let sym = if diameter { "⌀" } else { "R" };
                let dd = with_prefix(sym);
                let value = if diameter { radius * 2.0 } else { radius };
                let t = value_text(&dd, value, &std, false, value);
                if axis_side_on(&l, axis) {
                    let across = l.view.frame.toward.cross(axis).normalize_or(l.view.frame.x);
                    let (x, y) = (center - across * radius, center + across * radius);
                    let (sx, sy) = (d2(l.sheet(x)), d2(l.sheet(y)));
                    dims::draw_linear(&mut pt, sx, sy, sy - sx, at, &t, d.arrows);
                } else {
                    dims::draw_radial(
                        &mut pt,
                        d2(l.sheet(center)),
                        radius * l.view.scale,
                        at,
                        diameter,
                        d.foreshortened,
                        &t,
                    );
                }
                Ok(())
            }
            DimKind::Angular { a: pa, b: pb } => {
                let la = self.locate(sheet, &pa.target)?;
                let lb = self.locate(sheet, &pb.target)?;
                let (Geo::Line(a0, a1), Geo::Line(b0, b1)) = (la.geo, lb.geo) else {
                    return Err("an angle needs two straight edges".into());
                };
                angle_dim(&mut pt, d, &la, (a0, a1), (b0, b1), at, &std)
            }
            DimKind::ArcLength { target } => {
                let l = self.locate(sheet, &target.target)?;
                let Geo::Circle {
                    center,
                    radius,
                    axis,
                    ends: Some((s, e)),
                } = l.geo
                else {
                    return Err("an arc length needs an arc".into());
                };
                // The arc runs counter-clockwise about its axis from s to e.
                let (u, v) = (s - center, e - center);
                let mut sweep = u.cross(v).dot(axis).atan2(u.dot(v));
                if sweep <= 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                let value = sweep * radius;
                // On paper it turns the other way when the axis faces away.
                let (s, e) = if axis.dot(l.view.frame.toward) >= 0.0 {
                    (s, e)
                } else {
                    (e, s)
                };
                let dd = with_prefix("⌒");
                let t = value_text(&dd, value, &std, false, value);
                dims::draw_arc_length(
                    &mut pt,
                    d2(l.sheet(center)),
                    radius * l.view.scale,
                    d2(l.sheet(s)),
                    d2(l.sheet(e)),
                    at,
                    &t,
                );
                Ok(())
            }
            DimKind::Chamfer { edge, adjacent } => {
                let le = self.locate(sheet, &edge.target)?;
                let la = self.locate(sheet, &adjacent.target)?;
                let (Geo::Line(e0, e1), Geo::Line(a0, a1)) = (le.geo, la.geo) else {
                    return Err(
                        "a chamfer dimension needs the chamfer edge and an edge next to it".into(),
                    );
                };
                let de = e1 - e0;
                let da = (a1 - a0).normalize_or(DVec3::X);
                let dist = de.dot(da).abs();
                let mut ang = de.angle_between(da).to_degrees();
                if ang > 90.0 {
                    ang = 180.0 - ang;
                }
                let t1 = value_text(d, dist, &std, false, dist);
                let t2 = std.number(ang, std.angle_precision());
                let t = dims::ValueText {
                    main: format!("{} × {t2}°", t1.main),
                    stack: None,
                    after: t1.after,
                    below: t1.below,
                    boxed: t1.boxed,
                };
                dims::draw_callout(&mut pt, d2(le.sheet((e0 + e1) / 2.0)), at, &t);
                Ok(())
            }
            DimKind::Ordinate {
                origin,
                points,
                vertical,
            } => {
                let lo = self.locate(sheet, &origin.target)?;
                let o = pick_point(&lo, origin);
                let frame = lo.view.frame;
                let dir = if *vertical { frame.y } else { frame.x };
                let mut all = vec![(lo.sheet(o), 0.0)];
                for q in points {
                    let l = self.locate(sheet, &q.target)?;
                    let pq = pick_point(&l, q);
                    all.push((l.sheet(pq), (pq - o).dot(dir)));
                }
                let gap = std.extension_gap();
                let h = std.text_height();
                for (s, v) in all {
                    let s = d2(s);
                    let t = value_text(d, v.abs(), &std, false, v.abs());
                    if *vertical {
                        // Leaders run horizontally to x = at.x.
                        let end = DVec2::new(at.x, s.y);
                        let dirx = (end.x - s.x).signum();
                        pt.line(p(s + DVec2::new(dirx * gap, 0.0)), p(end));
                        pt.value(
                            &t,
                            end + DVec2::new(
                                dirx * (text::line_width(&t.main, h) / 2.0 + 1.0),
                                -h / 2.0,
                            ),
                            DVec2::X,
                            0.0,
                        );
                    } else {
                        let end = DVec2::new(s.x, at.y);
                        let diry = (end.y - s.y).signum();
                        pt.line(p(s + DVec2::new(0.0, diry * gap)), p(end));
                        // Text along the leader (vertical), past its end.
                        let w = text::line_width(&t.main, h);
                        pt.value(
                            &t,
                            end + DVec2::new(h / 2.0, diry * (w / 2.0 + 1.0)),
                            DVec2::Y,
                            0.0,
                        );
                    }
                }
                Ok(())
            }
            DimKind::Baseline {
                base,
                points,
                orientation,
                spacing,
            } => {
                let lb = self.locate(sheet, &base.target)?;
                let b3 = pick_point(&lb, base);
                let sb = d2(lb.sheet(b3));
                for (i, q) in points.iter().enumerate() {
                    let l = self.locate(sheet, &q.target)?;
                    let q3 = pick_point(&l, q);
                    let sq = d2(l.sheet(q3));
                    let o = match orientation {
                        Orient::Auto => auto_orient(sb, sq, at),
                        o => *o,
                    };
                    let (u, dir) = orient_dirs(o, &l.view, sb, sq);
                    let n = u.perp();
                    let side = (at - sb).dot(n).signum();
                    let at_i = at + n * side * (*spacing * i as f64);
                    let mid = (sb + sq) / 2.0;
                    let at_i = at_i + u * (mid - at_i).dot(u);
                    let value = (q3 - b3).dot(dir).abs();
                    let t = value_text(d, value, &std, false, value);
                    dims::draw_linear(&mut pt, sb, sq, u, at_i, &t, d.arrows);
                }
                Ok(())
            }
            DimKind::Chain {
                points,
                orientation,
            } => {
                let mut located = Vec::new();
                for q in points {
                    let l = self.locate(sheet, &q.target)?;
                    let q3 = pick_point(&l, q);
                    located.push((d2(l.sheet(q3)), q3, l.view.clone()));
                }
                for w in located.windows(2) {
                    let (sa, a3, view) = &w[0];
                    let (sb, b3, _) = &w[1];
                    let o = match orientation {
                        Orient::Auto => auto_orient(located[0].0, located[located.len() - 1].0, at),
                        o => *o,
                    };
                    let (u, dir) = orient_dirs(o, view, *sa, *sb);
                    let mid = (*sa + *sb) / 2.0;
                    let at_i = at + u * (mid - at).dot(u);
                    let value = (*b3 - *a3).dot(dir).abs();
                    let t = value_text(d, value, &std, false, value);
                    dims::draw_linear(&mut pt, *sa, *sb, u, at_i, &t, d.arrows);
                }
                Ok(())
            }
            DimKind::AngularRunning { base, lines } => {
                let lb = self.locate(sheet, &base.target)?;
                let Geo::Line(b0, b1) = lb.geo else {
                    return Err("the base of running angles must be a straight edge".into());
                };
                for (i, q) in lines.iter().enumerate() {
                    let l = self.locate(sheet, &q.target)?;
                    let Geo::Line(c0, c1) = l.geo else {
                        return Err("running angles need straight edges".into());
                    };
                    let vtx = intersect2(
                        d2(lb.sheet(b0)),
                        d2(lb.sheet(b1)),
                        d2(l.sheet(c0)),
                        d2(l.sheet(c1)),
                    )
                    .unwrap_or(d2(lb.sheet(b0)));
                    let r = at.distance(vtx) + 6.0 * i as f64;
                    let at_i = vtx + (at - vtx).normalize_or(DVec2::X) * r;
                    angle_dim(&mut pt, d, &lb, (b0, b1), (c0, c1), at_i, &std)?;
                }
                Ok(())
            }
            DimKind::Model { view, key } => {
                let r = self.resolve(sheet, *view)?;
                let path = self.model_path(&r.model);
                if crate::model::is_assembly(&path) {
                    return Err("model dimensions come from part views".into());
                }
                let part =
                    self.sources
                        .part(&path, r.model.configuration, r.model.flat.is_some())?;
                let m = model_measure(&part, key)?;
                let anchor = measure_anchor(&m, &r);
                let at = DVec2::new(anchor[0] + d.at[0], anchor[1] + d.at[1]);
                let l = view_only(r);
                match m {
                    Measure::Linear { a, b, dir } => {
                        let (sa, sb) = (d2(l.sheet(a)), d2(l.sheet(b)));
                        let u = d2(p(l.view.frame.project(dir))).normalize_or(DVec2::X);
                        let t = value_text(d, m.value(), &std, false, m.value());
                        dims::draw_linear(&mut pt, sa, sb, u, at, &t, d.arrows);
                    }
                    Measure::Radial {
                        center,
                        radius,
                        axis,
                        diameter,
                    } => {
                        let dd = with_prefix(if diameter { "⌀" } else { "R" });
                        let t = value_text(&dd, m.value(), &std, false, m.value());
                        if axis_side_on(&l, axis) {
                            let across =
                                l.view.frame.toward.cross(axis).normalize_or(l.view.frame.x);
                            let (x, y) = (center - across * radius, center + across * radius);
                            let (sx, sy) = (d2(l.sheet(x)), d2(l.sheet(y)));
                            dims::draw_linear(&mut pt, sx, sy, sy - sx, at, &t, d.arrows);
                        } else {
                            dims::draw_radial(
                                &mut pt,
                                d2(l.sheet(center)),
                                radius * l.view.scale,
                                at,
                                diameter,
                                d.foreshortened,
                                &t,
                            );
                        }
                    }
                    Measure::Angle {
                        vertex,
                        d1,
                        d2: dd2,
                    } => {
                        angle_dim(
                            &mut pt,
                            d,
                            &l,
                            (vertex, vertex + d1 * 10.0),
                            (vertex, vertex + dd2 * 10.0),
                            at,
                            &std,
                        )?;
                    }
                    Measure::Arc { .. } => return Err("arc model dimensions are not shown".into()),
                }
                Ok(())
            }
        }
    }

    /// BOM item numbers on a sheet: for the given (or first) BOM table, each
    /// row's item and the component keys it counts.
    fn bom_items(&mut self, sheet: &Sheet, table: Option<u32>) -> Vec<BomItem> {
        let found = self
            .doc
            .sheets
            .iter()
            .flat_map(|s| s.annotations.iter())
            .find_map(|a| match &a.kind {
                AnnotationKind::Table(t) if table.is_none_or(|id| id == a.id) => match &t.kind {
                    TableKind::Bom { .. } => Some(t.clone()),
                    _ => None,
                },
                _ => None,
            });
        let Some(t) = found else { return Vec::new() };
        let TableKind::Bom {
            view,
            bom,
            numbering,
            start,
            increment,
            ..
        } = &t.kind
        else {
            return Vec::new();
        };
        let Ok(rows) = self.bom_rows(sheet, *view, *bom) else {
            return Vec::new();
        };
        let (_, items) = tables::bom_cells(&rows, *numbering, &[], *start, *increment);
        rows.iter()
            .zip(items)
            .map(|(r, (item, components))| BomItem {
                item,
                quantity: r.quantity,
                properties: r.properties.clone(),
                components,
            })
            .collect()
    }

    /// BOM rows of the assembly in a view (on any sheet).
    pub fn bom_rows(
        &mut self,
        sheet: &Sheet,
        view: u32,
        kind: of_assembly::bom::BomType,
    ) -> Result<Vec<of_assembly::bom::BomRow>, String> {
        let owner = self
            .doc
            .sheet_of_view(view)
            .cloned()
            .unwrap_or_else(|| sheet.clone());
        let r = self.resolve(&owner, view)?;
        let path = self.model_path(&r.model);
        if !crate::model::is_assembly(&path) {
            return Err("a BOM needs a view of an assembly".into());
        }
        let asm = self.sources.assembly(&path, r.model.configuration)?;
        Ok(of_assembly::bom::bom(asm, kind))
    }

    /// Cells of a table.
    pub fn table_cells(
        &mut self,
        sheet: &Sheet,
        t: &tables::Table,
    ) -> Result<tables::Cells, String> {
        Ok(match &t.kind {
            TableKind::Bom {
                view,
                bom,
                numbering,
                columns,
                start,
                increment,
            } => {
                let rows = self.bom_rows(sheet, *view, *bom)?;
                tables::bom_cells(&rows, *numbering, columns, *start, *increment).0
            }
            TableKind::Revision { rows, numeric } => tables::revision_cells(rows, *numeric).0,
            TableKind::General { cells, header } => {
                let mut c = tables::Cells::default();
                let mut it = cells.iter();
                if *header && let Some(h) = it.next() {
                    c.header = h.clone();
                }
                c.rows = it.cloned().collect();
                c
            }
            TableKind::GeneralTolerance { class } => tables::tolerance_cells(*class),
            TableKind::CutList { view, columns } => {
                let list = self.view_cut_list(sheet, *view)?;
                let columns: Vec<String> = if columns.is_empty() {
                    ["DESCRIPTION", "LENGTH", "ANGLE1", "ANGLE2"]
                        .map(String::from)
                        .to_vec()
                } else {
                    columns.iter().map(|c| c.to_uppercase()).collect()
                };
                let rows = list
                    .items
                    .iter()
                    .filter(|i| !i.excluded)
                    .map(|i| {
                        let mut row = vec![
                            i.number.map_or_else(String::new, |n| n.to_string()),
                            i.quantity.to_string(),
                        ];
                        row.extend(
                            columns
                                .iter()
                                .map(|c| i.properties.get(c).cloned().unwrap_or_default()),
                        );
                        row
                    })
                    .collect();
                let mut header = vec!["ITEM NO.".to_string(), "QTY.".to_string()];
                header.extend(columns.iter().cloned());
                let mut align = vec![HAlign::Center, HAlign::Center];
                align.extend(columns.iter().map(|c| {
                    if c == "DESCRIPTION" || c == "MATERIAL" {
                        HAlign::Left
                    } else {
                        HAlign::Right
                    }
                }));
                tables::Cells {
                    header,
                    rows,
                    align,
                    title: None,
                }
            }
            TableKind::Weld { view } => {
                let std = self.doc.standard;
                let list = self.view_cut_list(sheet, *view)?;
                let rows = list
                    .welds
                    .beads
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        vec![
                            (i + 1).to_string(),
                            w.symbol.clone().unwrap_or_default(),
                            std.number(w.size, std.precision()),
                            std.number(w.length, std.precision()),
                            w.material.clone().unwrap_or_default(),
                            w.process.clone().unwrap_or_default(),
                        ]
                    })
                    .collect();
                tables::Cells {
                    header: ["ITEM", "SYMBOL", "SIZE", "LENGTH", "MATERIAL", "PROCESS"]
                        .map(String::from)
                        .to_vec(),
                    rows,
                    align: vec![
                        HAlign::Center,
                        HAlign::Left,
                        HAlign::Right,
                        HAlign::Right,
                        HAlign::Left,
                        HAlign::Left,
                    ],
                    title: None,
                }
            }
            TableKind::Bend { view, .. } => {
                let std = self.doc.standard;
                let (_, flat) = self.flat_items(sheet, *view)?;
                let rows = flat
                    .bends
                    .iter()
                    .enumerate()
                    .map(|(i, b)| {
                        vec![
                            (i + 1).to_string(),
                            if b.up { "UP" } else { "DOWN" }.to_string(),
                            format!("{}°", std.number(b.angle.to_degrees(), 1)),
                            std.number(b.radius, std.precision()),
                            allowance_text(&b.allowance, &std),
                        ]
                    })
                    .collect();
                tables::Cells {
                    header: ["TAG", "DIRECTION", "ANGLE", "RADIUS", "ALLOWANCE"]
                        .map(String::from)
                        .to_vec(),
                    rows,
                    align: vec![
                        HAlign::Center,
                        HAlign::Center,
                        HAlign::Right,
                        HAlign::Right,
                        HAlign::Left,
                    ],
                    title: None,
                }
            }
            TableKind::Punch { view, origin } => {
                let std = self.doc.standard;
                let (r, flat) = self.flat_items(sheet, *view)?;
                let o = match origin {
                    Some(pick) => {
                        let lo = self.locate(sheet, &pick.target)?;
                        let p = pick_point(&lo, pick);
                        DVec2::new(p.dot(r.frame.x), p.dot(r.frame.y))
                    }
                    None => flat.corner,
                };
                let ids: Vec<String> = flat
                    .punches
                    .iter()
                    .map(|p| p.id.clone().unwrap_or_else(|| "-".into()))
                    .collect();
                let rows = flat
                    .punches
                    .iter()
                    .zip(&ids)
                    .enumerate()
                    .map(|(i, (p, id))| {
                        vec![
                            (i + 1).to_string(),
                            std.number(p.at.x - o.x, std.precision()),
                            std.number(p.at.y - o.y, std.precision()),
                            format!("{}°", std.number(p.angle.to_degrees(), 1)),
                            id.clone(),
                            ids.iter().filter(|x| *x == id).count().to_string(),
                        ]
                    })
                    .collect();
                tables::Cells {
                    header: ["TAG", "X LOC", "Y LOC", "ANGLE", "PUNCH ID", "QTY"]
                        .map(String::from)
                        .to_vec(),
                    rows,
                    align: vec![
                        HAlign::Center,
                        HAlign::Right,
                        HAlign::Right,
                        HAlign::Right,
                        HAlign::Left,
                        HAlign::Right,
                    ],
                    title: None,
                }
            }
            TableKind::Hole {
                origin,
                holes,
                tags,
                combine,
                ..
            } => {
                let lo = self.locate(sheet, &origin.target)?;
                let o = pick_point(&lo, origin);
                let frame = lo.view.frame;
                let std = self.doc.standard;
                let tag_names = self.hole_tags(sheet, holes, *tags);
                let mut rows: Vec<Vec<String>> = Vec::new();
                let mut sizes: Vec<String> = Vec::new();
                for (r, tag) in holes.iter().zip(tag_names) {
                    let l = self.locate(sheet, r)?;
                    let c = l.geo.anchor();
                    let size = self.hole_text(sheet, r, &l).unwrap_or_default();
                    let size = strip_count(&size);
                    let x = std.number((c - o).dot(frame.x), std.precision());
                    let y = std.number((c - o).dot(frame.y), std.precision());
                    if *combine {
                        if let Some(i) = sizes.iter().position(|s| *s == size) {
                            rows[i][0] += &format!(", {tag}");
                            continue;
                        }
                        sizes.push(size.clone());
                    }
                    rows.push(vec![tag, x, y, size]);
                }
                tables::Cells {
                    header: vec!["TAG".into(), "X LOC".into(), "Y LOC".into(), "SIZE".into()],
                    rows,
                    align: vec![HAlign::Center, HAlign::Right, HAlign::Right, HAlign::Left],
                    title: None,
                }
            }
        })
    }

    /// Pieces of a table to draw on `sheet`: rows, corner position, anchor
    /// and cells.
    fn table_pieces(
        &mut self,
        sheet: &Sheet,
        a: &Annotation,
        t: &tables::Table,
    ) -> Result<Vec<TablePiece>, String> {
        let cells = self.table_cells(sheet, t)?;
        let ranges = tables::pieces(cells.rows.len(), t.max_rows, t.continued.len());
        let mut out = Vec::new();
        for (i, range) in ranges.into_iter().enumerate() {
            let (on, at) = if i == 0 {
                (None, t.at)
            } else {
                let piece = &t.continued[i - 1];
                (piece.sheet, piece.at)
            };
            if on.is_none_or(|s| s == sheet.id) {
                let at = self.ann_pos(sheet, a, at)?;
                out.push((range, at, t.anchor, cells.clone()));
            }
        }
        Ok(out)
    }

    /// Hole tags: by size (A1, A2, B1) or numbered.
    fn hole_tags(&mut self, sheet: &Sheet, holes: &[ViewRef], tags: tables::Tags) -> Vec<String> {
        let mut sizes: Vec<String> = Vec::new();
        let mut counts: Vec<usize> = Vec::new();
        let mut out = Vec::new();
        for (gi, r) in holes.iter().enumerate() {
            let size = match self.locate(sheet, r) {
                Ok(l) => strip_count(&self.hole_text(sheet, r, &l).unwrap_or_default()),
                Err(_) => String::new(),
            };
            let si = match sizes.iter().position(|s| *s == size) {
                Some(i) => i,
                None => {
                    sizes.push(size);
                    counts.push(0);
                    sizes.len() - 1
                }
            };
            out.push(tables::hole_tag(tags, si, counts[si], gi));
            counts[si] += 1;
        }
        out
    }

    /// Callout text of a hole edge: Hole Wizard data when a hole feature made
    /// it, else the cylinder's diameter.
    pub fn hole_text(
        &mut self,
        sheet: &Sheet,
        r: &ViewRef,
        loc: &Located,
    ) -> Result<String, String> {
        let std = self.doc.standard;
        let num = |v: f64| std.number(v, std.precision().max(1));
        let Geo::Circle {
            center,
            radius,
            axis,
            ..
        } = loc.geo
        else {
            return Err("a hole callout needs a circular edge".into());
        };
        let path = self.model_path(&loc.view.model);
        let (part, pose) = self.sources.locate(
            &path,
            loc.view.model.configuration,
            loc.view.model.flat.is_some(),
            loc.view.exploded,
            &r.entity,
        )?;
        // Back into part coordinates.
        let inv = |q: DVec3| pose.rotation.inverse() * (q - pose.translation);
        let (c, ax) = (inv(center), pose.rotation.inverse() * axis);
        let doc = part.doc();
        let state = part.state();
        let size = state
            .body
            .as_ref()
            .and_then(|b| b.shape.bbox())
            .map(|b| b.diagonal())
            .unwrap_or(100.0);
        let tol = size * 1e-6 + 1e-6;
        for f in &doc.features {
            let FeatureKind::Hole(h) = &f.kind else {
                continue;
            };
            let Some((plane, sk)) = state.sketches.get(&h.sketch) else {
                continue;
            };
            if plane.normal.cross(ax).length() > 1e-6 {
                continue;
            }
            let Ok(dims) = of_model::hole_dims(h) else {
                continue;
            };
            let points: Vec<DVec3> = sk
                .entities
                .iter()
                .filter(|(id, e)| {
                    **id != of_model::sketch::ORIGIN
                        && !e.construction
                        && matches!(e.geo, of_model::sketch::Geometry::Point { .. })
                        && !sk.entities.values().any(|o| o.point_refs().contains(id))
                })
                .filter_map(|(id, _)| sk.point(*id).ok())
                .map(|q| plane.to_world(q))
                .collect();
            let on_axis = points.iter().any(|q| {
                let v = c - *q;
                (v - plane.normal * v.dot(plane.normal)).length() < tol
            });
            let radii = [
                Some(dims.diameter),
                dims.cbore.map(|x| x.0),
                dims.csink.map(|x| x.0),
            ];
            let matches_radius = radii
                .iter()
                .flatten()
                .any(|d| (d / 2.0 - radius).abs() < tol * 10.0)
                || h.kind == HoleKind::Tapped;
            if !(on_axis && matches_radius) {
                continue;
            }
            let count = if points.len() > 1 {
                format!("{}X ", points.len())
            } else {
                String::new()
            };
            let depth = match h.end {
                HoleEnd::ThroughAll => "THRU".to_string(),
                HoleEnd::Blind { depth } => format!("↧{}", num(depth)),
            };
            let sep = if std.base == StandardBase::Iso {
                " "
            } else {
                "\n"
            };
            let body = match h.kind {
                HoleKind::Simple => format!("⌀{} {depth}", num(dims.diameter)),
                HoleKind::Counterbore => {
                    let (cd, cdepth) = dims.cbore.unwrap_or_default();
                    format!(
                        "⌀{} {depth}{sep}⌴⌀{} ↧{}",
                        num(dims.diameter),
                        num(cd),
                        num(cdepth)
                    )
                }
                HoleKind::Countersink => {
                    let (cd, ang) = dims.csink.unwrap_or_default();
                    format!(
                        "⌀{} {depth}{sep}⌵⌀{} × {}°",
                        num(dims.diameter),
                        num(cd),
                        std.number(ang.to_degrees(), 0)
                    )
                }
                HoleKind::Tapped => {
                    let thread_depth = state
                        .hole_threads
                        .get(&f.id)
                        .and_then(|t| t.first())
                        .map(|t| t.length);
                    let tdepth = match (&h.end, thread_depth) {
                        (HoleEnd::ThroughAll, _) => "THRU".to_string(),
                        (_, Some(l)) => format!("↧{}", num(l)),
                        (HoleEnd::Blind { depth }, None) => format!("↧{}", num(*depth)),
                    };
                    format!("⌀{} {depth}{sep}{} {tdepth}", num(dims.diameter), h.size)
                }
            };
            return Ok(format!("{count}{body}"));
        }
        let _ = sheet;
        Ok(format!("⌀{}", num(radius * 2.0)))
    }

    /// Import model items into views: dimensions marked for drawing (each in
    /// the view that shows it truest, skipping ones already on the sheet),
    /// hole callouts and cosmetic thread callouts. Returns the new
    /// annotations (not yet added).
    pub fn model_items(
        &mut self,
        sheet_id: u32,
        views: &[u32],
        opts: &ModelItems,
    ) -> Result<Vec<Annotation>, String> {
        let sheet = self
            .doc
            .sheet(sheet_id)
            .cloned()
            .ok_or_else(|| format!("no sheet {sheet_id}"))?;
        // View boxes, so dimensions go outside the outlines.
        let boxes: std::collections::HashMap<u32, [P2; 2]> = self
            .render(sheet_id)?
            .views
            .iter()
            .filter_map(|v| v.bbox.map(|b| (v.id, b)))
            .collect();
        let mut stacked: std::collections::HashMap<(u32, i64, i64), usize> = Default::default();
        let existing: Vec<String> = self
            .doc
            .sheets
            .iter()
            .flat_map(|s| s.annotations.iter())
            .filter_map(|a| match &a.kind {
                AnnotationKind::Dimension(d) => d.driving().map(String::from),
                _ => None,
            })
            .collect();
        let mut out: Vec<Annotation> = Vec::new();
        let mut next = self.doc.next_id;
        let mut alloc = || {
            let id = next;
            next += 1;
            id
        };
        // Candidate views: part views among those asked for.
        let mut cands = Vec::new();
        for v in views {
            let r = self.resolve(&sheet, *v)?;
            let path = self.model_path(&r.model);
            if crate::model::is_assembly(&path) {
                continue;
            }
            cands.push((*v, r, path));
        }
        let Some((_, first, path)) = cands.first().cloned() else {
            return Ok(Vec::new());
        };
        let part =
            self.sources
                .part(&path, first.model.configuration, first.model.flat.is_some())?;
        let doc = part.doc().clone();
        if opts.dimensions {
            let mut taken: Vec<String> = existing;
            for f in &doc.features {
                for prm in of_model::equations::parameters(f) {
                    if prm.integer || taken.contains(&prm.key) {
                        continue;
                    }
                    if !doc.dimension_display.get(&prm.key).is_none_or(|d| d.show) {
                        continue;
                    }
                    let Ok(m) = model_measure(&part, &prm.key) else {
                        continue;
                    };
                    // The view showing it truest; among equals, the one
                    // looking at its sketch, then the first asked for.
                    let normal = sketch_normal(&part, &prm.key);
                    let mut best: Option<(u32, &Resolved, f64)> = None;
                    for (id, r, _) in &cands {
                        let t = trueness(&m, r);
                        if t <= 0.999 {
                            continue;
                        }
                        let score = t + normal.map_or(0.0, |n| n.dot(r.frame.toward).abs() * 1e-3);
                        if best.is_none_or(|b| score > b.2 + 1e-12) {
                            best = Some((*id, r, score));
                        }
                    }
                    let Some((vid, r, _)) = best else { continue };
                    let at = default_at(&m, r, boxes.get(&vid).copied(), |side| {
                        let k = stacked.entry((vid, side[0], side[1])).or_default();
                        *k += 1;
                        *k - 1
                    });
                    // Imported dimensions keep their place relative to
                    // what they measure, so they follow model changes.
                    let anchor = measure_anchor(&m, r);
                    let rel = [at[0] - anchor[0], at[1] - anchor[1]];
                    let mut dim = Dimension::new(
                        DimKind::Model {
                            view: vid,
                            key: prm.key.clone(),
                        },
                        rel,
                    );
                    if let Some(name) = doc
                        .dimension_display
                        .get(&prm.key)
                        .and_then(|d| d.name.clone())
                    {
                        dim.below = name;
                    }
                    out.push(Annotation {
                        id: alloc(),
                        attach: Some(vid),
                        layer: None,
                        kind: AnnotationKind::Dimension(Box::new(dim)),
                    });
                    taken.push(prm.key);
                }
            }
        }
        if opts.hole_callouts {
            // One callout per hole feature, on a circular edge seen end-on.
            let mut done: Vec<String> = Vec::new();
            for (vid, r, _) in &cands {
                let shape = part.shape().cloned();
                let Some(shape) = shape else { continue };
                for i in 0..shape.count(of_model::geom::Kind::Edge) {
                    let Ok(info) = shape.edge_info(i) else {
                        continue;
                    };
                    if info.kind != of_model::geom::CurveKind::Circle || !info.closed {
                        continue;
                    }
                    if info.axis.normalize().dot(r.frame.toward).abs() < 0.999 {
                        continue;
                    }
                    let Some(entity) = part.state().ref_for(of_model::geom::Kind::Edge, i) else {
                        continue;
                    };
                    let target = ViewRef {
                        view: *vid,
                        entity: CompRef {
                            component: Vec::new(),
                            entity,
                        },
                    };
                    let Ok(l) = self.locate(&sheet, &target) else {
                        continue;
                    };
                    let Ok(t) = self.hole_text(&sheet, &target, &l) else {
                        continue;
                    };
                    // Plain cylinders only get callouts when asked for every hole.
                    if !t.contains("THRU") && !t.contains('↧') {
                        continue;
                    }
                    if done.contains(&t) {
                        continue;
                    }
                    done.push(t);
                    let c = l.sheet(info.center);
                    let rr = info.radius * r.scale;
                    let at = [c[0] + rr + 8.0, c[1] + rr + 8.0];
                    out.push(Annotation {
                        id: alloc(),
                        attach: Some(*vid),
                        layer: None,
                        kind: AnnotationKind::HoleCallout {
                            at: [at[0] - r.origin.x, at[1] - r.origin.y],
                            target,
                            text: None,
                        },
                    });
                }
            }
        }
        if opts.threads {
            let state = part.state();
            for (fid, th) in &state.cosmetic_threads {
                let Ok(ann) = of_model::cosmetic_thread::annotation(th, state) else {
                    continue;
                };
                let Some((vid, r, _)) = cands.first() else {
                    continue;
                };
                let target = ViewRef {
                    view: *vid,
                    entity: CompRef {
                        component: Vec::new(),
                        entity: th.face.clone(),
                    },
                };
                let p3 = ann.start + ann.axis * (th.length / 2.0);
                let s = r.to_sheet(r.frame.project(p3));
                let text = format!("M{}×{}", std_num(th.diameter), std_num(th.pitch));
                let _ = fid;
                out.push(Annotation {
                    id: alloc(),
                    attach: Some(*vid),
                    layer: None,
                    kind: AnnotationKind::Note {
                        at: [s[0] + 10.0 - r.origin.x, s[1] + 10.0 - r.origin.y],
                        text,
                        height: None,
                        leader: Some(Anchor::Model {
                            target,
                            at: [r.frame.project(p3).x, r.frame.project(p3).y],
                        }),
                        jog: false,
                        border: crate::annotations::Border::None,
                        angle: 0.0,
                    },
                });
            }
        }
        if opts.axes {
            // Centerlines on cylinders seen from the side, center marks on
            // circles seen end-on.
            let shape = part.shape().cloned();
            if let Some(shape) = shape {
                for (vid, r, _) in &cands {
                    let mut marks = Vec::new();
                    for i in 0..shape.count(of_model::geom::Kind::Face) {
                        let Ok(info) = shape.face_info(i) else {
                            continue;
                        };
                        if info.kind != of_model::geom::SurfaceKind::Cylinder {
                            continue;
                        }
                        let Some(entity) = part.state().ref_for(of_model::geom::Kind::Face, i)
                        else {
                            continue;
                        };
                        let target = ViewRef {
                            view: *vid,
                            entity: CompRef {
                                component: Vec::new(),
                                entity,
                            },
                        };
                        let along = info.axis_dir.normalize().dot(r.frame.toward).abs();
                        if along < 1e-6 {
                            out.push(Annotation {
                                id: alloc(),
                                attach: Some(*vid),
                                layer: None,
                                kind: AnnotationKind::Centerline {
                                    a: target,
                                    b: None,
                                    extend: 2.0,
                                },
                            });
                        } else if along > 0.999 {
                            // A circular edge of the face for the mark.
                            if let Some(e) = shape.face_edges().get(i as usize).and_then(|es| {
                                es.iter().copied().find(|e| {
                                    shape
                                        .edge_info(*e)
                                        .is_ok_and(|x| x.kind == of_model::geom::CurveKind::Circle)
                                })
                            }) && let Some(entity) =
                                part.state().ref_for(of_model::geom::Kind::Edge, e)
                            {
                                marks.push(ViewRef {
                                    view: *vid,
                                    entity: CompRef {
                                        component: Vec::new(),
                                        entity,
                                    },
                                });
                            }
                        }
                    }
                    if !marks.is_empty() {
                        out.push(Annotation {
                            id: alloc(),
                            attach: Some(*vid),
                            layer: None,
                            kind: AnnotationKind::CenterMark {
                                targets: marks,
                                pattern: MarkPattern::Single,
                                size: None,
                                extended: true,
                            },
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    /// Change a part through an imported (driving) dimension: the part file
    /// is edited and saved, and every view showing it updates.
    /// Returns the part file saved.
    pub fn set_model_dimension(
        &mut self,
        annotation: u32,
        value: f64,
    ) -> Result<std::path::PathBuf, String> {
        let (sheet, d) = self
            .doc
            .sheets
            .iter()
            .find_map(|s| {
                s.annotations
                    .iter()
                    .find(|a| a.id == annotation)
                    .and_then(|a| match &a.kind {
                        AnnotationKind::Dimension(d) => Some((s.clone(), d.clone())),
                        _ => None,
                    })
            })
            .ok_or_else(|| format!("no dimension {annotation}"))?;
        let DimKind::Model { view, key } = &d.kind else {
            return Err("only imported model dimensions drive the model".into());
        };
        let r = self.resolve(&sheet, *view)?;
        let path = self.model_path(&r.model);
        let mut doc = of_model::file::load(&path)?;
        let target = doc
            .features
            .iter()
            .flat_map(of_model::equations::parameters)
            .find(|t| &t.key == key)
            .ok_or_else(|| format!("{key} no longer exists"))?;
        let angle = target.value.angle != 0;
        let v = if angle { value.to_radians() } else { value };
        of_model::equations::set_parameter(
            doc.feature_mut(target.feature)
                .ok_or("the feature is gone")?,
            &target,
            of_model::equations::Quantity {
                value: v,
                ..target.value
            },
        )?;
        doc.equations.remove(key);
        let mut m = of_model::Model::new(doc.clone());
        m.rebuild();
        if let Some(e) = m.statuses().into_iter().find_map(|s| s.error) {
            return Err(format!("the part would not rebuild: {e}"));
        }
        of_model::file::save(&doc, &path)?;
        self.sources.forget(&path);
        Ok(path)
    }

    /// Balloons for the components of an assembly view, laid out around it.
    pub fn auto_balloons(
        &mut self,
        sheet_id: u32,
        view: u32,
        layout: BalloonLayout,
        ignore_multiple: bool,
    ) -> Result<Vec<Annotation>, String> {
        let sheet = self
            .doc
            .sheet(sheet_id)
            .cloned()
            .ok_or_else(|| format!("no sheet {sheet_id}"))?;
        let render = self.render(sheet_id)?;
        let bbox = render
            .views
            .iter()
            .find(|v| v.id == view)
            .and_then(|v| v.bbox)
            .ok_or("the view has nothing drawn")?;
        let r = self.resolve(&sheet, view)?;
        // A weldment part's view is ballooned by cut list item instead.
        let path = self.model_path(&r.model);
        let weldment = if crate::model::is_assembly(&path) {
            None
        } else {
            let part = self.sources.part(&path, r.model.configuration, false)?;
            if part.state().weldment.is_some() {
                let list = of_model::weldment::cutlist::cut_list(part.doc(), part.state())?;
                Some((part, list))
            } else {
                None
            }
        };
        // Which cut list item, by one of its bodies, an edge belongs to.
        let item_of = |e: &CompRef| -> Option<(Vec<String>, of_model::BodyId)> {
            let (part, list) = weldment.as_ref()?;
            let state = part.state();
            let (of_model::geom::Kind::Edge, global) = state.index_of(&e.entity).ok()? else {
                return None;
            };
            let whole = &state.body.as_ref()?.shape;
            let body = state.bodies.iter().find(|b| {
                b.geometry
                    .shape
                    .indices_in(whole, of_model::geom::Kind::Edge)
                    .is_ok_and(|ix| ix.contains(&global))
            })?;
            let item = list
                .items
                .iter()
                .find(|i| !i.excluded && i.bodies.contains(&body.id))?;
            Some((vec![format!("item{}", item.number?)], body.id.clone()))
        };
        // One edge per top-level component (or cut list item), from what the
        // view drew.
        let mut picks: Vec<(Vec<String>, CompRef, P2)> = Vec::new();
        let mut bodies: Vec<Option<of_model::BodyId>> = Vec::new();
        for item in &render.graphics.items {
            let Owner::View {
                view: v,
                entity: Some(e),
            } = &item.owner
            else {
                continue;
            };
            if *v != view {
                continue;
            }
            let (top, body) = match &weldment {
                Some(_) => match item_of(e) {
                    Some((key, body)) => (key, Some(body)),
                    None => continue,
                },
                None if e.component.is_empty() => continue,
                None => (vec![e.component[0].clone()], None),
            };
            if picks.iter().any(|p| p.0 == top) {
                continue;
            }
            let at = match &item.prim {
                Prim::Line { a, b } => [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0],
                Prim::Arc {
                    center,
                    radius,
                    start,
                    end,
                } => {
                    let m = (start + end) / 2.0;
                    [center[0] + radius * m.cos(), center[1] + radius * m.sin()]
                }
                Prim::Polyline { points, .. } => points[points.len() / 2],
                _ => continue,
            };
            picks.push((top, e.clone(), at));
            bodies.push(body);
        }
        if ignore_multiple && weldment.is_none() {
            // One balloon per BOM row.
            let rows = self
                .bom_rows(&sheet, view, of_assembly::bom::BomType::TopLevel)
                .unwrap_or_default();
            let mut seen: Vec<usize> = Vec::new();
            picks.retain(|(k, _, _)| {
                match rows.iter().position(|row| row.components.contains(k)) {
                    Some(i) if seen.contains(&i) => false,
                    Some(i) => {
                        seen.push(i);
                        true
                    }
                    None => true,
                }
            });
        }
        let c = [
            (bbox[0][0] + bbox[1][0]) / 2.0,
            (bbox[0][1] + bbox[1][1]) / 2.0,
        ];
        let margin = 12.0;
        let n = picks.len().max(1);
        let mut out = Vec::new();
        let mut places: Vec<P2> = Vec::new();
        for (next, (i, ((key, entity, on), body))) in
            (self.doc.next_id..).zip(picks.into_iter().zip(bodies).enumerate())
        {
            let at = match layout {
                BalloonLayout::Circular => {
                    let dir = DVec2::new(on[0] - c[0], on[1] - c[1])
                        .normalize_or(DVec2::from_angle(i as f64));
                    let rx = (bbox[1][0] - bbox[0][0]) / 2.0 + margin;
                    let ry = (bbox[1][1] - bbox[0][1]) / 2.0 + margin;
                    [c[0] + dir.x * rx, c[1] + dir.y * ry]
                }
                BalloonLayout::Top => [
                    bbox[0][0] + (bbox[1][0] - bbox[0][0]) * (i as f64 + 0.5) / n as f64,
                    bbox[1][1] + margin,
                ],
                BalloonLayout::Bottom => [
                    bbox[0][0] + (bbox[1][0] - bbox[0][0]) * (i as f64 + 0.5) / n as f64,
                    bbox[0][1] - margin,
                ],
                BalloonLayout::Left => [
                    bbox[0][0] - margin,
                    bbox[1][1] - (bbox[1][1] - bbox[0][1]) * (i as f64 + 0.5) / n as f64,
                ],
                BalloonLayout::Right => [
                    bbox[1][0] + margin,
                    bbox[1][1] - (bbox[1][1] - bbox[0][1]) * (i as f64 + 0.5) / n as f64,
                ],
                BalloonLayout::Square => {
                    // Around the box, nearest side to the edge picked.
                    let dx = (on[0] - c[0]) / (bbox[1][0] - bbox[0][0]).max(1e-9);
                    let dy = (on[1] - c[1]) / (bbox[1][1] - bbox[0][1]).max(1e-9);
                    if dx.abs() > dy.abs() {
                        [
                            if dx > 0.0 {
                                bbox[1][0] + margin
                            } else {
                                bbox[0][0] - margin
                            },
                            on[1],
                        ]
                    } else {
                        [
                            on[0],
                            if dy > 0.0 {
                                bbox[1][1] + margin
                            } else {
                                bbox[0][1] - margin
                            },
                        ]
                    }
                }
            };
            let local = r.from_sheet(on);
            places.push(at);
            out.push(Annotation {
                id: next,
                attach: Some(view),
                layer: None,
                kind: AnnotationKind::Balloon {
                    at: [at[0] - r.origin.x, at[1] - r.origin.y],
                    leader: Anchor::Model {
                        target: ViewRef { view, entity },
                        at: [local.x, local.y],
                    },
                    content: match body {
                        Some(body) => BalloonText::CutListItem { body },
                        None => BalloonText::Item { component: key },
                    },
                    lower: None,
                    shape: crate::annotations::BalloonShape::Circle,
                    size: None,
                    stacked: Vec::new(),
                    table: None,
                },
            });
        }
        // Balloons must not overlap: spread them along their side.
        let size = self.doc.standard.text_height() * 3.0;
        spread(&mut places, size + 2.0);
        for (a, at) in out.iter_mut().zip(places) {
            if let AnnotationKind::Balloon { at: rel, .. } = &mut a.kind {
                *rel = [at[0] - r.origin.x, at[1] - r.origin.y];
            }
        }
        Ok(out)
    }
}

/// Push points apart until none are closer than `gap`. Points on a common
/// horizontal or vertical line move along it, so balloons stay on their side
/// of a view.
fn spread(points: &mut [P2], gap: f64) {
    for _ in 0..200 {
        let mut moved = false;
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                let (a, b) = (DVec2::from(points[i]), DVec2::from(points[j]));
                let d = b - a;
                let len = d.length();
                if len >= gap - 1e-9 {
                    continue;
                }
                let dir = if (a.x - b.x).abs() < 1e-9 {
                    DVec2::new(0.0, if d.y >= 0.0 { 1.0 } else { -1.0 })
                } else if (a.y - b.y).abs() < 1e-9 {
                    DVec2::new(if d.x >= 0.0 { 1.0 } else { -1.0 }, 0.0)
                } else {
                    d / len
                };
                let push = dir * ((gap - len) / 2.0 + 1e-6);
                points[i] = p(a - push);
                points[j] = p(b + push);
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}

/// Which model items to import.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct ModelItems {
    #[serde(default = "yes")]
    pub dimensions: bool,
    #[serde(default = "yes")]
    pub hole_callouts: bool,
    #[serde(default = "yes")]
    pub threads: bool,
    /// Centerlines and center marks of cylinders.
    #[serde(default)]
    pub axes: bool,
}

fn yes() -> bool {
    true
}

impl Default for ModelItems {
    fn default() -> Self {
        ModelItems {
            dimensions: true,
            hole_callouts: true,
            threads: true,
            axes: false,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BalloonLayout {
    /// On the nearest side of the view's box.
    #[default]
    Square,
    Circular,
    Top,
    Bottom,
    Left,
    Right,
}

fn std_num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// A hole callout without its `4X ` count.
fn strip_count(s: &str) -> String {
    match s.split_once("X ") {
        Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => {
            rest.to_string()
        }
        _ => s.to_string(),
    }
}

/// A BOM row as balloons see it.
struct BomItem {
    item: String,
    quantity: u32,
    properties: BTreeMap<String, String>,
    components: Vec<Vec<String>>,
}

/// Rows of a table shown at a sheet position from a corner.
type TablePiece = (std::ops::Range<usize>, P2, tables::Corner, tables::Cells);

/// A bend of a flat pattern view, in view-plane mm.
struct ViewBend {
    a: DVec2,
    b: DVec2,
    /// Toward the viewer.
    up: bool,
    angle: f64,
    radius: f64,
    allowance: of_model::sheet_metal::Allowance,
}

/// A form of a flat pattern view, in view-plane mm.
struct ViewPunch {
    at: DVec2,
    angle: f64,
    id: Option<String>,
}

/// What a flat pattern view shows of its bends and forms.
struct FlatItems {
    bends: Vec<ViewBend>,
    punches: Vec<ViewPunch>,
    /// Lower left corner of the flat, view-plane mm.
    corner: DVec2,
}

/// How a bend's allowance reads in a bend table.
fn allowance_text(a: &of_model::sheet_metal::Allowance, std: &crate::standard::Standard) -> String {
    use of_model::sheet_metal::Allowance;
    match a {
        Allowance::KFactor { k } => format!("K {}", std.number(*k, 3)),
        Allowance::BendAllowance { value } => format!("BA {}", std.number(*value, std.precision())),
        Allowance::BendDeduction { value } => format!("BD {}", std.number(*value, std.precision())),
        Allowance::BendTable { .. } => "BEND TABLE".into(),
        Allowance::BendCalculation { .. } => "CALCULATION".into(),
    }
}

impl Drawing {
    /// The part a view shows (not flattened, every body).
    fn view_part(
        &mut self,
        sheet: &Sheet,
        view: u32,
    ) -> Result<std::rc::Rc<of_assembly::cache::Part>, String> {
        let r = self.resolve(sheet, view)?;
        let path = self.model_path(&r.model);
        if crate::model::is_assembly(&path) {
            return Err("cut lists and weld tables come from a part view".into());
        }
        self.sources.part(&path, r.model.configuration, false)
    }

    /// The weldment cut list of the part a view shows.
    pub fn view_cut_list(
        &mut self,
        sheet: &Sheet,
        view: u32,
    ) -> Result<of_model::weldment::cutlist::CutList, String> {
        let part = self.view_part(sheet, view)?;
        of_model::weldment::cutlist::cut_list(part.doc(), part.state())
    }

    /// Bends and forms of flat pattern view `view` of `sheet`.
    fn flat_items(&mut self, sheet: &Sheet, view: u32) -> Result<(Resolved, FlatItems), String> {
        let r = self.resolve(sheet, view)?;
        if r.model.flat.is_none() {
            return Err("bend and punch tables need a flat pattern view".into());
        }
        let path = self.model_path(&r.model);
        let part = self.sources.part(&path, r.model.configuration, true)?;
        let state = part.state();
        let at = |p: [f64; 3]| r.frame.project(DVec3::from(p));
        let mut out = FlatItems {
            bends: Vec::new(),
            punches: Vec::new(),
            corner: DVec2::ZERO,
        };
        for record in &state.sheet_metal {
            let Some(plane) = record.flat_plane else {
                continue;
            };
            let facing = plane.normal.dot(r.frame.toward);
            if facing.abs() < 1.0 - 1e-9 {
                continue;
            }
            for l in &record.bend_lines {
                let allowance = record
                    .bends
                    .iter()
                    .find(|b| Some(b.feature) == l.feature && Some(b.index) == l.index)
                    .map_or_else(|| record.params.allowance.clone(), |b| b.allowance.clone());
                out.bends.push(ViewBend {
                    a: at(l.a),
                    b: at(l.b),
                    up: l.up == (facing > 0.0),
                    angle: l.angle,
                    radius: l.radius,
                    allowance,
                });
            }
            for m in &record.form_marks {
                out.punches.push(ViewPunch {
                    at: at(m.center),
                    angle: m.angle,
                    id: m.punch_id.clone(),
                });
            }
        }
        if let Some(body) = &state.body {
            let shape = &body.shape;
            let pts: Vec<DVec2> = (0..shape.count(of_model::geom::Kind::Vertex))
                .filter_map(|v| shape.vertex_point(v).ok())
                .map(|p| r.frame.project(p))
                .collect();
            if !pts.is_empty() {
                out.corner = pts.iter().fold(DVec2::splat(f64::MAX), |a, b| a.min(*b));
            }
        }
        Ok((r, out))
    }
}

fn balloon_text(
    c: &BalloonText,
    items: &[BomItem],
    cut: Option<&of_model::weldment::cutlist::CutList>,
) -> String {
    let item_of = |body: &of_model::BodyId| {
        cut.and_then(|l| l.items.iter().find(|i| i.bodies.contains(body)))
    };
    let find = |component: &Vec<String>| {
        items.iter().find(|r| {
            r.components
                .iter()
                .any(|k| k == component || component.starts_with(k))
        })
    };
    match c {
        BalloonText::Text { text } => text.clone(),
        BalloonText::Item { component } => find(component)
            .map(|r| r.item.clone())
            .unwrap_or_else(|| "?".into()),
        BalloonText::Quantity { component } => find(component)
            .map(|r| r.quantity.to_string())
            .unwrap_or_else(|| "?".into()),
        BalloonText::Property { component, name } => find(component)
            .and_then(|r| r.properties.get(name).cloned())
            .unwrap_or_default(),
        BalloonText::CutListItem { body } => item_of(body)
            .and_then(|i| i.number)
            .map_or_else(|| "?".into(), |n| n.to_string()),
        BalloonText::CutListQuantity { body } => {
            item_of(body).map_or_else(|| "?".into(), |i| i.quantity.to_string())
        }
    }
}

fn circumcenter(a: DVec2, b: DVec2, c: DVec2) -> Option<DVec2> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (a.length_squared(), b.length_squared(), c.length_squared());
    Some(DVec2::new(
        (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    ))
}

fn intersect2(a0: DVec2, a1: DVec2, b0: DVec2, b1: DVec2) -> Option<DVec2> {
    let (da, db) = (a1 - a0, b1 - b0);
    let den = da.perp_dot(db);
    (den.abs() > 1e-12).then(|| a0 + da * ((b0 - a0).perp_dot(db) / den))
}

/// Whether a cylinder or circle axis lies in the view plane (seen from the
/// side).
fn axis_side_on(l: &Located, axis: DVec3) -> bool {
    axis.normalize_or(DVec3::Z).dot(l.view.frame.toward).abs() < 0.5
}

fn parallel(l: &Located, a: DVec3, b: DVec3) -> bool {
    let (pa, pb) = (l.view.frame.project(a), l.view.frame.project(b));
    pa.perp_dot(pb).abs() < 1e-9 * pa.length().max(1e-9) * pb.length().max(1e-9) + 1e-12
        && pa.length() > 1e-9
        && pb.length() > 1e-9
}

/// Horizontal when the text is above or below the points, vertical when
/// beside them, aligned otherwise.
fn auto_orient(a: DVec2, b: DVec2, at: DVec2) -> Orient {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    let in_x = at.x >= x0 && at.x <= x1;
    let in_y = at.y >= y0 && at.y <= y1;
    if (x1 - x0) < 1e-9 {
        return Orient::Vertical;
    }
    if (y1 - y0) < 1e-9 {
        return Orient::Horizontal;
    }
    match (in_x, in_y) {
        (true, false) => Orient::Horizontal,
        (false, true) => Orient::Vertical,
        _ => Orient::Aligned,
    }
}

fn orient_dirs(o: Orient, view: &Resolved, a: DVec2, b: DVec2) -> (DVec2, DVec3) {
    match o {
        Orient::Horizontal => (DVec2::X, view.frame.x),
        Orient::Vertical => (DVec2::Y, view.frame.y),
        _ => {
            let u = (b - a).normalize_or(DVec2::X);
            (u, view.frame.unproject(u).normalize())
        }
    }
}

/// How true a measure is shown in a view: 1 when it is parallel to paper.
fn trueness(m: &Measure, r: &Resolved) -> f64 {
    match *m {
        Measure::Linear { dir, .. } => r.frame.project(dir).length(),
        Measure::Radial { axis, .. } => axis.normalize().dot(r.frame.toward).abs(),
        Measure::Angle { d1, d2, .. } => d1
            .cross(d2)
            .normalize_or(DVec3::Z)
            .dot(r.frame.toward)
            .abs(),
        Measure::Arc { .. } => 0.0,
    }
}

/// The sheet point an imported dimension's position is measured from: the
/// middle of a distance, a circle's center or an angle's vertex.
fn measure_anchor(m: &Measure, r: &Resolved) -> P2 {
    let sheet = |q: DVec3| DVec2::from(r.to_sheet(r.frame.project(q)));
    match *m {
        Measure::Linear { a, b, .. } => p((sheet(a) + sheet(b)) / 2.0),
        Measure::Radial { center, .. } | Measure::Arc { center, .. } => p(sheet(center)),
        Measure::Angle { vertex, .. } => p(sheet(vertex)),
    }
}

/// Where an imported dimension's text goes: outside the view's box on the
/// side nearest its geometry, stacked 8 mm apart; `slot` numbers the
/// dimensions already put on a side.
fn default_at(
    m: &Measure,
    r: &Resolved,
    bbox: Option<[P2; 2]>,
    mut slot: impl FnMut([i64; 2]) -> usize,
) -> P2 {
    let center = bbox.map_or(r.origin, |b| (DVec2::from(b[0]) + DVec2::from(b[1])) / 2.0);
    let sheet = |q: DVec3| DVec2::from(r.to_sheet(r.frame.project(q)));
    match *m {
        Measure::Linear { a, b, dir } => {
            let (sa, sb) = (sheet(a), sheet(b));
            let u = r.frame.project(dir).normalize_or(DVec2::X);
            let mid = (sa + sb) / 2.0;
            let n = u.perp();
            let out = if (mid - center).dot(n) < 0.0 { -n } else { n };
            // How far the box reaches past the dimensioned points.
            let reach = bbox.map_or(0.0, |b| {
                [b[0], [b[1][0], b[0][1]], b[1], [b[0][0], b[1][1]]]
                    .iter()
                    .map(|c| (DVec2::from(*c) - mid).dot(out))
                    .fold(0.0, f64::max)
            });
            let k = slot([out.x.round() as i64, out.y.round() as i64]);
            p(mid + out * (reach + 10.0 + 8.0 * k as f64))
        }
        Measure::Radial {
            center: c, radius, ..
        } => {
            let c = sheet(c);
            let away = (c - center).normalize_or(DVec2::new(1.0, 1.0).normalize());
            let d = (away + DVec2::new(1.0, 1.0).normalize()).normalize_or(away);
            p(c + d * (radius * r.scale + 8.0))
        }
        Measure::Angle { vertex, d1, d2 } => {
            let v = sheet(vertex);
            let bis = (r.frame.project(d1).normalize_or(DVec2::X)
                + r.frame.project(d2).normalize_or(DVec2::Y))
            .normalize_or(DVec2::X);
            p(v + bis * 15.0)
        }
        Measure::Arc { center: c, .. } => p(sheet(c)),
    }
}

/// Draw an angle between two 3D lines seen in a view, in the quadrant of
/// `at`.
fn angle_dim(
    pt: &mut Painter,
    d: &Dimension,
    l: &Located,
    a: (DVec3, DVec3),
    b: (DVec3, DVec3),
    at: DVec2,
    std: &crate::standard::Standard,
) -> Result<(), String> {
    let (a0, a1) = (DVec2::from(l.sheet(a.0)), DVec2::from(l.sheet(a.1)));
    let (b0, b1) = (DVec2::from(l.sheet(b.0)), DVec2::from(l.sheet(b.1)));
    let vtx = intersect2(a0, a1, b0, b1).ok_or("the edges are parallel")?;
    let mut d1 = (a1 - a0).normalize_or(DVec2::X);
    let mut d2v = (b1 - b0).normalize_or(DVec2::Y);
    let mut s1 = 1.0;
    let mut s2 = 1.0;
    // Pick the ray directions that enclose `at`.
    let w = (at - vtx).normalize_or(DVec2::X);
    let mut best = (f64::MAX, 1.0, 1.0);
    for (x, y) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        let (u, v) = (d1 * x, d2v * y);
        let total = u.angle_to(v).abs();
        let parts = u.angle_to(w).abs() + w.angle_to(v).abs();
        let miss = (parts - total).abs();
        if miss < best.0 - 1e-9 {
            best = (miss, x, y);
        }
    }
    s1 *= best.1;
    s2 *= best.2;
    d1 *= s1;
    d2v *= s2;
    // Counter-clockwise from d1 to d2.
    let (r1, r2) = if d1.perp_dot(d2v) >= 0.0 {
        (d1, d2v)
    } else {
        (d2v, d1)
    };
    let da = (a.1 - a.0).normalize_or(DVec3::X) * s1;
    let db = (b.1 - b.0).normalize_or(DVec3::Y) * s2;
    let value = da.angle_between(db).to_degrees();
    let t = value_text(d, value, std, true, value);
    // Far ends of the edges along the chosen rays, for extension lines.
    let end = |p0: DVec2, p1: DVec2, r: DVec2| {
        if (p1 - vtx).dot(r) > (p0 - vtx).dot(r) {
            p1
        } else {
            p0
        }
    };
    let (e1, e2) = if d1.perp_dot(d2v) >= 0.0 {
        (end(a0, a1, r1), end(b0, b1, r2))
    } else {
        (end(b0, b1, r1), end(a0, a1, r2))
    };
    dims::draw_angle(pt, vtx, r1, r2, [e1, e2], at, &t);
    Ok(())
}

impl Resolved {
    /// The model point under a sheet position (view plane, depth 0).
    pub fn model_point(&self, at: P2) -> DVec3 {
        let q = self.from_sheet(at);
        self.frame.unproject(q)
    }
}

/// Picks of every pick-like field, for tools that rename views.
pub fn picks_of(k: &DimKind) -> Vec<&Pick> {
    match k {
        DimKind::Linear { a, b, .. } => std::iter::once(a).chain(b.iter()).collect(),
        DimKind::Angular { a, b } => vec![a, b],
        DimKind::Radius { target }
        | DimKind::Diameter { target }
        | DimKind::ArcLength { target } => vec![target],
        DimKind::Chamfer { edge, adjacent } => vec![edge, adjacent],
        DimKind::Ordinate { origin, points, .. } => std::iter::once(origin).chain(points).collect(),
        DimKind::Baseline { base, points, .. } => std::iter::once(base).chain(points).collect(),
        DimKind::Chain { points, .. } => points.iter().collect(),
        DimKind::AngularRunning { base, lines } => std::iter::once(base).chain(lines).collect(),
        DimKind::Model { .. } => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balloons_spread_along_their_side() {
        let mut pts = vec![[0.0, 10.0], [0.0, 11.0], [0.0, 10.5], [50.0, 30.0]];
        spread(&mut pts, 12.0);
        for i in 0..pts.len() {
            for j in i + 1..pts.len() {
                let d = DVec2::from(pts[i]).distance(DVec2::from(pts[j]));
                assert!(d >= 12.0 - 1e-6, "{pts:?}");
            }
        }
        // Balloons on the left side stay on it.
        assert!(pts[..3].iter().all(|q| q[0] == 0.0), "{pts:?}");
    }
}
