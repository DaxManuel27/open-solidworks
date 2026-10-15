import { create } from 'zustand'
import { call } from '../core'
import type { CompRef } from '../../../shared/tools.gen'
import type { DrawingSnapshot, Item, P2, Prim, SheetRender } from './types'

/** Something picked on the sheet. */
export type SheetPick =
  /** A model edge (or other entity) drawn in a view. */
  | { type: 'entity'; view: number; entity: CompRef; at: P2; prim?: Prim }
  /** A view, picked on an outline without a model entity. */
  | { type: 'view'; view: number; at: P2 }
  | { type: 'annotation'; id: number; at: P2 }
  /** Empty paper. */
  | { type: 'point'; at: P2; view: number | null }

export type PickKind = SheetPick['type']

/** A panel that wants sheet picks routed to it. */
export interface SheetTarget {
  kinds: PickKind[]
  /** Short prompt shown in the status bar. */
  prompt: string
  onPick: (pick: SheetPick) => void
  /** Called while the pointer moves, for rubber-band previews. */
  onHover?: (at: P2) => void
}

/** Temporary geometry a panel draws over the sheet (sheet mm). */
export interface Overlay {
  lines: [P2, P2][]
  circles: { center: P2; radius: number }[]
  points: P2[]
}

export const EMPTY_OVERLAY: Overlay = { lines: [], circles: [], points: [] }

interface DrawingState {
  /** Sheet shown. */
  sheet: number | null
  render: SheetRender | null
  /** Revision of the document the render was made from. */
  renderedRevision: number
  rendering: boolean
  selection: SheetPick[]
  target: SheetTarget | null
  overlay: Overlay
  /** Bumped to ask the canvas to zoom to the sheet. */
  fitSeq: number
  setSheet(id: number): void
  refresh(force?: boolean): Promise<void>
  select(p: SheetPick | null, additive?: boolean): void
  setTarget(t: SheetTarget | null): void
  setOverlay(o: Overlay): void
  fit(): void
}

let renderSequence = 0

export const useDrawing = create<DrawingState>((set, get) => ({
  sheet: null,
  render: null,
  renderedRevision: -1,
  rendering: false,
  selection: [],
  target: null,
  overlay: EMPTY_OVERLAY,
  fitSeq: 0,

  setSheet(id) {
    if (get().sheet === id) return
    set({ sheet: id, render: null, selection: [], renderedRevision: -1 })
    void get().refresh(true)
    get().fit()
  },

  async refresh(force = false) {
    const id = get().sheet
    if (id === null) return
    const sequence = ++renderSequence
    set({ rendering: true })
    try {
      const r = await call<SheetRender & { revision: number }>('drawing.render', { sheet: id })
      if (sequence !== renderSequence || get().sheet !== id) return
      if (!force && r.revision < get().renderedRevision) return
      set({ render: r, renderedRevision: r.revision })
    } catch {
      // A deleted sheet: the shell picks another one.
    } finally {
      if (sequence === renderSequence) set({ rendering: false })
    }
  },

  select(p, additive = false) {
    if (!p) return set({ selection: [] })
    const key = pickKey(p)
    set((s) => {
      const exists = s.selection.some((x) => pickKey(x) === key)
      if (additive)
        return {
          selection: exists ? s.selection.filter((x) => pickKey(x) !== key) : [...s.selection, p],
        }
      return { selection: [p] }
    })
  },

  setTarget(t) {
    set({ target: t, overlay: t ? get().overlay : EMPTY_OVERLAY })
  },

  setOverlay(o) {
    set({ overlay: o })
  },

  fit() {
    set((s) => ({ fitSeq: s.fitSeq + 1 }))
  },
}))

export function pickKey(p: SheetPick): string {
  switch (p.type) {
    case 'entity':
      return `e:${p.view}:${JSON.stringify(p.entity)}`
    case 'view':
      return `v:${p.view}`
    case 'annotation':
      return `a:${p.id}`
    case 'point':
      return `p:${p.at[0]},${p.at[1]}`
  }
}

/** Whether an item belongs to a pick (for highlighting). */
export function itemMatches(item: Item, p: SheetPick): boolean {
  const o = item.owner
  switch (p.type) {
    case 'entity':
      return (
        o.type === 'view' &&
        o.view === p.view &&
        !!o.entity &&
        JSON.stringify(o.entity) === JSON.stringify(p.entity)
      )
    case 'view':
      return o.type === 'view' && o.view === p.view
    case 'annotation':
      return o.type === 'annotation' && o.id === p.id
    case 'point':
      return false
  }
}

/** Pick the drawing's current sheet after a document change. */
export function syncSheet(d: DrawingSnapshot): void {
  const s = useDrawing.getState()
  const ids = d.drawing.sheets.map((x) => x.id)
  if (s.sheet === null || !ids.includes(s.sheet)) {
    useDrawing.setState({
      sheet: ids[0] ?? null,
      render: null,
      selection: [],
      renderedRevision: -1,
    })
    useDrawing.getState().fit()
  } else {
    // Drop selections of deleted things.
    const views = new Set(d.drawing.sheets.flatMap((x) => x.views.map((v) => v.id)))
    const notes = new Set(d.drawing.sheets.flatMap((x) => (x.annotations ?? []).map((a) => a.id)))
    const kept = s.selection.filter((p) =>
      p.type === 'annotation' ? notes.has(p.id) : p.type === 'point' || views.has(p.view),
    )
    if (kept.length !== s.selection.length) useDrawing.setState({ selection: kept })
  }
  void useDrawing.getState().refresh()
}
