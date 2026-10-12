# R5 circular spherical dome

Implementation checkpoint, 2026-10-11. This delivers the circular spherical-cap
domain established by the R0 prototype. R5 and Phase 2 remain open for their other
required features and unsupported dome domains.

## Inputs and supported geometry

The shared feature payload is:

```json
{
  "type": "dome",
  "face": { "type": "body_topology", "body": "<body ID>", "kind": "face", "name": "<face name>" },
  "height": 3.0,
  "inward": false
}
```

The selected face must be a complete planar circular disk with one boundary and no
holes. One circular edge or multiple arcs of the same circle are accepted. The
base radius follows the selected face; it is not an independent dimension.
`height` is measured perpendicular to that face and must be positive, exceed the
face tolerance and be no greater than its radius. `inward` defaults to false.

An outward dome adds an attached spherical cap. An inward dome removes the same
cap and requires supporting material beneath its entire volume. The operation
rejects intersections with other material, removal beyond the support, changes
in the number of solids and invalid results. It supports selected body scope and
preserves unselected bodies. Invalid features leave the preceding valid model
available for repair.

A face shared topologically by two split solids needs a single owning body in
Feature scope. The aggregate face cannot express both opposing local normals;
unscoped use reports ambiguity instead of cutting the wrong side.

The height participates in equations and configuration overrides as
`f<feature ID>.height`. The desktop Dome panel provides viewport face selection,
a planar-face list with body-qualified references, height with document units,
direction, scope, previews, cancellation, editing and the shared parameter/formula
editor. Its help states the supported domain.

Noncircular and holed faces, general freeform domes and authored tangent/curvature
blend controls are not implemented. No control claims those effects. A spherical
cap's rim is positional attachment; its tangent relationship is the one implied by
the chosen radius and height.

## Kernel construction and naming

Installed OpenCASCADE 7.9.3 headers for `BRepPrimAPI_MakeSphere` were inspected.
The R0 prototype intersects a sphere with a half-space-sized box and attaches the
result to a cylinder. The production implementation uses the documented spherical
segment constructor for the same exact analytic cap, avoiding a fixture-specific
clipping box.

For circular base radius `a` and height `h`, the sphere radius is
`R = (a² + h²)/(2h)` and its center lies `h - R` from the attachment plane along
the chosen direction. The segment starts at latitude `asin(1 - h/R)` and ends at
the pole. Its analytic volume is `πh(3a² + h²)/6`.

Validation checks the selected face's surface, wire count, circular arc centers,
radii, axes, total circumference and disk area. Linear tolerance is
`max(10 × Precision::Confusion(), a × 1e-8)`. Cap volume and the actual signed
material change must agree with the analytic value within
`max(expected volume × 1e-6, linear tolerance × a² × 10)`.

The safe wrapper returns existing kernel history plus generated cap-face ancestry
from the selected source face. Persistent dome names therefore survive supported
height, source-radius and source-depth edits. Added/removed volume is registered
with the existing feature-pattern mechanism. Body identity and scope use the
existing derived aggregate cache.

## Verification

Focused checks:

- `cargo test -p of-geom --test dome`: exact cap volume, sphere radius, apex, circular
  attachment and generated face history at scales 0.1, 1 and 10 with axial/tilted
  normals in both directions; invalid heights, nondisk faces and missing support.
- `cargo test -p of-model --test dome`: preview/commit names, body identity, upstream
  depth/radius edits, suppression, save/reopen, equations, configuration switching,
  failed-geometry recovery, explicit scope, downstream rim fillet and through cut,
  STEP round-trip and STL output, patterned caps on copied supporting bodies,
  and correct direction on a shared split face with an explicit owner scope.
- `cargo test -p openforge --test serve spherical_dome`: shared RPC preview, creation,
  direction/height editing, undo/redo, persistence and invalid-input recovery.
- Focused Rust Clippy and desktop ESLint checks.

The initial kernel corpus took approximately 18 seconds and the initial four
model cases approximately 22 seconds on this development machine; these are
whole-test timings, not established per-feature latency budgets. Integrated
performance qualification remains open.

`scripts/phase-2-dome-smoke.mjs` exercises actual desktop creation and editing,
inward/outward numeric volumes, undo/redo, radius-limit diagnostics, cancellation
and save/reopen. It is syntax/lint checked and is intended for the integrated
native smoke run; no native-run success is claimed by this checkpoint alone.
