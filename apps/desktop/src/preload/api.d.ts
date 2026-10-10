import type {
  AssistantEvent,
  AssistantRequest,
  AssistantSettings,
  ChatGPTState,
} from '../shared/assistant'
export type CallResult = { ok: true; value: unknown } | { ok: false; error: string; code: number }

export interface OpenForgeApi {
  platform: string
  chatGPTState(): Promise<ChatGPTState>
  chatGPTModels(): Promise<ChatGPTState['models']>
  signInChatGPT(id?: string): Promise<ChatGPTState>
  selectChatGPT(id: string): Promise<ChatGPTState>
  signOutChatGPT(id: string): Promise<string>
  cancelChatGPT(): void
  openChatGPTUsage(): Promise<void>
  assistantSettings(): Promise<AssistantSettings>
  saveAssistantSettings(settings: AssistantSettings, key?: string): Promise<AssistantSettings>
  testAssistant(): Promise<string>
  askAssistant(request: AssistantRequest): Promise<void>
  stopAssistant(): void
  acceptAssistant(id: string): Promise<void>
  discardAssistant(): Promise<void>
  onAssistant(handler: (event: AssistantEvent) => void): () => void
  /** Call a core method (JSON-RPC). */
  call(method: string, params?: unknown): Promise<CallResult>
  openDialog(kind: 'part' | 'import'): Promise<string | null>
  saveDialog(kind: 'part' | 'export', defaultName: string): Promise<string | null>
  /** Report the window title and unsaved-changes state. */
  setDocState(title: string, dirty: boolean): void
  /** Close the window without asking again. */
  closeWindow(): void
  onMenu(handler: (command: string) => void): () => void
  onCoreCrashed(handler: () => void): () => void
}

declare global {
  interface Window {
    openforge: OpenForgeApi
  }
}
