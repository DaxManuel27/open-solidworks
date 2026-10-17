# Phase 6 implementation plan

Status: proposed 2026-10-16, awaiting the owner's [decisions](#decisions-needed). To be built
on `phase-6`, branched from `phase-5`. It is rebased onto `main` once Phases 4 and 5 merge
(see [Phase 5 status](phase-5-status.md)). Work starts after Phase 5's release gate (P5.9)
passes, apart from P6.0 prototypes, which touch only `of-kernel-sys` and new test files.

## Goal and starting point

Deliver the surfaces, mold and direct editing scope in [the roadmap](plan.md#scope-and-phases):
all surface features, mold tools (draft, undercut, parting line, core/cavity), move, delete and
replace face, and mesh and scan data. The phase covers four catalog areas:

- [5 Surfaces](features/05-surfaces.md);
- [6 Direct editing and body tools](features/06-direct-editing-and-body-tools.md);
- [9 Mesh and scan data](features/09-mesh-and-scan-data.md);
- [12 Mold design](features/12-mold-design.md).

It also covers the shape-changing part features from area 3 that no other phase covers. Every
delivered capability must work through the desktop and through the shared core used by
JSON-RPC, CLI and MCP.

The Phase 5 baseline provides primitives worth keeping:

- **Multibody parts:**
  - `BodyId` and feature scope;
  - combine, move/copy (translate and XYZ rotate) and split by planes, faces and tool bodies.
  - The aggregate `State.body` is already a compound of every body, so references to faces of
    new body kinds resolve through the same naming.
- **Sweep, loft and boundary.** These already build their faces with
  `BRepOffsetAPI_ThruSections`, `MakePipeShell` and `GeomFill` before closing them into
  solids. Open profiles only need the closing step skipped.
- **Draft.** Neutral plane, parting line and step draft, with signed angles.
- **Kernel tools that surfaces reuse:**
  - sewing (`BRepBuilderAPI_Sewing`), `ShapeFix` and `ShapeUpgrade_UnifySameDomain`;
  - `BRepAlgoAPI_Splitter` and `BRepOffsetAPI_MakeThickSolid`;
  - projection, and faceted B-rep from triangles (`polyhedron`, from the lofted bend).
- **Drawings.** HLR, including the polygonal HLR in `BRep_PolyAlgo`, which suits mesh bodies.
- **Assemblies.** The part cache and linked parts (forming tools in Phase 5), which a derived
  part needs.
- **Imports.** STEP and IGES imports embedded in the part file, with faces named by their
  position in the file and a content hash.

Gaps that shape this phase:

- **Bodies are solids only.** `SolidBody` has no kind. Mass properties, validity checks,
  `aggregate_bodies` users and every feature assume closed solids. IGES and STEP files that
  hold open shells fail or fuse wrongly.
- **No face-local operations.** Nothing removes a face and extends its neighbours, or moves
  or replaces one. Fillet, shell and draft are the only operations that change faces in place.
- **No triangle data.** The core has no triangle mesh type besides display tessellation. There
  is no import of STL, OBJ, PLY or 3MF, and no spatial index for ray casting.
- **No analysis display.** The viewport can't colour faces by a computed class. Draft and
  undercut analysis need this, and Phase 7's analysis tools will reuse it.
- **No ribbon tabs.** There are no Surfaces, Mold Tools, Direct Editing or Mesh tabs.

## Delivery rules

The Phase 5 rules carry over unchanged:

- **Vertical slices.** Each milestone is a vertical slice: kernel, model, references,
  protocol, desktop creation and editing, persistence and regression tests.
- **Code layout.** Keep the crate dependency direction. Keep all C++ and `unsafe` in `*-sys`
  crates.
- **Units.** Model units stay millimetres and radians.
- **Failures.** A failed operation keeps the last valid state and reports an error the user
  can act on.
- **Unsupported variants.** Record unsupported variants explicitly. Do not mark Phase 6
  complete while a required item is blocked.

Three rules specific to this phase:

- **Every body has a kind, and features say which kinds they take.**
  - The kinds are solid, surface and mesh.
  - Each feature declares which body kinds it accepts and produces.
  - The model rejects wrong inputs with an error before calling the kernel. It never fuses a
    surface into a solid by accident.
  - Mass properties, interference and solid exports count solids only, and say so.
- **Direct edits are history features.**
  - Move, delete and replace face are features in the tree, as in SOLIDWORKS.
  - They reference faces by stable name, so editing an earlier feature re-applies them.
  - A face they create is named from the face it replaces. References downstream survive.
- **Analysis never changes the model.**
  - Draft, undercut and parting line analysis, and mesh deviation, are queries.
  - They return per-face or per-vertex classes.
  - Only features (Parting Line, Split Line) write geometry.

## Scope

Required in this phase (roadmap items plus what they cannot work without):

| Area | Required |
| --- | --- |
| Surface bodies | A body kind for surfaces; a Surface Bodies folder in the tree; selection, display (both sides shaded, open edges highlighted), hide/show, delete/keep, move/copy, STEP and IGES import and export of open shells; surfaces in drawing views; existing features accept surfaces where SOLIDWORKS does (up to surface and offset from surface end conditions, split body, split line, combine refused with a clear error) |
| Surface features | Extruded, revolved, swept, lofted, boundary, planar, offset, ruled (all five types), radiate, filled (contact, tangent and curvature per boundary, constraint curves, merge, try to form solid), knit (create solid, merge entities, gap control), trim (standard and mutual, keep or remove pieces, split all), untrim, extend (distance, up to point or surface, same or linear), mid-surface (manual pairs and find pairs), thicken (side 1, 2 or both, fill volume, boss or cut), cut with surface, freeform, surface flatten (fixed vertex, accuracy, relief cuts, deviation plot), projected surface area |
| Face editing | Delete face (delete, patch, fill, fill with tangent), delete hole, move face (offset, translate with every end condition, rotate; copy for surfaces), replace face |
| Body tools | Delete/keep body; intersect (regions, exclude regions, merge, consume, cap planar openings); move/copy body extras (copy count, along an edge, to a vertex, about an edge, mate-based positioning); split by sketches and saving pieces to files; save bodies and create assembly; insert part and derived part (with transfer options and link, lock or break); mirror part; defeature (manual on parts, silhouette on assemblies); body compare; slicing tool |
| Shape features | Scale (uniform and per axis, about centroid, origin or a coordinate system), indent, flex (bend, twist, taper, stretch), deform (point, curve to curve, surface push), freeform on solids |
| Interactive editing | Instant3D drag handles on faces and features with an on-screen ruler; feature reordering with reference checks and parent/child display |
| Mold tools | Draft analysis (positive, negative, requires draft, straddle, gradual transition, steep faces), undercut analysis (direction 1 and 2, occluded, straddle, none), parting line analysis and Parting Line feature (auto core/cavity split, split straddle faces), shut-off surfaces (contact, tangent, no fill), parting surface (tangent, normal, perpendicular to pull; smoothing), interlock surface, tooling split (block depths, interlock angle), core (side cores and lifters), cavity, scale for shrinkage from the material, mold folders (core, cavity, parting surface bodies) |
| Mesh | Import STL, OBJ, OFF, PLY, 3MF and AMF as a graphics body, a surface or a solid (units, facet limit, group facets into faces, colours); mesh BREP and hybrid bodies in booleans and cuts; sketches and reference planes and axes on facets; convert to mesh body (chord height, angle, group facets); decimate (body or region, percentage or tolerance, preserve boundary); segment by curvature; paint select facets |
| Scan data | Point cloud import (XYZ, TXT, ASC); mesh prep (orientation, noise and outlier removal, simplify, smooth, fill holes, smooth boundaries); surface from mesh (plane, cylinder, cone, sphere, torus and freeform fits per region, extended to meet neighbours); guided surface wizard (recognised regions to a knitted solid); curve wizard (section curves by plane into sketches); deviation analysis |
| Output | Surface bodies in STEP and IGES; mesh bodies in STL; CLI and MCP access to every feature and analysis; `doc.analysis` queries |
| Drawings | Views of surface and mesh bodies; parting line and mold bodies in views; surface flatten outlines as views and DXF |

Deferred to later phases or extensions, and recorded in the status document (pending the
[decisions](#decisions-needed)):

- Automatic whole-mesh NURBS patch networks in the ScanTo3D surface wizard. Guided creation
  covers the same parts region by region.
- The legacy assembly-level Cavity feature. The derived-part workflow covers it.
- VDA and IGES point clouds.
- Mesh textures in viewport rendering. This waits for Phase 7 appearances; colours are kept.
- FeatureXpert and DraftXpert (automatic fixes).
- Mesh export to OBJ, PLY and 3MF. This belongs to area 19 export work.

## Decisions needed

| Decision | Recommendation | Why |
| --- | --- | --- |
| Body model | Keep one body list. Give each body a `kind` (solid, surface or mesh) and rename `SolidBody` to `PartBody`. Surface bodies are OCCT shells, or faces, with the same `Naming`. Graphics mesh bodies are Rust indexed triangle arrays outside OCCT. Mesh BREP bodies are OCCT shells or solids whose faces are facet groups. | One list keeps `BodyId`, feature scope, folders and the aggregate compound working unchanged. Graphics meshes of millions of triangles would be slow and memory-hungry as B-rep, and they only need display, measure and mate references |
| Face editing engine | Build one face-replacement routine in `of-kernel-sys` for move face, replace face and delete-and-patch. It removes the faces, extends the neighbouring surfaces, intersects them with the new surface (or with each other, for patch) and rebuilds the loops. Delete-and-patch is tried first with `BRepAlgoAPI_Defeaturing`. Offset move face uses `BRepOffset_MakeOffset` with per-face offsets. P6.0 measures both on the corpus before committing. | OCCT has no general move-face. Defeaturing covers removal but not moved or replacement surfaces, and one routine keeps the naming rules in one place. Measuring first avoids betting the milestone on one OCCT algorithm |
| Undercut and draft analysis | Draft analysis is exact. Sample each face's normal against the pull direction, and split straddle faces at silhouette curves from `Contap_Contour`. Undercut analysis ray casts on the display mesh with a Rust BVH, then classifies faces by their samples. The Parting Line feature uses the exact silhouette curves. | Classification only needs to be right per face, and ray casting on triangles is fast and robust. Geometry written into the model (parting lines and split faces) stays exact |
| Mesh storage | Embed imported meshes in the part file, compressed, as STEP and IGES imports already are, with an optional link to the source file. Default facet limit for mesh BREP conversion: 100,000. Above it, imports stay graphics bodies with a message. | Matches the existing Import feature and keeps parts self-contained. The limit keeps booleans responsive; P6.0 confirms the number |
| Surface from mesh and ScanTo3D | Build primitive fits (plane, cylinder, cone, sphere, torus) by least squares, and freeform fits of single regions with `GeomAPI_PointsToBSplineSurface` after parameterising the region. Neighbouring fits are extended and trimmed into a knitted body. Defer the automatic whole-mesh patch network. | Region-by-region guided creation is how most scan-to-CAD work is done, and it yields editable analytic faces. Automatic quad-patch layout is a research-grade problem of its own |
| Surface flatten | A Rust flattener on a fine triangle mesh of the faces: a conformal starting layout, then a stretch-minimising solve with the fixed vertex pinned. The outline is fitted back to B-splines, with relief cuts along tear edges and a per-triangle stretch plot. | No OCCT equivalent exists, and the Phase 5 unfolder handles only developable bends. A triangle-based solver gives the deviation plot for free |
| Shape features from area 3 | Include scale, freeform, indent, flex and deform. Flex and deform convert the affected faces to B-splines and map their control points, within a stated tolerance. | Scale is needed for mold shrinkage, and freeform is a surface feature. No later phase covers the other three, and they share the B-spline mapping |
| Area 6 scope | Include all of area 6, including derived and mirror parts, save bodies with an assembly, defeature, body compare, slicing, Instant3D and dependency editing | The roadmap's "direct editing" maps to this area, and no later phase covers it. Derived parts are also the route to cavity molds |
| Cavity | Mold a cavity in a part: insert the design part as a derived part, scale it for shrinkage, then subtract it from a block (combine), or use tooling split. Defer the legacy assembly Cavity feature until top-down assembly design exists. | Phase 3 deferred in-context editing. The part workflow gives the same core and cavity bodies |
| Part file and protocol | Part file v7: body kinds, mesh data, mold folders and the new feature kinds; migration from v6 is the identity. Protocol stays at 5: new feature types go through `feature.add` and the catalog; snapshot fields (body kind, open edges, analysis colours) are additive. Bump to 6 only if P6.0 finds that large meshes need a binary transfer path. | Older cores refuse v7 files cleanly. Clients that only read v5 snapshots keep working |

## Milestones and dependencies

| Milestone | Deliverable | Depends on | Acceptance example |
| --- | --- | --- | --- |
| P6.0 | Contracts, schema and kernel prototypes | Phase 5 complete (prototypes can start earlier) | Body kinds and part v7 fixtures; knit of a 6-face open box into a solid; delete-and-patch of a fillet and a boss on a bracket; offset and translate move face on a block; `Contap` silhouette on a drafted cup; a 1M-triangle STL loads as a graphics body and a 100k one converts to mesh BREP within budget |
| P6.1 | Surface bodies and core surface features | P6.0 | A mouse shell: boundary and lofted surfaces, a planar surface, an offset surface, knit with create solid, then thicken and cut with surface on a second body; surfaces export to STEP and re-import as surface bodies |
| P6.2 | Surface shaping and repair | P6.1 | A filled surface with curvature continuity closing a 5-sided hole (deviation at the boundary under 1e-4 mm); ruled and radiate surfaces on a part edge; mutual trim; extend (same and linear); untrim; mid-surface of a shelled box; surface flatten of a saddle checked against an analytic development |
| P6.3 | Face editing | P6.0, P6.1 | On an imported STEP bracket: delete a hole and patch, move a face by 5 mm up to a surface, rotate a face 3°, replace a face with an offset surface; every downstream fillet still references its edges after editing the base sketch |
| P6.4 | Body tools, shape features and interactive editing | P6.1 | Intersect a bottle with a plane to get its inner volume; derived part with mirror; save bodies and create an assembly; scale 1.006; flex bend and twist; indent; Instant3D drag of an extrude face to 25 mm; reordering a fillet before its shell is refused with the reference named |
| P6.5 | Mold tools | P6.1, P6.2, P6.4 (scale) | A drafted ABS box with a through-hole and a side window: draft and undercut analysis classes match hand checks; parting line found automatically; shut-off patches; parting surface; tooling split with a 5° interlock into core and cavity; a side core for the window; core and cavity rebuild when the part changes |
| P6.6 | Mesh bodies | P6.0 | Import STL, OBJ, PLY, 3MF and AMF fixtures; a mesh BREP bracket cut with a hole (hybrid body); a reference plane fitted to facets; convert a solid to mesh and decimate to 25 % with the boundary preserved; segment by curvature into the expected regions |
| P6.7 | Scan data | P6.6 | A scanned pump housing (point cloud and STL in the corpus): mesh prep, guided surface wizard to a knitted solid of plane, cylinder and freeform regions, deviation under 0.1 mm at 95 % of points; curve wizard sections loft into a solid |
| P6.8 | Corpus, conformance, performance and release gate | All above | Hosted three-platform checks pass on the exact release candidate |

Once P6.1 passes, P6.2, P6.3, P6.4 and P6.6 are independent of each other. P6.6 needs only
P6.0, so mesh work can run alongside surfaces. No calendar commitment: estimate after P6.0 has
measured the kernel prototypes.

## P6.0 — Contracts and prototypes

**Body kinds.** `PartBody { id, kind, geometry }` with `BodyKind::{Solid, Surface, Mesh}`:

- **Mesh bodies.** A mesh body holds a `TriMesh` (positions, indices, optional colours,
  facet-group ids) in place of a `Shape`. A mesh BREP body is a `Surface` or `Solid` whose
  `Naming` marks facet groups.
- **Kind checks.** `State` helpers `solids()` and `surfaces()` replace direct uses of
  `bodies`. Each existing op is audited and either restricted to solids or extended.
- **Mass and checks.** Mass properties and validity checks skip non-solids. A surface body
  that sews closed is promoted to solid only when a feature asks for it (knit with create
  solid, thicken fill volume).

**Kernel prototypes.** Each one goes in `of-kernel-sys`, with a safe wrapper in `of-geom` and
a test:

- **Knit.** Sewing with tolerance and a gap report, then `MakeSolid` with orientation fixing.
- **Fill.** `BRepOffsetAPI_MakeFilling` with per-edge `GeomAbs_C0/G1/G2` and constraint
  curves.
- **Delete and patch.** `BRepAlgoAPI_Defeaturing`, plus the face-replacement routine from the
  [decision](#decisions-needed).
- **Offset.** `BRepOffsetAPI_MakeOffsetShape` for offset surfaces. Per-face offsets go
  through `BRepOffset_MakeOffset`.
- **Silhouettes.** `Contap_Contour` silhouette curves for a direction on B-spline and
  analytic faces.
- **Trim and extend.** Trimming uses the existing splitter. Extension uses
  `GeomLib::ExtendSurfByLength` for B-splines and widened parameter bounds for analytic
  surfaces.
- **Mesh to B-rep.** Mesh to faceted B-rep for 100k triangles, timed. The `polyhedron` path
  from Phase 5 is the starting point.

**Rust geometry.** A new `of-mesh` crate (pure Rust, no C++) for:

- `TriMesh`, half-edge adjacency, and a BVH for ray casts and closest points;
- the mesh readers: STL, OBJ, OFF, PLY, 3MF and AMF (3MF and AMF are zipped XML);
- decimation by quadric error, segmentation by curvature, and hole filling;
- least-squares primitive fits;
- the flattening solver.

It depends on nothing in the workspace, so `of-model` and `of-drawing` can both use it.

**Schema.** Part file v7: `kind` on bodies; mesh data blocks; mold folder assignments; and the
new feature kinds for surfaces, face edits, body tools, shape features, mold tools, mesh and
scan. Migration from v6 is the identity, since every existing body is a solid.

**Measurements.** Time each kernel prototype on the corpus, and record its success rate in
`docs/phase-6-feasibility.md`, like Phase 2's kernel feasibility report. The face-editing
engine choice and the mesh facet limit are confirmed there.

## P6.1 — Surface bodies and core surface features

- **Surface versions of existing features.**
  - Extruded, revolved, swept, lofted and boundary surfaces share their solid features' code
    paths. They skip closing and allow open profiles.
  - The extruded surface adds draft and cap ends per direction. Extruding from a face can
    delete the original face and knit.
- **New basic surfaces.** Planar surface from a closed sketch or coplanar edges. Offset
  surface, where distance 0 copies.
- **Knit.** Create solid, merge entities, knit tolerance and gap filters. Open-edge gaps are
  returned for display.
- **Thicken.** Side 1, side 2 or both; fill an enclosed volume; boss or cut; merge and feature
  scope.
- **Cut with surface.** The surface must cross the body fully, or the feature fails with a
  clear error. Flip and feature scope are supported.
- **Delete face, delete-only option.** This is the simplest face edit. It turns a solid into
  a surface body, which many surfacing workflows start from.
- **Existing features.**
  - Up to surface and offset from surface end conditions accept surface bodies.
  - Split body and split line take surfaces as tools.
  - Combine refuses surfaces with a clear error.
- **Desktop.** Shown in a Surfaces ribbon tab:
  - a panel per feature and a Surface Bodies folder;
  - open edges drawn in their own colour;
  - two-sided shading for surfaces, with a different back colour.

## P6.2 — Surface shaping and repair

- **Filled surface.** Boundary from edges, sketches and curves, and curvature control per
  boundary. Also: constraint curves, a resolution setting and optimise for 3- and 4-sided
  patches; fix up boundary; merge and try to form solid.
- **Ruled surface.** Tangent, normal, tapered to vector, perpendicular to vector and sweep.
  Options: alternate side, connect at corners, trim and knit.
- **Radiate surface.** Edges projected along a plane, with tangent propagation.
- **Trim.**
  - Standard trim and mutual trim, with keep or remove selections and split all.
  - The pieces are named from their source face plus a region index. Re-selecting after an
    upstream edit picks the same piece.
- **Untrim.** Restore the natural boundary, fill internal holes and extend edges.
- **Extend.**
  - Distance, up to point or up to surface. Picking a face extends all its edges.
  - The extension continues the same surface or runs linear (tangent).
- **Mid-surface.**
  - Face pairs, picked manually or found by threshold, with a position percentage and knit.
  - Pairs must be offsets of each other within tolerance. Other pairs are reported, not
    approximated.
- **Freeform.**
  - Convert the face to a B-spline and expose a control-curve grid.
  - Drag points with the triad, and keep continuity to neighbours (contact, tangent,
    curvature).
- **Surface flatten.** Follows the [decision](#decisions-needed): fixed vertex, accuracy,
  relief cuts at tear edges, guide and map edges, and a stretch/compression plot. The flat is
  a planar surface body that drawings and DXF export can use.
- **Projected surface area.** An evaluate query on faces and a plane, with or without hidden
  faces.

## P6.3 — Face editing

All four features use the face-replacement routine, and name the faces they create after the
faces they replace (`{face}.moved`, `{face}.patch`).

- **Delete face** with patch, fill or fill with tangent; **delete hole** removes inner loops
  from surface faces.
- **Move face:**
  - offset along normals;
  - translate along an edge, axis, plane normal or the triad, with every end condition;
  - rotate about an axis;
  - copy, for surfaces only.
  - Moves that change the topology too much, such as a face moved past a neighbour, fail with
    the neighbour named.
- **Replace face:** the replacement surface must cover the replaced faces' boundary, or the
  feature names the gap.

Face edits work on imported bodies, where they matter most. The acceptance part is an
imported STEP bracket.

## P6.4 — Body tools, shape features and interactive editing

- **Delete/keep body.** Removes or keeps bodies from this point in the tree on. Later
  references to removed bodies report as missing.
- **Intersect.** Find regions between solids, surfaces and planes, then exclude regions one
  by one. Merge, consume surfaces and cap planar openings. Regions are identified by their
  bounding faces' names, so exclusions survive rebuilds.
- **Move/copy body extras.**
  - Copy counts.
  - Moves along an edge by a distance or to a vertex, and rotation about an edge.
  - Constraints mode: coincident, parallel, perpendicular, tangent, concentric, distance and
    angle mates, solved with `of-mate`.
- **Split extras.** Sketch tools, and saving pieces as parts with a template.
- **Save bodies and create assembly.** Each body becomes a derived part, and an assembly
  places them.
- **Insert part and derived part.**
  - A linked `.ofp` body through the Phase 3 part cache, with the transfer options listed in
    the spec. Sheet metal information is included: the Phase 5 records carry over.
  - The link can be locked or broken, and the part is placed by move/copy.
- **Mirror part.** A new linked part with the opposite hand.
- **Defeature.**
  - Manual mode on parts: remove features or faces, holes, fillets, or features below a size.
  - Silhouette mode on assemblies: bounding box, polygon extrusion or cylinder per component.
  - The result can be saved as a new file, linked or not.
- **Body compare.** Colours matching, modified and unique faces, and reports the volume
  difference.
- **Slicing tool.** Section sketches at even spacing from a plane, which convert to sketches
  for lofting.
- **Shape features.**
  - **Scale:** uniform or per axis, about the centroid, origin or a coordinate system.
  - **Indent:** target body, tool body, clearance, keep or remove selections, and a cut
    option.
  - **Flex:** bend, twist, taper and stretch, with trim planes and a triad.
  - **Deform:** point, curve to curve and surface push.
  - Following the [decision](#decisions-needed), flex and deform map B-spline control
    points. Results within the stated tolerance are kept; others fail with the measured
    deviation.
- **Interactive editing.**
  - **Instant3D:** drag handles on a selected face or feature, with a ruler for exact values.
    Dropping the handle commits a feature edit.
  - **Dependency editing:** drag features in the tree to reorder them. The model refuses a
    move that breaks a reference and names it. Hovering shows parent and child arrows.

## P6.5 — Mold tools

**Draft analysis.**

- Inputs: a pull direction (plane, face or edge), a draft angle, gradual transition and find
  steep faces.
- The result is colours plus counts per class.
- The viewport gains an analysis colour mode that any query can drive.

**Undercut analysis.**

- Inputs: a pull direction, a parting line to analyse each side, or coordinate input.
- Classes: direction 1, direction 2, occluded, straddle and no undercut.

**Parting line analysis and the Parting Line feature.**

- Inputs: a pull direction, a draft angle and use for core/cavity split.
- Straddle faces are split at the silhouette or at chosen entities.
- The feature reports when the line is incomplete or doesn't separate the part. Its edges get
  stable names from the faces either side.

**Shut-off surfaces.** Finds the loops of through openings automatically. Each loop gets a
contact, tangent or no fill patch, and the face can be flipped.

**Parting surface.**

- Mold parameters: tangent to surface, normal to surface or perpendicular to pull.
- Distance and angle, sharp or smooth transitions, and knit all.
- Manual mode exposes handles at vertices.

**Tooling split.**

- A block sketch with depths in both directions.
- It reads core, cavity and parting surfaces from the mold folders. An optional interlock
  surface takes a draft angle.
- The result is two bodies, core and cavity, made with the splitter.

**Core.** A bounding sketch, a target body, an extraction direction, depths, draft and cap
ends. It is used for side cores and lifters.

**Scale for shrinkage.**

- The Scale feature, with a default factor taken from the part's material.
- Material entries gain an optional `shrinkage` value, the way Phase 5 added sheet metal
  defaults.

**Mold folders.** Cavity Surface Bodies, Core Surface Bodies and Parting Surface Bodies under
Surface Bodies, filled by the features above and editable.

**Desktop.** A Mold Tools ribbon tab, panels per feature, analysis legends and the mold
folders in the tree.

## P6.6 — Mesh bodies

- **Import.**
  - Formats: STL (ASCII and binary), OBJ, OFF, PLY (ASCII and binary), 3MF and AMF, read by
    `of-mesh`.
  - Options: graphics body, surface or solid; units when the file has none; a facet limit;
    group facets into faces; and colours.
  - The file's data is embedded in the Import feature. The supported formats are listed in
    `ImportFormat`.
- **Mesh BREP and hybrid bodies.**
  - Booleans and cuts work between mesh and analytic bodies.
  - Shell, fillet and draft refuse facet faces with a clear error.
  - Sketches can convert facet edges and take intersection curves.
- **Reference geometry from facets.** Planes fitted to selected facets, and axes through
  cylindrical facet groups.
- **Convert to mesh body.** Keep the original, refinement, chord height, angle, and group
  facets.
- **Decimate.**
  - Applies to the whole body or a painted region.
  - Reduce by percentage or by tolerance, with boundary preservation and reset.
  - Graphics bodies are decimated in `of-mesh`. Mesh BREP bodies are decimated, then
    rebuilt.
- **Segment.** By curvature sensitivity, with boundaries shown and editable segments.
  Segments become facet groups.
- **Paint select.** A brush select of facets in the viewport. The selection is stored as
  facet ids under the body's content hash.
- **Graphics bodies in the viewport.** They come through the existing mesh path. P6.0
  decides whether meshes of millions of triangles need a binary transfer.
- **Desktop.** A Mesh ribbon tab.

## P6.7 — Scan data

- **Point clouds.** XYZ, TXT and ASC are imported, then triangulated into a mesh. A
  ball-pivoting or screened-Poisson-style reconstruction goes in `of-mesh`. P6.0 picks the
  algorithm by quality on the corpus.
- **Mesh prep wizard.** Orientation to the origin and planes, noise and outlier removal,
  simplification, smoothing, hole filling and boundary smoothing. Each step is stored in the
  feature, so it can be reapplied.
- **Surface from mesh.**
  - For a segment or a painted region, fit a plane, cylinder, cone, sphere, torus or freeform
    surface.
  - The fit reports RMS and maximum deviation. It extends to meet its neighbours, and can
    trim and knit.
- **Guided surface wizard.** Recognise regions by type, fit each, then trim and knit them to
  a solid. Regions that fail are listed for manual fitting. Automatic patch networks are
  deferred, following the [decision](#decisions-needed).
- **Curve wizard.** Section curves through the mesh at planes, fitted into sketch splines and
  lines for rebuilding by sketch.
- **Deviation analysis.** Colours each mesh vertex by its distance to the model, with
  statistics.

## Code integration map

| Area | Primary files and intended changes |
| --- | --- |
| Kernel | `of-kernel-sys`: knit, filling, defeaturing, the face-replacement routine, per-face offset, `Contap` silhouettes, surface extension, mesh to B-rep. `of-geom`: safe wrappers and a `Surface`-aware `Shape` API (open edges, shell orientation) |
| Mesh | New `of-mesh` crate (pure Rust): `TriMesh`, BVH, readers, decimation, segmentation, hole filling, fitting, point-cloud reconstruction, flattening |
| Parts | `crates/of-model`: `PartBody` with kinds and the audit of every op; `ops/surface/` (each surface feature), `ops/face_edit.rs`, `ops/body_tools/` (intersect, delete/keep, derived and mirror part, defeature), `ops/shape/` (scale, indent, flex, deform, freeform), `ops/mold/`, `ops/mesh/`; `analysis.rs` (draft, undercut, parting line, deviation, body compare, projected area); part file v7 and migration; material shrinkage |
| Drawings | `crates/of-drawing`: surface and mesh bodies in views (polygonal HLR for meshes), parting lines in views, surface flatten views and DXF |
| Core | `crates/openforge`: catalog entries, `doc.analysis` (draft, undercut, deviation, compare) and the matching MCP tools, mesh import in CLI and MCP, save bodies and derived part file handling |
| Desktop | Surfaces, Direct Editing, Mold Tools and Mesh ribbon tabs; a panel per feature; Surface Bodies and mold folders; analysis colour mode with legends; open-edge display; Instant3D handles and ruler; tree reordering with dependency arrows; paint select brush; the freeform control grid |
| Tests and docs | Kernel prototype tests, analytic fixtures (filled-surface continuity, flatten against development, scale and shrink, draft classes), mesh reader round trips, scan deviation fixtures, file round trips, serve tests, conformance checks for areas 5, 6, 9 and 12, smoke scenarios, `docs/phase-6-feasibility.md`, `docs/phase-6-status.md`, `docs/protocol.md`, feature docs 5, 6, 9 and 12 |

## P6.8 — Acceptance and release

Every behaviour change gets a regression test at the lowest useful layer:

- surfaces are checked by deviation from analytic shapes;
- continuity is checked by normals and curvature sampled along the shared edges;
- analysis classes are checked against hand-classified fixtures;
- mesh readers are checked by round trips through the STL writer.

Build a reproducible corpus:

- **Consumer housing:** a mouse shell from boundary, lofted and filled surfaces, knit into a
  solid, shelled and split into top and bottom, with Instant3D edits and reordering.
- **Injection mold:** a drafted ABS box with a through-hole and a side window, taken from
  analysis to shrink, parting line, shut-off, parting surface, interlocked tooling split and a
  side core. A drawing of the core and cavity.
- **Imported bracket:** a STEP bracket edited by delete-and-patch, move, rotate and replace
  face. A derived mirror part and save bodies to an assembly.
- **Scanned housing:** point cloud and STL to a prepped mesh, a guided surface solid within
  the deviation target, and curve wizard sections lofted into a solid.
- **Formed panel:** a doubly curved panel flattened with relief cuts. DXF of the flat checked
  against its stretch plot.

Extend the native desktop smoke test with three scenarios:

- a surfacing scenario: a boundary surface, knit to a solid, then thicken;
- a mold scenario: draft analysis, parting line and tooling split;
- a mesh scenario: import an STL, segment it, fit a surface from the mesh.

Add catalog conformance checks for areas 5, 6, 9 and 12, and for the area 3 shape features.

Set budgets in `docs/phase-6-budgets.json`:

- knit of 50 faces;
- filled surface;
- move face on the bracket;
- tooling split of the mold box;
- undercut analysis at 200k triangles;
- import of a 1M-triangle graphics body;
- mesh BREP conversion at 100k facets;
- decimation;
- guided surface fit.

Measure them with `scripts/phase-6-performance.mjs`, like Phase 5's, and check them in CI.

Run the repository gates, the native smoke test, and the hosted macOS, Windows and Linux
workflows on the release candidate. Publish supported modes and remaining limits in
`phase-6-status.md`. Merge `phase-6` to `main` only when the required scope and hosted checks
are complete.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Face editing robustness in OCCT | Move, replace and delete-and-patch fail on real parts | Measure in P6.0 before committing; one routine with a corpus of failing cases; refuse with the face that couldn't be rebuilt named, never return an invalid body |
| Filled surface and knit tolerances | Gaps or bad continuity make solids fail downstream | Report gaps and continuity deviation in the feature; validate with `BRepCheck` and fail rather than keep an invalid shell |
| Solid-only assumptions in existing ops | Surfaces silently break mass, export or booleans | The P6.0 audit of every op with a test per kind; kind checks before kernel calls |
| Large meshes | Slow imports, memory and viewport lag | Graphics bodies outside OCCT, a facet limit for B-rep conversion, budgets at 1M triangles, binary transfer if needed |
| Scan fitting quality | Users get lumpy or wrong surfaces | Primitive fits first; deviation reported on every fit; automatic patch networks deferred |
| Topological naming through trims, face edits and derived parts | References break after upstream edits | Pieces named by source face plus region bounding names; edits named after what they replace; rebuild tests that edit base sketches and check downstream references |
| Scope (four catalog areas and part of a fifth) | Phase slips | P6.2, P6.3, P6.4 and P6.6 are independent after P6.1; the ScanTo3D automatic network and the legacy Cavity are deferred (pending decisions) |
