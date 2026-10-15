//! Drafting symbols drawn as strokes and text: geometric tolerance frames
//! (ISO 1101 / ASME Y14.5), weld symbols (ISO 2553 / AWS A2.4), surface
//! texture (ISO 1302), datum features and targets, balloons, note borders and
//! revision clouds. Positions are sheet millimetres.

use of_model::geom::DVec2;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::annotations::{BalloonShape, Border, CloudShape, FinishProcess, TargetKind};
use crate::dims::p;
use crate::graphics::{Graphics, HAlign, LineStyle, Owner, P2, Pen, Prim, arc_points};
use crate::standard::{Standard, StandardBase};
use crate::text;

/// Geometric characteristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Characteristic {
    Straightness,
    Flatness,
    Circularity,
    Cylindricity,
    LineProfile,
    SurfaceProfile,
    Angularity,
    Perpendicularity,
    Parallelism,
    Position,
    Concentricity,
    Symmetry,
    CircularRunout,
    TotalRunout,
}

impl Characteristic {
    pub fn symbol(&self) -> char {
        use Characteristic::*;
        match self {
            Straightness => '⏤',
            Flatness => '⏥',
            Circularity => '○',
            Cylindricity => '⌭',
            LineProfile => '⌒',
            SurfaceProfile => '⌓',
            Angularity => '∠',
            Perpendicularity => '⟂',
            Parallelism => '∥',
            Position => '⌖',
            Concentricity => '◎',
            Symmetry => '⌯',
            CircularRunout => '↗',
            TotalRunout => '⌰',
        }
    }
}

/// Material condition and other modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    /// Maximum material condition Ⓜ.
    Mmc,
    /// Least material condition Ⓛ.
    Lmc,
    /// Projected tolerance zone Ⓟ.
    Projected,
    /// Free state Ⓕ.
    FreeState,
    /// Tangent plane Ⓣ.
    TangentPlane,
    /// Regardless of feature size Ⓢ.
    Rfs,
    /// Unequally disposed profile Ⓤ.
    Unequal,
}

impl Modifier {
    pub fn symbol(&self) -> char {
        match self {
            Modifier::Mmc => 'Ⓜ',
            Modifier::Lmc => 'Ⓛ',
            Modifier::Projected => 'Ⓟ',
            Modifier::FreeState => 'Ⓕ',
            Modifier::TangentPlane => 'Ⓣ',
            Modifier::Rfs => 'Ⓢ',
            Modifier::Unequal => 'Ⓤ',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DatumRef {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<Modifier>,
}

/// One row of a feature control frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GtolRow {
    pub characteristic: Characteristic,
    /// Tolerance value text, e.g. `0.05`.
    pub tolerance: String,
    /// Cylindrical zone: `⌀` before the value.
    #[serde(default)]
    pub diameter: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<Modifier>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub datums: Vec<DatumRef>,
}

/// Weld symbols on one side of the reference line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WeldSymbol {
    Fillet,
    Square,
    V,
    Bevel,
    U,
    J,
    FlareV,
    FlareBevel,
    Plug,
    Spot,
    Seam,
    Backing,
    Surfacing,
    Edge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Contour {
    #[default]
    None,
    Flat,
    Convex,
    Concave,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeldSide {
    pub symbol: WeldSymbol,
    /// Size (or throat) before the symbol, e.g. `a5` or `6`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub size: String,
    /// Length and pitch after it, e.g. `50-100`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub length: String,
    #[serde(default)]
    pub contour: Contour,
}

/// A stroking context.
/// The requirement shown by a surface texture symbol.
pub struct Finish<'a> {
    pub process: FinishProcess,
    pub roughness: &'a str,
    pub method: &'a str,
    pub lay: &'a str,
    pub all_around: bool,
}

pub struct Ink<'a> {
    pub g: &'a mut Graphics,
    pub std: Standard,
    pub owner: Owner,
    pub layer: String,
}

impl Ink<'_> {
    pub fn pen(&self) -> Pen {
        Pen::new(LineStyle::Continuous, self.std.thin_line())
    }

    pub fn push(&mut self, prim: Prim) {
        let pen = self.pen();
        let layer = self.layer.clone();
        self.g.push(prim, pen, &layer, &self.owner);
    }

    pub fn styled(&mut self, prim: Prim, style: LineStyle) {
        let pen = Pen::new(style, self.std.thin_line());
        let layer = self.layer.clone();
        self.g.push(prim, pen, &layer, &self.owner);
    }

