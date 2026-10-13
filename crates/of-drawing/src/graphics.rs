//! The sheet display list: everything drawn on a sheet as simple 2D
//! primitives in sheet millimetres (origin bottom-left, y up). The desktop
//! canvas, PDF, DXF and print all render this same list, so every output
//! matches the screen.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub type P2 = [f64; 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineStyle {
    Continuous,
    Hidden,
    Center,
    Phantom,
}

/// How a primitive is stroked or filled.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pen {
    pub style: LineStyle,
    /// Line weight, mm.
    pub weight: f64,
    /// RGB; black by default.
    #[serde(default)]
    pub color: [u8; 3],
}

impl Pen {
    pub fn new(style: LineStyle, weight: f64) -> Pen {
        Pen {
            style,
            weight,
            color: [0, 0, 0],
        }
    }
}

/// Horizontal text alignment relative to the anchor point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Prim {
    Line {
        a: P2,
        b: P2,
    },
    /// Counter-clockwise from `start` to `end` radians.
    Arc {
        center: P2,
        radius: f64,
        start: f64,
        end: f64,
    },
    Polyline {
        points: Vec<P2>,
        #[serde(default)]
        closed: bool,
    },
    /// Solid fill of closed loops (even-odd).
    Fill {
        loops: Vec<Vec<P2>>,
    },
    /// One line of text in the bundled font. `at` is the start of the
    /// baseline; the run extends along `angle` (radians). Text holds only
    /// characters the font has: drafting symbols are drawn as strokes.
    Text {
        at: P2,
        /// Font (em) size, mm; see [`crate::text::em_for`].
        height: f64,
        #[serde(default)]
        angle: f64,
        text: String,
        /// Advance width of the run, mm (for picking and alignment).
        width: f64,
    },
}

/// What a primitive belongs to, for picking and selection.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Owner {
    /// Border, zones and title block.
    Format,
    View {
        view: u32,
        /// The model entity drawn, when the primitive shows one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entity: Option<of_assembly::CompRef>,
    },
    Annotation {
        id: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Item {
    pub prim: Prim,
    pub pen: Pen,
    /// Layer name used by DXF and PDF optional content.
    pub layer: String,
    pub owner: Owner,
}

/// A sheet's drawing.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Graphics {
    pub width: f64,
    pub height: f64,
    pub items: Vec<Item>,
}

/// Standard layer names.
pub mod layers {
    pub const FORMAT: &str = "FORMAT";
    pub const VISIBLE: &str = "VISIBLE";
    pub const HIDDEN: &str = "HIDDEN";
    pub const TANGENT: &str = "TANGENT";
    pub const CENTER: &str = "CENTER";
    pub const HATCH: &str = "HATCH";
    pub const DIMENSIONS: &str = "DIMENSIONS";
    pub const NOTES: &str = "NOTES";
    pub const TABLES: &str = "TABLES";
}

impl Graphics {
    pub fn new(width: f64, height: f64) -> Graphics {
        Graphics {
            width,
            height,
            items: Vec::new(),
        }
    }

    pub fn push(&mut self, prim: Prim, pen: Pen, layer: &str, owner: &Owner) {
        self.items.push(Item {
            prim,
            pen,
            layer: layer.to_string(),
            owner: owner.clone(),
        });
    }

    pub fn line(&mut self, a: P2, b: P2, pen: Pen, layer: &str, owner: &Owner) {
        self.push(Prim::Line { a, b }, pen, layer, owner);
    }

    /// Axis-aligned rectangle outline from two corners.
    pub fn rect(&mut self, a: P2, b: P2, pen: Pen, layer: &str, owner: &Owner) {
        self.push(
            Prim::Polyline {
                points: vec![a, [b[0], a[1]], b, [a[0], b[1]]],
                closed: true,
            },
            pen,
            layer,
            owner,
        );
    }

    /// Layer names in first-use order.
    pub fn layers(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for i in &self.items {
            if !out.contains(&i.layer.as_str()) {
                out.push(&i.layer);
            }
        }
        out
    }

    /// Bounding box of every primitive (text by its baseline box).
    pub fn extents(&self) -> Option<(P2, P2)> {
        let mut lo = [f64::MAX; 2];
        let mut hi = [f64::MIN; 2];
        let mut add = |p: P2| {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        };
        for i in &self.items {
            match &i.prim {
                Prim::Line { a, b } => {
                    add(*a);
                    add(*b);
                }
                Prim::Arc { center, radius, .. } => {
                    add([center[0] - radius, center[1] - radius]);
                    add([center[0] + radius, center[1] + radius]);
                }
                Prim::Polyline { points, .. } => points.iter().for_each(|p| add(*p)),
                Prim::Fill { loops } => loops.iter().flatten().for_each(|p| add(*p)),
                Prim::Text {
                    at,
                    height,
                    angle,
                    width,
                    ..
                } => {
                    let (s, c) = angle.sin_cos();
                    add(*at);
                    add([at[0] + c * width, at[1] + s * width]);
                    add([at[0] - s * height, at[1] + c * height]);
                }
            }
        }
        (lo[0] <= hi[0]).then_some((lo, hi))
    }
}

/// Points along an arc, at most `max_angle` radians apart.
pub fn arc_points(center: P2, radius: f64, start: f64, end: f64, max_angle: f64) -> Vec<P2> {
    let n = (((end - start).abs() / max_angle.max(1e-3)).ceil() as usize).max(1);
    (0..=n)
        .map(|i| {
            let a = start + (end - start) * i as f64 / n as f64;
            [center[0] + radius * a.cos(), center[1] + radius * a.sin()]
        })
        .collect()
}
