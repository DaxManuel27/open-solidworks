# Phase 2 implementation plan

Status: complete, 2026-10-12. Built on `phase-2`, based on merged `origin/main`
commit `8fff08d`; prepared and started 2026-10-10. See
[Phase 2 status](phase-2-status.md) for delivered checkpoints, verification and
the extensions left out of scope.

Execution update, 2026-10-10: follow the [completion plan](phase-2-remaining-plan.md)
for the remaining implementation sequence, dependencies and release gates. The owner
has deferred AI assistant work to prioritize Phase 2 CAD completion. This document
remains the feature specification; the completion plan supersedes its suggested
execution order without reducing required scope.

## Goal and starting point

Deliver the advanced part modeling scope in [the roadmap](plan.md): sweep, loft,
boundary, draft, rib, dome, wrap, modeled and cosmetic threads, variable and face
fillets, helix/projected/composite curves and split lines, 3D sketching, multibody
operations, equations, global variables and configurations. Every delivered feature
must work through the desktop and the shared core used by JSON-RPC, CLI and MCP.

Phase 1 is merged. The pre-merge
[Phase 1 status](phase-1-status.md) records the validation available when it was
written; this update does not independently establish hosted CI results. The local
checkout was synchronized with `origin/main` and `phase-2` was created from that
baseline, preserving the planning changes.

The Phase 1 baseline provides primitives worth retaining:

- Feature history, preview, rollback, suppression and document-based undo/redo.
- Safe geometry wrappers, booleans, rigid transforms, kernel history and stable
  face/edge naming.
- Multiple solids inside one compound, including `merge: false` bosses. However,
  `State.body` is still one `Option<Body>` and there are no independent body IDs.
- A native 2D solver, numeric feature parameters and version 1 JSON part files.
- One body mesh in `views::scene`, manually mirrored TypeScript protocol types,
  and generic `feature.add`, `feature.update` and `feature.preview` RPC methods.

The largest architectural changes are explicit body ownership, reusable curve
references, parameter evaluation and configuration isolation. Implement those
before adding feature families that would otherwise duplicate this work.

## Delivery rules

Each milestone is a usable vertical slice: kernel operation, safe wrapper,
document model, reference history, protocol, desktop creation/editing, persistence
and regression tests. A button or a kernel-only operation does not complete a
feature. Split large milestones into reviewable PRs in that order.

Retain the existing crate dependency direction and keep all C++ and unsafe Rust
inside `of-kernel-sys`. Keep the native Rust sketch solver. Internal units remain
millimetres and radians. New operations must validate inputs and kernel results,
report actionable errors and preserve the preceding valid state on failure.

Basic modes below are implementation checkpoints, not permission to drop the
remaining roadmap scope. Record unsupported variants explicitly. Do not mark
Phase 2 complete while a required feature is blocked; a smaller release needs an
explicit roadmap revision.

## Milestones and dependencies

| Milestone | Deliverable | Depends on | Acceptance example |
| --- | --- | --- | --- |
| P2.0 | Baseline, contracts and difficult-kernel prototypes | Merged Phase 1 baseline (owner confirmed); synchronize checkout | Existing part fixtures survive proposed migrations; prototype results recorded |
| P2.1 | Body identity, feature scope, combine, split, move/copy | P2.0 | Split a block, move one piece, subtract it from another; edit upstream dimensions |
| P2.2 | Global variables and equations | P2.0 | One variable drives sketch dimensions, depth and pattern spacing with unit checking |
| P2.3 | Manual configurations | P2.1, P2.2 | Switch between three variants without leaking dimensions, suppression or material |
| P2.4 | Curve infrastructure, helix, projection, composite and split line | P2.1 | Project a curve, split a face and preserve a downstream reference after an edit |
| P2.5 | Constrained 3D sketches | P2.2, P2.4 | Edit a dimensioned spatial path while retaining valid constraints and diagnostics |
| P2.6 | Sweep and loft bosses/cuts | P2.1, P2.4; P2.5 for spatial-sketch paths | Swept tube and lofted adapter rebuild after path/profile changes |
| P2.7 | Draft, rib and dome | P2.1, P2.4, P2.0 feasibility results | Drafted enclosure with ribs and a domed cap remains a valid solid |
| P2.8 | Variable and face fillets | P2.1, P2.0 feasibility results | Variable blend and two-face-set blend survive supported upstream edits |
| P2.9 | Boundary bosses/cuts and wrap | P2.4, P2.6, P2.0 feasibility results | Two-direction blend and wrapped emboss/deboss/scribe on documented face types |
| P2.10 | Modeled and cosmetic threads | P2.4, P2.6 | Internal/external thread features plus lightweight cosmetic representation |
| P2.11 | Integrated corpus, desktop workflows and release gate | All above | Hosted three-platform checks pass on the exact release candidate |

