import { useEffect, useRef, useState } from 'react'
import { useStore } from './store'
import { Toolbar, ToolbarControl } from './toolbars/controls'
import {
  annotationGroups,
  drawingEvaluateGroups,
  drawingGroups,
  drawingTabOrder,
  drawingTabs,
  assemblyEvaluateGroups,
  assemblyGroups,
  assemblyTabOrder,
  assemblyTabs,
  availableTabs as partTabs,
  evaluateGroups,
  featureGroups,
  sketchGroups,
  tabOrder as partTabOrder,
  unavailableTabs,
} from './toolbars/layout'
import { StandardToolbar } from './toolbars/StandardToolbar'
import './toolbars/toolbars.css'
export { beginSketch, startFeature } from './toolbars/actions'
export { SKETCH_KEYS } from './toolbars/commands'

export function CommandManager() {
  const tab = useStore((s) => s.tab)
  const editing = useStore((s) => !!s.sketch)
  const isAssembly = useStore((s) => !!s.assembly)
  const isDrawing = useStore((s) => !!s.drawing)
  const availableTabs = isDrawing ? drawingTabs : isAssembly ? assemblyTabs : partTabs
  const tabOrder = isDrawing ? drawingTabOrder : isAssembly ? assemblyTabOrder : partTabOrder
  const wasEditing = useRef(editing)
  useEffect(() => {
    if (wasEditing.current && !editing) useStore.getState().setTab('features')
    wasEditing.current = editing
  }, [editing])
  const track = useRef<HTMLDivElement>(null)
  const [overflow, setOverflow] = useState({ left: false, right: false })
  const groups =
    tab === 'drawing'
      ? drawingGroups
      : tab === 'annotation'
        ? annotationGroups
        : tab === 'assembly'
          ? assemblyGroups
          : tab === 'evaluate'
            ? isDrawing
              ? drawingEvaluateGroups
              : isAssembly
                ? assemblyEvaluateGroups
                : evaluateGroups
            : tab === 'features'
              ? featureGroups
              : sketchGroups(editing)
  useEffect(() => {
    const el = track.current!
    const update = () =>
      setOverflow({
        left: el.scrollLeft > 1,
        right: el.scrollLeft + el.clientWidth < el.scrollWidth - 1,
      })
    el.scrollLeft = 0
    update()
    const observer = new ResizeObserver(update)
    observer.observe(el)
    el.addEventListener('scroll', update)
    return () => {
      observer.disconnect()
      el.removeEventListener('scroll', update)
    }
  }, [tab, editing, isAssembly, isDrawing])
  return (
    <header className="command-manager">
      <StandardToolbar />
      <div className="of-ribbon" role="tabpanel" id={`panel-${tab}`} aria-labelledby={`tab-${tab}`}>
        {(overflow.left || overflow.right) && (
          <button
            className="of-scroll"
            aria-label="Scroll commands left"
            disabled={!overflow.left}
            onClick={() => track.current?.scrollBy({ left: -260, behavior: 'smooth' })}
          >
            ‹
          </button>
        )}
        <div ref={track} className="of-ribbon-track">
          <Toolbar
            label={`${availableTabs.find((t) => t.id === tab)?.label ?? 'Assembly'} commands`}
            className="of-ribbon-commands"
            key={tab}
          >
            {groups.map((group) => (
              <div
                className="of-command-group"
                role="group"
                aria-label={group.label}
                key={group.label}
              >
                {group.columns.map((column, index) => (
                  <div className="of-command-column" key={index}>
                    {column.map((control, i) => (
                      <ToolbarControl key={i} control={control} />
                    ))}
                  </div>
                ))}
              </div>
            ))}
          </Toolbar>
        </div>
        {(overflow.left || overflow.right) && (
          <button
            className="of-scroll"
            aria-label="Scroll commands right"
            disabled={!overflow.right}
            onClick={() => track.current?.scrollBy({ left: 260, behavior: 'smooth' })}
          >
            ›
          </button>
        )}
      </div>
      <div
        className="of-tabs"
        role="tablist"
        aria-label="CommandManager"
        onKeyDown={(e) => {
          e.stopPropagation()
          if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return
          e.preventDefault()
          const index = availableTabs.findIndex((t) => t.id === tab)
          const n = availableTabs.length
          const next =
            e.key === 'Home'
              ? 0
              : e.key === 'End'
                ? n - 1
                : (index + (e.key === 'ArrowRight' ? 1 : n - 1)) % n
          const target = availableTabs[next]!.id
          useStore.getState().setTab(target)
          document.getElementById(`tab-${target}`)?.focus()
        }}
      >
        {tabOrder.map((label) => {
          const item = availableTabs.find((t) => t.label === label)
          if (!item)
            return (
              <button
                key={label}
                role="tab"
                aria-selected={false}
                disabled
                title={`${label} — Not implemented yet`}
                data-placeholder={unavailableTabs.includes(
                  label as (typeof unavailableTabs)[number],
                )}
              >
                {label}
              </button>
            )
          return (
            <button
              key={item.id}
              id={`tab-${item.id}`}
              role="tab"
              aria-selected={tab === item.id}
              aria-controls={`panel-${item.id}`}
              tabIndex={tab === item.id ? 0 : -1}
              onClick={() => useStore.getState().setTab(item.id)}
            >
              {label}
            </button>
          )
        })}
      </div>
    </header>
  )
}
