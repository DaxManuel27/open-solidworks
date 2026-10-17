//! The drawing document: plain data saved in `.ofd` files.
//!
//! Sheet coordinates are millimetres of paper with the origin at the sheet's
//! bottom-left corner and y up. Model coordinates are millimetres of model
//! space. Views and annotations never store hidden-line output: annotations
//! anchor to model geometry through [`ViewRef`]s, which survive rebuilds.

use std::collections::BTreeMap;

use of_assembly::CompRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A drawing scale `num:den`; 1:2 draws the model at half size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scale {
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub num: f64,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub den: f64,
}

impl Scale {
    pub const ONE: Scale = Scale { num: 1.0, den: 1.0 };

    pub fn factor(&self) -> f64 {
        self.num / self.den
    }

    /// The simplest `num:den` within 0.1% of a factor (whole numbers on
    /// one side when possible).
    pub fn from_factor(f: f64) -> Scale {
        if !(f.is_finite() && f > 0.0) {
            return Scale::ONE;
        }
        for den in 1..=100u32 {
            let num = (f * den as f64).round();
            if num >= 1.0 && ((num / den as f64) - f).abs() <= f * 1e-3 {
                return Scale {
                    num,
                    den: den as f64,
                };
            }
        }
        if f < 1.0 {
            Scale {
                num: 1.0,
                den: (1.0 / f * 100.0).round() / 100.0,
            }
        } else {
            Scale { num: f, den: 1.0 }
        }
    }

    pub fn is_valid(&self) -> bool {
        self.num.is_finite() && self.den.is_finite() && self.num > 0.0 && self.den > 0.0
    }

    /// `1:2`, `2:1`, `1:1`, or decimals when needed.
    pub fn label(&self) -> String {
        let f = |v: f64| {
            if (v - v.round()).abs() < 1e-9 {
                format!("{}", v.round() as i64)
            } else {
                format!("{v}")
            }
        };
        format!("{}:{}", f(self.num), f(self.den))
    }
}

impl Default for Scale {
    fn default() -> Self {
        Scale::ONE
    }
}

/// Paper sizes. Standard sizes are landscape unless the sheet says portrait.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum SheetSize {
    A0,
    A1,
    A2,
    A3,
    A4,
    /// ANSI A, 8.5 x 11 in.
    AnsiA,
    AnsiB,
    AnsiC,
    AnsiD,
    AnsiE,
    Custom {
        #[schemars(range(min = 10.0, max = 10000.0))]
        width: f64,
        #[schemars(range(min = 10.0, max = 10000.0))]
        height: f64,
    },
}

impl SheetSize {
    /// Landscape width and height in mm (custom sizes as given).
    pub fn landscape(&self) -> (f64, f64) {
        const IN: f64 = 25.4;
        match *self {
            SheetSize::A0 => (1189.0, 841.0),
            SheetSize::A1 => (841.0, 594.0),
            SheetSize::A2 => (594.0, 420.0),
            SheetSize::A3 => (420.0, 297.0),
            SheetSize::A4 => (297.0, 210.0),
            SheetSize::AnsiA => (11.0 * IN, 8.5 * IN),
            SheetSize::AnsiB => (17.0 * IN, 11.0 * IN),
            SheetSize::AnsiC => (22.0 * IN, 17.0 * IN),
            SheetSize::AnsiD => (34.0 * IN, 22.0 * IN),
            SheetSize::AnsiE => (44.0 * IN, 34.0 * IN),
            SheetSize::Custom { width, height } => (width, height),
        }
    }

    pub fn name(&self) -> String {
        match self {
            SheetSize::A0 => "A0".into(),
            SheetSize::A1 => "A1".into(),
            SheetSize::A2 => "A2".into(),
            SheetSize::A3 => "A3".into(),
            SheetSize::A4 => "A4".into(),
            SheetSize::AnsiA => "A".into(),
            SheetSize::AnsiB => "B".into(),
            SheetSize::AnsiC => "C".into(),
            SheetSize::AnsiD => "D".into(),
            SheetSize::AnsiE => "E".into(),
            SheetSize::Custom { width, height } => format!("{width} x {height}"),
        }
    }
}

