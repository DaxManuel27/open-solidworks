# 9. Mesh and scan data

Detailed specs for [catalog section 9](../solidworks-feature-catalog.md#9-mesh-and-scan-data). See the [README](README.md) for sources and conventions.

SOLIDWORKS has three kinds of mesh body, each with different abilities:

| Kind | What it is | Can be used in features? |
| --- | --- | --- |
| Graphics body | Triangles for display only | No; display, measuring and mating only |
| Mesh BREP body | Each facet becomes a B-rep face, grouped into facet faces | Yes, with booleans and cuts; slow on large meshes |
| Hybrid body | Mixed analytic B-rep faces and facets in one body | Yes |

**OpenForge today:** STL export only. No mesh import or mesh bodies.

## Import mesh

File formats: STL, OBJ, OFF, PLY, PLY2, 3MF, VRML, and AMF. Import options:

- Import as graphics body, as a solid body, or as a surface body.
- Units, when the file doesn't carry them.
- Maximum number of facets for BREP conversion (large meshes stay graphics).
- Group facets into faces (planar regions become one face).
- Import texture and colour information (3MF, OBJ, VRML).

## Mesh BREP and hybrid bodies in features

- Booleans (combine, cuts) work between mesh BREP bodies and analytic bodies, giving hybrid bodies.
- Sketches can reference facet edges and vertices; convert entities and intersection curves work on facets.
- Shell, fillet and draft don't work on facet faces.
- Reference planes and axes can be fitted to selected facets (plane through facets, axis through a cylindrical facet group).

## Convert to mesh body

Converts a solid or surface body to a mesh BREP body. Options: keep the original body, refinement (coarse to fine), maximum distance (chord height) and maximum angle deviation, group facets into faces.

## Decimate mesh

Reduces facet count on a mesh body. Options: whole body or a region, reduce by percentage or tolerance, preserve boundary, and reset.

## Segment imported mesh

Groups facets into face regions by curvature, so they can be selected and converted to analytic faces. Options: segmentation sensitivity, show segment boundaries, edit segments.

## Paint select facets

Selects facets by painting a region on the mesh with a brush of a set size; used to select regions for surface fitting or deletion.

## ScanTo3D

Turns scan point clouds and meshes into solid models.

- **Mesh prep wizard:** orientation, noise removal, extraneous data removal, simplification, smoothing, filling holes, boundary smoothing.
- **Surface wizard:** automatic creation (fits a NURBS surface patch network to the whole mesh with detail control) or guided creation (feature recognition: plane, cylinder, cone, sphere, extrusion, revolve, freeform regions).
- **Curve wizard:** section curves through the mesh by plane, for sketch-based reconstruction.
- **Deviation analysis:** colours the distance between the mesh and the new surfaces.
- Input formats include point clouds in XYZ, TXT, ASC, VDA and IGES.
