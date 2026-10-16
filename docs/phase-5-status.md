# Phase 5 implementation status

Updated 2026-10-16. Built on `phase-5`, branched from `phase-4`. Work follows
[the completion plan](phase-5-remaining-plan.md); this page is filled in as its steps land
and is final once the release step (R) has run.

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
- A part's flat pattern view takes its frame from its first flattened body.
