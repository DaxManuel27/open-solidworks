# 22. Built-in add-ins and Xpress tools

Detailed specs for [catalog section 22](../solidworks-feature-catalog.md#22-built-in-add-ins-and-xpress-tools). See the [README](README.md) for sources and conventions.

**OpenForge today:** none of these.

## SimulationXpress

A wizard for linear static stress on a single part. Steps: fixtures (fixed faces), loads (force or pressure on faces, direction), material, run (with mesh density slider), results (von Mises stress, displacement, factor of safety, deformed shape animation, report). Optimisation step varies one dimension to meet a factor of safety or stress target.

## FloXpress

A wizard for internal water or air flow through a part. Steps: fluid (water or air), inlet (volume flow rate or pressure), outlet (pressure), check geometry (closed cavity with lids), run, results (flow trajectories coloured by velocity, animated).

## DFMXpress

Checks a part for manufacturability against rules per process. Rules include: hole depth-to-diameter ratio, hole entrance and exit perpendicularity, flat-bottom holes, holes intersecting cavities, partial holes, minimum internal corner radius for milling, fillets on outside corners, slot and pocket depth ratios, sheet metal hole sizes and distances to edges and bends, minimum bend radius, turned part checks. Each rule has a configurable value per material/process.

## DriveWorksXpress

Rules-based configuration: capture model dimensions, features and custom properties; build a form of inputs; write rules (IF, lookups) mapping inputs to captured items; generate new part, assembly and drawing files with names from rules.

## SustainabilityXpress and Sustainability

Life cycle assessment from material, manufacturing process, region of manufacture and use, transport and end of life; reports carbon footprint, energy consumption, air acidification and water eutrophication; compares alternative materials. Sustainability (full) adds assemblies and configurable use phase.

## Costing

Estimates manufacturing cost from templates per process:

| Process | Cost drivers |
| --- | --- |
| Machining | Stock (block, plate, cylinder), material cost, features recognised as operations (drilling, milling, turning), time-based rates, setup |
| Sheet metal | Blank size, material, cutting length and method (laser, plasma, waterjet), bends, forming tools, operations |
| Weldments | Members, cuts, welds per length |
| Casting, plastic molding | Volume-based rates, cycle time |
| 3D printing | Volume, support, printer rates |
| Assemblies | Sum of parts with purchased costs and assembly operations |

Outputs: cost breakdown per feature/operation, report, quantity breaks, and markup.

## Design Checker

Builds a standards file of checks (document properties, dimension and note styles, fonts, units, materials, feature checks such as fillets or holes, custom properties, sketch fully defined, file locations); runs checks on documents with a pass/fail report and auto-corrects where possible; can run in Task Scheduler.

## Motion studies

| Study type | Capability |
| --- | --- |
| Animation | Keyframe animation of component positions, explode/collapse, rotations, camera views, appearance changes; motors drive motion along mates |
| Basic Motion | Approximate physics: motors, springs, gravity, contact; no forces results |
| Motion Analysis | Full rigid-body dynamics (requires the Motion product) with forces, dampers, results plots |

Timeline with keys, playback speed, AVI export, and view orientation and camera keys.

## Design studies

Parameter sweeps and optimisation: variables (dimensions, global variables, simulation parameters) with ranges or discrete values, constraints (sensors), goals (minimise/maximise a sensor); evaluation mode lists scenarios; optimisation mode finds the best combination.

## Toolbox

Standard hardware library generated from a database: bolts, screws, nuts, washers, pins, retaining rings, bearings, keys, O-rings, gears, sprockets, cams, structural steel, and jig bushings, for standards ANSI, AS, BSI, CISC, DIN, GB, IS, ISO, JIS, KS, GOST, PEM, SKF, Torrington, Truarc, Unistrut. Options per component: size, length, thread length, thread display, part number; configuration or copied-part creation; property links.

## CircuitWorks

Imports IDF and PADS PCB files into a board part and component models, with libraries mapping footprints to models; exports SOLIDWORKS boards back to ECAD.

## Routing

Pipes and tubes (route along 3D sketches with fittings, auto-route, bends, spools), electrical (cables, wires, harnesses, connectors with pin data, from-to lists, flattened harness drawings), and fittings libraries.

## SOLIDWORKS Utilities

Compare (documents, features, geometry, BOM), find/modify (features by type and parameter value), geometry analysis, power select (select by filter), simplify (suppress features below thresholds), symmetry check, thickness analysis. See [evaluate tools](17-evaluate-and-analysis-tools.md) for the analysis utilities.

## SLDXML data exchange

Export and import model data (features, dimensions, properties) as XML for PLM integration.

## ScanTo3D

See [mesh and scan data](09-mesh-and-scan-data.md#scanto3d).
