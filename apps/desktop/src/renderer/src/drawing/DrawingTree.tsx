import { useEffect, useState } from 'react'
import { useStore } from '../store'
import { deleteSelected } from './SheetCanvas'
import { type SheetPick, pickKey, useDrawing } from './state'
import type { Annotation, DrawingView, Sheet } from './types'
import { parentOf, scaleLabel, sheetSizeLabel } from './types'

interface Menu {
  x: number
  y: number
  pick: SheetPick
  sheet: number
}

const VIEW_ICONS: Record<string, string> = {
  model: '▣',
  projected: '⇲',
  auxiliary: '⟋',
  section: '⊟',
  detail: '◎',
}

/** A short label for an annotation in the tree. */
export function annotationLabel(a: Annotation): string {
  switch (a.type) {
    case 'note':
      return `Note: ${a.text.split('\n')[0]!.slice(0, 24)}`
    case 'dimension':
      return a.dim === 'model'
        ? `Model dimension ${a.key}`
        : `Dimension (${a.dim.replace('_', ' ')})`
    case 'balloon':
      return 'Balloon'
    case 'hole_callout':
      return 'Hole callout'
    case 'center_mark':
      return 'Center mark'
    case 'centerline':
      return 'Centerline'
    case 'surface_finish':
      return 'Surface finish'
    case 'datum_feature':
      return `Datum ${a.label}`
    case 'datum_target':
      return `Datum target ${a.label}`
    case 'gtol':
      return 'Geometric tolerance'
    case 'weld':
      return 'Weld symbol'
    case 'area_hatch':
      return 'Area hatch'
    case 'revision_cloud':
      return 'Revision cloud'
    case 'revision_symbol':
      return `Revision ${a.revision}`
    case 'table':
      return `${
        {
          bom: 'Bill of materials',
          hole: 'Hole table',
          revision: 'Revision table',
          general: 'General table',
          bend: 'Bend table',
          punch: 'Punch table',
          cut_list: 'Cut list table',
          weld: 'Weld table',
          general_tolerance: 'General tolerances',
        }[a.table]
      }`
  }
}

