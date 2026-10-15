import { useState } from 'react'
import type {
  Overrides,
  Projection,
  Scale,
  SheetSize,
  Standard,
  StandardBase,
} from '../../../../shared/tools.gen'
import { call } from '../../core'
import { Check, Choice, NumberField, PanelShell, Section } from '../../panels/fields'
import { useStore } from '../../store'
import { useDrawing } from '../state'
import { scaleLabel, sheetSizeLabel } from '../types'
import { ScaleField, TextField, closeDrawingPanel } from './common'

const SIZES: SheetSize['type'][] = [
  'a0',
  'a1',
  'a2',
  'a3',
  'a4',
  'ansi_a',
  'ansi_b',
  'ansi_c',
  'ansi_d',
  'ansi_e',
  'custom',
]

/** Paper, scale, projection, format and title block source of a sheet. */
export function SheetPropertiesPanel({ editing }: { editing?: number }) {
  const drawing = useStore((s) => s.drawing)!
  const current = useDrawing((s) => s.sheet)
  const id = editing ?? current
  const sheet = drawing.drawing.sheets.find((s) => s.id === id)
  const [name, setName] = useState(sheet?.name ?? '')
  const [size, setSize] = useState<SheetSize>(sheet?.size ?? { type: 'a3' })
  const [portrait, setPortrait] = useState(sheet?.portrait ?? false)
  const [scale, setScale] = useState<Scale>(sheet?.scale ?? { num: 1, den: 1 })
  const [projection, setProjection] = useState<Projection>(sheet?.projection ?? 'first')
  const [template, setTemplate] = useState(sheet?.format.template ?? 'iso')
  const [visible, setVisible] = useState(sheet?.format.visible ?? true)
  const [propsView, setPropsView] = useState<number | null>(sheet?.properties_view ?? null)
  if (!sheet) return null
  const ok = async () => {
    const r = await useStore.getState().run('drawing.sheet.update', {
      id: sheet.id,
      name,
      size,
      portrait,
      scale,
      projection,
      template,
      format_visible: visible,
      properties_view: propsView,
    })
    if (r) await closeDrawingPanel()
  }
  return (
    <PanelShell
      title="Sheet Properties"
      onOk={() => void ok()}
      onCancel={() => void closeDrawingPanel()}
    >
      <TextField label="Name" value={name} onChange={setName} />
      <Section title="Paper">
        <Choice
          label="Size"
          value={size.type}
          options={SIZES.map((t) => ({
            value: t,
            label: t === 'custom' ? 'Custom' : sheetSizeLabel({ type: t } as SheetSize),
          }))}
          onChange={(t) =>
            setSize(
              t === 'custom'
                ? { type: 'custom', width: 420, height: 297 }
                : ({ type: t } as SheetSize),
            )
          }
        />
        {size.type === 'custom' && (
          <>
            <NumberField
              label="Width (mm)"
              value={size.width}
              min={10}
              onChange={(w) => setSize({ ...size, width: w })}
            />
            <NumberField
              label="Height (mm)"
              value={size.height}
              min={10}
              onChange={(h) => setSize({ ...size, height: h })}
            />
          </>
        )}
        {size.type !== 'custom' && (
          <Check label="Portrait" value={portrait} onChange={setPortrait} />
        )}
      </Section>
      <Section title="Scale and projection">
        <ScaleField
          label="Sheet scale"
          value={scale}
          inheritLabel={scaleLabel(scale)}
          onChange={(s) => s && setScale(s)}
        />
        <Choice
          label="Projection"
          value={projection}
          options={[
            { value: 'first', label: 'First angle (ISO E)' },
            { value: 'third', label: 'Third angle (ASME)' },
          ]}
          onChange={setProjection}
        />
      </Section>
      <Section title="Sheet format">
        <Choice
          label="Format"
          value={template === 'iso' || template === 'ansi' ? template : 'file'}
          options={[
            { value: 'iso', label: 'ISO (ISO 5457, ISO 7200 title block)' },
            { value: 'ansi', label: 'ANSI (ASME Y14.1)' },
            { value: 'file', label: 'Sheet format file (.ofdt)…' },
          ]}
          onChange={async (v) => {
            if (v !== 'file') return setTemplate(v)
            const path = await window.openforge.openDialog('template')
            if (path) setTemplate(path)
          }}
        />
        {template !== 'iso' && template !== 'ansi' && <small>{template}</small>}
        <Check label="Show border and title block" value={visible} onChange={setVisible} />
        <Choice
          label="Title block properties from"
          value={propsView ?? 0}
          options={[
            { value: 0, label: 'First view' },
            ...sheet.views.map((v) => ({ value: v.id, label: v.name })),
          ]}
          onChange={(v) => setPropsView(v === 0 ? null : v)}
        />
      </Section>
    </PanelShell>
  )
}

