# Phase 5 completion plan

Prepared 2026-10-16 against `phase-5` at `4dd2305`. This document gives the execution
order for everything left in [the Phase 5 plan](phase-5-plan.md). That plan's scope,
delivery rules and acceptance examples remain in force; this one does not relax them.

## Baseline

Delivered and tested on `phase-5` (see `handoff.md`, session 10):

- **Sheet metal model (P5.0–P5.4):** bend records, exact unfolder, sheet metal parameters
  with K-factor, allowance, deduction, bend table, calculation table, gauge tables and
  material defaults; base flange and tab; edge, miter, swept flanges; hems; jogs; sketched
  and lofted bends; cross breaks; convert to sheet metal, insert bends, rip; unfold, fold and
  normal cut; multibody merge by up-to-edge flange; corner relief, closed corner, welded
  corner, break corner, bend notch, tab and slot, sheet metal gusset; forming tools from the
  starter library or a part file.
- **Sheet metal output (P5.5):** flat outline and cut list properties (`of-model/src/flat.rs`),
  flat DXF (R12 and 2018) through `export`, CLI and MCP, `doc.sheet_metal`, flat pattern
  drawing views with bend lines and notes, bend and punch tables, property links.
- **Files and protocol:** part file v6 (v5 reads unchanged), protocol stays at 5,
  capabilities `sheet_metal` and `flat_pattern_dxf`.

Not started: every desktop sheet metal surface, sheet metal acceptance (corpus,
conformance area 10, smoke, budgets), all of weldments (P5.6–P5.8), and the release gate
(P5.9).

## Owner decisions

The Phase 5 plan's [decisions](phase-5-plan.md#decisions-needed) were never formally
recorded. The sheet metal work followed their recommendations. Steps W1–W7 assume the
remaining recommendations too:

| Decision | Assumed |
| --- | --- |
| Weldment profiles | Generated in code from published dimension tables (ISO and ANSI families listed in the plan); a custom profile is any part file's chosen sketch |
| Weld beads | Both kinds: cosmetic beads (no solid change) and modeled fillet beads as separate bodies |
| Structure System | Deferred to a later extension |
| Excel tables | Deferred; CSV and JSON only |

Decisions taken during implementation that the owner should confirm, since they shape
user-visible behaviour:

| Decision | Taken |
| --- | --- |
| Closed corner types | Butt: the selected flange stops `gap` short of the other's inside face, and the other extends across the corner. Overlap and underlap use an overlap ratio (0–1). Only the flange walls close; bend regions keep their miter and relief |
| Flat view orientation | Front of the flat (its back for `Back`); no other orientations, no rotation yet |
| Flat DXF curves | Non-line, non-arc curves are written as polylines at 0.001 mm deflection |

## Execution order

Two tracks. Sheet metal (S) finishes what is half built; weldments (W) are independent
until cut lists meet sheet metal bodies in W5. Do S1 → S4 first, so the delivered model
becomes usable in the app, then W1 → W7, then R. W1 may start earlier if desktop work is
blocked.

| Step | Scope | Depends on | Size | Exit evidence |
| --- | --- | --- | --- | --- |
| S1 | Sheet metal model gaps | — | M | Every row of the plan's scope table is covered by a model test or recorded as unsupported |
| S2 | Desktop: sheet metal features | S1 (fields only) | L | Tray from the P5.1 example built, edited and flattened entirely in the desktop |
| S3 | Desktop: sheet metal output and drawing panels | S2 | M | Flat DXF and a drawing with flat view, bend table and punch table made from the desktop |
| S4 | Sheet metal acceptance | S1–S3 | M | Enclosure, duct and converted housing corpus; conformance area 10; smoke scenario; budgets |
| W1 | Weldment foundations and profile library | — | M | Profile prototype within 0.5 % of table section properties; Weldment feature; part v6 kinds |
| W2 | Structural members | W1 | L | The 1200 × 600 × 800 tube frame with mitered corners builds and rebuilds by sketch edits |
| W3 | Trim/extend, end caps, gussets | W2 | M | Cross members trimmed to the frame, capped ends, corner gussets |
| W4 | Weld beads and weld folder | W2 | M | Cosmetic and fillet beads; fillet beads add the expected mass |
| W5 | Cut lists | W2, W3, S1 | L | Frame grouped into items with lengths and end angles; welded-on sheet metal bracket gets its flat size |
| W6 | Desktop: weldments | W2–W5 | L | The frame built from a sketch in the desktop, with cut list and weld folders in the tree |
| W7 | Weldment drawings | W5, W6 | M | Frame drawing with member view, cut list table, matching balloons, weld table |
| R | Corpus, budgets, docs and release | All | M | Hosted three-platform CI passes on the exact release candidate |

