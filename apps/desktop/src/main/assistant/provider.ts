import type {
  AssistantSettings,
  ModelReply,
  ToolCall,
  ToolSpec,
  Turn,
} from '../../shared/assistant'

export function localEndpoint(value: string): string {
  const url = new URL(value)
  if (
    !['http:', 'https:'].includes(url.protocol) ||
    !['localhost', '127.0.0.1', '[::1]'].includes(url.hostname) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    !['', '/'].includes(url.pathname)
  )
    throw new Error('Local mode requires a loopback URL, such as http://127.0.0.1:11434.')
  return url.origin
}

// Stream readers are bounded and require the provider's completion marker.
export async function* lines(response: Response): AsyncGenerator<string> {
  if (!response.ok)
    throw new Error(
      `Model service returned HTTP ${response.status}. Check the connection, API key and model.`,
    )
  if (!response.body) throw new Error('The model service returned no stream.')
  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let pending = '',
    size = 0
  try {
    while (true) {
      const { value, done } = await reader.read()
      if (done) break
      size += value.length
      if (size > 2_000_000) throw new Error('Model response exceeded the size limit.')
      pending += decoder.decode(value, { stream: true })
      let end: number
      while ((end = pending.indexOf('\n')) >= 0) {
        yield pending.slice(0, end).replace(/\r$/, '')
        pending = pending.slice(end + 1)
      }
    }
    pending += decoder.decode()
    if (pending.trim()) yield pending
  } finally {
    await reader.cancel().catch(() => {})
    reader.releaseLock()
  }
}

interface Block {
  type: string
  text?: string
  id?: string
  name?: string
  input?: Record<string, unknown>
}
interface StreamEvent {
  type?: string
  index?: number
  content_block?: Block
  delta?: { type?: string; text?: string; partial_json?: string; stop_reason?: string }
  message?: {
    content?: string
    tool_calls?: { function: { name: string; arguments: Record<string, unknown> } }[]
    usage?: { input_tokens?: number }
  }
  usage?: { output_tokens?: number }
  done?: boolean
  done_reason?: string
  prompt_eval_count?: number
  eval_count?: number
  error?: unknown
}
export async function modelReply(
  settings: AssistantSettings,
  key: string,
  system: string,
  turns: Turn[],
  tools: ToolSpec[],
  signal: AbortSignal,
  onText: (text: string) => void,
): Promise<ModelReply> {
  if (settings.provider === 'chatgpt')
    return responsesReply(settings.model, key, system, turns, tools, signal, onText)
  const local = settings.provider === 'ollama'
  const messages = local
    ? [
        { role: 'system', content: system },
        ...turns.map((t) => ({
          role: t.role,
          content: t.text,
          ...(t.calls?.length
            ? {
                tool_calls: t.calls.map((c) => ({
                  function: { name: c.name, arguments: c.input },
                })),
              }
            : {}),
          ...(t.call ? { tool_name: t.call.name } : {}),
        })),
      ]
    : turns.map((t) => ({
        role: t.role === 'tool' ? 'user' : t.role,
        content:
          t.role === 'tool'
            ? [{ type: 'tool_result', tool_use_id: t.call!.id, content: t.text }]
            : [
                ...(t.text ? [{ type: 'text', text: t.text }] : []),
                ...(t.calls ?? []).map((c) => ({
                  type: 'tool_use',
                  id: c.id,
                  name: c.name,
                  input: c.input,
                })),
              ],
      }))
  const body = local
    ? {
        model: settings.model,
        messages,
        stream: true,
        think: false,
        options: { num_predict: 4096 },
        tools: tools.map((t) => ({
          type: 'function',
          function: { name: t.name, description: t.description, parameters: t.parameters },
        })),
      }
    : {
        model: settings.model,
        system,
        messages,
        stream: true,
        max_tokens: 4096,
        tools: tools.map((t) => ({
          name: t.name,
          description: t.description,
          input_schema: t.parameters,
        })),
      }
  const response = await fetch(
    local
      ? `${localEndpoint(settings.endpoint)}/api/chat`
      : 'https://api.anthropic.com/v1/messages',
    {
      method: 'POST',
      redirect: 'error',
      signal: AbortSignal.any([signal, AbortSignal.timeout(90000)]),
      headers: {
        'content-type': 'application/json',
        ...(local ? {} : { 'x-api-key': key, 'anthropic-version': '2023-06-01' }),
      },
      body: JSON.stringify(body),
    },
  )
  let text = '',
    complete = false,
    tokens = 0,
    inputTokens = 0,
    outputTokens = 0
  const calls: ToolCall[] = []
  const blocks = new Map<number, { block: Block; json: string }>()
  for await (const line of lines(response)) {
    if (!line.trim() || (!local && !line.startsWith('data: '))) continue
    const event = JSON.parse(local ? line : line.slice(6)) as StreamEvent
    if (event.error || event.type === 'error')
      throw new Error('The model service reported a streaming error. No draft was committed.')
    if (local) {
      const chunk = event.message?.content ?? ''
      text += chunk
      if (chunk) onText(chunk)
      for (const c of event.message?.tool_calls ?? [])
        calls.push({
          id: `call-${calls.length}`,
          name: c.function.name,
          input: c.function.arguments,
        })
      if (event.done) {
        if (event.done_reason === 'length') throw new Error('Model output limit reached.')
        complete = true
        tokens = (event.prompt_eval_count ?? 0) + (event.eval_count ?? 0)
      }
    } else {
      if (event.type === 'content_block_start')
        blocks.set(event.index!, { block: event.content_block!, json: '' })
      if (event.type === 'content_block_delta') {
        if (event.delta?.type === 'text_delta') {
          const chunk = event.delta.text ?? ''
          text += chunk
          onText(chunk)
        }
        if (event.delta?.type === 'input_json_delta') {
          const b = blocks.get(event.index!)
          if (!b) throw new Error('Invalid tool stream.')
          b.json += event.delta.partial_json ?? ''
        }
      }
      if (event.type === 'message_start') inputTokens = event.message?.usage?.input_tokens ?? 0
      if (event.type === 'message_delta') {
        outputTokens = event.usage?.output_tokens ?? outputTokens
        if (event.delta?.stop_reason === 'max_tokens')
          throw new Error('Model output limit reached.')
      }
      if (event.type === 'message_stop') complete = true
    }
  }
  if (!complete)
    throw new Error('Model connection ended before completion. No draft was committed.')
  if (!local) {
    tokens = inputTokens + outputTokens
    for (const { block, json } of blocks.values())
      if (block.type === 'tool_use') {
        const input: unknown = json ? JSON.parse(json) : block.input
        if (!block.id || !block.name || !input || typeof input !== 'object' || Array.isArray(input))
          throw new Error('Invalid tool arguments from model.')
        calls.push({ id: block.id, name: block.name, input: input as Record<string, unknown> })
      }
  }
  if (calls.length > 12) throw new Error('Too many simultaneous tool requests.')
  return { text, calls, tokens }
}

