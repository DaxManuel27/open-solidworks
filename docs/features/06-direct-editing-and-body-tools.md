# 6. Direct editing and body tools

Detailed specs for [catalog section 6](../solidworks-feature-catalog.md#6-direct-editing-and-body-tools). See the [README](README.md) for sources and conventions.

## Move face

Edits faces directly, without going back to the feature that made them. Stored as a feature in the tree. API: `IMoveFaceFeatureData`, type in `swMoveFaceType_e` (Offset 0, Translate 1, Rotate 2).

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Type | Offset (along each face's normal), translate (along a direction), rotate (about an axis) | `MoveType`. |
| Faces | One or more faces, from solids or surfaces | `Faces`. |
| Distance or angle | | `Distance`, `Angle`. |
| Direction or axis | Edge, axis, plane normal, or the triad | `DirectionReference`, `TriadTranslationParameters`, `TriadRotationParameters`. |
| End condition (translate) | Blind, up to vertex, up to surface, offset from surface, up to body | `EndCondition`, `EndConditionEntity`, `OffsetDistance`. |
| Reverse | | `ReverseDirection`. |
| Copy | Copies the faces instead (surfaces only) | |

**Rules:** neighbouring faces extend or trim to stay connected. A move that would change the topology too much, for example moving a face past a neighbour, fails.

**OpenForge today:** not available.

## Delete/keep body

Removes bodies from the part from this point in the tree on, or keeps only the chosen ones. API: `IDeleteBodyFeatureData` (`Bodies`, `KeepBodies`).

**OpenForge today:** not as a feature. Bodies can be hidden, and split can drop pieces.

## Move/copy body

Moves, rotates or copies bodies, either by numbers or with mates as in an assembly. API: `IMoveCopyBodyFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Bodies | Solid or surface | `Bodies`. |
| Copy | On/off, number of copies | `Copy`, `NumberOfCopies`. |
| Translate | Delta X/Y/Z; along an edge by a distance; to a vertex | `TransformType` (`swMoveCopyBodyFeatureTransformType_e`), `TransformX/Y/Z`, `TransformReferenceEntity`, `TransformValue`, `TranslateToVertex`. |
| Rotate | About an origin with angles about X, Y and Z; about an edge by an angle | `RotationOriginX/Y/Z`. |
| Constraints mode | Coincident, parallel, perpendicular, tangent, concentric, distance and angle mates between the body and other geometry | `AddMate`. |

**OpenForge today:** rigid move and copy by translation and XYZ rotation about an origin.
**Missing:** copy count, move along an edge, move to a vertex, rotate about an edge, mate-based positioning.

## Combine

Boolean operations between solid bodies. API: `ICombineBodiesFeatureData`, type in `swCombineBodiesOperationType_e` (Add 0, Subtract 1, Common 2).

| Option | Values | Notes |
| --- | --- | --- |
| Operation | Add, subtract, common | `OperationType`. |
| Main body | The body others are subtracted from | `MainBody`. |
| Bodies to combine | | `BodiesToCombine`. |

**Failure cases** (from `swBodyOperationError_e`): Boolean failure, result would be disjoint bodies, empty result, empty or invalid input body, failed geometry condition, failure to cut, intersecting a solid with sheets, missing geometry.

**OpenForge today:** union, subtraction and intersection, with the first selected body as the subtraction target.

## Intersect

Builds solids from the regions where solids, surfaces and planes overlap or enclose space. API: `IIntersectFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Tools | Solids, surfaces, planes | `IntersectionsTools`. |
| Mode | Create intersection, create internal regions, create both | Picked from the PropertyManager. |
| Regions to exclude | After Intersect is pressed, the regions found can be excluded one by one | `Intersections`. |
| Merge result | Merge kept regions into one body | `Merge`. |
| Consume surfaces | Removes the input surfaces | `Consume`. |
| Cap planar openings | Closes flat openings in surfaces | `CapPlanar`. |

Common use: model a bottle's outside, then intersect with a plane to get the inner volume.

**OpenForge today:** not available (combine's common operation covers the solid–solid case).

## Split

Splits a body into several bodies using planes, faces, surfaces or sketches, optionally saving each as a part. API: `ISplitBodyFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Trim tools | Planes, faces, surfaces, sketches | `TrimTools`. |
| Target bodies | Auto-select or picked | |
| Cut part | Computes the pieces | |
| Resulting bodies | Pick which pieces to keep, each with an optional file name | `SplitBodies`. |
| Consume cut bodies | Removes unselected pieces | `Consume`. |
| Template | Template for saved part files | `TemplatePath`, `OverrideDefaultTemplateSettings`. |

**OpenForge today:** general split by planes, faces and tool bodies, with chosen pieces kept by stable IDs.
**Missing:** sketch tools, saving pieces to files.

## Save bodies and create assembly

Saves bodies as separate part files, optionally with an assembly that places them. API: `ISaveBodyFeatureData` (`AddSaveBodies`, `AssemblyPath`, `ConsumeBody`, `CopyCustomProperties`). The new parts are derived parts linked to the original.

**OpenForge today:** not available; needs assemblies first.

## Insert part and derived part

Inserts another part as a body that stays linked to its source. API: `IDerivedPartFeatureData`.

| Option | Values |
| --- | --- |
| Source configuration | `PartConfiguration` |
| What to transfer | Solid bodies, surface bodies, axes, planes, cosmetic threads, absorbed and unabsorbed sketches, custom properties, cut list properties, coordinate systems, model dimensions, Hole Wizard data, material, sheet metal information |
| Link | Break or lock the external reference; locate the part with Move/Copy (`MoveFeature`) |

**OpenForge today:** STEP and IGES import stores a body in the document, which isn't linked.
**Missing:** linked insert of `.ofp` parts.

## Mirror part

Creates a new, opposite-hand part linked to the original. API: `IMirrorPartFeatureData`. Transfer options are the same as derived parts plus DimXpert annotations and unlocked sheet metal properties. Also available: break link to original part.

**OpenForge today:** not available. Mirror of the whole body inside a part works.

## Defeature

Simplifies a part or assembly for sharing or simulation.

- **Manual (part):** pick features or faces to remove, with a preview; or silhouette mode.
- **Silhouette (assembly):** replaces components with simple shapes, such as a bounding box, polygon extrusion or cylinder.
- Options: remove internal components, holes, fillets, small features below a size; keep moving parts; save as a new file with or without a link.

**OpenForge today:** not available.

## Dependency editing and dynamic feature editing

Drag features in the tree to reorder them, with a check that references allow it. Dynamic Reference Visualization shows parent and child arrows when hovering in the tree. Instant3D drags feature handles in the graphics area.

**OpenForge today:** feature reordering is not exposed. Viewport handles exist for spatial sketches.

## Instant3D

Drag arrows on a selected face or feature to change its size, with an on-screen ruler for exact values; drag sketch contours of a feature to edit its sketch; alt-drag to copy. Toggle on or off in the Features toolbar.

**OpenForge today:** not available.

## Body compare

Compares two bodies, or a body with another file, and colours the differences: matching faces, modified faces, unique faces, and volume differences.

**OpenForge today:** not available.

## Slicing tool

Creates a series of section sketches through bodies at evenly spaced planes. Options: reference plane, number of slices, offset and spacing, slice bounding plane size. Results can be converted to sketches for lofting.

**OpenForge today:** not available.
