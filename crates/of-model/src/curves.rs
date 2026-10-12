//! Referenceable construction curves and section/path consumers.
use crate::{FeatureId, Ref, Resolved, State, resolve};
use of_geom::{DVec3, Kind, Plane, Shape};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct CurveBody {
    pub shape: Shape,
    /// Source-derived identities. A source split into several projected pieces
    /// names them `#1`, `#2`, ... in order along the source.
    pub segments: BTreeMap<String, Shape>,
}

/// An explicit complete closed contour. The first traversed entity is its seam.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ContourSelection {
    pub sketch: FeatureId,
    #[schemars(length(min = 1))]
    pub entities: Vec<of_sketch::Id>,
    #[serde(default)]
    pub reverse: bool,
    /// Entity at the start/seam; must be a member of the selected contour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<of_sketch::Id>,
    /// Put the seam inside the start entity, this fraction along it in the
    /// traversal direction (0 < seam < 1): by length for lines, arcs and
    /// circles, by curve parameter for Bezier curves. The entity is split
    /// there, except a circle, which just starts there. Not for splines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seam: Option<OrderedFloat>,
}

/// A finite number that compares and hashes by value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct OrderedFloat(pub f64);
impl Eq for OrderedFloat {}
impl std::hash::Hash for OrderedFloat {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

/// Legacy sketch IDs retain their original whole-sketch behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ProfileSelection {
    Sketch(FeatureId),
    Contour(ContourSelection),
}
impl ProfileSelection {
    pub fn sketch(&self) -> FeatureId {
        match self {
            Self::Sketch(id) => *id,
            Self::Contour(c) => c.sketch,
        }
    }
}
impl From<FeatureId> for ProfileSelection {
    fn from(id: FeatureId) -> Self {
        Self::Sketch(id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Helix {
    /// Base plane; the helix axis is its normal through its origin.
    pub plane: Ref,
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub radius: f64,
    /// mm per turn.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub pitch: f64,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub turns: f64,
    /// Radians.
    #[serde(default)]
    pub start_angle: f64,
    #[serde(default)]
    pub left_handed: bool,
    /// Which two of pitch, turns and height define the helix.
    #[serde(default)]
    pub defined_by: HelixInput,
    /// Total height, mm, for the height-based definitions.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub height: f64,
    /// Pitch rows for a variable-pitch helix: the first at turn 0, the last at
    /// the total turns. Pitch changes linearly in turn count between rows. When
    /// present it replaces pitch, turns and height.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variable_pitch: Vec<HelixPitch>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HelixInput {
    #[default]
    PitchAndTurns,
    HeightAndTurns,
    HeightAndPitch,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HelixPitch {
    /// Turn count at which this pitch applies.
    #[schemars(range(min = 0.0))]
    pub turn: f64,
    /// mm per turn.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub pitch: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CurveSelection {
    pub reference: Ref,
    #[serde(default)]
    pub reverse: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CompositeCurve {
    /// Connected curves in order.
    #[schemars(length(min = 1))]
    pub segments: Vec<CurveSelection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectedCurve {
    /// A sketch or curve feature to project; the second sketch for sketch on
    /// sketch; the plane, face or body that cuts the target for an intersection
    /// split line. Unused for silhouettes.
    #[serde(default = "origin")]
    pub source: Ref,
    /// A face or body; the first sketch for sketch on sketch. A split line splits
    /// this face.
    pub target: Ref,
    /// Projection direction, XYZ; the pull direction for silhouettes.
    #[schemars(with = "[f64; 3]")]
    pub direction: DVec3,
    #[serde(default)]
    pub mode: ProjectionMode,
}

fn origin() -> Ref {
    Ref::Origin
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionMode {
    /// Project the source curve along `direction` onto the target face or body.
    #[default]
    OntoFace,
    /// The curve where two sketches' extrusions normal to their planes meet.
    SketchOnSketch,
    /// Split line only: where the source plane, face or body cuts the target face.
    Intersection,
    /// Split line only: where the target face turns away from `direction`
    /// (cylindrical and spherical faces).
    Silhouette,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SweepOrientation {
    #[default]
    MinimumTwist,
    Frenet,
    Fixed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Sweep {
    /// A whole sketch with one closed profile, or one explicitly selected closed
    /// contour. Omit it to sweep a circle of `circle_diameter`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProfileSelection>,
    /// Explicit closed holes inside the selected outer contour, from the same sketch.
    #[serde(default)]
    pub inner_profiles: Vec<ContourSelection>,
    /// Sweep a circle of this diameter (mm), centred on the path start and normal
    /// to it, instead of a sketch profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub circle_diameter: Option<f64>,
    /// Turn the section by this angle (radians) from the path start to its end,
    /// spread by arc length. Replaces `orientation`.
    #[serde(default)]
    pub twist: f64,
    /// A curve beside the path that sets the section's rotation (follow path and
    /// guide). Replaces `orientation`. It scales the section only with
    /// `guide_scale`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide: Option<Ref>,
    /// With `guide`: also scale the section uniformly so the point where the
    /// guide crosses each plane normal to the path keeps its place on the
    /// section. Needs an open, tangent-continuous path.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub guide_scale: bool,
    /// A second guide curve. The section turns with the first guide and
    /// stretches so both guides keep their places on it. Needs `guide`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide2: Option<Ref>,
    /// Thicken the profile sketch's single open or closed chain instead of
    /// sweeping a closed region. Requires a whole-sketch profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thin: Option<crate::Thin>,
    /// One connected sketch, construction curve or edge.
    pub path: Ref,
    #[serde(default)]
    pub reverse_path: bool,
    #[serde(default)]
    pub orientation: SweepOrientation,
    #[serde(default)]
    pub cut: bool,
    #[serde(default = "yes")]
    pub merge: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Loft {
    /// Ordered closed sections; legacy sketch IDs or explicit contour/seam selections.
    #[schemars(length(min = 2))]
    pub profiles: Vec<ProfileSelection>,
    /// Profiles whose direction is reversed.
    #[serde(default)]
    pub reversed: Vec<FeatureId>,
    #[serde(default)]
    pub ruled: bool,
    /// Guide curves crossing every section in order; the loft surface passes
    /// through each guide. Use two or more; `ruled` does not apply with guides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guides: Vec<Ref>,
    /// Join the last section back to the first, making a ring without end caps.
    /// Needs at least three sections and no guides.
    #[serde(default)]
    pub closed: bool,
    /// A curve crossing every section's plane in order; the sections blend
    /// along it. Not combined with guides, end conditions or `closed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centerline: Option<Ref>,
    /// How the loft leaves the first section.
    #[serde(default, skip_serializing_if = "LoftEnd::is_free")]
    pub start: LoftEnd,
    /// How the loft arrives at the last section.
    #[serde(default, skip_serializing_if = "LoftEnd::is_free")]
    pub end: LoftEnd,
    #[serde(default)]
    pub cut: bool,
    #[serde(default = "yes")]
    pub merge: bool,
}

/// A loft end condition. The surface leaves (or meets) the end section along
/// the given tangent; `tangent_length` scales it relative to the spacing of the
/// sections (1 is a natural length; larger values bulge further).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoftEnd {
    #[default]
    None,
    /// Perpendicular to the end section's plane.
    Normal {
        #[serde(default = "one")]
        #[schemars(extend("exclusiveMinimum" = 0))]
        tangent_length: f64,
    },
    /// Along a direction (edge, axis, plane or face normal).
    Direction {
        direction: Ref,
        #[serde(default)]
        flip: bool,
        #[serde(default = "one")]
        #[schemars(extend("exclusiveMinimum" = 0))]
        tangent_length: f64,
    },
}

impl LoftEnd {
    pub fn is_free(&self) -> bool {
        *self == LoftEnd::None
    }
}

fn one() -> f64 {
    1.0
}

/// A solid through a network of curves: closed sections in direction 1 and
/// curves crossing every section in direction 2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Boundary {
    /// Ordered closed sections. The first and last must be planar; they cap the solid.
    #[schemars(length(min = 2))]
    pub sections: Vec<ProfileSelection>,
    /// Curves that cross every section in order (sketch, 3D sketch, edge or curve paths).
    #[schemars(length(min = 2))]
    pub curves: Vec<Ref>,
    #[serde(default)]
    pub cut: bool,
    #[serde(default = "yes")]
    pub merge: bool,
}

/// Return an entire connected sketch path or a persistent entity reference.
pub(crate) fn path(state: &State, reference: &Ref) -> Result<Shape, String> {
    match reference {
        Ref::CurveSegment { curve, segment } => state
            .curves
            .get(curve)
            .and_then(|c| c.segments.get(segment))
            .cloned()
            .ok_or_else(|| {
                "the selected curve segment is missing or ambiguous; select it again".into()
            }),
        Ref::OrderedSketchContour(c) => ordered_contour(state, c, false).map(|(_, wire)| wire),
        Ref::SketchContour { sketch, entities } => {
            let (plane, chain) = sketch_contour(state, *sketch, entities)?;
            Shape::wire(
                &plane,
                &chain
                    .curves
                    .iter()
                    .map(crate::ops::to_geom)
                    .collect::<Vec<_>>(),
            )
            .map_err(|e| e.0)
        }
        Ref::Feature { id } => {
            if state.sketches3d.contains_key(id) {
                return spatial_path(state, *id, None);
            }
            if let Some(curve) = state.curves.get(id) {
                return Ok(curve.shape.clone());
            }
            let (plane, sketch) = state
                .sketches
                .get(id)
                .ok_or("the path feature is missing, failed or comes later in the feature tree")?;
            let contours = sketch.contours().map_err(|e| e.to_string())?;
            let mut chains = contours.loops.iter().chain(&contours.open);
            let chain = chains.next().ok_or("the sketch has no path")?;
            if chains.next().is_some() {
                return Err("select a sketch with one connected path".into());
            }
            Shape::wire(
                plane,
                &chain
                    .curves
                    .iter()
                    .map(crate::ops::to_geom)
                    .collect::<Vec<_>>(),
            )
            .map_err(|e| e.0)
        }
        Ref::SketchEntity { sketch, entity } => {
            if state.sketches3d.contains_key(sketch) {
                return spatial_path(state, *sketch, Some(*entity));
            }
            let (plane, sketch) = state
                .sketches
                .get(sketch)
                .ok_or("the path sketch is missing, failed or comes later in the feature tree")?;
            let curve = sketch.curve(*entity).map_err(|e| e.to_string())?;
            Shape::wire(plane, &[crate::ops::to_geom(&curve)]).map_err(|e| e.0)
        }
        _ => {
            let (kind, index) = state.index_of(reference)?;
            if kind != Kind::Edge {
                return Err("select an edge or curve path".into());
            }
            let shape = state
                .body
                .as_ref()
                .ok_or("there is no body")?
                .shape
                .sub_shape(kind, index)
                .map_err(|e| e.0)?;
            Shape::join_curves(&[shape]).map_err(|e| e.0)
        }
    }
}

fn spatial_curve(
    sketch: &of_sketch::spatial::Sketch3D,
    entity: of_sketch::Id,
) -> Result<of_geom::Curve3, String> {
    use of_sketch::spatial::Geometry3;
    match sketch.entity(entity)?.geometry {
        Geometry3::Line { a, b } => Ok(of_geom::Curve3::Line {
            a: sketch.point(a)?,
            b: sketch.point(b)?,
        }),
        Geometry3::Bezier { points } => Ok(of_geom::Curve3::Bezier {
            points: [
                sketch.point(points[0])?,
                sketch.point(points[1])?,
                sketch.point(points[2])?,
                sketch.point(points[3])?,
            ],
        }),
        Geometry3::Point { .. } => Err("select a spatial line or spline as a path".into()),
    }
}

fn spatial_path(
    state: &State,
    id: FeatureId,
    entity: Option<of_sketch::Id>,
) -> Result<Shape, String> {
    let (sketch, report) = &state.sketches3d[&id];
    if report.status == of_sketch::SolveStatus::Failed {
        return Err("the spatial path sketch has conflicting constraints".into());
    }
    let ids: Vec<_> = match entity {
        Some(id) => vec![id],
        None => sketch
            .entities
            .iter()
            .filter(|e| {
                !e.construction
                    && !matches!(e.geometry, of_sketch::spatial::Geometry3::Point { .. })
            })
            .map(|e| e.id)
            .collect(),
    };
    if ids.is_empty() {
        return Err("the spatial sketch has no path curves".into());
    }
    let curves = ids
        .iter()
        .map(|id| spatial_curve(sketch, *id))
        .collect::<Result<Vec<_>, _>>()?;
    let endpoints: Vec<_> = curves
        .iter()
        .map(|c| match c {
            of_geom::Curve3::Line { a, b } => (*a, *b),
            of_geom::Curve3::Bezier { points } => (points[0], points[3]),
        })
        .collect();
    let near = |a: DVec3, b: DVec3| (a - b).length() < 1e-6;
    let degree = |point| {
        endpoints
            .iter()
            .flat_map(|(a, b)| [a, b])
            .filter(|p| near(**p, point))
            .count()
    };
    if endpoints
        .iter()
        .flat_map(|(a, b)| [a, b])
        .any(|point| degree(*point) > 2)
    {
        return Err("the spatial path branches; select one connected unbranched chain".into());
    }
    let mut end = endpoints
        .iter()
        .flat_map(|(a, b)| [*a, *b])
        .find(|point| degree(*point) == 1)
        .unwrap_or(endpoints[0].0);
    let mut used = vec![false; curves.len()];
    let mut wires = Vec::new();
    while wires.len() < curves.len() {
        let index = endpoints.iter().enumerate().position(|(i,(a,b))| !used[i] && (near(*a,end) || near(*b,end)))
            .ok_or("the spatial sketch has disconnected paths; select a single entity or join connected curves")?;
        let reverse = !near(endpoints[index].0, end);
        let wire = Shape::wire3(&[curves[index].clone()]).map_err(|e| e.0)?;
        wires.push(if reverse { wire.reversed() } else { wire });
        end = if reverse {
            endpoints[index].0
        } else {
            endpoints[index].1
        };
        used[index] = true;
    }
    Shape::join_curves(&wires).map_err(|e| e.0)
}

/// A contour's identity is its full entity membership. If an edit splits, joins
/// or replaces the chain, require a new selection instead of following its index.
pub(crate) fn sketch_contour(
    state: &State,
    id: FeatureId,
    entities: &[of_sketch::Id],
) -> Result<(of_geom::Plane, of_sketch::Chain), String> {
    let (plane, sketch) = state
        .sketches
        .get(&id)
        .ok_or("the path sketch is missing or failed")?;
    let wanted: std::collections::BTreeSet<_> = entities.iter().copied().collect();
    if wanted.is_empty() || wanted.len() != entities.len() {
        return Err("a contour reference needs distinct sketch entity IDs".into());
    }
    let contours = sketch.contours().map_err(|e| e.to_string())?;
    let chain = contours
        .loops
        .into_iter()
        .chain(contours.open)
        .find(|c| {
            c.entities
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                == wanted
        })
        .ok_or("the selected contour changed or no longer exists; select the path again")?;
    Ok((*plane, chain))
}

/// Resolve and orient a complete contour without making its array position an identity.
pub(crate) fn ordered_contour(
    state: &State,
    selection: &ContourSelection,
    require_closed: bool,
) -> Result<(Plane, Shape), String> {
    let (plane, mut chain) = sketch_contour(state, selection.sketch, &selection.entities)?;
    let (_, sketch) = &state.sketches[&selection.sketch];
    let contours = sketch.contours().map_err(|e| e.to_string())?;
    let closed = contours.loops.iter().any(|c| c.entities == chain.entities);
    if require_closed && !closed {
        return Err("select a complete closed profile contour".into());
    }
    if selection.reverse {
        chain.entities.reverse();
    }
    if let Some(start) = selection.start {
        let position = chain
            .entities
            .iter()
            .position(|id| *id == start)
            .ok_or("the start/seam entity is not in the selected contour")?;
        if !closed && position != 0 {
            return Err(
                "an open path must start at an endpoint; reverse its direction to change ends"
                    .into(),
            );
        }
        chain.entities.rotate_left(position);
    }
    let curves = chain
        .entities
        .iter()
        .map(|id| sketch.curve(*id).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut oriented = Vec::new();
    let mut end: Option<of_geom::DVec2> = None;
    for (i, curve) in curves.iter().enumerate() {
        let reverse = if let Some(end) = end {
            (curve.start() - end).length() > (curve.end() - end).length()
        } else if curves.len() > 1 {
            let next = &curves[1];
            let distance =
                |p: of_geom::DVec2| (next.start() - p).length().min((next.end() - p).length());
            distance(curve.start()) < distance(curve.end())
        } else {
            selection.reverse
        };
        let (start, finish) = if reverse {
            (curve.end(), curve.start())
        } else {
            (curve.start(), curve.end())
        };
        if i > 0 && (start - end.unwrap()).length() > 1e-6 {
            return Err("selected contour is not connected in traversal order".into());
        }
        end = Some(finish);
        oriented.push((crate::ops::to_geom(curve), reverse));
    }
    if let Some(seam) = selection.seam {
        let f = seam.0;
        if !closed {
            return Err("an open path has no seam to move".into());
        }
        if !(f.is_finite() && f > 0.0 && f < 1.0) {
            return Err("the seam position must be between 0 and 1".into());
        }
        // Split the first entity at the seam: the contour starts after the seam
        // and comes back to it at the end.
        let (first, reverse) = oriented.remove(0);
        let t = if reverse { 1.0 - f } else { f };
        let (lo, hi) = first
            .split(t)
            .ok_or("a seam can be placed inside lines, arcs, circles and Bezier curves")?;
        let (after, before) = if reverse { (lo, hi) } else { (hi, lo) };
        if matches!(first, of_geom::Curve2::Circle { .. }) {
            oriented.insert(0, (after, reverse));
        } else {
            oriented.insert(0, (after, reverse));
            oriented.push((before, reverse));
        }
    }
    let wires = oriented
        .iter()
        .map(|(curve, reverse)| {
            let wire = Shape::wire(&plane, std::slice::from_ref(curve)).map_err(|e| e.0)?;
            Ok(if *reverse { wire.reversed() } else { wire })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((plane, Shape::join_curves(&wires).map_err(|e| e.0)?))
}

pub(crate) fn section(
    state: &State,
    selection: &ProfileSelection,
) -> Result<(Plane, Shape), String> {
    if let ProfileSelection::Contour(contour) = selection {
        return ordered_contour(state, contour, true);
    }
    let id = selection.sketch();
    let (plane, sketch) = state
        .sketches
        .get(&id)
        .ok_or("the profile sketch is missing, failed or comes later in the feature tree")?;
    let contours = sketch.contours().map_err(|e| e.to_string())?;
    if contours.loops.len() != 1 || !contours.open.is_empty() {
        return Err(
            "this operation requires one closed profile without holes; select a contour explicitly"
                .into(),
        );
    }
    Ok((*plane, path(state, &Ref::Feature { id })?))
}

/// Validate hole ownership and containment before invoking the solid kernel.
pub(crate) fn validate_inner_profiles(state: &State, sweep: &Sweep) -> Result<(), String> {
    if sweep.inner_profiles.is_empty() {
        return Ok(());
    }
    let Some(ProfileSelection::Contour(outer)) = &sweep.profile else {
        return Err("select an explicit outer contour before selecting profile holes".into());
    };
    let (_, outer_chain) = sketch_contour(state, outer.sketch, &outer.entities)?;
    let polygon = |chain: &of_sketch::Chain| {
        let mut points: Vec<of_geom::DVec2> = Vec::new();
        for curve in &chain.curves {
            let mut sampled = curve.sample(0.001);
            if let Some(end) = points.last()
                && sampled[0].distance(*end) > sampled[sampled.len() - 1].distance(*end)
            {
                sampled.reverse();
            }
            points.extend(sampled);
        }
        points
    };
    let outer_polygon = polygon(&outer_chain);
    let mut holes: Vec<(
        std::collections::BTreeSet<of_sketch::Id>,
        Vec<of_geom::DVec2>,
    )> = Vec::new();
    for inner in &sweep.inner_profiles {
        if inner.sketch != outer.sketch {
            return Err("profile holes must belong to the same sketch as the outer contour".into());
        }
        ordered_contour(state, inner, true)?;
        let (_, chain) = sketch_contour(state, inner.sketch, &inner.entities)?;
        let key: std::collections::BTreeSet<_> = inner.entities.iter().copied().collect();
        if outer
            .entities
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            == key
        {
            return Err("the outer contour cannot also be a profile hole".into());
        }
        let points = polygon(&chain);
        if !of_sketch::geom2::point_in_polygon(points[points.len() / 2], &outer_polygon) {
            return Err("every profile hole must lie strictly inside the outer contour".into());
        }
        for (other_key, other) in &holes {
            if key == *other_key {
                return Err("select each profile hole only once".into());
            }
            if of_sketch::geom2::point_in_polygon(points[points.len() / 2], other)
                || of_sketch::geom2::point_in_polygon(other[other.len() / 2], &points)
            {
                return Err("profile holes must be disjoint and cannot be nested".into());
            }
        }
        holes.push((key, points));
    }
    Ok(())
}

/// Persistent source identities, independent of generated kernel edge numbering.
fn source_segments(state: &State, reference: &Ref) -> Result<BTreeMap<String, Shape>, String> {
    match reference {
        Ref::Feature { id } if state.sketches3d.contains_key(id) => {
            spatial_path(state, *id, None)?;
            state.sketches3d[id]
                .0
                .entities
                .iter()
                .filter(|e| {
                    !e.construction
                        && !matches!(e.geometry, of_sketch::spatial::Geometry3::Point { .. })
                })
                .map(|entity| {
                    Ok((
                        format!("sketch3d:{id}/entity:{}", entity.id),
                        spatial_path(state, *id, Some(entity.id))?,
                    ))
                })
                .collect()
        }
        Ref::Feature { id } if state.curves.contains_key(id) => Ok(state.curves[id]
            .segments
            .iter()
            .map(|(name, shape)| (format!("curve:{id}/{name}"), shape.clone()))
            .collect()),
        Ref::Feature { id }
        | Ref::SketchContour { sketch: id, .. }
        | Ref::OrderedSketchContour(ContourSelection { sketch: id, .. })
            if state.sketches.contains_key(id) =>
        {
            // Validate complete membership before resolving any entities.
            path(state, reference)?;
            let (plane, sketch) = &state.sketches[id];
            let ids = match reference {
                Ref::SketchContour { entities, .. } => entities.clone(),
                Ref::OrderedSketchContour(c) => c.entities.clone(),
                _ => sketch
                    .curve_ids()
                    .filter(|id| !sketch.entities[id].construction)
                    .collect(),
            };
            ids.into_iter()
                .map(|entity| {
                    let curve = sketch.curve(entity).map_err(|e| e.to_string())?;
                    Ok((
                        format!("sketch:{id}/entity:{entity}"),
                        Shape::wire(plane, &[crate::ops::to_geom(&curve)]).map_err(|e| e.0)?,
                    ))
                })
                .collect()
        }
        _ => Ok(BTreeMap::from([(
            serde_json::to_string(reference).expect("reference serializes"),
            path(state, reference)?,
        )])),
    }
}

pub(crate) fn generated(
    kind: &crate::FeatureKind,
    state: &State,
) -> Result<Option<CurveBody>, String> {
    let (shape, candidates) = match kind {
        crate::FeatureKind::Helix(p) => {
            let shape = helix(p, state)?;
            (shape.clone(), vec![("helix".into(), shape)])
        }
        crate::FeatureKind::CompositeCurve(p) => {
            let shape = composite(p, state)?;
            let mut segments = Vec::new();
            for selected in &p.segments {
                segments.extend(
                    source_segments(state, &selected.reference)?
                        .into_iter()
                        .map(|(name, shape)| {
                            (
                                name,
                                if selected.reverse {
                                    shape.reversed()
                                } else {
                                    shape
                                },
                            )
                        }),
                );
            }
            (shape, segments)
        }
        crate::FeatureKind::ProjectedCurve(p) if p.mode == ProjectionMode::SketchOnSketch => {
            // One curve from two sketches; its edges have no single source segment.
            let shape = projected(p, state)?;
            let segments = (0..shape.count(Kind::Edge))
                .map(|i| {
                    shape
                        .sub_shape(Kind::Edge, i)
                        .map(|edge| (format!("intersect:{i}"), edge))
                        .map_err(|e| e.0)
                })
                .collect::<Result<Vec<_>, String>>()?;
            (shape, segments)
        }
        crate::FeatureKind::ProjectedCurve(p) => {
            let shape = projected(p, state)?;
            let target = projection_target(state, &p.target)?;
            let target_name = serde_json::to_string(&p.target).expect("reference serializes");
            let mut segments = Vec::new();
            for (source, segment) in source_segments(state, &p.source)? {
                // Partial projections can legitimately omit a source segment. Naming
                // must not make an otherwise valid whole-curve projection fail.
                let Ok(projected) = segment.project_onto(&target, p.direction) else {
                    continue;
                };
                let name = format!("project:{source}@{target_name}");
                match projected.count(Kind::Edge) {
                    0 => {}
                    1 => segments.push((name, projected)),
                    // A source split into several pieces (by holes or face edges)
                    // names them in order along the source: #1 is nearest its start.
                    _ => {
                        for (k, piece) in along_source(&segment, &projected, p.direction)?
                            .into_iter()
                            .enumerate()
                        {
                            segments.push((format!("{name}#{}", k + 1), piece));
                        }
                    }
                }
            }
            (shape, segments)
        }
        _ => return Ok(None),
    };
    let mut segments = BTreeMap::new();
    let mut ambiguous = std::collections::BTreeSet::new();
    for (name, shape) in candidates {
        if segments.insert(name.clone(), shape).is_some() {
            ambiguous.insert(name);
        }
    }
    for name in ambiguous {
        segments.remove(&name);
    }
    Ok(Some(CurveBody { shape, segments }))
}

/// The edges of `projected` ordered by where they come from along `source`,
/// matching each edge's midpoint to the source across the projection direction.
fn along_source(
    source: &Shape,
    projected: &Shape,
    direction: of_geom::DVec3,
) -> Result<Vec<Shape>, String> {
    let d = direction.normalize_or_zero();
    let mut samples: Vec<(f64, of_geom::DVec3)> = Vec::new();
    let mut length = 0.0;
    for e in 0..source.count(Kind::Edge) {
        for q in source.edge_samples(e, 1e-3, 0.05).map_err(|e| e.0)? {
            if let Some((_, last)) = samples.last() {
                length += (q - *last).length();
            }
            samples.push((length, q));
        }
    }
    let mut pieces = (0..projected.count(Kind::Edge))
        .map(|i| {
            let mid = projected.edge_info(i).map_err(|e| e.0)?.mid;
            let across = |q: of_geom::DVec3| {
                let v = mid - q;
                (v - d * v.dot(d)).length()
            };
            let key = samples
                .iter()
                .min_by(|a, b| across(a.1).total_cmp(&across(b.1)))
                .map_or(0.0, |s| s.0);
            Ok((key, projected.sub_shape(Kind::Edge, i).map_err(|e| e.0)?))
        })
        .collect::<Result<Vec<_>, String>>()?;
    pieces.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(pieces.into_iter().map(|(_, piece)| piece).collect())
}

pub(crate) fn helix(p: &Helix, prev: &State) -> Result<Shape, String> {
    let plane = resolve(prev, &p.plane)?.plane()?;
    if !p.variable_pitch.is_empty() {
        let rows: Vec<(f64, f64)> = p.variable_pitch.iter().map(|r| (r.turn, r.pitch)).collect();
        return Shape::helix_variable(&plane, p.radius, &rows, p.start_angle, p.left_handed)
            .map_err(|e| e.0);
    }
    let height = || {
        if p.height.is_finite() && p.height > 0.0 {
            Ok(p.height)
        } else {
            Err("the helix height must be more than zero".to_string())
        }
    };
    let (pitch, turns) = match p.defined_by {
        HelixInput::PitchAndTurns => (p.pitch, p.turns),
        HelixInput::HeightAndTurns => (height()? / p.turns, p.turns),
        HelixInput::HeightAndPitch => (p.pitch, height()? / p.pitch),
    };
    Shape::helix(&plane, p.radius, pitch, turns, p.start_angle, p.left_handed).map_err(|e| e.0)
}

pub(crate) fn composite(p: &CompositeCurve, prev: &State) -> Result<Shape, String> {
    if p.segments.is_empty() {
        return Err("select curves in path order".into());
    }
    let shapes = p
        .segments
        .iter()
        .map(|s| {
            let shape = path(prev, &s.reference)?;
            Ok(if s.reverse { shape.reversed() } else { shape })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Shape::join_curves(&shapes).map_err(|e| e.0)
}

pub(crate) fn projected(p: &ProjectedCurve, prev: &State) -> Result<Shape, String> {
    match p.mode {
        ProjectionMode::OntoFace => {}
        ProjectionMode::SketchOnSketch => return sketch_on_sketch(p, prev),
        ProjectionMode::Intersection | ProjectionMode::Silhouette => {
            return Err("intersection and silhouette modes split faces; use a split line".into());
        }
    }
    let source = path(prev, &p.source)?;
    let target = projection_target(prev, &p.target)?;
    let projected = source.project_onto(&target, p.direction).map_err(|e| e.0)?;
    // A face can interrupt a curve into pieces; a whole body is usually hit on
    // both sides, which is rarely what was meant.
    if matches!(resolve(prev, &p.target)?, Resolved::Body { .. })
        && Shape::join_curves(std::slice::from_ref(&projected)).is_err()
    {
        return Err("projection produces multiple paths; select a single target face".into());
    }
    Ok(projected)
}

/// Where two sketches meet when each is extruded both ways along its plane normal.
fn sketch_on_sketch(p: &ProjectedCurve, prev: &State) -> Result<Shape, String> {
    let sketch = |r: &Ref| -> Result<(Shape, DVec3), String> {
        let Ref::Feature { id } = r else {
            return Err("sketch on sketch needs two sketches".into());
        };
        let (plane, _) = prev
            .sketches
            .get(id)
            .ok_or("sketch on sketch needs two sketches")?;
        Ok((path(prev, r)?, plane.normal))
    };
    let (a, na) = sketch(&p.target)?;
    let (b, nb) = sketch(&p.source)?;
    if na.cross(nb).length() < 1e-9 {
        return Err("the two sketches are on parallel planes".into());
    }
    let reach = Shape::compound([&a, &b])
        .bbox()
        .map_or(1.0, |bb| bb.diagonal())
        * 10.0
        + 10.0;
    let sheet = |wire: &Shape, normal: DVec3| -> Result<Shape, String> {
        let start = wire
            .transform(&of_geom::DAffine3::from_translation(-normal * reach))
            .map_err(|e| e.0)?
            .shape;
        Ok(start.prism(normal * (2.0 * reach)).map_err(|e| e.0)?.shape)
    };
    let edges = sheet(&a, na)?.section(&sheet(&b, nb)?).map_err(|e| e.0)?;
    Shape::join_curves(&[edges])
        .map_err(|_| "the sketches' extrusions meet in more than one curve".to_string())
}

fn projection_target(prev: &State, reference: &Ref) -> Result<Shape, String> {
    match resolve(prev, reference)? {
        Resolved::Body { id, .. } => Ok(prev.solid(&id)?.geometry.shape.clone()),
        Resolved::Face { index, .. } => prev
            .body
            .as_ref()
            .ok_or("there is no body")?
            .shape
            .sub_shape(Kind::Face, index)
            .map_err(|e| e.0),
        _ => Err("select a face or body to project onto".into()),
    }
}
