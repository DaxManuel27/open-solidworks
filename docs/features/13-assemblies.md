# 13. Assemblies

Detailed specs for [catalog section 13](../solidworks-feature-catalog.md#13-assemblies). See the [README](README.md) for sources and conventions.

An assembly (`.sldasm`) places instances of parts and subassemblies (components) and positions them with mates. Each component references its file and a configuration, and has its own transform, state and properties.

**OpenForge today:** Phase 3 delivers components, subassemblies (rigid and flexible), every standard, advanced and mechanical mate listed here, move/rotate with collision detection, component patterns (all but chain), mirror components with opposite hands, interference and clearance checks, exploded views, an assembly BOM and a standard hardware library. Supported modes and limits are in [Phase 3 status](../phase-3-status.md); SmartMates, top-down design, smart components, envelopes and the large-assembly tools are deferred.

## Components

### Insert components

Options: browse for part or assembly files, or pick from open documents; place by clicking (the first component is fixed at the origin by default); keep visible to insert several copies; start command when creating a new assembly; graphics preview; make virtual; envelope; choose configuration and display state; show rotate context toolbar to rotate before placing.

### Create assembly from part

Makes a new assembly with the open part fixed at the origin.

### Move and rotate components

| Mode | Behaviour |
| --- | --- |
| Free drag | Moves within the remaining degrees of freedom |
| Along assembly XYZ | Constrained to an axis |
| Along entity | Along a selected edge or axis |
| By delta XYZ | Typed offsets |
| To XYZ position | Puts a picked point at typed coordinates |
| Rotate: free drag, about entity, by delta XYZ | Rotation equivalents |
| Triad | Drag the arrows and rings of a triad on the component |

Options while moving: standard drag, collision detection (stop at collision, dragged part only, highlight faces, sound, ignore complex surfaces), physical dynamics (push components by contact, with sensitivity), dynamic clearance (shows the gap to a chosen component and stops at a minimum).

### Fix and float

A fixed component has no degrees of freedom and can't be moved; a floating one moves within its mates. The first component inserted is fixed. The tree shows (f) for fixed and (-) for under defined.

### Replace components

Swaps a component for another file, keeping its position. Options: all instances or selected ones, configuration (match name or manually select), re-attach mates (with a mate entity mapping dialog for broken references).

### Mirror components and opposite-hand versions

API: `IMirrorComponentFeatureData`.

| Option | Values |
| --- | --- |
| Mirror plane | Plane or planar face |
| Components | Each either an instance copy (re-oriented) or an opposite-hand version (new file) |
| Position | Center of bounding box, center of mass, or component origin (`swMirrorComponentMirrorType_e`) |
| Orientation of copies | Mirrored/flipped X and Y combinations (`swMirrorComponentOrientation2_e`), align to component origin or to a selection |
| Opposite-hand files | Name prefix, suffix or custom; folder; create derived configurations; break link; transfer options (custom properties, DimXpert, cosmetic threads, etc.); preserve Z axis |
| Synchronise | Flexible subassembly movement and seed configuration |

### Component patterns

| Pattern | Notes |
| --- | --- |
| Linear | Like the part linear pattern (`ILocalLinearPatternFeatureData`), plus rotate instances about an axis, align to seed, synchronise flexible components, use seed configuration, modify individual instances |
| Circular | About an axis, with direction 2 and the same component options |
| Pattern driven | Follows a pattern feature in a part (`IDerivedPatternFeatureData`); seed position can be any instance |
| Sketch driven | One instance per sketch point |
| Curve driven | Along a curve, tangent or aligned to seed |
| Chain | Along a path like chain links (`IChainPatternFeatureData`); distance, distance linkage or connected linkage; static or dynamic |

### Copy with mates

Copies components and re-creates their mates, asking for new mate references for each. Options: repeat a mate's reference, skip a mate, edit distances and angles.

### Selection tools

Volume select (box select by containment), select by size (smallest percentage), select identical, select hidden, select suppressed, select toolbox, select internal components, advanced select (query by property, mass, file name, etc., saved as searches), isolate (hide all except the selection, with save-as display state).

### Component properties

Per instance: instance name and ID, full file name, configuration and display state (or "use named configuration"), suppression state (suppressed, resolved, lightweight), solve as rigid or flexible, exclude from BOM, hide, envelope, referenced display state, component reference (a text label).

### Subassemblies

Form new subassembly from selected components (keeps mates where possible), dissolve subassembly, insert new subassembly, rigid vs flexible (flexible lets its parts move under top-level mates), reorganise by drag and drop in the tree.

### Virtual components

Parts or subassemblies stored inside the assembly file rather than as separate files. Names appear in brackets. They can be saved externally later. Created by default by Insert New Part.

### Smart components

A part bundled with associated components and features. Inserting it brings the extra components and cuts the features (holes, cuts) into the mated part. Defined with Make Smart Component (components, features, references); auto-size by a cylindrical face diameter range in a table.

### Envelopes

Reference-only components excluded from BOMs and mass properties, used to select components inside them ("select using envelope": inside, outside, crossing).

### Joined parts

Join creates one body inside a part from several assembly components. API: `IJoinFeatureData` (`JoinedParts`, `ForceSurfaceContact`, `HideParts`).

### Flexible components

A part whose geometry is driven by references in the assembly (2020 and later): external references are redirected to geometry in the assembly so the same part file shows different shapes in different places.

## Mates

A mate constrains a pair of entities on two components. API: `IMateFeatureData` and per-type interfaces; types in `swMateType_e`.

### Shared mate options

| Option | Values |
| --- | --- |
| Alignment | Aligned, anti-aligned, closest (`swMateAlign_e`) |
| Flip dimension | For distance and angle mates, puts entities on the other side |
| Mates folder | Mates are listed in a Mates folder, by component in the tree, and can be grouped in sub-folders |
| Use for positioning only | Positions without adding a mate |
| Add to new folder, make first selection transparent, show pop-up dialog | UI options |

### Standard mates

| Mate | Entities | Options |
| --- | --- | --- |
| Coincident | Faces, edges, planes, vertices, points, axes | Alignment |
| Parallel | Planar faces, planes, lines, axes | Alignment |
| Perpendicular | Planar faces, planes, lines, axes | |
| Tangent | Cylinder, cone, sphere or curved face with another face | Alignment |
| Concentric | Cylinders, cones, circles, axes, points | Lock rotation; alignment |
| Lock | Two components | Keeps their relative position fixed |
| Distance | Faces, planes, lines, points | Distance; flip; limit min/max |
| Angle | Planar faces, planes, lines | Angle; flip; direction reference; limit min/max |

### Advanced mates

| Mate | Behaviour and options |
| --- | --- |
| Profile center | Centres a rectangular or circular profile on another; offset distance; lock rotation |
| Symmetric | Two entities symmetric about a plane |
| Width | Centres a tab between two width faces; constraint centered, free, dimension, or percent (`swMateWidthOptions_e`) |
| Path | A point follows a path; pitch/yaw control (follow path, free), roll control (up vector), path constraint (free, distance along path, percent along path) |
| Linear/linear coupler | Two components translate in a ratio; reverse |
| Distance limit, angle limit | Distance or angle with min and max, letting the component move within the range |

### Mechanical mates

| Mate | Behaviour and options |
| --- | --- |
| Cam | A cylinder, plane or point follows a tangent face loop (cam profile) |
| Slot | An axis or cylinder moves in a slot; constraint free, center in slot, distance along slot, percent along slot |
| Hinge | Concentric plus coincident in one mate; optional angle limits |
| Gear | Two components rotate in a ratio (numerator/denominator); reverse |
| Rack and pinion | Linear travel of the rack per revolution of the pinion, or pinion pitch diameter; reverse |
| Screw | Revolutions per distance or distance per revolution; reverse |
| Universal joint | Rotation of one shaft drives another about its axis; optional joint point |

### SmartMates

Alt-drag a component onto another to mate automatically: peg in hole (concentric and coincident), flange (circular edge to circular edge), edge to edge, face to face. A tab cycles alignment. Fastener recognition (2025 and later, AI-assisted) suggests mates for hardware.

### Quick mates context toolbar

Selecting two entities shows a pop-up with the mates that fit those entities, with flip and lock-rotation buttons.

### Mate controller

Saves named positions of a set of mates (distances, angles), animates between them, and can generate configurations from positions. API: `IMateControllerFeatureData`.

### Mate folders, errors, MateXpert and replace mate entities

- Errors show as red icons; over defining mates show the conflicting set.
- MateXpert diagnoses the minimum set of conflicting mates and suggests fixes.
- Replace mate entities swaps an entity in many mates at once, for when geometry changes.
- Mate diagnostics also offer "analyse" and "suppress problem mates".

### Magnetic mates and connection points

Asset publisher (in the part) defines ground planes and connection points with directions; inserting assets snaps connection points together magnetically. API: `IConnectionPointFeatureData`, `IGroundPlaneFeatureData`.

## Assembly tools

### Interference detection

Finds overlapping volumes between components. Options: selected components or whole assembly; treat coincidence as interference; show ignored interferences; treat subassembly as components; include multibody part interferences; make interfering parts transparent; create fasteners folder; create matching cosmetic thread folder; ignore hidden bodies; non-interfering components display (wireframe, hidden, transparent, current). Results list each interference with its volume; each can be ignored.

### Clearance verification

Checks minimum gaps between selected components or faces against a value; lists all gaps below it.

### Hole alignment

Finds holes across mated components whose centres don't line up within a deviation; lists each misalignment.

### Exploded views

Explode steps move components along a triad direction by a distance, or rotate them. Options: regular step, radial step (components arranged around an axis), auto-space components on drag, select subassembly parts, re-use subassembly explode, smart explode lines (automatic routing) or manual explode line sketches, animate collapse and explode. Stored per configuration.

### Assembly features

Cuts (extrude, revolve, sweep), holes (simple, Hole Wizard), fillets and chamfers, weld beads and belt/chain applied in the assembly. Feature scope picks the affected components; propagate feature to parts optionally pushes them down into the part files.

### Smart fasteners

Adds Toolbox bolts and screws to Hole Wizard holes and hole series automatically, with a stack of washers and nuts, length matching the stack, and grouped edits.

### Top-down design

- **In-context parts:** create or edit a part inside the assembly, referencing other components (external references, shown with ->).
- **Layout sketches:** a master sketch in the assembly that drives part features.
- **External references:** list, lock, break; out-of-context indicator (->?); reference visibility.
- Options to prevent creating external references and to set default templates for new parts.

### Assembly visualization

Lists components sorted by any property (mass, density, file name, custom property), with a colour gradient in the graphics area by value.

### BOM in the assembly

A table-based BOM in the assembly without a drawing, with the same options as drawing BOMs (see [drawings](15-drawings-and-detailing.md)).

### Save assembly as part

Saves an assembly as a multibody or single-body part. Options: exterior faces, exterior components or all components; preserve geometry references; include surface bodies.

### Large assembly tools

- **Large Assembly Mode:** turns off performance-heavy display options above a component threshold.
- **Lightweight components:** load only graphics until needed.
- **SpeedPak:** a simplified configuration of a subassembly with chosen faces and bodies kept.
- **Large Design Review:** opens huge assemblies for viewing, measuring and sectioning without loading models.
- **Defeature:** see [body tools](06-direct-editing-and-body-tools.md#defeature).

### Display states

Named sets of visibility, display mode, appearance and transparency per component, independent of configurations. Options: link display states to configurations.

### Facility layout tools

Asset publisher (ground plane, connection points), magnetic mates, and the Facility Layout command manager for factory layouts.

### Motion studies

See [built-in add-ins](22-built-in-add-ins-and-xpress-tools.md).
