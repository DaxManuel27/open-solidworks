//! Arguments of the assembly methods (`asm.*`). Lengths are millimetres,
//! angles radians, and component keys come from `assembly_info`.

use std::path::PathBuf;

use of_assembly::hardware::HardwareSpec;
use of_assembly::{
    CompRef, ExplodeMotion, MateKind, MirrorItem, MirrorPosition, PatternKind, Transform,
};
use of_model::FeatureKind;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct InsertComponent {
    /// `.ofp` part or `.ofa` assembly file.
    pub path: PathBuf,
    /// Placement; omitted puts the component at the origin.
    #[serde(default)]
    pub transform: Option<Transform>,
    /// Part configuration ID; omitted uses the file's active configuration.
    #[serde(default)]
    pub configuration: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct InsertHardware {
    pub hardware: HardwareSpec,
    #[serde(default)]
    pub transform: Option<Transform>,
    /// Library folder; omitted uses `OPENFORGE_HARDWARE_DIR` or
    /// `~/OpenForge/Hardware`.
    #[serde(default)]
    pub folder: Option<PathBuf>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ComponentKeys {
    /// Top-level component keys from `assembly_info` `components`.
    #[schemars(length(min = 1))]
    pub keys: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateComponent {
    /// Top-level component key.
    pub key: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub fixed: Option<bool>,
    #[serde(default)]
    pub hidden: Option<bool>,
    /// Suppression in the active assembly configuration.
    #[serde(default)]
    pub suppressed: Option<bool>,
    #[serde(default)]
    pub exclude_from_bom: Option<bool>,
    /// Subassemblies: solve their components with this assembly's mates.
    #[serde(default)]
    pub flexible: Option<bool>,
    /// Part configuration ID in the active assembly configuration; null
    /// returns to the file's active configuration.
    #[serde(default, deserialize_with = "some")]
    pub configuration: Option<Option<u32>>,
    #[serde(default)]
    pub transform: Option<Transform>,
}

/// Distinguish an explicit `null` from a missing field.
fn some<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(d).map(Some)
}

#[derive(Deserialize, JsonSchema)]
pub struct ReplaceComponent {
    pub key: String,
    pub path: PathBuf,
    #[serde(default)]
    pub configuration: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddMate {
    pub mate: MateKind,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateMate {
    /// Mate ID from `assembly_info` `mates`.
    pub id: u32,
    #[serde(default)]
    pub mate: Option<MateKind>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub suppressed: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ItemIds {
    #[schemars(length(min = 1))]
    pub ids: Vec<u32>,
}

/// How to move a component.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Motion {
    /// Move by an offset in the assembly frame.
    Delta { translation: [f64; 3] },
    /// Move so that a point of the component (its own frame) lands at
    /// `point` in the assembly frame.
    To {
        #[serde(default)]
        local: [f64; 3],
        point: [f64; 3],
    },
    /// Move along an edge, axis or plane normal.
    Along { reference: CompRef, distance: f64 },
    /// Rotate about the assembly X, Y and Z axes through the component origin.
    Rotate { angles: [f64; 3] },
    /// Rotate about an edge, axis or cylinder.
    About { reference: CompRef, angle: f64 },
    /// Free drag toward a whole pose.
    Pose { transform: Transform },
    /// Free drag of a component point (own frame) toward an assembly point.
    Drag { local: [f64; 3], point: [f64; 3] },
}

#[derive(Deserialize, JsonSchema)]
pub struct MoveComponent {
    /// Component key path: `["2"]`, or a flexible subassembly's child `["4", "1"]`.
    #[schemars(length(min = 1))]
    pub component: Vec<String>,
    pub motion: Motion,
    /// Stop before parts overlap.
    #[serde(default)]
    pub collision: Option<Collision>,
    /// Interactive drags: `start` snapshots for undo, `move` previews and
    /// returns only positions, `end` (the default) records the change.
    #[serde(default)]
    pub phase: Option<DragPhase>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Collision {
    /// Check only the dragged component.
    #[serde(default)]
    pub dragged_only: bool,
}

#[derive(Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DragPhase {
    Start,
    Move,
    End,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddPattern {
    /// Linear, circular, pattern-driven, sketch-driven or curve-driven.
    pub pattern: PatternKind,
    /// Seed component keys.
    #[schemars(length(min = 1))]
    pub seeds: Vec<String>,
    #[serde(default)]
    pub skip: Vec<u32>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdatePattern {
    pub id: u32,
    #[serde(flatten)]
    pub pattern: AddPattern,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddMirror {
    /// Plane or flat face to mirror about.
    pub plane: CompRef,
    #[schemars(length(min = 1))]
    pub components: Vec<MirrorItem>,
    #[serde(default)]
    pub position: MirrorPosition,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateMirror {
    pub id: u32,
    #[serde(flatten)]
    pub mirror: AddMirror,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddExplodeStep {
    /// Component key paths to move.
    #[schemars(length(min = 1))]
    pub components: Vec<Vec<String>>,
    /// Translate along a direction, rotate about an axis, or spread radially.
    pub motion: ExplodeMotion,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateExplodeStep {
    pub id: u32,
    #[serde(flatten)]
    pub step: AddExplodeStep,
}

#[derive(Deserialize, JsonSchema)]
pub struct ShowExplode {
    /// Steps to show; omitted or null shows the assembly collapsed, and a
    /// number larger than the step count shows every step.
    #[serde(default)]
    pub steps: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Interference {
    /// Check only among these component key paths; empty checks everything.
    #[serde(default)]
    pub components: Vec<Vec<String>>,
    #[serde(default)]
    pub treat_coincidence: bool,
    #[serde(default)]
    pub subassemblies_as_components: bool,
    #[serde(default)]
    pub include_hidden: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct IgnoreInterference {
    pub a: Vec<String>,
    pub b: Vec<String>,
    #[serde(default = "yes")]
    pub ignored: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize, JsonSchema)]
pub struct Clearance {
    #[serde(default)]
    pub components: Vec<Vec<String>>,
    /// Report gaps smaller than this, mm.
    pub minimum: f64,
}

#[derive(Deserialize, JsonSchema)]
pub struct Bom {
    #[serde(default, rename = "type")]
    pub kind: of_assembly::bom::BomType,
    /// Also write the BOM as CSV here.
    #[serde(default)]
    pub csv: Option<PathBuf>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ImportStep {
    /// `.step` or `.stp` file with one or more parts.
    pub path: PathBuf,
    /// Folder for the generated part files; omitted uses a folder named
    /// after the STEP file beside it.
    #[serde(default)]
    pub folder: Option<PathBuf>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddReference {
    /// A reference plane, axis, point or coordinate system in the assembly.
    pub feature: FeatureKind,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct PickRef {
    /// Component key path of the picked instance.
    pub component: Vec<String>,
    pub kind: of_model::geom::Kind,
    /// Face, edge or vertex index in that component's part mesh.
    pub index: u32,
}

#[derive(Deserialize, JsonSchema)]
pub struct MeasureRefs {
    #[schemars(length(min = 1, max = 2))]
    pub refs: Vec<CompRef>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AssemblyConfigurationArgs {
    /// Assembly configuration ID.
    pub id: u32,
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateAssemblyConfiguration {
    pub name: String,
    /// Copy suppression, part configurations and explode steps from this one.
    #[serde(default)]
    pub copy_from: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ComponentPath {
    /// Component key path from `assembly_info` `components`, such as `["2"]`.
    #[schemars(length(min = 1))]
    pub component: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Defeature {
    /// The new part file.
    pub path: PathBuf,
    /// Boxes or cylinders around the components.
    pub mode: of_assembly::exchange::Silhouette,
}
