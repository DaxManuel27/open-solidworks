# Phase 5 implementation status

Updated 2026-10-16. Built on `phase-5`, branched from `phase-4`. Work follows
[the completion plan](phase-5-remaining-plan.md); this page is filled in as its steps land
and is final once the release step (R) has run.

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

## Decisions taken during implementation

| Decision | Taken |
| --- | --- |
| Closed corner types | Butt: the selected flange stops `gap` short of the other's inside face, and the other extends across the corner. Overlap and underlap use an overlap ratio (0–1). Only the flange walls close; bend regions keep their miter and relief |
| Welded corners | Cosmetic, like weld beads: the bead (seam, size, symbol, stop point, fillet radius) is recorded and shown; the solid is not changed, so flat patterns are unaffected |
| Flat view orientation | Front of the flat (its back for `Back`); no other orientations or rotation |
| Flat DXF curves | Non-line, non-arc curves are written as polylines at 0.001 mm deflection |
| Flat-Pattern corner treatment | Rounds the outline's inside corners by half the thickness where both sides are long enough |
| Excluded flat faces | The excluded face's region in the flat is its mapped boundary's convex outline (excluded faces are walls) |

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
