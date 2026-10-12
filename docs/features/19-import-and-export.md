# 19. Import and export

Detailed specs for [catalog section 19](../solidworks-feature-catalog.md#19-import-and-export). See the [README](README.md) for sources and conventions.

**OpenForge today:** STEP import and export, IGES import and export, STL export. Imported bodies are stored inside the document.

## Shared import options

| Option | Meaning |
| --- | --- |
| Import as | Solid/surface bodies, or graphics only |
| Units | From the file, or a chosen unit when the file has none |
| Run Import Diagnostics | Repair faulty faces and gaps after import (see [evaluate](17-evaluate-and-analysis-tools.md#import-diagnostics)) |
| Assembly structure | Import multi-body files as an assembly, a multibody part, or one part per body |
| 3D Interconnect | Keep a link to the source file and update when it changes; or break the link |
| Free point/curve entities | Import loose points and curves as sketches |
| Map configuration data | STEP/IGES configurations or named views |
| Custom properties | Import file properties (STEP, Parasolid) |
| Import material and appearance | Colours and materials where present |

## Shared export options

| Option | Meaning |
| --- | --- |
| Output coordinate system | Export relative to a coordinate system instead of the origin |
| Export bodies | Selected bodies only, or all |
| Configuration | Active, all or selected configurations |
| Units | Output units |
| Split periodic faces, flat surfaces as planes | Compatibility with older systems |

## Format details

| Format | Import | Export | Format-specific options |
| --- | --- | --- | --- |
| STEP AP203, AP214, AP242 | ✓ | ✓ | Protocol; export appearances, sketches and curves, 3D curves, PMI (AP242), face/edge properties, custom properties; split periodic faces; tessellated geometry |
| IGES 5.3 | ✓ | ✓ | Surface representation (trimmed surfaces, manifold solid B-rep, wireframe); 3D curves; surfaces as NURBS; sketch entities |
| Parasolid (`.x_t`, `.x_b`) | ✓ | ✓ | Version; text or binary |
| ACIS (`.sat`) | ✓ | ✓ | Version; units |
| STL | ✓ | ✓ | Binary or ASCII; resolution (coarse, fine, custom with deviation and angle tolerances); units; output each body as a separate file; check for interferences; include colour (non-standard) |
| 3MF | ✓ | ✓ | Resolution as STL; include materials and appearances |
| AMF | ✓ | ✓ | Resolution; colours |
| OBJ, PLY, VRML | ✓ | ✓ | Resolution; textures for OBJ and VRML |
| glTF/GLB | ✓ | ✓ | Resolution; appearances; binary or JSON |
| DXF/DWG | ✓ (2D and 3D) | ✓ | See below |
| PDF and 3D PDF | | ✓ | 3D PDF uses templates ([MBD](16-tolerancing-and-mbd.md)) |
| eDrawings | | ✓ | Allow measure, save STL, include shaded data |
| 3D XML, HCG, HOOPS, SMG | | ✓ | Lightweight viewer formats |
| IFC 2x3, IFC4 | ✓ | ✓ | Entity mapping for building elements |
| JT | ✓ | | Lightweight or B-rep |
| CATIA V5, Creo, NX, Inventor, Solid Edge, Rhino | ✓ | | Via 3D Interconnect: native file read, linked updates, configurations from family tables |
| JPEG, PNG, TIFF, PSD, AI | | ✓ | Resolution, DPI, print capture |
| Save as previous version | | ✓ | Saves to the last two releases (2024 and later) |

**OpenForge today:** STEP in and out without configuration data or PMI; IGES in and out; STL out.
**Missing:** every other format; STL resolution options and per-body export; output coordinate system; export of selected bodies; import options above.

## DXF/DWG import wizard and 2D-to-3D

- **Import wizard:** import to a new part as a 2D sketch, as a 3D curve, or to a new drawing; choose layers to import; units; document template; merge points within a distance; fix overlapping geometry; position the sketch origin.
- **2D to 3D:** in a part, pick sketch regions from an imported drawing and assign them to front, top, right or auxiliary sketch planes, align them, then extrude or cut to build the part.

## Export to AEC

Simplified export for building information software: removes internal detail, exports IFC or a simplified part with connection points.
