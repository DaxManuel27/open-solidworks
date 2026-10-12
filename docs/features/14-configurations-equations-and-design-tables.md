# 14. Configurations, equations and design tables

Detailed specs for [catalog section 14](../solidworks-feature-catalog.md#14-configurations-equations-and-design-tables). See the [README](README.md) for sources and conventions.

## Configurations

Variants of a part or assembly stored in one file. Each configuration has its own values for configurable items; one is active at a time. API: `IConfiguration`, `IConfigurationManager`.

**Configuration properties**

| Property | Meaning |
| --- | --- |
| Name, description | Description can be used in BOMs |
| BOM part number | Document name, configuration name, link to parent, or user specified |
| Child components display in BOM | Show, hide, or promote (dissolve) for assemblies (`swConfigOption_DontShowPartsInBOM`, `DoDisolveInBOM`) |
| Advanced options | Suppress new features and mates; hide new components; suppress new components; use configuration-specific colour; add rebuild/save mark (keeps geometry so it opens without rebuilding) |
| Parent/child | Derived configurations link to a parent |

**Rules:** an item not set per configuration takes its value from the base for all configurations. When editing a dimension, the user chooses "this configuration", "all configurations" or "specify configurations".

**OpenForge today:** create, rename, delete and activate; typed overrides of parameters, equations, variables, suppression and material; base vs active editing; reset to base; undo; persistence; export by configuration.
**Missing:** descriptions and BOM options, derived configurations, advanced options (suppress new features, rebuild marks), assembly component configurations.

## Derived configurations

A child configuration inherits everything from its parent except the values it overrides. Changes to the parent flow down unless overridden.

**OpenForge today:** not available (no inheritance).

## Configurable items

| Item | Parts | Assemblies |
| --- | --- | --- |
| Dimension values | ✓ | ✓ (assembly features, mate values) |
| Feature and sketch suppression | ✓ | ✓ |
| End conditions, sketch relations (some) | ✓ | |
| Material | ✓ | |
| Custom properties | ✓ | ✓ |
| Colours/appearances | ✓ | ✓ |
| Equations (on/off) and global variable values | ✓ | ✓ |
| Hole Wizard size and type | ✓ | |
| Component configuration, suppression, visibility, fixed/float | | ✓ |
| Mates (suppress, values) | | ✓ |
| Lights, cameras, scenes | ✓ | ✓ |
| Sheet metal parameters, weldment profile sizes | ✓ | |

**OpenForge today:** parameters, formulas, variables, suppression and material.

## Modify Configurations dialog

A table with configurations as rows and chosen dimensions, suppression states, materials or components as columns; edit cells directly, add configurations by typing a new row name, save named table views.

**OpenForge today:** not available as a table; the configurations panel edits one configuration at a time.

## Design tables

An embedded Excel sheet driving configurations: row 2 holds parameter headers, column A holds configuration names. Header syntax examples:

| Header | Meaning |
| --- | --- |
| `D1@Sketch1` | Dimension value |
| `$STATE@Fillet1` | Suppression (S/U) |
| `$CONFIGURATION@Part<1>` | Component configuration |
| `$PRP@Description` | Custom property |
| `$COLOR` | Colour as an RGB integer |
| `$PARENT` | Parent configuration (derived) |
| `$SW-MASS`, `$SW-COG` | Mass and center of gravity overrides |
| `$TOLERANCE@D1` | Tolerance |
| `$USER_NOTES`, `$COMMENT` | Ignored columns |

Options: auto-create from selected dimensions, allow model edits to update the table, block model edits, warn on update, link to an external file.

**OpenForge today:** not available.

## Configuration Publisher

Builds a custom PropertyManager for a configurable part (drop-down lists, text boxes, number inputs, checkboxes bound to design table columns), with rules that limit combinations. Used for 3D ContentCentral and smart catalogue parts.

**OpenForge today:** not available.

## Equations

Formulas in the Equations dialog, which has three views: equations, sketch equations, dimensions, and ordered view (solve order). API: `IEquationMgr`.

| Element | Syntax and behaviour |
| --- | --- |
| Dimension equation | `"D1@Boss-Extrude1" = "D2@Sketch1" * 2` |
| Functions | sin, cos, tan, sec, cosec, cotan, arcsin, arccos, arctan, arcsec, arccosec, arccotan, abs, exp, log, sqr (square root), int, sgn, pi |
| Conditionals | `IIF(condition, true, false)` |
| Properties | File properties and mass, volume, area, center of mass references ("SW-Mass@Part1.SLDPRT") |
| Units | Values with units such as `25.4mm` or `1in` |
| Feature suppression | `"Fillet1" = "suppressed"` or IIF returning suppressed/unsuppressed |
| Evaluation | Solve order follows dependencies; circular references are flagged |
| Per configuration | Equations can be configured (this, all, or specified configurations) |
| In dimension boxes | Typing `=` in any dimension field creates an equation |

**OpenForge today:** typed expressions with units, functions abs, sqrt, min, max, sin, cos and tan, globals, stable keys, cycle diagnostics, formula editing on existing features, `=formula` entry in feature fields while creating or editing, syntax errors rejected before saving (previews still show them per field), rebuilding only from the first feature an edit affects.
**Missing:** inverse trig, exp, log, int, sgn and IIF conditionals, suppression equations, file-property and mass-property references, ordered view.

## Global variables

Named values defined in the equations list (`"Width" = 50`) and referenced by dimensions and other equations. Can be configured per configuration.

**OpenForge today:** globals available.

## Linked values

Several dimensions share one named value (link values); changing one changes all. Older than global variables; still supported.

**OpenForge today:** covered by globals; no separate link-values command.

## Equations in external files

Link equations to a `.txt` file so several models share them; options to link (automatic update) or import once, and to export equations.

**OpenForge today:** not available.

## Custom and configuration properties

Name/type/value properties stored in the file, used in BOMs, title blocks and PDM. Types: text, date, number, yes/no. Values can be linked to model properties (`"SW-Mass@Part1.SLDPRT"`, material, dimensions). Two levels: custom (file) and configuration specific. Summary information also stores author, keywords, comments, title, subject.

**OpenForge today:** not available.

## Property Tab Builder

A tool that designs a custom properties tab (text boxes, lists, number fields, checkboxes, radio buttons) for the Custom Properties task pane, saved as a template so users fill in properties consistently.

**OpenForge today:** not available.
