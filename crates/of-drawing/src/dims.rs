//! Dimensions: what they measure (model geometry through view references,
//! or a model parameter for imported, driving dimensions), their value text
//! with tolerances, fits and dual units, and their drafting per ISO 129 /
//! ASME Y14.5: extension lines, dimension lines, arrows and text.

use of_model::geom::{DVec2, DVec3};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::document::ViewRef;
use crate::graphics::{Graphics, HAlign, LineStyle, Owner, P2, Pen, Prim};
use crate::locate::Geo;
use crate::standard::Standard;
use crate::text;

/// Which point of an edge a dimension uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Which {
    /// The whole edge (a line's length, a circle's center).
    #[default]
    Auto,
    Start,
    End,
    Mid,
    Center,
}

/// Model geometry picked in a view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pick {
    #[serde(flatten)]
    pub target: ViewRef,
    #[serde(default, skip_serializing_if = "is_auto")]
    pub point: Which,
    /// Where the user picked, in view-plane mm (model mm relative to the
    /// model origin): with `point` auto, the edge's end, middle or center
    /// nearest this point is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub near: Option<[f64; 2]>,
}

fn is_auto(w: &Which) -> bool {
    *w == Which::Auto
}

/// Direction a linear dimension measures in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Orient {
    /// Picked from where the text is placed.
    #[default]
    Auto,
    Horizontal,
    Vertical,
    /// Along the line between the two points.
    Aligned,
}

/// What a dimension measures. Tagged `dim` because a dimension annotation's
/// own `type` is `dimension`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "dim")]
pub enum DimKind {
    /// Distance between two picks, or a line's length (one pick).
    Linear {
        a: Pick,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        b: Option<Pick>,
        #[serde(default)]
        orientation: Orient,
    },
    /// Angle between two straight edges.
    Angular {
        a: Pick,
        b: Pick,
    },
    Radius {
        target: Pick,
    },
    Diameter {
        target: Pick,
    },
    ArcLength {
        target: Pick,
    },
    /// Chamfer: the chamfer edge and an edge next to it; `d × angle`.
    Chamfer {
        edge: Pick,
        adjacent: Pick,
    },
    /// Ordinate values of points from an origin, along paper x (or y).
    Ordinate {
        origin: Pick,
        points: Vec<Pick>,
        #[serde(default)]
        vertical: bool,
    },
    /// Dimensions from a common base, stacked `spacing` mm apart.
    Baseline {
        base: Pick,
        points: Vec<Pick>,
        #[serde(default)]
        orientation: Orient,
        #[serde(default = "spacing")]
        spacing: f64,
    },
    /// Consecutive dimensions on one line.
    Chain {
        #[schemars(length(min = 2))]
        points: Vec<Pick>,
        #[serde(default)]
        orientation: Orient,
    },
    /// Angles of straight edges from a base edge.
    AngularRunning {
        base: Pick,
        lines: Vec<Pick>,
    },
    /// An imported model dimension: the part parameter `key` shown in a view.
    Model {
        view: u32,
        key: String,
    },
}

fn spacing() -> f64 {
    8.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FitDisplay {
    /// `⌀20 H7/g6`.
    #[default]
    Classes,
    /// `⌀20 H7/g6` with the limit deviations.
    ClassesWithTolerance,
    /// Only the limit deviations.
    ToleranceOnly,
}

/// Tolerance of a dimension; values in mm (or degrees for angles).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Tolerance {
    #[default]
    None,
    /// Theoretically exact: the value in a box.
    Basic,
    /// `+plus` over `-minus` beside the value.
    Bilateral { plus: f64, minus: f64 },
    /// Upper and lower limits instead of the value.
    Limit { plus: f64, minus: f64 },
    /// `±value`.
    Symmetric { value: f64 },
    /// `MIN` after the value.
    Min,
    /// `MAX` after the value.
    Max,
    /// ISO 286 classes; a hole class, a shaft class or both.
    Fit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hole: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shaft: Option<String>,
        #[serde(default)]
        display: FitDisplay,
    },
    /// Covered by the general tolerance table.
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Arrows {
    #[default]
    Auto,
    Inside,
    Outside,
}

