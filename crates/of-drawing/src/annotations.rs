//! Annotations placed on a sheet: notes, dimensions, symbols and tables.
//!
//! An annotation either belongs to a view (`view` set: its positions are
//! sheet millimetres relative to the view's origin, so it moves with the
//! view) or to the sheet (positions are sheet millimetres). Geometry it
//! points at is named through [`ViewRef`]s, never through drawn curves, so it
//! survives model rebuilds.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::document::{Anchor, ViewRef};
use crate::hatch::Pattern;

/// One annotation on a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Annotation {
    pub id: u32,
    /// The view it moves with; `None` places it on the sheet. Named apart
    /// from the `view` some kinds measure in, since kinds are flattened in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attach: Option<u32>,
    /// Layer override; the kind picks one otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(flatten)]
    pub kind: AnnotationKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AnnotationKind {
    /// Text, optionally with a leader to model geometry. `text` may hold
    /// property links (`$PRP:"Description"`) and drafting symbols as Unicode
    /// (`⌀`, `⌴`, `⌵`, `↧`, `±`, `°`).
    Note {
        /// Where the text's bottom-left corner sits.
        at: [f64; 2],
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        leader: Option<Anchor>,
        /// Leader jog (a horizontal shoulder at the text).
        #[serde(default)]
        jog: bool,
        #[serde(default)]
        border: Border,
        /// Radians, counter-clockwise.
        #[serde(default)]
        angle: f64,
    },
    Dimension(Box<crate::dims::Dimension>),
    /// A BOM balloon or a free balloon.
    Balloon {
        at: [f64; 2],
        leader: Anchor,
        content: BalloonText,
        /// Second line of a split circle (quantity by default).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lower: Option<BalloonText>,
        #[serde(default)]
        shape: BalloonShape,
        /// Diameter (or side), mm; default from the standard.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<f64>,
        /// Further balloons stacked beside this one, one leader for all.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        stacked: Vec<BalloonText>,
        /// BOM table the item numbers come from; the first on the sheet when
        /// unset.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        table: Option<u32>,
    },
    /// Hole callout from Hole Wizard data (or the cylinder's size).
    HoleCallout {
        at: [f64; 2],
        /// A circular edge of the hole.
        target: ViewRef,
        /// Replaces the generated text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// Center marks on circular edges: one each, or a pattern's marks joined
    /// by connection lines.
    CenterMark {
        targets: Vec<ViewRef>,
        #[serde(default)]
        pattern: MarkPattern,
        /// Mark size, mm of paper; default from the standard.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<f64>,
        /// Extend the lines past the circle.
        #[serde(default = "yes")]
        extended: bool,
    },
    /// Centerline midway between two straight edges (or the axis of one
    /// cylindrical face seen from the side).
    Centerline {
        a: ViewRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        b: Option<ViewRef>,
        /// Overshoot past the geometry, mm of paper.
        #[serde(default = "two")]
        extend: f64,
    },
    /// Surface texture symbol (ISO 1302 / ASME Y14.36).
    SurfaceFinish {
        at: [f64; 2],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        leader: Option<Anchor>,
        #[serde(default)]
        process: FinishProcess,
        /// Roughness requirement text, e.g. `Ra 1.6`.
        #[serde(default)]
        roughness: String,
        /// Production method (above the long leg).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        method: String,
        /// Lay symbol: `=`, `⊥`, `X`, `M`, `C`, `R`, `P`.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        lay: String,
        /// The same requirement all around the part outline.
        #[serde(default)]
        all_around: bool,
        #[serde(default)]
        angle: f64,
    },
    /// Datum feature symbol: a framed letter with a filled triangle on the
    /// feature.
    DatumFeature {
        at: [f64; 2],
        label: String,
        target: Anchor,
    },
    /// Datum target: a circle split in two with size and label.
    DatumTarget {
        at: [f64; 2],
        label: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        size: String,
        target: Anchor,
        /// Point (X), line or area target drawn at the anchor.
        #[serde(default)]
        kind: TargetKind,
    },
    /// Geometric tolerance frames (ISO 1101 / ASME Y14.5), one row each
    /// (two rows make a composite frame).
    Gtol {
        at: [f64; 2],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        leader: Option<Anchor>,
        #[schemars(length(min = 1))]
        rows: Vec<crate::symbols::GtolRow>,
        /// Text above the frame, e.g. `6X`.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        above: String,
    },
    /// Weld symbol (ISO 2553 / AWS A2.4).
    Weld {
        at: [f64; 2],
        leader: Anchor,
        arrow: Option<crate::symbols::WeldSide>,
        other: Option<crate::symbols::WeldSide>,
        #[serde(default)]
        all_around: bool,
        #[serde(default)]
        field: bool,
        /// Tail text (process, specification).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        tail: String,
        /// A weld bead feature of the model whose size, symbol text and
        /// process fill the arrow side and tail.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bead: Option<of_model::FeatureId>,
    },
    /// A hatched or filled closed region (sheet or view coordinates).
    AreaHatch {
        #[schemars(length(min = 3))]
        boundary: Vec<[f64; 2]>,
        #[serde(default)]
        pattern: Pattern,
        #[serde(default = "one")]
        scale: f64,
        #[serde(default)]
        angle: f64,
    },
    /// Revision cloud around a region.
    RevisionCloud {
        shape: CloudShape,
        /// Arc size, mm of paper.
        #[serde(default = "cloud_arc")]
        arc: f64,
        /// Revision letter shown in a triangle beside it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        revision: Option<String>,
    },
    /// A revision letter in a triangle.
    RevisionSymbol {
        at: [f64; 2],
        revision: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        leader: Option<Anchor>,
    },
    /// A table: BOM, hole, revision, general or general tolerance.
    Table(Box<crate::tables::Table>),
}

fn yes() -> bool {
    true
}
fn one() -> f64 {
    1.0
}
fn two() -> f64 {
    2.0
}
fn cloud_arc() -> f64 {
    6.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Border {
    #[default]
    None,
    Box,
    Circle,
}

/// What a balloon shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum BalloonText {
    /// The BOM item number of a component.
    Item {
        component: Vec<String>,
    },
    /// The component's quantity in the BOM.
    Quantity {
        component: Vec<String>,
    },
    /// A custom property of the component's part.
    Property {
        component: Vec<String>,
        name: String,
    },
    Text {
        text: String,
    },
    /// The weldment cut list item number of the item holding a body.
    CutListItem {
        body: of_model::BodyId,
    },
    /// That item's quantity.
    CutListQuantity {
        body: of_model::BodyId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BalloonShape {
    #[default]
    Circle,
    Triangle,
    Hexagon,
    Box,
    Diamond,
    /// Two lines: item over quantity.
    SplitCircle,
    Flag,
    /// Text without a border.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkPattern {
    /// A mark on every circle.
    #[default]
    Single,
    /// Marks joined by a center line along a row.
    Linear,
    /// Marks joined by a center circle through them.
    Circular,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FinishProcess {
    /// Any process allowed (basic symbol).
    #[default]
    Any,
    /// Material removal required (bar across).
    Removal,
    /// Material removal prohibited (circle).
    NoRemoval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    #[default]
    Point,
    Line,
    /// A hatched circular area of the given size.
    Area,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum CloudShape {
    Rectangle {
        a: [f64; 2],
        b: [f64; 2],
    },
    Ellipse {
        center: [f64; 2],
        rx: f64,
        ry: f64,
    },
    Polygon {
        points: Vec<[f64; 2]>,
    },
    /// A freehand path, closed automatically.
    Freehand {
        points: Vec<[f64; 2]>,
    },
}

impl AnnotationKind {
    /// Anchors the annotation points at, for dangling checks.
    pub fn anchors(&self) -> Vec<&Anchor> {
        match self {
            AnnotationKind::Note { leader, .. }
            | AnnotationKind::SurfaceFinish { leader, .. }
            | AnnotationKind::Gtol { leader, .. }
            | AnnotationKind::RevisionSymbol { leader, .. } => leader.iter().collect(),
            AnnotationKind::Balloon { leader, .. } | AnnotationKind::Weld { leader, .. } => {
                vec![leader]
            }
            AnnotationKind::DatumFeature { target, .. }
            | AnnotationKind::DatumTarget { target, .. } => vec![target],
            _ => Vec::new(),
        }
    }

    /// Views the annotation's geometry references.
    pub fn views(&self) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .anchors()
            .into_iter()
            .filter_map(|a| match a {
                Anchor::Model { target, .. } => Some(target.view),
                Anchor::Sheet { .. } => None,
            })
            .collect();
        match self {
            AnnotationKind::Dimension(d) => out.extend(d.views()),
            AnnotationKind::HoleCallout { target, .. } => out.push(target.view),
            AnnotationKind::CenterMark { targets, .. } => {
                out.extend(targets.iter().map(|t| t.view))
            }
            AnnotationKind::Centerline { a, b, .. } => {
                out.push(a.view);
                out.extend(b.iter().map(|b| b.view));
            }
            AnnotationKind::Table(t) => match &t.kind {
                crate::tables::TableKind::Bom { view, .. } => out.push(*view),
                crate::tables::TableKind::Hole { origin, holes, .. } => {
                    out.push(origin.target.view);
                    out.extend(holes.iter().map(|h| h.view));
                }
                crate::tables::TableKind::Bend { view, .. }
                | crate::tables::TableKind::CutList { view, .. }
                | crate::tables::TableKind::Weld { view } => out.push(*view),
                crate::tables::TableKind::Punch { view, origin } => {
                    out.push(*view);
                    out.extend(origin.iter().map(|o| o.target.view));
                }
                _ => {}
            },
            _ => {}
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Drop optional leaders that point into `views`.
    pub fn drop_leaders_to(&mut self, views: &[u32]) {
        match self {
            AnnotationKind::Note { leader, .. }
            | AnnotationKind::SurfaceFinish { leader, .. }
            | AnnotationKind::Gtol { leader, .. }
            | AnnotationKind::RevisionSymbol { leader, .. } => {
                if matches!(leader, Some(Anchor::Model { target, .. }) if views.contains(&target.view))
                {
                    *leader = None;
                }
            }
            _ => {}
        }
    }

    /// Move every sheet position of the annotation by `d` mm (model anchors
    /// stay where they are).
    pub fn translate(&mut self, d: [f64; 2]) {
        let mv = |p: &mut [f64; 2]| {
            p[0] += d[0];
            p[1] += d[1];
        };
        let mv_anchor = |a: &mut Anchor| {
            if let Anchor::Sheet { at } = a {
                at[0] += d[0];
                at[1] += d[1];
            }
        };
        match self {
            AnnotationKind::Note { at, .. }
            | AnnotationKind::Balloon { at, .. }
            | AnnotationKind::HoleCallout { at, .. }
            | AnnotationKind::SurfaceFinish { at, .. }
            | AnnotationKind::Gtol { at, .. }
            | AnnotationKind::Weld { at, .. }
            | AnnotationKind::RevisionSymbol { at, .. } => mv(at),
            AnnotationKind::DatumFeature { at, target, .. }
            | AnnotationKind::DatumTarget { at, target, .. } => {
                mv(at);
                mv_anchor(target);
            }
            AnnotationKind::Dimension(dim) => mv(&mut dim.at),
            AnnotationKind::AreaHatch { boundary, .. } => boundary.iter_mut().for_each(mv),
            AnnotationKind::RevisionCloud { shape, .. } => match shape {
                CloudShape::Rectangle { a, b } => {
                    mv(a);
                    mv(b);
                }
                CloudShape::Ellipse { center, .. } => mv(center),
                CloudShape::Polygon { points } | CloudShape::Freehand { points } => {
                    points.iter_mut().for_each(mv)
                }
            },
            AnnotationKind::Table(t) => mv(&mut t.at),
            AnnotationKind::CenterMark { .. } | AnnotationKind::Centerline { .. } => {}
        }
    }
}

impl Annotation {
    /// Every view the annotation depends on: the one it moves with and the
    /// ones its geometry is in.
    pub fn views(&self) -> Vec<u32> {
        let mut v = self.kind.views();
        v.extend(self.attach);
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// A leader line from `from` to `to` with a filled arrowhead at `to`.
pub fn leader(
    g: &mut crate::graphics::Graphics,
    from: crate::graphics::P2,
    to: crate::graphics::P2,
    arrow: f64,
    pen: crate::graphics::Pen,
    owner: &crate::graphics::Owner,
) {
    use crate::graphics::{Prim, layers};
    let d = [to[0] - from[0], to[1] - from[1]];
    let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if len < 1e-9 {
        return;
    }
    let u = [d[0] / len, d[1] / len];
    let n = [-u[1], u[0]];
    let w = arrow / 6.0;
    let base = [to[0] - u[0] * arrow, to[1] - u[1] * arrow];
    g.line(from, base, pen, layers::NOTES, owner);
    g.push(
        Prim::Fill {
            loops: vec![vec![
                to,
                [base[0] + n[0] * w, base[1] + n[1] * w],
                [base[0] - n[0] * w, base[1] - n[1] * w],
            ]],
        },
        pen,
        layers::NOTES,
        owner,
    );
}
