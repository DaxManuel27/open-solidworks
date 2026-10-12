# 16. Tolerancing and MBD

Detailed specs for [catalog section 16](../solidworks-feature-catalog.md#16-tolerancing-and-mbd). See the [README](README.md) for sources and conventions.

Model-based definition (MBD) puts tolerances and annotations on the 3D model instead of a drawing. DimXpert creates the dimensions and tolerances; 3D views and publishing tools share them.

**OpenForge today:** none of these.

## DimXpert for parts

Recognises manufacturing features (holes, slots, pockets, bosses, planes, cylinders, cones, chamfers, fillets, notches, widths) and applies dimensions and tolerances that follow ASME Y14.41 or ISO 16792.

| Option | Values |
| --- | --- |
| Tolerance standard | ASME Y14.5 or ISO 1101 |
| Geometric tolerance or plus/minus | Scheme style |
| Block tolerances | General tolerance by number of decimals |
| General tolerance | ISO 2768 class (fine, medium, coarse, very coarse) |
| Feature recognition | Which features to recognise; pattern recognition; chamfer and fillet limits |
| Display | Dimension placement, annotation views |

## Auto dimension scheme

Builds a full scheme in one step. Options: part type (prismatic or turned), tolerance type (plus/minus or geometric), pattern dimensioning (linear or polar), primary, secondary and tertiary datums, scope (all features or selected), feature filters.

## DimXpert size, location, angle and basic dimensions

Manual tools for each: size dimensions on features (diameters, widths), location dimensions between features, angle dimensions, basic dimensions for GD&T, with tolerance per dimension.

## Datums and geometric tolerances

Datum feature symbols on faces or features; feature control frames with the same characteristics and modifiers as drawings (see [drawings](15-drawings-and-detailing.md#symbols)); composite tolerances; datum targets.

## Show tolerance status

Colours features by status: fully constrained (green), under constrained (yellow), over constrained (red), not toleranced.

## DimXpert in assemblies and auto pair tolerance

DimXpert annotations on assembly features; auto pair tolerance applies matching tolerances to mating features of two components (for example a pin and hole) based on fit.

## TolAnalyst

Tolerance stack-up study: pick a measurement between two features, an assembly sequence and constraints; it computes worst-case and RSS (root sum square) min and max, lists contributors with percentage contribution and sensitivity.

## 3D views (MBD)

Saved combinations of orientation, display state, configuration, zoom and visible annotations, listed in the 3D Views tab at the bottom of the window. Options: capture a view, update, delete, show in 3D PDF.

## Annotation views

Named planes (front, top, right, isometric, unassigned, custom) that annotations belong to; annotations rotate to face their view. Options: insert, edit, hide, flip, and move annotations between views.

## Publish to 3D PDF

Publishes the model with 3D views, tables, images and text to a PDF using templates. Template editor places 3D view areas, tables (BOM, notes), images and properties. Options: include 3D views selected, STEP attachment, security settings.

## Publish to STEP 242 with PMI

Exports STEP AP242 with semantic PMI (dimensions, GD&T, datums tied to geometry) and 3D views.

## 3D PMI compare

Compares annotations between two versions of a model and reports added, removed and changed PMI.

## Import PMI

Reads semantic PMI from STEP 242, Creo, NX and CATIA files into DimXpert annotations.
