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
    /// Make the drawing from this `.ofp` or `.ofa` file: a standard 3 view
    /// and an isometric, scaled to fit.
    #[serde(default)]
    pub model: Option<std::path::PathBuf>,
    /// Part or assembly configuration ID for `model`.
    #[serde(default)]
    pub configuration: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct StandardViews {
    pub sheet: u32,
    /// `.ofp` or `.ofa` file.
    pub model: std::path::PathBuf,
    #[serde(default)]
    pub configuration: Option<u32>,
    /// Also add an isometric view (default true).
    #[serde(default = "yes")]
    pub isometric: bool,
    /// Omitted picks the largest preferred scale that fits and sets it as
    /// the sheet scale.
    #[serde(default)]
    pub scale: Option<Scale>,
}

fn yes() -> bool {
    true
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
    /// The view the annotation moves with: its positions are then sheet mm
    /// relative to that view's origin. Omitted places it on the sheet.
    #[serde(default)]
    pub view: Option<u32>,
    /// Layer; omitted uses the kind's (DIMENSIONS, NOTES, CENTER, TABLES,
    /// HATCH).
    #[serde(default)]
    pub layer: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateAnnotation {
    pub id: u32,
    /// Replaces the annotation's content (same or different kind).
    #[serde(default)]
    pub annotation: Option<AnnotationKind>,
    /// Null moves it to the sheet, keeping where it is drawn.
    #[serde(default, deserialize_with = "some")]
    pub view: Option<Option<u32>>,
    #[serde(default, deserialize_with = "some")]
    pub layer: Option<Option<String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct MoveAnnotations {
    #[schemars(length(min = 1))]
    pub ids: Vec<u32>,
    /// Offset in sheet mm.
    pub by: [f64; 2],
}

#[derive(Deserialize, JsonSchema)]
pub struct ModelItemsParams {
    pub sheet: u32,
    /// Part views to import into; omitted uses every part view on the
    /// sheet. Each dimension goes to the view that shows it truest.
    #[serde(default)]
    pub views: Option<Vec<u32>>,
    #[serde(default)]
    pub items: of_drawing::annot::ModelItems,
}

#[derive(Deserialize, JsonSchema)]
pub struct DriveDimension {
    /// An imported model dimension.
    pub id: u32,
    /// New value: mm, or degrees for angles.
    pub value: f64,
}

#[derive(Deserialize, JsonSchema)]
pub struct AutoBalloons {
    pub sheet: u32,
    /// An assembly view on the sheet.
    pub view: u32,
    #[serde(default)]
    pub layout: of_drawing::annot::BalloonLayout,
    /// One balloon per BOM row instead of one per component.
    #[serde(default = "yes")]
    pub ignore_multiple: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct TableCsv {
    /// A table annotation.
    pub id: u32,
    /// Write the CSV here; omitted returns it as `csv`.
    #[serde(default)]
    pub path: Option<std::path::PathBuf>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SetStandard {
    /// Replaces the drafting standard (base and overrides).
    #[serde(default)]
    pub standard: Option<of_drawing::standard::Standard>,
    /// Load it from an `.ofstd` file instead.
    #[serde(default)]
    pub load: Option<std::path::PathBuf>,
    /// Also save the result to an `.ofstd` file for other drawings.
    #[serde(default)]
    pub save: Option<std::path::PathBuf>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ExportDrawing {
    /// `.pdf` or `.dxf` file.
    pub path: std::path::PathBuf,
    /// Sheet IDs to write; omitted writes every sheet.
    #[serde(default)]
    pub sheets: Option<Vec<u32>>,
    /// PDF: black lines and text instead of each pen's colour.
    #[serde(default)]
    pub monochrome: bool,
    /// DXF: one file per sheet (`name-Sheet1.dxf`, ...) instead of all
    /// sheets side by side in one.
    #[serde(default)]
    pub per_sheet: bool,
    /// DXF version: `r12` (AC1009) or `r2018` (AC1032, default).
    #[serde(default)]
    pub dxf_version: Option<of_drawing::dxf::DxfVersion>,
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
