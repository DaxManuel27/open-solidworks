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
  setDocState: (title, dirty) => ipcRenderer.send('doc:state', title, dirty),
  closeWindow: () => ipcRenderer.send('window:close'),
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
