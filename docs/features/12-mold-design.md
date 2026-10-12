# 12. Mold design

Detailed specs for [catalog section 12](../solidworks-feature-catalog.md#12-mold-design). See the [README](README.md) for sources and conventions.

The mold workflow takes a plastic part, checks it for moldability, scales it for shrinkage, defines where the mold splits, and produces core and cavity bodies. The tools appear in the Mold Tools tab.

**OpenForge today:** none of these. Neutral-plane draft and split are the closest existing features.

## Draft analysis

Colours faces by their draft relative to a pull direction. Options: direction of pull (plane, face or edge), draft angle, face classification (gradual transition), find steep faces, colour settings for positive draft, negative draft, requires draft and straddle faces. Output counts faces in each class.

## Undercut analysis

Finds faces that can't be pulled from the mold in the direction of pull (trapped areas). Options: direction of pull, parting line (analyse each side), coordinate input, and colour classes: direction 1 undercut, direction 2 undercut, occluded undercut, straddle undercut, no undercut.

## Parting line

Selects the edges where core and cavity split, usually the silhouette at the pull direction. API: `IPartingLineFeatureData`.

| Option | Values | Notes |
| --- | --- | --- |
| Direction of pull | Plane, face or edge; reverse | `PullDirectionBase`. |
| Draft angle | Angle used for the built-in draft analysis | `Angle`, `DraftAnalysis`. |
| Use for core/cavity split | Picks edges between positive and negative draft automatically | `CoreCavitySplit`. |
| Split faces | Splits straddle faces at the transition, or at specified entities | `SplitFaces`, `SplitFacesOption`, `EntitiesToSplit`. |
| Parting lines | Edge chain; add or remove edges | `PartingLines`. |

Status messages report when the line is incomplete or doesn't separate the part (`Status`).

## Shut-off surfaces

Patches through-holes so core and cavity can meet. API: `IShutOffSurfaceFeatureData`. Options: edges of each loop (found automatically), patch type per loop: contact, tangent or no fill (`swShutOffSurfacePatchType_e`), flip the tangent face, knit.

## Parting surface

Extends a surface outward from the parting line, which becomes the split plane between core and cavity. API: `IPartingSurfaceFeatureData`.

| Option | Values |
| --- | --- |
| Mold parameters | Tangent to surface, normal to surface, perpendicular to pull (`swPartingSurfaceMoldParmType_e`) |
| Parting line | `PartingLines` |
| Distance and angle | `OffsetDistance`, `OffsetAngle`; reverse direction |
| Smoothing | Sharp or smooth between adjacent edges, with a distance (`swPartingSurfaceSmoothingType_e`, `TransitionType`, `TransitionDistance`) |
| Options | Knit all surfaces, show preview, manual mode (edit the surface with handles) |

## Interlock surface

A tapered band around the parting surface so the mold halves locate on each other. Created inside Tooling Split with an angle (`InterlockSurface`, `Angle`).

## Tooling split

Creates core and cavity solid bodies from a block sketch and the core, cavity and parting surfaces. API: `IToolingSplitFeatureData`. Options: block sketch on a plane, depth in direction 1 and 2 (`Depth`), core surfaces, cavity surfaces, parting surfaces (all found from the mold folders), interlock surface with draft angle.

## Core

Extracts a side core or lifter from the core or cavity block. API: `ICoreFeatureData`. Options: bounding sketch, target body, extraction direction, end condition and depth per direction, draft (angle, outward), cap ends.

## Cavity

The legacy assembly approach: a cavity feature subtracts a design part from a mold base part inside an assembly, scaled for shrinkage. API: `ICavityFeatureData`. Options: design components, scale about part centroid, part origin, mold base origin or a coordinate system; uniform scale percentage or per-axis X, Y and Z.

## Scale (shrinkage factor)

Uses the [Scale feature](03-part-features.md#scale) to enlarge the part by the material's shrink factor, uniform or per axis, about the centroid, origin or a coordinate system, before the mold split.

## Mold folders

Surface Bodies contains special folders created by the mold features: Cavity Surface Bodies, Core Surface Bodies and Parting Surface Bodies. Tooling Split reads its inputs from them.
