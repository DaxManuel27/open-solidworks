# 2. 3D sketching, curves and splines

Detailed specs for [catalog section 2](../solidworks-feature-catalog.md#2-3d-sketching-curves-and-splines). See the [README](README.md) for sources and conventions.

## 3D sketch

A sketch whose entities live in model space rather than on one plane. Used for sweep paths, guide curves, weldment frames and routing. API: `ISketch` with `Is3D`.

| Aspect | SOLIDWORKS behaviour |
| --- | --- |
| Entities | Lines, centerlines, splines, points, arcs and circles (on a sketch plane inside the 3D sketch), spline on surface, intersection curves, face curves |
| Space handle | While drawing a line, a triad shows the current axis; Tab cycles the drawing plane between XY, YZ and ZX |
| Planes inside the sketch | 3D sketch planes created from references, so 2D-style entities can be drawn on them |
| Relations | All the 2D relations plus along X, along Y, along Z, parallel YZ, parallel ZX, and their point-pair variants (`ALONGX3D`, `ALONGXPOINTS3D`, etc.); on-plane and on-surface |
| Dimensions | True 3D lengths and angles; also dimensions projected to a plane |
| Numeric input | Type X, Y and Z coordinates of points |

**OpenForge today:** points, lines, cubic Beziers, construction, dimensions and relations, DOF and conflict reporting, workplanes with depth locks, typed XYZ coordinates, handles, snapping, dimensions in the viewport.
Fixed and point-on-plane relations can follow a model vertex, reference point, plane or planar face and update on rebuild.
**Missing:** arcs and circles inside 3D sketches, spline on surface, along-axis and parallel-plane relations as named, other associative external geometry (edges, curves).

## Spline tools

Splines in 2D and 3D sketches share these controls. API: `ISketchSpline`, `ISplineHandle`.

| Tool | Options |
| --- | --- |
| Spline handles | Tangent direction and magnitude at each point; curvature control; relax |
| Control polygon | Edit the B-spline control vertices directly |
| Simplify spline | Reduces points to within a tolerance; smoothing |
| Fit spline | Fits one spline to several entities within a tolerance; closed, constrained or unconstrained |
| Style spline | Bezier-style, degree 3, 5 or 7, defined only by control vertices |
| Inflection points, minimum radius | Display aids |
| Curvature combs | Comb display with density and scale |
| Proportional | Keeps shape while dragging endpoints |
| Insert spline point, add tangency/curvature control | Editing |
| Spline on surface | Spline constrained to lie on a face |
| Equation-driven curve | Explicit y=f(x) between x1 and x2, or parametric x(t), y(t), z(t) |

**OpenForge today:** cubic Bezier and spline entities.
**Missing:** handle controls, control polygon editing, simplify, fit and style splines, curvature combs, spline on surface, equation-driven curves.

## Split line

Splits faces into several faces so features can use the new edges. API: `ISplitLineFeatureData`.

| Type | Inputs |
| --- | --- |
| Projection | A sketch projected onto faces; single direction or both; reverse |
| Silhouette | A pull direction (plane or face); splits along the outline seen from that direction; angle |
| Intersection | Split faces and bodies with planes, surfaces or bodies; split all or selected; surface split options (natural or linear) |

Options: `Faces`, `Contours`, `SplitTargets`, `SplitTools`, `SplitAll`, `PullDirectionBase`, `SingleDirection`, `ReverseDirection`, `Angle`.

**OpenForge today:** projection split of a curve onto a selected face, using kernel history; intersection split by a plane, face or body; silhouette split of cylindrical and spherical faces along a pull direction.
**Missing:** silhouettes of other surfaces, several faces at once, bidirectional projection.

## Projected curve

Projects a sketch onto faces, or two sketches onto each other to make a 3D curve. API: `IProjectionCurveFeatureData` (`Sketch`, `FaceArray`, `Bidirectional`, `Reverse`).

- **Sketch on faces:** projects along the sketch normal, or along a chosen direction, onto one or more faces.
- **Sketch on sketch:** two sketches on intersecting planes; the result is the intersection of their two extrusions.

**OpenForge today:** projection of a source curve onto a target along a direction (where a hole interrupts it, each piece is a selectable segment named in order along its source); sketch on sketch (where both sketches' extrusions meet).
**Missing:** bidirectional projection.

## Composite curve

Joins sketches, edges and curves end to end into one curve, typically for sweep paths and guide curves. API: `ICompositeCurveFeatureData` (`EntitiesToJoin`).

**OpenForge today:** composite curves from ordered, reversible segments.

## Curve through XYZ points

A spline through typed coordinates or points loaded from a `.sldcrv` or `.txt` file. API: `IFreePointCurveFeatureData` (`PointArray`, `LoadPointsFromFile`, `SavePointsToFile`). Options: closed curve, insert or delete rows, save the table.

## Curve through reference points

A spline through existing sketch points, vertices or reference points. API: `IReferencePointCurveFeatureData` (`ThroughPoints`, `ClosedCurve`).

## Helix and spiral

API: `IHelixFeatureData`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Base | A sketch with one circle; its diameter sets the starting diameter | |
| Defined by | Pitch and revolutions, height and revolutions, height and pitch, spiral | `DefinedBy` (`swHelixDefinedBy_e`). |
| Parameters | Pitch, revolutions, height | `Pitch`, `Revolution`, `Height`. |
| Constant or variable pitch | Variable uses a table of regions, each with pitch, revolutions, height and diameter | `VariablePitch`, `RegionParameterAtIndex`, `InsertRecord`, `AddLastRecord`, `DeleteRecord`. |
| Taper | Angle, outward | `Taper`, `TaperAngle`, `TaperOutward`. Constant pitch only. |
| Start angle | Degrees | `StartingAngle`. |
| Direction | Clockwise or counterclockwise; reverse (extends behind the sketch) | `Clockwise`, `ReverseDirection`. |
| Spiral | A flat spiral with pitch and revolutions | |

**OpenForge today:** helix on a plane with radius, start angle and handedness, defined by pitch and turns, height and turns, or height and pitch; variable pitch rows (pitch linear in turn count, exact).
**Missing:** variable diameter, taper, spiral, reverse direction, deriving the diameter from a sketch circle.
