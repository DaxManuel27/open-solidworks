# MCP baseline (M0)

Status: recorded 2026-10-10 on `phase-2`, before the M1 schema work. Part of the
[MCP plan](mcp-plan.md).

## What was run

Ten benchmark-style tasks live in `bench/tasks/*.json`. Each has a prompt, an
optional starting part in `bench/fixtures/` and geometry assertions. The core checks
those assertions (`openforge check <task> <part>`): the outcome (committed change, or
untouched part for a question or refusal), valid solid, body count, volume, bounding
box size, no new feature errors, kept feature IDs and per-configuration geometry. No
model judges the result.

`scripts/bench.mjs` runs the real assistant code: `coordinator.ts` and `provider.ts`
are compiled as-is and driven against `openforge serve`, with the same draft, inspect
and commit path the desktop uses. Only the Electron settings store is replaced.

- `node scripts/bench.mjs replay [label] [task…]` (also `pnpm bench`, run in CI)
  serves recorded tool calls from an Ollama-compatible local endpoint. Deterministic,
  no model calls.
- `node scripts/bench.mjs live --provider anthropic|ollama --model <id>` runs a real
  model and records its transcript for later replay.
- `node scripts/bench.mjs fixtures` rebuilds the starting parts.

**Live-model numbers are not in this baseline.** The M0 session had no API key and no
local model service, so no live run was possible. The numbers below come from
recorded transcripts:

- `reference`: a correct solution for each task using the current catalog. The M0
  author (Claude Opus 5.5 in Claude Code) wrote these while able to read the core
  source, so they are an expert lower bound on calls, not a model success rate.
- `first-attempts`: the author's own first tries at tasks 02 and 06, before the
  results were read closely. They are real failures from this session, kept as
  regression fixtures for M2.

## Results

| Task | Reference calls | Calls by tool | Result | Notes |
| --- | ---: | --- | --- | --- |
| 01 plate with four holes | 27 | 20 `edit_sketch`, 2 `new_sketch`, 2 `close_sketch`, 2 `add_feature`, 1 `mass_properties` | pass | Both sketches fully defined; 18 of the 27 calls are relations and dimensions |
| 02 revolved shaft | 10 | 6 `edit_sketch` lines, then revolve | pass | Profile sketch left under defined |
| 03 shell | 3 | `topology`, `add_feature`, `mass_properties` | pass | Face picked by reading the full topology dump |
| 04 fillet | 3 | same | pass | Four edges picked from start/end coordinates |
| 05 edit a feature | 3 | `part_info`, `set_parameter`, `mass_properties` | pass | Key `f2.end1.depth` found in `parameters` |
| 06 hole in selected face | 5 | sketch on face, circle, cut, check | pass | Needs `reverse: true`; see finding 1 |
| 07 configurations | 6 | `create_configuration` ×2, `update_configuration` ×2 | pass | New configuration IDs are not returned |
| 08 repair a failed fillet | 4 | `part_info`, `get_feature`, `update_feature`, check | pass | Existing error repeated in 2 results |
| 09 ambiguous request | 0 | none | pass | Clarifying reply; part untouched |
| 10 unsupported request | 1 | `core_info` | pass | Refusal; part untouched |

Reference total: 62 tool calls, 10/10 pass. First attempts: 0/2 pass.

| First attempt | Calls | Failed calls | Error messages | Outcome |
| --- | ---: | ---: | --- | --- |
| 02 revolved shaft | 10 | 6 | `entity 5 is not a point`; `no sketch entity 7`, `9`, `11`; feature error `the sketch has no closed contours`; `there's no body` | Commit blocked; the coordinator reported "No changes applied" |
| 06 hole in selected face | 5 | 0 | none | **Committed** "Cut a 10 mm through hole…" with volume unchanged at 24000 mm³ |

## Findings

1. **Silent no-op cut.** A `through_all` cut sketched on a planar face cuts *outward*
   along the face normal unless `reverse: true` is set. It removes nothing and
   reports no feature error, so the call succeeds, `finish_task` commits, and the
   summary claims a hole that does not exist (first attempt 06). Only a volume check
   catches it. *Since fixed:* a cut that removes no material now fails with the
   feature error "the cut does not remove material" (reported as `E_NO_EFFECT`).
2. **Misspelled fields are ignored.** `add_feature` with `"radios": 9` next to
   `"radius": 3` succeeds (`isError: false`). Missing fields are reported, but
   without a JSON path: `missing field \`depth\``.
3. **IDs must be guessed or read back.** `add_line` returns `created: [line, a, b]`,
   `rectangle` returns four corners then the bottom, right, top and left lines, and
   `add_circle` does not return its center point. None of this is documented. In
   first attempt 02 the author used line IDs where point IDs were needed, which
   cascaded into four failed sketch edits and an empty profile.
4. **Pre-existing errors look like new ones.** Every result repeats every feature
   error in `feature_errors`. In task 08 the two read-only calls report the broken
   fillet as though they had caused it. The harness separates new from existing
   errors; the tool results do not.
5. **Sketch frames on faces cannot be discovered.** Nothing in the catalog says
   where a face sketch's origin and axes are. On the block's +Z face, local `(0, 0)`
   is the model origin projected onto the face, so a circle "at the origin" cut a
   quarter hole in the corner.
