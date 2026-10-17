import { tmpdir } from 'node:os'
import { join } from 'node:path'
import {
  BrowserWindow,
  Menu,
  app,
  dialog,
  ipcMain,
  shell,
  type MenuItemConstructorOptions,
  type WebContents,
} from 'electron'
import { CoreError, CoreProcess, findCore } from './core'
import { chatGPT, chatGPTState, openChatGPTUsage, cancelChatGPTSignIn } from './assistant/chatgpt'
import { AssistantCoordinator } from './assistant/coordinator'
import { loadSettings, publicSettings, saveSettings } from './assistant/settings'
import type { AssistantRequest, AssistantSettings } from '../shared/assistant'

/** One open document: its window and the core process that owns it. */
interface Doc {
  win: BrowserWindow
  core: CoreProcess | null
  dirty: boolean
  closing: boolean
  assistant: AssistantCoordinator
  /** The document's file, once saved or opened. */
  path: string | null
}

const docs = new Map<number, Doc>()
let quitting = false

const PART_FILTER = { name: 'OpenForge Part', extensions: ['ofp'] }
const ASSEMBLY_FILTER = { name: 'OpenForge Assembly', extensions: ['ofa'] }
const DRAWING_FILTER = { name: 'OpenForge Drawing', extensions: ['ofd'] }
const DOCUMENT_FILTER = { name: 'OpenForge Document', extensions: ['ofp', 'ofa', 'ofd'] }
const MODEL_FILTER = { name: 'Part or Assembly', extensions: ['ofp', 'ofa'] }

function docOf(sender: WebContents): Doc | undefined {
  return docs.get(sender.id)
}

function startCore(doc: Doc): void {
  const path = findCore(app.getAppPath(), process.resourcesPath)
  const core = new CoreProcess(path)
  doc.core = core
  core.onExit = (code) => {
    if (doc.closing || quitting) return
    doc.assistant.stop()
    void doc.assistant.discard()
    doc.win.webContents.send('core:crashed', code)
    // Restart so the user can keep working (unsaved changes are lost).
    setTimeout(() => startCore(doc), 500)
  }
  core.start()
}

/** The window menu commands go to. */
function target(): BrowserWindow | null {
  return BrowserWindow.getFocusedWindow() ?? [...docs.values()][0]?.win ?? null
}

function send(command: string): void {
  target()?.webContents.send('menu', command)
}

function buildMenu(): void {
  const mac = process.platform === 'darwin'
  const item = (
    label: string,
    command: string,
    accelerator?: string,
  ): MenuItemConstructorOptions => ({
    label,
    accelerator,
    click: () => send(command),
  })
  const template: MenuItemConstructorOptions[] = [
    ...(mac ? [{ role: 'appMenu' as const }] : []),
    {
      label: 'File',
      submenu: [
        item('New Part', 'new', 'CmdOrCtrl+N'),
        item('New Assembly', 'new-assembly', 'CmdOrCtrl+Shift+N'),
        {
          label: 'New Drawing',
          click: () => createWindow({ kind: 'drawing' }),
        },
        item('Make Drawing from Part/Assembly', 'make-drawing'),
        {
          label: 'New Window',
          click: () => createWindow({}),
        },
        item('Open…', 'open', 'CmdOrCtrl+O'),
        item('Save', 'save', 'CmdOrCtrl+S'),
        item('Save As…', 'save-as', 'CmdOrCtrl+Shift+S'),
        { type: 'separator' },
        item('Import STEP/IGES…', 'import'),
        item('Import STEP as Assembly…', 'import-assembly'),
        item('Export…', 'export'),
        item('Print…', 'print', 'CmdOrCtrl+P'),
        { type: 'separator' },
        mac ? { role: 'close' } : { role: 'quit' },
      ],
    },
    {
      label: 'Edit',
      submenu: [
        item('Undo', 'undo', 'CmdOrCtrl+Z'),
        item('Redo', 'redo', mac ? 'Cmd+Shift+Z' : 'Ctrl+Y'),
        { type: 'separator' },
        { role: 'cut' },
        { role: 'copy' },
        { role: 'paste' },
        { role: 'selectAll' },
      ],
    },
    {
      label: 'View',
      submenu: [
        // Bare-key accelerators would swallow typing, so F is handled in the renderer.
        item('Zoom to Fit (F)', 'zoom-fit'),
        item('Front', 'view-front', 'CmdOrCtrl+1'),
        item('Back', 'view-back', 'CmdOrCtrl+2'),
        item('Left', 'view-left', 'CmdOrCtrl+3'),
        item('Right', 'view-right', 'CmdOrCtrl+4'),
        item('Top', 'view-top', 'CmdOrCtrl+5'),
        item('Bottom', 'view-bottom', 'CmdOrCtrl+6'),
        item('Isometric', 'view-iso', 'CmdOrCtrl+7'),
        item('Normal To', 'view-normal', 'CmdOrCtrl+8'),
        { type: 'separator' },
        { role: 'toggleDevTools' },
        { role: 'togglefullscreen' },
      ],
    },
    {
      label: 'Tools',
      submenu: [
        item('Units…', 'units'),
        item('Material…', 'material'),
        item('Mass Properties', 'mass'),
      ],
    },
    { role: 'windowMenu' },
  ]
  Menu.setApplicationMenu(Menu.buildFromTemplate(template))
}

