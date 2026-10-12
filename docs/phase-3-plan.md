# Phase 3 implementation plan

Status: approved, 2026-10-12. Not started. Built on `phase-3`, created from `main`
after the Phase 2 merge (see [Phase 2 status](phase-2-status.md)). The mate solver
is native Rust (`of-mate`), by owner decision on 2026-10-12.

## Goal and starting point

Deliver the assembly scope in [the roadmap](plan.md#scope-and-phases): insert
components, standard, advanced and mechanical mates, move and rotate with collision
detection, component patterns and mirror, interference detection, exploded views,
an assembly BOM and a standard hardware library. Every delivered capability must
work through the desktop and the shared core used by JSON-RPC, CLI and MCP.
Detailed behaviour comes from [the assemblies spec](features/13-assemblies.md).

The Phase 2 baseline provides primitives worth retaining:

- Part documents (`.ofp`, file version 3) with feature history, configurations,
  equations, materials, undo/redo and stable, body-qualified face/edge names.
- `Shape::transform`, `distance`, `mass_props`, `bbox`, booleans and STEP/IGES/STL
  I/O in `of-geom`, with all C++ in `of-kernel-sys`.
- A native damped-least-squares sketch solver with DOF and conflict diagnostics.
- A generated tool catalog (`tools.gen.ts`, MCP schemas) and catalog conformance
  harness (`pnpm conformance`).

Gaps that shape this phase:

- The core owns exactly one part document per session; the desktop has one window
  and one core process. There is no notion of a document that references files.
- The scene carries one mesh per body in world coordinates. Assemblies need shared
  part meshes drawn many times with per-instance transforms, and picking that
  returns a component path plus a part face/edge name.
- Parts have no custom properties (part number, description), which a BOM needs.
- STEP export writes a single shape; there is no product structure.

## Delivery rules

The Phase 2 rules carry over unchanged. Each milestone is a vertical slice: kernel
or solver, model, references, protocol, desktop creation/editing, persistence and
regression tests. Keep the crate dependency direction; keep all C++ and `unsafe` in
`*-sys` crates. Internal units remain millimetres and radians. Failed operations
keep the last valid state and report actionable errors. Record unsupported variants
explicitly; do not mark Phase 3 complete while a required item is blocked.

## Scope

Required in this phase (roadmap items plus what they cannot work without):

| Area | Required |
| --- | --- |
| Documents | Assembly file (`.ofa`), new assembly, create assembly from part, open/save, missing-file handling, reload on part change |
| Components | Insert parts and subassemblies, configuration choice, fix/float, hide/show, suppress, rename, exclude from BOM, rigid subassemblies, flexible subassemblies, replace component |
| Mates | Standard (coincident, parallel, perpendicular, tangent, concentric, lock, distance, angle), advanced (profile center, symmetric, width, path, linear coupler, distance/angle limits), mechanical (cam, slot, hinge, gear, rack and pinion, screw, universal joint); alignment and flip; quick-mate filtering; DOF and conflict diagnostics |
| Motion | Free drag, along axis/entity, by delta, to XYZ, rotate equivalents, triad; collision detection (stop at collision, dragged part only, highlight faces) |
| Patterns | Linear, circular, pattern-driven, sketch-driven, curve-driven component patterns; mirror components (instance copy and opposite-hand part) |
| Tools | Interference detection, clearance verification, assembly mass properties and measure |
| Exploded views | Regular and radial steps, translate/rotate, per configuration, explode lines, animate explode/collapse |
| BOM | Top-level, parts-only and indented BOM with quantities and part properties; CSV export; part custom properties |
| Hardware | Parametric fastener library (bolts, screws, nuts, washers, pins) from published standard dimensions, inserted as components |
| Exchange | STEP export with product structure; IGES/STL export of the assembled geometry |

Deferred to later phases or extensions, recorded in the status document: chain
patterns, copy with mates, SmartMates and magnetic mates, mate controller, smart
components and smart fasteners, top-down design (in-context editing, layout
sketches, external references, virtual components), flexible parts, envelopes,
joined parts, assembly features, display states, large assembly tools (lightweight,
SpeedPak, Large Design Review), selection tools beyond select/isolate, hole
alignment, assembly visualization, save assembly as part, physical dynamics and
dynamic clearance, motion studies (Phase 8), and drawing BOM tables (Phase 4, which
reuses this phase's BOM data). STEP assembly import is a candidate extension; it
is the natural way to bring multi-part files from other CAD systems in, and the
XCAF reader prototyped for export makes it inexpensive.

## Milestones and dependencies

| Milestone | Deliverable | Depends on | Acceptance example |
| --- | --- | --- | --- |
| P3.0 | Contracts, schema, solver prototype, kernel prototypes | `phase-2` merged | `of-mate` prototype meets its targets; `.ofa` v1 schema and fixtures |
| P3.1 | Assembly documents, components and display | P3.0 | Insert three parts and a subassembly, save, move a part file, reopen with a missing-file state, relink |
| P3.2 | Mate solver and standard mates | P3.0, P3.1 | Fully define a bracket on a plate; over-define it and see the conflicting mates named |
| P3.3 | Move/rotate, drag solving and collision detection | P3.2 | Drag a hinged lid; it stops at the base with the touching faces highlighted |
| P3.4 | Advanced and mechanical mates | P3.2, P3.3 | Drag the input shaft of a two-stage gearbox; the output turns at the gear ratio |
| P3.5 | Component patterns and mirror | P3.2 | Pattern bolts from a flange's hole pattern; mirror a bracket as an opposite-hand part |
| P3.6 | Interference, clearance and exploded views | P3.1, P3.3 | Find a 0.5 mm shaft/bore overlap with its volume; explode and collapse the gearbox |
| P3.7 | Part properties, BOM and hardware library | P3.1, P3.5 | Indented BOM of the gearbox with ISO 4762 screws, exported as CSV |
| P3.8 | Integrated corpus, parity, performance and release gate | All above | Hosted three-platform checks pass on the exact release candidate |

P3.4 to P3.7 are independent once their prerequisites pass and may run in any order.
No calendar commitment yet: estimate after P3.0 measures the solver and P3.1
establishes the cost of multi-document sessions.

## P3.0 — Contracts and decisions

**Assembly schema.** A version 1 `.ofa` JSON document with the same header and
version dispatch as `.ofp`:

- `components`: stable `ComponentId`, instance name, source (path relative to the
  assembly file, plus the content hash last loaded), configuration ID, rigid
  transform (translation + unit quaternion, never a general matrix), fixed flag,
  state (resolved/suppressed), hidden, exclude-from-BOM, and for subassemblies
  rigid/flexible.
- `mates`, `patterns`, `mirrors`, `explode` (per configuration), `reference`
  (assembly-level planes, axes, points and sketches reusing `of-model` datums and
  sketches), `configurations` and `units`.
- Component references are `CompRef { path: Vec<ComponentId>, entity: Ref }`. The
  path walks subassemblies; `entity` is the existing body-qualified part name. Never
  store kernel indices or instance order.
- Pattern and mirror instances get derived IDs (`feature id + index`) so mates and
  BOM rows that reference them survive edits that keep the instance.

**Session model.** The core holds a document enum (part or assembly). An assembly
session owns a part cache keyed by `(canonical path, content hash, configuration)`,
loading and building each part once, read-only. `core.info` reports document kinds
so older clients refuse assemblies instead of misreading them. Editing a part is
done by opening it as its own document.

**Mate solver.** Native Rust in a new `of-mate` crate, decided by the owner on
2026-10-12 in place of the OndselSolver named earlier in the roadmap. Mates reduce
to equations over 6-DOF rigid poses (translation plus unit quaternion), solved with
damped least squares like the sketch solver, sharing its numerical utilities where
they are dimension-independent. `of-mate` depends only on `glam`; it knows poses,
geometric primitives and constraints, never part files or kernel shapes. Before
P3.2, prototype it against these targets and record results:

| Target | Measure |
| --- | --- |
| Correctness | Corpus of standard, limit and coupler mates with analytic answers |
| Diagnostics | DOF per component; minimal conflicting mate set; last valid poses kept on failure |
| Dragging | Minimal-motion solve toward a target pose at interactive rates |
| Inequalities | Distance/angle limits held while dragging |
| Speed | 200 components / 600 mates full solve and drag step, debug and release |

**Kernel prototypes.** Pick collision and interference methods with measurements:
pairwise `BRepAlgoAPI_Common` with bounding-box and BVH pre-filtering for exact
interference volumes; `BRepExtrema_DistShapeShape` (already wrapped as `distance`)
for clearance; and either `BRepExtrema_ShapeProximity` or a Rust BVH over the
display triangles for collision during drag. Prototype STEP product structure
through XCAF (`STEPCAFControl_Writer`, and the reader for the import extension).

## P3.1 — Assembly documents, components and display

Add an `of-assembly` crate (`openforge` → `of-assembly` → `of-model`, `of-mate`).
It owns the document, part cache, reference resolution, mass properties, BOM data
and later patterns and explode steps. Resolve sources relative to the assembly,
then the last known absolute path; a missing file is a component state, not a load
failure, with a relink action. Detect subassembly cycles. Reload a part when its
hash changes on disk, re-resolving mates and reporting any that lost references.

Scene protocol: `parts` holds one mesh per cache key; `instances` lists component
paths, part key, world transform and display state. Drag updates send transforms
only. The viewport draws shared geometry per instance, and picking returns
`(component path, face/edge name)`. Selection, highlighting and measure work across
components.

Desktop: New Assembly, Create Assembly from Part, open/save `.ofa`, an Assembly
ribbon tab, Insert Components (file browse, configuration, place by click, first
component fixed at origin), a component tree with fixed `(f)` / under-defined `(-)`
markers and a Mates folder, and component properties. Open a second document in
a new window with its own core process; when a part window saves, open assemblies
that reference it reload it.

Rigid subassemblies insert as one component; flexible ones expose their mates to
the parent solve (completed in P3.2). Replace component swaps the source and keeps
mates whose entity names still resolve, listing the rest for re-selection.

Exports: STEP with product structure and instance names; IGES and STL of the
transformed compound. Assembly mass properties combine part densities and
transforms and skip suppressed and excluded components.

## P3.2 — Mate solver and standard mates

Extract mate geometry in component frames from resolved references: plane
(point, normal), axis (point, direction), point, and cylinder/cone/sphere (axis,
radius, apex, center). Reject unsupported entity pairs with the reason before
solving. Implement coincident, parallel, perpendicular, tangent, concentric (with
lock rotation), lock, distance and angle, with aligned/anti-aligned/closest
alignment and flip. Closest alignment is resolved once at creation and stored, so
later solves cannot flip it.

Solve per connected group, fixed components as ground. Report each component's
remaining DOF and the assembly's status; on failure keep the last valid poses, mark
the failing mates and name the minimal conflicting set. Mates are tree items with
edit, suppress, delete, rename, reorder in folders, undo/redo and preview. The mate
panel filters mate types to those valid for the current selections (quick mates).

## P3.3 — Move, rotate and collision detection

Drag solving moves the selected component toward a target pose within its
remaining DOF, minimizing motion of everything else. Modes: free drag, along
assembly XYZ, along an entity, by delta XYZ, to XYZ, and rotation equivalents,
plus an on-component triad. Persist the resulting poses of under-defined
components. Collision detection stops motion at first contact, optionally checks
only the dragged component, and highlights the touching faces. Use the method
chosen in P3.0 and keep drag steps within the performance budget.

## P3.4 — Advanced and mechanical mates

Advanced: profile center (with offset and lock rotation), symmetric, width
(centered, free, dimension, percent), path (pitch/yaw and roll control, distance
or percent along path), linear coupler, and distance/angle limits. Limits are
inequality constraints; the solver must keep components inside the range while
dragging. Mechanical: cam (tangent face loop), slot (free, centered, distance,
percent), hinge, gear (ratio, reverse), rack and pinion, screw, and universal
joint. Couplers relate motion of two components; validate ratios and that the
coupled axes stay meaningful.

## P3.5 — Component patterns and mirror

Linear and circular (both directions, skip instances, seed configuration),
pattern-driven (follow a part's pattern feature, including Phase 2 patterns),
sketch-driven (assembly sketch points) and curve-driven (tangent or aligned)
component patterns. Instances follow seed edits and reference updates. Mirror
components places reoriented instance copies about a plane or planar face, or
creates an opposite-hand part file built with the existing part mirror and
linked to its source. Use bounding-box center, center of mass or origin
positioning.

## P3.6 — Interference, clearance and exploded views

Interference detection over selected components or the whole assembly, with
options to treat coincidence as interference, include subassembly internals and
ignore hidden bodies. List each interference with its volume, highlight it, and
let users ignore it persistently. Clearance verification lists gaps below a value.
Exploded views store ordered steps per configuration: translate or rotate along a
triad direction, radial steps about an axis, automatic spacing, and straight
explode lines that update with the steps. Animate explode/collapse in the viewport;
the explode state is display-only and never changes mates or poses.

## P3.7 — Part properties, BOM and hardware library

Add custom and configuration-specific properties to parts (text, number, date,
yes/no, with links to material, mass and dimensions), editable in the desktop and
persisted in `.ofp` with a version bump and migration. The BOM is a core view:
top-level only, parts only or indented; quantities merge identical
part/configuration instances; columns come from properties; excluded components are
left out. Show it in an assembly panel and export it as CSV. Phase 4 drawing BOMs
consume the same data.

The hardware library generates real parametric parts: socket head cap screws
(ISO 4762, ASME B18.3), hex bolts and screws (ISO 4014/4017), hex nuts
(ISO 4032), plain washers (ISO 7089/7090) and dowel pins (ISO 8734). Dimension
tables come from published standards. Each item is built as a feature tree
(sketch, revolve, chamfers, cosmetic thread) with size and length as
configurations, saved in a user library folder and reused. Insertion picks
standard, type, size and length and fills part-number/description properties.
Smart fasteners build on this later.

## Code integration map

| Area | Primary files and intended changes |
| --- | --- |
| Solver | New `crates/of-mate`: poses, mate equations, damped least squares, DOF and conflict diagnostics, limits and couplers |
| Assembly model | New `crates/of-assembly`: document, file I/O, part cache, `CompRef` resolution, patterns, mirror, explode, BOM, mass properties |
| Kernel | `of-kernel-sys` and `of-geom`: XCAF STEP structure, interference volumes, proximity/collision queries |
| Parts | `crates/of-model`: custom properties, file version bump, pattern data exposed for pattern-driven component patterns |
| Core | `crates/openforge/src/{session.rs,views.rs,tools/}` and `of-protocol`: document kinds, assembly methods, instance scene, drag fast path, tool catalog and MCP schemas |
| Desktop | `main/index.ts` (window and core per document), `store.ts`, `protocol.ts`, `FeatureTree.tsx`, viewport instancing, picking and triad, assembly panels and ribbon |
| Library | Fastener generators and standard tables (core), library folder management |
| Tests/docs | Solver unit tests, assembly corpus, serve tests, conformance checks for area 13, desktop smoke scenario, `docs/phase-3-status.md`, `docs/protocol.md` |

## P3.8 — Acceptance and release

Every behaviour change gets a regression test at the lowest useful layer. The
solver has analytic tests per mate type, DOF counts, conflict sets and limit
behaviour. Build a reproducible assembly corpus with expected poses, DOF and
measured results:

- A two-stage spur gearbox (housing, shafts, gears, bearings, hardware): gear
  mates, interference, BOM, explode.
- A hinge and a slider-crank: drag motion and limits.
- A bolted flange: pattern-driven fasteners from the hardware library.
- A mirrored bracket pair: instance and opposite-hand mirrors.
- A nested subassembly, rigid and flexible.

Extend the native desktop smoke test with insert, mate, drag with collision,
interference, explode and BOM export, with save/reopen and undo/redo throughout.
Add catalog conformance checks for area 13. Set performance budgets in
`docs/phase-3-budgets.json` (open, full solve, drag step, interference) using the
existing `perf` tooling, and check them in CI.

Run the repository gates, the native smoke test and hosted macOS, Windows and
Linux workflows on the release candidate. Publish supported modes and remaining
limits in `phase-3-status.md`. Merge `phase-3` to `main` only when the required
scope and hosted checks are complete.
