//! Drawing one view: its model (cut for sections and broken-out sections)
//! projected with hidden lines removed, hatching, shading, clipping for
//! detail and crop views, break bands, labels, and the marks a derived view
//! leaves on its parent (cutting lines, detail circles, view arrows).
//!
//! Geometry is built in view-plane millimetres (model mm relative to the
//! model origin, paper axes) and placed on the sheet last.

use std::collections::HashSet;

use of_assembly::CompRef;
use of_model::geom::{Curve2, Curve2d, DVec2, DVec3, EdgeClass};

use crate::clip::{Gap, Region};
use crate::document::*;
use crate::drawing::{Drawing, Resolved};
use crate::graphics::{Graphics, HAlign, LineStyle, Owner, P2, Pen, Prim, layers};
use crate::hatch::{self, Pattern};
use crate::model::Scene;
use crate::section::{self, Cut, Tool, ViewShape, frame_plane};
use crate::standard::StandardBase;
use crate::text;

/// A primitive in view-plane coordinates.
pub(crate) struct Local {
    pub prim: Prim,
    pub pen: Pen,
    pub layer: String,
    pub entity: Option<CompRef>,
}

impl Local {
    fn new(prim: Prim, pen: Pen, layer: &str) -> Local {
        Local {
            prim,
            pen,
            layer: layer.into(),
            entity: None,
        }
    }
}

fn p2(v: DVec2) -> P2 {
    [v.x, v.y]
}

/// The removed side of a cutting line in the parent's paper coordinates.
fn removed_side(parent: &Resolved, view: &Resolved) -> DVec2 {
    parent
        .frame
        .project(view.frame.toward)
        .try_normalize()
        .unwrap_or(DVec2::Y)
}

/// Letters for labels: A, B, ... skipping I, O and Q, then AA, AB, ...
pub fn label_letters(index: usize) -> String {
    const LETTERS: &[u8] = b"ABCDEFGHJKLMNPRSTUVWXYZ";
    let n = LETTERS.len();
    if index < n {
        (LETTERS[index] as char).to_string()
    } else {
        let i = index - n;
        format!("{}{}", LETTERS[(i / n) % n] as char, LETTERS[i % n] as char)
    }
}

/// The first label letter not used on any sheet of a drawing.
pub fn next_label(doc: &DrawingDocument) -> String {
    let used: HashSet<String> = doc
        .sheets
        .iter()
        .flat_map(|s| s.views.iter().filter_map(|v| v.label.clone()))
        .collect();
    (0..)
        .map(label_letters)
        .find(|l| !used.contains(l))
        .expect("letters never run out")
}

/// Break bands of a view in view-plane mm, highest first so earlier bands
/// stay where they are.
pub fn gaps(v: &View, scale: f64) -> Vec<Gap> {
    let mut out: Vec<Gap> = v
        .breaks
        .iter()
        .filter(|b| b.to > b.from)
        .map(|b| Gap {
            vertical: b.vertical,
            from: b.from,
            to: b.to,
            gap: v.break_gap / scale,
        })
        .collect();
    out.sort_by(|a, b| b.from.total_cmp(&a.from));
    out
}

fn profile_curves(p: &Profile) -> Vec<Curve2> {
    match p {
        Profile::Circle { center, radius } => vec![Curve2::Circle {
            center: DVec2::from(*center),
            radius: *radius,
        }],
        Profile::Polygon { points } => {
            let pts: Vec<DVec2> = points.iter().map(|p| DVec2::from(*p)).collect();
            section::polygon(&pts)
        }
    }
}

/// Front-most depth of a scene along a direction.
fn front_depth(scene: &Scene, toward: DVec3) -> f64 {
    let Some(b) = scene.shape.bbox() else {
        return 0.0;
    };
    let mut best = f64::MIN;
    for i in 0..8 {
        let c = DVec3::new(
            if i & 1 == 0 { b.min.x } else { b.max.x },
            if i & 2 == 0 { b.min.y } else { b.max.y },
            if i & 4 == 0 { b.min.z } else { b.max.z },
        );
        best = best.max(c.dot(toward));
    }
    best
}

