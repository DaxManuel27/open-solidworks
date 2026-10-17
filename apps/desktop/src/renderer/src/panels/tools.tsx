import { useEffect, useState } from 'react'
import { call } from '../core'
import type { MassProperties, Measurement, Ref, Units, Vec3 } from '../protocol'
import { useStore } from '../store'
import { formatAngle, formatLength } from '../units'
import {
  Check,
  Choice,
  IntField,
  LengthField,
  NumberField,
  PanelShell,
  RefBox,
  Section,
} from './fields'

const MM: Record<Units['length'], number> = { mm: 1, cm: 10, m: 1000, in: 25.4, ft: 304.8 }
const KG: Record<Units['mass'], number> = { g: 1e-3, kg: 1, lb: 0.45359237 }

function formatArea(mm2: number, u: Units): string {
  return `${(mm2 / MM[u.length] ** 2).toFixed(u.decimals)} ${u.length}²`
}

function formatVolume(mm3: number, u: Units): string {
  return `${(mm3 / MM[u.length] ** 3).toFixed(u.decimals)} ${u.length}³`
}

function formatMass(kg: number, u: Units): string {
  return `${(kg / KG[u.mass]).toFixed(u.decimals)} ${u.mass}`
}

function formatPoint(p: Vec3, u: Units): string {
  return p.map((x) => formatLength(x, u, false)).join(', ') + ` ${u.length}`
}

function useClose() {
  const close = useStore((s) => s.closePanel)
  return () => void close()
}

