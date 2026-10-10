import { useEffect, useState } from 'react'
import { call } from '../core'
import type { EquationValue } from '../protocol'
import { useStore } from '../store'
import { Choice, PanelShell, Section } from './fields'

export function EquationsPanel() {
  const snapshot = useStore((s) => s.snapshot)!
  const run = useStore((s) => s.run)
  const close = useStore((s) => s.closePanel)
  const [variables, setVariables] = useState(snapshot.variables)
  const [equations, setEquations] = useState(snapshot.equations)
  const [name, setName] = useState('')
  const [parameter, setParameter] = useState(snapshot.parameters[0]?.key ?? '')
  const [values, setValues] = useState<Record<string, EquationValue>>(snapshot.equation_values)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let current = true
    const timer = setTimeout(() => {
      void call<{ values: Record<string, EquationValue> }>('equations.preview', {
        variables,
        equations,
      })
        .then((result) => {
          if (current) {
            setValues(result.values)
            setError(null)
          }
        })
        .catch((e: Error) => {
          if (current) setError(e.message)
        })
    }, 200)
    return () => {
      current = false
      clearTimeout(timer)
    }
  }, [variables, equations])
  const result = (key: string) => {
    const v = values[key]
    if (v?.error) return <span className="panel-error">{v.error}</span>
    if (!v?.value) return null
    const q = v.value
    return (
      <small>
        {Number(q.value.toPrecision(8))}
        {q.length !== 0 ? ` mm${q.length !== 1 ? `^${q.length}` : ''}` : ''}
        {q.angle !== 0 ? ` rad${q.angle !== 1 ? `^${q.angle}` : ''}` : ''}
      </small>
    )
  }
  const remove = (map: Record<string, string>, key: string) =>
    Object.fromEntries(Object.entries(map).filter(([k]) => k !== key))
  return (
    <PanelShell
      title="Equations and Variables"
      error={error}
      okDisabled={!!error}
      onCancel={() => void close()}
      onOk={() => {
        void run('equations.set', { variables, equations }).then((saved) => {
          if (saved) void close()
        })
      }}
    >
      <p>
        Use explicit units, for example <code>width / 2 + 1mm</code> or <code>45deg</code>. Bare
        numbers are dimensionless.
      </p>
      <Section title="Global variables">
        {Object.entries(variables).map(([key, expression]) => (
          <div key={key}>
            <label className="field">
              <span>{key}</span>
              <input
                aria-label={`Expression for ${key}`}
                value={expression}
                onChange={(e) => setVariables({ ...variables, [key]: e.target.value })}
              />
            </label>
            {result(key)}{' '}
            <button className="link" onClick={() => setVariables(remove(variables, key))}>
              Remove
            </button>
          </div>
        ))}
        <label className="field">
          <span>New variable</span>
          <input
            aria-label="New variable name"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <button
          disabled={!name.trim() || name.trim() in variables}
          onClick={() => {
            setVariables({ ...variables, [name.trim()]: '10mm' })
            setName('')
          }}
        >
          Add variable
        </button>
      </Section>
      <Section title="Driven parameters">
        <Choice
          label="Parameter"
          value={parameter}
          options={snapshot.parameters.map((p) => ({
            value: p.key,
            label: `${snapshot.features.find((f) => f.id === p.feature)?.name ?? p.feature} · ${p.path} (${p.key})`,
          }))}
          onChange={setParameter}
        />
        <button
          disabled={!parameter || parameter in equations}
          onClick={() => {
            const p = snapshot.parameters.find((p) => p.key === parameter)!
            setEquations({
              ...equations,
              [parameter]: `${p.value.value}${p.value.angle ? 'rad' : p.value.length ? 'mm' : ''}`,
            })
          }}
        >
          Drive with equation
        </button>
        {Object.entries(equations).map(([key, expression]) => (
          <div key={key}>
            <label className="field">
              <span>{key}</span>
              <input
                aria-label={`Equation for ${key}`}
                value={expression}
                onChange={(e) => setEquations({ ...equations, [key]: e.target.value })}
              />
            </label>
            {result(key)}{' '}
            <button className="link" onClick={() => setEquations(remove(equations, key))}>
              Remove
            </button>
          </div>
        ))}
      </Section>
    </PanelShell>
  )
}
