# Phase 1 implementation status

The Rust/Electron Phase 1 implementation is complete for the source alpha.
Hosted CI passes on Windows, macOS and Linux for `d059df7` (see
[Release gate](#release-gate)).

This is the current implementation checklist for [the plan](plan.md). The feature
catalog's status column describes the earlier prototype.

## Completed scope

- Constraint-based sketching: lines, arcs, circles, rectangles, slots, polygons,
  splines, dimensions, relations, construction geometry and converted edges.
- Sketch trim, extend, offset, mirror and fillet, including spline trim/offset,
  arc extension and fillets between lines, arcs and splines at shared corners.
- Reference planes, axes, points and coordinate systems; face/edge selection.
- Extrude/revolve bosses and cuts, thin features, reference termination and
  independently selectable end conditions in both directions.
- Edge fillets and chamfers, shell, Hole Wizard, linear/circular patterns and
  mirror. Fillets, chamfers and shells can be selected as pattern/mirror seeds;
  multiple selected seeds replay in feature-tree order.
- Measure, mass properties, materials and display units.
- Feature editing, suppression, ordering, rollback, undo/redo and `.ofp` save/load.
- STEP/IGES import and STEP/IGES/STL export.
- Electron desktop backed by the same Rust JSON-RPC core as the CLI and MCP server.

## Solver decision and curve behavior

The owner approved retaining the native Rust solver on 2026-10-10 in place of the
originally planned PlaneGCS integration. It uses damped least squares and reports
constraint conflicts, redundant relations and remaining degrees of freedom.

Hardening includes input validation before numerical indexing, finite-value and
positive-radius guards, reorthogonalized rank analysis, and unchanged geometry
when solving fails. Regression tests cover a fully constrained 20-line sketch,
dimensions from 0.001 to 100,000 mm, invalid references, nonfinite drags, malformed
fixed constraints, and a point constrained to a spline with one remaining degree
of freedom. Corrupt sketches in part files are rejected before rebuild.

Sketches and OpenCASCADE now use the same Catmull–Rom cubic spline definition.
Trimming retains exact cubic segments. Spline offsets are fitted cubic segments
with a 0.0001 mm target tolerance and reject cusps or fits that cannot meet that
tolerance. Extending an open spline adds a tangent straight continuation to the
next boundary. Sketch fillet operates on curves sharing a corner. These are the
Phase 1 tool semantics, not a claim that every CAD editing variant is supported.

## Geometry regression coverage

The corpus includes a drilled mounting plate, revolved and shelled parts,
filleted/chamfered patterned blocks, thin sections, nested contours, and spline
profiles. Assertions cover volume, body count, feature errors, constraints,
reference dependencies, suppression, rollback, edits and save/reload.

Reference termination tests cover planar, cylindrical and freeform targets,
offset direction, a collapsed offset, invalid/nonterminating targets, multiple
contours, thin revolves and cuts. A downstream fillet survives target-offset
edits. Independent revolve directions survive reference edits and serialization.
Mass integration splits spline spans and uses adaptive Gauss–Kronrod integration.

## Acceptance checks

- Rust formatting, Clippy with warnings denied, and the complete test suite.
- Frontend lint, formatting, TypeScript checks and production build.
- Native Electron smoke: pointer-drawn rectangle, extrusion preview/commit,
  undo/redo, save/new/reopen, STEP/IGES/STL exports, and independent revolve
  directions with a selected reference plane. Renderer errors fail the test;
  Node integration remains disabled.
- CI provisions OpenCASCADE 7.9 through `environment.yml` and runs the core,
  frontend and native smoke checks on Windows, macOS and Linux. Screenshots are
  uploaded as workflow artifacts.

See [README](../README.md#building-from-source) for setup, launch and test commands.

## Release gate

Passed 2026-10-11. The hosted CI workflow ran every check above, including the
native Electron smoke, on `windows-latest`, `macos-latest` and `ubuntu-latest`
against `d059df7`, in runs [38089993738](https://github.com/DaxManuel27/open-solidworks/actions/runs/38089993738)
and [38107808470](https://github.com/DaxManuel27/open-solidworks/actions/runs/38107808470)
(pull request #2). That commit adds the smoke-test layout synchronization that the
first `main` run (38089373458) lacked; its macOS smoke failed before the ribbon
layout settled.

Kernel operations can still reject unsuitable geometry; the alpha reports a
feature error and preserves the preceding body. The regression corpus is a
starting point for real-user part testing, not a guarantee for arbitrary parts.