export function MeasurePanel() {
  const units = useStore((s) => s.snapshot!.units)
  const [refs, setRefs] = useState<Ref[]>(() => useStore.getState().selection.map((s) => s.ref))
  const [result, setResult] = useState<Measurement | null>(null)
  const [error, setError] = useState<string | null>(null)
  const json = JSON.stringify(refs)
  useEffect(() => {
    const list = JSON.parse(json) as Ref[]
    if (!list.length) return
    let live = true
    call<Measurement>('measure', { refs: list })
      .then((m) => {
        if (!live) return
        setResult(m)
        setError(null)
      })
      .catch((e: Error) => {
        if (!live) return
        setResult(null)
        setError(e.message)
      })
    return () => {
      live = false
    }
  }, [json])
  const format = (q: Measurement['values'][number]) => {
    switch (q.quantity) {
      case 'angle':
        return formatAngle(q.value, units)
      case 'area':
        return formatArea(q.value, units)
      default:
        return formatLength(q.value, units)
    }
  }
  return (
    <PanelShell title="Measure" onCancel={useClose()} error={refs.length ? error : null}>
      <Section title="Selections">
        <RefBox
          label="Faces, edges, vertices or planes"
          refs={refs}
          onChange={setRefs}
          kinds={['vertex', 'edge', 'face', 'datum', 'std_plane']}
          origin
          active
          onActivate={() => {}}
        />
        {refs.length > 0 && (
          <button className="link" onClick={() => setRefs([])}>
            Clear selections
          </button>
        )}
      </Section>
      {result && refs.length > 0 && (
        <Section title="Results">
          <table className="results">
            <tbody>
              {result.values.map((v) => (
                <tr key={v.label}>
                  <th>{v.label}</th>
                  <td>{format(v)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      )}
    </PanelShell>
  )
}

export function MassPanel() {
  const units = useStore((s) => s.snapshot!.units)
  const snapshot = useStore((s) => s.snapshot)
  const openPanel = useStore((s) => s.openPanel)
  const [mp, setMp] = useState<MassProperties | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    call<MassProperties>('mass_properties')
      .then((m) => {
        setMp(m)
        setError(null)
      })
      .catch((e: Error) => setError(e.message))
  }, [snapshot])
  const inertia = (kgmm2: number) => {
    const k = KG[units.mass] * MM[units.length] ** 2
    return `${(kgmm2 / k).toFixed(units.decimals)} ${units.mass}·${units.length}²`
  }
  return (
    <PanelShell title="Mass Properties" onCancel={useClose()} error={error}>
      {mp && (
        <>
          <Section title="Material">
            <p className="hint">
              {mp.material ?? 'No material set'} · {mp.density} kg/m³{' '}
              <button className="link" onClick={() => openPanel({ type: 'material' })}>
                Change…
              </button>
            </p>
          </Section>
          <Section title="Properties">
            <table className="results">
              <tbody>
                <tr>
                  <th>Mass</th>
                  <td>{formatMass(mp.mass, units)}</td>
                </tr>
                <tr>
                  <th>Volume</th>
                  <td>{formatVolume(mp.volume, units)}</td>
                </tr>
                <tr>
                  <th>Surface area</th>
                  <td>{formatArea(mp.area, units)}</td>
                </tr>
                <tr>
                  <th>Center of mass</th>
                  <td>{formatPoint(mp.center, units)}</td>
                </tr>
              </tbody>
            </table>
          </Section>
          <Section title="Principal Moments of Inertia" open={false}>
            <table className="results">
              <tbody>
                {mp.principal_moments.map((m, i) => (
                  <tr key={i}>
                    <th>
                      I{['x', 'y', 'z'][i]} along (
                      {mp.principal_axes[i]!.map((a) => a.toFixed(3)).join(', ')})
                    </th>
                    <td>{inertia(m)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </Section>
        </>
      )}
    </PanelShell>
  )
}

export function MaterialPanel() {
  const current = useStore((s) => s.snapshot!.material)
  const run = useStore((s) => s.run)
  const close = useClose()
  const [list, setList] = useState<{ name: string; density: number }[]>([])
  const [name, setName] = useState(current?.name ?? '')
  const [custom, setCustom] = useState(current?.density ?? 1000)
  useEffect(() => {
    void call<{ name: string; density: number }[]>('materials.list').then(setList)
  }, [])
  const isCustom = name !== '' && !list.some((m) => m.name === name)
  const apply = async () => {
    const params = name === '' ? { name: null } : isCustom ? { name, density: custom } : { name }
    if (await run('doc.set_material', params)) close()
  }
  return (
    <PanelShell title="Material" onOk={() => void apply()} onCancel={close}>
      <Section title="Material">
        <Choice
          label="Material"
          value={isCustom ? '__custom' : name}
          options={[
            { value: '', label: 'None (water density)' },
            ...list.map((m) => ({ value: m.name, label: `${m.name} — ${m.density} kg/m³` })),
            { value: '__custom', label: 'Custom…' },
          ]}
          onChange={(v) => setName(v === '__custom' ? 'Custom material' : v)}
        />
        {isCustom && (
          <>
            <label className="field">
              <span>Name</span>
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                onKeyDown={(e) => e.stopPropagation()}
              />
            </label>
            <NumberField label="Density" unit="kg/m³" value={custom} onChange={setCustom} min={0} />
          </>
        )}
      </Section>
    </PanelShell>
  )
}

const PRESETS: { label: string; units: Omit<Units, 'decimals'> }[] = [
  { label: 'MMGS (millimetre, gram)', units: { length: 'mm', angle: 'deg', mass: 'g' } },
  { label: 'CGS (centimetre, gram)', units: { length: 'cm', angle: 'deg', mass: 'g' } },
  { label: 'MKS (metre, kilogram)', units: { length: 'm', angle: 'deg', mass: 'kg' } },
  { label: 'IPS (inch, pound)', units: { length: 'in', angle: 'deg', mass: 'lb' } },
]

export function UnitsPanel() {
  const current = useStore((s) => s.snapshot!.units)
  const run = useStore((s) => s.run)
  const close = useClose()
  const [u, setU] = useState<Units>(current)
  const preset = PRESETS.findIndex(
    (p) => p.units.length === u.length && p.units.mass === u.mass && p.units.angle === u.angle,
  )
  const apply = async () => {
    if (await run('doc.set_units', { units: u })) close()
  }
  return (
    <PanelShell title="Units" onOk={() => void apply()} onCancel={close}>
      <Section title="Unit System">
        <Choice
          label="System"
          value={preset}
          options={[
            ...PRESETS.map((p, i) => ({ value: i, label: p.label })),
            ...(preset < 0 ? [{ value: -1, label: 'Custom' }] : []),
          ]}
          onChange={(i) => i >= 0 && setU({ ...u, ...PRESETS[i]!.units })}
        />
        <Choice
          label="Length"
          value={u.length}
          options={(['mm', 'cm', 'm', 'in', 'ft'] as const).map((v) => ({ value: v, label: v }))}
          onChange={(length) => setU({ ...u, length })}
        />
        <Choice
          label="Angle"
          value={u.angle}
          options={[
            { value: 'deg', label: 'degrees' },
            { value: 'rad', label: 'radians' },
          ]}
          onChange={(angle) => setU({ ...u, angle })}
        />
        <Choice
          label="Mass"
          value={u.mass}
          options={(['g', 'kg', 'lb'] as const).map((v) => ({ value: v, label: v }))}
          onChange={(mass) => setU({ ...u, mass })}
        />
        <IntField
          label="Decimal places"
          value={u.decimals}
          onChange={(decimals) => setU({ ...u, decimals: Math.min(8, decimals) })}
          min={0}
        />
      </Section>
    </PanelShell>
  )
}

type ExportKind = 'step' | 'iges' | 'stl' | 'dxf'

export function ExportPanel() {
  const path = useStore((s) => s.snapshot!.path)
  const toast = useStore((s) => s.toast)
  const close = useClose()
  const [kind, setKind] = useState<ExportKind>('step')
  const sheets = useStore((s) => s.snapshot!.sheet_metal ?? [])
  const [bodies, setBodies] = useState<string[] | null>(null)
  const [perBody, setPerBody] = useState(false)
  const [notes, setNotes] = useState(true)
  const [r12, setR12] = useState(false)
  const [ascii, setAscii] = useState(false)
  const [deviation, setDeviation] = useState(0.01)
  const [angle, setAngle] = useState(10)
  const go = async () => {
    const base =
      path
        ?.split(/[\\/]/)
        .pop()
        ?.replace(/\.ofp$/i, '') ?? 'Part1'
    const file =
      kind === 'dxf'
        ? await window.openforge.saveDialog('dxf', `${base}-flat.dxf`)
        : await window.openforge.saveDialog('export', `${base}.${kind}`)
    if (!file) return
    const params =
      kind === 'dxf'
        ? {
            path: file,
            bodies,
            per_body: perBody,
            bend_notes: notes,
            dxf_version: r12 ? 'r12' : 'r2018',
          }
        : {
            path: file,
            format:
              kind === 'stl'
                ? { format: 'stl', ascii, deviation, angle: (angle * Math.PI) / 180 }
                : { format: kind },
          }
    try {
      await call('export', params)
      toast(`Exported ${file.split(/[\\/]/).pop()}`, 'info')
      close()
    } catch (e) {
      toast((e as Error).message)
    }
  }
  return (
    <PanelShell title="Export" onOk={() => void go()} onCancel={close}>
      <Section title="Format">
        <Choice
          label="Format"
          value={kind}
          options={[
            { value: 'step', label: 'STEP AP214 (.step)' },
            { value: 'iges', label: 'IGES (.iges)' },
            { value: 'stl', label: 'STL (.stl) for 3D printing' },
            ...(sheets.length ? [{ value: 'dxf' as const, label: 'Flat pattern DXF (.dxf)' }] : []),
          ]}
          onChange={setKind}
        />
        {kind === 'dxf' && (
          <>
            {sheets.length > 1 &&
              sheets.map((sm) => (
                <Check
                  key={sm.feature}
                  label={sm.name ?? 'Sheet metal body'}
                  value={!bodies || (!!sm.body && bodies.includes(sm.body))}
                  onChange={(on) => {
                    const all = sheets.flatMap((x) => (x.body ? [x.body] : []))
                    const cur = bodies ?? all
                    const next = on ? [...cur, sm.body!] : cur.filter((b) => b !== sm.body)
                    setBodies(next.length === all.length ? null : next)
                  }}
                />
              ))}
            {sheets.length > 1 && (
              <Check label="One file per body" value={perBody} onChange={setPerBody} />
            )}
            <Check label="Bend notes" value={notes} onChange={setNotes} />
            <Check label="DXF R12 (for older software)" value={r12} onChange={setR12} />
            <p className="hint">
              True size in mm: outline on CUT, bend lines on BEND_UP and BEND_DOWN, forms on FORM.
            </p>
          </>
        )}
        {kind === 'stl' && (
          <>
            <LengthField label="Deviation" value={deviation} onChange={setDeviation} />
            <NumberField
              label="Angle tolerance"
              unit="°"
              value={angle}
              onChange={setAngle}
              min={0.5}
            />
            <Check label="ASCII instead of binary" value={ascii} onChange={setAscii} />
          </>
        )}
        <p className="hint">
          Exports the model at the rollback bar. Press ✓ to choose where to save.
        </p>
      </Section>
    </PanelShell>
  )
}
