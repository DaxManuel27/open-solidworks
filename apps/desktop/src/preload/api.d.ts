export type CallResult = { ok: true; value: unknown } | { ok: false; error: string; code: number }

export interface OpenForgeApi {
  platform: string
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
