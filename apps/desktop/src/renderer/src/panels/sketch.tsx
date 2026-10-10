import { useState } from 'react'
import type { SketchConstraint, SketchCurve, SketchPoint, SketchView } from '../protocol'
import { useStore } from '../store'
import { Check, IntField, LengthField, Section } from './fields'

type Item =
  | { kind: 'point'; id: number; v: SketchPoint }
  | { kind: SketchCurve['type']; id: number; v: SketchCurve }
  | { kind: 'constraint'; id: number; v: SketchConstraint }

function itemsOf(view: SketchView, ids: number[]): Item[] {
  const out: Item[] = []
  for (const id of ids) {
    const p = view.points.find((x) => x.id === id)
    if (p) {
      out.push({ kind: 'point', id, v: p })
      continue
    }
    const c = view.curves.find((x) => x.id === id)
    if (c) {
      out.push({ kind: c.type, id, v: c })
      continue
    }
    const k = view.constraints.find((x) => x.id === id)
    if (k) out.push({ kind: 'constraint', id, v: k })
  }
  return out
}

interface Relation {
  label: string
  constraint: Record<string, unknown>
}

const ROUND = new Set(['circle', 'arc'])

/** Relations that make sense for the selected entities. */
export function relationsFor(items: Item[]): Relation[] {
  const ents = items.filter((i) => i.kind !== 'constraint')
  const pts = ents.filter((i) => i.kind === 'point')
  const lines = ents.filter((i) => i.kind === 'line')
  const rounds = ents.filter((i) => ROUND.has(i.kind))
  const curves = ents.filter((i) => i.kind !== 'point')
  const out: Relation[] = []
  const add = (label: string, constraint: Record<string, unknown>) =>
    out.push({ label, constraint })
  const n = ents.length
  if (n === 1 && lines.length === 1) {
    add('Horizontal', { type: 'horizontal', line: lines[0]!.id })
    add('Vertical', { type: 'vertical', line: lines[0]!.id })
  }
  if (n === 2 && pts.length === 2) {
    const [a, b] = [pts[0]!.id, pts[1]!.id]
    add('Coincident', { type: 'coincident', a, b })
    add('Horizontal', { type: 'horizontal_points', a, b })
    add('Vertical', { type: 'vertical_points', a, b })
  }
  if (n === 2 && pts.length === 1 && curves.length === 1) {
    const point = pts[0]!.id
    const curve = curves[0]!
    add('On curve', { type: 'point_on_curve', point, curve: curve.id })
    if (curve.kind === 'line') add('Midpoint', { type: 'midpoint', point, line: curve.id })
  }
  if (n === 2 && lines.length === 2) {
    const [a, b] = [lines[0]!.id, lines[1]!.id]
    add('Parallel', { type: 'parallel', a, b })
    add('Perpendicular', { type: 'perpendicular', a, b })
    add('Collinear', { type: 'collinear', a, b })
    add('Equal', { type: 'equal', a, b })
  }
  if (n === 2 && lines.length === 1 && rounds.length === 1) {
    add('Tangent', { type: 'tangent', a: lines[0]!.id, b: rounds[0]!.id })
  }
  if (n === 2 && rounds.length === 2) {
    const [a, b] = [rounds[0]!.id, rounds[1]!.id]
    add('Equal', { type: 'equal', a, b })
    add('Concentric', { type: 'concentric', a, b })
    add('Tangent', { type: 'tangent', a, b })
  }
  if (n === 2 && curves.length === 2 && curves.some((c) => c.kind === 'spline')) {
    add('Tangent at shared endpoint', { type: 'tangent', a: curves[0]!.id, b: curves[1]!.id })
  }
  if (n === 3 && pts.length === 2 && lines.length === 1) {
    add('Symmetric', { type: 'symmetric', a: pts[0]!.id, b: pts[1]!.id, about: lines[0]!.id })
  }
  if (n === 1) add('Fix', { type: 'fix', entity: ents[0]!.id })
  return out
}

const NAMES: Record<string, string> = {
  point: 'Point',
  line: 'Line',
  circle: 'Circle',
  arc: 'Arc',
  spline: 'Spline',
}

