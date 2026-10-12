# Phase 2 kernel experiments and first contour slice

Recorded 2026-10-10 on macOS arm64, OpenCASCADE 7.9.3, debug Cargo profile.
Baseline: `aaaa94d` plus the working-tree changes described here. No hosted CI
result or Phase 2 release completion is claimed.

## Delivered

- An opt-in `kernel-probes` feature in `of-kernel-sys`, separate from production
  modeling commands, with compiled C++ experiments and Rust regression assertions.
- Six bounded experiment families, each with three variations at scales 0.1, 1 and
  10: 54 measured cases. The [recorded CSV](phase-2-probe-results.csv) contains the
  actual measurements, including failures to fit a guide.
- A production `sketch_contour` path reference and desktop path picker. A reference
  stores the sketch ID and the complete set of member entity IDs. Removing an
  unrelated contour does not redirect it. Replacing an entity or splitting/joining
  the selected chain requires reselection and reports an editable feature error.
- `sketch.get` returns contour references with open/closed metadata;
  `core.info` advertises `sketch_contour_paths`. Sweep paths, composite paths and
  projected/split-line sources use the same reference resolver. Generated tool
  schemas and TypeScript/protocol definitions include the new reference.

The contour picker is available after choosing a sketch in a feature's path field.
The whole-sketch option retains its previous one-connected-path behavior. This
slice does not yet add multi-contour sweep/loft profiles, generated construction-curve
segment identities, viewport contour highlighting or general retained-piece splits.
Existing V1/V2 files keep their behavior. The new reference is a tagged enum variant:
older readers reject it rather than silently interpreting it as another path.

## Measurement method

Each positive case constructs a B-rep, checks validity and exactly one solid, measures
volume, and compares analytic volume where available. It then performs a through cut,
checks reduced positive volume and retained/modified face history, and finds a valid
downstream edge fillet among the first 16 edges. This fillet is a consumer check, not
the implementation of the face-set blend experiment.

Fit tolerance is `1e-5 * scale` mm, one part per million of the nominal 10 mm fixture
length. Volume tolerance is `2e-6 * expected_volume`; the planar blend additionally
checks contact-normal alignment below `1e-7` rad. Surface fitting measures sampled
guide points against the skin, not the solid interior, where distance would always
be zero. These finite samples do not establish a global surface-error bound.

Source-face retention counts exact construction-face identity after the operation.
They are not persistent model names, a complete history mapping, or proof of stability
after arbitrary topology changes. A low retention count identifies where generated
and modified faces need explicit provenance when the feature is integrated.

## Results and implementation decisions

All 54 cases produced valid single solids and passed the downstream cut and fillet
checks. All analytic-volume checks passed. 51 cases passed the fit tolerance; the
three coarse guided-loft cases deliberately failed it. Representative scale-1 results:

| Candidate | Measured behavior | Decision and remaining work |
| --- | --- | --- |
| Four-boundary Coons patch, sewn to side/bottom faces | Flat volume 320 mm³; perturbing either curve family gives 360 mm³. Boundary gap below CSV precision; all six construction faces accounted for after sewing | Viable four-sided C0 patch candidate. Interior curve networks, endpoint continuity, general caps and model naming remain; this is not a complete boundary feature |
| Rolling circle between perpendicular planar supports, trimmed and fused | Radii 2/3/4 mm match analytic volumes; contact-normal error below `1e-7` rad; two of six original block faces remain unchanged | Viable limited analytic face-blend case, independent of the edge-fillet API. General face sets, curved supports, orientation, gaps and trim/keep semantics remain unresolved |
| Spherical cap attached to a circular cylinder | Heights 3/4/5 mm match analytic volume and sampled apex/boundary positions; one of three original cylinder faces remains unchanged | Viable circular spherical-cap dome. Tangency to the support is not established; noncircular/nonplanar boundaries and tangency control require further algorithms |
| Rectangular cylindrical emboss using exact angular/axial mapping | Expected volume 2546.874122872 mm³; mapped points fit before, across and around the angular seam | Viable analytic rectangular emboss. Arbitrary sketch contours, conical/freeform targets, deboss, scribe and persistent split-face references remain |
| Auxiliary-spine sweep | Volume about 80 mm³; at 30°/60° guide rotation sampled corner errors about `2.98e-7`/`2.89e-7` mm | Use the auxiliary-spine API as a sweep candidate. This test uses a straight spine and one guide controlling frame orientation, not profile contact or multiple guides |
| Loft through guide-sampled sections | Five sections miss a quadratic guide by about 0.001790918 mm, versus 0.00001 mm tolerance. Nine sections reduce sampled error to about 0.000002556 mm | Reject coarse sampling as exact guided loft. Denser sampling passes this fixture only; require a constrained algorithm or a proven bounded approximation before exposing guide controls |

Elapsed times in the CSV cover construction, measurements and downstream operations
together. See each row's `elapsed_ms` for the recorded run. These are single-run
debug observations, not rebuild latency budgets, cold/warm benchmarks or promises
about arbitrary parts. R9 still needs repeated application-level performance measures.

## Invalid-input findings

The boundary fill API did not itself reject one disconnected-corner experiment.
The harness now validates all four joins before invoking it. This is a required
precondition for future production boundary construction, not a kernel guarantee.

Regression cases also reject an oversized planar rolling circle, zero dome height,
a wrap covering the cylinder more than once, an auxiliary guide crossing the spine,
duplicate guide-section parameters, and nonfinite/out-of-range harness arguments.
Some are explicit domain guards for these bounded experiments. Broader concave,
self-intersecting, freeform and near-degenerate input corpora remain open.

## Reproduction and verification

```sh
cargo test -p of-kernel-sys --features kernel-probes --test phase2_probes
cargo run -p of-kernel-sys --features kernel-probes --example phase2_probe > /tmp/phase2-probes.csv
cargo test -p of-model --test contours
cargo test -p openforge --test serve contour_references_are_discoverable_and_work_over_rpc
```

The CSV emits `fit_passed=false` for the coarse guided-loft counterexample. Its
regression expects that failure, so a passing harness does not mean every candidate
meets feature acceptance. The example exits unsuccessfully if a positive case throws;
fit and downstream acceptance are asserted by the regression test.

Local verification passed: repository Rust tests and build, all-feature Clippy,
generated-file checks, ESLint, desktop TypeScript/build, deterministic benchmark
replay, offline provider fixtures and native Electron smoke. The new desktop smoke
selects one of two sketch chains, creates a sweep, checks its volume, then exercises
undo/redo and save/reopen. No live provider call was needed. CI now includes the
opt-in kernel regression suite; hosted results are still pending.

## Next work

R0 has a reproducible baseline and initial measured candidates; its general geometry
and reference-stability questions remain open. R1 has its first usable contour-path
slice. Continue with ordered/seam-aware profile selection and persistent generated
curve segments, then general body splits and retained-piece identity. Keep the wider
[completion plan](phase-2-remaining-plan.md) and original acceptance gates in force.
