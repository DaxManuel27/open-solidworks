import { useEffect, useState } from 'react'
import { call } from '../core'
import type { EquationValue, FeatureView, Scene } from '../protocol'
import { useStore } from '../store'
import { Choice, FormulaField, NumberField, PanelShell, ParameterField, Section } from './fields'

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
  const [evaluated, setEvaluated] = useState<{
    request: string
    values: Record<string, EquationValue>
  }>()
  const request = JSON.stringify({ configuration: config, revision: snapshot.revision })
  useEffect(() => {
    let current = true
    const timer = setTimeout(() => {
      void call<{
        values: Record<string, EquationValue>
        features: FeatureView[]
        scene: Scene
        revision: number
      }>('configuration.preview', JSON.parse(request))
        .then((result) => {
          if (!current || result.revision !== useStore.getState().snapshot?.revision) return
          setEvaluated({ request, values: result.values })
          setError(
            Object.values(result.values).find((value) => value.error)?.error ??
              result.features.find((f) => f.error && !f.suppressed && !f.rolled_back)?.error ??
              null,
          )
          useStore.setState({ previewScene: result.scene, previewError: null })
        })
        .catch((e: Error) => {
          if (current) setError(e.message)
        })
    }, 200)
    return () => {
      current = false
      clearTimeout(timer)
      useStore.setState({ previewScene: null })
    }
  }, [request])
  const values = evaluated?.request === request ? evaluated.values : {}
  const without = <T,>(map: Record<string, T>, key: string) =>
    Object.fromEntries(Object.entries(map).filter(([k]) => k !== key))
  const keys = [...new Set([...Object.keys(config.parameters), ...Object.keys(config.equations)])]
  return (
    <PanelShell
      title="Configuration Overrides"
      error={error}
      okDisabled={!!error || evaluated?.request !== request}
      onCancel={() => void close()}
      onOk={() =>
        void run('configuration.update', {
          configuration: config,
          revision: snapshot.revision,
        }).then((saved) => {
          if (saved) void close()
          else setError('Could not save the configuration. Check its name and values.')
        })
      }
    >
      <p>
        Edit scope: active configuration {config.name}. These overrides affect only this variant.
        Reset to base removes an override. Preview and Cancel keep shared values unchanged.
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
              parameters: {
                ...config.parameters,
                [parameter]: values[parameter]?.value?.value ?? p.value.value,
              },
            })
          }}
        >
          Override parameter
        </button>
        {keys.map((key) => {
          const p = snapshot.parameters.find((p) => p.key === key)
          return (
            <div key={key}>
              {p ? (
                <ParameterField
                  parameter={p}
                  value={config.parameters[key] ?? values[key]?.value?.value ?? p.value.value}
                  expression={config.equations[key]}
                  result={values[key]}
                  onChange={(value) =>
                    setConfig({
                      ...config,
                      parameters:
                        typeof value === 'number'
                          ? { ...config.parameters, [key]: value }
                          : without(config.parameters, key),
                      equations:
                        typeof value === 'string'
                          ? { ...config.equations, [key]: value }
                          : without(config.equations, key),
                    })
                  }
                />
              ) : (
                <p className="panel-error">Parameter {key} no longer exists.</p>
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
                Reset to base
              </button>
            </div>
          )
        })}
      </Section>
      <Section title="Variables">
        {Object.entries(config.variables).map(([key, value]) => (
          <div key={key}>
            <FormulaField
              label={key}
              expression={value}
              result={values[key]}
              onChange={(value) =>
                setConfig({ ...config, variables: { ...config.variables, [key]: value } })
              }
            />
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
