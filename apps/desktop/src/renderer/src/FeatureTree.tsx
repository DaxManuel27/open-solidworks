import { useEffect, useState } from 'react'
import type { FeatureView, Ref } from './protocol'
import { useCutList } from './panels/weldment/cutlist'
import { type PanelType, useStore } from './store'
import { FORM_FEATURES } from './toolbars/phase6'

const ICONS: Record<string, string> = {
  sketch: '✎',
  sketch3d: '✎',
  cosmetic_thread: '〰',
  plane: '▱',
  axis: '┆',
  point: '•',
  coord_sys: '⌖',
  extrude: '▣',
  revolve: '◐',
  fillet: '◜',
  chamfer: '◸',
  shell: '▢',
  hole: '◎',
  linear_pattern: '⋯',
  circular_pattern: '✲',
  mirror: '⧓',
  import: '⬇',
  base_flange: '▭',
  edge_flange: '⌐',
  miter_flange: '⌐',
  hem: '⊃',
  flat_pattern: '▬',
  weldment: '⌗',
  structural_member: '▥',
  trim_extend: '✂',
  end_cap: '▣',
  weld_gusset: '◺',
  weld_bead: '〰',
}

const STD: { ref: Ref; name: string }[] = [
  { ref: { type: 'front_plane' }, name: 'Front Plane' },
  { ref: { type: 'top_plane' }, name: 'Top Plane' },
  { ref: { type: 'right_plane' }, name: 'Right Plane' },
  { ref: { type: 'origin' }, name: 'Origin' },
]

/** The panel that edits a feature, if it has one. */
export function panelFor(f: FeatureView): PanelType | null {
  switch (f.type) {
    case 'extrude':
      return f.params['cut'] ? 'cut' : 'extrude'
    case 'revolve':
      return f.params['cut'] ? 'cut_revolve' : 'revolve'
    case 'sweep':
      return f.params['cut'] ? 'sweep_cut' : 'sweep'
    case 'loft':
      return f.params['cut'] ? 'loft_cut' : 'loft'
    case 'boundary':
      return f.params['cut'] ? 'boundary_cut' : 'boundary'
    case 'structural_member':
    case 'trim_extend':
    case 'end_cap':
    case 'weld_gusset':
    case 'weld_bead':
    case 'base_flange':
    case 'edge_flange':
    case 'miter_flange':
    case 'hem':
    case 'jog':
    case 'sketched_bend':
    case 'swept_flange':
    case 'lofted_bend':
    case 'cross_break':
    case 'insert_bends':
    case 'convert_to_sheet_metal':
    case 'rip':
    case 'unfold':
    case 'fold':
    case 'normal_cut':
    case 'corner_relief':
    case 'closed_corner':
    case 'welded_corner':
    case 'break_corner':
    case 'bend_notch':
    case 'tab_and_slot':
    case 'sheet_metal_gusset':
    case 'form_tool':
    case 'forming_tool':
    case 'flat_pattern':
    case 'cosmetic_thread':
    case 'sketch3d':
    case 'draft':
    case 'dome':
    case 'rib':
    case 'thread':
    case 'face_fillet':
    case 'wrap':
    case 'variable_fillet':
    case 'helix':
    case 'composite_curve':
    case 'split_line':
    case 'projected_curve':
    case 'combine':
    case 'move_body':
    case 'split_body':
    case 'fillet':
    case 'chamfer':
    case 'shell':
    case 'hole':
    case 'linear_pattern':
    case 'circular_pattern':
    case 'mirror':
    case 'plane':
    case 'axis':
    case 'point':
    case 'coord_sys':
      return f.type
    default:
      return null
  }
}

/** Open whatever edits this feature: the sketcher or its panel. */
export function editFeature(f: FeatureView): void {
  const s = useStore.getState()
  if (f.type === 'sketch') {
    void s.editSketch(f.id)
    return
  }
  if (FORM_FEATURES[f.type]) {
    s.openPanel({ type: 'feature_form', variant: f.type, editing: f.id })
    return
  }
  const p = panelFor(f)
  if (p) s.openPanel({ type: p, editing: f.id })
}

async function deleteFeature(id: number): Promise<void> {
  const s = useStore.getState()
  const f = s.snapshot?.features.find((x) => x.id === id)
  if (!f) return
  if (!confirm(`Delete ${f.name}? Features that use it will show errors until they're fixed.`))
    return
  if (await s.run('feature.delete', { ids: [id] })) s.selectFeature(null)
}

interface Menu {
  x: number
  y: number
  feature: FeatureView
  index: number
}

