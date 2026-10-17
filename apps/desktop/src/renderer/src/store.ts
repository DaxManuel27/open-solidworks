import { create } from 'zustand'
import { CallFailed, call } from './core'
import type { DrawingSnapshot } from './drawing/types'
import { syncSheet, useDrawing } from './drawing/state'
import type {
  AsmGeometry,
  AsmInstance,
  AsmScene,
  AsmSnapshot,
  CompRef,
  CoreInfo,
  FeatureParams,
  Kind,
  MeshPayload,
  PreviewResult,
  Ref,
  Scene,
  SketchView,
  Snapshot,
} from './protocol'

/** Something picked in the viewport. */
export type Pick =
  | { kind: Kind; index: number }
  | { kind: 'datum'; id: number; ref?: Ref }
  | { kind: 'std_plane'; ref: Ref }

export type PickKind = Pick['kind']

/** Something picked on an assembly component. */
export interface AsmPick {
  component: string[]
  kind: Kind
  index: number
}

/** A picked assembly reference with its world geometry. */
export interface AsmSelected {
  pick: AsmPick | null
  ref: CompRef
  geometry: AsmGeometry | null
}

/** A panel that wants assembly picks routed to it. */
export interface AsmPickTarget {
  kinds: (Kind | 'component')[]
  onPick: (selected: AsmSelected) => void
}

/** Things to mark in the assembly: collision faces and interference volumes. */
export interface AsmMarks {
  faces: { component: string[]; faces: number[] }[]
  meshes: MeshPayload[]
}

export type DragPhase = 'start' | 'move' | 'end'

/** A panel that wants viewport picks routed to it. */
export interface PickTarget {
  kinds: PickKind[]
  onPick: (pick: Pick, ref: Ref) => void
}

export type PanelType =
  | 'structural_member'
  | 'trim_extend'
  | 'end_cap'
  | 'weld_gusset'
  | 'weld_bead'
  | 'cut_list'
  | 'base_flange'
  | 'edge_flange'
  | 'miter_flange'
  | 'hem'
  | 'jog'
  | 'sketched_bend'
  | 'swept_flange'
  | 'lofted_bend'
  | 'cross_break'
  | 'insert_bends'
  | 'convert_to_sheet_metal'
  | 'rip'
  | 'unfold'
  | 'fold'
  | 'normal_cut'
  | 'corner_relief'
  | 'closed_corner'
  | 'welded_corner'
  | 'break_corner'
  | 'bend_notch'
  | 'tab_and_slot'
  | 'sheet_metal_gusset'
  | 'form_tool'
  | 'forming_tool'
  | 'flat_pattern'
  | 'dome'
  | 'rib'
  | 'thread'
  | 'face_fillet'
  | 'wrap'
  | 'cosmetic_thread'
  | 'sketch3d'
  | 'draft'
  | 'variable_fillet'
  | 'sweep'
  | 'sweep_cut'
  | 'loft'
  | 'loft_cut'
  | 'boundary'
  | 'boundary_cut'
  | 'helix'
  | 'composite_curve'
  | 'split_line'
  | 'projected_curve'
  | 'configurations'
  | 'equations'
  | 'combine'
  | 'move_body'
  | 'split_body'
  | 'extrude'
  | 'cut'
  | 'revolve'
  | 'cut_revolve'
  | 'fillet'
  | 'chamfer'
  | 'shell'
  | 'hole'
  | 'linear_pattern'
  | 'circular_pattern'
  | 'mirror'
  | 'plane'
  | 'axis'
  | 'point'
  | 'coord_sys'
  | 'measure'
  | 'mass'
  | 'material'
  | 'units'
  | 'export'
  | 'asm_insert'
  | 'hardware'
  | 'mate'
  | 'move_component'
  | 'rotate_component'
  | 'component_pattern'
  | 'mirror_components'
  | 'explode'
  | 'interference'
  | 'clearance'
  | 'bom'
  | 'component_properties'
  | 'asm_configurations'
  | 'asm_reference'
  | 'asm_measure'
  | 'asm_mass'
  | 'sheet_properties'
  | 'drawing_properties'
  | 'part_properties'
  | 'standard_views'
  | 'model_view'
  | 'projected_view'
  | 'auxiliary_view'
  | 'section_view'
  | 'detail_view'
  | 'crop_view'
  | 'broken_out'
  | 'break_view'
  | 'view_properties'
  | 'smart_dimension'
  | 'model_items'
  | 'note'
  | 'balloon'
  | 'auto_balloon'
  | 'symbol'
  | 'hole_callout'
  | 'center_mark'
  | 'centerline'
  | 'area_hatch'
  | 'revision_cloud'
  | 'table'
  | 'drafting_standard'
  | 'drawing_export'
  | 'annotation_properties'

