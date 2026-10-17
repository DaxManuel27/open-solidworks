//! The part document: feature tree, rebuild, rollback and suppression,
//! stable face/edge IDs (topological naming), and the `.ofp` file format.
//!
//! A [`Document`] is plain data: an ordered list of [`Feature`]s. A
//! [`Model`] owns a document and rebuilds it into geometry, keeping the state
//! after every feature so edits rebuild only from the changed feature on, and
//! so the model can be shown rolled back to any point.
//!
//! Features never refer to faces or edges by kernel index. They hold
//! [`Ref`]s with stable names (see [`naming`]) that are resolved against the
//! model as it is at that feature during every rebuild.

pub mod bodies;
mod build;
pub mod configurations;
pub mod cosmetic_thread;
pub use cosmetic_thread::{CosmeticThread, Thread};
pub mod curves;
pub mod direct;
pub use curves::{
    Boundary, CompositeCurve, ContourSelection, CurveSelection, Helix, HelixInput, HelixPitch,
    Loft, LoftEnd, OrderedFloat, ProfileSelection, ProjectedCurve, ProjectionMode, Sweep,
    SweepOrientation,
};
pub mod equations;
mod features;
pub mod file;
pub mod flat;
pub mod materials;
pub mod measure;
pub mod meshes;
pub mod naming;
mod ops;
pub mod properties;
mod resolve;
pub mod sheet_metal;
pub mod sheet_tables;
pub mod surfaces;
pub mod units;
pub mod weldment;

pub use bodies::{BodyDisplay, BodyId, Combine, KeepSide, MoveBody, SolidBody, SplitBody};
pub use build::{Body, Datum, FeatureStatus, Model, State};
pub use features::*;
pub use naming::Naming;
pub use ops::{CUT_REMOVES_NOTHING, HoleDims, convert_edges, hole_dims};
pub use resolve::{Resolved, resolve};
pub use units::Units;

pub use of_geom as geom;
pub use of_sketch as sketch;

use of_geom::DVec3;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub type FeatureId = u32;

/// A reference to geometry, by stable name rather than kernel index.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Ref {
    /// A whole solid body, by its stable ID from `doc.info` `bodies`.
    Body { id: BodyId },
    /// A face, edge or vertex of one body, as returned by selection.
    BodyTopology {
        body: BodyId,
        kind: of_geom::Kind,
        /// Stable topology name, such as `2.end` or `E(2.end|2.s5)`.
        name: String,
    },
    /// The model origin point.
    Origin,
    /// The XY plane.
    FrontPlane,
    /// The XZ plane.
    TopPlane,
    /// The YZ plane.
    RightPlane,
    /// A face by stable name, such as `2.end`, from `doc.topology`.
    Face { name: String },
    /// An edge by stable name, such as `E(2.end|2.s5)`, from `doc.topology`.
    Edge { name: String },
    /// A vertex by stable name, from `doc.topology`.
    Vertex { name: String },
    /// A reference plane, axis, point, coordinate system or sketch.
    Feature { id: FeatureId },
    /// A point or line in a sketch.
    SketchEntity {
        sketch: FeatureId,
        entity: of_sketch::Id,
    },
    /// A generated segment with an unambiguous source-derived identity from the scene.
    CurveSegment { curve: FeatureId, segment: String },
    /// A complete contour with explicit direction and start/seam entity.
    OrderedSketchContour(ContourSelection),
    /// One complete sketch loop or open chain, identified by its member entity
    /// IDs rather than a contour array index. Order does not affect identity.
    SketchContour {
        sketch: FeatureId,
        #[schemars(length(min = 1))]
        entities: Vec<of_sketch::Id>,
    },
}

/// A feature in the tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: FeatureId,
    pub name: String,
    #[serde(default)]
    pub suppressed: bool,
    /// None preserves legacy whole-part behavior; Some selects explicit bodies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<BodyId>>,
    #[serde(flatten)]
    pub kind: FeatureKind,
}

