# Phase 5 implementation status

Updated 2026-10-17. Built on `phase-5`, branched from `phase-4`. Work followed
[the completion plan](phase-5-remaining-plan.md). Every step through R4 is done and verified
locally; the release push, hosted CI and the merge into `main` wait for the owner (see
[Release](#release)).

## Sheet metal: delivered and verified

| Step | Delivered | Evidence |
| --- | --- | --- |
| Model (P5.0–P5.4, S1) | Every feature in the plan's scope table, with the options listed there except the limits below | `of-model/tests/sheet_*.rs` (flat lengths against the allowance formula, exact volumes for corners, tabs, gussets and flat options) |
| Output and drawings (P5.5) | Flat outline and cut list properties; flat DXF (R12, 2018); `doc.sheet_metal`, `sheet_metal.gauges`; flat pattern views with bend lines and notes; bend and punch tables; property links; part file v6 | `of-model/tests/sheet_flat.rs`, `of-drawing/tests/flat.rs`, `openforge/tests/sheet_metal.rs` |
| Desktop (S2, S3) | Sheet Metal tab, a panel per feature, Flatten toggle with bend lines in the viewport, Sheet Metal tree folder, gauge picker, flat DXF export options, flat pattern view option, bend and punch tables | `scripts/sheet-metal-smoke.mjs` in the native desktop smoke |
| Acceptance (S4) | Corpus: enclosure (flanges, hems, closed corner, reliefs, louvers, cut across a bend), duct transition (bent and formed), converted housing; conformance area 10 (28 checks); budgets | `of-drawing/tests/sheet_corpus.rs`, `scripts/catalog-conformance.mjs`, `scripts/phase-5-performance.mjs`, `docs/phase-5-budgets.json` |

Bugs found by the acceptance work and fixed: a closed profile on a plane coplanar with
a sheet's thickness face was taken as a tab of that sheet; an up-to-edge-and-merge flange
left two sheet metal records for the merged body; keyboard tab order skipped the new tab.

## Weldments: delivered and verified

| Step | Delivered | Evidence |
| --- | --- | --- |
| Foundations (W1) | Weldment feature (one per part); ISO and ANSI profile tables generated in code with outlines, area, centroid and second moments; custom profiles from a part's sketch; `weldment.profiles` | `of-model/tests/weldment.rs`, profile property checks in `of-model/src/weldment/profiles.rs` |
| Members (W2) | Groups of 2D and 3D sketch lines; end miter, end butt 1 and 2, gap, merged miter bodies; pierce point, offset, rotation, mirror, alignment; allow protrusion; connected segments overlapping, simple or coped; stable body IDs across sketch edits | `of-model/tests/weldment.rs` |
| Details (W3, W4) | Trim/extend (faces or planes, bodies simple or coped, miter and butt pairs, extension, gap); end caps; gussets (triangle, polygon, leg and angle); cosmetic beads (edges, from/to faces, smart selection, sides, intermittent, staggered, from/to length, symbol) and modeled fillet beads; weld folder | `of-model/tests/weldment.rs` |
| Cut lists (W5) | Automatic grouping of members, sheet metal and other bodies; manual groups, exclusion, overrides, sub-weldment folders, sort, collect identical off, item names from properties; `doc.cut_list`, `doc.cut_list.set`; saved with the part | `of-model/tests/weldment.rs`, conformance area 11 |
| Desktop (W6) | Weldments tab, a panel per feature with a profile picker and section preview, Cut List and Weld Folder in the tree, Cut List panel, beads in the viewport | `scripts/weldment-smoke.mjs` in the native desktop smoke |
| Drawings (W7) | Views of chosen bodies; cut list and weld tables (CSV through `drawing.table.csv`); balloons numbered by cut list item and auto balloons per item; weld symbols filled from a linked bead; `drawing.model.bodies`; desktop view and table panels | `of-drawing/tests/weldment.rs` |
| Acceptance (R) | Machine frame corpus; conformance area 11 (20 checks); frame and cut list budgets | `of-drawing/tests/frame_corpus.rs`, `scripts/catalog-conformance.mjs`, `docs/phase-5-budgets.json` |

The machine frame corpus is a mitered SHS 40 base rectangle at z = 0 with two RHS 60 × 40
cross members trimmed to its sides, a butted SHS 40 rectangle at z = 500, four legs from a
3D sketch trimmed between the rectangles' faces, an SHS 20 outrigger trimmed to the base's
outer side with an end cap and a 100 mm gusset, three cosmetic beads and a modeled fillet
bead on its seam, and a 3 mm sheet metal bracket. Its cut list matches the hand calculation
item for item: mitered 1240 × 2 and 640 × 2; cross members 1160 × 2; butted 1240, 600, 1200 and
560; legs 460 × 4; outrigger 180; the bracket's 100 × 40 blank with a 280 mm cutting length;
the cap and gusset by mass. The weld folder totals 48 mm. Its drawing has the outrigger
alone at true length, both tables, a cut list CSV of 12 items and one balloon per item the
front view shows (the base rectangle and legs are hidden under the top rectangle).

Bugs found by the weldment work and fixed: trimmed bodies shared face names, so cut list
items lost their body IDs on refresh (trim faces are now named per member); a select
element was read before its options loaded in the smoke (the smoke now waits).

## Decisions taken during implementation

| Decision | Taken |
| --- | --- |
| Closed corner types | Butt: the selected flange stops `gap` short of the other's inside face, and the other extends across the corner. Overlap and underlap use an overlap ratio (0–1). Only the flange walls close; bend regions keep their miter and relief |
| Welded corners | Cosmetic, like weld beads: the bead (seam, size, symbol, stop point, fillet radius) is recorded and shown; the solid is not changed, so flat patterns are unaffected |
| Flat view orientation | Front of the flat (its back for `Back`); no other orientations or rotation |
| Flat DXF curves | Non-line, non-arc curves are written as polylines at 0.001 mm deflection |
| Flat-Pattern corner treatment | Rounds the outline's inside corners by half the thickness where both sides are long enough |
| Excluded flat faces | The excluded face's region in the flat is its mapped boundary's convex outline (excluded faces are walls) |
| Weld beads | Cosmetic by default, as in the plan. Modeled fillet beads are triangular prisms along straight seams, as their own bodies; they are not offered on both sides at once (a second bead models the other side) |
| Weld mass | Modeled beads by their volume and the part's density; cosmetic beads by their mass per length |
| Smart weld selection | The faces a joint crosses are picked (no stroke); every seam where two of them, on different bodies, meet is welded |
| Allow protrusion | Off, butted members stop at the sketch's corner point instead of covering the other member |
| Connected segments | Default leaves members overlapping; simple and coped cuts are opt in, so existing parts don't change |
| Cut list item names | `Item <n>` unless the settings give a `{PROPERTY}` template |
| Auto balloons on a weldment view | One per cut list item the view draws; items hidden in that view get none |

## Limits of the delivered modes

Recorded behaviour, not open items:

- Edge flange **trim side bends** is not offered: it needs an offset of the existing sheet
  to leave a clearance, which the kernel wrapper does not expose. Flanges that overlap an
  existing bend fuse with it.
- Closed corner **open bend region** off and **narrow corner** are not offered: only the
  flange walls are closed.
- Sheet metal gussets are solid wedges in the bend's inside (legs, angle, width, flat or
  rounded back); **draft** and **inner/outer fillets** of a pressed gusset are not
  offered.
- Forming tools from part files rebuild when their file changes (checked on each core
  snapshot by content hash); paths are as given, not relative to the part.
- Lofted bends (bent and formed) are strips of flat facets hinged without radius. Their flat
  develops the profiles' side of the sheet exactly; the K-factor is not applied at the
  hinges, so the flat differs from a neutral-surface development by about ½ t × the total
  turn × the slant height (1.9 % more material on the corpus duct, whose profiles are its
  outside).
- A part's flat pattern view takes its frame from its first flattened body.
- The **Structure System** (primary and secondary members, corner management, connection
  elements) is deferred, as the plan decided.
- Members take the part's material; a profile file's material is not transferred.
- Gusset profiles are flat plates; weld cost is not recorded; sub-weldments can't be saved
  as their own part; relative views of bodies are not offered.
- Modeled fillet beads need straight seams between planar faces.

## Verification

Run locally on 2026-10-17 on `phase-5` (Apple M3 Max, macOS, debug builds):
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (all crates),
`openforge generate` current, `pnpm lint`, `format:check`, `typecheck`, `build`, the native
desktop smoke (including the sheet metal and weldment scenarios), catalog conformance
(146 of 146), and `perf:check`, `perf:asm:check`, `perf:draw:check` and `perf:phase5:check`
(all within budgets).

## Release

Not done yet, by the branching rule: `phase-5` is local. With the owner's go-ahead: push
`phase-5`, run hosted CI on macOS, Windows and Linux, record the run here, then rebase onto
`main` once Phase 4 has merged and merge.
