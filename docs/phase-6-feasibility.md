# Phase 6 kernel feasibility (P6.0)

Findings from the P6.0 prototypes, recorded 2026-10-17. Each item names the test that keeps
it true. See [the plan](phase-6-plan.md#p60--contracts-and-prototypes) and
[the status](phase-6-status.md).

| Prototype | Result | Kept by |
| --- | --- | --- |
| Body kinds and part v7 | Surface bodies in their own list; v6 files load unchanged; surface parts round-trip | `of-model/tests/surfaces.rs` (`surface_parts_round_trip_through_the_file`), `compatibility.rs` |
| Knit a six-face open box | `BRepBuilderAPI_Sewing` then `BRepBuilderAPI_MakeSolid` and `ShapeFix_Solid`; an open set reports its free edges and won't make a solid | `of-geom/tests/surface.rs` (`knit_box_faces_into_a_solid`, `an_open_shell_reports_free_edges_and_wont_solidify`) |
| Delete and patch | `BRepAlgoAPI_Defeaturing` removes a fillet and heals a through hole; the general face-replacement routine covers what it refuses | `of-geom/tests/surface.rs` (`defeature_removes_a_fillet`), `of-model/tests/direct.rs` (`delete_face_patch_heals_a_through_hole`) |
| Move face | Offset, translate and rotate through one routine: extend the neighbouring faces, build cells with `BOPAlgo_MakerVolume`, classify each against the body, fuse or cut | `of-geom/tests/faceedit.rs`, `of-model/tests/direct.rs` |
| Thicken | OCCT 7.9's `PerformByJoin` has no thickening flag (its last flag removes internal edges); `BRepOffset_MakeOffset::Initialize(..., Thickening = true)` works, with bare faces wrapped in a shell | `of-geom/tests/surface.rs` (`offset_and_thicken_a_face`) |
| Silhouettes | Not prototyped with `Contap`: Parting Line uses the analytic cylinder and sphere silhouettes Split Line already had | `of-model/tests/mold.rs` |
| Large meshes | A 1M-triangle STL loads as a graphics body in 5.6 s and a 99,224-facet sphere becomes a solid B-rep in 12.7 s (debug core). That needed shapes to cache their sub-shape maps (per-index queries were quadratic) and closed, consistently oriented meshes to skip `ShapeFix`. The 100,000-facet default limit stands, and base64 JSON is fast enough, so the protocol stays at version 5 | `scripts/phase-6-performance.mjs`, `docs/phase-6-budgets.json`, `of-model/tests/meshes.rs` |
| Display triangulations | Shapes the core has displayed carry triangulations that `BRepBuilderAPI_GTransform` and NURBS conversion copy unchanged onto the result, which then fails `BRepCheck`. Non-uniform scale, flex and deform work on a copy without them | `of-geom/tests/deform.rs` (`a_displayed_shape_scales_and_deforms`) |
