import { useEffect, useRef, useState } from 'react'
import type { AssistantMode, AssistantSettings, ChatGPTState } from '../../../shared/assistant'
import type { Scene, Snapshot } from '../protocol'
import { useStore } from '../store'
import './assistant.css'

type Entry = { role: 'user' | 'assistant' | 'status'; text: string }
export function Assistant() {
  const [open, setOpen] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [settings, setSettings] = useState<AssistantSettings | null>(null)
  const [chatgpt, setChatgpt] = useState<ChatGPTState | null>(null)
  const [account, setAccount] = useState('')
  const [authenticating, setAuthenticating] = useState(false)
  const [key, setKey] = useState('')
  const [settingsMessage, setSettingsMessage] = useState('')
  const [testing, setTesting] = useState(false)
  const [prompt, setPrompt] = useState('')
  const [mode, setMode] = useState<AssistantMode>('edit')
  const [busy, setBusy] = useState(false)
  const [progress, setProgress] = useState('')
  const [entries, setEntries] = useState<Entry[]>([])
  const [proposal, setProposal] = useState<string | null>(null)
  const [usage, setUsage] = useState<number | null>(null)
  const [includeSelection, setIncludeSelection] = useState(true)
  const selection = useStore((s) => s.selection)
  const treeSelection = useStore((s) => s.treeSelection)
  const snapshot = useStore((s) => s.snapshot)
  const panel = useStore((s) => s.panel)
  const scroll = useRef<HTMLDivElement>(null)
  const preview = useRef<Scene | null>(null)
  const add = (role: Entry['role'], text: string) =>
    setEntries((e) => [...e, { role, text }].slice(-100))

  useEffect(() => {
    const clearPreview = () => {
      if (preview.current && useStore.getState().previewScene === preview.current)
        useStore.setState({ previewScene: null })
      preview.current = null
      setProposal(null)
    }
    return window.openforge.onAssistant((event) => {
      if (event.type === 'started') {
        setBusy(true)
        setUsage(null)
        setProgress('Reading the current part…')
        setEntries((e) => [...e, { role: 'assistant', text: '' }])
      }
      if (event.type === 'text')
        setEntries((e) => {
          const next = [...e]
          const last = next[next.length - 1]
          if (last?.role === 'assistant')
            next[next.length - 1] = { ...last, text: last.text + (event.text ?? '') }
          else next.push({ role: 'assistant', text: event.text ?? '' })
          return next
        })
      if (event.type === 'progress') setProgress(event.text ?? '')
      if (event.type === 'cleared') clearPreview()
      if (event.type === 'preview') {
        preview.current = event.scene as Scene
        useStore.setState({ previewScene: preview.current })
        setProposal(event.proposal ?? null)
      }
      if (event.type === 'committed') {
        clearPreview()
        const snap = event.snapshot as Snapshot
        const current = useStore.getState().snapshot
        if (current?.generation === snap.generation && snap.revision >= current.revision) {
          useStore.setState({
            snapshot: snap,
            scene: snap.scene ?? null,
            selection: [],
            highlights: [],
            treeSelection: null,
          })
          window.openforge.setDocState(
            `${snap.path?.split(/[\\/]/).pop() ?? 'Untitled'} • — OpenForge`,
            snap.dirty,
          )
        }
      }
      if (['preview', 'committed', 'done', 'error'].includes(event.type)) {
        setBusy(false)
        setProgress('')
        if (event.text)
          setEntries(
            (e) =>
              [
                ...e,
                {
                  role: 'status',
                  text: `${event.type === 'committed' ? 'Applied. ' : event.type === 'preview' ? 'Preview only. ' : ''}${event.text}`,
                },
              ].slice(-100) as Entry[],
          )
        if (event.usage !== undefined) setUsage(event.usage)
      }
    })
  }, [])
  useEffect(() => {
    scroll.current?.scrollTo(0, scroll.current.scrollHeight)
  }, [entries, progress])
  const load = async () => {
    try {
      const current = await window.openforge.assistantSettings()
      setSettings(current)
      setSettingsOpen(true)
      const state = await window.openforge.chatGPTState()
      setChatgpt(state)
      setAccount(state.active ?? '')
      if (
        current.provider === 'chatgpt' &&
        state.profiles.find((p) => p.id === state.active)?.sharing
      )
        setChatgpt({ ...state, models: await window.openforge.chatGPTModels() })
    } catch (e) {
      add('status', String(e))
    }
  }
  const save = async (test: boolean) => {
    if (!settings) return
    setTesting(true)
    setSettingsMessage('')
    try {
      setSettings(await window.openforge.saveAssistantSettings(settings, key || undefined))
      setKey('')
      setSettingsMessage(test ? await window.openforge.testAssistant() : 'Settings saved.')
    } catch (e) {
      setSettingsMessage((e as Error).message)
    } finally {
      setTesting(false)
    }
  }
  const accountAction = async (action: 'sign-in' | 'select' | 'sign-out' | 'models') => {
    setAuthenticating(true)
    setSettingsMessage(
      action === 'sign-in' ? 'Finish signing in using your browser…' : 'Connecting…',
    )
    try {
      let state: ChatGPTState
      let message = ''
      if (action === 'sign-out') {
        message = await window.openforge.signOutChatGPT(account)
        state = await window.openforge.chatGPTState()
      } else if (action === 'sign-in')
        state = await window.openforge.signInChatGPT(account || undefined)
      else if (action === 'select') state = await window.openforge.selectChatGPT(account)
      else state = await window.openforge.chatGPTState()
      setChatgpt(state)
      setAccount(state.active ?? '')
      const sharing = state.profiles.find((p) => p.id === state.active)?.sharing
      const models = sharing ? await window.openforge.chatGPTModels() : []
      setChatgpt({ ...state, models })
      setSettings((s) =>
        s
          ? {
              ...s,
              model: models?.some((m) => m.id === s.model) ? s.model : (models?.[0]?.id ?? ''),
            }
          : s,
      )
      setSettingsMessage(
        message ||
          (sharing
            ? 'ChatGPT plan usage is enabled. Save settings to use this connection.'
            : 'Connected without plan usage permission. Reconnect and allow plan usage to send requests.'),
      )
    } catch (e) {
      setSettingsMessage((e as Error).message)
    } finally {
      setAuthenticating(false)
    }
  }
  const send = async () => {
    const s = useStore.getState()
    if (!s.snapshot || !prompt.trim() || busy) return
    const text = prompt.trim()
    setPrompt('')
    add('user', text)
    setBusy(true)
    try {
      if (mode === 'edit' && /^(please )?undo( the)?( last)?( change)?[.!]?$/i.test(text)) {
        await useStore.getState().undo()
        add('status', 'Undid the last change.')
        return
      }
      if (
        mode === 'edit' &&
        proposal &&
        /^(use (that|this) version|apply( the)? preview)[.!]?$/i.test(text)
      ) {
        await window.openforge.acceptAssistant(proposal)
        return
      }
      if (proposal && /^discard( the)? preview[.!]?$/i.test(text)) {
        await window.openforge.discardAssistant()
        add('status', 'Preview discarded.')
        return
      }
      await window.openforge.askAssistant({
        prompt: text,
        mode,
        generation: s.snapshot.generation,
        revision: s.snapshot.revision,
        selection: includeSelection
          ? [
              ...s.selection.map((x) => x.ref),
              ...(s.treeSelection ? [{ type: 'feature', id: s.treeSelection }] : []),
            ]
          : [],
        view: s.view.name,
        editing: !!s.panel || !!s.sketch,
      })
    } catch (e) {
      add('status', (e as Error).message)
    } finally {
      setBusy(false)
    }
  }
  return (
    <>
      <button
        className="assistant-toggle"
        aria-label="Toggle AI assistant"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        AI Assistant
      </button>
      {open && (
        <aside className="assistant-pane" aria-label="AI assistant">
          <div className="assistant-head">
            <strong>Assistant</strong>
            <button onClick={() => void load()}>Settings</button>
            <button aria-label="Close assistant" onClick={() => setOpen(false)}>
              ×
            </button>
          </div>
          {settingsOpen && settings && (
            <section className="assistant-settings">
              <label>
                Connection
                <select
                  aria-label="AI connection"
                  value={settings.provider}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      provider: e.target.value as AssistantSettings['provider'],
                      model: '',
                    })
                  }
                >
                  <option value="chatgpt">ChatGPT subscription</option>
                  <option value="ollama">Local model (Ollama)</option>
                  <option value="anthropic">API key (Anthropic)</option>
                </select>
              </label>
              {settings.provider === 'chatgpt' ? (
                <>
                  <p>
                    Use an eligible ChatGPT Plus or Pro plan. Selected part context is sent to
                    OpenAI and requests count toward your plan limits.
                  </p>
                  <label>
                    Account
                    <select
                      aria-label="ChatGPT account"
                      disabled={authenticating || busy}
                      value={account}
                      onChange={(e) => setAccount(e.target.value)}
                    >
                      <option value="">Add account…</option>
                      {chatgpt?.profiles.map((p) => (
                        <option key={p.id} value={p.id}>
                          {p.label}
                          {p.email ? ` · ${p.email}` : ''}
                          {p.id === chatgpt.active ? ' (active)' : ''}
                          {!p.connected ? ' · signed out' : ''}
                        </option>
                      ))}
                    </select>
                  </label>
                  <div className="assistant-actions">
                    <button
                      disabled={authenticating || busy}
                      onClick={() => void accountAction('sign-in')}
                    >
                      Continue with ChatGPT
                    </button>
                    {account && (
                      <>
                        <button
                          disabled={authenticating || busy}
                          onClick={() => void accountAction('select')}
                        >
                          Use account
                        </button>
                        <button
                          disabled={authenticating || busy}
                          onClick={() => void accountAction('sign-out')}
                        >
                          Sign out
                        </button>
                      </>
                    )}
                    {authenticating && (
                      <button onClick={() => window.openforge.cancelChatGPT()}>
                        Cancel sign-in
                      </button>
                    )}
                  </div>
                  <p>
                    {chatgpt?.profiles.find((p) => p.id === chatgpt.active)?.sharing
                      ? 'Using ChatGPT plan'
                      : 'ChatGPT plan usage is not enabled.'}
                  </p>
                  {chatgpt?.storage === 'session' && (
                    <p>Secure storage unavailable. Sign-in lasts for this app session.</p>
                  )}
                  <button
                    onClick={() =>
                      void window.openforge
                        .openChatGPTUsage()
                        .catch((e) => setSettingsMessage(String(e)))
                    }
                  >
                    Manage usage
                  </button>
                </>
              ) : settings.provider === 'ollama' ? (
                <label>
                  Local server
                  <input
                    aria-label="Local model endpoint"
                    value={settings.endpoint}
                    onChange={(e) => setSettings({ ...settings, endpoint: e.target.value })}
                  />
                </label>
              ) : (
                <>
                  <p>
                    Selected part context is sent to Anthropic. API usage is billed to your account.
                  </p>
                  <label>
                    API key
                    <input
                      aria-label="AI API key"
                      type="password"
                      autoComplete="off"
                      placeholder={
                        settings.hasKey ? 'Key saved; leave empty to keep' : 'Paste your API key'
                      }
                      value={key}
                      onChange={(e) => setKey(e.target.value)}
                    />
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={settings.rememberKey}
                      onChange={(e) => setSettings({ ...settings, rememberKey: e.target.checked })}
                    />{' '}
                    Remember key securely
                  </label>
                  {settings.storage === 'session' && (
                    <p>Secure storage unavailable. Keys are kept only for this session.</p>
                  )}
                  <button
                    onClick={() =>
                      void window.openforge
                        .saveAssistantSettings(settings, '')
                        .then(setSettings)
                        .catch((e) => setSettingsMessage(String(e)))
                    }
                  >
                    Remove saved key
                  </button>
                </>
              )}
              {settings.provider === 'chatgpt' ? (
                <label>
                  Model
                  <select
                    aria-label="AI model"
                    value={settings.model}
                    onChange={(e) => setSettings({ ...settings, model: e.target.value })}
                  >
                    <option value="">Choose a model</option>
                    {chatgpt?.models?.map((m) => (
                      <option key={m.id} value={m.id}>
                        {m.name}
                      </option>
                    ))}
                  </select>
                  <button
                    disabled={authenticating || busy}
                    onClick={() => void accountAction('models')}
                  >
                    Refresh models
                  </button>
                </label>
              ) : (
                <label>
                  Model
                  <input
                    aria-label="AI model"
                    value={settings.model}
                    placeholder="Model ID from your provider or Ollama"
                    onChange={(e) => setSettings({ ...settings, model: e.target.value })}
                  />
                </label>
              )}
              <p>
                Choose a model with tool calling. Local mode never falls back to a cloud service.
              </p>
              <div className="assistant-actions">
                <button
                  disabled={testing || busy || authenticating}
                  onClick={() => void save(true)}
                >
                  Save & Test connection
                </button>
                <button
                  disabled={testing || busy || authenticating}
                  onClick={() => void save(false)}
                >
                  Save settings
                </button>
                <button onClick={() => setSettingsOpen(false)}>Close settings</button>
              </div>
              {testing && (
                <button onClick={() => window.openforge.stopAssistant()}>
                  Cancel connection test
                </button>
              )}
              <p role="status">{settingsMessage}</p>
            </section>
          )}
          <div className="assistant-messages" ref={scroll} aria-label="Assistant conversation">
            {!entries.length && (
              <p>
                Describe a part or select geometry and ask for a change. Changes remain editable and
                can be undone.
              </p>
            )}
            {entries.map((entry, i) => (
              <div key={i} className={`assistant-message ${entry.role}`}>
                <strong>
                  {entry.role === 'user'
                    ? 'You'
                    : entry.role === 'assistant'
                      ? 'Assistant'
                      : 'Result'}
                </strong>
                <p>{entry.text}</p>
              </div>
            ))}
          </div>
          {progress && (
            <p className="assistant-progress" role="status">
              {progress}
            </p>
          )}
          {proposal && (
            <div className="assistant-actions">
              <button
                disabled={busy || !!panel}
                onClick={() => {
                  setBusy(true)
                  void window.openforge
                    .acceptAssistant(proposal)
                    .catch((e) => add('status', String(e)))
                    .finally(() => setBusy(false))
                }}
              >
                Apply preview
              </button>
              <button disabled={busy} onClick={() => void window.openforge.discardAssistant()}>
                Discard preview
              </button>
            </div>
          )}
          <div className="assistant-context">
            <label>
              <input
                type="checkbox"
                checked={includeSelection}
                onChange={(e) => setIncludeSelection(e.target.checked)}
              />{' '}
              Use selection ({selection.length + (treeSelection ? 1 : 0)})
            </label>
            <span>
              {
                snapshot?.configurations.items.find((c) => c.id === snapshot.configurations.active)
                  ?.name
              }
            </span>
          </div>
          {!!panel && (
            <p className="assistant-progress">
              Finish the open editor to make AI changes. Questions still work.
            </p>
          )}
          <form
            onSubmit={(e) => {
              e.preventDefault()
              void send()
            }}
          >
            <select
              aria-label="Assistant mode"
              value={mode}
              disabled={busy}
              onChange={(e) => setMode(e.target.value as AssistantMode)}
            >
              <option value="edit">Make changes</option>
              <option value="preview">Preview only</option>
              <option value="ask">Ask / explain</option>
            </select>
            <textarea
              aria-label="Message assistant"
              placeholder="Describe a change…"
              maxLength={12000}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                  e.preventDefault()
                  void send()
                }
              }}
            />
            <div className="assistant-actions">
              <button type="submit" disabled={busy || authenticating || !prompt.trim()}>
                Send
              </button>
              <button
                type="button"
                disabled={!busy}
                onClick={() => window.openforge.stopAssistant()}
              >
                Stop
              </button>
              <button
                type="button"
                disabled={busy || !snapshot?.can_undo}
                onClick={() => void useStore.getState().undo()}
              >
                Undo
              </button>
            </div>
            {usage !== null && (
              <small>{usage.toLocaleString()} tokens reported for this task</small>
            )}
          </form>
        </aside>
      )}
    </>
  )
}