impl Feature {
    pub fn depends_on(&self) -> Vec<FeatureId> {
        let mut ids = self.kind.depends_on();
        if let Some(scope) = &self.scope {
            ids.extend(scope.iter().filter_map(BodyId::creator));
        }
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeatureKind {
    /// Replace a planar circular disk face with an exact spherical cap.
    Dome(Dome),
    /// A thin web from an open sketch, extended until it meets material.
    Rib(Rib),
    /// Wrap closed sketch regions onto a cylindrical face: emboss, deboss or scribe.
    Wrap(Wrap),
    /// Persistent thread annotation on a cylindrical face; does not change geometry.
    CosmeticThread(CosmeticThread),
    /// A modeled ISO-profile thread cut into a cylindrical face.
    Thread(Thread),
    /// A constrained spatial sketch, with independent XYZ points and curves.
    Sketch3d(of_sketch::spatial::Sketch3D),
    /// Split a face along a curve projected onto it.
    SplitLine(ProjectedCurve),
    /// Taper faces by `angle` about a neutral plane.
    Draft(Draft),
    /// Fillet one edge with radii varying between stations.
    VariableFillet(VariableFillet),
    /// Blend two sets of planar faces with a rolling-ball radius.
    FaceFillet(FaceFillet),
    /// A helical construction curve, usable as a sweep path.
    Helix(Helix),
    /// Join connected curves into one path.
    CompositeCurve(CompositeCurve),
    /// Project a sketch or curve onto a face along `direction`.
    ProjectedCurve(ProjectedCurve),
    /// Sweep a closed profile along a path; `cut` removes material.
    Sweep(Sweep),
    /// Loft through two or more closed profiles in order.
    Loft(Loft),
    /// A solid bounded by sections in one direction and crossing curves in the other.
    Boundary(Boundary),
    /// Boolean of bodies: fuse, cut (from the first) or common.
    Combine(Combine),
    /// Move, rotate or copy bodies.
    MoveBody(MoveBody),
    /// Split bodies with a plane.
    SplitBody(SplitBody),
    /// Sketches are created with `sketch.new` and edited with `sketch.edit`.
    #[schemars(skip)]
    Sketch(SketchFeature),
    /// A reference plane, defined by `mode`.
    Plane(PlaneDef),
    /// A reference axis, defined by `mode`.
    Axis(AxisDef),
    /// A reference point, defined by `mode`.
    Point(PointDef),
    /// A coordinate system at a point.
    CoordSys(CoordSysDef),
    /// Extrude a sketch's closed profiles: a boss, or a cut with `cut: true`.
    Extrude(Extrude),
    /// Revolve a sketch's closed profiles about an axis.
    Revolve(Revolve),
    /// Round edges, or every edge of the given faces, with a constant radius.
    Fillet(Fillet),
    /// Bevel edges, or every edge of the given faces.
    Chamfer(Chamfer),
    /// Hollow the part to `thickness`, removing the given faces.
    Shell(ShellDef),
    /// Standard holes at each standalone point of a sketch.
    Hole(Hole),
    /// Repeat features along one or two directions.
    LinearPattern(LinearPattern),
    /// Repeat features around an axis.
    CircularPattern(CircularPattern),
    /// Mirror features, or the whole body, about a plane.
    Mirror(MirrorDef),
    /// Imported bodies come from the `import` method.
    #[schemars(skip)]
    Import(Import),
    /// The first sheet metal feature, or a tab on a sheet metal face.
    BaseFlange(sheet_metal::BaseFlange),
    /// Walls bent up from edges of a sheet metal body.
    EdgeFlange(sheet_metal::EdgeFlange),
    /// Every sheet metal body flattened; suppress it to show the part formed.
    FlatPattern(sheet_metal::FlatPattern),
    /// Fold sheet edges back on themselves.
    Hem(sheet_metal::Hem),
    /// Flanges from one profile along a chain of edges, mitered at corners.
    MiterFlange(sheet_metal::MiterFlange),
    /// Bend the sheet along a sketched line.
    SketchedBend(sheet_metal::SketchedBend),
    /// A Z-shaped offset of the sheet along a sketched line.
    Jog(sheet_metal::Jog),
    /// Cosmetic stiffening crosses on a flat sheet face.
    CrossBreak(sheet_metal::CrossBreak),
    /// An open sheet metal profile swept along a path.
    SweptFlange(sheet_metal::SweptFlange),
    /// A faceted sheet metal transition between two open profiles.
    LoftedBend(sheet_metal::LoftedBend),
    /// Cut gaps along corner edges so walls can unfold.
    Rip(sheet_metal::Rip),
    /// Turn a thin-walled solid's sharp corners into bends.
    InsertBends(sheet_metal::InsertBends),
    /// Make sheet metal walls from a solid's faces.
    ConvertToSheetMetal(sheet_metal::ConvertToSheetMetal),
    /// Flatten a sheet metal body until the next Fold.
    Unfold(sheet_metal::Unfold),
    /// Form an unfolded body again, keeping cuts made while flat.
    Fold(sheet_metal::Fold),
    /// A cut through sheet metal square to the sheet.
    NormalCut(sheet_metal::NormalCut),
    /// Relief cuts where two bends meet at a corner.
    CornerRelief(sheet_metal::CornerRelief),
    /// Extend flanges so they close a corner.
    ClosedCorner(sheet_metal::ClosedCorner),
    /// A weld bead along a closed corner's seam.
    WeldedCorner(sheet_metal::WeldedCorner),
    /// Chamfer or round corners of the sheet's outline.
    BreakCorner(sheet_metal::BreakCorner),
    /// Notches at the ends of bend lines.
    BendNotch(sheet_metal::BendNotch),
    /// Tabs on one sheet metal body through slots in another.
    TabAndSlot(sheet_metal::TabAndSlot),
    /// A stiffening rib pressed across a bend.
    SheetMetalGusset(sheet_metal::SheetMetalGusset),
    /// Forming tools pressed into a sheet metal face.
    FormTool(sheet_metal::FormTool),
    /// Marks this part as a forming tool for other parts to use.
    FormingTool(sheet_metal::FormingToolDef),
    /// Marks the part as a weldment (one per part, before its members).
    Weldment(weldment::WeldmentDef),
    /// A profile swept along sketch segments, one body per segment.
    StructuralMember(weldment::StructuralMember),
    /// Members cut back or lengthened to faces, planes or other bodies.
    TrimExtend(weldment::TrimExtend),
    /// Plates closing members' ends.
    EndCap(weldment::EndCap),
    /// A plate in the corner between two members.
    WeldGusset(weldment::WeldGusset),
    /// A cosmetic or modeled fillet weld bead.
    WeldBead(weldment::WeldBead),
    /// A surface extruded from a sketch's open or closed contours.
    ExtrudedSurface(surfaces::ExtrudedSurface),
    /// A surface revolved from a sketch's contours about an axis.
    RevolvedSurface(surfaces::RevolvedSurface),
    /// A surface swept from an open or closed profile along a path.
    SweptSurface(surfaces::SweptSurface),
    /// A surface lofted through profiles in order.
    LoftedSurface(surfaces::LoftedSurface),
    /// A surface bounded by curves in one or two directions.
    BoundarySurface(surfaces::BoundarySurface),
    /// A flat surface inside a closed sketch or loop of edges.
    PlanarSurface(surfaces::PlanarSurface),
    /// Copies of faces moved along their normals, as a surface.
    OffsetSurface(surfaces::OffsetSurface),
    /// Join surface bodies along shared edges, optionally into a solid.
    Knit(surfaces::Knit),
    /// Give a surface body thickness, making a solid (or a cut).
    Thicken(surfaces::Thicken),
    /// Cut away the solids on one side of a surface.
    CutWithSurface(surfaces::CutWithSurface),
    /// Remove faces, leaving a gap, patching it or filling it.
    DeleteFace(surfaces::DeleteFace),
    /// Offset, translate or rotate faces of a solid.
    MoveFace(direct::MoveFace),
    /// Replace faces of a solid with a surface.
    ReplaceFace(direct::ReplaceFace),
    /// Remove the holes of surface faces.
    DeleteHole(direct::DeleteHole),
    /// A patch filling a loop of edges and curves.
    FilledSurface(surfaces::FilledSurface),
    /// Strips of straight lines from edges.
    RuledSurface(surfaces::RuledSurface),
    /// Strips from edges parallel to a plane.
    RadiateSurface(surfaces::RadiateSurface),
    /// Cut surfaces with a tool or each other, keeping chosen pieces.
    TrimSurface(surfaces::TrimSurface),
    /// Restore surface faces to their natural boundary, or fill their holes.
    UntrimSurface(surfaces::UntrimSurface),
    /// Extend surfaces past their open edges.
    ExtendSurface(surfaces::ExtendSurface),
    /// A surface midway between pairs of faces.
    MidSurface(surfaces::MidSurface),
    /// Reshape a face by moving its control points.
    Freeform(surfaces::Freeform),
    /// Lay doubly curved faces out flat.
    SurfaceFlatten(surfaces::SurfaceFlatten),
}

impl FeatureKind {
    /// Base for default feature names ("Boss-Extrude1", "Fillet2", ...).
    pub fn base_name(&self) -> &'static str {
        match self {
            FeatureKind::Dome(_) => "Dome",
            FeatureKind::Rib(_) => "Rib",
            FeatureKind::Wrap(_) => "Wrap",
            FeatureKind::CosmeticThread(_) => "Cosmetic Thread",
            FeatureKind::Thread(_) => "Thread",
            FeatureKind::Sketch3d(_) => "3D Sketch",
            FeatureKind::SplitLine(_) => "Split Line",
            FeatureKind::Draft(_) => "Draft",
            FeatureKind::VariableFillet(_) => "Variable Fillet",
            FeatureKind::FaceFillet(_) => "Face Fillet",
            FeatureKind::Helix(_) => "Helix",
            FeatureKind::CompositeCurve(_) => "Composite Curve",
            FeatureKind::ProjectedCurve(_) => "Projected Curve",
            FeatureKind::Sweep(p) if p.cut => "Swept Cut",
            FeatureKind::Sweep(_) => "Sweep",
            FeatureKind::Loft(p) if p.cut => "Lofted Cut",
            FeatureKind::Loft(_) => "Loft",
            FeatureKind::Boundary(p) if p.cut => "Boundary-Cut",
            FeatureKind::Boundary(_) => "Boundary",
            FeatureKind::Combine(_) => "Combine",
            FeatureKind::MoveBody(p) if p.copy => "Copy Bodies",
            FeatureKind::MoveBody(_) => "Move Bodies",
            FeatureKind::SplitBody(_) => "Split Bodies",
            FeatureKind::Sketch(_) => "Sketch",
            FeatureKind::Plane(_) => "Plane",
            FeatureKind::Axis(_) => "Axis",
            FeatureKind::Point(_) => "Point",
            FeatureKind::CoordSys(_) => "Coordinate System",
            FeatureKind::Extrude(e) if e.cut => "Cut-Extrude",
            FeatureKind::Extrude(_) => "Boss-Extrude",
            FeatureKind::Revolve(r) if r.cut => "Cut-Revolve",
            FeatureKind::Revolve(_) => "Revolve",
            FeatureKind::Fillet(_) => "Fillet",
            FeatureKind::Chamfer(_) => "Chamfer",
            FeatureKind::Shell(_) => "Shell",
            FeatureKind::Hole(_) => "Hole",
            FeatureKind::LinearPattern(_) => "LPattern",
            FeatureKind::CircularPattern(_) => "CirPattern",
            FeatureKind::Mirror(_) => "Mirror",
            FeatureKind::Import(_) => "Imported",
            FeatureKind::BaseFlange(_) => "Base-Flange",
            FeatureKind::EdgeFlange(_) => "Edge-Flange",
            FeatureKind::FlatPattern(_) => "Flat-Pattern",
            FeatureKind::Hem(_) => "Hem",
            FeatureKind::MiterFlange(_) => "Miter Flange",
            FeatureKind::SketchedBend(_) => "Sketched Bend",
            FeatureKind::Jog(_) => "Jog",
            FeatureKind::CrossBreak(_) => "Cross-Break",
            FeatureKind::SweptFlange(_) => "Swept Flange",
            FeatureKind::LoftedBend(_) => "Lofted-Bend",
            FeatureKind::Rip(_) => "Rip",
            FeatureKind::InsertBends(_) => "Insert Bends",
            FeatureKind::ConvertToSheetMetal(_) => "Convert-Solid",
            FeatureKind::Unfold(_) => "Unfold",
            FeatureKind::Fold(_) => "Fold",
            FeatureKind::NormalCut(_) => "Cut-Normal",
            FeatureKind::CornerRelief(_) => "Corner Relief",
            FeatureKind::ClosedCorner(_) => "Closed Corner",
            FeatureKind::WeldedCorner(_) => "Welded Corner",
            FeatureKind::BreakCorner(_) => "Break-Corner",
            FeatureKind::BendNotch(_) => "Bend Notch",
            FeatureKind::TabAndSlot(_) => "Tab and Slot",
            FeatureKind::SheetMetalGusset(_) => "Sheet Metal Gusset",
            FeatureKind::FormTool(_) => "Form Tool",
            FeatureKind::FormingTool(_) => "Forming Tool",
            FeatureKind::Weldment(_) => "Weldment",
            FeatureKind::StructuralMember(_) => "Structural Member",
            FeatureKind::TrimExtend(_) => "Trim/Extend",
            FeatureKind::EndCap(_) => "End Cap",
            FeatureKind::WeldGusset(_) => "Gusset",
            FeatureKind::WeldBead(_) => "Weld Bead",
            FeatureKind::ExtrudedSurface(_) => "Surface-Extrude",
            FeatureKind::RevolvedSurface(_) => "Surface-Revolve",
            FeatureKind::SweptSurface(_) => "Surface-Sweep",
            FeatureKind::LoftedSurface(_) => "Surface-Loft",
            FeatureKind::BoundarySurface(_) => "Boundary-Surface",
            FeatureKind::PlanarSurface(_) => "Surface-Plane",
            FeatureKind::OffsetSurface(_) => "Surface-Offset",
            FeatureKind::Knit(_) => "Surface-Knit",
            FeatureKind::Thicken(t) if t.cut => "Thicken-Cut",
            FeatureKind::Thicken(_) => "Thicken",
            FeatureKind::CutWithSurface(_) => "SurfaceCut",
            FeatureKind::DeleteFace(_) => "Delete Face",
            FeatureKind::MoveFace(_) => "Move Face",
            FeatureKind::ReplaceFace(_) => "Replace Face",
            FeatureKind::DeleteHole(_) => "Delete Hole",
            FeatureKind::FilledSurface(_) => "Surface-Fill",
            FeatureKind::RuledSurface(_) => "Ruled Surface",
            FeatureKind::RadiateSurface(_) => "Surface-Radiate",
            FeatureKind::TrimSurface(_) => "Surface-Trim",
            FeatureKind::UntrimSurface(_) => "Surface-Untrim",
            FeatureKind::ExtendSurface(_) => "Surface-Extend",
            FeatureKind::MidSurface(_) => "Surface-MidSurface",
            FeatureKind::Freeform(_) => "Freeform",
            FeatureKind::SurfaceFlatten(_) => "Surface Flatten",
        }
    }

    /// Features this one depends on directly (not counting geometry refs).
    pub fn depends_on(&self) -> Vec<FeatureId> {
        let mut out = Vec::new();
        let mut refs: Vec<&Ref> = Vec::new();
        match self {
            FeatureKind::Dome(p) => refs.push(&p.face),
            FeatureKind::Thread(p) => refs.push(&p.face),
            FeatureKind::CosmeticThread(p) => {
                refs.push(&p.face);
                // Legacy single-body cylinder faces carry the creating feature
                // in their persistent topology name, such as "2.s1".
                if let Ref::Face { name } = &p.face {
                    out.extend(
                        name.split('.')
                            .next()
                            .and_then(|prefix| prefix.parse::<FeatureId>().ok()),
                    );
                }
            }
            FeatureKind::Draft(p) => {
                refs.extend(&p.faces);
                refs.extend(&p.parting_line);
                refs.push(&p.neutral_plane);
                refs.extend(&p.direction);
            }
            FeatureKind::VariableFillet(p) => refs.push(&p.edge),
            FeatureKind::FaceFillet(p) => refs.extend(p.faces1.iter().chain(&p.faces2)),
            FeatureKind::Helix(p) => refs.push(&p.plane),
            FeatureKind::CompositeCurve(p) => refs.extend(p.segments.iter().map(|s| &s.reference)),
            FeatureKind::ProjectedCurve(p) | FeatureKind::SplitLine(p) => {
                refs.extend([&p.source, &p.target])
            }
            FeatureKind::Sweep(p) => {
                out.extend(p.profile.as_ref().map(ProfileSelection::sketch));
                refs.extend(&p.guide);
                refs.extend(&p.guide2);
                out.extend(p.inner_profiles.iter().map(|profile| profile.sketch));
                refs.push(&p.path);
            }
            FeatureKind::Loft(p) => {
                out.extend(p.profiles.iter().map(ProfileSelection::sketch));
                refs.extend(&p.guides);
                refs.extend(&p.centerline);
                for end in [&p.start, &p.end] {
                    if let LoftEnd::Direction { direction, .. } = end {
                        refs.push(direction);
                    }
                }
            }
            FeatureKind::Boundary(p) => {
                out.extend(p.sections.iter().map(ProfileSelection::sketch));
                refs.extend(&p.curves);
            }
            FeatureKind::Combine(p) => out.extend(p.bodies.iter().filter_map(BodyId::creator)),
            FeatureKind::MoveBody(p) => out.extend(p.bodies.iter().filter_map(BodyId::creator)),
            FeatureKind::SplitBody(p) => {
                out.extend(p.bodies.iter().filter_map(BodyId::creator));
                refs.push(&p.plane);
                refs.extend(&p.tools);
            }
            FeatureKind::Sketch(s) => refs.push(&s.plane),
            FeatureKind::Extrude(e) => {
                out.push(e.sketch);
                for end in std::iter::once(&e.end1).chain(e.end2.as_ref()) {
                    refs.extend(end.target());
                }
            }
            FeatureKind::Revolve(r) => {
                out.push(r.sketch);
                refs.push(&r.axis);
                refs.extend(r.angle.target());
                refs.extend(r.end2.as_ref().and_then(RevolveAngle::target));
            }
            FeatureKind::Hole(h) => out.push(h.sketch),
            FeatureKind::Rib(r) => out.push(r.sketch),
            FeatureKind::BaseFlange(b) => {
                out.push(b.sketch);
                for end in b.end1.iter().chain(&b.end2) {
                    refs.extend(end.target());
                }
            }
            FeatureKind::EdgeFlange(e) => {
                refs.extend(&e.edges);
                refs.extend(e.length.target());
                refs.extend(e.angle_from.iter().map(|a| &a.face));
            }
            FeatureKind::FlatPattern(f) => {
                refs.extend(&f.fixed_face);
                refs.extend(&f.exclude);
            }
            FeatureKind::Hem(h) => refs.extend(&h.edges),
            FeatureKind::MiterFlange(m) => {
                out.push(m.sketch);
                refs.extend(&m.edges);
            }
            FeatureKind::SketchedBend(b) => {
                out.push(b.sketch);
                refs.extend(&b.fixed);
            }
            FeatureKind::Jog(j) => {
                out.push(j.sketch);
                refs.extend(&j.fixed);
            }
            FeatureKind::CrossBreak(c) => refs.push(&c.face),
            FeatureKind::SweptFlange(s) => {
                out.push(s.profile);
                refs.push(&s.path);
            }
            FeatureKind::LoftedBend(l) => out.extend([l.profile1, l.profile2]),
            FeatureKind::Rip(r) => refs.extend(r.edges.iter().map(|e| &e.edge)),
            FeatureKind::InsertBends(b) => {
                refs.push(&b.fixed_face);
                refs.extend(b.rips.iter().map(|e| &e.edge));
            }
            FeatureKind::ConvertToSheetMetal(c) => {
                refs.push(&c.fixed_face);
                refs.extend(&c.bend_edges);
            }
            FeatureKind::Unfold(u) => refs.extend(&u.fixed_face),
            FeatureKind::Fold(_) => {}
            FeatureKind::NormalCut(n) => out.push(n.sketch),
            FeatureKind::CornerRelief(c) => refs.extend(&c.bends),
            FeatureKind::ClosedCorner(c) => refs.extend(&c.faces),
            FeatureKind::WeldedCorner(w) => {
                refs.push(&w.edge);
                refs.extend(&w.stop);
            }
            FeatureKind::BreakCorner(b) => refs.extend(&b.edges),
            FeatureKind::BendNotch(b) => refs.extend(&b.bends),
            FeatureKind::TabAndSlot(t) => {
                refs.extend([&t.edge, &t.face]);
                refs.extend(t.start.iter().chain(&t.end));
            }
            FeatureKind::SheetMetalGusset(g) => refs.push(&g.bend),
            FeatureKind::FormTool(f) => {
                refs.push(&f.face);
                out.extend(f.sketch);
            }
            FeatureKind::Weldment(_) => {}
            FeatureKind::StructuralMember(m) => {
                refs.extend(m.groups.iter().flat_map(|g| &g.segments));
                refs.extend(&m.align);
            }
            FeatureKind::EndCap(c) => refs.extend(&c.faces),
            FeatureKind::WeldGusset(g) => refs.extend(&g.faces),
            FeatureKind::WeldBead(b) => match &b.path {
                weldment::BeadPath::Edges { edges } => refs.extend(edges),
                weldment::BeadPath::Faces { from, to } => refs.extend(from.iter().chain(to)),
                weldment::BeadPath::Smart { faces } => refs.extend(faces),
            },
            FeatureKind::TrimExtend(t) => {
                out.extend(t.bodies.iter().filter_map(BodyId::creator));
                match &t.boundary {
                    weldment::TrimBoundary::Faces { faces } => refs.extend(faces),
                    weldment::TrimBoundary::Bodies { bodies, .. } => {
                        out.extend(bodies.iter().filter_map(BodyId::creator))
                    }
                }
            }
            FeatureKind::FormingTool(f) => {
                refs.push(&f.stopping_face);
                refs.extend(&f.remove_faces);
                refs.extend(&f.insertion_point);
            }
            FeatureKind::Wrap(w) => {
                out.push(w.sketch);
                refs.push(&w.face);
            }
            FeatureKind::ExtrudedSurface(e) => {
                out.push(e.sketch);
                for end in std::iter::once(&e.end1).chain(e.end2.as_ref()) {
                    refs.extend(end.target());
                }
            }
            FeatureKind::RevolvedSurface(r) => {
                out.push(r.sketch);
                refs.push(&r.axis);
            }
            FeatureKind::SweptSurface(s) => refs.extend([&s.profile, &s.path]),
            FeatureKind::LoftedSurface(l) => refs.extend(&l.profiles),
            FeatureKind::BoundarySurface(b) => refs.extend(b.curves1.iter().chain(&b.curves2)),
            FeatureKind::PlanarSurface(p) => {
                out.extend(p.sketch);
                refs.extend(&p.edges);
            }
            FeatureKind::OffsetSurface(o) => refs.extend(&o.faces),
            FeatureKind::Knit(k) => out.extend(k.bodies.iter().filter_map(BodyId::creator)),
            FeatureKind::Thicken(t) => out.extend(t.body.creator()),
            FeatureKind::CutWithSurface(c) => refs.push(&c.surface),
            FeatureKind::DeleteFace(d) => refs.extend(&d.faces),
            FeatureKind::MoveFace(m) => {
                refs.extend(&m.faces);
                match &m.motion {
                    direct::FaceMotion::Offset { .. } => {}
                    direct::FaceMotion::Translate {
                        direction, up_to, ..
                    } => {
                        refs.push(direction);
                        refs.extend(up_to);
                    }
                    direct::FaceMotion::Rotate { axis, .. } => refs.push(axis),
                }
            }
            FeatureKind::ReplaceFace(r) => {
                refs.extend(&r.faces);
                refs.push(&r.replacement);
            }
            FeatureKind::DeleteHole(d) => refs.extend(&d.faces),
            FeatureKind::FilledSurface(f) => {
                refs.extend(f.boundary.iter().map(|b| &b.edge));
                refs.extend(&f.constraints);
            }
            FeatureKind::RuledSurface(r) => {
                refs.extend(&r.edges);
                match &r.direction {
                    surfaces::RuledDirection::TaperedToVector { direction, .. }
                    | surfaces::RuledDirection::PerpendicularToVector { direction }
                    | surfaces::RuledDirection::Sweep { direction } => refs.push(direction),
                    _ => {}
                }
            }
            FeatureKind::RadiateSurface(r) => {
                refs.extend(&r.edges);
                refs.push(&r.plane);
            }
            FeatureKind::TrimSurface(t) => {
                refs.extend(&t.tool);
                out.extend(t.bodies.iter().filter_map(BodyId::creator));
            }
            FeatureKind::UntrimSurface(u) => refs.extend(&u.faces),
            FeatureKind::ExtendSurface(e) => {
                refs.extend(&e.edges);
                match &e.end {
                    surfaces::ExtendEnd::UpToPoint { point } => refs.push(point),
                    surfaces::ExtendEnd::UpToSurface { target } => refs.push(target),
                    surfaces::ExtendEnd::Distance { .. } => {}
                }
            }
            FeatureKind::MidSurface(m) => {
                refs.extend(m.pairs.iter().flat_map(|p| [&p.a, &p.b]));
                out.extend(m.body.as_ref().and_then(BodyId::creator));
            }
            FeatureKind::Freeform(f) => refs.push(&f.face),
            FeatureKind::SurfaceFlatten(f) => {
                refs.extend(&f.faces);
                refs.extend(&f.fixed);
                refs.extend(&f.tears);
            }
            FeatureKind::Fillet(f) => refs.extend(f.edges.iter().chain(&f.faces)),
            FeatureKind::Chamfer(c) => refs.extend(c.edges.iter().chain(&c.faces)),
            FeatureKind::Shell(s) => refs.extend(&s.faces),
            FeatureKind::LinearPattern(p) => {
                out.extend(&p.features);
                refs.push(&p.direction1.reference);
                if let Some(d) = &p.direction2 {
                    refs.push(&d.reference);
                }
            }
            FeatureKind::CircularPattern(p) => {
                out.extend(&p.features);
                refs.push(&p.axis);
            }
            FeatureKind::Mirror(m) => {
                out.extend(&m.features);
                refs.push(&m.plane);
            }
            FeatureKind::Plane(p) => refs.extend(p.refs()),
            FeatureKind::Axis(a) => refs.extend(a.refs()),
            FeatureKind::Point(p) => refs.extend(p.refs()),
            FeatureKind::CoordSys(c) => {
                refs.push(&c.origin);
                refs.extend(c.x_axis.iter().chain(&c.z_axis));
            }
            _ => {}
        }
        for r in refs {
            match r {
                Ref::Body { id } | Ref::BodyTopology { body: id, .. } => out.extend(id.creator()),
                Ref::Feature { id }
                | Ref::SketchEntity { sketch: id, .. }
                | Ref::SketchContour { sketch: id, .. }
                | Ref::CurveSegment { curve: id, .. }
                | Ref::OrderedSketchContour(ContourSelection { sketch: id, .. }) => out.push(*id),
                _ => {}
            }
        }
        out
    }
}

/// The part document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub configurations: configurations::Configurations,
    #[serde(default)]
    pub variables: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub equations: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub body_display: std::collections::BTreeMap<BodyId, BodyDisplay>,
    /// Custom properties for BOMs and title blocks.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub properties: std::collections::BTreeMap<String, properties::Property>,
    /// How dimensions appear in drawings, by parameter key (`f2.end1.depth`,
    /// `f1.dimension.4`). Dimensions not listed are shown (part file v5).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub dimension_display: std::collections::BTreeMap<String, DimensionDisplay>,
    pub units: Units,
    /// Material name from [`materials::LIBRARY`], or a custom density.
    #[serde(default)]
    pub material: Option<materials::Material>,
    pub features: Vec<Feature>,
    /// Only the first `rollback` features are built. `None` builds all.
    #[serde(default)]
    pub rollback: Option<usize>,
    /// Weldment cut list settings.
    #[serde(
        default,
        skip_serializing_if = "weldment::cutlist::CutListSettings::is_empty"
    )]
    pub cut_list: weldment::cutlist::CutListSettings,
    next_id: FeatureId,
}

