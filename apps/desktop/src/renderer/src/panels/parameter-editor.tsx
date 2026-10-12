import { useEffect, useState } from 'react'
import { call } from '../core'
import type { EquationValue, FeatureView, Scene } from '../protocol'
import { useStore } from '../store'
import { Choice, ParameterField, Section } from './fields'

export const without = <T,>(map: Record<string, T>, key: string) =>
  Object.fromEntries(Object.entries(map).filter(([k]) => k !== key))

type Draft = {
  scope: 'base' | 'active'
  variables: Record<string, string>
  equations: Record<string, string | null>
  parameters: Record<string, number>
}

type Evaluation = {
  values: Record<string, EquationValue>
  features: FeatureView[]
  scene: Scene
  revision: number
}

/** All edits stay local until OK. Preview responses are discarded after any newer edit. */
export function useEquationDraft(enabled = true) {
  const snapshot = useStore((s) => s.snapshot)!
  const active = snapshot.configurations.items.find((c) => c.id === snapshot.configurations.active)!
  const initial = (scope: Draft['scope']): Draft => ({
    scope,
    variables: { ...(scope === 'base' ? snapshot.variables : active.variables) },
    equations: { ...(scope === 'base' ? snapshot.equations : active.equations) },
    parameters: scope === 'base' ? {} : { ...active.parameters },
  })
  const [draft, setDraft] = useState<Draft>(() => initial('base'))
  const [evaluation, setEvaluation] = useState<{
    request: string
    result?: Evaluation
    error?: string
  }>()
  const params = {
    variables: draft.variables,
    equations: draft.equations,
    parameters: draft.parameters,
    configuration: draft.scope === 'active' ? active.id : undefined,
    revision: snapshot.revision,
  }
  const request = JSON.stringify(params)
  useEffect(() => {
    if (!enabled) return
    let current = true
    const timer = setTimeout(() => {
      void call<Evaluation>('equations.preview', JSON.parse(request))
        .then((result) => {
          if (!current || result.revision !== useStore.getState().snapshot?.revision) return
          setEvaluation({ request, result })
          useStore.setState({ previewScene: result.scene, previewError: null })
        })
        .catch((e: Error) => {
          if (current) {
            setEvaluation({ request, error: e.message })
            useStore.setState({ previewScene: null })
          }
        })
    }, 200)
    return () => {
      current = false
      clearTimeout(timer)
      useStore.setState({ previewScene: null })
    }
  }, [request, enabled])
  const current = evaluation?.request === request
  const values = current ? (evaluation.result?.values ?? {}) : {}
  const error = current
    ? (evaluation.error ??
      Object.values(values).find((v) => v.error)?.error ??
      evaluation.result?.features.find((f) => f.error && !f.suppressed && !f.rolled_back)?.error)
    : undefined
  return {
    snapshot,
    active,
    draft,
    setDraft,
    params,
    values,
    pending: enabled && !current,
    error,
    setScope: (scope: Draft['scope']) => setDraft(initial(scope)),
    save: () => useStore.getState().run('equations.set', params),
    change: (key: string, value: string | number) =>
      setDraft((d) => ({
        ...d,
        equations:
          typeof value === 'string' ? { ...d.equations, [key]: value } : without(d.equations, key),
        parameters:
          typeof value === 'number'
            ? { ...d.parameters, [key]: value }
            : without(d.parameters, key),
      })),
    reset: (key: string) =>
      setDraft((d) => ({
        ...d,
        equations: without(d.equations, key),
        parameters: without(d.parameters, key),
      })),
  }
}

export type EquationDraft = ReturnType<typeof useEquationDraft>

export function EditScope({ editor }: { editor: EquationDraft }) {
  return (
    <>
      <Choice
        label="Edit scope"
        value={editor.draft.scope}
        options={[
          { value: 'base', label: 'Shared base values' },
          { value: 'active', label: `Active configuration: ${editor.active.name}` },
        ]}
        onChange={editor.setScope}
      />
      <p>
        {editor.draft.scope === 'base'
          ? 'Editing shared base values. The preview shows the base; variant overrides apply again after saving.'
          : `Editing only ${editor.active.name}. Reset to base removes this variant’s override.`}
      </p>
    </>
  )
}

export function ParameterEditor({ editor, feature }: { editor: EquationDraft; feature: number }) {
  const parameters = editor.snapshot.parameters.filter((p) => p.feature === feature)
  return (
    <>
      <EditScope editor={editor} />
      <Section title="Parameters and formulas">
        {parameters.length === 0 && <p>This feature has no editable scalar parameters.</p>}
        {parameters.map((p) => {
          const overridden = p.key in editor.draft.equations || p.key in editor.draft.parameters
          const expression =
            editor.draft.equations[p.key] ??
            (editor.draft.scope === 'active' && !overridden
              ? editor.snapshot.equations[p.key]
              : undefined)
          return (
            <div key={p.key}>
              <ParameterField
                parameter={p}
                value={
                  editor.draft.parameters[p.key] ??
                  editor.values[p.key]?.value?.value ??
                  p.value.value
                }
                expression={expression}
                result={editor.values[p.key]}
                onChange={(value) => editor.change(p.key, value)}
              />
              {editor.draft.scope === 'active' && (
                <button className="link" disabled={!overridden} onClick={() => editor.reset(p.key)}>
                  Reset to base
                </button>
              )}
            </div>
          )
        })}
      </Section>
    </>
  )
}
