# 5. Surfaces

Detailed specs for [catalog section 5](../solidworks-feature-catalog.md#5-surfaces). See the [README](README.md) for sources and conventions.

Surface features make zero-thickness sheet bodies. They're used to build shapes that solid features can't, then turned into solids with Knit (try to form solid), Thicken, Cut With Surface or Replace Face. Surface bodies go in the Surface Bodies folder.

**OpenForge today:** surface bodies in their own Surface Bodies folder and every surface feature below, in the core, CLI/MCP and the desktop Surfaces tab. Each section says what is supported; [the Phase 6 status](../phase-6-status.md) records the limits.

## Extruded surface

Like an extruded boss, but makes open faces. API: `ISurfExtrudeFeatureData`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Sketch or face edges | Open or closed sketch; picking a face extrudes its edges | |
| End conditions, direction 2 | Same set as the solid extrude | `EndCondition`, `Depth`, `Face`, `Vertex`, `ReverseOffset`, `TranslateSurface`, `BothDirections`. |
| Draft per direction | On, angle, outward | `D1DraftOn`, `D1DraftAngle`, `D1DraftOutward`, `D2*`. |
| Cap end per direction | Closes the end of a closed profile, making a closed volume | `D1CapEnd`, `D2CapEnd`. |
| Delete original faces, knit result | When extruding from a face: remove the face, then knit | `DeleteOriginalFace`, `KnitResult`. |

**OpenForge today:** open or closed contours, every end condition with direction 2, reverse, and cap ends for a closed surface body.

## Revolved surface

Like a revolved boss, making open faces. API: `ISurfRevolveFeatureData` (`Type`, `RevolutionAngle`, `ReverseDirection`). Same end-condition options as the solid revolve.

**OpenForge today:** blind, mid-plane, two-direction and up-to angles about any axis reference.

## Swept, lofted and boundary surfaces

The same features as their solid versions in [part features](03-part-features.md), making surfaces instead. Differences:

- Open profiles are allowed without thin.
- A boundary surface supports curvature continuity to adjacent faces in both directions and is the preferred class-A surfacing tool.
- A swept surface can sweep an edge along a path to extend a surface tangentially.

**OpenForge today:** swept along a path with an open or closed profile; lofted through ordered profiles, optionally ruled; boundary through curves in one or two directions.

## Filled surface

Patches an area bounded by edges, sketches or curves, with chosen continuity to adjacent faces. API: `IFillSurfaceFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Patch boundary | Model edges, sketch curves, curves | `PatchBoundary`. Can include gaps. |
| Curvature control per boundary | Contact, tangent, curvature | `CurvatureControl` (`swContactType_e`). Alternate face swaps the reference face on a shared edge (`AlternateFace`). |
| Constraint curves | Curves the patch must pass through | `ConstraintCurves`. |
| Resolution | Quality slider | `ResolutionControl`. |
| Optimize surface | Simpler surface for 3- and 4-sided patches | `OptimizeSurface`. |
| Fix up boundary | Builds a valid boundary from imperfect edges | |
| Merge result, try to form solid, reverse | Joins the patch to the adjacent faces; closes a volume | `Merge`, `TryToFormSolid`, `ReverseDirection`, `ReverseSurface`. |

**OpenForge today:** a boundary loop of edges and curves with contact, tangent or curvature per edge (or the other face's side), constraint curves, merge into the surface the edges belong to, and make a solid when the result closes.

## Freeform surface

See [Freeform](03-part-features.md#freeform). Applied to surface bodies, it reshapes a face by moving control points.

**OpenForge today:** a grid of control points across a face of a surface or a solid, moved by offsets, with contact, tangent or curvature at its edges.

## Planar surface

A flat surface from a closed sketch or a closed set of coplanar edges. API: `ISurfacePlanarFeatureData` (`BoundingEntities`).

**OpenForge today:** the closed regions of a sketch, or a loop of coplanar edges.

## Offset surface

Copies faces at a distance; distance 0 makes an exact copy. API: `ISurfaceOffsetFeatureData` (`Entities`, `Distance`, `Flip`).

**Rule:** like shell, the offset fails if the distance exceeds the smallest radius of curvature on the offset side.

**OpenForge today:** faces offset along or against their normals; distance 0 copies them.

## Ruled surface

Builds a strip of straight-line surface from edges. API: `IRuledSurfaceFeatureData`, type in `swRuledSurfaceType_e`.

| Type | Meaning |
| --- | --- |
| Tangent to surface | Strip tangent to the face at the edge |
| Normal to surface | Strip normal to the face |
| Tapered to vector | At an angle to a direction |
| Perpendicular to vector | Perpendicular to a direction |
| Sweep | Swept along the edge, around a reference vector |

Options: distance, angle (tapered), direction vector, alternate side, connect surfaces at corners, trim and knit.

**OpenForge today:** tangent to surface, normal to surface, tapered to a vector, perpendicular to a vector and sweep, with alternate side and knitting the strips.

## Radiate surface

Projects edges outward along a plane, like a flange for parting surfaces. API: `ISurfaceRadiateFeatureData` (`RadiatedEntities`, `DirectionReference` plane, `Distance`, `Flip`, `PropagateToTangentFaces`).

**OpenForge today:** strips from edges parallel to a plane, with flip.

## Surface from mesh

Fits analytic or freeform surfaces to regions of a mesh body. Picks a facet region, fits a plane, cylinder, cone, sphere or freeform surface, and extends it to intersect neighbours. See [mesh and scan data](09-mesh-and-scan-data.md).

**OpenForge today:** plane, cylinder, cone, sphere, torus or freeform fits (or the simplest that fits) to painted triangles or a Segment Mesh region, extended past them by a distance.

## Knit surface

Joins surfaces that share edges into one surface body, optionally a closed solid. API: `ISurfaceKnitFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Surfaces and faces | | `Entities`. |
| Create solid | Tries to make a solid when the knit is watertight | `UseTryToFormSolid`. |
| Merge entities | Removes redundant edges and vertices | `UseMergeEntities`. |
| Gap control | Knit tolerance; gap filter range; shows gaps in the graphics | `KnitTolerance`, `UseGapFilters`, `MinValueForGapRange`, `MaxValueForGapRange`. |
| Seed face | Orientation reference | `SeedFace`. |

**OpenForge today:** surface bodies joined within a tolerance, and a solid when they close; it fails when they don't. No merge entities option.

## Trim surface and untrim surface

**Trim** cuts surfaces with planes, sketches or other surfaces. API: `ISurfaceTrimFeatureData`, type in `swSurfaceTrimType_e`.

- **Standard trim** (`swTypeTrimTool`): a tool trims the target surfaces.
- **Mutual trim** (`swTypeMutualTrim`): surfaces trim each other.
- Keep selections or remove selections picks the pieces (`PiecesToKeep`).
- Split all splits without discarding; natural or linear surface split options.

**Untrim** restores a trimmed surface's natural boundary or fills internal holes. Options: fill holes, extend edges, merge with original.

**OpenForge today:** trim by a plane, face or surface, or mutually, keeping or removing the pieces at picked points, or splitting all; untrim to the natural bounds or by filling holes.

## Extend surface

Extends surface edges. API: `ISurfaceExtendFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Edges or faces | Picking a face extends all its edges | `Items`, `PropagatingEdges`. |
| End condition | Distance, up to point, up to surface | `EndCondition` (`swSurfaceExtendEndCond_e`), `Distance`, `Vertex`, `Face`. |
| Extension type | Same surface (continues the curvature) or linear (tangent) | `Type`. |

**OpenForge today:** open edges extended by a distance, up to a point or up to a surface, on the same surface or linearly.

## Delete face and delete hole

Removes faces from a solid or surface body. API: `IDeleteFaceFeatureData`, option in `swFaceDeleteOption_e`.

| Option | Meaning |
| --- | --- |
| Delete | Removes the faces, leaving a surface body |
| Delete and patch | Removes them and lets the neighbouring faces extend to close the gap |
| Delete and fill | Removes them and fills the gap with a new patch |
| Delete and fill with tangent | Fill, tangent to the neighbours |

**Delete hole** removes internal loops (holes) from a surface.

**OpenForge today:** delete, patch, fill and fill tangent; delete hole on surface faces.

## Replace face

Replaces faces of a solid with surfaces; the solid grows or shrinks to the new surface. API: `IReplaceFaceFeatureData` (`FacesForReplacement`, `ReplacementSurfaces`). The replacement surface must fully cover the replaced faces' boundary.

**OpenForge today:** faces of a solid replaced by a surface body or face; the neighbours extend or trim to meet it.

## Mid-surface

Creates a surface midway between pairs of offset faces, for simulation shell meshes. Options: face pairs (manual or find face pairs with a threshold), position (percentage between faces), knit surfaces.

**OpenForge today:** found or picked face pairs, a threshold, position between the faces, and knitting.

## Thicken

Turns a surface into a solid by adding thickness, or fills a closed volume. API: `IThickenFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Surface | One surface body | `Surface`. |
| Thickness and side | Side 1, side 2, both | `Thickness`, `ThicknessSide` (`swThickenThicknessType_e`). |
| Create solid from enclosed volume | For knitted closed surfaces | `FillVolume`. |
| Boss or cut | Thickened cut removes material | `BossFeature`. |
| Merge result, feature scope | | `Merge`, `FeatureScope`. |

**OpenForge today:** side 1, side 2 or both, cut, merge, and fill the volume of a closed surface body.

## Cut with surface

Removes the part of a solid on one side of a surface. API: `ISurfaceCutFeatureData` (`SurfaceForCut`, `Flip`, `FeatureScope`). The surface must extend fully across the body.

**OpenForge today:** the side the surface's normal points to is removed, or the other with flip.

## Surface flatten

Flattens a doubly-curved face or surface to a flat pattern, for fabric, leather and formed sheet. API: `ISurfaceFlattenFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Faces | Faces or surfaces to flatten | `Faces`. |
| Fixed vertex | Anchor point | `FixPointVertex`. |
| Accuracy | Mesh accuracy | `AccuracyFactor`. |
| Relief cuts | Tear edges where the flat needs to split | `ShouldMakeTears`, `TearEdges`. |
| Guide and map edges | Control the flat shape | `GuideEdges`, `MapEdges`, `KeepInternalControlCurves`. |

Also produces a deviation plot showing stretch and compression.

**OpenForge today:** faces flattened with a fixed point, accuracy and tear edges; seams open by themselves. The flat surface body and a per-triangle stretch plot are shown, and the outline goes to the part's DXF. Guide and map edges are not offered; outlines are lines, not fitted curves, and are not drawing views.

## Projected surface area

Evaluate tool that measures the area of selected faces projected on a plane. Options: faces, projection plane, include or exclude hidden faces. Covered in [evaluate tools](17-evaluate-and-analysis-tools.md).

**OpenForge today:** `doc.analysis` and the Evaluate panel: the area of faces seen along a plane's normal, with hidden overlaps counted once or twice.
