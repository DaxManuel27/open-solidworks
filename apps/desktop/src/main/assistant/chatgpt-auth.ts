import { createHash, randomBytes, randomUUID } from 'node:crypto'
import { createServer } from 'node:http'
import { createRemoteJWKSet, jwtVerify } from 'jose'
import type { ChatGPTState } from '../../shared/assistant'

const ISSUER = 'https://auth.openai.com'
const RESOURCE = 'https://api.openai.com/v1'
const TOKEN = `${ISSUER}/api/accounts/oauth/token`
const SCOPE = 'openid profile email offline_access resource.invoke chatgpt.tokens.use.direct'
const jwks = createRemoteJWKSet(new URL(`${ISSUER}/.well-known/jwks.json`))
interface Credentials {
  access: string
  refresh: string
  identity: string
  expires: number
  scopes: string[]
}
interface Profile {
  id: string
  client: string
  label: string
  subject?: string
  email?: string
  credentials?: Credentials
}
export interface AuthData {
  host: string
  active?: string
  profiles: Profile[]
}
interface Dependencies {
  save: (data: AuthData) => void
  open: (url: string) => Promise<void>
  verify?: (
    token: string,
    client: string,
    nonce?: string,
  ) => Promise<{ subject: string; email?: string }>
}
export async function verifyIdentity(
  token: string,
  client: string,
  nonce?: string,
  keys: Parameters<typeof jwtVerify>[1] = jwks,
) {
  const { payload } = await jwtVerify(token, keys, {
    issuer: ISSUER,
    audience: client,
    requiredClaims: ['sub', 'exp', 'iat'],
    clockTolerance: 5,
  })
  if (
    !payload.sub ||
    (nonce !== undefined && payload.nonce !== nonce) ||
    payload.iat! > Date.now() / 1000 + 5
  )
    throw new Error('ChatGPT identity validation failed.')
  return {
    subject: payload.sub,
    email: typeof payload.email === 'string' ? payload.email : undefined,
  }
}
async function tokenRequest(
  body: Record<string, string>,
  signal?: AbortSignal,
): Promise<Record<string, unknown>> {
  const response = await fetch(TOKEN, {
    method: 'POST',
    redirect: 'error',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({ ...body, resource: RESOURCE }),
    signal: signal
      ? AbortSignal.any([signal, AbortSignal.timeout(20000)])
      : AbortSignal.timeout(20000),
  })
  if (!response.ok)
    throw new Error(
      `ChatGPT authorization returned HTTP ${response.status}. Reconnect this account.`,
    )
  return (await response.json()) as Record<string, unknown>
}
function credentials(data: Record<string, unknown>, old?: Credentials): Credentials {
  if (
    typeof data.access_token !== 'string' ||
    !data.access_token ||
    typeof data.refresh_token !== 'string' ||
    !data.refresh_token ||
    typeof data.expires_in !== 'number' ||
    !Number.isFinite(data.expires_in) ||
    data.expires_in <= 0 ||
    String(data.token_type).toLowerCase() !== 'bearer'
  )
    throw new Error('ChatGPT returned an invalid token response.')
  const identity = typeof data.id_token === 'string' ? data.id_token : old?.identity
  if (!identity) throw new Error('ChatGPT did not return an identity token.')
  return {
    access: data.access_token,
    refresh: data.refresh_token,
    identity,
    expires: Date.now() + data.expires_in * 1000,
    scopes: typeof data.scope === 'string' ? data.scope.split(/\s+/) : (old?.scopes ?? []),
  }
}
export class ChatGPTAuth {
  private signingOut = new Set<string>()
  private pending: AbortController | null = null
  private refreshes = new Map<string, Promise<string>>()
  constructor(
    private data: AuthData,
    private deps: Dependencies,
  ) {}
  state(storage: 'encrypted' | 'session'): ChatGPTState {
    return {
      storage,
      active: this.data.active,
      profiles: this.data.profiles.map((p) => ({
        id: p.id,
        label: p.label,
        email: p.email,
        connected: !!p.credentials,
        sharing: !!p.credentials?.scopes.includes('chatgpt.tokens.use.direct'),
      })),
    }
  }
  cancel(): void {
    this.pending?.abort()
  }
  select(id: string): void {
    if (this.pending) throw new Error('Finish or cancel sign-in first.')
    const p = this.profile(id)
    if (!p.subject || !p.credentials) throw new Error('Reconnect this account first.')
    this.data.active = id
    this.deps.save(this.data)
  }
  private profile(id = this.data.active): Profile {
    const p = this.data.profiles.find((p) => p.id === id)
    if (!p) throw new Error('Continue with ChatGPT in Assistant settings first.')
    return p
  }
  async signIn(id?: string): Promise<void> {
    if (this.pending) throw new Error('A ChatGPT sign-in is already in progress.')
    const existing = id ? this.profile(id) : undefined
    const controller = new AbortController()
    this.pending = controller
    const state = randomBytes(32).toString('base64url'),
      nonce = randomBytes(32).toString('base64url'),
      verifier = randomBytes(32).toString('base64url')
    let settle: (value: URLSearchParams) => void = () => {}
    let fail: (error: Error) => void = () => {}
    const callback = new Promise<URLSearchParams>((resolve, reject) => {
      settle = resolve
      fail = reject
    })
    // Handle cancellation before the browser-opening promise settles, too.
    void callback.catch(() => {})
    let consumed = false
    const server = createServer((req, res) => {
      const url = new URL(req.url ?? '/', 'http://127.0.0.1')
      res.setHeader('Cache-Control', 'no-store')
      res.setHeader('Content-Type', 'text/plain; charset=utf-8')
      if (req.method !== 'GET' || url.pathname !== '/auth/callback') {
        res.writeHead(404).end()
        return
      }
      if (
        url.searchParams.get('state') !== state ||
        url.searchParams.getAll('state').length !== 1 ||
        consumed
      ) {
        res.writeHead(400).end('Invalid sign-in callback.')
        return
      }
      consumed = true
      res.end('Return to OpenForge to finish connecting your ChatGPT account.')
      settle(url.searchParams)
    })
    const timer = setTimeout(() => controller.abort(), 300000)
    controller.signal.addEventListener(
      'abort',
      () => fail(new Error('ChatGPT sign-in cancelled or timed out.')),
      { once: true },
    )
    try {
      await new Promise<void>((resolve, reject) => {
        server.once('error', reject)
        server.listen(0, '127.0.0.1', resolve)
      })
      const address = server.address()
      if (!address || typeof address === 'string')
        throw new Error('Unable to start the sign-in callback.')
      const redirect = `http://127.0.0.1:${address.port}/auth/callback`
      const params = new URLSearchParams({
        client_id: existing?.client ?? 'dynamic_agent_client',
        ext_agent_host_id: this.data.host,
        response_type: 'code',
        redirect_uri: redirect,
        scope: SCOPE,
        resource: RESOURCE,
        state,
        nonce,
        code_challenge_method: 'S256',
        code_challenge: createHash('sha256').update(verifier).digest('base64url'),
      })
      if (!existing) params.set('agent_name_hint', 'OpenForge')
      if (existing?.credentials?.identity)
        params.set('id_token_hint', existing.credentials.identity)
      if (existing?.email) params.set('login_hint', existing.email)
      await this.deps.open(`${ISSUER}/api/accounts/authorize?${params}`)
      const result = await callback
      controller.signal.throwIfAborted()
      if (result.has('error'))
        throw new Error('ChatGPT sign-in was not authorized. You can try again.')
      const code = result.get('code'),
        client = result.get('client_id') ?? existing?.client
      if (
        !code ||
        !client ||
        client === 'dynamic_agent_client' ||
        (existing && client !== existing.client)
      )
        throw new Error('ChatGPT returned an invalid registration.')
      const profile = existing ?? {
        id: randomUUID(),
        client,
        label: `Connection ${this.data.profiles.length + 1}`,
      }
      if (!existing) {
        this.data.profiles.push(profile)
        this.deps.save(this.data)
      }
      const data = await tokenRequest(
        {
          grant_type: 'authorization_code',
          client_id: client,
          code,
          code_verifier: verifier,
          redirect_uri: redirect,
        },
        controller.signal,
      )
      const creds = credentials(data)
      const identity = await (this.deps.verify ?? verifyIdentity)(creds.identity, client, nonce)
      if (profile.subject && identity.subject !== profile.subject)
        throw new Error('This identity does not match the saved ChatGPT connection.')
      controller.signal.throwIfAborted()
      Object.assign(profile, {
        subject: identity.subject,
        email: identity.email,
        credentials: creds,
      })
      this.data.active = profile.id
      this.deps.save(this.data)
    } finally {
      clearTimeout(timer)
      server.close()
      server.closeAllConnections()
      this.pending = null
    }
  }
  async access(id = this.data.active): Promise<string> {
    const p = this.profile(id)
    if (this.pending || this.signingOut.has(p.id))
      throw new Error('Finish connecting or signing out of ChatGPT first.')
    if (!p.credentials?.scopes.includes('chatgpt.tokens.use.direct'))
      throw new Error(
        'ChatGPT plan usage is not enabled. Reconnect and grant plan usage, or choose another connection.',
      )
    if (p.credentials.expires > Date.now() + 60000) return p.credentials.access
    const running = this.refreshes.get(p.id)
    if (running) return running
    const refresh = (async () => {
      const before = p.credentials!
      const data = await tokenRequest({
        grant_type: 'refresh_token',
        client_id: p.client,
        refresh_token: before.refresh,
      })
      const next = credentials(data, before)
      if (typeof data.id_token === 'string') {
        const identity = await (this.deps.verify ?? verifyIdentity)(next.identity, p.client)
        if (identity.subject !== p.subject)
          throw new Error('Refreshed ChatGPT identity does not match.')
      }
      if (p.credentials !== before) throw new Error('ChatGPT connection changed during renewal.')
      p.credentials = next
      this.deps.save(this.data)
      if (!next.scopes.includes('chatgpt.tokens.use.direct'))
        throw new Error('ChatGPT plan usage permission was removed. Reconnect this account.')
      return next.access
    })()
    this.refreshes.set(p.id, refresh)
    try {
      return await refresh
    } finally {
      this.refreshes.delete(p.id)
    }
  }
  async models(): Promise<ChatGPTState['models']> {
    const access = await this.access()
    const response = await fetch(`${RESOURCE}/models`, {
      headers: { Authorization: `Bearer ${access}` },
      redirect: 'error',
      signal: AbortSignal.timeout(20000),
    })
    if (!response.ok)
      throw new Error(
        `ChatGPT model discovery returned HTTP ${response.status}. Reconnect or check plan access.`,
      )
    const data = (await response.json()) as {
      models?: { slug: string; display_name: string; visibility: string }[]
    }
    if (!Array.isArray(data.models)) throw new Error('ChatGPT returned an invalid model catalog.')
    return data.models
      .filter(
        (m) =>
          m.visibility === 'list' &&
          typeof m.slug === 'string' &&
          typeof m.display_name === 'string',
      )
      .map((m) => ({ id: m.slug, name: m.display_name }))
  }
  async signOut(id: string): Promise<string> {
    if (this.pending) throw new Error('Finish or cancel sign-in first.')
    const p = this.profile(id)
    if (this.signingOut.has(id)) throw new Error('Sign-out is already in progress.')
    this.signingOut.add(id)
    await this.refreshes.get(id)?.catch(() => {})
    const refresh = p.credentials?.refresh
    // Invalidate locally before awaits so no new inference can start with this account.
    p.credentials = undefined
    try {
      this.deps.save(this.data)
    } catch (e) {
      this.signingOut.delete(id)
      throw e
    }
    let revoked = !refresh
    if (refresh)
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          if (attempt) await new Promise((r) => setTimeout(r, attempt * 500))
          const discovery = await fetch(`${ISSUER}/.well-known/openid-configuration`, {
            signal: AbortSignal.timeout(10000),
            redirect: 'error',
          })
          if (!discovery.ok) throw new Error('Discovery unavailable')
          const config = (await discovery.json()) as { revocation_endpoint: string }
          const url = new URL(config.revocation_endpoint)
          if (url.origin !== ISSUER) throw new Error('Invalid revocation endpoint')
          const response = await fetch(url, {
            method: 'POST',
            headers: { 'content-type': 'application/x-www-form-urlencoded' },
            body: new URLSearchParams({
              token: refresh,
              token_type_hint: 'refresh_token',
              client_id: p.client,
            }),
            signal: AbortSignal.timeout(10000),
            redirect: 'error',
          })
          if (response.status === 200) {
            revoked = true
            break
          }
          if (response.status < 500) break
        } catch {
          /* Retry bounded transient failures without logging tokens. */
        }
      }
    this.signingOut.delete(id)
    return revoked
      ? 'Signed out of ChatGPT.'
      : 'Signed out locally. Remote revocation was not confirmed; disconnect OpenForge in ChatGPT Settings → Usage.'
  }
}