The original recommended execution order was the table order; use the completion
plan's R0–R9 order for the remaining work. Independent milestones may be
reordered once their prerequisites pass. No calendar commitment yet: estimate
after P2.0 resolves the difficult geometry paths and P2.1 establishes migration
cost. The first implementation PR should establish P2.0 contracts and fixtures,
followed by the body model migration in P2.1.

## P2.0 — Baseline and contracts

Capture representative version 1 `.ofp` fixtures and the existing geometry corpus.
Document the current protocol in a new `docs/protocol.md`; the Rust protocol
module already refers to that missing file. Cover scene, selection, preview,
errors and transaction behavior, with Rust/TypeScript contract fixtures.

Define these foundations before their first consumers:

- `BodyId`, body-qualified topology references and deterministic result identity.
- `CurveRef` with ordered segments and explicit orientation, plus profile/contour
  selection independent of a whole sketch.
- Stable parameter keys for feature fields and sketch constraint dimensions;
  keys use IDs, never display names or array positions.
- A version 2 part schema, explicit version dispatch and v1-to-v2 migration.
  Parse the file header before version-specific document decoding so newer files
  receive a useful version error. Preserve old defaults, references and units.
- Structured operation diagnostics and a document revision/configuration identity
  in preview responses so the UI can discard stale previews.

Prototype boundary construction, guide-controlled loft/sweep, face fillets, dome
and wrap early using the project's pinned OpenCASCADE 7.9 environment. For each,
record supported inputs, failure cases, topology history quality and measured
continuity/tolerance. A visually plausible mesh is insufficient: require valid
B-rep geometry, expected solid count and downstream boolean/fillet use.

