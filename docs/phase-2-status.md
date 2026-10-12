# Phase 2 implementation status

Updated 2026-10-12. Built on `phase-2`, based on the Phase 1 merge `8fff08d`.
**Phase 2 is complete** and merged into `main`. Every planned mode in
[the implementation plan](phase-2-plan.md) and the
[completion plan](phase-2-remaining-plan.md) is implemented; extensions left out of
scope are listed in the last column of the tables below.

AI assistant development is deferred by owner decision on 2026-10-10; shared CAD
tool contracts remain in scope. This planning update adds no completion or test claims.

Implementation checkpoint, 2026-10-12: the remaining modes are implemented —
step draft, loft centerline and start/end tangency, sweep guides that scale or
stretch the section, conical and geodesic (freeform) wrap, domes on non-circular
faces, seams inside contour entities, ordered projected pieces and content-qualified
import references — with performance budgets and a CI gate. All local gates, the
native smoke and 65 conformance checks pass, and hosted macOS/Windows/Linux CI passed
(run 38205003310).

Implementation checkpoint, 2026-10-11 (later): every Phase 2 feature family now
exists end to end — rib, wrap, boundary, face fillets, modeled threads and
tapped-hole cosmetic threads are new; sweep, loft, draft, variable fillet, dome,
helix, projected curve, split line, 3D sketch and equations gained the modes listed
below. All repository gates, the native desktop smoke and the new
[catalog conformance check](../scripts/catalog-conformance.mjs) (60 checks against
`docs/features`) pass locally. Hosted three-platform CI has not run, and the modes
listed as remaining are still open, so Phase 2 is not marked complete.

Implementation checkpoint, 2026-10-11: ordered profile/curve references, general
tool splitting, scoped parameter/formula editing, constrained spatial sketches,
hollow sweeps, circular spherical domes and cosmetic threads are implemented.
See [supported inputs and remaining acceptance work](phase-2-checkpoint-2026-10-11.md).
These changes advance several milestones; they do not complete the entire phase.

Implementation checkpoint, 2026-10-10: the [kernel experiments and contour-path
slice](phase-2-kernel-feasibility.md) now have measured results and regression coverage.
This advances R0/R1; neither milestone nor Phase 2 is marked complete.

## Implemented checkpoints

These operations have core implementations, desktop panels and regression tests.
New features use the shared add/update/preview RPC methods, including MCP.

| Area | Available behavior | Limits / remaining work |
| --- | --- | --- |
| File and protocol | V3 files/protocol/scene, V1/V2 migration, ID validation, matching-version handshake, revision-checked previews | Hosted CI remains; V3 prevents older readers ignoring new split semantics |
| Bodies | Stable lineage-based IDs, qualified topology refs, body tree, name/visibility, solid feature scope | Aggregate `State.body` remains as a derived compatibility cache; broader import/pattern/topology-change corpus remains |
| Body operations | Union/subtraction/intersection, rigid move/copy, legacy plane-side split, general plane/face/body-tool split and arbitrary retained pieces | Indistinguishable oriented provenance fails explicitly; broader imported/pattern cases remain |
| Equations | Typed expressions/units, globals, stable keys, cycle diagnostics, formula editing on existing features, `=formula` entry in creation fields, syntax errors rejected before saving, rebuild from the first feature whose effective definition changed | Inverse trig/conditionals and the other catalog functions remain |
| Configurations | CRUD/activation, typed overrides, explicit base/active parameter editing, reset-to-base, undo, persistence, export by ID | Structural feature editing is shared; full rebuild on formula/variant edits; no inheritance or spreadsheet tables |
| Construction curves | Helix by pitch/turns, height/turns or height/pitch, variable pitch; composite; projection onto faces and sketch on sketch; ordered contours and generated segment refs; a projection interrupted by holes yields pieces named in order along their source | Variable diameter, taper and spiral helices; bidirectional projection |
| Spatial sketches | Points, lines, cubic Beziers, construction, dimensions/relations, DOF/conflicts, workplanes/depth locks, XYZ coordinates/handles, snapping, viewport dimensions; fixed and point-on-plane relations that follow model vertices, points, planes and faces | Arcs and circles, associative edges/curves, richer contour selection |
| Split line | Projection, intersection (plane, face or body) and silhouette (cylindrical and spherical faces) | Silhouettes of other surfaces; several faces at once |
| Sweep | Closed outer/inner contours, circular profile or thin open/closed profiles along sketch/spatial/entity/edge/construction paths; minimum twist, Frenet, fixed, twist angle, or follow path and guide; a guide that also scales the section, or a second guide that stretches it (exact sections at stations, refined until sampled profile points between them are within 1e-5 × the model size); mitred corners; self-intersection rejected; boss/cut and scope | Scaling guides need an open, tangent-continuous path; more than two guides; closed-path seam controls |
| Loft | Ordered contours, reversal/start-entity seams and seams inside lines, arcs, circles and Bezier curves, ruled/smooth, two or more guide curves, closed lofts, centerline lofts, start/end tangency (normal to the profile or along a direction, with tangent length), boss/cut and scope | Holes in sections, single guide, tangency/curvature to adjacent faces |
| Draft | Neutral plane with pull direction and signed angle; parting line on straight or circular edges; step draft (level hinge with perpendicular step faces on planar faces) | Tapered steps, face propagation |
| Variable fillet | Tangent chain through one edge, stations along the chain in the edge's direction, reversed, smooth or straight transition | Several separate edges, setbacks |
| Dome | Spherical cap (height up to the radius) or elliptical dome (any height, tangent to the rim normal) on a complete planar circular disk; on other planar faces a smooth filling through the boundary and the apex, optionally tangent to the side faces where the boundary has no corners; inward/outward; scope | Holed faces; tangency on faces with corners |
| Rib | Open line/arc sketches extended to meet material, parallel or normal to the sketch, side, flip, draft | Closed rib sketches, natural extension |
| Wrap | Emboss, deboss and scribe on cylinders (exact arc-length mapping) and cones (exact development about the apex), across seams; other curved faces along geodesics from the point under the sketch origin, rejected with the measured value when lengths stretch more than 1% | Freeform faces whose wrap crosses a parametric singularity other than a sphere's pole |
| Boundary | Two or more planar-ended sections and two or more crossing curves; Coons-patch network; boss/cut and scope | Direction 1 only, end tangency/curvature conditions |
| Face fillet | Two sets of planar faces at any angle, adjacent or not, convex or concave, tangent to both | Curved face sets, hold lines |
| Modeled threads | ISO 68-1 basic 60° profile, internal/external, either hand, trimmed ends, up to 200 turns | Other thread standards, runouts; rebuild cost about 1.4 s (debug) for 10 turns |
| Cosmetic threads | Persistent complete-cylinder metadata, lightweight viewport, editing/configurations; tapped holes can show cosmetic threads with table pitch | Interrupted cylinders |

