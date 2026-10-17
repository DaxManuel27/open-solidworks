import type { IconId } from './icons'

/** What a Phase 6 feature needs before it can start. */
export type Needs = 'nothing' | 'sketch' | 'solid' | 'surface' | 'mesh' | 'body' | 'parting'

/** A feature edited through its schema in the generic feature panel. */
export interface FormFeature {
  label: string
  icon: IconId
  needs: Needs
}

/** Every feature type the generic panel edits, by its schema type name. */
export const FORM_FEATURES: Record<string, FormFeature> = {
  extruded_surface: { label: 'Extruded Surface', icon: 'surfaceExtrude', needs: 'sketch' },
  revolved_surface: { label: 'Revolved Surface', icon: 'surfaceRevolve', needs: 'sketch' },
  swept_surface: { label: 'Swept Surface', icon: 'surfaceSweep', needs: 'sketch' },
  lofted_surface: { label: 'Lofted Surface', icon: 'surfaceLoft', needs: 'sketch' },
  boundary_surface: { label: 'Boundary Surface', icon: 'boundarySurface', needs: 'sketch' },
  planar_surface: { label: 'Planar Surface', icon: 'planarSurface', needs: 'nothing' },
  offset_surface: { label: 'Offset Surface', icon: 'offsetSurface', needs: 'body' },
  filled_surface: { label: 'Filled Surface', icon: 'fillSurface', needs: 'body' },
  ruled_surface: { label: 'Ruled Surface', icon: 'ruledSurface', needs: 'body' },
  radiate_surface: { label: 'Radiate Surface', icon: 'radiateSurface', needs: 'body' },
  knit: { label: 'Knit Surface', icon: 'knit', needs: 'surface' },
  thicken: { label: 'Thicken', icon: 'thicken', needs: 'surface' },
  cut_with_surface: { label: 'Cut With Surface', icon: 'surfaceCut', needs: 'solid' },
  trim_surface: { label: 'Trim Surface', icon: 'trimSurface', needs: 'surface' },
  untrim_surface: { label: 'Untrim Surface', icon: 'untrimSurface', needs: 'surface' },
  extend_surface: { label: 'Extend Surface', icon: 'extendSurface', needs: 'surface' },
  mid_surface: { label: 'Mid-Surface', icon: 'midSurface', needs: 'solid' },
  freeform: { label: 'Freeform', icon: 'freeform', needs: 'body' },
  surface_flatten: { label: 'Surface Flatten', icon: 'flattenSurface', needs: 'body' },
  delete_face: { label: 'Delete Face', icon: 'deleteFace', needs: 'body' },
  move_face: { label: 'Move Face', icon: 'moveFace', needs: 'solid' },
  replace_face: { label: 'Replace Face', icon: 'replaceFace', needs: 'solid' },
  delete_hole: { label: 'Delete Hole', icon: 'deleteHole', needs: 'surface' },
  delete_body: { label: 'Delete/Keep Body', icon: 'deleteBody', needs: 'body' },
  intersect: { label: 'Intersect', icon: 'intersect', needs: 'body' },
  scale: { label: 'Scale', icon: 'scale', needs: 'body' },
  flex: { label: 'Flex', icon: 'flex', needs: 'solid' },
  deform: { label: 'Deform', icon: 'deform', needs: 'solid' },
  indent: { label: 'Indent', icon: 'indent', needs: 'solid' },
  insert_part: { label: 'Insert Part', icon: 'insertPart', needs: 'nothing' },
  defeature: { label: 'Defeature', icon: 'defeature', needs: 'solid' },
  slicing: { label: 'Slicing', icon: 'slicing', needs: 'body' },
  parting_line: { label: 'Parting Line', icon: 'partingLine', needs: 'solid' },
  shut_off_surfaces: { label: 'Shut-off Surfaces', icon: 'shutOff', needs: 'parting' },
  parting_surface: { label: 'Parting Surface', icon: 'partingSurface', needs: 'parting' },
  tooling_split: { label: 'Tooling Split', icon: 'toolingSplit', needs: 'parting' },
  mold_core: { label: 'Core', icon: 'moldCore', needs: 'solid' },
  convert_to_mesh: { label: 'Convert to Mesh Body', icon: 'meshBody', needs: 'body' },
  decimate_mesh: { label: 'Decimate Mesh', icon: 'decimate', needs: 'mesh' },
  segment_mesh: { label: 'Segment Mesh', icon: 'segmentMesh', needs: 'mesh' },
  mesh_prep: { label: 'Mesh Prep Wizard', icon: 'meshPrep', needs: 'mesh' },
  mesh_to_brep: { label: 'Mesh to Surface/Solid', icon: 'meshBody', needs: 'mesh' },
  surface_from_mesh: { label: 'Surface from Mesh', icon: 'meshSurface', needs: 'mesh' },
  mesh_to_solid: { label: 'Mesh to Solid Wizard', icon: 'meshSolid', needs: 'mesh' },
  mesh_sections: { label: 'Curve Wizard', icon: 'meshSections', needs: 'mesh' },
}

/** Toolbar command names for the form features: `p6` and their type in PascalCase. */
export function commandName(type: string): string {
  return 'p6' + type.replace(/(^|_)([a-z0-9])/g, (_, __, c: string) => c.toUpperCase())
}
