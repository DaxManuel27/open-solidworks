# Phase 3 implementation status

Updated 2026-10-12. Built on `phase-3`, branched from `main` after the Phase 2 merge
(PR #3). Every milestone in [the implementation plan](phase-3-plan.md) is implemented
and verified locally; the plan's release gate also needs hosted macOS, Windows and Linux
CI on the release candidate, recorded at the end of this page once it has run.

## What was built

| Milestone | Delivered | Where |
| --- | --- | --- |
| P3.0 Contracts and solver | `.ofa` v1 schema with version dispatch; `CompRef` component references; native Rust mate solver (owner decision 2026-10-12): damped least squares over rigid poses, per connected group, reverse Cuthill–McKee skyline Cholesky, per-body and total DOF, redundant mates, minimal conflicting set by deletion filtering, weak drag targets, range terms, couplers with unwrapped angles | `crates/of-mate`, `crates/of-assembly/src/{document,file}.rs` |
| P3.1 Documents and components | New assembly, open/save with relative paths, insert parts and subassemblies (cycles refused), part cache per file, content hash and configuration with reload on change, missing-file state and relink, fixed/float, hide, suppress (per configuration), rename, exclude from BOM, replace component, rigid and flexible subassemblies, assembly reference planes and axes, instanced scene with per-component picking | `crates/of-assembly/src/{assembly,cache}.rs`, `crates/openforge/src/asm.rs` |
| P3.2 Standard mates | Coincident, parallel, perpendicular, tangent, concentric (lock rotation), lock, distance, angle; aligned / anti-aligned / closest captured once; refused if unsatisfiable with the conflicting mates named; quick-mate filtering in the panel | `crates/of-assembly/src/mates.rs`, `panels/mates.tsx` |
| P3.3 Move and collision | Free drag, along assembly X/Y/Z, by delta, to XYZ, rotate (free and by delta), triad arrows and rings; collision detection stops at first contact along the swept path, optionally for the dragged part only, and highlights the touching faces | `crates/of-assembly/src/analysis.rs`, `viewport/assembly-view.ts` |
| P3.4 Advanced and mechanical mates | Profile center, symmetric, width (centered, free, dimension, percent), path (distance, percent, follow tangent, roll), linear coupler, distance and angle limits; cam, slot (free, center, distance, percent), hinge with angle limits, gear, rack and pinion, screw, universal joint | `crates/of-assembly/src/mates.rs` |
| P3.5 Patterns and mirror | Linear (two directions, skip), circular, pattern-driven (part linear/circular pattern features), sketch-driven (part or assembly sketch points), curve-driven (tangent option); mirror as reoriented copies or opposite-hand part files (regenerated when the source changes) by bounding box, center of mass or origin | `crates/of-assembly/src/patterns.rs` |
| P3.6 Interference, clearance, explode | Exact interference volumes with ignore list, coincidence option, subassemblies-as-components and hidden options; clearance below a value; translate, rotate and radial explode steps per configuration with auto-spacing, explode lines and step-by-step explode/collapse | `crates/of-assembly/src/{analysis,explode}.rs`, `panels/assembly.tsx` |
| P3.7 Properties, BOM, hardware | Part custom properties (document and configuration, links to mass, volume, material, density, configuration, parameters; part file v4); top-level, parts-only and indented BOM with CSV; ISO 4762, ASME B18.3, ISO 4014, ISO 4017, ISO 4032, ISO 7089 and ISO 8734 hardware generated as parametric parts | `crates/of-model/src/properties.rs`, `crates/of-assembly/src/{bom,hardware}.rs` |
| P3.8 Integration | STEP export with product structure (XCAF), IGES/STL of the assembled geometry, STEP assembly import, `openforge info`/`export` for `.ofa`, MCP tools, desktop windows per document | `crates/of-kernel-sys/cpp/assembly.cpp`, `crates/of-assembly/src/exchange.rs` |

The desktop opens each document in its own window with its own core process: double-clicking
a component opens its file, and saving a part reloads the assemblies open in other windows.
Protocol and scene version 4 carry the assembly scene (one mesh per part, instance transforms).

## Verification

Run locally on the release candidate (macOS, Apple M3 Max):

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (workspace).
- `of-mate`: analytic tests for each term type, DOF counts, redundancy, conflict sets,
  limits while dragging, gear coupling across turns, mirror and width sums, path tangents,
  and a 200-body / 600-mate stack (about 55 ms to solve and 21 ms to analyse, debug build).
- `of-assembly`: 26 integration tests on real part files — every mate type, drags and
  collision (including passing through a part between samples), patterns of every kind,
  mirrors and opposite hands, flexible subassemblies, explode, BOM, mass, interference,
  clearance, STEP export/import round trip and the hardware library.
- `openforge`: RPC tests drive insert, mates, conflicts, undo/redo, move phases, patterns,
  explode, BOM/CSV, mass, STEP/STL export, interference, save/reopen, STEP import and
  hardware; CLI `info`/`export` on `.ofa`; every catalog tool example replays on an
  example assembly.
- `pnpm conformance`: 76 of 76 (11 new checks for catalog area 13 and assembly STEP).
- `pnpm perf:asm:check`: 60 components, 176 mates — open 105 ms, full solve 35 ms,
  drag step 29 ms, collision drag step 123 ms, interference 103 ms, BOM 3 ms (debug build);
  budgets in [`phase-3-budgets.json`](phase-3-budgets.json), checked in CI scaled by 4.
- Native desktop smoke: the existing part, Phase 2 and assistant scenarios plus an assembly
  scenario through the real panels — insert, mates, a refused conflicting mate, a collision
  drag, a component pattern, interference, an exploded view, BOM CSV export, undo/redo and
  save/reopen.

## Limits of the delivered modes

These are recorded behaviour, not open Phase 3 items:

- Gear, rack-and-pinion and linear-coupler motions are measured against the assembly
  frame; a gear carried by another moving component (planetary trains) needs a carrier
  reference. The screw mate measures relative to its nut.
- Flexible subassemblies are solved one level deep; their own subassemblies stay rigid.
- Mates to pattern or mirror instances treat the instance as fixed during a solve and are
  solved twice so instances follow their seeds.
- Redundant-mate reporting covers groups of up to 40 floating components; the minimal
  conflicting set is searched for groups of up to 48 mates (larger groups list all their
  mates).
- Tangency covers planes, cylinders and spheres; cones and tori are refused with a reason.
  Cam profiles are the boundary of a flat face across the cam with point or cylinder
  followers; slots are straight slots with rounded ends.
- Collision detection counts overlapping volume, not touching, and samples the motion at a
  quarter of the smallest moving part; physical dynamics and dynamic clearance are deferred.
- Explode lines are straight segments between component centers; smart explode-line
  routing is deferred.
- STEP assembly import flattens subassemblies into top-level components, fixed where the
  file placed them; colours and PMI are not read.
- Hardware sizes are separate part files (generated once per size and length) rather than
  configurations of one file; smart fasteners are deferred.
- Assembly layout sketches are deferred with top-down design, so sketch-driven patterns
  use part sketches from the desktop (an assembly sketch works through the API).

## Deferred catalog items

As planned: chain patterns, copy with mates, SmartMates and magnetic mates, mate
controller, smart components and fasteners, top-down design (in-context editing, layout
sketches, external references, virtual components), flexible parts, envelopes, joined parts,
assembly features, display states, large assembly tools, selection tools beyond select and
isolate, hole alignment, assembly visualization, save assembly as part, motion studies
(Phase 8) and drawing BOM tables (Phase 4, which will read this phase's BOM data).

## Hosted CI

`phase-3` was merged into `main` as PR #4 (merge `bfd9a03`). Its run (38297867137)
stopped at clippy on all three platforms: clippy 1.99 reports `clone_on_copy` in the
code cxx generates for `StepNode` in `of-kernel-sys`. Commit `229da3b` on `main`
allows the lint there, as was already done for `MassProps`, and run 38327262287 on
that commit passed every step (tests, conformance, budgets and the desktop smoke
including the assembly scenario) on macOS, Windows and Linux.
