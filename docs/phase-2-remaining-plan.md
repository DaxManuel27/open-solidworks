# Phase 2 completion plan

Prepared 2026-10-10 against `phase-2` at `aaaa94d` and the current working tree.
Status: complete, 2026-10-12; see [Phase 2 status](phase-2-status.md). The [first measured checkpoint](phase-2-kernel-feasibility.md)
delivers initial R0 experiments and the R1 sketch-contour path slice. The milestones
below are not completion claims; remaining acceptance criteria still apply.
The owner has prioritized finishing advanced part modeling and deferred the AI
assistant. This document supplies the execution order for the remaining work in
[the Phase 2 specification](phase-2-plan.md). Its acceptance criteria remain in force.

## Scope and baseline

Keep the delivered body, equation, configuration, curve, sweep, loft, draft and
variable-fillet implementations. Complete their missing modes alongside the missing
feature families; do not restart those foundations. [Current status](phase-2-status.md)
records their limitations. The current code also has generated tool schemas,
TypeScript types, protocol tables and common result envelopes; extend those systems
instead of introducing another command catalog.

Defer assistant connection debugging, provider/OAuth changes, chat improvements,
live-model benchmarking and new AI-specific integrations until Phase 2 is complete
and the owner resumes that track. Preserve existing assistant code and uncommitted
work. Keep deterministic offline regression checks working. A new CAD operation
must still be available through the shared Rust core and existing CLI/JSON-RPC/MCP
interfaces; that parity is CAD work and requires no model connection. New live-session
MCP transport and AI orchestration are outside this completion sequence.

## Execution order

Each row is a milestone, normally delivered in several reviewable changes. Start
with R0, then follow the listed order. Dependencies describe technical prerequisites,
not a request to start parallel agents. Do not schedule a release date until R0
establishes the difficult geometry paths and R1 establishes reference/migration cost.

| Step | Remaining scope | Prerequisites | Exit evidence |
| --- | --- | --- | --- |
| R0 | Baseline and geometry feasibility (P2.0) | Current checkpoint | Repeatable baseline; measured prototypes for boundary, face fillet, dome, wrap and guided sweep/loft |
| R1 | Persistent curves, contours and general body splits (P2.1/P2.4) | R0 | Stable segment/piece references and selectable ordered contours survive edits and reload |
| R2 | Equations and configuration editing completion (P2.2/P2.3) | R1 contracts | Formula-aware fields and explicit edit scope; full/incremental rebuild equivalence |
| R3 | Constrained 3D sketching (P2.5) | R1, R2 | Dimensioned spatial path edited through viewport controls with solver diagnostics |
| R4 | Complete sweep, loft and curve modes (P2.4/P2.6) | R0, R1, R3 | Guided/thin/twisted sweep and controlled loft corpus, including spatial paths |
| R5 | Advanced draft, rib and dome (P2.7) | R0, R1, R2 | Editable drafted/ribbed housing and domed cover with valid downstream geometry |
| R6 | Variable and face-set fillets (P2.8) | R0, R1 | Measured variable-radius chains and independently constructed face-set blends |
| R7 | Boundary and wrap (P2.9) | R0, R1, R4 | Both boundary directions constrain the solid; wrap mapping and seams verified |
| R8 | Modeled and cosmetic threads (P2.10) | R1, R2, R4 | Internal/external thread geometry and persistent cosmetic representation |
| R9 | Integrated qualification and release (P2.11) | R0–R8 | Complete corpus, performance report and hosted checks on one release candidate |

## R0 — Establish the baseline and resolve geometry risks

1. Record the candidate commit, local modifications, installed kernel version and
   build environment. Run the repository gates and native desktop smoke with an
   isolated test profile. Save fixtures for existing Phase 1 and Phase 2 behavior.
   Earlier status notes are historical evidence, not proof that this candidate passes.
2. Create compiled experiments inside the existing kernel/geometry boundaries for
   two-direction boundary construction, face-set fillets, dome, analytic/freeform
   wrap and guide-controlled sweep/loft. Verify candidate APIs against installed
   headers before selecting an algorithm.
3. For each experiment record inputs, validity, solid count, boundary attachment,
   measured positional/angular continuity where applicable, naming history, downstream
   boolean/fillet behavior and execution time. Define scale-aware numerical tolerances
   in the fixture before judging its result; a mesh screenshot is insufficient.
4. Check both valid and failing examples: seams, degeneracy, nonintersecting guides,
   concave targets and self-intersections. Write results and chosen algorithms in
   `docs/phase-2-kernel-feasibility.md` when experiments exist.

A failed prototype leaves its family open. Investigate another algorithm or record
the precise blocker; do not silently replace a boundary with a loft or a face fillet
with an edge fillet. Reducing required scope needs a separate owner decision.

## R1 — Finish reusable geometry and selection contracts