export interface Panel {
  type: PanelType
  /** Feature (or mate, pattern, mirror) being edited, if not creating a new one. */
  editing?: number
  /** Sketch to start a new feature from. */
  sketch?: number
  /** Component a component panel is about. */
  component?: string[]
  /** Pattern kind or mate type a panel starts with. */
  variant?: string
}

export interface Toast {
  id: number
  text: string
  kind: 'error' | 'info'
}

export type Tab =
  | 'features'
  | 'sheet_metal'
  | 'weldments'
  | 'sketch'
  | 'evaluate'
  | 'assembly'
  | 'drawing'
  | 'annotation'

export type ViewName =
  'front' | 'back' | 'left' | 'right' | 'top' | 'bottom' | 'iso' | 'normal' | 'fit'

export type SketchTool =
  | 'select'
  | 'line'
  | 'centerline'
  | 'rectangle'
  | 'center_rectangle'
  | 'circle'
  | 'center_arc'
  | 'arc3'
  | 'tangent_arc'
  | 'slot'
  | 'polygon'
  | 'spline'
  | 'point'
  | 'dimension'
  | 'trim'
  | 'extend'
  | 'fillet'
  | 'convert'

interface State {
  snapshot: Snapshot | null
  scene: Scene | null
  previewScene: Scene | null
  previewError: string | null
  /** Sketch being edited. */
  sketch: SketchView | null
  sketchTool: SketchTool
  /** Selected sketch entity and constraint ids. */
  sketchSelection: number[]
  sketchFilletRadius: number
  polygonSides: number
  panel: Panel | null
  /** General selection in part mode. */
  selection: { pick: Pick; ref: Ref }[]
  /** Extra items to highlight (from panel selection boxes), as kernel indices. */
  highlights: Pick[]
  pickTarget: PickTarget | null
  treeSelection: number | null
  hoverFeature: number | null
  toasts: Toast[]
  busy: boolean
  /** Bumped to ask the viewport to change view. */
  view: { name: ViewName; seq: number }
  status: string
  /** CommandManager tab. */
  tab: Tab
  /** The open assembly, when the document is one; `snapshot` then mirrors its file state. */
  assembly: AsmSnapshot | null
  asmScene: AsmScene | null
  /** Instance positions to show instead of the solved ones (mate preview, drags). */
  asmPreview: AsmInstance[] | null
  /** Picked faces, edges and vertices. */
  asmSelection: AsmSelected[]
  /** Selected components (tree or viewport). */
  asmComponents: string[][]
  asmTarget: AsmPickTarget | null
  asmMarks: AsmMarks
  /** The open drawing, when the document is one; `snapshot` then mirrors its file state. */
  drawing: DrawingSnapshot | null
}

interface Actions {
  init(): Promise<void>
  run(method: string, params?: unknown): Promise<Snapshot | null>
  toast(text: string, kind?: Toast['kind']): void
  dismiss(id: number): void
  setView(name: ViewName): void
  setStatus(text: string): void
  setTab(tab: Tab): void