impl Drawing {
    /// The shape a view shows: the scene, cut for sections and broken-out
    /// sections.
    fn view_shape(
        &mut self,
        sheet: &Sheet,
        v: &View,
        r: &Resolved,
        scene: &Scene,
    ) -> Result<ViewShape, String> {
        let reach = section::reach(scene);
        let mut tools = Vec::new();
        let mut aligned = None;
        let mut excluded: &[Vec<String>] = &[];
        let mut fasteners = false;
        if let ViewKind::Section {
            parent,
            line,
            kind,
            depth,
            excluded: ex,
            exclude_fasteners,
            ..
        } = &v.kind
        {
            excluded = ex;
            fasteners = *exclude_fasteners;
            let p = self.resolve(sheet, *parent)?;
            let side = removed_side(&p, r);
            let plane = frame_plane(&p.frame);
            let pts: Vec<DVec2> = line.points.iter().map(|q| DVec2::from(*q)).collect();
            if *kind == SectionKind::Aligned {
                if pts.len() != 3 {
                    return Err("an aligned section needs a cutting line of three points".into());
                }
                let (a, b, c) = (pts[0], pts[1], pts[2]);
                let d1 = (b - a).normalize();
                let d2 = (c - b)
                    .try_normalize()
                    .ok_or("the cutting line has no length")?;
                // Turn the removed side with the line.
                let angle = d1.perp_dot(d2).atan2(d1.dot(d2));
                let side2 = DVec2::from_angle(angle).rotate(side);
                for (seg, s) in [([a, b], side), ([b, c], side2)] {
                    tools.push(Tool {
                        plane,
                        region: section::polygon(&section::beyond(&seg, s, reach)?),
                        near: -reach,
                        far: reach,
                    });
                }
                let to3 = |q: DVec2| plane.to_world(q);
                let split_n = ((c - b).normalize() - (a - b).normalize())
                    .try_normalize()
                    .ok_or("the two arms of an aligned section are in line")?;
                let split =
                    of_model::geom::Plane::new(to3(b), p.frame.toward, p.frame.unproject(split_n));
                aligned = Some(section::Aligned {
                    split,
                    rotation: section::turn(
                        to3(b),
                        p.frame.toward,
                        p.frame.unproject(c - b),
                        p.frame.unproject(b - a),
                    ),
                });
            } else {
                tools.push(Tool {
                    plane,
                    region: section::polygon(&section::beyond(&pts, side, reach)?),
                    near: -reach,
                    far: reach,
                });
            }
            if let Some(d) = depth {
                // Material more than `d` behind the cut is left out.
                let n = r.frame.toward;
                let at = p.frame.unproject(pts[0]).dot(n);
                let big = [
                    DVec2::new(-reach, -reach),
                    DVec2::new(reach, -reach),
                    DVec2::new(reach, reach),
                    DVec2::new(-reach, reach),
                ];
                let back = Tool {
                    plane: frame_plane(&r.frame),
                    region: section::polygon(&big),
                    near: -2.0 * reach,
                    far: at - d,
                };
                if aligned.is_some() {
                    return Err("an aligned section cannot have a depth".into());
                }
                tools.push(back);
            }
        }
        for b in &v.broken_out {
            let front = front_depth(scene, r.frame.toward);
            tools.push(Tool {
                plane: frame_plane(&r.frame),
                region: profile_curves(&b.profile),
                near: front - b.depth,
                far: front + reach,
            });
        }
        if tools.is_empty() {
            return Ok(ViewShape::whole(scene));
        }
        let key = format!(
            "{:?}|{:?}|{:?}|{:.9?}",
            v.kind,
            v.broken_out,
            v.hidden_components,
            r.frame.toward.to_array()
        );
        section::cut(
            scene,
            &Cut {
                tools,
                excluded,
                exclude_fasteners: fasteners,
                toward: r.frame.toward,
                key,
                aligned,
            },
        )
    }

