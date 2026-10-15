# 15. Drawings and detailing

Detailed specs for [catalog section 15](../solidworks-feature-catalog.md#15-drawings-and-detailing). See the [README](README.md) for sources and conventions.

A drawing (`.slddrw`) holds sheets; each sheet has a sheet format (border and title block) and views of parts or assemblies. Views stay associative: model changes update views, and model dimensions shown in a drawing can drive the model. API: `IDrawingDoc`, `ISheet`, `IView`; view types in `swDrawingViewTypes_e`.

**OpenForge today:** drawings, sheets and formats, every view kind listed below except alternate position, detached, 3D and flat pattern views, model items and driving dimensions, every dimension scheme and tolerance type, notes, balloons, symbols, cosmetic threads, BOM, hole, revision, general and general tolerance tables, ISO and ANSI standards, PDF and DXF output (Phase 4; see [status](../phase-4-status.md) for limits). Blocks, DrawCompare, auto-generation, other standards, DWG, eDrawings and detailing mode are deferred.

## Sheets and views

### Drawing sheets, sheet formats and title blocks

| Option | Values |
| --- | --- |
| Sheet size | Standard (A0–A4, ANSI A–E) or custom width and height |
| Sheet format | `.slddrt` file with border, title block and linked note fields; display or hide |
| Scale | Sheet scale, inherited by views by default |
| Projection | First angle or third angle |
| Next view label, next datum label | Letters used for new section, detail and datum labels |
| Use custom property values from model shown in | Which view's model feeds title block properties |
| Zones | Grid labels along the border, used in view labels and revision tables |

Title block fields link to custom properties (`$PRP:"Description"`, `$PRPSHEET:"PartNo"`), sheet properties (scale, sheet number of total) and system values (date, file name).

### View types

| View | Inputs and options |
| --- | --- |
| Standard 3 view | Front, top and right (or first-angle equivalents) from a model |
| Model view | Any named or standard orientation (`swStandardViews_e`: front, back, left, right, top, bottom, isometric, dimetric, trimetric), plus current model view and saved views; import annotations; number of views |
| Projected view | Folded out from a parent in any of eight directions |
| Auxiliary view | Normal to a selected edge in a parent view |
| Section view | Cutting line from a sketch: horizontal, vertical, auxiliary or aligned; full, half section (eight layouts), offset (jogged line), aligned (rotated segment), partial (section depth). Options: flip direction, scale with model, hatch per body with material hatching, exclude ribs and fasteners from section, show surface bodies, emphasise outline, slice (show only the cut faces), section depth |
| Detail view | Circle or sketch profile; styles standard, broken, leader, no leader, connected (`swDetViewStyle_e`); full outline; jagged outline with intensity; pin position; scale |
| Broken-out section | Closed sketch on a view cut to a depth or up to an edge, with preview |
| Break view | Vertical or horizontal break lines, gap size and style (straight, curved, zig-zag, small zig-zag, jagged); dimensions show true length |
| Crop view | Trims a view to a closed sketch |
| Alternate position view | Overlays a second component position in phantom lines, from a new or existing configuration |
| Exploded view | Assembly view in its exploded state; shows explode lines |
| Relative to model | Orientation from two picked model faces |
| Detached view | View that can be moved without updating its parent |
| 3D drawing view | Temporarily rotate a view to select hidden edges |
| Flat pattern view | Sheet metal flat, with bend lines and notes |

### Display options

- **Display style:** wireframe, hidden lines visible, hidden lines removed, shaded with edges, shaded, draft-quality variants (`swDisplayMode_e`).
- **Tangent edges:** visible, with font, or removed.
- **Edge visibility:** show or hide individual edges and components (hide/show edges, component line font).
- **Layers:** named layers with colour, style and thickness; on/off and print control.
- **Line formats:** colour, thickness and style per entity.

### Auto-generate drawings (beta)

Places views and dimensions automatically from a model (2025 and later).

### DrawCompare

Overlays two drawings and colours what was added, removed or unchanged.

## Annotations and dimensions

### Model items

Imports dimensions and annotations from the model into chosen views. Source: entire model, selected feature or selected component. Types: dimensions marked for drawing, not marked, instance/revolution counts, tolerance dimensions, hole wizard locations and profiles, hole callouts, annotations (notes, surface finish, geometric tolerance, datums, cosmetic threads, welds), reference geometry, sketches. Options: eliminate duplicates, include items from hidden features, use dimension placement in sketch.

### Dimensions in drawings

All the sketch dimension types plus ordinate (horizontal, vertical, angular), baseline, chain, chamfer and angular running dimensions (`swDimensionType_e`). Rapid dimension places dimensions with an on-screen selector; autodimension creates baseline or ordinate schemes from datums.

**Dimension properties:** tolerance type (`swTolType_e`: none, basic, bilateral, limit, symmetric, min, max, fit, fit with tolerance, fit tolerance only, block, general), precision for value and tolerance, fit classes (hole and shaft ISO fits), primary and dual units, text before and after the value, leader and arrow style, break lines, inspection dimension, foreshortened, show parentheses.

### Notes and balloons

- **Notes:** text with links to properties, symbols library, hyperlinks, leaders (with jog, bent leader), borders, flag notes, fit text to box.
- **Balloons:** item number, quantity, custom property or text; styles circular, triangle, hexagon, box, diamond, pentagon, split circle, flag and more (`swBalloonStyle_e`); auto balloon with layout (square, circular, top, bottom, left, right) and ignore multiple instances; stacked balloons.

### Hole callouts

Pick a hole edge; the callout reads Hole Wizard data (`⌀6.6 THRU, ⌴ ⌀11 ↧6.5`) using a callout format file per standard. Works on simple holes and cuts too, with less information.

### Center marks and centerlines

Center marks on circles and arcs (single, linear, circular patterns), mark size, extended lines, connection lines; centerlines between parallel edges and on cylindrical faces; auto-insert options per view.

### Symbols

| Symbol | Contents |
| --- | --- |
| Surface finish | Symbol type (basic, machining required, prohibited, etc.), lay direction, roughness values, production method |
| Weld symbol | ANSI/ISO weld symbols, arrow and other side, contour, field weld, all-around, stagger (`swWeldSymbol*`) |
| Datum feature | Label, style, leader |
| Datum target | Point, circle or rectangle target, size |
| Geometric tolerance | Characteristics (straightness, flatness, circularity, cylindricity, line/surface profile, angularity, perpendicularity, parallelism, position, concentricity, symmetry, circular and total runout; `swGtolGeomCharSymbol_e`), tolerance zone with diameter, modifiers (MMC, LMC, RFS, projected, free state, tangent plane, statistical), datum references, composite frames |

### Cosmetic threads in drawings

Shown per standard: thin lines for minor/major diameter in side views, three-quarter arc in end views (ISO) or full circle (ANSI); callout text from the thread feature; shaded display optional.

### Area hatch/fill

Hatch or solid fill in closed regions, with pattern library (ANSI31, ISO steel, etc.), scale and angle; material hatch in sections.

### Revision clouds and symbols

Cloud shapes (rectangle, ellipse, polygon, freehand) and revision symbols linked to a revision table row.

### Blocks in drawings

Reusable sketch and note groups, saved as `.sldblk`, with attributes and scaling.

### Format painter, styles and drafting standards

Copies formatting between annotations; saved styles for dimensions and notes; document drafting standard (ANSI, ISO, DIN, JIS, BSI, GOST, GB) controls fonts, arrows, tolerances and view labels; standards can be saved and loaded.

### Spell check and find/replace

Checks note text against a dictionary; find and replace across notes and annotations.

## Tables

Table types from `swTableAnnotationType_e`: general, hole chart, BOM, revision block, weldment cut list, title block, weld, bend, punch, general tolerance, family.

### Bill of materials

| Option | Values |
| --- | --- |
| BOM type | Top-level only, parts only, indented (with no, detailed or flat numbering) (`swBomType_e`, `swNumberingType_e`) |
| Template | Columns: item number, part number, description, quantity, any custom property, mass, material, equation |
| Configurations | Which configurations to list; quantity per configuration |
| Grouping | Display as one item number across configurations; keep missing items; strike out missing |
| Item numbering | Start, increment, do not change, follow assembly order |
| Part number source | Document name, configuration name or property |
| Excluded components | Exclude from BOM; envelope components excluded |
| Excel-based BOM | Embedded Excel table (legacy) |

### Hole table

Origin (datum), holes picked by face or edge; columns tag, X, Y, size; options: combine same tags, combine same sizes, tag style (A1/A2), alphabetic or numeric, show ATC tags, scheduled positions.

### Revision table

Columns revision, description, date, approved; add revision with automatic letters or numbers; revision cloud and symbol linking; zones; sheet-specific or all sheets.

### Bend and punch tables

Bend table lists each bend's tag, direction, angle, radius and order. Punch table lists forming tool IDs, locations and quantities.

### Weld and cut list tables

Weld table lists weld size, symbol, length, quantity. Weldment cut list table lists items with length, angles and quantity.

### General, general tolerance and title block tables

General tables with free cells and equations; general tolerance tables per ISO 2768; title block tables as an alternative to sheet format notes.

### Design and family tables

Design table shown as a picture of the Excel sheet; family table lists configurations with chosen dimension values.

### Table equations

Cells can hold equations referencing other cells or functions (SUM, COUNT, AVG, MIN, MAX) and properties.

## Output

### Print and print to PDF

Page setup per sheet (paper size, orientation, scale to fit, line weights, colour or black and white), print range, print all sheets or current, PDF export with layers and embedded fonts, include sheets selection.

### Export DXF/DWG

Options: version (R12 to 2018), fonts (TrueType or AutoCAD standard), line styles, custom map file for layers, scale 1:1 output, splines (export as splines or polylines), sheets to export (active, all, all into one file), and merging end points within a tolerance.

### eDrawings and markups

Lightweight `.edrw`, `.eprt`, `.easm` files with measurement, section, animation and markup tools; options to allow measure, save STL and include shaded data.

### Detailing mode and lightweight drawings

Detailing mode opens large drawings without models, allowing annotation edits only. Resolved vs lightweight drawing loading; hide/show models.
