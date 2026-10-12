# 7. Reference geometry

Detailed specs for [catalog section 7](../solidworks-feature-catalog.md#7-reference-geometry). See the [README](README.md) for sources and conventions.

## Default planes and origin

Every new part starts with Front, Top and Right planes and the origin. In SOLIDWORKS the default orientation is Y up: Top plane is the XZ plane, Front is XY, Right is YZ. The origin is a fixed point; sketch origins project from it.

**OpenForge today:** Top (XY), Front (XZ) and Right (YZ) planes and the origin; Z up.
**Gap to note:** OpenForge uses Z up while SOLIDWORKS is Y up. This matters for STEP exchange and for users' expectations of named views and planes.

## Reference plane

Since SOLIDWORKS 2010, planes are defined by up to three references, each with a constraint. API: `IRefPlaneFeatureData`, constraints in `swRefPlaneReferenceConstraints_e` (bitmask).

| Constraint | Reference types | Meaning |
| --- | --- | --- |
| Coincident | Plane, face, point, line | Plane passes through it |
| Parallel | Plane, face, line | Parallel to it |
| Perpendicular | Plane, face, line | Perpendicular to it |
| Distance (offset) | Plane or face | Offset by a distance; number of planes to create |
| Angle | Plane or face plus an edge or axis | Rotated about the line; number of planes to create |
| Tangent | Cylindrical, conical or curved face | Tangent to it |
| Mid plane | Two planes or faces | Halfway between |
| Project | Point onto a non-planar face | With "project to nearest location" or "along sketch normal" |
| Parallel to screen | View | Through a point, parallel to the screen |

Options: flip offset, flip normal (`OptionReferenceFlip`), origin on curve for normal-to-curve planes, plane at a point on a curve, auto-size to geometry, number of planes, solution index where several planes satisfy the constraints.

Older plane types (`swRefPlaneType_e`) still read from old files: through lines/points, parallel at point, at angle, offset distance, normal to curve, on surface.

**OpenForge today:** offset, angle, three points, mid plane, parallel at point, normal to curve.
**Missing:** a general constraint-based definition, tangent planes, projection onto surfaces, parallel to screen, number of planes, solution choice, auto-size.

## Reference axis

API: `IRefAxisFeatureData`, type in `swRefAxisType_e`.

| Type | References |
| --- | --- |
| One line, edge or axis | `swAxisTypeOneLine` |
| Two planes | Their intersection; `swAxisTypeTwoPlanes` |
| Two points or vertices | `swAxisTypeTwoPoints` |
| Cylindrical or conical face | Its axis; `swAxisTypeCylOrConeFace` |
| Point and face or plane | Through the point, normal to the face; `swAxisTypePtAndPlane` |

**OpenForge today:** all five types.

## Temporary axes

Every cylindrical and conical face has an implied axis, shown with View > Temporary Axes. They can be used anywhere an axis is accepted, without creating a feature.

**OpenForge today:** cylindrical faces are accepted directly as axes; there's no display toggle.

## Coordinate system

API: `ICoordinateSystemFeatureData`.

| Option | Values |
| --- | --- |
| Origin | Vertex, point, midpoint, or defined by numbers (2019 and later: position and rotation by values) |
| X, Y and Z axes | Edge, axis, line, or planar face normal; each can be flipped |

Used for mass properties, exports, measurement and table-driven patterns.

**OpenForge today:** origin reference with optional X and Z axis references.
**Missing:** a Y axis reference, axis flips, numeric position and rotation.

## Reference point

API: `IRefPointFeatureData`, type in `swRefPointType_e`.

| Type | Meaning |
| --- | --- |
| Arc center | Center of an arc or circular edge |
| Center of face | Area centroid of a face |
| Intersection | Of two edges, curves or sketch segments |
| Projection | Of a point onto a face or plane |
| On point | On a sketch point or vertex |
| Along curve distance | Points spaced along an edge by distance, percentage or evenly distributed, with count |
| XYZ coordinates | (2022 and later) |

**OpenForge today:** XYZ coordinates; on a vertex or sketch point; at the center of a face or circular edge.
**Missing:** intersection, projection and along-curve types.

## Center of mass point

Places a point at the center of mass that updates with the model. It can be used in references and measurements. Assemblies have their own center of mass point.

**OpenForge today:** center of mass is shown in mass properties, not as a referenceable point.

## Bounding box

The smallest box around the part, kept as a feature with properties (length, width, thickness, volume). API: `IBoundingBoxFeatureData`.

| Option | Values |
| --- | --- |
| Orientation | Best fit, or aligned to a reference face or plane (`ReferenceFaceOrPlane`, `PlanarEntity`) |
| Include | Hidden bodies, surfaces, envelope and hidden components (assemblies), reference planes and sketches (`swBoundingBoxOptions_e`) |

**OpenForge today:** bounding box size in the status bar and mass properties.
**Missing:** a bounding box feature with properties, best-fit orientation.

## Mate reference

Stores primary, secondary and tertiary entities and mate types in a part, so that dropping the part into an assembly snaps it into place. Covered in [assemblies](13-assemblies.md).

## Grid system

A building-style grid of named lines and levels for structure systems and weldments. Options: grid sketch, level count and spacing, bubble labels.

**OpenForge today:** not available.
