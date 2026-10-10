import { create } from 'zustand'
import { CallFailed, call } from './core'
import type { FeatureParams, Kind, Ref, Scene, SketchView, Snapshot } from './protocol'

/** Something picked in the viewport. */
export type Pick =
  { kind: Kind; index: number } | { kind: 'datum'; id: number } | { kind: 'std_plane'; ref: Ref }

export type PickKind = Pick['kind']

/** A panel that wants viewport picks routed to it. */
export interface PickTarget {
  kinds: PickKind[]
  onPick: (pick: Pick, ref: Ref) => void
}

export type PanelType =
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

export interface Panel {
  type: PanelType
  /** Feature being edited, if not creating a new one. */
  editing?: number
  /** Sketch to start a new feature from. */
  sketch?: number
}

export interface Toast {
  id: number
  text: string
  kind: 'error' | 'info'
}

export type Tab = 'features' | 'sketch' | 'evaluate'

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
  highlights: { kind: Kind; index: number }[]
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
  commit(params: FeatureParams, editing?: number): Promise<boolean>

  startSketch(plane: Ref): Promise<void>
  editSketch(id: number): Promise<void>
  sketchEdit(op: Record<string, unknown>): Promise<SketchView | null>
  sketchDrag(point: number, to: [number, number]): Promise<void>
  closeSketch(): Promise<void>
  setSketchTool(t: SketchTool): void
  setSketchSelection(ids: number[]): void
}

let toastId = 1
let dragInFlight = false
let pendingDrag: { point: number; to: [number, number] } | null = null

function fileName(path: string | null): string {
  if (!path) return 'Part1'
  return (
    path
      .split(/[\\/]/)
      .pop()
      ?.replace(/\.ofp$/i, '') ?? 'Part1'
  )
}

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

  async init() {
    await get().run('doc.info')
  },

  async run(method, params) {
    set({ busy: true })
    try {
      const snap = await call<Snapshot>(method, params)
      const update: Partial<State> = { snapshot: snap }
      if (snap.scene) update.scene = snap.scene
      set(update)
      const title = `${fileName(snap.path)}${snap.dirty ? ' •' : ''} — OpenForge`
      window.openforge.setDocState(title, snap.dirty)
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
    if (as || !path) {
      path = await window.openforge.saveDialog('part', `${fileName(path)}.ofp`)
      if (!path) return false
    }
    const r = await get().run('doc.save', { path })
    if (r) get().toast('Saved', 'info')
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
    if (pick.kind === 'datum') ref = { type: 'feature', id: pick.id }
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
    const out: { kind: Kind; index: number }[] = []
    for (const r of refs) {
      if (r.type !== 'face' && r.type !== 'edge' && r.type !== 'vertex') continue
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
    set({ panel: p, previewScene: null, previewError: null, highlights: [], pickTarget: null })
    if (p.editing !== undefined) void get().run('edit.begin', { id: p.editing })
  },

  async closePanel() {
    const editing = get().panel?.editing
    set({ panel: null, previewScene: null, previewError: null, highlights: [], pickTarget: null })
    await call('feature.preview', { feature: null })
    if (editing !== undefined) await get().run('edit.end')
  },

  async preview(params, editing) {
    try {
      const r = await call<{ scene: Scene | null; error: string | null }>('feature.preview', {
        feature: params,
        editing,
      })
      set({ previewScene: r.scene, previewError: r.error })
    } catch (e) {
      set({ previewScene: null, previewError: (e as Error).message })
    }
  },

  async commit(params, editing) {
    const snap =
      editing !== undefined
        ? await get().run('feature.update', { id: editing, feature: params })
        : await get().run('feature.add', { feature: params })
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
        set({ snapshot: { ...get().snapshot!, dirty: true, can_undo: true } })
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
