# Core protocol

The desktop preload, `openforge serve` and MCP tools use the same Rust session.
`serve` accepts one JSON-RPC 2.0 request per line on stdin and writes one response
per line on stdout. Lengths are millimetres, angles radians and density kg/m³.

```json
{"jsonrpc":"2.0","id":1,"method":"core.info","params":{}}
```

Call `core.info` before consuming scenes. The current protocol version is **5**
and the scene version **4**; `core.info` also reports the `.ofp`, `.ofa` and `.ofd`
writer versions (`part_file_version`, `assembly_file_version`,
`drawing_file_version`). The desktop rejects mismatched protocol/scene versions
with an instruction to rebuild both components. Capability flags describe
implemented operations, not the roadmap; `drawings` marks the drawing methods
below.

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
operation failures code `1`; errors may carry `data: {code, path?, allowed?}` with a
stable code from [Tool results and error codes](#tool-results-and-error-codes).
On these desktop methods, invalid feature geometry appears in the successful
snapshot's `features[].error`, allowing the feature to remain editable. Catalog tools
(MCP and `draft.apply`) report it as `ok: false` instead.
Exports reject active unsuppressed feature errors to avoid exporting a partial
failed configuration as though it were complete.

## Drawings

`drawing.new` (`{standard?, size?, projection?, model?, configuration?}`; with `model`, a
standard 3 view and isometric of that file) or `doc.open` of an `.ofd` file
replaces the open document with a drawing, as `asm.new` does for assemblies. While
a drawing is open it answers `drawing.*`, `doc.info`, `doc.save`, `doc.undo`,
`doc.redo` and `export`; part and assembly methods fail with `E_NOT_ALLOWED`.
Snapshots are `{kind: "drawing", drawing, path, dirty, can_undo, can_redo,
revision, generation}`, where `drawing` is the `.ofd` document. Every change is
one undo step.

| Methods | Parameters / behavior |
| --- | --- |
| `drawing.info` | Snapshot |
| `drawing.render` | `{sheet?, summary?}`; `{sheet, graphics, views, frame, title_block}`, or `{sheet, views, items}` with `summary` |
| `drawing.sheet.add` | `{name?, size?, portrait?, scale?, projection?, template?}`; omitted fields copy the last sheet; returns `id` |
| `drawing.sheet.update` | `{id, name?, size?, portrait?, scale?, projection?, template?, format_visible?, properties_view?, index?}` |
| `drawing.sheet.delete` | `{ids}`; one sheet always remains |
| `drawing.view.add` | `{sheet, view, origin, name?, scale?, display?, tangent?}`; returns `id` |
| `drawing.view.update` | `{id, name?, view?, origin?, scale?, display?, tangent?, label?, aligned?, hidden_edges?, hidden_components?, breaks?, break_style?, break_gap?, crop?, broken_out?, hatch?}`; null resets optional values |
| `drawing.views.standard` | `{sheet, model, configuration?, isometric?, scale?}`; laid out per the sheet's projection; without `scale` the largest preferred scale that fits becomes the sheet scale; returns `views {front, top, side, isometric}` and `scale` |
| `drawing.model.faces` | `{model, configuration?}`; planar faces `{entity, normal, area}` for relative-to-model views |
| `drawing.update` | Records the current model hashes, clearing views' `stale` marks |
| `drawing.view.delete` | `{ids}`; also deletes views derived from them; returns `deleted` |
| `drawing.annotation.add`, `drawing.annotation.delete` | `{sheet, annotation, view?, layer?}` (returns `id`) or `{ids}`; with `view` the annotation's positions are relative to that view and it moves with it (stored as `attach`) |
| `drawing.annotation.update` | `{id, annotation?, view?, layer?}`; changing `view` keeps where it is drawn |
| `drawing.annotation.move` | `{ids, by}`; sheet positions move, model anchors stay |
| `drawing.model_items` | `{sheet, views?, items? {dimensions, hole_callouts, threads, axes}}`; returns `added` |
| `drawing.dimension.drive` | `{id, value}`; edits, rebuilds and saves the part of an imported dimension; returns `saved_models` |
| `drawing.balloons.auto` | `{sheet, view, layout?, ignore_multiple?}`; returns `added` |
| `drawing.table.csv` | `{id, path?}`; returns `csv` or writes `path` |
| `drawing.standard.set` | `{standard?, load?, save?}`; `.ofstd` files |
| `drawing.export` | `{path, sheets?, monochrome?, per_sheet?, dxf_version?}`; returns `paths` |
| `drawing.properties.set` | `{properties}`; `$PRPSHEET` values, null removes |
| `export` | `{path}`; every sheet to `.pdf` (one page per sheet, true size) or `.dxf` (2018: views at true size in model space, one paper space layout per sheet) |

Sheet coordinates are mm of paper from the bottom-left corner, y up. A view's
`origin` is the sheet position of its model origin; projected and auxiliary views
stay aligned with their parent unless `aligned` is false. Model paths are stored
relative to a saved drawing and absolute while it is unsaved; the files are
reloaded whenever they change on disk, so renders always show saved geometry.

`render` draws a display list: `graphics.items[]` are `{prim, pen, layer, owner}`
with `prim` one of `line {a, b}`, `arc {center, radius, start, end}` (CCW,
radians), `polyline {points, closed}`, `fill {loops}` and `text {at, height (em
size), angle, text, width}`. `owner` is `format`, `annotation {id}` or `view
{view, entity?}`, where `entity` is the model edge (`CompRef`) the curve shows;
silhouettes of curved faces have none. `views[]` give each view's sheet `bbox`,
`scale`, paper axes `x`/`y`, view direction `toward` and `error` when it could not
be drawn, `origin` (the sheet position of its model origin) and `stale` when its
model changed on disk since the last save or `drawing.update`. A failed view does not
fail the render; `export` refuses to write a drawing with failed views. Annotations
whose model anchor no longer resolves are drawn where possible and listed in
`dangling`.

Annotation kinds (`type`): `note`, `dimension` (with `dim`: `linear`, `angular`,
`radius`, `diameter`, `arc_length`, `chamfer`, `ordinate`, `baseline`, `chain`,
`angular_running` or `model` for an imported part parameter), `balloon`,
`hole_callout`, `center_mark`, `centerline`, `surface_finish`, `datum_feature`,
`datum_target`, `gtol`, `weld`, `area_hatch`, `revision_cloud`, `revision_symbol` and
`table` (with `table`: `bom`, `hole`, `revision`, `general` or `general_tolerance`).
Geometry is named through `{view, entity}` picks; a pick's `near` (view-plane mm)
chooses the end, middle or center of an edge nearest the click.

`openforge info drawing.ofd` lists sheets and views with their errors;
`openforge export drawing.ofd out.pdf|out.dxf` writes it.

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

## Feature payloads

All use `feature.add/update/preview`, so desktop, CLI RPC and MCP share semantics.
Optional fields are marked `?`; flags default to false, except `merge` and
`equal_spacing`, which default to true. The table is generated from the same Rust
types as the tool schemas; field units and bounds are in those schemas.

<!-- generated:features -->
<!-- Generated from the core tool catalog (crates/openforge/src/tools). Do not edit:
run `cargo run -p openforge -- generate` instead. -->

| `type` | Fields | Notes |
| --- | --- | --- |
| `dome` | `face`, `height`, `elliptical?`, `inward?`, `scope?` | Replace a planar circular disk face with an exact spherical cap. |
| `rib` | `sketch`, `thickness`, `direction?`, `draft?`, `flip?`, `scope?`, `side?` | A thin web from an open sketch, extended until it meets material. |
| `wrap` | `sketch`, `face`, `depth?`, `mode?`, `scope?` | Wrap closed sketch regions onto a cylindrical face: emboss, deboss or scribe. |
| `cosmetic_thread` | `face`, `diameter`, `pitch`, `length`, `internal?`, `left_handed?`, `reverse?`, `scope?`, `start_angle?`, `start_offset?` | Persistent thread annotation on a cylindrical face; does not change geometry. |
| `thread` | `face`, `diameter`, `pitch`, `length`, `internal?`, `left_handed?`, `reverse?`, `scope?`, `start_angle?`, `start_offset?` | A modeled ISO-profile thread cut into a cylindrical face. |
| `sketch3d` | `next_id`, `constraints?`, `entities?`, `scope?` | A constrained spatial sketch, with independent XYZ points and curves. |
| `split_line` | `target`, `direction`, `mode?`, `scope?`, `source?` | Split a face along a curve projected onto it. |
| `draft` | `neutral_plane`, `angle`, `direction?`, `faces?`, `other_side?`, `parting_line?`, `scope?`, `step?` | Taper faces by `angle` about a neutral plane. |
| `variable_fillet` | `edge`, `stations`, `reverse?`, `scope?`, `straight?` | Fillet one edge with radii varying between stations. |
| `face_fillet` | `faces1`, `faces2`, `radius`, `scope?` | Blend two sets of planar faces with a rolling-ball radius. |
| `helix` | `plane`, `radius`, `pitch`, `turns`, `defined_by?`, `height?`, `left_handed?`, `scope?`, `start_angle?`, `variable_pitch?` | A helical construction curve, usable as a sweep path. |
| `composite_curve` | `segments`, `scope?` | Join connected curves into one path. |
| `projected_curve` | `target`, `direction`, `mode?`, `scope?`, `source?` | Project a sketch or curve onto a face along `direction`. |
| `sweep` | `path`, `circle_diameter?`, `cut?`, `guide?`, `guide2?`, `guide_scale?`, `inner_profiles?`, `merge?`, `orientation?`, `profile?`, `reverse_path?`, `scope?`, `thin?`, `twist?` | Sweep a closed profile along a path; `cut` removes material. |
| `loft` | `profiles`, `centerline?`, `closed?`, `cut?`, `end?`, `guides?`, `merge?`, `reversed?`, `ruled?`, `scope?`, `start?` | Loft through two or more closed profiles in order. |
| `boundary` | `sections`, `curves`, `cut?`, `merge?`, `scope?` | A solid bounded by sections in one direction and crossing curves in the other. |
| `combine` | `bodies`, `operation`, `scope?` | Boolean of bodies: fuse, cut (from the first) or common. |
| `move_body` | `bodies`, `copy?`, `origin?`, `rotation?`, `scope?`, `translation?` | Move, rotate or copy bodies. |
| `split_body` | `bodies`, `keep?`, `plane?`, `retained_pieces?`, `scope?`, `tools?` | Split bodies with a plane. |
| `plane` | `mode`, `reference`, `distance`, `flip?`, `scope?` | Parallel to a plane or planar face, `distance` along its normal. |
| `plane` | `mode`, `reference`, `axis`, `angle`, `scope?` | Through a line, at `angle` to a plane. |
| `plane` | `mode`, `points`, `scope?` | Through three points. |
| `plane` | `mode`, `a`, `b`, `scope?` | Halfway between two planes or faces. |
| `plane` | `mode`, `reference`, `point`, `scope?` | Parallel to `reference`, through `point`. |
| `plane` | `mode`, `edge`, `at_end?`, `scope?` | Normal to an edge at its start (or end). |
| `axis` | `mode`, `a`, `b`, `scope?` | Through two points. |
| `axis` | `mode`, `a`, `b`, `scope?` | Where two planes meet. |
| `axis` | `mode`, `face`, `scope?` | The axis of a cylindrical or conical face, or a circular edge. |
| `axis` | `mode`, `edge`, `scope?` | Along a straight edge or sketch line. |
| `axis` | `mode`, `point`, `face`, `scope?` | Through a point, normal to a plane. |
| `point` | `mode`, `x`, `y`, `z`, `scope?` | Model coordinates, mm. |
| `point` | `mode`, `target`, `scope?` | On a vertex or sketch point; at the center of a face or circular edge. |
| `coord_sys` | `origin`, `scope?`, `x_axis?`, `z_axis?` | A coordinate system at a point. |
| `extrude` | `sketch`, `end1`, `cut?`, `end2?`, `flip_side_to_cut?`, `merge?`, `reverse?`, `scope?`, `start_offset?`, `thin?` | Extrude a sketch's closed profiles: a boss, or a cut with `cut: true`. |
| `revolve` | `sketch`, `axis`, `angle`, `cut?`, `end2?`, `merge?`, `reverse?`, `scope?`, `thin?` | Revolve a sketch's closed profiles about an axis. |
| `fillet` | `radius`, `edges?`, `faces?`, `scope?` | Round edges, or every edge of the given faces, with a constant radius. |
| `chamfer` | `mode`, `edges?`, `faces?`, `flip?`, `scope?` | Bevel edges, or every edge of the given faces. |
| `shell` | `thickness`, `faces?`, `outward?`, `scope?` | Hollow the part to `thickness`, removing the given faces. |
| `hole` | `sketch`, `kind`, `standard`, `size`, `end`, `cbore_depth_override?`, `cbore_diameter_override?`, `cosmetic_thread?`, `csink_angle_override?`, `csink_diameter_override?`, `diameter_override?`, `fit?`, `reverse?`, `scope?` | Standard holes at each standalone point of a sketch. |
| `linear_pattern` | `features`, `direction1`, `direction2?`, `scope?` | Repeat features along one or two directions. |
| `circular_pattern` | `features`, `axis`, `count`, `angle`, `equal_spacing?`, `reverse?`, `scope?` | Repeat features around an axis. |
| `mirror` | `plane`, `features?`, `keep_original?`, `scope?` | Mirror features, or the whole body, about a plane. |
| `base_flange` | `sketch`, `sheet`, `end1?`, `end2?`, `reverse?`, `scope?`, `side?` | The first sheet metal feature, or a tab on a sheet metal face. |
| `edge_flange` | `edges`, `length`, `angle?`, `angle_from?`, `bend?`, `end_offset?`, `gap?`, `measure?`, `position?`, `position_offset?`, `profiles?`, `reverse?`, `scope?`, `start_offset?` | Walls bent up from edges of a sheet metal body. |
| `flat_pattern` | `break_corners?`, `corner_treatment?`, `exclude?`, `fixed_face?`, `scope?`, `simplify_bends?`, `split_bend_regions?` | Every sheet metal body flattened; suppress it to show the part formed. |
| `hem` | `edges`, `kind`, `allowance?`, `gap?`, `length?`, `position?`, `reverse?`, `scope?` | Fold sheet edges back on themselves. |
| `miter_flange` | `sketch`, `edges`, `bend?`, `end_offset?`, `gap?`, `position?`, `scope?`, `start_offset?` | Flanges from one profile along a chain of edges, mitered at corners. |
| `sketched_bend` | `sketch`, `angle?`, `bend?`, `fixed?`, `position?`, `reverse?`, `scope?` | Bend the sheet along a sketched line. |
| `jog` | `sketch`, `offset`, `angle?`, `bend?`, `dimension?`, `fix_projected_length?`, `fixed?`, `position?`, `reverse?`, `scope?` | A Z-shaped offset of the sheet along a sketched line. |
| `cross_break` | `face`, `angle?`, `radius?`, `reverse?`, `scope?` | Cosmetic stiffening crosses on a flat sheet face. |
| `swept_flange` | `profile`, `path`, `bend?`, `scope?`, `sheet?`, `side?` | An open sheet metal profile swept along a path. |
| `lofted_bend` | `profile1`, `profile2`, `sheet`, `deviation?`, `facets?`, `method?`, `reverse?`, `scope?` | A faceted sheet metal transition between two open profiles. |
| `rip` | `edges`, `gap?`, `scope?` | Cut gaps along corner edges so walls can unfold. |
| `insert_bends` | `fixed_face`, `sheet`, `rip_gap?`, `rips?`, `scope?` | Turn a thin-walled solid's sharp corners into bends. |
| `convert_to_sheet_metal` | `fixed_face`, `sheet`, `bend_edges`, `keep_body?`, `rip_gap?`, `scope?` | Make sheet metal walls from a solid's faces. |
| `unfold` | `fixed_face?`, `scope?` | Flatten a sheet metal body until the next Fold. |
| `fold` | `scope?` | Form an unfolded body again, keeping cuts made while flat. |
| `normal_cut` | `sketch`, `scope?` | A cut through sheet metal square to the sheet. |
| `corner_relief` | `bends?`, `centered?`, `kind?`, `ratio?`, `scope?`, `slit?` | Relief cuts where two bends meet at a corner. |
| `closed_corner` | `faces`, `coplanar?`, `gap?`, `kind?`, `overlap_ratio?`, `scope?` | Extend flanges so they close a corner. |
| `welded_corner` | `edge`, `fillet?`, `scope?`, `size?`, `stop?`, `symbol?` | A weld bead along a closed corner's seam. |
| `break_corner` | `corner`, `edges?`, `internal_only?`, `scope?` | Chamfer or round corners of the sheet's outline. |
| `bend_notch` | `bends?`, `both_ends?`, `depth?`, `scope?`, `shape?`, `width?` | Notches at the ends of bend lines. |
| `tab_and_slot` | `edge`, `face`, `length`, `clearance?`, `corners?`, `count?`, `end?`, `height?`, `length_clearance?`, `scope?`, `spacing?`, `start?`, `through_all?`, `width_clearance?` | Tabs on one sheet metal body through slots in another. |
| `sheet_metal_gusset` | `bend`, `depth`, `width`, `angle?`, `depth2?`, `position?`, `punch_id?`, `rounded_back?`, `scope?` | A stiffening rib pressed across a bend. |
| `form_tool` | `shape`, `face`, `flip?`, `punch_id?`, `rotation?`, `scope?`, `sketch?` | Forming tools pressed into a sheet metal face. |
| `forming_tool` | `stopping_face`, `insertion_point?`, `remove_faces?`, `scope?` | Marks this part as a forming tool for other parts to use. |
| `weldment` | `scope?` | Marks the part as a weldment (one per part, before its members). |
| `structural_member` | `profile`, `groups`, `merge_miter_trimmed?`, `mirror?`, `offset?`, `pierce?`, `rotation?`, `scope?` | A profile swept along sketch segments, one body per segment. |
| `trim_extend` | `bodies`, `boundary`, `allow_extension?`, `gap?`, `kind?`, `scope?` | Members cut back or lengthened to faces, planes or other bodies. |
| `end_cap` | `faces`, `thickness`, `corners?`, `direction?`, `offset?`, `scope?` | Plates closing members' ends. |
| `weld_gusset` | `faces`, `shape`, `thickness`, `chamfer?`, `location?`, `offset?`, `scope?`, `side?` | A plate in the corner between two members. |

<!-- /generated:features -->

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
| `draft.apply` | `{id,method,params}` → the tool's result envelope plus `draft_revision`; only catalog operations are allowed. A failed call leaves the draft unchanged; a failed feature stays, editable |
| `draft.inspect`, `draft.preview` | `{id}` → `{snapshot,changed,feature_errors,blocking_errors}`; closes draft editing state |
| `draft.commit` | `{id}` → live snapshot, with one undo checkpoint if changed |
| `draft.discard` | `{id}` → `{discarded:true}` |

Drafts are private sessions with independent authored documents. Failed operations
restore the previous draft document. New/open/save/import/export/global undo are
unavailable in drafts. The live revision and editing context must still match at
commit; new errors and changed failing features block it. Existing untouched errors
are distinguished in `feature_errors` versus `blocking_errors`, which also lists any
added or changed material feature that adds or removes no material. At most four drafts
are retained, each for ten minutes. The last 32 commit IDs are idempotent within the
document generation: re-delivery returns the current snapshot with
`already_committed:true`, including after undo, without reapplying the task.

## Tool catalog

MCP `tools/list` and `assistant.tools` publish the same catalog. Each tool's input
schema is generated from the Rust argument types the core deserializes (`schemars`),
with units and bounds in field descriptions. Every object is closed
(`additionalProperties: false`) and tagged unions use `oneOf`. Arguments are
validated before a tool runs: a misspelled, missing or out-of-range field fails with
JSON-RPC `-32602` and the JSON path, for example
`invalid add_feature arguments: /feature: Additional properties are not allowed
('radios' was unexpected)`. Each schema carries `examples`, which CI validates and
replays on the core. Desktop JSON-RPC calls are not schema-checked.
`apps/desktop/src/shared/tools.gen.ts` holds the matching TypeScript types.

<!-- generated:tools -->
<!-- Generated from the core tool catalog (crates/openforge/src/tools). Do not edit:
run `cargo run -p openforge -- generate` instead. -->

| Tool | Method | Access | Arguments |
| --- | --- | --- | --- |
| `core_info` | `core.info` | read | `detail?` |
| `part_info` | `doc.info` | read | `detail?` |
| `get_feature` | `doc.feature` | read | `id`, `detail?` |
| `topology` | `doc.topology` | read | `detail?` |
| `get_sketch` | `sketch.get` | read | `id`, `detail?` |
| `solve_spatial_sketch` | `sketch3d.solve` | read | `sketch`, `detail?`, `drag?` |
| `measure` | `measure` | read | `refs`, `detail?` |
| `mass_properties` | `mass_properties` | read | `detail?` |
| `new_sketch` | `sketch.new` | edit (draft) | `plane`, `detail?` |
| `edit_sketch` | `sketch.edit` | edit (draft) | `id`, `op`, `detail?` |
| `close_sketch` | `sketch.close` | edit (draft) | `detail?` |
| `add_feature` | `feature.add` | edit (draft) | `feature`, `detail?`, `formulas?`, `name?` |
| `update_feature` | `feature.update` | edit (draft) | `id`, `feature`, `detail?`, `formulas?` |
| `delete_features` | `feature.delete` | edit (draft) | `ids`, `detail?` |
| `set_parameter` | `parameter.set` | edit (draft) | `key`, `value`, `detail?` |
| `set_equations` | `equations.set` | edit (draft) | `variables`, `equations`, `configuration?`, `detail?`, `parameters?`, `revision?` |
| `locate_reference` | `ref.locate` | read | `reference`, `detail?` |
| `evaluate_formula` | `equations.evaluate` | read | `expression`, `detail?` |
| `create_configuration` | `configuration.create` | edit (draft) | `name`, `copy_from?`, `detail?` |
| `update_configuration` | `configuration.update` | edit (draft) | `configuration`, `detail?`, `revision?` |
| `activate_configuration` | `configuration.activate` | edit (draft) | `id`, `detail?` |
| `delete_configuration` | `configuration.delete` | edit (draft) | `id`, `detail?` |
| `body_display` | `body.display` | edit (draft) | `id`, `detail?`, `hidden?`, `name?` |
| `set_material` | `doc.set_material` | edit (draft) | `density?`, `detail?`, `name?` |
| `new_part` | `doc.new` | file/session (not in drafts) | `detail?` |
| `open_part` | `doc.open` | file/session (not in drafts) | `path`, `detail?` |
| `save_part` | `doc.save` | file/session (not in drafts) | `detail?`, `path?` |
| `export` | `export` | file/session (not in drafts) | `path`, `bend_notes?`, `bodies?`, `configuration?`, `detail?`, `dxf_version?`, `per_body?` |
| `weldment_profiles` | `weldment.profiles` | read | `detail?`, `family?`, `size?` |
| `sheet_metal_gauges` | `sheet_metal.gauges` | read | `detail?`, `table?` |
| `sheet_metal_info` | `doc.sheet_metal` | read | `detail?`, `outline?` |
| `set_part_properties` | `doc.properties.set` | edit (draft) | `properties`, `additionalProperties?`, `detail?` |
| `set_dimension_display` | `doc.dimension_display.set` | edit (draft) | `key`, `detail?`, `name?`, `show?` |
| `import` | `import` | file/session (not in drafts) | `path`, `detail?` |
| `undo` | `doc.undo` | file/session (not in drafts) | `detail?` |
| `new_assembly` | `asm.new` | file/session (not in drafts) | `detail?` |
| `assembly_info` | `asm.info` | read | `detail?` |
| `component_topology` | `asm.topology` | read | `component`, `detail?` |
| `insert_component` | `asm.insert` | file/session (not in drafts) | `path`, `configuration?`, `detail?`, `transform?` |
| `hardware_catalog` | `asm.hardware_catalog` | read | `detail?` |
| `insert_hardware` | `asm.insert_hardware` | file/session (not in drafts) | `hardware`, `detail?`, `folder?`, `transform?` |
| `update_component` | `asm.component.update` | file/session (not in drafts) | `key`, `configuration?`, `detail?`, `exclude_from_bom?`, `fixed?`, `flexible?`, `hidden?`, `name?`, `suppressed?`, `transform?` |
| `delete_components` | `asm.component.delete` | file/session (not in drafts) | `keys`, `detail?` |
| `add_mate` | `asm.mate.add` | file/session (not in drafts) | `mate`, `detail?`, `name?` |
| `update_mate` | `asm.mate.update` | file/session (not in drafts) | `id`, `detail?`, `mate?`, `name?`, `suppressed?` |
| `delete_mates` | `asm.mate.delete` | file/session (not in drafts) | `ids`, `detail?` |
| `move_component` | `asm.move` | file/session (not in drafts) | `component`, `motion`, `collision?`, `detail?`, `phase?` |
| `add_component_pattern` | `asm.pattern.add` | file/session (not in drafts) | `pattern`, `seeds`, `detail?`, `name?`, `skip?` |
| `mirror_components` | `asm.mirror.add` | file/session (not in drafts) | `plane`, `components`, `detail?`, `name?`, `position?` |
| `add_explode_step` | `asm.explode.add` | file/session (not in drafts) | `components`, `motion`, `detail?`, `name?` |
| `interference` | `asm.interference` | read | `components?`, `detail?`, `include_hidden?`, `subassemblies_as_components?`, `treat_coincidence?` |
| `clearance` | `asm.clearance` | read | `minimum`, `components?`, `detail?` |
| `bill_of_materials` | `asm.bom` | read | `csv?`, `detail?`, `type?` |
| `import_step_assembly` | `asm.import_step` | file/session (not in drafts) | `path`, `detail?`, `folder?` |
| `new_drawing` | `drawing.new` | file/session (not in drafts) | `configuration?`, `detail?`, `model?`, `projection?`, `size?`, `standard?` |
| `standard_views` | `drawing.views.standard` | file/session (not in drafts) | `sheet`, `model`, `configuration?`, `detail?`, `isometric?`, `scale?` |
| `model_planar_faces` | `drawing.model.faces` | read | `model`, `configuration?`, `detail?` |
| `update_drawing` | `drawing.update` | file/session (not in drafts) | `detail?` |
| `drawing_info` | `drawing.info` | read | `detail?` |
| `render_sheet` | `drawing.render` | read | `detail?`, `sheet?`, `summary?` |
| `add_sheet` | `drawing.sheet.add` | file/session (not in drafts) | `detail?`, `name?`, `portrait?`, `projection?`, `scale?`, `size?`, `template?` |
| `update_sheet` | `drawing.sheet.update` | file/session (not in drafts) | `id`, `detail?`, `format_visible?`, `index?`, `name?`, `portrait?`, `projection?`, `properties_view?`, `scale?`, `size?`, `template?` |
| `delete_sheets` | `drawing.sheet.delete` | file/session (not in drafts) | `ids`, `detail?` |
| `add_drawing_view` | `drawing.view.add` | file/session (not in drafts) | `sheet`, `view`, `origin`, `detail?`, `display?`, `name?`, `scale?`, `tangent?` |
| `update_drawing_view` | `drawing.view.update` | file/session (not in drafts) | `id`, `aligned?`, `break_gap?`, `break_style?`, `breaks?`, `broken_out?`, `crop?`, `detail?`, `display?`, `hatch?`, `hidden_components?`, `hidden_edges?`, `label?`, `name?`, `origin?`, `scale?`, `tangent?`, `view?` |
| `delete_drawing_views` | `drawing.view.delete` | file/session (not in drafts) | `ids`, `detail?` |
| `add_annotation` | `drawing.annotation.add` | file/session (not in drafts) | `sheet`, `annotation`, `detail?`, `layer?`, `view?` |
| `update_annotation` | `drawing.annotation.update` | file/session (not in drafts) | `id`, `annotation?`, `detail?`, `layer?`, `view?` |
| `move_annotations` | `drawing.annotation.move` | file/session (not in drafts) | `ids`, `by`, `detail?` |
| `insert_model_items` | `drawing.model_items` | file/session (not in drafts) | `sheet`, `detail?`, `items?`, `views?` |
| `drive_dimension` | `drawing.dimension.drive` | file/session (not in drafts) | `id`, `value`, `detail?` |
| `auto_balloons` | `drawing.balloons.auto` | file/session (not in drafts) | `sheet`, `view`, `detail?`, `ignore_multiple?`, `layout?` |
| `table_csv` | `drawing.table.csv` | read | `id`, `detail?`, `path?` |
| `set_drafting_standard` | `drawing.standard.set` | file/session (not in drafts) | `detail?`, `load?`, `save?`, `standard?` |
| `export_drawing` | `drawing.export` | file/session (not in drafts) | `path`, `detail?`, `dxf_version?`, `monochrome?`, `per_sheet?`, `sheets?` |
| `delete_annotations` | `drawing.annotation.delete` | file/session (not in drafts) | `ids`, `detail?` |
| `set_drawing_properties` | `drawing.properties.set` | file/session (not in drafts) | `properties`, `additionalProperties?`, `detail?` |

#### Sketch operations (`edit_sketch` `op`)

| `op` | Fields | Notes |
| --- | --- | --- |
| `add_point` | `at` | A standalone point; hole features drill at standalone points. |
| `add_line` | `a`, `b` | `created`: the line, then its two points. |
| `add_circle` | `center`, `radius` | `created`: the circle. Its center point is not listed. |
| `add_arc3` | `start`, `end`, `via` | Arc from `start` to `end` passing through `via`. |
| `add_center_arc` | `center`, `start`, `end`, `clockwise?` | Arc around `center` from `start`, counter-clockwise to the direction of `end`. |
| `add_tangent_arc` | `start`, `tangent`, `end` | Arc from point `start`, leaving in direction `tangent`. |
| `add_spline` | `points`, `closed?` |  |
| `rectangle` | `a`, `b`, `center?` | Corner rectangle from `a` to `b`, or `center: true` for center `a` and corner `b`. Corner `created`: the 4 corners from `a`, then the bottom, right, top and left lines. |
| `slot` | `a`, `b`, `width` | Straight slot between arc centers `a` and `b`. |
| `polygon` | `center`, `vertex`, `sides` | Regular polygon around `center` with one vertex at `vertex`. |
| `constraint` | `constraint` | A geometric relation. Entity 0 is the fixed sketch origin. |
| `dimension` | `constraint`, `value?` | A dimension. The value in `constraint` is ignored: the dimension is measured from the geometry, then set to `value` if given. |
| `set_value` | `id`, `value` | Change a dimension's value (mm or radians). |
| `set_driving` | `id`, `driving` | Make a dimension driving (`true`) or driven (`false`). |
| `delete` | `ids` | Delete entities or relations, with anything that depends on them. |
| `move` | `ids`, `delta` | Translate entities by `delta`. |
| `trim` | `entity`, `pick` | Remove the part of `entity` between the intersections around `pick`. |
| `extend` | `entity`, `pick` | Extend a line from the end nearest `pick` to the next curve. |
| `offset` | `ids`, `distance` | Offset a chain or circle to its left (negative: right). |
| `mirror` | `ids`, `about` | Mirror entities about line `about`. |
| `fillet` | `point`, `radius` | Round the corner at `point` where two lines meet. |
| `construction` | `ids`, `construction` | Toggle construction geometry. |
| `convert` | `refs` | Project model edges into the sketch; a face converts all of its edge loops. |

#### Sketch relations and dimensions (`constraint`)

| `type` | Fields | Notes |
| --- | --- | --- |
| `coincident` | `a`, `b` | Two points at the same place. |
| `point_on_curve` | `point`, `curve` | A point lying on a line (extended), circle or arc. |
| `midpoint` | `point`, `line` |  |
| `horizontal` | `line` |  |
| `vertical` | `line` |  |
| `horizontal_points` | `a`, `b` |  |
| `vertical_points` | `a`, `b` |  |
| `parallel` | `a`, `b` |  |
| `perpendicular` | `a`, `b` |  |
| `collinear` | `a`, `b` |  |
| `equal` | `a`, `b` | Equal length (lines) or equal radius (circles and arcs). |
| `tangent` | `a`, `b` | Line–circle or circle–circle tangency. `inside` is for a circle tangent inside another; it's worked out when the relation is added. |
| `concentric` | `a`, `b` |  |
| `symmetric` | `a`, `b`, `about` | Points `a` and `b` mirrored about line `about`. |
| `fix` | `entity` | Hold an entity where it is. `at` is filled in when added. |
| `distance` | `a`, `b`, `value` | Point–point, point–line, or line–line (parallel) distance, mm. |
| `horizontal_distance` | `a`, `b`, `value` | Signed horizontal distance `b.x - a.x`, mm. |
| `vertical_distance` | `a`, `b`, `value` | Signed vertical distance `b.y - a.y`, mm. |
| `length` | `line`, `value` | Line length, mm. |
| `radius` | `curve`, `value` | Circle or arc radius, mm. |
| `diameter` | `curve`, `value` | Circle or arc diameter, mm. |
| `angle` | `a`, `b`, `value` | Signed angle from line `a`'s direction to line `b`'s, radians. |

#### Geometry references (`Ref`)

| `type` | Fields | Notes |
| --- | --- | --- |
| `body` | `id` | A whole solid body, by its stable ID from `doc.info` `bodies`. |
| `body_topology` | `body`, `kind`, `name` | A face, edge or vertex of one body, as returned by selection. |
| `origin` | none | The model origin point. |
| `front_plane` | none | The XY plane. |
| `top_plane` | none | The XZ plane. |
| `right_plane` | none | The YZ plane. |
| `face` | `name` | A face by stable name, such as `2.end`, from `doc.topology`. |
| `edge` | `name` | An edge by stable name, such as `E(2.end\|2.s5)`, from `doc.topology`. |
| `vertex` | `name` | A vertex by stable name, from `doc.topology`. |
| `feature` | `id` | A reference plane, axis, point, coordinate system or sketch. |
| `sketch_entity` | `sketch`, `entity` | A point or line in a sketch. |
| `curve_segment` | `curve`, `segment` | A generated segment with an unambiguous source-derived identity from the scene. |
| `ordered_sketch_contour` | `sketch`, `entities`, `reverse?`, `seam?`, `start?` | A complete contour with explicit direction and start/seam entity. |
| `sketch_contour` | `sketch`, `entities` | One complete sketch loop or open chain, identified by its member entity IDs rather than a contour array index. Order does not affect identity. |

<!-- /generated:tools -->

## Tool results and error codes

Every catalog tool returns one envelope: MCP `structuredContent` (its `outputSchema`
is the same for every tool), and the `draft.apply` result. The MCP text content is a
one-line summary followed by the same JSON.

```json
{"ok": false,
 "error": {"code": "E_NO_EFFECT", "message": "Cut-Extrude1 added or removed no material; …",
           "feature": 4, "repair": "Fix the feature with update_feature: …"},
 "revision": 7, "generation": "…", "draft": "…",
 "changed": [{"id": 4, "name": "Cut-Extrude1", "kind": "extrude", "change": "added"}],
 "created": {"feature": 4, "entities": []}, "deleted": [],
 "feature_errors": {}, "existing_errors": {},
 "warnings": [], "measurements": {"volume_mm3": 24000.0, "bbox_mm": [[0,0,0],[40,30,20]], "body_count": 1},
 "next": ["Fix the feature with update_feature: …"], "result": {"id": 4}}
```

- `ok` is false when the call fails, when it introduces a feature error or leaves one
  on a feature it changed, or when an added or changed material feature (extrude,
  revolve, sweep, loft, hole, shell, fillet, chamfer, variable fillet, draft,
  pattern, mirror) adds or removes no material. MCP `isError` equals `!ok`.
- `feature_errors` holds those errors; `existing_errors` holds errors the part
  already had, untouched by the call.
- `changed`, `created` and `deleted` come from comparing the document before and
  after; `created.entities` lists new sketch entity and relation IDs.
- `measurements` (active configuration) follow every successful change.
- `result` is the tool's own result without scenes. By default, mutations omit the
  snapshot (keeping `id` and `sketch`), sketch curves omit display samples and
  `part_info` bodies omit topology index arrays; pass `detail: "full"` for everything.

<!-- generated:errors -->
<!-- Generated from the core tool catalog (crates/openforge/src/tools). Do not edit:
run `cargo run -p openforge -- generate` instead. -->

| Code | Meaning | Repair |
| --- | --- | --- |
| `E_INVALID_ARGS` | Arguments do not match the tool's input schema. | Fix the argument at `path`; `allowed` lists valid values. The tool schema and its examples show the shape. |
| `E_UNKNOWN_FEATURE` | No feature has this ID. | Read feature IDs from part_info `features`; do not guess them. |
| `E_NOT_A_SKETCH` | The feature exists but is not a sketch. | Pass a sketch feature ID (type sketch in part_info `features`). |
| `E_UNKNOWN_ENTITY` | No sketch entity or relation has this ID, or it is the wrong kind. | Read entity IDs from get_sketch or the `created` list of the edit that made them. |
| `E_UNKNOWN_REF` | A face, edge, plane or other reference does not resolve. | Take face and edge refs from topology or the user's selection, and plane refs from part_info. |
| `E_UNKNOWN_PARAMETER` | No registered parameter has this key. | Use a key from part_info `parameters`. |
| `E_UNKNOWN_CONFIGURATION` | No configuration has this ID. | Use an ID from part_info `configurations.items`. |
| `E_UNKNOWN_BODY` | No body has this ID. | Use a body ID from part_info `bodies`. |
| `E_INVALID_FORMULA` | A variable, equation or configuration is invalid. | Check variable names, units (lengths need mm, in, …) and that every key exists in part_info `parameters`. |
| `E_REBUILD_FAILED` | The call added or changed a feature that failed to rebuild. | Read the feature with get_feature, then fix it with update_feature (smaller radius, other direction, valid refs) or delete it. |
| `E_NO_EFFECT` | A material feature rebuilt but added or removed no material. | Fix the feature with update_feature: a cut sketched on a face usually needs `reverse: true`; check the sketch lies on or inside the part. |
| `E_STALE_REVISION` | The document changed since the request's revision. | The part changed; read part_info again and start over from the current state. |
| `E_EDIT_IN_PROGRESS` | A manual feature or sketch edit is open. | Wait until the user finishes the open feature or sketch edit. |
| `E_DRAFT_MISSING` | The draft does not exist or expired. | Start a new draft; drafts expire after ten minutes. |
| `E_NOT_ALLOWED` | The operation is not available here (for example, a file operation in a draft). | Use a tool from the catalog; file operations and undo stay with the user. |
| `E_FAILED` | Any other failure. | Read the message, inspect the part with part_info, and try a different approach. |

| Warning | Meaning |
| --- | --- |
| `W_SKETCH_UNDER_DEFINED` | The sketch can still move; add dimensions or relations. |
| `W_SKETCH_OVER_DEFINED` | The sketch has conflicting or redundant relations. |

<!-- /generated:errors -->