/** Open a window for a document: a file, a new assembly or drawing (maybe of a model), or a new part. */
function createWindow(options: {
  open?: string
  kind?: 'part' | 'assembly' | 'drawing'
  model?: string
}): BrowserWindow {
  const win = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 900,
    minHeight: 600,
    title: 'OpenForge',
    backgroundColor: '#e7e9ec',
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
    },
  })
  const id = win.webContents.id
  const doc: Doc = {
    win,
    core: null,
    dirty: false,
    closing: false,
    path: options.open ?? null,
    assistant: new AssistantCoordinator(
      (method, params) => {
        if (!doc.core) return Promise.reject(new Error('The core is not running.'))
        return doc.core.call(method, params)
      },
      (event) => {
        if (event.type === 'committed') doc.dirty = true
        win.webContents.send('assistant:event', event)
      },
    ),
  }
  docs.set(id, doc)
  startCore(doc)

  win.on('close', (event) => {
    if (!doc.dirty || doc.closing || quitting) return
    event.preventDefault()
    const choice = dialog.showMessageBoxSync(win, {
      type: 'question',
      buttons: ['Save', "Don't Save", 'Cancel'],
      defaultId: 0,
      cancelId: 2,
      message: 'Save changes to this document before closing?',
    })
    if (choice === 0) win.webContents.send('menu', 'save-and-close')
    if (choice === 1) {
      doc.closing = true
      win.close()
    }
  })
  win.on('closed', () => {
    doc.closing = true
    doc.assistant.stop()
    doc.core?.stop()
    docs.delete(id)
  })

  const query: Record<string, string> = {}
  if (options.open) query['open'] = options.open
  if (options.kind) query['kind'] = options.kind
  if (options.model) query['model'] = options.model
  const devUrl = process.env['ELECTRON_RENDERER_URL']
  if (!app.isPackaged && devUrl) {
    const url = new URL(devUrl)
    for (const [k, v] of Object.entries(query)) url.searchParams.set(k, v)
    void win.loadURL(url.toString())
  } else {
    void win.loadFile(join(__dirname, '../renderer/index.html'), { query })
  }
  return win
}

ipcMain.handle('core:call', async (e, method: string, params: unknown) => {
  const doc = docOf(e.sender)
  if (!doc?.core) throw new Error('The core is not running.')
  try {
    const value = await doc.core.call(method, params)
    doc.assistant.changed(value as Parameters<typeof doc.assistant.changed>[0])
    return { ok: true, value }
  } catch (err) {
    // Errors cross the IPC boundary as values so the message survives intact.
    const error = err as CoreError
    return { ok: false, error: error.message, code: error.code ?? -1 }
  }
})

