import { contextBridge, ipcRenderer, type IpcRendererEvent } from 'electron'
import type { CallResult, OpenForgeApi } from './api'
import type { AssistantEvent } from '../shared/assistant'

// The renderer's only door to the outside world.
const api: OpenForgeApi = {
  platform: process.platform,
  chatGPTState: () => ipcRenderer.invoke('chatgpt:state'),
  chatGPTModels: () => ipcRenderer.invoke('chatgpt:models'),
  signInChatGPT: (id) => ipcRenderer.invoke('chatgpt:sign-in', id),
  selectChatGPT: (id) => ipcRenderer.invoke('chatgpt:select', id),
  signOutChatGPT: (id) => ipcRenderer.invoke('chatgpt:sign-out', id),
  cancelChatGPT: () => ipcRenderer.send('chatgpt:cancel'),
  openChatGPTUsage: () => ipcRenderer.invoke('chatgpt:usage'),
  assistantSettings: () => ipcRenderer.invoke('assistant:settings'),
  saveAssistantSettings: (settings, key) =>
    ipcRenderer.invoke('assistant:save-settings', settings, key),
  testAssistant: () => ipcRenderer.invoke('assistant:test'),
  askAssistant: (request) => ipcRenderer.invoke('assistant:run', request),
  stopAssistant: () => ipcRenderer.send('assistant:stop'),
  acceptAssistant: (id) => ipcRenderer.invoke('assistant:accept', id),
  discardAssistant: () => ipcRenderer.invoke('assistant:discard'),
  onAssistant: (handler) => {
    const listener = (_e: IpcRendererEvent, event: AssistantEvent) => handler(event)
    ipcRenderer.on('assistant:event', listener)
    return () => ipcRenderer.removeListener('assistant:event', listener)
  },
  call: (method, params) => ipcRenderer.invoke('core:call', method, params) as Promise<CallResult>,
  openDialog: (kind) => ipcRenderer.invoke('dialog:open', kind) as Promise<string | null>,
  saveDialog: (kind, name) =>
    ipcRenderer.invoke('dialog:save', kind, name) as Promise<string | null>,
  setDocState: (title, dirty, path) => ipcRenderer.send('doc:state', title, dirty, path),
  closeWindow: () => ipcRenderer.send('window:close'),
  openWindow: (path) => ipcRenderer.send('window:open', path),
  openDrawing: (model) => ipcRenderer.send('window:drawing', model),
  printPath: () => ipcRenderer.invoke('print:path') as Promise<string>,
  openPrint: (path) => ipcRenderer.invoke('print:open', path) as Promise<void>,
  notifySaved: (path) => ipcRenderer.send('doc:saved', path),
  onFileSaved: (handler) => {
    const listener = (_e: IpcRendererEvent, path: string) => handler(path)
    ipcRenderer.on('file:saved', listener)
    return () => ipcRenderer.removeListener('file:saved', listener)
  },
  launch: () => {
    // The preload runs in the page, which has a location; its types are Node's.
    const search = (globalThis as { location?: { search: string } }).location?.search ?? ''
    const q = new URLSearchParams(search)
    const kind = q.get('kind')
    return {
      open: q.get('open'),
      kind: kind === 'assembly' || kind === 'part' || kind === 'drawing' ? kind : null,
      model: q.get('model'),
    }
  },
  onMenu: (handler) => {
    const listener = (_e: IpcRendererEvent, command: string) => handler(command)
    ipcRenderer.on('menu', listener)
    return () => ipcRenderer.removeListener('menu', listener)
  },
  onCoreCrashed: (handler) => {
    const listener = () => handler()
    ipcRenderer.on('core:crashed', listener)
    return () => ipcRenderer.removeListener('core:crashed', listener)
  },
}

contextBridge.exposeInMainWorld('openforge', api)
