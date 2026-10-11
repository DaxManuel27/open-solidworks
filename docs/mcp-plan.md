# Model Context Protocol plan: one accurate tool layer for every AI client

Status: planning, 2026-10-10. Supplements [the AI implementation plan](ai-implementation-plan.md)
and changes two of its decisions (see "Decisions this plan changes").

Goal: any AI client, whether the built-in assistant, Claude Desktop, Claude Code or a
local model, drives the user's **live** OpenForge document through one tool catalog.
Calls land on the first try, results say exactly what happened, and a wrong call
fails loudly with a repair hint rather than silently producing the wrong part.

## What the code does today (audit)

| Area | Today | Why it hurts accuracy or seamlessness |
| --- | --- | --- |
| Schemas | `mcp.rs` hand-writes JSON Schema. `feature`, `op`, `plane`, `refs` and `configuration` are bare `{"type":"object"}`; the real shapes live in prose descriptions (the `add_feature` description is thousands of characters). | Models guess field names, units and enum values. Nothing validates the prose, so docs can drift from the Rust types. |
| Results | A feature that fails to rebuild comes back as a *successful* call with `feature_errors` buried in the snapshot. `isError` is true only for RPC failures. | Clients that trust `isError` report success on a broken part. |
| Granularity | Low-level: one `edit_sketch` op per call, and the model must read back entity IDs to place the next op. | A plate with four holes costs 20+ round trips. Every round trip is a chance for an ID or coordinate mistake. |
| Geometry targeting | The model reads the full `topology` dump and picks faces by index or ref. | "The top face" is a spatial inference the model gets wrong. |
| Live state | `openforge mcp` builds its **own** `Session`; the desktop runs a different one. | An external client never sees what the user is looking at. |
| Two catalogs | `assistant.tools` reuses `TOOLS`, but the coordinator adds `finish_task` and a separate system prompt. | Behavior differs per client, and fixes land in one place only. |
| MCP surface | Tools only. No resources, prompts, annotations, structured output, progress or cancellation. Protocol version `2025-06-18`. | Clients can't tell read-only from destructive calls, can't preload context, can't cancel. |
| Verification | Unit tests of the core; the assistant has no benchmark of model task success. | We can't say whether a change to a tool description made the AI better or worse. |

Already in place and worth keeping: draft begin/apply/inspect/commit/discard in the
core, revision and generation guards, stable face/edge refs, parameter keys, and a
coordinator loop with step, token and time limits.

## Design principles

1. **One source of truth.** Rust types define the tools. Schemas, docs, TypeScript
   types and test fixtures are generated from them. Prose never carries the contract.
2. **One catalog, many clients.** The built-in assistant is just an MCP client of the
   same catalog and the same live session. No private tools.
3. **Unambiguous results.** Every mutating call returns the same envelope (below).
   "Succeeded but the feature failed" is reported as a failure of the call.