/// How projected views are arranged around their parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Projection {
    /// ISO E: the view of the right side is placed on the left.
    #[default]
    First,
    /// ASME: the view of the right side is placed on the right.
    Third,
}

/// The border and title block drawn on a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SheetFormat {
    /// A built-in format (`iso`, `ansi`) or a `.ofdt` file relative to the
    /// drawing.
    pub template: String,
    #[serde(default = "yes")]
    pub visible: bool,
}

fn yes() -> bool {
    true
}

impl Default for SheetFormat {
    fn default() -> Self {
        SheetFormat {
            template: "iso".into(),
            visible: true,
        }
    }
}

/// A model file shown in a view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModelSource {
    /// `.ofp` or `.ofa` file, relative to the drawing (forward slashes) or
    /// absolute while the drawing is unsaved.
    pub path: String,
    /// Content hash of the file when it was last loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Part or assembly configuration ID; `None` uses the file's active one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<u32>,
    /// Show a sheet metal part's flat pattern, looking at the flat's front.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flat: Option<FlatView>,
    /// Show only these bodies of a part (a member, a sub-weldment, a sheet
    /// metal body); every body when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bodies: Option<Vec<of_model::BodyId>>,
}

/// What a flat pattern view draws beside the outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FlatView {
    /// Bend lines, up and down on their own layers.
    #[serde(default = "yes")]
    pub bend_lines: bool,
    /// Bend notes (`UP 90° R1`) beside the bend lines.
    #[serde(default = "yes")]
    pub bend_notes: bool,
}

impl Default for FlatView {
    fn default() -> Self {
        FlatView {
            bend_lines: true,
            bend_notes: true,
        }
    }
}

/// A view direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Orientation {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
    Isometric,
    Dimetric,
    Trimetric,
    /// Any direction: `toward` points from the model at the viewer and `up`
    /// is the paper's up direction (made perpendicular to `toward`).
    Custom {
        toward: [f64; 3],
        up: [f64; 3],
    },
    /// Relative to model: planar face `front` faces the viewer and planar
    /// face `top` faces up (its normal is made perpendicular).
    Relative {
        front: CompRef,
        top: CompRef,
    },
}

/// Where a projected view sits relative to its parent on the sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Left,
    Right,
    Above,
    Below,
    AboveLeft,
    AboveRight,
    BelowLeft,
    BelowRight,
}

/// Line display of a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DisplayStyle {
    #[default]
    HiddenRemoved,
    HiddenVisible,
    Wireframe,
    ShadedEdges,
    Shaded,
}

/// How edges between tangent faces are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TangentEdges {
    #[default]
    Visible,
    /// In a phantom line font.
    Font,
    Removed,
}

/// The cutting line of a section view, in the parent view's model plane:
/// points are model coordinates projected into the parent (view-plane mm,
/// relative to the model origin, unscaled). Two points make a straight cut;
/// more make an offset (jogged) or aligned cut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CuttingLine {
    #[schemars(length(min = 2))]
    pub points: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    /// Straight or jogged cut through the whole model.
    #[default]
    Full,
    /// Half section: the cutting line's first segment cuts only up to its
    /// second point.
    Half,
    /// The last segment is rotated into the plane of the first.
    Aligned,
}

/// A closed profile on a parent view (view-plane mm relative to the model
/// origin, unscaled), used by detail, broken-out section and crop views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Profile {
    Circle { center: [f64; 2], radius: f64 },
    Polygon { points: Vec<[f64; 2]> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DetailStyle {
    #[default]
    Standard,
    Broken,
    Leader,
    NoLeader,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BreakStyle {
    #[default]
    Straight,
    Curved,
    ZigZag,
    SmallZigZag,
    Jagged,
}

/// One gap of a break view, along the view's x (vertical break lines) or
/// y axis (horizontal), in view-plane mm relative to the model origin.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Break {
    pub vertical: bool,
    pub from: f64,
    pub to: f64,
}

/// What a view shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ViewKind {
    /// A part or assembly in a named or custom orientation.
    Model {
        model: ModelSource,
        orientation: Orientation,
        /// Assemblies: show the active configuration's explode steps.
        #[serde(default)]
        exploded: bool,
    },
    /// Folded out from a parent view.
    Projected { parent: u32, placement: Placement },
    /// Normal to an edge of the parent view.
    Auxiliary {
        parent: u32,
        edge: CompRef,
        #[serde(default)]
        flip: bool,
    },
    Section {
        parent: u32,
        line: CuttingLine,
        #[serde(default)]
        kind: SectionKind,
        /// Look the other way along the cut.
        #[serde(default)]
        flip: bool,
        /// Section depth in mm beyond the cut (partial section); `None`
        /// shows everything behind it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        depth: Option<f64>,
        /// Show only the cut faces.
        #[serde(default)]
        slice: bool,
        /// Assembly components not cut (drawn whole), by key path.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        excluded: Vec<Vec<String>>,
        /// Leave hardware library parts uncut.
        #[serde(default = "yes")]
        exclude_fasteners: bool,
    },
    Detail {
        parent: u32,
        profile: Profile,
        #[serde(default)]
        style: DetailStyle,
        #[serde(default)]
        full_outline: bool,
    },
}