impl Default for Document {
    fn default() -> Self {
        Document {
            configurations: Default::default(),
            variables: Default::default(),
            equations: Default::default(),
            body_display: Default::default(),
            properties: Default::default(),
            dimension_display: Default::default(),
            units: Units::default(),
            material: None,
            features: Vec::new(),
            rollback: None,
            cut_list: Default::default(),
            next_id: 1,
        }
    }
}

/// Drawing display of one dimension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DimensionDisplay {
    /// Imported into drawings by model items.
    #[serde(default = "shown")]
    pub show: bool,
    /// Name shown with the value in drawings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn shown() -> bool {
    true
}

impl Default for DimensionDisplay {
    fn default() -> Self {
        DimensionDisplay {
            show: true,
            name: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ModelError {
    #[error("no feature {0}")]
    NoFeature(FeatureId),
    #[error("{0}")]
    Invalid(String),
}

impl Document {
    /// Validate persistent identity before any geometry is rebuilt.
    pub fn validate(&self) -> Result<(), String> {
        self.configurations.validate()?;
        for (name, p) in self.properties.iter().chain(
            self.configurations
                .items
                .iter()
                .flat_map(|c| c.properties.iter()),
        ) {
            p.validate(name)?;
        }
        if self
            .variables
            .keys()
            .any(|name| !equations::valid_variable(name))
        {
            return Err("invalid or reserved global variable name".into());
        }
        let mut ids = std::collections::HashSet::new();
        for f in &self.features {
            if f.id == 0 || f.id == FeatureId::MAX || !ids.insert(f.id) {
                return Err(format!("invalid or duplicate feature id {}", f.id));
            }
        }
        if self.next_id == 0
            || self.next_id == FeatureId::MAX
            || self.features.iter().any(|f| f.id >= self.next_id)
        {
            return Err("next feature id must be greater than existing ids".into());
        }
        if self.rollback.is_some_and(|r| r > self.features.len()) {
            return Err("rollback position is outside the feature tree".into());
        }
        Ok(())
    }

    pub fn new() -> Document {
        Document::default()
    }

    pub fn feature(&self, id: FeatureId) -> Option<&Feature> {
        self.features.iter().find(|f| f.id == id)
    }

    pub fn feature_mut(&mut self, id: FeatureId) -> Option<&mut Feature> {
        self.features.iter_mut().find(|f| f.id == id)
    }

    pub fn index_of(&self, id: FeatureId) -> Option<usize> {
        self.features.iter().position(|f| f.id == id)
    }

    /// A name like "Fillet3" that isn't used yet.
    pub fn next_name(&self, kind: &FeatureKind) -> String {
        let base = kind.base_name();
        (1..)
            .map(|i| format!("{base}{i}"))
            .find(|n| !self.features.iter().any(|f| &f.name == n))
            .unwrap()
    }

    /// Insert a feature at the rollback position (the end when not rolled
    /// back), returning its id.
    pub fn add(&mut self, kind: FeatureKind) -> FeatureId {
        let id = self.next_id;
        self.next_id += 1;
        let name = self.next_name(&kind);
        let at = self
            .rollback
            .unwrap_or(self.features.len())
            .min(self.features.len());
        self.features.insert(
            at,
            Feature {
                id,
                name,
                suppressed: false,
                scope: None,
                kind,
            },
        );
        if let Some(r) = &mut self.rollback {
            *r += 1;
        }
        id
    }

    /// Delete features, and every feature that depends on them.
    pub fn delete(&mut self, ids: &[FeatureId]) -> Vec<FeatureId> {
        let mut doomed: Vec<FeatureId> = ids.to_vec();
        loop {
            let more: Vec<FeatureId> = self
                .features
                .iter()
                .filter(|f| !doomed.contains(&f.id))
                .filter(|f| f.depends_on().iter().any(|d| doomed.contains(d)))
                .map(|f| f.id)
                .collect();
            if more.is_empty() {
                break;
            }
            doomed.extend(more);
        }
        if let Some(r) = &mut self.rollback {
            let before = self.features[..(*r).min(self.features.len())]
                .iter()
                .filter(|f| doomed.contains(&f.id))
                .count();
            *r -= before;
        }
        self.features.retain(|f| !doomed.contains(&f.id));
        for config in &mut self.configurations.items {
            config.suppressed.retain(|id, _| !doomed.contains(id));
            config
                .parameters
                .retain(|key, _| !doomed.iter().any(|id| key.starts_with(&format!("f{id}."))));
            config
                .equations
                .retain(|key, _| !doomed.iter().any(|id| key.starts_with(&format!("f{id}."))));
        }
        self.equations
            .retain(|key, _| !doomed.iter().any(|id| key.starts_with(&format!("f{id}."))));
        doomed
    }

    /// Move a feature to a new position, if that keeps every feature after
    /// the things it depends on.
    pub fn reorder(&mut self, id: FeatureId, to: usize) -> Result<(), ModelError> {
        let from = self.index_of(id).ok_or(ModelError::NoFeature(id))?;
        let mut trial = self.features.clone();
        let f = trial.remove(from);
        trial.insert(to.min(trial.len()), f);
        for (i, f) in trial.iter().enumerate() {
            for d in f.depends_on() {
                if !trial[..i].iter().any(|g| g.id == d) {
                    return Err(ModelError::Invalid(format!(
                        "{} must come after the features it uses",
                        f.name
                    )));
                }
            }
        }
        self.features = trial;
        Ok(())
    }
}

/// Unit vector helper used across features.
pub(crate) fn unit(v: DVec3) -> DVec3 {
    v.normalize_or(DVec3::Z)
}
