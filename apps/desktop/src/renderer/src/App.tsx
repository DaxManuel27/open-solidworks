import { useEffect } from 'react'
import { CommandManager, SKETCH_KEYS, beginSketch } from './CommandManager'
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
  const view = VIEW_COMMANDS[command]
  if (view) return s.setView(view)
  switch (command) {
    case 'new':
      return s.newDoc()
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
      return s.importFile()
    case 'export':
      return s.exportFile()
    case 'undo':
      return s.undo()
    case 'redo':
      return s.redo()
    case 'units':
      return s.openPanel({ type: 'units' })
    case 'material':
      return s.openPanel({ type: 'material' })
    case 'mass':
      return s.openPanel({ type: 'mass' })
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

  useEffect(() => {
    void useStore.getState().init()
    const offMenu = window.openforge.onMenu((c) => void onMenu(c))
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
      offCrash()
    }
  }, [])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTyping(e) || e.metaKey || e.ctrlKey || e.altKey) return
      const s = useStore.getState()
      const k = e.key.toLowerCase()
      if (k === 'f') s.setView('fit')
      else if (s.sketch && SKETCH_KEYS[k]) s.setSketchTool(SKETCH_KEYS[k])
      else if (!s.sketch && !s.panel && k === 's') beginSketch()
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
    (sketch
      ? `Editing ${editing?.name ?? 'sketch'}`
      : selection.length
        ? `${selection.length} selected`
        : 'Ready')

  return (
    <div className="app">
      <CommandManager />
      <div className="main">
        <aside className="sidebar">
          {sketch ? <SketchPanel /> : panel ? <PanelHost panel={panel} /> : <FeatureTree />}
        </aside>
        <div className="of-viewport">
          <Viewport />
          <HeadsUpToolbar />
        </div>
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
        <span title="Units (Tools ▸ Units)">{unitLabel}</span>
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
