# 24. New in SOLIDWORKS 2026

Details for [catalog section 24](../solidworks-feature-catalog.md#24-new-in-solidworks-2026). See the [README](README.md) for sources and conventions. Each item points to the spec it extends.

| Feature | Behaviour | Extends |
| --- | --- | --- |
| Escape to cancel slow operations | Pressing Esc during a long fillet, chamfer or pattern calculation cancels it and returns to the PropertyManager with inputs kept | [Fillets](03-part-features.md#fillets), [patterns](04-patterns-and-mirroring.md) |
| Reference points by numeric XYZ | Reference point type with typed X, Y and Z, relative to the origin or a coordinate system | [Reference point](07-reference-geometry.md#reference-point) |
| Bounding box by coordinate system | Box aligned to a chosen coordinate system's axes | [Bounding box](07-reference-geometry.md#bounding-box) |
| Fixed bounding box directions | Length, width and thickness stay assigned to the same axes when the model changes, instead of swapping by size | [Bounding box](07-reference-geometry.md#bounding-box) |
| Select bodies by size | Picks bodies smaller than a percentage of the part size, then deletes or keeps them (for cleaning imports) | [Delete/keep body](06-direct-editing-and-body-tools.md#deletekeep-body) |
| Base flange "From" option | Base flange can start from a sketch plane offset, a vertex, or a face, like extrude | [Base flange](10-sheet-metal.md#base-flangetab) |
| Break corner "Internal corners only" | Applies break corners only to internal corners | [Corner treatments](10-sheet-metal.md#corner-treatments) |
| Structure system corner treatment UI | Corner groups colour-coded by type, with trim order control in the graphics area | [Corner treatments](11-weldments-and-structure-system.md#corner-treatments-structure-system) |
| Cut list to custom property link | Part custom properties can link to cut list item properties | [Cut lists](11-weldments-and-structure-system.md#cut-lists-cut-list-properties-and-folders) |

**OpenForge today:** XYZ reference points exist.