- Add persistent segment and contour references, ordered selections, reversal and
  explicit start/seam selection. Track source ownership through rebuilds, deletion,
  rollback and patterns. Ambiguous topology changes must report unresolved references.
- Replace whole-sketch-only profile inputs with explicit outer/inner contour selection
  where supported. Define validation for connected paths, holes, branching and closure
  once, and reuse it in sweep, loft, boundary, rib and wrap panels.
- Extend split from one solid per plane side to face/tool splitting and arbitrary
  retained-piece selection. Use provenance-based piece identity, including split-count
  changes; never bind saved selections solely by enumeration order.
- Expand imported/touching/disjoint body and pattern cases. Audit the aggregate body
  compatibility cache: retain it only as derived data, with no competing ownership.
- Make old V1/V2 fixtures readable. Default only fields with equivalent old semantics;
  bump the file/protocol version when old readers would otherwise misinterpret data.
  Regenerate schemas/types and add cross-language contract fixtures with each change.

Exit: split a tool-cut part into multiple pieces, retain selected pieces, copy/combine
them and edit the upstream tool. Unaffected identities and total retained volumes
must be correct. Ordered contour selections must survive save/reopen.

## R2 — Complete equations and configuration workflows

- Introduce shared numeric/formula controls showing authored expression, evaluated
  value and units. Register remaining applicable scalar feature parameters, including
  new families as they land; reject unsupported or dimensionally invalid bindings.
- Make base-versus-active-configuration editing explicit. Route variant edits through
  the existing override model, with reset-to-base behavior and no base-value leakage.
- Consolidate dependency discovery for parameters, geometry and configuration overrides.
  Rebuild from the earliest affected feature where safe; compare with a full rebuild.
  Keep a conservative full rebuild for cases whose dependency safety is not proven.
- Test cycles, deleted parameters, units, integer fields, failed variants and A→B→A
  switching through both core and real desktop controls.

Derived configuration inheritance and external spreadsheet design tables remain
outside the manual-configuration acceptance scope, as in the original specification.

## R3 — Deliver constrained 3D sketching

1. Add a distinct `Sketch3D` entity/constraint model in `of-sketch`, retaining the 2D
   plane-based model. Start with points, lines, cubic splines and construction entities.
2. Add coincidence, fixed, distance, axis alignment, parallel/perpendicular and
   point-on-plane constraints, followed by spline endpoint tangency and point-on-curve.
   Report degrees of freedom and conflicts; a failed solve preserves valid geometry.
3. Add active workplanes, plane/depth locks, XYZ handles, coordinate entry, snapping,
   dimension placement and clear selection state in the viewport. Connect the resulting
   paths to R1 curve references and the existing feature history.
4. Test underconstrained movement, contradictory constraints, zero-length entities,
   scale extremes, referenced geometry edits and 2D solver regressions.

Exit: create, constrain, drag, dimension and reopen a spatial pipe path through the
desktop. Sweep along it and verify that subsequent dimension edits rebuild the part.

## R4 — Complete curves, sweeps and lofts

- Curves: finish height/pitch/turns input combinations, variable-pitch helix, projected
  sketch intersection and intersection/silhouette split-line modes. Reuse persistent
  segments and contour picking rather than creating another path representation.
- Sweep: circular sections, profiles with holes, explicit attachment/orientation,
  twist and guides, thin/open sections with thickness side, and closed-path seams.
  Keep boss/cut, merge and body scope consistent across modes.
- Loft: ordered contour sections, correspondence/seam editing, guides and centerlines,
  start/end continuity constraints and closed lofts. Validate section compatibility
  rather than leaving arbitrary kernel ordering to determine the result.
- Measure continuity before exposing continuity controls. Include reverse orientation,
  seam crossings, self-intersections and downstream cap/side references in fixtures.

Exit: a hollow swept tube, twisted guided part and lofted adapter remain valid after
path/guide/section edits, preview, undo and reload. The guide and continuity controls
must produce the measured effects stated by the UI.

## R5 — Finish draft, rib and dome

- Draft: add parting-line and step modes, neutral-boundary behavior, direction and
  signed-angle controls, with self-intersection and scope diagnostics.
- Rib: open-sketch support, thickness side, direction, extension to surrounding
  material, merge scope and draft. Reject missing support and zero-thickness webs.
- Dome: implement the R0 algorithm with height/direction and tested boundary/tangency
  controls. Cover circular/planar cases first, then the additional supported domains
  established by the prototype; keep unsupported selections explicit.

Exit: a housing with editable ribs/draft and a domed cover passes thickness, height,
volume, watertightness and downstream cut/fillet checks after upstream changes.

## R6 — Finish variable and face fillets

- Extend radius stations to ordered edge chains, interpolation and tangent propagation
  controls. Preserve radius-position meaning when chain orientation is reversed.
