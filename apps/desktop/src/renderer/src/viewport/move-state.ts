import { create } from 'zustand'

/** How dragging a component in the viewport moves it. */
export interface MoveSettings {
  /** The Move or Rotate Component panel is open: plain drags move components. */
  active: boolean
  /** Translate freely in the screen plane or along an assembly axis, or rotate. */
  mode: 'free' | 'x' | 'y' | 'z' | 'rotate'
  /** Stop before parts overlap. */
  collision: boolean
  /** Check collisions only for the dragged component. */
  draggedOnly: boolean
}

export const useMove = create<MoveSettings>(() => ({
  active: false,
  mode: 'free',
  collision: false,
  draggedOnly: false,
}))