    pub fn line(&mut self, a: P2, b: P2) {
        self.push(Prim::Line { a, b });
    }

    pub fn poly(&mut self, pts: Vec<P2>, closed: bool) {
        self.push(Prim::Polyline {
            points: pts,
            closed,
        });
    }

    pub fn circle(&mut self, c: P2, r: f64) {
        self.push(Prim::Arc {
            center: c,
            radius: r,
            start: 0.0,
            end: std::f64::consts::TAU,
        });
    }

    pub fn fill(&mut self, pts: Vec<P2>) {
        self.push(Prim::Fill { loops: vec![pts] });
    }

    /// Text centered on `c` (both ways).
    pub fn text_mid(&mut self, s: &str, c: P2, h: f64) {
        for prim in text::layout(s, [c[0], c[1] - h / 2.0], h, 0.0, HAlign::Center) {
            self.push(prim);
        }
    }

    pub fn text_at(&mut self, s: &str, at: P2, h: f64, align: HAlign) {
        for prim in text::layout(s, at, h, 0.0, align) {
            self.push(prim);
        }
    }

    /// Leader from `from` to `to`, ending in an arrow (or a dot).
    pub fn leader(&mut self, from: P2, to: P2, dot: bool) {
        let (a, b) = (DVec2::from(from), DVec2::from(to));
        let l = self.std.arrow_length();
        let d = (b - a).normalize_or(DVec2::X);
        if dot {
            self.line(from, to);
            self.fill(arc_points(to, l / 4.0, 0.0, std::f64::consts::TAU, 0.3));
        } else {
            let base = b - d * l;
            self.line(from, p(base));
            let n = d.perp() * (l / 6.0);
            self.fill(vec![to, p(base + n), p(base - n)]);
        }
    }

    /// Note border around a text block whose box is `lo`..`hi`.
    pub fn border(&mut self, border: Border, lo: P2, hi: P2) {
        let m = 1.0;
        match border {
            Border::None => {}
            Border::Box => self.poly(
                vec![
                    [lo[0] - m, lo[1] - m],
                    [hi[0] + m, lo[1] - m],
                    [hi[0] + m, hi[1] + m],
                    [lo[0] - m, hi[1] + m],
                ],
                true,
            ),
            Border::Circle => {
                let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
                let r = ((hi[0] - lo[0]).hypot(hi[1] - lo[1])) / 2.0 + m;
                self.circle(c, r);
            }
        }
    }

    /// A balloon shape around `c`; returns its outline radius toward `dir`
    /// (to start the leader on the border).
    pub fn balloon(
        &mut self,
        shape: BalloonShape,
        c: P2,
        size: f64,
        upper: &str,
        lower: Option<&str>,
    ) -> f64 {
        let r = size / 2.0;
        let h = self.std.text_height();
        let ngon = |n: usize, rot: f64| -> Vec<P2> {
            (0..n)
                .map(|i| {
                    let a = rot + std::f64::consts::TAU * i as f64 / n as f64;
                    [c[0] + r * a.cos(), c[1] + r * a.sin()]
                })
                .collect()
        };
        match shape {
            BalloonShape::Circle => self.circle(c, r),
            BalloonShape::SplitCircle => {
                self.circle(c, r);
                self.line([c[0] - r, c[1]], [c[0] + r, c[1]]);
            }
            BalloonShape::Triangle => self.poly(ngon(3, std::f64::consts::FRAC_PI_2), true),
            BalloonShape::Hexagon => self.poly(ngon(6, 0.0), true),
            BalloonShape::Diamond => self.poly(ngon(4, 0.0), true),
            BalloonShape::Box => self.poly(ngon(4, std::f64::consts::FRAC_PI_4), true),
            BalloonShape::Flag => self.poly(
                vec![
                    [c[0] - r, c[1] - r * 0.7],
                    [c[0] + r * 0.6, c[1] - r * 0.7],
                    [c[0] + r, c[1]],
                    [c[0] + r * 0.6, c[1] + r * 0.7],
                    [c[0] - r, c[1] + r * 0.7],
                ],
                true,
            ),
            BalloonShape::None => {}
        }
        match (shape, lower) {
            (BalloonShape::SplitCircle, Some(l)) => {
                self.text_mid(upper, [c[0], c[1] + r * 0.45], h * 0.8);
                self.text_mid(l, [c[0], c[1] - r * 0.45], h * 0.8);
            }
            _ => {
                let fit = (text::line_width(upper, h)).max(1e-9);
                let hh = if fit > size * 0.8 {
                    h * size * 0.8 / fit
                } else {
                    h
                };
                self.text_mid(upper, c, hh);
            }
        }
        r
    }