/// A dimension on a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Dimension {
    #[serde(flatten)]
    pub kind: DimKind,
    /// Where the value text goes, sheet mm: relative to the annotation's
    /// view when it has one; for imported model dimensions, relative to the
    /// middle of what they measure (so they follow model changes). The
    /// dimension line passes through it.
    pub at: [f64; 2],
    #[serde(default)]
    pub tolerance: Tolerance,
    /// Decimal places of the value; the standard's when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance_precision: Option<u8>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub suffix: String,
    /// A second line of text under the value.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub below: String,
    /// Replaces the value; `<DIM>` stands for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// A reference dimension: value in parentheses.
    #[serde(default)]
    pub parentheses: bool,
    /// An inspection dimension: value in a rounded frame.
    #[serde(default)]
    pub inspection: bool,
    /// Also show the value in inches; the standard decides when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dual: Option<bool>,
    /// A radius whose center is off the sheet: jogged dimension line.
    #[serde(default)]
    pub foreshortened: bool,
    #[serde(default)]
    pub arrows: Arrows,
}

impl Dimension {
    pub fn new(kind: DimKind, at: [f64; 2]) -> Dimension {
        Dimension {
            kind,
            at,
            tolerance: Tolerance::None,
            precision: None,
            tolerance_precision: None,
            prefix: String::new(),
            suffix: String::new(),
            below: String::new(),
            text: None,
            parentheses: false,
            inspection: false,
            dual: None,
            foreshortened: false,
            arrows: Arrows::Auto,
        }
    }

    /// Views whose geometry this dimension measures.
    pub fn views(&self) -> Vec<u32> {
        let picks: Vec<&Pick> = match &self.kind {
            DimKind::Linear { a, b, .. } => std::iter::once(a).chain(b.iter()).collect(),
            DimKind::Angular { a, b } => vec![a, b],
            DimKind::Radius { target }
            | DimKind::Diameter { target }
            | DimKind::ArcLength { target } => vec![target],
            DimKind::Chamfer { edge, adjacent } => vec![edge, adjacent],
            DimKind::Ordinate { origin, points, .. } => {
                std::iter::once(origin).chain(points).collect()
            }
            DimKind::Baseline { base, points, .. } => std::iter::once(base).chain(points).collect(),
            DimKind::Chain { points, .. } => points.iter().collect(),
            DimKind::AngularRunning { base, lines } => std::iter::once(base).chain(lines).collect(),
            DimKind::Model { view, .. } => return vec![*view],
        };
        let mut v: Vec<u32> = picks.iter().map(|p| p.target.view).collect();
        v.dedup();
        v
    }

    /// The parameter an imported dimension drives.
    pub fn driving(&self) -> Option<&str> {
        match &self.kind {
            DimKind::Model { key, .. } => Some(key),
            _ => None,
        }
    }
}

/// What is measured, in model space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Measure {
    /// Distance from `a` to `b` along `dir` (unit, model space).
    Linear { a: DVec3, b: DVec3, dir: DVec3 },
    /// A circle; `diameter` dimensions show twice the radius.
    Radial {
        center: DVec3,
        radius: f64,
        axis: DVec3,
        diameter: bool,
    },
    /// The angle at `vertex` from `d1` to `d2` (unit directions).
    Angle { vertex: DVec3, d1: DVec3, d2: DVec3 },
    /// Arc from `start` to `end` about `center`.
    Arc {
        center: DVec3,
        radius: f64,
        start: DVec3,
        end: DVec3,
    },
}

