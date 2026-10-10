# Core protocol

The desktop preload, `openforge serve` and MCP tools use the same Rust session.
`serve` accepts one JSON-RPC 2.0 request per line on stdin and writes one response
per line on stdout. Lengths are millimetres, angles radians and density kg/m³.

```json
{"jsonrpc":"2.0","id":1,"method":"core.info","params":{}}
```

Call `core.info` before consuming scenes. The current protocol and scene versions
are both **2**, and the current `.ofp` writer version is **2**. The reader accepts
versions 1 and 2. The desktop rejects mismatched protocol/scene versions with an
instruction to rebuild both components. Capability flags describe implemented
operations, not the entire Phase 2 roadmap.

## State and transactions

`doc.info` returns a snapshot: feature definitions/statuses, units, base `material`
and active `effective_material`,
variables, equations, registered parameters, evaluated values/errors,
configurations, body metadata, path, dirty/undo/redo state, editing feature and scene.
Object results carry a monotonically increasing session `revision`. New/open and
undo/redo also advance the revision; preview, measurement and save do not change
geometry revision. Editing-context changes advance it because picking/preview
geometry changes. Revisions are transient and are not written into part files.

`feature.preview` accepts `{feature, editing?, revision?}`. It builds on the visible
state without storing a feature; null clears preview. It returns
`{scene, error, revision}`. An obsolete requested revision is rejected. The desktop
also tracks request order and panel identity to discard late results after cancel,
commit, document replacement or another preview. Previews are never pick targets.

Feature mutations create undo checkpoints. A geometric failure is stored as a
feature error and preserves the preceding valid model state. Equation failure
blocks its consumer instead of substituting a stale value. Configuration switches
clear edit state and rebuild from the base document with that variant's overrides.

## Method inventory

| Methods | Parameters / behavior |
| --- | --- |
| `core.info` | Protocol, scene, file versions and capabilities |
| `doc.info`, `doc.new` | Read snapshot or replace with an empty document |
| `doc.open`, `doc.save` | `{path}`; save may reuse the open path |
| `doc.undo`, `doc.redo` | Restore authored document state, including variants and formulas |
| `doc.set_units` | `{units: {length, angle, mass, decimals}}`; display units only |
| `doc.set_material` | `{name?, density?}`; change the shared base material |
| `doc.rollback` | `{index: number or null}`; build the feature prefix |
| `feature.add` | `{feature, name?}`; returns the new `id` and snapshot |
| `feature.update` | `{id, feature}`; replaces base feature parameters |
| `feature.delete`, `feature.rename` | `{ids}` or `{id, name}` |
| `feature.suppress`, `feature.reorder` | `{id, suppressed}` or `{id, to}`; shared feature tree |
| `feature.preview` | `{feature, editing?, revision?}` |
| `edit.begin`, `edit.end` | `{id}` or `{}`; show state before an edited feature |
| `sketch.new`, `sketch.get`, `sketch.edit`, `sketch.close` | Plane reference, sketch ID, or `{id, op}` |
| `doc.topology` | Named faces/edges with geometric properties |
| `select.ref`, `select.locate`, `select.plane` | `{kind,index}`, `{ref}`, `{ref}` respectively |
| `body.display` | `{id, name?, hidden?}`; visibility does not change mass or export |
| `equations.set`, `equations.preview` | `{variables, equations}` replaces complete base maps; preview returns `{values, revision}` |
| `configuration.create` | `{name, copy_from?}`; create and activate a new stable ID |
| `configuration.update` | `{configuration}`; replace one variant's overrides, retaining its ID |
| `configuration.activate`, `configuration.delete` | `{id}`; at least one variant must remain |
| `measure`, `mass_properties` | `{refs}` or `{}`; active configuration geometry/material |
| `import`, `export` | `{path}`; export also accepts `format` and optional `configuration` ID |
| `hole.sizes`, `hole.dims`, `materials.list`, `units.parse` | Existing size/material/unit utilities |

Invalid parameters use JSON-RPC error `-32602`, unknown methods `-32601` and
operation failures code `1`. Invalid feature geometry normally appears in the
successful snapshot's `features[].error`, allowing the feature to remain editable.
Exports reject active unsuppressed feature errors to avoid exporting a partial
failed configuration as though it were complete.

## Bodies, references and scenes

Body IDs are opaque strings such as `b2:...`. They derive from creating feature and
topology provenance, never a kernel solid index. Move preserves an ID; copies,
combine results and split children get new IDs. Ambiguous legacy-operation lineage
creates a new identity, so a consumer of the vanished ID fails explicitly.

Scenes contain an aggregate `body` mesh and `bodies[]` entries with `id`, `name`,
`hidden`, and `faces`, `edges`, `vertices` arrays. These arrays map each body's local
topology to aggregate mesh indices. Hidden geometry is excluded from drawing and
picking while remaining available to modeling and export. `curves[]` carries
feature IDs and display polylines for parametric construction curves; `sketches[]`
and `datums[]` carry existing sketch/reference displays. Mesh float32 and uint32
arrays are little-endian base64; face/edge ranges are grouped by aggregate index.

`select.ref` emits body-qualified topology references:

```json
{"type":"body_topology","body":"<body ID>","kind":"face","name":"<face name>"}
```

