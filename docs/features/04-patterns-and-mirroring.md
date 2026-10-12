# 4. Patterns and mirroring

Detailed specs for [catalog section 4](../solidworks-feature-catalog.md#4-patterns-and-mirroring). See the [README](README.md) for sources and conventions.

## Shared pattern options

Every part pattern repeats a seed. These options appear on all or most of them.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| What to pattern | Features, faces, or bodies | `PatternElement` (`swPatternElementSelection_e`: FeatureFaces 1, BodiesToPattern 2), `PatternFeatureArray`, `PatternFaceArray`, `PatternBodyArray`. |
| Instances to skip | Picked instances are left out | `SkippedItemArray`. |
| Geometry pattern | Copies only faces and edges instead of re-solving each feature | `GeometryPattern`. Faster, but an instance won't adapt to its surroundings; for example, an extrude Up To Surface keeps the seed's depth. |
| Vary sketch | Lets each instance's sketch re-solve against its surroundings | `VarySketch`. The seed sketch must be dimensioned to the pattern direction. |
| Propagate visual properties | Instances inherit colours, textures and cosmetic threads | `PropagateVisualProperty`. |
| Feature scope | Which bodies the instances affect | `FeatureScope`, `FeatureScopeBodies`, `AutoSelect`. |
| Bodies option | Patterning bodies (and structure systems) instead of features | `BodyPattern`, `StructureSystemToPatternArray`. |
| Instances to vary | Change spacing or seed dimensions for specific instances | `InstancesToVary`, `InstanceToVaryOptions`. |

**Rules:** patterning a feature also patterns its child features only if they're selected. A pattern of a cut that misses the body at some positions makes those instances fail silently or with a warning, depending on settings. Pattern features themselves can be patterned and mirrored.

## Linear pattern

API: `ILinearPatternFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Direction 1 | Edge, sketch line, axis, linear dimension, planar face or plane (normal) | `D1Axis`, `D1AxisType`. A dimension as the direction patterns along the dimension. |
| Spacing mode | Spacing and instances, or up to reference | `D1EndCondition` (`swPatternEndCondition_e`). |
| Spacing, instances | Distance, total count including the seed | `D1Spacing`, `D1TotalInstances`. |
| Up to reference | Reference geometry; offset; offset from seed reference or centroid; use spacing or count | `D1EndReference`, `D1EndRefOffset`, `D1EndRefReverseOffset`, `D1EndUseSeedReference`, `D1EndSeedReference`, `D1EndUseSpacing`. |
| Reverse | Flip the direction | `D1ReverseDirection`. |
| Direction 2 | Same set of options | `D2*`. |
| Pattern seed only | Direction 2 copies only the seed, not the direction 1 instances | `D2PatternSeedOnly`. |

**OpenForge today:** features, one or two directions with edge, sketch line, axis or plane/face references; spacing and count; reverse.
**Missing:** faces and bodies as seeds, dimension as direction, up to reference, pattern seed only, skip instances, geometry pattern, vary sketch, visual property propagation, instances to vary.

## Circular pattern

API: `ICircularPatternFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Axis | Axis, temporary axis, circular edge, cylindrical face, straight edge, angular dimension | `Axis`, `AxisType`. |
| Count and spacing | Total instances; total angle with equal spacing, or angle between instances | `TotalInstances`, `Spacing`, `EqualSpacing`. |
| Reverse | | `ReverseDirection`. |
| Direction 2 | A second set of instances the other way round; symmetric or its own count and angle | `Direction2`, `Symmetric`, `TotalInstances2`, `Spacing2`, `EqualSpacing2`. |

**OpenForge today:** features about an axis, cylindrical face, circular edge or straight edge; count; total angle or angle between instances; reverse.
**Missing:** direction 2, faces and bodies as seeds, skip instances, geometry pattern, vary sketch, instances to vary.

## Mirror

Mirrors features, faces or bodies about a plane or planar face. API: `IMirrorPatternFeatureData` for features and faces; `IMirrorSolidFeatureData` for bodies.

| Option | Values | Notes |
| --- | --- | --- |
| Mirror plane | Plane or planar face | `Plane`, `MirrorPlaneType`. |
| What to mirror | Features, faces, bodies | `PatternFeatureArray`, `MirrorFaceArray`, `PatternBodyArray`. |
| Bodies options | Merge solids; knit surfaces | `Merge`, `KnitSurface`. Merge needs the mirrored body to touch the original. |
| Geometry pattern, propagate visual properties, feature scope | As shared | |

Mirroring a whole body about one of its own planar faces is the quick way to model symmetric parts.

**OpenForge today:** mirror features, or the whole body when no features are given, about a plane or planar face.
**Missing:** faces as seeds, separate bodies, merge and knit options, geometry pattern.

## Curve driven pattern

Places instances along a curve or edge chain. API: `ICurveDrivenPatternFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Direction 1 curve | Sketch, edge, or edge chain | `D1Direction`. |
| Count and spacing | Instances; equal spacing over the curve or a fixed spacing | `D1IsEqualSpaced`, `D1Spacing`. |
| Curve method | Transform curve (instances keep their distance to the curve origin) or offset curve (instances keep their distance to the curve) | `D1CurveMethod` (`swCurveDrivenPatternCurveMethod_e`). |
| Alignment | Tangent to curve or align to seed | `D1AlignmentMethod` (`swCurveDrivenPatternAlignment_e`). |
| Face normal | Face the 3D curve lies on, for orientation | `D1FaceNormal`. |
| Direction 2, pattern seed only | Second curve or direction | `D2*`, `Dir2Specified`. |

**OpenForge today:** not available.

## Sketch driven pattern

One instance at every point in a sketch. API: `ISketchPatternFeatureData` (`Sketch`, `ReferencePoint`, `UseCentroid`). The reference point, either the seed's centroid or a chosen point, is placed on each sketch point.

**OpenForge today:** not available. Holes already place one instance per sketch point, which is the main use.

## Table driven pattern

Instances at X–Y coordinates from a table, relative to a coordinate system. API: `ITablePatternFeatureData` (`CoordinateSystem`, `PointArray`, `ReferencePoint`, `UseCentroid`, `LoadPointsFromFile`, `SavePointsToFile` with `.sldptab` files; text files also import).

**OpenForge today:** not available.

## Fill pattern

Fills a boundary with instances in a layout, mainly for perforations and vents. API: `IFillPatternFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Fill boundary | Sketch on a face, or a planar face | `PatternFillBoundaryArray`. |
| Layout | Perforation, circular, square, polygon | `PatternLayoutType` (`swPatternLayoutType_e`). |
| Spacing | Instance spacing, loop spacing, stagger angle, margins, instances per loop or side, polygon sides | `InstanceSpacing`, `LoopSpacing`, `StaggerAngle`, `Margins`, `NoOfInstances`, `LayoutSpacingType`, `PatternLayoutPolygonSides`. |
| Pattern direction | Edge or line that orients the layout | `PatternDirection`. |
| Seed | An existing feature, or a generated cut: circle, square, diamond or polygon with size, rotation and flip | `FeaturesToPatternType`, `CreateSeedCutType`, `Diameter`, `Dimension`, `Diagonal`, `CreateSeedCutPolygonSides`, `InnerRadius`, `OuterRadius`, `Rotation`, `SeedCutFlipShapeDirection`. |
| Vertex or sketch point | Where the seed's center goes | `SeedFeatureCenter`, `SeedFeatureCenterType`. |

**OpenForge today:** not available.

## Variable pattern

A pattern table where each instance can have different dimension values from the seed. API: `IDimPatternFeatureData`. Columns are seed dimensions picked by the user, rows are instances. Rows can be suppressed, and the table can be exported to and imported from Excel.

**OpenForge today:** not available. The equations and configurations engine already resolves per-instance values, which is groundwork.

## Chain pattern (assemblies)

Patterns components along a path like chain links. API: `IChainPatternFeatureData`. Pitch methods: distance, distance linkage and connected linkage (`swChainPatternPitchMethod_e`); alignment to seed or tangent to curve; static copies or dynamic instances with mates; fill path. Covered further in [assemblies](13-assemblies.md).
