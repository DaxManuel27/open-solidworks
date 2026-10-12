# 8. Multibody parts

Detailed specs for [catalog section 8](../solidworks-feature-catalog.md#8-multibody-parts). See the [README](README.md) for sources and conventions.

## Multiple solid bodies in one part

A part can hold several solid and surface bodies. A new boss creates a separate body when Merge result is off or when it doesn't touch existing material. Each body has a name, appearance, material (via weldment cut lists or per-body material) and visibility.

**Rules**

- A boss that touches an existing body merges into it when Merge result is on.
- Features that remove material (cuts, fillets, shells) act on the bodies their feature scope picks.
- Bodies are identified across rebuilds by the feature that created them. Renaming a body keeps it stable.

**OpenForge today:** multiple bodies with stable lineage-based IDs, names and visibility.

## Solid Bodies and Surface Bodies folders

The feature tree shows a Solid Bodies folder and a Surface Bodies folder with counts. Each body's shortcut menu offers: hide/show, isolate, delete/keep, appearance, material (per body), insert into new part, move/copy, combine, and properties. Bodies can be grouped into sub-folders.

**OpenForge today:** a body tree with names and visibility.
**Missing:** surface bodies, per-body material, isolate, sub-folders, insert into new part.

## Feature scope

Controls which bodies a feature affects. Options on most features: **all bodies** (auto-select, which also includes bodies created later) or **selected bodies**. API: `FeatureScope`, `AutoSelect`, `FeatureScopeBodies` on each feature's data interface.

**OpenForge today:** solid feature scope on extrude, revolve, sweep, loft, dome and others.

## Hide/show bodies

Visibility per body. Hidden bodies still exist for mass properties and features. Options: hide, show, isolate (hide all others temporarily), show with transparency.

**OpenForge today:** visibility per body. Hidden bodies stay in mass properties, modeling scope and exports.

## Exploded views of multibody parts

Like assembly explode steps, applied to bodies: pick bodies, drag along a triad, auto-space, with a list of steps. Used for drawings of weldments.

**OpenForge today:** not available.

## Interference detection between bodies

Finds overlapping volumes between bodies in a part, with a list of interferences and their volumes. Options: treat coincidence as interference, show ignored, make interfering bodies transparent.

**OpenForge today:** not available.

## Modeling techniques

SOLIDWORKS documents these multibody workflows, which rely on the features above:

- **Bridging:** model separate regions, then join them with a connecting feature.
- **Local operations:** fillet or shell one body before merging.
- **Symmetry:** model half and mirror the body.
- **Tool bodies:** model a shape as a body and use Combine subtract or Indent with it.
- **Insert part / Save bodies:** split a multibody "master" part into parts for an assembly (top-down design).

**OpenForge today:** combine, split, move/copy and mirror support bridging, symmetry and tool-body workflows.