  newDoc(): Promise<void>
  open(): Promise<void>
  save(as?: boolean): Promise<boolean>
  importFile(): Promise<void>
  exportFile(): Promise<void>
  undo(): Promise<void>
  redo(): Promise<void>

  pick(pick: Pick, additive: boolean): Promise<void>
  clearSelection(): void
  setHighlights(refs: Ref[]): Promise<void>
  setPickTarget(t: PickTarget | null): void
  selectFeature(id: number | null): void

  openPanel(p: Panel): void
  closePanel(): Promise<void>
  preview(params: FeatureParams | null, editing?: number): Promise<void>
  commit(
    params: FeatureParams,
    editing?: number,
    formulas?: Record<string, string | null>,
  ): Promise<boolean>

  startSketch(plane: Ref): Promise<void>
  editSketch(id: number): Promise<void>
  sketchEdit(op: Record<string, unknown>): Promise<SketchView | null>
  sketchDrag(point: number, to: [number, number]): Promise<void>
  closeSketch(): Promise<void>
  setSketchTool(t: SketchTool): void
  setSketchSelection(ids: number[]): void

  newAssembly(): Promise<void>
  importAssembly(): Promise<void>
  /** Pick on an assembly component: routed to the active panel, else selected. */
  asmPick(pick: AsmPick, additive: boolean): Promise<void>
  selectComponents(keys: string[][], additive?: boolean): void
  setAsmTarget(t: AsmPickTarget | null): void
  setAsmMarks(marks: AsmMarks): void
  /** Move a component; `move` phases preview without recording undo. */
  asmMove(
    component: string[],
    motion: Record<string, unknown>,
    phase: DragPhase,
    collision?: { dragged_only?: boolean } | null,
  ): Promise<void>
  openComponent(key: string[]): Promise<void>
  /** Take an assembly result as the current document state. */
  adoptAssembly(a: AsmSnapshot): Snapshot
  /** Take a drawing result as the current document state. */
  adoptDrawing(d: DrawingSnapshot): Snapshot
  newDrawing(): Promise<void>
  /** Open a new drawing window of the saved part or assembly. */
  makeDrawing(): Promise<void>
  print(): Promise<void>
}

let toastId = 1
let previewSequence = 0
let dragInFlight = false
let pendingDrag: { point: number; to: [number, number] } | null = null

function fileName(path: string | null, fallback = 'Part1'): string {
  if (!path) return fallback
  return (
    path
      .split(/[\\/]/)
      .pop()
      ?.replace(/\.of[pad]$/i, '') ?? fallback
  )
}

/** The part-shaped file state the shell reads while an assembly is open. */
function asPartSnapshot(a: AsmSnapshot): Snapshot {
  return {
    generation: a.generation,
    configurations: { active: 1, next_id: 2, items: [] },
    variables: {},
    equations: {},
    equation_values: {},
    parameters: [],
    revision: a.revision,
    features: [],
    rollback: null,
    units: a.units,
    material: null,
    effective_material: null,
    path: a.path,
    dirty: a.dirty,
    can_undo: a.can_undo,
    can_redo: a.can_redo,
    editing: null,
  }
}

/** The part-shaped file state the shell reads while a drawing is open. */
function drawingAsPart(d: DrawingSnapshot): Snapshot {
  return {
    generation: '',
    configurations: { active: 1, next_id: 2, items: [] },
    variables: {},
    equations: {},
    equation_values: {},
    parameters: [],
    revision: d.revision,
    features: [],
    rollback: null,
    // Drawings are dimensioned per their drafting standard; panels use mm.
    units: { length: 'mm', angle: 'deg', mass: 'kg', decimals: 2 },
    material: null,
    effective_material: null,
    path: d.path,
    dirty: d.dirty,
    can_undo: d.can_undo,
    can_redo: d.can_redo,
    editing: null,
  }
}