    /// A feature control frame with its top-left at `at`. Returns the
    /// frame's box.
    pub fn gtol(&mut self, rows: &[GtolRow], at: P2, above: &str) -> [P2; 2] {
        let h = self.std.text_height();
        let row_h = h * 2.0;
        let pad = h * 0.5;
        let width = |s: &str| text::line_width(s, h) + 2.0 * pad;
        let cells: Vec<Vec<String>> = rows
            .iter()
            .map(|r| {
                let mut v = vec![r.characteristic.symbol().to_string()];
                let mut tol = String::new();
                if r.diameter {
                    tol.push('⌀');
                }
                tol += &r.tolerance;
                for m in &r.modifiers {
                    tol.push(m.symbol());
                }
                v.push(tol);
                for d in &r.datums {
                    let mut s = d.label.clone();
                    if let Some(m) = d.modifier {
                        s.push(m.symbol());
                    }
                    v.push(s);
                }
                v
            })
            .collect();
        let ncol = cells.iter().map(Vec::len).max().unwrap_or(1);
        // Column widths shared across rows (the characteristic column is square).
        let mut widths = vec![row_h; ncol];
        for row in &cells {
            for (i, c) in row.iter().enumerate().skip(1) {
                widths[i] = widths[i].max(width(c));
            }
        }
        let composite = rows.len() > 1
            && rows
                .iter()
                .all(|r| r.characteristic == rows[0].characteristic);
        let x0 = at[0];
        let top = at[1];
        let mut right_most = x0;
        for (ri, row) in cells.iter().enumerate() {
            let y1 = top - row_h * ri as f64;
            let y0 = y1 - row_h;
            let mut x = x0;
            for (ci, c) in row.iter().enumerate() {
                let w = widths[ci];
                let merged = composite && ci == 0;
                if !(merged && ri > 0) {
                    let ybot = if merged {
                        top - row_h * rows.len() as f64
                    } else {
                        y0
                    };
                    self.poly(vec![[x, ybot], [x + w, ybot], [x + w, y1], [x, y1]], true);
                    self.text_mid(c, [x + w / 2.0, (ybot + y1) / 2.0], h);
                } else {
                    // Under a merged cell: nothing to draw.
                }
                x += w;
            }
            right_most = right_most.max(x);
        }
        if !above.is_empty() {
            self.text_at(above, [x0, top + h * 0.6], h, HAlign::Left);
        }
        [[x0, top - row_h * rows.len() as f64], [right_most, top]]
    }

    /// Surface texture symbol with its vertex at `tip`, legs up, turned by
    /// `angle`.
    pub fn surface_finish(&mut self, tip: P2, finish: &Finish, angle: f64) {
        let Finish {
            process,
            roughness,
            method,
            lay,
            all_around,
        } = *finish;
        let h = self.std.text_height();
        let (s, c) = angle.sin_cos();
        let t = DVec2::from(tip);
        let at = |x: f64, y: f64| p(t + DVec2::new(x * c - y * s, x * s + y * c));
        let short = h * 1.4;
        let long = h * 3.0;
        let a = 30f64.to_radians();
        let left = [-short * a.sin() * 2.0 / 2.0, short * a.cos()];
        let right = [long * a.sin(), long * a.cos()];
        self.poly(
            vec![at(left[0], left[1]), at(0.0, 0.0), at(right[0], right[1])],
            false,
        );
        match process {
            FinishProcess::Any => {}
            FinishProcess::Removal => self.line(
                at(left[0], left[1]),
                at(
                    left[0] + (right[0] - left[0]) * (short / long) * 1.5,
                    left[1],
                ),
            ),
            FinishProcess::NoRemoval => {
                let r = h * 0.4;
                let cpos = at(0.0, r * 1.9);
                self.circle(cpos, r);
            }
        }
        let text_w = text::line_width(roughness, h)
            .max(text::line_width(method, h))
            .max(h);
        let shelf_end = [right[0] + text_w + 1.0, right[1]];
        if !roughness.is_empty() || !method.is_empty() {
            self.line(at(right[0], right[1]), at(shelf_end[0], shelf_end[1]));
        }
        if all_around {
            self.circle(at(right[0], right[1]), h * 0.35);
        }
        let place = |x: f64, y: f64| at(x, y);
        if !method.is_empty() {
            let q = place(right[0] + 0.5, right[1] + 0.6);
            for prim in text::layout(method, q, h, angle, HAlign::Left) {
                self.push(prim);
            }
        }
        if !roughness.is_empty() {
            let q = place(right[0] + 0.5, right[1] - h - 0.6);
            for prim in text::layout(roughness, q, h, angle, HAlign::Left) {
                self.push(prim);
            }
        }
        if !lay.is_empty() {
            let q = place(right[0] - h * 0.2, h * 0.1);
            for prim in text::layout(lay, q, h * 0.8, angle, HAlign::Left) {
                self.push(prim);
            }
        }
    }