impl Measure {
    /// The measured value: mm, or degrees for angles.
    pub fn value(&self) -> f64 {
        match *self {
            Measure::Linear { a, b, dir } => (b - a).dot(dir).abs(),
            Measure::Radial {
                radius, diameter, ..
            } => {
                if diameter {
                    radius * 2.0
                } else {
                    radius
                }
            }
            Measure::Angle { d1, d2, .. } => d1.angle_between(d2).to_degrees(),
            Measure::Arc {
                center,
                radius,
                start,
                end,
            } => (start - center).angle_between(end - center) * radius,
        }
    }
}

/// A point of a pick's geometry.
pub fn point_of(g: &Geo, w: Which) -> DVec3 {
    match (g, w) {
        (Geo::Line(a, _), Which::Start) => *a,
        (Geo::Line(_, b), Which::End) => *b,
        (
            Geo::Circle {
                ends: Some((a, _)), ..
            },
            Which::Start,
        ) => *a,
        (
            Geo::Circle {
                ends: Some((_, b)), ..
            },
            Which::End,
        ) => *b,
        (Geo::Circle { center, .. }, _) => *center,
        _ => g.anchor(),
    }
}

/// The text of a value with tolerance, prefix and suffix: main line and an
/// optional stacked pair (upper, lower) set beside it.
pub struct ValueText {
    pub main: String,
    pub stack: Option<(String, String)>,
    pub after: String,
    pub below: String,
    pub boxed: bool,
}

/// Format a dimension's value.
pub fn value_text(
    d: &Dimension,
    value: f64,
    std: &Standard,
    angle: bool,
    nominal: f64,
) -> ValueText {
    let precision = d.precision.unwrap_or(if angle {
        std.angle_precision()
    } else {
        std.precision()
    });
    // ISO 286 deviations are whole micrometres.
    let tp = d.tolerance_precision.unwrap_or(match d.tolerance {
        Tolerance::Fit { .. } => 3,
        _ => precision.max(if angle { 1 } else { 2 }),
    });
    let unit = if angle { "°" } else { "" };
    let num = |v: f64| format!("{}{unit}", std.number(v, precision));
    let dev = |v: f64| {
        if v.abs() < 1e-12 {
            "0".to_string()
        } else {
            format!(
                "{}{}",
                if v > 0.0 { "+" } else { "-" },
                std.number(v.abs(), tp)
            )
        }
    };
    let mut main = num(value);
    let mut stack = None;
    let mut after = String::new();
    let mut boxed = false;
    match &d.tolerance {
        Tolerance::None | Tolerance::General => {}
        Tolerance::Basic => boxed = true,
        Tolerance::Bilateral { plus, minus } => stack = Some((dev(*plus), dev(-minus.abs()))),
        Tolerance::Limit { plus, minus } => {
            main = String::new();
            stack = Some((num(value + plus), num(value - minus.abs())));
        }
        Tolerance::Symmetric { value: t } => main = format!("{main}±{}", std.number(*t, tp)),
        Tolerance::Min => after = " MIN".into(),
        Tolerance::Max => after = " MAX".into(),
        Tolerance::Fit {
            hole,
            shaft,
            display,
        } => {
            let classes = match (hole, shaft) {
                (Some(h), Some(s)) => format!("{h}/{s}"),
                (Some(c), None) | (None, Some(c)) => c.clone(),
                (None, None) => String::new(),
            };
            let limits = hole
                .as_ref()
                .or(shaft.as_ref())
                .and_then(|c| crate::fits::deviations(c, nominal).ok());
            match display {
                FitDisplay::Classes => main = format!("{main} {classes}"),
                FitDisplay::ClassesWithTolerance => {
                    main = format!("{main} {classes}");
                    if let Some((u, l)) = limits {
                        stack = Some((dev(u), dev(l)));
                    }
                }
                FitDisplay::ToleranceOnly => {
                    if let Some((u, l)) = limits {
                        stack = Some((dev(u), dev(l)));
                    }
                }
            }
        }
    }
    if d.dual.unwrap_or(std.dual_units()) && !angle {
        after += &format!(" [{}]", std.number(value / 25.4, precision + 2));
    }
    let mut main = format!("{}{main}", d.prefix);
    if let Some(t) = &d.text {
        main = t.replace("<DIM>", &main);
    }
    if d.parentheses {
        main = format!("({main}");
        after = format!("{after}{})", d.suffix);
    } else {
        after = format!("{after}{}", d.suffix);
    }
    ValueText {
        main,
        stack,
        after,
        below: d.below.clone(),
        boxed: boxed || d.inspection,
    }
}

