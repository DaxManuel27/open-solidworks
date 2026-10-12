import { useEffect, useState } from 'react'
import type { AsmComponent, MateView } from './protocol'
import { useStore } from './store'

interface Menu {
  x: number
  y: number
  component?: AsmComponent
  mate?: MateView
  feature?: { id: number; name: string; suppressed: boolean; kind: 'pattern' | 'mirror' }
}

const MATE_ICONS: Record<string, string> = {
  Coincident: '⫽',
  Parallel: '∥',
  Perpendicular: '⊥',
  Tangent: '◠',
  Concentric: '◎',
  Lock: '🔒',
  Distance: '↔',
  DistanceLimit: '↔',
  Angle: '∠',
  AngleLimit: '∠',
  Gear: '⚙',
  RackPinion: '⚙',
  Screw: '⟲',
  Hinge: '⌒',
}

function same(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((x, i) => x === b[i])
}

/** Delete selected components with Delete; rename-free tree like FeatureManager. */
export function AssemblyTree() {
  const assembly = useStore((s) => s.assembly)
  const chosen = useStore((s) => s.asmComponents)
  const busy = useStore((s) => s.busy || !!s.panel)
  const run = useStore((s) => s.run)
  const [menu, setMenu] = useState<Menu | null>(null)
  const [open, setOpen] = useState<Record<string, boolean>>({ mates: true })

  useEffect(() => {
    if (!menu) return
    const close = () => setMenu(null)
    window.addEventListener('pointerdown', close)
    return () => window.removeEventListener('pointerdown', close)
  }, [menu])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useStore.getState()
      if (['INPUT', 'TEXTAREA', 'SELECT'].includes((e.target as HTMLElement).tagName) || s.panel)
        return
      if ((e.key === 'Delete' || e.key === 'Backspace') && s.asmComponents.length) {
        const keys = s.asmComponents.filter((k) => k.length === 1 && !k[0]!.includes('@'))
        if (keys.length && confirm(`Delete ${keys.length} component(s) and their mates?`))
          void s.run('asm.component.delete', { keys: keys.map((k) => k[0]) })
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  if (!assembly) return null
  const name =
    assembly.path
      ?.split(/[\\/]/)
      .pop()
      ?.replace(/\.ofa$/i, '') ?? 'Assembly1'
  const active = assembly.configurations.items.find((c) => c.id === assembly.configurations.active)

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

  const component = (c: AsmComponent, depth: number): React.ReactNode => {
    const key = c.key.join('/')
    const selected = chosen.some((k) => same(k, c.key))
    const marker = c.fixed ? '(f) ' : c.under_defined ? '(-) ' : ''
    const kids = c.children ?? []
    const expanded = open[key] ?? false
    const derivedTag = c.derived ? ' ⧉' : ''
    return (
      <div key={key}>
        <div
          className={[
            'tree-item',
            selected ? 'selected' : '',
            c.suppressed || c.kind === 'suppressed' ? 'suppressed' : '',
            c.error || c.kind === 'missing' ? 'error' : '',
            c.hidden ? 'meta' : '',
          ].join(' ')}
          style={{ paddingLeft: 8 + depth * 14 }}
          data-component={key}
          onClick={(e) =>
            useStore.getState().selectComponents([c.key], e.shiftKey || e.metaKey || e.ctrlKey)
          }
          onDoubleClick={() => {
            if (c.kind === 'part' || c.kind === 'assembly')
              void useStore.getState().openComponent(c.key)
          }}
          onContextMenu={(e) => {
            e.preventDefault()
            useStore.getState().selectComponents([c.key])
            setMenu({ x: e.clientX, y: e.clientY, component: c })
          }}
          title={c.error ?? `${c.file}${c.dof ? ` — ${c.dof} degrees of freedom left` : ''}`}
        >
          {kids.length > 0 ? (
            <button
              className="link"
              aria-label={expanded ? 'Collapse' : 'Expand'}
              onClick={(e) => {
                e.stopPropagation()
                setOpen((o) => ({ ...o, [key]: !expanded }))
              }}
            >
              {expanded ? '▾' : '▸'}
            </button>
          ) : (
            <span className="icon">{c.kind === 'assembly' ? '⧈' : '◧'}</span>
          )}
          <span className="name">
            {marker}
            {c.name}
            {derivedTag}
            {c.flexible ? ' ↻' : ''}
          </span>
          {(c.error || c.kind === 'missing') && (
            <span className="badge" title={c.error ?? 'File missing'}>
              !
            </span>
          )}
        </div>
        {expanded && kids.map((k) => component(k, depth + 1))}
      </div>
    )
  }

  const mates = assembly.mates
  const features = [
    ...assembly.patterns.map((p) => ({
      id: p.id,
      name: p.name,
      suppressed: p.suppressed,
      kind: 'pattern' as const,
    })),
    ...assembly.mirrors.map((m) => ({
      id: m.id,
      name: m.name,
      suppressed: m.suppressed,
      kind: 'mirror' as const,
    })),
  ]

  return (
    <div className="tree assembly-tree">
      <div className="tree-header">{name}</div>
      <div className="section-body">
        <label className="field">
          <span>Configuration</span>
          <select
            aria-label="Active assembly configuration"
            disabled={busy}
            value={assembly.configurations.active}
            onChange={(e) => void run('asm.configuration.activate', { id: Number(e.target.value) })}
          >
            {assembly.configurations.items.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        <button
          disabled={busy}
          onClick={() => useStore.getState().openPanel({ type: 'asm_configurations' })}
        >
          Configurations…
        </button>
      </div>
      <div className={`tree-item meta ${assembly.diagnostics.solved ? '' : 'error'}`}>
        <span className="icon">{assembly.diagnostics.solved ? '✓' : '!'}</span>
        <span className="name">
          {assembly.diagnostics.solved
            ? assembly.diagnostics.free
              ? `${assembly.diagnostics.free} free motion(s)`
              : 'Fully defined'
            : 'Mate errors'}
        </span>
      </div>
      {['Front Plane', 'Top Plane', 'Right Plane', 'Origin'].map((p) => (
        <div key={p} className="tree-item">
          <span className="icon">{p === 'Origin' ? '⊹' : '▱'}</span>
          <span className="name">{p}</span>
        </div>
      ))}
      {assembly.reference.map((r) => (
        <div key={`r${r.id}`} className="tree-item">
          <span className="icon">▱</span>
          <span className="name">{r.name}</span>
        </div>
      ))}
      {assembly.components.length === 0 && (
        <div className="hint">Insert components to begin (Assembly ▸ Insert Components).</div>
      )}
      {assembly.components.map((c) => component(c, 0))}
      <div
        className="tree-item meta"
        onClick={() => setOpen((o) => ({ ...o, mates: !(o['mates'] ?? true) }))}
      >
        <span className="icon">{open['mates'] === false ? '▸' : '▾'}</span>
        <span className="name">Mates ({mates.length})</span>
      </div>
      {open['mates'] !== false &&
        mates.map((m) => (
          <div
            key={`m${m.id}`}
            className={['tree-item', m.suppressed ? 'suppressed' : '', m.error ? 'error' : ''].join(
              ' ',
            )}
            style={{ paddingLeft: 22 }}
            data-mate={m.id}
            title={
              m.error ?? (m.redundant ? 'Redundant: holds without adding anything' : undefined)
            }
            onClick={() => useStore.getState().selectComponents(m.components.map((k) => [k]))}
            onDoubleClick={() =>
              useStore
                .getState()
                .openPanel({ type: 'mate', editing: m.id, variant: m.definition.type })
            }
            onContextMenu={(e) => {
              e.preventDefault()
              setMenu({ x: e.clientX, y: e.clientY, mate: m })
            }}
          >
            <span className="icon">{MATE_ICONS[m.type] ?? '⫽'}</span>
            <span className="name">
              {m.conflicting ? '(+) ' : ''}
              {m.name}
            </span>
            {m.error && (
              <span className="badge" title={m.error}>
                !
              </span>
            )}
          </div>
        ))}
      {features.map((f) => (
        <div
          key={`f${f.id}`}
          className={`tree-item ${f.suppressed ? 'suppressed' : ''}`}
          onDoubleClick={() =>
            useStore.getState().openPanel({
              type: f.kind === 'pattern' ? 'component_pattern' : 'mirror_components',
              editing: f.id,
            })
          }
          onContextMenu={(e) => {
            e.preventDefault()
            setMenu({ x: e.clientX, y: e.clientY, feature: f })
          }}
        >
          <span className="icon">{f.kind === 'pattern' ? '⋯' : '⧓'}</span>
          <span className="name">{f.name}</span>
        </div>
      ))}
      {menu?.component && (
        <div
          className="context-menu"
          style={{ left: menu.x, top: menu.y }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          {menu.component.key.length === 1 && !menu.component.key[0]!.includes('@') ? (
            <>
              {item('Component Properties…', () =>
                useStore
                  .getState()
                  .openPanel({ type: 'component_properties', component: menu.component!.key }),
              )}
              {item(
                menu.component.fixed ? 'Float' : 'Fix',
                () =>
                  void run('asm.component.update', {
                    key: menu.component!.key[0],
                    fixed: !menu.component!.fixed,
                  }),
              )}
              {item(
                menu.component.hidden ? 'Show' : 'Hide',
                () =>
                  void run('asm.component.update', {
                    key: menu.component!.key[0],
                    hidden: !menu.component!.hidden,
                  }),
              )}
              {item(
                menu.component.suppressed ? 'Unsuppress' : 'Suppress',
                () =>
                  void run('asm.component.update', {
                    key: menu.component!.key[0],
                    suppressed: !menu.component!.suppressed,
                  }),
              )}
              {menu.component.kind === 'assembly' &&
                item(
                  menu.component.flexible ? 'Make Rigid' : 'Make Flexible',
                  () =>
                    void run('asm.component.update', {
                      key: menu.component!.key[0],
                      flexible: !menu.component!.flexible,
                    }),
                )}
              {item('Replace…', async () => {
                const path = await window.openforge.openDialog('component')
                if (path) {
                  const r = (await run('asm.component.replace', {
                    key: menu.component!.key[0],
                    path,
                  })) as unknown as {
                    broken_mates?: number[]
                  } | null
                  if (r?.broken_mates?.length)
                    useStore
                      .getState()
                      .toast(`${r.broken_mates.length} mate(s) need new references`)
                }
              })}
              {item('Open', () => void useStore.getState().openComponent(menu.component!.key), {
                disabled: menu.component.kind === 'missing' || menu.component.kind === 'suppressed',
              })}
              <div className="sep" />
              {item(
                'Delete',
                () => {
                  if (confirm(`Delete ${menu.component!.name} and its mates?`))
                    void run('asm.component.delete', { keys: [menu.component!.key[0]] })
                },
                { danger: true },
              )}
            </>
          ) : (
            item('Open', () => void useStore.getState().openComponent(menu.component!.key))
          )}
        </div>
      )}
      {menu?.mate && (
        <div
          className="context-menu"
          style={{ left: menu.x, top: menu.y }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          {item('Edit Mate', () =>
            useStore.getState().openPanel({
              type: 'mate',
              editing: menu.mate!.id,
              variant: menu.mate!.definition.type,
            }),
          )}
          {item('Rename', () => {
            const name = prompt('Mate name', menu.mate!.name)
            if (name?.trim()) void run('asm.mate.update', { id: menu.mate!.id, name })
          })}
          {item(
            menu.mate.suppressed ? 'Unsuppress' : 'Suppress',
            () =>
              void run('asm.mate.update', {
                id: menu.mate!.id,
                suppressed: !menu.mate!.suppressed,
              }),
          )}
          <div className="sep" />
          {item('Delete', () => void run('asm.mate.delete', { ids: [menu.mate!.id] }), {
            danger: true,
          })}
        </div>
      )}
      {menu?.feature && (
        <div
          className="context-menu"
          style={{ left: menu.x, top: menu.y }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          {item('Edit Feature', () =>
            useStore.getState().openPanel({
              type: menu.feature!.kind === 'pattern' ? 'component_pattern' : 'mirror_components',
              editing: menu.feature!.id,
            }),
          )}
          {item(
            menu.feature.suppressed ? 'Unsuppress' : 'Suppress',
            () =>
              void run('asm.feature.suppress', {
                id: menu.feature!.id,
                suppressed: !menu.feature!.suppressed,
              }),
          )}
          <div className="sep" />
          {item('Delete', () => void run('asm.feature.delete', { id: menu.feature!.id }), {
            danger: true,
          })}
        </div>
      )}
      {active && (active.explode?.length ?? 0) > 0 && (
        <div
          className="tree-item meta"
          onClick={() => useStore.getState().openPanel({ type: 'explode' })}
        >
          <span className="icon">✦</span>
          <span className="name">Exploded View ({active.explode?.length} steps)</span>
        </div>
      )}
    </div>
  )
}
