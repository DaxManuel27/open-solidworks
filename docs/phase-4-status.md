# Phase 4 implementation status

Updated 2026-10-15. Built on `phase-4`, branched from `main` after the Phase 3 merge
and the clippy fix that turned hosted CI green. Every milestone in
[the implementation plan](phase-4-plan.md) is implemented and verified, locally and on
hosted macOS, Windows and Linux CI ([below](#hosted-ci)).

## What was built

| Milestone | Delivered | Where |
| --- | --- | --- |
| P4.0 Contracts | `.ofd` v1 schema (sheets, views, annotations anchored through `ViewRef`s, never HLR output); exact `HLRBRep_Algo` hidden-line removal traced to model edges, cached per geometry, direction and accuracy; one display list per sheet; DejaVu Sans Condensed with stroked drafting and GD&T symbols; part file v5 with per-dimension drawing display and names (older files show every dimension) | `crates/of-kernel-sys/cpp/hlr.cpp`, `crates/of-geom/src/hlr.rs`, `crates/of-drawing/src/{document,file,graphics,text}.rs`, `crates/of-model/src/{lib,file}.rs` |
| P4.1 Documents and sheets | New drawing, make drawing from a part or assembly (own window and core), open/save with relative model paths, multiple sheets (add, rename, reorder, delete), ISO A0–A4 and ANSI A–E and custom sizes, ISO 5457/7200 and ASME Y14.1 formats with zones and title blocks, `.ofdt` format files, linked fields (`$PRP`, `$PRPSHEET`, `$SHEET`, `$SCALE`, `$DATE`, `$FILENAME`, material, mass), first/third angle, stale views after a model save; desktop sheet canvas (pan, zoom, picking, snapping, dragging views and annotations), sheet tabs, drawing tree, View Layout / Annotation / Evaluate ribbons | `crates/of-drawing/src/{drawing,formats,model}.rs`, `crates/openforge/src/drawing.rs`, `apps/desktop/src/renderer/src/drawing/` |
| P4.2 Views | Standard 3 view laid out and scaled to fit, model views in every named, custom and relative-to-model orientation, projected views in eight directions, auxiliary views, exploded assembly views with explode lines; scale, alignment and labels per standard; hidden removed/visible, wireframe, shaded with edges, shaded; tangent edges visible, with font, removed; hidden edges and components | `crates/of-drawing/src/{views,viewdraw,layout}.rs` |
| P4.3 Derived views | Full, offset, half, aligned and partial-depth sections with flip, slice, excluded components and hardware left uncut, hatching per material with scale and angle; detail views (circle or polygon, every style, jagged outline); broken-out sections; break views in five styles with true-length dimensions; crop views | `crates/of-drawing/src/{section,clip,hatch,viewdraw}.rs` |
| P4.4 Dimensions and standards | Model items (dimensions marked for drawing once each in the truest view, hole callouts, thread callouts, centerlines and center marks); imported dimensions drive the part (edit, rebuild, save, refused when it would not rebuild); linear, aligned, angular, radius, diameter, arc length, ordinate, baseline, chain, chamfer and angular running dimensions; tolerances none, basic, bilateral, limit, symmetric, MIN, MAX, ISO 286 fits (classes, with limits, limits only), general; precisions, prefix/suffix, below text, text override, reference, inspection, dual units, arrows, foreshortened radii; ISO and ANSI standards with overrides, saved in the drawing and as `.ofstd` files | `crates/of-drawing/src/{dims,fits,standard,annot}.rs` |
| P4.5 Annotations | Notes with links, leaders, jog, border and angle; balloons (item, quantity, property, text; eight shapes; split; stacked) and auto balloons (six layouts, one per BOM row, never overlapping); hole callouts from Hole Wizard data per standard, plain holes by size; center marks (single, linear, circular) and centerlines; cosmetic threads per ISO (¾ arc) and ANSI (hidden circle) with side-view lines; surface finish (ISO 1302), datum features and targets, geometric tolerance frames with all characteristics, modifiers and composite rows, weld symbols (both sides, contour, all around, field, tail); area hatch/fill; revision clouds (rectangle, ellipse, polygon, freehand) and symbols | `crates/of-drawing/src/{annotations,annot,symbols}.rs` |
| P4.6 Tables | BOM (top-level, parts-only, indented; detailed, flat or no numbering; any property columns; start and increment) linked to balloons; hole tables (tags by size or number, combined sizes, positions from a picked origin); revision tables (automatic letters or numbers, zones) feeding the title block revision; general tables; ISO 2768-1 general tolerance tables; splitting across sheets; CSV | `crates/of-drawing/src/tables.rs` |
| P4.7 Output | PDF: vector, true paper size, subset embedded font, layers as optional content, chosen sheets, colour or black and white. DXF: R12 (AC1009) or 2018 (AC1032) with lineweights and true colour, views at true size in model space and each sheet as a paper space layout with a viewport at the sheet scale, one file or one per sheet; print through the PDF; CLI `openforge export x.ofd out.pdf\|dxf` and MCP `export_drawing` | `crates/of-drawing/src/{pdf,dxf,dxf2018}.rs` |
| P4.8 Release | Area 15 conformance, Phase 4 budgets in CI, native desktop drawing scenario, this page | `scripts/{catalog-conformance,phase-4-performance,drawing-smoke}.mjs`, `docs/phase-4-budgets.json` |

Protocol version 5 adds the drawing document kind and `drawing.*` methods (scene version
stays 4). Part windows gain File Properties (custom properties such as the part number,
and which dimensions drawings import) and Make Drawing.

## Verification

Run locally on the release candidate (macOS, Apple M3 Max):

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  (workspace); `openforge generate --check`.
- `of-drawing`: 64 tests — HLR and view frames, every view kind on real parts and
  assemblies, every dimension scheme, tolerance and fit, symbols, tables and their
  values, model items without duplicates in the truest view, driving dimensions,
  cosmetic threads per standard, file round trips, and PDF and DXF (R12 and 2018) parsed
  back. Sheets were also checked by eye: PDFs rendered with MuPDF, DXF audited by ezdxf
  (no errors or fixes) and its layouts and model space rendered.
- `openforge`: every catalog example replays on an example drawing (views, annotations,
  model items, a driven dimension, auto balloons, tables, exports); RPC and CLI tests.
- `pnpm conformance`: 98 of 98 (22 new checks for catalog area 15).
- `pnpm perf:draw:check` (debug build): standard 3 view of a shelled housing 92 ms,
  section view 21 ms, open 63 ms, cold rebuild of a 23-component gearbox sheet 184 ms, PDF
  of both sheets 171 ms, DXF 74 ms; budgets in [`phase-4-budgets.json`](phase-4-budgets.json),
  checked in CI scaled by 4. HLR timings per view are in the plan (P4.0).
- Native desktop smoke: every earlier scenario plus drawings through the real window and
  panels — Make Drawing opens a drawing window, standard views with the title block from
  part properties, model items, driving a dimension (the part file changes), a smart
  dimension, a note with a property link, section and detail views, a revision table
  driving the title block, undo/redo, PDF and DXF export, save/reopen, and an assembly
  drawing with a BOM and auto balloons matching its item numbers.

## Limits of the delivered modes

These are recorded behaviour, not open Phase 4 items:

- Model items import sketch dimensions (distances, lengths, radii, diameters, angles) and
  blind extrusion depths. Other feature parameters (revolve angles, fillet radii, pattern
  spacing, Hole Wizard sizes) are not placed as dimensions; holes get callouts instead.
- Silhouette lines of curved faces carry no model edge, so they cannot be picked for
  dimensions or balloons; their component is still known for hidden-component display.
  Only edges are pickable on the sheet: faces (relative-to-model views) are chosen from a
  list, and points come from edge ends, middles and centers nearest the click.
- Cosmetic threads are drawn in views that see the thread from the side or end-on;
  oblique views omit them.
- Annotation placement avoids the view outline (imported dimensions) and other balloons
  (auto balloon); dimensions are not otherwise de-collided.
- DXF model space holds every view scaled by the sheet scale, so views at the sheet scale
  measure true size there; views at their own scale (details) keep paper proportions.
  Text is written as `TEXT`, curves as lines, arcs and polylines (no splines). R12 has
  no paper space.
- Printing writes the PDF and opens it in the system viewer, so paper size and scale
  match the export; there is no in-app print dialog.
- The desktop draws detail profiles as circles, revision clouds as rectangles, ellipses
  and polygons, and splits tables by rows per piece; polygon details, freehand clouds and
  table continuations on other sheets work through the API and MCP.

## Deferred catalog items

As planned: auto-generated drawings, DrawCompare, alternate position views, 3D drawing view
mode, detached views, blocks, format painter, spell check and find/replace, Excel-based
BOMs, design and family tables, table equations, bend, punch, weld and cut-list tables and
flat pattern views (Phase 5), DWG export (owner decision: no compatible open-source
writer), eDrawings and markups, detailing mode and lightweight drawings, DimXpert and MBD
(Phase 7), and drafting standards other than ISO and ANSI.

## Hosted CI

Passed on macOS, Windows and Linux on 2026-10-15:
[run 38687852412](https://github.com/DaxManuel27/open-solidworks/actions/runs/38687852412)
on commit 4d53943, including `pnpm perf:draw:check` and the native desktop smoke with
the drawing scenario. The first run (38686480880, on 24b149d) failed on Windows only:
the smoke clicked Save while closing a panel still held commands disabled. Its `click`
helper now waits for the button to become available.
