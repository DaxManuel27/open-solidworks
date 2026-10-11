//! Feature definitions: the parameters a user edits in each feature's panel.
//! Lengths are millimetres and angles radians.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{FeatureId, Ref};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchFeature {
    /// A standard plane, reference plane or planar face.
    pub plane: Ref,
    pub sketch: of_sketch::Sketch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum PlaneDef {
    /// Parallel to a plane or planar face, `distance` along its normal.
    Offset {
        reference: Ref,
        /// mm along the reference normal.
        distance: f64,
        #[serde(default)]
        flip: bool,
    },
    /// Through a line, at `angle` to a plane.
    Angle {
        reference: Ref,
        axis: Ref,
        /// Radians.
        angle: f64,
    },
    /// Through three points.
    ThreePoints { points: [Ref; 3] },
    /// Halfway between two planes or faces.
    MidPlane { a: Ref, b: Ref },
    /// Parallel to `reference`, through `point`.
    ParallelAtPoint { reference: Ref, point: Ref },
    /// Normal to an edge at its start (or end).
    NormalToCurve {
        edge: Ref,
        #[serde(default)]
        at_end: bool,
    },
}

impl PlaneDef {
    pub fn refs(&self) -> Vec<&Ref> {
        match self {
            PlaneDef::Offset { reference, .. } => vec![reference],
            PlaneDef::Angle {
                reference, axis, ..
            } => vec![reference, axis],
            PlaneDef::ThreePoints { points } => points.iter().collect(),
            PlaneDef::MidPlane { a, b } => vec![a, b],
            PlaneDef::ParallelAtPoint { reference, point } => vec![reference, point],
            PlaneDef::NormalToCurve { edge, .. } => vec![edge],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum AxisDef {
    /// Through two points.
    TwoPoints { a: Ref, b: Ref },
    /// Where two planes meet.
    TwoPlanes { a: Ref, b: Ref },
    /// The axis of a cylindrical or conical face, or a circular edge.
    Cylinder { face: Ref },
    /// Along a straight edge or sketch line.
    Edge { edge: Ref },
    /// Through a point, normal to a plane.
    PointAndFace { point: Ref, face: Ref },
}

impl AxisDef {
    pub fn refs(&self) -> Vec<&Ref> {
        match self {
            AxisDef::TwoPoints { a, b } | AxisDef::TwoPlanes { a, b } => vec![a, b],
            AxisDef::Cylinder { face } => vec![face],
            AxisDef::Edge { edge } => vec![edge],
            AxisDef::PointAndFace { point, face } => vec![point, face],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum PointDef {
    /// Model coordinates, mm.
    Coordinates { x: f64, y: f64, z: f64 },
    /// On a vertex or sketch point; at the center of a face or circular edge.
    At { target: Ref },
}

impl PointDef {
    pub fn refs(&self) -> Vec<&Ref> {
        match self {
            PointDef::Coordinates { .. } => vec![],
            PointDef::At { target } => vec![target],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CoordSysDef {
    pub origin: Ref,
    #[serde(default)]
    pub x_axis: Option<Ref>,
    #[serde(default)]
    pub z_axis: Option<Ref>,
}

/// How far an extrude goes in one direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EndCondition {
    /// A fixed distance from the sketch plane.
    Blind {
        /// mm, positive.
        #[schemars(extend("exclusiveMinimum" = 0))]
        depth: f64,
    },
    /// Through every body in the extrude direction.
    ThroughAll,
    /// To the next face that blocks the whole profile.
    UpToNext,
    UpToVertex {
        target: Ref,
    },
    UpToSurface {
        target: Ref,
    },
    OffsetFromSurface {
        target: Ref,
        /// mm from the target surface.
        #[schemars(range(min = 0.0))]
        offset: f64,
        #[serde(default)]
        reverse_offset: bool,
    },
    /// `depth` total, split equally either side of the sketch plane.
    MidPlane {
        /// mm, positive.
        #[schemars(extend("exclusiveMinimum" = 0))]
        depth: f64,
    },
}

impl EndCondition {
    pub fn target(&self) -> Option<&Ref> {
        match self {
            EndCondition::UpToVertex { target }
            | EndCondition::UpToSurface { target }
            | EndCondition::OffsetFromSurface { target, .. } => Some(target),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThinKind {
    OneDirection,
    MidPlane,
    TwoDirection,
}

/// A thin feature: the sketch contour thickened into a wall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Thin {
    pub kind: ThinKind,
    /// Wall thickness, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub thickness: f64,
    /// Second-side thickness for `two_direction`, mm.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub thickness2: f64,
    #[serde(default)]
    pub reverse: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Extrude {
    /// Sketch feature ID with closed profiles.
    #[schemars(range(min = 1))]
    pub sketch: FeatureId,
    /// Remove material instead of adding it.
    #[serde(default)]
    pub cut: bool,
    /// Start this far from the sketch plane, mm.
    #[serde(default)]
    pub start_offset: f64,
    pub end1: EndCondition,
    /// Extrude against the sketch normal. A cut on a face sketch needs
    /// `true` to go into the material.
    #[serde(default)]
    pub reverse: bool,
    /// Also extrude the other way.
    #[serde(default)]
    pub end2: Option<EndCondition>,
    #[serde(default)]
    pub thin: Option<Thin>,
    /// Cut away everything outside the profile instead.
    #[serde(default)]
    pub flip_side_to_cut: bool,
    /// Merge with existing bodies (bosses only).
    #[serde(default = "yes")]
    pub merge: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevolveAngle {
    Blind {
        /// Radians; 2π (6.2832) is a full turn.
        #[schemars(extend("exclusiveMinimum" = 0), range(max = std::f64::consts::TAU))]
        angle: f64,
    },
    /// `angle` total, split equally either side of the sketch plane.
    MidPlane {
        /// Radians.
        #[schemars(extend("exclusiveMinimum" = 0), range(max = std::f64::consts::TAU))]
        angle: f64,
    },
    TwoDirection {
        /// Radians.
        #[schemars(range(min = 0.0, max = std::f64::consts::TAU))]
        angle1: f64,
        /// Radians.
        #[schemars(range(min = 0.0, max = std::f64::consts::TAU))]
        angle2: f64,
    },
    UpToVertex {
        target: Ref,
    },
    UpToSurface {
        target: Ref,
    },
    OffsetFromSurface {
        target: Ref,
        /// mm from the target surface.
        #[schemars(range(min = 0.0))]
        offset: f64,
        #[serde(default)]
        reverse_offset: bool,
    },
}

impl RevolveAngle {
    pub fn target(&self) -> Option<&Ref> {
        match self {
            Self::UpToVertex { target }
            | Self::UpToSurface { target }
            | Self::OffsetFromSurface { target, .. } => Some(target),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Revolve {
    #[schemars(range(min = 1))]
    pub sketch: FeatureId,
    /// A sketch line, reference axis or straight edge.
    pub axis: Ref,
    pub angle: RevolveAngle,
    /// Optional independent end condition in the opposite direction.
    #[serde(default)]
    pub end2: Option<RevolveAngle>,
    #[serde(default)]
    pub reverse: bool,
    #[serde(default)]
    pub cut: bool,
    #[serde(default)]
    pub thin: Option<Thin>,
    #[serde(default = "yes")]
    pub merge: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fillet {
    #[serde(default)]
    pub edges: Vec<Ref>,
    /// All edges of these faces are filleted.
    #[serde(default)]
    pub faces: Vec<Ref>,
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RadiusStation {
    /// Fraction along the edge, 0 to 1.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub position: f64,
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VariableFillet {
    pub edge: Ref,
    /// Ordered stations; include positions 0 and 1.
    #[schemars(length(min = 2))]
    pub stations: Vec<RadiusStation>,
    #[serde(default)]
    pub reverse: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Draft {
    #[schemars(length(min = 1))]
    pub faces: Vec<Ref>,
    pub neutral_plane: Ref,
    pub direction: Option<Ref>,
    /// Signed draft angle, radians.
    #[schemars(range(min = -std::f64::consts::FRAC_PI_2, max = std::f64::consts::FRAC_PI_2))]
    pub angle: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChamferMode {
    /// Equal setbacks, mm.
    Equal {
        #[schemars(extend("exclusiveMinimum" = 0))]
        distance: f64,
    },
    /// Two setbacks, mm.
    DistanceDistance {
        #[schemars(extend("exclusiveMinimum" = 0))]
        d1: f64,
        #[schemars(extend("exclusiveMinimum" = 0))]
        d2: f64,
    },
    /// Setback in mm and angle in radians.
    AngleDistance {
        #[schemars(extend("exclusiveMinimum" = 0))]
        distance: f64,
        #[schemars(extend("exclusiveMinimum" = 0, "exclusiveMaximum" = std::f64::consts::FRAC_PI_2))]
        angle: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Chamfer {
    #[serde(default)]
    pub edges: Vec<Ref>,
    #[serde(default)]
    pub faces: Vec<Ref>,
    pub mode: ChamferMode,
    /// Swap which face each distance is measured on.
    #[serde(default)]
    pub flip: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShellDef {
    /// Faces to remove. None hollows the part completely.
    #[serde(default)]
    pub faces: Vec<Ref>,
    /// Wall thickness, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub thickness: f64,
    #[serde(default)]
    pub outward: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HoleKind {
    Simple,
    Counterbore,
    Countersink,
    Tapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HoleStandard {
    Iso,
    Ansi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HoleFit {
    Close,
    Normal,
    Loose,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HoleEnd {
    Blind {
        /// mm.
        #[schemars(extend("exclusiveMinimum" = 0))]
        depth: f64,
    },
    ThroughAll,
}

/// Hole Wizard: holes at each point of a sketch, drilled into the sketch
/// plane. Sizes come from the standard tables in [`crate::features::holes`];
/// any `*_override` replaces the table value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hole {
    /// Sketch whose standalone points locate the holes.
    #[schemars(range(min = 1))]
    pub sketch: FeatureId,
    pub kind: HoleKind,
    pub standard: HoleStandard,
    /// Fastener size, like "M6" or "1/4-20".
    pub size: String,
    #[serde(default = "normal_fit")]
    pub fit: HoleFit,
    pub end: HoleEnd,
    #[serde(default)]
    pub reverse: bool,
    /// mm.
    #[serde(default)]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub diameter_override: Option<f64>,
    /// mm.
    #[serde(default)]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub cbore_diameter_override: Option<f64>,
    /// mm.
    #[serde(default)]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub cbore_depth_override: Option<f64>,
    /// mm.
    #[serde(default)]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub csink_diameter_override: Option<f64>,
    /// Included angle, radians.
    #[serde(default)]
    #[schemars(extend("exclusiveMinimum" = 0, "exclusiveMaximum" = std::f64::consts::PI))]
    pub csink_angle_override: Option<f64>,
}

fn normal_fit() -> HoleFit {
    HoleFit::Normal
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PatternDirection {
    /// A straight edge, sketch line, axis, or plane/face (its normal).
    pub reference: Ref,
    /// Distance between instances, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub spacing: f64,
    /// Total instances including the original.
    #[schemars(range(min = 1))]
    pub count: u32,
    #[serde(default)]
    pub reverse: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LinearPattern {
    #[schemars(length(min = 1))]
    pub features: Vec<FeatureId>,
    pub direction1: PatternDirection,
    #[serde(default)]
    pub direction2: Option<PatternDirection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CircularPattern {
    #[schemars(length(min = 1))]
    pub features: Vec<FeatureId>,
    /// An axis, cylindrical face, circular edge or straight edge.
    pub axis: Ref,
    /// Total instances including the original.
    #[schemars(range(min = 1))]
    pub count: u32,
    /// Total angle with equal spacing, or the angle between instances; radians.
    #[schemars(extend("exclusiveMinimum" = 0), range(max = std::f64::consts::TAU))]
    pub angle: f64,
    #[serde(default = "yes")]
    pub equal_spacing: bool,
    #[serde(default)]
    pub reverse: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorDef {
    /// Features to mirror. Empty mirrors the whole body.
    #[serde(default)]
    pub features: Vec<FeatureId>,
    /// A plane or planar face.
    pub plane: Ref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    Step,
    Iges,
}

/// A body imported from a STEP or IGES file, stored inside the document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Import {
    pub format: ImportFormat,
    /// File name it came from, for display.
    pub source: String,
    /// The file's contents, base64-encoded.
    pub data: String,
}

pub mod holes {
    //! Hole sizes for the Hole Wizard. Values are in millimetres.

    use super::{HoleFit, HoleStandard};

    /// Dimensions for one fastener size.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct HoleSize {
        pub name: &'static str,
        /// Nominal (major) diameter.
        pub nominal: f64,
        /// Tap drill diameter.
        pub tap_drill: f64,
        /// Clearance hole diameters: close, normal, loose fit.
        pub clearance: [f64; 3],
        /// Counterbore diameter and depth for a socket head cap screw.
        pub cbore: (f64, f64),
        /// Countersink diameter for a flat head screw.
        pub csink: f64,
    }

    /// ISO metric coarse sizes (ISO 273 clearance, ISO 4762 counterbores,
    /// ISO 10642 countersinks).
    pub const ISO: &[HoleSize] = &[
        HoleSize {
            name: "M2",
            nominal: 2.0,
            tap_drill: 1.6,
            clearance: [2.2, 2.4, 2.6],
            cbore: (4.3, 2.2),
            csink: 4.4,
        },
        HoleSize {
            name: "M2.5",
            nominal: 2.5,
            tap_drill: 2.05,
            clearance: [2.7, 2.9, 3.1],
            cbore: (5.0, 2.7),
            csink: 5.5,
        },
        HoleSize {
            name: "M3",
            nominal: 3.0,
            tap_drill: 2.5,
            clearance: [3.2, 3.4, 3.6],
            cbore: (6.5, 3.3),
            csink: 6.7,
        },
        HoleSize {
            name: "M4",
            nominal: 4.0,
            tap_drill: 3.3,
            clearance: [4.3, 4.5, 4.8],
            cbore: (8.0, 4.4),
            csink: 9.0,
        },
        HoleSize {
            name: "M5",
            nominal: 5.0,
            tap_drill: 4.2,
            clearance: [5.3, 5.5, 5.8],
            cbore: (10.0, 5.4),
            csink: 11.2,
        },
        HoleSize {
            name: "M6",
            nominal: 6.0,
            tap_drill: 5.0,
            clearance: [6.4, 6.6, 7.0],
            cbore: (11.0, 6.4),
            csink: 13.4,
        },
        HoleSize {
            name: "M8",
            nominal: 8.0,
            tap_drill: 6.8,
            clearance: [8.4, 9.0, 10.0],
            cbore: (15.0, 8.6),
            csink: 17.9,
        },
        HoleSize {
            name: "M10",
            nominal: 10.0,
            tap_drill: 8.5,
            clearance: [10.5, 11.0, 12.0],
            cbore: (18.0, 10.6),
            csink: 22.4,
        },
        HoleSize {
            name: "M12",
            nominal: 12.0,
            tap_drill: 10.2,
            clearance: [13.0, 13.5, 14.5],
            cbore: (20.0, 12.6),
            csink: 26.9,
        },
        HoleSize {
            name: "M16",
            nominal: 16.0,
            tap_drill: 14.0,
            clearance: [17.0, 17.5, 18.5],
            cbore: (26.0, 16.6),
            csink: 33.6,
        },
        HoleSize {
            name: "M20",
            nominal: 20.0,
            tap_drill: 17.5,
            clearance: [21.0, 22.0, 24.0],
            cbore: (33.0, 20.6),
            csink: 40.3,
        },
        HoleSize {
            name: "M24",
            nominal: 24.0,
            tap_drill: 21.0,
            clearance: [25.0, 26.0, 28.0],
            cbore: (40.0, 24.8),
            csink: 47.0,
        },
    ];

    const IN: f64 = 25.4;

    /// ANSI inch sizes (UNC), converted to millimetres.
    pub const ANSI: &[HoleSize] = &[
        HoleSize {
            name: "#4-40",
            nominal: 0.112 * IN,
            tap_drill: 0.089 * IN,
            clearance: [0.116 * IN, 0.120 * IN, 0.128 * IN],
            cbore: (0.219 * IN, 0.112 * IN),
            csink: 0.255 * IN,
        },
        HoleSize {
            name: "#6-32",
            nominal: 0.138 * IN,
            tap_drill: 0.1065 * IN,
            clearance: [0.144 * IN, 0.149 * IN, 0.157 * IN],
            cbore: (0.270 * IN, 0.138 * IN),
            csink: 0.307 * IN,
        },
        HoleSize {
            name: "#8-32",
            nominal: 0.164 * IN,
            tap_drill: 0.136 * IN,
            clearance: [0.170 * IN, 0.177 * IN, 0.185 * IN],
            cbore: (0.313 * IN, 0.164 * IN),
            csink: 0.359 * IN,
        },
        HoleSize {
            name: "#10-24",
            nominal: 0.190 * IN,
            tap_drill: 0.1495 * IN,
            clearance: [0.196 * IN, 0.201 * IN, 0.210 * IN],
            cbore: (0.375 * IN, 0.190 * IN),
            csink: 0.411 * IN,
        },
        HoleSize {
            name: "1/4-20",
            nominal: 0.250 * IN,
            tap_drill: 0.201 * IN,
            clearance: [0.257 * IN, 0.266 * IN, 0.281 * IN],
            cbore: (0.438 * IN, 0.250 * IN),
            csink: 0.531 * IN,
        },
        HoleSize {
            name: "5/16-18",
            nominal: 0.3125 * IN,
            tap_drill: 0.257 * IN,
            clearance: [0.323 * IN, 0.332 * IN, 0.344 * IN],
            cbore: (0.531 * IN, 0.3125 * IN),
            csink: 0.656 * IN,
        },
        HoleSize {
            name: "3/8-16",
            nominal: 0.375 * IN,
            tap_drill: 0.3125 * IN,
            clearance: [0.386 * IN, 0.397 * IN, 0.406 * IN],
            cbore: (0.625 * IN, 0.375 * IN),
            csink: 0.781 * IN,
        },
        HoleSize {
            name: "1/2-13",
            nominal: 0.500 * IN,
            tap_drill: 0.4219 * IN,
            clearance: [0.515 * IN, 0.531 * IN, 0.562 * IN],
            cbore: (0.813 * IN, 0.500 * IN),
            csink: 1.031 * IN,
        },
        HoleSize {
            name: "5/8-11",
            nominal: 0.625 * IN,
            tap_drill: 0.5312 * IN,
            clearance: [0.640 * IN, 0.656 * IN, 0.688 * IN],
            cbore: (1.000 * IN, 0.625 * IN),
            csink: 1.281 * IN,
        },
        HoleSize {
            name: "3/4-10",
            nominal: 0.750 * IN,
            tap_drill: 0.6562 * IN,
            clearance: [0.765 * IN, 0.781 * IN, 0.812 * IN],
            cbore: (1.125 * IN, 0.750 * IN),
            csink: 1.531 * IN,
        },
    ];

    pub fn table(standard: HoleStandard) -> &'static [HoleSize] {
        match standard {
            HoleStandard::Iso => ISO,
            HoleStandard::Ansi => ANSI,
        }
    }

    pub fn find(standard: HoleStandard, name: &str) -> Option<&'static HoleSize> {
        table(standard)
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn clearance(size: &HoleSize, fit: HoleFit) -> f64 {
        size.clearance[match fit {
            HoleFit::Close => 0,
            HoleFit::Normal => 1,
            HoleFit::Loose => 2,
        }]
    }
}
