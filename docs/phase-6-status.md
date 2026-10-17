# Phase 6 implementation status

Updated 2026-10-17. Built on `phase-6`, branched from `phase-5` and rebased onto it as it
moved. Work followed [the plan](phase-6-plan.md) with every decision taken as recommended.
Every step through P6.8's local checks is done and verified; the release push, hosted CI and
the merge into `main` wait for the owner and for Phase 5 (see [Release](#release)).

## Delivered and verified

| Step | Delivered | Evidence |
| --- | --- | --- |
| Bodies (P6.0, P6.1) | Surface bodies beside solids (`State.surfaces`), graphics mesh bodies (`State.meshes`, outside OCCT); every feature says which kinds it takes; mass properties count solids only; references, display and export go through one compound of solids then surfaces, so solid indices are unchanged; open shells in STEP and IGES imports become surface bodies; part file v7 | `of-model/tests/surfaces.rs`, `compatibility.rs` |
| Surface features (P6.1, P6.2) | Extruded (capped ends), revolved, swept, lofted, boundary, planar, offset, ruled (all five directions), radiate, filled (contact, tangent, curvature per edge; constraint curves; merge; make solid), knit (tolerance, make solid), thicken (either side, both, cut, fill volume, merge), cut with surface, trim (tool or mutual, keep or remove pieces, split all), untrim (natural bounds, fill holes), extend (distance, up to point or surface, same or linear), mid-surface, freeform (on surfaces and solids), surface flatten with stretch plot | `of-model/tests/surfaces.rs`, `shaping.rs`, `surface_flatten.rs`, `of-geom/tests/surface.rs` |
| Face editing (P6.3) | Delete face (delete, patch, fill, fill tangent), delete hole, move face (offset, translate, rotate), replace face; one face-replacement routine (extend the neighbours, build closed cells, classify, fuse or cut) | `of-model/tests/direct.rs`, `of-geom/tests/faceedit.rs` |
| Body tools and shape features (P6.4) | Delete/keep body, intersect (regions, exclude, merge, consume, cap planar), move/copy body extras (copies, along an edge, to a point, about an axis, mates), save bodies with an assembly, insert part (transfer options, body filter, mirror, linked by file hash), mirror part, defeature (part faces, holes and fillets below a size; assembly silhouette), body compare, slicing; scale (uniform, per axis, centroid, origin or coordinate system, shrinkage), indent, flex (bend, twist, taper, stretch, trim planes), deform (point, curve to curve, surface push) | `of-model/tests/body_tools.rs`, `of-geom/tests/deform.rs`, `openforge/tests/surfacing.rs` |
| Mould tools (P6.5) | Draft analysis (positive, negative, requires draft, straddle, steep flag, min and max per face), undercut analysis (direction 1 and 2, occluded, straddle, none), parting line (split straddle faces at silhouettes), shut-off surfaces (contact, tangent, no fill per opening), parting surface (perpendicular to pull, tangent, normal, with corner patches), tooling split with interlock, side cores, shrinkage scale from the material, mould folders | `of-model/tests/mold.rs` |
| Mesh and scan data (P6.6, P6.7) | Import STL, OBJ, OFF, PLY, 3MF, AMF as graphics, surface or solid (scale, facet limit, group facets, colours); point clouds (XYZ, TXT, ASC) reconstructed into meshes; mesh B-rep in booleans; planes and axes fitted to facets; convert to mesh; decimate (fraction or tolerance, painted region, preserve boundary); segment; mesh prep (orient, remove pieces, noise, simplify, smooth, fill holes, smooth boundaries); surface from mesh (plane, cylinder, cone, sphere, torus, freeform, auto); mesh to solid wizard; curve wizard; deviation analysis | `of-mesh` unit tests, `of-model/tests/meshes.rs` |
| Core, CLI and MCP | Every feature through `feature.add` and the catalog; `doc.analysis` (draft, undercut, deviation, projected area, compare); `bodies.save`, `part.mirror`, `asm.defeature`; scene meshes and flattenings; snapshot mould folders and `depends_on`; STL export writes mesh bodies beside the B-rep; the part DXF export includes flattened surfaces | `openforge/tests/surfacing.rs` |
| Desktop | Surfaces, Direct Editing, Mold Tools and Mesh tabs; one schema-driven panel for every Phase 6 feature (selection boxes, sketch contours, body pickers including mesh bodies, painted triangles, live preview); analysis overlay panels; Save Bodies, Mirror Part and assembly Defeature; mesh bodies and surface edges in the viewport; facet painting; an Instant3D arrow that drags an extrude's depth or moves a face; Solid, Surface and Mesh Bodies and Mould folders in the tree; parent and child highlights | `scripts/surfacing-smoke.mjs` in the native desktop smoke |
| Drawings | Views of surface and mesh bodies (meshes up to 200,000 triangles, as faceted shells); flattened surface outlines in the part's DXF | `openforge/tests/surfacing.rs` |
| Acceptance (P6.8) | Conformance areas 5, 6, 9 and 12 (22 checks); budgets for knit, filled surface, move face, tooling split, a 1M-triangle import, mesh B-rep conversion, undercut analysis at 100k facets, decimation and a surface fit, in CI; the desktop smoke runs a capped extruded surface, knit to solid, move face, draft analysis, parting line, parting surface, tooling split, mesh import, deviation, segment, surface from mesh and thicken | `scripts/catalog-conformance.mjs`, `scripts/phase-6-performance.mjs`, `docs/phase-6-budgets.json` |

Bugs found by the acceptance work and fixed:

- Non-uniform scale, flex and deform failed on any body the core had already tessellated
  for display: the triangulations were copied onto the stretched shape and failed the
  validity check. They now work on a copy without them.
- Surface Flatten of a whole cylinder side made a sliver; seams are now opened as tears.
- The scene of a 100k-facet body took minutes: every per-index kernel query rebuilt the
  shape's sub-shape map, so walking a shape was quadratic. Shapes now keep their maps.
- Mesh solids went through ShapeFix (41 s for 100k facets); closed, consistently oriented
  meshes now become solids directly (13 s). Exact welding takes one hash lookup per corner
  (a 1M-triangle import went from 24.8 s to 5.6 s).
- Parting surfaces of undrafted parts ran inward over the part, so tooling split failed.
- Feature panels could not pick mesh bodies, and enums with partly documented variants
  (fit kind, relief kind and others) showed as text boxes.

## Decisions taken during implementation

| Decision | Taken |
| --- | --- |
| Body list | Two lists, not one with a kind: `State.bodies` (solids) and `State.surfaces`, plus `State.meshes` for graphics meshes. Existing solid indices and every solid-only feature stay as they were; `everything()` gives the compound of both for references and display |
| Thicken | `BRepOffset_MakeOffset` with thickening on (OCCT 7.9's join mode has no thickening flag); bare faces are wrapped in a shell first |
| Face replacement | Extend the neighbouring faces, build closed cells with `BOPAlgo_MakerVolume`, classify each cell against the body and fuse or cut it. One routine for move, replace and patch |
| Mutual trim | Each body is split by the others and the section edges are the cuts |
| Freeform | Edits a copy of the body; edge parameter curves are rebuilt afterwards |
| Straddle face silhouettes | The existing Split Line silhouettes (cylinders and spheres), not `Contap` |
| Upright faces at zero draft | Go with the core, so an undrafted part parts at its top outline |
| Surface Flatten outline | A polyline of the flat mesh's boundary (lines), not fitted back to B-splines |
| Mesh drawings | Mesh bodies of up to 200,000 triangles appear in views as faceted shells; larger ones are left out |
| Protocol | Stays at version 5: a 1M-triangle mesh goes as base64 in JSON in 5.6 s (debug core), so no binary path is needed |

## Limits of the delivered modes

Recorded behaviour, not open items:

- Draft analysis classifies whole faces; **gradual transition** colouring across a face is
  not offered (each face reports its smallest and largest draft and a steep flag).
- Parting Line's **split faces** uses the analytic silhouettes Split Line already had, so
  it splits cylindrical and spherical straddle faces only (the plan proposed `Contap` for
  every kind); other straddle faces are left whole and go to the side most of them faces.
- Parting surfaces have no **smoothing** option.
- Knit has no **merge entities** option; faces are always merged where they meet.
- Insert Part rebuilds when its file changes (by content hash); **lock** and **break link**
  are not offered.
- Instant3D shows a drag arrow on the picked face; there is no on-screen **ruler**.
- Surface Flatten outlines are not offered as **drawing views**; they go to the part's DXF.
- Mesh B-rep conversion is limited to 100,000 facets by default (`max_facets`); undercut
  analysis is budgeted at that size, not at 200k triangles as the plan suggested, since
  larger meshes stay graphics bodies.
- Deferred by the plan: the automatic whole-mesh NURBS network (guided region-by-region
  fits cover it), the legacy assembly Cavity feature (insert part, scale and combine cover
  it), VDA and IGES point clouds, mesh textures, FeatureXpert and DraftXpert, and mesh
  export to OBJ, PLY and 3MF.

## Verification

Run locally on 2026-10-17 on `phase-6` rebased onto `phase-5` (3ad6bf4), macOS arm64, debug
builds:

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`: 539 tests
  pass.
- `openforge generate`: generated files are current.
- `pnpm lint`, `pnpm format:check`, desktop `tsc` (web and node), `electron-vite build`.
- `pnpm conformance`: 167 of 167 checks pass, 22 of them Phase 6.
- `scripts/phase-6-performance.mjs --check`: within budgets. Slowest of three runs, in ms:

  | Measurement | Observed | Budget |
  | --- | ---: | ---: |
  | Knit of 50 faces into a solid | 24 | 73 |
  | Filled surface | 335 | 1,007 |
  | Move face on a plate with six holes | 36 | 108 |
  | Tooling split | 78 | 236 |
  | Import of a 1M-triangle STL | 5,582 | 16,748 |
  | Mesh to solid B-rep, 99,224 facets | 12,732 | 38,197 |
  | Undercut analysis of that solid | 5,277 | 15,832 |
  | Decimation to a quarter | 6,842 | 20,525 |
  | Sphere fit | 1,328 | 3,985 |

- Native desktop smoke (`pnpm --filter @openforge/desktop smoke`), every scenario including
  Phase 6.

## Release

Left for the owner:

1. Phase 5's release (push, hosted CI, merge), since `phase-6` is built on it.
2. Push `phase-6` and run the hosted macOS, Windows and Linux workflows; record the run here.
3. Rebase onto `main` after Phases 4 and 5 merge, rerun the gates and merge.
