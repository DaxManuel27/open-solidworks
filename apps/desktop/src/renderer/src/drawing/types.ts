// Types for drawing documents and rendered sheets. Mirrors crates/of-drawing
// (document.rs, graphics.rs, drawing.rs); parameter types come from the
// generated tool catalog. Sheet coordinates are mm of paper, origin at the
// bottom-left, y up.

import type {
  AnnotationKind,
  CompRef,
  DisplayStyle,
  Profile,
  Projection,
  Scale,
  SheetSize,
  Standard,
  TangentEdges,
  ViewKind,
} from '../../../shared/tools.gen'

export type P2 = [number, number]

export type BreakStyle = 'straight' | 'curved' | 'zig_zag' | 'small_zig_zag' | 'jagged'

export interface Break {
  vertical: boolean
  from: number
  to: number
}

export interface BrokenOut {
  profile: Profile
  depth: number
}

export interface HatchOptions {
  pattern?: string | null
  scale: number
  angle: number
}

export interface DrawingView {
  id: number
  name: string
  kind: ViewKind
  /** Sheet position of the model origin. */
  origin: P2
  scale?: Scale | null
  display?: DisplayStyle | null
  tangent?: TangentEdges | null
  label?: string | null
  aligned: boolean
  breaks?: Break[]
  break_style: BreakStyle
  break_gap: number
  crop?: Profile | null
  broken_out?: BrokenOut[]
  hatch?: HatchOptions
  hidden_edges?: CompRef[]
  hidden_components?: string[][]
}

export type Annotation = AnnotationKind & {
  id: number
  /** The view it moves with. */
  attach?: number | null
  layer?: string | null
}

export interface Sheet {
  id: number
  name: string
  size: SheetSize
  portrait: boolean
  scale: Scale
  projection: Projection
  format: { template: string; visible: boolean }
  properties_view?: number | null
  views: DrawingView[]
  annotations?: Annotation[]
}

export interface DrawingDocument {
  standard: Standard
  sheets: Sheet[]
  properties?: Record<string, string>
  next_id: number
}

export interface DrawingSnapshot {
  kind: 'drawing'
  drawing: DrawingDocument
  path: string | null
  dirty: boolean
  can_undo: boolean
  can_redo: boolean
  revision: number
  /** Set by some methods. */
  id?: number
  added?: number[]
  deleted?: number[]
  saved_models?: string[]
}

export type LineStyle = 'continuous' | 'hidden' | 'center' | 'phantom'

export interface Pen {
  style: LineStyle
  weight: number
  color: [number, number, number]
}

export type Prim =
  | { type: 'line'; a: P2; b: P2 }
  | { type: 'arc'; center: P2; radius: number; start: number; end: number }
  | { type: 'polyline'; points: P2[]; closed?: boolean }
  | { type: 'fill'; loops: P2[][] }
  | { type: 'text'; at: P2; height: number; angle?: number; text: string; width: number }

export type Owner =
  | { type: 'format' }
  | { type: 'view'; view: number; entity?: CompRef | null }
  | { type: 'annotation'; id: number }

export interface Item {
  prim: Prim
  pen: Pen
  layer: string
  owner: Owner
}

export interface ViewInfo {
  id: number
  bbox: [P2, P2] | null
  scale: number
  origin: P2
  x: [number, number, number]
  y: [number, number, number]
  toward: [number, number, number]
  error?: string
  stale?: boolean
}

export interface SheetRender {
  sheet: number
  graphics: { width: number; height: number; items: Item[] }
  views: ViewInfo[]
  frame: [P2, P2]
  title_block: [P2, P2]
  dangling?: { annotation: number; reason: string }[]
}

/** Paper size of a sheet in mm, orientation applied. */
export function paperSize(s: Sheet): P2 {
  const IN = 25.4
  const sizes: Record<string, P2> = {
    a0: [1189, 841],
    a1: [841, 594],
    a2: [594, 420],
    a3: [420, 297],
    a4: [297, 210],
    ansi_a: [11 * IN, 8.5 * IN],
    ansi_b: [17 * IN, 11 * IN],
    ansi_c: [22 * IN, 17 * IN],
    ansi_d: [34 * IN, 22 * IN],
    ansi_e: [44 * IN, 34 * IN],
  }
  if (s.size.type === 'custom') return [s.size.width, s.size.height]
  const [w, h] = sizes[s.size.type]!
  return s.portrait ? [h, w] : [w, h]
}

export function scaleLabel(s: Scale): string {
  const f = (v: number) => (Math.abs(v - Math.round(v)) < 1e-9 ? String(Math.round(v)) : String(v))
  return `${f(s.num)}:${f(s.den)}`
}

/** Parse `1:2`, `2:1` or a number into a scale. */
export function parseScale(t: string): Scale | null {
  const m = t.trim().match(/^(\d+(?:\.\d+)?)\s*[:/]\s*(\d+(?:\.\d+)?)$/)
  if (m) {
    const num = Number(m[1])
    const den = Number(m[2])
    return num > 0 && den > 0 ? { num, den } : null
  }
  const v = Number(t)
  return Number.isFinite(v) && v > 0 ? { num: v, den: 1 } : null
}

export function sheetSizeLabel(s: SheetSize): string {
  if (s.type === 'custom') return `${s.width} × ${s.height}`
  return s.type.startsWith('ansi_') ? `ANSI ${s.type.slice(5).toUpperCase()}` : s.type.toUpperCase()
}

/** Every annotation of a sheet. */
export function annotationsOf(s: Sheet | undefined): Annotation[] {
  return s?.annotations ?? []
}

/** The parent view of a derived view. */
export function parentOf(v: DrawingView): number | null {
  return v.kind.type === 'model' ? null : v.kind.parent
}
