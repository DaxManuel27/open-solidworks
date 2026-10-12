import { app, safeStorage } from 'electron'
import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { AssistantSettings } from '../../shared/assistant'
import { localEndpoint } from './provider'
import { chatGPT, chatGPTState } from './chatgpt'

let key = ''
let settings: AssistantSettings = {
  provider: 'ollama',
  model: '',
  endpoint: 'http://127.0.0.1:11434',
  rememberKey: false,
}
const path = () => join(app.getPath('userData'), 'assistant.json')
function secure(): boolean {
  return (
    safeStorage.isEncryptionAvailable() &&
    (process.platform !== 'linux' || safeStorage.getSelectedStorageBackend() !== 'basic_text')
  )
}
export function loadSettings(): void {
  try {
    const saved = JSON.parse(readFileSync(path(), 'utf8')) as {
      settings: AssistantSettings
      encrypted?: string
    }
    settings = validate(saved.settings)
    if (saved.encrypted && secure())
      key = safeStorage.decryptString(Buffer.from(saved.encrypted, 'base64'))
  } catch {
    /* First run or key store unavailable: require setup. */
  }
}
function validate(s: AssistantSettings): AssistantSettings {
  if (
    !s ||
    !['chatgpt', 'anthropic', 'ollama'].includes(s.provider) ||
    typeof s.model !== 'string' ||
    s.model.length > 200
  )
    throw new Error('Choose a provider and model.')
  return {
    provider: s.provider,
    model: s.model.trim(),
    endpoint: localEndpoint(s.endpoint || 'http://127.0.0.1:11434'),
    rememberKey: !!s.rememberKey,
  }
}
export function publicSettings(): AssistantSettings {
  return { ...settings, hasKey: !!key, storage: secure() ? 'encrypted' : 'session' }
}
export function saveSettings(s: AssistantSettings, replacement?: string): AssistantSettings {
  const validated = validate(s)
  if (replacement !== undefined && (typeof replacement !== 'string' || replacement.length > 4096))
    throw new Error('Invalid API key.')
  const nextKey = replacement === undefined ? key : replacement.trim()
  const encrypted =
    validated.rememberKey && nextKey && secure()
      ? safeStorage.encryptString(nextKey).toString('base64')
      : undefined
  mkdirSync(app.getPath('userData'), { recursive: true })
  const temp = `${path()}.tmp`
  writeFileSync(temp, JSON.stringify({ settings: validated, encrypted }), { mode: 0o600 })
  renameSync(temp, path())
  settings = validated
  key = nextKey
  return publicSettings()
}
export function connection(): { settings: AssistantSettings; key: string; profile?: string } {
  if (!settings.model) throw new Error('Choose a model in Assistant settings.')
  if (settings.provider === 'anthropic' && !key)
    throw new Error('Enter an API key in Assistant settings.')
  const profile = settings.provider === 'chatgpt' ? chatGPTState().active : undefined
  if (settings.provider === 'chatgpt' && !profile)
    throw new Error('Continue with ChatGPT in Assistant settings first.')
  return { settings: { ...settings }, key, profile }
}

export async function connectionKey(value: ReturnType<typeof connection>): Promise<string> {
  return value.settings.provider === 'chatgpt' ? chatGPT().access(value.profile) : value.key
}
