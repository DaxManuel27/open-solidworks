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
  /** `part`/`document`: open an .ofp, .ofa or .ofd; `component`/`model`: an .ofp or .ofa; `import`: STEP/IGES; `step`: STEP only; `standard`: .ofstd; `template`: .ofdt. */
  openDialog(
    kind: 'part' | 'document' | 'component' | 'import' | 'step' | 'model' | 'standard' | 'template',
  ): Promise<string | null>
  saveDialog(
    kind:
      | 'part'
      | 'assembly'
      | 'drawing'
      | 'export'
      | 'csv'
      | 'pdf'
      | 'dxf'
      | 'drawing-export'
      | 'standard',
    defaultName: string,
  ): Promise<string | null>
  /** Report the window title, unsaved-changes state and file. */
  setDocState(title: string, dirty: boolean, path?: string | null): void
  /** Close the window without asking again. */
  closeWindow(): void
  /** Open a file in its own window (or focus the window that has it). */
  openWindow(path: string): void
  /** Open a new drawing window with a standard 3 view of a saved model. */
  openDrawing(model: string): void
  /** A temporary file to write a PDF to for printing. */
  printPath(): Promise<string>
  /** Open a PDF in the system viewer to print it. */
  openPrint(path: string): Promise<void>
  /** Tell other windows a file was saved, so assemblies using it reload. */
  notifySaved(path: string): void
  onFileSaved(handler: (path: string) => void): () => void
  /** What this window was opened for. */
  launch(): {
    open: string | null
    kind: 'part' | 'assembly' | 'drawing' | null
    /** A model to make the new drawing from. */
    model: string | null
  }
  onMenu(handler: (command: string) => void): () => void
  onCoreCrashed(handler: () => void): () => void
}

declare global {
  interface Window {
    openforge: OpenForgeApi
  }
}
