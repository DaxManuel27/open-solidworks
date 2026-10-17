import { create } from 'zustand'
import { useStore, type PanelType, type SketchTool, type ViewName } from '../store'
import {
  beginSketch,
  ensureWeldment,
  isFlattened,
  startFeature,
  startWeldmentFeature,
  toggleFlatten,
} from './actions'
import type { IconId } from './icons'
import { FORM_FEATURES, type Needs } from './phase6'

type State = ReturnType<typeof useStore.getState>
interface Definition {
  label: string
  icon: IconId
  shortcut?: string
  run?: (s: State) => unknown
  unavailable?: (s: State) => string | undefined
  active?: (s: State) => boolean
  view?: boolean
}
const body = (s: State) =>
  s.assembly ? 'Open a part to use this' : !s.scene?.body ? 'Requires a solid body' : undefined
const assemblyOnly = (s: State) =>
  !s.assembly ? 'Requires an assembly' : s.panel ? 'Finish or cancel the current panel' : undefined
const components = (s: State) =>
  assemblyOnly(s) ?? (!s.assembly?.components.length ? 'Insert components first' : undefined)
const asm = (
  type: PanelType,
  label: string,
  icon: IconId,
  needsComponents = true,
  variant?: string,
): Definition => ({
  label,
  icon,
  run: (s) => s.openPanel({ type, variant }),
  unavailable: needsComponents ? components : assemblyOnly,
})
const idlePanel = (s: State) => (s.panel ? 'Finish or cancel the current panel' : undefined)
const part = (s: State) => idlePanel(s) ?? (s.sketch ? 'Exit the sketch first' : undefined)
const feature = (
  type: PanelType,
  label: string,
  icon: IconId,
  needsBody = false,
  needsSketch = false,
): Definition => ({
  label,
  icon,
  run: () => startFeature(type),
  unavailable: (s) =>
    idlePanel(s) ??
    (needsBody ? body(s) : undefined) ??
    (needsSketch && !s.snapshot?.features.some((f) => f.type === 'sketch' && !f.suppressed)
      ? 'Requires a sketch'
      : undefined),
})
/** A sheet metal feature; most need a sheet metal body to work on. */
const sheetMetal = (
  type: PanelType,
  label: string,
  icon: IconId,
  needsSheet = true,
  needsSketch = false,
): Definition => ({
  label,
  icon,
  run: () => startFeature(type),
  unavailable: (s) =>
    (s.assembly ? 'Open a part to use this' : undefined) ??
    idlePanel(s) ??
    (needsSheet && !s.snapshot?.sheet_metal?.length ? 'Requires a sheet metal body' : undefined) ??
    (needsSketch && !s.snapshot?.features.some((f) => f.type === 'sketch' && !f.suppressed)
      ? 'Requires a sketch'
      : undefined),
})
/** A weldment feature; the part becomes a weldment first if it isn't. */
const weld = (
  type: PanelType,
  label: string,
  icon: IconId,
  needs: 'nothing' | 'sketch' | 'body' = 'body',
  variant?: string,
): Definition => ({
  label,
  icon,
  run: () => startWeldmentFeature(type, variant),
  unavailable: (s) =>
    (s.assembly ? 'Open a part to use this' : undefined) ??
    idlePanel(s) ??
    (needs === 'sketch' &&
    !s.snapshot?.features.some(
      (f) => (f.type === 'sketch' || f.type === 'sketch3d') && !f.suppressed,
    )
      ? 'Requires a sketch'
      : undefined) ??
    (needs === 'body' && !s.scene?.body ? 'Requires a solid body' : undefined),
})
const panel = (type: PanelType, label: string, icon: IconId, needsBody = false): Definition => ({
  label,
  icon,
  run: (s) => s.openPanel({ type }),
  unavailable: (s) => part(s) ?? (needsBody ? body(s) : undefined),
})
const tool = (name: SketchTool, label: string, icon: IconId, shortcut?: string): Definition => ({
  label,
  icon,
  shortcut,
  run: (s) => s.setSketchTool(name),
  unavailable: (s) => (!s.sketch ? 'Requires an active sketch' : undefined),
  active: (s) => !!s.sketch && s.sketchTool === name,
})
const view = (name: ViewName, label: string, icon: IconId = 'orientation'): Definition => ({
  label,
  icon,
  view: true,
  run: (s) => s.setView(name),
})
const placeholder = (label: string, icon: IconId): Definition => ({ label, icon })
const needsReason = (s: State, needs: Needs): string | undefined => {
  const bodies = s.scene?.bodies ?? []
  const has = (kind: string) => bodies.some((b) => (b.kind ?? 'solid') === kind)
  switch (needs) {
    case 'sketch':
      return !s.snapshot?.features.some(
        (f) => (f.type === 'sketch' || f.type === 'sketch3d') && !f.suppressed,
      )
        ? 'Requires a sketch'
        : undefined
    case 'solid':
      return has('solid') ? undefined : 'Requires a solid body'
    case 'surface':
      return has('surface') ? undefined : 'Requires a surface body'
    case 'mesh':
      return has('mesh')
        ? undefined
        : 'Requires a mesh body; import an STL, OBJ, PLY or point cloud'
    case 'body':
      return bodies.length ? undefined : 'Requires a body'
    case 'parting':
      return s.snapshot?.features.some((f) => f.type === 'parting_line' && !f.error)
        ? undefined
        : 'Add a Parting Line first'
    default:
      return undefined
  }
}
/** A Phase 6 feature, edited in the generic feature panel. */
const form = (type: keyof typeof FORM_FEATURES & string): Definition => {
  const f = FORM_FEATURES[type]!
  return {
    label: f.label,
    icon: f.icon,
    run: () => startFeature('feature_form', type),
    unavailable: (s) =>
      (s.assembly ? 'Open a part to use this' : undefined) ??
      idlePanel(s) ??
      needsReason(s, f.needs),
  }
}
const analysisPanel = (type: PanelType, label: string, icon: IconId, needs: Needs): Definition => ({
  label,
  icon,
  run: (s) => s.openPanel({ type }),
  unavailable: (s) =>
    (s.assembly ? 'Open a part to use this' : undefined) ?? part(s) ?? needsReason(s, needs),
})
const drawingOnly = (s: State) =>
  !s.drawing ? 'Requires a drawing' : s.panel ? 'Finish or cancel the current panel' : undefined
