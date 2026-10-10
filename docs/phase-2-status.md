# Phase 2 implementation status

Updated 2026-10-10. Work is on `phase-2`, based on the Phase 1 merge `8fff08d`.
**Phase 2 is in progress, not release-complete.** The roadmap and the remaining
acceptance criteria in [the implementation plan](phase-2-plan.md) still apply.

## Implemented checkpoints

These operations have core implementations, desktop panels and regression tests.
New features use the shared add/update/preview RPC methods, including MCP.

| Area | Available behavior | Limits / remaining work |
| --- | --- | --- |
| File and protocol | V2 files with V1 fixture migration, ID validation, protocol/scene handshake, revision-checked previews | Cross-platform contract fixture expansion and hosted CI remain |
| Bodies | Stable lineage-based IDs, qualified topology refs, body tree, name/visibility, solid feature scope | Aggregate `State.body` remains as a derived compatibility cache; broader import/pattern/topology-change corpus remains |
| Body operations | Union/subtraction/intersection, rigid move/copy, plane split with both/positive/negative retention | Split accepts at most one resulting solid per side per input body; tool-based splitting and arbitrary piece selection remain |
| Equations | Typed arithmetic/functions/units, global variables, persistent parameter keys, cycle diagnostics, equation editor | Registered scalar fields and sketch dimensions only; inline formula controls, optimized invalidation and broader parameter coverage remain |
| Configurations | Create/duplicate/rename/delete/activate, numeric/formula/variable/suppression/material overrides, undo, persistence, export by ID | Panels edit shared base values; the separate override editor edits a variant. No derived inheritance or spreadsheet tables |
| Construction curves | Parametric helix, ordered/reversed composite paths, directional projection onto a face/body | No variable-pitch helix, projected-sketch intersection, silhouette mode, contour picker or persistent per-segment curve IDs yet |
| Split line | Project a curve onto a selected face and partition it with kernel history | Projection mode only; changed split-face identity reports missing refs instead of silently rebinding |
| Sweep | Closed sketch profile along a sketch/entity/edge/construction path; minimum-twist, Frenet or fixed frame; boss/cut and scope | One contour without holes; twist, guides, thin/open sections and advanced seam controls remain |
| Loft | Ordered closed sketch profiles, reversal, ruled/smooth, boss/cut and scope | One contour per sketch without holes; guides, centerlines, explicit seam correspondence, endpoint continuity and closed lofts remain |
| Draft | Selected faces, neutral plane, pull direction and signed angle | Parting-line/step draft remain |
| Variable fillet | One edge with ordered normalized radius stations, reversed parameterization | Chain editing, interpolation controls, tangent propagation controls and measured-radius corpus remain |

Construction curves are selectable viewport paths. Hidden bodies stay in mass
properties, modeling scope and exports. Invalid features preserve the preceding
valid geometry and report an editable feature error. Exports reject a configuration
with active feature errors.

Configurations resolve into temporary definitions; their solved dimensions and
materials do not overwrite the base document. Changes involving formulas or variants
conservatively rebuild the full history. Editing previews use the same resolution
rules, including equations that make a later parameter drive an earlier feature.

## Required feature families still open

- Constrained 3D sketches and their viewport editing tools (P2.5).
- Rib and dome; advanced draft modes (P2.7).
- Face-set fillets and the remaining variable fillet controls (P2.8).
- Two-direction boundary features and wrap (P2.9).
- Modeled and cosmetic threads (P2.10).
- Remaining curve/sweep/loft modes, difficult-kernel feasibility measurements,
  reference stability corpus, performance measurements and release gates (P2.11).

The difficult boundary/face-fillet/dome/wrap prototypes are not complete. No
continuity or supported-input claim is made for those families. Their toolbar
entries remain unavailable. This is an implementation checkpoint, not a scope
reduction or permission to merge Phase 2 into `main`.

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

Local checks passed on this working tree on 2026-10-10:

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
