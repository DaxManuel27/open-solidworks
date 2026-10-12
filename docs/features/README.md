# SOLIDWORKS feature specs

Detailed, per-feature specs for every area in the [SOLIDWORKS feature catalog](../solidworks-feature-catalog.md). The catalog says *what* exists; these pages say *how each feature behaves*: its inputs and allowed values, how options interact, how it fails, and what OpenForge has today.

- **As of:** 2026-10-11, against OpenForge branch `phase-2`.
- **Coverage:** every catalog area. Modeling areas (sketching, part features, patterns, surfaces, body tools, reference geometry, sheet metal, weldments, mold, assemblies) are specified option by option. Drawings, display, UI and add-ins are specified at the level of their options and behaviour. Separately licensed products are summarised only.

## Clean-room use

These specs come only from public documentation (see [Sources](#sources)). They follow the repository's clean-room rule in [AGENTS.md](../../AGENTS.md):

- API names such as `IExtrudeFeatureData2` or `swEndConditions_e` are cited so each claim can be checked against the public reference. **Don't copy them into OpenForge code, file formats, UI labels or protocol names.** Use OpenForge's own naming.
- Nothing here comes from SOLIDWORKS binaries, file internals or screenshots.
- Where SOLIDWORKS behaviour is a design choice rather than a requirement, treat it as one option, not the spec OpenForge must match.

## How to read a page

Each feature section has:

- **What it does**, in a sentence or two, and the public API interface that describes it.
- **Inputs:** a table of options with allowed values. API property names are in the notes column. Enum values are listed with their numeric codes where the reference gives them; they're useful for spotting every option, not for reuse.
- **Rules and interactions:** constraints between options and with other features.
- **Failure cases:** what SOLIDWORKS reports as errors, so OpenForge can match the specificity of its diagnostics.
- **OpenForge today / Missing:** checked against the code on `phase-2` (`crates/of-model`, `crates/of-sketch`, `crates/of-io`) and [phase-2-status.md](../phase-2-status.md). Where this page and the catalog disagree, this page is newer.

## Pages

| # | Area | Depth |
| --- | --- | --- |
| 1 | [Sketching (2D)](01-sketching.md) | Every entity, tool, dimension and relation |
| 2 | [3D sketching, curves and splines](02-3d-sketching-and-curves.md) | Every curve feature |
| 3 | [Part features](03-part-features.md) | Option by option |
| 4 | [Patterns and mirroring](04-patterns-and-mirroring.md) | Option by option |
| 5 | [Surfaces](05-surfaces.md) | Option by option |
| 6 | [Direct editing and body tools](06-direct-editing-and-body-tools.md) | Option by option |
| 7 | [Reference geometry](07-reference-geometry.md) | Option by option |
| 8 | [Multibody parts](08-multibody-parts.md) | Behaviour |
| 9 | [Mesh and scan data](09-mesh-and-scan-data.md) | Behaviour and options |
| 10 | [Sheet metal](10-sheet-metal.md) | Option by option, with bend math |
| 11 | [Weldments and structure system](11-weldments-and-structure-system.md) | Option by option |
| 12 | [Mold design](12-mold-design.md) | Option by option |
| 13 | [Assemblies](13-assemblies.md) | Components, every mate type, tools |
| 14 | [Configurations, equations and design tables](14-configurations-equations-and-design-tables.md) | Behaviour and syntax |
| 15 | [Drawings and detailing](15-drawings-and-detailing.md) | Every view type, annotation and table |
| 16 | [Tolerancing and MBD](16-tolerancing-and-mbd.md) | Behaviour and options |
| 17 | [Evaluate and analysis tools](17-evaluate-and-analysis-tools.md) | Behaviour and options |
| 18 | [Model display and rendering](18-model-display-and-rendering.md) | Behaviour and options |
| 19 | [Import and export](19-import-and-export.md) | Formats and their options |
| 20 | [User interface and workflow](20-user-interface-and-workflow.md) | Behaviour |
| 21 | [Fundamentals: documents, files and automation](21-fundamentals-documents-files-and-automation.md) | Behaviour |
| 22 | [Built-in add-ins and Xpress tools](22-built-in-add-ins-and-xpress-tools.md) | Capabilities |
| 23 | [Separately licensed products](23-separately-licensed-products.md) | Summary only |
| 24 | [New in SOLIDWORKS 2026](24-new-in-solidworks-2026.md) | Linked to the specs they extend |

## Sources

- [SOLIDWORKS 2025 API Help](https://help.solidworks.com/2025/english/api/sldworksapi/): every feature data interface (170 interfaces read in full on 2026-10-11) and the [swconst enumerations](https://help.solidworks.com/2025/english/api/swconst/) (946 enumerations listed, about 470 relevant ones read).
- [SOLIDWORKS 2025 Design Help](https://help.solidworks.com/2025/english/SolidWorks/sldworks/c_Features_Top.htm), as already used for the catalog.
- The catalog's other sources for SOLIDWORKS 2026 additions.
- General SOLIDWORKS documentation knowledge for behaviour not stated in the API reference, such as UI options, failure messages and workflows. These parts are the least directly verified; check them against the Design Help before relying on exact option names.