const hasViews = (s: State) =>
  drawingOnly(s) ??
  (!s.drawing?.drawing.sheets.some((x) => x.views.length) ? 'Add a view first' : undefined)
const draw = (
  type: PanelType,
  label: string,
  icon: IconId,
  needsViews = true,
  variant?: string,
): Definition => ({
  label,
  icon,
  run: (s) => s.openPanel({ type, variant }),
  unavailable: needsViews ? hasViews : drawingOnly,
})

export const commands = {
  new: { label: 'New', icon: 'new', shortcut: 'Ctrl/Cmd+N', run: (s) => s.newDoc() },
  newAssembly: {
    label: 'New Assembly',
    icon: 'new',
    shortcut: 'Ctrl/Cmd+Shift+N',
    run: (s) => s.newAssembly(),
  },
  importAssembly: {
    label: 'Import STEP as Assembly',
    icon: 'open',
    run: (s) => s.importAssembly(),
    unavailable: idlePanel,
  },
  insertComponents: asm('asm_insert', 'Insert Components', 'open', false),
  hardware: asm('hardware', 'Hardware Library', 'hole', false),
  mate: asm('mate', 'Mate', 'mate'),
  advancedMate: asm('mate', 'Advanced Mates', 'mate', true, 'width'),
  mechanicalMate: asm('mate', 'Mechanical Mates', 'mate', true, 'gear'),
  moveComponent: asm('move_component', 'Move Component', 'move'),
  rotateComponent: asm('rotate_component', 'Rotate Component', 'revolve'),
  linearComponentPattern: asm(
    'component_pattern',
    'Linear Component Pattern',
    'pattern',
    true,
    'linear',
  ),
  circularComponentPattern: asm(
    'component_pattern',
    'Circular Component Pattern',
    'circular',
    true,
    'circular',
  ),
  patternDrivenPattern: asm(
    'component_pattern',
    'Pattern Driven Component Pattern',
    'pattern',
    true,
    'pattern_driven',
  ),
  sketchDrivenPattern: asm(
    'component_pattern',
    'Sketch Driven Component Pattern',
    'pattern',
    true,
    'sketch_driven',
  ),
  curveDrivenPattern: asm(
    'component_pattern',
    'Curve Driven Component Pattern',
    'curve',
    true,
    'curve_driven',
  ),
  mirrorComponents: asm('mirror_components', 'Mirror Components', 'mirror'),
  explodedView: asm('explode', 'Exploded View', 'section'),
  interference: asm('interference', 'Interference Detection', 'intersect'),
  clearance: asm('clearance', 'Clearance Verification', 'measure'),
  bom: asm('bom', 'Bill of Materials', 'properties'),
  asmPlane: asm('asm_reference', 'Plane', 'plane', false, 'plane'),
  asmAxis: asm('asm_reference', 'Axis', 'axis', false, 'axis'),
  asmMeasure: asm('asm_measure', 'Measure', 'measure'),
  asmMass: asm('asm_mass', 'Mass Properties', 'mass'),
  asmConfigurations: asm('asm_configurations', 'Configurations', 'properties', false),
  open: { label: 'Open', icon: 'open', shortcut: 'Ctrl/Cmd+O', run: (s) => s.open() },
  save: { label: 'Save', icon: 'save', shortcut: 'Ctrl/Cmd+S', run: (s) => s.save() },
  saveAs: {
    label: 'Save As',
    icon: 'save',
    shortcut: 'Ctrl/Cmd+Shift+S',
    run: (s) => s.save(true),
  },
  import: {
    label: 'Import',
    icon: 'open',
    run: (s) => s.importFile(),
    unavailable: (s) =>
      s.drawing
        ? 'Drawings show saved parts and assemblies; add a model view'
        : s.assembly
          ? 'Use Import STEP as Assembly'
          : part(s),
  },
  export: {
    label: 'Export',
    icon: 'export',
    run: (s) => s.openPanel({ type: s.drawing ? 'drawing_export' : 'export' }),
    unavailable: (s) =>
      s.drawing
        ? idlePanel(s)
        : s.assembly
          ? s.assembly.components.length
            ? idlePanel(s)
            : 'Insert components first'
          : (part(s) ?? body(s)),
  },
  undo: {
    label: 'Undo',
    icon: 'undo',
    shortcut: 'Ctrl/Cmd+Z',
    run: (s) => s.undo(),
    unavailable: (s) => idlePanel(s) ?? (!s.snapshot?.can_undo ? 'Nothing to undo' : undefined),
  },
  redo: {
    label: 'Redo',
    icon: 'redo',
    shortcut: 'Ctrl+Y / Cmd+Shift+Z',
    run: (s) => s.redo(),
    unavailable: (s) => idlePanel(s) ?? (!s.snapshot?.can_redo ? 'Nothing to redo' : undefined),
  },
  select: {
    label: 'Select',
    icon: 'select',
    shortcut: 'Esc',
    run: (s) => (s.sketch ? s.setSketchTool('select') : s.setPickTarget(null)),
    active: (s) => !!s.sketch && s.sketchTool === 'select',
  },
  print: {
    label: 'Print',
    icon: 'print',
    shortcut: 'Ctrl/Cmd+P',
    run: (s) => s.print(),
    unavailable: (s) => (!s.drawing ? 'Make a drawing to print' : idlePanel(s)),
  },
  rebuild: {
    label: 'Rebuild',
    icon: 'rebuild',
    run: (s) => (s.drawing ? s.run('drawing.update') : s.run('doc.info')),
    unavailable: (s) => (s.assembly ? 'Assemblies rebuild as you change them' : idlePanel(s)),
  },
  properties: {
    label: 'File Properties',
    icon: 'properties',
    run: (s) => s.openPanel({ type: s.drawing ? 'drawing_properties' : 'part_properties' }),
    unavailable: (s) =>
      s.assembly
        ? 'Set properties on the parts of an assembly'
        : (idlePanel(s) ?? (s.sketch ? 'Exit the sketch first' : undefined)),
  },
  makeDrawing: {
    label: 'Make Drawing',
    icon: 'drawing',
    run: (s) => s.makeDrawing(),
    unavailable: (s) =>
      s.drawing
        ? undefined
        : s.assembly && !s.assembly.components.length
          ? 'Insert components first'
          : !s.assembly && !s.scene?.body
            ? 'Requires a solid body'
            : idlePanel(s),
  },
  newDrawing: { label: 'New Drawing', icon: 'drawing', run: (s) => s.newDrawing() },
  standardViews: draw('standard_views', 'Standard 3 View', 'views3', false),
  modelView: draw('model_view', 'Model View', 'view', false),
  projectedView: draw('projected_view', 'Projected View', 'projected'),
  auxiliaryView: draw('auxiliary_view', 'Auxiliary View', 'auxiliary'),
  sectionView: draw('section_view', 'Section View', 'sectionView'),
  alignedSection: draw('section_view', 'Aligned Section View', 'sectionView', true, 'aligned'),
  detailView: draw('detail_view', 'Detail View', 'detail'),
  brokenOut: draw('broken_out', 'Broken-out Section', 'sectionView'),
  breakView: draw('break_view', 'Break', 'breakView'),
  cropView: draw('crop_view', 'Crop View', 'crop'),
  updateViews: {
    label: 'Update Views',
    icon: 'rebuild',
    run: (s) => s.run('drawing.update'),
    unavailable: hasViews,
  },
  addSheet: {
    label: 'Add Sheet',
    icon: 'new',
    run: (s) => s.run('drawing.sheet.add'),
    unavailable: drawingOnly,
  },
  sheetProperties: draw('sheet_properties', 'Sheet Properties', 'drawing', false),
  drawingProperties: draw('drawing_properties', 'Drawing Properties', 'properties', false),
  draftingStandard: draw('drafting_standard', 'Drafting Standard', 'options', false),
  drawingExport: draw('drawing_export', 'Export PDF/DXF', 'export', false),
  smartDimension: draw('smart_dimension', 'Smart Dimension', 'dimension', true, 'smart'),
  ordinateDimension: draw('smart_dimension', 'Ordinate Dimension', 'dimension', true, 'ordinate'),
  baselineDimension: draw('smart_dimension', 'Baseline Dimension', 'dimension', true, 'baseline'),
  chainDimension: draw('smart_dimension', 'Chain Dimension', 'dimension', true, 'chain'),
  chamferDimension: draw('smart_dimension', 'Chamfer Dimension', 'chamfer', true, 'chamfer'),
  angularRunning: draw(
    'smart_dimension',
    'Angular Running Dimension',
    'dimension',
    true,
    'angular_running',
  ),
  arcLength: draw('smart_dimension', 'Arc Length', 'arc', true, 'arc_length'),
  modelItems: draw('model_items', 'Model Items', 'modelItems'),
  note: draw('note', 'Note', 'note', false),
  balloon: draw('balloon', 'Balloon', 'balloon'),
  autoBalloon: draw('auto_balloon', 'Auto Balloon', 'balloon'),
  surfaceFinish: draw('symbol', 'Surface Finish', 'finish', false, 'surface_finish'),
  weldSymbol: draw('symbol', 'Weld Symbol', 'weld', false, 'weld'),
  geometricTolerance: draw('symbol', 'Geometric Tolerance', 'gtol', false, 'gtol'),
  datumFeature: draw('symbol', 'Datum Feature', 'datum', true, 'datum_feature'),
  datumTarget: draw('symbol', 'Datum Target', 'datum', true, 'datum_target'),
  holeCallout: draw('hole_callout', 'Hole Callout', 'callout'),
  centerMark: draw('center_mark', 'Center Mark', 'centerMark'),
  drawCenterline: draw('centerline', 'Centerline', 'centerlines'),
  areaHatch: draw('area_hatch', 'Area Hatch/Fill', 'hatchArea', false),
  revisionCloud: draw('revision_cloud', 'Revision Cloud', 'cloud', false),
  revisionSymbol: draw('revision_cloud', 'Revision Symbol', 'cloud', false, 'symbol'),
  bomTable: draw('table', 'Bill of Materials', 'table', true, 'bom'),
  holeTable: draw('table', 'Hole Table', 'table', true, 'hole'),
  revisionTable: draw('table', 'Revision Table', 'table', false, 'revision'),
  generalTable: draw('table', 'General Table', 'table', false, 'general'),
  toleranceTable: draw('table', 'General Tolerance Table', 'table', false, 'general_tolerance'),
  bendTable: draw('table', 'Bend Table', 'table', true, 'bend'),
  punchTable: draw('table', 'Punch Table', 'table', true, 'punch'),
  cutListTable: draw('table', 'Weldment Cut List', 'table', true, 'cut_list'),
  weldTable: draw('table', 'Weld Table', 'table', true, 'weld'),
  equations: panel('equations', 'Equations', 'dimension'),
  configurations: panel('configurations', 'Configurations', 'properties'),
  options: {
    label: 'Options',
    icon: 'options',
    run: (s) => s.openPanel({ type: s.drawing ? 'drafting_standard' : 'units' }),
    unavailable: (s) => idlePanel(s) ?? (s.sketch ? 'Exit the sketch first' : undefined),
  },
  sketch3d: feature('sketch3d', '3D Sketch', 'sketch'),
  extrude: feature('extrude', 'Extruded Boss/Base', 'extrude', false, true),
  revolve: feature('revolve', 'Revolved Boss/Base', 'revolve', false, true),
  sweep: feature('sweep', 'Swept Boss/Base', 'sweep', false, true),
  loft: feature('loft', 'Lofted Boss/Base', 'loft', false, true),
  boundary: feature('boundary', 'Boundary Boss/Base', 'loft', false, true),
  cut: feature('cut', 'Extruded Cut', 'cut', true, true),
  hole: feature('hole', 'Hole Wizard', 'hole', true),
  cutRevolve: feature('cut_revolve', 'Revolved Cut', 'revolve', true, true),
  sweepCut: feature('sweep_cut', 'Swept Cut', 'sweep', true, true),
  loftCut: feature('loft_cut', 'Lofted Cut', 'loft', true, true),
  boundaryCut: feature('boundary_cut', 'Boundary Cut', 'loft', true, true),
  fillet: feature('fillet', 'Fillet', 'fillet', true),
  variableFillet: feature('variable_fillet', 'Variable Fillet', 'fillet', true),
  faceFillet: feature('face_fillet', 'Face Fillet', 'fillet', true),
  chamfer: feature('chamfer', 'Chamfer', 'chamfer', true),
  linearPattern: feature('linear_pattern', 'Linear Pattern', 'pattern', true),
  circularPattern: feature('circular_pattern', 'Circular Pattern', 'circular', true),
  mirror: feature('mirror', 'Mirror', 'mirror', true),
  shell: feature('shell', 'Shell', 'shell', true),
  draft: feature('draft', 'Draft', 'draft', true),
  dome: feature('dome', 'Dome', 'dome', true),
  cosmeticThread: feature('cosmetic_thread', 'Cosmetic Thread', 'curve', true),
  thread: feature('thread', 'Thread', 'curve', true),
  rib: feature('rib', 'Rib', 'rib', true),
  wrap: feature('wrap', 'Wrap', 'wrap', true),
  intersect: feature('combine', 'Combine Bodies', 'intersect', true),
  moveBody: feature('move_body', 'Move/Copy Bodies', 'move', true),
  splitBody: feature('split_body', 'Split Bodies', 'section', true),
  plane: feature('plane', 'Plane', 'plane'),
  axis: feature('axis', 'Axis', 'axis'),
  point: feature('point', 'Point', 'point'),
  coordinates: feature('coord_sys', 'Coordinate System', 'coordinates'),
  curves: feature('helix', 'Helix', 'curve'),
  compositeCurve: feature('composite_curve', 'Composite Curve', 'curve'),
  projectedCurve: feature('projected_curve', 'Projected Curve', 'curve'),
  splitLine: feature('split_line', 'Split Line', 'curve', true),
  direct: placeholder('Direct Manipulation', 'move'),
  baseFlange: sheetMetal('base_flange', 'Base Flange/Tab', 'baseFlange', false, true),
  edgeFlange: sheetMetal('edge_flange', 'Edge Flange', 'edgeFlange'),
  miterFlange: sheetMetal('miter_flange', 'Miter Flange', 'miterFlange', true, true),
  hem: sheetMetal('hem', 'Hem', 'hem'),
  sweptFlange: sheetMetal('swept_flange', 'Swept Flange', 'sweep', false, true),
  loftedBend: sheetMetal('lofted_bend', 'Lofted-Bend', 'loft', false, true),
  sketchedBend: sheetMetal('sketched_bend', 'Sketched Bend', 'sketchedBend', true, true),
  jog: sheetMetal('jog', 'Jog', 'jog', true, true),
  crossBreak: sheetMetal('cross_break', 'Cross-Break', 'crossBreak'),
  insertBends: sheetMetal('insert_bends', 'Insert Bends', 'insertBends', false),
  convertToSheetMetal: sheetMetal(
    'convert_to_sheet_metal',
    'Convert to Sheet Metal',
    'convertSheet',
    false,
  ),
  rip: sheetMetal('rip', 'Rip', 'rip', false),
  unfold: sheetMetal('unfold', 'Unfold', 'unfold'),
  fold: sheetMetal('fold', 'Fold', 'fold'),
  normalCut: sheetMetal('normal_cut', 'Extruded Cut (Normal)', 'normalCut', true, true),
  cornerRelief: sheetMetal('corner_relief', 'Corner Relief', 'cornerRelief'),
  closedCorner: sheetMetal('closed_corner', 'Closed Corner', 'closedCorner'),
  weldedCorner: sheetMetal('welded_corner', 'Welded Corner', 'weldedCorner'),
  breakCorner: sheetMetal('break_corner', 'Break-Corner/Corner-Trim', 'breakCorner'),
  bendNotch: sheetMetal('bend_notch', 'Bend Notch', 'bendNotch'),
  tabAndSlot: sheetMetal('tab_and_slot', 'Tab and Slot', 'tabSlot'),
  sheetMetalGusset: sheetMetal('sheet_metal_gusset', 'Sheet Metal Gusset', 'gusset'),
  formTool: sheetMetal('form_tool', 'Forming Tool', 'formTool'),
  formingToolDef: sheetMetal('forming_tool', 'Make Forming Tool', 'formingToolDef', false),
  flatPattern: sheetMetal('flat_pattern', 'Flat-Pattern Options', 'flatPattern'),
  weldment: {
    label: 'Weldment',
    icon: 'weldment',
    run: () => ensureWeldment(),
    active: (s) => !!s.snapshot?.features.some((f) => f.type === 'weldment'),
    unavailable: (s) => (s.assembly ? 'Open a part to use this' : undefined) ?? idlePanel(s),
  },
  structuralMember: weld('structural_member', 'Structural Member', 'structuralMember', 'sketch'),
  trimExtend: weld('trim_extend', 'Trim/Extend', 'trimExtend'),
  endCap: weld('end_cap', 'End Cap', 'endCap'),
  weldGusset: weld('weld_gusset', 'Gusset', 'weldGusset'),
  weldBead: weld('weld_bead', 'Weld Bead', 'weldBead'),
  filletBead: weld('weld_bead', 'Fillet Bead', 'filletBead', 'body', 'fillet'),
  cutList: {
    label: 'Cut List',
    icon: 'cutList',
    run: (s) => s.openPanel({ type: 'cut_list' }),
    unavailable: (s) =>
      idlePanel(s) ??
      (!s.snapshot?.features.some((f) => f.type === 'weldment')
        ? 'Requires a weldment'
        : undefined),
  },
  flatten: {
    label: 'Flatten',
    icon: 'flatten',
    run: () => toggleFlatten(),
    active: (s) => isFlattened(s),
    unavailable: (s) =>
      (s.assembly ? 'Open a part to use this' : undefined) ??
      (s.sketch ? 'Exit the sketch first' : undefined) ??
      (!s.snapshot?.sheet_metal?.length ? 'Requires a sheet metal body' : undefined),
  },
  sketch: {
    label: 'Sketch',
    icon: 'sketch',
    shortcut: 'S',
    run: beginSketch,
    unavailable: idlePanel,
  },
  exit: { label: 'Exit Sketch', icon: 'exit', run: (s) => s.closeSketch() },
  sketchSelect: tool('select', 'Select', 'select', 'Esc'),
  line: tool('line', 'Line', 'line', 'L'),
  centerline: tool('centerline', 'Centerline', 'centerline'),
  rectangle: tool('rectangle', 'Corner Rectangle', 'rectangle', 'R'),
  centerRectangle: tool('center_rectangle', 'Center Rectangle', 'centerRectangle'),
  circle: tool('circle', 'Circle', 'circle', 'C'),
  centerArc: tool('center_arc', 'Centerpoint Arc', 'arc'),
  arc3: tool('arc3', '3 Point Arc', 'arc3', 'A'),
  tangentArc: tool('tangent_arc', 'Tangent Arc', 'tangent'),
  slot: tool('slot', 'Slot', 'slot'),
  polygon: tool('polygon', 'Polygon', 'polygon'),
  spline: tool('spline', 'Spline', 'curve', 'S'),
  sketchPoint: tool('point', 'Point', 'point', 'P'),
  dimension: tool('dimension', 'Smart Dimension', 'dimension', 'D'),
  trim: tool('trim', 'Trim', 'trim', 'T'),
  extend: tool('extend', 'Extend', 'extend', 'E'),
  sketchFillet: tool('fillet', 'Sketch Fillet', 'fillet'),
  convert: tool('convert', 'Convert Entities', 'convert'),
  measure: panel('measure', 'Measure', 'measure'),
  mass: panel('mass', 'Mass Properties', 'mass', true),
  material: panel('material', 'Material', 'material'),
  units: panel('units', 'Units', 'units'),
  fit: { ...view('fit', 'Zoom to Fit', 'fit'), shortcut: 'F' },
  front: view('front', 'Front'),
  back: view('back', 'Back'),
  left: view('left', 'Left'),
  right: view('right', 'Right'),
  top: view('top', 'Top'),
  bottom: view('bottom', 'Bottom'),
  iso: view('iso', 'Isometric'),
  normal: {
    ...view('normal', 'Normal To', 'normal'),
    unavailable: (s) =>
      !s.sketch &&
      !s.selection.some(
        (x) =>
          ['face', 'front_plane', 'top_plane', 'right_plane'].includes(x.ref.type) ||
          (x.ref.type === 'body_topology' && x.ref.kind === 'face') ||
          x.pick.kind === 'datum',
      )
        ? 'Select a plane or planar face, or edit a sketch'
        : undefined,
  },
  zoomArea: placeholder('Zoom Area', 'zoom'),
  previousView: placeholder('Previous View', 'undo'),
  section: placeholder('Section View', 'section'),
  display: placeholder('Display Style', 'extrude'),
  visibility: placeholder('Visibility', 'visibility'),
  appearance: placeholder('Appearance', 'appearance'),
  scene: placeholder('Scene', 'scene'),
  viewSettings: placeholder('View Settings', 'settings'),
  p6ExtrudedSurface: form('extruded_surface'),
  p6RevolvedSurface: form('revolved_surface'),
  p6SweptSurface: form('swept_surface'),
  p6LoftedSurface: form('lofted_surface'),
  p6BoundarySurface: form('boundary_surface'),
  p6PlanarSurface: form('planar_surface'),
  p6OffsetSurface: form('offset_surface'),
  p6FilledSurface: form('filled_surface'),
  p6RuledSurface: form('ruled_surface'),
  p6RadiateSurface: form('radiate_surface'),
  p6Knit: form('knit'),
  p6Thicken: form('thicken'),
  p6CutWithSurface: form('cut_with_surface'),
  p6TrimSurface: form('trim_surface'),
  p6UntrimSurface: form('untrim_surface'),
  p6ExtendSurface: form('extend_surface'),
  p6MidSurface: form('mid_surface'),
  p6Freeform: form('freeform'),
  p6SurfaceFlatten: form('surface_flatten'),
  p6DeleteFace: form('delete_face'),
  p6MoveFace: form('move_face'),
  p6ReplaceFace: form('replace_face'),
  p6DeleteHole: form('delete_hole'),
  p6DeleteBody: form('delete_body'),
  p6Intersect: form('intersect'),
  p6Scale: form('scale'),
  p6Flex: form('flex'),
  p6Deform: form('deform'),
  p6Indent: form('indent'),
  p6InsertPart: form('insert_part'),
  p6Defeature: form('defeature'),
  p6Slicing: form('slicing'),
  p6PartingLine: form('parting_line'),
  p6ShutOffSurfaces: form('shut_off_surfaces'),
  p6PartingSurface: form('parting_surface'),
  p6ToolingSplit: form('tooling_split'),
  p6MoldCore: form('mold_core'),
  p6ConvertToMesh: form('convert_to_mesh'),
  p6DecimateMesh: form('decimate_mesh'),
  p6SegmentMesh: form('segment_mesh'),
  p6MeshPrep: form('mesh_prep'),
  p6MeshToBrep: form('mesh_to_brep'),
  p6SurfaceFromMesh: form('surface_from_mesh'),
  p6MeshToSolid: form('mesh_to_solid'),
  p6MeshSections: form('mesh_sections'),
  draftAnalysis: analysisPanel('draft_analysis', 'Draft Analysis', 'draftAnalysis', 'solid'),
  undercutAnalysis: analysisPanel('undercut_analysis', 'Undercut Analysis', 'undercut', 'solid'),
  deviationAnalysis: analysisPanel('deviation_analysis', 'Deviation Analysis', 'deviation', 'mesh'),
  compareBodies: analysisPanel('compare_bodies', 'Body Compare', 'compareBodies', 'body'),
  projectedArea: analysisPanel('projected_area', 'Projected Area', 'measure', 'body'),
  saveBodies: analysisPanel('save_bodies', 'Save Bodies', 'saveBodies', 'body'),
  mirrorPart: analysisPanel('mirror_part', 'Mirror Part', 'mirrorPart', 'body'),
  asmDefeature: asm('asm_defeature', 'Defeature', 'defeature'),
  instant3d: {
    label: 'Instant3D',
    icon: 'instant3d',
    run: (s) => s.toggleInstant3d(),
    active: (s) => s.instant3d,
    unavailable: (s) => (s.assembly ? 'Open a part to use this' : undefined),
  },
  importMesh: {
    label: 'Import Mesh or Scan',
    icon: 'meshBody',
    run: (s) => s.importFile(),
    unavailable: (s) => (s.assembly ? 'Open a part to use this' : idlePanel(s)),
  },
} satisfies Record<string, Definition>