6. **Configuration IDs are implicit.** `create_configuration` returns a full
   snapshot rather than the new ID, so `update_configuration` relies on reading
   `configurations.items` or on guessing the next ID.

### Catalog contract audit

| Measure | Value |
| --- | --- |
| Tools in `tools/list` | 27 (10,894 bytes of JSON) |
| Parameters typed only as `{"type":"object"}` | 6: `update_configuration.configuration`, `new_sketch.plane`, `edit_sketch.op`, `add_feature.feature`, `update_feature.feature`, `measure.refs[]` |
| Schemas with `additionalProperties: false` | 0 of 27 |
| Descriptions over 600 characters | `add_feature` (2,849), `edit_sketch` (649) |
| JSON examples in descriptions | 35 |
| Examples that are not valid JSON (placeholders such as `[edge refs]`, `<face ref>`, `{...}`, `N`) | 8 |
| Sketch-op examples that fail verbatim after a `rectangle` | 4 of 10: `add_line` (`entity 5 is not a point`), `constraint` and `dimension` (`entity 3 is not a line`), `set_value` (`this relation has no value`) |
| Feature examples that refer to IDs that cannot exist in context | 9 (`<id1>`, sketch 5, feature 7, …): they parse but cannot be replayed |

Result sizes on the 40 × 30 × 20 mm block, with the scene removed: `part_info`
3,022 characters, `topology` 5,297 characters.

## What M1 to M3 should move

- M1 (schemas): bare objects to 0, strict schemas 27 of 27, and every example valid
  and replayed in CI. Finding 2 becomes an error with a path.
- M2 (envelope): finding 1 needs a geometric "no effect" check; finding 4 needs
  `existing_errors`; finding 6 needs `created` IDs.
- M3 (intent tools): task 01 from 27 calls to 6 or fewer; finding 3 goes away with
  `sketch_batch` local names, and finding 5 with `find_faces` and sketch frames.

Re-run these tasks, and add live runs once a key or local model is available, before
changing tool names, schemas or descriptions.

## After M1 (schemas)

Same tasks and transcripts, re-run after the M1 commits on 2026-10-10.

| Measure | M0 | After M1 |
| --- | --- | --- |
| Reference replay | 10/10, 62 calls | 10/10, 62 calls (unchanged: M1 changes no tool behavior) |
| First attempts | 0/2 | 0/2 (findings 1 and 3 are M2/M3 work) |
| Bare `object` parameters | 6 | 0 |
| Schemas closed with `additionalProperties: false` | 0 of 27 | 27 of 27, every nested object included |
| Longest description | 2,849 characters | 238 characters |
| Examples | 35 in prose, 8 not JSON | 54 in schema `examples`, all validated and replayed in CI |
| Misspelled field (`radios`) | ignored, `isError: false` | `invalid add_feature arguments: /feature: Additional properties are not allowed ('radios' was unexpected)` |
| Unknown feature type | `unknown variant` from serde | `/feature/type: "sheet_metal" is not one of: split_line, draft, …` |
| `tools/list` size | 10,894 bytes | 99,749 bytes |

The size increase is the cost of a real contract: `add_feature` and `update_feature`
each carry the full 24-variant feature union (about 30 KB each) and `edit_sketch`
the sketch-op and relation unions (about 16 KB). The built-in assistant sends 21
tools, about 92 KB, with every model request. That is affordable for cloud models,
but it may crowd small local context windows. Measure it in the first live
benchmark (M4). The M3 intent tools, or sending only the feature types a task
needs, are the levers if it hurts.

## After M2 (result envelope)

Every tool now returns the envelope described in
[the protocol](protocol.md#tool-results-and-error-codes). Re-run on 2026-10-10:

| Measure | M0 | After M2 |
| --- | --- | --- |
| Reference replay | 10/10, 62 calls | 10/10, 62 calls |
| First attempt 06 (face cut without `reverse`) | **committed** a hole that does not exist | `add_feature` fails with `E_NO_EFFECT` ("Cut-Extrude1 added or removed no material; a cut on a face sketch usually needs reverse: true"); the draft cannot be committed |
| First attempt 02 (guessed IDs) | plain messages | `E_UNKNOWN_ENTITY` ×4, `E_REBUILD_FAILED` ("Revolve1 failed to rebuild: the sketch has no closed contours"), each with a repair hint |
| Failed rebuild through MCP | `isError: false` | `isError: true`, `ok: false`, `E_REBUILD_FAILED` |
| Task 08 read calls | broken fillet reported in every result | under `existing_errors`; `feature_errors` empty |
| New configuration ID | not returned | `created.configuration` |
| New sketch entities | `created` only on the sketch result | `created.entities` in every `edit_sketch` envelope |

Finding 1 is fixed at the contract level: a model gets `ok: false` and a repair
hint, and the commit gate refuses the draft. Findings 2, 4 and 6 are fixed. Findings 3
(guessing IDs) and 5 (face sketch frames) remain for M3. A replay cannot react to
these results, so the first attempts still fail their checks, but they no longer
end in a false success.