Use `{type:"body",id}` for a body reference, or `{type:"feature",id}` for an entire
sketch or construction curve in path inputs. Standard planes and sketch-entity
references retain their Phase 1 shapes. Legacy unqualified face/edge/vertex
references remain readable. Qualified references require an exact primary topology
name inside the named body; they do not silently choose a new split-face alias.

Solid feature payloads can include `scope: [body IDs]`. Omitted/null scope keeps
whole-part behavior. An explicit scope must be nonempty and every body must exist.
Combine, split and move/copy instead have their own ordered `bodies` selection.

## New feature payloads

All use `feature.add/update/preview`, so desktop, CLI RPC and MCP share semantics.
Optional flags default to false, except `merge`, which defaults to true.

| `type` | Fields |
| --- | --- |
| `combine` | `bodies`, `operation`: `fuse`, `cut`, `common`; first body is subtraction target |
| `move_body` | `bodies`, `translation`, `rotation`, `origin`, `copy`; XYZ radians rotate X then Y then Z |
| `split_body` | `bodies`, `plane`, `keep`: `both`, `positive`, `negative` |
| `helix` | `plane`, `radius`, `pitch`, `turns`, `start_angle`, `left_handed` |
| `composite_curve` | Ordered `segments: [{reference,reverse}]` |
| `projected_curve` / `split_line` | `source`, `target`, `direction: [x,y,z]`; split line requires a face target |
| `sweep` | `profile` sketch ID, `path` reference, `reverse_path`, `orientation`: `minimum_twist`, `frenet`, `fixed`; `cut`, `merge` |
| `loft` | Ordered `profiles` sketch IDs, `reversed` profile IDs, `ruled`, `cut`, `merge` |
| `draft` | `faces`, `neutral_plane`, optional `direction` reference, signed `angle` |
| `variable_fillet` | `edge`, ordered `stations: [{position,radius}]`, `reverse`; positions include 0 and 1 |

See [Phase 2 status](phase-2-status.md) for each operation's current supported modes.

## Formulas and variants

`variables` maps identifiers to expressions. `equations` maps registered parameter
keys to expressions. Discover keys through `doc.info.parameters`; examples include
`f2.end1.depth`, `f7.direction1.spacing`, and `f1.dimension.17`. Constraint IDs are
persistent IDs, not array positions. Renaming a feature does not change its keys.

Expressions support `+ - * /`, parentheses, unary signs, `min`, `max`, `abs`, `sqrt`,
`sin`, `cos`, `tan`, `pi`, and `mm/cm/m/in/ft/deg/rad`. Examples:
`width / 2 + 1mm`, `sin(45deg) * 10mm`, `f2.end1.depth * 2`, `3/8 * in`.
Bare constants are dimensionless. Units are checked, including function domains,
finite results, integer counts and dependency cycles. Geometry-derived quantities
are not equation inputs. The current implementation conservatively rebuilds the
whole history when a formula/configuration may change an earlier consumer.

Configurations share one authored feature tree. Each variant stores a stable ID,
unique name, parameter values, formula/variable overrides, suppression by feature
ID and material override (`inherit`, `clear`, or `set`). Numeric overrides use core
units and replace a base equation at that parameter. A variant equation then takes
precedence over the numeric override. Null equation overrides disable inheritance
of a base equation. Base formulas resolve against the variant's effective variables.

Feature panels and the Equations editor edit base values; the Configuration
Overrides panel edits the active variant. Shared feature deletion removes that
feature's override targets. Save/load preserves all variants and the active ID.
Export uses the active variant unless an explicit configuration ID is supplied.
The CLI exposes the same choice as `openforge export part.ofp output.step --configuration 2`.


## Built-in assistant transactions

Object responses now include `generation`, an opaque document-lifetime identifier,
as well as `revision`. New/open/core restart replace the generation. An assistant
request captures both; a matching numeric revision alone is insufficient.

| Method | Parameters / result |
| --- | --- |
| `assistant.tools` | Restricted catalog: name, RPC method, description, schema, read-only flag |
| `doc.feature` | `{id}` → full authored feature definition |
| `parameter.set` | `{key,value}` → sets a registered base parameter and removes its base equation; configuration overrides still take precedence |
| `draft.begin` | `{generation,revision}` → `{id,base_revision,snapshot}`; refuses an open manual editor |
| `draft.apply` | `{id,method,params}` → `{result,feature_errors,draft_revision}`; only catalog operations are allowed |
| `draft.inspect`, `draft.preview` | `{id}` → `{snapshot,changed,feature_errors,blocking_errors}`; closes draft editing state |
| `draft.commit` | `{id}` → live snapshot, with one undo checkpoint if changed |
| `draft.discard` | `{id}` → `{discarded:true}` |

Drafts are private sessions with independent authored documents. Failed operations
restore the previous draft document. New/open/save/import/export/global undo are
unavailable in drafts. The live revision and editing context must still match at
commit; new errors and changed failing features block it. Existing untouched errors
are distinguished in `feature_errors` versus `blocking_errors`. At most four drafts
are retained, each for ten minutes. The last 32 commit IDs are idempotent within the
document generation: re-delivery returns the current snapshot with
`already_committed:true`, including after undo, without reapplying the task.
