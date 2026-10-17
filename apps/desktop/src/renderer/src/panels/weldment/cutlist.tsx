import { useEffect, useState } from 'react'
import type { CutListSettings } from '../../../../shared/tools.gen'
import { call } from '../../core'
import { useStore } from '../../store'
import { Check, Choice, PanelShell, Section } from '../fields'
import { TextField } from '../sheet-metal/common'

export interface CutListItem {
  number: number | null
  name: string
  kind: 'member' | 'sheet_metal' | 'body'
  bodies: string[]
  quantity: number
  folder: string | null
  excluded: boolean
  properties: Record<string, string>
}

export interface WeldFolder {
  beads: { feature: number; size: number; length: number; symbol: string | null }[]
  total_length: number
  total_mass: number
}

export interface CutList {
  settings: CutListSettings
  items: CutListItem[]
  welds: WeldFolder
}

/** The part's cut list, refetched whenever the model changes. */
export function useCutList(): CutList | null {
  const revision = useStore((s) => s.snapshot?.revision)
  const weldment = useStore((s) => s.snapshot?.features.some((f) => f.type === 'weldment'))
  const [list, setList] = useState<CutList | null>(null)
  useEffect(() => {
    if (!weldment) return
    let live = true
    call<CutList>('doc.cut_list')
      .then((l) => live && setList(l))
      .catch(() => live && setList(null))
    return () => {
      live = false
    }
  }, [revision, weldment])
  return weldment ? list : null
}

/** Edit the cut list: item descriptions, exclusions, folders, grouping. */
export function CutListPanel() {
  const list = useCutList()
  const run = useStore((s) => s.run)
  const close = useStore((s) => s.closePanel)
  const [edited, setSettings] = useState<CutListSettings | null>(null)
  // The saved settings until the first edit.
  const settings = edited ?? list?.settings ?? null
  if (!list || !settings) return null
  const key = (item: CutListItem) => item.bodies[0]!
  const override = (item: CutListItem, prop: string) =>
    settings.overrides?.[key(item)]?.[prop] ?? ''
  const setOverride = (item: CutListItem, prop: string, value: string) => {
    const overrides = { ...(settings.overrides ?? {}) }
    const mine = { ...(overrides[key(item)] ?? {}) }
    if (value) mine[prop] = value
    else delete mine[prop]
    if (Object.keys(mine).length) overrides[key(item)] = mine
    else delete overrides[key(item)]
    setSettings({ ...settings, overrides })
  }
  const setFolder = (item: CutListItem, name: string) => {
    const folders = (settings.sub_weldments ?? [])
      .map((f) => ({ ...f, bodies: f.bodies.filter((b) => !item.bodies.includes(b)) }))
      .filter((f) => f.bodies.length)
    if (name) {
      const f = folders.find((x) => x.name === name)
      if (f) f.bodies.push(...item.bodies)
      else folders.push({ name, bodies: [...item.bodies] })
    }
    setSettings({ ...settings, sub_weldments: folders })
  }
  const setExcluded = (item: CutListItem, on: boolean) => {
    const rest = (settings.excluded ?? []).filter((b) => !item.bodies.includes(b))
    setSettings({ ...settings, excluded: on ? [...rest, ...item.bodies] : rest })
  }
  return (
    <PanelShell
      title="Cut List"
      onOk={() =>
        void run('doc.cut_list.set', { settings }).then((ok) => {
          if (ok) void close()
        })
      }
      onCancel={() => void close()}
    >
      <Section title="Options">
        <Check
          label="Collect identical bodies"
          value={!settings.separate}
          onChange={(on) => setSettings({ ...settings, separate: !on })}
        />
        <Choice
          label="Sort"
          value={settings.sort ?? 'default'}
          options={[
            { value: 'default', label: 'Members first, as made' },
            { value: 'profile', label: 'By profile and length' },
          ]}
          onChange={(sort) => setSettings({ ...settings, sort })}
        />
        <TextField
          label="Item names"
          value={settings.name ?? ''}
          placeholder="{DESCRIPTION} x {LENGTH}"
          onChange={(name) => setSettings({ ...settings, name: name || null })}
        />
      </Section>
      {list.items.map((item, i) => (
        <Section
          key={key(item)}
          title={`${item.name} · ${item.properties['DESCRIPTION']} × ${item.quantity}`}
          open={i < 3}
        >
          <table className="prop-table">
            <tbody>
              {Object.entries(item.properties)
                .filter(([k]) => k !== 'DESCRIPTION')
                .map(([k, v]) => (
                  <tr key={k}>
                    <td>{k}</td>
                    <td>{v}</td>
                  </tr>
                ))}
            </tbody>
          </table>
          <label className="field">
            <span>Description</span>
            <input
              value={override(item, 'DESCRIPTION')}
              placeholder={item.properties['DESCRIPTION']}
              onChange={(e) => setOverride(item, 'DESCRIPTION', e.target.value)}
            />
          </label>
          <label className="field">
            <span>Sub-weldment</span>
            <input
              defaultValue={item.folder ?? ''}
              placeholder="None"
              onBlur={(e) => setFolder(item, e.target.value.trim())}
            />
          </label>
          <Check
            label="Exclude from cut list"
            value={(settings.excluded ?? []).some((b) => item.bodies.includes(b))}
            onChange={(on) => setExcluded(item, on)}
          />
        </Section>
      ))}
      <Section title={`Weld Folder · ${list.welds.beads.length} beads`} open={false}>
        <small>
          Total length {list.welds.total_length.toFixed(1)} mm · mass{' '}
          {list.welds.total_mass.toFixed(3)} kg
        </small>
      </Section>
    </PanelShell>
  )
}