/** Sheets, views (derived views under their parents) and annotations. */
export function DrawingTree() {
  const drawing = useStore((s) => s.drawing)
  const busy = useStore((s) => s.busy || !!s.panel)
  const current = useDrawing((s) => s.sheet)
  const selection = useDrawing((s) => s.selection)
  const [open, setOpen] = useState<Record<string, boolean>>({})
  const [menu, setMenu] = useState<Menu | null>(null)

  useEffect(() => {
    if (!menu) return
    const close = () => setMenu(null)
    window.addEventListener('pointerdown', close)
    return () => window.removeEventListener('pointerdown', close)
  }, [menu])

  if (!drawing) return null
  const doc = drawing.drawing
  const name =
    drawing.path
      ?.split(/[\\/]/)
      .pop()
      ?.replace(/\.ofd$/i, '') ?? 'Draw1'
  const selected = new Set(selection.map(pickKey))

  const pickView = (sheet: Sheet, v: DrawingView, additive: boolean) => {
    useDrawing.getState().setSheet(sheet.id)
    useDrawing.getState().select({ type: 'view', view: v.id, at: v.origin }, additive)
  }
  const pickNote = (sheet: Sheet, a: Annotation, additive: boolean) => {
    useDrawing.getState().setSheet(sheet.id)
    useDrawing.getState().select({ type: 'annotation', id: a.id, at: [0, 0] }, additive)
  }

  const viewRow = (sheet: Sheet, v: DrawingView, depth: number): React.ReactNode => {
    const children = sheet.views.filter((c) => parentOf(c) === v.id)
    const key = `v:${v.id}`
    const pick: SheetPick = { type: 'view', view: v.id, at: v.origin }
    const notes = (sheet.annotations ?? []).filter((a) => a.attach === v.id)
    return (
      <div key={key}>
        <div
          className={`tree-item ${selected.has(pickKey(pick)) ? 'selected' : ''}`}
          style={{ paddingLeft: 8 + depth * 14 }}
          onClick={(e) => pickView(sheet, v, e.shiftKey || e.metaKey || e.ctrlKey)}
          onDoubleClick={() =>
            !busy && useStore.getState().openPanel({ type: 'view_properties', editing: v.id })
          }
          onContextMenu={(e) => {
            e.preventDefault()
            pickView(sheet, v, false)
            setMenu({ x: e.clientX, y: e.clientY, pick, sheet: sheet.id })
          }}
          title={v.label ? `${v.name} (${v.label})` : v.name}
        >
          {(children.length > 0 || notes.length > 0) && (
            <button
              className="tree-toggle"
              onClick={(e) => {
                e.stopPropagation()
                setOpen({ ...open, [key]: !(open[key] ?? true) })
              }}
            >
              {(open[key] ?? true) ? '▾' : '▸'}
            </button>
          )}
          <span className="icon">{VIEW_ICONS[v.kind.type] ?? '▣'}</span>
          <span className="name">{v.name}</span>
          {v.scale && <span className="note">{scaleLabel(v.scale)}</span>}
        </div>
        {(open[key] ?? true) && (
          <>
            {notes.map((a) => noteRow(sheet, a, depth + 1))}
            {children.map((c) => viewRow(sheet, c, depth + 1))}
          </>
        )}
      </div>
    )
  }

  const noteRow = (sheet: Sheet, a: Annotation, depth: number) => {
    const pick: SheetPick = { type: 'annotation', id: a.id, at: [0, 0] }
    return (
      <div
        key={`a:${a.id}`}
        className={`tree-item ${selected.has(pickKey(pick)) ? 'selected' : ''}`}
        style={{ paddingLeft: 8 + depth * 14 }}
        onClick={(e) => pickNote(sheet, a, e.shiftKey || e.metaKey || e.ctrlKey)}
        onDoubleClick={() =>
          !busy && useStore.getState().openPanel({ type: 'annotation_properties', editing: a.id })
        }
        onContextMenu={(e) => {
          e.preventDefault()
          pickNote(sheet, a, false)
          setMenu({ x: e.clientX, y: e.clientY, pick, sheet: sheet.id })
        }}
      >
        <span className="icon">
          {a.type === 'table' ? '▦' : a.type === 'dimension' ? '↔' : 'A'}
        </span>
        <span className="name">{annotationLabel(a)}</span>
      </div>
    )
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
    <div className="tree drawing-tree">
      <div className="tree-header">{name}</div>
      {doc.sheets.map((sheet) => {
        const key = `s:${sheet.id}`
        const roots = sheet.views.filter((v) => parentOf(v) === null)
        const loose = (sheet.annotations ?? []).filter(
          (a) => a.attach === undefined || a.attach === null,
        )
        return (
          <div key={key}>
            <div
              className={`tree-item tree-sheet ${sheet.id === current ? 'current' : ''}`}
              onClick={() => useDrawing.getState().setSheet(sheet.id)}
              onDoubleClick={() =>
                !busy &&
                useStore.getState().openPanel({ type: 'sheet_properties', editing: sheet.id })
              }
            >
              <button
                className="tree-toggle"
                onClick={(e) => {
                  e.stopPropagation()
                  setOpen({ ...open, [key]: !(open[key] ?? true) })
                }}
              >
                {(open[key] ?? true) ? '▾' : '▸'}
              </button>
              <span className="icon">▭</span>
              <span className="name">{sheet.name}</span>
              <span className="note">
                {sheetSizeLabel(sheet.size)} · {scaleLabel(sheet.scale)}
              </span>
            </div>
            {(open[key] ?? true) && (
              <>
                <div
                  className="tree-item"
                  style={{ paddingLeft: 22 }}
                  onDoubleClick={() =>
                    !busy &&
                    useStore.getState().openPanel({ type: 'sheet_properties', editing: sheet.id })
                  }
                >
                  <span className="icon">▤</span>
                  <span className="name">
                    Sheet Format ({sheet.format.template.split(/[\\/]/).pop()})
                  </span>
                </div>
                {roots.map((v) => viewRow(sheet, v, 1))}
                {loose.map((a) => noteRow(sheet, a, 1))}
              </>
            )}
          </div>
        )
      })}
      {menu && (
        <div
          className="context-menu"
          style={{ left: menu.x, top: menu.y }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          {menu.pick.type === 'view' &&
            item('Properties…', () =>
              useStore.getState().openPanel({
                type: 'view_properties',
                editing: (menu.pick as { view: number }).view,
              }),
            )}
          {menu.pick.type === 'annotation' &&
            item('Properties…', () =>
              useStore.getState().openPanel({
                type: 'annotation_properties',
                editing: (menu.pick as { id: number }).id,
              }),
            )}
          {menu.pick.type === 'view' &&
            item(
              'Use for title block properties',
              () =>
                void useStore.getState().run('drawing.sheet.update', {
                  id: menu.sheet,
                  properties_view: (menu.pick as { view: number }).view,
                }),
            )}
          {item(
            'Delete',
            () =>
              void deleteSelected(
                menu.pick.type === 'view' ? [menu.pick.view] : [],
                menu.pick.type === 'annotation' ? [menu.pick.id] : [],
              ),
            busy,
            true,
          )}
        </div>
      )}
    </div>
  )
}
