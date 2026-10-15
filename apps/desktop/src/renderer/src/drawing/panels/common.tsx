import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import type {
  Anchor,
  AnnotationKind,
  CompRef,
  Pick,
  Scale,
  ViewRef,
} from '../../../../shared/tools.gen'
import { useStore } from '../../store'
import { EMPTY_OVERLAY, type Overlay, type PickKind, type SheetPick, useDrawing } from '../state'
import type { DrawingView, P2, Sheet, ViewInfo } from '../types'
import { parseScale, scaleLabel } from '../types'

/** The sheet shown. */
export function useSheet(): Sheet | undefined {
  const drawing = useStore((s) => s.drawing)
  const id = useDrawing((s) => s.sheet)
  return drawing?.drawing.sheets.find((s) => s.id === id)
}

/** Every view of the drawing. */
export function allViews(): DrawingView[] {
  return useStore.getState().drawing?.drawing.sheets.flatMap((s) => s.views) ?? []
}

/** How a view was drawn on the current sheet. */
export function viewInfo(id: number): ViewInfo | undefined {
  return useDrawing.getState().render?.views.find((v) => v.id === id)
}

/** View-plane model coordinates (unscaled, relative to the model origin) of a sheet point. */
export function toViewPlane(view: number, at: P2): P2 {
  const v = viewInfo(view)
  if (!v) return at
  return [(at[0] - v.origin[0]) / v.scale, (at[1] - v.origin[1]) / v.scale]
}

/** Sheet point of view-plane model coordinates. */
export function fromViewPlane(view: number, p: P2): P2 {
  const v = viewInfo(view)
  if (!v) return p
  return [v.origin[0] + p[0] * v.scale, v.origin[1] + p[1] * v.scale]
}

/** A sheet point relative to a view's origin (annotations that move with it). */
export function relativeTo(view: number | null, at: P2): P2 {
  if (view === null) return at
  const v = viewInfo(view)
  return v ? [at[0] - v.origin[0], at[1] - v.origin[1]] : at
}

/** The view whose box holds a sheet point. */
export function viewAt(at: P2): number | null {
  return (
    useDrawing
      .getState()
      .render?.views.find(
        (v) =>
          v.bbox &&
          at[0] >= v.bbox[0][0] - 5 &&
          at[0] <= v.bbox[1][0] + 5 &&
          at[1] >= v.bbox[0][1] - 5 &&
          at[1] <= v.bbox[1][1] + 5,
      )?.id ?? null
  )
}

/** Route sheet picks to a panel while it is open (or while `enabled`). */
export function useSheetPicks(
  kinds: PickKind[],
  prompt: string,
  onPick: (p: SheetPick) => void,
  enabled = true,
  onHover?: (at: P2) => void,
): void {
  const handler = useRef(onPick)
  const hover = useRef(onHover)
  // The latest callbacks, without re-registering the target each render.
  useLayoutEffect(() => {
    handler.current = onPick
    hover.current = onHover
  })
  const key = kinds.join(',')
  useEffect(() => {
    if (!enabled) return
    useDrawing.getState().setTarget({
      kinds,
      prompt,
      onPick: (p) => handler.current(p),
      onHover: (at) => hover.current?.(at),
    })
    return () => useDrawing.getState().setTarget(null)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, prompt, enabled])
}

