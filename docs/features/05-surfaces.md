# 5. Surfaces

Detailed specs for [catalog section 5](../solidworks-feature-catalog.md#5-surfaces). See the [README](README.md) for sources and conventions.

Surface features make zero-thickness sheet bodies. They're used to build shapes that solid features can't, then turned into solids with Knit (try to form solid), Thicken, Cut With Surface or Replace Face. Surface bodies go in the Surface Bodies folder.

**OpenForge today:** no surface bodies or surface features. The section below describes each feature for when they're built.

## Extruded surface

Like an extruded boss, but makes open faces. API: `ISurfExtrudeFeatureData`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Sketch or face edges | Open or closed sketch; picking a face extrudes its edges | |
| End conditions, direction 2 | Same set as the solid extrude | `EndCondition`, `Depth`, `Face`, `Vertex`, `ReverseOffset`, `TranslateSurface`, `BothDirections`. |
| Draft per direction | On, angle, outward | `D1DraftOn`, `D1DraftAngle`, `D1DraftOutward`, `D2*`. |
| Cap end per direction | Closes the end of a closed profile, making a closed volume | `D1CapEnd`, `D2CapEnd`. |
| Delete original faces, knit result | When extruding from a face: remove the face, then knit | `DeleteOriginalFace`, `KnitResult`. |

## Revolved surface

Like a revolved boss, making open faces. API: `ISurfRevolveFeatureData` (`Type`, `RevolutionAngle`, `ReverseDirection`). Same end-condition options as the solid revolve.

## Swept, lofted and boundary surfaces

The same features as their solid versions in [part features](03-part-features.md), making surfaces instead. Differences:

- Open profiles are allowed without thin.
- A boundary surface supports curvature continuity to adjacent faces in both directions and is the preferred class-A surfacing tool.
- A swept surface can sweep an edge along a path to extend a surface tangentially.

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

## Freeform surface

See [Freeform](03-part-features.md#freeform). Applied to surface bodies, it reshapes a face by moving control points.

## Planar surface

A flat surface from a closed sketch or a closed set of coplanar edges. API: `ISurfacePlanarFeatureData` (`BoundingEntities`).

## Offset surface

Copies faces at a distance; distance 0 makes an exact copy. API: `ISurfaceOffsetFeatureData` (`Entities`, `Distance`, `Flip`).

**Rule:** like shell, the offset fails if the distance exceeds the smallest radius of curvature on the offset side.

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

## Radiate surface

Projects edges outward along a plane, like a flange for parting surfaces. API: `ISurfaceRadiateFeatureData` (`RadiatedEntities`, `DirectionReference` plane, `Distance`, `Flip`, `PropagateToTangentFaces`).

## Surface from mesh

Fits analytic or freeform surfaces to regions of a mesh body. Picks a facet region, fits a plane, cylinder, cone, sphere or freeform surface, and extends it to intersect neighbours. See [mesh and scan data](09-mesh-and-scan-data.md).

## Knit surface

Joins surfaces that share edges into one surface body, optionally a closed solid. API: `ISurfaceKnitFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Surfaces and faces | | `Entities`. |
| Create solid | Tries to make a solid when the knit is watertight | `UseTryToFormSolid`. |
| Merge entities | Removes redundant edges and vertices | `UseMergeEntities`. |
| Gap control | Knit tolerance; gap filter range; shows gaps in the graphics | `KnitTolerance`, `UseGapFilters`, `MinValueForGapRange`, `MaxValueForGapRange`. |
| Seed face | Orientation reference | `SeedFace`. |

## Trim surface and untrim surface

**Trim** cuts surfaces with planes, sketches or other surfaces. API: `ISurfaceTrimFeatureData`, type in `swSurfaceTrimType_e`.

- **Standard trim** (`swTypeTrimTool`): a tool trims the target surfaces.
- **Mutual trim** (`swTypeMutualTrim`): surfaces trim each other.
- Keep selections or remove selections picks the pieces (`PiecesToKeep`).
- Split all splits without discarding; natural or linear surface split options.

**Untrim** restores a trimmed surface's natural boundary or fills internal holes. Options: fill holes, extend edges, merge with original.

## Extend surface

Extends surface edges. API: `ISurfaceExtendFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Edges or faces | Picking a face extends all its edges | `Items`, `PropagatingEdges`. |
| End condition | Distance, up to point, up to surface | `EndCondition` (`swSurfaceExtendEndCond_e`), `Distance`, `Vertex`, `Face`. |
| Extension type | Same surface (continues the curvature) or linear (tangent) | `Type`. |

## Delete face and delete hole

Removes faces from a solid or surface body. API: `IDeleteFaceFeatureData`, option in `swFaceDeleteOption_e`.

| Option | Meaning |
| --- | --- |
| Delete | Removes the faces, leaving a surface body |
| Delete and patch | Removes them and lets the neighbouring faces extend to close the gap |
| Delete and fill | Removes them and fills the gap with a new patch |
| Delete and fill with tangent | Fill, tangent to the neighbours |

**Delete hole** removes internal loops (holes) from a surface.

## Replace face

Replaces faces of a solid with surfaces; the solid grows or shrinks to the new surface. API: `IReplaceFaceFeatureData` (`FacesForReplacement`, `ReplacementSurfaces`). The replacement surface must fully cover the replaced faces' boundary.

## Mid-surface

Creates a surface midway between pairs of offset faces, for simulation shell meshes. Options: face pairs (manual or find face pairs with a threshold), position (percentage between faces), knit surfaces.

## Thicken

Turns a surface into a solid by adding thickness, or fills a closed volume. API: `IThickenFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Surface | One surface body | `Surface`. |
| Thickness and side | Side 1, side 2, both | `Thickness`, `ThicknessSide` (`swThickenThicknessType_e`). |
| Create solid from enclosed volume | For knitted closed surfaces | `FillVolume`. |
| Boss or cut | Thickened cut removes material | `BossFeature`. |
| Merge result, feature scope | | `Merge`, `FeatureScope`. |

## Cut with surface

Removes the part of a solid on one side of a surface. API: `ISurfaceCutFeatureData` (`SurfaceForCut`, `Flip`, `FeatureScope`). The surface must extend fully across the body.

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

## Projected surface area

Evaluate tool that measures the area of selected faces projected on a plane. Options: faces, projection plane, include or exclude hidden faces. Covered in [evaluate tools](17-evaluate-and-analysis-tools.md).
