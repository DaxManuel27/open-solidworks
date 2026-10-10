import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from 'react'
import { createPortal } from 'react-dom'
import { type CommandId, executeCommand, useCommands } from './commands'
import { CommandIcon, type IconId } from './icons'

export type Presentation = 'icon' | 'large' | 'compact'
export type Control =
  | { command: CommandId; variant?: Presentation; menu?: CommandId[] }
  | { label: string; icon: IconId; menu: CommandId[]; variant?: Presentation }

const buttons = (el: HTMLElement) =>
  Array.from(el.querySelectorAll<HTMLButtonElement>('button:not(:disabled)'))
function focusButton(button: HTMLButtonElement | undefined) {
  button?.focus({ preventScroll: true })
  button?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
}

/** One Tab stop per toolbar; arrow keys visit its enabled controls. */
export function Toolbar({
  label,
  className = '',
  children,
}: {
  label: string
  className?: string
  children: ReactNode
}) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const items = buttons(ref.current!)
    const active = items.find((b) => b === document.activeElement) ?? items[0]
    items.forEach((b) => {
      b.tabIndex = b === active ? 0 : -1
    })
  })
  const navigate = (e: KeyboardEvent<HTMLDivElement>) => {
    e.stopPropagation()
    const keys = ['ArrowRight', 'ArrowLeft', 'Home', 'End']
    if (!keys.includes(e.key)) return
    e.preventDefault()
    const items = buttons(e.currentTarget)
    const index = items.indexOf(document.activeElement as HTMLButtonElement)
    const next =
      e.key === 'Home'
        ? 0
        : e.key === 'End'
          ? items.length - 1
          : (index + (e.key === 'ArrowRight' ? 1 : -1) + items.length) % items.length
    focusButton(items[next])
  }
  return (
    <div
      ref={ref}
      className={`of-toolbar ${className}`}
      role="toolbar"
      aria-label={label}
      onKeyDown={navigate}
      onFocus={(e) => {
        buttons(e.currentTarget).forEach((b) => {
          b.tabIndex = b === (e.target as HTMLElement) ? 0 : -1
        })
      }}
      onPointerDown={(e) => e.stopPropagation()}
      onPointerUp={(e) => e.stopPropagation()}
      onPointerMove={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
      onWheel={(e) => e.stopPropagation()}
      onContextMenu={(e) => {
        e.preventDefault()
        e.stopPropagation()
      }}
    >
      {children}
    </div>
  )
}

export function CommandButton({
  id,
  variant = 'compact',
}: {
  id: CommandId
  variant?: Presentation
}) {
  const c = useCommands()(id)
  const title = `${c.label}${c.shortcut ? ` (${c.shortcut})` : ''}${c.reason ? ` — ${c.reason}` : ''}`
  return (
    <button
      className={`of-command of-${variant}${c.active ? ' on' : ''}`}
      data-command={id}
      aria-label={c.label}
      aria-pressed={c.active || undefined}
      disabled={!!c.reason}
      title={title}
      onMouseDown={(e) => e.preventDefault()}
      onClick={() => void executeCommand(id)}
    >
      <CommandIcon id={c.icon} />
      {variant !== 'icon' && <span className="command-label">{c.label}</span>}
    </button>
  )
}