/// Drawing context for one dimension.
pub struct Painter<'a> {
    pub g: &'a mut Graphics,
    pub std: Standard,
    pub owner: Owner,
    pub layer: String,
}

impl Painter<'_> {
    fn thin(&self) -> Pen {
        Pen::new(LineStyle::Continuous, self.std.thin_line())
    }

    pub fn line(&mut self, a: P2, b: P2) {
        let pen = self.thin();
        let layer = self.layer.clone();
        self.g.line(a, b, pen, &layer, &self.owner);
    }

    pub fn push(&mut self, prim: Prim) {
        let pen = self.thin();
        let layer = self.layer.clone();
        self.g.push(prim, pen, &layer, &self.owner);
    }

    /// A filled arrowhead with its tip at `tip`, pointing along `dir`.
    pub fn arrow(&mut self, tip: DVec2, dir: DVec2) {
        let l = self.std.arrow_length();
        let d = dir.normalize_or(DVec2::X);
        let base = tip - d * l;
        let n = d.perp() * (l / 6.0);
        self.push(Prim::Fill {
            loops: vec![vec![p(tip), p(base + n), p(base - n)]],
        });
    }

    /// Text laid along `dir` (made readable) with its middle at `at`, raised
    /// by `lift` mm off the line. Returns the box half-width along the line.
    pub fn value(&mut self, t: &ValueText, at: DVec2, dir: DVec2, lift: f64) -> f64 {
        let h = self.std.text_height();
        let th = h * self.std.tolerance_scale();
        let angle = if self.std.unidirectional() {
            0.0
        } else {
            text::readable(dir.y.atan2(dir.x))
        };
        let (s, c) = angle.sin_cos();
        let u = DVec2::new(c, s);
        let n = u.perp();
        let w_main = text::line_width(&t.main, h);
        let w_stack = t
            .stack
            .as_ref()
            .map(|(a, b)| text::line_width(a, th).max(text::line_width(b, th)) + 0.8)
            .unwrap_or(0.0);
        let w_after = text::line_width(&t.after, h);
        let width = w_main + w_stack + w_after;
        // Baseline start.
        let base = at - u * (width / 2.0) + n * lift;
        let mut x = 0.0;
        let put = |g: &mut Painter, s: &str, x: f64, y: f64, hh: f64| {
            let q = base + u * x + n * y;
            for prim in text::layout(s, p(q), hh, angle, HAlign::Left) {
                g.push(prim);
            }
        };
        if !t.main.is_empty() {
            put(self, &t.main, x, 0.0, h);
            x += w_main;
        }
        if let Some((up, lo)) = &t.stack {
            let gap = if t.main.is_empty() { 0.0 } else { 0.8 };
            // Limits replace the value at full height; deviations are small.
            let hh = if t.main.is_empty() { h } else { th };
            put(self, up, x + gap, hh * 0.75, hh);
            put(self, lo, x + gap, -hh * 0.75, hh);
            x += w_stack;
        }
        if !t.after.is_empty() {
            put(self, &t.after, x, 0.0, h);
        }
        if !t.below.is_empty() {
            put(
                self,
                &t.below,
                (width - text::line_width(&t.below, h)) / 2.0,
                -h * text::LINE_PITCH,
                h,
            );
        }
        if t.boxed {
            let m = 1.0;
            let lo = base - u * m - n * m;
            let corners = [
                lo,
                lo + u * (width + 2.0 * m),
                lo + u * (width + 2.0 * m) + n * (h + 2.0 * m),
                lo + n * (h + 2.0 * m),
            ];
            self.push(Prim::Polyline {
                points: corners.iter().map(|c| p(*c)).collect(),
                closed: true,
            });
        }
        width / 2.0 + 1.0
    }
}