Construction curves are selectable viewport paths. Hidden bodies stay in mass
properties, modeling scope and exports. Invalid features preserve the preceding
valid geometry and report an editable feature error. Exports reject a configuration
with active feature errors.

Configurations resolve into temporary definitions; their solved dimensions and
materials do not overwrite the base document. Changes involving formulas or variants
conservatively rebuild the full history. Editing previews use the same resolution
rules, including equations that make a later parameter drive an earlier feature.

## Remaining Phase 2 work

- The limits in the table's last column are out-of-scope extensions or later refinements;
  every planned R0–R8 mode now has an implementation, tests and desktop controls.
- Imported references are qualified by file content (a different file reports missing
  references instead of rebinding); patterned and generated-curve references are covered
  by `crates/of-model/tests/reference_corpus.rs`.
- Performance budgets are recorded in [phase-2-budgets.json](phase-2-budgets.json) from three
  runs of [the measurement script](../scripts/phase-2-performance.mjs) and gated by
  `pnpm perf:check` (also in CI, scaled 4× for hosted runners).
- Hosted macOS/Windows/Linux CI passed on `phase-2` at `3ced10a` (GitHub Actions run
  38205003310), including the native desktop smoke on each platform.

## Verification

The automated regressions include:

- Real V1 block/multisolid fixtures, newer-version errors and invalid persistent IDs.
- Multibody move/copy/split/combine and scoped operations with identity/volume checks.
- Equation units, parser domains, cycles, invalid consumers, preview/commit equality
  and authored parameter preservation.
- Configuration A→B→A isolation, material/suppression, numeric override precedence,
  undo, save/load and CLI STEP exports with different expected volumes.
- Analytic sweep/loft volumes, helix geometry/handedness, disconnected paths and
  downstream cap references after edits/reload.
- Draft volume, valid/invalid radius stations and unchanged split-line volume.
- Native Electron controls for body visibility/copy, equation edits, configuration
  overrides/switching, undo/redo and V2 save/reopen, alongside Phase 1 smoke coverage.

The earlier implementation checkpoint recorded these local passes on 2026-10-10
(see the feasibility report above for the newer R0/R1 verification):

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  (96 tests) and `cargo build`.
- ESLint, Prettier, desktop TypeScript checks and the production desktop build.
- Native Electron smoke, including the Phase 2 workflow above.
- A final sweep-pattern regression passed after adding sweep/loft to desktop
  pattern selection; formatting, clippy and desktop checks were repeated for that change.

Hosted macOS/Windows/Linux CI and the complete Phase 2 acceptance corpus have not run.

## Using the new capabilities

See [the protocol reference](protocol.md) for payloads, parameter keys, precedence
and supported inputs. `core.info` lists implemented capabilities. The desktop
requires the matching rebuilt Rust binary and renderer.

```sh
cargo build
corepack pnpm --filter @openforge/desktop build
corepack pnpm --filter @openforge/desktop smoke
openforge export variants.ofp short.step --configuration 2
```

Feature tools are in the Features ribbon; Equations and Configurations are in
Evaluate. The feature tree contains the body list and active configuration picker.
