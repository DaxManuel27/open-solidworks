import { contextBridge } from 'electron'
import type { OpenForgeApi } from './api'

// The renderer's only door to the outside world. Core RPC is added here in M1.
const api: OpenForgeApi = {
  platform: process.platform,
  versions: {
    electron: process.versions.electron ?? '',
    chrome: process.versions.chrome ?? '',
  },
}

contextBridge.exposeInMainWorld('openforge', api)