pub fn p(v: DVec2) -> P2 {
    [v.x, v.y]
}

/// The paper geometry of a linear dimension: feature points `a`, `b` on the
/// sheet, measured along paper direction `u`, dimension line through `at`.
pub fn draw_linear(
    pt: &mut Painter,
    a: DVec2,
    b: DVec2,
    u: DVec2,
    at: DVec2,
    t: &ValueText,
    arrows: Arrows,
) {
    let u = u.normalize_or(DVec2::X);
    let n = u.perp();
    // Dimension line offset from the features along n.
    let off = (at - a).dot(n);
    let fa = a + n * off;
    let fb = a + n * off + u * (b - a).dot(u);
    let gap = pt.std.extension_gap();
    let over = pt.std.extension_overshoot();
    for (feature, foot) in [(a, fa), (b, fb)] {
        let d = foot - feature;
        let len = d.length();
        if len > gap + 1e-9 {
            let dir = d / len;
            pt.line(p(feature + dir * gap), p(foot + dir * over));
        }
    }
    let span = fa.distance(fb);
    let along = (fb - fa).normalize_or(u);
    let arrow = pt.std.arrow_length();
    let room = match arrows {
        Arrows::Inside => true,
        Arrows::Outside => false,
        Arrows::Auto => span > arrow * 2.5,
    };
    // Text position along the line: `at`'s position.
    let mid = fa + along * (at - fa).dot(along);
    if room {
        pt.line(p(fa), p(fb));
        pt.arrow(fa, -along);
        pt.arrow(fb, along);
    } else {
        pt.line(p(fa - along * (arrow * 2.0)), p(fb + along * (arrow * 2.0)));
        pt.arrow(fa, along);
        pt.arrow(fb, -along);
    }
    // Extend the line to text placed outside the feet.
    let s = (mid - fa).dot(along);
    if s < 0.0 || s > span {
        let end = if s < 0.0 { fa } else { fb };
        pt.line(p(end), p(mid));
    }
    if pt.std.unidirectional() {
        // ANSI: horizontal text in a gap of the line, centered on it.
        let h = pt.std.text_height();
        pt.value(t, mid - DVec2::new(0.0, h / 2.0), DVec2::X, 0.0);
    } else {
        pt.value(t, mid, along, 1.0);
    }
}

/// A radius or diameter dimension of a circle seen as a circle.
pub fn draw_radial(
    pt: &mut Painter,
    center: DVec2,
    radius: f64,
    at: DVec2,
    diameter: bool,
    foreshortened: bool,
    t: &ValueText,
) {
    let dir = (at - center).normalize_or(DVec2::new(1.0, 1.0).normalize());
    let edge = center + dir * radius;
    let outside = at.distance(center) > radius;
    if diameter {
        let far = center - dir * radius;
        if outside {
            pt.line(p(far), p(at));
            pt.arrow(edge, dir);
            pt.arrow(far, -dir);
        } else {
            pt.line(p(far), p(edge));
            pt.arrow(edge, dir);
            pt.arrow(far, -dir);
        }
    } else if foreshortened {
        // Jogged line from a false center.
        let start = center + dir * (radius * 0.6);
        let j1 = start + dir.perp() * 2.0 + dir * 2.0;
        pt.push(Prim::Polyline {
            points: vec![
                p(start),
                p(j1),
                p(j1 + dir * 2.0 - dir.perp() * 2.0),
                p(edge),
            ],
            closed: false,
        });
        pt.arrow(edge, dir);
    } else if outside {
        pt.line(p(center), p(at));
        pt.arrow(edge, dir);
    } else {
        pt.line(p(center), p(edge));
        pt.arrow(edge, dir);
    }
    // Text beside the end of the leader, horizontal with a short shoulder.
    let h = pt.std.text_height();
    if pt.std.unidirectional() || outside {
        let right = dir.x >= 0.0;
        let tw = text::line_width(&t.main, h);
        let shoulder = if right { 3.0 } else { -3.0 };
        let leader_end = if outside { at } else { edge };
        let sh = leader_end + DVec2::new(shoulder, 0.0);
        pt.line(p(leader_end), p(sh));
        let mid = sh
            + DVec2::new(
                if right {
                    tw / 2.0 + 1.0
                } else {
                    -tw / 2.0 - 1.0
                },
                0.0,
            );
        pt.value(t, mid - DVec2::new(0.0, h / 2.0), DVec2::X, 0.0);
    } else {
        pt.value(t, at, dir, 1.0);
    }
}

