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
const body = (s: State) => (!s.scene?.body ? 'Requires a solid body' : undefined)
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
  open: { label: 'Open', icon: 'open', shortcut: 'Ctrl/Cmd+O', run: (s) => s.open() },
  save: { label: 'Save', icon: 'save', shortcut: 'Ctrl/Cmd+S', run: (s) => s.save() },
  saveAs: {
    label: 'Save As',
    icon: 'save',
    shortcut: 'Ctrl/Cmd+Shift+S',
    run: (s) => s.save(true),
  },
  import: { label: 'Import', icon: 'open', run: (s) => s.importFile(), unavailable: part },
  export: panel('export', 'Export', 'export', true),
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
  options: placeholder('Options', 'options'),
  extrude: feature('extrude', 'Extruded Boss/Base', 'extrude', false, true),
  revolve: feature('revolve', 'Revolved Boss/Base', 'revolve', false, true),
  sweep: placeholder('Swept Boss/Base', 'sweep'),
  loft: placeholder('Lofted Boss/Base', 'loft'),
  boundary: placeholder('Boundary Boss/Base', 'loft'),
  cut: feature('cut', 'Extruded Cut', 'cut', true, true),
  hole: feature('hole', 'Hole Wizard', 'hole', true),
  cutRevolve: feature('cut_revolve', 'Revolved Cut', 'revolve', true, true),
  sweepCut: placeholder('Swept Cut', 'sweep'),
  loftCut: placeholder('Lofted Cut', 'loft'),
  boundaryCut: placeholder('Boundary Cut', 'loft'),
  fillet: feature('fillet', 'Fillet', 'fillet', true),
  chamfer: feature('chamfer', 'Chamfer', 'chamfer', true),
  linearPattern: feature('linear_pattern', 'Linear Pattern', 'pattern', true),
  circularPattern: feature('circular_pattern', 'Circular Pattern', 'circular', true),
  mirror: feature('mirror', 'Mirror', 'mirror', true),
  shell: feature('shell', 'Shell', 'shell', true),
  draft: placeholder('Draft', 'draft'),
  rib: placeholder('Rib', 'rib'),
  wrap: placeholder('Wrap', 'wrap'),
  intersect: placeholder('Intersect', 'intersect'),
  plane: feature('plane', 'Plane', 'plane'),
  axis: feature('axis', 'Axis', 'axis'),
  point: feature('point', 'Point', 'point'),
  coordinates: feature('coord_sys', 'Coordinate System', 'coordinates'),
  curves: placeholder('Curves', 'curve'),
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