export function FeatureTree() {
  const snapshot = useStore((s) => s.snapshot)
  const bodies = useStore((s) => s.scene?.bodies) ?? []
  const hoverFeature = useStore((s) => s.hoverFeature)
  const mold = useStore((s) => s.snapshot?.mold)
  const cutList = useCutList()
  const editing = useStore((s) => !!s.panel || !!s.sketch || s.busy)
  const treeSelection = useStore((s) => s.treeSelection)
  const selection = useStore((s) => s.selection)
  const run = useStore((s) => s.run)
  const [menu, setMenu] = useState<Menu | null>(null)
  const [renaming, setRenaming] = useState<number | null>(null)
  const [drop, setDrop] = useState<number | null>(null)

  useEffect(() => {
    if (!menu) return
    const close = () => setMenu(null)
    window.addEventListener('pointerdown', close)
    return () => window.removeEventListener('pointerdown', close)
  }, [menu])

  // Rename the selected feature with F2, delete it with Delete.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useStore.getState()
      if (
        ['INPUT', 'TEXTAREA', 'SELECT'].includes((e.target as HTMLElement).tagName) ||
        (e.target as HTMLElement).isContentEditable ||
        s.sketch ||
        s.panel ||
        s.treeSelection === null
      )
        return
      if (e.key === 'F2') setRenaming(s.treeSelection)
      if (e.key === 'Delete' || e.key === 'Backspace') void deleteFeature(s.treeSelection)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  if (!snapshot) return null
  const features = snapshot.features
  const rollback = snapshot.rollback ?? features.length

  const onDrop = (e: React.DragEvent, index: number) => {
    e.preventDefault()
    setDrop(null)
    const data = e.dataTransfer.getData('text/plain')
    if (data === 'rollback') {
      void run('doc.rollback', { index: index >= features.length ? null : index })
      return
    }
    const id = Number(data)
    const from = features.findIndex((f) => f.id === id)
    if (from < 0) return
    const to = index > from ? index - 1 : index
    if (to !== from) void run('feature.reorder', { id, to })
  }

  const dropZone = (index: number) => ({
    onDragOver: (e: React.DragEvent) => {
      e.preventDefault()
      setDrop(index)
    },
    onDragLeave: () => setDrop((d) => (d === index ? null : d)),
    onDrop: (e: React.DragEvent) => onDrop(e, index),
  })

  const selectStd = (ref: Ref) => {
    const s = useStore.getState()
    s.selectFeature(null)
    const pick = ref.type === 'origin' ? null : ({ kind: 'std_plane', ref } as const)
    if (pick) useStore.setState({ selection: [{ pick, ref }] })
  }

  const item = (
    label: string,
    onClick: () => void,
    opts: { disabled?: boolean; danger?: boolean } = {},
  ) => (
    <button
      key={label}
      className={opts.danger ? 'danger' : ''}
      disabled={opts.disabled}
      onClick={() => {
        setMenu(null)
        onClick()
      }}
    >
      {label}
    </button>
  )

  // Dependency display: the hovered feature's parents and children.
  const hovered = features.find((f) => f.id === hoverFeature)
  const parents = new Set(hovered?.depends_on ?? [])
  const children = new Set(
    hovered ? features.filter((f) => f.depends_on?.includes(hovered.id)).map((f) => f.id) : [],
  )
  const rows: React.ReactNode[] = []
  features.forEach((f, i) => {
    if (i === rollback) rows.push(<RollbackBar key="rb" />)
    const selected = treeSelection === f.id
    rows.push(
      <div
        key={f.id}
        className={[
          'tree-item',
          selected ? 'selected' : '',
          f.suppressed ? 'suppressed' : '',
          f.rolled_back ? 'rolled-back' : '',
          f.error ? 'error' : '',
          drop === i ? 'drop' : '',
          parents.has(f.id) ? 'parent' : '',
          children.has(f.id) ? 'child' : '',
        ].join(' ')}
        draggable={renaming !== f.id}
        onDragStart={(e) => e.dataTransfer.setData('text/plain', String(f.id))}
        {...dropZone(i)}
        onClick={() => useStore.getState().selectFeature(f.id)}
        onDoubleClick={() => editFeature(f)}
        onContextMenu={(e) => {
          e.preventDefault()
          useStore.getState().selectFeature(f.id)
          setMenu({ x: e.clientX, y: e.clientY, feature: f, index: i })
        }}
        onMouseEnter={() => useStore.setState({ hoverFeature: f.id })}
        onMouseLeave={() => useStore.setState({ hoverFeature: null })}
        title={f.error ?? (f.type === 'sketch' ? (f.sketch_status ?? undefined) : undefined)}
      >
        <span className="icon">{ICONS[f.type] ?? '◆'}</span>
        {renaming === f.id ? (
          <RenameBox
            name={f.name}
            onDone={(name) => {
              setRenaming(null)
              if (name && name !== f.name) void run('feature.rename', { id: f.id, name })
            }}
          />
        ) : (
          <span className="name">
            {f.type === 'sketch' && f.sketch_status === 'Under defined' ? '(-) ' : ''}
            {f.type === 'sketch' && f.sketch_status === 'Over defined' ? '(+) ' : ''}
            {f.name}
          </span>
        )}
        {f.error && (
          <span className="badge" title={f.error}>
            !
          </span>
        )}
      </div>,
    )
  })
  if (rollback >= features.length) rows.push(<RollbackBar key="rb" />)

  return (
    <div className="tree">
      <div className="tree-header">
        {snapshot.path
          ?.split(/[\\/]/)
          .pop()
          ?.replace(/\.ofp$/i, '') ?? 'Part1'}
      </div>
      <div
        className="tree-item meta"
        onClick={() => useStore.getState().openPanel({ type: 'material' })}
        title="Active configuration material; click to edit shared base material"
      >
        <span className="icon">◈</span>
        <span className="name">
          {snapshot.effective_material?.name ?? 'Material <not specified>'}
        </span>
      </div>
      <div className="section-body">
        <label className="field">
          <span>Configuration</span>
          <select
            aria-label="Active configuration"
            disabled={editing}
            value={snapshot.configurations.active}
            onChange={(e) => void run('configuration.activate', { id: Number(e.target.value) })}
          >
            {snapshot.configurations.items.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        <button
          aria-label="New configuration"
          disabled={editing}
          onClick={() => {
            const name = prompt('New configuration name')
            if (name?.trim()) void run('configuration.create', { name })
          }}
        >
          New
        </button>
        <button
          aria-label="Duplicate configuration"
          disabled={editing}
          onClick={() => {
            const name = prompt('Copy configuration as')
            if (name?.trim())
              void run('configuration.create', { name, copy_from: snapshot.configurations.active })
          }}
        >
          Duplicate
        </button>
        <button
          disabled={editing || snapshot.configurations.items.length < 2}
          onClick={() => {
            if (confirm('Delete this configuration?'))
              void run('configuration.delete', { id: snapshot.configurations.active })
          }}
        >
          Delete
        </button>
      </div>
      {STD.map((p) => {
        const on = selection.some((s) => JSON.stringify(s.ref) === JSON.stringify(p.ref))
        return (
          <div
            key={p.name}
            className={`tree-item ${on ? 'selected' : ''}`}
            onClick={() => selectStd(p.ref)}
            onDoubleClick={() =>
              p.ref.type !== 'origin' && void useStore.getState().startSketch(p.ref)
            }
            onMouseEnter={() => useStore.setState({ hoverFeature: -1 })}
            onMouseLeave={() => useStore.setState({ hoverFeature: null })}
            title={p.ref.type === 'origin' ? undefined : 'Double-click to sketch on this plane'}
          >
            <span className="icon">{p.ref.type === 'origin' ? '⊹' : '▱'}</span>
            <span className="name">{p.name}</span>
          </div>
        )
      })}
      {!!snapshot.sheet_metal?.length && (
        <>
          <div className="tree-item meta">Sheet Metal ({snapshot.sheet_metal.length})</div>
          {snapshot.sheet_metal.map((sm) => {
            const owner = snapshot.features.find((f) => f.id === sm.feature)
            const g = sm.parameters.gauge
            return (
              <div
                className="tree-item"
                key={sm.feature}
                title="Double-click to edit this body's sheet metal parameters"
                onDoubleClick={() => owner && editFeature(owner)}
              >
                <span className="icon">▭</span>
                <span className="name">
                  {sm.name ?? 'Sheet metal'} ·{' '}
                  {g ? `gauge ${g.gauge}` : `${sm.parameters.thickness} mm`} · {sm.bends} bend
                  {sm.bends === 1 ? '' : 's'}
                  {sm.flattened ? ' · flat' : ''}
                </span>
              </div>
            )
          })}
        </>
      )}
      {cutList && (
        <>
          <div
            className="tree-item meta"
            title="Double-click to edit the cut list"
            onDoubleClick={() => useStore.getState().openPanel({ type: 'cut_list' })}
          >
            Cut List ({cutList.items.filter((i) => !i.excluded).length})
          </div>
          {cutList.items.map((item) => (
            <div
              className={`tree-item ${item.excluded ? 'suppressed' : ''}`}
              key={item.bodies[0]}
              onDoubleClick={() => useStore.getState().openPanel({ type: 'cut_list' })}
            >
              <span className="icon">▤</span>
              <span className="name">
                {item.folder ? `${item.folder} / ` : ''}
                {item.number ? `${item.number}. ` : ''}
                {cutList.settings.name ? (
                  item.name
                ) : (
                  <>
                    {item.properties['DESCRIPTION']}
                    {item.properties['LENGTH'] ? ` · ${item.properties['LENGTH']} mm` : ''}
                  </>
                )}{' '}
                × {item.quantity}
              </span>
            </div>
          ))}
          {cutList.welds.beads.length > 0 && (
            <div className="tree-item meta">
              Weld Folder ({cutList.welds.beads.length} beads,{' '}
              {cutList.welds.total_length.toFixed(0)} mm)
            </div>
          )}
        </>
      )}
      {(
        [
          ['solid', 'Solid Bodies'],
          ['surface', 'Surface Bodies'],
          ['mesh', 'Mesh Bodies'],
        ] as const
      ).map(([kind, title]) => {
        const list = bodies.filter((b) => (b.kind ?? 'solid') === kind)
        if (!list.length && kind !== 'solid') return null
        return (
          <div key={kind}>
            <div className="tree-item meta">
              {title} ({list.length})
            </div>
            {list.map((body) => (
              <div className="tree-item" key={body.id}>
                <input
                  type="checkbox"
                  aria-label={`Show ${body.name}`}
                  checked={!body.hidden}
                  onChange={() => void run('body.display', { id: body.id, hidden: !body.hidden })}
                />
                <button
                  className="link"
                  onClick={() => {
                    const index = body.faces[0]
                    if (index !== undefined)
                      void useStore.getState().pick({ kind: 'face', index }, false)
                  }}
                  onDoubleClick={() => {
                    const name = prompt('Body name', body.name)
                    if (name?.trim()) void run('body.display', { id: body.id, name })
                  }}
                  title={
                    body.kind === 'mesh'
                      ? `${body.triangles ?? 0} triangles; double-click to rename`
                      : 'Select; double-click to rename'
                  }
                >
                  {body.name}
                </button>
              </div>
            ))}
          </div>
        )
      })}
      {mold && (mold.parting_lines.length > 0 || mold.cores.length > 0) && (
        <>
          <div className="tree-item meta">Mould</div>
          {mold.parting_lines.map((p) => (
            <div className="tree-item" key={`pl${p.feature}`}>
              Parting line: {p.edges} edges, {p.openings} openings
            </div>
          ))}
          {mold.shut_off.length > 0 && (
            <div className="tree-item">Shut-off surfaces ({mold.shut_off.length})</div>
          )}
          {mold.parting_surfaces.length > 0 && (
            <div className="tree-item">Parting surfaces ({mold.parting_surfaces.length})</div>
          )}
          {mold.cavities.length > 0 && (
            <div className="tree-item">Cavity bodies ({mold.cavities.length})</div>
          )}
          {mold.cores.length > 0 && (
            <div className="tree-item">Core bodies ({mold.cores.length})</div>
          )}
        </>
      )}
      {rows}
      <div className="tree-tail" {...dropZone(features.length)} />
      {menu && (
        <div
          className="context-menu"
          style={{ left: menu.x, top: menu.y }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          {item('Edit Feature', () => editFeature(menu.feature), {
            disabled: !panelFor(menu.feature) && menu.feature.type !== 'sketch',
          })}
          {typeof menu.feature.params['sketch'] === 'number' &&
            item(
              'Edit Sketch',
              () => void useStore.getState().editSketch(menu.feature.params['sketch'] as number),
            )}
          {item('Rename', () => setRenaming(menu.feature.id))}
          {item(
            menu.feature.suppressed ? 'Unsuppress' : 'Suppress',
            () =>
              void run('feature.suppress', {
                id: menu.feature.id,
                suppressed: !menu.feature.suppressed,
              }),
          )}
          {item('Roll Back to Here', () => void run('doc.rollback', { index: menu.index }))}
          {snapshot.rollback !== null &&
            item('Roll to End', () => void run('doc.rollback', { index: null }))}
          <div className="sep" />
          {item('Delete', () => void deleteFeature(menu.feature.id), { danger: true })}
        </div>
      )}
    </div>
  )
}

function RollbackBar() {
  return (
    <div
      className="rollback-bar"
      draggable
      onDragStart={(e) => e.dataTransfer.setData('text/plain', 'rollback')}
      title="Drag to roll back the model"
    />
  )
}

function RenameBox(props: { name: string; onDone: (name: string | null) => void }) {
  const [text, setText] = useState(props.name)
  return (
    <input
      className="rename"
      autoFocus
      value={text}
      onFocus={(e) => e.currentTarget.select()}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => props.onDone(text.trim())}
      onKeyDown={(e) => {
        e.stopPropagation()
        if (e.key === 'Enter') props.onDone(text.trim())
        if (e.key === 'Escape') props.onDone(null)
      }}
    />
  )
}
