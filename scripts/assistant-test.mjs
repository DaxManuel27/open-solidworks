import assert from 'node:assert/strict'
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { resolve, join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { createRequire } from 'node:module'
import ts from 'typescript'
const temporary = mkdtempSync(join(tmpdir(), 'openforge-assistant-tests-'))
const desktop = createRequire(resolve('apps/desktop/package.json'))
const joseURL = pathToFileURL(desktop.resolve('jose')).href
const { SignJWT, generateKeyPair, exportJWK, createLocalJWKSet } = await import(joseURL)
function compile(name, imports = {}) {
  const source = readFileSync(`apps/desktop/src/main/assistant/${name}.ts`, 'utf8')
  let code = ts
    .transpileModule(source, {
      compilerOptions: { target: ts.ScriptTarget.ES2023, module: ts.ModuleKind.ESNext },
    })
    .outputText.replace("from 'jose'", `from ${JSON.stringify(joseURL)}`)
  for (const [from, to] of Object.entries(imports))
    code = code.replace(`from '${from}'`, `from ${JSON.stringify(to)}`)
  const path = join(temporary, `${name}.mjs`)
  writeFileSync(path, code)
  return import(pathToFileURL(path).href)
}
const fetch = globalThis.fetch
const { Response, AbortController, URL, URLSearchParams } = globalThis
const response = (value, status = 200) =>
  new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } })
