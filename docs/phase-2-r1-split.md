# R1 general body splitting

Implemented 2026-10-11. This is evidence for the general-split slice, not a Phase 2
completion claim.

## Supported contract

`split_body` keeps its existing `bodies`, `plane` and `keep` fields. A missing or
empty `tools` list uses the original plane-side operation, including the existing
`positive`/`negative` body IDs and the restriction to one solid per plane side.

A nonempty `tools` list enables general splitting. Each entry can be a reference
plane, a sketch plane, a bounded planar or curved face, or a whole solid body.
Plane references provide unbounded cutting surfaces; face references use the face's
actual trimmed boundary. Tools must divide every selected target into at least
two solids. Tool bodies remain in the part and cannot also be target bodies.

`retained_pieces: null` (the default) retains every resulting piece. An array retains
exactly those piece IDs, obtained from an all-pieces preview. Empty, duplicate and
unresolved selections produce explicit errors. General tools require `keep: both`;
side selection remains a legacy plane operation. `plane` defaults to the front
plane when omitted and is ignored in general mode.

The desktop Split Bodies panel provides both modes, bounded face/plane picking,
whole-tool body checkboxes, all-piece preview, retained-piece checkboxes, editing
and cancellation. Clearing “Show all pieces for selection” previews the retained
result. Tools remain visible and included in mass until another operation changes
them; retaining split pieces does not delete tool bodies.

## Algorithm and identity

The installed `BRepAlgoAPI_Splitter.hxx` documents separate argument/tool groups,
non-destructive processing and history. A compiled safe-wrapper experiment verifies
that a crossing solid slab yields three valid solids, a bounded face or an
unbounded plane yields two, a nonintersecting plane leaves one, source volume is
conserved and tool-face history reaches the result. A shared cutting face has
opposite topological orientations in its two adjacent solids.

The bridge records face history from both the target and the compound of cutting
tools. Each piece ID hashes a sorted set of source-face ancestry and oriented
cutting-face ancestry, qualified by the target and split feature. Positional
naming suffixes and OCCT enumeration indices do not enter that hash. Reordering
tools therefore does not rename pieces. Split-count changes retain unaffected
identities; changed/removed pieces do not silently bind to an unrelated result.

Two disconnected pieces can have identical ancestry, for example the two central
segments of a torus cut by a slab. Such a split fails with an explicit ambiguous
identity error. It does not assign arbitrary numbered identities. Unsupported
history omissions also fail explicitly. This conservative limitation remains
visible to the user.

New-feature previews now use the document's next feature ID without consuming it,
so discovered piece IDs are identical at commit. The existing revision checks
reject stale previews. File writing must use the accompanying V3 migration guard;
older readers must not ignore general-tool fields and reinterpret the operation
as a legacy plane split. V1/V2 defaults retain the old behavior.

## Verification

Passing focused checks:

- `cargo test -p of-geom --test tool_split`
- `cargo test -p of-model --test general_split --test multibody`
- `cargo test -p openforge --test serve general_split_preview`
- `cargo clippy -p of-model -p of-geom --all-targets -- -D warnings`
- `corepack pnpm exec eslint apps/desktop/src/renderer/src/panels/bodies.tsx`

The six general-split model cases cover arbitrary retained pieces, three pieces
from a whole-body tool, bounded faces, oriented plane sides, tool reordering,
upstream plane/body movement, split-count changes, retained volume, copy/combine,
save/reopen, invalid selections and ambiguous torus segments. The four original
multibody regressions still pass. The RPC case verifies preview/commit equality,
undo/redo, reload and unresolved saved selections through the shared tool contract.

`scripts/phase-2-split-smoke.mjs` adds real desktop controls for creation, retained
piece editing, cancellation, undo/redo, save/reopen and moving the upstream slab.
It identifies the middle piece from face centroids instead of assuming piece order.
The helper passed the integrated native desktop smoke run before the later dome
slice was added. The final combined candidate still requires its integrated gates.
