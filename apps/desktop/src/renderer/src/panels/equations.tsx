import { useState } from 'react'
import { useStore } from '../store'
import { Choice, FormulaField, PanelShell, ParameterField, Section } from './fields'
import { EditScope, useEquationDraft, without } from './parameter-editor'

export function EquationsPanel() {
  const close = useStore((s) => s.closePanel)
  const editor = useEquationDraft()
  const { snapshot, draft, setDraft, values } = editor
  const [name, setName] = useState('')
  const [parameter, setParameter] = useState(snapshot.parameters[0]?.key ?? '')
  const keys = [...new Set([...Object.keys(draft.equations), ...Object.keys(draft.parameters)])]
  return (
    <PanelShell
      title="Equations and Variables"
      error={editor.error}
      okDisabled={editor.pending || !!editor.error}
      onCancel={() => void close()}
      onOk={() => {
        void editor.save().then((saved) => {
          if (saved) void close()
        })
      }}
    >
      <EditScope editor={editor} />
      <p>
        Use explicit units, for example <code>width / 2 + 1mm</code> or <code>45deg</code>. Bare
        numbers in formulas are dimensionless.
      </p>
      <Section title="Global variables">
        {Object.entries(draft.variables).map(([key, expression]) => (
          <div key={key}>
            <FormulaField
              label={key}
              ariaLabel={`Expression for ${key}`}
              expression={expression}
              result={values[key]}
              onChange={(value) =>
                setDraft({ ...draft, variables: { ...draft.variables, [key]: value } })
              }
            />
            <button
              className="link"
              onClick={() => setDraft({ ...draft, variables: without(draft.variables, key) })}
            >
              {draft.scope === 'base' ? 'Remove' : 'Reset to base'}
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
          disabled={!name.trim() || name.trim() in draft.variables}
          onClick={() => {
            setDraft({
              ...draft,
              variables: {
                ...draft.variables,
                [name.trim()]: snapshot.variables[name.trim()] ?? '10mm',
              },
            })
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
          disabled={!parameter || keys.includes(parameter)}
          onClick={() => {
            const p = snapshot.parameters.find((p) => p.key === parameter)!
            editor.change(
              parameter,
              `${values[parameter]?.value?.value ?? p.value.value}${p.value.angle ? 'rad' : p.value.length ? 'mm' : ''}`,
            )
          }}
        >
          Drive with equation
        </button>
        <button
          disabled={!parameter || keys.includes(parameter)}
          onClick={() => {
            const p = snapshot.parameters.find((p) => p.key === parameter)!
            editor.change(parameter, values[parameter]?.value?.value ?? p.value.value)
          }}
        >
          Edit numeric value
        </button>
        {keys.map((key) => {
          const p = snapshot.parameters.find((p) => p.key === key)
          return (
            <div key={key}>
              {p ? (
                <ParameterField
                  parameter={p}
                  value={draft.parameters[key] ?? values[key]?.value?.value ?? p.value.value}
                  expression={draft.equations[key]}
                  result={values[key]}
                  onChange={(value) => editor.change(key, value)}
                />
              ) : (
                <p className="panel-error">Parameter {key} no longer exists.</p>
              )}
              <button className="link" onClick={() => editor.reset(key)}>
                {draft.scope === 'base' ? 'Remove' : 'Reset to base'}
              </button>
            </div>
          )
        })}
      </Section>
    </PanelShell>
  )
}
