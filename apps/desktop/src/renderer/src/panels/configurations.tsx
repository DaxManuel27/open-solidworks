import { useState } from 'react'
import { useStore } from '../store'
import { AngleField, Check, Choice, LengthField, NumberField, PanelShell, Section } from './fields'

export function ConfigurationsPanel() {
  const snapshot = useStore((s) => s.snapshot)!
  const run = useStore((s) => s.run)
  const close = useStore((s) => s.closePanel)
  const [config, setConfig] = useState(() =>
    structuredClone(
      snapshot.configurations.items.find((c) => c.id === snapshot.configurations.active)!,
    ),
  )
  const [parameter, setParameter] = useState(snapshot.parameters[0]?.key ?? '')
  const [variable, setVariable] = useState('')
  const [error, setError] = useState<string | null>(null)
  const without = <T,>(map: Record<string, T>, key: string) =>
    Object.fromEntries(Object.entries(map).filter(([k]) => k !== key))
  const keys = [...new Set([...Object.keys(config.parameters), ...Object.keys(config.equations)])]
  return (
    <PanelShell
      title="Configuration Overrides"
      error={error}
      onCancel={() => void close()}
      onOk={() =>
        void run('configuration.update', { configuration: config }).then((saved) => {
          if (saved) void close()
          else setError('Could not save the configuration. Check its name and values.')
        })
      }
    >
      <p>
        These overrides affect only this variant. Feature panels and the Equations editor change
        shared base values.
      </p>
      <label className="field">
        <span>Name</span>
        <input
          value={config.name}
          onChange={(e) => setConfig({ ...config, name: e.target.value })}
        />
      </label>
      <Section title="Dimensions">
        <Choice
          label="Parameter"
          value={parameter}
          options={snapshot.parameters.map((p) => ({ value: p.key, label: p.key }))}
          onChange={setParameter}
        />
        <button
          disabled={!parameter || keys.includes(parameter)}
          onClick={() => {
            const p = snapshot.parameters.find((p) => p.key === parameter)!
            setConfig({
              ...config,
              parameters: { ...config.parameters, [parameter]: p.value.value },
            })
          }}
        >
          Override parameter
        </button>
        {keys.map((key) => {
          const p = snapshot.parameters.find((p) => p.key === key)
          const Field = p?.integer
            ? NumberField
            : p?.value.angle
              ? AngleField
              : p?.value.length
                ? LengthField
                : NumberField
          const expression = config.equations[key]
          return (
            <div key={key}>
              <Check
                label={`${key} — formula`}
                value={typeof expression === 'string'}
                onChange={(on) =>
                  setConfig({
                    ...config,
                    equations: on
                      ? {
                          ...config.equations,
                          [key]: `${config.parameters[key] ?? p?.value.value ?? 0}${p?.value.angle ? 'rad' : p?.value.length ? 'mm' : ''}`,
                        }
                      : without(config.equations, key),
                    parameters: on
                      ? without(config.parameters, key)
                      : { ...config.parameters, [key]: p?.value.value ?? 0 },
                  })
                }
              />
              {typeof expression === 'string' ? (
                <input
                  aria-label={`Override equation ${key}`}
                  value={expression}
                  onChange={(e) =>
                    setConfig({
                      ...config,
                      equations: { ...config.equations, [key]: e.target.value },
                    })
                  }
                />
              ) : (
                <Field
                  label={key}
                  value={config.parameters[key] ?? p?.value.value ?? 0}
                  min={-Number.MAX_VALUE}
                  onChange={(value) =>
                    setConfig({ ...config, parameters: { ...config.parameters, [key]: value } })
                  }
                />
              )}
              <button
                className="link"
                onClick={() =>
                  setConfig({
                    ...config,
                    parameters: without(config.parameters, key),
                    equations: without(config.equations, key),
                  })
                }
              >
                Inherit base
              </button>
            </div>
          )
        })}
      </Section>
      <Section title="Variables">
        {Object.entries(config.variables).map(([key, value]) => (
          <div key={key}>
            <label className="field">
              <span>{key}</span>
              <input
                value={value}
                onChange={(e) =>
                  setConfig({
                    ...config,
                    variables: { ...config.variables, [key]: e.target.value },
                  })
                }
              />
            </label>
            <button
              className="link"
              onClick={() => setConfig({ ...config, variables: without(config.variables, key) })}
            >
              Inherit base
            </button>
          </div>
        ))}
        <label className="field">
          <span>Variable name</span>
          <input value={variable} onChange={(e) => setVariable(e.target.value)} />
        </label>
        <button
          disabled={!variable.trim()}
          onClick={() => {
            setConfig({
              ...config,
              variables: {
                ...config.variables,
                [variable.trim()]: snapshot.variables[variable.trim()] ?? '1mm',
              },
            })
            setVariable('')
          }}
        >
          Override variable
        </button>
      </Section>
      <Section title="Suppression" open={false}>
        {snapshot.features.map((f) => (
          <Choice
            key={f.id}
            label={f.name}
            value={
              f.id in config.suppressed
                ? config.suppressed[f.id]
                  ? 'suppressed'
                  : 'active'
                : 'inherit'
            }
            options={[
              { value: 'inherit', label: 'Inherit base' },
              { value: 'suppressed', label: 'Suppressed' },
              { value: 'active', label: 'Active' },
            ]}
            onChange={(value) =>
              setConfig({
                ...config,
                suppressed:
                  value === 'inherit'
                    ? without(config.suppressed, String(f.id))
                    : { ...config.suppressed, [f.id]: value === 'suppressed' },
              })
            }
          />
        ))}
      </Section>
      <Section title="Material">
        <Choice
          label="Material"
          value={config.material.mode}
          options={[
            { value: 'inherit', label: 'Inherit base' },
            { value: 'clear', label: 'Unspecified' },
            { value: 'set', label: 'Custom' },
          ]}
          onChange={(mode) =>
            setConfig({
              ...config,
              material:
                mode === 'set' ? { mode, value: { name: 'Custom', density: 1000 } } : { mode },
            })
          }
        />
        {config.material.mode === 'set' && (
          <NumberField
            label="Density (kg/m³)"
            value={config.material.value.density}
            min={0.000001}
            onChange={(density) =>
              setConfig({
                ...config,
                material: { mode: 'set', value: { name: 'Custom', density } },
              })
            }
          />
        )}
      </Section>
    </PanelShell>
  )
}