Kernel candidates are implementation hypotheses, not a claim of product parity.
The official references describe [section-based lofts](https://occt3d.com/dev/doc/refman/html/class_b_rep_offset_a_p_i___thru_sections.html),
[fillet construction](https://occt3d.com/dev/doc/refman/html/class_b_rep_fillet_a_p_i___make_fillet.html)
and [shape splitting](https://occt3d.com/dev/doc/refman/html/class_b_rep_algo_a_p_i___splitter.html).
The documentation URLs currently redirect to 8.0.1; verify all selected APIs
against installed 7.9 headers and compiled prototypes before adopting them.

## P2.1 — Explicit multibody modeling

Replace the aggregate `State.body` with an ordered body collection. Each body has
an ID, shape and naming map; persistent labels/visibility are keyed separately by
ID. Build aggregate compounds only for operations that need them, including
whole-part export and measurement. Do not use solid enumeration order as identity.

Use creator/output lineage for new bodies and history to track edits. A move
preserves identity; a copy receives feature-derived identity and names. Combine
creates a result owned by the combine feature and consumes selected inputs. Split
creates children tied to parent and splitting-feature lineage. If a topology
change makes matching ambiguous, report a missing/ambiguous reference instead of
silently binding a different piece. Preserve provenance for dependency checks.

Add `Ref::Body` and body qualification to face/edge/vertex references. Resolve
legacy unqualified names only when unique. Update converted sketch references,
measurements, reference geometry, pattern tool replay and imported multi-solid
parts together. Generalize naming history from its current two named inputs to
multiple sources for combine and loft, with bounds-checked source indices.

Define feature scope as all applicable bodies or an explicit nonempty set. Legacy
features retain current whole-part behavior through migration. New bosses expose
merge versus new-body behavior; cuts and dress-up features expose affected bodies.
Preserve unselected bodies and reject invalid cross-body edge/face selections.

Deliver combine union/subtract/intersect, plane/face/tool-based split with retained
piece selection, and rigid translate/rotate/copy. Show a Bodies group with select,
rename and hide/show controls. Scene meshes and picking carry body IDs. Visibility
affects display, not whether a body participates in a feature or default export.

Test migration of disjoint and touching solids, scoped cuts, consumed/missing
bodies, copy-name collisions, split-count changes, rollback, patterns and exports.
Check volume conservation for split and rigid transforms and unaffected-body
identity after operations on another body.

## P2.2 — Equations and global variables

Add model-owned expression parsing and typed evaluation in focused modules.
Keep authored numeric parameters plus an expression binding map keyed by stable
parameter IDs; evaluate into a temporary effective definition before building.
This avoids rewriting all existing feature structs into untyped values.

Support arithmetic, parentheses, named variables, parameter references, explicit
length/angle units and a documented small function set (`min`, `max`, `abs`,
`sqrt`, trigonometry). Validate dimensional compatibility, finite results, division
by zero, domain errors and integer-only targets such as pattern counts. Bare
numeric fields retain their existing unit conversion; expression constants have
documented types and cannot change meaning when display units change. Do not use
JavaScript evaluation or execute scripts in the core.

Build a dependency graph for variable and parameter bindings. Detect cycles and
missing references with the dependency path in the error. Parameter references
read authored/evaluated parameters, not mass properties or other geometry outputs
in this phase. Geometry-dependent equations require a separate rebuild policy.
Invalidate from the earliest affected feature; configuration changes may initially
rebuild everything. Consolidate reference dependency discovery for deletion and
reordering, including body/curve provenance and expression bindings.

Provide an equation/global-variable editor and expression-enabled numeric fields
that show formula, evaluated value and units. Make a failed evaluation block its
consumer with a clear error rather than reusing an old numeric result. Test chains,
renames, deleted parameters, mixed units, cycles, rollback and undo/redo.

## P2.3 — Configurations

Persist a base document, stable configuration IDs and typed overrides for numeric
parameters/expressions, global variables, suppression and part material. The first
configuration represents the existing document. Resolve overrides without mutating
base feature definitions or solver results belonging to another configuration.

Add create/duplicate/rename/delete/activate operations, a configuration picker and
a table for the supported overrides. Make editing scope explicit: base/all variants
or active variant. Feature creation/deletion/reordering changes the shared tree;
variant-specific presence uses suppression. Remove stale overrides on deletion
and include configuration operations in undo/redo. Save the active configuration;
export uses it unless a core request explicitly names another one.

Initially invalidate the full rebuild cache on switch. Add per-configuration
caching only after correctness, with keys containing document revision and resolved
configuration inputs. Cancel editing/preview state on switch and clear invalid
selections. Test A→B→A equality, override precedence, solver isolation, failed
variants, save/load and exports with distinguishable expected volumes.

Derived configuration inheritance and external spreadsheet design tables are
separate catalog capabilities; they are not prerequisites for manual configurations.

## P2.4 — Curves and split lines

Add safe 3D curve/wire operations in `of-geom`, storing parametric source definitions
in `of-model`. Curves must be referenceable geometry, not just viewport polylines.
Preserve segment IDs, direction, closure and source ownership across rebuilds.
Expose evaluation/tangent/sampling helpers and ordered path selection in the UI.

Deliver helix with pitch/height/turns, radius, handedness and start angle; composite
curves from selected sketch/model/curve segments; projected curves from sketch to
faces and intersection of projected sketches; and split lines that partition faces
without changing material. Stage projection, intersection and silhouette modes
separately with explicit supported targets. Validate disconnected chains, branch
ambiguity, degenerate curves, seam crossings and projection failures.

Face splits must return history for downstream selections. Check unchanged volume
and stable references where the same topological region persists. Display curves
and their control parameters with selectable paths and direction indicators.

## P2.5 — 3D sketching

Introduce a separate `Sketch3D` feature and entity model in `of-sketch`; do not make
the existing 2D sketch plane optional. Reuse numerical solver utilities where they
are dimension-independent while retaining the 2D constraint behavior and tests.

Start with points, lines and cubic splines, construction entities, active workplanes
and coordinate entry. Add coincidence, fixed, distance, axis alignment,
parallel/perpendicular and point-on-plane relations, then spline endpoint tangency
and point-on-curve support. Provide DOF/conflict diagnostics and unchanged geometry
on failed solves. Treat 3D sketch curves as consumers of the P2.4 curve contract.

The viewport needs plane/depth locks, XYZ handles, snapping, dimension placement
and an explicit active workplane. Test spatial paths, zero-length entities,
underconstrained motion, conflicting dimensions and edits over the Phase 1 scale
range. A set of unconstrained XYZ fields alone is not completion of 3D sketching.

## P2.6 — Sweep and loft

Sweep starts with a closed profile or circular section along a connected path,
boss/cut mode and body scope. Follow with profile orientation, twist and guide
controls. Validate profile/path attachment, path continuity, self-intersection and
closed-path seams. Thin/open-profile modes need explicit thickness semantics.

Loft starts with ordered closed profiles, ruled/smooth modes, seam alignment and
boss/cut mode. Follow with profile synchronization, guide/centerline controls,
start/end constraints and closed lofts. Preserve the user's section ordering and
allow reversal without depending on arbitrary kernel edge order.

Prototype-backed implementations must distinguish internal surface smoothness
from continuity to a selected adjacent face. Do not expose a continuity option
until it is measured and tested. Carry source profile/path provenance into naming,
including generated caps and side faces. Test edits to intermediate sections,
guide curves and downstream fillets as well as final volume and validity.

## P2.7–P2.10 — Advanced feature families

| Family | Implementation sequence and UI requirements | Required geometry checks |
| --- | --- | --- |
| Draft | Neutral plane, selected faces, pull direction and angle first; parting-line and step modes next | Expected taper, correct sign, fixed neutral boundary, rejected self-intersections |
| Rib | Open sketch, thickness side, direction, extension to surrounding material and merge scope; draft option after draft support | Connected web, correct thickness, missing-support and zero-thickness errors |
| Dome | Selected face, height/direction and documented boundary/tangency controls; expand beyond basic circular/planar cases only through tested algorithms | Boundary attachment, height, watertight solid and downstream booleans |
| Variable fillet | Ordered radius stations along normalized edge/chain length, endpoint radii, interpolation and tangent propagation | Positive radii, station order, measured radius variation and corner failures |
| Face fillet | Two selected face sets, radius, trim/keep behavior and orientation; implement independently of existing face-to-edge selection | Actual face-set blend, continuity, trimming, gaps and invalid combinations |
| Boundary | Curves in two directions, section ordering, endpoint continuity and boss/cut integration; build/sew/cap valid solids | Both curve families constrain result, junction continuity and solid validity |
| Wrap | Explicit mapping on analytic cylindrical/conical targets first, then supported freeform targets; emboss/deboss depth and scribe modes | Seam handling, orientation, bounded mapping error and scribe volume invariance |
| Threads | Helical profile sweep with internal/external, pitch, diameter, length, handedness and start controls; documented profile tables | Pitch, major/minor diameter, valid root/crest and termination geometry |
| Cosmetic threads | Persistent metadata referencing cylindrical faces, lightweight viewport display and editable parameters | No mass/solid geometry change; references and metadata survive edits/save/load |

Boundary, face fillet, dome and freeform wrap are the highest uncertainty items.
Their P2.0 prototypes must establish algorithms and supported input domains before
full UI work. Boundary cannot be completed by relabeling a loft; face fillet cannot
reuse the existing “all edges of these faces” behavior as its final implementation.
Prototype failure leaves the corresponding milestone open.

Link cosmetic thread information to compatible tapped-hole definitions without
silently turning every tapped hole into modeled geometry. Drawing representation
belongs to Phase 4; preserve sufficient thread metadata for that later consumer.
Generic solid-tool sweep cuts, automated feature repair and specialized fillet
profiles remain separate catalog extensions unless explicitly added to this phase.

## Code integration map

| Area | Primary files and intended changes |
| --- | --- |
| Kernel | `crates/of-kernel-sys/{src/lib.rs,cpp/ofk.h,cpp/ofk.cpp}`: operation bridges, validation, multi-input history, diagnostics; split C++ implementation files as needed |
| Geometry | `crates/of-geom/src/`: safe curve, sweep, loft, split and blend wrappers; keep kernel indices internal to this layer |
| Sketching | `crates/of-sketch/src/`: separate 3D entities/constraints and shared numerical helpers |
| Model | `crates/of-model/src/{lib.rs,features.rs,build.rs,resolve.rs,naming.rs,file.rs}`: IDs, references, effective parameters, configurations and migrations |
| Feature operations | Split `crates/of-model/src/ops.rs` into focused modules as families land; share body scope/history application rather than duplicating it |
| Core clients | `crates/openforge/src/{session.rs,views.rs,mcp.rs,main.rs}` and `crates/of-protocol/src/lib.rs`: body/curve scenes, parameter/configuration methods, command parity and capability/version reporting |
| Desktop | `protocol.ts`, `store.ts`, `FeatureTree.tsx`, panels, toolbar commands and viewport modules under `apps/desktop/src/renderer/src/`: selection, ordered inputs, preview, body tree, equation/configuration editors and 3D sketch interaction |
| Tests/docs | Kernel tests, model tests, RPC tests, `scripts/desktop-smoke.mjs`, fixture corpus, protocol documentation and a new `docs/phase-2-status.md` maintained during implementation |

Keep existing generic feature RPC methods for new `FeatureKind` variants. Add
typed parameter/configuration methods rather than bypassing model validation with
arbitrary document writes. Include scene/protocol capability negotiation so an
older client cannot silently misinterpret multiple bodies. Update MCP tool schemas
and CLI examples alongside methods, not after desktop completion.

The built-in conversational assistant follows the separate
[AI implementation plan](ai-implementation-plan.md). Its API-key/local-model
connections, shared desktop session and draft transactions are additional work;
the tool exposure described here does not constitute that assistant.

## P2.11 — Acceptance and release

For every behavior change, add a regression at the lowest useful layer. Feature
coverage includes valid/invalid inputs, preview versus commit, dependency errors,
edit, suppression, reorder, rollback, undo/redo and save/reopen. Compare full and
incremental rebuilds. Verify that new naming preserves the intended reference or
fails explicitly when topology genuinely changes.

Build a small reproducible part corpus: swept pipe, lofted adapter, constrained 3D
path, drafted/ribbed housing, domed cover, variable/face-blended part, two-direction
boundary part, wrapped sleeve, threaded boss/hole and split/combined fixture. Add
configuration variants driven by equations. Record expected volumes/bounds or
tolerance-based surface measurements and downstream operations, not screenshots
alone. Cosmetic-only operations must leave mass properties unchanged.

Extend native desktop smoke coverage for body selection/scope, ordered profile/path
inputs, 3D sketch input, equation diagnostics and configuration switching. Exercise
preview/cancel, commit, undo/redo, save/reopen and STEP/STL export in integrated
workflows. Record representative rebuild/preview times and high-cost thread cases
before setting performance budgets; discard stale previews and keep UI error states
responsive throughout.

Run the repository gates against the release candidate:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
pnpm lint
pnpm format:check
pnpm typecheck
pnpm build
```

Run the documented native Electron smoke tests and require hosted macOS, Windows
and Linux workflows on that same candidate. Publish the exact supported modes and
remaining limitations in `phase-2-status.md`. Merge `phase-2` to `main` only when
the required Phase 2 scope and hosted checks are complete.