Sizes are relative (S, M, L), not dates. The Phase 5 plan's rule holds: no calendar
commitment.

## S1 — Close the sheet metal model gaps

An audit of the delivered features against the [scope table](phase-5-plan.md#scope) found
these options missing. Each is one model change with a test, in `of-model/src/sheet_metal.rs`
and `ops/sheet/*`:

| Feature | Missing |
| --- | --- |
| Edge flange | Angle relative to a reference face; trim side bends; an editable profile sketch per edge |
| Flat-Pattern | Merge faces, simplify bends, corner treatment and excluded faces (only fixed face and break corners exist) |
| Hem | Miter gaps where hems on adjacent edges meet |
| Corner relief | 3-bend corners (only pairs of bend lines are found); the add-slit option |
| Closed corner | Open bend region, coplanar faces and narrow corner options |
| Welded corner | Stop point and fillet radius |
| Tab and slot | Start and end references, corner type (sharp, fillet, chamfer), separate slot length and width offsets, through-all slots |
| Sheet metal gusset | Profile by d1/d2 or leg and angle, rounded or flat back, draft, inner and outer fillets |
| Forming tool | Tool configuration; the placed tool follows edits to its part file through the part cache |

Also confirm that gauge tables load from a user CSV path and that bend tables interpolate
over thickness, radius and angle, each with a test (only the built-in tables are tested
now). Anything that cannot be built is listed in `phase-5-status.md` with a reason, not
dropped silently.

## S2 — Desktop sheet metal features

The ribbon already lists Sheet Metal as an unavailable tab
(`toolbars/layout.ts` `unavailableTabs`).

1. **Ribbon tab.** Move Sheet Metal into the available tabs, shown when the part has a
   sheet metal body and always reachable from the ribbon menu. Groups: Flanges (base
   flange/tab, edge, miter, hem, swept, lofted bend), Bends (sketched bend, jog, cross
   break, insert bends, convert, rip, unfold, fold), Corners (relief, closed, welded, break,
   bend notch), Details (tab and slot, gusset, forming tool, normal cut) and Flat (Flatten
   toggle). Icons are drawn here, never copied.
2. **Panels.** One panel per feature in `panels/sheet-metal/`, built on `panels/fields.tsx`
   and registered in `PanelHost.tsx`. Each panel creates and edits its feature through
   `feature.add` and `feature.update`, previews like existing features, and picks edges
   and faces through the viewport selection filters. Shared pieces: a sheet metal
   parameters block (thickness, radius, allowance, relief, gauge, use material) and a
   bend override block.
3. **Gauge picker.** Table and gauge choice with thickness, radius and K-factor shown, from
   a new read method listing built-in tables and their rows.
4. **Flatten toggle.** Unsuppresses or suppresses the Flat-Pattern feature, adding it if
   absent. Shows the flat in the viewport with bend lines (from `doc.sheet_metal` with
   `outline`) drawn as overlay lines.
5. **Feature tree.** A Sheet Metal folder holding each body's Sheet-Metal parameters,
   editable like a feature; the Flat-Pattern feature last, greyed while formed.
6. **Errors.** Each panel shows the feature's error text from the snapshot; failed edits
   keep the last valid state (already true in the core).

Exit: the P5.1 tray acceptance example (base flange, four edge flanges with auto relief,
flatten and re-form, change gauge and see the flat update) is done by hand in the desktop
and scripted in the smoke test (S4).

## S3 — Desktop sheet metal output and drawing panels

1. **Export.** The export dialog offers DXF for parts with sheet metal: bodies, one file per
   body, bend notes and DXF version (the `export` parameters added in P5.5).
2. **Flat views.** The model view panel (`drawing/panels/views.tsx`) gets a Flat pattern
   option for sheet metal parts, with bend lines and bend notes switches.
3. **Tables.** Bend table and punch table in the Tables group (`toolbars/commands.ts`), with
   the view pick, show tags, and the punch origin pick.
4. **Notes.** The property link picker lists the sheet metal links (`{flat_length}` …).

## S4 — Sheet metal acceptance

1. **Corpus** (fixtures generated by a script, checked numerically):
   - Electrical enclosure: base flange, edge and miter flanges, hems, closed corners and
     corner reliefs, a louver array, a cut across a bend. Flat DXF and a drawing with flat
     view, bend table and punch table.
   - Duct transition: round to square lofted bend, bent (12 facets) and formed, the flat
     checked against the analytic development.
   - Converted housing: a shelled box with rips and bends.
   Every flat length is checked against the allowance formula and every hole position
   against hand-computed values. Flat DXF is parsed back and its layers and extents
   checked.
2. **Conformance area 10** in `scripts/catalog-conformance.mjs`: one check per scope row,
   driving the core through JSON-RPC like the existing areas.
3. **Smoke scenario** in `scripts/desktop-smoke.mjs`: base and edge flanges, flatten, flat
   DXF, flat pattern drawing view.
4. **Budgets:** `scripts/phase-5-performance.mjs` and `docs/phase-5-budgets.json` for
   enclosure rebuild, flatten and flat DXF, with `perf:sheet:check` added to CI next to
   the Phase 4 drawing budgets.

## W1 — Weldment foundations and profile library

1. **Profiles prototype (P5.0 leftover).** Generate one profile of each family, sweep it
   along a two-segment path, and check area and second moments against the published
   table values to 0.5 %. Measure sweep and miter times; they set W2's budget.
2. **Profile library.** `of-model/src/profiles.rs` (or a new `of-library` crate shared with
   `of-assembly/src/hardware.rs`, decided by the prototype): ISO EN 10056 angles, EN 10279
   channels, EN 10365 IPE and HEA, EN 10219 hollow sections; ANSI AISC W, C and L, HSS,
   ASME B36.10 pipe. Each profile is a parametric sketch with named pierce points (corners,
   center, centroid). Custom profiles: a part file and a sketch in it.
3. **Weldment feature.** One per part, before the first member. While present, new bodies
   are not merged, and the part carries a cut list folder and weldment properties.
4. **Schema.** Add the weldment, member, trim, cap, gusset and bead feature kinds and the
   `cut_list` and `weld_folder` document fields to part file v6 (still unreleased, so no
   new version), with round-trip tests.
5. **Catalog.** `weldment_profiles` read method and MCP tool listing families and sizes.

## W2 — Structural members

Profile, groups of segments from 2D and 3D sketches, per-group corner treatment (end
miter, end butt 1 and 2, gap, merge miter-trimmed bodies), simple and coped cuts between
connected segments, profile position (pierce point, rotation, mirror, align), and allow
protrusion. Each segment is its own body with a stable `BodyId` from feature and segment
index, so cut list items and drawing references survive sketch edits. Corners are cut with
planar half-spaces at the bisector or butt planes; coped cuts only against the other
member's outer faces. Members record their profile, length and end angles for the cut list.

Tests: the frame acceptance example, a 3D-sketch member, every corner treatment, and a
rebuild after editing the sketch that keeps every body ID.

## W3 — Trim/extend, end caps and gussets

- **Trim/extend:** end trim, end miter, end butt 1 and 2; against a face or plane (flush)
  or bodies (simple or coped); allow extension; weld gap.
- **End cap:** face, thickness direction (outward, inward, inset), thickness, offset as a
  ratio of wall thickness or a distance, chamfered or filleted corners.
- **Gusset:** two adjacent planar faces, triangular or polygonal profile (d1–d4, or leg and
  angle), inner corner chamfer, thickness inner, both or outer, location start, middle or
  end with an offset.

## W4 — Weld beads and weld folder

- **Cosmetic beads:** weld path or from/to faces, smart weld selection (a stroke across
  faces picks the intersecting edges), size, tangent propagation, one or both sides,
  intermittent (length and gap or pitch), staggered, from/to length; a weld symbol per
  bead; drawn as lightweight viewport graphics through the scene.
- **Fillet beads:** separate bodies swept as triangles along the edges (full length,
  intermittent, staggered, other side).
- **Weld folder:** beads with weld material, process, mass per length and total length.

## W5 — Cut lists

Automatic grouping of identical bodies: members by profile, length and end angles; sheet
metal by flat outline (from `flat.rs`); others by volume, surface area, bounding box and
principal moments within tolerance. Manual grouping, exclude from cut list, sort by
profile, item names from properties, editable item property overrides, and sub-weldment
folders. Item properties: length, angles, description, material, mass, quantity, bounding
box; for sheet metal items, the P5.5 properties. Core methods `doc.cut_list` and
`doc.cut_list.set`, with MCP tools.

## W6 — Desktop weldments

A Weldments ribbon tab (Weldment, Structural Member, Trim/Extend, End Cap, Gusset, Weld
Bead, Fillet Bead); a profile picker with the library tree and a section preview; group
editing on the sketch; trim, cap and gusset panels; cut list and weld folders in the
feature tree with a properties panel for items.

## W7 — Weldment drawings

Model views of selected bodies (one member, a sub-weldment, a sheet metal body) with their
own orientation, also used for sheet metal bodies of multibody parts; a cut list table
(item, quantity, description, length, angles, material, any property; splits across sheets
and exports CSV like the Phase 4 tables); balloons linked to cut list item numbers and auto
balloon per item; a weld table from the weld folder; weld symbols attached to bead edges.
Desktop: view body selection and the two new tables in the drawing panels.

## R — Release

1. Machine frame corpus: square and rectangular tube members, mitered and butted corners,
   trims, end caps, gussets, cosmetic and fillet beads, a welded-on sheet metal bracket;
   cut list and a drawing with member view, cut list table, balloons and weld table.
2. Conformance area 11; smoke weldment scenario (members from a sketch, trim, cut list,
   cut list table).
3. Budgets for the frame rebuild (about 40 members) and cut list in
   `docs/phase-5-budgets.json`.
4. Docs: `docs/phase-5-status.md` (delivered modes, limits, decisions, corpus results),
   `docs/protocol.md` (generated parts and the new methods), feature docs 10 and 11
   updated with what is supported.
5. Run every repository gate, the native smoke and `perf:*` locally; then, with the owner's
   go-ahead, push `phase-5`, run hosted CI on macOS, Windows and Linux, record the run in
   the status doc, and merge into `main` once Phase 4 has merged (rebase `phase-5` onto
   `main` first).

## Risks

| Risk | Mitigation |
| --- | --- |
| Desktop work is larger than the model work it exposes (about 25 sheet metal and 8 weldment panels) | Shared parameter, edge-pick and override blocks first; panels are thin over the generated schemas |
| Profile sweeps and miter cuts are slow on 40-member frames | Measure in W1 before committing to W2's design; cache member bodies per segment |
| Cut list grouping tolerance groups unlike bodies or splits identical ones | Exact grouping keys for members and sheet metal; tolerance-based only for other bodies, with manual grouping as the override |
| Phase 4 not merged when Phase 5 is ready | Keep rebasing `phase-5` onto `phase-4`; merge order is Phase 4 then Phase 5 |