let moveInFlight = false
let pendingMove: {
  component: string[]
  motion: Record<string, unknown>
  phase: DragPhase
  collision?: { dragged_only?: boolean } | null
} | null = null

export const useStore = create<State & Actions>((set, get) => ({
  snapshot: null,
  scene: null,
  previewScene: null,
  previewError: null,
  sketch: null,
  sketchTool: 'select',
  sketchSelection: [],
  sketchFilletRadius: 2,
  polygonSides: 6,
  panel: null,
  selection: [],
  highlights: [],
  pickTarget: null,
  treeSelection: null,
  hoverFeature: null,
  toasts: [],
  busy: false,
  view: { name: 'iso', seq: 0 },
  status: '',
  tab: 'features',
  assembly: null,
  asmScene: null,
  asmPreview: null,
  asmSelection: [],
  asmComponents: [],
  asmTarget: null,
  asmMarks: { faces: [], meshes: [] },
  drawing: null,

  async init() {
    try {
      const info = await call<CoreInfo>('core.info')
      if (info.protocol_version !== 5 || info.scene_version !== 4)
        throw new Error(
          'The core and desktop versions do not match. Rebuild both before opening a part.',
        )
      const launch = window.openforge.launch()
      // Restarting after a crash shows the core's (empty) document instead.
      if (!get().snapshot && launch.open) {
        if (await get().run('doc.open', { path: launch.open })) get().setView('fit')
        else await get().run('doc.info')
      } else if (!get().snapshot && launch.kind === 'assembly') {
        await get().run('asm.new')
      } else if (!get().snapshot && launch.kind === 'drawing') {
        if (!(await get().run('drawing.new', launch.model ? { model: launch.model } : {})))
          await get().run('drawing.new')
      } else {
        await get().run('doc.info')
      }
    } catch (e) {
      get().toast((e as Error).message)
    }
  },

  async run(method, params) {
    ++previewSequence
    set({ busy: true, previewScene: null, previewError: null })
    try {
      const result = await call<Snapshot | AsmSnapshot | DrawingSnapshot>(method, params)
      if ((result as AsmSnapshot).kind === 'assembly')
        return get().adoptAssembly(result as AsmSnapshot)
      if ((result as DrawingSnapshot).kind === 'drawing')
        return get().adoptDrawing(result as DrawingSnapshot)
      const snap = result as Snapshot
      if (get().drawing) {
        set({ drawing: null, tab: 'features', panel: null })
        useDrawing.setState({ sheet: null, render: null, selection: [], target: null })
      }
      if (get().assembly) {
        // Back to a part: drop every assembly state.
        set({
          assembly: null,
          asmScene: null,
          asmPreview: null,
          asmSelection: [],
          asmComponents: [],
          asmTarget: null,
          asmMarks: { faces: [], meshes: [] },
          tab: 'features',
        })
      } else if (snap.revision < (get().snapshot?.revision ?? -1)) return null
      const update: Partial<State> = { snapshot: snap }
      if (method.startsWith('configuration.')) {
        Object.assign(update, {
          sketch: null,
          panel: null,
          selection: [],
          highlights: [],
          pickTarget: null,
          treeSelection: null,
        })
      }
      if (snap.scene) update.scene = snap.scene
      set(update)
      const title = `${fileName(snap.path)}${snap.dirty ? ' •' : ''} — OpenForge`
      window.openforge.setDocState(title, snap.dirty, snap.path)
      return snap
    } catch (e) {
      get().toast(e instanceof CallFailed ? e.message : String(e))
      return null
    } finally {
      set({ busy: false })
    }
  },

  toast(text, kind = 'error') {
    const id = toastId++
    set((s) => ({ toasts: [...s.toasts, { id, text, kind }] }))
    setTimeout(() => get().dismiss(id), kind === 'error' ? 6000 : 3000)
  },

  dismiss(id) {
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }))
  },

  setView(name) {
    set((s) => ({ view: { name, seq: s.view.seq + 1 } }))
  },

  setStatus(text) {
    set({ status: text })
  },

  setTab(tab) {
    set({ tab })
  },

  async newDoc() {
    if (get().snapshot?.dirty && !confirm('Discard unsaved changes?')) return
    set({ sketch: null, panel: null, selection: [], previewScene: null })
    await get().run('doc.new')
    get().setView('iso')
  },

  async open() {
    if (get().snapshot?.dirty && !confirm('Discard unsaved changes?')) return
    const path = await window.openforge.openDialog('part')
    if (!path) return
    set({ sketch: null, panel: null, selection: [], previewScene: null })
    if (await get().run('doc.open', { path })) get().setView('fit')
  },

  async save(as = false) {
    let path = get().snapshot?.path ?? null
    const assembly = !!get().assembly
    if (as || !path) {
      path = get().drawing
        ? await window.openforge.saveDialog('drawing', `${fileName(path, 'Draw1')}.ofd`)
        : assembly
          ? await window.openforge.saveDialog('assembly', `${fileName(path, 'Assembly1')}.ofa`)
          : await window.openforge.saveDialog('part', `${fileName(path)}.ofp`)
      if (!path) return false
    }
    const r = await get().run('doc.save', { path })
    if (r) {
      get().toast('Saved', 'info')
      window.openforge.notifySaved(path)
    }
    return !!r
  },

  async importFile() {
    const path = await window.openforge.openDialog('import')
    if (!path) return
    if (await get().run('import', { path })) get().setView('fit')
  },

  async exportFile() {
    get().openPanel({ type: 'export' })
  },

  async undo() {
    if (get().sketch) {
      await get().run('doc.undo')
      const id = get().sketch?.id
      if (id !== undefined && get().snapshot?.features.some((f) => f.id === id)) {
        set({ sketch: await call<SketchView>('sketch.get', { id }) })
      } else {
        set({ sketch: null })
      }
      return
    }
    await get().run('doc.undo')
  },

  async redo() {
    await get().run('doc.redo')
    const id = get().sketch?.id
    if (id !== undefined) set({ sketch: await call<SketchView>('sketch.get', { id }) })
  },

  async pick(pick, additive) {
    const target = get().pickTarget
    let ref: Ref
    if (pick.kind === 'datum') ref = pick.ref ?? { type: 'feature', id: pick.id }
    else if (pick.kind === 'std_plane') ref = pick.ref
    else {
      try {
        ref = (await call<{ ref: Ref }>('select.ref', { kind: pick.kind, index: pick.index })).ref
      } catch (e) {
        get().toast(String((e as Error).message))
        return
      }
    }
    if (target) {
      if (target.kinds.includes(pick.kind)) target.onPick(pick, ref)
      else get().toast(`Select ${target.kinds.map(describeKind).join(' or ')}`, 'info')
      return
    }
    const key = JSON.stringify(ref)
    set((s) => {
      const exists = s.selection.some((x) => JSON.stringify(x.ref) === key)
      if (additive) {
        return {
          selection: exists
            ? s.selection.filter((x) => JSON.stringify(x.ref) !== key)
            : [...s.selection, { pick, ref }],
        }
      }
      return { selection: [{ pick, ref }], treeSelection: null }
    })
  },

  clearSelection() {
    set({ selection: [], treeSelection: null })
  },

  async setHighlights(refs) {
    const out: Pick[] = []
    for (const r of refs) {
      if (r.type === 'curve_segment') {
        out.push({ kind: 'datum', id: r.curve, ref: r })
        continue
      }
      if (r.type === 'feature') {
        out.push({ kind: 'datum', id: r.id })
        continue
      }
      if (!['face', 'edge', 'vertex', 'body_topology'].includes(r.type)) continue
      try {
        const loc = await call<{ kind: Kind; index: number } | null>('select.locate', { ref: r })
        if (loc) out.push(loc)
      } catch {
        // A reference that no longer resolves just isn't highlighted.
      }
    }
    set({ highlights: out })
  },

  setPickTarget(t) {
    set({ pickTarget: t })
  },

  selectFeature(id) {
    set({ treeSelection: id, selection: [] })
  },

  openPanel(p) {
    ++previewSequence
    set({ panel: p, previewScene: null, previewError: null, highlights: [], pickTarget: null })
    // Drawing panels edit views and annotations, not features.
    if (p.editing !== undefined && !get().drawing) void get().run('edit.begin', { id: p.editing })
  },

  async closePanel() {
    ++previewSequence
    const editing = get().panel?.editing
    set({ panel: null, previewScene: null, previewError: null, highlights: [], pickTarget: null })
    if (!get().drawing) await call('feature.preview', { feature: null })
    if (editing !== undefined && !get().drawing) await get().run('edit.end')
  },

  async preview(params, editing) {
    const sequence = ++previewSequence
    const revision = get().snapshot?.revision
    const panel = get().panel
    const current = () =>
      sequence === previewSequence && get().panel === panel && get().snapshot?.revision === revision
    try {
      const r = await call<PreviewResult>('feature.preview', { feature: params, editing, revision })
      if (current() && r.revision === revision)
        set({ previewScene: r.scene, previewError: r.error })
    } catch (e) {
      if (current()) set({ previewScene: null, previewError: (e as Error).message })
    }
  },

  async commit(params, editing, formulas = {}) {
    // A new feature has no equations to clear.
    const sent =
      editing !== undefined
        ? formulas
        : Object.fromEntries(Object.entries(formulas).filter(([, f]) => f !== null))
    const snap =
      editing !== undefined
        ? await get().run('feature.update', { id: editing, feature: params, formulas: sent })
        : await get().run('feature.add', { feature: params, formulas: sent })
    if (!snap) return false
    set({
      panel: null,
      previewScene: null,
      previewError: null,
      highlights: [],
      pickTarget: null,
      selection: [],
    })
    const changed = editing ?? snap.features[snap.features.length - 1]?.id
    const f = snap.features.find((x) => x.id === changed)
    if (f?.error) get().toast(`${f.name}: ${f.error}`)
    return true
  },

  async startSketch(plane) {
    try {
      const r = await call<Snapshot & { id: number; sketch: SketchView }>('sketch.new', { plane })
      set({
        snapshot: r,
        scene: r.scene ?? get().scene,
        sketch: r.sketch,
        sketchTool: 'line',
        tab: 'sketch',
        sketchSelection: [],
        selection: [],
        panel: null,
      })
      get().setView('normal')
    } catch (e) {
      get().toast((e as Error).message)
    }
  },

  async editSketch(id) {
    const snap = await get().run('edit.begin', { id })
    if (!snap) return
    try {
      const sketch = await call<SketchView>('sketch.get', { id })
      set({
        sketch,
        sketchTool: 'select',
        sketchSelection: [],
        selection: [],
        panel: null,
        tab: 'sketch',
      })
      get().setView('normal')
    } catch (e) {
      get().toast((e as Error).message)
    }
  },

  async sketchEdit(op) {
    const id = get().sketch?.id
    if (id === undefined) return null
    try {
      const v = await call<SketchView>('sketch.edit', { id, op })
      set({ sketch: v })
      if (op['op'] !== 'drag')
        set({ snapshot: { ...get().snapshot!, revision: v.revision, dirty: true, can_undo: true } })
      return v
    } catch (e) {
      get().toast((e as Error).message)
      return null
    }
  },

  async sketchDrag(point, to) {
    // Coalesce drags: only one request in flight, the latest position wins.
    pendingDrag = { point, to }
    if (dragInFlight) return
    dragInFlight = true
    while (pendingDrag) {
      const d = pendingDrag
      pendingDrag = null
      await get().sketchEdit({ op: 'drag', point: d.point, to: d.to })
    }
    dragInFlight = false
  },

  async closeSketch() {
    set({ sketch: null, sketchSelection: [], sketchTool: 'select', tab: 'features' })
    await get().run('sketch.close')
  },

  setSketchTool(t) {
    set({ sketchTool: t, sketchSelection: t === 'select' ? get().sketchSelection : [] })
  },

  setSketchSelection(ids) {
    set({ sketchSelection: ids })
  },

  async newAssembly() {
    if (get().snapshot?.dirty && !confirm('Discard unsaved changes?')) return
    set({ sketch: null, panel: null, selection: [], previewScene: null })
    await get().run('asm.new')
    get().setView('iso')
  },

  async importAssembly() {
    if (get().snapshot?.dirty && !confirm('Discard unsaved changes?')) return
    const path = await window.openforge.openDialog('step')
    if (!path) return
    set({ sketch: null, panel: null, selection: [], previewScene: null })
    if (await get().run('asm.import_step', { path })) get().setView('fit')
  },

  async asmPick(pick, additive) {
    const target = get().asmTarget
    if (target?.kinds.includes('component') && !target.kinds.includes(pick.kind)) {
      target.onPick({
        pick,
        ref: { component: pick.component, entity: { type: 'origin' } },
        geometry: null,
      })
      return
    }
    let picked: { ref: CompRef; geometry: AsmGeometry | null }
    try {
      picked = await call('asm.pick', pick)
    } catch (e) {
      get().toast((e as Error).message)
      return
    }
    const selected: AsmSelected = { pick, ...picked }
    if (target) {
      if (target.kinds.includes(pick.kind)) target.onPick(selected)
      else
        get().toast(
          `Select ${target.kinds.map((k) => (k === 'component' ? 'a component' : describeKind(k))).join(' or ')}`,
          'info',
        )
      return
    }
    const key = JSON.stringify(selected.ref)
    set((s) => {
      const exists = s.asmSelection.some((x) => JSON.stringify(x.ref) === key)
      if (additive)
        return {
          asmSelection: exists
            ? s.asmSelection.filter((x) => JSON.stringify(x.ref) !== key)
            : [...s.asmSelection, selected],
        }
      return { asmSelection: [selected], asmComponents: [pick.component.slice(0, 1)] }
    })
  },

  selectComponents(keys, additive = false) {
    set((s) => {
      if (!additive) return { asmComponents: keys, asmSelection: [] }
      const out = [...s.asmComponents]
      for (const k of keys) {
        const i = out.findIndex((x) => x.join('/') === k.join('/'))
        if (i >= 0) out.splice(i, 1)
        else out.push(k)
      }
      return { asmComponents: out }
    })
  },

  setAsmTarget(t) {
    set({ asmTarget: t })
  },

  setAsmMarks(marks) {
    set({ asmMarks: marks })
  },

  async asmMove(component, motion, phase, collision) {
    // Coalesce drags: one request in flight, the latest target wins; start
    // and end always go through.
    pendingMove = { component, motion, phase, collision }
    if (moveInFlight) return
    moveInFlight = true
    try {
      while (pendingMove) {
        const m = pendingMove
        pendingMove = null
        const params = {
          component: m.component,
          motion: m.motion,
          phase: m.phase,
          ...(m.collision ? { collision: m.collision } : {}),
        }
        if (m.phase === 'end') {
          try {
            const r = await call<
              AsmSnapshot & {
                collision: {
                  components: string[][] | null
                  faces: { component: string[]; faces: number[] }[]
                }
              }
            >('asm.move', params)
            get().adoptAssembly(r)
            set({ asmPreview: null, asmMarks: { faces: r.collision.faces, meshes: [] } })
            get().setStatus(r.collision.components ? 'Stopped at a collision' : '')
          } catch (e) {
            set({ asmPreview: null })
            get().toast((e as Error).message)
          }
          continue
        }
        try {
          const r = await call<{
            instances: AsmInstance[]
            collision: {
              components: string[][] | null
              faces: { component: string[]; faces: number[] }[]
            }
          }>('asm.move', params)
          set({
            asmPreview: r.instances,
            asmMarks: { faces: r.collision.faces, meshes: [] },
          })
          if (r.collision.components) get().setStatus('Collision: the components touch here')
          else get().setStatus('')
        } catch (e) {
          get().toast((e as Error).message)
          pendingMove = null
        }
      }
    } finally {
      moveInFlight = false
    }
  },

  adoptAssembly(a) {
    const snap = asPartSnapshot(a)
    const switched = !get().assembly
    set({
      assembly: a,
      snapshot: snap,
      scene: null,
      ...(a.scene ? { asmScene: a.scene, asmPreview: null } : {}),
      ...(switched
        ? {
            tab: 'assembly' as Tab,
            sketch: null,
            selection: [],
            asmSelection: [],
            asmComponents: [],
          }
        : {}),
    })
    const title = `${fileName(a.path, 'Assembly1')}${a.dirty ? ' •' : ''} — OpenForge`
    window.openforge.setDocState(title, a.dirty, a.path)
    return snap
  },

  adoptDrawing(d) {
    const snap = drawingAsPart(d)
    const switched = !get().drawing
    if (!switched && d.revision < (get().drawing?.revision ?? -1)) return snap
    set({
      drawing: d,
      snapshot: snap,
      scene: null,
      ...(switched
        ? {
            tab: 'drawing' as Tab,
            // Panels of the previous document do not apply.
            panel: null,
            pickTarget: null,
            previewScene: null,
            sketch: null,
            selection: [],
            assembly: null,
            asmScene: null,
            asmPreview: null,
            asmSelection: [],
            asmComponents: [],
            asmTarget: null,
          }
        : {}),
    })
    syncSheet(d)
    const title = `${fileName(d.path, 'Draw1')}${d.dirty ? ' •' : ''} — OpenForge`
    window.openforge.setDocState(title, d.dirty, d.path)
    return snap
  },

  async newDrawing() {
    if (get().snapshot?.dirty && !confirm('Discard unsaved changes?')) return
    set({ sketch: null, panel: null, selection: [], previewScene: null })
    await get().run('drawing.new')
  },

  async makeDrawing() {
    const s = get()
    if (s.drawing) {
      const path = await window.openforge.openDialog('model')
      if (path) window.openforge.openDrawing(path)
      return
    }
    let path = s.snapshot?.path ?? null
    if (!path || s.snapshot?.dirty) {
      if (!confirm('The drawing shows the saved file. Save it now?')) return
      if (!(await s.save())) return
      path = get().snapshot?.path ?? null
    }
    if (path) window.openforge.openDrawing(path)
  },

  async print() {
    if (!get().drawing) {
      get().toast('Printing is for drawings: make a drawing of this model first', 'info')
      return
    }
    try {
      const path = await window.openforge.printPath()
      await call('drawing.export', { path })
      await window.openforge.openPrint(path)
      get().toast('Opened the drawing as a PDF to print at true size', 'info')
    } catch (e) {
      get().toast((e as Error).message)
    }
  },

  async openComponent(key) {
    try {
      const r = await call<{ path: string }>('asm.component.open', { component: key })
      window.openforge.openWindow(r.path)
    } catch (e) {
      get().toast((e as Error).message)
    }
  },
}))

export function describeKind(k: PickKind): string {
  switch (k) {
    case 'face':
      return 'a face'
    case 'edge':
      return 'an edge'
    case 'vertex':
      return 'a vertex'
    case 'datum':
      return 'a reference plane or axis'
    case 'std_plane':
      return 'a plane'
  }
}
