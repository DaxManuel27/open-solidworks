//! Arguments of the drawing methods (`drawing.*`). Sheet positions are
//! millimetres of paper from the sheet's bottom-left corner, y up; sheet,
//! view and annotation IDs come from `drawing_info`.

use std::collections::BTreeMap;

use of_assembly::CompRef;
use of_drawing::annotations::AnnotationKind;
use of_drawing::standard::StandardBase;
use of_drawing::{DisplayStyle, Projection, Scale, SheetSize, TangentEdges, ViewKind};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct NewDrawing {
    /// Drafting standard; omitted uses ISO.
    #[serde(default)]
    pub standard: Option<StandardBase>,
    /// First sheet's paper; omitted uses A3 (ISO) or ANSI B.
    #[serde(default)]
    pub size: Option<SheetSize>,
    /// Omitted uses first angle (ISO) or third angle (ANSI).
    #[serde(default)]
    pub projection: Option<Projection>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddSheet {
    #[serde(default)]
    pub name: Option<String>,
    /// Omitted fields copy the last sheet.
    #[serde(default)]
    pub size: Option<SheetSize>,
    #[serde(default)]
    pub portrait: Option<bool>,
    #[serde(default)]
    pub scale: Option<Scale>,
    #[serde(default)]
    pub projection: Option<Projection>,
    /// Sheet format: `iso`, `ansi` or an `.ofdt` file.
    #[serde(default)]
    pub template: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateSheet {
    pub id: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub size: Option<SheetSize>,
    #[serde(default)]
    pub portrait: Option<bool>,
    #[serde(default)]
    pub scale: Option<Scale>,
    #[serde(default)]
    pub projection: Option<Projection>,
    #[serde(default)]
    pub template: Option<String>,
    /// Show the border and title block.
    #[serde(default)]
    pub format_visible: Option<bool>,
    /// View whose model fills `$PRP` title block fields; null uses the first
    /// view.
    #[serde(default, deserialize_with = "some")]
    pub properties_view: Option<Option<u32>>,
    /// New position in the sheet order, from 0.
    #[serde(default)]
    pub index: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddView {
    pub sheet: u32,
    /// What the view shows. Model paths are `.ofp` or `.ofa` files.
    pub view: ViewKind,
    /// Sheet position of the model origin, mm. Projected and auxiliary
    /// views stay aligned with their parent along the fold.
    pub origin: [f64; 2],
    #[serde(default)]
    pub name: Option<String>,
    /// Omitted uses the sheet scale (model views) or the parent's.
    #[serde(default)]
    pub scale: Option<Scale>,
    #[serde(default)]
    pub display: Option<DisplayStyle>,
    #[serde(default)]
    pub tangent: Option<TangentEdges>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateView {
    pub id: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub view: Option<ViewKind>,
    #[serde(default)]
    pub origin: Option<[f64; 2]>,
    /// Null returns to the sheet or parent scale.
    #[serde(default, deserialize_with = "some")]
    pub scale: Option<Option<Scale>>,
    /// Null follows the parent view.
    #[serde(default, deserialize_with = "some")]
    pub display: Option<Option<DisplayStyle>>,
    #[serde(default, deserialize_with = "some")]
    pub tangent: Option<Option<TangentEdges>>,
    #[serde(default, deserialize_with = "some")]
    pub label: Option<Option<String>>,
    #[serde(default)]
    pub aligned: Option<bool>,
    /// Model edges not drawn (replaces the list).
    #[serde(default)]
    pub hidden_edges: Option<Vec<CompRef>>,
    /// Assembly components not drawn, by key path (replaces the list).
    #[serde(default)]
    pub hidden_components: Option<Vec<Vec<String>>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddAnnotation {
    pub sheet: u32,
    pub annotation: AnnotationKind,
}

#[derive(Deserialize, JsonSchema)]
pub struct DrawingProperties {
    /// Drawing properties for `$PRPSHEET` fields (`DrawnBy`, `Revision`,
    /// ...); null removes one.
    pub properties: BTreeMap<String, Option<String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct RenderSheet {
    /// Omitted renders the first sheet.
    #[serde(default)]
    pub sheet: Option<u32>,
    /// Only view boxes, scales and errors, without the display list.
    #[serde(default)]
    pub summary: bool,
}

/// Distinguish an explicit `null` from a missing field.
fn some<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(d).map(Some)
}
