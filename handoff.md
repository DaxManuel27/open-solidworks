# Handoff: Phase 4 drawings (2026-10-15, session 8)

Read this first. Earlier sections below are history.

## State

- Branch `phase-4`, nothing pushed. Every Phase 4 milestone (P4.0–P4.8) is implemented
  and verified locally; see `docs/phase-4-status.md` for what was built, how it was
  verified and the recorded limits. Session 8 commits on top of `c2d85af`: annotations
  and tables (`64810e2`), threads and DXF 2018 (`be2eb65`), the desktop drawing UI
  (`be900aa`), conformance and budgets (`0150cf6`), and the docs commit.
- The only open gate is hosted three-platform CI on the release candidate, which needs
  `phase-4` pushed (owner's call per the branching rule), then the merge into `main`.

## Verified on this tree

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`,
`openforge generate --check`, `pnpm lint`, `format:check`, `typecheck`, `build`,
`test:assistant`, `conformance` (98/98), `perf:draw:check`, and the full native desktop
smoke (`pnpm --filter @openforge/desktop smoke`, now with the drawing scenario). Not rerun
this session: `perf:check`, `perf:asm:check`, `bench`.

## Where things live

| Area | Files |
| --- | --- |
| Annotation model | `of-drawing/src/annotations.rs` (kinds; `Annotation.attach` is the view it moves with, named apart from kinds' own `view` because kinds are flattened in) |
| Drawing annotations | `of-drawing/src/annot.rs` (draw every kind, model items, driving dimensions, hole text, auto balloons, tables' cells), `dims.rs` (dimension model, value text, drafting), `symbols.rs`, `tables.rs`, `fits.rs` (ISO 286), `locate.rs` (`ViewRef` → model geometry) |
| Output | `pdf.rs`, `dxf.rs` (R12 + `DxfSheet`), `dxf2018.rs` (AC1032: model space at true size, layouts, viewports) |
| Core | `openforge/src/drawing.rs` (all `drawing.*` methods), params in `tools/drawing_params.rs`; part `doc.properties.set` / `doc.dimension_display.set` in `session.rs` |
| Desktop | `renderer/src/drawing/` (`SheetCanvas.tsx`, `state.ts` sheet store + pick routing, `SheetTabs.tsx`, `DrawingTree.tsx`, `panels/{common,sheet,views,dimensions,annotate}.tsx`); ribbon in `toolbars/{commands,layout}.ts`; `store.ts` `adoptDrawing`, `makeDrawing`, `print`; main process `.ofd` dialogs, `window:drawing`, `print:*` |
| Tests | `of-drawing/tests/annotations.rs`, `output.rs` (DXF 2018 structure), area 15 in `scripts/catalog-conformance.mjs`, `scripts/drawing-smoke.mjs`, `scripts/phase-4-performance.mjs` + `docs/phase-4-budgets.json` |

## Gotchas

- Drafting symbols (⌀ ⌴ ⌵ ↧ □, GD&T) are strokes, not text: text checks compare with the
  symbols removed (`plain()` in tests and conformance).
- Imported model dimensions store `at` relative to the middle of what they measure, so
  they follow model changes; other annotations with `attach` are relative to the view
  origin (the resolved one, reported as `views[].origin` by render).
- Panels route sheet clicks through `useDrawing().target` (`useSheetPicks`); the smoke
  calls `target.onPick` directly. `window.__openforgeDrawing` exposes that store.
- `openPanel` with `editing` must not call `edit.begin` in drawings (it is the view or
  annotation ID there); switching document kind clears the open panel.
- pnpm: `corepack enable --install-directory <scratch>/tools` and put it on PATH.

## Next steps

1. Push `phase-4` (owner's call) and run hosted CI on macOS, Windows and Linux. Likely
   attention points: the font import from `crates/of-drawing/assets` in the renderer
   build (Vite `?url` outside the app root), DXF/PDF paths on Windows, and the drawing
   smoke's new window under Xvfb.
2. Record the run in `docs/phase-4-status.md` (Hosted CI) and, with the owner's
   confirmation, merge `phase-4` into `main`.

# Handoff: Phase 4 drawings (2026-10-12, session 7)

Read this first. Earlier sections below are history (session 6 has the of-drawing
module map, still accurate).

## State

- Branch `phase-4`, nothing pushed. This session's commits on top of `bb7ff18`:
  `4208046` render integration tests, `8225bd3` HLR timings + auxiliary-view fix, and
  the P4.1 core commit (drawing session, methods, CLI, catalog, protocol v5).
- P4.0 is done except **part file v5** (per-dimension "show in drawing" flag and
  display names), which is not started. HLR decision is final: exact `HLRBRep_Algo`
  only (measurements and budgets in `docs/phase-4-plan.md` → P4.0).
- P4.1 core is done; P4.1 desktop is not started.

## Done this session

| Area         | Where                                                                                                                 | Notes                                                                                                                                                                                                                                  |
| ------------ | --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Render tests | `of-drawing/tests/render.rs`, helpers in `tests/common/mod.rs`                                                        | 3-view first/third angle with alignment, edge refs, hidden lines, title block fields (incl. mass/material), cache hits, hidden edges/components, note leaders, assembly + exploded views, auxiliary views, PDF/DXF of a rendered sheet |
| HLR timings  | `of-drawing/tests/hlr_timing.rs` (ignored; run with `--ignored --nocapture`, add `--release`)                         | Shelled, filleted housing and a 23-part gearbox stand-in (no gearbox fixture existed in the repo). Exact beats preview everywhere                                                                                                      |
| Aux view fix | `of-drawing/src/model.rs` `Sources::locate`; `Drawing::resolve`/`anchor_point` now take `&mut self`                   | Auxiliary views use the drawing's model cache instead of loading a fresh `Sources` per resolve                                                                                                                                         |
| Core session | `openforge/src/drawing.rs` (`DrawSession`), routing in `session.rs` (`drawing.new`, `.ofd` `doc.open`, `for_drawing`) | Undo/redo of whole documents; model paths checked and stored relative once saved; view delete cascades to derived views and drops leaders to them                                                                                      |
| Methods      | `drawing.info/render/sheet.*/view.*/annotation.*/properties.set`, `export` to `.pdf`/`.dxf`                           | Documented in `docs/protocol.md` → Drawings                                                                                                                                                                                            |
| Catalog/MCP  | `tools/drawing_params.rs`, 12 tools in `tools/mod.rs`; fixture `example_drawing` + `example_for`                      | Every example replays; envelopes trim drawing snapshots like part snapshots                                                                                                                                                            |
| CLI          | `main.rs` `drawing_info`; `export` works for `.ofd`                                                                   | `openforge info x.ofd`, `openforge export x.ofd out.pdf`                                                                                                                                                                               |
| Protocol     | `PROTOCOL_VERSION` 5 (scene stays 4), `CoreInfo.drawing_file_version`, capability `drawings`                          | Desktop check in `store.ts` updated to 5/4                                                                                                                                                                                             |
| Tests        | `openforge/tests/drawing.rs` (serve + CLI)                                                                            |                                                                                                                                                                                                                                        |

## Verified on this tree

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`,
`openforge generate --check` (via the generate test), `pnpm format:check`, `lint`,
`typecheck`, `build`, `test:assistant`, `conformance` (76/76). Not rerun: `perf:*`,
desktop smoke.
Hosted CI has not run for `phase-4`.

## Known gaps

- Silhouette lines of curved faces have no `entity` (HLR outlines carry no source),
  so in assembly views they cannot be picked to a component. P4.1 desktop picking
  needs either a face trace for outlines in `of-geom/src/hlr.rs` or an owner
  component on `Owner::View`.
- Section and detail views still return "not drawn yet" (P4.3); breaks, crop,
  broken-out, view labels and shaded styles are not drawn.
- No "Make Drawing from Part" (standard 3 views in one call) yet: P4.2.
- Conformance area 15 and `docs/phase-4-budgets.json` do not exist yet.
- `docs/phase-4-status.md` does not exist yet.

## Next steps

1. P4.1 desktop: `.ofd` windows (open/save dialogs accept `.ofd`; a drawing window
   runs its own core), sheet tabs, drawing tree, 2D canvas rendering
   `drawing.render` graphics (DejaVu via `@font-face` from
   `crates/of-drawing/assets/fonts/`; text items carry em size and width), pan/zoom,
   picking by `owner`, and reload on part save. Decide the outline-ownership gap
   first.
2. Part file v5 (P4.0 leftover).
3. P4.2 → P4.8 per `docs/phase-4-plan.md`.

## Gotchas

- Session 6 gotchas still apply (HLR frame, seams on silhouettes, em vs cap height).
- A cylinder's silhouette that coincides with its seam is traced to the seam edge;
  the opposite silhouette has no source. Tests allow either.
- `Drawing::render` rehashes model files on every call, so edits on disk show up
  without a reload method; the HLR cache key includes the part hash.
- The ISO title block has no `$FILENAME` cell.

# Handoff: Phase 4 drawings (2026-10-13, session 6)

Read this first. Earlier sections below are history.

## State

- Phase 3 is merged into `main` (PR #4, `bfd9a03`). Hosted CI on `main` failed at clippy 1.99
  (`clone_on_copy` in cxx code for `StepNode`); fixed on `main` by `229da3b` and run 38327262287
  passed on macOS, Windows and Linux (recorded in `docs/phase-3-status.md`).
- Branch `phase-4`, created from `main` after that fix. Nothing pushed. Commits so far:
  `67a5e66` plan, `b028627` HLR kernel + Phase 3 CI record, then the `of-drawing` work below.
- Uncommitted UI work found on `phase-3` was parked on a local branch `ui-parity` (`65f2da8`,
  not pushed). It includes `docs/soliworks-ui.jpeg`, which must never be committed to a shipped
  branch (clean-room rule); remove it before that branch goes anywhere.
- Owner decisions (2026-10-13): **DXF only** (DWG deferred); **native Rust PDF writer**
  (`pdf-writer` + `subsetter`); font and HLR method on the agent's recommendation. All are
  recorded in `docs/phase-4-plan.md` → Decisions.
- Font decision taken: DejaVu Sans Condensed (Bitstream Vera licence, permissive; bundled at
  `crates/of-drawing/assets/fonts/` with its licence). No permissive font covers every drafting
  symbol, so ⌀ ⌴ ⌵ ↧ □, all GD&T characteristics and circled modifiers are drawn as strokes in
  `text.rs`.

## Done (P4.0 mostly, P4.1 started)

| Area          | Where                                                                                         | Notes                                                                                                                                                                                                                                                                                         |
| ------------- | --------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HLR kernel    | `of-kernel-sys/cpp/hlr.cpp`, `src/lib.rs` (`hlr`, `seam_edges`), `TKHLR` linked in `build.rs` | Exact `HLRBRep_Algo` or polygonal `HLRBRep_PolyAlgo`; edges by class (sharp, smooth, seam, outline) as lines, CCW arcs or polylines in view-plane coords                                                                                                                                      |
| HLR wrapper   | `of-geom/src/hlr.rs`, tests `of-geom/tests/hlr.rs` (5)                                        | `Shape::hidden_lines`; traces each edge to its model edge (nearest-to-viewer wins); drops seams unless they are the silhouette; removes hidden lines under visible ones (splits partly covered lines); straightens edge-on curves                                                             |
| `.ofd` schema | `of-drawing/src/document.rs`, `file.rs`                                                       | Sheets (sizes, orientation, scale, projection, format), views (model, projected, auxiliary, section, detail; origin = sheet position of the model origin; breaks, crop, broken-out, hidden edges/components), `ViewRef`/`Anchor`, shared `next_id`, `validate()`                              |
| Standards     | `standard.rs`                                                                                 | ISO/ANSI base + overrides: text and label heights, arrows, line weights, precision, dash patterns                                                                                                                                                                                             |
| Display list  | `graphics.rs`                                                                                 | `Graphics`/`Item`/`Prim` (line, arc, polyline, fill, text with em size), pens, layers, owners (format, view + `CompRef`, annotation)                                                                                                                                                          |
| Text          | `text.rs`                                                                                     | Font metrics via `ttf-parser`, capital-height sizing, multi-line layout, stroked symbols                                                                                                                                                                                                      |
| PDF           | `pdf.rs`                                                                                      | One page per sheet at true size, subset Type0/Identity-H font with ToUnicode, layers as OCGs, dashes scaled by weight, Béziers for arcs                                                                                                                                                       |
| DXF           | `dxf.rs`                                                                                      | R12 ASCII, mm, LTYPE/LAYER/STYLE tables, SOLID fills by ear clipping, `\U+XXXX` text escapes, sheets side by side                                                                                                                                                                             |
| Output check  | `of-drawing/tests/output.rs`                                                                  | Set `OPENFORGE_OUTPUT_DIR` to keep files. Verified by hand: MuPDF renders the PDF and extracts text, pypdf parses it, ezdxf audits the DXF with 0 errors and renders it                                                                                                                       |
| Formats       | `formats.rs`                                                                                  | Built-in `iso` (ISO 5457 border, zones, centering marks, ISO 7200 title block) and `ansi` (ASME Y14.1, tolerance block); `.ofdt` files hold the same `Format` data; fields `$PRP`, `$PRPSHEET`, `$SHEET(S)`, `$SCALE`, `$SIZE`, `$DATE`, `$FILENAME`, `$MATERIAL`, `$MASS`; projection symbol |
| View frames   | `views.rs` (5 tests)                                                                          | Y-up standard orientations, iso/dimetric/trimetric, first/third-angle projected views, `folded` for auxiliary/section                                                                                                                                                                         |
| Models        | `model.rs`                                                                                    | `Sources` reuses `of_assembly::cache::Cache`; assemblies solved per configuration, reloaded on file change; `Scene` = combined compound + pieces so compound edge index → `CompRef`                                                                                                           |
| Runtime       | `drawing.rs`                                                                                  | `Drawing` open/save (relative model paths), `resolve` (parent chains, alignment), HLR cache keyed by geometry/direction/accuracy, `render(sheet)` → `SheetRender` (graphics, view infos, frame, title block), notes with leaders                                                              |

Verified on this tree: `cargo fmt`, `cargo clippy -p of-drawing -p of-geom -p of-kernel-sys
--all-targets -D warnings`, `cargo test -p of-drawing -p of-geom` (all pass). The full workspace
gates, JS gates and conformance have **not** been rerun since `b028627`.

## Not done yet / known gaps

- `Drawing::render` has **no integration test on real part/assembly files** yet. Next step:
  add `of-drawing/tests/common` (copy the part builders from `of-assembly/tests/common`) and test
  a standard 3 view of a plate with a hole (third and first angle), alignment, edge `CompRef`s,
  title block fields, an assembly view (incl. exploded), cache hits (`hlr_runs`), and PDF/DXF of
  the rendered sheet.
- `Drawing::edge_direction` (auxiliary views) builds a fresh `Sources` each call; make `resolve`
  take `&mut self` or cache scenes instead.
- Section and detail views resolve their frames but `draw_view` returns "not drawn yet" (P4.3).
  Breaks, crop, broken-out, view labels and shaded display styles are not drawn yet.
- HLR performance has not been measured on the Phase 3 corpus (P4.0 target). Use the gearbox
  fixture from `scripts/phase-3-performance.mjs`; record exact vs preview timings in the plan.
- Part file v5 ("show in drawing" dimension flag + display names, P4.0) is not started.
- Nothing is wired into the core yet: no drawing document kind in `crates/openforge/src/session.rs`,
  no `drawing.*` RPC methods, CLI `info`/`export` for `.ofd`, MCP tools, protocol v5, or desktop
  2D canvas (P4.1).
- DXF is R12 only; R2000+ (lineweights, true colour, paper space) is planned for P4.7.
- `docs/phase-4-status.md` does not exist yet.

## Next steps

1. Integration tests for `Drawing::render` (above), then HLR timings → finish P4.0 in the plan.
2. P4.1 core: drawing kind in the session, `drawing.new/open/save/render/sheet.*/view.*`
   methods, `openforge export x.ofd out.pdf|out.dxf`, protocol bump, tool catalog + `generate`.
3. P4.1 desktop: drawing window, sheet tabs, 2D canvas rendering `Graphics` (DejaVu loaded via
   `@font-face` from the bundled TTF; text items carry em size and width), picking by `Owner`.
4. Continue P4.2 → P4.8 per `docs/phase-4-plan.md`.

## Gotchas

- HLR frame: `Plane::new(origin, x_dir, normal)` with `normal` pointing **at the viewer**; paper
  y = normal × x. The front view looks along −Z (model Y up).
- OCCT emits a cylinder seam instead of a duplicate silhouette when they coincide; that is why
  `seam_edges` keeps seams lying on the silhouette.
- `Prim::Text.height` is the **em size**; drafting heights are capital heights
  (`text::em_for` / `text::cap_for`). DXF writes capital height.
- Combined assembly shapes rely on each located child keeping its own edge order; two coincident
  instances of one part merge edges, so `Sources::scene` refuses that case.
- Python validation tools live only in the session scratchpad (venv with pymupdf, pypdf, ezdxf);
  recreate with `python3 -m venv venv && venv/bin/pip install pymupdf pypdf ezdxf matplotlib`.

# Handoff: Phase 3 assemblies (2026-10-12, session 5)

Read this first. Earlier sections below are history.

## State

- Branch `phase-3`, created from `origin/main` after the Phase 2 merge (PR #3), with the
  leftover Phase 2 docs commit `ff0bc10` merged in. No upstream is set; nothing is pushed.
- Everything is committed (`git log main..phase-3`). The only untracked file is
  `docs/soliworks-ui.jpeg`, which predates this work and must not be committed (clean-room
  rule: no SolidWorks screenshots).
- All Phase 3 milestones (P3.0–P3.8) in `docs/phase-3-plan.md` are implemented. What was
  delivered, how it was verified and the recorded limits are in `docs/phase-3-status.md`.
- Owner decisions: native Rust mate solver (`of-mate`) instead of OndselSolver
  (2026-10-12); `main` only moves when a phase is complete (merge after hosted CI and the
  owner's confirmation).

## Verified locally (macOS, this tree)

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`
(48 suites), `openforge generate --check`, `pnpm lint`, `format:check`, `typecheck`,
`build`, `bench`, `test:assistant`, `conformance` (76/76), `perf:check`, `perf:asm:check`,
and the native desktop smoke including the new assembly scenario.

Hosted three-platform CI has **not** run for `phase-3`.

## Next steps

1. Push `phase-3` (owner's call) and run hosted CI on macOS, Windows and Linux. Things most
   likely to need attention on other platforms:
   - Linking the XCAF libraries added in `crates/of-kernel-sys/build.rs`
     (`TKCDF`, `TKLCAF`, `TKCAF`, `TKXCAF`; legacy installs also `TKXDESTEP`).
   - `assembly.cpp` uses a pimpl for `StepAssembly`/`StepRead`; MSVC should be fine.
   - The desktop smoke now opens windows per document; Linux runs it under Xvfb.
   - `pnpm perf:asm:check` budgets were recorded on an M3 Max (debug); CI scales by 4.
2. Record the CI run in `docs/phase-3-status.md` (section "Hosted CI").
3. With the owner's confirmation, merge `phase-3` into `main` and push; then branch `phase-4`
   (Drawings) from `main`.

## Where things live

| Area              | Files                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mate solver       | `crates/of-mate/src/{lib,eval,solve,sparse}.rs`, tests in `crates/of-mate/tests/solve.rs`                                                                                                                                                                                                                                                                                                                                                            |
| Assembly model    | `crates/of-assembly/src/`: `document.rs` (`.ofa` types), `file.rs`, `cache.rs` (part loading, hashes), `assembly.rs` (instances, bodies, `CompRef` resolution, solve, insert/delete), `mates.rs` (mate kinds → solver terms, captured values), `patterns.rs` (patterns, mirrors, opposite hands), `analysis.rs` (interference, clearance, collision drag, mass), `explode.rs`, `bom.rs`, `hardware.rs`, `exchange.rs` (STEP structure export/import) |
| Kernel            | `crates/of-kernel-sys/cpp/assembly.cpp` (located shapes, common volume, XCAF writer/reader), wrappers in `crates/of-geom/src/assembly.rs`                                                                                                                                                                                                                                                                                                            |
| Part model        | `crates/of-model/src/properties.rs` (custom properties, part file v4); `MirrorDef.keep_original` in `features.rs`/`ops.rs`                                                                                                                                                                                                                                                                                                                           |
| Core              | `crates/openforge/src/asm.rs` (`asm.*` methods, scene, tree); routing in `session.rs` (`Session::handle`, `asm.new`, `.ofa` open, `asm.import_step`); params in `tools/asm_params.rs`; MCP tools in `tools/mod.rs`; CLI in `main.rs`                                                                                                                                                                                                                 |
| Desktop           | `src/main/index.ts` (window + core per document, `window:open`, `doc:saved`), `preload/`, `renderer/src/store.ts` (assembly state, `asmPick`, `asmMove`, `adoptAssembly`), `AssemblyTree.tsx`, `viewport/assembly-view.ts` (instancing, picking, triad), `viewport/move-state.ts`, `panels/{mates,assembly,asm-common}.tsx`, ribbon in `toolbars/{commands,layout}.ts`                                                                               |
| Tests and tooling | `crates/of-assembly/tests/{assembly,mechanisms,tools,hardware}.rs`, `crates/openforge/tests/assembly.rs`, `scripts/assembly-smoke.mjs`, `scripts/phase-3-performance.mjs` (+ `docs/phase-3-budgets.json`), area 13 checks in `scripts/catalog-conformance.mjs`                                                                                                                                                                                       |

## How it fits together

- An assembly component is a solver body; flexible subassembly children are bodies too
  (fixed children are locked to their subassembly). Rigid subassemblies are one body.
- Mates store values captured when they are made (`Captured`: alignment sign, side,
  lock relative pose, reference vectors, coupler phase and unwrapped motions) so later
  solves never flip or jump. `Assembly::add_mate` refuses a mate that can't be satisfied
  and names the conflicting mates.
- Pattern and mirror instances have keys like `2@p5.1` / `2@m6`; mates may reference them
  (treated as fixed during a solve, solved twice).
- The desktop gets one mesh per part (`parts`, keyed by path#hash#configuration) and
  instance transforms; `asm.move` with `phase: "move"` returns transforms only.
- `window.__openforgeStore` exposes the renderer store for the smoke test and DevTools.

## Gotchas

- `pnpm` is not on PATH here; use `corepack enable --install-directory <scratch>/tools` and
  prepend that directory to PATH (nested `pnpm -r` scripts need it).
- Protocol and scene version are 4; the desktop refuses other cores.
- Empty maps/lists in assembly JSON are omitted (`skip_serializing_if`); renderer types
  mark them optional (a missing `explode` crashed the tree once).
- Data exchange in OCCT prints to stdout, which carries JSON-RPC: every reader/writer must
  remove the console printers first (`quiet()` in `ofk.cpp`, `quiet_step()` in `assembly.cpp`).
- Collision detection samples the drag path; budgets for `collision_drag_step_ms` assume
  the 60-component fixture.

# Handoff: Phase 2 continuation (2026-10-12, session 4)

Branch `phase-2`, nothing committed. Earlier sections below are history.

## Done this session (core, RPC/MCP schema, desktop controls and tests for each)

- Step draft (`step` on parting-line drafts): level hinge through the parting line's nearest point, perpendicular step faces, planar faces.
- Loft: `centerline` (kernel `loft_centerline`, PipeShell through sections) and `start`/`end` conditions (`normal` or `direction`, `tangent_length`) via a new skinning routine (`cpp/skin.cpp`) that keeps rational sections exact when weights match.
- Sweep: `guide_scale` (one guide also scales the section) and `guide2` (two guides stretch it); exact sections at stations, refined until sampled profile points between stations are within 1e-5 × size (`sweep_scaled` in `skin.cpp`). Needs an open, tangent-continuous path.
- Wrap: cones (exact development about the apex) and other curved faces (geodesic exponential map from the point under the sketch origin; spheres are reparametrized away from their poles so dome apexes work; rejected with the measured value above 1% stretch). Emboss/deboss/scribe all modes.
- Dome on non-circular planar faces without holes: plate filling through boundary and apex, replacing the face in place; `elliptical` means tangent to side faces (needs a boundary without corners).
- R1: `seam` fraction inside the start entity of contour selections (circle seams start there; other entities split, the second piece named `….before`); projections interrupted by holes produce pieces named `#1`, `#2` in order along the source (face targets only); import face names qualified by a content hash (legacy index names remain aliases), so a different file reports missing references; corpus tests in `crates/of-model/tests/reference_corpus.rs`.
- Performance: four new fixtures, `--runs`, `--set-budgets`, `--check`; budgets in `docs/phase-2-budgets.json`; `pnpm perf`/`perf:check`; CI step with `OPENFORGE_BUDGET_SCALE: 4`.
- Conformance 65 checks (new: loft tangency/centerline/seams, scaling and two-guide sweeps, step draft, dome geodesic wrap, rectangular dome, projected pieces, holed-face dome rejection). Native smoke adds a tangent loft through the real panel.

## Verified locally

`cargo fmt --check`, clippy `-D warnings`, `cargo test --workspace`, `generate --check`, `pnpm lint`, `format:check`, `typecheck`, `build`, `bench` (10/10), `test:assistant`, `conformance` (65/65), `perf:check`, native desktop smoke.

## Hosted CI

`phase-2` is pushed. Run 38205003310 at `3ced10a` passed on macOS, Windows and Linux after three CI fixes: kernel sources include `of-kernel-sys/cpp/ofk.h` by one path (MSVC), `bench/results/reference.json` is committed and skipped by Prettier, and the serve tests wait for the core to exit before deleting their folder (Windows file locks).

## Remaining

Merge `phase-2` into `main` when the owner confirms the phase is complete. Out-of-scope extensions are listed in the last column of `docs/phase-2-status.md`. Commit only when asked; don't merge to main.

# Handoff: Phase 2 continuation (2026-10-11, session 3)

Branch `phase-2`, nothing committed. Earlier sections below are history.

## Done this session (all with core, RPC/MCP schema, desktop panel and tests)

- Fixes: formula syntax rejected in `equations.set`/`configuration.update` (preview still reports per field); convert entities accepts faces (all loops).
- New features: Rib (parallel/normal, sides, flip, draft), Wrap (cylindrical emboss/deboss/scribe, exact arc-length mapping across seams), Boundary (Coons-patch curve network), Face fillet (planar face sets), modeled Thread (ISO 68-1 profile), tapped-hole cosmetic threads.
- Extended: sweep circular profile/twist/guide (rotation)/thin; loft guide curves; parting-line draft; variable fillet over tangent chains with straight/smooth law; elliptical dome; helix height/pitch/turns combos and variable pitch; projected curve sketch-on-sketch; split line intersection/silhouette; associative 3D-sketch fixed/point-on-plane relations (`ref.locate` RPC).
- Equations: `=formula` entry in creation fields (`formulas` on feature.add/update, `equations.evaluate` RPC); rebuild cache keyed by effective feature inputs (no more full rebuild when equations exist).
- Kernel: `mass_props` integrates against an oblique plane outside the bbox (large speedup, same accuracy); fillet/chamfer serialized with a mutex (OCCT blend builder not thread-safe).
- Docs: `docs/features/*` status lines and `docs/mcp-baseline.md` updated.

## Verified

- All gates on the final tree: `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test --workspace`, `cargo build`, `generate --check`, `pnpm lint`, `format:check`, `typecheck`, `build`, `bench` (10/10), `test:assistant`, `conformance` (60/60), kernel probe regressions.
- Native desktop smoke passed 3/3 after fixing a smoke hang (renderer `confirm()` blocking on stale dirty state; Phase 2 scenarios now stub it; `js()` helper times out at 20 s).

## Catalog conformance

`scripts/catalog-conformance.mjs` (`pnpm conformance`, also in CI after the generator check) drives `openforge serve` with 60 checks across catalog areas 01–04, 06–08, 14, 17, 19–21, including the spec failure cases. 60/60 pass. Real bugs it found and fixed: closed-spline mass properties failed (integration now falls back), a closed shell as thick as half the part "succeeded" with no cavity (now an error), and Combine Add of non-touching bodies silently kept two bodies (now an error per the spec; `general_split` test updated).

## Also since session 3 began

- Native smoke has an advanced-feature scenario (`scripts/phase-2-advanced-smoke.mjs`: rib with `=formula` entry, modeled thread, boundary). The cosmetic-thread scenario now commits fields with Enter: it relied on `blur`, which Chromium suppresses when the window lacks OS focus, so it failed whenever the smoke window was in the background. The panel also uses functional state updates.
- `scripts/phase-2-performance.mjs` measures seven fixtures (adds thread, rib, wrap, boundary) with a feature-rebuild timing; results in `docs/phase-2-performance.md`. Modeled thread ≈1.4 s per rebuild (debug), others < 0.15 s.
- Closed lofts (`closed: true`, three or more sections, no guides).
- `docs/phase-2-status.md` rewritten for the current state.

## Remaining Phase 2 scope

Conical/freeform wrap; sweep guides that scale the profile and multiple guides; loft centerline and end constraints; step draft; domes on noncircular faces; R1 corpus items (pattern inheritance of generated curves, interior seam positions); performance budgets from repeated measurements; hosted three-platform CI. Commit only when the user asks; don't merge to main.

# Handoff: catalog conformance fixes

Date: 2026-10-11. Branch `phase-2`. Nothing is committed. These changes sit in the same dirty working tree as the earlier Phase 2 work described in the next section. The user asked to stop here.

## Background

The Phase 1 and 2 features were tested against the specs in `docs/features/`. 161 of 182 checks passed. The harness is a set of Python scripts that drive `target/debug/openforge serve` over JSON-RPC. It lives in a session scratchpad that may be deleted: `/private/tmp/claude-501/-Users-daxmanuel-Documents-Projects-open-solidworks/22b2c46d-0db9-461a-8be7-89b66f4879d4/scratchpad/conf/` (`core.py`, `suite.py`, `t_a.py`, `t_b.py`, `t_c.py`). Five real problems came out of it, and the user approved a plan to fix them. The user chose mitred corners for sweeps, matching SOLIDWORKS.

| #   | Problem                                                                                          | Status                                                                                          |
| --- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| 1   | A sweep along a sharp-cornered path built only the first segment, with no error                  | **Done**                                                                                        |
| 2   | Self-intersecting sweeps were accepted and could be exported                                     | **Done**                                                                                        |
| 3   | An extruded or other cut that removes no material succeeded silently                             | **Done**                                                                                        |
| 4   | `equations.set` accepts a formula with a syntax error; the feature fails and the body disappears | **Not started.** A first attempt failed before writing anything, so `equations.rs` is unchanged |
| 5   | Convert entities rejects a face and only accepts edges                                           | **Not started**                                                                                 |

## Done

### 1–2. Sweeps: `crates/of-kernel-sys/cpp/ofk.cpp`, `sweep()`

- Calls `op.SetTransitionMode(BRepBuilderAPI_RightCorner)`, so sharp path corners are mitred. The default transition stopped silently at the first corner.
- After `require_valid`, runs `BOPAlgo_ArgumentAnalyzer` with `SelfInterMode` on the result. On a fault it fails with "the swept body intersects itself; reduce the profile size, increase the path's bend radius or change the profile orientation". `BRepCheck_Analyzer` doesn't catch either folded or truncated pipes; this check does. With mitring disabled, it also catches the original truncation.
- Added `#include <BOPAlgo_ArgumentAnalyzer.hxx>`. TKBO was already linked.
- A fixed-orientation sweep around a corner whose profile ends up lying in the plane of travel is degenerate. The new check rejects it, which is correct.
- I tried an end-of-path check (compare the section's distance from the path at each end) and removed it: it never fired.
- New tests:
  - `crates/of-geom/tests/sweep_corners.rs` (4 tests): mitred L-paths for orientations 0 and 1, the fixed-orientation rejection, a 3D path with several corners, the self-intersection rejection, and a profile just inside the bend radius.
  - `crates/of-model/tests/sweep_corners.rs` (5 tests): 2D sketch, composite-curve and 3D sketch paths with corners (analytic volumes 35π and 45π), a hollow sweep around a corner, and the self-intersection feature error.
- **Not measured:** how much time the self-intersection check adds. Check it with `scripts/phase-2-performance.mjs` or the sweep tests.

### 3. Cuts that remove nothing: `crates/of-model/src/ops.rs`

- New `apply_feature_tool()` wraps `apply_tool()`. For a cut it compares the body volume before and after. If nothing was removed (relative tolerance 1e-9), it fails with `CUT_REMOVES_NOTHING` ("the cut does not remove material"). The constant is exported from `of_model` (`lib.rs`).
- Extrude, revolve, sweep, loft and hole use `apply_feature_tool`. Pattern and mirror copies still call `apply_tool` directly, so a copy that lands off the part is skipped rather than failing the pattern.
- `crates/openforge/src/tools/envelope.rs`: a rebuild error equal to `CUT_REMOVES_NOTHING` is reported as `E_NO_EFFECT` with the existing hint ("a cut on a face sketch usually needs reverse: true"), not `E_REBUILD_FAILED`. This keeps the assistant behaviour that the test `a_cut_that_removes_nothing_fails_and_blocks_the_draft` expects. The envelope's own `no_effect()` check still covers bosses that add nothing.
- New tests at the end of `crates/of-model/tests/features.rs`: `cut_that_removes_no_material_fails` (a missed cut, and a cut that only touches a face) and `patterned_cut_copies_off_the_part_are_skipped`.

### Verified

- `cargo test -p of-geom`, `cargo test -p of-model` and `cargo test -p openforge` all passed after fixes 1–3, including all the existing sweep, hollow-sweep, spatial, contour, dome and RPC tests.
- `cargo fmt -p openforge -p of-model` was run.
- **Not run yet:** workspace-wide `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo run -q -p openforge -- generate --check`, the desktop smoke tests, and a rerun of the conformance scripts.

## Remaining work

### 4. Reject formula syntax errors on entry

Plan: split the parser in `crates/of-model/src/equations.rs`, which currently evaluates while it parses (`Parser` at about line 140, `evaluate()` at about line 286), into two steps:

1. `parse(text) -> Expr`. This catches syntax errors, unknown function names and wrong argument counts (`min`/`max` take 2 arguments; `abs`, `sqrt`, `sin`, `cos` and `tan` take 1).
2. `Expr::eval(lookup)`, which does the unit arithmetic and must keep every current error message.

Keep these existing behaviours:

- A number followed by a length or angle unit name multiplies by that unit.
- A name followed by `(` is a function call; otherwise it's a constant or a variable lookup.
- The 64-level nesting limit.
- `checked()` on each result.

Then add `pub fn check_syntax(text)`, and call it on every variable and equation in `equations.set` (**not** `equations.preview`; the desktop needs preview to return per-field errors so it can disable OK, see `panels/equations.tsx:17`), and in `configuration.update`/`create` (`crates/openforge/src/session.rs`, `equations.set` handler at about line 374). Reject with `ErrorCode::InvalidFormula`, naming the parameter.

Don't reject when opening a file or rebuilding; existing files with bad formulas must still open. Circular references and missing variables stay as per-field errors.

Tests:

- A syntax error is rejected and the revision doesn't change.
- A configuration override with a syntax error is rejected.
- Preview still returns the error.
- The existing tests in `crates/of-model/tests/parameters.rs` still pass.
- `scripts/phase-2-equation-smoke.mjs` still passes.
- The assistant's offline tests still pass.

### 5. Convert entities accepts a face

In `convert_edges` (`crates/of-model/src/ops.rs`, about line 1630, "select model edges to convert"), expand a face reference into persistent references for all its boundary edges, outer and inner loops. Use `Shape::face_edges()` (`crates/of-geom/src/lib.rs:453`), as `edge_set` does for fillets, and the body's edge names. Remove duplicates when a face and one of its edges are both selected.

Tests: converting a box's top face and extruding gives the expected volume; a face with a hole converts both loops.

### Then

- Docs:
  - `docs/features/03-part-features.md`: "OpenForge today" for sweep (mitred corners, self-intersection error) and extrude ("cut does not remove material").
  - `docs/features/01-sketching.md:96`: wrongly lists convert entities as missing.
  - `docs/features/14-...md`: formula syntax rejection.
  - `docs/mcp-baseline.md`: says a wrong-direction cut "succeeds silently", which is no longer true.
- Run the full checks from AGENTS.md, the generator `--check`, the desktop smoke tests, and the conformance scripts.
- Commit to `phase-2` only when the user asks. Don't merge to `main`.

---

# Phase 2 handoff

Date: 2026-10-11

## Stop state

The user originally requested “Implement the rest of phase 2,” then explicitly requested: **“Stop now, create a handoff.md.”** Implementation was stopped. Do not resume without a new instruction.

- Actual Git repository: `/Users/daxmanuel/Documents/Projects/open-solidworks/open-solidworks` (nested inside the initial workspace directory).
- Branch: `phase-2`.
- No commits, merges, pushes, or releases were made during this work.
- All changes remain in the working tree. It contains substantial changes that existed before this session; do not reset, clean, or assume every diff belongs to this implementation.
- The three delegated agents (`general_split`, `profile_contracts`, `equation_workflows`) were interrupted or already finished.
- The known main verification command was interrupted with Ctrl-C, exit 130, while compiling during `cargo build`. Its preceding formatting, Clippy, and workspace test stages had passed. Its final generation check was not reached.
- **Phase 2 is not complete.** This is an implementation checkpoint with significant remaining feature and qualification work.

## Read first

- Repository `AGENTS.md` and [project vision](docs/plan.md).
- [Remaining implementation plan](docs/phase-2-remaining-plan.md): R0–R9 work breakdown.
- [Checkpoint and scope](docs/phase-2-checkpoint-2026-10-11.md).
- [Phase status](docs/phase-2-status.md).
- [General split details](docs/phase-2-r1-split.md).
- [Circular dome details](docs/phase-2-r5-dome.md).
- [Kernel feasibility](docs/phase-2-kernel-feasibility.md) and [existing probe results](docs/phase-2-probe-results.csv). The existing results are not qualification of the final working tree.

The original phase plan is broader than the implemented subset below. AI assistant work was deferred by the owner: preserve existing assistant behavior and offline tests; live provider work is outside this phase continuation.

## Implemented in the working tree

### Persistent profile, contour, and curve references

- Sweep and loft profiles accept backward-compatible whole-sketch selections or explicit `ContourSelection { sketch, entities, reverse, start }` values.
- Entity membership defines contour identity. Missing/replaced members require reselection rather than silently binding to another contour.
- Ordered sketch contours and persistent generated curve segments are first-class references. Segment identities derive from source entities, references, and roles rather than output enumeration.
- Ambiguous projected descendants do not receive misleading persistent segment bindings.
- Explicit loft sections preserve selected seam correspondence and require equal edge counts; automatic OCCT correspondence remains available for legacy selections.
- Desktop controls support explicit profiles, ordered paths, direction/start selection, and generated segment picking/highlighting.
- Current limit: a closed seam starts on an entity boundary; arbitrary interior seam angles are not implemented.

Key files: `crates/of-model/src/{curves,resolve,build,ops,lib}.rs`, `crates/of-geom/src/curves.rs`, kernel loft bridge, desktop `panels/advanced.tsx`, `panels/fields.tsx`, `store.ts`, and viewport `engine.ts`.

Tests include `crates/of-model/tests/profile_selection.rs`, contour/curve tests, and additions to `crates/openforge/tests/serve.rs`. One geometric check verifies that advancing a square loft seam changes volume from 1000 to approximately 666.6667 mm³.

### General body splitting

- `SplitBody.tools` accepts reference planes, bounded faces (including curved faces), and whole tool bodies. An empty tool list retains the legacy plane/keep behavior.
- Optional `retained_pieces` selects a nonempty set of persistent resulting body IDs; `None` retains all pieces. Duplicate/unresolved IDs fail explicitly.
- Piece identities derive from source/tool face ancestry, cutting-face orientation, creator, and parent. Ambiguous ancestry fails rather than using enumeration suffixes.
- Tool bodies remain in the model and cannot also be target bodies. Each selected target must actually split.
- General-tool mode requires `keep: Both`; its legacy plane field is ignored.
- New-feature preview uses `doc.next_id` without consuming it, so preview piece IDs agree with commit. Revision guards still protect stale requests.
- Desktop controls provide tool picking, all-piece previews, and retained-piece checkboxes.

Key files: `crates/of-model/src/bodies/split.rs`, `bodies.rs`, OCCT `BRepAlgoAPI_Splitter` bridge, desktop `panels/bodies.tsx`.

Tests: `crates/of-geom/tests/tool_split.rs`, `crates/of-model/tests/general_split.rs`, legacy multibody tests, RPC lifecycle coverage. `scripts/phase-2-split-smoke.mjs` passed in a native integrated run after fixing its Cancel selector to `.panel .cancel`.

Minor UI follow-up: a whole-body tool can appear both in the face/plane reference box and its body checkbox; functionality works, but presentation can be improved.

### Equations and configuration editing

- Shared equation operations support explicit base/active-configuration scope, atomic numeric parameter edits, nullable equations to disable inheritance, and revision checks.
- `equations.set` and `equations.preview` support scoped previews and reset-to-base without leaking configuration edits into base values.
- Existing feature editing offers definition editing or “Parameters and formulas.” Formula fields display authored values, evaluated values, and units.
- Registered scalar parameters were expanded, including variable fillet stations, projection direction, and spatial sketch dimensions (`fN.dimension.CONSTRAINT_ID`).
- Fixed stale parameterized geometry when the last equation is removed, plus missing/deleted-parameter propagation.
- Equation/configuration rebuilds conservatively perform full rebuilding; fine-grained dependency invalidation remains future work.
- Feature creation controls are still primarily numeric; inline formula authoring throughout creation is not complete.

Key files: `crates/of-model/src/equations.rs`, `build.rs` (`parameterized_cache`), tool parameter contracts, `panels/parameter-editor.tsx`, equation/configuration/feature panels, and disabled-preview handling.

Core parameter and scoped RPC tests passed. `scripts/phase-2-equation-smoke.mjs` passed native base/active scope, reset, cancel, invalid formula, and configuration A→B→A scenarios.

### Constrained spatial sketches

- Added `Sketch3D`, entities for points, lines, and cubic Béziers, plus construction geometry.
- Relations: coincidence, fixed point, distance, axis alignment, parallel, perpendicular, explicit point-on-plane, endpoint tangency, and point-on-curve.
- A damped numerical solver reports degrees of freedom, conflicting/redundant relations, and entity status. Failed solves preserve supplied geometry.
- Point-on-curve uses bounded auxiliary curve parameters and exact cubic residuals. Auxiliary-only freedom at stationary tangents is removed from geometric DOF reporting.
- Tangency checks endpoint coincidence and tangent direction, rejecting backtracking cusps.
- Validation covers shared unique IDs, finite values, valid references, nonzero line/end tangents, and positive distance dimensions.
- Spatial sketches provide connected, unbranched line/Bézier paths and stable source-entity segment references for downstream features. Invalid sources block dependent paths.
- Added pure `sketch3d.solve` RPC with optional dragging; generic feature operations commit sketches.
- Desktop editor includes XY/XZ/YZ workplanes with depth, grid/point snapping, coordinate editing, XYZ handles, plane/axis-constrained dragging, dimensions, and relation controls.
- Latest UI fixes apply optimistic geometry synchronously, reject stale solve replies after cancellation, and disable commit while solving.

Key files: `crates/of-sketch/src/spatial.rs`, `crates/of-model/src/{features,curves,resolve,build}.rs`, `crates/openforge/src/{session,views}.rs`, desktop `panels/spatial.tsx`, `viewport/spatial-state.ts`, and `viewport/spatial-sketcher.ts`.

Tests: `crates/of-sketch/tests/spatial.rs` (including scales 0.001, 1, 1000, and 100000), `crates/of-model/tests/spatial_paths.rs`, and `crates/openforge/tests/spatial_paths.rs`. Coverage includes dimensions/equations, downstream sweeps, full/incremental/preview agreement, stable references, invalid sources, save/reopen, suppression, rollback, undo, and STEP/STL.

Native `scripts/phase-2-spatial-smoke.mjs` passed real pointer drawing, dimension editing, sweep creation, undo, and reopening **before** the final pending-state/DOF/tangency changes. Those final changes require a fresh integrated native run.

Limits: workplanes and point-on-plane relations use explicit world coordinates; associative external references/workplanes and broader spatial contour workflows remain open.

### Hollow sweeps

- `Sweep.inner_profiles` accepts explicit closed contours in the same sketch as the outer contour.
- Validation requires contained, disjoint, nonnested inner loops. Inner and outer sweeps use the same path/frame; subtraction preserves history and inner side-face references.
- Existing boss/cut/merge/body-scope behavior is retained.
- Desktop offers profile-hole selection after explicit outer contour selection.

`crates/of-model/tests/hollow_sweeps.rs` passed straight/curved analytic volume checks, inner reference edit/reload, annular cuts, and invalid inputs.

The contour native smoke was updated to expect a hollow volume of `30π` instead of solid `40π`, use “Profile sketch,” and expect persistent ordered-contour references. **This updated hollow scenario has not yet passed a final native run.**

Limits: no new circular-without-sketch, thin/open, twist, guide, closed-path, or loft-hole semantics.

### Circular spherical domes

- Added `Dome { face, height, inward }` using an exact spherical cap on a complete planar circular disk.
- Height must exceed scale tolerance and be no greater than the disk radius.
- Outward domes unite material; inward domes remove material only when fully supported.
- Rejects noncircular, holed, or nonplanar inputs, insufficient supporting material, unwanted outward intersections, and invalid resulting solid counts.
- Integrated body scope, persistent naming, feature patterns, equations/configurations, parameter editing, toolbar/panel, and capability reporting.

Key files: `crates/of-kernel-sys/cpp/dome.cpp`, kernel build/bridge files, `crates/of-geom/src/curves.rs`, `crates/of-model/src/ops.rs`, desktop `panels/dome.tsx`.

Geometry, model, and RPC tests passed earlier revisions across scale/orientation/direction, upstream edits, scope, save/reload, suppression, equations/configurations, patterns, downstream operations, and STEP/STL.

**Last-minute change to verify:** a face shared by two split bodies can have opposite local orientation versus the aggregate shape. `ops.rs` now requires an explicit single owning-body scope for this ambiguous dome case. New test `shared_split_face_requires_explicit_owner_scope_for_direction` is in `crates/of-model/tests/dome.rs`. This landed around the stop request; treat its exact final-tree verification as outstanding.

`scripts/phase-2-dome-smoke.mjs` is integrated into the native suite but has **not yet been run there**. Broader dome profiles/tangency controls remain unimplemented.

### Cosmetic threads

- Added a metadata-only cosmetic thread feature referencing a persistent complete cylindrical face, with diameter, pitch, length, start offset/angle, direction, handedness, and internal/external classification.
- Requires two complete coaxial circular rims, an uninterrupted cylinder, matching diameter/orientation, sufficient available length, and no more than 500 turns.
- Saves/reopens annotations and exposes typed scene metadata with lightweight orange helix rendering and previews.
- Revalidates annotations against final geometry after rebuilding, including moved bodies and changed references.
- Does not modify solids, topology, mass properties, or geometric STEP/STL exports.

Key files: `crates/of-model/src/cosmetic_thread.rs`, model/build/scene contracts, desktop `panels/cosmetic-thread.tsx` and viewport rendering.

Core tests in `crates/of-model/tests/cosmetic_threads.rs` and RPC lifecycle/configuration/export coverage passed. `scripts/phase-2-cosmetic-thread-smoke.mjs` is integrated but **has not yet run natively**.

Limits: no modeled thread solids, standards table, interrupted-cylinder support, or shared nominal thread-size/pitch metadata linking tapped holes. A tapped-hole cylinder is usable only through the generic supported cylindrical-face contract.

### Compatibility and generated interfaces

- `.ofp` writer version is 3; reader supports versions 1–3 and validates envelope version before feature decoding.
- Protocol and scene versions are 3; desktop handshake was updated accordingly.
- Generated capabilities describe the supported bounded feature set.
- TypeScript generation now preserves common properties on flattened unions, including spatial entity IDs/construction flags.
- `apps/desktop/src/shared/tools.gen.ts` and `docs/protocol.md` were regenerated repeatedly. Regenerate these from Rust contracts; do not manually patch generated output.
- Migration coverage includes version-2 equation/body identity roundtripping.

## Verification and exact limits

### Successful checks during the session

- Baseline workspace tests passed before implementation.
- Numerous focused geometry, model, solver, persistence, and RPC tests passed as described above.
- The latest main combined check passed `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and all reported `cargo test --workspace --quiet` groups, then was stopped during `cargo build`.
- Desktop TypeScript checking and production builds passed multiple times, including a build after dome/cosmetic UI integration.
- ESLint passed in the latest lint/format gate.
- Offline assistant benchmark passed 10/10; assistant regression tests passed. These preceded some final unrelated feature edits.
- A native Electron integrated smoke passed original feature workflows, persistent contour selection, general split, scoped equations, spatial sketch workflows, and existing assistant scenarios.

### Still pending / not established

- The **exact final working tree has not passed all checks together**. Edits were still landing around the last Rust run, especially the shared-face dome guard.
- The latest format check failed on `scripts/phase-2-smoke.mjs`. Further documentation/scripts changed afterward; format and recheck when resuming.
- The interrupted combined command never completed its `cargo build` or final `cargo run -q -p openforge -- generate --check` stages.
- The final native suite must be rerun against freshly built Rust and desktop code, including updated hollow sweep, dome, cosmetic thread, and latest spatial changes.
- Native smoke timeout was raised from 180000 to 360000 ms. Smoke helper integration is in `scripts/phase-2-smoke.mjs`; native input forwarding is in `scripts/desktop-smoke.mjs`.
- `/tmp/openforge-phase2-smoke.png` may exist from prior native runs; it is not proof of all final feature scenarios.
- `scripts/phase-2-performance.mjs` was written but **not run**. No final performance report or accepted budgets were produced.
- No hosted CI or three-platform release qualification was run. No phase-completion claim or merge is justified.

The performance script is intended to measure five fresh-process startup/open runs, five forced rebuilds, and five previews for straight/hollow spatial sweeps and domes, with analytic volume checks, available `/usr/bin/time` peak RSS, and OS/CPU/OCCT/binary-hash metadata. Review it against the final executable before running and interpreting results.

## Environment notes

- macOS arm64; local OCCT 7.9.3; debug Rust builds were used for this work.
- `corepack pnpm` is available; bare `pnpm` was absent from the original PATH. Recursive package scripts needed a temporary shim:

  ```sh
  mkdir -p /tmp/openforge-tools
  corepack enable --install-directory /tmp/openforge-tools
  env PATH="/tmp/openforge-tools:$PATH" corepack pnpm typecheck
  env PATH="/tmp/openforge-tools:$PATH" corepack pnpm build
  ```

  Direct `corepack pnpm --filter @openforge/desktop typecheck` / `build` also worked.

- Sandboxed native Electron launch aborted; an approved unsandboxed macOS GUI launch worked. Offline assistant listener tests also required approved escalation after sandbox EPERM. There were no automatic approval rejections.
- Keep build outputs out of commits. `target/` and desktop build output are ignored artifacts.
- Internal units are millimeters/radians; only `of-kernel-sys` should touch C++/unsafe; model references must use persistent topology identities.

## Remaining phase scope

Use the R0–R9 plan as the authority, not this checkpoint as a reduced definition of phase 2:

1. **R0:** qualify algorithms for general boundary surfaces, true face blends, wrap, and exact guide behavior. Existing coarse guided-loft candidates do not meet required tolerance.
2. **R1:** larger imported/patterned topology-reference corpus, generated-curve pattern inheritance, multiple projected descendants, and interior closed-curve seam positions.
3. **R2:** broader formula authoring in creation workflows and optimized dependency invalidation. Conservative full rebuilding currently protects correctness.
4. **R3:** associative external geometry/workplanes, richer spatial contour selection, and broader degeneracy/scale qualification.
5. **R4:** helix pitch/height combinations and variable pitch, intersection-based projection, silhouette/intersection split lines, circular/thin/open/twisted/guided/closed sweeps, and loft holes/guides/centerlines/endpoint continuity/closed lofts.
6. **R5:** ribs, parting-line/step drafts, and broader dome profiles/tangency.
7. **R6:** variable-fillet chain interpolation/propagation and true face-set blends.
8. **R7:** boundary and wrap feature families.
9. **R8:** modeled threads and tapped-hole nominal metadata integration.
10. **R9:** complete corpus, measured runtime/memory budgets (including guides/modeled threads when implemented), final performance artifacts, and hosted platform qualification of the exact candidate.

## Suggested next steps only after authorization to resume

1. Read this handoff, `AGENTS.md`, the remaining plan, and current diffs. Preserve all pre-existing changes.
2. Verify the last shared-face dome guard and the latest spatial solver/UI changes; fix any discovered failures.
3. Format changed JS/TS/Markdown using repository conventions and run the required format/lint/type checks.
4. Run Rust formatting, Clippy, and workspace tests on a stable tree; build, regenerate interfaces if needed, and run generator `--check`.
5. Rebuild the desktop app and run the complete native smoke with hollow sweep, dome, cosmetic thread, and spatial scenarios enabled.
6. Review/run the performance script against the verified binary; record real measurements and limitations.
7. Continue the remaining R0–R9 scope. Update status documents with evidence and retain honest partial-support labels. Do not merge the phase branch into main before phase completion.

## Pre-existing dirty work to preserve

At the start, dirty files already included CI, assistant coordinator/provider and tests, advanced/field/protocol/generated desktop files, kernel build/bridge files, model curve/reference files, session/views/RPC tests, project/phase/assistant docs, and desktop/phase smoke scripts. Pre-existing untracked work included the kernel probe implementation/examples/tests, model contour tests, feasibility/probe-result/remaining-plan documents, and `docs/soliworks-ui.jpeg`.

The current diff combines that earlier work with this session's implementation. Git status alone cannot establish authorship. No cleanup or rollback was performed when stopping.