    /// Datum feature symbol: a framed letter at `at` joined to a filled
    /// triangle on the feature at `on`.
    pub fn datum_feature(&mut self, label: &str, at: P2, on: P2) {
        let h = self.std.text_height();
        let s = h * 2.0;
        let box_lo = [at[0] - s / 2.0, at[1] - s / 2.0];
        self.poly(
            vec![
                box_lo,
                [box_lo[0] + s, box_lo[1]],
                [box_lo[0] + s, box_lo[1] + s],
                [box_lo[0], box_lo[1] + s],
            ],
            true,
        );
        self.text_mid(label, at, h);
        let (a, b) = (DVec2::from(at), DVec2::from(on));
        let d = (b - a).normalize_or(DVec2::NEG_Y);
        // Leave the box on its side facing the feature.
        let start = a + d * (s / 2.0 / d.x.abs().max(d.y.abs()).max(1e-9));
        let tri = self.std.arrow_length();
        let base = b - d * tri;
        self.line(p(start), p(base));
        let n = d.perp() * (tri * 0.6);
        // The triangle's base sits on the feature.
        self.fill(vec![p(b + n), p(b - n), p(base)]);
    }

    /// Datum target: a split circle with size over label, a leader to the
    /// target and its point, line or area marker.
    pub fn datum_target(&mut self, label: &str, size: &str, at: P2, on: P2, kind: TargetKind) {
        let h = self.std.text_height();
        let r = h * 1.8;
        self.circle(at, r);
        self.line([at[0] - r, at[1]], [at[0] + r, at[1]]);
        if !size.is_empty() {
            self.text_mid(size, [at[0], at[1] + r * 0.45], h * 0.8);
        }
        self.text_mid(label, [at[0], at[1] - r * 0.45], h * 0.8);
        let (a, b) = (DVec2::from(at), DVec2::from(on));
        let d = (b - a).normalize_or(DVec2::X);
        self.line(p(a + d * r), on);
        let m = h * 0.6;
        match kind {
            TargetKind::Point => {
                self.line([on[0] - m, on[1] - m], [on[0] + m, on[1] + m]);
                self.line([on[0] - m, on[1] + m], [on[0] + m, on[1] - m]);
            }
            TargetKind::Line => {
                let pen = Pen::new(LineStyle::Phantom, self.std.thick_line());
                let layer = self.layer.clone();
                self.g.line(
                    [on[0] - 3.0 * m, on[1]],
                    [on[0] + 3.0 * m, on[1]],
                    pen,
                    &layer,
                    &self.owner,
                );
            }
            TargetKind::Area => {
                let size_mm: f64 = size
                    .trim_start_matches('⌀')
                    .trim()
                    .parse()
                    .unwrap_or(m * 4.0);
                let rr = size_mm / 2.0;
                let ring = arc_points(on, rr, 0.0, std::f64::consts::TAU, 0.2);
                self.styled(
                    Prim::Polyline {
                        points: ring.clone(),
                        closed: true,
                    },
                    LineStyle::Phantom,
                );
                for [a, b] in crate::hatch::lines(&[ring], crate::hatch::Pattern::General, 0.5, 0.0)
                {
                    self.line(a, b);
                }
            }
        }
    }