const assistantOf = (sender: WebContents) => {
  const doc = docOf(sender)
  if (!doc) throw new Error('This window has no document.')
  return doc.assistant
}

ipcMain.handle('assistant:settings', () => publicSettings())
ipcMain.handle('assistant:save-settings', (e, settings: AssistantSettings, key?: string) => {
  assistantOf(e.sender).assertIdle()
  return saveSettings(settings, key)
})
ipcMain.handle('assistant:test', (e) => assistantOf(e.sender).test())
ipcMain.handle('assistant:run', (e, request: AssistantRequest) =>
  assistantOf(e.sender).run(request),
)
ipcMain.handle('assistant:accept', (e, id: string) => assistantOf(e.sender).accept(id))
ipcMain.handle('assistant:discard', (e) => assistantOf(e.sender).discard())
ipcMain.handle('chatgpt:state', () => chatGPTState())
ipcMain.handle('chatgpt:models', () => chatGPT().models())
ipcMain.handle('chatgpt:sign-in', async (e, id?: string) => {
  assistantOf(e.sender).assertIdle()
  await chatGPT().signIn(id)
  return chatGPTState()
})
ipcMain.handle('chatgpt:select', (e, id: string) => {
  assistantOf(e.sender).assertIdle()
  chatGPT().select(id)
  return chatGPTState()
})
ipcMain.handle('chatgpt:sign-out', (e, id: string) => {
  assistantOf(e.sender).assertIdle()
  return chatGPT().signOut(id)
})
ipcMain.handle('chatgpt:usage', () => openChatGPTUsage())
ipcMain.on('chatgpt:cancel', () => chatGPT().cancel())
ipcMain.on('assistant:stop', (e) => assistantOf(e.sender).stop())

type OpenKind =
  | 'part'
  | 'document'
  | 'component'
  | 'import'
  | 'step'
  | 'model'
  | 'standard'
  | 'template'
  | 'folder'
ipcMain.handle('dialog:open', async (e, kind: OpenKind) => {
  if (kind === 'folder') {
    const win = BrowserWindow.fromWebContents(e.sender) ?? undefined
    const r = await dialog.showOpenDialog(win!, {
      properties: ['openDirectory', 'createDirectory'],
    })
    return r.canceled ? null : (r.filePaths[0] ?? null)
  }
  const filters =
    kind === 'part' || kind === 'document'
      ? [DOCUMENT_FILTER, PART_FILTER, ASSEMBLY_FILTER, DRAWING_FILTER]
      : kind === 'component' || kind === 'model'
        ? [MODEL_FILTER, PART_FILTER, ASSEMBLY_FILTER]
        : kind === 'standard'
          ? [{ name: 'Drafting Standard', extensions: ['ofstd'] }]
          : kind === 'template'
            ? [{ name: 'Sheet Format', extensions: ['ofdt'] }]
            : kind === 'step'
              ? [{ name: 'STEP', extensions: ['step', 'stp'] }]
              : [
                  {
                    name: 'Models, meshes and point clouds',
                    extensions: [
                      'step',
                      'stp',
                      'iges',
                      'igs',
                      'stl',
                      'obj',
                      'off',
                      'ply',
                      '3mf',
                      'amf',
                      'xyz',
                      'txt',
                      'asc',
                    ],
                  },
                  { name: 'STEP or IGES', extensions: ['step', 'stp', 'iges', 'igs'] },
                  { name: 'Meshes', extensions: ['stl', 'obj', 'off', 'ply', '3mf', 'amf'] },
                  { name: 'Point clouds', extensions: ['xyz', 'txt', 'asc'] },
                ]
  const win = BrowserWindow.fromWebContents(e.sender) ?? undefined
  const r = await dialog.showOpenDialog(win!, { properties: ['openFile'], filters })
  return r.canceled ? null : (r.filePaths[0] ?? null)
})