const COMMON_PROPERTIES = [
  'DrawnBy',
  'DrawnDate',
  'CheckedBy',
  'Approved',
  'Company',
  'Title',
  'Tolerance',
]

/** Drawing properties for `$PRPSHEET` title block fields. */
export function DrawingPropertiesPanel() {
  const drawing = useStore((s) => s.drawing)!
  const start = drawing.drawing.properties ?? {}
  const [rows, setRows] = useState<[string, string][]>(() => {
    const out = Object.entries(start)
    for (const k of COMMON_PROPERTIES) if (!(k in start)) out.push([k, ''])
    return out
  })
  const ok = async () => {
    const properties: Record<string, string | null> = {}
    for (const k of Object.keys(start)) properties[k] = null
    for (const [k, v] of rows) if (k.trim() && v.trim()) properties[k.trim()] = v
    const r = await useStore.getState().run('drawing.properties.set', { properties })
    if (r) await closeDrawingPanel()
  }
  return (
    <PanelShell
      title="Drawing Properties"
      onOk={() => void ok()}
      onCancel={() => void closeDrawingPanel()}
    >
      <small>
        Title block fields show these as $PRPSHEET:"Name". The revision comes from a revision table
        when the drawing has one.
      </small>
      <table className="prop-table">
        <tbody>
          {rows.map(([k, v], i) => (
            <tr key={i}>
              <td>
                <input
                  value={k}
                  aria-label="Property name"
                  onChange={(e) =>
                    setRows(rows.map((r, j) => (j === i ? [e.target.value, r[1]] : r)))
                  }
                  onKeyDown={(e) => e.stopPropagation()}
                />
              </td>
              <td>
                <input
                  value={v}
                  aria-label={`Value of ${k}`}
                  onChange={(e) =>
                    setRows(rows.map((r, j) => (j === i ? [r[0], e.target.value] : r)))
                  }
                  onKeyDown={(e) => e.stopPropagation()}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <button onClick={() => setRows([...rows, ['', '']])}>Add property</button>
    </PanelShell>
  )
}

type OverrideKey = keyof Overrides
const LENGTHS: [OverrideKey, string][] = [
  ['text_height', 'Text height (mm)'],
  ['label_height', 'View label height (mm)'],
  ['arrow_length', 'Arrow length (mm)'],
  ['thick_line', 'Visible line weight (mm)'],
  ['thin_line', 'Thin line weight (mm)'],
  ['extension_gap', 'Extension line gap (mm)'],
  ['extension_overshoot', 'Extension line overshoot (mm)'],
  ['tolerance_scale', 'Tolerance text scale'],
]
const COUNTS: [OverrideKey, string][] = [
  ['precision', 'Linear decimals'],
  ['angle_precision', 'Angle decimals'],
]
const FLAGS: [OverrideKey, string][] = [
  ['trailing_zeros', 'Trailing zeros'],
  ['unidirectional', 'Horizontal dimension text'],
  ['dual_units', 'Dual units (inches)'],
  ['decimal_comma', 'Decimal comma'],
]

/** ISO or ANSI base with overrides; load and save `.ofstd` files. */
export function DraftingStandardPanel() {
  const drawing = useStore((s) => s.drawing)!
  const [std, setStd] = useState<Standard>(drawing.drawing.standard)
  const base: StandardBase = std.base ?? 'iso'
  const o: Overrides = std.overrides ?? {}
  const set = (k: OverrideKey, v: number | boolean | null) =>
    setStd({ ...std, overrides: { ...o, [k]: v } })
  const ok = async (save?: string) => {
    const overrides = Object.fromEntries(
      Object.entries(o).filter(([, v]) => v !== null && v !== undefined),
    )
    const r = await useStore.getState().run('drawing.standard.set', {
      standard: { base, overrides },
      ...(save ? { save } : {}),
    })
    if (r && !save) await closeDrawingPanel()
    if (r && save) useStore.getState().toast(`Saved ${save}`, 'info')
  }
  return (
    <PanelShell
      title="Drafting Standard"
      onOk={() => void ok()}
      onCancel={() => void closeDrawingPanel()}
    >
      <Choice
        label="Base standard"
        value={base}
        options={[
          { value: 'iso', label: 'ISO' },
          { value: 'ansi', label: 'ANSI' },
        ]}
        onChange={(b) => setStd({ ...std, base: b })}
      />
      <small>Empty fields follow the base standard.</small>
      <Section title="Text, arrows and lines">
        {LENGTHS.map(([k, label]) => (
          <OptionalNumber
            key={k}
            label={label}
            value={o[k] as number | null | undefined}
            onChange={(v) => set(k, v)}
          />
        ))}
      </Section>
      <Section title="Numbers">
        {COUNTS.map(([k, label]) => (
          <OptionalNumber
            key={k}
            label={label}
            integer
            value={o[k] as number | null | undefined}
            onChange={(v) => set(k, v)}
          />
        ))}
        {FLAGS.map(([k, label]) => (
          <Choice
            key={k}
            label={label}
            value={o[k] === undefined || o[k] === null ? 'default' : o[k] ? 'yes' : 'no'}
            options={[
              { value: 'default', label: 'Standard default' },
              { value: 'yes', label: 'Yes' },
              { value: 'no', label: 'No' },
            ]}
            onChange={(v) => set(k, v === 'default' ? null : v === 'yes')}
          />
        ))}
      </Section>
      <Section title="Files">
        <div className="button-row">
          <button
            onClick={async () => {
              const path = await window.openforge.openDialog('standard')
              if (!path) return
              const r = await useStore.getState().run('drawing.standard.set', { load: path })
              if (r) setStd(useStore.getState().drawing!.drawing.standard)
            }}
          >
            Load…
          </button>
          <button
            onClick={async () => {
              const path = await window.openforge.saveDialog('standard', 'standard.ofstd')
              if (path) await ok(path)
            }}
          >
            Save as file…
          </button>
        </div>
      </Section>
    </PanelShell>
  )
}

function OptionalNumber(props: {
  label: string
  value: number | null | undefined
  integer?: boolean
  onChange: (v: number | null) => void
}) {
  const [text, setText] = useState(
    props.value === null || props.value === undefined ? '' : String(props.value),
  )
  const [bad, setBad] = useState(false)
  const commit = () => {
    if (!text.trim()) {
      setBad(false)
      return props.onChange(null)
    }
    const v = Number(text)
    const okv = Number.isFinite(v) && v >= 0 && (!props.integer || Number.isInteger(v))
    setBad(!okv)
    if (okv) props.onChange(v)
  }
  return (
    <label className="field">
      <span>{props.label}</span>
      <input
        className={bad ? 'bad' : ''}
        value={text}
        placeholder="Standard"
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

/** Export sheets to PDF or DXF. */
export function DrawingExportPanel() {
  const drawing = useStore((s) => s.drawing)!
  const sheets = drawing.drawing.sheets
  const [format, setFormat] = useState<'pdf' | 'dxf'>('pdf')
  const [chosen, setChosen] = useState<number[]>(sheets.map((s) => s.id))
  const [mono, setMono] = useState(false)
  const [version, setVersion] = useState<'r2018' | 'r12'>('r2018')
  const [perSheet, setPerSheet] = useState(false)
  const name =
    drawing.path
      ?.split(/[\\/]/)
      .pop()
      ?.replace(/\.ofd$/i, '') ?? 'Draw1'
  const ok = async () => {
    const path = await window.openforge.saveDialog(format, `${name}.${format}`)
    if (!path) return
    try {
      const r = await call<{ paths: string[] }>('drawing.export', {
        path,
        sheets: chosen.length === sheets.length ? undefined : chosen,
        ...(format === 'pdf'
          ? { monochrome: mono }
          : { dxf_version: version, per_sheet: perSheet }),
      })
      useStore.getState().toast(`Exported ${r.paths.length} file(s)`, 'info')
      await closeDrawingPanel()
    } catch (e) {
      useStore.getState().toast((e as Error).message)
    }
  }
  return (
    <PanelShell
      title="Export Drawing"
      onOk={() => void ok()}
      okDisabled={!chosen.length}
      onCancel={() => void closeDrawingPanel()}
    >
      <Choice
        label="Format"
        value={format}
        options={[
          { value: 'pdf', label: 'PDF (vector, true paper size)' },
          { value: 'dxf', label: 'DXF (true scale, layers)' },
        ]}
        onChange={setFormat}
      />
      <Section title="Sheets">
        {sheets.map((s) => (
          <Check
            key={s.id}
            label={s.name}
            value={chosen.includes(s.id)}
            onChange={(v) => setChosen(v ? [...chosen, s.id] : chosen.filter((x) => x !== s.id))}
          />
        ))}
      </Section>
      {format === 'pdf' ? (
        <Check label="Black and white" value={mono} onChange={setMono} />
      ) : (
        <>
          <Choice
            label="DXF version"
            value={version}
            options={[
              { value: 'r2018', label: '2018 (AC1032): layouts, lineweights' },
              { value: 'r12', label: 'R12 (AC1009): widest support' },
            ]}
            onChange={setVersion}
          />
          <Check label="One file per sheet" value={perSheet} onChange={setPerSheet} />
        </>
      )}
    </PanelShell>
  )
}

/** Part custom properties and which dimensions drawings import. */
export function PartPropertiesPanel() {
  const snapshot = useStore((s) => s.snapshot)!
  const info = snapshot as typeof snapshot & {
    properties?: Record<string, { value: string; kind?: string }>
    dimension_display?: Record<string, { show?: boolean; name?: string | null }>
  }
  const start = info.properties ?? {}
  const [rows, setRows] = useState<[string, string][]>(() => {
    const out = Object.entries(start).map(([k, v]) => [k, v.value] as [string, string])
    for (const k of ['PartNo', 'Description', 'Revision']) if (!(k in start)) out.push([k, ''])
    return out
  })
  const [display, setDisplay] = useState(info.dimension_display ?? {})
  const ok = async () => {
    const properties: Record<string, { value: string } | null> = {}
    for (const k of Object.keys(start)) properties[k] = null
    for (const [k, v] of rows) if (k.trim() && v.trim()) properties[k.trim()] = { value: v }
    const run = useStore.getState().run
    if (!(await run('doc.properties.set', { properties }))) return
    for (const p of snapshot.parameters) {
      const was = info.dimension_display?.[p.key]
      const now = display[p.key]
      if (JSON.stringify(was ?? {}) === JSON.stringify(now ?? {})) continue
      await run('doc.dimension_display.set', {
        key: p.key,
        show: now?.show ?? true,
        name: now?.name ?? null,
      })
    }
    await useStore.getState().closePanel()
  }
  return (
    <PanelShell
      title="Part Properties"
      onOk={() => void ok()}
      onCancel={() => void useStore.getState().closePanel()}
    >
      <Section title="Custom properties">
        <small>
          Values may link to {'{mass}'}, {'{material}'}, {'{volume}'} or {'{param:f2.end1.depth}'}.
        </small>
        <table className="prop-table">
          <tbody>
            {rows.map(([k, v], i) => (
              <tr key={i}>
                <td>
                  <input
                    value={k}
                    aria-label="Property name"
                    onChange={(e) =>
                      setRows(rows.map((r, j) => (j === i ? [e.target.value, r[1]] : r)))
                    }
                    onKeyDown={(e) => e.stopPropagation()}
                  />
                </td>
                <td>
                  <input
                    value={v}
                    aria-label={`Value of ${k}`}
                    onChange={(e) =>
                      setRows(rows.map((r, j) => (j === i ? [r[0], e.target.value] : r)))
                    }
                    onKeyDown={(e) => e.stopPropagation()}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <button onClick={() => setRows([...rows, ['', '']])}>Add property</button>
      </Section>
      <Section title="Dimensions in drawings" open={false}>
        <small>Model Items imports the checked dimensions, showing the name under the value.</small>
        {snapshot.parameters
          .filter((p) => !p.integer)
          .map((p) => {
            const d = display[p.key]
            return (
              <div key={p.key} className="dimension-display">
                <Check
                  label={p.key}
                  value={d?.show ?? true}
                  onChange={(show) => setDisplay({ ...display, [p.key]: { ...d, show } })}
                />
                <input
                  value={d?.name ?? ''}
                  placeholder="Name in drawings"
                  aria-label={`Drawing name of ${p.key}`}
                  onChange={(e) =>
                    setDisplay({ ...display, [p.key]: { ...d, name: e.target.value || null } })
                  }
                  onKeyDown={(e) => e.stopPropagation()}
                />
              </div>
            )
          })}
      </Section>
    </PanelShell>
  )
}