    /// A weld symbol: arrow to `on`, reference line from the arrow's elbow
    /// at `at`. Arrow-side symbols go below the line, other-side above
    /// (ISO adds a dashed identification line on the other side).
    #[allow(clippy::too_many_arguments)]
    pub fn weld(
        &mut self,
        at: P2,
        on: P2,
        arrow: Option<&WeldSide>,
        other: Option<&WeldSide>,
        all_around: bool,
        field: bool,
        tail: &str,
    ) {
        let h = self.std.text_height();
        let len = h * 8.0;
        let right = on[0] <= at[0];
        let dir = if right { 1.0 } else { -1.0 };
        let end = [at[0] + dir * len, at[1]];
        self.leader(at, on, false);
        self.line(at, end);
        if self.std.base == StandardBase::Iso && other.is_some() {
            let pen = Pen::new(LineStyle::Hidden, self.std.thin_line());
            let layer = self.layer.clone();
            self.g.line(
                [at[0], at[1] + h * 0.6],
                [end[0], end[1] + h * 0.6],
                pen,
                &layer,
                &self.owner,
            );
        }
        let mid = [at[0] + dir * len * 0.45, at[1]];
        for (side, up) in [(arrow, -1.0), (other, 1.0)] {
            let Some(w) = side else { continue };
            let base = [
                mid[0],
                mid[1]
                    + if up > 0.0 && self.std.base == StandardBase::Iso {
                        h * 0.6
                    } else {
                        0.0
                    },
            ];
            self.weld_symbol(w.symbol, base, up, h * 1.6);
            let y = base[1] + up * h * 0.9 - if up < 0.0 { h } else { 0.0 } * 0.0;
            if !w.size.is_empty() {
                let x = base[0] - h * 1.2 - text::line_width(&w.size, h);
                self.text_at(
                    &w.size,
                    [x, y + if up < 0.0 { -h * 0.6 } else { -h * 0.3 }],
                    h,
                    HAlign::Left,
                );
            }
            if !w.length.is_empty() {
                let x = base[0] + h * 1.2;
                self.text_at(
                    &w.length,
                    [x, y + if up < 0.0 { -h * 0.6 } else { -h * 0.3 }],
                    h,
                    HAlign::Left,
                );
            }
            let cy = base[1] + up * h * 2.0;
            match w.contour {
                Contour::None => {}
                Contour::Flat => self.line([base[0] - h * 0.8, cy], [base[0] + h * 0.8, cy]),
                Contour::Convex | Contour::Concave => {
                    let bulge = if (w.contour == Contour::Convex) == (up > 0.0) {
                        0.0
                    } else {
                        std::f64::consts::PI
                    };
                    self.push(Prim::Arc {
                        center: [base[0], cy - up * h * 0.6],
                        radius: h,
                        start: bulge + 0.6,
                        end: bulge + std::f64::consts::PI - 0.6,
                    });
                }
            }
        }
        if all_around {
            self.circle(at, h * 0.5);
        }
        if field {
            self.line(at, [at[0], at[1] + h * 2.2]);
            self.fill(vec![
                [at[0], at[1] + h * 2.2],
                [at[0] + dir * h * 1.2, at[1] + h * 1.8],
                [at[0], at[1] + h * 1.4],
            ]);
        }
        if !tail.is_empty() {
            let tl = h * 1.2;
            self.line(end, [end[0] + dir * tl, end[1] + tl]);
            self.line(end, [end[0] + dir * tl, end[1] - tl]);
            let x = end[0] + dir * (tl + 0.8);
            self.text_at(
                tail,
                [x, end[1] - h / 2.0],
                h,
                if right { HAlign::Left } else { HAlign::Right },
            );
        }
    }

