import { create } from 'zustand'
import { useStore, type PanelType, type SketchTool, type ViewName } from '../store'
import { beginSketch, startFeature } from './actions'
import type { IconId } from './icons'

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
    unavailable: (s) => (s.assembly ? 'Use Import STEP as Assembly' : part(s)),
  },
  export: {
    label: 'Export',
    icon: 'export',
    run: (s) => s.openPanel({ type: 'export' }),
    unavailable: (s) =>
      s.assembly
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
  print: placeholder('Print', 'print'),
  rebuild: placeholder('Rebuild', 'rebuild'),
  properties: placeholder('File Properties', 'properties'),
  equations: panel('equations', 'Equations', 'dimension'),
  configurations: panel('configurations', 'Configurations', 'properties'),
  options: placeholder('Options', 'options'),
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
