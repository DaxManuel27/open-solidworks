import { useCallback, useEffect, useRef, useState } from 'react'
import type { Constraint3, Entity3 } from '../../../shared/tools.gen'
import type { Ref } from '../protocol'
import { call } from '../core'
import { type Panel, useStore } from '../store'
import {
  type SpatialReport,
  type SpatialSketch,
  type SpatialDrag,
  useSpatial,
} from '../viewport/spatial-state'
import { Check, Choice, LengthField, PanelShell, RefBox, Section } from './fields'
import { useFeature } from './features'

type WithoutId<T> = T extends unknown ? Omit<T, 'id'> : never

export function SpatialPanel({ panel }: { panel: Panel }) {
  const [sketch, setSketch] = useState<SpatialSketch>(() => {
    const p = useStore.getState().snapshot?.features.find((f) => f.id === panel.editing)?.params
    return p
      ? {
          entities: p['entities'] as Entity3[],
          constraints: p['constraints'] as Constraint3[],
          next_id: p['next_id'] as number,
        }
      : { entities: [], constraints: [], next_id: 1 }
  })
  const [report, setReport] = useState<SpatialReport | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [pending, setPending] = useState(false)
  const sequence = useRef(0)
  const spatial = useSpatial()
  const change = useCallback(async (next: SpatialSketch, drag?: SpatialDrag) => {
    const seq = ++sequence.current
    setSketch(next)
    setPending(true)
    setReport(null)
    setError(null)
    useSpatial.setState({ sketch: next })
    try {
      const result = await call<{ sketch: SpatialSketch; report: SpatialReport }>(
        'sketch3d.solve',
        { sketch: next, drag },
      )
      if (seq !== sequence.current) return
      setSketch(result.sketch)
      setReport(result.report)
      setError(
        result.report.status === 'failed'
          ? `Conflicting relations: ${result.report.conflicting.join(', ') || 'degenerate geometry'}`
          : null,
      )
    } catch (e) {
      if (seq !== sequence.current) return
      setSketch(next)
      setError(String(e))
    } finally {
      if (seq === sequence.current) setPending(false)
    }
  }, [])
  useEffect(() => {
    useSpatial.setState({ sketch, report, change })
  }, [sketch, report, change])
  useEffect(
    () => () => {
      sequence.current++
      useSpatial.setState({
        sketch: null,
        report: null,
        selection: [],
        tool: 'select',
        change: async () => {},
      })
    },
    [],
  )
  const wiring = useFeature(panel, error || pending ? null : { type: 'sketch3d', ...sketch })
  const selected = spatial.selection
    .map((id) => sketch.entities.find((e) => e.id === id))
    .filter((e): e is Entity3 => !!e)
  const points = selected.filter((e) => e.kind === 'point')
  const lines = selected.filter((e) => e.kind === 'line')
  const curves = selected.filter((e) => e.kind !== 'point')
  const [modelRef, setModelRef] = useState<Ref | null>(null)
  type Vec = [number, number, number]
  const follow = async (kind: 'point' | 'plane', apply: (a: Vec, n?: Vec) => void) => {
    try {
      const r = await call<{ point: Vec | null; plane: { origin: Vec; normal: Vec } | null }>(
        'ref.locate',
        { reference: modelRef },
      )
      if (kind === 'point' && r.point) apply(r.point)
      else if (kind === 'plane' && r.plane) apply(r.plane.origin, r.plane.normal)
      else
        useStore
          .getState()
          .toast(`Select a model ${kind === 'point' ? 'vertex or point' : 'plane or planar face'}`)
    } catch (e) {
      useStore.getState().toast((e as Error).message)
    }
  }
  const relation = (c: WithoutId<Constraint3>) =>
    void change({
      ...sketch,
      next_id: sketch.next_id + 1,
      constraints: [...sketch.constraints, { ...c, id: sketch.next_id } as Constraint3],
    })
  const remove = () => {
    const ids = new Set(spatial.selection)
    for (const e of sketch.entities) {
      if (
        (e.kind === 'line' && [e.a, e.b].some((p) => ids.has(p))) ||
        (e.kind === 'bezier' && e.points.some((p) => ids.has(p)))
      )
        ids.add(e.id)
    }
    const refs = (c: Constraint3): number[] => {
      switch (c.kind) {
        case 'coincident':
        case 'distance':
        case 'parallel':
        case 'perpendicular':
        case 'tangent':
          return [c.a, c.b]
        case 'axis':
          return [c.line]
        case 'point_on_curve':
          return [c.point, c.curve]
        default:
          return [c.point]
      }
    }
    void change({
      ...sketch,
      entities: sketch.entities.filter((e) => !ids.has(e.id)),
      constraints: sketch.constraints.filter(
        (c) => !ids.has(c.id) && !refs(c).some((id) => ids.has(id)),
      ),
    })
    useSpatial.setState({ selection: [] })
  }
  return (
    <PanelShell title="3D Sketch" {...wiring} error={error ?? wiring.error}>
      <p className="hint">
        Click to draw on the active workplane. Select points or curves; Shift adds to the selection.
        Drag points or their XYZ handles. Alt-drag or the middle button orbits.
      </p>
      <p role="status">
        {report
          ? `${report.dof} degrees of freedom · ${report.status}${report.redundant.length ? ` · redundant relations ${report.redundant.join(', ')}` : ''}`
          : 'Add geometry or solve to see degrees of freedom'}
      </p>
      <Choice
        label="Spatial tool"
        value={spatial.tool}
        options={[
          { value: 'select', label: 'Select / drag' },
          { value: 'point', label: 'Point' },
          { value: 'line', label: 'Connected lines' },
          { value: 'bezier', label: 'Cubic spline (4 control points)' },
        ]}
        onChange={(tool) => useSpatial.setState({ tool })}
      />
      <Choice
        label="Active workplane"
        value={spatial.plane}
        options={[
          { value: 'xy', label: 'XY' },
          { value: 'xz', label: 'XZ' },
          { value: 'yz', label: 'YZ' },
        ]}
        onChange={(plane) => useSpatial.setState({ plane })}
      />
      <LengthField
        label="Workplane depth"
        min={-Number.MAX_VALUE}
        value={spatial.depth}
        onChange={(depth) => useSpatial.setState({ depth })}
      />
      <LengthField
        label="Snap grid (0 disables)"
        value={spatial.grid}
        onChange={(grid) => useSpatial.setState({ grid: Math.max(0, grid) })}
      />
      <Choice
        label="Drag lock"
        value={spatial.lock}
        options={[
          { value: 'plane', label: 'Active plane at point depth' },
          { value: 'x', label: 'X axis' },
          { value: 'y', label: 'Y axis' },
          { value: 'z', label: 'Z axis' },
        ]}
        onChange={(lock) => useSpatial.setState({ lock })}
      />
      <Section title="Entities and coordinates">
        {sketch.entities.map((e) => (
          <div key={e.id}>
            <button
              className={spatial.selection.includes(e.id) ? 'active' : ''}
              onClick={(event) =>
                useSpatial.setState({
                  selection: event.shiftKey ? [...new Set([...spatial.selection, e.id])] : [e.id],
                })
              }
            >
              {e.kind} {e.id}
            </button>
            {spatial.selection.includes(e.id) && (
              <>
                {e.kind === 'point' && (
                  <SpatialCoordinates entity={e} sketch={sketch} change={change} />
                )}
                <Check
                  label="Construction"
                  value={e.construction ?? false}
                  onChange={(construction) =>
                    void change({
                      ...sketch,
                      entities: sketch.entities.map((x) =>
                        x.id === e.id ? { ...x, construction } : x,
                      ),
                    })
                  }
                />
              </>
            )}
          </div>
        ))}
        <button
          onClick={() => {
            const id = sketch.next_id
            void change({
              ...sketch,
              next_id: id + 1,
              entities: [...sketch.entities, { id, kind: 'point', at: [0, 0, 0] }],
            })
            useSpatial.setState({ selection: [id] })
          }}
        >
          Add point by coordinates
        </button>
        <button disabled={!spatial.selection.length} onClick={remove}>
          Delete selected
        </button>
      </Section>
      <Section title="Follow model geometry" open={false}>
        <RefBox
          label="Model vertex, point, plane or face"
          refs={modelRef ? [modelRef] : []}
          kinds={['vertex', 'face', 'datum', 'std_plane']}
          single
          active
          onActivate={() => {}}
          onChange={(refs) => setModelRef(refs[0] ?? null)}
        />
        <button
          disabled={points.length !== 1 || !modelRef}
          onClick={() =>
            void follow('point', (at) =>
              relation({
                kind: 'fixed',
                point: points[0]!.id,
                at,
                external: JSON.stringify(modelRef),
              }),
            )
          }
        >
          Fix point to model point
        </button>
        <button
          disabled={points.length !== 1 || !modelRef}
          onClick={() =>
            void follow('plane', (origin, normal) =>
              relation({
                kind: 'point_on_plane',
                point: points[0]!.id,
                origin,
                normal: normal!,
                external: JSON.stringify(modelRef),
              }),
            )
          }
        >
          Keep point on model plane
        </button>
        <p className="hint">
          These relations follow the selected geometry when the model rebuilds.
        </p>
      </Section>
      <Section title="Relations and dimensions">
        <button
          disabled={points.length !== 1}
          onClick={() =>
            points[0]?.kind === 'point' &&
            relation({ kind: 'fixed', point: points[0].id, at: points[0].at })
          }
        >
          Fix point
        </button>
        <button
          disabled={points.length !== 2}
          onClick={() => relation({ kind: 'coincident', a: points[0]!.id, b: points[1]!.id })}
        >
          Coincident points
        </button>
        <button
          disabled={points.length !== 2}
          onClick={() => {
            const [a, b] = points
            if (a?.kind === 'point' && b?.kind === 'point')
              relation({
                kind: 'distance',
                a: a.id,
                b: b.id,
                value: Math.hypot(...a.at.map((v, i) => v - b.at[i]!)),
              })
          }}
        >
          Distance dimension
        </button>
        {(['x', 'y', 'z'] as const).map((axis) => (
          <button
            key={axis}
            disabled={lines.length !== 1}
            onClick={() => relation({ kind: 'axis', line: lines[0]!.id, axis })}
          >
            Along {axis.toUpperCase()}
          </button>
        ))}
        <button
          disabled={lines.length !== 2}
          onClick={() => relation({ kind: 'parallel', a: lines[0]!.id, b: lines[1]!.id })}
        >
          Parallel
        </button>
        <button
          disabled={lines.length !== 2}
          onClick={() => relation({ kind: 'perpendicular', a: lines[0]!.id, b: lines[1]!.id })}
        >
          Perpendicular
        </button>
        <button
          disabled={points.length !== 1}
          onClick={() => {
            const i = spatial.plane === 'xy' ? 2 : spatial.plane === 'xz' ? 1 : 0
            const origin: [number, number, number] = [0, 0, 0],
              normal: [number, number, number] = [0, 0, 0]
            origin[i] = spatial.depth
            normal[i] = 1
            relation({ kind: 'point_on_plane', point: points[0]!.id, origin, normal })
          }}
        >
          On active plane
        </button>
        <button
          disabled={points.length !== 1 || curves.length !== 1}
          onClick={() =>
            relation({ kind: 'point_on_curve', point: points[0]!.id, curve: curves[0]!.id })
          }
        >
          Point on curve
        </button>
        <button
          disabled={curves.length !== 2}
          onClick={() =>
            relation({
              kind: 'tangent',
              a: curves[0]!.id,
              b: curves[1]!.id,
              end_a: true,
              end_b: false,
            })
          }
        >
          Tangent (first end to second start)
        </button>
        {sketch.constraints.map((c) => (
          <div key={c.id}>
            <button onClick={() => useSpatial.setState({ selection: [c.id] })}>
              {c.kind} {c.id}
              {report?.conflicting.includes(c.id) ? ' · conflict' : ''}
            </button>
            {c.kind === 'distance' && (
              <LengthField
                label={`Distance ${c.id}`}
                value={c.value}
                onChange={(value) =>
                  void change({
                    ...sketch,
                    constraints: sketch.constraints.map((x) =>
                      x.id === c.id ? { ...c, value } : x,
                    ),
                  })
                }
              />
            )}
            {c.kind === 'tangent' && (
              <>
                <Check
                  label="First curve end"
                  value={c.end_a ?? false}
                  onChange={(end_a) =>
                    void change({
                      ...sketch,
                      constraints: sketch.constraints.map((x) =>
                        x.id === c.id ? { ...c, end_a } : x,
                      ),
                    })
                  }
                />
                <Check
                  label="Second curve end"
                  value={c.end_b ?? false}
                  onChange={(end_b) =>
                    void change({
                      ...sketch,
                      constraints: sketch.constraints.map((x) =>
                        x.id === c.id ? { ...c, end_b } : x,
                      ),
                    })
                  }
                />
              </>
            )}
          </div>
        ))}
        <button onClick={() => void change(sketch)}>Solve sketch</button>
      </Section>
    </PanelShell>
  )
}

function SpatialCoordinates({
  entity,
  sketch,
  change,
}: {
  entity: Extract<Entity3, { kind: 'point' }>
  sketch: SpatialSketch
  change: (sketch: SpatialSketch, drag?: SpatialDrag) => Promise<void>
}) {
  return (['X', 'Y', 'Z'] as const).map((axis, i) => (
    <LengthField
      key={axis}
      label={`${axis} · point ${entity.id}`}
      min={-Number.MAX_VALUE}
      value={entity.at[i]!}
      onChange={(value) => {
        const to = [...entity.at] as [number, number, number]
        to[i] = value
        void change(sketch, { point: entity.id, to })
      }}
    />
  ))
}
