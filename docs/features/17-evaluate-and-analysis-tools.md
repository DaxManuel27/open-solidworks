# 17. Evaluate and analysis tools

Detailed specs for [catalog section 17](../solidworks-feature-catalog.md#17-evaluate-and-analysis-tools). See the [README](README.md) for sources and conventions.

## Measure

Measures between any selected entities. API: `IMeasure`.

| Selection | Results |
| --- | --- |
| One edge | Length; for arcs, radius, diameter, arc length, center |
| One face | Area, perimeter; for cylinders, radius and axis |
| Two entities | Distance (center-to-center, minimum or maximum, set per arc), normal distance, dX/dY/dZ, angle |
| Point | XYZ coordinates |

Options: arc/circle measurement mode (center, minimum, maximum distance), units and precision, show XYZ, projected on (none, screen, selected plane), relative to a coordinate system, point-to-point mode, measurement history, create sensor from measurement.

**OpenForge today:** length, angle, area and coordinate values between selected references, with the measured points.
**Missing:** arc distance modes, dX/dY/dZ, projection onto planes, coordinate system choice, history, sensors.

## Mass properties

API: `IMassProperty`.

| Result | Notes |
| --- | --- |
| Density, mass, volume, surface area | From material per body |
| Center of mass | Relative to the output coordinate system |
| Principal axes of inertia and principal moments | At the center of mass |
| Moments of inertia | At the center of mass, aligned with the output coordinate system; and at the output coordinate system (`swMassPropertyMoment_e`) |
| Products of inertia | |

Options: output coordinate system, include hidden bodies/components, show weld bead mass, report coordinate values relative to, accuracy level, assign or override mass properties, recalculate, copy to clipboard, print.

**OpenForge today:** mass, volume, area, center of mass, inertia tensor, principal moments and axes, per material density.
**Missing:** output coordinate system, moments at the coordinate system, products of inertia shown separately, accuracy level, override.

## Override mass properties

Overrides mass, center of mass and moments of inertia with typed values (for purchased parts modelled as simple shapes). Overrides propagate to assemblies.

## Section properties

For a planar face or a sketch region: area, centroid, area moments of inertia, polar moment, principal axes and moments, relative to the centroid or a coordinate system.

## Materials library and custom materials

Materials carry physical properties used by mass properties and simulation:

| Property group | Properties |
| --- | --- |
| Physical | Density, elastic modulus, Poisson's ratio, shear modulus, thermal expansion, thermal conductivity, specific heat |
| Strength | Tensile strength, yield strength, compressive strength |
| Appearance | Linked appearance, crosshatch pattern, custom colour |
| Sustainability | Environmental data for SustainabilityXpress |
| Sheet metal | Thickness and bend data (2020 and later) |

Libraries are `.sldmat` XML files; users create custom libraries and categories, and copy materials. Material per body is supported.

**OpenForge today:** 18 built-in materials with density; per-configuration material.
**Missing:** mechanical and thermal properties, appearance links, hatching, custom libraries, per-body material.

## Sensors

Monitored values with alerts. Types: mass properties, measurement, dimension, interference detection (assembly), proximity (motion), simulation data. Each has a limit (greater than, less than, within, outside) and notification on change.

## Performance evaluation

Lists each feature's rebuild time and percentage of total, sorted, with suppress and roll-back options; for assemblies lists component open times, mate solve times, and graphics triangle counts.

**OpenForge today:** total rebuild time.

## Check entity/check geometry

Checks bodies for invalid faces and edges, short edges below a length, minimum radius of curvature, maximum edge or vertex gap, and open surfaces. Options: strict entity check, check solids and surfaces, check selected entities.

## Geometry analysis

Finds problem areas: small edges below a length, small faces below an area, sliver faces below a width, knife edges and vertices below an angle, discontinuous faces and edges.

## Import diagnostics

For imported bodies: finds faulty faces and gaps between faces; attempts to heal all, heal faces, or fill gaps; reports remaining problems.

## Deviation analysis

Measures the angle between faces along shared edges at sample points, reporting maximum, minimum and average deviation for continuity checks.

## Zebra stripes, curvature display and curvature combs

- **Zebra stripes:** number and width of stripes, precision, direction (horizontal, vertical, spherical); reveals tangency and curvature discontinuities.
- **Curvature:** colours faces by curvature value, with a scale.
- **Curvature combs:** on curves and sketch entities, with density and scale.

## Draft and undercut analysis

See [mold design](12-mold-design.md).

## Thickness analysis

Colours a part by wall thickness. Options: target thickness, show thin regions (below target) or thick regions (above), resolution, full colour or under-target only, report with per-region volumes. Uses ray or sphere methods.

## Symmetry check

Checks a part for symmetry about a plane and colours symmetric, non-symmetric and unique faces.

## Compare documents, features, geometry and BOMs

Utilities compare two versions of a part or assembly: document properties, features (added, deleted, modified), geometry (volume and face comparison), and BOMs.

## Statistics

Feature statistics (rebuild time, feature count), assembly statistics (component counts by type and state, mate count, top-level vs total), drawing statistics.