try {
  const { modelReply, localEndpoint } = await compile('provider')
  assert.equal(localEndpoint('http://localhost:11434/'), 'http://localhost:11434')
  for (const url of [
    'https://example.org',
    'file:///tmp/x',
    'http://localhost:11434/path',
    'http://key@localhost:11434',
    'http://localhost.evil:11434',
  ])
    assert.throws(() => localEndpoint(url))
  const tool = {
    name: 'part_info',
    method: 'doc.info',
    description: 'Read the current part',
    parameters: { type: 'object', properties: {} },
    read_only: true,
  }
  let sent
  const model = async (provider, data, turns = [{ role: 'user', text: 'Inspect' }]) => {
    globalThis.fetch = async (_url, options) => {
      sent = JSON.parse(options.body)
      return new Response(data)
    }
    return modelReply(
      { provider, model: 'fixture', endpoint: 'http://127.0.0.1:11434', rememberKey: false },
      'fixture-token',
      'system',
      turns,
      [tool],
      new AbortController().signal,
      () => {},
    )
  }
  let result = await model(
    'ollama',
    JSON.stringify({
      message: { tool_calls: [{ function: { name: 'part_info', arguments: {} } }] },
      done: true,
    }) + '\n',
  )
  assert.equal(result.calls[0].name, 'part_info')
  await assert.rejects(
    model('ollama', '{"message":{"content":"unfinished"}}\n'),
    /before completion/,
  )
  const sse = (events) => events.map((e) => `data: ${JSON.stringify(e)}\n\n`).join('')
  result = await model(
    'anthropic',
    sse([
      {
        type: 'content_block_start',
        index: 0,
        content_block: { type: 'tool_use', id: 'a', name: 'part_info', input: {} },
      },
      {
        type: 'content_block_delta',
        index: 0,
        delta: { type: 'input_json_delta', partial_json: '{}' },
      },
      { type: 'message_stop' },
    ]),
  )
  assert.equal(result.calls[0].id, 'a')
  const raw = [
    { type: 'reasoning', id: 'r', encrypted_content: 'opaque' },
    { type: 'function_call', call_id: 'c', namespace: 'cad', name: 'part_info', arguments: '{}' },
  ]
  result = await model(
    'chatgpt',
    sse([
      { type: 'response.output_text.delta', delta: 'Checking' },
      {
        type: 'response.completed',
        response: { status: 'completed', output: raw, usage: { total_tokens: 12 } },
      },
    ]),
  )
  assert.equal(result.tokens, 12)
  assert.equal(result.text, 'Checking')
  assert.equal(result.calls[0].name, 'part_info')
  assert.equal(sent.store, false)
  assert.equal(sent.stream, true)
  assert.ok(
    !('tool_choice' in sent),
    'ordinary requests must still allow plain-text answers and clarification',
  )
  assert.equal(sent.tools[0].type, 'namespace')
  assert.ok(!('max_output_tokens' in sent))
  await model(
    'chatgpt',
    sse([{ type: 'response.completed', response: { status: 'completed', output: [] } }]),
    [
      { role: 'assistant', text: result.text, calls: result.calls, raw: result.raw },
      { role: 'tool', call: result.calls[0], text: '{}' },
    ],
  )
  assert.deepEqual(sent.input.slice(0, 2), raw)
  assert.equal(sent.input[2].call_id, 'c')
  await assert.rejects(
    model('chatgpt', sse([{ type: 'response.output_text.delta', delta: 'incomplete' }])),
    /before completion/,
  )
  await assert.rejects(model('chatgpt', sse([{ type: 'response.failed' }])), /did not complete/)
  // Exercise the actual coordinator check, including the outgoing Responses request.
  // Supply fixture credentials in memory; no user settings or subscription is accessed.
  const fixtureSettings = join(temporary, 'fixture-settings.mjs')
  writeFileSync(
    fixtureSettings,
    `export const connection=()=>({settings:{provider:'chatgpt',model:'gpt-6.1-sol',endpoint:'http://127.0.0.1:11434',rememberKey:false},key:'fixture'}); export const connectionKey=async()=> 'fixture';`,
  )
  const { AssistantCoordinator } = await compile('coordinator', {
    './provider': pathToFileURL(join(temporary, 'provider.mjs')).href,
    './settings': pathToFileURL(fixtureSettings).href,
  })
  let probeMode = 'correct',
    probes = 0
  globalThis.fetch = async (url, options) => {
    assert.equal(url, 'https://api.openai.com/v1/responses')
    const body = JSON.parse(options.body)
    probes++
    assert.equal(body.model, 'gpt-6.1-sol')
    assert.equal(
      body.tool_choice,
      'required',
      'the connection check must require an actual tool call',
    )
    assert.equal(body.store, false)
    assert.equal(body.stream, true)
    assert.equal(body.tools.length, 1)
    assert.equal(body.tools[0].type, 'namespace')
    assert.equal(body.tools[0].tools.length, 1)
    assert.equal(body.tools[0].tools[0].name, 'connection_check')
    const nonce = body.input[0].content.split('nonce: ')[1]
    assert.match(nonce, /^[a-f0-9-]{36}$/)
    if (probeMode === 'http') return new Response(null, { status: 401 })
    const output =
      probeMode === 'text'
        ? []
        : [
            {
              type: 'function_call',
              call_id: 'probe',
              namespace: 'cad',
              name: probeMode === 'wrong-name' ? 'part_info' : 'connection_check',
              arguments: JSON.stringify({
                nonce: probeMode === 'wrong-value' ? 'incorrect' : nonce,
              }),
            },
          ]
    return new Response(
      sse([{ type: 'response.completed', response: { status: 'completed', output } }]),
    )
  }
  const coordinator = new AssistantCoordinator(
    async () => {
      throw new Error('Connection check must not call the CAD core')
    },
    () => {},
  )
  assert.equal(await coordinator.test(), 'Connected. Tool calling works.')
  for (const mode of ['text', 'wrong-name', 'wrong-value']) {
    probeMode = mode
    await assert.rejects(coordinator.test(), (error) => {
      assert.match(error.message, /gpt-6.1-sol/)
      assert.match(error.message, mode === 'text' ? /without calling/ : /did not match/)
      assert.ok(!error.message.includes('Select a model with tool support'))
      return true
    })
    coordinator.assertIdle()
  }
  probeMode = 'http'
  await assert.rejects(coordinator.test(), /Reconnect your account/)
  assert.equal(
    probes,
    5,
    'do not automatically repeat requests that consume subscription allowance',
  )
  probeMode = 'correct'
  assert.equal(await coordinator.test(), 'Connected. Tool calling works.')
  const { ChatGPTAuth, verifyIdentity } = await compile('chatgpt-auth')
  const { publicKey, privateKey } = await generateKeyPair('RS256')
  const keys = createLocalJWKSet({ keys: [await exportJWK(publicKey)] })
  const jwt = await new SignJWT({ nonce: 'nonce', email: 'example@example.com' })
    .setProtectedHeader({ alg: 'RS256' })
    .setIssuer('https://auth.openai.com')
    .setAudience('client-1')
    .setSubject('user-1')
    .setIssuedAt()
    .setExpirationTime('1h')
    .sign(privateKey)
  assert.equal((await verifyIdentity(jwt, 'client-1', 'nonce', keys)).subject, 'user-1')
  await assert.rejects(verifyIdentity(jwt, 'wrong', 'nonce', keys))
  await assert.rejects(verifyIdentity(jwt, 'client-1', 'wrong', keys))
  const data = { host: 'urn:uuid:fixture-host', profiles: [] }
  let saved = 0,
    tokenCalls = 0,
    authorizations = [],
    badState = false,
    deny = false,
    changedClient = false,
    identity = 'user-1',
    nowExpiry = 3600,
    scopes = 'chatgpt.tokens.use.direct openid',
    issued = 0
  globalThis.fetch = async (url, options) => {
    assert.equal(String(url), 'https://auth.openai.com/api/accounts/oauth/token')
    tokenCalls++
    const body = new URLSearchParams(options.body)
    assert.notEqual(body.get('client_id'), 'dynamic_agent_client')
    if (body.get('grant_type') === 'authorization_code') {
      const auth = authorizations.at(-1)
      assert.equal(body.get('redirect_uri'), auth.searchParams.get('redirect_uri'))
      assert.ok(body.get('code_verifier'))
      assert.ok(saved > 0, 'registration saved before exchange')
    } else {
      assert.equal(body.get('grant_type'), 'refresh_token')
      assert.ok(!body.has('scope'))
    }
    return response({
      access_token: `access-${tokenCalls}`,
      refresh_token: `refresh-${tokenCalls}`,
      id_token: 'jwt',
      token_type: 'Bearer',
      expires_in: nowExpiry,
      scope: scopes,
    })
  }
  const auth = new ChatGPTAuth(data, {
    save: () => {
      saved++
    },
    verify: async () => ({ subject: identity, email: 'example@example.com' }),
    open: async (address) => {
      const url = new URL(address)
      authorizations.push(url)
      const callback = new URL(url.searchParams.get('redirect_uri'))
      callback.searchParams.set('state', badState ? 'wrong' : url.searchParams.get('state'))
      callback.searchParams.set('code', 'code')
      const client = url.searchParams.get('client_id')
      if (client === 'dynamic_agent_client')
        callback.searchParams.set('client_id', `client-${++issued}`)
      else if (changedClient) callback.searchParams.set('client_id', 'wrong')
      if (deny) callback.searchParams.set('error', 'access_denied')
      const reply = await fetch(callback)
      if (badState) {
        assert.equal(reply.status, 400)
        auth.cancel()
      }
    },
  })
  await auth.signIn()
  assert.equal(auth.state('session').profiles.length, 1)
  assert.equal(await auth.access(), 'access-1')
  const first = data.active
  await auth.signIn(first)
  assert.equal(authorizations.at(-1).searchParams.get('client_id'), 'client-1')
  assert.ok(!authorizations.at(-1).searchParams.has('agent_name_hint'))
  assert.equal(
    authorizations[0].searchParams.get('ext_agent_host_id'),
    authorizations[1].searchParams.get('ext_agent_host_id'),
  )
  const before = tokenCalls
  badState = true
  await assert.rejects(auth.signIn(), /cancelled/)
  badState = false
  assert.equal(tokenCalls, before)
  deny = true
  await assert.rejects(auth.signIn(), /not authorized/)
  deny = false
  assert.equal(tokenCalls, before)
  changedClient = true
  await assert.rejects(auth.signIn(first), /invalid registration/)
  changedClient = false
  identity = 'different'
  await assert.rejects(auth.signIn(first), /does not match/)
  identity = 'user-1'
  assert.equal(data.active, first)
  await auth.signIn()
  assert.equal(data.profiles.length, 2)
  assert.notEqual(data.active, first)
  auth.select(first)
  data.profiles[0].credentials.expires = 0
  const refreshBefore = tokenCalls
  const renewed = await Promise.all([auth.access(), auth.access()])
  assert.equal(tokenCalls, refreshBefore + 1)
  assert.equal(renewed[0], renewed[1])
  scopes = 'openid'
  await auth.signIn()
  await assert.rejects(auth.access(), /not enabled/)
  auth.select(first)
  globalThis.fetch = async (url) =>
    String(url).includes('openid-configuration')
      ? response({ revocation_endpoint: 'https://auth.openai.com/revoke' })
      : new Response(null, { status: 200 })
  assert.equal(await auth.signOut(first), 'Signed out of ChatGPT.')
  assert.equal(data.profiles[0].client, 'client-1')
  assert.ok(!data.profiles[0].credentials)
  console.log(
    'Assistant tests passed: provider streams, completion guards, Responses history, JWT verification, OAuth callbacks, account isolation, rotating refresh and sign-out.',
  )
} finally {
  globalThis.fetch = fetch
  rmSync(temporary, { recursive: true, force: true })
}
