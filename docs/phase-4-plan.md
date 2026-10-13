# Phase 4 implementation plan

Status: in progress, 2026-10-13. Built on `phase-4`, created from `main` after the
Phase 3 merge (PR #4; see [Phase 3 status](phase-3-status.md)) and the clippy fix
that turned hosted CI green. The owner decided output and tooling questions on
2026-10-13; see [Decisions](#decisions).

## Goal and starting point

Deliver the drawings scope in [the roadmap](plan.md#scope-and-phases): sheets and
title blocks, standard, projected, section, detail, auxiliary and break views,
model dimensions, annotations, GD&T symbols, BOM, hole and revision tables, and PDF
and DXF export (DWG is deferred; see [Decisions](#decisions)). Every delivered capability must work through the desktop and
the shared core used by JSON-RPC, CLI and MCP. Detailed behaviour comes from
[the drawings spec](features/15-drawings-and-detailing.md).

The Phase 3 baseline provides primitives worth retaining:

- Part (`.ofp` v4) and assembly (`.ofa` v1) documents, a core document enum, a part
  cache keyed by path, content hash and configuration, and reload when a referenced
  file changes. A drawing is a third document kind that references both.
- Stable, body-qualified face and edge names, and `CompRef` paths into assemblies,
  so annotations can attach to model geometry and survive rebuilds.
- Hole Wizard data (kind, standard, size, fit, counterbore/countersink dimensions)
  and cosmetic thread metadata kept for this phase (see the Phase 2 plan).
- Part custom properties with links to mass, material and configuration, and the
  top-level, parts-only and indented BOM data from `of-assembly`.
- Exploded view steps per configuration.
- A generated tool catalog, MCP schemas and the `pnpm conformance` harness.

Gaps that shape this phase:

- Nothing in the codebase produces 2D output. There is no hidden-line removal, no
  2D geometry type, no text layout or font handling, and the kernel bridge does not
  link `TKHLR`.
- The desktop renders only 3D (three.js). Drawings need a 2D sheet canvas with
  paper-space units, pan/zoom, snapping and 2D picking.
- Sketch dimensions and feature parameters have no "show in drawing" flag or
  stable display names that a drawing could import.
- There is no PDF or DXF writer.

**Prerequisite (done).** Hosted CI on `main` failed after the Phase 3 merge because
clippy 1.99 reports `clone_on_copy` in cxx-generated code for `StepNode` in
`crates/of-kernel-sys`. Commit 229da3b on `main` allows it, as was already done for
`MassProps`; the hosted run is recorded in `phase-3-status.md`.

## Delivery rules

The Phase 3 rules carry over unchanged. Each milestone is a vertical slice: kernel,
model, references, protocol, desktop creation/editing, persistence and regression
tests. Keep the crate dependency direction; keep all C++ and `unsafe` in `*-sys`
crates. Internal model units remain millimetres and radians; sheet coordinates are
millimetres of paper. Failed operations keep the last valid state and report
actionable errors. Record unsupported variants explicitly; do not mark Phase 4
complete while a required item is blocked.

One rule specific to this phase: **the core produces a single 2D display list per
sheet** (lines, arcs, splines, polylines, text runs, fills, with layer, line style
and weight). The desktop canvas, PDF, DXF and print all render that same list, so
the screen and every output agree and headless CLI/MCP export needs no desktop.

## Scope

Required in this phase (roadmap items plus what they cannot work without):

| Area | Required |
| --- | --- |
| Documents | Drawing file (`.ofd`), new drawing, make drawing from part or assembly, open/save, multiple sheets, missing-model handling, update when a referenced model changes |
| Sheets | Standard sizes (ISO A0–A4, ANSI A–E) and custom, sheet formats with border, zones and title block, linked fields (model properties, sheet properties, system values), sheet scale, first/third angle projection, view and datum label sequences |
| Views | Standard 3 view, model view (standard and named orientations), projected (eight directions), auxiliary, relative to model, exploded view; view scale, alignment and labels; display styles (hidden lines visible/removed, wireframe, shaded with edges), tangent edge modes, hide/show edges and components |
| Derived views | Section (full, half, offset, aligned, partial depth) with material hatching and exclusions, detail (circle or profile; standard, broken, leader, no leader, connected), broken-out section, break view (all line styles, true-length dimensions), crop view |
| Dimensions | Model items (dimensions marked for drawing, hole callouts, cosmetic threads, reference geometry), smart dimensions (linear, angular, radial, diameter, arc length), ordinate, baseline, chain, chamfer and angular running dimensions; tolerance types, precision, ISO hole/shaft fits, dual units, prefix/suffix text, inspection dimensions; editing an imported model dimension drives the model |
| Annotations | Notes with property links and leaders, balloons (styles, auto balloon, stacked), hole callouts from Hole Wizard data, center marks and centerlines (manual and auto-insert), cosmetic thread display per standard, surface finish, datum feature, datum target, geometric tolerance frames (all characteristics and modifiers, composite), weld symbols, area hatch/fill, revision clouds and symbols |
| Standards | Drafting standards ISO and ANSI (fonts, arrows, tolerance display, view labels, thread display) as editable, saveable document options; layers and line formats |
| Tables | BOM table (top-level, parts-only, indented with numbering; columns from properties; configurations; linked to balloons), hole table, revision table, general table, general tolerance table (ISO 2768), title block fields fed by tables |
| Output | PDF (vector, embedded fonts, selected sheets, colour or black and white), print, DXF (one file per sheet or all sheets, layers, true-scale model space) |

Deferred to later phases or extensions, recorded in the status document:
auto-generate drawings, DrawCompare, alternate position views, 3D drawing view mode,
detached views, blocks (`.sldblk` equivalent), format painter, spell check and
find/replace, Excel-based BOMs, design and family tables in drawings, table
equations, bend, punch, weld and cut-list tables and flat pattern views (Phase 5),
DWG export, eDrawings and markups, detailing mode and lightweight drawings, DimXpert and MBD
(Phase 7), and drafting standards other than ISO and ANSI (DIN, JIS, BSI, GOST and
GB are option presets that can follow without schema changes).

## Decisions

Decided by the owner on 2026-10-13 (the font and HLR method on the recommendation
given with the plan):

| Decision | Choice | Why |
| --- | --- | --- |
| DWG export | DXF only for now. DWG is deferred | No open-source DWG writer is compatible with Apache-2.0/MIT: LibreDWG is GPL-3.0 and the ODA SDK is proprietary. DXF is readable by every tool that reads DWG |
| PDF writer | Native Rust (`pdf-writer`, MIT/Apache) from the sheet display list | Works headless from CLI/MCP; vector output with embedded fonts; avoids Chromium print differences between platforms |
| Drafting font | DejaVu Sans Condensed (Bitstream Vera licence, permissive) for text; drafting and GD&T symbols drawn as OpenForge strokes, since no permissive font has them all | Output must be identical on every platform and embeddable in PDF/DXF |
| Hidden-line removal | OCCT exact `HLRBRep_Algo` for final output and polygonal `HLRBRep_PolyAlgo` for interactive preview if exact misses the budget, settled by P4.0 measurements | Exact HLR is the main performance risk of this phase |
| Driving dimensions | Editing an imported model dimension changes the part through the part's own document and saves it, reloading dependants as Phase 3 does | Matches the spec's associativity without a second editing path |

## Milestones and dependencies

| Milestone | Deliverable | Depends on | Acceptance example |
| --- | --- | --- | --- |
| P4.0 | Contracts, schema, HLR and output prototypes | `main` CI green | HLR of the Phase 3 gearbox housing meets its budget; `.ofd` v1 schema and fixtures; a test sheet round-trips to PDF and DXF |
| P4.1 | Drawing documents, sheets, formats, title blocks and the 2D canvas | P4.0 | New A3 ISO drawing of a bracket; title block shows its part number and mass; change the material, the mass updates |
| P4.2 | Standard, model, projected, auxiliary and exploded views | P4.1 | Standard 3 view plus isometric of the bracket in first and third angle; edit the part, views update |
| P4.3 | Section, detail, broken-out, break and crop views | P4.2 | Offset section through the gearbox with fasteners and shafts excluded and per-material hatching; 3:1 detail of a groove |
| P4.4 | Dimensions, tolerances and drafting standards | P4.2 | Import model dimensions into the bracket views without duplicates; change one on the drawing, the part rebuilds; add an H7/g6 fit |
| P4.5 | Annotations and symbols | P4.2, P4.4 | Hole callouts read `⌀6.6 THRU ⌴⌀11 ↧6.4`; position tolerance frame referencing datums A and B; cosmetic threads drawn per ISO |
| P4.6 | Tables | P4.1, P4.5 | Indented BOM of the gearbox with auto balloons matching item numbers; hole table of the flange; revision table driving the title block revision |
| P4.7 | PDF, print and DXF output | P4.1–P4.6 | Two-sheet gearbox drawing exported as PDF and DXF; DXF reopens in a third-party viewer at true scale with layers intact |
| P4.8 | Integrated corpus, parity, performance and release gate | All above | Hosted three-platform checks pass on the exact release candidate |

P4.3, P4.4 and P4.6 are independent once their prerequisites pass. P4.7's writers
can start in P4.0 against the display list and grow with each milestone. No
calendar commitment yet: estimate after P4.0 measures HLR cost.

## P4.0 — Contracts and decisions

**Drawing schema.** A version 1 `.ofd` JSON document with the same header and
version dispatch as `.ofp` and `.ofa`:

- `models`: referenced parts and assemblies (path relative to the drawing, last
  content hash, last known absolute path), reusing the Phase 3 resolution rules.
- `sheets`: stable `SheetId`, name, size, orientation, scale, projection, sheet
  format (embedded or referenced `.ofdt` template), zones.
- `views`: stable `ViewId`, kind and parameters, model and configuration, parent
  view (projected, auxiliary, section, detail), orientation as a rotation (never a
  general matrix), scale (sheet or custom), position on sheet, alignment
  constraint, display style, hidden edges and components, label.
- `annotations`, `dimensions`, `tables`: each anchored either to sheet coordinates
  or to model geometry through `ViewRef { view: ViewId, entity: CompRef }` (a part
  view uses an empty component path). Anchors never store HLR edge indices.
- `standard`: the drafting standard and its overrides; `layers` and `line_styles`.

**Model metadata.** Add a per-dimension "show in drawing" flag and display name to
sketch dimensions and feature parameters in `of-model`, with a part file version
bump (v5) and migration that marks existing driving dimensions as shown.

**HLR prototype.** Link `TKHLR` and wrap `HLRBRep_Algo` and `HLRBRep_PolyAlgo` in
`of-kernel-sys`, returning visible/hidden, sharp/smooth (tangent)/outline edges as
2D curves tagged with their source edge name, so dimensions can attach to them.
Measure on the Phase 3 corpus (bracket, flange, gearbox housing, full gearbox) and
record results:

| Target | Measure |
| --- | --- |
| Correctness | Visible/hidden classification against hand-checked fixtures, including silhouettes of cylinders and tori |
| Traceability | Every output curve maps back to a model edge name or is marked silhouette |
| Speed | Single part view and full gearbox view, debug and release; preview (interactive) and final quality |
| Caching | Results keyed by (model cache key, configuration, orientation, quality), so moving a view never recomputes it |

**Section prototype.** Cut the shape with a half-space (`BRepAlgoAPI_Cut`), take the
cut faces for hatching, and run HLR on the remainder; confirm aligned and offset
sections compose from the same primitive.

**Output prototypes.** Write a test display list (lines, arcs, splines, text with
symbols, hatch fills, three layers, two weights) to PDF and to DXF (AC1009 R12 and
AC1032 2018), and check both in at least two independent viewers.

## P4.1 — Drawing documents, sheets and the 2D canvas

Add an `of-drawing` crate (`openforge` → `of-drawing` → `of-assembly`, `of-model`).
It owns the document, sheets, views, annotation anchors, the display list builder
and the PDF/DXF writers. The core document enum gains a drawing kind; `core.info`
reports it so older clients refuse drawings.

Sheet formats are `.ofdt` templates: border, zones, title block geometry and
linked fields (`$PRP:"Description"`, `$PRPSHEET:"PartNo"`, `$SHEET`, `$SCALE`,
`$DATE`, `$FILENAME`). Ship ISO and ANSI templates for every standard size.
Properties come from the model of the view chosen in sheet properties.

Desktop: New Drawing, Make Drawing from Part/Assembly (from the part or assembly
window), open/save `.ofd` in its own window and core process, a Drawing ribbon
tab, a sheet tab bar (add, rename, reorder, delete), a drawing tree (sheets,
sheet format, views, tables), and a 2D canvas that renders the display list with
pan, zoom to sheet, snapping and picking of curves, annotations and views. Saving a
part or assembly reloads the drawings open in other windows.

## P4.2 — Standard and projected views

Standard 3 view, model view (standard orientations and part/assembly named
views), projected views in eight directions, auxiliary views normal to a picked
edge, relative-to-model views from two faces, and exploded views using the Phase 3
explode steps and explode lines. Projected and auxiliary views stay aligned with
their parent unless alignment is broken. View labels and scale text follow the
drafting standard.

Display: hidden lines visible/removed, wireframe, shaded with edges (shaded fill as
a raster or flat-filled faces, decided in P4.0 by output quality), tangent edges
visible, with font or removed, per-edge hide/show and per-component hide. Views
show a "needs update" state when their model changed and update on rebuild;
dangling anchors are listed, not dropped.

## P4.3 — Section, detail, break and crop views

Section views from a cutting line: horizontal, vertical, auxiliary and aligned;
full, half (eight layouts), offset (jogged) and partial-depth sections; flip;
exclude ribs and selected components (fasteners from the hardware library are
excluded by default); slice mode; per-body hatching from a material hatch map
(steel, aluminium, plastic, generic) with scale and angle overrides. Detail views
from a circle or closed profile with every connection style and a jagged outline.
Broken-out sections cut a closed profile to a depth or up to an edge. Break views
with all line styles, where dimensions across a break show true length. Crop views
trim to a closed profile.

## P4.4 — Dimensions, tolerances and drafting standards

Model items imports dimensions marked for drawing, hole callouts, cosmetic thread
callouts and reference geometry into chosen views, eliminating duplicates and
placing each dimension in the view where it is shown truest. Imported dimensions
are driving: editing one updates the model through its document. Smart dimensions
on view geometry are driven (reference) dimensions: linear, aligned, angular,
radial, diameter, arc length, and ordinate, baseline, chain, chamfer and angular
running schemes.

Dimension properties: tolerance types (none, basic, bilateral, limit, symmetric,
min, max, fit, fit with tolerance, fit tolerance only, general), separate value and
tolerance precision, ISO 286 hole and shaft fits, dual units, prefix and suffix
text, parentheses, inspection, foreshortened, arrow and leader style. ISO and ANSI
drafting standards control text height, arrows, extension line gaps, tolerance
display and leading zeros, saved as document options and as reusable files.

## P4.5 — Annotations and symbols

Notes (multi-line, property links, leaders with jog, borders), balloons (item,
quantity, property or text; circle, triangle, hexagon, box, diamond, split circle,
flag), auto balloon with layouts and ignore-multiple-instances, stacked balloons.
Hole callouts read Hole Wizard data per standard and degrade gracefully for
plain cylindrical cuts. Center marks (single, linear, circular pattern) and
centerlines, manually and by auto-insert per view. Cosmetic threads drawn per
standard: ISO three-quarter arc in end views, ANSI full circle, thin lines in side
views, with callout text.

Symbols: surface finish (ISO 1302 / ASME Y14.36), datum feature, datum target,
geometric tolerance frames with every characteristic, zone diameter, modifiers and
composite frames (ISO 1101 / ASME Y14.5), weld symbols (ISO 2553 / AWS A2.4) as 2D
annotations. Area hatch/fill in closed regions with the hatch library. Revision
clouds (rectangle, ellipse, polygon, freehand) with revision symbols linked to the
revision table. Every annotation can anchor to view geometry or the sheet and
moves with its view.

## P4.6 — Tables

BOM tables read the Phase 3 BOM data: top-level, parts-only or indented with none,
detailed or flat numbering; columns from item number, part number, description,
quantity, material, mass and any property; configurations to list; item number
start and increment; excluded components left out. Balloons read the BOM item
number and update when the BOM changes.

Hole tables from a datum origin and picked faces: tag, X, Y, size (from Hole Wizard
data), combine same sizes, alphabetic or numeric tags. Revision tables: revision,
description, date, approved, automatic letters or numbers, zone, linked revision
symbols, and the current revision feeding `$PRP:"Revision"` in the title block.
General tables with free cells; general tolerance table per ISO 2768 (f, m, c, v).
Tables split across sheets and export as CSV.

## P4.7 — Output

PDF: vector output of selected sheets at true paper size, embedded font subset,
line weights per standard, colour or black and white, layers as optional content
groups. Print through the desktop using the PDF output. DXF: AC1009 (R12) and
AC1032 (2018), one file per sheet or all sheets in one, layers and line types, text
as `TEXT`/`MTEXT` with the bundled font named, splines as splines or polylines,
model-space content at 1:1 with the sheet in paper space. CLI `openforge export
drawing.ofd out.pdf|out.dxf` and MCP tools use the same path.

## Code integration map

| Area | Primary files and intended changes |
| --- | --- |
| Kernel | `of-kernel-sys` (link `TKHLR`; HLR wrappers returning tagged 2D curves; section cut faces) and `of-geom` (safe HLR and section API, result cache) |
| Parts | `crates/of-model`: dimension "show in drawing" flag and display names, part file v5 with migration; hole and thread callout text helpers |
| Assemblies | `crates/of-assembly`: BOM item numbering exposed for balloons; explode state for drawing views |
| Drawing model | New `crates/of-drawing`: document, file I/O, sheets and templates, views, anchors, dimension/annotation/table model, display list builder, PDF and DXF writers |
| Core | `crates/openforge/src/{session.rs,views.rs,tools/}` and `of-protocol`: drawing document kind, drawing methods, sheet display list protocol (v5), tool catalog and MCP schemas, CLI `info`/`export` for `.ofd` |
| Desktop | `main/index.ts` (drawing windows), `store.ts`, `protocol.ts`, a new 2D sheet canvas (picking, snapping, drag), drawing tree, Drawing ribbon and Annotation tab, property panels for views, dimensions, annotations and tables |
| Assets | ISO and ANSI sheet templates, drafting standard files, bundled font, hatch library |
| Tests/docs | HLR fixtures, display-list golden files, PDF/DXF structural checks, serve tests, conformance checks for area 15, desktop smoke scenario, `docs/phase-4-status.md`, `docs/protocol.md` |

## P4.8 — Acceptance and release

Every behaviour change gets a regression test at the lowest useful layer. HLR and
section outputs have golden fixtures with tolerance comparison; the display list
has golden files per view type and annotation; PDF and DXF outputs are parsed back
and checked structurally (entities, layers, text, scale). Build a reproducible
drawing corpus from the Phase 3 corpus:

- Bracket part drawing: standard 3 view, isometric, imported model dimensions,
  tolerances and fits, hole callouts, surface finish, title block from properties.
- Flange drawing: hole table, datum features, position tolerance, cosmetic threads.
- Shaft drawing: break view, detail view of a groove, chamfer and diameter
  dimensions, general tolerance table.
- Gearbox assembly drawing, two sheets: offset section with exclusions and material
  hatching, exploded isometric, indented BOM with auto balloons, revision table.

Extend the native desktop smoke test with make drawing, add views, a section and a
detail, import model items, edit a driving dimension, auto balloon and BOM, PDF and
DXF export, with save/reopen and undo/redo throughout. Add catalog conformance
checks for area 15. Set performance budgets in `docs/phase-4-budgets.json` (open,
standard 3 view, section view, full gearbox sheet rebuild, PDF and DXF export)
using the existing `perf` tooling, and check them in CI.

Run the repository gates, the native smoke test and hosted macOS, Windows and
Linux workflows on the release candidate. Publish supported modes and remaining
limits in `phase-4-status.md`. Merge `phase-4` to `main` only when the required
scope and hosted checks are complete.