4. **Query instead of infer.** Geometry the user means ("top face", "the four corner
   edges", "the big hole") is resolved by core queries, never by model arithmetic.
5. **Verify with measurements.** A task is finished only when post-conditions
   (volume, bounding box, hole positions, dimensions) are checked against the
   user's numbers by the core.
6. **Measure the AI.** A repeatable benchmark gates every change to tool names,
   schemas or descriptions.

## Workstreams

### W1. Typed, generated tool schemas

- Derive JSON Schema from the Rust command types (`schemars` or equivalent) in
  `of-protocol` / `of-model`. Features, sketch ops, planes, refs and configurations
  become tagged `oneOf` unions with enums, `minimum`/`maximum`, units in field
  descriptions (`mm`, `rad`) and a short `description` per field.
- Replace prose examples with schema `examples` that are **validated in CI** against
  the schema and replayed against the core, so an example can never go stale.
- Generate `apps/desktop` TypeScript types and `docs/protocol.md` tables from the
  same schemas. A drift check fails CI if committed output is stale.
- Strictness: `additionalProperties: false` on every object, so a misspelled field
  is an error, not ignored. Reject numbers that are not finite.
- Keep each tool's description under ~600 characters: when to use it, what it does
  not do, and the unit convention. Everything else belongs in the schema.

### W2. Result envelope and structured errors

Every tool returns `structuredContent` (with an `outputSchema`) plus a short text
summary for clients that only read text:

```json
{
  "ok": true,
  "revision": 41,
  "draft": "d3",
  "changed": [{"id": 7, "name": "Boss-Extrude1", "kind": "extrude"}],
  "created": {"sketch": 5, "feature": 7},
  "feature_errors": [],
  "warnings": [],
  "measurements": {"volume_mm3": 12000.0},
  "next": ["Inspect face refs with find_faces", "Add holes with add_hole_pattern"]
}
```

- `ok` is false whenever any **new** feature error appears; the MCP `isError` flag
  mirrors it. Pre-existing errors are listed separately under `existing_errors`.
- Errors carry a stable `code` (`E_UNKNOWN_REF`, `E_UNDER_CONSTRAINED`,
  `E_REBUILD_FAILED`, `E_STALE_REVISION`, …), the offending JSON path, the allowed
  values when relevant, and a `repair` hint. Codes are documented and tested.
- Results are size-bounded: summaries by default, `detail: "full"` on request. Meshes
  and scenes never reach the model.
- Create calls return IDs and refs in a form the next call accepts verbatim.

### W3. Intent-level tools and geometry queries

Keep the low-level tools for experts. Add composite tools that the model should reach
for first, each atomic (all-or-nothing) and each emitting ordinary editable history:

| New tool | What it replaces |
| --- | --- |
| `sketch_batch` (ordered ops with local names such as `$rect`, `$c1`) | 10+ calls where each needs a prior ID |
| `create_profile_sketch` (rectangle, circle, slot, polygon with dimensions and a fully constrained result) | rectangle + dimension + constraint calls |
| `add_hole_pattern` (sketch on face, positions, diameter, end condition) | a sketch and a cut by hand |
| `find_faces`, `find_edges` (filters: type, normal, position, size, `largest`, `adjacent_to`) | reading the whole topology dump |
| `describe_selection` | guessing what the user's picked refs are |
| `check_part` (expected volume, bounding box, dimension, body count, tolerances) | the model eyeballing numbers |
| `explain_error` (feature id) | free-form debugging |

`sketch_batch` is the key accuracy win: names are resolved inside the core, so the
model never has to copy an ID it hasn't seen yet.

### W4. Live attach (built last)

- Built after the entire app exists, so external clients see every feature. Until then
  the built-in assistant uses the same catalog inside the app and `openforge mcp`
  stays headless.
- The desktop owns the single document session and exposes it to external clients
  (Claude Desktop, Claude Code) through a loopback-only endpoint with a per-launch
  token; `openforge mcp --attach` is a stdio proxy to it. The transport is an internal
  detail: default loopback HTTP with a launch token, confirmed when M7 starts.
- Manual and AI edits share the existing revision guards; a commit is one undo step.
- External clients are read and edit only. Open, save, export and import stay
  desktop-only (owner decision, 2026-10-10).
- From the start, every new feature ships with its schema, validated examples and
  benchmark tasks, so the catalog grows with the app instead of being retrofitted.

### W5. Full MCP surface

Verify against the current MCP specification before building, since it moves; the
items below are the ones that matter to us:

- **Tool annotations:** `readOnlyHint`, `destructiveHint`, `idempotentHint`,
  `openWorldHint`, plus a human `title`.
- **Resources:** `openforge://part/summary`, `…/features`, `…/feature/{id}`,
  `…/sketch/{id}`, `…/topology`, `…/capabilities`, `…/selection`, with
  `resources/subscribe` and `list_changed` notifications so clients see user edits.
- **Prompts:** reusable workflows such as "model a plate with holes", "repair this
  failed feature", "make a configuration".
- **Progress and cancellation:** `notifications/progress` for rebuilds,
  `notifications/cancelled` honored between operations (kernel calls are synchronous,
  so cancel takes effect at the next operation boundary, not mid-call).
- **Elicitation** (if the client supports it): ask the user which of two faces is
  meant instead of guessing.
- **Versioning:** the catalog reports a content hash and protocol version; clients
  that cached an older catalog are told to refresh.

### W6. Accuracy benchmark ("OpenForge-Bench")

- 40–60 tasks across difficulty tiers: single feature, plate with holes, revolve
  parts, shells and fillets, edit-an-existing-feature, selected-face operations,
  parameter and configuration changes, repair a failure, and ambiguous or
  unsupported requests where the right answer is a question or a refusal.
- Each task has a prompt, optional starting `.ofp`, and **geometry assertions** run by
  the core: valid solid, volume, bounding box, feature-tree shape (editable, fully
  constrained sketches), hole centers, no new errors. No LLM judges.
- Metrics per model and per config: success rate, first-try tool-call validity,
  tool calls per task, repair rate, wasted/incorrect calls, tokens, wall time.
- Runs: a deterministic replay mode in CI (recorded transcripts, no model calls) for
  contract regressions, and a live mode run on demand against the chosen cloud and
  local models. Pin model identifiers and publish the results table.
- Rule: a change to a tool name, schema or description merges only with a benchmark
  run showing no regression.

### W7. Safety and robustness

- All document text (feature names, sketch labels, imported file metadata) is data,
  never instructions; tool results mark it as such. Add injection fixtures to the
  benchmark.
- Per-task budgets (steps, tokens, time) remain. Add per-tool rate limits for external
  clients and a kill switch in the assistant pane.
- Idempotent commits via commit token (already planned) and idempotent retries for
  read tools.
- Every tool call is logged to an operation record: client, tool, arguments, result
  code, duration. Exportable for bug reports; never contains credentials.
- Fuzz the dispatcher with schema-generated valid and invalid inputs; invalid input
  must produce `E_*` errors, never a panic or a partially applied change.

## Decisions this plan changes

1. **Live MCP attach moves to the end** (owner decision, 2026-10-10): it is built once
   the entire app exists. The contract work (W1 to W3) comes first and grows with every phase.
2. **The built-in assistant stops being special.** It consumes the same catalog;
   assistant-only logic (`finish_task`, prompt text) becomes standard MCP prompts and results.

## Delivery sequence

| Milestone | Scope | Gate |
| --- | --- | --- |
| M0: Baseline | Run the existing assistant on 10 benchmark-style tasks by hand; record failures and call counts. Build the transcript-replay harness. | Baseline numbers written down. |
| M1: Schemas | W1: generated schemas, validated examples, TS and doc generation, drift check in CI. | Every example validates and replays; no bare `object` parameters remain. |
| M2: Envelope | W2: result envelope, error codes, `isError` correctness, `outputSchema`. | A failed rebuild is never reported as success. |
| M3: Intent tools | W3: `sketch_batch`, profile and hole-pattern tools, `find_*`, `check_part`. | Plate-with-holes in 6 calls or fewer with fully constrained sketches. |
| M4: Benchmark | W6: full suite, replay in CI, first live results for one cloud and one local model. | Release thresholds set from the baseline. |
| M5: MCP surface | W5: annotations, resources, prompts, progress, cancellation, catalog hash. | Spec-conformance checks and an inspector run pass. |
| M6: Hardening | W7: injection corpus, fuzzing, operation log, budgets. | Fuzz and injection suites green. |
| M7: Live attach | W4, after the entire app is built. | Claude Desktop or Claude Code edits the document open in the app; one Undo reverts it. |

Progress: M0, M1 and M2 done on 2026-10-10; results and the remaining live-model runs are in
[the baseline](mcp-baseline.md).

M1 and M2 come first because they raise accuracy for the built-in assistant with no new
features, and every later step is measured against them.

## Decisions made and open items

- [x] Live MCP attach is built last, once the entire app exists.
- [x] External MCP clients cannot save, export or import files; those stay desktop-only.
- [x] Endpoint transport is an internal detail, not an owner decision. Default: loopback
  HTTP with a launch token, chosen when M7 starts.
- [ ] Benchmark models: default to the adapters already in the app (a Claude model for
  cloud, a model served through Ollama for local). Confirm when M4 starts.