- Add actual two-face-set blends with radius, orientation and trim/keep behavior,
  using the independently validated R0 algorithm.
- Measure radius variation and continuity; test corners, gaps, very small radii,
  invalid face combinations and changed upstream topology.

Exit: the variable blend and face-set blend corpus survives supported edits and
save/reopen, and invalid inputs leave the preceding valid body available for repair.

## R7 — Deliver boundary and wrap

- Boundary: two ordered curve families, junction/correspondence validation, endpoint
  continuity and boss/cut integration. Sew/cap valid solids using the R0 approach.
  Changing either curve family must change the expected surface region.
- Wrap: cylindrical/conical mapping first, then the supported freeform domain from
  R0. Provide emboss, deboss and scribe, depth/direction controls and explicit seam,
  distortion and target-boundary diagnostics.

Exit: the boundary fixture measures the specified continuity and accepts downstream
operations. Wrapped sleeves pass orientation, depth and mapping-error checks across
seams; scribe partitions topology without changing volume.

## R8 — Deliver modeled and cosmetic threads

- Modeled threads: internal/external profiles, pitch, diameter, length, handedness,
  start controls and tested end termination. Add documented profile tables only after
  checking their public definitions during implementation; retain explicit dimensions.
- Cosmetic threads: persistent cylinder references and parameters, lightweight viewport
  representation and editing. Keep metadata compatible with tapped-hole definitions
  without automatically generating expensive solids. Drawing annotations remain Phase 4.
- Test pitch and major/minor diameters, root/crest validity, interference at termination,
  both handedness values, upstream cylinder edits and configuration overrides.

Exit: threaded boss/hole fixtures rebuild and export; cosmetic threads survive reload
and leave mass unchanged. Record modeled-thread rebuild cost before selecting budgets.

## Completion contract for every feature

A milestone is complete only when all applicable layers are delivered together:

- Kernel validation and safe geometry wrapper; no unsafe code outside `of-kernel-sys`.
- Parametric model, dependency discovery, persistent naming, body scope and migration.
- Shared tool schema, generated types/protocol, capability reporting and actionable
  error envelope. CLI/RPC/MCP must exercise the same model behavior as the desktop.
- Desktop creation, editing, selection, units, validation, preview/cancel and help.
- Tests for valid/invalid inputs, preview/commit equality, upstream edits, suppression,
  reorder/rollback, undo/redo, save/reopen, configurations and STEP/STL where applicable.
- Updated status with exact supported inputs and limitations; no placeholder advertised
  as complete and no automatic rebinding to unrelated topology.

Split changes by feature/mode into reviewable vertical slices. Reuse the existing
`of-model` body/curve/equation/configuration modules, add focused operation modules as
needed, and extend `crates/openforge/src/tools` rather than hand-editing generated
`apps/desktop/src/shared/tools.gen.ts` or `docs/protocol.md`. Extend viewport/panels and
`scripts/phase-2-smoke.mjs` with the same feature slice.

## R9 — Qualification and release gate

Build the integrated corpus incrementally in R1–R8: multibody fixture, equation-driven
variants, constrained spatial path, swept tube, lofted adapter, ribbed housing, domed
cover, variable/face blend, boundary solid, wrapped sleeve and threaded boss/hole.
Record numeric expectations and tolerances, reference identities, downstream behavior
and expected failure diagnostics. Include V1 migration and existing V2 parts.

Measure cold/warm rebuilds, preview latency, peak memory and expensive thread/guide
cases on a recorded machine and kernel build. Use repeatable runs to set explicit
budgets, then gate regressions. Exercise stale previews, cancellation and recovery from
failed geometry without a model connection. Verify matching desktop/core versions and
clear startup errors so a stale development process cannot mask test results.

Required checks on the exact release candidate:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
cargo run -q -p openforge -- generate --check
corepack pnpm lint
corepack pnpm format:check
corepack pnpm typecheck
corepack pnpm build
corepack pnpm bench
corepack pnpm test:assistant
corepack pnpm --filter @openforge/desktop smoke
```

The existing assistant fixtures are offline compatibility checks, not resumed AI
development or live-provider release requirements. Extend the current hosted
macOS/Windows/Linux workflow with new fixtures and retain its platform-specific
native smoke invocation. Require successful hosted runs on the same candidate,
not only a local pass or workflow definition.

Before merging Phase 2, reconcile every original P2.0–P2.11 criterion with its test
and supported-input documentation, publish measured performance and list any remaining
out-of-scope catalog extensions. Required failures keep Phase 2 open. Resume assistant
work only as a separate owner-directed track after this gate.

## First implementation batch

The R0 baseline, bounded geometry experiments and first R1 contour-path selection
slice are recorded in the feasibility report. Continue with ordered/seam-aware
profile selection, persistent generated-curve segments and general body splitting.
Do not treat the successful bounded prototypes as completed desktop feature families.
