# 18. Model display and rendering

Detailed specs for [catalog section 18](../solidworks-feature-catalog.md#18-model-display-and-rendering). See the [README](README.md) for sources and conventions.

## Display styles

Shaded with edges, shaded, hidden lines removed, hidden lines visible, wireframe. Per-component display styles in assemblies; draft quality options; tangent edge display (visible, as phantom, removed); silhouette edges.

**OpenForge today:** shaded, shaded with edges, wireframe and x-ray.
**Missing:** hidden lines removed and visible, tangent edge options, per-body styles.

## Standard views

Front, back, left, right, top, bottom, isometric, dimetric, trimetric and normal to (`swStandardViews_e`). Normal to: selecting a face and pressing it again flips to the other side. Saved named views can be created in the orientation dialog.

**OpenForge today:** front, top, right and isometric.
**Missing:** back, left, bottom, dimetric, trimetric, normal to, named views.

## View selector cube and orientation dialog

A cube around the model showing all 26 views (faces, edges and corners) for one-click orientation, plus the orientation dialog with saved views, pin dialog and update standard views (redefine which way is front).

**OpenForge today:** axis triad only.

## View manipulation

Zoom to fit (F), zoom to area, zoom in/out, zoom to selection, pan, rotate (about the screen center or about a picked entity), roll, previous view, rotation increment for arrow keys. Mouse gestures map commands to drag directions.

**OpenForge today:** orbit, pan, zoom and fit.
**Missing:** zoom to area and selection, rotate about an entity, roll, previous view, mouse gestures.

## Perspective and cameras

Perspective toggle with an object-size ratio; cameras with target, position, field of view, and lock; camera views for walkthroughs.

**OpenForge today:** perspective only.

## Section view (models)

Live cutaway with up to three section planes (planes or faces), offsets and rotations; zonal section (box regions); section by graphics only or by geometry (for selecting cut faces); keep cap colour; show section cap; save as a named view or as a drawing annotation view.

**OpenForge today:** not available.

## Model break view

Shortens long models in the graphics area for easier viewing, with break lines and gap.

## Viewports

Single, two horizontal, two vertical, or four viewports with linked orientations (link views for the four-view layout).

## Hide/show types

View menu toggles for planes, axes, temporary axes, origins, coordinate systems, curves, sketches, sketch relations, 3D sketch planes, dimension names, annotations, lights, cameras, decals, and more; Hide All Types.

**OpenForge today:** planes toggle.

## Appearances

Appearances define colour, texture and optical properties; applied at face, feature, body, part or component level, with a priority order.

| Property | Options |
| --- | --- |
| Colour | Base colour, or image texture |
| Illumination | Diffuse, specular, specular spread, reflectivity, blurry reflections, transparency, translucency, luminous intensity |
| Surface finish | Bump or displacement maps, procedural finishes |
| Mapping | Surface, projection, spherical, cylindrical, box; scale, rotation, mirror |
| Advanced | Per-appearance render settings |

## Decals

Images applied to faces with mask (image alpha, colour mask), mapping (as with appearances), size and orientation, and transparency.

## Scenes and HDRI backgrounds

Scenes combine a background, environment image for reflections, floor (reflections and shadows) and lighting. Background options: none, colour, gradient, image, environment.

## Lights

Ambient, directional, point, spot and sunlight (location and date), each with colour, intensity, position, shadows, and whether it's on in SOLIDWORKS display and in rendering.

## RealView, shadows and ambient occlusion

RealView uses GPU shaders for realistic appearances; shadows in shaded mode; ambient occlusion darkens crevices. Graphics card certification governs availability.

## Cartoon rendering

Draws the model with flat shading and black outlines.

## Snapshots

Saved graphics states with orientation and display, listed in the DisplayManager; useful for quick return to a view.

## Walk-through

First-person navigation through a model with a camera, keyboard and mouse controls, and recordable paths.

## Solar access studies

Models sun position for a location, date and time range, and animates shading on the model.

## Image capture and screen recording

Save as image (JPEG, PNG, TIFF, BMP) at a resolution and DPI; copy to clipboard; screen capture video (AVI) of the graphics area.

## SOLIDWORKS Visualize export

Sends the model with appearances, decals and cameras to SOLIDWORKS Visualize for GPU ray-traced rendering, with a live link for updates.
