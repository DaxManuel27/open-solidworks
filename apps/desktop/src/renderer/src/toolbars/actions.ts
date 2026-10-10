import type { Ref } from '../protocol'
import { type PanelType, useStore } from '../store'

const PLANE_LIKE = new Set(['face', 'front_plane', 'top_plane', 'right_plane', 'feature'])

/** Sketch on the selected plane or face, or ask for one. */
export function beginSketch(): void | Promise<void> {
  const s = useStore.getState()
  const sel = s.selection[0]?.ref
  if (
    sel &&
    PLANE_LIKE.has(sel.type) &&
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
