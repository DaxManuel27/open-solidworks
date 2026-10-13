//! Annotations placed on a sheet: notes, dimensions, symbols and tables.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::document::Anchor;

/// One annotation on a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Annotation {
    pub id: u32,
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
    },
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
