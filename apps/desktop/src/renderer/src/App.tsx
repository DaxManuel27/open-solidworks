import { useEffect } from 'react'
import { Assistant } from './assistant/Assistant'
import { AssemblyTree } from './AssemblyTree'
import { CommandManager, SKETCH_KEYS, beginSketch } from './CommandManager'
import { DrawingTree } from './drawing/DrawingTree'
import { SheetCanvas } from './drawing/SheetCanvas'
import { SheetTabs } from './drawing/SheetTabs'
import { useDrawing } from './drawing/state'
import { HeadsUpToolbar } from './toolbars/HeadsUpToolbar'
import { FeatureTree } from './FeatureTree'
import { PanelHost } from './panels/PanelHost'
import { SketchPanel } from './panels/sketch'
import { type ViewName, useStore } from './store'
import { Viewport } from './viewport/Viewport'

const VIEW_COMMANDS: Record<string, ViewName> = {
  'zoom-fit': 'fit',
  'view-front': 'front',
  'view-back': 'back',
  'view-left': 'left',
  'view-right': 'right',
  'view-top': 'top',
  'view-bottom': 'bottom',
  'view-iso': 'iso',
  'view-normal': 'normal',
}

async function onMenu(command: string): Promise<void> {
  const s = useStore.getState()
  if (s.drawing && command === 'zoom-fit') return useDrawing.getState().fit()
  const view = VIEW_COMMANDS[command]
  if (view) return s.drawing ? undefined : s.setView(view)
  switch (command) {
    case 'make-drawing':
      return s.makeDrawing()
    case 'print':
      return s.print()
    case 'new':
      return s.newDoc()
    case 'new-assembly':
      return s.newAssembly()
    case 'import-assembly':
      return s.importAssembly()
    case 'open':
      return s.open()
    case 'save':
      return void (await s.save())
    case 'save-as':
      return void (await s.save(true))
    case 'save-and-close':
      if (await s.save()) window.openforge.closeWindow()
      return
    case 'import':
      if (s.drawing) return s.toast('Drawings show saved models; add a model view', 'info')
      return s.assembly ? s.importAssembly() : s.importFile()
    case 'export':
      return s.drawing ? s.openPanel({ type: 'drawing_export' }) : s.exportFile()
    case 'undo':
      return s.undo()
    case 'redo':
      return s.redo()
    case 'units':
      return s.openPanel({ type: s.drawing ? 'drafting_standard' : 'units' })
    case 'material':
      if (s.assembly || s.drawing)
        return s.toast('Materials belong to parts; open a part to set one', 'info')
      return s.openPanel({ type: 'material' })
    case 'mass':
      if (s.drawing) return s.toast('Open the part or assembly for mass properties', 'info')
      return s.openPanel({ type: s.assembly ? 'asm_mass' : 'mass' })
  }
}

function isTyping(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement
  return t.tagName === 'INPUT' || t.tagName === 'SELECT' || t.tagName === 'TEXTAREA'
}

export function App() {
  const snapshot = useStore((s) => s.snapshot)
  const panel = useStore((s) => s.panel)
  const sketch = useStore((s) => s.sketch)
  const toasts = useStore((s) => s.toasts)
  const status = useStore((s) => s.status)
  const busy = useStore((s) => s.busy)
  const selection = useStore((s) => s.selection)
  const assembly = useStore((s) => s.assembly)
  const asmComponents = useStore((s) => s.asmComponents)
  const drawing = useStore((s) => s.drawing)
  const sheetTarget = useDrawing((s) => s.target)
  const sheetSelection = useDrawing((s) => s.selection)

  useEffect(() => {
    void useStore.getState().init()
    const offMenu = window.openforge.onMenu((c) => void onMenu(c))
    // A part saved in another window may be used here.
    const offSaved = window.openforge.onFileSaved(() => {
      const s = useStore.getState()
      if (s.assembly && !s.panel) void s.run('asm.reload')
      // Drawings redraw from the saved models.
      if (s.drawing) void useDrawing.getState().refresh(true)
    })
    const offCrash = window.openforge.onCoreCrashed(() => {
      useStore.setState({
        sketch: null,
        panel: null,
        selection: [],
        previewScene: null,
        pickTarget: null,
      })
      useStore
        .getState()
        .toast(
          'The geometry core stopped unexpectedly and was restarted. Unsaved changes were lost.',
        )
      setTimeout(() => void useStore.getState().init(), 800)
    })
    return () => {
      offMenu()
      offSaved()
      offCrash()
    }
  }, [])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTyping(e) || e.metaKey || e.ctrlKey || e.altKey) return
      const s = useStore.getState()
      const k = e.key.toLowerCase()
      if (s.drawing) {
        if (k === 'f') useDrawing.getState().fit()
        return
      }
      if (k === 'f') s.setView('fit')
      else if (s.sketch && SKETCH_KEYS[k]) s.setSketchTool(SKETCH_KEYS[k])
      else if (!s.sketch && !s.panel && !s.assembly && k === 's') beginSketch()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  if (!snapshot) {
    return <div className="loading">Starting OpenForge…</div>
  }

  const units = snapshot.units
  const unitLabel = `${units.length.toUpperCase()} · ${units.mass} · ${units.angle === 'deg' ? '°' : 'rad'}`
  const editing =
    snapshot.editing !== null ? snapshot.features.find((f) => f.id === snapshot.editing) : null
  const statusText =
    status ||
    (drawing
      ? sheetTarget
        ? sheetTarget.prompt
        : sheetSelection.length
          ? `${sheetSelection.length} selected — drag to move, Delete removes, double-click for properties`
          : 'Ready — wheel zooms, right-drag pans, F fits the sheet'
      : sketch
        ? `Editing ${editing?.name ?? 'sketch'}`
        : assembly
          ? asmComponents.length
            ? `${asmComponents.length} component(s) selected — Alt+drag moves a component`
            : assembly.diagnostics.solved
              ? 'Ready'
              : 'Some mates cannot be satisfied'
          : selection.length
            ? `${selection.length} selected`
            : 'Ready')

  return (
    <div className="app">
      <CommandManager />
      <div className="main">
        <aside className="sidebar">
          {sketch ? (
            <SketchPanel />
          ) : panel ? (
            <PanelHost panel={panel} />
          ) : drawing ? (
            <DrawingTree />
          ) : assembly ? (
            <AssemblyTree />
          ) : (
            <FeatureTree />
          )}
        </aside>
        {drawing ? (
          <div className="of-viewport drawing-area">
            <SheetCanvas />
            <SheetTabs />
          </div>
        ) : (
          <div className="of-viewport">
            <Viewport />
            <HeadsUpToolbar />
          </div>
        )}
        <Assistant />
      </div>
      <footer className="statusbar">
        <span>{statusText}</span>
        <span className="spacer" />
        {busy && <span className="busy">Rebuilding…</span>}
        {sketch && (
          <span className={`sketch-status ${sketch.status.split(' ')[0]!.toLowerCase()}`}>
            {sketch.status}
          </span>
        )}
        {snapshot.rollback !== null && <span>Rolled back</span>}
        {drawing ? (
          <span title="Drafting standard (Tools ▸ Units)">
            {(drawing.drawing.standard.base ?? 'iso').toUpperCase()}
          </span>
        ) : (
          <span title="Units (Tools ▸ Units)">{unitLabel}</span>
        )}
      </footer>
      <div className="toasts">
        {toasts.map((t) => (
          <div
            key={t.id}
            className={`toast ${t.kind}`}
            onClick={() => useStore.getState().dismiss(t.id)}
          >
            {t.text}
          </div>
        ))}
      </div>
    </div>
  )
}