/// An angle between two paper rays from `vertex`, arc through `at`.
pub fn draw_angle(
    pt: &mut Painter,
    vertex: DVec2,
    d1: DVec2,
    d2: DVec2,
    ends: [DVec2; 2],
    at: DVec2,
    t: &ValueText,
) {
    let r = at.distance(vertex).max(1.0);
    let a1 = d1.y.atan2(d1.x);
    let mut a2 = d2.y.atan2(d2.x);
    while a2 < a1 {
        a2 += std::f64::consts::TAU;
    }
    let pts = crate::graphics::arc_points(p(vertex), r, a1, a2, 0.05);
    pt.push(Prim::Polyline {
        points: pts.clone(),
        closed: false,
    });
    let s = DVec2::from(pts[0]);
    let e = DVec2::from(*pts.last().unwrap());
    pt.arrow(s, -DVec2::from_angle(a1).perp());
    pt.arrow(e, DVec2::from_angle(a2).perp());
    // Extension lines from the edges to the arc when it lies beyond them.
    let gap = pt.std.extension_gap();
    let over = pt.std.extension_overshoot();
    for (end, arc_pt, dir) in [(ends[0], s, d1), (ends[1], e, d2)] {
        let along = (arc_pt - vertex).length();
        let edge_len = (end - vertex).dot(dir);
        if along > edge_len + gap {
            pt.line(
                p(vertex + dir * (edge_len + gap)),
                p(vertex + dir * (along + over)),
            );
        }
    }
    let mid_angle = (a1 + a2) / 2.0;
    let mid = vertex + DVec2::from_angle(mid_angle) * r;
    if pt.std.unidirectional() {
        let h = pt.std.text_height();
        pt.value(
            t,
            mid + DVec2::from_angle(mid_angle) * (h * 0.8) - DVec2::new(0.0, h / 2.0),
            DVec2::X,
            0.0,
        );
    } else {
        pt.value(t, mid, DVec2::from_angle(mid_angle).perp(), 1.0);
    }
}

/// Arc length: a concentric arc with extension lines and an arc symbol.
pub fn draw_arc_length(
    pt: &mut Painter,
    center: DVec2,
    radius: f64,
    s: DVec2,
    e: DVec2,
    at: DVec2,
    t: &ValueText,
) {
    let r = at.distance(center).max(radius + 2.0);
    let a1 = (s - center).y.atan2((s - center).x);
    let mut a2 = (e - center).y.atan2((e - center).x);
    while a2 < a1 {
        a2 += std::f64::consts::TAU;
    }
    let pts = crate::graphics::arc_points(p(center), r, a1, a2, 0.05);
    pt.push(Prim::Polyline {
        points: pts.clone(),
        closed: false,
    });
    let gap = pt.std.extension_gap();
    let over = pt.std.extension_overshoot();
    for a in [a1, a2] {
        let d = DVec2::from_angle(a);
        pt.line(p(center + d * (radius + gap)), p(center + d * (r + over)));
    }
    pt.arrow(DVec2::from(pts[0]), -DVec2::from_angle(a1).perp());
    pt.arrow(
        DVec2::from(*pts.last().unwrap()),
        DVec2::from_angle(a2).perp(),
    );
    let mid_angle = (a1 + a2) / 2.0;
    let mid = center + DVec2::from_angle(mid_angle) * r;
    pt.value(t, mid, DVec2::from_angle(mid_angle).perp(), 1.0);
}

