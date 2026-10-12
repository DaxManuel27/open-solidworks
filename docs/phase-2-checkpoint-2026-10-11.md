# Phase 2 implementation checkpoint — 2026-10-11

Phase 2 remains open. This checkpoint implements the next reusable foundations
and several tested feature modes without changing the original acceptance scope.
Existing assistant work is preserved; its offline compatibility checks remain in
the verification suite.

## References, profiles and bodies

- A profile is either its legacy sketch ID or an explicit contour with persistent
  member entity IDs, reversal and a start entity. Loft preserves explicit section
  correspondence; the measured square loft changes from 1000 to 666.6667 mm³ when
  one section's seam advances one edge. Missing/replaced members require reselection.
- Composite/helix/projected paths expose source-derived segment references.
  Ambiguous generated pieces have no selectable segment identity. Replacing a
  source never silently redirects a saved segment. Desktop picks and dropdowns use
  those references; legacy whole-curve inputs retain their behavior.
- General splits accept multiple planes, bounded faces and body tools, with
  arbitrary retained-piece selection. Identity uses oriented source/tool ancestry,
  not solid enumeration. Indistinguishable ancestry fails explicitly. Tools remain
  in the part. [Split contract and regression details](phase-2-r1-split.md).
- Previews use the next real feature ID without consuming it, so a retained piece
  selected in preview has the same ID on commit. Revision checks reject stale work.
- Files, protocol and scenes now use version 3. Files from versions 1 and 2 retain
  equivalent defaults. Older readers reject version 3 before decoding features,
  preventing them from interpreting a tool split as an old plane-side split.

## Parameter editing and configurations

Existing feature panels offer **Edit feature → Parameters and formulas**. Choose
**Base** or **Active configuration**, see authored formulas and evaluated values,
and reset overrides to their inherited values. Preview/cancel and commit share core
evaluation. Stale writes and invalid expressions are rejected. Structural definition
edits still affect the shared tree; feature creation remains numeric.

Registered fields include variable-fillet stations, projection direction components
and 3D sketch distance constraints keyed by constraint ID. Tests cover units, integer
targets, cycles/missing parameters, failed variants, undo, A→B→A, and full/incremental
equivalence. Removing the last equation also invalidates earlier dependent geometry.
Formula/configuration changes retain a conservative full rebuild.

## Spatial sketches

**Sketch → 3D Sketch** opens the independent spatial editor. It supports points,
lines, cubic Beziers and construction entities. Draw on XY/XZ/YZ workplanes at a
chosen depth, snap to points/grid, enter XYZ values, or drag points/axis handles.
Alt-drag/middle-button navigation remains available. Dimensions appear in the viewport
and can be edited there or in the panel.

Constraints include fixed/coincident points, distance, axis alignment,
parallel/perpendicular lines, point-on-plane, point-on-curve and endpoint tangency.
Tangency checks endpoint sense and rejects a backtracking cusp. Solver diagnostics
report geometric DOF, conflicts and redundant relations; a stationary spline's
auxiliary parameter is not counted as geometric freedom. Failed solves preserve the
supplied geometry. Pending solves disable commit and stale replies cannot overwrite
later edits or reopen a cancelled editor.

Whole connected paths and persistent entity references feed sweeps/composites.
Branches, disconnected paths and failed source sketches produce explicit errors.
Distance formulas/configurations rebuild downstream geometry. Regressions cover
0.001–100000 mm dimensions, conflicts, dragging, full/incremental/preview equality,
suppression, rollback, undo/reload and STEP/STL export.

Workplanes and point-on-plane constraints currently use explicit world coordinates.
Associative external geometry/workplanes and richer spatial contour selection remain
open. Nonlinear singularities may fail with diagnostics rather than finding another
solution; the editor retains repairable geometry.

## Added solid and annotation modes

**Hollow sweeps:** select an explicit closed outer profile, then inner contours from
the same sketch. Containment, crossing/nesting and swept containment are checked.
Outer/inner sweeps share path/frame semantics and are combined with topology history.
Straight and curved tube volumes, inner-face persistence, annular cuts, preview,
undo and reopen are tested. This does not add thin/open-profile or guide modes.

**Circular dome:** select a complete planar disk face and a height no greater than
its radius. An exact spherical cap adds material outward or removes fully supported
material inward. Tests measure cap attachment and analytic volume across scales and
orientations, references, scope, formulas/configurations, downstream fillet/cut and
exports. Noncircular/holed faces and tangent-boundary controls are rejected/unavailable.

**Cosmetic thread:** select a complete cylinder bounded by two circular rims. Explicit
diameter must match the cylinder; pitch, length, start offset/angle, direction and
handedness control a lightweight helix. Internal/external selection is validated.
Metadata follows supported body movement and diagnoses changed references. It does
not alter mass, solid topology or exported geometry. Interrupted cylinders and more
than 500 displayed turns are rejected. Modeled thread solids and tapped-hole metadata
integration remain open; no standard thread profile is implied.

## Remaining phase acceptance

R0's general boundary, face-blend, wrap and exact guided-loft algorithms still need
measured solutions; the existing coarse guided-loft prototype fails its fit tolerance.
R1 still needs broader imported/pattern/reference-change qualification. R2 still needs
creation-time formula controls and proven dependency optimization. R3's associative
references and broader qualification remain open. R4 still needs variable-pitch and
intersection/silhouette curves, circular/thin/twisted/guided sweeps, advanced closed
path seams, guided/centerline/continuous/closed lofts and loft holes. R5 still needs rib,
parting-line/step draft and broader dome controls. R6 face-set blends/variable chains,
R7 boundary/wrap, and R8 modeled threads are not delivered here.

R9 requires the full integrated corpus, measured budgets including expensive guide
and modeled-thread cases, peak memory, and successful hosted macOS/Windows/Linux checks
on one release candidate. Local verification is not a replacement for that gate.
No merge into `main`, release or AI-track resumption is claimed by this checkpoint.
