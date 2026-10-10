import type { Units } from './protocol'

const MM: Record<Units['length'], number> = { mm: 1, cm: 10, m: 1000, in: 25.4, ft: 304.8 }

export function formatLength(mm: number, u: Units, withUnit = true): string {
  const v = (mm / MM[u.length]).toFixed(u.decimals)
  return withUnit ? `${v} ${u.length}` : v
}

export function formatAngle(rad: number, u: Units, withUnit = true): string {
  if (u.angle === 'rad') return withUnit ? `${rad.toFixed(4)} rad` : rad.toFixed(4)
  const v = ((rad * 180) / Math.PI).toFixed(Math.max(0, u.decimals - 1))
  return withUnit ? `${v}°` : v
}

/** Length in the document unit, as a plain number for an input box. */
export function toDisplayLength(mm: number, u: Units): number {
  return mm / MM[u.length]
}

export function toDisplayAngle(rad: number, u: Units): number {
  return u.angle === 'deg' ? (rad * 180) / Math.PI : rad
}

/** Evaluate + - * / and parentheses. */
export function evaluate(text: string): number | null {
  const s = text.replace(/\s+/g, '').replace(/^=/, '')
  let i = 0
  const expr = (): number | null => {
    let v = term()
    while (v !== null && (s[i] === '+' || s[i] === '-')) {
      const op = s[i++]
      const r = term()
      if (r === null) return null
      v = op === '+' ? v + r : v - r
    }
    return v
  }
  const term = (): number | null => {
    let v = factor()
    while (v !== null && (s[i] === '*' || s[i] === '/')) {
      const op = s[i++]
      const r = factor()
      if (r === null) return null
      v = op === '*' ? v * r : v / r
    }
    return v
  }
  const factor = (): number | null => {
    if (s[i] === '-') {
      i++
      const f = factor()
      return f === null ? null : -f
    }
    if (s[i] === '(') {
      i++
      const v = expr()
      if (s[i] !== ')') return null
      i++
      return v
    }
    const m = /^\d*\.?\d+(e[+-]?\d+)?/i.exec(s.slice(i))
    if (!m) return null
    i += m[0].length
    return parseFloat(m[0])
  }
  const v = expr()
  return v !== null && i === s.length && Number.isFinite(v) ? v : null
}

/** Parse a typed length ("12", "1 in", "3/8in", "=2*5") into mm. */
export function parseLength(text: string, u: Units): number | null {
  const t = text.trim().toLowerCase()
  const m = /^(.*?)(mm|cm|in|"|ft|'|m)?$/.exec(t)
  if (!m) return null
  const unit = m[2]
  const factor =
    unit === undefined
      ? MM[u.length]
      : unit === '"'
        ? 25.4
        : unit === "'"
          ? 304.8
          : MM[unit as Units['length']]
  const v = evaluate(m[1] ?? '')
  return v === null ? null : v * factor
}

/** Parse a typed angle (degrees unless the unit says otherwise) into radians. */
export function parseAngle(text: string, u: Units): number | null {
  const t = text.trim().toLowerCase()
  const m = /^(.*?)(deg|°|rad)?$/.exec(t)
  if (!m) return null
  const v = evaluate(m[1] ?? '')
  if (v === null) return null
  const unit = m[2] ?? (u.angle === 'deg' ? 'deg' : 'rad')
  return unit === 'rad' ? v : (v * Math.PI) / 180
}
