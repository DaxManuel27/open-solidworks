import { app, safeStorage, shell } from 'electron'
import { randomUUID } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { ChatGPTAuth, type AuthData } from './chatgpt-auth'

let auth: ChatGPTAuth | undefined
const secure = () =>
  safeStorage.isEncryptionAvailable() &&
  (process.platform !== 'linux' || safeStorage.getSelectedStorageBackend() !== 'basic_text')
export function chatGPT(): ChatGPTAuth {
  if (auth) return auth
  const directory = app.getPath('userData'),
    path = join(directory, 'chatgpt.json')
  mkdirSync(directory, { recursive: true })
  let data: AuthData = { host: `urn:uuid:${randomUUID()}`, profiles: [] }
  if (existsSync(path)) {
    // Do not silently replace a corrupt host/registration file and create duplicate clients.
    const saved = JSON.parse(readFileSync(path, 'utf8')) as { data: AuthData; encrypted?: string }
    if (!saved.data?.host?.startsWith('urn:uuid:') || !Array.isArray(saved.data.profiles))
      throw new Error('ChatGPT settings could not be read. Restore chatgpt.json from backup.')
    data = saved.data
    if (saved.encrypted && secure()) {
      try {
        const tokens = JSON.parse(
          safeStorage.decryptString(Buffer.from(saved.encrypted, 'base64')),
        ) as Record<string, AuthData['profiles'][number]['credentials']>
        for (const p of data.profiles) p.credentials = tokens[p.id]
      } catch {
        /* Keep registrations so the user can reconnect if OS decryption fails. */
      }
    }
  }
  const save = (value: AuthData) => {
    const tokens = Object.fromEntries(
      value.profiles.filter((p) => p.credentials).map((p) => [p.id, p.credentials]),
    )
    const metadata = {
      ...value,
      profiles: value.profiles.map((p) => ({
        id: p.id,
        client: p.client,
        label: p.label,
        subject: p.subject,
        email: p.email,
      })),
    }
    const encrypted =
      Object.keys(tokens).length > 0 && secure()
        ? safeStorage.encryptString(JSON.stringify(tokens)).toString('base64')
        : undefined
    const temp = `${path}.tmp`
    writeFileSync(temp, JSON.stringify({ data: metadata, encrypted }), { mode: 0o600 })
    renameSync(temp, path)
  }
  save(data)
  auth = new ChatGPTAuth(data, { save, open: (url) => shell.openExternal(url) })
  return auth
}
export const chatGPTState = () => chatGPT().state(secure() ? 'encrypted' : 'session')
export const openChatGPTUsage = () => shell.openExternal('https://chatgpt.com/settings/usage')

export function cancelChatGPTSignIn(): void {
  auth?.cancel()
}
