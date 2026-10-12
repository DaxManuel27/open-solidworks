# 11. Weldments and structure system

Detailed specs for [catalog section 11](../solidworks-feature-catalog.md#11-weldments-and-structure-system). See the [README](README.md) for sources and conventions.

A weldment is a multibody part made of structural members (profiles swept along sketch lines), plates, gussets, end caps and welds. Cut lists group identical bodies and report lengths for fabrication.

**OpenForge today:** no weldments. The sweep along sketch paths and multibody support are groundwork.

## Weldment feature

Inserting the Weldment feature marks the part as a weldment: it adds a cut list folder, turns off merge for new bodies, and adds weldment-specific custom properties. A part can have only one Weldment feature, placed before the first member.

## Structural member

Sweeps a standard profile along sketch segments from 2D or 3D sketches. API: `IStructuralMemberFeatureData`.

| Option | Values | Notes (API name) |
| --- | --- | --- |
| Profile | Standard (ANSI inch, ISO), type (angle iron, C channel, pipe, rectangular tube, square tube, S section, SB beam, I beam), size | `WeldmentProfilePath`, `ConfigurationName` (for configured profiles). |
| Transfer material | Use the material stored in the profile file | `TransferMaterial`, `LibraryProfileMaterial`. |
| Groups | Sets of segments, each set treated together (parallel or contiguous) | `Groups`, `PathSegments`. |
| Corner treatment per group | End miter, end butt 1, end butt 2; merge miter-trimmed bodies; gap between members | `ApplyCornerTreatment`, `CornerTreatmentType`, `ConnectionType` per connection point. |
| Connected segments | Simple cut or coped cut where members meet | `ConnectedSegmentsOption` (`swConnectedSegmentsOption_e`). |
| Profile position | Mirror profile horizontally or vertically; align to an edge or axis; rotation angle | `RotationAngle`. |
| Locate profile | Pick a sketch point in the profile to put on the path (default is the profile's origin) | `LocateProfilePoint`. |
| Allow protrusion | Lets the member extend beyond the path at corners | `AllowProtrusion`. |

**Rules:** each member is a separate body. Segments in a group must not cross. Members in one feature can't share a segment.

## Weldment profiles library and custom profiles

Profiles are library feature parts (`.sldlfp`) holding a single sketch, in folders by standard and type, with optional configurations for sizes. Custom profiles: draw a sketch, save it as a library feature in the profile folder. Additional profiles are downloadable from the Design Library.

## Trim/extend

Trims or extends members against other members or planar faces. API: `IWeldmentTrimExtendFeatureData`.

| Option | Values |
| --- | --- |
| Corner type | End trim, end miter, end butt 1, end butt 2 (`CornerType`) |
| Bodies to be trimmed | `BodiesToBeTrimmed` |
| Trimming boundary | Face/plane or bodies; with face/plane, members are cut flush (`TrimmingBoundary`) |
| Options | Allow extension, simple cut between bodies or coped cut, weld gap (`swWeldmentTrimExtendOptionType_e`) |

## Corner treatments (structure system)

In the structure system, corners are managed per corner group. Corner types (`swCornerType_e`): simple (two members, two trim options), two-member (three trim options) and complex (three or more members).

| Trim type | Meaning |
| --- | --- |
| Planar trim | Cut the member with a plane at first contact or full contact (`swCornerTreatmentPlanarTrimOptions_e`) |
| Body trim | Cut the member to the shape of the other member, in a set trim order |
| Miter trim | Both members cut at the bisecting plane; two-member corners only |

Options: weld trim gap, trim tool (automatic or user-defined faces), swap members.

## End cap

A plate closing an open tube end. API: `IEndCapFeatureData`.

Options: face to cap, thickness direction (outward, inward, or inset; `swEndCapThicknessDirection_e`), thickness, offset by ratio of wall thickness or distance, corner treatment (chamfer or fillet with distance).

## Gusset

A plate between two members' adjacent faces. API: `IGussetFeatureData`.

| Option | Values |
| --- | --- |
| Supporting faces | Two adjacent planar faces |
| Profile | Triangular or polygonal (`swGussetProfileType_e`), sizes d1 to d4, or a leg and an angle; inner corner chamfer |
| Thickness | Inner, both sides, outer (`swGussetThicknessType_e`), thickness value |
| Location | Start, middle or end of the edge (`swGussetProfileLocationType_e`), with offset |

## Weld beads

Two kinds:

- **Weld bead (cosmetic, 2014 and later):** lightweight graphics with weld symbol and properties. API: `ICosmeticWeldBeadFeatureData`. Options: weld path or weld from/to faces, smart weld selection, bead size, tangent propagation, both sides or one side (`swWeldBeadSide_e`), intermittent weld with length and gap or pitch, staggered, from/to length with start point, weld symbol.
- **Fillet weld bead (legacy, modeled):** real geometry. API: `IWeldmentBeadFeatureData`. Options: type full length, intermittent or staggered (`swWeldBeadType_e`), bead size (leg length), length and pitch, other side, tangent propagation.

Weld beads appear in a Weld Folder with properties (weld material, process, mass per length, cost) and in weld tables on drawings.

## Smart weld selection

Draws a path across the faces to be welded, and SOLIDWORKS picks the intersecting edges automatically.

## Cut lists, cut list properties and folders

The Cut List folder groups identical bodies into cut list items automatically (update automatically option) or by hand. Each item has properties:

| Property | Source |
| --- | --- |
| Length | Member length along the path, including trims |
| Angle1, Angle2 | End cut angles |
| Description | From the profile |
| Material, mass | From the body |
| Quantity | Number of identical bodies |
| Bounding box length, width, thickness, volume | For plates and generic bodies |

Options: sort by profile, keep identical bodies together, collect identical bodies, exclude from cut list, cut list item names from properties.

## Sub-weldments and groups

Sub-weldment folders group bodies for separate fabrication; they can be saved as their own part (insert into new part). Groups inside structural members organise segments.

## Structure System

A frame-design mode (2019 and later) separate from weldment structural members.

| Item | Ways to create |
| --- | --- |
| Primary members | Sketch segment; reference plane intersection; point and length (up to a length, point, or plane; `swPrimaryMemberPointLengthEndCondition_e`); face-plane intersection |
| Secondary members | Between points on two primary members (distance or length ratio from the ends); support plane; up to members (`ISecondaryMemberUpToMembersFeatureData`) |
| Member profile | Per member, with rotation, mirroring and alignment |
| Extend and split | Extend each end by a distance; split a member at points |
| Corner management | One feature managing all corners with simple, two-member and complex groups (above) |
| Connection elements | Library parts (base plates, brackets) placed at corners |

## Weldment drawings and cut list tables

Drawing views of single bodies (relative view, select bodies), weldment cut list tables with item number, quantity, description, length and angles; balloon links to cut list items; weld tables; weld symbols attached to beads.
