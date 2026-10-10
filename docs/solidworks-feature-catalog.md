# SOLIDWORKS Feature Catalog

A complete inventory of SOLIDWORKS functionality, grouped by area, to scope OpenForge against.

- **Source:** the official SOLIDWORKS 2025 Design Help. I crawled four levels of its table of contents (about 3,000 topics across 25 areas), then added SOLIDWORKS 2026 additions and the separately licensed products.
- **What's counted:** user-facing commands and capabilities. Help pages that are only procedures ("To create…"), dialog-box references or examples are folded into the feature they describe.
- **As of:** 2026-10-10.

## How to read the status column

| Mark | Meaning |
| --- | --- |
| ✅ | In the OpenForge prototype today |
| ◐ | Partly in the prototype (limits noted) |
| (blank) | Not built yet |

## Contents

1. [Sketching (2D)](#1-sketching-2d)
2. [3D sketching, curves and splines](#2-3d-sketching-curves-and-splines)
3. [Part features](#3-part-features)
4. [Patterns and mirroring](#4-patterns-and-mirroring)
5. [Surfaces](#5-surfaces)
6. [Direct editing and body tools](#6-direct-editing-and-body-tools)
7. [Reference geometry](#7-reference-geometry)
8. [Multibody parts](#8-multibody-parts)
9. [Mesh and scan data](#9-mesh-and-scan-data)
10. [Sheet metal](#10-sheet-metal)
11. [Weldments and structure system](#11-weldments-and-structure-system)
12. [Mold design](#12-mold-design)
13. [Assemblies](#13-assemblies)
14. [Configurations, equations and design tables](#14-configurations-equations-and-design-tables)
15. [Drawings and detailing](#15-drawings-and-detailing)
16. [Tolerancing and MBD](#16-tolerancing-and-mbd)
17. [Evaluate and analysis tools](#17-evaluate-and-analysis-tools)
18. [Model display and rendering](#18-model-display-and-rendering)
19. [Import and export](#19-import-and-export)
20. [User interface and workflow](#20-user-interface-and-workflow)
21. [Fundamentals: documents, files and automation](#21-fundamentals-documents-files-and-automation)
22. [Built-in add-ins and Xpress tools](#22-built-in-add-ins-and-xpress-tools)
23. [Separately licensed products](#23-separately-licensed-products)
24. [New in SOLIDWORKS 2026](#24-new-in-solidworks-2026)
25. [Summary: prototype coverage](#25-summary-prototype-coverage)

---

## 1. Sketching (2D)

### Sketch entities

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Line | Straight line by endpoints | |
| Centerline / construction line | Reference geometry that doesn't form profiles | |
| Midpoint line | Line drawn symmetric about a start point | |
| Infinite line | Unbounded construction line | |
| Corner rectangle | Rectangle by two corners | ◐ Center-placed rectangle by width and height |
| Center rectangle | Rectangle from its center | ◐ Same as above |
| 3-point corner / center rectangle | Rotated rectangles | |
| Parallelogram | Four-sided parallel shape | |
| Circle (center) | Circle by center and radius | ◐ Typed center and radius only |
| Perimeter circle | Circle through three points | |
| Centerpoint arc | Arc by center, start, end | |
| Tangent arc | Arc tangent to a previous entity | |
| 3-point arc | Arc through three points | |
| Ellipse / partial ellipse | Full or partial ellipse | ◐ Full ellipse only |
| Parabola | Parabolic curve | |
| Conic | General conic with rho value | |
| Polygon | Regular polygon, inscribed or circumscribed | ◐ Inscribed, typed sides and radius |
| Straight slot | Slot by two points and width | ◐ Rounded-rectangle slot only |
| Centerpoint straight slot | Slot from its center | |
| 3-point arc slot / centerpoint arc slot | Curved slots | |
| Point | Sketch point | |
| Sketch text | Text as sketch geometry, extrudable | |
| Equation-driven curve | Curve from an explicit or parametric equation | |
| Spline (B-spline) | Free-form curve through points | |
| Style spline | Bézier-style spline by control vertices | |
| Fit spline | Spline fitted to existing entities | |
| Belt / chain | Belt around pulleys in layout sketches | |
| Sketch picture | Image placed on a sketch plane for tracing | |
| Rapid Sketch | Start sketching by clicking a face, no plane step | ◐ Click a face to set the sketch plane |

### Sketch tools

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Sketch fillet | Rounds a sketch corner | ◐ Rectangle corner radius only |
| Sketch chamfer | Bevels a sketch corner | |
| Offset entities | Offsets curves by a distance, with cap ends | |
| Offset on surface | Offsets edges along a 3D face | |
| Convert entities | Projects model edges into the sketch | |
| Intersection curve | Sketch curve where a plane meets geometry | |
| Silhouette entities | Projects a body's outline | |
| Face curves | Iso-parametric curves on a face | |
| Trim entities (power trim, corner, inside, outside, to closest) | Removes or extends segments | |
| Extend entities | Extends to the next entity | |
| Split entities | Splits an entity at a point | |
| Jog line | Adds a jog into a line | |
| Mirror entities / dynamic mirror | Mirrors sketch geometry about a line | |
| Linear sketch pattern | Repeats sketch entities in rows | ◐ Feature-level linear pattern |
| Circular sketch pattern | Repeats sketch entities around a point | ◐ Feature-level circular pattern about origin |
| Move / copy / rotate / scale / stretch entities | Transforms sketch geometry | |
| Modify sketch | Moves, rotates or scales a whole sketch | |
| Replace entity | Swaps one entity for another, keeping references | |
| Make path | Treats chained entities as one path | |
| Construction geometry toggle | Switches entities to construction | |
| Close sketch to model | Closes an open profile using model edges | |
| Repair sketch | Finds and fixes gaps and overlaps | |
| Check sketch for feature usage | Validates a sketch for a chosen feature type | |
| Shaded sketch contours | Shows closed regions; extrude by clicking inside | |
| Sketch ink / touch tools | Pen and touch input converted to geometry | |
| Blocks (make, insert, edit, explode, nested) | Reusable grouped sketch geometry | |
| Derived sketch | Sketch copy that stays linked to its parent | |
| Shared sketches | One sketch used by several features | |
| Numeric sketch input | Typing coordinates while drawing | ◐ All geometry is typed in fields |
| Autotransitioning | Switches line to arc while drawing | |
| Snaps / quick snaps / inferencing | Automatic snapping and alignment hints | |
| Sketch grid and align grid/origin | Grid display and moving the sketch origin | |

### Dimensions and relations

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Smart dimension | Linear, angular, radial, diameter dimensions | ◐ Fixed parameter fields, not placed dimensions |
| Horizontal / vertical dimension | Axis-aligned dimensions | |
| Path length dimension | Length of a chain of entities | |
| Arc length / arc conditions (min, max, center) | Dimensioning arcs | |
| Driven (reference) dimensions | Dimensions that report but don't drive | |
| Ordinate / baseline / chain dimensions in sketches | Dimension schemes | |
| Fully defined / under / over defined status | Color-coded sketch status | |
| Instant2D | Drag dimensions on screen to change them | |
| Lock dimensions | Prevent edits | |
| Relations: horizontal, vertical, collinear, coradial, perpendicular, parallel, tangent, concentric, midpoint, intersection, coincident, equal, symmetric, fix, pierce, merge | Geometric constraints between entities | |
| Equal curve length | Constrains curve lengths equal | |
| Equidistant points / segments | Divides an entity evenly | |
| Curvature continuity / torsion continuity | Smoothness constraints with splines | |
| Automatic relations | Relations added while sketching | |
| Display / delete relations | Inspect and remove constraints | |
| Sketch solver (constraint solving) | Solves geometry to satisfy constraints | |

## 2. 3D sketching, curves and splines

| Feature | What it does | OpenForge |
| --- | --- | --- |
| 3D sketch | Lines, splines and points in 3D space | |
| 3D sketch planes and space handles | Sketching on planes inside a 3D sketch | |
| 3D sketch dimensions and relations | Constraints in 3D | |
| Spline on surface | Spline constrained to a face | |
| Spline tools: tangency/curvature control, control polygon, simplify, inflection points, minimum radius, curvature combs | Spline editing and quality checks | |
| Split line | Splits faces by a sketch, silhouette or intersection | |
| Projected curve | Sketch projected onto faces or another sketch | |
| Composite curve | Joins curves into one | |
| Curve through XYZ points | Curve from a coordinate list or file | |
| Curve through reference points | Curve through existing points | |
| Helix and spiral | Helical and spiral curves for springs and threads | |

## 3. Part features

### Extrude and revolve

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Extruded boss/base | Adds material by extruding a sketch | ✅ |
| Extruded cut | Removes material by extruding a sketch | ✅ |
| End conditions: blind, through all, through all both, up to next, up to vertex, up to surface, offset from surface, up to body, mid plane | Controls how far an extrude goes | ◐ Blind, reversed, mid plane, through all |
| Direction 2 | A second extrude direction with its own end condition | |
| Draft while extruding | Tapers walls during the extrude | |
| Thin feature | Extrudes a sketch as a wall of set thickness | |
| Start from: sketch plane, surface/face/plane, vertex, offset | Where the extrude starts | ◐ Plane plus offset, or a picked flat face |
| Feature scope (multibody) | Chooses which bodies a feature affects | |
| Selected contours | Uses only some regions of a sketch | |
| Revolved boss/base | Adds material by revolving a profile | ◐ About Z axis only, from typed profile |
| Revolved cut | Removes material by revolving | ◐ Same limits |
| Revolve angle, two directions, mid plane, thin revolve | Revolve options | ◐ Angle only |

### Sweeps, lofts and boundaries

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Swept boss/base and swept cut | Moves a profile along a path | |
| Sweep options: guide curves, twist along path, profile orientation, circular profile, merge tangent faces | Controls the sweep shape | |
| Solid-body sweep cut | Sweeps a tool body along a path (machining simulation) | |
| Lofted boss/base and lofted cut | Blends between profiles | |
| Loft options: guide curves, centerline, start/end constraints, synchronization, close loft | Controls the loft shape | |
| Boundary boss/base and boundary cut | High-quality blend from curves in two directions | |

### Fillets and chamfers

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Constant size fillet | Rounds edges with one radius | ◐ Edge groups (all, vertical, top, bottom), not picked edges |
| Variable size fillet | Radius changes along an edge | |
| Face fillet | Blends between two face sets | |
| Full round fillet | Rounds across three faces | |
| Setback fillets, conic and curvature-continuous profiles | Advanced fillet shapes | |
| FilletXpert | Automatic fillet ordering and repair | |
| Chamfer: angle-distance, distance-distance, vertex, offset face, face-face | Bevels edges | ◐ Equal-distance on edge groups |
| Partial chamfer | Chamfer on part of an edge | |

### Holes and threads

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Simple hole | Plain drilled hole | ◐ Circular cut works as a hole |
| Hole Wizard | Standard holes: counterbore, countersink, tapped, pipe tap, legacy | |
| Advanced Hole | Stacked hole elements in one feature | |
| Hole series (assemblies) | Matching holes through several parts | |
| Thread | Modeled helical threads | |
| Cosmetic thread | Thread shown in display and drawings, not modeled | |
| Stud Wizard | Threaded studs on cylinders | |

### Other features

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Shell | Hollows a part, leaving faces open | ◐ One open face (top, bottom, front), uniform walls |
| Shell with multi-thickness faces | Different wall thickness per face | |
| Draft (neutral plane, parting line, step draft) | Tapers faces for molding | |
| DraftXpert | Automatic draft management | |
| Rib | Thin web from an open sketch | |
| Dome | Dome on a face | |
| Wrap | Wraps a sketch onto a face (emboss, deboss, scribe) | |
| Indent | Indents one body into another | |
| Flex (bend, twist, taper, stretch) | Global deformation of bodies | |
| Deform (point, curve to curve, surface push) | Freeform deformation | |
| Freeform | Push/pull control points on a face | |
| Scale | Scales a part about a point | |
| Fastening features: mounting boss, snap hook, snap hook groove, vent, lip/groove | Plastic part features | |
| 3D Texture | Turns an appearance texture into geometry | |
| Library features / Design Library | Reusable saved features | |
| FeatureWorks | Recognizes features in imported geometry | |
| FeatureXpert | Fixes failing fillets and drafts | |
| Feature freeze | Excludes features from rebuilds | |
| Missing reference ghosting | Shows lost references while editing | |
| Make Multibody Part | Converts an assembly into a linked multibody part | |

## 4. Patterns and mirroring

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Linear pattern | Repeats features in one or two directions | ◐ One direction, inside a single feature |
| Circular pattern | Repeats features around an axis | ◐ About the origin, inside a single feature |
| Mirror | Mirrors features, faces or bodies across a plane | |
| Curve driven pattern | Repeats along a curve | |
| Sketch driven pattern | Places instances at sketch points | |
| Table driven pattern | Places instances from an XY table | |
| Fill pattern | Fills a region with instances | |
| Variable pattern | Instances with changing dimensions | |
| Pattern options: skip instances, vary sketch, geometry pattern, propagate visual properties | Pattern control | |

## 5. Surfaces

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Extruded surface | Surface from an extruded sketch | |
| Revolved surface | Surface from a revolved sketch | |
| Swept surface | Surface along a path | |
| Lofted surface | Surface between profiles | |
| Boundary surface | Surface from curves in two directions | |
| Filled surface | Patches a boundary with set continuity | |
| Freeform surface | Push/pull face shaping | |
| Planar surface | Flat surface from a closed boundary | |
| Offset surface | Copies faces at an offset | |
| Ruled surface | Surface perpendicular or tapered to edges | |
| Radiate surface | Surface radiating from edges along a plane | |
| Surface from mesh | Fits surfaces to mesh data | |
| Knit surface | Joins surfaces, optionally into a solid | |
| Trim / untrim surface | Cuts surfaces with other geometry | |
| Extend surface | Extends surface edges | |
| Delete face / delete hole | Removes faces or patches holes | |
| Replace face | Swaps faces for surfaces | |
| Mid-surface | Surface midway between offset faces | |
| Thicken | Turns surfaces into a solid | |
| Cut with surface | Splits or cuts a solid with a surface | |
| Surface flatten | Flattens a doubly curved face into 2D | |
| Projected surface area | Measures area projected on a plane | |

## 6. Direct editing and body tools

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Move face (offset, translate, rotate) | Edits geometry directly without history | |
| Delete / keep body | Removes bodies from a part | |
| Move / copy body | Positions bodies with mates or transforms | |
| Combine (add, subtract, common) | Boolean operations between bodies | |
| Intersect | Builds solids from intersecting regions | |
| Split | Splits a part into bodies, optionally saved as files | |
| Save bodies / create assembly | Writes bodies out as parts and an assembly | |
| Insert part / derived part | Brings another part in as a linked body | |
| Mirror part (opposite hand) | Creates a mirrored part file | |
| Defeature | Simplifies a model for sharing | |
| Dependency editing / dynamic feature editing | Drag-edit features in the tree or graphics | |
| Instant3D | Drag handles to change feature sizes | |
| Body compare | Highlights differences between bodies | |
| Slicing tool | Creates section sketches through bodies | |

## 7. Reference geometry

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Default planes and origin | Front, Top, Right planes and origin | ✅ Top (XY), Front (XZ), Right (YZ) |
| Reference plane (offset, angle, through points, normal to curve, mid plane, tangent) | Construction planes | ◐ Plane offset in each extrude |
| Reference axis | Construction axes | |
| Temporary axes | Automatic axes through cylinders | |
| Coordinate system | User coordinate systems | |
| Reference point | Construction points, including numeric XYZ | |
| Center of mass point | Marks the center of mass | |
| Bounding box | Smallest box around a part, with properties | ◐ Size shown in status bar and mass properties |
| Mate reference | Predefined mate geometry for reuse | |
| Grid system | Building-style grid for structures | |

## 8. Multibody parts

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Multiple solid bodies in one part | Separate bodies in one file | |
| Solid Bodies and Surface Bodies folders | Organizing bodies in the tree | |
| Feature scope | Which bodies a feature affects | |
| Hide / show bodies | Visibility per body | |
| Exploded views of multibody parts | Exploded body layout | |
| Interference detection between bodies | Overlap checks | |
| Modeling techniques: bridging, local operations, symmetry, tool bodies | Multibody workflows | |

## 9. Mesh and scan data

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Import mesh as graphics body, solid or surface | STL, OBJ, 3MF, PLY, VRML import | |
| Mesh BREP and hybrid bodies | Using mesh bodies in features | |
| Convert to mesh body | Solid or surface to mesh | |
| Decimate mesh | Reduce triangle count | |
| Segment imported mesh | Group facets into faces | |
| Paint select facets | Select mesh regions | |
| ScanTo3D (mesh prep and surface wizards) | Scan data to solid model | |

## 10. Sheet metal

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Base flange / tab | First sheet metal body or a flat tab | |
| Edge flange | Bent wall along an edge | |
| Miter flange | Flanges along several edges with mitered corners | |
| Hem | Folded edge (closed, open, teardrop, rolled) | |
| Jog | Z-shaped offset bend | |
| Sketched bend | Bend along a sketched line | |
| Lofted bend | Transition between two profiles | |
| Swept flange | Flange along a path | |
| Cross break | Stiffening cross pattern | |
| Insert bends / convert to sheet metal | Turns a solid into sheet metal | |
| Rip | Gap along an edge so a part can unfold | |
| No bends / unfold / fold | Bend state editing | |
| Flatten / flat pattern | Flat blank for cutting | |
| Flat pattern configurations and DXF/DWG export | Output for laser or punch | |
| Normal cut | Cuts square to the sheet | |
| Corner relief, closed corner, welded corner, break corner / corner trim | Corner treatments | |
| Bend notch | Notches along bend lines | |
| Tab and slot | Interlocking tabs for welding fixtures | |
| Sheet metal gusset | Stiffening gussets across bends | |
| Forming tools and stamp tool | Louvers, lances, embosses | |
| Gauge tables and bend tables | Material thickness and bend data | |
| K-factor, bend allowance, bend deduction | Bend calculations | |
| Bend lines and bend notes in drawings | Flat pattern documentation | |
| Multibody sheet metal and merging bodies | Several sheet metal bodies in one part | |
| Sheet metal cut list and bounding box | Blank size and properties | |

## 11. Weldments and structure system

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Weldment feature | Turns a part into a weldment | |
| Structural member | Profiles along sketch segments | |
| Weldment profiles library and custom profiles | Standard and custom cross sections | |
| Trim / extend | Trims members to each other | |
| Corner treatments (end miter, butt, trim order) | How members meet | |
| End cap | Caps open tube ends | |
| Gusset | Triangular plate between members | |
| Weld bead / fillet weld bead / groove bead | Weld geometry | |
| Smart weld selection | Picks weld paths automatically | |
| Cut lists, cut list properties and folders | Member lengths and properties | |
| Sub-weldments and groups | Organizing members | |
| Structure System: primary and secondary members, corner management, connection elements | Frame design mode | |
| Weldment drawings and cut list tables | Fabrication documentation | |

## 12. Mold design

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Draft analysis | Checks faces for draft | |
| Undercut analysis | Finds trapped areas | |
| Parting line analysis and parting line | Where the mold splits | |
| Shut-off surfaces | Closes through holes | |
| Parting surface | Surface that splits core and cavity | |
| Interlock surface | Tapered interlock around the parting line | |
| Tooling split | Creates core and cavity bodies | |
| Core | Side cores and lifters | |
| Cavity | Mold cavity from a part in a block | |
| Scale (shrinkage factor) | Accounts for material shrink | |
| Mold folders | Organizes mold surfaces | |

## 13. Assemblies

### Components

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Insert components | Adds parts and subassemblies | |
| Create assembly from part | New assembly from the open part | |
| Move / rotate components (free drag, along entity, by delta, to XYZ) | Positions parts | |
| Collision detection, physical dynamics, dynamic clearance | Contact-aware moving | |
| Fix / float | Locks a component in place | |
| Replace components | Swaps a part for another | |
| Mirror components and opposite-hand versions | Mirrored instances | |
| Component patterns: linear, circular, pattern driven, sketch driven, curve driven, chain | Repeats components | |
| Copy with mates | Duplicates with mates reapplied | |
| Selection tools: volume select, select by size, select identical, advanced select, isolate | Finding components | |
| Component properties and references | Per-instance settings | |
| Subassemblies: form, dissolve, flexible, reorganize | Assembly structure | |
| Virtual components | Parts saved inside the assembly | |
| Smart components | Components that bring features and parts with them | |
| Envelopes | Reference-only components | |
| Joined parts | One part from several components | |
| Flexible components | Part geometry driven by the assembly | |

### Mates

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Standard mates: coincident, parallel, perpendicular, tangent, concentric, lock, distance, angle | Basic positioning constraints | |
| Advanced mates: profile center, symmetric, width, path, linear/linear coupler, distance and angle limits | Advanced constraints | |
| Mechanical mates: cam, slot, hinge, gear, rack and pinion, screw, universal joint | Motion relationships | |
| SmartMates (peg-in-hole, flange, Alt-drag) and AI fastener recognition | Automatic mates by dragging | |
| Quick mates context toolbar | Fast mate creation | |
| Mate controller | Saved positions and animations | |
| Mate folders, mate errors, MateXpert, replace mate entities | Managing and fixing mates | |
| Magnetic mates and connection points | Fast layout of equipment | |

### Assembly tools

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Interference detection | Finds overlaps between parts | |
| Clearance verification | Checks minimum gaps | |
| Hole alignment | Finds misaligned holes | |
| Exploded views (regular, radial, smart explode lines) | Exploded layouts | |
| Assembly features (cuts, holes, fillets, weld beads) | Features applied at assembly level | |
| Smart fasteners | Auto-adds bolts to holes | |
| Top-down design: in-context parts, layout sketches, external references | Designing parts inside the assembly | |
| Assembly visualization | Sorts and colors components by property | |
| Bill of materials in the assembly | BOM without a drawing | |
| Save assembly as part | Exports a single part | |
| Large assembly mode, lightweight, SpeedPak, Large Design Review, defeature | Performance tools | |
| Display states | Per-state visibility and appearance | |
| Facility layout tools | Ground plane, asset publisher | |
| Motion studies (animation, basic motion) | See section 22 | |

## 14. Configurations, equations and design tables

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Configurations (manual) | Variants of a part or assembly in one file | |
| Derived configurations | Child variants | |
| Configurable items: dimensions, suppression, materials, properties, colors, component configs | What can vary | |
| Modify Configurations dialog | Table editing of configured values | |
| Design tables (Excel) | Variants driven from a spreadsheet | |
| Configuration Publisher | Custom picker UI for variants | |
| Equations | Dimensions driven by formulas | |
| Global variables | Named values used across features | |
| Linked values | Dimensions tied together | |
| Equations in external files | Shared formulas across models | |
| Custom and configuration properties | Metadata such as part number and description | |
| Property Tab Builder | Custom property entry forms | |

## 15. Drawings and detailing

### Sheets and views

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Drawing sheets, sheet formats, title blocks | Paper layout and borders | |
| Standard 3 view | Front, top, right views at once | |
| Model view and named views | Any saved orientation | |
| Projected view | Views projected from a parent | |
| Auxiliary view | View normal to an inclined edge | |
| Section view (full, half, offset, aligned, partial) | Cut-through views with hatching | |
| Detail view | Enlarged region | |
| Broken-out section | Local section into a view | |
| Break view | Shortens long parts | |
| Crop view | Trims a view to a sketch | |
| Alternate position view | Overlays an assembly in a second position | |
| Exploded views in drawings | Assembly instructions | |
| Relative to model view | Orientation by two faces | |
| 3D drawing view mode | Rotates a view temporarily | |
| First and third angle projection | Projection standards | |
| Hidden lines visible / removed, tangent edge display | Line display | |
| Layers, line formats | Drawing styles | |
| Auto-generate drawings (beta) | Automatic views and dimensions | |
| DrawCompare | Compares two drawings | |

### Annotations and dimensions

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Model items (import dimensions from the model) | Reuses part dimensions | |
| Smart dimensions in drawings | Adds driven dimensions | |
| Ordinate, baseline, chain, chamfer, angular running dimensions | Dimension schemes | |
| Rapid dimension and autodimension | Fast dimensioning | |
| Tolerance and precision | Limits, fits, bilateral tolerances | |
| Notes, flag notes, balloons, auto balloons, stacked balloons | Text callouts | |
| Hole callouts | Automatic hole descriptions | |
| Center marks and centerlines | Feature centers | |
| Surface finish, weld, datum, datum target, geometric tolerance symbols | Standard symbols | |
| Cosmetic threads in drawings | Thread representation | |
| Area hatch / fill | Hatching regions | |
| Revision clouds and revision symbols | Change marking | |
| Blocks in drawings | Reusable drawing content | |
| Format painter, styles, drafting standards | Consistent formatting | |
| Spell check, find and replace | Text tools | |

### Tables

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Bill of materials (table-based and Excel-based) | Parts lists, indented or flat | |
| Hole table | Hole positions and sizes | |
| Revision table | Change history | |
| Bend table and punch table | Sheet metal documentation | |
| Weld table and weldment cut list table | Weldment documentation | |
| General table, general tolerance table, title block table | Custom tables | |
| Design table and family table in drawings | Variant tables | |
| Table equations | Calculations in tables | |

### Output

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Print and print to PDF | Paper and PDF output | |
| Export DXF/DWG | 2D CAD exchange | |
| eDrawings and markups | Lightweight sharing and review | |
| Detailing mode and lightweight drawings | Fast opening of large drawings | |

## 16. Tolerancing and MBD

| Feature | What it does | OpenForge |
| --- | --- | --- |
| DimXpert for parts | Tolerance-ready dimensions on the 3D model | |
| Auto dimension scheme | Automatic GD&T scheme | |
| DimXpert size, location, angle, basic dimensions | Manual tolerancing | |
| Datums and geometric tolerances | GD&T on the model | |
| Show tolerance status | Shows under- and fully-toleranced features | |
| DimXpert in assemblies and auto pair tolerance | Assembly tolerancing | |
| TolAnalyst | Tolerance stack-up analysis | |
| 3D views (MBD) | Saved annotated views | |
| Annotation views | Orientations for 3D annotations | |
| Publish to 3D PDF with templates | Drawing-free documentation | |
| Publish to STEP 242 with PMI | Model-based definition export | |
| 3D PMI compare | Compares annotations between versions | |
| Import PMI | Reads PMI from other CAD files | |

## 17. Evaluate and analysis tools

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Measure | Distance, angle, radius, area between entities | |
| Mass properties | Mass, volume, area, center of mass, inertia | ◐ Mass, volume, area, bounding box; no center of mass or inertia |
| Override mass properties | Manual mass values | |
| Section properties | Area properties of a planar section | |
| Materials library and custom materials | Physical properties per material | ◐ Nine built-in materials, density only |
| Sensors | Alerts when a measured value changes | |
| Performance evaluation | Rebuild time per feature | ◐ Total rebuild time shown |
| Check entity / check geometry | Finds invalid geometry | |
| Geometry analysis | Finds small faces, sharp edges, gaps | |
| Import diagnostics | Repairs imported geometry | |
| Deviation analysis | Angle between faces along edges | |
| Zebra stripes, curvature display, curvature combs | Surface quality visualization | |
| Draft and undercut analysis | Moldability checks | |
| Thickness analysis | Thin and thick regions | |
| Symmetry check | Checks a model for symmetry | |
| Compare documents, features, geometry, BOMs | Version comparison | |
| Statistics (feature, assembly, drawing) | Model statistics | |

## 18. Model display and rendering

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Shaded, shaded with edges, wireframe, hidden lines visible/removed | Display styles | ✅ Shaded, shaded with edges, wireframe, x-ray |
| Standard views (front, back, top, bottom, left, right, isometric, dimetric, trimetric, normal to) | View orientations | ◐ Front, top, right, isometric |
| View selector cube and orientation dialog | Picking views | ◐ Axis triad only |
| Zoom to fit, zoom to area, zoom to selection, pan, rotate, roll | View manipulation | ◐ Orbit, pan, zoom, fit |
| Perspective and cameras | Camera control | ◐ Perspective only |
| Section view (models) | Live cutaway | |
| Model break view | Shortened display of long models | |
| Viewports (two and four views) | Multiple simultaneous views | |
| Hide / show types (planes, axes, sketches, origins) | Display filters | ◐ Planes toggle |
| Appearances (colors, textures, illumination, mapping) | Visual materials | |
| Decals | Images applied to faces | |
| Scenes and HDRI backgrounds | Environment lighting | |
| Lights (ambient, directional, point, spot, sunlight) | Lighting | |
| RealView, shadows, ambient occlusion | Realistic display | |
| Cartoon rendering | Stylized display | |
| Snapshots | Saved views | |
| Walk-through | First-person navigation | |
| Solar access studies | Sun shading studies | |
| Image capture and screen recording | Screenshots and video | |
| SOLIDWORKS Visualize export | Photorealistic rendering | |

## 19. Import and export

| Format | Direction | OpenForge |
| --- | --- | --- |
| STEP (AP203, AP214, AP242) | Import and export | ◐ Import and export, no configuration data or PMI |
| IGES | Import and export | |
| Parasolid (x_t, x_b) | Import and export | |
| ACIS (sat) | Import and export | |
| STL | Import and export | ◐ Export only |
| 3MF and AMF | Import and export | |
| OBJ, PLY, VRML | Import and export | |
| glTF / GLB | Import and export | |
| DXF / DWG (2D and 3D) | Import and export | |
| DXF/DWG import wizard and 2D-to-3D conversion | Turns 2D drawings into sketches and parts | |
| PDF and 3D PDF | Export | |
| eDrawings | Export | |
| 3D XML, HCG, HOOPS, SMG | Export | |
| IFC | Import and export | |
| JT | Import | |
| Native third-party CAD via 3D Interconnect: CATIA V5, Creo/Pro-E, NX, Inventor, Solid Edge, Rhino | Import, linked to the source file | |
| Images: JPEG, PNG, TIFF, PSD, AI | Export | |
| Export to AEC | Simplified models for building software | |
| Save as previous version | Older SOLIDWORKS versions | |

## 20. User interface and workflow

| Feature | What it does | OpenForge |
| --- | --- | --- |
| CommandManager (tabbed toolbar) | Main command ribbon | ✅ Features, Evaluate, Import/Export tabs |
| FeatureManager design tree | Feature history tree | ✅ |
| Rollback bar | Edit the model as of an earlier feature | |
| Reorder features by dragging | Changes build order | ✅ |
| Suppress / unsuppress | Turns features off | ✅ |
| Rename features | Custom feature names | ✅ |
| Feature folders, favorites, tags, comments, attachments | Tree organization | |
| Design Journal and Design Binder | Notes stored in the model | |
| Parent/child relationships | Shows feature dependencies | |
| What's Wrong? | Lists rebuild errors | ◐ Errors shown on features and in the editor |
| PropertyManager | Feature options panel with OK and Cancel | ✅ |
| Live feature preview | Preview while editing | ✅ Live rebuild and sketch outline |
| Undo / redo | Reverses changes | ✅ |
| Context toolbars and shortcut menus | Right-click and selection menus | ◐ Right-click menu in the tree |
| Shortcut bar (S key) and mouse gestures | Fast command access | |
| Keyboard shortcut customization | User shortcuts | ◐ Fixed shortcuts |
| Command search | Find commands by name | |
| Heads-up view toolbar | View controls over the graphics | ✅ |
| Reference triad | Orientation indicator | ✅ |
| Selection filters, box/lasso/cross selection, select other, selection sets | Selection tools | |
| SelectionManager | Picking contours and regions | |
| Task pane: Design Library, File Explorer, View Palette, Appearances, Custom Properties | Side panels | |
| Touch mode and multi-touch | Tablet input | |
| Welcome dialog | Start screen | ✅ Welcome screen with sample part |
| Units and dimension standards (MMGS, IPS, and others) | Unit systems | ◐ Millimeters only |
| Options (system options and document properties) | Settings | |
| Dark and light themes | Appearance | ◐ Dark only |

## 21. Fundamentals: documents, files and automation

| Feature | What it does | OpenForge |
| --- | --- | --- |
| Part, assembly and drawing documents | Three document types | ◐ Parts only (.ofp) |
| New, open, save, save as, save all | File handling | ✅ New, open, save, save as |
| Auto-recover and backup | Crash protection | |
| Unsaved-changes prompt | Prevents data loss | ✅ |
| Document templates | Starting settings | |
| Recent documents | Quick reopening | |
| Pack and Go | Collects all referenced files | |
| Referenced files and external references | Managing file links | |
| Print3D | Sends a model to a 3D printer or slicer | ◐ STL export |
| Macros (record and play) | Automation | |
| SOLIDWORKS API | Programmable interface | |
| Task Scheduler | Batch conversion, printing and updates | |
| Search (files, commands, help) | Finding things | |
| Design Clipart | Reuses geometry from existing files | |
| OLE objects | Embedding other documents | |
| Multi-user environment | Read-only and reload handling | |
| 3DEXPERIENCE platform integration | Cloud data management | |

## 22. Built-in add-ins and Xpress tools

| Tool | What it does | OpenForge |
| --- | --- | --- |
| SimulationXpress | Basic linear stress analysis on a part | |
| FloXpress | Basic fluid flow through a part | |
| DFMXpress | Manufacturability checks | |
| DriveWorksXpress | Rules-based design automation | |
| SustainabilityXpress / Sustainability | Environmental impact estimates | |
| Costing | Manufacturing cost estimates for machined, sheet metal, weldment, cast, molded and 3D printed parts | |
| Design Checker | Checks files against company standards | |
| Motion studies: animation, basic motion | Animating assemblies | |
| Design studies | Parameter sweeps and optimization | |
| Toolbox | Standard hardware library (bolts, nuts, bearings, gears) | |
| CircuitWorks | Imports PCB data | |
| Routing | Pipes, tubes, electrical cables and harnesses | |
| SOLIDWORKS Utilities: compare, find/modify, geometry analysis, power select, simplify, symmetry check, thickness analysis | Model utilities | |
| SLDXML data exchange | XML-based data exchange | |
| ScanTo3D | Scan data conversion | |

## 23. Separately licensed products

These are separate products in the SOLIDWORKS family, listed for completeness. They are likely out of scope for OpenForge's core.

| Product | What it does |
| --- | --- |
| SOLIDWORKS Simulation (Standard, Professional, Premium) | FEA: static, thermal, frequency, buckling, fatigue, drop test, nonlinear, dynamics |
| SOLIDWORKS Motion | Full rigid-body motion analysis |
| SOLIDWORKS Flow Simulation | CFD for fluid flow and heat transfer |
| SOLIDWORKS Plastics | Injection molding simulation |
| SOLIDWORKS Electrical (Schematic and 3D) | Electrical schematics and 3D wiring |
| SOLIDWORKS PDM (Standard, Professional) | Product data management and vaulting |
| SOLIDWORKS Manage | Project and process management |
| SOLIDWORKS CAM / CAMWorks | CNC toolpaths |
| SOLIDWORKS Inspection | Inspection reports and ballooning |
| SOLIDWORKS Composer | Technical illustrations and animations |
| SOLIDWORKS Visualize | Photorealistic rendering |
| SOLIDWORKS MBD | Model-based definition (see section 16) |
| eDrawings Professional | Viewing and markup |
| DraftSight | 2D CAD |

## 24. New in SOLIDWORKS 2026

| Feature | What it does |
| --- | --- |
| Escape to cancel slow operations | Cancels long fillet, chamfer and pattern calculations |
| Reference points by numeric XYZ | Places points without existing geometry |
| Bounding box by coordinate system | Aligns the box to a chosen coordinate system |
| Fixed bounding box directions | Keeps length, width and thickness from swapping |
| Select bodies by size | Picks bodies by percent of part size, then deletes or keeps them |
| Base flange "From" option | Starts a base flange from an offset, vertex or face |
| Break corner "Internal corners only" | Limits corner breaks to internal corners |
| Structure system corner treatment UI | Color-coded corner types and trim order control |
| Cut list to custom property link | Links part properties to cut list properties |

SOLIDWORKS 2026 also adds AI-assisted features across design, PDM, simulation and electrical. Only the items above are confirmed from a named source.

## 25. Summary: prototype coverage

| Area | Prototype status |
| --- | --- |
| Sketching | ◐ Parametric shapes only; no free sketching, dimensions or relations |
| Part features | ◐ Extrude, cut, revolve, fillet, chamfer, shell (basic options) |
| Patterns | ◐ Linear and circular inside a feature |
| Surfaces, sheet metal, weldments, mold | Not started |
| Assemblies | Not started |
| Configurations and equations | Not started |
| Drawings | Not started |
| Evaluate | ◐ Basic mass properties and materials |
| Display | ◐ Display styles, standard views, orbit and zoom |
| Import/export | ◐ STEP in and out, STL out |
| UI and workflow | ✅ Feature tree, PropertyManager, undo/redo, save/open |

The biggest gaps for real-world use are a constraint-based 2D sketcher, picking edges and faces directly, reference planes and axes, sweeps and lofts, assemblies with mates, and 2D drawings.

## Sources

- [SOLIDWORKS 2025 Design Help: Features](https://help.solidworks.com/2025/english/SolidWorks/sldworks/c_Features_Top.htm), plus its full table of contents (crawled 2026-10-10)
- [GoEngineer: SOLIDWORKS 2026 Parts and Features, What's New](https://www.goengineer.com/en-us/blog/solidworks-2026-parts-and-features-whats-new)
- [Javelin: SOLIDWORKS Sketch Relations Summary](https://www.javelin-tech.com/blog/2014/01/solidworks-sketch-relations-summary/)
- [Whole-Spec: SolidWorks Sheet Metal, Weldments, Surface, and Toolbox](https://whole-spec.com/en/cad-tips/solidworks/sheetmetal-weldments-surface/)
- [SOLIDWORKS 2026 Top Enhancements](https://www.solidworks.com/product/whats-new/solidworks-2026-top-enhancements.md)
