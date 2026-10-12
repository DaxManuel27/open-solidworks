# Phase 2 measured performance

Recorded 2026-10-12T03:09:47.275Z. This is a local development-build measurement of the delivered fixtures.

Run `cargo build -p openforge && node scripts/phase-2-performance.mjs` to repeat. `OPENFORGE_CORE` can select another binary. Raw samples and exact metadata are in [phase-2-performance.json](phase-2-performance.json).

- Commit: `aaaa94d26c0fa7eca53a1a6a4bad8e31f1dbf0cd`; working tree modified as recorded in the JSON.
- Binary SHA-256: `857735ce3b8aa981b6fa40199f20248ae27a8cd72a1c57d38335683a20aad01e`; build profile: debug.
- Machine: Apple M3 Max, 14 logical processors; darwin 25.6.0 arm64; 36.0 GiB system memory.
- Kernel header: OpenCASCADE 7.9.3; rustc 1.98.1 (48a229cea 2026-09-01); Node v24.21.0.

Five repetitions per operation, 3 runs (the table shows the last run); the table reports medians in milliseconds. Startup opens a fresh core process each time (a cold rebuild of the whole part); it does not flush operating-system caches. Feature rebuild resubmits the measured feature's definition, which rebuilds it and every later feature while reusing earlier cached states (a warm rebuild). Preview edits the existing measured feature. All timed RPCs include tessellation and response serialization.

| Fixture | Startup + open (ms) | Feature rebuild RPC (ms) | Preview RPC (ms) | Peak core RSS (MiB) |
| --- | ---: | ---: | ---: | ---: |
| straight_spatial_sweep | 74.11 | 3.13 | 3.70 | 37.33 |
| hollow_spatial_sweep | 82.31 | 9.74 | 9.55 | 39.89 |
| spherical_cap_dome | 89.05 | 18.39 | 17.40 | 42.30 |
| modeled_thread | 1489.76 | 1417.17 | 1422.08 | 114.86 |
| housing_rib | 106.25 | 20.31 | 20.12 | 44.81 |
| cylinder_wrap | 86.07 | 14.65 | 13.26 | 49.28 |
| boundary_prism | 150.19 | 80.12 | 79.61 | 60.14 |
| scaled_guide_sweep | 104.11 | 33.42 | 33.38 | 45.72 |
| tangent_loft | 156.73 | 82.95 | 83.27 | 50.69 |
| centerline_loft | 78.88 | 6.71 | 6.81 | 39.28 |
| dome_geodesic_wrap | 274.85 | 191.39 | 191.59 | 54.66 |

The straight sweep has a 4 mm circular section along a fully constrained 80 mm spatial line. The hollow sweep uses outer radius 4 mm and inner radius 2 mm on the same path. The dome adds a 3 mm spherical cap to the planar circular top of a radius 10 mm, depth 8 mm cylinder. The modeled thread is an M10 x 1.5 external thread 15 mm long on a radius 5 mm, 20 mm cylinder. The rib is a 2 mm web normal to its sketch inside a 40 x 20 x 10 housing shelled 2 mm. The wrap embosses a 5 x 4 mm rectangle 1 mm onto a radius 10 mm cylinder. The boundary is a 10 x 10 x 10 prism bounded by two sections and two straight curves. The scaled guide sweep grows a radius 2 circle to radius 4 over 20 mm along a straight guide. The tangent loft joins 10 and 6 mm squares 10 mm apart, leaving both normal to them. The centerline loft joins the same squares along a straight centerline. The dome wrap embosses a radius 2 circle 0.5 mm along geodesics onto a 5 mm spherical dome. Each fixture's final volume was checked against its analytical value (within 0.0001 mm³, or the tolerance recorded in the JSON).

/usr/bin/time -l maximum core resident set size over each complete core process, including RPC scenes, preview states and undo checkpoints; no polling estimate.

Budgets for these fixtures are in [phase-2-budgets.json](phase-2-budgets.json): three times the slowest of three runs' medians (at least 50 ms) and one and a half times the largest peak memory, on this machine and build. `node scripts/phase-2-performance.mjs --check` (`pnpm perf:check`) fails when a median or the peak memory exceeds its budget; `OPENFORGE_BUDGET_SCALE` scales them for slower machines. Native UI, hosted-platform qualification and release-profile costs require their own evidence.