/** Draw temporary geometry over the sheet while the panel is open. */
export function useOverlay(o: Overlay): void {
  const json = JSON.stringify(o)
  useEffect(() => {
    useDrawing.getState().setOverlay(o)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [json])
  useEffect(() => () => useDrawing.getState().setOverlay(EMPTY_OVERLAY), [])
}

export async function closeDrawingPanel(): Promise<void> {
  useDrawing.getState().setTarget(null)
  useDrawing.getState().setOverlay(EMPTY_OVERLAY)
  await useStore.getState().closePanel()
}

/** Add an annotation to the current sheet; returns whether it worked. */
export async function addAnnotation(
  annotation: AnnotationKind,
  view: number | null = null,
): Promise<boolean> {
  const sheet = useDrawing.getState().sheet
  if (sheet === null) return false
  const r = await useStore.getState().run('drawing.annotation.add', {
    sheet,
    annotation,
    ...(view !== null ? { view } : {}),
  })
  return !!r
}

/** A model anchor for a picked entity at a sheet point. */
export function modelAnchor(p: SheetPick & { type: 'entity' }): Anchor {
  return {
    type: 'model',
    target: { view: p.view, entity: p.entity },
    at: toViewPlane(p.view, p.at),
  }
}

/** An anchor for any pick: model geometry when picked, else the sheet. */
export function anchorOf(p: SheetPick): Anchor {
  return p.type === 'entity' ? modelAnchor(p) : { type: 'sheet', at: p.at }
}

export function viewRef(p: SheetPick & { type: 'entity' }): ViewRef {
  return { view: p.view, entity: p.entity }
}

export function pickOf(p: SheetPick & { type: 'entity' }, point: Pick['point'] = 'auto'): Pick {
  return { view: p.view, entity: p.entity, ...(point !== 'auto' ? { point } : {}) }
}

/** Describe a picked entity briefly. */
export function entityLabel(e: CompRef): string {
  const r = e.entity as { type: string; name?: string }
  const where = e.component?.length ? `${e.component.join('/')}: ` : ''
  return `${where}${r.type}${r.name ? ` ${r.name}` : ''}`
}

/** A text field committed on blur or Enter. */
export function TextField(props: {
  label: string
  value: string
  onChange: (v: string) => void
  placeholder?: string
  multiline?: boolean
}) {
  const [text, setText] = useState(props.value)
  const [shown, setShown] = useState(props.value)
  if (props.value !== shown) {
    setShown(props.value)
    setText(props.value)
  }
  return (
    <label className="field">
      <span>{props.label}</span>
      {props.multiline ? (
        <textarea
          value={text}
          rows={3}
          placeholder={props.placeholder}
          onChange={(e) => {
            setText(e.target.value)
            props.onChange(e.target.value)
          }}
          onKeyDown={(e) => e.stopPropagation()}
        />
      ) : (
        <input
          value={text}
          placeholder={props.placeholder}
          onChange={(e) => {
            setText(e.target.value)
            props.onChange(e.target.value)
          }}
          onKeyDown={(e) => e.stopPropagation()}
        />
      )}
    </label>
  )
}

/** A scale such as `1:2`; empty means `inherit` (sheet or parent scale). */
export function ScaleField(props: {
  label: string
  value: Scale | null
  onChange: (s: Scale | null) => void
  inheritLabel?: string
}) {
  const [text, setText] = useState(props.value ? scaleLabel(props.value) : '')
  const [bad, setBad] = useState(false)
  const commit = () => {
    if (!text.trim()) {
      setBad(false)
      props.onChange(null)
      return
    }
    const s = parseScale(text)
    setBad(!s)
    if (s) props.onChange(s)
  }
  return (
    <label className="field">
      <span>{props.label}</span>
      <input
        className={bad ? 'bad' : ''}
        value={text}
        placeholder={props.inheritLabel ?? 'Sheet scale'}
        title="A scale such as 1:2 or 2:1; leave empty to use the sheet's"
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          e.stopPropagation()
          if (e.key === 'Enter') commit()
        }}
      />
    </label>
  )
}

/** A short status line telling the user what to click. */
export function Prompt(props: { children: React.ReactNode }) {
  return <div className="panel-prompt">{props.children}</div>
}

/** One of the eight projection directions from a parent view toward a point. */
export function placementToward(from: P2, to: P2): string {
  const a = Math.atan2(to[1] - from[1], to[0] - from[0])
  const dirs = [
    'right',
    'above_right',
    'above',
    'above_left',
    'left',
    'below_left',
    'below',
    'below_right',
  ]
  const i = Math.round(((a + Math.PI * 2) % (Math.PI * 2)) / (Math.PI / 4)) % 8
  return dirs[i]!
}
