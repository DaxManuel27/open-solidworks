# 10. Sheet metal

Detailed specs for [catalog section 10](../solidworks-feature-catalog.md#10-sheet-metal). See the [README](README.md) for sources and conventions.

A sheet metal part is a constant-thickness body with bends, which always knows how to flatten. Every sheet metal body is governed by a Sheet-Metal feature holding its defaults; each bend and flange can override them.

**OpenForge today:** sheet metal parts, flat patterns, flat DXF and flat pattern drawings, in the core, CLI/MCP and the desktop Sheet Metal tab. Each section says what is supported; [the Phase 5 status](../phase-5-status.md) records the limits.

## Sheet metal parameters

API: `ISheetMetalFeatureData` (per body), with overrides on each bend feature.

| Parameter | Values | Notes (API name) |
| --- | --- | --- |
| Thickness | Length | `Thickness`. |
| Bend radius | Length (inside radius) | `BendRadius`. |
| Gauge table | Excel table of thickness, radius and K-factor per material and gauge | `UseGaugeTable`, gauge table path. Picks thickness by gauge name. |
| Use material parameters | Takes thickness, radius and allowance from the material (2020 and later) | `UseMaterialSheetMetalParameters`. |
| Bend allowance | K-factor, bend allowance, bend deduction, bend table, bend calculation table, gauge table | `swBendAllowanceTypes_e`: BendTable 1, KFactor 2, Direct 3, Deduction 4, BendCalculationTable 5, GaugeTable 6. |
| Auto relief | Rectangular, tear, obround, none, tear-bend; relief ratio or width and depth | `UseAutoRelief`, `AutoReliefType` (`swSheetMetalReliefTypes_e`), `ReliefRatio`. |
| Fixed face | The face that stays put when flattening | `FixedReference`. |

**Bend math the specs depend on**

- **K-factor:** the neutral axis sits at `t × K` from the inside of the bend (0 ≤ K ≤ 0.5 in practice; SOLIDWORKS allows up to 1).
- **Bend allowance:** `BA = θ × (R + K × t)`, with θ in radians, R the inside radius and t the thickness. Flat length = sum of flat legs + sum of BA.
- **Bend deduction:** flat length = sum of outside (virtual sharp) lengths − sum of BD.
- **Bend tables** give BA or BD per thickness, radius and angle, interpolated between entries.

**OpenForge today:** per body thickness and inside radius; K-factor, bend allowance, bend deduction, bend table and bend calculation table; gauge tables (mild steel, stainless, aluminium) that set thickness, radius and K-factor by gauge name; material sheet metal defaults; auto relief rectangular, tear or obround by ratio or width and depth; per-feature overrides. The fixed face is chosen on the Flat-Pattern feature.

## Base flange/tab

The first sheet metal feature, made from a sketch.

| Sketch | Result |
| --- | --- |
| Single closed contour | A flat plate (tab) |
| Single open contour | A bent profile, extruded to a depth, with bends at each corner |
| Several closed contours | Several plates in one feature, or holes inside a plate |

Options: thickness and direction (reverse or symmetric for open profiles), bend radius, gauge table, end conditions for open profiles (blind, mid plane, up to vertex, up to surface, offset from surface; direction 2), bend allowance, auto relief. API: `IBaseFlangeFeatureData`.

A later closed-sketch **tab** on a face adds material at the sheet thickness and merges into the existing body.

**OpenForge today:** closed, open and multiple contours; normal, reverse or mid-plane thickness; every end condition with direction 2; tabs on a sheet's face merge into it.

## Edge flange

A wall bent up from an edge. API: `IEdgeFlangeFeatureData`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Edges | One or more linear edges; a profile sketch can be edited for each | `Edges`, `AddEdges`, `RemoveEdges`. |
| Angle | Bend angle, optionally relative to a reference face (perpendicular or parallel to it) | `BendAngle`, `AngleReference`, `PerpendicularToFace`. |
| Length end condition | Blind, up to vertex, up to surface, offset from surface, mid plane, up to edge and merge | `OffsetType` (`swFlangeOffsetTypes_e`), `OffsetDistance`, `OffsetReference`, `ReverseOffset`. |
| Length measured from | Outer virtual sharp, inner virtual sharp, tangent to bend | `OffsetDimType` (`swFlangeDimTypes_e`). |
| Flange position | Material inside, material outside, bend outside, bend from virtual sharp, tangent to bend | `PositionType` (`swFlangePositionTypes_e`). |
| Position offset | Offset the flange from the edge, with its own end condition | `UsePositionOffset`, `PositionOffsetType`, `PositionOffsetDistance`. |
| Trim side bends | Removes material from neighbouring bends | `UsePositionTrimSideBends`. |
| Gap distance | Between flanges on adjacent edges | `GapDistance`. |
| Custom bend radius, allowance, relief | Override the defaults | `UseDefaultBendRadius`, `UseDefaultBendAllowance`, `UseDefaultBendRelief`, `ReliefTearType` (rip or extend). |

**OpenForge today:** every option above except trim side bends: several edges, angle with a reference face (perpendicular or parallel), every length end condition and measure, every position, position offset, start and end offsets, gap, an edited wall profile per edge, custom radius, allowance and relief.

## Miter flange

Flanges along a chain of edges from one profile sketch, with mitered corners. API: `IMiterFlangeFeatureData`. Options: edges (tangent-propagated), flange position, trim side bends, gap distance, start and end offsets, custom radius, allowance and relief. The profile sketch must be perpendicular to the first edge.

**OpenForge today:** profile sketch along a chain of edges, position, gap, start and end offsets, custom radius, allowance and relief. Trim side bends is not offered.

## Hem

Folds an edge back on itself. API: `IHemFeatureData`, type in `swHemTypes_e`.

| Type | Size inputs |
| --- | --- |
| Closed | Length |
| Open | Length, gap |
| Tear drop | Angle, radius |
| Rolled | Angle, radius |
| Double | Length, gap (2018 and later) |

Options: edges, position (material inside or bend outside), reverse direction, miter gap where hems meet, custom bend allowance.

**OpenForge today:** every type, position, reverse, gap and length, custom allowance. Miter gaps where hems meet are not offered.

## Jog

Adds two bends to make a Z-shaped offset from a sketch line on a face. API: `IJogFeatureData`.

Options: fixed face, offset end condition and distance (blind, up to vertex, up to surface, offset from surface), dimension position (outside offset, inside offset, overall dimension), fix projected length, jog position (bend centerline, material inside, material outside, bend outside; `swJogPositionType_e`), jog angle, bend radius, custom bend allowance.

**OpenForge today:** fixed face, offset, dimension position, fix projected length, every jog position, angle, custom radius and allowance. The offset is a distance (no up-to end conditions).

## Sketched bend

Bends a flat area along a sketch line. API: `ISketchedBendFeatureData`. Options: fixed face, bend position (bend centerline, material inside, material outside, bend outside), bend angle, reverse, bend radius, custom allowance.

**OpenForge today:** every option above.

## Lofted bend

A sheet metal loft between two open profiles, typically a round-to-square transition, which can be flattened. API: `ILoftedBendsFeatureData`.

| Option | Values |
| --- | --- |
| Manufacturing method | Bent (faceted, brake-formed) or formed (smooth, rolled) |
| Profiles | Two open sketches, each with a small gap |
| Faceting | Number of bend lines per transition, or maximum deviation, or segment length or angle |
| Reference end point | Facet transitions from theoretical vertices |
| Thickness and direction | Including symmetric |

**OpenForge today:** bent (by facet count or maximum deviation) and formed, thickness and direction, flattening. Facet length or angle and reference end points are not offered; see the status doc for the K-factor at bent hinges.

## Swept flange

Sweeps an open profile along a path to make a flange. API: `ISweptFlangeFeatureData`. Options: profile, path, flatten along path (with material inside), cylindrical or conical edge for flat pattern, start and end offsets, flange position, trim side bends, thickness, custom radius, allowance and relief.

**OpenForge today:** an open profile along a path, rolled out along it in the flat, with thickness side and custom parameters. Start and end offsets and trim side bends are not offered.

## Cross break

A visual X pattern on a flat face (stiffening), drawn as lines with a break angle and radius. API: `ICrossBreakFeatureData` (`Face`, `BreakAngle`, `BreakRadius`, `ReverseDirection`). Shown in drawings as cross lines; doesn't change geometry noticeably.

**OpenForge today:** recorded on a face with angle, radius and direction, and drawn; the solid is unchanged.

## Insert bends and convert to sheet metal

Turns a shelled or thin solid into sheet metal.

- **Insert bends:** picks a fixed face, bend radius and rips on a thin-walled solid; sharp corners become bends.
- **Convert to sheet metal:** picks a fixed face, bend edges and rip edges on any solid; thickness is set, and the solid's outer faces become the sheet. Options: keep the original body, gauge table, rip gap and overlap ratio, corner defaults, rip sketches. API: `IConvertSolidFeatureData`.

**OpenForge today:** insert bends (fixed face, radius, rips and rip gap) and convert to sheet metal (fixed face, bend edges, rip gap, keep body).

## Rip

Cuts a gap along an edge of a hollow shape so it can unfold. API: `IRipFeatureData` (`Edges`, `Gap`, direction per edge: one way, the other way or both).

**OpenForge today:** edges with a direction each and a gap.

## No bends, unfold and fold

- **Flatten:** toggles the whole part between formed and flat (the Flat-Pattern feature).
- **Unfold:** flattens selected bends so a cut can be added across them; **Fold** re-forms them. API: `IFoldsFeatureData` (`Bends`, `FixedFace`).
- **No bends** (legacy): rolls back to the flat state for legacy parts.

**OpenForge today:** the Flatten toggle and Flat-Pattern feature; unfold (with a fixed face) and fold for cuts across bends. No bends is not offered.

## Flatten and flat pattern

The Flat-Pattern feature, always last in the tree and suppressed while formed. API: `IFlatPatternFeatureData`.

| Option | Values |
| --- | --- |
| Fixed face | `FixedFace2` |
| Merge faces | Merge coplanar faces so bend lines are only lines |
| Simplify bends | Replace complex curved bend edges with lines |
| Corner treatment | Smooth corner edges in the flat |
| Show slit in corner relief | |
| Add corner trim | Relief type and distance or ratio to thickness |
| Break corners | Chamfer or fillet with a radius (`swBreakCornerTypes_e`) |
| Excluded faces | Faces left out of the flat |

**OpenForge today:** fixed face, simplify bends, split bend regions in place of merge faces, corner treatment, break corners and excluded faces; corner relief slits come from the corner relief feature.

## Flat pattern configurations and DXF/DWG export

Export Sheet Metal to DXF/DWG directly from the part. Options: what to export (sheet metal, faces, loops, annotation views), entities (geometry, hidden edges, bend lines, sketches, library feature or forming tool profiles), flip view, custom map file for layers, splines as polylines, and single file per body. Flat pattern view in drawings uses the same data.

**OpenForge today:** flat DXF (R12 or 2018) from the part, in the CLI, MCP and desktop export: outline, bend lines up and down on their own layers, form outlines and bend notes; one file per body. DWG and layer map files are not offered.

## Normal cut

Makes a cut square to the sheet surface even when the sketch isn't perpendicular, so the flat pattern is right. API: `ISMNormalCutFeatureData2`. Options: auto-propagation of tangent faces, cut direction, offset plane reference (to the K-factor or a layer value), optimize geometry, face groups.

**OpenForge today:** a sketch cut square to the sheet through flanges and bends. The other options are not offered.

## Corner treatments

| Feature | What it does | Options |
| --- | --- | --- |
| Corner relief | Relief cuts where two or three bends meet | Corner type (2-bend, 3-bend); relief type: rectangular, circular, tear, obround, constant width, full round, suitcase (`swCornerReliefType_e`); size relative to thickness; centered on bend lines; tangent to bend; add slit. API: `ICornerReliefFeatureData`. |
| Closed corner | Extends flanges to close a corner | Faces to extend; corner type butt, overlap or underlap (`swClosedCornerTypes_e`); gap; overlap ratio; open bend region; coplanar faces; narrow corner. API: `IClosedCornerFeatureData`. |
| Welded corner | Adds a weld bead shape to a closed corner | Corner face, stop point, fillet radius, texture, weld symbol. |
| Break corner/corner trim | Chamfers or fillets outside corners | Break type chamfer or fillet, distance, internal corners only. API: `IBreakCornerFeatureData`. |

**OpenForge today:** corner relief (2- and 3-bend, every relief type, ratio, centered, slit); closed corner (butt, overlap, underlap, gap, overlap ratio, coplanar faces); welded corner (cosmetic, with stop point, fillet radius and symbol); break corner (chamfer or fillet, internal only). See the status doc for the closed corner limits.

## Bend notch

Adds notch cuts at bend lines, for bending accuracy. Options: bend lines, notch shape and size, both sides.

**OpenForge today:** V or rectangular notches with width and depth, at one or both ends of chosen bends.

## Tab and slot

Adds interlocking tabs on one body and matching slots in another for welded assemblies. API: `ITabAndSlotFeatureData` (groups). Options: tab edge, start and end references, spacing (equal by count or by distance), tab length, height and width, slot clearance (length and width offsets), corner type (sharp, fillet, chamfer), and through-all slots.

**OpenForge today:** tab edge and slot face, start and end references, spacing by count or distance, tab length and height, slot clearance (overall, or by length and width), sharp, fillet or chamfer corners, through-all slots.

## Sheet metal gusset

A stamped rib across a bend. API: `ISMGussetFeatureData`.

| Option | Values |
| --- | --- |
| Position | Supporting faces (the bend's two legs), reference line, reference point, offset |
| Profile | Indent depth, or profile dimensions (d1, d2 legs or a leg and angle); rounded or flat back |
| Dimensions | Indent width and thickness |
| Draft and fillets | Side face draft angle; inner corner, outer corner and edge fillets |
| Flat pattern display | Show profile, show center |

**OpenForge today:** position along the bend, depth (or two legs, or a leg and angle), width, flat or rounded back, punch ID. Draft and fillets are not offered.

## Forming tools and stamp tool

Forming tools are parts with a stopping face and optional faces to remove; dropped onto a sheet metal face, they form a louver, lance, emboss, extruded flange or rib. API: `ILibraryFormToolFeatureData`. Options: placement face, rotation angle, flip tool, configuration, link to tool, flat pattern display (show punch, profile, center), punch ID. Making a tool: Forming Tool feature with stopping face, faces to remove and insertion point.

**OpenForge today:** the Forming Tool feature (stopping face, faces to remove, insertion point); placing a starter tool (louver, lance, emboss, dimple, rib) or a tool part file with rotation, flip, configuration and punch ID; flat display as punch outlines and centers.

## Gauge tables and bend tables

- **Gauge table:** Excel file listing material, gauge names, thicknesses, allowed radii and K-factors or allowances.
- **Bend table:** Excel or text file of bend allowance or deduction by thickness, radius and angle.
- **Bend calculation table:** equations for allowance by angle ranges.

**OpenForge today:** built-in gauge tables, and bend and bend calculation tables given as text in place of Excel files.

## Bend lines and bend notes in drawings

Flat pattern views show bend lines (up and down, as separate layers), bend notes (direction, angle, radius, e.g. "UP 90° R 1"), and a bend table listing each bend with order, direction, angle and radius. Options: bend note style, display of up/down, show bend direction.

**OpenForge today:** flat pattern views with bend lines (up and down layers) and bend notes, bend tables (order, direction, angle, radius, with tags on the view) and punch tables.

## Multibody sheet metal and merging bodies

A part can hold several sheet metal bodies, each with its own Sheet-Metal feature and flat pattern. Edge flanges can merge two bodies (up to edge and merge). Each body gets its own cut list item.

**OpenForge today:** every behaviour above.

## Sheet metal cut list and bounding box

Each sheet metal body's cut list item has automatic properties: bounding box length and width (smallest rectangle around the flat), thickness, bend count, cutting length (outer and inner), cut-outs count, surface treatment, bend allowance, material, mass, and sheet metal gauge.

**OpenForge today:** flat bounding box, thickness, bend count, cutting lengths, cut-outs, material, mass and gauge, as cut list properties and `{flat_length}`-style property links. Surface treatment is not recorded.