    /// One weld symbol on the reference line at `base`, toward `up` (±1).
    fn weld_symbol(&mut self, sym: WeldSymbol, base: P2, up: f64, s: f64) {
        let (x, y) = (base[0], base[1]);
        let q = |dx: f64, dy: f64| [x + dx * s, y + up * dy * s];
        match sym {
            WeldSymbol::Fillet => self.poly(vec![q(-0.4, 0.0), q(-0.4, 0.8), q(0.4, 0.0)], false),
            WeldSymbol::Square => {
                self.line(q(-0.2, 0.0), q(-0.2, 0.8));
                self.line(q(0.2, 0.0), q(0.2, 0.8));
            }
            WeldSymbol::V => self.poly(vec![q(-0.45, 0.8), q(0.0, 0.0), q(0.45, 0.8)], false),
            WeldSymbol::Bevel => {
                self.line(q(-0.2, 0.0), q(-0.2, 0.8));
                self.line(q(-0.2, 0.0), q(0.4, 0.8));
            }
            WeldSymbol::U => {
                self.line(q(0.0, 0.0), q(0.0, 0.3));
                self.push(Prim::Arc {
                    center: q(0.0, 0.6),
                    radius: 0.3 * s,
                    start: if up > 0.0 { std::f64::consts::PI } else { 0.0 },
                    end: if up > 0.0 {
                        std::f64::consts::TAU
                    } else {
                        std::f64::consts::PI
                    },
                });
                self.line(q(-0.3, 0.6), q(-0.3, 0.9));
                self.line(q(0.3, 0.6), q(0.3, 0.9));
            }
            WeldSymbol::J => {
                self.line(q(-0.2, 0.0), q(-0.2, 0.9));
                self.push(Prim::Arc {
                    center: q(0.1, 0.6),
                    radius: 0.3 * s,
                    start: if up > 0.0 {
                        std::f64::consts::PI
                    } else {
                        std::f64::consts::FRAC_PI_2
                    },
                    end: if up > 0.0 {
                        std::f64::consts::PI * 1.5
                    } else {
                        std::f64::consts::PI
                    },
                });
            }
            WeldSymbol::FlareV => {
                for side in [-1.0, 1.0] {
                    self.push(Prim::Arc {
                        center: q(side * 0.6, 0.8),
                        radius: 0.5 * s,
                        start: 0.0,
                        end: std::f64::consts::TAU,
                    });
                }
            }
            WeldSymbol::FlareBevel => {
                self.line(q(-0.3, 0.0), q(-0.3, 0.8));
                self.push(Prim::Arc {
                    center: q(0.5, 0.8),
                    radius: 0.5 * s,
                    start: 0.0,
                    end: std::f64::consts::TAU,
                });
            }
            WeldSymbol::Plug => self.poly(
                vec![q(-0.4, 0.0), q(-0.4, 0.5), q(0.4, 0.5), q(0.4, 0.0)],
                false,
            ),
            WeldSymbol::Spot => self.circle(q(0.0, 0.0), 0.35 * s),
            WeldSymbol::Seam => {
                self.circle(q(0.0, 0.0), 0.35 * s);
                self.line(q(-0.5, 0.1), q(0.5, 0.1));
                self.line(q(-0.5, -0.1), q(0.5, -0.1));
            }
            WeldSymbol::Backing => self.push(Prim::Arc {
                center: q(0.0, 0.0),
                radius: 0.35 * s,
                start: if up > 0.0 { 0.0 } else { std::f64::consts::PI },
                end: if up > 0.0 {
                    std::f64::consts::PI
                } else {
                    std::f64::consts::TAU
                },
            }),
            WeldSymbol::Surfacing => {
                for dx in [-0.3, 0.3] {
                    self.push(Prim::Arc {
                        center: q(dx, 0.0),
                        radius: 0.3 * s,
                        start: if up > 0.0 { 0.0 } else { std::f64::consts::PI },
                        end: if up > 0.0 {
                            std::f64::consts::PI
                        } else {
                            std::f64::consts::TAU
                        },
                    });
                }
            }
            WeldSymbol::Edge => {
                self.line(q(-0.3, 0.0), q(-0.3, 0.8));
                self.line(q(0.3, 0.0), q(0.3, 0.8));
                self.line(q(-0.3, 0.8), q(0.3, 0.8));
            }
        }
    }

    /// A revision cloud along a shape's outline.
    pub fn cloud(&mut self, shape: &CloudShape, arc: f64) -> P2 {
        let outline: Vec<P2> = match shape {
            CloudShape::Rectangle { a, b } => vec![*a, [b[0], a[1]], *b, [a[0], b[1]]],
            CloudShape::Ellipse { center, rx, ry } => (0..64)
                .map(|i| {
                    let t = std::f64::consts::TAU * i as f64 / 64.0;
                    [center[0] + rx * t.cos(), center[1] + ry * t.sin()]
                })
                .collect(),
            CloudShape::Polygon { points } | CloudShape::Freehand { points } => points.clone(),
        };
        // Bulge outward: walk counter-clockwise, arcs on the right.
        let ccw = crate::clip::signed_area(&outline) > 0.0;
        let n = outline.len();
        let mut corner = outline.first().copied().unwrap_or([0.0, 0.0]);
        for i in 0..n {
            let (a, b) = (DVec2::from(outline[i]), DVec2::from(outline[(i + 1) % n]));
            let len = a.distance(b);
            let k = ((len / arc).round() as usize).max(1);
            for j in 0..k {
                let s = a + (b - a) * (j as f64 / k as f64);
                let e = a + (b - a) * ((j + 1) as f64 / k as f64);
                let c = (s + e) / 2.0;
                let r = s.distance(e) / 2.0;
                let d = e - s;
                let base = d.y.atan2(d.x);
                // Outside is to the right of a counter-clockwise outline.
                let (start, end) = if ccw {
                    (base - std::f64::consts::PI, base)
                } else {
                    (base, base + std::f64::consts::PI)
                };
                self.push(Prim::Arc {
                    center: p(c),
                    radius: r,
                    start,
                    end,
                });
            }
            if outline[i][0] > corner[0]
                || (outline[i][0] == corner[0] && outline[i][1] > corner[1])
            {
                corner = outline[i];
            }
        }
        corner
    }

