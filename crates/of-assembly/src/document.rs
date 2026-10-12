//! The assembly document: plain data saved in `.ofa` files.

use std::collections::BTreeMap;

use of_mate::{DQuat, DVec3, Pose};
use of_model::{Document, Ref, Units};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A rigid placement: translation in mm and a unit quaternion `[x, y, z, w]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transform {
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
}

impl Default for Transform {
    fn default() -> Self {
        Transform::from(Pose::IDENTITY)
    }
}

impl From<Pose> for Transform {
    fn from(p: Pose) -> Self {
        let q = p.rotation.normalize();
        Transform {
            translation: p.translation.to_array(),
            rotation: [q.x, q.y, q.z, q.w],
        }
    }
}

impl From<Transform> for Pose {
    fn from(t: Transform) -> Self {
        let [x, y, z, w] = t.rotation;
        let q = DQuat::from_xyzw(x, y, z, w);
        let q = if q.length_squared() > 1e-24 {
            q.normalize()
        } else {
            DQuat::IDENTITY
        };
        Pose::new(DVec3::from(t.translation), q)
    }
}

impl Transform {
    pub fn is_valid(&self) -> bool {
        self.translation
            .iter()
            .chain(&self.rotation)
            .all(|v| v.is_finite())
            && self.rotation.iter().map(|v| v * v).sum::<f64>() > 1e-24
    }
}

/// An instance of a part or subassembly file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Component {
    pub id: u32,
    /// Instance name shown in the tree, such as `bracket<1>`.
    pub name: String,
    /// `.ofp` or `.ofa` file, relative to the assembly file (forward slashes)
    /// or absolute while the assembly is unsaved.
    pub path: String,
    /// Content hash of the file when it was last loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Part configuration ID; `None` uses the configuration saved in the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<u32>,
    #[serde(default)]
    pub transform: Transform,
    /// Fixed components cannot move; the first inserted component is fixed.
    #[serde(default)]
    pub fixed: bool,
    #[serde(default)]
    pub suppressed: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub exclude_from_bom: bool,
    /// Subassemblies only: solve the children with this assembly's mates.
    #[serde(default)]
    pub flexible: bool,
    /// Flexible subassemblies: child poses relative to this component, by
    /// child key path (`"3"`, `"3/2"`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub flexible_poses: BTreeMap<String, Transform>,
}

/// A reference to geometry in the assembly: a component path (empty for the
/// assembly's own planes, axes and sketches) and a part reference inside it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct CompRef {
    /// Instance keys from the top: `["2"]`, `["4", "1"]` inside subassembly
    /// 4, or `["2@p7.1"]` for a pattern instance. Empty means the assembly.
    #[serde(default)]
    pub component: Vec<String>,
    /// Face, edge, vertex, plane, axis or point inside that component.
    pub entity: Ref,
}

/// Direction sense for mates between oriented entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    Aligned,
    AntiAligned,
    /// Whichever is nearer to the current positions when the mate is made.
    #[default]
    Closest,
}

/// A range for limit mates, in mm or radians.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Limit {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WidthConstraint {
    /// The tab stays midway between the width faces.
    Centered,
    /// The tab may slide anywhere between the width faces.
    Free,
    /// Distance from the first width face to the first tab face, mm.
    Dimension {
        distance: f64,
        #[serde(default)]
        flip: bool,
    },
    /// Position of the tab across the free travel, 0 to 1.
    Percent { percent: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TravelConstraint {
    /// Anywhere along the path or slot.
    Free,
    /// Slots only: at the middle.
    Center,
    /// At a distance along it from its start, mm.
    Distance {
        distance: f64,
        #[serde(default)]
        flip: bool,
    },
    /// At a fraction of its length, 0 to 1.
    Percent {
        percent: f64,
        #[serde(default)]
        flip: bool,
    },
}

/// The mate types: standard, advanced and mechanical.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MateKind {
    /// Faces, edges, planes, points or axes touch or line up.
    Coincident {
        a: CompRef,
        b: CompRef,
        #[serde(default)]
        alignment: Alignment,
    },
    Parallel {
        a: CompRef,
        b: CompRef,
        #[serde(default)]
        alignment: Alignment,
    },
    Perpendicular {
        a: CompRef,
        b: CompRef,
    },
    /// A cylinder or sphere touches a plane, cylinder or sphere.
    Tangent {
        a: CompRef,
        b: CompRef,
        #[serde(default)]
        alignment: Alignment,
    },
    /// Axes of cylinders, cones, circles or axes line up.
    Concentric {
        a: CompRef,
        b: CompRef,
        #[serde(default)]
        alignment: Alignment,
        /// Also stop rotation about the shared axis.
        #[serde(default)]
        lock_rotation: bool,
    },
    /// Keep two components where they are relative to each other.
    Lock {
        a: Vec<String>,
        b: Vec<String>,
    },
    /// A distance in mm; with `limit`, anywhere in the range.
    Distance {
        a: CompRef,
        b: CompRef,
        distance: f64,
        #[serde(default)]
        flip: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<Limit>,
    },
    /// An angle in radians; with `limit`, anywhere in the range.
    Angle {
        a: CompRef,
        b: CompRef,
        angle: f64,
        #[serde(default)]
        flip: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<Limit>,
    },
    /// Centers planar faces (rectangular or circular profiles) on each other.
    ProfileCenter {
        a: CompRef,
        b: CompRef,
        #[serde(default)]
        offset: f64,
        #[serde(default)]
        flip: bool,
        #[serde(default)]
        lock_rotation: bool,
    },
    /// Two entities symmetric about a plane.
    Symmetric {
        a: CompRef,
        b: CompRef,
        plane: CompRef,
    },
    /// Centers a tab between two width faces.
    Width {
        width: [CompRef; 2],
        tab: [CompRef; 2],
        constraint: WidthConstraint,
    },
    /// A point follows a path of edges or curves.
    Path {
        point: CompRef,
        #[schemars(length(min = 1))]
        path: Vec<CompRef>,
        constraint: TravelConstraint,
        /// Keep this axis of the moving component tangent to the path.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        follow: Option<CompRef>,
        /// Roll control: keep this axis of the moving component toward `up`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        roll: Option<Roll>,
    },
    /// Two components translate in a ratio along their directions.
    LinearCoupler {
        a: CompRef,
        b: CompRef,
        /// When `a` moves `ratio_a`, `b` moves `ratio_b`.
        ratio_a: f64,
        ratio_b: f64,
        #[serde(default)]
        reverse: bool,
    },
    /// A point or cylinder follows a cam profile face.
    Cam {
        cam: CompRef,
        follower: CompRef,
    },
    /// An axis or cylinder moves in a slot.
    Slot {
        slot: CompRef,
        pin: CompRef,
        constraint: TravelConstraint,
    },
    /// Concentric plus coincident, with optional angle limits.
    Hinge {
        axis: [CompRef; 2],
        faces: [CompRef; 2],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<HingeLimit>,
    },
    /// Two components rotate in a ratio about their axes (teeth counts).
    Gear {
        a: CompRef,
        b: CompRef,
        ratio_a: f64,
        ratio_b: f64,
        #[serde(default)]
        reverse: bool,
    },
    /// The rack travels `distance_per_revolution` per pinion turn.
    RackPinion {
        rack: CompRef,
        pinion: CompRef,
        distance_per_revolution: f64,
        #[serde(default)]
        reverse: bool,
    },
    /// `b` advances `distance_per_revolution` along `a` per turn.
    Screw {
        a: CompRef,
        b: CompRef,
        distance_per_revolution: f64,
        #[serde(default)]
        reverse: bool,
    },
    /// Rotation of one shaft drives another through a cross joint.
    UniversalJoint {
        a: CompRef,
        b: CompRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        joint_point: Option<CompRef>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Roll {
    pub axis: CompRef,
    pub up: CompRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HingeLimit {
    pub faces: [CompRef; 2],
    pub min: f64,
    pub max: f64,
}

/// Values fixed when a mate is made: resolved alignment, relative poses,
/// reference vectors and coupler phases.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Captured {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative: Option<Transform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<f64>,
    /// Coupler motion values after the last solve (rotations unwrapped).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motions: Vec<f64>,
    /// Captured vectors and points, meaning depends on the mate type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vectors: Vec<[f64; 3]>,
    /// Tangent mates: internal (true) or external contact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal: Option<bool>,
    /// Which side of a plane the other entity stays on (+1 or −1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Mate {
    pub id: u32,
    pub name: String,
    #[serde(flatten)]
    pub kind: MateKind,
    #[serde(default)]
    pub suppressed: bool,
    #[serde(default)]
    pub captured: Captured,
}

/// One direction of a linear component pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ComponentPatternDirection")]
pub struct PatternDirection {
    /// Edge, axis or plane (normal) for the direction.
    pub reference: CompRef,
    pub spacing: f64,
    #[schemars(range(min = 1))]
    pub count: u32,
    #[serde(default)]
    pub reverse: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatternKind {
    Linear {
        direction1: PatternDirection,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction2: Option<PatternDirection>,
    },
    Circular {
        axis: CompRef,
        /// Angle between instances, or the total angle with `equal_spacing`.
        angle: f64,
        #[schemars(range(min = 1))]
        count: u32,
        #[serde(default)]
        equal_spacing: bool,
        #[serde(default)]
        reverse: bool,
    },
    /// Follow a linear, circular or mirror pattern feature in a part.
    PatternDriven {
        /// The pattern feature: a component path and `{type: feature, id}`.
        feature: CompRef,
    },
    /// One instance per point of an assembly sketch (`reference` features).
    SketchDriven {
        sketch: u32,
        /// The seed point the instances are placed by; default the seed's origin.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reference: Option<CompRef>,
    },
    /// Instances along a curve of edges.
    CurveDriven {
        #[schemars(length(min = 1))]
        curve: Vec<CompRef>,
        spacing: f64,
        #[schemars(range(min = 1))]
        count: u32,
        /// Rotate instances to follow the curve tangent.
        #[serde(default)]
        tangent: bool,
        #[serde(default)]
        reverse: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pattern {
    pub id: u32,
    pub name: String,
    #[serde(flatten)]
    pub kind: PatternKind,
    /// Seed component keys at the top level.
    #[schemars(length(min = 1))]
    pub seeds: Vec<String>,
    /// Instance indices to leave out (1-based; 0 is the seed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<u32>,
    #[serde(default)]
    pub suppressed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MirrorPosition {
    /// Mirror the center of the bounding box.
    #[default]
    BoundingBox,
    CenterOfMass,
    /// Mirror the component origin.
    Origin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorItem {
    pub component: String,
    /// Write an opposite-hand part here (relative to the assembly) instead
    /// of placing a reoriented copy of the same part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opposite_hand: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorComponents {
    pub id: u32,
    pub name: String,
    pub plane: CompRef,
    #[schemars(length(min = 1))]
    pub components: Vec<MirrorItem>,
    #[serde(default)]
    pub position: MirrorPosition,
    #[serde(default)]
    pub suppressed: bool,
}

/// One explode step: move components along a direction, rotate them about
/// an axis, or spread them radially.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExplodeStep {
    pub id: u32,
    pub name: String,
    #[schemars(length(min = 1))]
    pub components: Vec<Vec<String>>,
    #[serde(flatten)]
    pub motion: ExplodeMotion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExplodeMotion {
    /// Move along a unit direction in the assembly frame.
    Translate {
        direction: [f64; 3],
        distance: f64,
        /// Space components evenly: each moves `distance` further than the last.
        #[serde(default)]
        auto_space: bool,
    },
    /// Rotate about an axis through `origin`.
    Rotate {
        origin: [f64; 3],
        axis: [f64; 3],
        angle: f64,
    },
    /// Push components away from an axis through `origin`.
    Radial {
        origin: [f64; 3],
        axis: [f64; 3],
        distance: f64,
    },
}

/// An assembly configuration: suppression, part configurations and an
/// exploded view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AssemblyConfiguration {
    pub id: u32,
    pub name: String,
    /// Component key to suppression, overriding the component's own state.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub suppressed: BTreeMap<String, bool>,
    /// Component key to part configuration ID.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub part_configurations: BTreeMap<String, u32>,
    /// Mate ID to suppression.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mates_suppressed: BTreeMap<String, bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explode: Vec<ExplodeStep>,
}

impl AssemblyConfiguration {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        AssemblyConfiguration {
            id,
            name: name.into(),
            suppressed: BTreeMap::new(),
            part_configurations: BTreeMap::new(),
            mates_suppressed: BTreeMap::new(),
            explode: Vec::new(),
        }
    }
}

fn default_configurations() -> Vec<AssemblyConfiguration> {
    vec![AssemblyConfiguration::new(1, "Default")]
}

fn one() -> u32 {
    1
}

/// The assembly document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssemblyDocument {
    pub units: Units,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub mates: Vec<Mate>,
    #[serde(default)]
    pub patterns: Vec<Pattern>,
    #[serde(default)]
    pub mirrors: Vec<MirrorComponents>,
    #[serde(default = "default_configurations")]
    pub configurations: Vec<AssemblyConfiguration>,
    #[serde(default = "one")]
    pub active_configuration: u32,
    /// Assembly-level planes, axes, points and sketches, as a part document
    /// that holds only reference features.
    #[serde(default)]
    pub reference: Document,
    /// Assembly custom properties (`Description`, `PartNo`, ...).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
    /// Interferences the user chose to ignore, as sorted component key paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored_interferences: Vec<[String; 2]>,
    pub next_id: u32,
}

impl Default for AssemblyDocument {
    fn default() -> Self {
        AssemblyDocument {
            units: Units::default(),
            components: Vec::new(),
            mates: Vec::new(),
            patterns: Vec::new(),
            mirrors: Vec::new(),
            configurations: default_configurations(),
            active_configuration: 1,
            reference: Document::default(),
            properties: BTreeMap::new(),
            ignored_interferences: Vec::new(),
            next_id: 1,
        }
    }
}

impl AssemblyDocument {
    pub fn new() -> Self {
        Self::default()
    }

    /// A fresh ID for a component, mate, pattern, mirror or explode step.
    pub fn take_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn component(&self, key: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.id.to_string() == key)
    }

    pub fn component_mut(&mut self, key: &str) -> Option<&mut Component> {
        self.components.iter_mut().find(|c| c.id.to_string() == key)
    }

    pub fn mate(&self, id: u32) -> Option<&Mate> {
        self.mates.iter().find(|m| m.id == id)
    }

    pub fn active(&self) -> &AssemblyConfiguration {
        self.configurations
            .iter()
            .find(|c| c.id == self.active_configuration)
            .or(self.configurations.first())
            .expect("an assembly always has a configuration")
    }

    pub fn active_mut(&mut self) -> &mut AssemblyConfiguration {
        let id = self.active_configuration;
        let index = self
            .configurations
            .iter()
            .position(|c| c.id == id)
            .unwrap_or(0);
        &mut self.configurations[index]
    }

    /// Whether a component is suppressed in the active configuration.
    pub fn is_suppressed(&self, c: &Component) -> bool {
        self.active()
            .suppressed
            .get(&c.id.to_string())
            .copied()
            .unwrap_or(c.suppressed)
    }

    /// The part configuration a component uses in the active configuration.
    pub fn part_configuration(&self, c: &Component) -> Option<u32> {
        self.active()
            .part_configurations
            .get(&c.id.to_string())
            .copied()
            .or(c.configuration)
    }

    pub fn is_mate_suppressed(&self, m: &Mate) -> bool {
        self.active()
            .mates_suppressed
            .get(&m.id.to_string())
            .copied()
            .unwrap_or(m.suppressed)
    }

    /// A default instance name: `stem<n>` with the next free number.
    pub fn next_name(&self, path: &str) -> String {
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("component")
            .to_string();
        let mut n = 1;
        while self
            .components
            .iter()
            .any(|c| c.name == format!("{stem}<{n}>"))
        {
            n += 1;
        }
        format!("{stem}<{n}>")
    }

    /// Check persistent identity and references before anything is built.
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        let all = self
            .components
            .iter()
            .map(|c| c.id)
            .chain(self.mates.iter().map(|m| m.id))
            .chain(self.patterns.iter().map(|p| p.id))
            .chain(self.mirrors.iter().map(|m| m.id))
            .chain(
                self.configurations
                    .iter()
                    .flat_map(|c| c.explode.iter().map(|s| s.id)),
            );
        for id in all {
            if id == 0 || id >= self.next_id || !ids.insert(id) {
                return Err(format!("invalid or duplicate assembly item id {id}"));
            }
        }
        for c in &self.components {
            if !c.transform.is_valid() {
                return Err(format!("component {} has an invalid transform", c.name));
            }
            if c.path.trim().is_empty() {
                return Err(format!("component {} has no file", c.name));
            }
        }
        let mut configs = std::collections::HashSet::new();
        if self.configurations.is_empty() {
            return Err("an assembly needs at least one configuration".into());
        }
        for c in &self.configurations {
            if !configs.insert(c.id) {
                return Err(format!("duplicate configuration id {}", c.id));
            }
        }
        if !configs.contains(&self.active_configuration) {
            return Err("the active configuration does not exist".into());
        }
        self.reference.validate()?;
        for f in &self.reference.features {
            use of_model::FeatureKind as K;
            if !matches!(
                f.kind,
                K::Plane(_)
                    | K::Axis(_)
                    | K::Point(_)
                    | K::CoordSys(_)
                    | K::Sketch(_)
                    | K::Sketch3d(_)
            ) {
                return Err(format!(
                    "assembly reference feature {} must be a plane, axis, point, coordinate system or sketch",
                    f.name
                ));
            }
        }
        Ok(())
    }
}