async function responsesReply(
  model: string,
  token: string,
  instructions: string,
  turns: Turn[],
  tools: ToolSpec[],
  signal: AbortSignal,
  onText: (text: string) => void,
): Promise<ModelReply> {
  const input = turns.flatMap<unknown>((t) =>
    t.role === 'tool'
      ? [{ type: 'function_call_output', call_id: t.call!.id, output: t.text }]
      : (t.raw ?? [{ role: t.role, content: t.text }]),
  )
  const response = await fetch('https://api.openai.com/v1/responses', {
    method: 'POST',
    redirect: 'error',
    signal: AbortSignal.any([signal, AbortSignal.timeout(90000)]),
    headers: { 'content-type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({
      model,
      instructions,
      input,
      store: false,
      stream: true,
      include: ['reasoning.encrypted_content'],
      tools: [
        {
          type: 'namespace',
          name: 'cad',
          description: 'Tools for editing the current OpenForge part.',
          tools: tools.map((t) => ({
            type: 'function',
            name: t.name,
            description: t.description,
            parameters: t.parameters,
            strict: false,
          })),
        },
      ],
    }),
  })
  if (response.status === 429)
    throw new Error(
      'ChatGPT plan limit reached. Open Manage usage in Assistant settings, or try again later.',
    )
  if (response.status === 401 || response.status === 403)
    throw new Error(
      'ChatGPT access is unavailable. Reconnect your account and check plan usage permission.',
    )
  let text = '',
    tokens = 0,
    raw: unknown[] | undefined
  const calls: ToolCall[] = []
  for await (const line of lines(response)) {
    if (!line.startsWith('data:')) continue
    const data = line.slice(5).trim()
    if (data === '[DONE]') continue
    const event = JSON.parse(data) as {
      type: string
      delta?: string
      response?: {
        status: string
        output: {
          type: string
          call_id?: string
          name?: string
          namespace?: string
          arguments?: string
        }[]
        usage?: { total_tokens: number }
      }
    }
    if (['error', 'response.failed', 'response.incomplete'].includes(event.type))
      throw new Error('ChatGPT did not complete the response. No draft changes were applied.')
    if (event.type === 'response.output_text.delta') {
      const chunk = event.delta ?? ''
      text += chunk
      onText(chunk)
    }
    if (event.type === 'response.completed') {
      if (raw || event.response?.status !== 'completed' || !Array.isArray(event.response.output))
        throw new Error('Invalid ChatGPT completion.')
      raw = event.response.output
      tokens = event.response.usage?.total_tokens ?? 0
      for (const item of event.response.output)
        if (item.type === 'function_call') {
          if (!item.call_id || !item.name || (item.namespace && item.namespace !== 'cad'))
            throw new Error('Invalid ChatGPT tool request.')
          const args: unknown = JSON.parse(item.arguments ?? 'null')
          if (!args || typeof args !== 'object' || Array.isArray(args))
            throw new Error('Invalid ChatGPT tool arguments.')
          calls.push({ id: item.call_id, name: item.name, input: args as Record<string, unknown> })
        }
    }
  }
  if (!raw)
    throw new Error('ChatGPT connection ended before completion. No draft changes were applied.')
  if (calls.length > 12) throw new Error('Too many simultaneous tool requests.')
  return { text, calls, tokens, raw }
}