    /// Draw a view; returns the sheet box of its geometry.
    pub(crate) fn draw_view(
        &mut self,
        sheet: &Sheet,
        v: &View,
        g: &mut Graphics,
    ) -> Result<Option<[P2; 2]>, String> {
        let r = self.resolve(sheet, v.id)?;
        let path = self.model_path(&r.model);
        let scene = self.sources.scene(
            &path,
            r.model.configuration,
            r.exploded,
            &v.hidden_components,
        )?;
        let vs = self.view_shape(sheet, v, &r, &scene)?;
        let std = self.doc.standard;
        let thick = Pen::new(LineStyle::Continuous, std.thick_line());
        let thin = Pen::new(LineStyle::Continuous, std.thin_line());
        let mut locals: Vec<Local> = Vec::new();

        let slice = matches!(v.kind, ViewKind::Section { slice: true, .. });
        if matches!(r.display, DisplayStyle::Shaded | DisplayStyle::ShadedEdges) && !slice {
            locals.extend(shading(&vs, &r)?);
        }
        if !slice && r.display != DisplayStyle::Shaded {
            let show_hidden = matches!(
                r.display,
                DisplayStyle::HiddenVisible | DisplayStyle::Wireframe
            );
            let edges = self.projection(&vs.key, &vs.shape, &r.frame, show_hidden, r.scale)?;
            let hidden: HashSet<&CompRef> = v.hidden_edges.iter().collect();
            for e in edges.iter() {
                if e.source.is_some_and(|s| vs.edge_on.contains(&s)) {
                    continue;
                }
                let entity = e
                    .source
                    .and_then(|s| vs.edges.get(s as usize).cloned().flatten());
                if entity.as_ref().is_some_and(|x| hidden.contains(x)) {
                    continue;
                }
                let (pen, layer) = match (e.visible, e.class) {
                    (_, EdgeClass::Seam) => continue,
                    (_, EdgeClass::Smooth) if r.tangent == TangentEdges::Removed => continue,
                    (true, EdgeClass::Smooth) => match r.tangent {
                        TangentEdges::Font => (
                            Pen::new(LineStyle::Phantom, std.thin_line()),
                            layers::TANGENT,
                        ),
                        _ => (thin, layers::TANGENT),
                    },
                    (true, _) => (thick, layers::VISIBLE),
                    (false, _) if r.display == DisplayStyle::Wireframe => (thick, layers::VISIBLE),
                    (false, _) => (Pen::new(LineStyle::Hidden, std.thin_line()), layers::HIDDEN),
                };
                locals.push(Local {
                    prim: curve_prim(&e.curve),
                    pen,
                    layer: layer.into(),
                    entity,
                });
            }
        }
        // Cut faces: outlined in slice mode, hatched always.
        for f in &vs.cut_faces {
            let loops: Vec<Vec<P2>> = f
                .loops
                .iter()
                .map(|l| l.iter().map(|q| p2(r.frame.project(*q))).collect())
                .collect();
            if slice {
                for l in &loops {
                    locals.push(Local::new(
                        Prim::Polyline {
                            points: l.clone(),
                            closed: true,
                        },
                        thick,
                        layers::VISIBLE,
                    ));
                }
            }
            let pattern = v
                .hatch
                .pattern
                .unwrap_or_else(|| Pattern::for_material(f.material.as_deref()));
            if pattern == Pattern::Solid {
                let mut pen = thin;
                pen.color = [80, 80, 80];
                locals.push(Local::new(Prim::Fill { loops }, pen, layers::HATCH));
                continue;
            }
            for [a, b] in hatch::lines(&loops, pattern, v.hatch.scale / r.scale, v.hatch.angle) {
                locals.push(Local::new(Prim::Line { a, b }, thin, layers::HATCH));
            }
        }
        if r.exploded {
            let pen = Pen::new(LineStyle::Phantom, std.thin_line());
            for [a, b] in &scene.explode_lines {
                let (a, b) = (r.frame.project(*a), r.frame.project(*b));
                if a.distance(b) * r.scale > 0.5 {
                    locals.push(Local::new(
                        Prim::Line { a: p2(a), b: p2(b) },
                        pen,
                        "EXPLODE",
                    ));
                }
            }
        }
        // Broken-out outlines: thin freehand boundaries.
        for b in &v.broken_out {
            let outline = Region::from_profile(&b.profile).outline();
            locals.push(Local::new(
                Prim::Polyline {
                    points: outline,
                    closed: true,
                },
                thin,
                layers::VISIBLE,
            ));
        }

        // Detail and crop views keep what lies inside their profile.
        let region = match &v.kind {
            ViewKind::Detail { profile, .. } => Some(Region::from_profile(profile)),
            _ => v.crop.as_ref().map(Region::from_profile),
        };
        if let Some(region) = &region {
            locals = locals
                .into_iter()
                .flat_map(|l| {
                    crate::clip::clip(&l.prim, region, true)
                        .into_iter()
                        .map(move |prim| Local {
                            prim,
                            pen: l.pen,
                            layer: l.layer.clone(),
                            entity: l.entity.clone(),
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            if let ViewKind::Detail {
                style,
                full_outline,
                ..
            } = &v.kind
            {
                if *style == DetailStyle::Broken {
                    locals.push(Local::new(
                        Prim::Polyline {
                            points: jagged(&region.outline(), 1.5 / r.scale),
                            closed: true,
                        },
                        thin,
                        layers::VISIBLE,
                    ));
                } else if *full_outline {
                    locals.push(Local::new(
                        Prim::Polyline {
                            points: region.outline(),
                            closed: true,
                        },
                        thin,
                        layers::VISIBLE,
                    ));
                }
            }
        }

        // Break bands, then the break lines across the closed-up gaps.
        for gap in &r.breaks {
            locals = locals
                .into_iter()
                .flat_map(|l| {
                    gap.apply(&l.prim)
                        .into_iter()
                        .map(move |prim| Local {
                            prim,
                            pen: l.pen,
                            layer: l.layer.clone(),
                            entity: l.entity.clone(),
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
        }
        if !r.breaks.is_empty() {
            let b = local_bounds(&locals);
            if let Some([lo, hi]) = b {
                let over = 2.0 / r.scale;
                for gap in &r.breaks {
                    for at in [gap.from, gap.from + gap.gap] {
                        let (a, bb) = if gap.vertical {
                            ([at, lo[1] - over], [at, hi[1] + over])
                        } else {
                            ([lo[0] - over, at], [hi[0] + over, at])
                        };
                        let pts = break_line(a, bb, v.break_style, 3.0 / r.scale);
                        locals.push(Local::new(
                            Prim::Polyline {
                                points: pts,
                                closed: false,
                            },
                            thin,
                            layers::VISIBLE,
                        ));
                    }
                }
            }
        }

        // Place on the sheet.
        let start = g.items.len();
        for l in locals {
            let owner = Owner::View {
                view: v.id,
                entity: l.entity,
            };
            g.push(place(&l.prim, &r), l.pen, &l.layer, &owner);
        }
        let bbox = crate::drawing::bounds(&g.items[start..]);
        if let Some(b) = bbox {
            self.draw_label(sheet, v, &r, b, g);
        }
        Ok(bbox)
    }

    /// The label under a view: section, detail and auxiliary letters and
    /// the view scale when it differs from the sheet's.
    fn draw_label(&self, sheet: &Sheet, v: &View, r: &Resolved, bbox: [P2; 2], g: &mut Graphics) {
        let std = self.doc.standard;
        let iso = std.base == StandardBase::Iso;
        let scale = Scale::from_factor(r.scale);
        let own_scale = (r.scale - sheet.scale.factor()).abs() > 1e-9;
        let letter = v.label.as_deref();
        let text = match (&v.kind, letter) {
            (ViewKind::Section { .. }, Some(l)) => {
                let mut t = if iso {
                    format!("{l}-{l}")
                } else {
                    format!("SECTION {l}-{l}")
                };
                if own_scale {
                    t += &if iso {
                        format!(" ({})", scale.label())
                    } else {
                        format!("\nSCALE {}", scale.label())
                    };
                }
                t
            }
            (ViewKind::Detail { .. }, Some(l)) => {
                if iso {
                    format!("{l} ({})", scale.label())
                } else {
                    format!("DETAIL {l}\nSCALE {}", scale.label())
                }
            }
            (ViewKind::Auxiliary { .. }, Some(l)) => {
                let mut t = if iso {
                    l.to_string()
                } else {
                    format!("VIEW {l}")
                };
                if own_scale {
                    t += &format!(" ({})", scale.label());
                }
                t
            }
            _ if v.scale.is_some() && own_scale => format!("SCALE {}", scale.label()),
            _ => return,
        };
        let h = std.label_height();
        let lines = text.lines().count() as f64;
        let x = (bbox[0][0] + bbox[1][0]) / 2.0;
        let y = bbox[0][1] - 4.0 - h - (lines - 1.0) * h * text::LINE_PITCH;
        let pen = Pen::new(LineStyle::Continuous, std.thin_line());
        let owner = Owner::View {
            view: v.id,
            entity: None,
        };
        for p in text::layout(&text, [x, y], h, 0.0, HAlign::Center) {
            g.push(p, pen, layers::NOTES, &owner);
        }
    }

    /// Marks a derived view draws on its parent.
    pub(crate) fn draw_marks(&mut self, sheet: &Sheet, v: &View, g: &mut Graphics) {
        let Some(parent) = v.parent() else { return };
        let (Ok(p), Ok(r)) = (self.resolve(sheet, parent), self.resolve(sheet, v.id)) else {
            return;
        };
        let std = self.doc.standard;
        let thin = Pen::new(LineStyle::Continuous, std.thin_line());
        let thick = Pen::new(LineStyle::Continuous, std.thick_line());
        let owner = Owner::View {
            view: v.id,
            entity: None,
        };
        let h = std.label_height();
        let letter = v.label.clone().unwrap_or_default();
        let arrow = std.arrow_length();
        match &v.kind {
            ViewKind::Section { line, kind, .. } => {
                let pts: Vec<P2> = line
                    .points
                    .iter()
                    .map(|q| p.to_sheet(DVec2::from(*q)))
                    .collect();
                if pts.len() < 2 {
                    return;
                }
                let chain = Pen::new(LineStyle::Center, std.thin_line());
                g.push(
                    Prim::Polyline {
                        points: pts.clone(),
                        closed: false,
                    },
                    chain,
                    layers::CENTER,
                    &owner,
                );
                // Thick strokes at the ends and at every change of direction.
                let stroke = 6.0;
                let n = pts.len();
                let dir = |a: P2, b: P2| {
                    let d = DVec2::from(b) - DVec2::from(a);
                    d.normalize_or_zero()
                };
                let first = dir(pts[0], pts[1]);
                let last = dir(pts[n - 2], pts[n - 1]);
                g.line(
                    pts[0],
                    p2(DVec2::from(pts[0]) + first * stroke),
                    thick,
                    layers::CENTER,
                    &owner,
                );
                g.line(
                    p2(DVec2::from(pts[n - 1]) - last * stroke),
                    pts[n - 1],
                    thick,
                    layers::CENTER,
                    &owner,
                );
                for i in 1..n - 1 {
                    let a = dir(pts[i - 1], pts[i]);
                    let b = dir(pts[i], pts[i + 1]);
                    let c = DVec2::from(pts[i]);
                    g.line(
                        p2(c - a * stroke / 2.0),
                        p2(c),
                        thick,
                        layers::CENTER,
                        &owner,
                    );
                    g.line(
                        p2(c),
                        p2(c + b * stroke / 2.0),
                        thick,
                        layers::CENTER,
                        &owner,
                    );
                }
                // Arrows show the direction of sight: away from the removed side.
                let sight = -removed_side(&p, &r);
                let arms: Vec<(P2, DVec2)> = if *kind == SectionKind::Half {
                    vec![(pts[0], first)]
                } else {
                    vec![(pts[0], -first), (pts[n - 1], last)]
                };
                for (end, out) in arms {
                    let tail = DVec2::from(end) + out * 2.0 - sight * (arrow * 2.5);
                    let tip = tail + sight * (arrow * 2.0);
                    crate::annotations::leader(g, p2(tail), p2(tip), arrow, thin, &owner);
                    let at = tail - sight * (h * 0.8) + out * 1.0;
                    for t in text::layout(&letter, [at.x, at.y - h / 2.0], h, 0.0, HAlign::Center) {
                        g.push(t, thin, layers::NOTES, &owner);
                    }
                }
            }
            ViewKind::Detail { profile, style, .. } => {
                let region = Region::from_profile(profile);
                let outline: Vec<P2> = region
                    .outline()
                    .into_iter()
                    .map(|q| p.to_sheet(DVec2::from(q)))
                    .collect();
                let [lo, hi] = region.bounds();
                let (lo, hi) = (p.to_sheet(DVec2::from(lo)), p.to_sheet(DVec2::from(hi)));
                let label_at = [hi[0] + 1.0, hi[1] + 1.0];
                match style {
                    DetailStyle::Broken => {
                        // Leave a gap in the outline around the letter.
                        let keep = Region::Circle {
                            center: label_at,
                            radius: h * 1.2,
                        };
                        for prim in crate::clip::clip(
                            &Prim::Polyline {
                                points: outline,
                                closed: true,
                            },
                            &keep,
                            false,
                        ) {
                            g.push(prim, thin, layers::VISIBLE, &owner);
                        }
                    }
                    _ => g.push(
                        Prim::Polyline {
                            points: outline,
                            closed: true,
                        },
                        thin,
                        layers::VISIBLE,
                        &owner,
                    ),
                }
                let mut at = label_at;
                match style {
                    DetailStyle::Leader => {
                        at = [hi[0] + 6.0, hi[1] + 6.0];
                        g.line(hi, [at[0] - 1.0, at[1] - 1.0], thin, layers::NOTES, &owner);
                    }
                    DetailStyle::Connected => {
                        if let Ok(bbox) = self.view_box(sheet, v) {
                            let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
                            let d = [
                                (bbox[0][0] + bbox[1][0]) / 2.0,
                                (bbox[0][1] + bbox[1][1]) / 2.0,
                            ];
                            g.line(c, d, thin, layers::NOTES, &owner);
                        }
                    }
                    _ => {}
                }
                for t in text::layout(&letter, at, h, 0.0, HAlign::Left) {
                    g.push(t, thin, layers::NOTES, &owner);
                }
            }
            ViewKind::Auxiliary { .. } if v.label.is_some() => {
                // An arrow beside the parent pointing along the line of sight.
                let sight = -removed_side(&p, &r);
                let Ok(bbox) = self.view_box(
                    sheet,
                    &View {
                        id: parent,
                        ..v.clone()
                    },
                ) else {
                    return;
                };
                let c = DVec2::new(
                    (bbox[0][0] + bbox[1][0]) / 2.0,
                    (bbox[0][1] + bbox[1][1]) / 2.0,
                );
                let half = DVec2::new(bbox[1][0] - bbox[0][0], bbox[1][1] - bbox[0][1]) / 2.0;
                let reach = half.x.abs() * sight.x.abs() + half.y.abs() * sight.y.abs();
                let tip = c - sight * (reach + 3.0);
                let tail = tip - sight * (arrow * 3.0);
                crate::annotations::leader(g, p2(tail), p2(tip), arrow, thin, &owner);
                let at = tail - sight * h;
                for t in text::layout(&letter, [at.x, at.y - h / 2.0], h, 0.0, HAlign::Center) {
                    g.push(t, thin, layers::NOTES, &owner);
                }
            }
            _ => {}
        }
    }

    /// Sheet box a view would have, from the last render's cache of boxes.
    fn view_box(&self, _sheet: &Sheet, v: &View) -> Result<[P2; 2], String> {
        self.boxes
            .get(&v.id)
            .copied()
            .ok_or_else(|| "the view has not been drawn".to_string())
    }
}

/// Place a view-plane primitive on the sheet.
fn place(prim: &Prim, r: &Resolved) -> Prim {
    let at = |p: &P2| r.place(*p);
    match prim {
        Prim::Line { a, b } => Prim::Line { a: at(a), b: at(b) },
        Prim::Arc {
            center,
            radius,
            start,
            end,
        } => Prim::Arc {
            center: at(center),
            radius: radius * r.scale,
            start: *start,
            end: *end,
        },
        Prim::Polyline { points, closed } => Prim::Polyline {
            points: points.iter().map(at).collect(),
            closed: *closed,
        },
        Prim::Fill { loops } => Prim::Fill {
            loops: loops.iter().map(|l| l.iter().map(at).collect()).collect(),
        },
        Prim::Text {
            at: p,
            height,
            angle,
            text,
            width,
        } => Prim::Text {
            at: at(p),
            height: height * r.scale,
            angle: *angle,
            text: text.clone(),
            width: width * r.scale,
        },
    }
}

fn curve_prim(c: &Curve2d) -> Prim {
    match c {
        Curve2d::Line(a, b) => Prim::Line {
            a: p2(*a),
            b: p2(*b),
        },
        Curve2d::Arc {
            center,
            radius,
            start,
            end,
        } => Prim::Arc {
            center: p2(*center),
            radius: *radius,
            start: *start,
            end: *end,
        },
        Curve2d::Polyline(p) => Prim::Polyline {
            points: p.iter().map(|q| p2(*q)).collect(),
            closed: false,
        },
    }
}

fn local_bounds(locals: &[Local]) -> Option<[P2; 2]> {
    let g = Graphics {
        width: 0.0,
        height: 0.0,
        items: locals
            .iter()
            .map(|l| crate::graphics::Item {
                prim: l.prim.clone(),
                pen: l.pen,
                layer: String::new(),
                owner: Owner::Format,
            })
            .collect(),
    };
    g.extents().map(|(a, b)| [a, b])
}

/// Flat-shaded faces, far first: one gray fill per mesh triangle.
fn shading(vs: &ViewShape, r: &Resolved) -> Result<Vec<Local>, String> {
    let linear = (0.3 / r.scale).max(0.01);
    let mesh = vs.shape.mesh(linear, 0.35).map_err(|e| e.to_string())?;
    let pos = |i: u32| {
        let k = i as usize * 3;
        DVec3::new(
            mesh.positions[k] as f64,
            mesh.positions[k + 1] as f64,
            mesh.positions[k + 2] as f64,
        )
    };
    let light = (r.frame.toward * 1.0 + r.frame.y * 0.6 + r.frame.x * 0.3).normalize();
    let mut tris: Vec<(f64, [P2; 3], u8)> = Vec::new();
    for t in mesh.indices.chunks_exact(3) {
        let (a, b, c) = (pos(t[0]), pos(t[1]), pos(t[2]));
        let n = (b - a).cross(c - a);
        let Some(n) = n.try_normalize() else { continue };
        // Back faces are hidden by the front ones.
        if n.dot(r.frame.toward) < -1e-9 {
            continue;
        }
        let depth = (a + b + c).dot(r.frame.toward) / 3.0;
        let shade = (0.45 + 0.5 * n.dot(light).abs()).clamp(0.0, 1.0);
        let level = (110.0 + 140.0 * shade) as u8;
        tris.push((
            depth,
            [
                p2(r.frame.project(a)),
                p2(r.frame.project(b)),
                p2(r.frame.project(c)),
            ],
            level,
        ));
    }
    tris.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(tris
        .into_iter()
        .map(|(_, t, level)| {
            let mut pen = Pen::new(LineStyle::Continuous, 0.05);
            pen.color = [level, level, level];
            Local::new(
                Prim::Fill {
                    loops: vec![t.to_vec()],
                },
                pen,
                "SHADING",
            )
        })
        .collect())
}

/// A break line from `a` to `b` in a style; `size` is the zigzag height.
fn break_line(a: P2, b: P2, style: BreakStyle, size: f64) -> Vec<P2> {
    let (a2, b2) = (DVec2::from(a), DVec2::from(b));
    let len = a2.distance(b2);
    if len < 1e-12 {
        return vec![a, b];
    }
    let d = (b2 - a2) / len;
    let n = d.perp();
    let at = |t: f64, off: f64| p2(a2 + d * t + n * off);
    match style {
        BreakStyle::Straight => vec![a, b],
        BreakStyle::ZigZag | BreakStyle::SmallZigZag => {
            let s = if style == BreakStyle::ZigZag {
                size
            } else {
                size / 2.0
            };
            let m = len / 2.0;
            vec![
                a,
                at(m - s, 0.0),
                at(m - s / 2.0, s),
                at(m + s / 2.0, -s),
                at(m + s, 0.0),
                b,
            ]
        }
        BreakStyle::Curved => {
            // One S-curve along the whole line.
            (0..=32)
                .map(|i| {
                    let t = i as f64 / 32.0;
                    at(t * len, (t * std::f64::consts::TAU).sin() * size / 2.0)
                })
                .collect()
        }
        BreakStyle::Jagged => {
            let steps = ((len / size).ceil() as usize).max(2);
            (0..=steps)
                .map(|i| {
                    let t = i as f64 / steps as f64;
                    // Deterministic jitter.
                    let j = ((i * 7919) % 13) as f64 / 13.0 - 0.5;
                    let off = if i == 0 || i == steps { 0.0 } else { j * size };
                    at(t * len, off)
                })
                .collect()
        }
    }
}

/// A closed outline with small teeth along it (broken detail outlines).
fn jagged(outline: &[P2], size: f64) -> Vec<P2> {
    let mut out = Vec::new();
    let n = outline.len();
    for i in 0..n {
        let (a, b) = (DVec2::from(outline[i]), DVec2::from(outline[(i + 1) % n]));
        let len = a.distance(b);
        let steps = ((len / size).ceil() as usize).max(1);
        let nrm = (b - a).perp().normalize_or_zero();
        for k in 0..steps {
            let t = k as f64 / steps as f64;
            let off = if (i + k) % 2 == 0 {
                size / 3.0
            } else {
                -size / 3.0
            };
            out.push(p2(a + (b - a) * t + nrm * off));
        }
    }
    out
}