/// Text at `at` with a leader to `to` (chamfer and ordinate callouts).
pub fn draw_callout(pt: &mut Painter, to: DVec2, at: DVec2, t: &ValueText) {
    let h = pt.std.text_height();
    let right = at.x >= to.x;
    let tw = text::line_width(&t.main, h) + text::line_width(&t.after, h);
    let shoulder = at + DVec2::new(if right { 2.0 } else { -2.0 }, 0.0);
    pt.line(p(at), p(shoulder));
    let base = at + DVec2::new(0.0, 0.0);
    pt.line(
        p(base),
        p(to + (base - to).normalize_or(DVec2::X) * pt.std.arrow_length()),
    );
    pt.arrow(to, (to - base).normalize_or(DVec2::X));
    let mid = shoulder
        + DVec2::new(
            if right {
                tw / 2.0 + 0.5
            } else {
                -tw / 2.0 - 0.5
            },
            0.0,
        );
    pt.value(t, mid, DVec2::X, 0.5);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim() -> Dimension {
        Dimension::new(
            DimKind::Model {
                view: 1,
                key: "f1.dimension.1".into(),
            },
            [0.0, 0.0],
        )
    }

    #[test]
    fn values_follow_the_standard() {
        let iso = Standard::iso();
        let ansi = Standard::ansi();
        let d = dim();
        assert_eq!(value_text(&d, 20.0, &iso, false, 20.0).main, "20");
        assert_eq!(value_text(&d, 20.25, &iso, false, 20.25).main, "20.3");
        assert_eq!(value_text(&d, 20.0, &ansi, false, 20.0).main, "20.00");
        assert_eq!(value_text(&d, 45.0, &iso, true, 0.0).main, "45°");
    }

    #[test]
    fn tolerances_and_fits_format() {
        let iso = Standard::iso();
        let mut d = dim();
        d.prefix = "⌀".into();
        d.tolerance = Tolerance::Fit {
            hole: Some("H7".into()),
            shaft: Some("g6".into()),
            display: FitDisplay::Classes,
        };
        assert_eq!(value_text(&d, 20.0, &iso, false, 20.0).main, "⌀20 H7/g6");
        d.tolerance = Tolerance::Fit {
            hole: Some("H7".into()),
            shaft: None,
            display: FitDisplay::ClassesWithTolerance,
        };
        let t = value_text(&d, 20.0, &iso, false, 20.0);
        assert_eq!(t.main, "⌀20 H7");
        assert_eq!(t.stack, Some(("+0.021".into(), "0".into())));
        d.tolerance = Tolerance::Bilateral {
            plus: 0.1,
            minus: 0.05,
        };
        let t = value_text(&d, 20.0, &iso, false, 20.0);
        assert_eq!(t.stack, Some(("+0.1".into(), "-0.05".into())));
        d.tolerance = Tolerance::Symmetric { value: 0.2 };
        assert_eq!(value_text(&d, 20.0, &iso, false, 20.0).main, "⌀20±0.2");
        d.tolerance = Tolerance::Limit {
            plus: 0.1,
            minus: 0.1,
        };
        let t = value_text(&d, 20.0, &iso, false, 20.0);
        assert_eq!(t.stack, Some(("20.1".into(), "19.9".into())));
        d.tolerance = Tolerance::None;
        d.parentheses = true;
        d.dual = Some(true);
        let t = value_text(&d, 25.4, &iso, false, 25.4);
        assert_eq!(t.main, "(⌀25.4");
        assert_eq!(t.after, " [1])");
    }
}