export function SketchPanel() {
  const sketch = useStore((s) => s.sketch)!
  const selection = useStore((s) => s.sketchSelection)
  const sketchEdit = useStore((s) => s.sketchEdit)
  const setSelection = useStore((s) => s.setSketchSelection)
  const closeSketch = useStore((s) => s.closeSketch)
  const filletRadius = useStore((s) => s.sketchFilletRadius)
  const polygonSides = useStore((s) => s.polygonSides)
  const tool = useStore((s) => s.sketchTool)
  const [offset, setOffset] = useState(2)

  const items = itemsOf(sketch, selection)
  const ents = items.filter((i) => i.kind !== 'constraint')
  const constraints = items.filter(
    (i): i is Extract<Item, { kind: 'constraint' }> => i.kind === 'constraint',
  )
  const relations = relationsFor(items)
  const curves = ents.filter((i) => i.kind !== 'point')
  const lastLine = [...ents].reverse().find((i) => i.kind === 'line')
  const allConstruction =
    curves.length > 0 && curves.every((c) => (c.v as SketchCurve).construction)
  const dims = constraints.filter((c) => c.v.value !== undefined)

  return (
    <div className="panel">
      <div className="panel-head">
        <span className="panel-title">Sketch</span>
        <button className="ok" onClick={() => void closeSketch()} title="Exit sketch">
          ✓
        </button>
      </div>
      <div className="panel-body">
        <Section title="Status">
          <p className={`sketch-status ${sketch.status.split(' ')[0]!.toLowerCase()}`}>
            {sketch.status}
          </p>
          {sketch.dof > 0 && (
            <p className="hint">
              {sketch.dof} degree{sketch.dof === 1 ? '' : 's'} of freedom left
            </p>
          )}
          {sketch.profile_error && <p className="panel-error inline">{sketch.profile_error}</p>}
        </Section>

        {(tool === 'fillet' || tool === 'polygon') && (
          <Section title="Tool Options">
            {tool === 'fillet' && (
              <LengthField
                label="Fillet radius"
                value={filletRadius}
                onChange={(v) => useStore.setState({ sketchFilletRadius: v })}
              />
            )}
            {tool === 'polygon' && (
              <IntField
                label="Sides"
                value={polygonSides}
                onChange={(v) => useStore.setState({ polygonSides: Math.max(3, v) })}
                min={3}
              />
            )}
          </Section>
        )}

        <Section title={`Selected (${items.length})`}>
          {items.length === 0 && (
            <p className="hint">
              Click sketch entities or dimensions to select them. Shift-click adds.
            </p>
          )}
          <ul className="selected-list">
            {items.map((i) => (
              <li key={i.id}>
                {i.kind === 'constraint'
                  ? `${i.v.type.replace(/_/g, ' ')} ${i.id}`
                  : `${NAMES[i.kind]} ${i.id}`}
                <button onClick={() => setSelection(selection.filter((x) => x !== i.id))}>×</button>
              </li>
            ))}
          </ul>
          {items.length > 0 && (
            <button
              className="danger"
              onClick={() => {
                void sketchEdit({ op: 'delete', ids: selection })
                setSelection([])
              }}
            >
              Delete
            </button>
          )}
        </Section>

        {relations.length > 0 && (
          <Section title="Add Relations">
            <div className="button-grid">
              {relations.map((r) => (
                <button
                  key={r.label}
                  onClick={() => void sketchEdit({ op: 'constraint', constraint: r.constraint })}
                >
                  {r.label}
                </button>
              ))}
            </div>
          </Section>
        )}

        {dims.length === 1 && (
          <Section title="Dimension">
            <Check
              label="Driven (reference only)"
              value={!dims[0]!.v.driving}
              onChange={(driven) =>
                void sketchEdit({ op: 'set_driving', id: dims[0]!.id, driving: !driven })
              }
            />
          </Section>
        )}

        {curves.length > 0 && (
          <Section title="Entities">
            <Check
              label="For construction"
              value={allConstruction}
              onChange={(construction) =>
                void sketchEdit({ op: 'construction', ids: curves.map((c) => c.id), construction })
              }
            />
            <div className="row">
              <LengthField label="Offset" value={offset} onChange={setOffset} min={-1e9} />
              <button
                onClick={() =>
                  void sketchEdit({ op: 'offset', ids: curves.map((c) => c.id), distance: offset })
                }
              >
                Apply
              </button>
            </div>
            {lastLine && curves.length > 1 && (
              <button
                onClick={() =>
                  void sketchEdit({
                    op: 'mirror',
                    ids: ents.filter((e) => e.id !== lastLine.id).map((e) => e.id),
                    about: lastLine.id,
                  })
                }
                title="Mirror the selection about the last selected line"
              >
                Mirror about Line {lastLine.id}
              </button>
            )}
          </Section>
        )}
      </div>
    </div>
  )
}