/// A view on a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct View {
    pub id: u32,
    pub name: String,
    pub kind: ViewKind,
    /// Sheet position of the model origin, mm.
    pub origin: [f64; 2],
    /// `None` uses the sheet scale (for root views) or the parent's scale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<Scale>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<DisplayStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tangent: Option<TangentEdges>,
    /// Letter for section, detail and auxiliary views (`A` labels `SECTION A-A`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Projected and auxiliary views stay aligned with their parent.
    #[serde(default = "yes")]
    pub aligned: bool,
    /// Break gaps (a break view).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breaks: Vec<Break>,
    #[serde(default)]
    pub break_style: BreakStyle,
    /// Gap between break lines, mm of paper.
    #[serde(default = "break_gap")]
    pub break_gap: f64,
    /// Crop the view to a profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<Profile>,
    /// Broken-out sections: a profile cut to a depth (mm from the nearest
    /// point of the model toward the back).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub broken_out: Vec<BrokenOut>,
    /// Section and broken-out hatching: pattern (default: by material),
    /// scale and angle.
    #[serde(default, skip_serializing_if = "HatchOptions::is_default")]
    pub hatch: HatchOptions,
    /// Model edges not drawn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_edges: Vec<CompRef>,
    /// Assembly components not drawn, by key path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_components: Vec<Vec<String>>,
}

fn break_gap() -> f64 {
    10.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HatchOptions {
    /// `None` picks a pattern from each body's material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<crate::hatch::Pattern>,
    #[serde(default = "one")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub scale: f64,
    /// Radians added to the pattern's own angle.
    #[serde(default)]
    pub angle: f64,
}

fn one() -> f64 {
    1.0
}

impl Default for HatchOptions {
    fn default() -> Self {
        HatchOptions {
            pattern: None,
            scale: 1.0,
            angle: 0.0,
        }
    }
}

impl HatchOptions {
    pub fn is_default(&self) -> bool {
        *self == HatchOptions::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BrokenOut {
    pub profile: Profile,
    pub depth: f64,
}

impl View {
    pub fn new(id: u32, name: String, kind: ViewKind, origin: [f64; 2]) -> View {
        View {
            id,
            name,
            kind,
            origin,
            scale: None,
            display: None,
            tangent: None,
            label: None,
            aligned: true,
            breaks: Vec::new(),
            break_style: BreakStyle::default(),
            break_gap: break_gap(),
            crop: None,
            broken_out: Vec::new(),
            hatch: HatchOptions::default(),
            hidden_edges: Vec::new(),
            hidden_components: Vec::new(),
        }
    }

    pub fn parent(&self) -> Option<u32> {
        match &self.kind {
            ViewKind::Model { .. } => None,
            ViewKind::Projected { parent, .. }
            | ViewKind::Auxiliary { parent, .. }
            | ViewKind::Section { parent, .. }
            | ViewKind::Detail { parent, .. } => Some(*parent),
        }
    }
}

/// Model geometry seen in a view: an edge, face, vertex or reference entity
/// of the view's model (a part: empty component path).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ViewRef {
    pub view: u32,
    pub entity: CompRef,
}

/// Where an annotation is attached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Anchor {
    /// A fixed point on the sheet, mm.
    Sheet { at: [f64; 2] },
    /// A point on model geometry; `at` (view-plane mm relative to the model
    /// origin, unscaled) picks the place along an edge or face.
    Model { target: ViewRef, at: [f64; 2] },
}

/// A sheet: paper, format and the views, annotations and tables on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Sheet {
    pub id: u32,
    pub name: String,
    pub size: SheetSize,
    #[serde(default)]
    pub portrait: bool,
    #[serde(default)]
    pub scale: Scale,
    #[serde(default)]
    pub projection: Projection,
    #[serde(default)]
    pub format: SheetFormat,
    /// View whose model feeds `$PRP` title block fields; the first view when
    /// unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties_view: Option<u32>,
    #[serde(default)]
    pub views: Vec<View>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<crate::annotations::Annotation>,
}

