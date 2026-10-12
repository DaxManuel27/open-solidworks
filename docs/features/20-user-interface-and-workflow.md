# 20. User interface and workflow

Detailed specs for [catalog section 20](../solidworks-feature-catalog.md#20-user-interface-and-workflow). See the [README](README.md) for sources and conventions. The OpenForge status column in the catalog is accurate for this area; only gaps are repeated here.

## CommandManager

A tabbed toolbar above the graphics area. Default part tabs: Features, Sketch, Surfaces, Sheet Metal, Weldments, Mold Tools, Evaluate, MBD Dimensions, Render Tools, add-ins. Options: use large buttons with text, customise tabs per document type, show tabs for enabled add-ins, context-sensitive tab switching.

**Missing in OpenForge:** customisable tabs, Sketch and other area tabs as features arrive.

## FeatureManager design tree

Lists history in order: annotations, sensors, material, equations, solid and surface bodies, planes, origin, then features. Each feature expands to show its sketches and children.

| Capability | Behaviour |
| --- | --- |
| Rollback bar | Drag to an earlier point; later features are rolled back and new features insert there; rebuild rolls forward |
| Reorder by dragging | Checks parent/child order; refuses invalid moves |
| Suppress/unsuppress | Removes from rebuild, with children; configurable |
| Rename | Slow double-click or F2 |
| Folders | Group features (add to new folder); folders can be suppressed together |
| Favorites, tags, comments, attachments | Mark and annotate features |
| Parent/child | Dialog listing parents and children of a feature |
| Dynamic reference visualization | Arrows showing parent and child links on hover |
| Show feature dimensions | Option to show all feature dimensions in the graphics area |
| Tree filter | Search the tree by name, tag or type |
| Split tree | Two panes showing different parts of the tree |

**Missing in OpenForge:** rollback bar, folders, favorites/tags/comments/attachments, parent/child dialog and reference visualization, tree filter.

## Design Journal and Design Binder

A Word document stored inside the model (Design Journal) and a folder of attachments (Design Binder).

## What's Wrong?

Lists rebuild errors and warnings per feature with descriptions, show-all and help links; the tree marks errors (red) and warnings (yellow) and points to the first.

**Missing in OpenForge:** a central error list.

## PropertyManager

Feature options in a side panel with OK, Cancel, Preview, Help, and push pins to keep open; group boxes collapse; selection boxes list picked entities with right-click clear; numeric fields accept equations and units; spin boxes with increment; dimension handles in the graphics area.

**OpenForge today:** PropertyManager-style panels with live preview.

## Context toolbars and shortcut menus

Selection shows a context toolbar of likely commands; right-click shows a shortcut menu with context commands and the context toolbar on top. Options: show on selection, show in shortcut menu.

**Missing in OpenForge:** viewport context toolbars and shortcut menus.

## Shortcut bar and mouse gestures

S key opens a customisable toolbar at the cursor; right-drag gestures (4, 8 or 12 directions) run assigned commands.

## Keyboard shortcut customisation

Customise dialog lists all commands with search and assigned keys; import and export settings. Defaults include Ctrl+Z/Y, F (zoom to fit), spacebar (orientation), Ctrl+1–8 (views), Enter (repeat last command).

**Missing in OpenForge:** user-assignable shortcuts.

## Command search

Search box in the title bar finding commands by name, with "show command location" highlighting the toolbar button; also searches files and help.

## Heads-up view toolbar and reference triad

Transparent toolbar over the graphics area with zoom, previous view, section view, view orientation, display style, hide/show items, edit appearance, apply scene, view settings. Triad in the corner; clicking an axis orients normal to it.

## Selection

| Tool | Behaviour |
| --- | --- |
| Selection filters | Restrict picks to faces, edges, vertices, planes, sketch entities, dimensions, etc.; toggle with F5 |
| Box, cross and lasso selection | Left-to-right box selects enclosed; right-to-left crosses; lasso |
| Select other | Cycle through hidden entities under the cursor |
| Selection sets | Named saved selections |
| Select tangency, select loop, select partial loop | Edge chain helpers |
| SelectionManager | Picks regions, open or closed groups and chains of sketch entities for features like sweep and loft |

**Missing in OpenForge:** filters, box/lasso/cross selection, select other, selection sets, loop and tangency selection, SelectionManager-style contour picking (partly covered by explicit contour selection).

## Task pane

Side tabs: SOLIDWORKS Resources, Design Library, File Explorer, View Palette (drag drawing views), Appearances/Scenes/Decals, Custom Properties, and add-in panes.

## Touch mode and multi-touch

Larger UI, touch gestures for orbit, pan and zoom, sketch ink, and touch-friendly radial menus.

## Welcome dialog

Start screen with new document buttons, recent files, recent folders, learning resources and alerts.

## Units and standards

Document unit systems: MKS (m, kg, s), CGS (cm, g, s), MMGS (mm, g, s), IPS (in, lb, s), and custom, each with length, angle, mass, force and time units, decimals or fractions, and dual dimensions. Drafting standard per document.

**OpenForge today:** display units mm, cm, m, in and ft with set decimals; core stays in mm and radians.
**Missing:** named unit systems with mass, force and time units, fractions, dual dimensions.

## Options

System options (apply to every document): general, drawings, colours, sketch, display, selection, performance, assemblies, external references, default templates, file locations, FeatureManager, spin box increments, view, backup/recover, touch, hole wizard/toolbox, file explorer, search, collaboration, messages.

Document properties (saved per file): drafting standard, annotations, dimensions, virtual sharps, tables, DimXpert, detailing, grid/snap, units, model display, material properties, image quality, sheet metal, plane display, configurations.

**Missing in OpenForge:** a full options system.

## Themes

Light, medium light, medium dark and dark UI themes.

**OpenForge today:** dark only.
