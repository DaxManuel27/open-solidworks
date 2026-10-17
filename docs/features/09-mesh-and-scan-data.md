# 9. Mesh and scan data

Detailed specs for [catalog section 9](../solidworks-feature-catalog.md#9-mesh-and-scan-data). See the [README](README.md) for sources and conventions.

SOLIDWORKS has three kinds of mesh body, each with different abilities:

| Kind | What it is | Can be used in features? |
| --- | --- | --- |
| Graphics body | Triangles for display only | No; display, measuring and mating only |
| Mesh BREP body | Each facet becomes a B-rep face, grouped into facet faces | Yes, with booleans and cuts; slow on large meshes |
| Hybrid body | Mixed analytic B-rep faces and facets in one body | Yes |

**OpenForge today:** mesh import, graphics mesh bodies, mesh B-rep, mesh tools and scan-to-solid wizards, in the core, CLI/MCP and the desktop Mesh tab; STL export includes mesh bodies. Each section says what is supported; [the Phase 6 status](../phase-6-status.md) records the limits.

## Import mesh

File formats: STL, OBJ, OFF, PLY, PLY2, 3MF, VRML, and AMF. Import options:

- Import as graphics body, as a solid body, or as a surface body.
- Units, when the file doesn't carry them.
- Maximum number of facets for BREP conversion (large meshes stay graphics).
- Group facets into faces (planar regions become one face).
- Import texture and colour information (3MF, OBJ, VRML).

**OpenForge today:** STL, OBJ, OFF, PLY, 3MF and AMF as a graphics body, a surface or a solid, with a scale for units, a facet limit (100,000 by default), grouping flat runs of facets into faces, and vertex colours; XYZ, TXT and ASC point clouds become meshes. VRML is not read, and textures are not shown.

## Mesh BREP and hybrid bodies in features

- Booleans (combine, cuts) work between mesh BREP bodies and analytic bodies, giving hybrid bodies.
- Sketches can reference facet edges and vertices; convert entities and intersection curves work on facets.
- Shell, fillet and draft don't work on facet faces.
- Reference planes and axes can be fitted to selected facets (plane through facets, axis through a cylindrical facet group).

**OpenForge today:** mesh B-rep solids and surfaces take part in booleans, cuts and every B-rep feature; reference planes and axes fit painted facets; graphics bodies show in drawing views up to 200,000 triangles.

## Convert to mesh body

Converts a solid or surface body to a mesh BREP body. Options: keep the original body, refinement (coarse to fine), maximum distance (chord height) and maximum angle deviation, group facets into faces.

**OpenForge today:** solids and surfaces by chord height and angle, keeping the original or not.

## Decimate mesh

Reduces facet count on a mesh body. Options: whole body or a region, reduce by percentage or tolerance, preserve boundary, and reset.

**OpenForge today:** the whole body or a painted region, to a fraction or a tolerance, preserving the boundary or not.

## Segment imported mesh

Groups facets into face regions by curvature, so they can be selected and converted to analytic faces. Options: segmentation sensitivity, show segment boundaries, edit segments.

**OpenForge today:** regions by fold angle with a minimum area; Surface from Mesh fits a region.

## Paint select facets

Selects facets by painting a region on the mesh with a brush of a set size; used to select regions for surface fitting or deletion.

**OpenForge today:** a brush in the viewport paints triangles for decimation, fits and reference geometry.

## ScanTo3D

Turns scan point clouds and meshes into solid models.

- **Mesh prep wizard:** orientation, noise removal, extraneous data removal, simplification, smoothing, filling holes, boundary smoothing.
- **Surface wizard:** automatic creation (fits a NURBS surface patch network to the whole mesh with detail control) or guided creation (feature recognition: plane, cylinder, cone, sphere, extrusion, revolve, freeform regions).
- **Curve wizard:** section curves through the mesh by plane, for sketch-based reconstruction.
- **Deviation analysis:** colours the distance between the mesh and the new surfaces.
- Input formats include point clouds in XYZ, TXT, ASC, VDA and IGES.

**OpenForge today:** Mesh Prep (orient, remove pieces, noise, simplify, smooth, fill holes, smooth boundaries), Surface from Mesh per region, the Mesh to Solid wizard (segment, fit, extend and knit), the Curve Wizard (sections into sketches) and deviation analysis. The automatic whole-mesh NURBS network is not offered.
