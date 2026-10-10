import { contextBridge, ipcRenderer, type IpcRendererEvent } from 'electron'
import type { CallResult, OpenForgeApi } from './api'

// The renderer's only door to the outside world.
const api: OpenForgeApi = {
  platform: process.platform,
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
