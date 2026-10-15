import { useEffect, useState } from 'react'
import { useStore } from '../store'
import { useDrawing } from './state'

interface Menu {
  x: number
  y: number
  sheet: number
}

/** Sheet tabs under the canvas: switch, add, rename, reorder and delete. */
export function SheetTabs() {
  const drawing = useStore((s) => s.drawing)
  const busy = useStore((s) => s.busy || !!s.panel)
  const current = useDrawing((s) => s.sheet)
  const [renaming, setRenaming] = useState<number | null>(null)
  const [name, setName] = useState('')
  const [menu, setMenu] = useState<Menu | null>(null)

  useEffect(() => {
    if (!menu) return
    const close = () => setMenu(null)
    window.addEventListener('pointerdown', close)
    return () => window.removeEventListener('pointerdown', close)
  }, [menu])

  if (!drawing) return null
  const sheets = drawing.drawing.sheets
  const run = useStore.getState().run

  const rename = async (id: number) => {
    setRenaming(null)
    const n = name.trim()
    if (n && n !== sheets.find((s) => s.id === id)?.name)
      await run('drawing.sheet.update', { id, name: n })
  }

  const item = (label: string, onClick: () => void, disabled = false, danger = false) => (
    <button
      key={label}
      className={danger ? 'danger' : ''}
      disabled={disabled}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={() => {
        setMenu(null)
        onClick()
      }}
    >
      {label}
    </button>
  )

  return (
    <div className="sheet-tabs" role="tablist" aria-label="Sheets">
      {sheets.map((s, i) =>
        renaming === s.id ? (
          <input
            key={s.id}
            autoFocus
            className="sheet-tab-rename"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onBlur={() => void rename(s.id)}
            onKeyDown={(e) => {
              e.stopPropagation()
              if (e.key === 'Enter') void rename(s.id)
              if (e.key === 'Escape') setRenaming(null)
            }}
          />
        ) : (
          <button
            key={s.id}
            role="tab"
            aria-selected={s.id === current}
            className={s.id === current ? 'active' : ''}
            onClick={() => useDrawing.getState().setSheet(s.id)}
            onDoubleClick={() => {
              if (busy) return
              setName(s.name)
              setRenaming(s.id)
            }}
            onContextMenu={(e) => {
              e.preventDefault()
              setMenu({ x: e.clientX, y: e.clientY, sheet: s.id })
            }}
            title={`${s.name} — double-click to rename, right-click for more`}
            data-index={i}
          >
            {s.name}
          </button>
        ),
      )}
      <button
        className="sheet-add"
        title="Add Sheet"
        disabled={busy}
        onClick={async () => {
          const r = await run('drawing.sheet.add')
          if (r) {
            const last = useStore.getState().drawing?.drawing.sheets.at(-1)
            if (last) useDrawing.getState().setSheet(last.id)
          }
        }}
      >
        +
      </button>
      {menu && (
        <div
          className="context-menu"
          style={{ left: menu.x, top: menu.y - 8, transform: 'translateY(-100%)' }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          {item(
            'Properties…',
            () => {
              useDrawing.getState().setSheet(menu.sheet)
              useStore.getState().openPanel({ type: 'sheet_properties', editing: menu.sheet })
            },
            busy,
          )}
          {item(
            'Rename',
            () => {
              setName(sheets.find((s) => s.id === menu.sheet)?.name ?? '')
              setRenaming(menu.sheet)
            },
            busy,
          )}
          {item(
            'Move Left',
            () => {
              const i = sheets.findIndex((s) => s.id === menu.sheet)
              void run('drawing.sheet.update', { id: menu.sheet, index: i - 1 })
            },
            busy || sheets[0]?.id === menu.sheet,
          )}
          {item(
            'Move Right',
            () => {
              const i = sheets.findIndex((s) => s.id === menu.sheet)
              void run('drawing.sheet.update', { id: menu.sheet, index: i + 1 })
            },
            busy || sheets.at(-1)?.id === menu.sheet,
          )}
          {item(
            'Delete',
            () => {
              const s = sheets.find((x) => x.id === menu.sheet)
              if (s && confirm(`Delete ${s.name} with everything on it?`))
                void run('drawing.sheet.delete', { ids: [menu.sheet] })
            },
            busy || sheets.length < 2,
            true,
          )}
        </div>
      )}
    </div>
  )
}