    /// A revision letter in a triangle centered at `c`.
    pub fn revision(&mut self, c: P2, rev: &str) {
        let h = self.std.text_height();
        let r = h * 1.3;
        let pts: Vec<P2> = (0..3)
            .map(|i| {
                let a = std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / 3.0;
                [c[0] + r * a.cos(), c[1] - r * 0.25 + r * a.sin()]
            })
            .collect();
        self.poly(pts, true);
        self.text_mid(rev, [c[0], c[1] - r * 0.3], h * 0.8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(g: &mut Graphics) -> Ink<'_> {
        Ink {
            g,
            std: Standard::iso(),
            owner: Owner::Annotation { id: 1 },
            layer: "NOTES".into(),
        }
    }

    #[test]
    fn composite_frames_share_the_characteristic_cell() {
        let mut g = Graphics::new(100.0, 100.0);
        let rows = vec![
            GtolRow {
                characteristic: Characteristic::Position,
                tolerance: "0.2".into(),
                diameter: true,
                modifiers: vec![Modifier::Mmc],
                datums: vec![
                    DatumRef {
                        label: "A".into(),
                        modifier: None,
                    },
                    DatumRef {
                        label: "B".into(),
                        modifier: Some(Modifier::Mmc),
                    },
                ],
            },
            GtolRow {
                characteristic: Characteristic::Position,
                tolerance: "0.05".into(),
                diameter: true,
                modifiers: Vec::new(),
                datums: vec![DatumRef {
                    label: "A".into(),
                    modifier: None,
                }],
            },
        ];
        let b = ink(&mut g).gtol(&rows, [10.0, 50.0], "4X");
        assert!((b[1][1] - b[0][1] - 14.0).abs() < 1e-9, "two rows of 7 mm");
        let cells = g
            .items
            .iter()
            .filter(|i| matches!(i.prim, Prim::Polyline { closed: true, .. }))
            .count();
        // Row 1: characteristic, tolerance, A, B; row 2: tolerance, A.
        assert_eq!(cells, 6);
        let texts: Vec<String> = g
            .items
            .iter()
            .filter_map(|i| match &i.prim {
                Prim::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t.contains("0.2")));
        assert!(texts.contains(&"4X".to_string()));
    }

    #[test]
    fn symbols_draw_something() {
        let mut g = Graphics::new(100.0, 100.0);
        let mut k = ink(&mut g);
        k.surface_finish(
            [10.0, 10.0],
            &Finish {
                process: FinishProcess::Removal,
                roughness: "Ra 1.6",
                method: "GROUND",
                lay: "⊥",
                all_around: true,
            },
            0.0,
        );
        k.datum_feature("A", [30.0, 30.0], [30.0, 10.0]);
        k.datum_target("A1", "⌀6", [50.0, 30.0], [50.0, 10.0], TargetKind::Area);
        let side = WeldSide {
            symbol: WeldSymbol::Fillet,
            size: "a5".into(),
            length: "50".into(),
            contour: Contour::Flat,
        };
        k.weld(
            [70.0, 40.0],
            [60.0, 20.0],
            Some(&side),
            Some(&side),
            true,
            true,
            "111",
        );
        k.cloud(
            &CloudShape::Rectangle {
                a: [10.0, 60.0],
                b: [40.0, 80.0],
            },
            6.0,
        );
        k.revision([45.0, 82.0], "B");
        k.balloon(
            BalloonShape::SplitCircle,
            [80.0, 80.0],
            10.0,
            "3",
            Some("2"),
        );
        assert!(g.items.len() > 30);
    }
}