impl Sheet {
    /// Paper width and height in mm, with orientation applied.
    pub fn paper(&self) -> (f64, f64) {
        let (w, h) = self.size.landscape();
        let (long, short) = (w.max(h), w.min(h));
        match (self.size, self.portrait) {
            (SheetSize::Custom { .. }, _) => (w, h),
            (_, false) => (long, short),
            (_, true) => (short, long),
        }
    }

    pub fn view(&self, id: u32) -> Option<&View> {
        self.views.iter().find(|v| v.id == id)
    }

    pub fn view_mut(&mut self, id: u32) -> Option<&mut View> {
        self.views.iter_mut().find(|v| v.id == id)
    }
}

/// A drawing: sheets plus document-wide options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DrawingDocument {
    #[serde(default)]
    pub standard: crate::standard::Standard,
    pub sheets: Vec<Sheet>,
    /// Drawing custom properties (`DrawnBy`, `CheckedBy`, ...), used by
    /// `$PRPSHEET` title block fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
    /// Next ID for sheets, views, annotations and tables (shared).
    pub next_id: u32,
}

impl Default for DrawingDocument {
    fn default() -> Self {
        DrawingDocument {
            standard: Default::default(),
            sheets: Vec::new(),
            properties: BTreeMap::new(),
            next_id: 1,
        }
    }
}

impl DrawingDocument {
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn sheet(&self, id: u32) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.id == id)
    }

    pub fn sheet_mut(&mut self, id: u32) -> Option<&mut Sheet> {
        self.sheets.iter_mut().find(|s| s.id == id)
    }

    /// The sheet holding a view.
    pub fn sheet_of_view(&self, view: u32) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.view(view).is_some())
    }

    /// Structural checks run after loading and before saving.
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for s in &self.sheets {
            if !ids.insert(s.id) {
                return Err(format!("duplicate ID {}", s.id));
            }
            if !s.scale.is_valid() {
                return Err(format!("sheet {} has an invalid scale", s.name));
            }
            let (w, h) = s.paper();
            if !(w.is_finite() && h.is_finite() && w >= 10.0 && h >= 10.0) {
                return Err(format!("sheet {} has an invalid size", s.name));
            }
            for v in &s.views {
                if !ids.insert(v.id) {
                    return Err(format!("duplicate ID {}", v.id));
                }
                if v.scale.is_some_and(|sc| !sc.is_valid()) {
                    return Err(format!("view {} has an invalid scale", v.name));
                }
                if !v.origin.iter().all(|c| c.is_finite()) {
                    return Err(format!("view {} has an invalid position", v.name));
                }
            }
            for v in &s.views {
                if let Some(p) = v.parent()
                    && s.view(p).is_none()
                {
                    return Err(format!(
                        "view {} refers to view {p}, which is not on its sheet",
                        v.name
                    ));
                }
            }
            for a in &s.annotations {
                if !ids.insert(a.id) {
                    return Err(format!("duplicate ID {}", a.id));
                }
            }
        }
        if ids.iter().any(|id| *id >= self.next_id) {
            return Err("next_id must be above every ID".into());
        }
        // Parent chains must end at a model view.
        for s in &self.sheets {
            for v in &s.views {
                let mut at = v;
                let mut steps = 0;
                while let Some(p) = at.parent() {
                    steps += 1;
                    if steps > s.views.len() {
                        return Err(format!("view {} is its own ancestor", v.name));
                    }
                    at = s.view(p).expect("checked above");
                }
            }
        }
        Ok(())
    }
}