export function ToolbarControl({ control }: { control: Control }) {
  const get = useCommands()
  const [open, setOpen] = useState(false)
  const [position, setPosition] = useState({ left: 0, top: 0 })
  const trigger = useRef<HTMLButtonElement>(null)
  const popup = useRef<HTMLDivElement>(null)
  const menuId = useId()
  const variant = control.variant ?? 'compact'
  const primary = 'command' in control ? get(control.command) : null
  const label = primary?.label ?? ('label' in control ? control.label : '')
  const icon = primary?.icon ?? ('icon' in control ? control.icon : 'options')
  const close = (restore = true) => {
    setOpen(false)
    if (restore) trigger.current?.focus({ preventScroll: true })
  }
  useLayoutEffect(() => {
    if (!open) return
    const place = () => {
      const anchor = trigger.current!.getBoundingClientRect()
      const menu = popup.current!.getBoundingClientRect()
      setPosition({
        left: Math.max(4, Math.min(anchor.left, window.innerWidth - menu.width - 4)),
        top:
          anchor.bottom + menu.height + 4 <= window.innerHeight
            ? anchor.bottom + 3
            : Math.max(4, anchor.top - menu.height - 3),
      })
    }
    place()
    const first = buttons(popup.current!)[0] ?? popup.current!
    first.focus()
    const outside = (e: PointerEvent) => {
      if (
        !popup.current?.contains(e.target as Node) &&
        !trigger.current?.contains(e.target as Node)
      )
        setOpen(false)
    }
    const focusOutside = (e: FocusEvent) => {
      if (
        !popup.current?.contains(e.target as Node) &&
        !trigger.current?.contains(e.target as Node)
      )
        setOpen(false)
    }
    document.addEventListener('pointerdown', outside, true)
    document.addEventListener('focusin', focusOutside)
    window.addEventListener('resize', place)
    window.addEventListener('scroll', place, true)
    return () => {
      document.removeEventListener('pointerdown', outside, true)
      document.removeEventListener('focusin', focusOutside)
      window.removeEventListener('resize', place)
      window.removeEventListener('scroll', place, true)
    }
  }, [open])
  if (!control.menu && 'command' in control)
    return <CommandButton id={control.command} variant={variant} />
  const menuKey = (e: KeyboardEvent<HTMLDivElement>) => {
    e.stopPropagation()
    if (e.key === 'Escape') {
      e.preventDefault()
      close()
      return
    }
    if (e.key === 'Tab') {
      // Restore the toolbar stop, then let Tab continue out of the toolbar.
      close()
      return
    }
    const items = buttons(e.currentTarget)
    const index = items.indexOf(document.activeElement as HTMLButtonElement)
    let next: number | undefined
    if (e.key === 'ArrowDown') next = (index + 1) % items.length
    if (e.key === 'ArrowUp') next = (index - 1 + items.length) % items.length
    if (e.key === 'Home') next = 0
    if (e.key === 'End') next = items.length - 1
    if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) {
      const ordered = [...items.slice(index + 1), ...items.slice(0, index + 1)]
      const match = ordered.find((b) =>
        b.textContent?.trim().toLowerCase().startsWith(e.key.toLowerCase()),
      )
      if (match) {
        e.preventDefault()
        match.focus()
      }
    }
    if (next !== undefined) {
      e.preventDefault()
      items[next]?.focus()
    }
  }
  return (
    <div className={`of-control of-${variant}${primary ? ' of-split' : ''}`}>
      {primary && <CommandButton id={primary.id} variant={variant} />}
      <button
        ref={trigger}
        className={`of-command of-menu-trigger of-${variant}`}
        aria-label={primary ? `${label} menu` : label}
        title={primary ? `${label} alternatives` : label}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={() => setOpen(!open)}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault()
            e.stopPropagation()
            setOpen(true)
          }
        }}
      >
        {!primary && (
          <>
            <CommandIcon id={icon} />
            {variant !== 'icon' && <span className="command-label">{label}</span>}
          </>
        )}
        <span className="of-chevron" aria-hidden="true" />
      </button>
      {open &&
        createPortal(
          <div
            ref={popup}
            id={menuId}
            className="of-menu"
            role="menu"
            tabIndex={-1}
            aria-label={label}
            style={position}
            onKeyDown={menuKey}
            onPointerDown={(e) => e.stopPropagation()}
            onClick={(e) => e.stopPropagation()}
          >
            {control.menu?.map((id) => {
              const c = get(id)
              return (
                <button
                  key={id}
                  role="menuitem"
                  aria-label={c.label}
                  data-command={id}
                  disabled={!!c.reason}
                  title={`${c.label}${c.shortcut ? ` (${c.shortcut})` : ''}${c.reason ? ` — ${c.reason}` : ''}`}
                  onClick={() => {
                    close()
                    void executeCommand(id)
                  }}
                >
                  <CommandIcon id={c.icon} />
                  <span>{c.label}</span>
                  <span className="of-menu-hint">{c.shortcut}</span>
                </button>
              )
            })}
          </div>,
          document.body,
        )}
    </div>
  )
}