type SaveKind =
  'part' | 'assembly' | 'drawing' | 'export' | 'csv' | 'pdf' | 'dxf' | 'drawing-export' | 'standard'
ipcMain.handle('dialog:save', async (e, kind: SaveKind, defaultName: string) => {
  const filters =
    kind === 'part'
      ? [PART_FILTER]
      : kind === 'assembly'
        ? [ASSEMBLY_FILTER]
        : kind === 'drawing'
          ? [DRAWING_FILTER]
          : kind === 'csv'
            ? [{ name: 'CSV', extensions: ['csv'] }]
            : kind === 'pdf'
              ? [{ name: 'PDF', extensions: ['pdf'] }]
              : kind === 'dxf'
                ? [{ name: 'DXF', extensions: ['dxf'] }]
                : kind === 'drawing-export'
                  ? [
                      { name: 'PDF', extensions: ['pdf'] },
                      { name: 'DXF', extensions: ['dxf'] },
                    ]
                  : kind === 'standard'
                    ? [{ name: 'Drafting Standard', extensions: ['ofstd'] }]
                    : [
                        { name: 'STEP', extensions: ['step'] },
                        { name: 'STL', extensions: ['stl'] },
                        { name: 'IGES', extensions: ['iges'] },
                      ]
  const win = BrowserWindow.fromWebContents(e.sender) ?? undefined
  const r = await dialog.showSaveDialog(win!, { defaultPath: defaultName, filters })
  return r.canceled ? null : (r.filePath ?? null)
})

ipcMain.on('doc:state', (e, title: string, isDirty: boolean, path?: string | null) => {
  const doc = docOf(e.sender)
  if (!doc) return
  doc.dirty = isDirty
  if (path !== undefined) doc.path = path
  doc.win.setTitle(title)
  doc.win.setDocumentEdited(isDirty)
})

// A saved file may be a part other windows' assemblies use.
ipcMain.on('doc:saved', (e, path: string) => {
  for (const [id, doc] of docs) {
    if (id !== e.sender.id) doc.win.webContents.send('file:saved', path)
  }
})

ipcMain.on('window:open', (_e, path: string) => {
  // Bring an existing window for this file forward instead of opening it twice.
  const same = (a: string, b: string) =>
    process.platform === 'linux' ? a === b : a.toLowerCase() === b.toLowerCase()
  const open = [...docs.values()].find((d) => d.path && same(d.path, path))
  if (open) {
    open.win.show()
    open.win.focus()
    return
  }
  createWindow({ open: path })
})

ipcMain.on('window:drawing', (_e, model: string) => {
  createWindow({ kind: 'drawing', model })
})

// Printing goes through the drawing's PDF so paper size, scale and line
// weights match the export; the system viewer prints it.
ipcMain.handle('print:path', () =>
  join(tmpdir(), `openforge-print-${process.pid}-${Date.now()}.pdf`),
)
ipcMain.handle('print:open', async (_e, path: string) => {
  const error = await shell.openPath(path)
  if (error) throw new Error(error)
})

ipcMain.on('window:close', (e) => {
  const doc = docOf(e.sender)
  if (!doc) return
  doc.closing = true
  doc.win.close()
})

const primaryInstance = app.requestSingleInstanceLock()
if (!primaryInstance) app.quit()
app.on('second-instance', () => {
  const win = target()
  win?.show()
  win?.focus()
})
void app.whenReady().then(() => {
  if (!primaryInstance) return
  try {
    loadSettings()
    findCore(app.getAppPath(), process.resourcesPath)
  } catch (e) {
    dialog.showErrorBox('OpenForge', (e as Error).message)
    app.quit()
    return
  }
  buildMenu()
  createWindow({})
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow({})
  })
})

app.on('before-quit', () => {
  quitting = true
  cancelChatGPTSignIn()
  for (const doc of docs.values()) {
    doc.assistant.stop()
    doc.core?.stop()
  }
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
