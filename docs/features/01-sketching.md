# 1. Sketching (2D)

Detailed specs for [catalog section 1](../solidworks-feature-catalog.md#1-sketching-2d). See the [README](README.md) for sources and conventions.

A 2D sketch lives on a plane or planar face. It holds entities (points, lines, arcs, splines and so on), relations that constrain them geometrically, and dimensions that set sizes. A solver moves the geometry to satisfy every relation and dimension. API: `ISketch`, `ISketchManager`, `ISketchSegment` (segment kinds in `swSketchSegments_e`: Line 0, Arc 1, Ellipse 2, Spline 3, Text 4, Parabola 5).

## Sketch status

Every sketch reports one status, shown by colour in SOLIDWORKS:

| Status | Meaning | Colour |
| --- | --- | --- |
| Under defined | Some geometry can still move | Blue |
| Fully defined | Nothing can move | Black |
| Over defined | Conflicting or redundant relations or dimensions | Red / yellow |
| Dangling | A relation or dimension refers to geometry that no longer exists | Brown |
| Not solved | The solver can't find a solution | Pink |

The feature tree prefixes under defined sketches with (-) and over defined ones with (+).

**OpenForge today:** degrees-of-freedom count, under, fully and over defined status, and conflict reporting.
**Missing:** dangling and not-solved states, and the feature-tree prefixes.

## Sketch entities

Each entity can be normal (forms profiles) or construction (reference only, drawn dashed). Each tool has its own creation modes, and every point on an entity can carry relations.

| Entity | Creation modes and options | Notes |
| --- | --- | --- |
| Line | Click-click or click-drag; as sketched, horizontal, vertical, angle; add dimension; infinite length | Midpoint line draws symmetric about the first click. |
| Centerline | Same as line, always construction | Used as the revolve axis and mirror line. |
| Infinite line | Construction line with no ends | |
| Rectangle | Corner, center, 3-point corner, 3-point center, parallelogram; optional construction diagonals | Created with horizontal/vertical or parallel/perpendicular relations so it stays a rectangle. |
| Slot | Straight, centerpoint straight, 3-point arc, centerpoint arc; center-to-center or overall length; add dimensions | `swSketchSlotCreationType_e`, `swSketchSlotLengthType_e`. A slot is one grouped entity with a fixed-slot relation. |
| Polygon | 3 to 40 sides; inscribed or circumscribed circle | Construction circle drives the size. |
| Circle | Center, perimeter (three points) | |
| Arc | Centerpoint, tangent, 3-point | Tangent arc direction follows the cursor; the line tool autotransitions to it. |
| Ellipse | Full ellipse, partial ellipse | Major and minor axis dimensions. |
| Parabola | Focus, apex, ends | |
| Conic | Endpoints, top vertex, rho value | Rho below 0.5 is elliptical, 0.5 parabolic, above 0.5 hyperbolic. |
| Spline | Through points; style spline (control vertices, degree 3, 5 or 7); fit spline (fits existing entities with a tolerance); equation-driven (explicit y=f(x) or parametric) | See [spline tools](02-3d-sketching-and-curves.md#spline-tools). |
| Point | Sketch point | Hole Wizard positions holes at points. |
| Sketch text | Text along curves or free; font, height, spacing, alignment, flip | Extrudable or usable as a cut profile. |
| Belt/chain | Wraps circles or arcs in an assembly layout sketch, with belt length and thickness | Drives a belt mate. |
| Sketch picture | Image on the sketch plane, with scale, rotation, transparency and auto-trace | For tracing only. |

**Rules**

- Entities whose endpoints coincide form contours. A closed contour bounds a region. Nested contours make holes.
- Construction geometry never forms a profile.
- SOLIDWORKS adds relations automatically while drawing (automatic relations), such as horizontal, vertical, coincident and tangent, when the cursor infers them.

**OpenForge today:** point, line, three-point arc, tangent arc, circle, cubic Bezier and spline, construction geometry, corner and center rectangles, polygon, straight slot.
**Missing:** centerline, infinite line, midpoint line, 3-point and parallelogram rectangles, other slot types, perimeter circle, centerpoint arc, ellipse, parabola, conic, style and fit splines, equation-driven curves, sketch text, belts, sketch pictures, polygon inscribed/circumscribed choice, automatic relations while drawing.

## Sketch tools

| Tool | Options | Notes |
| --- | --- | --- |
| Sketch fillet | Radius; keep constrained corners | Rounds a corner where two entities meet; trims both. Applies to several corners at once. |
| Sketch chamfer | Angle-distance or distance-distance; equal distance | |
| Offset entities | Distance; add dimensions; reverse; select chain; bidirectional; cap ends with arcs or lines; make base geometry construction | Picks a chain of entities or model edges. |
| Offset on surface | Distance along a 3D face; geodesic or Euclidean | 3D sketches only. |
| Convert entities | Model edges, loops, faces or other sketches; select chain; inner loops | Projects onto the sketch plane with an On Edge relation, so it updates with the model. |
| Intersection curve | Sketch curve where the sketch plane meets faces or bodies | |
| Silhouette entities | Outline of a body seen normal to the sketch plane | |
| Face curves | Iso-parametric curves on a face, by count or position | Creates a 3D sketch. |
| Trim | Power trim (drag across), corner, trim away inside, trim away outside, trim to closest | `swSketchTrimChoice_e`. Corner extends or trims two entities to meet. |
| Extend | Extends to the next intersection | |
| Split entities | Splits at a point; a closed entity needs two points | |
| Jog line | Adds a rectangular jog into a line | |
| Mirror entities | Mirror about a centerline, line, edge or plane; copy or not | Adds symmetric relations. Dynamic mirror mirrors as you draw. |
| Linear sketch pattern | Two directions, spacing, count, angle; instances to skip; add dimensions | |
| Circular sketch pattern | Center, count, total angle or spacing, radius; instances to skip | |
| Move, copy, rotate, scale, stretch | From-to or XY delta; keep relations; copy count | |
| Modify sketch | Moves, rotates or mirrors the whole sketch relative to its origin | |
| Replace entity | Swaps an entity while keeping the features that used it | |
| Make path | Chains entities so they select and drag as one | |
| Construction toggle | Switches selected entities between normal and construction | |
| Close sketch to model | Closes an open contour using model edges | |
| Repair sketch | Finds gaps, overlaps and tiny segments below a tolerance and fixes them | |
| Check sketch for feature | Validates a sketch for a chosen feature type | `swSketchCheckFeatureProfileUsage_e` lists 23 feature types; results in `swSketchCheckFeatureStatus_e`. |
| Shaded sketch contours | Fills closed regions; click a region to extrude it | |
| Blocks | Make, insert, edit, explode, nested; block-to-block relations; belts use blocks | Saved as `.sldblk`. |
| Derived sketch | Linked copy of a sketch that can move but not change shape | |
| Shared sketch | One sketch used by several features | |
| Sketch ink and touch | Pen input recognised as lines, arcs and shapes | |
| Numeric input | Type coordinates and sizes while drawing | |
| Autotransitioning | Line tool switches to tangent arc and back | |
| Snaps | Endpoint, midpoint, center, quadrant, intersection, nearest, tangent, perpendicular, parallel, horizontal/vertical, grid, angle | |
| Grid | Display, snap and spacing; align grid to an entity | |

**Check sketch failure messages** (from `swSketchCheckFeatureStatus_e`) give the rules every feature applies to its sketch: contours that intersect each other, disjoint contours where one was wanted, double nesting, zero-length entities, a shared endpoint used by three entities, mixed open and closed contours, a revolve contour crossing its centerline, and more. OpenForge should report the same conditions with the same specificity.

**OpenForge today:** trim, extend, offset, mirror, sketch fillet, convert entities (model edges, or every loop of a face, following the model on rebuild).
**Missing:** sketch chamfer, offset cap ends and bidirectional offset, select chain, intersection and silhouette curves, face curves, split, jog, sketch patterns, transforms, modify sketch, replace entity, make path, repair sketch, check sketch, blocks, derived and shared sketches, shaded contours, ink, snaps and grid.

## Dimensions

Dimensions drive geometry unless they're marked driven (reference). API: `IDisplayDimension`, `IDimension`.

| Type | Notes |
| --- | --- |
| Smart dimension | Picks linear, angular, radial or diameter from what's selected |
| Horizontal, vertical | Axis-aligned distance |
| Angular | Between two lines, or three points (`ANGLE3P`) |
| Radius, diameter | Arcs and circles; diameter dimension on a revolve profile |
| Arc length | Pick the arc and both endpoints |
| Arc conditions | Linear dimension to an arc's center, nearest or furthest point (`swArcEndCondition_e`) |
| Path length | Total length of a chain of entities |
| Ordinate, baseline, chain | Dimension schemes |
| Driven (reference) | Reports a value without driving geometry; shown in grey |

**Rules:** adding a dimension that over defines the sketch prompts to make it driven. Dimensions can be linked to equations and global variables, and lock dimensions prevent editing. Instant2D lets dimensions be dragged on screen.

**OpenForge today:** distance, horizontal and vertical distance, length, radius, diameter, angle; driving and driven toggle; dimensions placed in the viewport.
**Missing:** arc length, arc conditions, path length, three-point angle, ordinate, baseline and chain schemes, Instant2D, locked dimensions.

## Relations

Relation types from `swConstraintType_e` and the SOLIDWORKS help, with what each needs:

| Relation | Entities | Effect |
| --- | --- | --- |
| Horizontal, vertical | Line, or two points | Line along the sketch's X or Y; points aligned |
| Collinear | Two or more lines | On the same infinite line |
| Coradial | Two or more arcs or circles | Same center and radius |
| Perpendicular, parallel | Two lines | |
| Parallel YZ, parallel ZX, along Z | 3D sketch lines | See [3D sketching](02-3d-sketching-and-curves.md) |
| Tangent | Arc, ellipse or spline with a line, arc or spline | |
| Concentric | Arcs, circles, points | Same center |
| Midpoint | Point and line or arc | Point at the middle |
| Intersection | Two lines and a point | Point at their intersection |
| Coincident | Point and line, arc or ellipse | Point lies on the curve |
| Merge | Two points | One point |
| Equal | Lines, or arcs and circles | Equal length or radius |
| Equal curve length | Two curves | Equal arc length |
| Symmetric | Centerline and two points, lines, arcs or ellipses | Mirror images about the line |
| Fix | Any entity | Can't move |
| Pierce | Sketch point and an axis, edge, line or spline outside the sketch | Point lies where the curve passes through the sketch plane |
| On edge | Converted entity and its model edge | Follows the edge |
| Equal curvature, equal tangent | Spline and a curve | Curvature (G2) or tangent continuity |
| Torsion continuity | 3D splines | G3 match |
| Equidistant | Points on a line or arc | Equal spacing |
| Arc angle presets | Arc endpoints at 90°, 180°, 270°, top, bottom, left, right | `ARCANG*` |
| Fixed slot, block locks, belt traction | Special-purpose relations | |

**Rules:** each relation removes degrees of freedom. Display/Delete Relations lists all relations with filters for dangling, over defining, external, defined in context, locked, broken and selected (`swSketchRelationFilterType_e`). SOLIDWORKS suggests which relations to delete to fix an over defined sketch (SketchXpert).

**OpenForge today:** coincident, point on curve, midpoint, horizontal, vertical, horizontal and vertical points, parallel, perpendicular, collinear, equal, tangent, concentric, symmetric, fix.
**Missing:** coradial, intersection, merge, equal curve length, pierce, on edge, curvature and torsion continuity, equidistant, arc angle presets, relation filters, SketchXpert.

## Rapid Sketch

Starts a sketch by hovering over a face and picking a sketch tool, with no separate plane step.

**OpenForge today:** clicking a face sets the sketch plane.
