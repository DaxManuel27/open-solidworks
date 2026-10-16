//! Arguments of the catalog methods. The session deserializes these types and
//! the tool schemas are generated from them, so the two cannot drift apart.
//! Lengths are millimetres and angles radians.

use std::collections::BTreeMap;
use std::path::PathBuf;

use of_model::configurations::Configuration;
use of_model::sketch::{ConstraintKind, Id};
use of_model::{BodyId, FeatureId, FeatureKind, Ref};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct Empty {}

#[derive(Deserialize, JsonSchema)]
pub struct SolveSpatialSketch {
    pub sketch: of_model::sketch::spatial::Sketch3D,
    #[serde(default)]
    pub drag: Option<SpatialDrag>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SpatialDrag {
    pub point: Id,
    pub to: [f64; 3],
}

#[derive(Deserialize, JsonSchema)]
pub struct FeatureRef {
    /// Feature ID from `part_info` `features`.
    #[schemars(range(min = 1))]
    pub id: FeatureId,
}

#[derive(Deserialize, JsonSchema)]
pub struct ConfigurationRef {
    /// Configuration ID from `part_info` `configurations.items`.
    #[schemars(range(min = 1))]
    pub id: u32,
}

#[derive(Deserialize, JsonSchema)]
pub struct SetParameter {
    /// Stable key from `part_info` `parameters`, such as `f2.end1.depth`.
    pub key: String,
    /// Core units: mm for lengths, radians for angles.
    pub value: f64,
}

#[derive(Deserialize, JsonSchema)]
pub struct SetEquations {
    /// Omitted edits shared base values; otherwise replaces this configuration's
    /// variable/equation/numeric overrides. Removing an override restores the base.
    #[serde(default)]
    pub configuration: Option<u32>,
    /// Numeric parameter values in mm/radians. Base edits update only these keys;
    /// configuration edits replace that configuration's numeric override map.
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
    /// Reject a stale preview or commit if the document changed while editing.
    #[serde(default)]
    pub revision: Option<u64>,
    /// Every global variable: name to formula, such as `"8mm"` or `"width / 2"`.
    pub variables: BTreeMap<String, String>,
    /// Parameter key to formula; null disables the inherited equation in a variant.
    pub equations: BTreeMap<String, Option<String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateConfiguration {
    #[schemars(length(min = 1))]
    pub name: String,
    /// Configuration to duplicate; omitted inherits base values.
    #[schemars(range(min = 1))]
    pub copy_from: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateConfiguration {
    /// Reject an edit based on an older document revision.
    #[serde(default)]
    pub revision: Option<u64>,
    /// The complete configuration. Omitted maps and `material` are reset.
    pub configuration: Configuration,
}

#[derive(Deserialize, JsonSchema)]
pub struct BodyDisplay {
    /// Body ID from `part_info` `bodies`.
    pub id: BodyId,
    #[schemars(length(min = 1))]
    pub name: Option<String>,
    pub hidden: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
pub struct FilePath {
    pub path: PathBuf,
}

#[derive(Deserialize, JsonSchema)]
pub struct SavePath {
    /// Omitted saves to the path the part was opened from.
    pub path: Option<PathBuf>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Export {
    /// `.step`, `.stp`, `.iges`, `.igs` or `.stl`; the extension picks the
    /// format. `.dxf` writes the flat patterns of the part's sheet metal bodies.
    pub path: PathBuf,
    /// Configuration ID; omitted exports the active configuration.
    #[schemars(range(min = 1))]
    pub configuration: Option<u32>,
    #[serde(default)]
    #[schemars(skip)]
    pub format: Option<of_io::ExportFormat>,
    /// Flat pattern DXF: the sheet metal bodies to write (IDs from part_info
    /// `bodies`); omitted writes every sheet metal body.
    #[serde(default)]
    pub bodies: Option<Vec<BodyId>>,
    /// Flat pattern DXF: one file per body (`name-Body 1.dxf`, ...) instead of
    /// all side by side in one.
    #[serde(default)]
    pub per_body: bool,
    /// Flat pattern DXF: write bend notes (`UP 90° R1`) beside the bend lines.
    #[serde(default = "yes")]
    pub bend_notes: bool,
    /// Flat pattern DXF version.
    #[serde(default)]
    pub dxf_version: Option<of_drawing::dxf::DxfVersion>,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize, JsonSchema)]
pub struct SheetMetalInfo {
    /// Include each flat pattern's outline, bend lines and form marks in
    /// flat millimetres.
    #[serde(default)]
    pub outline: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct NewSketch {
    #[schemars(with = "SketchPlane")]
    pub plane: Ref,
}

/// Where a sketch lies. Sketch X/Y map to model X/Y on the front plane, X/Z on
/// the top plane and Y/Z on the right plane.
#[derive(JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum SketchPlane {
    /// XY.
    FrontPlane,
    /// XZ.
    TopPlane,
    /// YZ.
    RightPlane,
    /// A reference plane feature.
    Feature {
        #[schemars(range(min = 1))]
        id: FeatureId,
    },
    /// A planar face by stable name from `topology`.
    Face { name: String },
    /// A planar face from the user's selection.
    BodyTopology {
        body: String,
        kind: FaceKind,
        name: String,
    },
}

#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum FaceKind {
    Face,
}

#[derive(Deserialize, JsonSchema)]
pub struct EditSketch {
    /// Sketch feature ID.
    #[schemars(range(min = 1))]
    pub id: FeatureId,
    pub op: SketchOp,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddFeature {
    pub feature: FeatureInput,
    /// Tree label; omitted uses a default such as `Boss-Extrude2`.
    #[schemars(length(min = 1))]
    pub name: Option<String>,
    /// Parameter path within this feature (such as `end1.depth`) to a formula,
    /// stored as an equation on the new feature; null removes one.
    #[serde(default)]
    pub formulas: BTreeMap<String, Option<String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateFeature {
    #[schemars(range(min = 1))]
    pub id: FeatureId,
    /// The complete replacement definition, same shape as `add_feature`.
    pub feature: FeatureInput,
    /// Parameter path within this feature to a formula; null removes that
    /// parameter's equation. Paths left out keep their equations.
    #[serde(default)]
    pub formulas: BTreeMap<String, Option<String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct LocateRef {
    /// A vertex, reference point, plane or planar face.
    pub reference: Ref,
}

#[derive(Deserialize, JsonSchema)]
pub struct EvaluateFormula {
    /// A formula such as `width / 2 + 1mm`.
    pub expression: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct DeleteFeatures {
    #[schemars(length(min = 1))]
    pub ids: Vec<FeatureId>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Measure {
    /// One to four face, edge or vertex refs.
    #[schemars(length(min = 1, max = 4))]
    pub refs: Vec<Ref>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SetMaterial {
    /// Library name such as `Aluminum 6061`, or a custom name with `density`.
    /// Omitted clears the material.
    pub name: Option<String>,
    /// kg/m³, for a custom material.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub density: Option<f64>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SetProperties {
    /// Part custom properties for BOMs and title blocks (`PartNo`,
    /// `Description`, ...); values may link to `{mass}`, `{material}` or
    /// `{param:f2.end1.depth}`. Null removes one.
    pub properties: BTreeMap<String, Option<of_model::properties::Property>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct DimensionDisplay {
    /// Parameter key from part_info, such as `f2.end1.depth` or
    /// `f1.dimension.4`.
    pub key: String,
    /// Import it into drawings with model items.
    #[serde(default)]
    pub show: Option<bool>,
    /// Name shown under the value in drawings; null removes it.
    #[serde(default, deserialize_with = "some_value")]
    pub name: Option<Option<String>>,
}

/// Distinguish an explicit `null` from a missing field.
fn some_value<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(d).map(Some)
}

/// A feature definition: a `type` tag, its fields, and an optional body scope.
#[derive(Deserialize)]
pub struct FeatureInput {
    #[serde(flatten)]
    pub kind: FeatureKind,
    #[serde(default)]
    pub scope: Option<Vec<BodyId>>,
}

impl JsonSchema for FeatureInput {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Feature".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        // Each tagged variant also accepts a body scope; the catalog inlines
        // the variant fields so every branch is one strict object.
        let mut schema = FeatureKind::json_schema(generator);
        let scope = json_schema!({
            "description": "Body IDs this feature acts on; omitted acts on the whole part.",
            "type": ["array", "null"],
            "items": { "type": "string" },
            "minItems": 1,
        });
        if let Some(variants) = schema.get_mut("oneOf").and_then(|v| v.as_array_mut()) {
            for variant in variants {
                if let Some(props) = variant
                    .get_mut("properties")
                    .and_then(|p| p.as_object_mut())
                {
                    props.insert("scope".into(), scope.clone().to_value());
                }
            }
        }
        schema
    }
}

/// A point given by sketch entity ID, or a new point at a position.
#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PointArg {
    /// An existing point.
    Id { id: Id },
    /// A new point at sketch coordinates, mm.
    At { at: [f64; 2] },
}

/// One sketch edit, tagged by `op`. Coordinates are sketch-plane millimetres.
/// Results list new entity IDs in `created`.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SketchOp {
    /// A standalone point; hole features drill at standalone points.
    AddPoint { at: [f64; 2] },
    /// `created`: the line, then its two points.
    AddLine { a: PointArg, b: PointArg },
    /// `created`: the circle. Its center point is not listed.
    AddCircle {
        center: PointArg,
        #[schemars(extend("exclusiveMinimum" = 0))]
        radius: f64,
    },
    /// Arc from `start` to `end` passing through `via`.
    AddArc3 {
        start: PointArg,
        end: PointArg,
        via: [f64; 2],
    },
    /// Arc around `center` from `start`, counter-clockwise to the direction of `end`.
    AddCenterArc {
        center: PointArg,
        start: PointArg,
        end: [f64; 2],
        #[serde(default)]
        clockwise: bool,
    },
    /// Arc from point `start`, leaving in direction `tangent`.
    AddTangentArc {
        start: Id,
        tangent: [f64; 2],
        end: PointArg,
    },
    AddSpline {
        #[schemars(length(min = 2))]
        points: Vec<PointArg>,
        #[serde(default)]
        closed: bool,
    },
    /// Corner rectangle from `a` to `b`, or `center: true` for center `a` and
    /// corner `b`. Corner `created`: the 4 corners from `a`, then the bottom,
    /// right, top and left lines.
    Rectangle {
        a: [f64; 2],
        b: [f64; 2],
        #[serde(default)]
        center: bool,
    },
    /// Straight slot between arc centers `a` and `b`.
    Slot {
        a: [f64; 2],
        b: [f64; 2],
        #[schemars(extend("exclusiveMinimum" = 0))]
        width: f64,
    },
    /// Regular polygon around `center` with one vertex at `vertex`.
    Polygon {
        center: [f64; 2],
        vertex: [f64; 2],
        #[schemars(range(min = 3, max = 64))]
        sides: usize,
    },
    /// A geometric relation. Entity 0 is the fixed sketch origin.
    Constraint { constraint: ConstraintKind },
    /// A dimension. The value in `constraint` is ignored: the dimension is
    /// measured from the geometry, then set to `value` if given.
    Dimension {
        constraint: ConstraintKind,
        #[serde(default)]
        value: Option<f64>,
    },
    /// Change a dimension's value (mm or radians).
    SetValue { id: Id, value: f64 },
    /// Make a dimension driving (`true`) or driven (`false`).
    SetDriving { id: Id, driving: bool },
    /// Delete entities or relations, with anything that depends on them.
    Delete {
        #[schemars(length(min = 1))]
        ids: Vec<Id>,
    },
    /// Interactive dragging, for the desktop app only.
    #[schemars(skip)]
    Drag { point: Id, to: [f64; 2] },
    /// Translate entities by `delta`.
    Move { ids: Vec<Id>, delta: [f64; 2] },
    /// Remove the part of `entity` between the intersections around `pick`.
    Trim { entity: Id, pick: [f64; 2] },
    /// Extend a line from the end nearest `pick` to the next curve.
    Extend { entity: Id, pick: [f64; 2] },
    /// Offset a chain or circle to its left (negative: right).
    Offset { ids: Vec<Id>, distance: f64 },
    /// Mirror entities about line `about`.
    Mirror { ids: Vec<Id>, about: Id },
    /// Round the corner at `point` where two lines meet.
    Fillet {
        point: Id,
        #[schemars(extend("exclusiveMinimum" = 0))]
        radius: f64,
    },
    /// Toggle construction geometry.
    Construction { ids: Vec<Id>, construction: bool },
    /// Project model edges into the sketch; a face converts all of its edge loops.
    Convert {
        #[schemars(length(min = 1))]
        refs: Vec<Ref>,
    },
}
