# Phase 5 implementation plan

Status: proposed 2026-10-15, awaiting the owner's [decisions](#decisions-needed). To be
built on `phase-5`, branched from `phase-4`; it is rebased onto `main` once Phase 4 merges
(see [Phase 4 status](phase-4-status.md)).

## Goal and starting point

Deliver the sheet metal and weldments scope in [the roadmap](plan.md#scope-and-phases):
flanges, hems, jogs, bends, flat patterns, forming tools and gauge tables; structural
members, trim/extend, gussets, end caps, weld beads and cut lists. This phase also adds the
drawing items that Phase 4 deferred to it: flat pattern views, bend and punch tables, and
weldment cut list and weld tables. Every delivered capability must work through the desktop
and the shared core used by JSON-RPC, CLI and MCP. Detailed behaviour comes from
[the sheet metal spec](features/10-sheet-metal.md) and
[the weldments spec](features/11-weldments-and-structure-system.md).

The Phase 4 baseline provides primitives worth retaining:

- Multibody parts: `BodyId`, feature scope, body display, combine, move/copy and split
  bodies. Weldments are multibody parts, and sheet metal parts can hold several bodies.
- Extrude with every end condition, thin features, revolve, sweep (profile, path, guide,
  twist), loft, wrap onto cylinders, shell and boolean tools in `of-kernel-sys`.
- Stable, body-qualified face and edge names, so flanges can attach to edges and survive
  rebuilds.
- Configurations, equations and global variables, custom properties linked to mass and
  material, and the material library.
- A parametric standard-hardware library generated from published dimension tables
  (`of-assembly/src/hardware.rs`); weldment profiles and forming tools follow the same
  pattern.
- Drawings: model views of parts, hidden-component display, BOM, hole and revision tables,
  weld symbols as 2D annotations, and PDF and DXF writers working from one display list.

Gaps that shape this phase:

- Nothing in the model knows about bends. Flattening needs bend data (axis, angle, inside
  radius, thickness, K-factor) for every bend face, and an unfolder. OpenCASCADE has no
  sheet metal or unfolding API.
- Materials carry only a density: no sheet metal defaults, and no gauge or bend tables.
- No body grouping (cut list or weld folders), and no per-body properties beyond name and
  visibility.
- Drawing views always show the whole model: no single-body views, no flat state and no
  bend lines.
- Feature panels exist only for part, sketch and assembly features. There are no Sheet Metal
  or Weldments ribbon tabs.

## Delivery rules

The Phase 4 rules carry over unchanged. Each milestone is a vertical slice: kernel, model,
references, protocol, desktop creation and editing, persistence and regression tests. Keep
the crate dependency direction, and keep all C++ and `unsafe` in `*-sys` crates. Model units
stay millimetres and radians. Failed operations keep the last valid state and report
actionable errors. Record unsupported variants explicitly; do not mark Phase 5 complete while
a required item is blocked.

Two rules specific to this phase:

- **Bends are known by construction, never guessed.** Each sheet metal feature records the
  bends it creates in the body's sheet metal state, by stable face name. Only Convert to
  Sheet Metal and Insert Bends recognise bends from geometry, and they write the same
  records. The flat pattern, bend lines, bend notes, bend tables and cut list properties
  are all read from those records.
- **The flat pattern is a derived state of the same body, not a copy.** Flattening keeps
  face names, so a hole cut in the formed part, its flat outline, its drawing dimension and
  its DXF entity all name the same face.

## Scope

Required in this phase (roadmap items plus what they cannot work without):

| Area | Required |
| --- | --- |
| Parameters | Sheet-Metal feature per body: thickness, inside bend radius, bend allowance (K-factor, bend allowance, bend deduction, bend table, bend calculation table, gauge table), auto relief (rectangular, tear, obround, ratio or size), fixed face; per-feature overrides; gauge tables by material and gauge; material sheet metal defaults |
| Flanges and bends | Base flange and tab (closed, open and multiple contours, every end condition), edge flange (angle, every length end condition and measure, every position, position offset, trim side bends, gap, profile edit per edge), miter flange, hem (closed, open, tear drop, rolled, double), jog, sketched bend, swept flange, lofted bend (bent and formed), cross break |
| Conversion | Insert bends and convert to sheet metal (fixed face, bend and rip edges, keep body), rip |
| Bend state | Flatten toggle and the Flat-Pattern feature (fixed face, merge faces, simplify bends, corner treatment, corner trim, break corners, excluded faces); unfold and fold for cuts across bends; normal cut |
| Corners and details | Corner relief (2- and 3-bend; every relief type), closed corner (butt, overlap, underlap), welded corner, break corner/corner trim, bend notch, tab and slot, sheet metal gusset |
| Forming tools | Forming Tool feature (stopping face, faces to remove, insertion point) on ordinary parts; placing a tool on a face (rotation, flip, configuration, link); flat display (punch, profile, center); a starter library: louver, lance, emboss, dimple (extruded flange) and rib |
| Multibody sheet metal | Several sheet metal bodies in one part, each with its own parameters and flat pattern; merging by an edge flange up to edge |
| Weldments | Weldment feature; structural members along 2D and 3D sketch segments with groups, corner treatments (end miter, end butt 1 and 2, gap, merge miter-trimmed bodies, simple and coped cuts), profile position, mirroring, rotation and locate profile; trim/extend against faces, planes or bodies; end caps; gussets |
| Profiles | Built-in ISO and ANSI profile library (angles, channels, I beams, rectangular and square tubes, pipe) generated from published dimension tables; custom profiles from any part sketch; transfer of the profile's material |
| Weld beads | Cosmetic weld beads (path or from/to faces, smart weld selection, size, one or both sides, intermittent, staggered, weld symbol) and modeled fillet weld beads (full length, intermittent, staggered); Weld Folder with weld properties |
| Cut lists | Cut list folder with automatic grouping of identical bodies, manual grouping, exclude from cut list, sort by profile; item properties (length, angles, description, material, mass, quantity, bounding box sizes); sheet metal items (flat bounding box, thickness, bend count, cutting lengths, cut-outs, gauge); sub-weldment folders |
| Output | Flat pattern DXF from the part (bend lines up and down on their own layers, sketches, forming tool profiles, one file per body); CLI `openforge export part.ofp flat.dxf` and MCP |
| Drawings | Flat pattern view with bend lines and bend notes, bend table and punch table; views of selected bodies; weldment cut list table and weld table; balloons linked to cut list items; weld symbols attached to beads |

Deferred to later phases or extensions, recorded in the status document (pending the
[decisions](#decisions-needed)): Structure System (primary and secondary members, corner
management, connection elements), Excel gauge and bend tables (read as CSV instead), the
legacy No Bends state, sheet metal costing, inserting a sub-weldment into a new part as a
linked part, and assembly BOMs that expand cut lists.

## Decisions needed

| Decision | Recommendation | Why |
| --- | --- | --- |
| Flattening method | Flatten from the bend records each feature writes (above). Planar faces unfold by rigid transforms along the face graph from the fixed face. Cylindrical and conical bend faces are rolled out at the neutral radius `R + K·t`. Face boundaries are mapped exactly for lines and circles, and as fitted curves otherwise. Recognition from geometry is only for Convert to Sheet Metal and Insert Bends. | General geometric recognition is brittle on real parts. Construction records are exact and keep face names, and cuts made after flanges still unfold because they only trim faces whose bend data is known |
| Lofted bends, formed | Exact unrolling for cylindrical and conical patches, which covers round-to-square and cone transitions. Other ruled surfaces are refused with a clear error. Bent (faceted) lofted bends are exact. | Exact developable flattening for those surfaces needs no approximation. A general unroller would bring tolerance questions this phase does not need |
| Gauge and bend tables | CSV and JSON in place of Excel. Ship gauge tables for mild steel, stainless and aluminium (gauge names, thicknesses, radii, K-factors) from published gauge data. Bend tables use bilinear interpolation over thickness, radius and angle. | No xlsx dependency in the core, and the files are easy to diff. Excel import can follow |
| Material sheet metal defaults | Add an optional default K-factor and minimum bend radius to library materials, used when "use material parameters" is on | Matches the spec, and the material library is ours |
| Weldment profiles | Generated in code from dimension tables, like the hardware library. ISO: EN 10056 angles, EN 10279 channels, EN 10365 IPE and HEA beams, EN 10219 hollow sections. ANSI: AISC W, C and L shapes, HSS tubes, ASME B36.10 pipe. A custom profile is any part file whose chosen sketch is the profile. | Same clean-room route as the hardware library: published facts, our own code. Users already know how to make a part |
| Weld beads | Both kinds. Cosmetic beads are drawn and given properties and symbols without changing the solid. Fillet weld beads are modeled as triangular sweeps along the edge, as separate bodies. | Cosmetic beads are SOLIDWORKS' default and keep rebuilds fast. Fillet weld beads give mass and interference results |
| Structure System | Defer to a later extension. Structural members, trim/extend and per-group corner treatments cover frame design in this phase. | It is a second frame-design mode beyond the roadmap line. It shares the profile library and trimming code, so it can follow without schema changes |
| Part file and protocol | Part file v6 (sheet metal parameters, new feature kinds, cut list and weld folders). Protocol stays at 5: the new feature types come through `feature.add` and the catalog, and snapshot fields are additive. | Older cores refuse v6 files cleanly. Clients that only read v5 snapshots keep working |

## Milestones and dependencies

| Milestone | Deliverable | Depends on | Acceptance example |
| --- | --- | --- | --- |
| P5.0 | Contracts, schema and unfold prototype | Phase 4 merged or rebased | A 2 mm L-bracket with R2 and K 0.44: flat length matches `legs + θ(R + K·t)` to 1e-6 mm. A Ø8 hole across the bend unfolds to the correct slot shape. Part v6 schema and fixtures |
| P5.1 | Sheet metal parameters, base flange, edge flange, flatten | P5.0 | A 1.5 mm steel enclosure tray from a base flange and four edge flanges with auto relief; flatten and re-form; change the gauge, the flat updates |
| P5.2 | Miter flange, hem, jog, sketched bend, swept flange, lofted bend, cross break | P5.1 | A round-to-square duct transition flattened both bent (12 facets) and formed; a closed hem and a jog on the tray |
| P5.3 | Convert to sheet metal, insert bends, rip, unfold and fold, normal cut, multibody sheet metal | P5.1 | A shelled box with ripped corners converts and flattens; a slot cut across a bend while unfolded refolds correctly |
| P5.4 | Corner treatments, bend notch, tab and slot, gusset, forming tools | P5.1, P5.3 | Closed overlap corners and circular corner reliefs on the tray; a louver array from the starter library shows as punch outlines in the flat |
| P5.5 | Sheet metal output and drawings | P5.1–P5.4 | Flat DXF of the enclosure with UP and DOWN bend lines on separate layers at true size; drawing with a flat pattern view, bend notes, a bend table and a punch table |
| P5.6 | Weldment feature, profile library, structural members, trim/extend, end caps, gussets | P5.0 | A 1200 × 600 × 800 frame of 40 × 40 × 3 square tube with mitered corners, rectangular tube cross members trimmed to the frame, capped ends and corner gussets |
| P5.7 | Weld beads and cut lists | P5.6, P5.1 | Cut list groups the frame into items with lengths and end angles, and a sheet metal bracket welded on gets its flat size. Fillet beads add mass; cosmetic beads carry symbols |
| P5.8 | Weldment drawings | P5.7 | Frame drawing with a single-member view, a cut list table, balloons matching cut list items and a weld table |
| P5.9 | Corpus, conformance, performance and release gate | All above | Hosted three-platform checks pass on the exact release candidate |

P5.6 depends only on P5.0, so weldments can be built alongside sheet metal (P5.1–P5.5).
P5.2, P5.3 and P5.4 are independent once P5.1 passes. No calendar commitment: estimate after
P5.0 has measured the unfolder.

## P5.0 — Contracts and prototypes

**Sheet metal state.** A derived, per-body `SheetMetalState`, rebuilt with the body and
never stored in the file:

- `params`: thickness, inside radius, allowance rule, relief and fixed face, after
  per-feature overrides.
- `bends`: a stable `BendId` (creating feature + local index), the inner and outer bend face
  names, axis, angle, inside radius, allowance rule, direction (up or down from the fixed
  face side) and the two flat faces it joins.
- `forms`: forming tool instances, with punch outline, center and ID.

Features add bends through one builder API. Later features that trim or split a bend face
keep its `BendId`, because face naming already tracks splits.

**Unfolder.** A pure Rust module in `of-model` that walks the face graph of one side of the
sheet (inner faces) from the fixed face, places each planar face by a rigid transform, and
rolls each bend face out at its neutral radius. Edges and arcs are mapped exactly; other
curves are refitted to 1e-4 mm. The flat outline is then extruded to the thickness, with
holes and cuts as inner loops. The output is a flat body whose faces keep their names, plus
bend lines (center of each bend region, tagged up or down). Measure it on the L-bracket, the
tray, a U-channel with holes across both bends, and the duct.

**Kernel needs.** Thicken a sheet into a solid on either side or symmetric
(`BRepOffset_MakeOffset` with a thickness), cylindrical and conical bend patches between two
planar walls, face boundaries as 2D parametric curves (pcurves) for exact mapping, and
`BRepAlgoAPI_Splitter` for unfold-and-fold cuts. All of these go in `of-kernel-sys`, with
safe wrappers in `of-geom`.

**Schema.** Part file v6: `SheetMetal`, flange, bend, corner, form, weldment, member, trim,
cap, gusset and bead feature kinds; `cut_list` (manual groups, exclusions, item property
overrides keyed by the item's first body) and `weld_folder` properties. Migration from v5 is
the identity.

**Profiles prototype.** Generate one profile of each family from its table, sweep it along a
two-segment path, and check the area and second moments against the published table values
to 0.5 %.

## P5.1 — Parameters, base flange, edge flange and flatten

The Sheet-Metal feature holds the body's defaults. It is added automatically by the first
base flange, as in SOLIDWORKS, and can be edited like any feature. Allowance rules:

- K-factor: `BA = θ(R + K·t)`.
- Bend allowance or deduction: direct values.
- Bend table: by thickness, radius and angle, interpolated.
- Bend calculation table: allowance as an equation per angle range, using the existing
  equation evaluator.
- Gauge table: picks thickness, radius and K-factor by material and gauge name.

Gauge tables live in `assets/sheet-metal/` and can also be loaded from user files.

Base flange: a closed contour makes a tab; an open contour is thickened and extruded with
bends at every corner (blind, mid plane, up to vertex, up to surface, offset from surface,
direction 2); several closed contours make plates or inner holes. A later closed-sketch tab
on a face merges at the sheet thickness.

Edge flange: one or more linear edges, angle (absolute or relative to a reference face),
length end conditions measured from the outer or inner virtual sharp or tangent to the
bend, every flange position, position offset, trim side bends, gap between flanges on
adjacent edges, editable profile per edge, and custom radius, allowance and relief. Auto
relief cuts rectangular, tear or obround reliefs where a flange is shorter than its edge.

Flatten: a ribbon toggle and the Flat-Pattern feature, always last and suppressed while
formed. Fixed face, merge faces, simplify bends, corner treatment and excluded faces work in
this milestone; corner trim and break corners are added in P5.4.

Desktop: a Sheet Metal ribbon tab (shown when the part has sheet metal, or always from the
ribbon menu), panels for each feature, a Flatten toggle, a sheet metal folder in the feature
tree with its parameters, and gauge selection.

## P5.2 — More flanges and bends

- **Miter flange:** a chain of tangent edges from one profile sketch, mitered corners, start
  and end offsets, and the edge flange options.
- **Hem:** closed, open, tear drop, rolled and double, with miter gaps where hems meet. A
  closed hem is a 180° bend of radius `≈ 0` and flattens with the gauge table's hem
  allowance.
- **Jog:** a Z offset from a line on a face, with every offset end condition, dimension
  position, jog position, angle and fix projected length.
- **Sketched bend:** bends a flat region about a line, with the four bend positions.
- **Swept flange:** an open profile along a path; it flattens along the path or as a
  cylindrical or conical edge.
- **Lofted bend:** bent (facets by count, deviation, segment length or angle) and formed.
  Formed flattening follows the [decision](#decisions-needed) above.
- **Cross break:** cosmetic lines with break angle and radius, drawn in views and the flat.

## P5.3 — Conversion, rips and bend state

Convert to Sheet Metal takes a fixed face, a thickness, bend edges and rip edges on any
solid, with rip gap and overlap, corner defaults and an option to keep the original body.
Insert Bends works on a thin-walled solid (for example a shell): it finds the sheet's two
sides by constant-offset face pairs, replaces sharp edges with bends of the given radius,
and applies rips. Both write normal bend records, so everything after them behaves as if the
part had been made from flanges.

Rip cuts a gap along edges in one or both directions. Unfold flattens chosen bends about a
fixed face so that cuts can cross them; Fold re-forms them by mapping the cut regions back
through the same per-face transforms. Normal Cut makes cuts square to the sheet whatever the
sketch's direction.

Multibody sheet metal: each body has its own Sheet-Metal feature, state and flat pattern;
an edge flange set to up to edge and merge joins two bodies into one.

## P5.4 — Corners, details and forming tools

- **Corner relief** for 2- and 3-bend corners: rectangular, circular, tear, obround,
  constant width, full round and suitcase; sized by thickness ratio or value; centered on
  bend lines or tangent to the bend; optional slit.
- **Closed corner:** butt, overlap and underlap, with gap and overlap ratio, open bend
  region and coplanar faces.
- **Welded corner:** a bead shape on a closed corner, with stop point, fillet radius and weld
  symbol.
- **Break corner and corner trim:** chamfer or fillet of outside corners, also offered on
  the flat pattern.
- **Bend notch:** notches at bend lines, on one or both sides.
- **Tab and slot:** tabs on one body and matching slots in another (equal spacing by count
  or distance, sizes, clearances, corner type, through-all slots).
- **Sheet metal gusset:** across a bend, with indent depth or profile dimensions, rounded or
  flat back, width, thickness, draft and fillets.

Forming tools are ordinary `.ofp` parts with a Forming Tool feature (stopping face, faces to
remove, insertion point). Placing one on a face boolean-unites or cuts the tool's shape
(minus the removed faces) offset to the sheet thickness, linked to the tool file through the
Phase 3 part cache. The starter library (louver, lance, emboss, dimple and rib) is generated
parametrically, like the hardware. In the flat, forms are excluded and shown as punch
profile and center, with a punch ID for the punch table.

## P5.5 — Sheet metal output and drawings

Cut list properties for sheet metal bodies: flat bounding box length and width (smallest
rectangle over the flat outline's rotations), thickness, bend count, outer and inner cutting
length, cut-out count, gauge, material and mass.

Flat pattern DXF from the part, through the Phase 4 DXF writer: outline and holes on
`CUT`, bend lines on `BEND_UP` and `BEND_DOWN`, forms on `FORM`, optional sketches,
optional bend notes, one file per body or all bodies in one, R12 or 2018, splines as
polylines when asked. CLI `openforge export part.ofp flat.dxf` and an MCP tool.

Drawings: model views get a `flat` option (flat pattern of a body, with its configuration),
bend lines in their own line style and layer, bend notes (`UP 90° R1`, standard-dependent),
a bend table (tag, direction, angle, radius, K-factor or allowance, order) with bend tags
on the view, and a punch table (punch ID, X, Y, angle, quantity) from a picked origin. The
flat bounding box and cut list properties can be linked into notes and the title block.

## P5.6 — Weldments

The Weldment feature marks the part (one per part, before the first member). New bodies are
not merged, and the cut list folder plus weldment properties are added.

Structural members: profile (standard, type and size from the library, or a custom
profile), groups of segments from 2D and 3D sketches, corner treatment per group (end miter,
end butt 1, end butt 2, gap, merge miter-trimmed bodies), simple or coped cuts between
connected segments, mirror, align, rotate, locate profile and allow protrusion. Each segment
is its own body with a stable `BodyId` from feature and segment. Corners are cut with
half-spaces at the bisector or butt planes, so they never depend on mesh booleans.

Trim/extend: end trim, end miter, end butt 1 and 2, against a face or plane (flush) or
against bodies (simple or coped), with allow extension and a weld gap.

End caps: face, thickness direction (outward, inward, inset), thickness, offset as a ratio
of wall thickness or a distance, and chamfered or filleted corners. Gussets: two adjacent
planar faces, triangular or polygonal profile (d1–d4 or a leg and an angle), inner corner
chamfer, thickness inner, both or outer, and location at the start, middle or end with an
offset.

Desktop: a Weldments ribbon tab, a profile picker with the library tree and a section
preview, group editing on the sketch, and trim and cap panels.

## P5.7 — Weld beads and cut lists

Cosmetic weld beads: weld path or from/to faces, smart weld selection (a stroke across the
faces picks the intersecting edges), bead size, tangent propagation, one or both sides,
intermittent (length and gap or pitch), staggered, and a from/to length. Each bead carries a
weld symbol and is drawn as a lightweight graphic in the viewport. Fillet weld beads are
modeled as separate bodies (full length, intermittent, staggered, other side). The Weld
Folder lists beads with weld material, process, mass per length and total length.

Cut lists: automatic grouping of identical bodies by profile, length and end angles for
members, and by flat outline for sheet metal. Other bodies group by volume, surface area,
bounding box and principal moments, within tolerance. Also: manual grouping, exclude from
cut list, sort by profile, collect identical bodies, item names from properties, and
editable item properties as overrides. Sub-weldment folders group bodies for separate
fabrication. `doc.cut_list` and `doc.cut_list.set` serve the folder, which also appears in
the desktop feature tree with a properties panel.

## P5.8 — Weldment drawings

- Model views of selected bodies (one member, a sub-weldment, a sheet metal body), with
  their own orientation.
- Weldment cut list table: item, quantity, description, length, angles, material and any
  property. It splits across sheets and exports CSV, like the Phase 4 tables.
- Balloons: link to cut list item numbers, and auto balloon works per cut list item.
- Weld table from the Weld Folder.
- Weld symbols: attached to bead edges, read from the bead's properties.

## Code integration map

| Area | Primary files and intended changes |
| --- | --- |
| Kernel | `of-kernel-sys` (thicken sheets, bend patches, pcurves, splitter, profile sweeps along polylines) and `of-geom` (safe wrappers) |
| Parts | `crates/of-model`: `sheetmetal/` (parameters, tables, bend records, features, unfolder, corners, forms), `weldment/` (members, trims, caps, gussets, beads, cut lists), part file v6 and migration, material sheet metal defaults |
| Libraries | Weldment profiles and forming tools generated from tables (in `of-model` or a new `of-library` crate shared with the hardware library), gauge tables in `assets/sheet-metal/` |
| Drawings | `crates/of-drawing`: flat and body-selection model views, bend lines and notes, bend, punch, cut list and weld tables, flat DXF export |
| Core | `crates/openforge`: feature catalog entries, `doc.cut_list`, `doc.flat`, sheet metal state in the part snapshot, export of flat patterns from parts in CLI and MCP |
| Desktop | Sheet Metal and Weldments ribbon tabs, a panel per feature, flatten toggle, cut list and weld folders in the tree, profile and gauge pickers, drawing panels for flat views and the new tables |
| Tests and docs | Unfold fixtures against the bend formulas, profile section properties, cut list grouping, file round trips, serve tests, conformance checks for areas 10 and 11, smoke scenarios, `docs/phase-5-status.md`, `docs/protocol.md`, feature docs 10 and 11 |

## P5.9 — Acceptance and release

Every behaviour change gets a regression test at the lowest useful layer. Flat patterns are
checked numerically: every flat length against the allowance formula, and every hole and
cut position against hand-computed values. Flat DXF is parsed back and its layers and
extents checked. Build a reproducible corpus:

- **Electrical enclosure:** base flange, edge and miter flanges, hems, closed corners and
  corner reliefs, a louver array and a cut across a bend. Flat DXF and a drawing with a flat
  pattern view, bend table and punch table.
- **Duct transition:** round to square as a lofted bend, bent and formed, with the flat
  checked against the analytic development.
- **Converted housing:** a shelled box made into sheet metal with rips and bends.
- **Machine frame:** square and rectangular tube members, mitered and butted corners,
  trims, end caps, gussets, cosmetic and fillet beads, and a welded-on sheet metal bracket.
  Cut list and a drawing with a member view, cut list table, balloons and weld table.

Extend the native desktop smoke test with a sheet metal scenario (base and edge flanges,
flatten, flat DXF, flat pattern drawing view) and a weldment scenario (members from a
sketch, trim, cut list, cut list table). Add catalog conformance checks for areas 10 and 11.
Set budgets in `docs/phase-5-budgets.json` (enclosure rebuild, flatten, flat DXF, frame
rebuild of about 40 members, cut list), measured with a `scripts/phase-5-performance.mjs`
like Phase 4's, and check them in CI.

Run the repository gates, the native smoke test and hosted macOS, Windows and Linux
workflows on the release candidate. Publish supported modes and remaining limits in
`phase-5-status.md`. Merge `phase-5` to `main` only when the required scope and hosted
checks are complete.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Unfolding errors on complex parts | Wrong flat blanks: parts made scrap | Bend records instead of recognition; numerical checks against the formulas on every fixture; refuse rather than guess when a face has no bend record |
| OCCT offset and thickening failures | Flanges and conversions fail on real geometry | Build walls from 2D profiles where possible instead of offsetting faces; a corpus of failing cases with clear errors |
| Face naming through flanges, reliefs and corners | Features and drawing anchors break after edits | Name every created face from its bend or wall record; rebuild tests that edit the base sketch and check downstream references |
| Miter and coped cuts between members | Slivers or failed booleans at frame corners | Planar half-space cuts for miters and butts; coped cuts only against the other member's outer faces |
| Scope | Phase slips | P5.6–P5.8 can proceed alongside sheet metal; Structure System deferred (pending decision) |
