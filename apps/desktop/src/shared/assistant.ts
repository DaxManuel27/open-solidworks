export type AssistantMode = 'edit' | 'preview' | 'ask'
export interface AssistantSettings {
  provider: 'chatgpt' | 'anthropic' | 'ollama'
  model: string
  endpoint: string
  rememberKey: boolean
  hasKey?: boolean
  storage?: 'encrypted' | 'session'
}
export interface AssistantRequest {
  prompt: string
  mode: AssistantMode
  generation: string
  revision: number
  selection: unknown[]
  view: string
  editing: boolean
}
export type AssistantEvent = {
  type: 'started' | 'text' | 'progress' | 'preview' | 'committed' | 'done' | 'error' | 'cleared'
  text?: string
  snapshot?: unknown
  scene?: unknown
  proposal?: string
  usage?: number
}
export interface ToolSpec {
  name: string
  method: string
  description: string
  parameters: Record<string, unknown>
  read_only: boolean
}
export interface ToolCall {
  id: string
  name: string
  input: Record<string, unknown>
}
export interface Turn {
  role: 'user' | 'assistant' | 'tool'
  text: string
  raw?: unknown[]
  calls?: ToolCall[]
  call?: ToolCall
}
export interface ModelReply {
  text: string
  calls: ToolCall[]
  tokens: number
  raw?: unknown[]
}

export interface ChatGPTState {
  storage: 'encrypted' | 'session'
  active?: string
  profiles: { id: string; label: string; email?: string; connected: boolean; sharing: boolean }[]
  models?: { id: string; name: string }[]
}