export type CommandId = keyof typeof commands
export const SKETCH_KEYS: Record<string, SketchTool> = {
  l: 'line',
  r: 'rectangle',
  c: 'circle',
  a: 'arc3',
  s: 'spline',
  p: 'point',
  d: 'dimension',
  t: 'trim',
  e: 'extend',
}

// Shared across toolbar instances so dialog/transition promises cannot overlap.
const useOperation = create<{ pending: boolean }>(() => ({ pending: false }))
export function commandReason(id: CommandId, s: State, pending: boolean): string | undefined {
  const c: Definition = commands[id]
  if (!c.run) return 'Not implemented yet'
  if (!c.view && (s.busy || pending)) return 'An operation is in progress'
  return c.unavailable?.(s)
}
export function useCommands() {
  const state = useStore()
  const pending = useOperation((s) => s.pending)
  return (id: CommandId) => {
    const c: Definition = commands[id]
    return {
      ...c,
      id,
      reason: commandReason(id, state, pending),
      active: c.active?.(state) ?? false,
    }
  }
}
export async function executeCommand(id: CommandId) {
  const s = useStore.getState()
  if (commandReason(id, s, useOperation.getState().pending)) return
  const c: Definition = commands[id]
  if (!c.view) useOperation.setState({ pending: true })
  try {
    await c.run?.(s)
  } catch (error) {
    s.toast(error instanceof Error ? error.message : String(error))
  } finally {
    if (!c.view) useOperation.setState({ pending: false })
  }
}
