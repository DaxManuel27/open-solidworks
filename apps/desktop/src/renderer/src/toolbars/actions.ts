import type { Ref } from '../protocol'
import { type PanelType, useStore } from '../store'

const PLANE_LIKE = new Set(['face', 'front_plane', 'top_plane', 'right_plane', 'feature'])

/** Sketch on the selected plane or face, or ask for one. */
export function beginSketch(): void | Promise<void> {
  const s = useStore.getState()
  const sel = s.selection[0]?.ref
  if (
    sel &&
    (PLANE_LIKE.has(sel.type) || (sel.type === 'body_topology' && sel.kind === 'face')) &&
    (sel.type !== 'feature' || s.selection[0]!.pick.kind === 'datum')
  ) {
    return s.startSketch(sel)
  }
  const tree = s.snapshot?.features.find((f) => f.id === s.treeSelection && f.type === 'plane')
  if (tree) {
    return s.startSketch({ type: 'feature', id: tree.id })
  }
  s.toast('Select a plane or planar face to sketch on', 'info')
  s.setPickTarget({
    kinds: ['face', 'datum', 'std_plane'],
    onPick: (_pick, ref: Ref) => {
      useStore.getState().setPickTarget(null)
      void useStore.getState().startSketch(ref)
    },
  })
}

/** Open a feature panel, finishing the current sketch first and using it. */
export async function startFeature(type: PanelType): Promise<void> {
  const s = useStore.getState()
  const sketch = s.sketch?.id
  if (sketch !== undefined) await s.closeSketch()
  if (s.panel) await s.closePanel()
  useStore.getState().openPanel({ type, sketch })
}

/** Flatten every sheet metal body or form it again: the Flat-Pattern feature's suppression. */
export async function toggleFlatten(): Promise<void> {
  const s = useStore.getState()
  if (s.panel) await s.closePanel()
  const flat = s.snapshot?.features.find((f) => f.type === 'flat_pattern')
  if (!flat) await s.run('feature.add', { feature: { type: 'flat_pattern' }, formulas: {} })
  else await s.run('feature.suppress', { id: flat.id, suppressed: !flat.suppressed })
}

/** Whether the part shows its flat pattern. */
export function isFlattened(s: ReturnType<typeof useStore.getState>): boolean {
  const flat = s.snapshot?.features.find((f) => f.type === 'flat_pattern')
  return !!flat && !flat.suppressed && !flat.rolled_back
}

/** Make the part a weldment once. */
export async function ensureWeldment(): Promise<boolean> {
  const s = useStore.getState()
  if (s.snapshot?.features.some((f) => f.type === 'weldment')) return true
  return !!(await s.run('feature.add', { feature: { type: 'weldment' }, formulas: {} }))
}

/** Open a weldment feature's panel, adding the Weldment feature first. */
export async function startWeldmentFeature(type: PanelType, variant?: string): Promise<void> {
  const s = useStore.getState()
  const sketch = s.sketch?.id
  if (sketch !== undefined) await s.closeSketch()
  if (s.panel) await s.closePanel()
  if (!(await ensureWeldment())) return
  useStore.getState().openPanel({ type, sketch, variant })
}
