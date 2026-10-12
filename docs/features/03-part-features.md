# 3. Part features

Detailed specs for the SOLIDWORKS part features in [catalog section 3](../solidworks-feature-catalog.md#3-part-features). See the [README](README.md) for sources and how to read these pages.

## Extruded boss/base and extruded cut

Sweeps a closed sketch profile in a straight line to add material (boss) or remove it (cut). This is the most used feature in SOLIDWORKS. API: `IExtrudeFeatureData2`.

**Inputs**

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Sketch | One sketch, or selected contours inside it | `Contours`. With no contours picked, every closed region is used. |
| From | Sketch plane, surface/face/plane, vertex, offset | `FromType` (`swExtrudeFrom_e`: SketchPlane 0, SurfaceFacePlane 1, Vertex 2, Offset 3). Offset has its own distance and reverse flag (`FromOffsetDistance`, `FromOffsetReverse`). |
| Direction 1 end condition | Blind, Through All, Through All–Both, Up To Next, Up To Vertex, Up To Surface, Offset From Surface, Up To Body, Mid Plane | `swEndConditions_e`: Blind 0, ThroughAll 1, ThroughNext 2, UpToVertex 3, UpToSurface 4, OffsetFromSurface 5, MidPlane 6, UpToBody 7, UpToSelection 10, UpToNext 11, ThroughAllBoth 9. Vertex and Surface are now both stored as Up To Selection. |
| Depth | Length | Blind and Mid Plane only (`SetDepth`). Mid Plane splits it equally either side. |
| End condition reference | Vertex, face, surface, plane or body | `SetEndConditionReference`. Required for Up To Vertex/Surface/Body and Offset From Surface. |
| Offset from surface | Length, reverse offset, translate surface | `SetReverseOffset`, `SetTranslateSurface`. Translate keeps the end face's shape and moves it; without it the end face is a true offset of the target. |
| Direction of extrusion | Sketch normal, or any edge, line, axis or plane normal | `SetDirectionReference`. Lets an extrude run at an angle to its sketch. |
| Reverse direction | On/off | `ReverseDirection`. |
| Draft while extruding | Angle, draft outward | `SetDraftWhileExtruding`, `SetDraftAngle`, `SetDraftOutward`, set per direction. |
| Direction 2 | Its own end condition, depth, draft and reference | `BothDirections`. Not offered with Mid Plane. |
| Thin feature | One-direction, opposite direction, mid-plane, two-direction; thickness per side | `ThinWallType` (`swThinWallType_e`: OneDirection 0, OppDirection 1, MidPlane 2, TwoDirection 3), `SetWallThickness`. Works with open or closed sketches. |
| Thin options | Cap ends with cap thickness (closed sketch, boss only); auto-fillet corners with radius | `CapEnds`, `CapThickness`, `SetAutoFillet`. |
| Flip side to cut | On/off (cuts only) | `FlipSideToCut`. Removes everything outside the profile instead. |
| Merge result | On/off (boss only) | `Merge`. Off keeps the new material as a separate body. |
| Feature scope | All bodies (auto-select) or picked bodies | `FeatureScope`, `AutoSelect`, `FeatureScopeBodies`. |
| Link to thickness | On/off (sheet metal only) | `LinkToThickness`: depth follows the sheet thickness. |
| Normal cut | On/off, optimize geometry (sheet metal cuts only) | `NormalCut`, `OptimizeGeometry`. |

**Rules and interactions**

- The first feature in a part must be a boss and creates the base body.
- A boss needs a closed profile unless it is a thin feature. A cut works from a closed profile, or from an open one as a thin cut.
- Nested closed contours make islands and holes: an outer loop with inner loops extrudes as a solid with through-holes.
- Through All, Up To Next and Up To Body need existing bodies in the extrude direction.
- Up To Next stops at the first face that fully blocks the profile. If the profile is only partly covered, it carries on to the next face that blocks it completely.
- Up To Surface accepts curved faces, and the end of the extrude takes their shape.
- With Draft while extruding, the profile is the size at the sketch plane and grows or shrinks along the depth.
- In an assembly, extruded cuts are assembly features. They have assembly scope (`AssemblyFeatureScope`, `AutoSelectComponents`) and can optionally carry through to the parts (`PropagateFeatureToParts`).

**Failure cases SOLIDWORKS reports**

- Open or self-intersecting profile without Thin selected.
- The end condition target doesn't intersect the extrude path, or a vertex lies behind the sketch plane.
- A draft angle so large that the walls meet before the depth is reached.
- The cut doesn't intersect any body ("the cut does not remove material").
- A boss would create zero thickness geometry, for example touching only along an edge.

**OpenForge today:** sketch-plane start or offset; Blind, Through All, Up To Next, Up To Vertex, Up To Surface, Offset From Surface and Mid Plane; Direction 2; Thin (one-direction, mid-plane, two-direction); flip side to cut; merge; feature scope.
**Missing:** Through All–Both, Up To Body, extrude from face or vertex, custom extrude direction, draft while extruding, thin opposite-direction, cap ends and auto-fillet corners, translate surface, selected contours, sheet metal options, assembly cuts.

## Revolved boss/base and revolved cut

Spins a sketch profile around an axis. API: `IRevolveFeatureData2`.

**Inputs**

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Profile | Sketch or selected contours | `Contours`. The profile must lie fully on one side of the axis. |
| Axis | Sketch line or centerline, reference axis, straight edge, temporary axis | `Axis`. If the sketch has a single centerline, SOLIDWORKS picks it. |
| Type | One direction, mid plane, two direction, each also with a 360° shortcut | `Type` (`swRevolveType_e`: OneDirection 0, MidPlane 1, TwoDirection 2, plus 360° variants 3–5). |
| End condition per direction | Blind (angle), Up To Vertex, Up To Surface, Offset From Surface, Mid Plane | Uses `swEndConditions_e`. `RevolutionAngle` holds the angle per direction. |
| Reverse direction | On/off | `ReverseDirection`. |
| Thin feature | Same four thin types as extrude, thickness per side | `ThinWallType`, `WallThickness`. Open profiles must be thin. |
| Merge result, feature scope | As for extrude | `Merge`, `FeatureScope`, `FeatureScopeBodies`. |

**Rules and interactions**

- A profile that crosses the axis is rejected because the result would intersect itself. A profile edge lying on the axis is allowed.
- A full 360° closed revolve has no cap faces. Anything less leaves two planar caps.
- A revolved cut follows the same rules and removes the swept volume.

**Failure cases:** profile crosses the axis; zero or 360°+ angle for two-direction totals; up-to target not reached by the sweep.

**OpenForge today:** sketch-line, reference-axis or straight-edge axis; Blind, Mid Plane and two-direction angles; Up To Vertex/Surface and Offset From Surface; reverse; thin; merge; cut.
**Missing:** selected contours and temporary axes as the axis.

## Swept boss/base and swept cut

Moves a profile along a path. API: `ISweepFeatureData`.

**Inputs**

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Profile type | Sketch profile, circular profile (diameter only), solid tool body (cuts only) | `ProfileType`, `CircularProfile`, `CircularProfileDiameter`; cut kind in `CutSweepOption` (`swProfileSweep` 1, `swSolidSweep` 2). |
| Path | Open or closed sketch, 3D sketch, curve, edges, helix | `Path`. Must be one continuous chain. |
| Direction | Direction 1, both directions, direction 2 | `Direction` (`swSweepDirection_e`), for profiles that sit in the middle of the path. |
| Profile orientation and twist | Follow path, keep normal constant, follow path and first guide curve, follow first and second guide curves, twist along path (angle, turns or radians), twist along path with normal constant, minimum twist | `TwistControlType` (`swTwistControlType_e`), `TwistAngle`, `D1ReverseTwistDir`, `D2ReverseTwistDir`. |
| Path alignment (profile twist) | None, minimum twist, direction vector, all faces | `PathAlignmentType` with `swTangencyType_e` values (None 0, NormalToProfile 1, DirectionVector 2, AllFaces 3, MinimumTwist 10). |
| Guide curves | Any number of curves the profile must touch; merge smooth faces | `GuideCurves`, `MergeSmoothFaces`. The profile needs a pierce relation to each guide. |
| Start/end tangency | None or normal to profile | `StartTangencyType`, `EndTangencyType`. |
| Merge tangent faces | On/off | `MaintainTangency`. |
| Tangent propagation | On/off | `TangentPropagation`: carry the path to tangent edges. |
| Align with end faces | On/off (cuts) | `AlignWithEndFaces`: extends the cut so it meets the last faces it crosses. |
| Thin feature | Thin types, thickness | `ThinFeature`, `ThinWallType`, `WallThickness`. |
| Merge result, feature scope | As for extrude | |
| Advanced smoothing | On/off | `AdvancedSmoothing`. |

**Rules and interactions**

- The profile must be a closed sketch for a solid boss, unless thin is on.
- The profile's start point usually sits on the path. In 2D the profile plane is normally perpendicular to the path at its start.
- A solid-body sweep cut moves a revolved tool body along the path to simulate a cutter, for example a ball end mill.
- Guide curves change the profile's size along the path. With two guides, the profile can also scale non-uniformly.
- The sweep fails if the profile intersects itself along the path, which happens when the path's radius of curvature is smaller than the profile's size.

**OpenForge today:** explicit closed outer and inner contours or a circular profile along sketch, spatial, edge and construction-curve paths; minimum-twist, Frenet and fixed orientation; twist along path; follow path and a guide curve that turns the section, optionally scaling it to keep the guide on it, or a second guide that stretches it; thin profiles, open or closed; mitred sharp path corners; self-intersection rejected; boss/cut; scope.
**Missing:** guide curves that scale the profile, more than one guide, bidirectional sweeps, solid-body sweep cuts, end tangency and align with end faces.

## Lofted boss/base and lofted cut

Blends a shape through two or more profiles. API: `ILoftFeatureData`.

**Inputs**

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Profiles | Ordered sketches, faces, edges or points (first or last only) | `Profiles`. The pick location sets the matching start points (`PickPoints`). |
| Start and end constraints | None, direction vector, normal to profile, tangency to face, curvature to face | `StartTangencyType`, `EndTangencyType`, `StartTangentLength`, `EndTangentLength`, `StartDirectionVector`, `EndDirectionVector`, draft angle and direction, apply to all. |
| Guide curves | Curves the loft follows; influence to next guide, next sharp, next edge, or global | `GuideCurves`, `GuideCurveInfluence` (`swGuideCurveInfluence_e`), `GuideTangencyType`. |
| Centerline | A curve that sets the loft's direction | `Centerline`, `NumberOfSections` for intermediate sections. |
| Connectors | Handles that match vertices between profiles | Edited in the graphics area to control twist. |
| Close loft | On/off | `Close`: joins the last profile back to the first. |
| Merge tangent faces | On/off | `MaintainTangency`. |
| Thin feature | Thin types, thickness | `ThinWallType`, `WallThickness`. |
| Merge result, feature scope, advanced smoothing | As for sweep | |

**Rules and interactions**

- Profiles must all be closed for a solid boss unless thin is on.
- A point may only be the first or last profile, and makes the loft come to a tip.
- Matching vertex counts give the cleanest result. With different counts, SOLIDWORKS splits edges and the user adjusts the connectors.
- A loft with a centerline uses it as a path, rather like a sweep with blended profiles.

**Failure cases:** profiles that cross each other, wrong profile order, twisting that intersects itself, a point used as a middle profile.

**OpenForge today:** ordered closed contours, reversal, start-entity matching and seams inside an entity, ruled or smooth blends, two or more guide curves (the loft passes exactly through them), closed lofts through three or more sections, a centerline the sections blend along, start/end constraints (normal to the profile or along a direction, with tangent length), boss/cut, scope.
**Missing:** inner holes in profiles, a single guide, centerline, end constraints, connectors, thin, point profiles.

## Boundary boss/base and boundary cut

A loft-like blend driven by curves in two directions, giving better continuity at the edges. API: `IBoundaryBossFeatureData`.

**Inputs**

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Direction 1 curves | Ordered sketches, edges or curves | `D1Curves`. |
| Direction 2 curves | Optional cross curves | `D2Curves`. |
| Tangency per curve | None, default, direction vector, normal to profile, tangency to face, curvature to face | `GuideTangencyType` (`swBoundaryBossTangencyType_e`), `TangentLength`, `DraftAngle`, `TangentApplyToAll`. |
| Curve influence | To next curve, to next sharp, to edge, global, linear | `D1CurveInfluence`, `D2CurveInfluence` (`swBoundaryBossCurveInfluenceType_e`). |
| Alignment | Section normal, next section, other geometry, iso-parameter | `AlignmentType` (`swBoundaryBossAlignment_e`). |
| Trim by direction 1 | On/off | `TrimByD1`, for direction 2 curves that don't form a closed boundary. |
| Merge tangent faces, merge result, thin, feature scope | | `MergeTangentFaces`, `MergeResult`, `ThinFeatureType`, `ThinFeatureThickness`. |

**Rules:** curves in both directions must touch each other. Boundary is preferred over loft when the result has to meet adjacent faces smoothly.

**OpenForge today:** two or more closed direction 1 sections (planar ends) and two or more direction 2 curves crossing every section in order; four-sided Coons patches that interpolate both curve families (ruled cells where the direction 2 curves are straight); boss/cut; scope. Network order and contact are validated; self-intersection rejected.
**Missing:** direction 1 only, open profiles, tangency and curvature end conditions, influence and trim options, thin.

## Fillets

Rounds edges. Constant size, face and full round fillets share one interface: `ISimpleFilletFeatureData2` (type in `swSimpleFilletType_e`: ConstRadius 0, Face 2, FullRound 3). Variable fillets use `IVariableFilletFeatureData2`.

### Shared options

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Profile | Circular, conic rho (0.5–0.95), conic radius, curvature continuous | `ConicTypeForCrossSectionProfile` (`swFeatureFilletProfileType_e`), `ConicRhoOrRadius`, `CurvatureContinuous`. Curvature continuous overrides the other profiles. |
| Size type | Symmetric radius, asymmetric (two distances), chord length | `AsymmetricFillet`, `DefaultRadius`, `DefaultDistance`. An asymmetric circular fillet becomes elliptical. |
| Tangent propagation | On/off | `PropagateToTangentFaces`: extends the fillet along tangent edges. |
| Overflow | Default, keep edge, keep surface | `OverflowType` (`swFilletOverFlowType_e`). Controls what happens when a fillet runs past an adjacent face. |
| Keep features | On/off | `KeepFeatures`: whether a large fillet swallows small bosses and cuts or keeps them. |
| Round corners | On/off | `RoundCorners`: rounds the corner where fillets meet at an angle. |
| Setback | Setback distance per vertex | `SetbackVertices`, `SetbackVertexDistance`. Shapes the blend where three or more fillets meet. |
| Feature scope (assembly) | Propagate to parts | `PropagateFeatureToParts`. |

### Constant size fillet

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Items | Edges, faces (fillets all their edges), features (fillets all their edges), loops | `Edges`, `Faces`, `Features`, `Loops`. |
| Multiple radius | Each item can have its own radius | `MultipleRadius`, `FilletItemAtIndex`, `Radius`. |
| Partial edge | Start and end by distance, percentage or reference | `EnablePartialEdgeParameters`, `PartialEdgeFilletData` (`swSimpleFilletPartialEdgeCondition_e`). |
| Attach edges | Fillet the edges where two features meet | `swFeatureFilletAttachEdges`; `OmitAttachedEdges`. |

**Rules:** the radius must fit, so it can't exceed the length available on the adjacent faces. Fillets on tangent chains require propagation. Fillet order matters: SOLIDWORKS recommends larger fillets first and drafts before fillets. FilletXpert can reorder and repair them.

**Failure cases:** radius too large for the adjacent faces; fillet runs into another fillet with incompatible geometry; three or more edges meeting at a vertex that the fillet can't blend.

### Variable size fillet

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Edges | One or more edges | `FilletEdgeAtIndex`, `FilletEdgeCount`. |
| Radius at each vertex | Distance 1 and, if asymmetric, Distance 2 | `Radius`, `Radius2`. |
| Control points | Intermediate points by percentage along the edge, each with a radius | `ControlPointsCount`, `ControlPointRadiusAtIndex`, `ControlPointDistanceAtIndex`. |
| Transition | Smooth or straight between radii | `TransitionType` (`swVariableRadiusFilletOptions_e`: Smooth 0, Straight 1). |
| Profile, asymmetric, overflow, propagation, setback | As shared | |

**OpenForge today (variable):** the tangent chain through one edge, with ordered radius stations by fraction along the chain measured in the selected edge's direction; reversible; smooth or straight (linear) transition.
**Missing:** several separate edges, asymmetric sizes, profiles other than circular, setbacks.

### Face fillet

Blends two face sets that need not share an edge.

| Option | Values | Notes |
| --- | --- | --- |
| Face set 1 and 2 | Faces, with direction flips | `swFeatureFilletReverseFace1Dir`, `swFeatureFilletReverseFace2Dir`, `ReverseFaceNormal`. |
| Radius, or constant width, or hold lines | One size option | `DefaultRadius`, `ConstantWidth`, `HoldLines` (curves the fillet edge must follow). |
| Help point | Picks which of several valid placements to use | `HelpPoint`. |
| Trim and attach | For surface bodies: trim the faces and knit | `TrimAndAttachSurfaces`. |

### Full round fillet

Rounds across three neighbouring faces: two side faces and a centre face that disappears. Needs side face set 1, centre face set and side face set 2. The radius comes from the geometry, so there's no size input.

**OpenForge today (fillets):** constant radius on picked edges and on all edges of picked faces; variable fillet as above; face fillets between two sets of planar faces at any angle, adjacent or not, convex or concave, tangent to both planes.
**Missing:** multiple radii, conic and curvature-continuous profiles, asymmetric sizes, partial edges, setbacks, overflow and keep-features control, tangent propagation control, face fillets on curved faces, hold lines, full round fillets, FilletXpert.

## Chamfer

Bevels edges or vertices. API: `IChamferFeatureData2`; type in `swChamferType_e` (AngleDistance 1, DistanceDistance 2, Vertex 3, EqualDistance 16). SOLIDWORKS 2017 and later also offer offset-face and face-face chamfers, which share the fillet interface (`ISimpleFilletFeatureData2` with a chamfer profile, `swFeatureFilletConicRhoZeroChamfer`).

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Type | Angle-distance, distance-distance, vertex, offset face, face-face | `Type`. |
| Items | Edges, faces, loops; a vertex for vertex chamfers | `Edges`, `Faces`, `Loops`, `Vertex`. |
| Sizes | Distance and angle; two distances; three vertex distances; equal distance | `EdgeChamferDistance`, `EdgeChamferAngle`, `VertexChamferDistance`, `EqualDistance`. |
| Flip direction | Swaps which face the distance is on | `IsFlipped`. |
| Tangent propagation | On/off | `TangentPropagation`. |
| Keep features | On/off | `KeepFeatures`. |
| Partial edge | As for fillets (offset-face chamfers) | |

**Rules:** offset-face chamfers measure the distance along the faces' offsets rather than along the edge, which gives a consistent result on non-90° corners. Face-face chamfers bevel between two face sets that need not share an edge.

**OpenForge today:** equal distance, distance-distance and angle-distance on picked edges and faces, with flip.
**Missing:** vertex chamfers, offset-face and face-face chamfers, partial edges, tangent propagation and keep-features control.

## Holes

### Simple hole

A circular cut placed on a planar face. API: `ISimpleHoleFeatureData2`.

| Option | Values | Notes |
| --- | --- | --- |
| Placement face and point | Planar face; the point is then dimensioned in a sketch | |
| Diameter | Length | `Diameter`. |
| End condition | Blind, Through All, Up To Next, Up To Vertex, Up To Surface, Offset From Surface | `Type`, `Depth`, `Face`, `Vertex`, `SurfaceOffset`, `ReverseOffset`, `TranslateSurface`. |
| Draft | Angle, outward | `DraftWhileExtruding`, `DraftAngle`, `DraftOutward`. |
| Reverse, feature scope | | |

SOLIDWORKS treats Simple Hole as legacy and steers users to Hole Wizard.

### Hole Wizard

Standard holes sized from built-in tables for ISO, ANSI and other standards. API: `IWizardHoleFeatureData2`.

**Hole types** (`swWzdGeneralHoleTypes_e`): counterbore, countersink, hole (straight drilled), straight tap, tapered (pipe) tap, legacy, and counterbore, countersink and straight slots. `swWzdHoleTypes_e` lists 70+ detailed variants, such as counterbore through with near-side countersink.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Standard | ANSI Inch, ANSI Metric, AS, BSI, DIN, DME, GB, Hasco, Helicoil inch and metric, IS, ISO, JIS, KS, PCS, PEM inch and metric, Progressive, Superior | `Standard2` (`swWzdHoleStandards_e`). |
| Fastener type | Depends on hole type and standard, such as socket head cap screw or hex bolt for counterbores; flat head 82° or 100° for countersinks; drill sizes for holes | `FastenerType2` (`swWzdHoleStandardFastenerTypes_e`). |
| Size | From the table, such as M6 or 1/4-20 | `FastenerSize`. |
| Fit | Close, normal, loose | `HoleFit` (`swWzdHoleScrewClearanceTypes_e`). |
| Custom sizing | Override any table value | `HoleDiameter`, `CounterBoreDiameter`, `CounterBoreDepth`, `CounterSinkDiameter`, `CounterSinkAngle`, `DrillAngle`, etc. |
| End condition | Blind, Through All, Up To Next, Up To Vertex, Up To Surface, Offset From Surface | `EndCondition`, `Depth`, `EndConditionReference`, `OffsetDistance`. Drill-point angle applies to blind holes. |
| Thread (tapped holes) | Thread depth, thread class, cosmetic thread with or without callout, modeled thread, tap drill diameter, thread end condition (blind, through all, through next) | `ThreadDepth`, `ThreadClass`, `CosmeticThreadType` (`swWzdHoleCosmeticThreadTypes_e`), `TapDrillDiameter`, `ThreadEndCondition` (`swWzdHoleThreadEndCondition_e`). |
| Options | Near-side, under-head and far-side countersinks; head clearance; slot length | `NearSideCounterSink`, `UnderHeadCounterSink`, `FarSideCounterSink`, `HeadClearance`, `HeadClearanceType`, `Length`. |
| Positions | Points in a placement sketch on a face or plane; one hole per point | `SketchPoints`. Placing on a curved face gives a 3D sketch. |
| Favorites | Saved combinations of the above | |

**Rules:** each hole position is a sketch point; dimensioning and relations position them. Hole Wizard holes carry their callout data into drawings and hole tables. Changing the standard re-reads all table-driven values (`ChangeStandard`).

### Advanced hole

Stacks hole elements from the near side and far side into one hole, such as counterbore, then straight, then tap. API: `IAdvancedHoleFeatureData`.

- Element types (`swAdvWzdGeneralHoleTypes_e`): counterbore, countersink, straight, straight tap, tapered tap.
- Each element has its own standard, size, fit and depth, and belongs to the near or far side (`swHoleElementOrientation_e`).
- Options: baseline dimensions, customized callouts.

**OpenForge today (holes):** simple, counterbore, countersink and tapped holes at sketch points; ISO and ANSI tables; close, normal and loose fit; blind or through all; reverse; table overrides; tapped-hole cosmetic threads (nominal diameter, ISO 261 coarse or ANSI TPI pitch, length through the material).
**Missing:** other standards and fastener types, slots, pipe taps, legacy holes, near-side, under-head and far-side countersinks, Up To and offset end conditions, drill-point angle, thread class, placement on curved faces, favorites, Advanced Hole, hole series.

### Hole series (assemblies)

Creates matching holes through several parts of an assembly in one step. The start part gets a clearance hole, middle parts get clearance holes, and the end part gets a tapped or matching hole. API: `IHoleSeriesFeatureData2`. It belongs to assemblies, so the [assemblies spec](13-assemblies.md) covers it.

## Threads

### Thread (modeled)

Cuts or extrudes a real helical thread on a cylinder. API: `IThreadFeatureData`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Start | Circular edge of a cylinder, with optional offset | `Edge`, `StartEntity`, `Offset`, `OffsetDistance`, `ReverseOffset`. |
| End condition | Blind, revolutions, up to selection; maintain length; optional offset | `EndCondition` (`swThreadEndCondition_e`), `BlindDepth`, `Revolutions`, `MaintainThreadLength`, `EndConditionOffset`. |
| Profile type and size | Inch die, inch tap, metric die, metric tap, SP4xx bottle profiles; size from the library | `Type`, `Size`. Profiles are library sketches the user can edit. |
| Diameter and pitch override | Optional | `DiameterOverride`, `Diameter`, `PitchOverride`, `Pitch`. |
| Method | Cut thread or extrude thread | `ThreadMethod` (`swThreadMethod_e`). |
| Handedness | Right or left | `RightHanded`. |
| Mirror profile | Horizontally or vertically | `MirrorProfile`, `MirrorType`. |
| Rotation and start angle | Angles | `RotationAngle`, `ThreadStartAngle`. |
| Multiple starts | Number of starts | `MultipleStart`, `NumberOfStarts`. |
| Trim with start and end faces | On/off | `TrimStartFace`, `TrimEndFace`. |

### Cosmetic thread

A thread stored as data and shown as a texture or drawing lines, without changing geometry. API: `ICosmeticThreadFeatureData`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Circular edge | The edge where the thread starts | `Edge`. |
| Start face or plane | Optional | `StartFromFacePlane`. |
| Standard, type, size | None (custom), ANSI inch, ANSI metric, ISO, etc. | `Standard`, `StandardType`, `Size`. |
| Diameter | Minor for bosses, major for cuts and holes, conical offset for tapered holes | `Diameter`, `DiameterType` (`swCosmeticThreadDiameterType_e`). |
| End condition | Blind, up to next, through | `EndCondition` (`swCosmeticThreadType_e`), `BlindDepth`. |
| Callout | Text shown in drawings | `ThreadCallout`. |
| Configurations | This configuration, all, or specified | `ConfigurationOption`, `ConfigurationNames`. |

**OpenForge today (threads):** cosmetic threads on complete cylinders with explicit dimensions, handedness and start, shown in the viewport, editable and configurable; modeled threads with the same parameters cut with the ISO 68-1 basic 60° profile, internal or external, either hand, trimmed flat at both ends (up to 200 turns); tapped-hole cosmetic threads.
**Missing:** cosmetic threads on interrupted cylinders, thread standards other than the basic metric profile, thread callouts, runout and chamfered ends.

### Stud Wizard

Adds a threaded stud, either as a new body on a face or by threading an existing cylinder. Options are the standard, type, size, length, thread length, cosmetic thread and undercut. It shares the Hole Wizard standards database.

## Shell

Hollows a solid, leaving walls of a set thickness, with chosen faces removed. API: `IShellFeatureData`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Faces to remove | Zero or more faces | `FacesRemoved`. With none, the part becomes a closed hollow solid. |
| Thickness | Length | `Thickness`. |
| Shell outward | On/off | `Direction`. Outward grows the part rather than hollowing it. |
| Multi-thickness faces | Faces with their own thickness | `MultipleThicknessFaces`, `MultipleThicknessAtIndex`. |
| Show preview | UI only | |

**Rules:** thickness must be smaller than the smallest local radius of curvature on the inward side, or the offset faces intersect themselves. SOLIDWORKS's error diagnostics show the faces whose curvature is too tight.

**Failure cases:** a fillet radius smaller than the shell thickness on the inside; faces that can't be offset; removing faces that would leave a wall disconnected.

**OpenForge today:** any number of removed faces or none; uniform thickness; outward.
**Missing:** multi-thickness faces and the failure diagnostics.

## Draft

Tapers faces at an angle, for molding. API: `IDraftFeatureData2`; type in `swDraftType_e` (NeutralPlane 0, PartingLine 1, StepDraft 3). DraftXpert offers the same in a guided UI.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Type | Neutral plane, parting line, step draft (tapered or perpendicular steps) | `Type`, `StepType`. |
| Angle | Degrees | `Angle`. Allow reduced angle lets SOLIDWORKS lower it where the full angle won't fit (`AllowReducedAngle`). |
| Neutral plane | Plane or face; draft is measured from it | `NeutralPlane`. |
| Pull direction | For parting line and step drafts | `DirectionPull`. |
| Faces to draft | Faces | `FacesToDraft`. |
| Parting lines | Edges, with "other face" choice per segment | `PartingLines`, `OtherFacesFlagAtIndex`. |
| Face propagation | None, along tangent, all faces, inner faces, outer faces | `FacePropagation` (`swDraftFacePropagationType_e`). |
| Reverse direction | On/off | `ReverseDirection`. |

**OpenForge today:** neutral-plane draft on picked faces, with pull direction and signed angle; parting-line draft on straight or circular parting edges, each face hinging about its own edge, either side; step draft with a level hinge and perpendicular step faces on planar faces.
**Missing:** step drafts, face propagation, allow reduced angle, DraftXpert.

## Rib

Turns an open sketch into a thin wall that runs out to the existing geometry. API: `IRibFeatureData2`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Sketch | Open or closed sketch contours | |
| Thickness | Length; first side, both sides or second side | `Thickness`, `TwoSided`, `ReverseThicknessDir`. |
| Extrusion direction | Parallel to sketch or normal to sketch | `ExtrusionDirection` (`swRibExtrusionDirection_e`). |
| Flip material side | On/off | `FlipSide`. |
| Draft | On/off, angle, outward, from wall or from sketch plane | `EnableDraft`, `DraftAngle`, `DraftOutward`, `DraftFromWall`, `RefSketchIndex`. |
| Type (open sketch, normal direction) | Linear or natural extension | `Type` (`swRibType_e`). Natural follows the curvature of the sketch ends. |
| Body | Target body in multibody parts | `Body`. |

**Rules:** the sketch is extended until it meets the body. If it can't reach a boundary on every side, the rib fails.

**OpenForge today:** open sketch chains of lines and arcs, extended along their end tangents until they meet material; parallel or normal to the sketch; thickness centred or to either side; flip; draft that thickens the web away from the sketch; missing-support and zero-thickness errors.
**Missing:** closed rib sketches, natural (curved) extension, draft from the wall interface, several rib sketches in one feature as separate options.

## Dome

Raises or sinks a dome on a face. API: `IDomeFeatureData2`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Faces | One or more planar or non-planar faces | `Faces`. |
| Height (distance) | Length | `Height`. |
| Reverse | Concave instead of convex | `ReverseDir`. |
| Elliptical dome | Half ellipsoid with height equal to one radius | `Elliptical`. |
| Constraint point or sketch | Point the dome must pass through, or a sketch that shapes it | `ConstraintPointOrSketch`. |
| Direction | Edge or face defining the push direction | `Direction`. |
| Continuous dome | For polygonal faces: slopes up evenly on all sides | UI option. |

**OpenForge today:** an exact spherical cap on a complete planar circular disk (height up to the disk radius), or an elliptical dome (half an ellipsoid of revolution, any height, tangent to the face normal at the rim); on other planar faces without holes, a smooth dome through the boundary and the apex, optionally tangent to the side faces when the boundary has no corners; inward or outward; scope.
**Missing:** non-circular and non-planar faces, faces with holes, constraint point or sketch, direction, continuous dome.

## Wrap

Wraps a sketch onto a face as a raised area, an indented area or split lines. API: `IWrapSketchFeatureData`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Type | Emboss, deboss, scribe | `Type` (`swWrapSketchType_e`: Emboss 0, Engrave/Deboss 1, Scribe 2). |
| Method | Analytical (cylinders, cones, planes) or spline surface (any face) | `swWrapMethods_e`. |
| Source sketch | A sketch on a plane tangent to or near the face | `SourceSketch`. |
| Face | Target face | `Face`. |
| Thickness | For emboss and deboss | `Thickness`. |
| Pull direction, reverse | Optional direction for the projection | `PullDirection`, `ReverseDirection`. |

**OpenForge today:** closed sketch regions wrapped by true length onto cylinders (sketch parallel to the axis, exact), cones (sketch parallel to a tangent plane, exact development about the apex) and other curved faces (along geodesics from the point under the sketch origin, rejected above 1% length stretch), across seams; emboss, deboss and scribe; depth; holes in regions.
**Missing:** conical and freeform faces, the spline-surface method, pull direction, several faces.

## Indent

Uses a tool body to press a shaped pocket into a target body with clearance and thickness, like a molded recess around a component. API: `IIndentFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Target body | Solid or surface body | `TargetBody`. |
| Tool body region | Picked region of the tool, keep or remove selection | `ToolBodyRegion`, `SelectionState` (`swIndentSelectionState_e`). |
| Thickness | Wall thickness around the tool (solid targets) | `Thickness`. |
| Clearance | Gap between tool and target, with direction | `Clearance`, `ClearanceDirection`. |
| Cut | Removes the intersection instead of indenting | `Cut`, `CutDirection`. |

## Flex

Deforms bodies with one of four global shapes, controlled by two trim planes and a triad. The API exposes it through generic feature parameters and has no dedicated data interface.

- **Bending:** bend angle or radius about the triad's axis.
- **Twisting:** twist angle about an axis.
- **Tapering:** taper factor along an axis.
- **Stretching:** stretch distance along an axis.
- Shared options: bodies to flex, trim plane 1 and 2 positions (limit the affected region), triad position and rotation, hard edges (keep faces analytic where possible), accuracy slider.

## Deform

Freeform deformation of whole bodies.

- **Point deform:** pushes or pulls a point on a face by a distance, within a region radius; shape options for stiffness and accuracy.
- **Curve to curve:** maps a set of initial curves to target curves, with fixed curves and edges, and shape stiffness, accuracy and match options.
- **Surface push:** pushes one body into another using a tool body (sphere, ellipsoid, polygon, rectangle or a user body), with deform deviation and accuracy.

## Freeform

Reshapes a single face by moving control points on a mesh of curves laid over it. Options are the face, continuity at each boundary (contact, tangent or curvature), direction 1 and 2 curves, control points with triads, and a direction 1/2 symmetry option.

## Scale

Scales a part or selected bodies. API: `IScaleFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Scale about | Centroid, origin, coordinate system | `Type` (`swScaleType_e`), `CoordinateSystem`. |
| Uniform | One factor, or separate X, Y and Z factors | `Uniform`, `ScaleUniform`, `ScaleX`, `ScaleY`, `ScaleZ`. |
| Bodies | Which bodies to scale | `Bodies`. |

Scale changes geometry only. Dimensions on earlier features keep their values, which is why SOLIDWORKS puts Scale late in the tree.

## Fastening features (plastic parts)

These have no dedicated API data interfaces. They are built from a single dialog each.

- **Mounting boss:** a boss with optional hole or pin on a face, located on an edge's centre. Options: boss type (hardware or pin), boss height and diameter, draft, hole or pin diameter and depth, number of fins with fin height, width and draft, select mating face.
- **Snap hook:** a cantilever latch. Options: entities for placement and direction, height, thickness, lip length, overhang, draft, and lip angle.
- **Snap hook groove:** a matching groove cut into another body, sized from the snap hook with clearance.
- **Vent:** cuts a vent pattern from a sketch. Options: boundary, ribs and spars (thickness, depth, offset, draft), fill-in boundary, flow area readout, fillet radius.
- **Lip/groove:** matching lip and groove on two bodies. Options: lip and groove bodies, mating faces, edges, lip height and width, groove clearance, draft.

## 3D Texture

Turns an appearance's grayscale texture into real geometry by offsetting the mesh: white areas raise, black stay. Options: texture map, offset distance, refinement and element size, smoothing, and texture filter. The result is a mesh (graphics or BREP) body.

## Library features and Design Library

Reusable groups of features saved as `.sldlfp` files, with reference points and locating dimensions. Inserting one asks for each reference, places the features, and keeps their parameters editable. The Design Library task pane browses library features, parts, annotations and forming tools. API: `ILibraryFeatureData` (references, dimensions, link to library, dissolve).

## FeatureWorks

Recognises features (extrudes, revolves, holes, fillets, chamfers, drafts) in imported dumb solids and rebuilds a parametric tree. Modes: automatic, interactive (pick faces per feature type). Options: feature types to recognise, combine features, step-by-step recognition. It's a built-in add-in.

## FeatureXpert, FilletXpert and DraftXpert

- **FeatureXpert:** when a constant radius fillet or neutral plane draft fails, it automatically tries reordering and splitting fillets and drafts.
- **FilletXpert:** a guided UI for constant radius fillets. Add tab, change tab (resize or remove fillets) and corner tab (pick alternative corner blends).
- **DraftXpert:** a guided UI for neutral plane draft that lets you add, change and check drafts.

## Feature freeze

A bar in the feature tree. Features above it are frozen: they don't rebuild until unfrozen. This speeds up rebuilds of large parts. Option: enabled in system settings.

## Missing reference ghosting

When a feature loses a face or edge it referred to, the edit dialog shows the missing entity in the graphics as a ghost, so it can be swapped for a new one.

## Make Multibody Part

Converts an assembly into one multibody part, with each component a linked body. Options: target part name and location, and whether to keep a link to the assembly.
